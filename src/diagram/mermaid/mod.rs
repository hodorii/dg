//! mermaid 문법 파서 모음.

pub mod class;
pub mod er;
pub mod flow;
pub mod sequence;
pub mod state;
pub mod text;

use crate::diagram::layout;
use crate::diagram::options::DiagramOptions;
use crate::line::Line;
use crate::style::Theme;

/// 첫 지시어로 다이어그램 종류를 판별한다.
pub fn kind_of(source: &str) -> Option<&'static str> {
    for line in text::clean_lines(source) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("---") || trimmed.starts_with("title") {
            continue;
        }
        let keyword = text::keyword(trimmed).to_ascii_lowercase();
        let keyword = keyword.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
        return match keyword {
            "flowchart" | "flowchart-v2" | "graph" => Some("flowchart"),
            "sequencediagram" => Some("sequence"),
            "statediagram" | "statediagram-v2" => Some("state"),
            "erdiagram" => Some("er"),
            "classdiagram" | "classdiagram-v2" => Some("class"),
            _ => None,
        };
    }
    None
}

pub fn render(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let kind = kind_of(source)?;
    let graph_with_options = |mut graph: crate::diagram::ir::Graph| {
        options.apply_to_graph(&mut graph);
        graph
    };
    let body = match kind {
        "flowchart" => layout::graph::render(&graph_with_options(flow::parse(source)), theme, width)?,
        "state" => layout::graph::render(&graph_with_options(state::parse(source)), theme, width)?,
        "er" => layout::graph::render(&graph_with_options(er::parse(source)), theme, width)?,
        "class" => layout::graph::render(&graph_with_options(class::parse(source)), theme, width)?,
        "sequence" => layout::sequence::render(&sequence::parse(source), theme, width)?,
        _ => return None,
    };
    Some((kind, body))
}
