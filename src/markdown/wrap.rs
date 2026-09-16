//! 스타일 조각 목록을 폭에 맞춰 줄로 나눈다.

use crate::line::Span;
use crate::style::Style;
use crate::text::{char_width, width_of};

struct Word {
    text: String,
    style: Style,
    is_space: bool,
    is_break: bool,
}

fn tokenize(spans: &[Span]) -> Vec<Word> {
    let mut words = Vec::new();
    for span in spans {
        let mut current = String::new();
        let mut current_is_space = false;
        let flush = |current: &mut String, is_space: bool, words: &mut Vec<Word>| {
            if !current.is_empty() {
                words.push(Word { text: std::mem::take(current), style: span.style, is_space, is_break: false });
            }
        };
        for c in span.text.chars() {
            if c == '\n' {
                flush(&mut current, current_is_space, &mut words);
                words.push(Word { text: String::new(), style: span.style, is_space: false, is_break: true });
                continue;
            }
            let is_space = c == ' ';
            if !current.is_empty() && is_space != current_is_space {
                flush(&mut current, current_is_space, &mut words);
            }
            current_is_space = is_space;
            current.push(c);
        }
        flush(&mut current, current_is_space, &mut words);
    }
    words
}

pub fn wrap_spans(spans: &[Span], width: usize) -> Vec<Vec<Span>> {
    let width = width.max(1);
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut current: Vec<Span> = Vec::new();
    let mut current_width = 0;
    let push_span = |line: &mut Vec<Span>, text: &str, style: Style| {
        if let Some(last) = line.last_mut()
            && last.style == style
        {
            last.text.push_str(text);
        } else {
            line.push(Span::new(text, style));
        }
    };
    let finish = |current: &mut Vec<Span>, lines: &mut Vec<Vec<Span>>| {
        // 줄 끝 공백 제거
        while let Some(last) = current.last_mut() {
            let trimmed = last.text.trim_end().to_string();
            if trimmed.is_empty() {
                current.pop();
            } else {
                last.text = trimmed;
                break;
            }
        }
        lines.push(std::mem::take(current));
    };
    for word in tokenize(spans) {
        if word.is_break {
            finish(&mut current, &mut lines);
            current_width = 0;
            continue;
        }
        if word.is_space {
            if current_width > 0 && current_width < width {
                push_span(&mut current, " ", word.style);
                current_width += 1;
            }
            continue;
        }
        let word_width = width_of(&word.text);
        if current_width + word_width > width && current_width > 0 {
            finish(&mut current, &mut lines);
            current_width = 0;
        }
        if word_width <= width {
            push_span(&mut current, &word.text, word.style);
            current_width += word_width;
            continue;
        }
        // 폭보다 긴 낱말: 글자 단위로 쪼갠다.
        for c in word.text.chars() {
            let w = char_width(c);
            if current_width + w > width && current_width > 0 {
                finish(&mut current, &mut lines);
                current_width = 0;
            }
            push_span(&mut current, &c.to_string(), word.style);
            current_width += w;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        finish(&mut current, &mut lines);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: Vec<Vec<Span>>) -> Vec<String> {
        lines.iter().map(|l| l.iter().map(|s| s.text.as_str()).collect()).collect()
    }

    #[test]
    fn wraps_keeping_styles() {
        let spans = vec![Span::plain("hello "), Span::new("bold world", Style::PLAIN.bold()), Span::plain(" end")];
        let lines = wrap_spans(&spans, 11);
        assert_eq!(plain(lines.clone()), vec!["hello bold", "world end"]);
        assert!(lines[1][0].style.bold);
    }

    #[test]
    fn hard_break_and_long_word() {
        let spans = vec![Span::plain("a\nb"), Span::plain(" abcdefgh")];
        assert_eq!(plain(wrap_spans(&spans, 5)), vec!["a", "b", "abcde", "fgh"]);
    }
}
