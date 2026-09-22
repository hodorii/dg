//! crossterm 기반 페이저: 스크롤·검색·창 크기 대응.

use crate::line::Line;
use crate::links::{self, HistoryEntry, LinkKind, LinkPosition};
use crate::markdown::{self, DiagramBlock, Document, TextBlock};
use crate::style::{Style, Theme};
use crate::text::char_width;
use crate::watch::{self, PollResult, Watcher};
use std::path::PathBuf;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::{cursor, event, execute, queue, terminal};
use std::io::{self, Write};

const MOUSE_WHEEL_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_WHEEL_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// 토글 가능한 블록이 어느 목록의 몇 번째인지(markdown-source-view — 기존 다이어그램 전용
/// 토글을 텍스트 블록까지 일반화). `Document.diagrams`/`Document.text_blocks` 자체는 이
/// 스펙에서 건드리지 않는다 — 페이저 쪽에서만 두 목록을 한 화면 좌표계로 합쳐서 다룬다.
#[derive(Clone, Copy)]
enum BlockRef {
    Diagram(usize),
    Text(usize),
}

/// 화면에 나오는 토글 가능한 블록 하나의 줄 범위(펼쳤으면 그것까지 포함).
struct ShownBlock {
    start: usize,
    end: usize,
    block: BlockRef,
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
    /// 텍스트 블록(문단·헤딩·인용·목록 등)마다 원문으로 바꿔 보여주는지(markdown-source-view).
    text_expanded: Vec<bool>,
    /// 전역 원문 토글(`s` 키) — 켜져 있으면 `self.lines`는 문서 전체의 원문 하이라이팅이고,
    /// 블록별 펼침 상태(`expanded`/`text_expanded`)는 건드리지 않아 꺼도 그대로 유지된다.
    viewing_source: bool,
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
            text_expanded: Vec::new(),
            viewing_source: false,
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
        self.text_expanded.resize(self.document.text_blocks.len(), false);
        self.rendered_width = width;
        self.rendered_columns = columns;
        self.refresh_lines();
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
            KeyCode::Char('s') => {
                self.viewing_source = !self.viewing_source;
                self.refresh_lines();
            }
            _ => {}
        }
        false
    }

    /// 표시 줄 번호가 토글 가능한 블록(다이어그램·텍스트 블록 무관) 안이면 그 블록을 펼치거나
    /// 접는다. 바뀌면 true.
    fn toggle_block_at(&mut self, line: usize) -> bool {
        let Some(block) = self.shown_blocks.iter().find(|b| b.start <= line && line < b.end) else { return false };
        match block.block {
            BlockRef::Diagram(index) => self.expanded[index] = !self.expanded[index],
            BlockRef::Text(index) => self.text_expanded[index] = !self.text_expanded[index],
        }
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

    /// 다시 그려야 할 때(재렌더링·전역 원문 토글 전환) `self.lines`를 채운다. 전역 원문
    /// 보기가 켜져 있으면 문서 전체를 원문 하이라이팅으로 바꾸고(블록·헤딩·링크 인식은 이
    /// 동안 의미가 없어 비운다), 아니면 기존 블록 인식 경로(`rebuild_lines`)를 쓴다.
    fn refresh_lines(&mut self) {
        if self.viewing_source {
            self.lines = markdown::render_source_text(&self.source, self.theme, self.rendered_columns.max(10));
            self.shown_blocks = Vec::new();
            self.heading_lines = Vec::new();
            self.link_positions = Vec::new();
            self.focused_link = None;
            self.refresh_matches();
        } else {
            self.rebuild_lines();
        }
    }

    /// 문서 줄에 펼친/대체된 원문을 끼워 넣어 표시 줄을 만든다. 다이어그램과 텍스트 블록을
    /// 원본 줄 순서로 합쳐 한 좌표계(`shown_blocks`)로 다룬다(markdown-source-view) — 둘 다
    /// `Document`가 겹치지 않게 만들어 준다는 전제 위에서, 한쪽만 훑어도 되는 별도 루프 없이
    /// 이 순회 하나로 끝난다.
    fn rebuild_lines(&mut self) {
        let mut blocks: Vec<(usize, usize, BlockRef)> = Vec::new();
        for (index, block) in self.document.diagrams.iter().enumerate() {
            blocks.push((block.start, block.end, BlockRef::Diagram(index)));
        }
        for (index, block) in self.document.text_blocks.iter().enumerate() {
            blocks.push((block.start, block.end, BlockRef::Text(index)));
        }
        blocks.sort_by_key(|(start, _, _)| *start);

        let mut lines: Vec<Line> = Vec::with_capacity(self.document.lines.len());
        let mut shown = Vec::new();
        // (원본 block.end, 그 시점까지 늘거나(다이어그램 원문 삽입) 줄어든(텍스트 블록 원문
        // 대체) 누적 줄 수) — 헤딩 위치 보정에 쓴다. 대체는 줄어들 수도 있어 isize.
        let mut offsets: Vec<(usize, isize)> = Vec::new();
        let mut cumulative_offset: isize = 0;
        let mut cursor = 0;
        for (start, end, block_ref) in blocks {
            lines.extend(self.document.lines[cursor..start].iter().cloned());
            let shown_start = lines.len();
            match block_ref {
                BlockRef::Diagram(index) => {
                    let block = &self.document.diagrams[index];
                    // 캡션 줄은 항상 그대로, 원문은 그 바로 아래 "덧붙인다"(그린 다이어그램은
                    // 안 지운다) — 기존 동작 그대로(회귀 없음).
                    lines.push(self.document.lines[start].clone());
                    if self.expanded[index] {
                        let source_lines = self.source_lines(block);
                        cumulative_offset += source_lines.len() as isize;
                        lines.extend(source_lines);
                    }
                    lines.extend(self.document.lines[start + 1..end].iter().cloned());
                }
                BlockRef::Text(index) => {
                    // 텍스트 블록은 펼치면 그 구간을 원문으로 "바꾼다"(둘을 같이 보여주면
                    // 중복이라 다이어그램과 다르게 대체 방식을 쓴다 — design.md 참조).
                    if self.text_expanded[index] {
                        let block = &self.document.text_blocks[index];
                        let replacement = self.text_source_lines(block);
                        cumulative_offset += replacement.len() as isize - (end - start) as isize;
                        lines.extend(replacement);
                    } else {
                        lines.extend(self.document.lines[start..end].iter().cloned());
                    }
                }
            }
            shown.push(ShownBlock { start: shown_start, end: lines.len(), block: block_ref });
            offsets.push((end, cumulative_offset));
            cursor = end;
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
                (slug.clone(), (*original as isize + offset).max(0) as usize)
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

    fn text_source_lines(&self, block: &TextBlock) -> Vec<Line> {
        markdown::render_source_text(&block.source, self.theme, self.rendered_columns.max(10))
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
        pager.text_expanded = vec![false; document.text_blocks.len()];
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

    /// 2.1~2.3(markdown-source-view) `toggle_block_at`가 다이어그램·텍스트 블록을 가리지 않고
    /// 같은 좌표계로 다룬다. 텍스트 블록은 대체(원문으로 바뀜), 다이어그램은 기존처럼 덧붙임
    /// (회귀 없음) — 서로 상태가 안 섞인다.
    #[test]
    fn block_toggle_generalizes_across_diagram_and_text_blocks() {
        let theme = Theme::none();
        let block = DiagramBlock { start: 2, end: 5, lang: "mermaid".to_string(), source: "flowchart TB\n A --> B".to_string() };
        let text_block = TextBlock { start: 0, end: 1, source: "**굵게** 문단".to_string() };
        let lines = vec![
            Line::single("굵게 문단", Style::PLAIN),
            Line::single("본문", Style::PLAIN),
            Line::single("◈ mermaid · flowchart ─", Style::PLAIN),
            Line::single("그림 줄 1", Style::PLAIN),
            Line::single("그림 줄 2", Style::PLAIN),
        ];
        let document = Document { lines, diagrams: vec![block], text_blocks: vec![text_block], ..Document::default() };
        let mut pager = pager(document, &theme);

        // 텍스트 블록(줄 0) 토글 → 원문으로 대체.
        assert!(pager.toggle_block_at(0));
        assert!(pager.lines[0].plain().contains("**굵게**"), "{:?}", pager.lines[0].plain());
        assert!(pager.text_expanded[0]);
        assert!(!pager.expanded[0], "텍스트 블록 토글이 다이어그램 상태를 건드리면 안 된다");

        // 다시 토글하면 원래 렌더로 복귀.
        assert!(pager.toggle_block_at(0));
        assert_eq!(pager.lines[0].plain(), "굵게 문단");

        // 다이어그램(캡션 줄) 토글 → 기존처럼 원문이 캡션 아래 덧붙여지고 그린 그림도 남는다.
        let caption_row = pager.lines.iter().position(|l| l.plain().contains('◈')).unwrap();
        assert!(pager.toggle_block_at(caption_row));
        let plain: Vec<String> = pager.lines.iter().map(Line::plain).collect();
        assert!(plain.iter().any(|l| l.contains("flowchart TB")), "원문이 덧붙여져야 한다: {plain:?}");
        assert!(plain.iter().any(|l| l.contains("그림 줄 1")), "그린 그림도 남아 있어야 한다(회귀 없음): {plain:?}");
        assert!(!pager.text_expanded[0], "다이어그램 토글이 텍스트 블록 상태를 건드리면 안 된다");
    }

    /// 2.4 토글 가능한 블록 경계가 없는 위치(빈 줄 등)에서는 화면이 바뀌지 않는다.
    #[test]
    fn toggle_at_line_with_no_block_is_noop() {
        let theme = Theme::none();
        let document = Document { lines: vec![Line::single("그냥 본문", Style::PLAIN)], ..Document::default() };
        let mut pager = pager(document, &theme);
        assert!(!pager.toggle_block_at(0));
    }

    /// 1.1/1.2/1.5(markdown-source-view) `s` 키는 문서 전체를 원문 하이라이팅으로 전환하고,
    /// 다시 누르면 스크롤 위치를 유지한 채 렌더 화면으로 돌아가며, 그 사이 블록별 펼침 상태는
    /// 건드리지 않는다.
    #[test]
    fn global_source_toggle_round_trips_and_preserves_scroll_and_block_state() {
        let theme = Theme::none();
        let source = "# 제목\n\n**굵게** 문단\n".to_string();
        let render = (|src: &str, _: usize, _: usize| Document {
            lines: vec![Line::single("렌더된 줄", Style::PLAIN)],
            text_blocks: vec![TextBlock { start: 0, end: 1, source: src.to_string() }],
            ..Document::default()
        }) as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", &theme, 80, source.clone(), render, None, None);
        pager.rerender(80, 80);
        pager.top = 1;
        pager.text_expanded[0] = true;
        pager.rebuild_lines();
        let rendered_before: Vec<String> = pager.lines.iter().map(Line::plain).collect();

        pager.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), 10);
        assert!(pager.viewing_source);
        let shown: Vec<String> = pager.lines.iter().map(Line::plain).collect();
        assert_eq!(shown.join("\n"), source.trim_end_matches('\n'), "원문이 그대로 보여야 한다");
        assert!(pager.shown_blocks.is_empty());

        pager.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), 10);
        assert!(!pager.viewing_source);
        assert_eq!(pager.top, 1, "스크롤 위치가 유지돼야 한다");
        let rendered_after: Vec<String> = pager.lines.iter().map(Line::plain).collect();
        assert_eq!(rendered_after, rendered_before, "렌더 화면·블록 펼침 상태가 왕복 후에도 그대로여야 한다");
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

    /// 1.4 감시 모드에서 전역 원문 보기 중 파일이 바뀌면 원문 내용도 최신으로 갱신되고,
    /// 전역 보기 모드 자체는 꺼지지 않는다.
    #[test]
    fn global_source_toggle_updates_on_watch_reload() {
        let theme = Theme::none();
        let temp = TempFile::with_content("# A\n");
        let render =
            (|source: &str, _: usize, _: usize| Document { lines: vec![Line::single(source.to_string(), Style::PLAIN)], ..Document::default() })
                as fn(&str, usize, usize) -> Document;
        let mut pager = Pager::new("t", &theme, 80, "# A\n".to_string(), render, Some(Watcher::new(&temp.path)), None);
        pager.rerender(80, 80);
        pager.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), 10);
        assert!(pager.lines.iter().any(|l| l.plain().contains('A')));

        std::fs::write(&temp.path, "# B\n").unwrap();
        pager.manual_refresh();
        assert!(pager.viewing_source, "감시 갱신이 전역 원문 모드를 꺼서는 안 된다");
        assert!(pager.lines.iter().any(|l| l.plain().contains('B')), "{:?}", pager.lines.iter().map(Line::plain).collect::<Vec<_>>());
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
        let document =
            Document { lines, diagrams: vec![block], links: vec!["#target".into()], headings: vec![("target".into(), 3)], ..Document::default() };
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
