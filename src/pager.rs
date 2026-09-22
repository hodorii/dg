//! crossterm 기반 페이저: 스크롤·검색·창 크기 대응.

use crate::line::Line;
use crate::markdown::{self, DiagramBlock, Document};
use crate::style::Theme;
use crate::text::char_width;
use crate::watch::{self, PollResult, Watcher};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::{cursor, event, execute, queue, terminal};
use std::io::{self, Write};

const MOUSE_WHEEL_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_WHEEL_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// 화면에 나오는 다이어그램 블록의 줄 범위(원문을 펼쳤으면 그것까지).
struct ShownBlock {
    start: usize,
    end: usize,
    index: usize,
}

pub struct Pager<'a, F: Fn(&str, usize, usize) -> Document> {
    title: String,
    theme: &'a Theme,
    max_width: usize,
    render: F,
    /// 현재 그릴 원문. 감시 모드에서는 파일이 바뀔 때마다 갱신된다.
    source: String,
    watcher: Option<Watcher>,
    /// 감시 중 마지막으로 겪은 읽기 오류(있으면 상태 표시줄에 보여줌).
    watch_error: Option<String>,
    document: Document,
    /// 다이어그램마다 원문을 펼쳤는지.
    expanded: Vec<bool>,
    /// 실제로 보여 주는 줄(펼친 원문 포함).
    lines: Vec<Line>,
    shown_blocks: Vec<ShownBlock>,
    rendered_width: usize,
    rendered_columns: usize,
    top: usize,
    query: String,
    typing: bool,
    matches: Vec<usize>,
    message: String,
}

impl<'a, F: Fn(&str, usize, usize) -> Document> Pager<'a, F> {
    pub fn new(title: &str, theme: &'a Theme, max_width: usize, source: String, render: F, watcher: Option<Watcher>) -> Self {
        Pager {
            title: title.to_string(),
            theme,
            max_width,
            render,
            source,
            watcher,
            watch_error: None,
            document: Document { lines: Vec::new(), diagrams: Vec::new() },
            expanded: Vec::new(),
            lines: Vec::new(),
            shown_blocks: Vec::new(),
            rendered_width: 0,
            rendered_columns: 0,
            top: 0,
            query: String::new(),
            typing: false,
            matches: Vec::new(),
            message: String::new(),
        }
    }

    pub fn run(mut self) -> io::Result<()> {
        let mut out = io::stdout();
        terminal::enable_raw_mode()?;
        // 휠만 필요하므로 버튼 이벤트(1000)+SGR(1006)만 켠다. 이동 추적(1003)을 켜면
        // 마우스가 움직일 때마다 이벤트가 쏟아져 화면이 깜빡인다.
        execute!(out, terminal::EnterAlternateScreen, crossterm::style::Print(MOUSE_WHEEL_ON), cursor::Hide)?;
        let result = self.event_loop(&mut out);
        execute!(out, cursor::Show, crossterm::style::Print(MOUSE_WHEEL_OFF), terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        result
    }

    fn event_loop(&mut self, out: &mut io::Stdout) -> io::Result<()> {
        let mut needs_redraw = true;
        loop {
            let (columns, rows) = terminal::size()?;
            let (columns, rows) = (columns as usize, rows as usize);
            // 문단은 읽기 좋은 폭까지만 접고, 다이어그램·표·코드는 터미널 폭을 다 쓴다.
            let width = columns.min(self.max_width).max(10);

            let mut force_render = false;
            if let Some(watcher) = &mut self.watcher {
                let result = watcher.poll();
                force_render = self.apply_poll_result(result);
            }
            if width != self.rendered_width || columns != self.rendered_columns || force_render {
                self.rerender(width, columns);
                needs_redraw = true;
            }
            let page = rows.saturating_sub(1).max(1);
            self.clamp(page);
            if needs_redraw {
                self.draw(out, columns, rows)?;
            }
            // 감시 중이 아니면 기존과 동일하게 블로킹 대기. 감시 중이면 짧은 타임아웃으로 대기해
            // 키 입력엔 즉시 반응하면서 다음 루프에서 다시 파일을 확인한다.
            needs_redraw = if self.watcher.is_some() {
                if event::poll(watch::POLL_INTERVAL)? {
                    match self.handle_event(event::read()?, page) {
                        Some(redraw) => redraw,
                        None => return Ok(()),
                    }
                } else {
                    false
                }
            } else {
                match self.handle_event(event::read()?, page) {
                    Some(redraw) => redraw,
                    None => return Ok(()),
                }
            };
        }
    }

    /// 크로스텀 이벤트 하나를 처리한다. 종료해야 하면 `None`, 계속하면 다시 그려야 하는지를
    /// `Some(bool)`로 돌려준다. 마우스 이동·버튼 등 상태가 안 바뀌는 이벤트는 무시한다.
    fn handle_event(&mut self, event: Event, page: usize) -> Option<bool> {
        match event {
            Event::Key(key) => {
                if self.handle_key(key, page) {
                    return None;
                }
                Some(true)
            }
            Event::Mouse(mouse) => Some(match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.top += 3;
                    true
                }
                MouseEventKind::ScrollUp => {
                    self.top = self.top.saturating_sub(3);
                    true
                }
                MouseEventKind::Down(MouseButton::Left) => self.toggle_block_at(self.top + mouse.row as usize),
                _ => false,
            }),
            Event::Resize(_, _) => Some(true),
            _ => Some(false),
        }
    }

    /// 현재 소스를 주어진 폭·칸으로 다시 그린다(폭 변경·감시 갱신·수동 갱신에서 공유).
    fn rerender(&mut self, width: usize, columns: usize) {
        self.document = (self.render)(&self.source, width, columns.max(10));
        self.expanded.resize(self.document.diagrams.len(), false);
        self.rendered_width = width;
        self.rendered_columns = columns;
        self.rebuild_lines();
    }

    /// `r` 키: mtime과 무관하게 즉시 다시 읽는다. 감시 중이 아니면 아무 것도 하지 않는다.
    fn manual_refresh(&mut self) {
        let Some(watcher) = self.watcher.as_mut() else { return };
        let result = watcher.poll_forced();
        if self.apply_poll_result(result) {
            let (width, columns) = (self.rendered_width, self.rendered_columns);
            self.rerender(width, columns);
        }
    }

    /// 감시 폴 결과를 `source`/`watch_error`에 반영한다. 다시 그려야 하면(내용이 바뀌었으면) true.
    fn apply_poll_result(&mut self, result: PollResult) -> bool {
        match result {
            PollResult::Changed(new_source) => {
                self.source = new_source;
                self.watch_error = None;
                true
            }
            PollResult::ReadError(message) => {
                self.watch_error = Some(message);
                false
            }
            PollResult::Unchanged => false,
        }
    }

    fn clamp(&mut self, page: usize) {
        let max_top = self.lines.len().saturating_sub(page);
        self.top = self.top.min(max_top);
    }

    /// 종료하면 true.
    fn handle_key(&mut self, key: KeyEvent, page: usize) -> bool {
        if self.typing {
            match key.code {
                KeyCode::Esc => {
                    self.typing = false;
                    self.query.clear();
                    self.refresh_matches();
                }
                KeyCode::Enter => {
                    self.typing = false;
                    self.refresh_matches();
                    self.jump_to_match(true, page);
                }
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => self.query.push(c),
                _ => {}
            }
            return false;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if control => return true,
            KeyCode::Esc => {
                if self.query.is_empty() {
                    return true;
                }
                self.query.clear();
                self.refresh_matches();
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => self.top += 1,
            KeyCode::Char('k') | KeyCode::Up => self.top = self.top.saturating_sub(1),
            KeyCode::Char('d') if control => self.top += page / 2,
            KeyCode::Char('u') if control => self.top = self.top.saturating_sub(page / 2),
            KeyCode::Char('f') if control => self.top += page,
            KeyCode::Char('b') if control => self.top = self.top.saturating_sub(page),
            KeyCode::Char(' ') | KeyCode::PageDown => self.top += page,
            KeyCode::PageUp => self.top = self.top.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.top = 0,
            KeyCode::Char('G') | KeyCode::End => self.top = usize::MAX,
            KeyCode::Char('/') => {
                self.typing = true;
                self.query.clear();
                self.message.clear();
            }
            KeyCode::Char('n') => self.jump_to_match(true, page),
            KeyCode::Char('N') => self.jump_to_match(false, page),
            KeyCode::Char('o') => {
                // 화면에 보이는 첫 다이어그램의 원문을 펼치거나 접는다.
                let visible = self.top..self.top + page;
                if let Some(block) = self.shown_blocks.iter().find(|b| b.start < visible.end && visible.start < b.end) {
                    let start = block.start;
                    self.toggle_block_at(start);
                }
            }
            // 감시 중이 아니면 아무 일도 하지 않는다.
            KeyCode::Char('r') => self.manual_refresh(),
            _ => {}
        }
        false
    }

    /// 표시 줄 번호가 다이어그램 블록 안이면 그 블록의 원문을 펼치거나 접는다. 바뀌면 true.
    fn toggle_block_at(&mut self, line: usize) -> bool {
        let Some(block) = self.shown_blocks.iter().find(|b| b.start <= line && line < b.end) else { return false };
        let index = block.index;
        self.expanded[index] = !self.expanded[index];
        self.rebuild_lines();
        true
    }

    /// 문서 줄에 펼친 원문을 끼워 넣어 표시 줄을 만든다.
    fn rebuild_lines(&mut self) {
        let mut lines: Vec<Line> = Vec::with_capacity(self.document.lines.len());
        let mut shown = Vec::new();
        let mut cursor = 0;
        for (index, block) in self.document.diagrams.iter().enumerate() {
            lines.extend(self.document.lines[cursor..block.start].iter().cloned());
            let start = lines.len();
            let expanded = self.expanded[index];
            lines.push(self.document.lines[block.start].clone());
            // 원문은 캡션 줄 바로 아래 펼친다(그린 다이어그램 뒤가 아니라).
            if expanded {
                lines.extend(self.source_lines(block));
            }
            lines.extend(self.document.lines[block.start + 1..block.end].iter().cloned());
            shown.push(ShownBlock { start, end: lines.len(), index });
            cursor = block.end;
        }
        lines.extend(self.document.lines[cursor..].iter().cloned());
        self.lines = lines;
        self.shown_blocks = shown;
        self.refresh_matches();
    }

    fn source_lines(&self, block: &DiagramBlock) -> Vec<Line> {
        let mut lines = markdown::render_source_block(&block.lang, &block.source, self.theme, self.rendered_columns.max(10));
        lines.insert(0, Line::empty());
        lines
    }

    fn refresh_matches(&mut self) {
        let needle = self.query.to_lowercase();
        self.matches = if needle.is_empty() {
            Vec::new()
        } else {
            self.lines.iter().enumerate().filter(|(_, l)| l.text().to_lowercase().contains(&needle)).map(|(i, _)| i).collect()
        };
    }

    fn jump_to_match(&mut self, forward: bool, page: usize) {
        if self.matches.is_empty() {
            if !self.query.is_empty() {
                self.message = format!("'{}' 없음", self.query);
            }
            return;
        }
        let target = if forward {
            self.matches.iter().copied().find(|&i| i > self.top).or_else(|| self.matches.first().copied())
        } else {
            self.matches.iter().rev().copied().find(|&i| i < self.top).or_else(|| self.matches.last().copied())
        };
        if let Some(line) = target {
            self.top = line;
            self.clamp(page);
            let position = self.matches.iter().position(|&i| i == line).unwrap_or(0) + 1;
            self.message = format!("{}/{}", position, self.matches.len());
        }
    }

    fn draw(&self, out: &mut io::Stdout, columns: usize, rows: usize) -> io::Result<()> {
        // 동기화 갱신 안에서 줄을 쓴 뒤 남은 부분만 지워 깜빡임을 줄인다.
        queue!(out, terminal::BeginSynchronizedUpdate, cursor::MoveTo(0, 0))?;
        let page = rows.saturating_sub(1);
        for row in 0..page {
            queue!(out, cursor::MoveTo(0, row as u16))?;
            if let Some(line) = self.lines.get(self.top + row) {
                let line = if self.query.is_empty() { line.clone() } else { line.highlight(&self.query, self.theme.search_hit) };
                let truncated = truncate_line(&line, columns);
                queue!(out, crossterm::style::Print(truncated.to_ansi(self.theme)))?;
            }
            queue!(out, terminal::Clear(terminal::ClearType::UntilNewLine))?;
        }
        queue!(out, cursor::MoveTo(0, page as u16))?;
        let status = self.status(columns);
        queue!(out, crossterm::style::Print(status.to_ansi(self.theme)), terminal::EndSynchronizedUpdate)?;
        out.flush()
    }

    fn status(&self, columns: usize) -> Line {
        let text = if self.typing {
            format!(" /{}", self.query)
        } else {
            let percent = if self.lines.is_empty() {
                100
            } else {
                ((self.top + columns.min(1)).min(self.lines.len()) * 100 / self.lines.len().max(1)).min(100)
            };
            let extra = if self.message.is_empty() { String::new() } else { format!("  {}", self.message) };
            let hint = if self.shown_blocks.is_empty() { "" } else { "  o/클릭 원문" };
            let watch = match (&self.watcher, &self.watch_error) {
                (Some(_), Some(message)) => format!("  감시 불가: {message}"),
                (Some(_), None) => "  감시 중".to_string(),
                (None, _) => String::new(),
            };
            format!(" {}  {}%{}{}  ·  j/k 이동  / 검색{}  q 종료", self.title, percent, extra, watch, hint)
        };
        let mut padded = text;
        let mut used = crate::text::width_of(&padded);
        while used < columns {
            padded.push(' ');
            used += 1;
        }
        truncate_line(&Line::single(padded, self.theme.status), columns)
    }
}

/// 터미널 폭을 넘는 부분을 잘라낸다.
fn truncate_line(line: &Line, columns: usize) -> Line {
    if line.width() <= columns {
        return line.clone();
    }
    let mut out = Line::empty();
    let mut used = 0;
    for (text, style) in line.runs() {
        let mut end = 0;
        for c in text.chars() {
            let w = char_width(c);
            if used + w > columns {
                out.push_str(&text[..end], style);
                return out;
            }
            end += c.len_utf8();
            used += w;
        }
        out.push_str(text, style);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    /// 캡션 1줄 + 그린 다이어그램 2줄짜리 문서 하나를 만든다.
    fn fixture() -> (Document, DiagramBlock) {
        let block = DiagramBlock { start: 1, end: 4, lang: "mermaid".to_string(), source: "flowchart TB\n A --> B".to_string() };
        let lines = vec![
            Line::single("본문", Style::PLAIN),
            Line::single("◈ mermaid · flowchart ─", Style::PLAIN),
            Line::single("그림 줄 1", Style::PLAIN),
            Line::single("그림 줄 2", Style::PLAIN),
            Line::single("이후 본문", Style::PLAIN),
        ];
        (Document { lines, diagrams: vec![block.clone()] }, block)
    }

    fn pager(document: Document, theme: &Theme) -> Pager<'_, fn(&str, usize, usize) -> Document> {
        let render = (|_: &str, _, _| Document { lines: Vec::new(), diagrams: Vec::new() }) as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", theme, 80, String::new(), render, None);
        pager.expanded = vec![false; document.diagrams.len()];
        pager.document = document;
        pager.rendered_columns = 80;
        pager.rebuild_lines();
        pager
    }

    /// 캡션 줄을 클릭(토글)하면 원문이 다이어그램 뒤가 아니라 캡션 바로 아래 펼쳐진다.
    #[test]
    fn expanding_a_block_inserts_source_right_after_its_caption_not_after_its_body() {
        let theme = Theme::none();
        let (document, block) = fixture();
        let mut pager = pager(document, &theme);
        assert!(pager.toggle_block_at(block.start));

        let plain: Vec<String> = pager.lines.iter().map(Line::plain).collect();
        let caption_row = plain.iter().position(|line| line.contains('◈')).expect("캡션이 있어야 한다");
        // 캡션 바로 다음 줄부터 원문(flowchart TB 등)이 나오고, 그린 다이어그램 줄("그림 줄")은 원문 뒤에 와야 한다.
        let source_row = plain.iter().position(|line| line.contains("flowchart TB")).expect("펼친 원문이 있어야 한다");
        let body_row = plain.iter().position(|line| line.contains("그림 줄 1")).expect("그린 다이어그램 줄이 남아 있어야 한다");
        assert!(source_row > caption_row, "원문은 캡션 뒤에 와야 한다");
        assert!(source_row < body_row, "원문은 그린 다이어그램보다 앞, 즉 캡션 바로 아래여야 한다");

        assert!(pager.toggle_block_at(block.start));
        let collapsed: Vec<String> = pager.lines.iter().map(Line::plain).collect();
        assert!(!collapsed.iter().any(|line| line.contains("flowchart TB")), "접으면 원문이 사라져야 한다");
    }

    struct TempFile {
        path: std::path::PathBuf,
    }

    impl TempFile {
        fn with_content(content: &str) -> TempFile {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("dg-pager-watch-test-{}-{n}", std::process::id()));
            std::fs::write(&path, content).unwrap();
            TempFile { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// 2.1/5.1 `Changed`는 소스를 갱신하고 오류를 지우며 다시 그리라고 알린다. `ReadError`는
    /// 오류만 기록하고 마지막 성공 소스는 그대로 둔다. `Unchanged`는 아무 것도 건드리지 않는다.
    #[test]
    fn apply_poll_result_updates_source_and_error_state() {
        let theme = Theme::none();
        let mut pager = pager(Document { lines: Vec::new(), diagrams: Vec::new() }, &theme);

        assert!(pager.apply_poll_result(PollResult::Changed("새 내용".to_string())));
        assert_eq!(pager.source, "새 내용");
        assert!(pager.watch_error.is_none());

        assert!(!pager.apply_poll_result(PollResult::ReadError("파일 없음".to_string())));
        assert_eq!(pager.watch_error.as_deref(), Some("파일 없음"));
        assert_eq!(pager.source, "새 내용", "오류 중에도 마지막 성공 소스를 유지해야 한다");

        assert!(!pager.apply_poll_result(PollResult::Unchanged));
        assert_eq!(pager.watch_error.as_deref(), Some("파일 없음"), "Unchanged는 오류 상태를 건드리지 않는다");
    }

    /// 3.1 감시로 내용이 갱신돼도 스크롤 위치(top)는 그대로 유지된다.
    #[test]
    fn watch_update_preserves_scroll_position() {
        let theme = Theme::none();
        let render = (|source: &str, _: usize, _: usize| Document {
            lines: (0..10).map(|i| Line::single(format!("{source}-{i}"), Style::PLAIN)).collect(),
            diagrams: Vec::new(),
        }) as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", &theme, 80, "a".to_string(), render, None);
        pager.rerender(80, 80);
        pager.top = 4;

        assert!(pager.apply_poll_result(PollResult::Changed("b".to_string())));
        pager.rerender(80, 80);

        assert_eq!(pager.top, 4, "감시로 갱신돼도 스크롤 위치는 그대로 유지돼야 한다");
        assert!(pager.lines[0].plain().contains("b-0"));
    }

    /// 3.4 상태 표시줄은 감시 중이 아니면 아무 표시가 없고, 감시 중이면 "감시 중", 오류가 있으면
    /// "감시 불가: …"를 보여준다.
    #[test]
    fn status_line_shows_watch_state() {
        let theme = Theme::none();
        let mut pager = pager(Document { lines: Vec::new(), diagrams: Vec::new() }, &theme);
        assert!(!pager.status(80).plain().contains("감시"), "감시 중이 아니면 표시가 없어야 한다");

        pager.watcher = Some(Watcher::new("/dg-watch-mode-test-does-not-exist"));
        assert!(pager.status(80).plain().contains("감시 중"));

        pager.watch_error = Some("test: 없음".to_string());
        let status = pager.status(80).plain();
        assert!(status.contains("감시 불가"), "{status}");
        assert!(status.contains("test: 없음"), "{status}");
    }

    /// 3.5 `r` 키는 mtime과 무관하게 즉시 다시 읽고(`poll_forced`), 감시 중이 아니면 아무 일도
    /// 하지 않는다.
    #[test]
    fn r_key_forces_reread_through_handle_key() {
        let theme = Theme::none();
        let temp = TempFile::with_content("원본");
        let render = (|source: &str, _: usize, _: usize| Document { lines: vec![Line::single(source.to_string(), Style::PLAIN)], diagrams: Vec::new() })
            as fn(&str, usize, usize) -> Document;

        // 감시 중이 아니면 'r'은 아무 효과가 없다(6.1 회귀 없음).
        let mut not_watching = Pager::new("t", &theme, 80, "원본".to_string(), render, None);
        not_watching.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), 10);
        assert_eq!(not_watching.source, "원본");

        let mut pager = Pager::new("t", &theme, 80, "원본".to_string(), render, Some(Watcher::new(&temp.path)));
        pager.rerender(80, 80);

        std::fs::write(&temp.path, "바뀐 내용").unwrap();
        pager.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), 10);
        assert_eq!(pager.source, "바뀐 내용");
        assert!(pager.lines.iter().any(|l| l.plain().contains("바뀐 내용")));
    }
}
