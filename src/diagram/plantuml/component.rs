//! PlantUML 컴포넌트·배치·유스케이스 다이어그램 파서.

use super::relation;
use super::text::{clean_lines, keyword, label, split_alias, stereotype_of, strip_decorations};
use crate::diagram::ir::{Direction, Edge, Graph, Shape};

const CONTAINERS: &[&str] = &["package", "namespace", "node", "cloud", "folder", "frame", "rectangle", "database", "component", "together", "storage", "queue", "stack", "card"];
const ELEMENTS: &[&str] = &[
    "component", "interface", "database", "node", "cloud", "folder", "frame", "rectangle", "storage", "queue", "stack", "card",
    "file", "artifact", "hexagon", "collections", "actor", "usecase", "agent", "boundary", "control", "entity", "label", "person", "circle",
];

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    Bracket(String),
    Paren(String),
    Actor(String),
    Quoted(String),
    Word(String),
    Label(String),
}

fn tokenize(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if c.is_whitespace() {
            index += 1;
            continue;
        }
        let closing = match c {
            '[' => Some((']', 0)),
            '(' => Some((')', 1)),
            '"' => Some(('"', 2)),
            ':' => Some((':', 3)),
            _ => None,
        };
        if let Some((close, kind)) = closing {
            if kind == 3 {
                // `:Actor:` 또는 라벨 시작
                let rest: String = chars[index + 1..].iter().collect();
                // `:이름:`은 액터, `: 글`은 라벨 시작
                let is_actor = !rest.starts_with(char::is_whitespace) && rest.contains(':');
                if !is_actor {
                    tokens.push(Token::Label(rest.trim().to_string()));
                    break;
                }
            }
            if kind == 1 && index + 1 < chars.len() && chars[index + 1] == ')' {
                tokens.push(Token::Word("()".into()));
                index += 2;
                continue;
            }
            let mut end = index + 1;
            while end < chars.len() && chars[end] != close {
                end += 1;
            }
            let body: String = chars[index + 1..end.min(chars.len())].iter().collect();
            tokens.push(match kind {
                0 => Token::Bracket(body.trim().to_string()),
                1 => Token::Paren(body.trim().to_string()),
                2 => Token::Quoted(body),
                _ => Token::Actor(body.trim().to_string()),
            });
            index = end + 1;
            continue;
        }
        let mut end = index;
        while end < chars.len() && !chars[end].is_whitespace() {
            end += 1;
        }
        let word: String = chars[index..end].iter().collect();
        if word.starts_with('#') || word.starts_with("<<") {
            index = end;
            continue;
        }
        tokens.push(Token::Word(word));
        index = end;
    }
    tokens
}

fn element_key(token: &Token) -> Option<(String, Shape)> {
    match token {
        Token::Bracket(text) => Some((text.clone(), Shape::Rect)),
        Token::Paren(text) => Some((text.clone(), Shape::Stadium)),
        Token::Actor(text) => Some((text.clone(), Shape::Actor)),
        Token::Quoted(text) => Some((text.clone(), Shape::Rect)),
        Token::Word(text) if !text.starts_with('#') => Some((text.clone(), Shape::Rect)),
        _ => None,
    }
}

fn shape_of(type_name: &str) -> (Shape, Option<&'static str>) {
    match type_name {
        "interface" | "()" => (Shape::Interface, None),
        "database" => (Shape::Cylinder, None),
        "cloud" => (Shape::Round, Some("cloud")),
        "node" => (Shape::Rect, Some("node")),
        "actor" | "person" => (Shape::Actor, None),
        "usecase" => (Shape::Stadium, None),
        "hexagon" => (Shape::Hexagon, None),
        "storage" => (Shape::Round, Some("storage")),
        "queue" => (Shape::Rect, Some("queue")),
        "boundary" => (Shape::Round, Some("boundary")),
        "control" => (Shape::Round, Some("control")),
        "entity" => (Shape::Round, Some("entity")),
        "circle" => (Shape::Circle, None),
        "component" | "label" | "rectangle" | "card" | "agent" => (Shape::Rect, None),
        other => (Shape::Rect, Some(match other {
            "folder" => "folder",
            "frame" => "frame",
            "file" => "file",
            "artifact" => "artifact",
            "collections" => "collections",
            "stack" => "stack",
            _ => "",
        })),
    }
}

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut group_stack: Vec<usize> = Vec::new();
    let mut brace_is_group: Vec<bool> = Vec::new();
    let mut in_note = false;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if in_note {
            if lower.starts_with("end note") || lower.starts_with("endnote") {
                in_note = false;
            }
            continue;
        }
        if trimmed == "}" {
            if brace_is_group.pop() == Some(true) {
                group_stack.pop();
            }
            continue;
        }
        if lower.starts_with("left to right") {
            graph.direction = Some(Direction::LeftRight);
            continue;
        }
        if lower.starts_with("top to bottom") {
            graph.direction = Some(Direction::TopDown);
            continue;
        }
        if lower.starts_with("title") {
            graph.title = label(trimmed[5..].trim());
            continue;
        }
        if lower.starts_with("note") {
            if !trimmed.contains(':') {
                in_note = true;
            }
            continue;
        }
        if trimmed.contains("(0") || trimmed.contains("0)") || trimmed.contains("[hidden]") {
            continue;
        }
        let first = keyword(&lower);
        let opens_brace = trimmed.ends_with('{');
        let body = trimmed.trim_end_matches('{').trim();
        if opens_brace && (CONTAINERS.contains(&first) || first == "together") {
            let rest = body[first.len()..].trim();
            let stereotype = stereotype_of(rest);
            let (_, title) = split_alias(rest);
            let title = match (first, stereotype) {
                ("together", _) => String::new(),
                ("package" | "namespace" | "rectangle", None) => title,
                (kind, None) => format!("{title} «{kind}»"),
                (_, Some(s)) => format!("{title} «{s}»"),
            };
            let group = graph.add_group(title.trim(), group_stack.last().copied());
            group_stack.push(group);
            brace_is_group.push(true);
            continue;
        }
        if opens_brace {
            brace_is_group.push(false);
            continue;
        }
        let group = group_stack.last().copied();
        if ELEMENTS.contains(&first) && !relation::contains_relation(body) {
            declare(&mut graph, first, body[first.len()..].trim(), group);
            continue;
        }
        if body.starts_with("()") && !relation::contains_relation(body) {
            declare(&mut graph, "()", body[2..].trim(), group);
            continue;
        }
        let tokens = tokenize(body);
        if let Some(position) = tokens.iter().position(|t| matches!(t, Token::Word(w) if relation::parse(w).is_some())) {
            let Token::Word(relation_token) = &tokens[position] else { continue };
            let Some(relation) = relation::parse(relation_token) else { continue };
            let left = tokens[..position].iter().find_map(element_key);
            let right = tokens[position + 1..].iter().find_map(element_key);
            let text = tokens.iter().find_map(|t| match t {
                Token::Label(l) => Some(label(l.trim_matches(['<', '>']).trim())),
                _ => None,
            });
            let (Some((left_id, left_shape)), Some((right_id, right_shape))) = (left, right) else { continue };
            let a = intern(&mut graph, &left_id, left_shape, group);
            let b = intern(&mut graph, &right_id, right_shape, group);
            let (from, to) = if relation.left_first { (a, b) } else { (b, a) };
            graph.add_edge(Edge {
                from,
                to,
                label: text.unwrap_or_default(),
                tail_label: String::new(),
                head_label: String::new(),
                kind: relation.kind,
                tail: relation.tail,
                head: relation.head,
            });
            continue;
        }
        // 단독 선언: `[Comp]`, `[Comp] as c`, `(Use case)`, `:Actor:`
        let mut tokens = tokens.into_iter();
        if let Some(first_token) = tokens.next()
            && let Some((id, shape)) = element_key(&first_token)
        {
            let rest: Vec<Token> = tokens.collect();
            let alias = rest.iter().position(|t| matches!(t, Token::Word(w) if w == "as")).and_then(|p| rest.get(p + 1)).and_then(element_key);
            match alias {
                Some((alias_id, _)) => {
                    let index = intern(&mut graph, &alias_id, shape, group);
                    graph.set_label(index, &label(&id));
                    graph.nodes[index].shape = shape;
                }
                None if matches!(first_token, Token::Bracket(_) | Token::Paren(_) | Token::Actor(_)) => {
                    intern(&mut graph, &id, shape, group);
                }
                None => {}
            }
        }
    }
    graph
}

fn intern(graph: &mut Graph, id: &str, shape: Shape, group: Option<usize>) -> usize {
    let existing = graph.find(id);
    let index = graph.intern(id, &label(id), shape, group);
    if existing.is_none() {
        graph.nodes[index].shape = shape;
    }
    if graph.nodes[index].group.is_none() {
        graph.nodes[index].group = group;
    }
    index
}

fn declare(graph: &mut Graph, type_name: &str, rest: &str, group: Option<usize>) {
    let stereotype = stereotype_of(rest);
    let cleaned = strip_decorations(rest);
    let cleaned = cleaned.trim_start_matches('[').trim_end_matches(']');
    let cleaned = cleaned.replace("] as ", " as ").replace("[", "").replace(") as ", " as ");
    let (id, text) = split_alias(cleaned.trim().trim_matches(['(', ')']));
    let (shape, implicit) = shape_of(type_name);
    let index = intern(graph, &id, shape, group);
    graph.nodes[index].shape = shape;
    let mut lines: Vec<String> = Vec::new();
    if let Some(s) = stereotype.as_deref().or(implicit).filter(|s| !s.is_empty()) {
        lines.push(format!("«{s}»"));
    }
    lines.extend(text.lines().map(str::to_string));
    graph.nodes[index].sections[0] = lines;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::ir::Marker;

    #[test]
    fn parses_components_and_groups() {
        let source = "@startuml\nleft to right direction\nactor User\npackage \"Backend\" {\n  [API Server] as api\n  database \"Postgres\" as db\n}\nUser --> api : HTTP\napi ..> db : SQL\n[Cache] -up-> api\n@enduml";
        let g = parse(source);
        assert_eq!(g.direction, Some(Direction::LeftRight));
        assert_eq!(g.groups[0].title, "Backend");
        let api = g.find("api").unwrap();
        assert_eq!(g.nodes[api].sections[0], vec!["API Server"]);
        assert_eq!(g.nodes[api].group, Some(0));
        assert_eq!(g.nodes[g.find("db").unwrap()].shape, Shape::Cylinder);
        assert_eq!(g.nodes[g.find("User").unwrap()].shape, Shape::Actor);
        assert_eq!(g.edges[0].label, "HTTP");
        assert_eq!(g.edges[0].head, Marker::Arrow);
        assert_eq!(g.edges[1].kind, crate::diagram::ir::LineKind::Dashed);
        assert_eq!(g.nodes[g.edges[2].from].id, "Cache");
    }
}
