//! mermaid `classDiagram` 파서.

use super::text::{clean_lines, label};
use crate::diagram::ir::{Direction, Edge, Graph, LineKind, Marker, Shape};

const RELATIONS: &[&str] = &["<|..", "..|>", "<|--", "--|>", "*--", "--*", "o--", "--o", "<--", "-->", "<..", "..>", "--", ".."];

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut open_class: Option<usize> = None;
    let mut group_stack: Vec<usize> = Vec::new();
    for line in clean_lines(source) {
        let trimmed = line.trim();
        if let Some(index) = open_class {
            if trimmed.starts_with('}') {
                open_class = None;
            } else {
                add_member(&mut graph, index, trimmed);
            }
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("classdiagram") {
            continue;
        }
        if let Some(direction) = lower.strip_prefix("direction") {
            graph.direction = Some(if direction.trim().starts_with('l') || direction.trim().starts_with('r') {
                Direction::LeftRight
            } else {
                Direction::TopDown
            });
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "namespace") {
            let name = rest.trim_end_matches('{').trim();
            let group = graph.add_group(name, group_stack.last().copied());
            group_stack.push(group);
            continue;
        }
        if trimmed == "}" {
            group_stack.pop();
            continue;
        }
        if lower.starts_with("note") || lower.starts_with("style") || lower.starts_with("classdef") || lower.starts_with("cssclass") || lower.starts_with("click") || lower.starts_with("callback") || lower.starts_with("link") {
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "class") {
            let (name, remainder) = split_name(rest);
            let index = declare(&mut graph, name, group_stack.last().copied());
            let remainder = remainder.trim();
            if let Some(text) = remainder.strip_prefix('[') {
                let text = text.trim_end_matches('{').trim().trim_end_matches(']');
                graph.set_label(index, &label(text));
            }
            if remainder.ends_with('{') {
                open_class = Some(index);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("<<") {
            // `<<interface>> Name`
            if let Some(end) = rest.find(">>") {
                let stereotype = rest[..end].trim();
                let name = rest[end + 2..].trim();
                if !name.is_empty() {
                    let index = declare(&mut graph, name, group_stack.last().copied());
                    add_stereotype(&mut graph, index, stereotype);
                }
                continue;
            }
        }
        let member_line = trimmed.split_once(" : ").or_else(|| trimmed.split_once(':').filter(|(n, _)| !n.contains(' ')));
        if let Some((name, member)) = member_line
            && !RELATIONS.iter().any(|r| name.contains(r))
        {
            let index = declare(&mut graph, name.trim(), group_stack.last().copied());
            add_member(&mut graph, index, member.trim());
            continue;
        }
        parse_relation(&mut graph, trimmed, group_stack.last().copied());
    }
    graph
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.starts_with(char::is_whitespace) { Some(rest.trim()) } else { None }
}

/// `Name~T~`나 `Name[...]`에서 이름과 나머지를 나눈다.
fn split_name(text: &str) -> (&str, &str) {
    let end = text.find(|c: char| c.is_whitespace() || c == '[' || c == '{').unwrap_or(text.len());
    (&text[..end], &text[end..])
}

fn display_name(raw: &str) -> String {
    // 제네릭 `List~T~` → `List<T>`
    let mut out = String::new();
    let mut open = false;
    for c in raw.chars() {
        if c == '~' {
            out.push(if open { '>' } else { '<' });
            open = !open;
        } else {
            out.push(c);
        }
    }
    out
}

fn declare(graph: &mut Graph, name: &str, group: Option<usize>) -> usize {
    let index = graph.intern(name, &display_name(name), Shape::Rect, group);
    if graph.nodes[index].group.is_none() {
        graph.nodes[index].group = group;
    }
    index
}

fn add_stereotype(graph: &mut Graph, index: usize, stereotype: &str) {
    let text = format!("«{stereotype}»");
    let name_section = &mut graph.nodes[index].sections[0];
    if !name_section.contains(&text) {
        name_section.insert(0, text);
    }
}

fn add_member(graph: &mut Graph, index: usize, member: &str) {
    let member = member.trim();
    if member.is_empty() {
        return;
    }
    if let Some(rest) = member.strip_prefix("<<") {
        if let Some(end) = rest.find(">>") {
            add_stereotype(graph, index, rest[..end].trim());
        }
        return;
    }
    let text = display_name(member.trim_end_matches(['$', '*']));
    let is_method = text.contains('(');
    let node = &mut graph.nodes[index];
    while node.sections.len() < 3 {
        node.sections.push(Vec::new());
    }
    node.sections[if is_method { 2 } else { 1 }].push(text);
}

fn parse_relation(graph: &mut Graph, line: &str, group: Option<usize>) {
    // 가장 앞에 나오는 관계 기호(같은 위치면 긴 것)
    let mut best: Option<(usize, &str)> = None;
    for relation in RELATIONS {
        if let Some(position) = line.find(relation) {
            match best {
                Some((p, r)) if p < position || (p == position && r.len() >= relation.len()) => {}
                _ => best = Some((position, relation)),
            }
        }
    }
    let Some((position, relation)) = best else { return };
    let left = line[..position].trim();
    let right = &line[position + relation.len()..];
    let (right, text) = match right.split_once(':') {
        Some((r, t)) => (r.trim(), label(t)),
        None => (right.trim(), String::new()),
    };
    let (left_name, left_multiplicity) = split_multiplicity(left, true);
    let (right_name, right_multiplicity) = split_multiplicity(right, false);
    if left_name.is_empty() || right_name.is_empty() {
        return;
    }
    let a = declare(graph, &left_name, group);
    let b = declare(graph, &right_name, group);
    let dashed = relation.contains('.');
    let kind = if dashed { LineKind::Dashed } else { LineKind::Solid };
    let (from, to, tail, head, swap) = match relation {
        "<|--" | "<|.." => (a, b, Marker::Triangle, Marker::None, false),
        "--|>" | "..|>" => (b, a, Marker::Triangle, Marker::None, true),
        "*--" => (a, b, Marker::DiamondFilled, Marker::None, false),
        "--*" => (b, a, Marker::DiamondFilled, Marker::None, true),
        "o--" => (a, b, Marker::DiamondOpen, Marker::None, false),
        "--o" => (b, a, Marker::DiamondOpen, Marker::None, true),
        "-->" | "..>" => (a, b, Marker::None, Marker::Arrow, false),
        "<--" | "<.." => (b, a, Marker::None, Marker::Arrow, true),
        _ => (a, b, Marker::None, Marker::None, false),
    };
    let (tail_label, head_label) = if swap { (right_multiplicity, left_multiplicity) } else { (left_multiplicity, right_multiplicity) };
    graph.add_edge(Edge { from, to, label: text, tail_label, head_label, kind, tail, head });
}

/// `A "1"` 또는 `"*" B`에서 (이름, 다중성).
fn split_multiplicity(text: &str, name_first: bool) -> (String, String) {
    let text = text.trim();
    if let Some(start) = text.find('"') {
        let end = text[start + 1..].find('"').map(|e| start + 1 + e).unwrap_or(text.len());
        let multiplicity = text[start + 1..end].to_string();
        let name = if name_first { text[..start].trim() } else { text[(end + 1).min(text.len())..].trim() };
        return (name.to_string(), multiplicity);
    }
    (text.to_string(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classes_members_and_relations() {
        let g = parse("classDiagram\n class Animal {\n  +String name\n  +speak()\n }\n Animal <|-- Dog\n Dog \"1\" *-- \"*\" Leg : has\n <<interface>> Animal\n Dog : +bark()\n");
        assert_eq!(g.nodes[0].sections[0], vec!["«interface»", "Animal"]);
        assert_eq!(g.nodes[0].sections[1], vec!["+String name"]);
        assert_eq!(g.nodes[0].sections[2], vec!["+speak()"]);
        assert_eq!(g.edges[0].tail, Marker::Triangle);
        assert_eq!(g.edges[1].tail, Marker::DiamondFilled);
        assert_eq!(g.edges[1].tail_label, "1");
        assert_eq!(g.edges[1].head_label, "*");
        assert_eq!(g.edges[1].label, "has");
        assert_eq!(g.nodes[1].sections[2], vec!["+bark()"]);
    }
}
