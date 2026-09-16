//! mermaid 원문 다루기: 주석 제거, 라벨 정리.

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
