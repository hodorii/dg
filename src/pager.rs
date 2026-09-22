//! crossterm 기반 페이저: 스크롤·검색·창 크기 대응.

use crate::line::Line;
use crate::links::{self, HistoryEntry, LinkKind, LinkPosition};
use crate::markdown::{self, DiagramBlock, Document};
use crate::style::{Style, Theme};
use crate::text::char_width;
use crate::watch::{self, PollResult, Watcher};
use std::path::PathBuf;
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
    /// (슬러그, 최종 표시 줄 번호) — 다이어그램 펼침에 따른 줄 밀림이 보정된 값.
    heading_lines: Vec<(String, usize)>,
    /// 화면에 그려진 링크들의 위치(밑줄 스캔, `rebuild_lines`마다 갱신).
    link_positions: Vec<LinkPosition>,
    /// 키보드로 포커스된 링크(`link_positions`의 인덱스).
    focused_link: Option<usize>,
    /// 지금 보고 있는 파일의 경로(표준입력이면 `None` — 상대 링크·히스토리 둘 다 못 씀).
    current_path: Option<PathBuf>,
    history: links::History,
    rendered_width: usize,
    rendered_columns: usize,
    top: usize,
    query: String,
    typing: bool,
    matches: Vec<usize>,
    message: String,
}

impl<'a, F: Fn(&str, usize, usize) -> Document> Pager<'a, F> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: &str,
        theme: &'a Theme,
        max_width: usize,
        source: String,
        render: F,
        watcher: Option<Watcher>,
        current_path: Option<PathBuf>,
    ) -> Self {
        Pager {
            title: title.to_string(),
            theme,
            max_width,
            render,
            source,
            watcher,
            watch_error: None,
            document: Document::default(),
            expanded: Vec::new(),
            lines: Vec::new(),
            shown_blocks: Vec::new(),
            heading_lines: Vec::new(),
            link_positions: Vec::new(),
            focused_link: None,
            current_path,
            history: links::History::default(),
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
                MouseEventKind::Down(MouseButton::Left) => self.click_at(self.top + mouse.row as usize, mouse.column as usize, page),
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
            KeyCode::Char('j') | KeyCode::Down => self.top += 1,
            KeyCode::Enter => {
                if self.focused_link.is_some() {
                    self.follow_focused_link(page);
                } else {
                    self.top += 1;
                }
            }
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
            KeyCode::Tab => self.focus_next_link(true, page),
            KeyCode::BackTab => self.focus_next_link(false, page),
            KeyCode::Char('[') => self.go_back(),
            KeyCode::Char(']') => self.go_forward(),
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

    /// 클릭 좌표가 링크 위면 그 링크를 따라가고, 아니면 기존처럼 다이어그램 원문을 토글한다.
    /// 둘 다 아니면 무동작(false).
    fn click_at(&mut self, line: usize, col: usize, page: usize) -> bool {
        let hit = self.link_positions.iter().find(|p| p.line == line && col >= p.col_start && col < p.col_end);
        if let Some(position) = hit {
            let link_index = position.link_index;
            if let Some(dest) = self.document.links.get(link_index).cloned() {
                self.follow_link(&dest, page);
            }
            return true;
        }
        self.toggle_block_at(line)
    }

    /// 현재 뷰포트(`top..top+page`) 안의 링크 사이에서 포커스를 옮긴다. 보이는 링크가 없으면
    /// 무동작.
    fn focus_next_link(&mut self, forward: bool, page: usize) {
        let visible: Vec<usize> =
            self.link_positions.iter().enumerate().filter(|(_, p)| p.line >= self.top && p.line < self.top + page).map(|(i, _)| i).collect();
        if visible.is_empty() {
            return;
        }
        let current = self.focused_link.and_then(|idx| visible.iter().position(|&i| i == idx));
        let next = match current {
            Some(pos) if forward => (pos + 1) % visible.len(),
            Some(pos) => (pos + visible.len() - 1) % visible.len(),
            None => 0,
        };
        self.focused_link = Some(visible[next]);
    }

    /// 포커스된 링크를 따라가고 포커스를 지운다. 포커스가 없으면 무동작.
    fn follow_focused_link(&mut self, page: usize) {
        let Some(index) = self.focused_link.take() else { return };
        let Some(position) = self.link_positions.get(index) else { return };
        let link_index = position.link_index;
        let Some(dest) = self.document.links.get(link_index).cloned() else { return };
        self.follow_link(&dest, page);
    }

    /// 링크 목적지를 종류별로 따라간다: 앵커 점프·외부 브라우저 열기·다른 파일로 이동.
    fn follow_link(&mut self, dest: &str, page: usize) {
        match links::classify(dest) {
            LinkKind::Anchor(slug) => match self.heading_lines.iter().find(|(s, _)| *s == slug) {
                Some((_, line)) => {
                    self.top = *line;
                    self.clamp(page);
                    self.message.clear();
                }
                None => self.message = format!("'{slug}' 헤딩을 찾을 수 없습니다"),
            },
            LinkKind::External(url) => match links::open_external(&url) {
                Ok(()) => self.message.clear(),
                Err(error) => self.message = format!("열기 실패: {error}"),
            },
            LinkKind::File(path) => self.follow_file_link(&path),
        }
    }

    /// 다른 마크다운 파일로 이동한다. 표준입력으로 열었으면(`current_path`가 없으면) 상대
    /// 경로를 풀 기준이 없어 조용히 거부한다.
    fn follow_file_link(&mut self, relative: &str) {
        let Some(current_path) = self.current_path.clone() else {
            self.message = "표준입력에서는 파일 이동을 할 수 없습니다".to_string();
            return;
        };
        let base_dir = current_path.parent().unwrap_or(std::path::Path::new(""));
        let target = links::resolve_file_path(base_dir, relative);
        match std::fs::read_to_string(&target) {
            Ok(source) => {
                self.history.record(HistoryEntry { path: current_path, top: self.top });
                self.load_path(target, source);
            }
            Err(error) => self.message = format!("{}: {error}", target.display()),
        }
    }

    /// 뒤로/앞으로 히스토리로 이전/다음 파일·위치를 복원한다. 히스토리가 없거나 표준입력이면
    /// 무동작.
    fn go_back(&mut self) {
        let Some(current_path) = self.current_path.clone() else { return };
        let current = HistoryEntry { path: current_path, top: self.top };
        if let Some(entry) = self.history.go_back(current) {
            self.load_history_entry(entry);
        }
    }

    fn go_forward(&mut self) {
        let Some(current_path) = self.current_path.clone() else { return };
        let current = HistoryEntry { path: current_path, top: self.top };
        if let Some(entry) = self.history.go_forward(current) {
            self.load_history_entry(entry);
        }
    }

    fn load_history_entry(&mut self, entry: HistoryEntry) {
        let top = entry.top;
        match std::fs::read_to_string(&entry.path) {
            Ok(source) => {
                let path = entry.path;
                self.load_path(path, source);
                self.top = top;
            }
            Err(error) => self.message = format!("{}: {error}", entry.path.display()),
        }
    }

    /// 새 파일 내용으로 전환한다: `source`/`title`/`current_path`를 갱신하고 감시 중이면
    /// `Watcher`도 새 경로로 다시 만들고 나서 다시 그린다.
    fn load_path(&mut self, path: PathBuf, source: String) {
        self.title = path.display().to_string();
        if self.watcher.is_some() {
            self.watcher = Some(Watcher::new(&path));
        }
        self.watch_error = None;
        self.current_path = Some(path);
        self.source = source;
        self.top = 0;
        self.message.clear();
        let (width, columns) = (self.rendered_width, self.rendered_columns);
        self.rerender(width, columns);
    }

    /// 문서 줄에 펼친 원문을 끼워 넣어 표시 줄을 만든다.
    fn rebuild_lines(&mut self) {
        let mut lines: Vec<Line> = Vec::with_capacity(self.document.lines.len());
        let mut shown = Vec::new();
        // (원본 block.end, 그 시점까지 펼침으로 늘어난 누적 줄 수) — 헤딩 위치 보정에 쓴다.
        let mut offsets: Vec<(usize, usize)> = Vec::new();
        let mut cumulative_offset = 0usize;
        let mut cursor = 0;
        for (index, block) in self.document.diagrams.iter().enumerate() {
            lines.extend(self.document.lines[cursor..block.start].iter().cloned());
            let start = lines.len();
            let expanded = self.expanded[index];
            lines.push(self.document.lines[block.start].clone());
            // 원문은 캡션 줄 바로 아래 펼친다(그린 다이어그램 뒤가 아니라).
            if expanded {
                let source_lines = self.source_lines(block);
                cumulative_offset += source_lines.len();
                lines.extend(source_lines);
            }
            lines.extend(self.document.lines[block.start + 1..block.end].iter().cloned());
            shown.push(ShownBlock { start, end: lines.len(), index });
            offsets.push((block.end, cumulative_offset));
            cursor = block.end;
        }
        lines.extend(self.document.lines[cursor..].iter().cloned());
        self.lines = lines;
        self.shown_blocks = shown;
        self.heading_lines = self
            .document
            .headings
            .iter()
            .map(|(slug, original)| {
                let offset = offsets.iter().rev().find(|(end, _)| *end <= *original).map(|(_, o)| *o).unwrap_or(0);
                (slug.clone(), original + offset)
            })
            .collect();
        self.link_positions = links::locate_links(&self.lines);
        self.focused_link = None;
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
        let focused_position = self.focused_link.and_then(|index| self.link_positions.get(index));
        for row in 0..page {
            queue!(out, cursor::MoveTo(0, row as u16))?;
            let absolute = self.top + row;
            if let Some(line) = self.lines.get(absolute) {
                let mut line = if self.query.is_empty() { line.clone() } else { line.highlight(&self.query, self.theme.search_hit) };
                if let Some(position) = focused_position
                    && position.line == absolute
                {
                    line = line.highlight_span(position.col_start, position.col_end, Style::PLAIN.reverse());
                }
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
            let link_hint = if self.link_positions.is_empty() { "" } else { "  Tab 링크  [/] 이동기록" };
            let watch = match (&self.watcher, &self.watch_error) {
                (Some(_), Some(message)) => format!("  감시 불가: {message}"),
                (Some(_), None) => "  감시 중".to_string(),
                (None, _) => String::new(),
            };
            format!(" {}  {}%{}{}  ·  j/k 이동  / 검색{}{}  q 종료", self.title, percent, extra, watch, hint, link_hint)
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
        (Document { lines, diagrams: vec![block.clone()], ..Document::default() }, block)
    }

    fn pager(document: Document, theme: &Theme) -> Pager<'_, fn(&str, usize, usize) -> Document> {
        let render = (|_: &str, _, _| Document::default()) as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", theme, 80, String::new(), render, None, None);
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
        let mut pager = pager(Document::default(), &theme);

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
            ..Document::default()
        }) as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", &theme, 80, "a".to_string(), render, None, None);
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
        let mut pager = pager(Document::default(), &theme);
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
        let render = (|source: &str, _: usize, _: usize| Document { lines: vec![Line::single(source.to_string(), Style::PLAIN)], ..Document::default() })
            as fn(&str, usize, usize) -> Document;

        // 감시 중이 아니면 'r'은 아무 효과가 없다(6.1 회귀 없음).
        let mut not_watching = Pager::new("t", &theme, 80, "원본".to_string(), render, None, None);
        not_watching.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), 10);
        assert_eq!(not_watching.source, "원본");

        let mut pager = Pager::new("t", &theme, 80, "원본".to_string(), render, Some(Watcher::new(&temp.path)), None);
        pager.rerender(80, 80);

        std::fs::write(&temp.path, "바뀐 내용").unwrap();
        pager.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), 10);
        assert_eq!(pager.source, "바뀐 내용");
        assert!(pager.lines.iter().any(|l| l.plain().contains("바뀐 내용")));
    }

    /// 4.1 다이어그램 원문을 펼치면(`o`/클릭 토글) 헤딩·링크 위치가 밀린 줄만큼 함께 보정된다.
    #[test]
    fn heading_and_link_positions_stay_correct_after_expanding_a_diagram() {
        let theme = Theme::none();
        let block = DiagramBlock { start: 1, end: 3, lang: "mermaid".to_string(), source: "flowchart TB\n A --> B".to_string() };
        let mut link_line = Line::empty();
        link_line.push_str("링크", Style::PLAIN.underline());
        let lines = vec![
            Line::single("본문", Style::PLAIN),
            Line::single("◈ mermaid", Style::PLAIN),
            Line::single("그림", Style::PLAIN),
            Line::single("헤딩 줄", Style::PLAIN),
            link_line,
        ];
        let document = Document { lines, diagrams: vec![block], links: vec!["#target".into()], headings: vec![("target".into(), 3)] };
        let mut pager = pager(document, &theme);
        assert_eq!(pager.heading_lines, vec![("target".to_string(), 3)]);
        let link_line_before = pager.link_positions[0].line;
        assert_eq!(link_line_before, 4);

        assert!(pager.toggle_block_at(1));
        let pushed = pager.heading_lines[0].1;
        assert!(pushed > 3, "다이어그램을 펼치면 헤딩 줄 번호도 밀려야 한다");
        assert_eq!(pager.link_positions[0].line, link_line_before + (pushed - 3), "링크 위치도 같은 만큼 밀려야 한다");
    }

    /// 5.1 Tab/Shift-Tab은 현재 뷰포트 안의 링크 사이에서만 순환하고, 뷰포트에 링크가 없으면
    /// 무동작이다.
    #[test]
    fn tab_focuses_only_links_visible_in_current_viewport() {
        let theme = Theme::none();
        let mut lines = Vec::new();
        for i in 0..20 {
            let mut line = Line::empty();
            if i == 0 {
                line.push_str("첫 링크", Style::PLAIN.underline());
            } else if i == 15 {
                line.push_str("둘째 링크", Style::PLAIN.underline());
            } else {
                line.push_str(&format!("줄 {i}"), Style::PLAIN);
            }
            lines.push(line);
        }
        let document = Document { lines, links: vec!["#a".into(), "#b".into()], ..Document::default() };
        let mut pager = pager(document, &theme);
        pager.top = 0;

        // 뷰포트(0..10)엔 첫 링크만 보인다 — 둘째 링크(줄 15)는 제외된다.
        pager.focus_next_link(true, 10);
        assert_eq!(pager.focused_link, Some(0));
        pager.focus_next_link(true, 10);
        assert_eq!(pager.focused_link, Some(0), "뷰포트 안엔 링크가 하나뿐이라 그대로 순환한다");
    }

    #[test]
    fn tab_is_noop_when_viewport_has_no_links() {
        let theme = Theme::none();
        let document = Document { lines: vec![Line::single("링크 없는 줄", Style::PLAIN)], ..Document::default() };
        let mut pager = pager(document, &theme);
        pager.focus_next_link(true, 10);
        assert_eq!(pager.focused_link, None);
    }

    /// 5.2 Enter는 포커스된 링크가 있으면 따라가고 포커스를 지우며, 없으면 기존처럼 한 줄
    /// 스크롤한다(회귀 5.1).
    #[test]
    fn enter_follows_focused_link_then_clears_focus_and_otherwise_scrolls() {
        let theme = Theme::none();
        // 문서가 뷰포트(page)보다 길어야 앵커 점프가 실제로 스크롤을 일으킨다 — 전부 한 화면에
        // 들어오면 클램프가 top을 다시 0으로 되돌려 점프 여부를 구별할 수 없다.
        let mut link_line = Line::empty();
        link_line.push_str("헤딩보기", Style::PLAIN.underline());
        let mut lines = vec![link_line];
        lines.extend((1..5).map(|i| Line::single(format!("본문 {i}"), Style::PLAIN)));
        lines.push(Line::single("대상 헤딩", Style::PLAIN));
        lines.extend((0..5).map(|i| Line::single(format!("뒷내용 {i}"), Style::PLAIN)));
        let target_line = 5;
        let document = Document { lines, links: vec!["#target".into()], headings: vec![("target".into(), target_line)], ..Document::default() };
        let mut pager = pager(document, &theme);
        let page = 3;
        pager.focus_next_link(true, page);
        assert_eq!(pager.focused_link, Some(0));

        pager.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), page);
        assert_eq!(pager.top, target_line, "포커스된 앵커 링크를 따라가 헤딩 줄로 이동해야 한다");
        assert_eq!(pager.focused_link, None, "따라간 뒤에는 포커스가 풀려야 한다");

        // 포커스가 없으면 기존처럼 한 줄 스크롤(회귀 5.1).
        let before = pager.top;
        pager.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), page);
        assert_eq!(pager.top, before + 1);
    }

    /// 6.1 마우스 왼쪽 클릭은 좌표가 링크 범위 안이면 그 링크를 따라가고, 밖이면 기존
    /// `toggle_block_at`으로 넘어간다(다이어그램 클릭 토글 회귀 없음).
    #[test]
    fn click_on_link_follows_it_instead_of_toggling() {
        let theme = Theme::none();
        let mut link_line = Line::empty();
        link_line.push_str("앵커", Style::PLAIN.underline());
        let document = Document { lines: vec![link_line], links: vec!["#없음".into()], ..Document::default() };
        let mut pager = pager(document, &theme);
        assert!(pager.click_at(0, 0, 10));
        assert!(pager.message.contains("없음"), "{}", pager.message);
    }

    #[test]
    fn click_outside_any_link_falls_back_to_existing_toggle_behavior() {
        let theme = Theme::none();
        let (document, block) = fixture();
        let mut pager = pager(document, &theme);
        assert!(pager.click_at(block.start, 0, 10));
        assert!(pager.expanded[0], "링크가 없는 자리 클릭은 기존처럼 다이어그램을 펼쳐야 한다");
    }

    /// 7.1 앵커 점프: 찾으면 그 헤딩 줄로, 못 찾으면 위치를 유지하고 상태 메시지를 남긴다.
    #[test]
    fn follow_link_anchor_jumps_to_heading_line_or_sets_message_if_missing() {
        let theme = Theme::none();
        // 클램프가 top을 되돌리지 않도록 문서를 뷰포트(page)보다 길게 만든다.
        let lines: Vec<Line> = (0..10).map(|i| Line::single(format!("줄 {i}"), Style::PLAIN)).collect();
        let document = Document { lines, headings: vec![("heading".into(), 5)], ..Document::default() };
        let mut pager = pager(document, &theme);
        let page = 3;
        pager.follow_link("#heading", page);
        assert_eq!(pager.top, 5);
        assert!(pager.message.is_empty());

        pager.top = 0;
        pager.follow_link("#missing", page);
        assert_eq!(pager.top, 0, "못 찾으면 위치를 유지해야 한다");
        assert!(pager.message.contains("missing"), "{}", pager.message);
    }

    /// 7.3 다른 파일로 이동: 성공하면 히스토리에 기록하고 내용·경로·스크롤 위치를 갱신하며,
    /// 실패하면 기존 화면을 유지한 채 상태 메시지만 남긴다. 표준입력(경로 없음)에서는 파일
    /// 이동이 조용히 꺼진다(4.2).
    #[test]
    fn follow_link_file_navigates_records_history_and_reports_failure() {
        let theme = Theme::none();
        let a = TempFile::with_content("# A\n");
        let dir = a.path.parent().unwrap().to_path_buf();
        let b_path = dir.join(format!("dg-pager-link-test-{}-b.md", std::process::id()));
        std::fs::write(&b_path, "# B\n").unwrap();
        let render = (|source: &str, _: usize, _: usize| Document { lines: vec![Line::single(source.to_string(), Style::PLAIN)], ..Document::default() })
            as fn(&str, usize, usize) -> Document;

        let mut pager = Pager::new("A", &theme, 80, "# A".to_string(), render, None, Some(a.path.clone()));
        pager.rerender(80, 80);

        pager.follow_link(b_path.file_name().unwrap().to_str().unwrap(), 10);
        assert_eq!(pager.current_path.as_deref(), Some(b_path.as_path()));
        assert!(pager.source.contains("# B"));
        assert_eq!(pager.top, 0);

        let before_source = pager.source.clone();
        pager.follow_link("없는-파일.md", 10);
        assert_eq!(pager.source, before_source, "실패하면 기존 화면을 유지해야 한다");
        assert!(!pager.message.is_empty());

        let mut stdin_pager = Pager::new("stdin", &theme, 80, "# A".to_string(), render, None, None);
        stdin_pager.rerender(80, 80);
        stdin_pager.follow_link(b_path.file_name().unwrap().to_str().unwrap(), 10);
        assert_eq!(stdin_pager.current_path, None, "표준입력에서는 파일 이동이 꺼져야 한다");
        assert!(stdin_pager.message.contains("표준입력"), "{}", stdin_pager.message);

        let _ = std::fs::remove_file(&b_path);
    }

    /// 7.4 `[`/`]`는 히스토리로 이전/다음 파일·위치를 복원하고, 히스토리가 없으면 무동작이다.
    #[test]
    fn bracket_keys_go_back_and_forward_through_history() {
        let theme = Theme::none();
        let a = TempFile::with_content("문서 A");
        let b = TempFile::with_content("문서 B");
        let render = (|source: &str, _: usize, _: usize| Document { lines: vec![Line::single(source.to_string(), Style::PLAIN)], ..Document::default() })
            as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("A", &theme, 80, "문서 A".to_string(), render, None, Some(a.path.clone()));
        pager.rerender(80, 80);

        // 히스토리가 비어 있으면 무동작(4.7).
        pager.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE), 10);
        assert_eq!(pager.current_path.as_deref(), Some(a.path.as_path()));

        let b_name = b.path.file_name().unwrap().to_str().unwrap().to_string();
        pager.follow_link(&b_name, 10);
        assert_eq!(pager.current_path.as_deref(), Some(b.path.as_path()));

        pager.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE), 10);
        assert_eq!(pager.current_path.as_deref(), Some(a.path.as_path()), "뒤로 가면 A로 돌아가야 한다");

        pager.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE), 10);
        assert_eq!(pager.current_path.as_deref(), Some(b.path.as_path()), "앞으로 가면 다시 B로 가야 한다");
    }
}
