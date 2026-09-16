//! mermaid `erDiagram` 파서.

use super::text::{clean_lines, label};
use crate::diagram::ir::{Edge, Graph, LineKind, Marker, Shape};

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut open_entity: Option<usize> = None;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        if let Some(index) = open_entity {
            if trimmed.starts_with('}') {
                open_entity = None;
            } else {
                add_attribute(&mut graph, index, trimmed);
            }
            continue;
        }
        if trimmed.to_ascii_lowercase().starts_with("erdiagram") || trimmed.starts_with("direction") {
            continue;
        }
        if let Some(head) = trimmed.strip_suffix('{') {
            let index = declare(&mut graph, head.trim());
            open_entity = Some(index);
            continue;
        }
        parse_relation(&mut graph, trimmed);
    }
    graph
}

fn declare(graph: &mut Graph, raw: &str) -> usize {
    // `CUSTOMER["Customer"]`
    let (id, text) = match raw.find('[') {
        Some(p) => (raw[..p].trim(), label(raw[p + 1..].trim_end_matches(']'))),
        None => (raw.trim(), label(raw.trim())),
    };
    graph.intern(id, &text, Shape::Rect, None)
}

/// `string name PK "comment"` → `name : string [PK]`
fn add_attribute(graph: &mut Graph, index: usize, raw: &str) {
    let without_comment = match raw.find('"') {
        Some(p) => raw[..p].trim(),
        None => raw.trim(),
    };
    let mut words = without_comment.split_whitespace();
    let Some(type_name) = words.next() else { return };
    let Some(name) = words.next() else { return };
    let keys: Vec<&str> = words.collect();
    let mut text = format!("{name} : {type_name}");
    if !keys.is_empty() {
        text.push_str(&format!(" [{}]", keys.join(",")));
    }
    let node = &mut graph.nodes[index];
    if node.sections.len() < 2 {
        node.sections.push(Vec::new());
    }
    node.sections[1].push(text);
}

fn cardinality(token: &str) -> &'static str {
    match token {
        "||" => "1",
        "|o" | "o|" => "0..1",
        "}|" | "|{" => "1..N",
        "}o" | "o{" => "0..N",
        _ => "",
    }
}

fn parse_relation(graph: &mut Graph, line: &str) {
    let (body, text) = match line.split_once(':') {
        Some((b, t)) => (b, label(t)),
        None => (line, String::new()),
    };
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.len() < 3 {
        return;
    }
    let relation = words[1];
    let Some(line_start) = relation.find("--").or_else(|| relation.find("..")) else { return };
    let dashed = relation[line_start..].starts_with("..");
    let (left, right) = (&relation[..line_start], &relation[line_start + 2..]);
    let a = declare(graph, words[0]);
    let b = declare(graph, &words[2..].join(" "));
    graph.add_edge(Edge {
        from: a,
        to: b,
        label: text,
        tail_label: cardinality(left).to_string(),
        head_label: cardinality(right).to_string(),
        kind: if dashed { LineKind::Dashed } else { LineKind::Solid },
        tail: Marker::None,
        head: Marker::None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities_and_relations() {
        let g = parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER {\n  string name PK\n  int age\n }\n");
        assert_eq!(g.nodes[0].sections[1], vec!["name : string [PK]", "age : int"]);
        assert_eq!(g.edges[0].tail_label, "1");
        assert_eq!(g.edges[0].head_label, "0..N");
        assert_eq!(g.edges[0].label, "places");
    }
}
