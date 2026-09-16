//! mermaid `block-beta` 파서.
//!
//! 격자(`columns N`)에 채워 넣는 블록과 그 사이 화살표를 읽는다.
//! 흐름도와 달리 배치가 원문의 순서로 정해지므로 공통 `Graph` 대신 제 구조를 쓴다.

use super::text::{clean_lines, keyword, label, shape_delimited};
use crate::diagram::ir::Shape;

/// 격자 열 수·칸 넓이의 상한. 터무니없는 수에 거대한 배치를 시도하지 않게 막는다.
const MAX_COLUMNS: usize = 64;
/// `block … end` 중첩 깊이 상한. 더 깊어지면 그 묶음은 무시한다.
const MAX_NESTING: usize = 8;

#[derive(Clone, Debug)]
pub struct Block {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    /// 가로로 차지하는 격자 칸 수(`b:2`).
    pub span: usize,
}

#[derive(Clone, Debug)]
pub struct Group {
    /// `block:id`의 아이디. 익명 `block`이면 빈 문자열이다.
    pub id: String,
    pub columns: Option<usize>,
    pub span: usize,
    pub items: Vec<Item>,
}

impl Default for Group {
    fn default() -> Group {
        Group { id: String::new(), columns: None, span: 1, items: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Leaf(Block),
    /// `space`/`space:N` — 그리지 않고 칸만 차지한다.
    Space(usize),
    Nested(Group),
}

impl Item {
    pub fn span(&self) -> usize {
        match self {
            Item::Leaf(block) => block.span,
            Item::Space(span) => *span,
            Item::Nested(group) => group.span,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Arrow {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct BlockDiagram {
    pub root: Group,
    pub arrows: Vec<Arrow>,
}

pub fn parse(source: &str) -> BlockDiagram {
    let mut diagram = BlockDiagram::default();
    let mut stack: Vec<Group> = Vec::new();
    // 깊이 제한으로 버린 `block`의 수. 짝이 되는 `end`도 같이 버려야 구조가 어긋나지 않는다.
    let mut dropped_groups = 0usize;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let first = keyword(&lower);
        if trimmed.starts_with("---") || matches!(first, "block-beta" | "title" | "classdef" | "class" | "style" | "click") {
            continue;
        }
        if lower == "end" {
            if dropped_groups > 0 {
                dropped_groups -= 1;
            } else if let Some(group) = stack.pop() {
                scope(&mut diagram, &mut stack).items.push(Item::Nested(group));
            }
            continue;
        }
        if first == "columns" {
            if let Ok(count) = trimmed[first.len()..].trim().parse::<usize>() {
                scope(&mut diagram, &mut stack).columns = Some(count.clamp(1, MAX_COLUMNS));
            }
            continue;
        }
        if first == "block" || lower.starts_with("block:") {
            if stack.len() >= MAX_NESTING {
                dropped_groups += 1;
            } else {
                stack.push(group_header(trimmed));
            }
            continue;
        }
        if let Some(at) = arrow_position(trimmed) {
            if let Some(arrow) = parse_arrow(trimmed, at) {
                diagram.arrows.push(arrow);
            }
            continue;
        }
        parse_blocks(trimmed, scope(&mut diagram, &mut stack));
    }
    // `end`가 모자라도 열린 묶음을 그대로 닫아 준다.
    while let Some(group) = stack.pop() {
        scope(&mut diagram, &mut stack).items.push(Item::Nested(group));
    }
    diagram
}

/// 지금 선언을 받을 묶음: 열려 있는 `block`이 있으면 그것, 없으면 뿌리.
fn scope<'a>(diagram: &'a mut BlockDiagram, stack: &'a mut [Group]) -> &'a mut Group {
    match stack.last_mut() {
        Some(group) => group,
        None => &mut diagram.root,
    }
}

/// `block:id`, `block:id:2`, `block` → 빈 묶음.
fn group_header(line: &str) -> Group {
    let token = line.split_whitespace().next().unwrap_or("");
    let body = token.get(5..).unwrap_or("").strip_prefix(':').unwrap_or("");
    let (id, span) = split_span(body);
    Group { id: id.to_string(), columns: None, span, items: Vec::new() }
}

/// `id:2` → (`id`, 2). 숫자 꼬리가 없으면 폭은 1.
fn split_span(text: &str) -> (&str, usize) {
    let Some(colon) = text.rfind(':') else { return (text, 1) };
    let digits = &text[colon + 1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return (text, 1);
    }
    (&text[..colon], digits.parse::<usize>().unwrap_or(1).clamp(1, MAX_COLUMNS))
}

/// 따옴표·괄호 밖에 있는 `--`의 자리. 있으면 그 줄은 화살표다.
fn arrow_position(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut quoted = false;
    let mut depth = 0i32;
    for index in 0..bytes.len() {
        match bytes[index] {
            b'"' => quoted = !quoted,
            b'[' | b'(' | b'{' if !quoted => depth += 1,
            b']' | b')' | b'}' if !quoted => depth -= 1,
            b'-' if !quoted && depth <= 0 && bytes.get(index + 1) == Some(&b'-') => return Some(index),
            _ => {}
        }
    }
    None
}

/// `a --> b`, `a -- 글 --> b`, `a <-- b`. 사슬(`a --> b --> c`)은 첫 칸만 읽는다.
fn parse_arrow(line: &str, at: usize) -> Option<Arrow> {
    let mut left = line[..at].trim();
    let mut reversed = false;
    if let Some(head) = left.strip_suffix('<') {
        left = head.trim();
        reversed = true;
    }
    let rest = &line[at..];
    let body_end = rest.find(|c| c != '-' && c != '>').unwrap_or(rest.len());
    let mut after = &rest[body_end..];
    let mut text = String::new();
    if !rest[..body_end].ends_with('>')
        && !reversed
        && let Some(closing) = after.find("--")
    {
        text = label(after[..closing].trim());
        let tail = &after[closing..];
        after = &tail[tail.find(|c| c != '-' && c != '>').unwrap_or(tail.len())..];
    }
    let from = identifier(left.split_whitespace().next_back().unwrap_or(""));
    let to = identifier(after.split_whitespace().next().unwrap_or(""));
    if from.is_empty() || to.is_empty() || from == to {
        return None;
    }
    let (from, to) = if reversed { (to, from) } else { (from, to) };
    Some(Arrow { from, to, label: text })
}

/// 아이디로 쓸 수 있는 앞부분만 남긴다.
fn identifier(token: &str) -> String {
    token.chars().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.')).collect()
}

/// 한 줄에 공백으로 이어 붙인 블록 선언들(`a["A"] b:2 c`).
fn parse_blocks(line: &str, group: &mut Group) {
    let chars: Vec<char> = line.chars().collect();
    let mut cursor = 0usize;
    while cursor < chars.len() {
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        let start = cursor;
        while cursor < chars.len() && (chars[cursor].is_alphanumeric() || matches!(chars[cursor], '_' | '.')) {
            cursor += 1;
        }
        if cursor == start {
            cursor += 1;
            continue;
        }
        let id: String = chars[start..cursor].iter().collect();
        let rest: String = chars[cursor..].iter().collect();
        let mut text = String::new();
        let mut shape = Shape::Rect;
        if let Some((found, body, consumed)) = shape_delimited(&rest) {
            shape = found;
            text = body;
            cursor += consumed;
        }
        let mut span = 1usize;
        if chars.get(cursor) == Some(&':') {
            let mut end = cursor + 1;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            if end > cursor + 1 {
                let digits: String = chars[cursor + 1..end].iter().collect();
                span = digits.parse::<usize>().unwrap_or(1).clamp(1, MAX_COLUMNS);
                cursor = end;
            }
        }
        if id.eq_ignore_ascii_case("space") && text.is_empty() {
            group.items.push(Item::Space(span));
        } else {
            let label = if text.is_empty() { id.clone() } else { text };
            group.items.push(Item::Leaf(Block { id, label, shape, span }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(group: &Group, index: usize) -> &Block {
        match &group.items[index] {
            Item::Leaf(block) => block,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_columns_labels_shapes_and_spans() {
        let d = parse("block-beta\n  columns 3\n  a[\"Block A\"] b:2\n  c d((e)) f{g}\n");
        assert_eq!(d.root.columns, Some(3));
        assert_eq!(d.root.items.len(), 5);
        assert_eq!(leaf(&d.root, 0).label, "Block A");
        assert_eq!(leaf(&d.root, 1).id, "b");
        assert_eq!(leaf(&d.root, 1).span, 2);
        // 라벨이 없으면 아이디가 곧 라벨이다.
        assert_eq!(leaf(&d.root, 2).label, "c");
        assert_eq!(leaf(&d.root, 3).shape, Shape::Circle);
        assert_eq!(leaf(&d.root, 4).shape, Shape::Diamond);
    }

    #[test]
    fn parses_plain_and_labeled_arrows() {
        let d = parse("block-beta\n a b c\n a --> b\n b -- \"넘김\" --> c\n c <-- a\n");
        assert_eq!(d.arrows.len(), 3);
        assert_eq!((d.arrows[0].from.as_str(), d.arrows[0].to.as_str()), ("a", "b"));
        assert_eq!(d.arrows[1].label, "넘김");
        assert_eq!((d.arrows[2].from.as_str(), d.arrows[2].to.as_str()), ("a", "c"));
    }

    #[test]
    fn parses_nested_groups_with_own_columns() {
        let d = parse("block-beta\n columns 2\n block:group1\n  columns 1\n  x y\n end\n z\n");
        assert_eq!(d.root.items.len(), 2);
        let Item::Nested(inner) = &d.root.items[0] else { panic!("중첩 아님") };
        assert_eq!(inner.id, "group1");
        assert_eq!(inner.columns, Some(1));
        assert_eq!(inner.items.len(), 2);
        assert_eq!(leaf(&d.root, 1).id, "z");
    }

    #[test]
    fn label_with_dashes_is_not_an_arrow() {
        let d = parse("block-beta\n a[\"a--b\"]\n");
        assert!(d.arrows.is_empty());
        assert_eq!(leaf(&d.root, 0).label, "a--b");
    }

    #[test]
    fn unclosed_group_and_space_filler() {
        let d = parse("block-beta\n columns 2\n space a\n block:g\n  b\n");
        assert!(matches!(d.root.items[0], Item::Space(1)));
        assert!(matches!(d.root.items[2], Item::Nested(_)));
    }
}
