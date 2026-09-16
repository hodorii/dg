//! mermaid `quadrantChart` 파서.
//!
//! ```text
//! quadrantChart
//!     title This is a sample example
//!     x-axis Low Reach --> High Reach
//!     y-axis Low Engagement --> High Engagement
//!     quadrant-1 We should expand
//!     Campaign A: [0.3, 0.6]
//! ```
//!
//! 점 뒤에 붙는 꾸밈(`radius:`·`color:`·`:::클래스`)은 글자 그림에서 뜻이 없으므로 읽고 버린다.

use crate::diagram::mermaid::text;

/// 축 양 끝 라벨. 한쪽만 준 `x-axis <글>` 꼴이면 [`AxisLabels::high`]가 빈 글자다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AxisLabels {
    /// 값이 작은 쪽(가로축은 왼쪽, 세로축은 아래쪽).
    pub low: String,
    /// 값이 큰 쪽(가로축은 오른쪽, 세로축은 위쪽).
    pub high: String,
}

/// 좌표 위에 놓일 항목 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct QuadrantPoint {
    pub name: String,
    /// 가로 좌표. 원문 그대로이며 `0..1` 밖일 수도 있다(그릴 때 안쪽으로 붙인다).
    pub x: f64,
    /// 세로 좌표. 위로 갈수록 커진다.
    pub y: f64,
}

/// 사분면 차트 한 장.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quadrant {
    pub title: String,
    pub x_labels: AxisLabels,
    pub y_labels: AxisLabels,
    /// `quadrant-1`~`quadrant-4` 라벨. 0번이 1사분면(오른쪽 위)이고 반시계로 돈다. 없으면 빈 글자다.
    pub quadrant_labels: [String; 4],
    pub points: Vec<QuadrantPoint>,
}

/// 소스를 [`Quadrant`]로 읽는다. 알아볼 수 없는 줄은 건너뛰며 어떤 입력에도 패닉하지 않는다.
pub fn parse(source: &str) -> Quadrant {
    let mut quadrant = Quadrant::default();
    for line in text::clean_lines(source) {
        let trimmed = line.trim();
        let keyword = text::keyword(trimmed).to_ascii_lowercase();
        let rest = trimmed[keyword.len()..].trim();
        match keyword.as_str() {
            "quadrantchart" | "classdef" => continue,
            "title" => quadrant.title = one_line(&text::label(rest)),
            "x-axis" => quadrant.x_labels = parse_axis_labels(rest),
            "y-axis" => quadrant.y_labels = parse_axis_labels(rest),
            "quadrant-1" => quadrant.quadrant_labels[0] = one_line(&text::label(rest)),
            "quadrant-2" => quadrant.quadrant_labels[1] = one_line(&text::label(rest)),
            "quadrant-3" => quadrant.quadrant_labels[2] = one_line(&text::label(rest)),
            "quadrant-4" => quadrant.quadrant_labels[3] = one_line(&text::label(rest)),
            _ => {
                if let Some(point) = parse_point(trimmed) {
                    quadrant.points.push(point);
                }
            }
        }
    }
    quadrant
}

/// `낮음 --> 높음` 또는 `한쪽만`을 양 끝 라벨로 가른다.
fn parse_axis_labels(rest: &str) -> AxisLabels {
    match rest.split_once("-->") {
        Some((low, high)) => AxisLabels { low: one_line(&text::label(low)), high: one_line(&text::label(high)) },
        None => AxisLabels { low: one_line(&text::label(rest)), high: String::new() },
    }
}

/// `이름: [x, y]` 한 줄을 점으로 읽는다. 대괄호 뒤의 꾸밈은 무시한다.
fn parse_point(line: &str) -> Option<QuadrantPoint> {
    let open = line.find('[')?;
    let close = line[open..].find(']')? + open;
    let (x, y) = line[open + 1..close].split_once(',')?;
    let x = x.trim().parse::<f64>().ok().filter(|value| value.is_finite())?;
    let y = y.trim().parse::<f64>().ok().filter(|value| value.is_finite())?;
    let head = line[..open].trim_end();
    let head = head.strip_suffix(':').unwrap_or(head);
    // `이름:::클래스`의 클래스 지정은 글자 그림에서 그릴 것이 없다.
    let head = head.split(":::").next().unwrap_or(head);
    Some(QuadrantPoint { name: one_line(&text::label(head)), x, y })
}

/// 여러 줄 라벨을 한 줄로 편다. 사분면 차트의 라벨 자리는 모두 한 줄뿐이다.
fn one_line(label: &str) -> String {
    label.split('\n').map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "quadrantChart
    title This is a sample example
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
";

    // 4.1 title, 2.1 quadrant-1~4, 2.2 축 양끝 라벨, 3.1 좌표 항목.
    #[test]
    fn reads_every_part_of_the_official_sample() {
        let quadrant = parse(SAMPLE);
        assert_eq!(quadrant.title, "This is a sample example");
        assert_eq!(quadrant.x_labels, AxisLabels { low: "Low Reach".into(), high: "High Reach".into() });
        assert_eq!(quadrant.y_labels, AxisLabels { low: "Low Engagement".into(), high: "High Engagement".into() });
        assert_eq!(quadrant.quadrant_labels, ["We should expand", "Need to promote", "Re-evaluate", "May be improved"].map(String::from));
        assert_eq!(
            quadrant.points,
            vec![
                QuadrantPoint { name: "Campaign A".into(), x: 0.3, y: 0.6 },
                QuadrantPoint { name: "Campaign B".into(), x: 0.45, y: 0.23 },
            ]
        );
    }

    // 2.3 일부 사분면 라벨을 빼도 나머지는 그대로다.
    #[test]
    fn omitted_quadrant_labels_stay_empty() {
        let quadrant = parse("quadrantChart\n quadrant-2 왼쪽 위\n quadrant-4 오른쪽 아래\n");
        assert_eq!(quadrant.quadrant_labels, ["", "왼쪽 위", "", "오른쪽 아래"].map(String::from));
    }

    // 2.2 한쪽 라벨만 준 축은 작은 쪽만 채운다.
    #[test]
    fn single_label_axis_fills_only_the_low_end() {
        let quadrant = parse("quadrantChart\n x-axis Reach\n y-axis \"Engagement\"\n");
        assert_eq!(quadrant.x_labels, AxisLabels { low: "Reach".into(), high: String::new() });
        assert_eq!(quadrant.y_labels, AxisLabels { low: "Engagement".into(), high: String::new() });
    }

    // 3.1 이름에 공백·따옴표가 있어도 좌표를 읽는다.
    #[test]
    fn point_names_may_contain_spaces_or_quotes() {
        let quadrant = parse("quadrantChart\n Campaign A: [0.3, 0.6]\n \"block-beta\": [0.43, 1.0]\n");
        let names: Vec<&str> = quadrant.points.iter().map(|point| point.name.as_str()).collect();
        assert_eq!(names, vec!["Campaign A", "block-beta"]);
    }

    // 꾸밈 문법은 읽고 버리되 점은 남긴다.
    #[test]
    fn styling_suffixes_are_ignored_but_the_point_survives() {
        let quadrant = parse(
            "quadrantChart\n classDef hot color: #f00\n Campaign A: [0.3, 0.6] radius: 5, color: #ff0000\n Campaign B:::hot: [0.2, 0.1]\n",
        );
        assert_eq!(
            quadrant.points,
            vec![
                QuadrantPoint { name: "Campaign A".into(), x: 0.3, y: 0.6 },
                QuadrantPoint { name: "Campaign B".into(), x: 0.2, y: 0.1 },
            ]
        );
    }

    // 5.1 범위 밖 좌표는 원문 그대로 남기고(그릴 때 보정), 숫자가 아니면 버린다.
    #[test]
    fn out_of_range_values_are_kept_and_non_numbers_are_dropped() {
        let quadrant = parse("quadrantChart\n over: [1.5, -0.2]\n bad: [a, b]\n short: [0.5]\n nan: [NaN, inf]\n");
        assert_eq!(quadrant.points, vec![QuadrantPoint { name: "over".into(), x: 1.5, y: -0.2 }]);
    }
}
