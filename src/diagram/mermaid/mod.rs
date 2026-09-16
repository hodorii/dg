//! mermaid 문법 파서 모음.

pub mod block;
pub mod class;
pub mod er;
pub mod flow;
pub mod gantt;
pub mod gitgraph;
pub mod pie;
pub mod quadrant;
pub mod sequence;
pub mod state;
pub mod text;
pub mod xychart;

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
            "gitgraph" => Some("gitgraph"),
            "block-beta" => Some("block"),
            // 진짜 문법은 `pie showData title …`이라 첫 낱말만 본다.
            "pie" => Some("pie"),
            "xychart-beta" | "xychart" => Some("xychart"),
            "quadrantchart" => Some("quadrant"),
            "gantt" => Some("gantt"),
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
        "gitgraph" => layout::gitgraph::render(&gitgraph::parse(source), theme, width)?,
        "block" => layout::block::render(&block::parse(source), theme, width)?,
        "pie" => layout::pie::render(&pie::parse(source), theme, width)?,
        "xychart" => layout::xychart::render(&xychart::parse(source), theme, width)?,
        "quadrant" => layout::quadrant::render(&quadrant::parse(source), theme, width)?,
        "gantt" => layout::gantt::render(&gantt::parse(source), theme, width)?,
        _ => return None,
    };
    Some((kind, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_gitgraph_without_disturbing_other_kinds() {
        assert_eq!(kind_of("gitGraph\n commit\n"), Some("gitgraph"));
        assert_eq!(kind_of("gitGraph:\n  commit\n"), Some("gitgraph"));
        assert_eq!(kind_of("%% note\n\n   gitgraph LR:\n commit\n"), Some("gitgraph"));
        assert_eq!(kind_of("flowchart TB\n A --> B"), Some("flowchart"));
        assert_eq!(kind_of("sequenceDiagram\n A->>B: x"), Some("sequence"));
        assert_eq!(kind_of("classDiagram\n class A"), Some("class"));
        assert_eq!(kind_of("git log --oneline"), None);
    }

    #[test]
    fn detects_block_beta_without_disturbing_other_kinds() {
        assert_eq!(kind_of("block-beta\n columns 2\n a b\n"), Some("block"));
        assert_eq!(kind_of("---\ntitle: x\n---\nblock-beta\n a\n"), Some("block"));
        assert_eq!(kind_of("flowchart TB\n block --> a\n"), Some("flowchart"));
        assert_eq!(kind_of("block\n a b\n"), None);
    }

    #[test]
    fn renders_block_beta_end_to_end() {
        let (kind, body) = render("block-beta\n columns 2\n a[\"A\"] b\n a --> b\n", &Theme::none(), 80, DiagramOptions::default()).unwrap();
        assert_eq!(kind, "block");
        let text: String = body.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains('A') && text.contains('b') && text.contains('▶'), "{text}");
    }

    // 1.1 첫 지시어가 `pie`면 다른 종류를 건드리지 않고 pie로 판별된다.
    #[test]
    fn detects_pie_without_disturbing_other_kinds() {
        assert_eq!(kind_of("pie\n \"A\" : 1\n"), Some("pie"));
        assert_eq!(kind_of("pie showData title 제목\n \"A\" : 1\n"), Some("pie"));
        assert_eq!(kind_of("PIE showData\n \"A\" : 1\n"), Some("pie"));
        assert_eq!(kind_of("%% 주석\n\n  pie:\n \"A\" : 1\n"), Some("pie"));
        assert_eq!(kind_of("flowchart TB\n pie --> chart"), Some("flowchart"));
        assert_eq!(kind_of("piechart\n \"A\" : 1\n"), None);
    }

    // 1.1·2.1 판별부터 그리기까지 한 번에 이어진다.
    #[test]
    fn renders_pie_end_to_end() {
        let (kind, body) =
            render("pie showData title 반려동물\n \"Dogs\" : 50\n \"Cats\" : 50\n", &Theme::none(), 60, DiagramOptions::default())
                .unwrap();
        assert_eq!(kind, "pie");
        let text: String = body.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains("반려동물") && text.contains("Dogs") && text.contains("50 (50.0%)"), "{text}");
    }

    // 1.1 첫 지시어가 xychart-beta(또는 xychart)면 xy 차트로 판별한다.
    #[test]
    fn detects_xychart_without_disturbing_other_kinds() {
        assert_eq!(kind_of("xychart-beta\n bar [1, 2]\n"), Some("xychart"));
        assert_eq!(kind_of("xychart\n bar [1, 2]\n"), Some("xychart"));
        assert_eq!(kind_of("%% note\n\n  XYChart-Beta\n line [1]\n"), Some("xychart"));
        assert_eq!(kind_of("flowchart TB\n A --> B"), Some("flowchart"));
        assert_eq!(kind_of("xy chart\n"), None);
    }

    #[test]
    fn renders_xychart_end_to_end() {
        let source = "xychart-beta\n x-axis [a, b, c]\n y-axis \"값\" 0 --> 30\n bar [10, 30, 20]\n";
        let (kind, body) = render(source, &Theme::none(), 60, DiagramOptions::default()).unwrap();
        assert_eq!(kind, "xychart");
        let text: String = body.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains('█') && text.contains("20") && text.contains('a'), "{text}");
    }

    // 1.1 첫 지시어가 quadrantChart면 사분면 차트로 판별한다.
    #[test]
    fn detects_quadrant_chart_without_disturbing_other_kinds() {
        assert_eq!(kind_of("quadrantChart\n A: [0.1, 0.2]\n"), Some("quadrant"));
        assert_eq!(kind_of("%% 주석\n\n  quadrantchart\n"), Some("quadrant"));
        assert_eq!(kind_of("flowchart TB\n A --> B"), Some("flowchart"));
        assert_eq!(kind_of("quadrant\n A: [0, 0]\n"), None);
    }

    // 1.1 판별한 종류로 실제 그림까지 나온다.
    #[test]
    fn renders_quadrant_chart_end_to_end() {
        let source = "quadrantChart\n title 견주기\n x-axis 적음 --> 많음\n quadrant-1 확장\n 캠페인 A: [0.3, 0.6]\n";
        let (kind, body) = render(source, &Theme::none(), 60, DiagramOptions::default()).unwrap();
        assert_eq!(kind, "quadrant");
        let text: String = body.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains("견주기") && text.contains("확장") && text.contains("캠페인 A"), "{text}");
        assert!(text.contains('┼') && text.contains('●'), "{text}");
    }
}
