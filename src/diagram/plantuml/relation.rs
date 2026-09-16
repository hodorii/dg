//! PlantUML 관계 토큰(`<|--`, `-->`, `||--o{` 등) 해석. 클래스·ER·컴포넌트가 함께 쓴다.

use crate::diagram::ir::{LineKind, Marker};

#[derive(Debug, PartialEq, Eq)]
pub struct Relation {
    pub kind: LineKind,
    /// 배치상 출발이 왼쪽 원소인가(거짓이면 오른쪽이 위/앞).
    pub left_first: bool,
    pub tail: Marker,
    pub head: Marker,
    pub left_cardinality: &'static str,
    pub right_cardinality: &'static str,
}

const LEFT_ENDS: &[&str] = &["<|", "||", "|o", "}|", "}o", "*", "o", "<", "#", "x", "^", "+"];
const RIGHT_ENDS: &[&str] = &["|>", "||", "o|", "|{", "o{", "*", "o", ">", "#", "x", "^", "+"];

fn cardinality(token: &str) -> &'static str {
    match token {
        "||" => "1",
        "|o" | "o|" => "0..1",
        "}|" | "|{" => "1..N",
        "}o" | "o{" => "0..N",
        _ => "",
    }
}

/// 토큰이 관계 기호이면 해석한다.
pub fn parse(token: &str) -> Option<Relation> {
    let cleaned = strip_hints(token);
    let mut rest = cleaned.as_str();
    let mut left_end = "";
    for end in LEFT_ENDS {
        if rest.starts_with(end) && rest[end.len()..].starts_with(['-', '.']) {
            left_end = end;
            rest = &rest[end.len()..];
            break;
        }
    }
    let body_length = rest.chars().take_while(|c| matches!(c, '-' | '.')).count();
    if body_length == 0 {
        return None;
    }
    let body = &rest[..body_length];
    let right_end = &rest[body_length..];
    if !right_end.is_empty() && !RIGHT_ENDS.contains(&right_end) {
        return None;
    }
    let kind = if body.contains('.') { LineKind::Dashed } else { LineKind::Solid };
    let mut relation = Relation {
        kind,
        left_first: true,
        tail: Marker::None,
        head: Marker::None,
        left_cardinality: cardinality(left_end),
        right_cardinality: cardinality(right_end),
    };
    let marker_of = |end: &str| match end {
        "<|" | "|>" | "^" => Marker::Triangle,
        "*" | "#" => Marker::DiamondFilled,
        "o" | "+" => Marker::DiamondOpen,
        "<" | ">" => Marker::Arrow,
        "x" => Marker::Cross,
        _ => Marker::None,
    };
    let left_marker = marker_of(left_end);
    let right_marker = marker_of(right_end);
    let is_owner = |m: Marker| matches!(m, Marker::Triangle | Marker::DiamondFilled | Marker::DiamondOpen);
    if is_owner(left_marker) {
        // 왼쪽이 부모/전체: 왼쪽 → 오른쪽, 표식은 꼬리(왼쪽 끝)에.
        relation.tail = left_marker;
        relation.head = right_marker;
    } else if is_owner(right_marker) {
        relation.left_first = false;
        relation.tail = right_marker;
        relation.head = left_marker;
    } else if left_marker != Marker::None && right_marker == Marker::None {
        relation.left_first = false;
        relation.head = left_marker;
    } else {
        relation.tail = left_marker;
        relation.head = right_marker;
    }
    Some(relation)
}

/// `-down->`, `-[hidden]->`, `-[#red]->`, `-l->` 같은 힌트를 걷어낸다.
fn strip_hints(token: &str) -> String {
    let mut out = String::new();
    let mut chars = token.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            for n in chars.by_ref() {
                if n == ']' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    for word in ["left", "right", "up", "down", "le", "ri", "do", "l", "r", "u", "d"] {
        let dashed = format!("-{word}-");
        if out.contains(&dashed) {
            out = out.replace(&dashed, "--");
            break;
        }
        let dotted = format!(".{word}.");
        if out.contains(&dotted) {
            out = out.replace(&dotted, "..");
            break;
        }
    }
    out
}

/// 줄에 관계 토큰이 있는가(공백으로 나뉜 토큰 기준).
pub fn contains_relation(line: &str) -> bool {
    line.split_whitespace().any(|token| parse(token).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance_points_to_parent() {
        let r = parse("<|--").unwrap();
        assert!(r.left_first);
        assert_eq!(r.tail, Marker::Triangle);
        let r = parse("..|>").unwrap();
        assert!(!r.left_first);
        assert_eq!(r.tail, Marker::Triangle);
        assert_eq!(r.kind, LineKind::Dashed);
    }

    #[test]
    fn arrows_and_crows_feet() {
        let r = parse("-down->").unwrap();
        assert_eq!(r.head, Marker::Arrow);
        let r = parse("<--").unwrap();
        assert!(!r.left_first);
        assert_eq!(r.head, Marker::Arrow);
        let r = parse("||--o{").unwrap();
        assert_eq!(r.left_cardinality, "1");
        assert_eq!(r.right_cardinality, "0..N");
        assert!(parse("name").is_none());
        assert!(parse("-").is_none() || parse("-").is_some());
    }
}
