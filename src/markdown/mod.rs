//! 마크다운을 스타일 줄로 렌더링한다.

pub mod table;
pub mod wrap;

use crate::diagram;
use crate::line::{Line, Span};
use crate::style::{Style, Theme};
use crate::text::{char_width, width_of};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// 들여쓰기 한 단: 첫 줄에 쓸 조각과 그 뒤 줄에 쓸 조각.
struct Indent {
    first: Span,
    rest: Span,
    first_used: bool,
}

struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Vec<Span>>>,
    current_row: Vec<Vec<Span>>,
    current_cell: Vec<Span>,
    has_header: bool,
}

pub struct Renderer<'a> {
    theme: &'a Theme,
    width: usize,
    lines: Vec<Line>,
    inline: Vec<Span>,
    style_stack: Vec<Style>,
    indents: Vec<Indent>,
    list_counters: Vec<Option<u64>>,
    code: Option<(String, String)>,
    table: Option<TableState>,
    link_url: Option<String>,
    in_metadata: bool,
    in_heading: Option<HeadingLevel>,
}

pub fn render(source: &str, theme: &Theme, width: usize) -> Vec<Line> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
    let parser = Parser::new_ext(source, options);
    let mut renderer = Renderer {
        theme,
        width: width.max(10),
        lines: Vec::new(),
        inline: Vec::new(),
        style_stack: vec![theme.text],
        indents: Vec::new(),
        list_counters: Vec::new(),
        code: None,
        table: None,
        link_url: None,
        in_metadata: false,
        in_heading: None,
    };
    for event in parser {
        renderer.handle(event);
    }
    renderer.flush_paragraph();
    while renderer.lines.last().is_some_and(Line::is_blank) {
        renderer.lines.pop();
    }
    renderer.lines
}

impl Renderer<'_> {
    fn style(&self) -> Style {
        *self.style_stack.last().unwrap()
    }

    fn push_style(&mut self, over: Style) {
        let merged = self.style().merge(over);
        self.style_stack.push(merged);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn prefix_width(&self) -> usize {
        self.indents.iter().map(|i| i.rest.width()).sum()
    }

    fn available(&self) -> usize {
        self.width.saturating_sub(self.prefix_width()).max(8)
    }

    /// 들여쓰기 접두를 만든다. 아직 안 쓴 첫 줄 표식은 여기서 소비된다.
    fn take_prefix(&mut self) -> Line {
        let mut line = Line::empty();
        for indent in &mut self.indents {
            if indent.first_used {
                line.push(indent.rest.clone());
            } else {
                line.push(indent.first.clone());
                indent.first_used = true;
            }
        }
        line
    }

    fn emit(&mut self, content: Line) {
        let mut line = self.take_prefix();
        for span in content.spans {
            line.push(span);
        }
        self.lines.push(line);
    }

    fn emit_blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.is_blank()) {
            let prefix = self.take_prefix();
            let mut line = Line::empty();
            for span in prefix.spans {
                if span.text.trim().is_empty() {
                    continue;
                }
                line.push(Span::new(span.text.trim_end().to_string(), span.style));
            }
            self.lines.push(line);
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.in_metadata {
            return;
        }
        if let Some(table) = &mut self.table {
            table.current_cell.push(Span::new(text, self.style_stack.last().copied().unwrap_or_default()));
            return;
        }
        let style = self.style();
        self.inline.push(Span::new(text, style));
    }

    fn flush_paragraph(&mut self) {
        if self.inline.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.inline);
        let available = self.available();
        for wrapped in wrap::wrap_spans(&spans, available) {
            self.emit(Line::from_spans(wrapped));
        }
    }

    fn handle(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, buffer)) = &mut self.code {
                    buffer.push_str(&text);
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(text) => {
                let style = self.style().merge(self.theme.code);
                let shown = if self.theme.enabled { format!(" {text} ") } else { format!("`{text}`") };
                let span = Span::new(shown, style);
                if let Some(table) = &mut self.table {
                    table.current_cell.push(span);
                } else {
                    self.inline.push(span);
                }
            }
            Event::Html(html) => {
                if let Some((_, buffer)) = &mut self.code {
                    buffer.push_str(&html);
                    return;
                }
                self.flush_paragraph();
                for raw in html.lines() {
                    let line = Line::single(raw.to_string(), self.theme.html);
                    self.emit(line);
                }
            }
            Event::InlineHtml(html) => {
                let lower = html.to_ascii_lowercase();
                if lower.starts_with("<br") {
                    self.push_text("\n");
                } else {
                    let style = self.style().merge(self.theme.html);
                    self.inline.push(Span::new(html.to_string(), style));
                }
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush_paragraph();
                self.emit_blank();
                let rule = Line::single("─".repeat(self.available().min(60)), self.theme.rule);
                self.emit(rule);
                self.emit_blank();
            }
            Event::FootnoteReference(name) => {
                let style = self.style().merge(self.theme.link_url);
                self.inline.push(Span::new(format!("[^{name}]"), style));
            }
            Event::TaskListMarker(checked) => {
                if let Some(indent) = self.indents.last_mut() {
                    let marker = if checked { "✓ " } else { "☐ " };
                    let bullet_width = indent.first.width();
                    let mut text = " ".repeat(bullet_width.saturating_sub(2));
                    text.push_str(marker);
                    indent.first = Span::new(text, indent.first.style);
                }
            }
            Event::InlineMath(text) | Event::DisplayMath(text) => self.push_text(&text),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.flush_paragraph();
                self.emit_blank();
                self.in_heading = Some(level);
                let index = (level as usize).saturating_sub(1).min(5);
                self.push_style(self.theme.heading[index]);
            }
            Tag::BlockQuote(_) => {
                self.flush_paragraph();
                self.emit_blank();
                self.indents.push(Indent {
                    first: Span::new("│ ", self.theme.quote_bar),
                    rest: Span::new("│ ", self.theme.quote_bar),
                    first_used: true,
                });
                self.push_style(self.theme.quote);
            }
            Tag::CodeBlock(kind) => {
                self.flush_paragraph();
                self.emit_blank();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                self.flush_paragraph();
                if self.list_counters.is_empty() {
                    self.emit_blank();
                }
                self.list_counters.push(start);
            }
            Tag::Item => {
                self.flush_paragraph();
                let depth = self.list_counters.len().saturating_sub(1);
                let marker = match self.list_counters.last_mut() {
                    Some(Some(counter)) => {
                        let text = format!("{counter}. ");
                        *counter += 1;
                        text
                    }
                    _ => {
                        let bullet = ["•", "◦", "▪"][depth % 3];
                        format!("{bullet} ")
                    }
                };
                let rest = " ".repeat(width_of(&marker));
                self.indents.push(Indent { first: Span::new(marker, self.theme.bullet), rest: Span::new(rest, Style::PLAIN), first_used: false });
            }
            Tag::Table(alignments) => {
                self.flush_paragraph();
                self.emit_blank();
                self.table = Some(TableState { alignments, rows: Vec::new(), current_row: Vec::new(), current_cell: Vec::new(), has_header: false });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.has_header = true;
                }
            }
            Tag::TableRow => {}
            Tag::TableCell => {
                if let Some(table) = &mut self.table {
                    table.current_cell.clear();
                }
            }
            Tag::Emphasis => self.push_style(self.theme.emphasis),
            Tag::Strong => self.push_style(self.theme.strong),
            Tag::Strikethrough => self.push_style(self.theme.strike),
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
                self.push_style(self.theme.link);
            }
            Tag::Image { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
                self.push_style(self.theme.image);
                self.push_text("▣ ");
            }
            Tag::FootnoteDefinition(name) => {
                self.flush_paragraph();
                self.emit_blank();
                let marker = format!("[^{name}] ");
                let rest = " ".repeat(width_of(&marker));
                self.indents.push(Indent { first: Span::new(marker, self.theme.link_url), rest: Span::new(rest, Style::PLAIN), first_used: false });
            }
            Tag::MetadataBlock(_) => self.in_metadata = true,
            Tag::HtmlBlock => self.flush_paragraph(),
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition | Tag::Superscript | Tag::Subscript => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_paragraph();
                self.emit_blank();
            }
            TagEnd::Heading(level) => {
                let spans = std::mem::take(&mut self.inline);
                let available = self.available();
                let wrapped = wrap::wrap_spans(&spans, available);
                let text_width = wrapped.iter().map(|w| w.iter().map(Span::width).sum::<usize>()).max().unwrap_or(0);
                for line in wrapped {
                    self.emit(Line::from_spans(line));
                }
                let underline = match level {
                    HeadingLevel::H1 => Some("━"),
                    HeadingLevel::H2 => Some("─"),
                    _ => None,
                };
                if let Some(glyph) = underline {
                    let index = (level as usize).saturating_sub(1).min(5);
                    let line = Line::single(glyph.repeat(text_width.max(1)), self.theme.heading[index]);
                    self.emit(line);
                }
                self.pop_style();
                self.in_heading = None;
                self.emit_blank();
            }
            TagEnd::BlockQuote(_) => {
                self.flush_paragraph();
                self.pop_style();
                while self.lines.last().is_some_and(|l| !l.is_blank() && l.plain().trim().chars().all(|c| c == '│')) {
                    self.lines.pop();
                }
                self.indents.pop();
                self.emit_blank();
            }
            TagEnd::CodeBlock => {
                let Some((lang, buffer)) = self.code.take() else { return };
                self.emit_code_block(&lang, &buffer);
                self.emit_blank();
            }
            TagEnd::List(_) => {
                self.flush_paragraph();
                self.list_counters.pop();
                if self.list_counters.is_empty() {
                    self.emit_blank();
                }
            }
            TagEnd::Item => {
                self.flush_paragraph();
                if let Some(indent) = self.indents.last()
                    && !indent.first_used
                {
                    // 본문 없는 항목이라도 표식은 남긴다.
                    self.emit(Line::empty());
                }
                self.indents.pop();
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let available = self.available();
                    let lines = table::render(&table.rows, table.has_header, &table.alignments, available, self.theme);
                    for line in lines {
                        self.emit(line);
                    }
                    self.emit_blank();
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.current_row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                if let Some(table) = &mut self.table {
                    let cell = std::mem::take(&mut table.current_cell);
                    table.current_row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_style(),
            TagEnd::Link => {
                self.pop_style();
                if let Some(url) = self.link_url.take() {
                    let shown: String = self.inline.iter().rev().take(1).map(|s| s.text.clone()).collect();
                    if shown.trim() != url && !url.starts_with('#') {
                        let style = self.style().merge(self.theme.link_url);
                        self.inline.push(Span::new(format!(" ({url})"), style));
                    }
                }
            }
            TagEnd::Image => {
                self.pop_style();
                if let Some(url) = self.link_url.take() {
                    let style = self.style().merge(self.theme.link_url);
                    self.inline.push(Span::new(format!(" ({url})"), style));
                }
            }
            TagEnd::FootnoteDefinition => {
                self.flush_paragraph();
                self.indents.pop();
                self.emit_blank();
            }
            TagEnd::MetadataBlock(_) => self.in_metadata = false,
            TagEnd::HtmlBlock => self.emit_blank(),
            TagEnd::DefinitionList | TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition | TagEnd::Superscript | TagEnd::Subscript => {}
        }
    }

    fn emit_code_block(&mut self, lang: &str, buffer: &str) {
        let available = self.available();
        if let Some(language) = diagram::language_of_fence(lang)
            && let Some(lines) = diagram::render(language, buffer, self.theme, available.saturating_sub(2))
        {
            for line in lines {
                self.emit(line);
            }
            return;
        }
        let rule_width = available.min(60);
        let mut header = Line::single("╭─", self.theme.code_border);
        if !lang.is_empty() {
            header.push(Span::new(format!(" {lang} "), self.theme.code_lang));
        }
        let used = header.width();
        header.push(Span::new("─".repeat(rule_width.saturating_sub(used)), self.theme.code_border));
        self.emit(header);
        let body_width = available.saturating_sub(2).max(4);
        for raw in buffer.lines() {
            let expanded = raw.replace('\t', "    ");
            for piece in hard_split(&expanded, body_width) {
                let line = Line::from_spans(vec![Span::new("│ ", self.theme.code_border), Span::new(piece, self.theme.code_block)]);
                self.emit(line);
            }
        }
        self.emit(Line::single(format!("╰{}", "─".repeat(rule_width.saturating_sub(1))), self.theme.code_border));
    }
}

/// 글자 단위로 폭에 맞춰 자른다(코드용).
fn hard_split(text: &str, width: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for c in text.chars() {
        let w = char_width(c);
        if current_width + w > width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(c);
        current_width += w;
    }
    pieces.push(current);
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(source: &str, width: usize) -> Vec<String> {
        render(source, &Theme::none(), width).iter().map(Line::plain).collect()
    }

    #[test]
    fn heading_paragraph_and_list() {
        let out = plain("# Title\n\nSome *text* here.\n\n- one\n- two\n  - nested\n", 40);
        assert_eq!(out[0], "Title");
        assert_eq!(out[1], "━━━━━");
        assert!(out.contains(&"Some text here.".to_string()));
        assert!(out.contains(&"• one".to_string()));
        assert!(out.contains(&"  ◦ nested".to_string()));
    }

    #[test]
    fn code_block_and_quote() {
        let out = plain("```rust\nfn main() {}\n```\n\n> quoted\n", 40);
        assert!(out[0].starts_with("╭─ rust "));
        assert_eq!(out[1], "│ fn main() {}");
        assert!(out.contains(&"│ quoted".to_string()));
    }

    #[test]
    fn mermaid_fence_becomes_diagram() {
        let out = plain("```mermaid\nflowchart TB\n A --> B\n```\n", 60);
        assert!(out[0].contains("mermaid · flowchart"));
        assert!(out.iter().any(|l| l.contains("▼")));
    }

    #[test]
    fn front_matter_is_hidden() {
        let out = plain("---\ntitle: x\n---\n\nbody\n", 40);
        assert_eq!(out, vec!["body"]);
    }
}
