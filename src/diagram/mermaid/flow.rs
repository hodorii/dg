//! `flowchart` / `graph` 파서.

use super::text::{clean_lines, keyword, label};
use crate::diagram::ir::{Direction, Edge, Graph, LineKind, Marker, Shape};

struct NodeRef {
    id: String,
    label: Option<String>,
    shape: Option<Shape>,
}

struct Link {
    label: String,
    kind: LineKind,
    head: Marker,
    tail: Marker,
}

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut group_stack: Vec<usize> = Vec::new();
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let first = keyword(&lower);
        if first == "flowchart" || first == "flowchart-v2" || first == "graph" {
            let direction = lower[first.len()..].trim();
            graph.direction = Some(match direction {
                "lr" | "rl" => Direction::LeftRight,
                _ => Direction::TopDown,
            });
            continue;
        }
        if first == "subgraph" {
            let rest = trimmed[first.len()..].trim();
            let (id, title) = subgraph_id_and_title(rest);
            let group = graph.add_group_with_id(&id, &title, group_stack.last().copied());
            group_stack.push(group);
            continue;
        }
        if lower == "end" {
            group_stack.pop();
            continue;
        }
        if matches!(first, "direction" | "classdef" | "class" | "style" | "linkstyle" | "click" | "acctitle:" | "accdescr:" | "title") {
            continue;
        }
        for statement in trimmed.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            parse_statement(statement, &mut graph, group_stack.last().copied());
        }
    }
    graph
}

/// `Id["제목"]` → (Id, 제목), `제목` → (제목, 제목)
fn subgraph_id_and_title(rest: &str) -> (String, String) {
    if let Some(open) = rest.find('[') {
        let close = rest.rfind(']').unwrap_or(rest.len());
        if close > open {
            return (rest[..open].trim().to_string(), label(&rest[open + 1..close]));
        }
    }
    let title = label(rest);
    (title.clone(), title)
}

/// `A --> B & C --> D` 같은 문장 하나.
fn parse_statement(statement: &str, graph: &mut Graph, group: Option<usize>) {
    let chars: Vec<char> = statement.chars().collect();
    let mut cursor = 0;
    let Some(mut previous) = read_node_list(&chars, &mut cursor, graph, group) else { return };
    while let Some(link) = read_link(&chars, &mut cursor) {
        let Some(next) = read_node_list(&chars, &mut cursor, graph, group) else { break };
        for &from in &previous {
            for &to in &next {
                graph.add_edge(Edge {
                    from,
                    to,
                    label: link.label.clone(),
                    kind: link.kind,
                    head: link.head,
                    tail: link.tail,
                    ..Edge::default()
                });
            }
        }
        previous = next;
    }
}

fn skip_spaces(chars: &[char], cursor: &mut usize) {
    while *cursor < chars.len() && chars[*cursor].is_whitespace() {
        *cursor += 1;
    }
}

fn read_node_list(chars: &[char], cursor: &mut usize, graph: &mut Graph, group: Option<usize>) -> Option<Vec<usize>> {
    let mut nodes = Vec::new();
    loop {
        let node = read_node(chars, cursor)?;
        // 서브그래프 아이디를 간선 끝으로 쓰면 그 그룹의 닻에 잇는다.
        if node.label.is_none()
            && graph.find(&node.id).is_none()
            && let Some(target_group) = graph.groups.iter().position(|g| g.id == node.id)
        {
            nodes.push(graph.group_anchor(target_group));
            skip_spaces(chars, cursor);
            if *cursor < chars.len() && chars[*cursor] == '&' {
                *cursor += 1;
                continue;
            }
            return Some(nodes);
        }
        let index = graph.intern(&node.id, node.label.as_deref().unwrap_or(""), node.shape.unwrap_or_default(), group);
        if let Some(text) = &node.label {
            graph.set_label(index, text);
        }
        if let Some(shape) = node.shape {
            graph.nodes[index].shape = shape;
        }
        if graph.nodes[index].group.is_none() && group.is_some() {
            graph.nodes[index].group = group;
        }
        nodes.push(index);
        skip_spaces(chars, cursor);
        if *cursor < chars.len() && chars[*cursor] == '&' {
            *cursor += 1;
            continue;
        }
        return Some(nodes);
    }
}

const SHAPE_DELIMITERS: &[(&str, &str, Shape)] = &[
    ("(((", ")))", Shape::Circle),
    ("([", "])", Shape::Stadium),
    ("[[", "]]", Shape::Subroutine),
    ("[(", ")]", Shape::Cylinder),
    ("((", "))", Shape::Circle),
    ("{{", "}}", Shape::Hexagon),
    ("[/", "/]", Shape::Rect),
    ("[\\", "\\]", Shape::Rect),
    ("[/", "\\]", Shape::Rect),
    ("[\\", "/]", Shape::Rect),
    ("[", "]", Shape::Rect),
    ("(", ")", Shape::Round),
    ("{", "}", Shape::Diamond),
    (">", "]", Shape::Rect),
];

fn read_node(chars: &[char], cursor: &mut usize) -> Option<NodeRef> {
    skip_spaces(chars, cursor);
    let start = *cursor;
    while *cursor < chars.len() && (chars[*cursor].is_alphanumeric() || matches!(chars[*cursor], '_' | '-' | '.' | ':')) {
        // `-->` 시작을 아이디로 먹지 않는다.
        if chars[*cursor] == '-' && chars.get(*cursor + 1).is_some_and(|c| matches!(c, '-' | '.' | '=' | '>')) {
            break;
        }
        if chars[*cursor] == ':' && chars.get(*cursor + 1) == Some(&':') {
            break;
        }
        *cursor += 1;
    }
    if start == *cursor {
        return None;
    }
    let id: String = chars[start..*cursor].iter().collect();
    let mut node = NodeRef { id, label: None, shape: None };
    let rest: String = chars[*cursor..].iter().collect();
    for &(open, close, shape) in SHAPE_DELIMITERS {
        if !rest.starts_with(open) {
            continue;
        }
        let Some(body_end) = find_closing(&rest[open.len()..], close) else { continue };
        let body = &rest[open.len()..open.len() + body_end];
        node.label = Some(label(body));
        node.shape = Some(shape);
        *cursor += (open.len() + body_end + close.len()).min(rest.len());
        // 문자 수와 바이트 수가 다를 수 있으므로 다시 계산한다.
        let consumed = open.chars().count() + body.chars().count() + close.chars().count();
        *cursor = start + node.id.chars().count() + consumed;
        break;
    }
    // `:::class` 꾸밈은 버린다.
    let after: String = chars[*cursor..].iter().collect();
    if let Some(class_name) = after.strip_prefix(":::") {
        let consumed = class_name.chars().take_while(|c| !c.is_whitespace()).count();
        *cursor += 3 + consumed;
    }
    Some(node)
}

/// 따옴표 안을 건너뛰고 닫는 구분자를 찾는다(바이트 위치).
fn find_closing(text: &str, close: &str) -> Option<usize> {
    let mut in_quote = false;
    let mut index = 0;
    while index < text.len() {
        let c = text[index..].chars().next()?;
        if c == '"' {
            in_quote = !in_quote;
        } else if !in_quote && text[index..].starts_with(close) {
            return Some(index);
        }
        index += c.len_utf8();
    }
    None
}

fn read_link(chars: &[char], cursor: &mut usize) -> Option<Link> {
    skip_spaces(chars, cursor);
    let mut position = *cursor;
    let mut tail = Marker::None;
    match chars.get(position) {
        Some('<') => {
            tail = Marker::Arrow;
            position += 1;
        }
        Some('x') if chars.get(position + 1).is_some_and(|c| matches!(c, '-' | '=' | '.')) => {
            tail = Marker::Cross;
            position += 1;
        }
        Some('o') if chars.get(position + 1).is_some_and(|c| matches!(c, '-' | '=' | '.')) => {
            tail = Marker::Circle;
            position += 1;
        }
        _ => {}
    }
    let body_start = position;
    while position < chars.len() && matches!(chars[position], '-' | '=' | '.') {
        position += 1;
    }
    if position - body_start < 2 {
        return None;
    }
    let body: String = chars[body_start..position].iter().collect();
    let mut kind = if body.contains('.') {
        LineKind::Dashed
    } else if body.contains('=') {
        LineKind::Heavy
    } else {
        LineKind::Solid
    };
    let mut text = String::new();
    // `-- 글 -->` 꼴: 몸통 다음이 머리가 아니면 글이 이어진다.
    let head_char = chars.get(position).copied();
    let is_head = matches!(head_char, Some('>') | Some('x') | Some('o') | Some('|')) || head_char.is_none() || head_char == Some(' ');
    if !is_head || (head_char == Some(' ') && !body.ends_with('>')) {
        // 몸통 뒤 공백 후 글, 그 뒤 닫는 몸통
        let mut probe = position;
        skip_spaces(chars, &mut probe);
        let closing = find_closing_body(chars, probe);
        if let Some((text_end, closing_end)) = closing {
            let raw: String = chars[probe..text_end].iter().collect();
            text = label(raw.trim());
            let closing_body: String = chars[text_end..closing_end].iter().collect();
            if closing_body.contains('.') {
                kind = LineKind::Dashed;
            } else if closing_body.contains('=') {
                kind = LineKind::Heavy;
            }
            position = closing_end;
        }
    }
    let mut head = Marker::None;
    match chars.get(position) {
        Some('>') => {
            head = Marker::Arrow;
            position += 1;
        }
        Some('x') => {
            head = Marker::Cross;
            position += 1;
        }
        Some('o') => {
            head = Marker::Circle;
            position += 1;
        }
        _ => {}
    }
    let mut after = position;
    skip_spaces(chars, &mut after);
    if chars.get(after) == Some(&'|') {
        let text_start = after + 1;
        let mut end = text_start;
        while end < chars.len() && chars[end] != '|' {
            end += 1;
        }
        let raw: String = chars[text_start..end].iter().collect();
        text = label(raw.trim());
        position = (end + 1).min(chars.len());
    }
    *cursor = position;
    Some(Link { label: text, kind, head, tail })
}

/// `-- 글 -->`에서 글 다음의 닫는 몸통 위치 (글 끝, 몸통 끝).
fn find_closing_body(chars: &[char], from: usize) -> Option<(usize, usize)> {
    let mut index = from;
    while index < chars.len() {
        if matches!(chars[index], '-' | '=' | '.') {
            let mut end = index;
            while end < chars.len() && matches!(chars[end], '-' | '=' | '.') {
                end += 1;
            }
            if end - index >= 2 || (end - index == 1 && chars.get(end) == Some(&'>')) {
                return Some((index, end));
            }
            index = end;
            continue;
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shapes_and_links() {
        let g = parse("flowchart LR\n A[Start] --> B{Decide?}\n B -- yes --> C([Ok])\n B -.->|no| D[(db)]\n");
        assert_eq!(g.direction, Some(Direction::LeftRight));
        assert_eq!(g.nodes.len(), 4);
        assert_eq!(g.nodes[1].shape, Shape::Diamond);
        assert_eq!(g.nodes[3].shape, Shape::Cylinder);
        assert_eq!(g.edges[1].label, "yes");
        assert_eq!(g.edges[2].label, "no");
        assert_eq!(g.edges[2].kind, LineKind::Dashed);
    }

    #[test]
    fn edge_to_subgraph_id_uses_anchor() {
        let g = parse("graph TB\n subgraph NET[\"망\"]\n  A\n end\n X --> NET\n");
        assert_eq!(g.groups[0].id, "NET");
        let anchor = g.nodes.iter().position(|n| n.shape == Shape::Anchor).unwrap();
        assert_eq!(g.nodes[anchor].group, Some(0));
        assert_eq!(g.edges[0].to, anchor);
    }

    #[test]
    fn parses_subgraph_and_ampersand() {
        let g = parse("graph TD\n subgraph Back[\"Backend\"]\n  api --> db\n end\n web & app --> api\n");
        assert_eq!(g.groups[0].title, "Backend");
        assert_eq!(g.nodes[0].group, Some(0));
        assert_eq!(g.edges.len(), 3);
    }
}
