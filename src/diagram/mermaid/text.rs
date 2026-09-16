//! mermaid 원문 다루기: 주석 제거, 라벨 정리, 노드 모양 괄호.

use crate::diagram::ir::Shape;

/// 주석(`%%`)과 빈 줄을 걷어낸다.
pub fn clean_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in source.lines() {
        let mut line = raw.to_string();
        if let Some(position) = comment_start(&line) {
            line.truncate(position);
        }
        let trimmed = line.trim_end();
        if !trimmed.trim().is_empty() {
            out.push(trimmed.to_string());
        }
    }
    out
}

/// 따옴표 밖의 `%%` 위치.
fn comment_start(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut previous = '\0';
    for (index, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' => quote = Some(c),
            None if c == '%' && previous == '%' => return Some(index - 1),
            None => {}
        }
        previous = c;
    }
    None
}

/// 라벨 정리: 따옴표 제거, `<br>`를 줄바꿈으로, HTML 실체 해제.
pub fn label(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    for quote in ['"', '\''] {
        if text.len() >= 2 && text.starts_with(quote) && text.ends_with(quote) {
            text = text[1..text.len() - 1].to_string();
        }
    }
    let text = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("#quot;", "\"")
        .replace("&quot;", "\"")
        .replace("#35;", "#")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    text.lines().map(str::trim).collect::<Vec<_>>().join("\n")
}

pub fn keyword(line: &str) -> &str {
    line.trim().split(char::is_whitespace).next().unwrap_or("")
}

/// 노드 모양을 여는 괄호와 닫는 괄호. 긴 것부터 살펴야 `([`가 `(`로 먼저 걸리지 않는다.
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

/// 아이디 뒤에 붙은 모양 괄호를 읽어 (모양, 라벨, 먹은 글자 수)를 돌려준다.
pub fn shape_delimited(rest: &str) -> Option<(Shape, String, usize)> {
    for &(open, close, shape) in SHAPE_DELIMITERS {
        if !rest.starts_with(open) {
            continue;
        }
        let Some(body_end) = find_closing(&rest[open.len()..], close) else { continue };
        let body = &rest[open.len()..open.len() + body_end];
        let consumed = open.chars().count() + body.chars().count() + close.chars().count();
        return Some((shape, label(body), consumed));
    }
    None
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comments() {
        assert_eq!(clean_lines("graph TD %% c\n\n A --> B\n%% x"), vec!["graph TD", " A --> B"]);
        assert_eq!(clean_lines(r#"A["99%% up"]"#), vec![r#"A["99%% up"]"#]);
    }

    #[test]
    fn label_unwraps() {
        assert_eq!(label(r#""a<br/>b""#), "a\nb");
    }
}
