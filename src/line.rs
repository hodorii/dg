//! 스타일이 붙은 텍스트 조각과 줄.

use crate::style::{Style, Theme};
use crate::text::width_of;

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>, style: Style) -> Span {
        Span { text: text.into(), style }
    }
    pub fn plain(text: impl Into<String>) -> Span {
        Span::new(text, Style::PLAIN)
    }
    pub fn width(&self) -> usize {
        width_of(&self.text)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn empty() -> Line {
        Line::default()
    }
    pub fn from_spans(spans: Vec<Span>) -> Line {
        Line { spans }
    }
    pub fn single(text: impl Into<String>, style: Style) -> Line {
        Line { spans: vec![Span::new(text, style)] }
    }
    pub fn push(&mut self, span: Span) {
        if span.text.is_empty() {
            return;
        }
        if let Some(last) = self.spans.last_mut()
            && last.style == span.style
        {
            last.text.push_str(&span.text);
            return;
        }
        self.spans.push(span);
    }
    pub fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
    pub fn is_blank(&self) -> bool {
        self.spans.iter().all(|s| s.text.trim().is_empty())
    }
    /// 대소문자 구분 없이 `needle`과 일치하는 부분을 `hit` 스타일로 바꾼 줄.
    pub fn highlight(&self, needle: &str, hit: Style) -> Line {
        if needle.is_empty() {
            return self.clone();
        }
        let plain = self.plain();
        let lower = plain.to_lowercase();
        let needle_lower = needle.to_lowercase();
        let mut hit_ranges: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        while let Some(pos) = lower[from..].find(&needle_lower) {
            let start = from + pos;
            // 소문자 변환으로 바이트 길이가 달라질 수 있으므로 원문 기준 끝을 다시 찾는다.
            let end = plain[start..]
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .find(|&e| plain[start..start + e].to_lowercase().len() >= needle_lower.len())
                .map(|e| start + e)
                .unwrap_or(plain.len());
            hit_ranges.push((start, end));
            from = end.max(start + 1);
            if from >= lower.len() {
                break;
            }
        }
        if hit_ranges.is_empty() {
            return self.clone();
        }
        let mut out = Line::empty();
        let mut offset = 0;
        for span in &self.spans {
            let span_start = offset;
            let span_end = offset + span.text.len();
            let mut cursor = span_start;
            for &(hit_start, hit_end) in &hit_ranges {
                let start = hit_start.max(span_start);
                let end = hit_end.min(span_end);
                if start >= end {
                    continue;
                }
                if cursor < start {
                    out.push(Span::new(&plain[cursor..start], span.style));
                }
                out.push(Span::new(&plain[start..end], span.style.merge(hit)));
                cursor = end;
            }
            if cursor < span_end {
                out.push(Span::new(&plain[cursor..span_end], span.style));
            }
            offset = span_end;
        }
        out
    }

    /// ANSI 문자열로 만든다. 줄 끝에서 스타일을 되돌린다.
    pub fn to_ansi(&self, theme: &Theme) -> String {
        let mut out = String::new();
        let mut styled = false;
        for span in &self.spans {
            let code = span.style.ansi(theme.enabled);
            if styled {
                out.push_str("\x1b[0m");
                styled = false;
            }
            if !code.is_empty() {
                out.push_str(&code);
                styled = true;
            }
            out.push_str(&span.text);
        }
        if styled {
            out.push_str("\x1b[0m");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_merges_same_style() {
        let mut line = Line::empty();
        line.push(Span::plain("a"));
        line.push(Span::plain("b"));
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.plain(), "ab");
    }

    #[test]
    fn highlight_marks_matches_across_spans() {
        let line = Line::from_spans(vec![Span::plain("Hel"), Span::new("lo world", Style::PLAIN.bold())]);
        let hit = line.highlight("LLO", Style::PLAIN.reverse());
        assert_eq!(hit.plain(), "Hello world");
        assert!(hit.spans.iter().any(|s| s.style.reverse && s.text == "l"));
        assert!(hit.spans.iter().any(|s| s.style.reverse && s.style.bold && s.text == "lo"));
    }
}
