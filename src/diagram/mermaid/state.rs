//! mermaid `stateDiagram` 파서. 그래프 IR로 만든다.

use super::text::{clean_lines, label};
use crate::diagram::ir::{Edge, Graph, Marker, Shape};
use crate::diagram::options::parse_direction;

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut group_stack: Vec<usize> = Vec::new();
    let mut in_note = false;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if in_note {
            if lower.starts_with("end note") {
                in_note = false;
            }
            continue;
        }
        if lower.starts_with("statediagram") {
            continue;
        }
        if let Some(rest) = lower.strip_prefix("direction") {
            if let Some((direction, reversed)) = parse_direction(rest) {
                graph.direction = Some(direction);
                graph.direction_reversed = reversed;
            }
            continue;
        }
        if lower.starts_with("note") {
            if !trimmed.contains(':') {
                in_note = true;
            }
            continue;
        }
        if lower.starts_with("classdef") || lower.starts_with("class ") || lower.starts_with("style") {
            continue;
        }
        if trimmed == "}" {
            group_stack.pop();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("state ") {
            let rest = rest.trim();
            if let Some(body) = rest.strip_suffix('{') {
                let (id, text) = alias(body.trim());
                let group = graph.add_group(&text, group_stack.last().copied());
                let _ = id;
                group_stack.push(group);
                continue;
            }
            if let Some((name, stereotype)) = rest.split_once("<<") {
                let shape = match stereotype.trim_end_matches('>').trim() {
                    "choice" => Shape::Diamond,
                    _ => Shape::Rect,
                };
                let index = intern(&mut graph, name.trim(), group_stack.last().copied());
                graph.nodes[index].shape = shape;
                if shape == Shape::Rect {
                    graph.set_label(index, "━━━");
                }
                continue;
            }
            let (id, text) = alias(rest);
            let index = intern(&mut graph, &id, group_stack.last().copied());
            graph.set_label(index, &text);
            continue;
        }
        if let Some(position) = trimmed.find("-->") {
            let from_text = trimmed[..position].trim();
            let rest = &trimmed[position + 3..];
            let (to_text, text) = match rest.split_once(':') {
                Some((t, l)) => (t.trim(), label(l)),
                None => (rest.trim(), String::new()),
            };
            let group = group_stack.last().copied();
            let from = endpoint(&mut graph, from_text, group, true);
            let to = endpoint(&mut graph, to_text, group, false);
            graph.add_edge(Edge { from, to, label: text, head: Marker::Arrow, ..Edge::default() });
            continue;
        }
        if let Some((name, description)) = trimmed.split_once(':') {
            let index = intern(&mut graph, name.trim(), group_stack.last().copied());
            graph.set_label(index, &label(description));
        }
    }
    graph
}

/// `"설명" as Id` → (Id, 설명)
fn alias(text: &str) -> (String, String) {
    if let Some((description, id)) = text.split_once(" as ") {
        return (id.trim().to_string(), label(description));
    }
    (text.to_string(), label(text))
}

fn intern(graph: &mut Graph, id: &str, group: Option<usize>) -> usize {
    let index = graph.intern(id, &label(id), Shape::Round, group);
    if graph.nodes[index].group.is_none() {
        graph.nodes[index].group = group;
    }
    index
}

fn endpoint(graph: &mut Graph, text: &str, group: Option<usize>, is_source: bool) -> usize {
    if text == "[*]" {
        let scope = group.map_or(String::from("root"), |g| g.to_string());
        let (id, shape) = if is_source { (format!("[*]start@{scope}"), Shape::Start) } else { (format!("[*]end@{scope}"), Shape::End) };
        let index = graph.intern(&id, "", shape, group);
        graph.nodes[index].shape = shape;
        return index;
    }
    intern(graph, text, group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states_and_composites() {
        let g = parse("stateDiagram-v2\n [*] --> Idle\n Idle --> Busy : start\n state Busy {\n  [*] --> Working\n }\n Busy --> [*]\n");
        assert_eq!(g.nodes[0].shape, Shape::Start);
        assert_eq!(g.edges[1].label, "start");
        assert_eq!(g.groups[0].title, "Busy");
        assert!(g.nodes.iter().any(|n| n.shape == Shape::End));
    }
}
