//! 터미널 칸 단위 폭 계산과 줄바꿈.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width_of(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// 폭이 `max`를 넘으면 잘라내고 `…`를 붙인다.
pub fn truncate(s: &str, max: usize) -> String {
    if width_of(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = char_width(c);
        if used + w + 1 > max {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// 한 줄의 평문을 `width`칸에 맞춰 여러 줄로 나눈다.
/// 공백에서 끊고, 한 낱말이 너무 길면 글자 단위로 자른다.
pub fn wrap_plain(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for paragraph in s.split('\n') {
        let mut current = String::new();
        let mut current_width = 0;
        for word in paragraph.split(' ') {
            if word.is_empty() {
                continue;
            }
            for piece in split_long_word(word, width) {
                let piece_width = width_of(&piece);
                let separator = if current.is_empty() { 0 } else { 1 };
                if current_width + separator + piece_width > width && !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    current_width = 0;
                }
                if !current.is_empty() {
                    current.push(' ');
                    current_width += 1;
                }
                current.push_str(&piece);
                current_width += piece_width;
            }
        }
        lines.push(current);
    }
    lines
}

fn split_long_word(word: &str, width: usize) -> Vec<String> {
    if width_of(word) <= width {
        return vec![word.to_string()];
    }
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for c in word.chars() {
        let w = char_width(c);
        if current_width + w > width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(c);
        current_width += w;
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_spaces_and_splits_long_words() {
        assert_eq!(wrap_plain("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap_plain("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap_plain("한글 낱말 단위", 5), vec!["한글", "낱말", "단위"]);
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
    }
}
