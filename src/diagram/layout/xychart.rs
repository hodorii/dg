//! xy 차트 배치.
//!
//! 세로축(값)과 가로축(카테고리)이 만드는 격자에 계열을 얹는다. 카테고리마다 같은 폭의 자리를 잡고,
//! 막대 계열은 그 자리를 나눠 서로 붙여 세우며, 선 계열은 자리 가운데 점을 찍어 이웃과 잇는다.
//!
//! 세로축 눈금은 [`chart::Axis`]가 정한다. 축 칸 번호는 아래에서 위로 커지므로 줄 번호로 바꿀 때
//! 뒤집는다(`plot_bottom - position`).

use crate::diagram::canvas::{Canvas, LineKind, NORTH, WEST};
use crate::diagram::layout::chart::{Axis, format_value};
use crate::diagram::mermaid::xychart::{Series, SeriesKind, XyChart};
use crate::line::Line;
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

/// 그림밭 높이(줄). 눈금이 서너 개는 잡히면서 터미널 한 화면을 넘지 않는 선이다.
const PLOT_HEIGHT: usize = 12;
/// 카테고리 한 자리의 최소 폭(막대 한 칸 + 사이 한 칸). 이보다 좁으면 그리기를 포기한다.
const MIN_SLOT_WIDTH: usize = 2;
/// 막대 하나가 가질 수 있는 최대 폭. 카테고리가 적어도 막대가 지나치게 뚱뚱해지지 않게 한다.
const MAX_BAR_WIDTH: usize = 8;
/// 막대 계열 칠감. 색 없는 테마에서도 계열이 구분되도록 글자부터 다르다.
const BAR_FILLS: [char; 4] = ['█', '▓', '▒', '░'];
/// 선 계열 점 표시. 칠감과 마찬가지로 글자로 구분된다.
const LINE_MARKERS: [char; 4] = ['●', '◆', '▲', '■'];

pub fn render(chart: &XyChart, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    let series: Vec<&Series> = chart.series.iter().filter(|series| !series.values.is_empty()).collect();
    if series.is_empty() {
        return None;
    }
    let labels = category_labels(chart, &series);
    let count = labels.len();
    if count == 0 {
        return None;
    }
    let axis = value_axis(chart, &series, count)?;
    let bar_count = series.iter().filter(|series| series.kind == SeriesKind::Bar).count();
    let grid = Grid::new(&axis, count, width)?.with_bars(bar_count)?;
    let styles = series_styles(&series, theme);

    let rows = Rows::new(chart);
    let mut canvas = Canvas::new(width, rows.height);
    draw_frame(&mut canvas, &axis, &grid, &rows, theme);
    draw_series(&mut canvas, &series, &styles, &axis, &grid, &rows);
    draw_labels(&mut canvas, chart, &labels, &grid, &rows, width, theme);

    let mut lines = canvas.into_lines();
    lines.extend(legend(&series, &styles, width, theme));
    Some(lines)
}

/// 가로축에 적을 이름. `x-axis`가 없으면 1부터의 번호로 대신한다.
fn category_labels(chart: &XyChart, series: &[&Series]) -> Vec<String> {
    if !chart.categories.is_empty() {
        return chart.categories.clone();
    }
    let count = series.iter().map(|series| series.values.len()).max().unwrap_or(0);
    (1..=count).map(|index| index.to_string()).collect()
}

/// 세로축. 범위를 줬으면 그대로, 없으면 카테고리 안에 드는 값들에서 자동으로 잡는다.
fn value_axis(chart: &XyChart, series: &[&Series], count: usize) -> Option<Axis> {
    match chart.y_range {
        Some((min, max)) => Axis::new(min, max, PLOT_HEIGHT),
        None => {
            let values: Vec<f64> =
                series.iter().flat_map(|series| series.values.iter().take(count).copied()).filter(|value| value.is_finite()).collect();
            Axis::from_values(&values, PLOT_HEIGHT)
        }
    }
}

/// 가로 칸 나누기. 눈금값 자리, 축 기둥 자리, 카테고리 한 자리의 폭을 정한다.
struct Grid {
    /// 카테고리 개수.
    count: usize,
    label_width: usize,
    axis_column: usize,
    plot_x0: usize,
    plot_width: usize,
    slot_width: usize,
    /// 자리에서 오른쪽 한 칸(이웃과의 틈)을 뺀 폭. 막대와 이름이 여기에 들어간다.
    bar_area: usize,
    bar_count: usize,
    bar_width: usize,
}

impl Grid {
    fn new(axis: &Axis, count: usize, width: usize) -> Option<Grid> {
        let label_width = axis.ticks().iter().map(|tick| width_of(&format_value(tick.value))).max().unwrap_or(0);
        let axis_column = label_width + 1;
        let plot_x0 = axis_column + 1;
        let slot_width = width.saturating_sub(plot_x0) / count;
        if slot_width < MIN_SLOT_WIDTH {
            return None;
        }
        // 그림밭은 카테고리가 실제로 쓰는 만큼만이다. 남는 오른쪽 여백까지 축을 긋지 않는다.
        let plot_width = count * slot_width;
        Some(Grid { count, label_width, axis_column, plot_x0, plot_width, slot_width, bar_area: slot_width - 1, bar_count: 0, bar_width: 0 })
    }

    /// 막대 계열 수에 맞춰 막대 폭을 정한다. 한 자리에 다 못 세우면 `None`이다.
    fn with_bars(mut self, bar_count: usize) -> Option<Grid> {
        if bar_count > self.bar_area {
            return None;
        }
        self.bar_count = bar_count;
        self.bar_width = self.bar_area.checked_div(bar_count).map_or(0, |width| width.clamp(1, MAX_BAR_WIDTH));
        Some(self)
    }

    fn slot_x(&self, category: usize) -> usize {
        self.plot_x0 + category * self.slot_width
    }

    /// 가로축 오른쪽 끝 칸.
    fn right_x(&self) -> usize {
        self.plot_x0 + self.plot_width - 1
    }

    /// 카테고리 자리의 가운데 칸. 선 계열 점과 가로축 눈금이 여기에 선다.
    fn center_x(&self, category: usize) -> usize {
        self.slot_x(category) + self.bar_area / 2
    }

    /// `ordinal`번째 막대 계열이 이 카테고리에서 차지하는 첫 칸.
    fn bar_x(&self, category: usize, ordinal: usize) -> usize {
        let group = self.bar_width * self.bar_count;
        self.center_x(category).saturating_sub(group / 2) + ordinal * self.bar_width
    }
}

/// 다음 줄 번호를 집어 가며 하나 밀어 둔다.
fn take(next: &mut usize) -> usize {
    *next += 1;
    *next - 1
}

/// 줄 번호 배치. 제목·축 제목은 있을 때만 한 줄씩 차지한다.
struct Rows {
    title: Option<usize>,
    y_title: Option<usize>,
    plot_bottom: usize,
    axis: usize,
    labels: usize,
    x_title: Option<usize>,
    height: usize,
}

impl Rows {
    fn new(chart: &XyChart) -> Rows {
        let mut next = 0;
        let title = chart.title.is_some().then(|| take(&mut next));
        let y_title = chart.y_title.is_some().then(|| take(&mut next));
        let plot_bottom = next + PLOT_HEIGHT - 1;
        let axis = plot_bottom + 1;
        let labels = axis + 1;
        next = labels + 1;
        let x_title = chart.x_title.is_some().then(|| take(&mut next));
        Rows { title, y_title, plot_bottom, axis, labels, x_title, height: next }
    }

    fn plot_top(&self) -> usize {
        self.plot_bottom + 1 - PLOT_HEIGHT
    }

    /// 값이 놓이는 줄. 축 칸 번호는 아래에서 위로 커지므로 뒤집는다.
    fn row_of(&self, axis: &Axis, value: f64) -> usize {
        self.plot_bottom - axis.position_of(value)
    }
}

/// 계열 하나를 어떤 글자·색으로 그릴지. 그림과 범례가 같은 것을 쓰도록 한 곳에서 정한다.
struct SeriesStyle {
    glyph: char,
    style: Style,
    /// 막대 계열이면 몇 번째 막대인지. 선 계열은 `None`이다.
    bar_ordinal: Option<usize>,
}

fn series_styles(series: &[&Series], theme: &Theme) -> Vec<SeriesStyle> {
    let palette = [theme.diagram_accent, theme.diagram_box, theme.diagram_group, theme.diagram_note];
    let (mut bars, mut lines) = (0usize, 0usize);
    series
        .iter()
        .map(|series| match series.kind {
            SeriesKind::Bar => {
                let ordinal = bars;
                bars += 1;
                SeriesStyle { glyph: BAR_FILLS[ordinal % BAR_FILLS.len()], style: palette[ordinal % palette.len()], bar_ordinal: Some(ordinal) }
            }
            SeriesKind::Line => {
                let ordinal = lines;
                lines += 1;
                SeriesStyle {
                    glyph: LINE_MARKERS[ordinal % LINE_MARKERS.len()],
                    style: palette[(ordinal + 1) % palette.len()],
                    bar_ordinal: None,
                }
            }
        })
        .collect()
}

/// 축 기둥·바닥선·눈금과 눈금값. 선부터 그어야 뒤에 오는 막대·점이 덮어쓸 수 있다.
fn draw_frame(canvas: &mut Canvas, axis: &Axis, grid: &Grid, rows: &Rows, theme: &Theme) {
    let right = grid.right_x();
    canvas.vline(grid.axis_column, rows.plot_top(), rows.axis, LineKind::Solid, theme.rule);
    // 맨 윗줄에 눈금이 걸려도 `┐`가 아니라 `┤`가 되도록 축이 위로 이어지는 것처럼 둔다.
    canvas.join(grid.axis_column, rows.plot_top(), NORTH, LineKind::Solid, theme.rule, false);
    canvas.hline(grid.axis_column, right, rows.axis, LineKind::Solid, theme.rule);
    if axis.min() < 0.0 && axis.max() > 0.0 {
        canvas.hline(grid.axis_column, right, rows.row_of(axis, 0.0), LineKind::Solid, theme.rule);
    }
    for tick in axis.ticks() {
        let row = rows.plot_bottom - tick.position;
        canvas.join(grid.axis_column, row, WEST, LineKind::Solid, theme.rule, false);
        let text = format_value(tick.value);
        canvas.text(grid.label_width.saturating_sub(width_of(&text)), row, &text, theme.diagram_caption);
    }
}

fn draw_series(canvas: &mut Canvas, series: &[&Series], styles: &[SeriesStyle], axis: &Axis, grid: &Grid, rows: &Rows) {
    let count = grid.count;
    let base_row = rows.row_of(axis, 0.0);
    for (series, style) in series.iter().zip(styles) {
        let Some(ordinal) = style.bar_ordinal else { continue };
        for (category, value) in series.values.iter().take(count).enumerate() {
            if !value.is_finite() || *value == 0.0 {
                continue;
            }
            let row = rows.row_of(axis, *value);
            let x0 = grid.bar_x(category, ordinal);
            for y in row.min(base_row)..=row.max(base_row) {
                for x in x0..x0 + grid.bar_width {
                    canvas.put(x, y, style.glyph, style.style);
                }
            }
        }
    }
    for (series, style) in series.iter().zip(styles) {
        if style.bar_ordinal.is_some() {
            continue;
        }
        let points: Vec<(usize, usize)> = series
            .values
            .iter()
            .take(count)
            .enumerate()
            .filter(|(_, value)| value.is_finite())
            .map(|(category, value)| (grid.center_x(category), rows.row_of(axis, *value)))
            .collect();
        draw_polyline(canvas, &points, style.style);
        for (x, y) in points {
            canvas.put(x, y, style.glyph, style.style);
        }
    }
}

/// 점과 점을 ㄱ자로 잇는다. 대각선 대신 가운데 열에서 한 번 꺾는 방식이라 칸 격자와 어긋나지 않는다.
///
/// 점이 바로 옆 칸이면 사이에 이을 자리가 없으므로 점만 남긴다.
fn draw_polyline(canvas: &mut Canvas, points: &[(usize, usize)], style: Style) {
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if x1 <= x0 + 1 {
            continue;
        }
        if y0 == y1 {
            for x in x0 + 1..x1 {
                canvas.put(x, y0, '─', style);
            }
            continue;
        }
        let turn = (x0 + x1) / 2;
        for x in x0 + 1..turn {
            canvas.put(x, y0, '─', style);
        }
        for x in turn + 1..x1 {
            canvas.put(x, y1, '─', style);
        }
        let descending = y1 > y0;
        canvas.put(turn, y0, if descending { '┐' } else { '┘' }, style);
        canvas.put(turn, y1, if descending { '└' } else { '┌' }, style);
        for y in y0.min(y1) + 1..y0.max(y1) {
            canvas.put(turn, y, '│', style);
        }
    }
}

fn draw_labels(canvas: &mut Canvas, chart: &XyChart, labels: &[String], grid: &Grid, rows: &Rows, width: usize, theme: &Theme) {
    for (category, label) in labels.iter().enumerate() {
        canvas.join(grid.center_x(category), rows.axis, NORTH, LineKind::Solid, theme.rule, false);
        let text = truncate(label, grid.bar_area);
        canvas.text_centered(grid.slot_x(category), rows.labels, grid.bar_area, &text, theme.diagram_text);
    }
    if let (Some(row), Some(title)) = (rows.title, chart.title.as_deref()) {
        canvas.text_centered(0, row, width, &truncate(title, width), theme.diagram_label);
    }
    // 세로축 제목은 눕혀 쓰면 한글이 뭉개지므로 축 위에 화살표와 함께 가로로 적는다.
    if let (Some(row), Some(title)) = (rows.y_title, chart.y_title.as_deref()) {
        canvas.text(0, row, &truncate(&format!("↑ {title}"), width), theme.diagram_label);
    }
    if let (Some(row), Some(title)) = (rows.x_title, chart.x_title.as_deref()) {
        canvas.text_centered(grid.plot_x0, row, grid.plot_width, &truncate(title, grid.plot_width), theme.diagram_label);
    }
}

/// 이름 붙은 계열을 `█ 이름` 꼴로 한 줄에 모은다. 이름이 하나도 없으면 줄을 만들지 않는다.
fn legend(series: &[&Series], styles: &[SeriesStyle], width: usize, theme: &Theme) -> Option<Line> {
    if !series.iter().any(|series| series.name.is_some()) {
        return None;
    }
    let mut line = Line::empty();
    let mut used = 0usize;
    for (series, style) in series.iter().zip(styles) {
        let Some(name) = series.name.as_deref() else { continue };
        let gap = if used == 0 { "" } else { "  " };
        let entry = format!("{gap}{} {name}", style.glyph);
        if used + width_of(&entry) > width {
            break;
        }
        used += width_of(&entry);
        line.push_str(gap, Style::PLAIN);
        line.push_str(&style.glyph.to_string(), style.style);
        line.push_str(&format!(" {name}"), theme.diagram_text);
    }
    if line.is_blank() { None } else { Some(line) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::mermaid::xychart::parse;

    fn rows_of(source: &str, width: usize) -> Vec<String> {
        render(&parse(source), &Theme::none(), width).unwrap().iter().map(Line::plain).collect()
    }

    const SAMPLE: &str = "xychart-beta\n  title \"판매량\"\n  x-axis [1월, 2월, 3월]\n  y-axis \"권\" 0 --> 100\n  bar [50, 100, 25]\n";

    // 2.1 카테고리 이름이 가로축에 순서대로 놓인다. 4.1 제목은 맨 위에 온다.
    #[test]
    fn draws_title_categories_and_ticks() {
        let rows = rows_of(SAMPLE, 40);
        assert!(rows[0].contains("판매량"));
        assert!(rows[1].contains("권"));
        let labels = rows.iter().find(|row| row.contains("1월")).unwrap();
        let (first, second, third) = (labels.find("1월").unwrap(), labels.find("2월").unwrap(), labels.find("3월").unwrap());
        assert!(first < second && second < third, "{labels}");
        // 2.2 눈금값이 세로축에 붙는다.
        assert!(rows.iter().any(|row| row.trim_start().starts_with("100")));
        assert!(rows.iter().any(|row| row.contains('└')));
    }

    // 3.1 막대 높이는 값에 비례한다.
    #[test]
    fn bar_heights_follow_values() {
        let rows = rows_of(SAMPLE, 40);
        let column_of = |label: &str| rows.iter().find(|row| row.contains("1월")).unwrap().find(label).unwrap();
        let filled = |column: usize| rows.iter().filter(|row| row.chars().nth(column) == Some('█')).count();
        let (january, february, march) = (column_of("1월"), column_of("2월"), column_of("3월"));
        assert!(filled(february) > filled(january), "{rows:?}");
        assert!(filled(january) > filled(march), "{rows:?}");
    }

    // 2.3 범위를 안 주면 값에서 눈금을 뽑는다.
    #[test]
    fn missing_range_uses_auto_scale() {
        let rows = rows_of("xychart-beta\nx-axis [a, b]\nbar [3, 47]\n", 40);
        assert!(rows.iter().any(|row| row.trim_start().starts_with("50")), "{rows:?}");
    }

    // 3.2 선 계열은 점과 이음선으로 그려진다.
    #[test]
    fn line_series_connects_points() {
        let rows = rows_of("xychart-beta\nx-axis [a, b, c]\ny-axis 0 --> 30\nline [10, 30, 20]\n", 40);
        let text = rows.join("\n");
        assert_eq!(text.matches('●').count(), 3, "{text}");
        assert!(text.contains('│') && text.contains('─'), "{text}");
    }

    // 3.3 막대와 선이 같은 축 위에 서로 다른 글자로 놓이고, 이름은 범례에 남는다.
    #[test]
    fn bar_and_line_share_the_grid_with_a_legend() {
        let rows = rows_of("xychart-beta\nx-axis [a, b, c]\ny-axis 0 --> 30\nbar \"판매\" [10, 30, 20]\nline \"추세\" [20, 10, 30]\n", 50);
        let text = rows.join("\n");
        assert!(text.contains('█') && text.contains('●'), "{text}");
        let legend = rows.last().unwrap();
        assert!(legend.contains("█ 판매") && legend.contains("● 추세"), "{legend}");
    }

    // 3.4 값 개수가 카테고리와 달라도 있는 만큼만 그린다.
    #[test]
    fn mismatched_series_length_draws_what_it_can() {
        let short = rows_of("xychart-beta\nx-axis [a, b, c, d]\ny-axis 0 --> 10\nbar [5, 10]\n", 40);
        let labels = short.iter().find(|row| row.contains('d')).unwrap();
        assert!(labels.contains('a') && labels.contains('d'));
        let long = rows_of("xychart-beta\nx-axis [a, b]\ny-axis 0 --> 10\nbar [5, 10, 7, 2]\n", 40);
        assert!(long.iter().all(|row| width_of(row) <= 40));
        // 카테고리를 넘는 값은 버리므로 바닥 줄의 막대 덩어리는 둘뿐이다.
        let bottom = long.iter().rev().find(|row| row.contains('█')).unwrap();
        assert_eq!(bottom.split(|c| c != '█').filter(|run| !run.is_empty()).count(), 2, "{bottom}");
    }

    // 5.1 카테고리가 폭에 견줘 너무 많으면 그리지 않는다(코드블록으로 물러난다).
    #[test]
    fn too_many_categories_fall_back() {
        let categories: Vec<String> = (0..60).map(|index| format!("c{index}")).collect();
        let source = format!("xychart-beta\nx-axis [{}]\nbar [1]\n", categories.join(", "));
        assert!(render(&parse(&source), &Theme::none(), 40).is_none());
        assert!(render(&parse(SAMPLE), &Theme::none(), 8).is_none());
    }

    // 5.2 음수는 0 기준선 아래로 뻗는다.
    #[test]
    fn negative_values_hang_below_the_baseline() {
        let rows = rows_of("xychart-beta\nx-axis [up, down]\nbar [10, -10]\n", 40);
        let baseline = rows.iter().position(|row| row.contains('├')).unwrap();
        let labels = rows.iter().find(|row| row.contains("down")).unwrap();
        let (up, down) = (labels.find("up").unwrap(), labels.find("down").unwrap());
        let filled = |column: usize| -> Vec<usize> {
            rows.iter().enumerate().filter(|(_, row)| row.chars().nth(column) == Some('█')).map(|(index, _)| index).collect()
        };
        assert!(!filled(up).is_empty() && filled(up).iter().all(|row| *row <= baseline), "{rows:?}");
        assert!(!filled(down).is_empty() && filled(down).iter().all(|row| *row >= baseline), "{rows:?}");
    }

    // 계열이 없으면 그릴 것이 없다.
    #[test]
    fn empty_chart_has_nothing_to_draw() {
        assert!(render(&parse("xychart-beta\nx-axis [a, b]\n"), &Theme::none(), 40).is_none());
        assert!(render(&parse("xychart-beta\n"), &Theme::none(), 40).is_none());
    }

    // 5.3 어떤 입력·폭에도 패닉하지 않고, 줄은 폭을 넘지 않는다.
    #[test]
    fn never_panics_across_edge_cases() {
        let long = "아주 긴 카테고리 이름".repeat(4);
        let sources = [
            SAMPLE.to_string(),
            "xychart-beta\nbar [1]\n".to_string(),
            "xychart\nx-axis [a]\ny-axis 5 --> 5\nbar [0]\nline [0]\n".to_string(),
            "xychart-beta\nx-axis [a, b]\ny-axis 100 --> 0\nbar [-1e300, 1e300]\n".to_string(),
            format!("xychart-beta\nx-axis [{long}, b]\ntitle {long}\nbar [1, 2]\nline [2, 1]\n"),
            "xychart-beta\nx-axis [a, b, c]\nbar [1, 2, 3]\nbar [3, 2, 1]\nbar [2, 2, 2]\nline [1, 3, 2]\nline [3, 1, 2]\n".to_string(),
            "xychart-beta\nx-axis [a, b, c]\ny-axis 0 --> 1\nbar [0.0001, -0.0001, 0]\n".to_string(),
        ];
        for theme in [Theme::none(), Theme::dark()] {
            for source in &sources {
                for width in [0usize, 1, 3, 8, 20, 40, 80, 200] {
                    if let Some(lines) = render(&parse(source), &theme, width) {
                        assert!(lines.iter().all(|line| line.width() <= width), "폭 {width}: {source}");
                    }
                }
            }
        }
    }
}
