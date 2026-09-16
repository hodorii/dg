//! PlantUML 클래스·ER 다이어그램 파서.

use super::relation;
use super::text::{clean_lines, keyword, label, split_alias, stereotype_of, strip_decorations};
use crate::diagram::ir::{Direction, Edge, Graph, Shape};

enum Scope {
    Group,
    Class { index: usize, explicit_section: Option<usize> },
    Skip,
}

const TYPES: &[&str] = &["class", "interface", "enum", "annotation", "entity", "object", "struct", "protocol", "exception", "metaclass", "stereotype", "circle", "diamond"];

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut scopes: Vec<Scope> = Vec::new();
    let mut group_stack: Vec<usize> = Vec::new();
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
        if let Some(Scope::Class { index, explicit_section }) = scopes.last_mut() {
            if trimmed.starts_with('}') {
                scopes.pop();
            } else {
                add_body_line(&mut graph, *index, explicit_section, trimmed);
            }
            continue;
        }
        if trimmed == "}" {
            if let Some(Scope::Group) = scopes.pop() {
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
            if !trimmed.contains(':') && !trimmed.contains(" as ") {
                in_note = true;
            }
            continue;
        }
        if lower.starts_with("together") {
            scopes.push(Scope::Skip);
            continue;
        }
        let first = keyword(&lower);
        if first == "package" || first == "namespace" {
            let rest = trimmed[first.len()..].trim().trim_end_matches('{').trim();
            let (_, title) = split_alias(rest);
            let group = graph.add_group(&title, group_stack.last().copied());
            group_stack.push(group);
            scopes.push(Scope::Group);
            continue;
        }
        let (is_abstract, first, declaration_rest) = if first == "abstract" {
            let rest = trimmed["abstract".len()..].trim();
            let lowered = rest.to_ascii_lowercase();
            let next = keyword(&lowered).to_string();
            if TYPES.contains(&next.as_str()) { (true, next.clone(), rest[next.len()..].trim()) } else { (true, "class".to_string(), rest) }
        } else {
            (false, first.to_string(), trimmed[first.len()..].trim())
        };
        if TYPES.contains(&first.as_str()) && !relation::contains_relation(trimmed) {
            let opens_body = declaration_rest.ends_with('{');
            let index = declare(&mut graph, &first, is_abstract, declaration_rest.trim_end_matches('{').trim(), group_stack.last().copied());
            if opens_body {
                scopes.push(Scope::Class { index, explicit_section: None });
            }
            continue;
        }
        if trimmed.ends_with('{') {
            scopes.push(Scope::Skip);
            continue;
        }
        if parse_relation(&mut graph, trimmed, group_stack.last().copied()) {
            continue;
        }
        if let Some((name, member)) = trimmed.split_once(':') {
            let name = name.trim();
            if !name.is_empty() && !name.contains(char::is_whitespace) {
                let index = intern(&mut graph, name, group_stack.last().copied());
                add_body_line(&mut graph, index, &mut None, member.trim());
            }
        }
    }
    graph
}

fn intern(graph: &mut Graph, name: &str, group: Option<usize>) -> usize {
    let name = name.trim().trim_matches('"');
    let index = graph.intern(name, name, Shape::Rect, group);
    if graph.nodes[index].group.is_none() {
        graph.nodes[index].group = group;
    }
    index
}

fn declare(graph: &mut Graph, type_name: &str, is_abstract: bool, rest: &str, group: Option<usize>) -> usize {
    let stereotype = stereotype_of(rest);
    let mut remainder = rest;
    let mut parents: Vec<(String, bool)> = Vec::new();
    for (word, dashed) in [(" implements ", true), (" extends ", false)] {
        if let Some(p) = super::text::find_word(remainder, word) {
            let names = remainder[p + word.len()..].to_string();
            remainder = &remainder[..p];
            let names = strip_decorations(&names);
            let names = names.split(" extends ").next().unwrap_or("").to_string();
            for name in names.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                parents.push((name.to_string(), dashed));
            }
        }
    }
    let (id, text) = split_alias(remainder);
    let index = intern(graph, &id, group);
    let shape = match type_name {
        "circle" => Shape::Circle,
        "diamond" => Shape::Diamond,
        _ => Shape::Rect,
    };
    graph.nodes[index].shape = shape;
    let mut name_lines: Vec<String> = Vec::new();
    let implicit = match type_name {
        "interface" => Some("interface"),
        "enum" => Some("enum"),
        "annotation" => Some("annotation"),
        "struct" => Some("struct"),
        "protocol" => Some("protocol"),
        "exception" => Some("exception"),
        _ if is_abstract => Some("abstract"),
        _ => None,
    };
    if let Some(s) = stereotype.as_deref().or(implicit) {
        name_lines.push(format!("«{s}»"));
    }
    name_lines.extend(text.lines().map(str::to_string));
    graph.nodes[index].sections[0] = name_lines;
    for (parent, dashed) in parents {
        let parent_index = intern(graph, &parent, group);
        graph.add_edge(Edge {
            from: parent_index,
            to: index,
            kind: if dashed { crate::diagram::ir::LineKind::Dashed } else { crate::diagram::ir::LineKind::Solid },
            tail: crate::diagram::ir::Marker::Triangle,
            ..Edge::default()
        });
    }
    index
}

/// 클래스 본문 한 줄: 구분선이면 새 칸, 아니면 멤버.
fn add_body_line(graph: &mut Graph, index: usize, explicit_section: &mut Option<usize>, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    let is_separator = line.len() >= 2 && (line.starts_with("--") || line.starts_with("..") || line.starts_with("==") || line.starts_with("__"));
    let node = &mut graph.nodes[index];
    if is_separator {
        let title = line.trim_matches(['-', '.', '=', '_']).trim();
        let mut section = Vec::new();
        if !title.is_empty() {
            node.sections.push(vec![format!("« {title} »")]);
            *explicit_section = Some(node.sections.len() - 1);
            return;
        }
        section.clear();
        node.sections.push(section);
        *explicit_section = Some(node.sections.len() - 1);
        return;
    }
    let mut text = line.to_string();
    for marker in ["{static}", "{abstract}", "{field}", "{method}", "{classifier}"] {
        text = text.replace(marker, "");
    }
    let text = label(text.trim());
    match explicit_section {
        Some(section) => node.sections[*section].push(text),
        None => {
            let is_method = text.contains('(');
            while node.sections.len() < 3 {
                node.sections.push(Vec::new());
            }
            node.sections[if is_method { 2 } else { 1 }].push(text);
        }
    }
}

/// 따옴표를 존중해 토큰으로 나눈다.
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for c in line.chars() {
        if c == '"' {
            in_quote = !in_quote;
            current.push(c);
            continue;
        }
        if c.is_whitespace() && !in_quote {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

pub fn parse_relation(graph: &mut Graph, line: &str, group: Option<usize>) -> bool {
    let (body, text) = match label_colon(line) {
        Some(p) => (&line[..p], label(&line[p + 1..])),
        None => (line, String::new()),
    };
    let text = text.trim_matches(['<', '>']).trim().to_string();
    let tokens = tokenize(body);
    let Some(position) = tokens.iter().position(|t| relation::parse(t).is_some()) else { return false };
    let Some(relation) = relation::parse(&tokens[position]) else { return false };
    let left_tokens = &tokens[..position];
    let right_tokens = &tokens[position + 1..];
    let (left_name, left_multiplicity) = name_and_multiplicity(left_tokens, true);
    let (right_name, right_multiplicity) = name_and_multiplicity(right_tokens, false);
    if left_name.is_empty() || right_name.is_empty() {
        return false;
    }
    let a = intern(graph, &left_name, group);
    let b = intern(graph, &right_name, group);
    let left_multiplicity = if left_multiplicity.is_empty() { relation.left_cardinality.to_string() } else { left_multiplicity };
    let right_multiplicity = if right_multiplicity.is_empty() { relation.right_cardinality.to_string() } else { right_multiplicity };
    let (from, to, tail_label, head_label) =
        if relation.left_first { (a, b, left_multiplicity, right_multiplicity) } else { (b, a, right_multiplicity, left_multiplicity) };
    graph.add_edge(Edge { from, to, label: text, tail_label, head_label, kind: relation.kind, tail: relation.tail, head: relation.head });
    true
}

/// 따옴표 밖에 있는 라벨 구분 콜론의 위치.
fn label_colon(line: &str) -> Option<usize> {
    let mut in_quote = false;
    for (index, c) in line.char_indices() {
        if c == '"' {
            in_quote = !in_quote;
        } else if c == ':' && !in_quote {
            return Some(index);
        }
    }
    None
}

/// `A "1"` / `"*" B` 토큰들에서 (이름, 다중성).
fn name_and_multiplicity(tokens: &[String], name_first: bool) -> (String, String) {
    let mut name = String::new();
    let mut multiplicity = String::new();
    for token in tokens {
        if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
            if name.is_empty() && name_first {
                name = token.trim_matches('"').to_string();
            } else if !name.is_empty() && name_first || name.is_empty() && !name_first {
                multiplicity = token.trim_matches('"').to_string();
            } else {
                name = token.trim_matches('"').to_string();
            }
        } else {
            name = token.clone();
        }
    }
    (name, multiplicity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::ir::Marker;

    #[test]
    fn parses_classes_and_relations() {
        let source = "@startuml\nabstract class Shape {\n  +area(): float\n  --\n  -id: int\n}\ninterface Drawable\nclass Circle extends Shape implements Drawable\nShape \"1\" *-- \"many\" Point : has\nCircle ..> Renderer\n@enduml";
        let g = parse(source);
        assert_eq!(g.nodes[0].sections[0], vec!["«abstract»", "Shape"]);
        assert_eq!(g.nodes[0].sections[2], vec!["+area(): float"]);
        assert_eq!(g.nodes[0].sections[3], vec!["-id: int"]);
        assert_eq!(g.nodes[1].sections[0], vec!["«interface»", "Drawable"]);
        let extends = g.edges.iter().find(|e| e.from == 0).unwrap();
        assert_eq!(extends.tail, Marker::Triangle);
        assert_eq!(g.nodes[extends.to].id, "Circle");
        assert!(g.edges.iter().any(|e| e.from == 1 && e.kind == crate::diagram::ir::LineKind::Dashed));
        let has = g.edges.iter().find(|e| e.label == "has").unwrap();
        assert_eq!(has.tail, Marker::DiamondFilled);
        assert_eq!(has.tail_label, "1");
        assert_eq!(has.head_label, "many");
    }

    #[test]
    fn parses_er_entities() {
        let source = "entity User {\n  *id : int <<PK>>\n  --\n  name : text\n}\nentity Order {\n  *id : int\n}\nUser ||--o{ Order : places";
        let g = parse(source);
        assert_eq!(g.nodes[0].sections[1], vec!["*id : int «PK»"]);
        assert_eq!(g.edges[0].tail_label, "1");
        assert_eq!(g.edges[0].head_label, "0..N");
    }
}
