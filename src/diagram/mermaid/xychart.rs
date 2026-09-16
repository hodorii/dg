//! mermaid `xychart-beta` 문법 파싱.
//!
//! 지시어는 다섯 가지뿐이다: `title`, `x-axis`, `y-axis`, `bar`, `line`. 모르는 줄은 조용히 버린다.
//! 값은 카테고리 순서대로 놓이므로 개수가 카테고리와 달라도 파싱 단계에서는 그대로 두고, 맞추는 일은
//! 배치기가 한다.

use crate::diagram::mermaid::text::{clean_lines, keyword, label};

/// 계열을 어떤 그림으로 그릴지.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKind {
    /// 카테고리마다 세로 막대.
    Bar,
    /// 카테고리 지점을 잇는 꺾은선.
    Line,
}

/// 값 계열 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    /// 범례에 적을 이름. `bar [...]`처럼 이름을 안 주면 `None`이다.
    pub name: Option<String>,
    pub kind: SeriesKind,
    /// 카테고리 순서대로의 값. 숫자로 못 읽은 자리는 NaN이라 그리지 않는다.
    pub values: Vec<f64>,
}

/// xy 차트 한 벌.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XyChart {
    pub title: Option<String>,
    /// 가로축 제목.
    pub x_title: Option<String>,
    /// 가로축 카테고리. 비어 있으면 배치기가 1부터의 번호를 쓴다.
    pub categories: Vec<String>,
    pub y_title: Option<String>,
    /// 세로축 최소~최대. 없으면 값에서 자동으로 잡는다.
    pub y_range: Option<(f64, f64)>,
    pub series: Vec<Series>,
}

/// 소스를 [`XyChart`]로 읽는다. 어떤 입력에도 실패하지 않는다(모르는 줄은 버린다).
pub fn parse(source: &str) -> XyChart {
    let mut chart = XyChart::default();
    for raw in clean_lines(source) {
        let trimmed = raw.trim();
        let directive = keyword(trimmed).to_ascii_lowercase();
        let rest = trimmed.split_once(char::is_whitespace).map(|(_, rest)| rest.trim()).unwrap_or("");
        match directive.as_str() {
            "title" => chart.title = non_empty(label(rest)),
            "x-axis" => {
                let (title, categories) = parse_x_axis(rest);
                chart.x_title = title;
                chart.categories = categories;
            }
            "y-axis" => {
                let (title, range) = split_title_and_range(rest);
                chart.y_title = title;
                chart.y_range = range;
            }
            "bar" | "line" => {
                let kind = if directive == "bar" { SeriesKind::Bar } else { SeriesKind::Line };
                if let Some(series) = parse_series(kind, rest) {
                    chart.series.push(series);
                }
            }
            _ => {}
        }
    }
    chart
}

fn non_empty(text: String) -> Option<String> {
    if text.trim().is_empty() { None } else { Some(text) }
}

/// `"제목" [a, b, c]` 또는 `제목 0 --> 10`. 숫자 범위 꼴은 카테고리 없이 제목만 남긴다.
fn parse_x_axis(rest: &str) -> (Option<String>, Vec<String>) {
    match bracketed(rest) {
        Some((before, inside)) => (non_empty(label(before)), split_items(inside).into_iter().map(|item| label(&item)).collect()),
        None => (split_title_and_range(rest).0, Vec::new()),
    }
}

/// `"이름" [1, 2, 3]`에서 계열을 읽는다. 대괄호가 없거나 값이 하나도 없으면 계열이 아니다.
fn parse_series(kind: SeriesKind, rest: &str) -> Option<Series> {
    let (before, inside) = bracketed(rest)?;
    let values: Vec<f64> = split_items(inside).iter().map(|item| number_of(item).unwrap_or(f64::NAN)).collect();
    if values.is_empty() {
        return None;
    }
    Some(Series { name: non_empty(label(before)), kind, values })
}

/// `제목 min --> max`를 제목과 범위로 가른다. 숫자를 못 읽으면 전체가 제목이다.
fn split_title_and_range(rest: &str) -> (Option<String>, Option<(f64, f64)>) {
    if let Some((left, right)) = rest.split_once("-->") {
        let left = left.trim();
        let (title, min) = match left.rsplit_once(char::is_whitespace) {
            Some((head, tail)) => (head, number_of(tail)),
            None => ("", number_of(left)),
        };
        if let (Some(min), Some(max)) = (min, number_of(right)) {
            return (non_empty(label(title)), Some((min, max)));
        }
    }
    (non_empty(label(rest)), None)
}

/// 대괄호 안팎을 가른다: (여는 괄호 앞, 괄호 안).
fn bracketed(rest: &str) -> Option<(&str, &str)> {
    let open = rest.find('[')?;
    let close = rest.rfind(']')?;
    if close < open { None } else { Some((&rest[..open], &rest[open + 1..close])) }
}

/// 따옴표 안의 쉼표는 건너뛰고 쉼표로 가른다.
fn split_items(inside: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for c in inside.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            ',' if !in_quote => items.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    items.push(current);
    items.into_iter().filter(|item| !item.trim().is_empty()).collect()
}

/// 값 하나를 읽는다. 뒤에 붙은 점 라벨(`1.2 "label"`)은 버린다.
fn number_of(item: &str) -> Option<f64> {
    let token = item.trim().split([' ', '\t', '"']).find(|part| !part.is_empty())?;
    token.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(chart: &XyChart, index: usize) -> Vec<f64> {
        chart.series[index].values.clone()
    }

    // 2.1 카테고리 목록을 순서대로 읽는다.
    #[test]
    fn reads_title_categories_and_range() {
        let chart = parse(
            "xychart-beta\n    title \"This is a simple example\"\n    x-axis \"title with space\" [cat1, \"cat2 with space\", cat3]\n    y-axis \"y title\" 0 --> 100\n    bar [2.3, 45, .98, -3.4]\n",
        );
        assert_eq!(chart.title.as_deref(), Some("This is a simple example"));
        assert_eq!(chart.x_title.as_deref(), Some("title with space"));
        assert_eq!(chart.categories, vec!["cat1", "cat2 with space", "cat3"]);
        assert_eq!(chart.y_title.as_deref(), Some("y title"));
        assert_eq!(chart.y_range, Some((0.0, 100.0)));
        assert_eq!(values(&chart, 0), vec![2.3, 45.0, 0.98, -3.4]);
        assert_eq!(chart.series[0].kind, SeriesKind::Bar);
        assert_eq!(chart.series[0].name, None);
    }

    // 2.3 범위를 안 주면 `y_range`가 없다(배치기가 자동으로 잡는다).
    #[test]
    fn y_axis_range_and_title_are_each_optional() {
        assert_eq!(parse("xychart\ny-axis \"only title\"\n").y_range, None);
        assert_eq!(parse("xychart\ny-axis \"only title\"\n").y_title.as_deref(), Some("only title"));
        let no_title = parse("xychart\ny-axis 0 --> 40\n");
        assert_eq!(no_title.y_title, None);
        assert_eq!(no_title.y_range, Some((0.0, 40.0)));
        let bare_word = parse("xychart\ny-axis revenue 10 --> 40\n");
        assert_eq!(bare_word.y_title.as_deref(), Some("revenue"));
        assert_eq!(bare_word.y_range, Some((10.0, 40.0)));
        // 숫자가 아니면 통째로 제목이다.
        let broken = parse("xychart\ny-axis a --> b\n");
        assert_eq!(broken.y_title.as_deref(), Some("a --> b"));
        assert_eq!(broken.y_range, None);
    }

    // 3.3 이름 붙은 bar·line 계열이 함께 온다.
    #[test]
    fn reads_multiple_named_series() {
        let chart = parse("xychart-beta\nbar \"매출\" [1, 2]\nline \"추세\" [2, 1]\nbar [3, 4]\n");
        assert_eq!(chart.series.len(), 3);
        assert_eq!(chart.series[0].name.as_deref(), Some("매출"));
        assert_eq!(chart.series[1].kind, SeriesKind::Line);
        assert_eq!(chart.series[2].name, None);
        assert_eq!(values(&chart, 1), vec![2.0, 1.0]);
    }

    // 점 라벨은 읽지 않고 버린다.
    #[test]
    fn point_labels_are_dropped_without_breaking_values() {
        let chart = parse("xychart-beta\nline [1.2 \"label1\", 2.5 \"label2\", 3.1]\n");
        assert_eq!(values(&chart, 0), vec![1.2, 2.5, 3.1]);
    }

    // 5.3 이상한 줄에도 파싱이 멈추지 않는다.
    #[test]
    fn malformed_lines_are_ignored() {
        let chart = parse("xychart-beta\nbar\nbar [\nline []\nx-axis\nnonsense here\nbar [1, oops, 3]\n");
        assert_eq!(chart.categories, Vec::<String>::new());
        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.series[0].values.len(), 3);
        assert!(chart.series[0].values[1].is_nan());
        assert!(parse("").series.is_empty());
    }

    // 숫자 범위 꼴 x축은 카테고리 없이 제목만 남는다.
    #[test]
    fn numeric_x_axis_keeps_title_only() {
        let chart = parse("xychart-beta\nx-axis \"시간\" 0 --> 10\nline [1, 2]\n");
        assert_eq!(chart.x_title.as_deref(), Some("시간"));
        assert!(chart.categories.is_empty());
    }
}
