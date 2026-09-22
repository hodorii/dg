//! 스타일이 붙은 텍스트 줄.
//!
//! 줄 하나는 문자열 하나와 (길이, 스타일) 구간 목록으로 저장한다. 조각마다 문자열을 따로 갖는 것보다
//! 할당이 훨씬 적어, 수십만 줄짜리 문서도 가볍게 들고 있을 수 있다. 만들 때는 `Span`을 밀어 넣는다.

use crate::style::{Style, Theme};
use crate::text::{char_width, width_of};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Run {
    len: u32,
    style: Style,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    text: String,
    runs: Vec<Run>,
}

impl Line {
    pub fn empty() -> Line {
        Line::default()
    }
    pub fn from_spans(spans: Vec<Span>) -> Line {
        let mut line = Line::empty();
        for span in spans {
            line.push_str(&span.text, span.style);
        }
        line
    }
    pub fn single(text: impl Into<String>, style: Style) -> Line {
        let text = text.into();
        let runs = if text.is_empty() { Vec::new() } else { vec![Run { len: text.len() as u32, style }] };
        Line { text, runs }
    }
    pub fn push(&mut self, span: Span) {
        self.push_str(&span.text, span.style);
    }
    pub fn push_str(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        self.text.push_str(text);
        match self.runs.last_mut() {
            Some(last) if last.style == style => last.len += text.len() as u32,
            _ => self.runs.push(Run { len: text.len() as u32, style }),
        }
    }
    pub fn append(&mut self, other: &Line) {
        for (text, style) in other.runs() {
            self.push_str(text, style);
        }
    }
    /// (텍스트, 스타일) 구간을 차례로 돌려준다.
    pub fn runs(&self) -> impl Iterator<Item = (&str, Style)> {
        let mut offset = 0;
        self.runs.iter().map(move |run| {
            let start = offset;
            offset += run.len as usize;
            (&self.text[start..offset], run.style)
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn width(&self) -> usize {
        width_of(&self.text)
    }
    #[cfg(test)]
    pub fn plain(&self) -> String {
        self.text.clone()
    }
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
    /// 저장용으로 남는 용량을 줄인다.
    pub fn shrink(&mut self) {
        self.text.shrink_to_fit();
        self.runs.shrink_to_fit();
    }

    /// 대소문자 구분 없이 `needle`과 일치하는 부분을 `hit` 스타일로 바꾼 줄.
    pub fn highlight(&self, needle: &str, hit: Style) -> Line {
        if needle.is_empty() {
            return self.clone();
        }
        let lower = self.text.to_lowercase();
        let needle_lower = needle.to_lowercase();
        // 소문자 변환으로 길이가 달라질 수 있으므로 원문 글자 경계로 되돌린다.
        let mut hit_ranges: Vec<(usize, usize)> = Vec::new();
        if lower.len() == self.text.len() {
            let mut from = 0;
            while let Some(pos) = lower[from..].find(&needle_lower) {
                let start = from + pos;
                let end = start + needle_lower.len();
                hit_ranges.push((start, end));
                from = end.max(start + 1);
                if from >= lower.len() {
                    break;
                }
            }
        } else {
            let mut from = 0;
            while let Some(pos) = lower[from..].find(&needle_lower) {
                let start = from + pos;
                let end = self.text[start..]
                    .char_indices()
                    .map(|(i, c)| i + c.len_utf8())
                    .find(|&e| self.text[start..start + e].to_lowercase().len() >= needle_lower.len())
                    .map_or(self.text.len(), |e| start + e);
                hit_ranges.push((start, end));
                from = end.max(start + 1);
                if from >= lower.len() {
                    break;
                }
            }
        }
        if hit_ranges.is_empty() {
            return self.clone();
        }
        let mut out = Line::empty();
        let mut offset = 0;
        for (text, style) in self.runs() {
            let run_start = offset;
            let run_end = offset + text.len();
            let mut cursor = run_start;
            for &(hit_start, hit_end) in &hit_ranges {
                let start = hit_start.max(run_start);
                let end = hit_end.min(run_end);
                if start >= end {
                    continue;
                }
                if cursor < start {
                    out.push_str(&self.text[cursor..start], style);
                }
                out.push_str(&self.text[start..end], style.merge(hit));
                cursor = end;
            }
            if cursor < run_end {
                out.push_str(&self.text[cursor..run_end], style);
            }
            offset = run_end;
        }
        out
    }

    /// `[col_start, col_end)`(문자 폭 기준, CJK처럼 폭이 2인 글자 포함)에 대응하는 바이트
    /// 범위를 찾는다. `highlight_span`(강조)과 `text_between_cols`(드래그 선택 텍스트 추출,
    /// markdown-source-view)가 함께 쓴다 — 부분 문자열 찾기가 아니라 열 좌표로 바로 구간을
    /// 정해야 하는 두 용도가 같은 변환을 필요로 한다.
    fn byte_range_for_cols(&self, col_start: usize, col_end: usize) -> Option<(usize, usize)> {
        if col_start >= col_end {
            return None;
        }
        let mut byte_start = None;
        let mut byte_end = self.text.len();
        let mut col = 0;
        for (i, c) in self.text.char_indices() {
            if byte_start.is_none() && col >= col_start {
                byte_start = Some(i);
            }
            if col >= col_end {
                byte_end = i;
                break;
            }
            col += char_width(c);
        }
        byte_start.map(|start| (start, byte_end))
    }

    /// `[col_start, col_end)` 구간의 텍스트만 뽑는다(드래그 선택 텍스트 추출용,
    /// markdown-source-view). 구간이 줄 밖이면 빈 문자열.
    pub fn text_between_cols(&self, col_start: usize, col_end: usize) -> &str {
        match self.byte_range_for_cols(col_start, col_end) {
            Some((start, end)) => &self.text[start..end],
            None => "",
        }
    }

    /// 문자 폭 기준 `[col_start, col_end)` 구간에 `style`을 덧씌운 줄(포커스된 링크 강조용,
    /// `markdown-link-navigation`).
    pub fn highlight_span(&self, col_start: usize, col_end: usize, style: Style) -> Line {
        let Some((byte_start, byte_end)) = self.byte_range_for_cols(col_start, col_end) else { return self.clone() };
        let mut out = Line::empty();
        let mut offset = 0;
        for (text, run_style) in self.runs() {
            let run_start = offset;
            let run_end = offset + text.len();
            let start = byte_start.max(run_start);
            let end = byte_end.min(run_end);
            if start < end {
                if run_start < start {
                    out.push_str(&self.text[run_start..start], run_style);
                }
                out.push_str(&self.text[start..end], run_style.merge(style));
                if end < run_end {
                    out.push_str(&self.text[end..run_end], run_style);
                }
            } else {
                out.push_str(text, run_style);
            }
            offset = run_end;
        }
        out
    }

    /// ANSI 문자열로 만든다. 줄 끝에서 스타일을 되돌린다.
    pub fn to_ansi(&self, theme: &Theme) -> String {
        let mut out = String::with_capacity(self.text.len() + self.runs.len() * 12);
        let mut styled = false;
        for (text, style) in self.runs() {
            let code = style.ansi(theme.enabled);
            if styled {
                out.push_str("\x1b[0m");
                styled = false;
            }
            if !code.is_empty() {
                out.push_str(&code);
                styled = true;
            }
            out.push_str(text);
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
        assert_eq!(line.runs.len(), 1);
        assert_eq!(line.plain(), "ab");
    }

    #[test]
    fn highlight_marks_matches_across_runs() {
        let line = Line::from_spans(vec![Span::plain("Hel"), Span::new("lo world", Style::PLAIN.bold())]);
        let hit = line.highlight("LLO", Style::PLAIN.reverse());
        assert_eq!(hit.plain(), "Hello world");
        let runs: Vec<(&str, Style)> = hit.runs().collect();
        assert!(runs.iter().any(|(t, s)| s.reverse && *t == "l"));
        assert!(runs.iter().any(|(t, s)| s.reverse && s.bold && *t == "lo"));
    }

    #[test]
    fn highlight_handles_korean() {
        let line = Line::single("가나다 ABC", Style::PLAIN);
        let hit = line.highlight("abc", Style::PLAIN.reverse());
        assert_eq!(hit.plain(), "가나다 ABC");
        assert!(hit.runs().any(|(t, s)| s.reverse && t == "ABC"));
    }

    /// CJK(폭 2)가 섞여도 열 좌표가 바이트가 아니라 화면 칸 기준으로 올바르게 구간을 잡는다.
    #[test]
    fn highlight_span_handles_wide_characters() {
        let line = Line::single("가나 링크", Style::PLAIN);
        // "가나 "는 폭 5(2+2+1), 그 뒤 "링크"(폭 4)가 5..9.
        let hit = line.highlight_span(5, 9, Style::PLAIN.reverse());
        assert_eq!(hit.plain(), "가나 링크");
        assert!(hit.runs().any(|(t, s)| s.reverse && t == "링크"));
    }

    #[test]
    fn highlight_span_out_of_range_returns_clone_unchanged() {
        let line = Line::single("abc", Style::PLAIN);
        assert_eq!(line.highlight_span(10, 20, Style::PLAIN.reverse()), line);
        assert_eq!(line.highlight_span(2, 1, Style::PLAIN.reverse()), line);
    }

    /// text_between_cols가 highlight_span과 같은 헬퍼를 공유해 CJK 폭에서도 올바른 부분
    /// 문자열을 돌려준다(드래그 선택 텍스트 추출용, markdown-source-view).
    #[test]
    fn text_between_cols_slices_by_display_width() {
        let line = Line::single("가나 링크", Style::PLAIN);
        assert_eq!(line.text_between_cols(0, 4), "가나");
        assert_eq!(line.text_between_cols(5, 9), "링크");
        assert_eq!(line.text_between_cols(0, 0), "");
        assert_eq!(line.text_between_cols(100, 200), "");
    }
}
