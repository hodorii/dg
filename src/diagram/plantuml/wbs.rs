//! PlantUML `@startwbs`(작업 분류 체계) 파서.
//!
//! OrgMode 깊이 표기(`*`/`**`/`***`, …)만 지원한다. 좌우로 동시에 갈라지는 산술
//! 표기(`+`/`-`의 방향 의미)·`<`/`>` 방향 강제·노드 간 화살표·인라인 색상·
//! `<style>` 블록은 구조를 깨지 않는 선에서 건너뛴다(plantuml-wbs 요구사항 3.3).

use super::text::clean_lines;
use crate::diagram::ir::{Edge, Graph, Shape};

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    // (깊이, 노드 인덱스) — 마지막 항목이 "현재까지 열려 있는 가장 안쪽 조상"이다.
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut counter = 0usize;
    let mut lines = clean_lines(source).into_iter();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // 깊이 표시 글자가 없는 줄(노드 간 화살표, <style> 블록 내부 등)은 건너뛴다 —
        // 범위 밖 구문이 구조를 깨지 않게 하는 가장 단순한 방법이다.
        let depth = trimmed.chars().take_while(|c| matches!(c, '*' | '+' | '-')).count();
        if depth == 0 {
            continue;
        }
        let rest = trimmed[depth..].trim_start();
        let (plain, rest) = match rest.strip_prefix('_') {
            Some(r) => (true, r.trim_start()),
            None => (false, rest),
        };
        let rest = strip_inline_color(rest);
        let text = if let Some(body) = rest.strip_prefix(':') {
            collect_multiline(body, &mut lines)
        } else {
            rest.trim().to_string()
        };
        if text.is_empty() {
            continue;
        }
        while let Some(&(top_depth, _)) = stack.last() {
            if top_depth >= depth {
                stack.pop();
            } else {
                break;
            }
        }
        counter += 1;
        let id = format!("wbs{counter}");
        let shape = if plain { Shape::Plain } else { Shape::Rect };
        let node = graph.intern(&id, &text, shape, None);
        if let Some(&(_, parent)) = stack.last() {
            graph.add_edge(Edge { from: parent, to: node, ..Edge::default() });
        }
        stack.push((depth, node));
    }
    graph
}

/// `[#Color]` 같은 인라인 꾸밈은 값을 반영하지 않고 건너뛴다(요구사항 3.3).
fn strip_inline_color(rest: &str) -> &str {
    if let Some(r) = rest.strip_prefix('[')
        && let Some(end) = r.find(']')
    {
        return r[end + 1..].trim_start();
    }
    rest
}

/// `:`로 시작해 `;`로 끝나는 여러 줄 본문을 한 문자열로 이어 붙인다(줄바꿈으로
/// 구분 — `Graph::intern`이 `\n` 기준으로 노드 섹션을 나눈다).
fn collect_multiline(first: &str, lines: &mut std::vec::IntoIter<String>) -> String {
    let mut parts = Vec::new();
    let mut current = first.to_string();
    loop {
        if let Some(end) = current.find(';') {
            parts.push(current[..end].trim().to_string());
            break;
        }
        parts.push(current.trim().to_string());
        match lines.next() {
            Some(next) => current = next,
            None => break,
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(g: &Graph) -> Vec<String> {
        g.nodes.iter().map(|n| n.sections[0].join("\n")).collect()
    }

    #[test]
    fn depth_marks_parent_child_edges() {
        let g = parse("@startwbs\n* Root\n** Child A\n*** Grandchild\n** Child B\n@endwbs\n");
        assert_eq!(labels(&g), vec!["Root", "Child A", "Grandchild", "Child B"]);
        assert_eq!(g.edges.len(), 3);
        assert_eq!((g.edges[0].from, g.edges[0].to), (0, 1)); // Root -> Child A
        assert_eq!((g.edges[1].from, g.edges[1].to), (1, 2)); // Child A -> Grandchild
        assert_eq!((g.edges[2].from, g.edges[2].to), (0, 3)); // Root -> Child B
    }

    #[test]
    fn multiple_top_level_roots_stay_independent() {
        let g = parse("@startwbs\n* Root A\n** Child\n* Root B\n@endwbs\n");
        assert_eq!(labels(&g), vec!["Root A", "Child", "Root B"]);
        // Root A -> Child 하나뿐 — Root B는 어디에도 안 붙는다(독립 뿌리).
        assert_eq!(g.edges.len(), 1);
        assert_eq!((g.edges[0].from, g.edges[0].to), (0, 1));
    }

    #[test]
    fn skipped_depth_attaches_to_the_most_recent_item() {
        let g = parse("@startwbs\n* Root\n***** Deep\n@endwbs\n");
        assert_eq!(labels(&g), vec!["Root", "Deep"]);
        assert_eq!((g.edges[0].from, g.edges[0].to), (0, 1));
    }

    #[test]
    fn multiline_body_joins_with_newline() {
        let g = parse("@startwbs\n*: line one\nline two;\n@endwbs\n");
        assert_eq!(labels(&g), vec!["line one\nline two"]);
    }

    #[test]
    fn underscore_suffix_uses_plain_shape() {
        let g = parse("@startwbs\n* Root\n**_ Leaf\n@endwbs\n");
        assert_eq!(g.nodes[1].shape, Shape::Plain);
        assert_eq!(g.nodes[0].shape, Shape::Rect);
    }

    #[test]
    fn out_of_scope_syntax_is_ignored_without_breaking_structure() {
        let g = parse("@startwbs\n*[#red] Root\n** Child\nRoot --> Child\n@endwbs\n");
        assert_eq!(labels(&g), vec!["Root", "Child"]);
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn empty_document_yields_no_nodes() {
        let g = parse("@startwbs\n@endwbs\n");
        assert!(g.nodes.is_empty());
    }
}
