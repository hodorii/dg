//! PlantUML 원문 다루기.

/// `@startuml`/`@enduml`, 주석, 전처리·꾸밈 지시어를 걷어낸다.
pub fn clean_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block_comment = false;
    let mut in_skip_block: Option<&'static str> = None;
    for raw in source.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if in_block_comment {
            if trimmed.contains("'/") {
                in_block_comment = false;
            }
            continue;
        }
        if let Some(terminator) = in_skip_block {
            if trimmed.to_ascii_lowercase().starts_with(terminator) {
                in_skip_block = None;
            }
            continue;
        }
        if trimmed.starts_with("/'") {
            if !trimmed.contains("'/") {
                in_block_comment = true;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('\'') || trimmed.starts_with('@') || trimmed.starts_with('!') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        let first = keyword(&lower);
        if matches!(first, "skinparam" | "hide" | "show" | "scale" | "header" | "footer" | "caption" | "newpage" | "mainframe" | "remove" | "set" | "pragma" | "sprite" | "allowmixing" | "allow_mixing")
        {
            if first == "skinparam" && trimmed.ends_with('{') {
                in_skip_block = Some("}");
            }
            continue;
        }
        if first == "legend" && !trimmed.contains(':') {
            in_skip_block = Some("endlegend");
            if !lower.contains("end legend") {
                in_skip_block = Some("end legend");
            }
            continue;
        }
        if lower.starts_with("endlegend") || lower.starts_with("end legend") {
            continue;
        }
        out.push(line.to_string());
    }
    out
}

pub fn keyword(line: &str) -> &str {
    line.trim().split(char::is_whitespace).next().unwrap_or("")
}

/// 따옴표 제거, `\n` 줄바꿈, `<<x>>` → `«x»`, 색상 지시 제거.
pub fn label(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text = text[1..text.len() - 1].to_string();
    }
    let text = text.replace("\\n", "\n").replace("<<", "«").replace(">>", "»");
    let text = strip_creole(&text);
    text.lines().map(str::trim).collect::<Vec<_>>().join("\n")
}

/// `**굵게**`, `//기울임//`, `<b>` 같은 꾸밈을 벗긴다.
fn strip_creole(text: &str) -> String {
    let mut out = text.replace("**", "").replace("//", "").replace("__", "");
    for tag in ["<b>", "</b>", "<i>", "</i>", "<u>", "</u>", "<size:", "</size>", "<color:", "</color>"] {
        out = out.replace(tag, "");
    }
    out
}

/// `"긴 이름" as 별칭` / `이름 as "긴 이름"` / `이름` → (아이디, 라벨).
pub fn split_alias(text: &str) -> (String, String) {
    let text = strip_decorations(text);
    if let Some(position) = find_word(&text, " as ") {
        let (left, right) = (text[..position].trim(), text[position + 4..].trim());
        // `이름 as "긴 이름"`만 오른쪽이 라벨이고, 그 밖에는 왼쪽이 라벨·오른쪽이 별칭이다.
        if right.starts_with('"') {
            return (left.trim_matches('"').to_string(), label(right));
        }
        return (right.trim_matches('"').to_string(), label(left));
    }
    let id = text.trim().trim_matches('"').to_string();
    (id.clone(), label(&text))
}

/// `#color`, `<<stereotype>>`, `order N`을 떼어낸다(스테레오타입은 라벨에 남기고 싶으면 별도 처리).
pub fn strip_decorations(text: &str) -> String {
    let mut out = String::new();
    let mut in_quote = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            in_quote = !in_quote;
            out.push(c);
            continue;
        }
        if !in_quote && c == '#' {
            while chars.peek().is_some_and(|n| !n.is_whitespace()) {
                chars.next();
            }
            continue;
        }
        if !in_quote && c == '<' && chars.peek() == Some(&'<') {
            let mut previous = ' ';
            for n in chars.by_ref() {
                if previous == '>' && n == '>' {
                    break;
                }
                previous = n;
            }
            continue;
        }
        out.push(c);
    }
    let out = out.trim().to_string();
    match find_word(&out, " order ") {
        Some(p) => out[..p].trim().to_string(),
        None => out,
    }
}

/// 따옴표 밖에서 `needle`을 찾는다.
pub fn find_word(text: &str, needle: &str) -> Option<usize> {
    let mut in_quote = false;
    for (index, c) in text.char_indices() {
        if c == '"' {
            in_quote = !in_quote;
        }
        if !in_quote && text[index..].starts_with(needle) {
            return Some(index);
        }
    }
    None
}

/// 스테레오타입 `<<x>>`만 뽑는다.
pub fn stereotype_of(text: &str) -> Option<String> {
    let start = text.find("<<")?;
    let end = text[start + 2..].find(">>")? + start + 2;
    Some(text[start + 2..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_wrapper_and_comments() {
        let out = clean_lines("@startuml\n' note\nskinparam x y\nA -> B\n/' block\n more '/\nB -> A\n@enduml");
        assert_eq!(out, vec!["A -> B", "B -> A"]);
    }

    #[test]
    fn alias_forms() {
        assert_eq!(split_alias("\"Long Name\" as L"), ("L".into(), "Long Name".into()));
        assert_eq!(split_alias("L as \"Long Name\""), ("L".into(), "Long Name".into()));
        assert_eq!(split_alias("Plain #red"), ("Plain".into(), "Plain".into()));
    }
}
