//! 마크다운을 스타일 줄로 렌더링한다.

pub mod table;
pub mod wrap;

use crate::diagram::{self, DiagramOptions};
use crate::line::{Line, Span};
use crate::style::{Style, Theme};
use crate::text::{char_width, width_of};
use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

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

/// 렌더링된 문서. 다이어그램 블록의 위치와 원문을 함께 돌려주어 페이저가 원문을 펼칠 수 있게 한다.
#[derive(Default)]
pub struct Document {
    pub lines: Vec<Line>,
    pub diagrams: Vec<DiagramBlock>,
    /// 다이어그램이 아닌 최상위 블록(문단·헤딩·인용·목록·표·각주·일반 코드펜스 등)의 위치와
    /// 원문(markdown-source-view). 중첩된 하위 블록은 별도로 기록하지 않고 가장 바깥쪽 블록
    /// 하나로 묶는다.
    pub text_blocks: Vec<TextBlock>,
    /// 링크 목적지(등장 순서). 화면 위치는 `links::locate_links`가 밑줄 스타일로 찾아 이
    /// 목록과 순서대로 짝짓는다(markdown-link-navigation).
    pub links: Vec<String>,
    /// (슬러그, 원본 줄 번호) — 다이어그램 원문이 펼쳐져 줄이 밀리기 전 기준.
    pub headings: Vec<(String, usize)>,
}

/// GitHub 스타일 헤딩 슬러그 생성기. 같은 슬러그가 다시 나오면 `-1`/`-2`를 붙인다.
#[derive(Default)]
struct Slugger {
    seen: std::collections::HashMap<String, u32>,
}

impl Slugger {
    /// 소문자화, 공백·밑줄·하이픈을 하이픈으로, 그 외 영숫자 아닌 문자는 제거한다.
    fn slug(&mut self, heading_text: &str) -> String {
        let mut base = String::new();
        for c in heading_text.chars() {
            if c.is_alphanumeric() {
                base.extend(c.to_lowercase());
            } else if c == ' ' || c == '-' || c == '_' {
                base.push('-');
            }
        }
        let base = base.trim_matches('-');
        let base = if base.is_empty() { "section" } else { base };
        let count = self.seen.entry(base.to_string()).or_insert(0);
        let slug = if *count == 0 { base.to_string() } else { format!("{base}-{count}") };
        *count += 1;
        slug
    }
}

#[derive(Clone, Debug)]
pub struct DiagramBlock {
    /// 캡션 줄 번호.
    pub start: usize,
    /// 마지막 줄 다음 번호.
    pub end: usize,
    /// 코드 펜스에 적힌 언어 이름.
    pub lang: String,
    pub source: String,
}

/// 다이어그램이 아닌 최상위 블록 하나(markdown-source-view).
#[derive(Clone, Debug)]
pub struct TextBlock {
    /// 그 블록의 렌더된 첫 줄 번호.
    pub start: usize,
    /// 마지막 줄 다음 번호.
    pub end: usize,
    /// 원본 바이트 범위로 슬라이스한 마크다운 원문 그대로(가공 없음).
    pub source: String,
}

/// 최상위 블록 추적 중 열려 있는 항목 — 깊이가 0으로 돌아올 때 `TextBlock`으로 확정된다.
struct PendingTextBlock {
    start_line: usize,
    source_range: Range<usize>,
    is_code_block: bool,
    diagrams_before: usize,
}

pub struct Renderer<'a> {
    theme: &'a Theme,
    diagram_options: DiagramOptions,
    /// 문단 줄바꿈 폭.
    width: usize,
    /// 다이어그램·표·코드블록에 허용하는 폭(보통 터미널 전체 폭).
    block_width: usize,
    /// 오프셋 슬라이싱용 원본 마크다운 전체(`render_source_block`처럼 파서 루프를 안 쓰는
    /// 호출은 빈 문자열 — 그 경로는 블록 추적을 안 한다).
    source: &'a str,
    lines: Vec<Line>,
    diagrams: Vec<DiagramBlock>,
    text_blocks: Vec<TextBlock>,
    text_block_depth: u32,
    pending_text_block: Option<PendingTextBlock>,
    links: Vec<String>,
    headings: Vec<(String, usize)>,
    slugger: Slugger,
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

#[cfg(test)]
pub fn render(source: &str, theme: &Theme, width: usize) -> Vec<Line> {
    render_document(source, theme, width, width, DiagramOptions::default()).lines
}

/// GFM 알림 종류별 (라벨, 스타일). `xychart`/`gitgraph`와 같은 4색 카테고리 팔레트
/// (`diagram_accent`/`diagram_box`/`diagram_group`/`diagram_note`)를 재사용한다 — dg는
/// 마크다운 요소에 새 원색을 들이지 않고 명도·굵기로 구분하는 색상 철학을 쓰므로, 5종 전부에
/// 새 색을 배정하는 대신 기존 팔레트를 한 바퀴 돌리고 다섯 번째(Caution)만 굵게 더해 Note와
/// 구분한다. 참 구분 기준은 항상 라벨 텍스트다(`--style none`에서도 유효).
fn alert_style(theme: &Theme, kind: BlockQuoteKind) -> (&'static str, Style) {
    match kind {
        BlockQuoteKind::Note => ("Note", theme.diagram_accent),
        BlockQuoteKind::Tip => ("Tip", theme.diagram_box),
        BlockQuoteKind::Important => ("Important", theme.diagram_group),
        BlockQuoteKind::Warning => ("Warning", theme.diagram_note),
        BlockQuoteKind::Caution => ("Caution", theme.diagram_accent.bold()),
    }
}

/// 다이어그램 원문을 코드블록으로 그린다(펼쳐 보기용).
pub fn render_source_block(lang: &str, source: &str, theme: &Theme, width: usize) -> Vec<Line> {
    let mut renderer = Renderer::new(theme, "", width, width, DiagramOptions::default());
    renderer.emit_code_block(lang, source, false);
    renderer.lines
}

/// 마크다운 원문(`source`)을 문자 하나 안 바꾸고 그대로 보여주되, 헤딩(`#`)·강조(`**`/`*`)·
/// 링크(`[텍스트](url)`)·인용(`>`)·목록 마커(`-`/`1.`)·코드펜스 구분 기호(```)에 기존 렌더
/// 테마 색을 덧씌운다(markdown-source-view, 전역/블록 원문 토글 공용). `--style none`에서는
/// 색만 안 보일 뿐 문자는 그대로라 문법 요소 구분 자체는 유지된다(원문이니 원래 그 문자가 있다).
pub fn render_source_text(source: &str, theme: &Theme, width: usize) -> Vec<Line> {
    let width = width.max(10);
    let paints = collect_syntax_paints(source, theme);
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    for (i, c) in source.char_indices() {
        if c == '\n' {
            lines.extend(wrap_line_hard(&styled_physical_line(source, line_start, i, &paints), width));
            line_start = i + 1;
        }
    }
    if line_start < source.len() {
        lines.extend(wrap_line_hard(&styled_physical_line(source, line_start, source.len(), &paints), width));
    }
    if lines.is_empty() {
        lines.push(Line::empty());
    }
    lines
}

/// 원문을 훑어 문법 요소별 (바이트 범위, 스타일) 목록을 만든다. 범위가 겹치면(예: 헤딩 안
/// 링크) `styled_physical_line`이 바깥→안쪽 순서로 `Style::merge`해 안쪽이 우선하게 한다.
fn collect_syntax_paints(source: &str, theme: &Theme) -> Vec<(Range<usize>, Style)> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_GFM);
    let mut paints = Vec::new();
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let index = (level as usize).saturating_sub(1).min(5);
                paints.push((range, theme.heading[index]));
            }
            Event::Start(Tag::Strong) => paints.push((range, theme.strong)),
            Event::Start(Tag::Emphasis) => paints.push((range, theme.emphasis)),
            Event::Start(Tag::Link { .. }) => paints.push((range, theme.link)),
            Event::Start(Tag::BlockQuote(_)) => paints.push((range, theme.quote)),
            Event::Start(Tag::Item) => {
                if let Some(marker_end) = item_marker_end(source, range.start) {
                    paints.push((range.start..marker_end, theme.bullet));
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                for fence_range in fence_marker_ranges(source, &range) {
                    paints.push((fence_range, theme.code_border));
                }
            }
            _ => {}
        }
    }
    paints
}

/// 리스트 항목 시작에서 마커(`-`/`*`/`+`/`1.`/`1)`) + 뒤따르는 공백까지의 끝 위치. 마커로
/// 안 보이면(예상 밖 형태) `None` — 그 항목은 색 없이 그대로 둔다(패닉 없음).
fn item_marker_end(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start..];
    let mut chars = rest.chars();
    let first = chars.next()?;
    let mut end = start + first.len_utf8();
    if matches!(first, '-' | '*' | '+') {
        // 그대로 진행 — 불릿 한 글자로 끝.
    } else if first.is_ascii_digit() {
        let mut closed = false;
        for c in chars.by_ref() {
            end += c.len_utf8();
            if c == '.' || c == ')' {
                closed = true;
                break;
            }
            if !c.is_ascii_digit() {
                return None;
            }
        }
        if !closed {
            return None;
        }
    } else {
        return None;
    }
    while source[end..].starts_with(' ') || source[end..].starts_with('\t') {
        end += 1;
    }
    Some(end)
}

/// 펜스 코드블록의 여는 줄과 닫는 줄(``` 등 + 언어)만 범위로 돌려준다 — 안쪽 내용은 색 없이
/// 그대로 둔다.
fn fence_marker_ranges(source: &str, block: &Range<usize>) -> Vec<Range<usize>> {
    let text = &source[block.clone()];
    let mut ranges = Vec::new();
    if let Some(first_newline) = text.find('\n') {
        ranges.push(block.start..block.start + first_newline);
    }
    if let Some(last_newline) = text.rfind('\n') {
        let closing_start = block.start + last_newline + 1;
        if closing_start < block.end {
            ranges.push(closing_start..block.end);
        }
    }
    ranges
}

/// `[start, end)` 구간(원문 안 물리적 한 줄, 개행 문자는 제외)을 겹치는 `paints`로 칠해
/// `Line` 하나로 만든다.
fn styled_physical_line(source: &str, start: usize, end: usize, paints: &[(Range<usize>, Style)]) -> Line {
    let mut line = Line::empty();
    if start >= end {
        return line;
    }
    let mut run_start = start;
    let mut run_style = Style::PLAIN;
    let mut first = true;
    for (byte_i, _) in source[start..end].char_indices() {
        let abs = start + byte_i;
        let style = paints.iter().filter(|(r, _)| r.contains(&abs)).fold(Style::PLAIN, |acc, (_, s)| acc.merge(*s));
        if first {
            run_style = style;
            first = false;
        } else if style != run_style {
            line.push_str(&source[run_start..abs], run_style);
            run_start = abs;
            run_style = style;
        }
    }
    if run_start < end {
        line.push_str(&source[run_start..end], run_style);
    }
    line
}

/// 한 줄을 문자 폭 기준으로 강제 줄바꿈한다(`hard_split`의 스타일 보존 버전) — 원문 줄바꿈은
/// 이미 물리적 줄 단위로 나뉜 뒤라, 여기서는 터미널 폭을 넘는 줄만 자른다.
fn wrap_line_hard(line: &Line, width: usize) -> Vec<Line> {
    if line.width() <= width {
        return vec![line.clone()];
    }
    let mut out = Vec::new();
    let mut current = Line::empty();
    let mut used = 0usize;
    for (text, style) in line.runs() {
        for c in text.chars() {
            let w = char_width(c);
            if used + w > width && used > 0 {
                out.push(std::mem::replace(&mut current, Line::empty()));
                used = 0;
            }
            let mut buf = [0u8; 4];
            current.push_str(c.encode_utf8(&mut buf), style);
            used += w;
        }
    }
    out.push(current);
    out
}

/// `width`는 문단 줄바꿈 폭, `block_width`는 다이어그램·표·코드블록이 쓸 수 있는 폭.
pub fn render_document(source: &str, theme: &Theme, width: usize, block_width: usize, diagram_options: DiagramOptions) -> Document {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
    // GitHub 스타일 알림 블록(`> [!NOTE]` 등)의 종류를 파싱해 준다(markdown-gfm-alerts).
    options.insert(Options::ENABLE_GFM);
    // 오프셋 반복자를 쓰면 이벤트마다 원본 바이트 범위가 같이 나와, 블록별 원문을 재구성이
    // 아니라 그대로 슬라이스할 수 있다(markdown-source-view) — 렌더링 로직 자체는 그대로다.
    let parser = Parser::new_ext(source, options).into_offset_iter();
    let mut renderer = Renderer::new(theme, source, width, block_width, diagram_options);
    for (event, range) in parser {
        renderer.handle(event, range);
    }
    renderer.flush_paragraph();
    while renderer.lines.last().is_some_and(Line::is_blank) {
        renderer.lines.pop();
    }
    Document {
        lines: renderer.lines,
        diagrams: renderer.diagrams,
        text_blocks: renderer.text_blocks,
        links: renderer.links,
        headings: renderer.headings,
    }
}

impl<'a> Renderer<'a> {
    fn new(theme: &'a Theme, source: &'a str, width: usize, block_width: usize, diagram_options: DiagramOptions) -> Renderer<'a> {
        Renderer {
            theme,
            diagram_options,
            width: width.max(10),
            block_width: block_width.max(width).max(10),
            source,
            lines: Vec::new(),
            diagrams: Vec::new(),
            text_blocks: Vec::new(),
            text_block_depth: 0,
            pending_text_block: None,
            links: Vec::new(),
            headings: Vec::new(),
            slugger: Slugger::default(),
            inline: Vec::new(),
            style_stack: vec![theme.text],
            indents: Vec::new(),
            list_counters: Vec::new(),
            code: None,
            table: None,
            link_url: None,
            in_metadata: false,
            in_heading: None,
        }
    }

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

    fn available_block(&self) -> usize {
        self.block_width.saturating_sub(self.prefix_width()).max(8)
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
        line.append(&content);
        line.shrink();
        self.lines.push(line);
    }

    fn emit_blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.is_blank()) {
            let prefix = self.take_prefix();
            let mut line = Line::empty();
            for (text, style) in prefix.runs() {
                if text.trim().is_empty() {
                    continue;
                }
                line.push_str(text.trim_end(), style);
            }
            self.lines.push(line);
        }
    }

    /// 문단이든 표 칸이든 지금 쓰고 있는 인라인 버퍼에 조각을 넣는다.
    fn push_span(&mut self, span: Span) {
        match &mut self.table {
            Some(table) => table.current_cell.push(span),
            None => self.inline.push(span),
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

    fn handle(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(tag) => {
                self.enter_text_block(&tag, &range);
                self.start(tag);
            }
            Event::End(tag) => {
                self.end(tag);
                self.exit_text_block(tag);
            }
            Event::Text(text) => {
                if let Some((_, buffer)) = &mut self.code {
                    buffer.push_str(&text);
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(text) => {
                let style = self.style().merge(self.theme.code);
                // 줄바꿈 때 잘리지 않도록 여백은 NBSP로 붙인다.
                let shown = if self.theme.enabled { format!("\u{a0}{text}\u{a0}") } else { format!("`{text}`") };
                self.push_span(Span::new(shown, style));
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
                    self.push_span(Span::new(html.to_string(), style));
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
                self.push_span(Span::new(format!("[^{name}]"), style));
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
            Tag::BlockQuote(kind) => {
                self.flush_paragraph();
                self.emit_blank();
                self.indents.push(Indent {
                    first: Span::new("│ ", self.theme.quote_bar),
                    rest: Span::new("│ ", self.theme.quote_bar),
                    first_used: true,
                });
                self.push_style(self.theme.quote);
                // GFM 알림(`> [!NOTE]` 등)이면 라벨 줄을 먼저 그린다 — pulldown-cmark가 이미
                // `[!NOTE]` 마커 자체는 본문에서 제거해 주므로 라벨만 추가하면 된다
                // (markdown-gfm-alerts).
                if let Some(kind) = kind {
                    let (label, style) = alert_style(self.theme, kind);
                    self.emit(Line::single(label, style));
                }
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
                // 렌더 순서대로 쌓는다 — `links::locate_links`가 화면의 밑줄 구간과 순서대로
                // 짝짓는다(markdown-link-navigation).
                self.links.push(dest_url.to_string());
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
                // 이 헤딩이 그려질 첫 줄(원본 기준) + 슬러그를 기록한다(markdown-link-navigation).
                let heading_text: String = spans.iter().map(|s| s.text.as_str()).collect();
                self.headings.push((self.slugger.slug(&heading_text), self.lines.len()));
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
                while self.lines.last().is_some_and(|l| !l.is_blank() && l.text().trim().chars().all(|c| c == '│')) {
                    self.lines.pop();
                }
                self.indents.pop();
                self.emit_blank();
            }
            TagEnd::CodeBlock => {
                let Some((lang, buffer)) = self.code.take() else { return };
                self.emit_code_block(&lang, &buffer, true);
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
                    let available = self.available_block();
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
                    let buffer = match &self.table {
                        Some(table) => &table.current_cell,
                        None => &self.inline,
                    };
                    let shown: String = buffer.iter().rev().take(1).map(|s| s.text.clone()).collect();
                    if shown.trim() != url && !url.starts_with('#') {
                        let style = self.style().merge(self.theme.link_url);
                        self.push_span(Span::new(format!(" ({url})"), style));
                    }
                }
            }
            TagEnd::Image => {
                self.pop_style();
                if let Some(url) = self.link_url.take() {
                    let style = self.style().merge(self.theme.link_url);
                    self.push_span(Span::new(format!(" ({url})"), style));
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

    /// 최상위 블록 토글 대상 여부(markdown-source-view). 중첩 여부는 `text_block_depth`가
    /// 판단하므로 여기서는 종류만 가린다.
    fn is_text_block_tag(tag: &Tag) -> bool {
        matches!(
            tag,
            Tag::Paragraph
                | Tag::Heading { .. }
                | Tag::BlockQuote(_)
                | Tag::List(_)
                | Tag::Table(_)
                | Tag::FootnoteDefinition(_)
                | Tag::CodeBlock(_)
                | Tag::HtmlBlock
        )
    }

    fn is_text_block_tag_end(tag: &TagEnd) -> bool {
        matches!(
            tag,
            TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote(_)
                | TagEnd::List(_)
                | TagEnd::Table
                | TagEnd::FootnoteDefinition
                | TagEnd::CodeBlock
                | TagEnd::HtmlBlock
        )
    }

    /// 최상위(깊이 0→1) 블록만 시작 위치·원본 범위를 기억해 둔다. 중첩된 블록(리스트 항목 안
    /// 문단 등)은 깊이만 늘리고 별도로 기억하지 않아, 끝날 때 가장 바깥쪽 블록 하나로 묶인다.
    fn enter_text_block(&mut self, tag: &Tag, range: &Range<usize>) {
        if !Self::is_text_block_tag(tag) {
            return;
        }
        if self.text_block_depth == 0 {
            self.pending_text_block = Some(PendingTextBlock {
                start_line: self.lines.len(),
                source_range: range.clone(),
                is_code_block: matches!(tag, Tag::CodeBlock(_)),
                diagrams_before: self.diagrams.len(),
            });
        }
        self.text_block_depth += 1;
    }

    /// 깊이가 0으로 돌아오면 기억해 둔 시작점으로 `TextBlock`을 확정한다. 코드펜스가 다이어그램
    /// 으로 인식돼 이미 `self.diagrams`에 들어갔다면 중복으로 만들지 않는다.
    fn exit_text_block(&mut self, tag: TagEnd) {
        if !Self::is_text_block_tag_end(&tag) {
            return;
        }
        self.text_block_depth = self.text_block_depth.saturating_sub(1);
        if self.text_block_depth != 0 {
            return;
        }
        let Some(pending) = self.pending_text_block.take() else { return };
        if pending.is_code_block && self.diagrams.len() > pending.diagrams_before {
            return;
        }
        let mut end_line = self.lines.len();
        while end_line > pending.start_line && self.lines[end_line - 1].is_blank() {
            end_line -= 1;
        }
        if end_line > pending.start_line {
            self.text_blocks.push(TextBlock { start: pending.start_line, end: end_line, source: self.source[pending.source_range].to_string() });
        }
    }

    fn emit_code_block(&mut self, lang: &str, buffer: &str, allow_diagram: bool) {
        let available = self.available_block();
        if allow_diagram
            && let Some(language) = diagram::language_of_fence(lang)
            && let Some(lines) = diagram::render(language, buffer, self.theme, available.saturating_sub(2), self.diagram_options)
        {
            let start = self.lines.len();
            for line in lines {
                self.emit(line);
            }
            self.diagrams.push(DiagramBlock { start, end: self.lines.len(), lang: lang.to_string(), source: buffer.to_string() });
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

    /// 1.1~1.6(markdown-gfm-alerts) `alert_style`가 5종 전부에 라벨을 주고, Note·Caution만
    /// 같은 색이며 Caution만 굵다.
    #[test]
    fn alert_style_covers_all_kinds_and_bolds_caution() {
        let theme = Theme::dark();
        let (note_label, note_style) = alert_style(&theme, BlockQuoteKind::Note);
        let (tip_label, tip_style) = alert_style(&theme, BlockQuoteKind::Tip);
        let (important_label, important_style) = alert_style(&theme, BlockQuoteKind::Important);
        let (warning_label, warning_style) = alert_style(&theme, BlockQuoteKind::Warning);
        let (caution_label, caution_style) = alert_style(&theme, BlockQuoteKind::Caution);
        assert_eq!([note_label, tip_label, important_label, warning_label, caution_label], ["Note", "Tip", "Important", "Warning", "Caution"]);
        assert_eq!(note_style.fg, caution_style.fg, "팔레트가 4색뿐이라 다섯 번째는 첫 색을 재사용한다");
        assert!(!note_style.bold && caution_style.bold, "Caution만 굵어야 Note와 구분된다");
        let colors = [note_style.fg, tip_style.fg, important_style.fg, warning_style.fg];
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b, "Note/Tip/Important/Warning은 서로 다른 색이어야 한다: {colors:?}");
            }
        }
    }

    /// 3.1/3.2 GFM 알림은 라벨 줄이 먼저 나오고, 원본 `[!NOTE]` 마커는 본문에 남지 않는다.
    #[test]
    fn gfm_alert_shows_label_without_leaking_the_marker() {
        let out = plain("> [!NOTE]\n> 내용입니다.\n", 40);
        assert_eq!(out[0], "│ Note");
        assert_eq!(out[1], "│ 내용입니다.");
        assert!(!out.join("\n").contains("[!NOTE]"), "원본 마커가 본문에 남으면 안 된다: {out:?}");
    }

    /// 2.1 마커 없는 일반 인용문은 기존과 동일하다(회귀).
    #[test]
    fn plain_quote_is_unaffected_by_gfm_alerts() {
        let out = plain("> 그냥 인용문\n", 40);
        assert_eq!(out, vec!["│ 그냥 인용문"]);
    }

    /// 2.2 다섯 종류에 없는 마커는 일반 인용문으로 그려지고 마커 텍스트가 본문에 그대로 남는다.
    #[test]
    fn unsupported_alert_marker_falls_back_to_plain_quote() {
        let out = plain("> [!UNKNOWN]\n> 내용\n", 40);
        assert!(out.join(" ").contains("[!UNKNOWN]"), "{out:?}");
        assert!(!out.iter().any(|l| l == "│ UNKNOWN" || l == "│ Unknown"), "라벨로 오인되면 안 된다: {out:?}");
    }

    /// 1.1/2.1~2.3(markdown-link-navigation) `Document.links`/`headings`이 실제 등장 순서와
    /// 일치하고, 중복 헤딩은 GitHub 스타일로 슬러그가 갈린다.
    #[test]
    fn document_collects_links_and_heading_slugs_in_order() {
        let source = "# 시작\n\n[첫 링크](https://a.example) 그리고 [둘째](#끝)\n\n## 시작\n\n끝\n";
        let doc = render_document(source, &Theme::none(), 40, 40, DiagramOptions::default());
        assert_eq!(doc.links, vec!["https://a.example".to_string(), "#끝".to_string()]);
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(doc.headings[0].0, "시작");
        assert_eq!(doc.headings[1].0, "시작-1");
        assert!(doc.headings[0].1 < doc.headings[1].1, "헤딩 줄 번호는 등장 순서대로 증가해야 한다");
    }

    /// 2.1/2.3/2.4(markdown-source-view) 최상위 블록마다 원문이 그대로(마크업 포함) 슬라이스
    /// 되고, 서로 겹치지 않는다.
    #[test]
    fn text_blocks_collect_top_level_blocks_with_raw_source() {
        let source = "# 제목\n\n**굵게** 문단.\n\n- 목록1\n- 목록2\n";
        let doc = render_document(source, &Theme::none(), 40, 40, DiagramOptions::default());
        assert_eq!(doc.text_blocks.len(), 3, "{:?}", doc.text_blocks.iter().map(|b| &b.source).collect::<Vec<_>>());
        assert_eq!(doc.text_blocks[0].source, "# 제목\n");
        assert_eq!(doc.text_blocks[1].source, "**굵게** 문단.\n");
        assert_eq!(doc.text_blocks[2].source, "- 목록1\n- 목록2\n");
        for pair in doc.text_blocks.windows(2) {
            assert!(pair[0].end <= pair[1].start, "블록이 겹치면 안 된다: {:?}", doc.text_blocks.iter().map(|b| (b.start, b.end)).collect::<Vec<_>>());
        }
    }

    /// 2.1/2.4 중첩된 블록(리스트 항목 안 하위 리스트)은 가장 바깥쪽 블록 하나로 묶인다.
    #[test]
    fn text_blocks_merge_nested_blocks_into_outer() {
        let source = "- 항목1\n  - 하위1\n  - 하위2\n- 항목2\n";
        let doc = render_document(source, &Theme::none(), 40, 40, DiagramOptions::default());
        assert_eq!(doc.text_blocks.len(), 1, "{:?}", doc.text_blocks.iter().map(|b| &b.source).collect::<Vec<_>>());
        assert_eq!(doc.text_blocks[0].source, source);
    }

    /// 2.1 다이어그램으로 인식된 코드펜스는 `diagrams`에만 들어가고 `text_blocks`에는 중복으로
    /// 들어가지 않는다. 인식 안 되는 일반 코드펜스는 `text_blocks`에 들어간다.
    #[test]
    fn text_blocks_exclude_recognized_diagrams_but_include_plain_code_fences() {
        let source = "```mermaid\nflowchart TB\n A --> B\n```\n\n```rust\nfn f() {}\n```\n";
        let doc = render_document(source, &Theme::none(), 40, 40, DiagramOptions::default());
        assert_eq!(doc.diagrams.len(), 1, "mermaid 펜스는 다이어그램으로 인식돼야 한다");
        assert_eq!(doc.text_blocks.len(), 1, "{:?}", doc.text_blocks.iter().map(|b| &b.source).collect::<Vec<_>>());
        assert!(doc.text_blocks[0].source.contains("fn f() {}"), "{:?}", doc.text_blocks[0].source);
        assert!(!doc.text_blocks[0].source.contains("flowchart"), "다이어그램 원문이 중복되면 안 된다");
    }

    /// 3.1 원문 하이라이팅: 헤딩·강조·링크·인용·목록·코드펜스가 섞인 원문에서 문자는 그대로
    /// 보존되고, 각 요소가 대응하는 테마 색을 받는다.
    #[test]
    fn render_source_text_preserves_characters_and_colors_syntax_elements() {
        let source = "# 제목\n\n**굵게** [링크](https://x.com)\n\n> 인용\n\n- 목록1\n1. 순서목록\n\n```rust\ncode\n```\n";
        let theme = Theme::dark();
        let lines = render_source_text(source, &theme, 80);
        let plain: String = lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert_eq!(plain, source.trim_end_matches('\n'), "문자는 원문 그대로 보존돼야 한다");

        let heading_line = &lines[0];
        assert!(heading_line.runs().any(|(t, s)| t.contains('#') && s == theme.heading[0]), "{heading_line:?}");

        let strong_line = lines.iter().find(|l| l.plain().contains("굵게")).unwrap();
        assert!(strong_line.runs().any(|(t, s)| t.contains("**굵게**") && s == theme.strong), "{strong_line:?}");
        assert!(strong_line.runs().any(|(t, s)| t.contains("[링크]") && s == theme.link), "{strong_line:?}");

        let quote_line = lines.iter().find(|l| l.plain().contains("인용")).unwrap();
        assert!(quote_line.runs().any(|(t, s)| t.contains('>') && s == theme.quote), "{quote_line:?}");

        let bullet_line = lines.iter().find(|l| l.plain().starts_with("- 목록1")).unwrap();
        assert!(bullet_line.runs().any(|(t, s)| t == "- " && s == theme.bullet), "{bullet_line:?}");
        let ordered_line = lines.iter().find(|l| l.plain().starts_with("1. 순서목록")).unwrap();
        assert!(ordered_line.runs().any(|(t, s)| t == "1. " && s == theme.bullet), "{ordered_line:?}");

        let fence_open = lines.iter().find(|l| l.plain() == "```rust").unwrap();
        assert!(fence_open.runs().any(|(t, s)| t == "```rust" && s == theme.code_border), "{fence_open:?}");
        let fence_close = lines.iter().find(|l| l.plain() == "```").unwrap();
        assert!(fence_close.runs().any(|(t, s)| t == "```" && s == theme.code_border), "{fence_close:?}");
        let code_content = lines.iter().find(|l| l.plain() == "code").unwrap();
        assert!(code_content.runs().all(|(_, s)| s == Style::PLAIN), "코드 내용은 색이 없어야 한다: {code_content:?}");
    }

    /// 3.2 `--style none`에서도 문자 구성이 안 바뀐다(색만 안 보일 뿐 문법 마커 문자는 원문
    /// 그대로 있어 구분 자체는 유지된다).
    #[test]
    fn render_source_text_keeps_characters_unchanged_when_style_is_none() {
        let source = "# 제목\n\n**굵게** 문단\n";
        let colored = render_source_text(source, &Theme::dark(), 80);
        let plain = render_source_text(source, &Theme::none(), 80);
        let colored_text: Vec<String> = colored.iter().map(Line::plain).collect();
        let plain_text: Vec<String> = plain.iter().map(Line::plain).collect();
        assert_eq!(colored_text, plain_text, "테마와 무관하게 글자 구성은 같아야 한다");
    }

    /// 3.3 코드펜스 안쪽은 원본 그대로, 펜스 기호(```)만 구분 표시된다 — 위 종합 테스트에서
    /// 이미 확인했지만, 언어 정보 없는 순수 ``` 펜스도 문제없는지 별도로 확인한다.
    #[test]
    fn render_source_text_handles_fence_without_language() {
        let source = "```\nplain code\n```\n";
        let lines = render_source_text(source, &Theme::dark(), 80);
        let plain: String = lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert_eq!(plain, "```\nplain code\n```");
    }

    /// 빈 문자열도 패닉 없이 최소 한 줄을 돌려준다.
    #[test]
    fn render_source_text_handles_empty_input_without_panicking() {
        let lines = render_source_text("", &Theme::dark(), 80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain().is_empty());
    }

    /// 1.7 `--style none`에서도 라벨 텍스트만으로 다섯 종류가 서로 구분된다.
    #[test]
    fn alert_labels_are_distinct_without_color() {
        let source = "> [!NOTE]\n> a\n\n> [!TIP]\n> b\n\n> [!IMPORTANT]\n> c\n\n> [!WARNING]\n> d\n\n> [!CAUTION]\n> e\n";
        let out = plain(source, 40);
        for label in ["Note", "Tip", "Important", "Warning", "Caution"] {
            assert!(out.contains(&format!("│ {label}")), "{label} 라벨이 있어야 한다: {out:?}");
        }
    }
}
