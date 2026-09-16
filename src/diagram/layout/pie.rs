//! 파이 차트 배치.
//!
//! 터미널에 원을 그려 봐야 몫을 눈으로 재기 어렵다. 그래서 값의 합을 기준으로 삼은 가로 막대
//! ([`BarScale::Total`])로 옮긴다. 막대 길이가 곧 그 항목의 몫이라 조각 크기를 견주는 일이
//! 원보다 오히려 쉽다. 제목은 [`BarChart`]에 없으므로 여기서 한 줄을 따로 얹는다.

use super::chart::{BarChart, BarScale, ValueDisplay};
use crate::diagram::mermaid::pie::Pie;
use crate::line::Line;
use crate::style::Theme;
use crate::text::truncate;

/// 파이 차트를 줄 목록으로 그린다. 항목이 없거나 폭이 모자라면 `None`(코드블록으로 대체된다).
pub fn render(pie: &Pie, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if pie.items.is_empty() {
        return None;
    }
    // `showData`면 원본 값을 백분율과 나란히 적는다. 없으면 몫만 보인다.
    let value_display = if pie.show_data { ValueDisplay::ValueAndPercent } else { ValueDisplay::Percent };
    let chart = BarChart { scale: BarScale::Total, value_display, ..BarChart::new(&pie.items, width) };
    let bars = chart.render(theme)?;
    let Some(title) = title_line(pie, theme, width) else {
        return Some(bars);
    };
    let mut lines = vec![title];
    lines.extend(bars);
    Some(lines)
}

/// 제목 줄. 제목이 비었으면 `None`이라 줄을 만들지 않는다.
fn title_line(pie: &Pie, theme: &Theme, width: usize) -> Option<Line> {
    // 라벨 정리가 `<br>`을 줄바꿈으로 바꿔 놓을 수 있다. 제목은 한 줄이어야 폭 계산이 맞는다.
    let title = pie.title.replace('\n', " ");
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    Some(Line::single(truncate(title, width), theme.diagram_label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::mermaid::pie::parse;

    fn rows(pie: &Pie, width: usize) -> Vec<String> {
        render(pie, &Theme::none(), width).unwrap().iter().map(Line::plain).collect()
    }

    // 2.1·2.4 `showData`가 없으면 이름과 백분율이 함께 나온다.
    #[test]
    fn shows_name_and_percent_without_show_data() {
        let pie = parse("pie\n \"Dogs\" : 50\n \"Cats\" : 30\n \"Birds\" : 20\n");
        let rendered = rows(&pie, 40);
        assert_eq!(rendered.len(), 3);
        assert!(rendered[0].starts_with("Dogs") && rendered[0].ends_with("50.0%"), "{:?}", rendered[0]);
        assert!(rendered[1].starts_with("Cats") && rendered[1].ends_with("30.0%"), "{:?}", rendered[1]);
        assert!(rendered[2].starts_with("Birds") && rendered[2].ends_with("20.0%"), "{:?}", rendered[2]);
        // 원본 값은 나오지 않는다.
        assert!(!rendered[0].contains("50 "), "{:?}", rendered[0]);
    }

    // 2.3 `showData`면 원본 값이 백분율과 함께 나온다.
    #[test]
    fn shows_raw_value_with_show_data() {
        let rendered = rows(&parse("pie showData\n \"Dogs\" : 50\n \"Cats\" : 30\n \"Birds\" : 20\n"), 40);
        assert!(rendered[0].ends_with("50 (50.0%)"), "{:?}", rendered[0]);
        assert!(rendered[2].ends_with("20 (20.0%)"), "{:?}", rendered[2]);
    }

    // 2.2 제목은 막대 위 첫 줄에 온다.
    #[test]
    fn title_sits_above_the_bars() {
        let rendered = rows(&parse("pie title 반려동물 분포\n \"Dogs\" : 50\n \"Cats\" : 50\n"), 40);
        assert_eq!(rendered[0].trim(), "반려동물 분포");
        assert_eq!(rendered.len(), 3);
        // 제목이 없으면 그 줄도 없다.
        assert_eq!(rows(&parse("pie\n \"Dogs\" : 50\n \"Cats\" : 50\n"), 40).len(), 2);
    }

    // 3.1 막대 길이는 값에 비례하고, 합이 막대밭을 가득 채운다.
    #[test]
    fn bar_length_is_proportional_to_the_share() {
        let rendered = rows(&parse("pie\n \"A\" : 60\n \"B\" : 30\n \"C\" : 10\n"), 40);
        let bars: Vec<usize> = rendered.iter().map(|row| row.chars().filter(|c| *c == '█').count()).collect();
        assert!(bars[0] > bars[1] && bars[1] > bars[2], "{bars:?}");
        // A는 B의 두 배, B는 C의 세 배다(한 칸 반올림 오차까지 허용).
        assert!(bars[0].abs_diff(bars[1] * 2) <= 1, "{bars:?}");
        assert!(bars[1].abs_diff(bars[2] * 3) <= 1, "{bars:?}");
        // 합은 막대밭을 가득 채운다.
        assert!(bars.iter().sum::<usize>() >= 30, "{bars:?}");
    }

    // 4.1 항목이 없으면 그리지 않는다(코드블록으로 물러난다).
    #[test]
    fn no_items_renders_nothing() {
        assert!(render(&parse("pie"), &Theme::none(), 40).is_none());
        assert!(render(&parse("pie showData title 제목만 있다"), &Theme::none(), 40).is_none());
    }

    // 4.1 값이 모두 0이어도 오류 없이 항목 줄이 나온다(막대만 비어 있다).
    #[test]
    fn all_zero_values_render_empty_bars() {
        let rendered = rows(&parse("pie\n \"A\" : 0\n \"B\" : 0\n"), 40);
        assert_eq!(rendered.len(), 2);
        assert!(rendered.iter().all(|row| !row.contains('█')), "{rendered:?}");
        assert!(rendered[0].ends_with("0.0%"), "{:?}", rendered[0]);
    }

    // 4.2 이스케이프된 큰따옴표는 풀린 채로 그려진다.
    #[test]
    fn escaped_quotes_appear_unescaped() {
        let rendered = rows(&parse("pie\n \"그는 #quot;예#quot;\" : 4\n \"아니오\" : 6\n"), 40);
        assert!(rendered[0].contains("그는 \"예\""), "{:?}", rendered[0]);
    }

    // 4.3 폭이 모자라면 그리지 않는다.
    #[test]
    fn too_narrow_returns_none() {
        let pie = parse("pie showData\n \"아주 긴 항목 이름을 넣어 봅니다\" : 123456\n \"B\" : 1\n");
        assert!(render(&pie, &Theme::none(), 4).is_none());
        // 넉넉하면 폭 안에 들어온다.
        let lines = render(&pie, &Theme::none(), 60).unwrap();
        assert!(lines.iter().all(|line| line.width() <= 60));
    }

    // 4.3 제목이 길어도 폭을 넘지 않는다.
    #[test]
    fn long_title_is_truncated_to_width() {
        let pie = parse("pie title 아주 길고 긴 제목을 붙여 보면 과연 어떻게 되는지 확인해 봅니다\n \"A\" : 1\n");
        let lines = render(&pie, &Theme::none(), 30).unwrap();
        assert!(lines.iter().all(|line| line.width() <= 30), "{:?}", lines.iter().map(Line::plain).collect::<Vec<_>>());
    }

    // 4.4 어떤 폭·입력에도 패닉하지 않는다.
    #[test]
    fn never_panics_across_widths() {
        for source in [
            "pie",
            "pie showData title t\n \"A\" : 1e400\n \"B\" : NaN\n \"C\" : -5\n",
            "pie\n \"\" : 0\n",
            "pie title 제목<br/>두 줄\n \"A\" : 1\n",
        ] {
            let pie = parse(source);
            for width in [0usize, 1, 3, 8, 20, 40, 200] {
                let _ = render(&pie, &Theme::none(), width);
                let _ = render(&pie, &Theme::dark(), width);
            }
        }
    }
}
