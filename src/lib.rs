//! 마크다운과 mermaid·PlantUML 다이어그램을 터미널 문자 그림으로 렌더링하는 라이브러리.
//!
//! 렌더링 결과는 [`Line`](line::Line)(스타일 구간이 붙은 한 줄)의 목록이다. ANSI 문자열이 필요하면
//! [`Line::to_ansi`](line::Line::to_ansi), 색 없는 평문이 필요하면 [`Line::text`](line::Line::text)를 쓴다.
//!
//! ```
//! use dg::{render_markdown, RenderOptions};
//!
//! let options = RenderOptions { width: 60, ..RenderOptions::default() };
//! let lines = render_markdown("# 제목\n\n```mermaid\nflowchart LR\n A --> B\n```\n", &options);
//! let plain: Vec<&str> = lines.iter().map(|l| l.text()).collect();
//! assert_eq!(plain[0], "제목");
//! assert!(plain.iter().any(|l| l.contains("──▶")));
//! ```
//!
//! 다이어그램만 그리려면 [`render_diagram`], 마크다운 문서에서 다이어그램 블록 위치까지 필요하면
//! [`markdown::render_document`]를 직접 쓴다. 페이저(TUI)는 `cli` 피처에서만 들어온다.

#![allow(clippy::too_many_arguments)]

pub mod diagram;
pub mod line;
pub mod markdown;
#[cfg(feature = "cli")]
pub mod pager;
pub mod style;
pub mod text;
#[cfg(feature = "cli")]
pub mod watch;

pub use diagram::{DiagramOptions, ErNotation, Language};
pub use line::{Line, Span};
pub use style::{Style, Theme};

/// 마크다운 렌더링 옵션.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    /// 문단 줄바꿈 폭.
    pub width: usize,
    /// 다이어그램·표·코드블록에 허용하는 폭. `None`이면 `width`와 같다.
    pub block_width: Option<usize>,
    /// 색 테마. 색 없이 문자만 필요하면 [`Theme::none`].
    pub theme: Theme,
    pub diagram: DiagramOptions,
    /// [`render_diagram`]이 첫 줄에 `◈ mermaid · flowchart ───` 캡션을 붙일지.
    pub diagram_caption: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { width: 80, block_width: None, theme: Theme::none(), diagram: DiagramOptions::default(), diagram_caption: true }
    }
}

/// 마크다운 문서를 줄 목록으로 렌더링한다. 코드 펜스의 mermaid·plantuml은 그림으로 바뀐다.
pub fn render_markdown(source: &str, options: &RenderOptions) -> Vec<Line> {
    markdown::render_document(source, &options.theme, options.width, options.block_width.unwrap_or(options.width), options.diagram).lines
}

/// 다이어그램 소스 하나를 그린다. 지원하지 않는 종류이거나 폭에 들어가지 않으면 `None`.
/// `language`가 `None`이면 본문으로 mermaid/PlantUML을 추정한다.
pub fn render_diagram(source: &str, language: Option<Language>, options: &RenderOptions) -> Option<Vec<Line>> {
    let language = language.or_else(|| diagram::language_of_source(None, source))?;
    let width = options.block_width.unwrap_or(options.width);
    if options.diagram_caption {
        diagram::render(language, source, &options.theme, width, options.diagram)
    } else {
        diagram::render_body(language, source, &options.theme, width, options.diagram).map(|(_, lines)| lines)
    }
}

/// 줄 목록을 줄바꿈으로 이은 ANSI 문자열.
pub fn to_ansi(lines: &[Line], theme: &Theme) -> String {
    lines.iter().map(|l| l.to_ansi(theme)).collect::<Vec<_>>().join("\n")
}

/// 줄 목록을 줄바꿈으로 이은 평문.
pub fn to_text(lines: &[Line]) -> String {
    lines.iter().map(Line::text).collect::<Vec<_>>().join("\n")
}
