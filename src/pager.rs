//! crossterm 기반 페이저: 스크롤·검색·창 크기 대응.

use crate::line::Line;
use crate::style::Theme;
use crate::text::char_width;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crossterm::{cursor, event, execute, queue, terminal};
use std::io::{self, Write};

const MOUSE_WHEEL_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_WHEEL_OFF: &str = "\x1b[?1006l\x1b[?1000l";

pub struct Pager<'a, F: Fn(usize) -> Vec<Line>> {
    title: String,
    theme: &'a Theme,
    max_width: usize,
    render: F,
    lines: Vec<Line>,
    rendered_width: usize,
    top: usize,
    query: String,
    typing: bool,
    matches: Vec<usize>,
    message: String,
}

impl<'a, F: Fn(usize) -> Vec<Line>> Pager<'a, F> {
    pub fn new(title: &str, theme: &'a Theme, max_width: usize, render: F) -> Self {
        Pager {
            title: title.to_string(),
            theme,
            max_width,
            render,
            lines: Vec::new(),
            rendered_width: 0,
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
            let width = columns.min(self.max_width).max(10);
            if width != self.rendered_width {
                self.lines = (self.render)(width);
                self.rendered_width = width;
                self.refresh_matches();
                needs_redraw = true;
            }
            let page = rows.saturating_sub(1).max(1);
            self.clamp(page);
            if needs_redraw {
                self.draw(out, columns, rows)?;
            }
            // 상태가 바뀐 이벤트만 다시 그린다. 마우스 이동·버튼은 무시.
            needs_redraw = match event::read()? {
                Event::Key(key) => {
                    if self.handle_key(key, page) {
                        return Ok(());
                    }
                    true
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        self.top += 3;
                        true
                    }
                    MouseEventKind::ScrollUp => {
                        self.top = self.top.saturating_sub(3);
                        true
                    }
                    _ => false,
                },
                Event::Resize(_, _) => true,
                _ => false,
            };
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
            _ => {}
        }
        false
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
            format!(" {}  {}%{}  ·  j/k 이동  / 검색  q 종료", self.title, percent, extra)
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
