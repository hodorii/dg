//! 사분면 차트 배치.
//!
//! `0..1` 좌표 두 개로 항목을 네 구역에 흩뿌린다. 구역을 가르는 십자선은 점밭 밖에 제 줄·제 칸을
//! 따로 차지하므로 점이나 이름이 선을 지우는 일이 없고, 점은 언제나 제 사분면 안에만 머문다.
//! 축 라벨도 틀 바깥에 두어 점이 놓일 자리를 잡아먹지 않는다.
//!
//! ```text
//!               ▲ High Engagement
//! ┌───────────────────────┬───────────────────────┐
//! │     Need to promote   │    We should expand   │
//! │                       │ ● Campaign A          │
//! ├───────────────────────┼───────────────────────┤
//! │                       │ ● Campaign B          │
//! │      Re-evaluate      │    May be improved    │
//! └───────────────────────┴───────────────────────┘
//!               ▼ Low Engagement
//! ◀ Low Reach                            High Reach ▶
//! ```

use crate::diagram::canvas::{Canvas, LineKind};
use crate::diagram::layout::chart::{Axis, overflow_notice};
use crate::diagram::mermaid::quadrant::Quadrant;
use crate::line::{Line, Span};
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

/// 이보다 좁으면 네 구역이 뜻을 잃으므로 코드블록으로 물러난다.
const MIN_QUADRANT_WIDTH: usize = 24;
/// 테두리 두 줄과 가로 구분선 한 줄. 점밭 바깥에 언제나 이만큼이 더 붙는다.
const FRAME_OVERHEAD: usize = 3;
/// 점밭의 최소·최대 줄 수. 폭에 맞춰 이 사이에서 고른다.
const MIN_PLOT_ROWS: usize = 6;
const MAX_PLOT_ROWS: usize = 20;
/// 폭 몇 칸마다 점밭을 한 줄 늘릴지. 터미널 글자가 세로로 길어 가로보다 줄을 적게 잡는다.
const COLUMNS_PER_PLOT_ROW: usize = 4;
/// 이름을 적으려면 점 옆에 최소한 이만큼은 있어야 한다(`…` 한 칸만 남으면 뜻이 없다).
const MIN_NAME_WIDTH: usize = 2;
/// 점 표시.
const POINT_MARKER: char = '●';

/// 점 하나를 어디에 어떻게 놓을지. 칸 번호는 모두 점밭 기준이다.
struct Placement {
    /// 차지하는 첫 칸(이름이 왼쪽에 붙으면 이름의 첫 칸이다).
    start: usize,
    /// 차지하는 칸 수.
    span: usize,
    /// 점 표시가 놓일 칸. 언제나 좌표가 가리키는 칸이다.
    marker: usize,
    /// 이름이 놓일 칸과 글자. 이름을 못 적으면 `None`.
    name: Option<(usize, String)>,
    /// 이 자리를 골랐을 때 이름에서 잘려 나가는 폭. 이름을 아예 못 적으면 [`usize::MAX`]다.
    lost_width: usize,
}

/// 테두리와 십자선을 뺀 점밭. 칸 계산에 쓰는 자와 이미 글자가 놓인 자리를 함께 들고 있는다.
///
/// 칸·줄 번호는 모두 점밭 기준(0부터)이며 절반씩 나뉜다. 왼쪽 절반은 `0..half_columns`, 위쪽 절반은
/// `0..half_rows`다. 캔버스 좌표로 옮길 때 [`Plot::canvas_column`]·[`Plot::canvas_row`]가 테두리와
/// 구분선 몫을 더한다.
struct Plot {
    x_axis: Axis,
    y_axis: Axis,
    taken: Vec<Vec<bool>>,
}

impl Plot {
    /// 그릴 수 있는 폭에서 점밭을 잡는다. 양쪽 절반이 같은 크기가 되도록 칸 수·줄 수는 짝수다.
    fn new(width: usize) -> Option<Plot> {
        let columns = (width - FRAME_OVERHEAD) & !1;
        let rows = (columns / COLUMNS_PER_PLOT_ROW).clamp(MIN_PLOT_ROWS, MAX_PLOT_ROWS) & !1;
        Some(Plot {
            x_axis: Axis::new(0.0, 1.0, columns)?,
            y_axis: Axis::new(0.0, 1.0, rows)?,
            taken: vec![vec![false; columns]; rows],
        })
    }

    fn columns(&self) -> usize {
        self.x_axis.length()
    }

    fn rows(&self) -> usize {
        self.y_axis.length()
    }

    fn half_columns(&self) -> usize {
        self.columns() / 2
    }

    fn half_rows(&self) -> usize {
        self.rows() / 2
    }

    fn frame_width(&self) -> usize {
        self.columns() + FRAME_OVERHEAD
    }

    fn frame_height(&self) -> usize {
        self.rows() + FRAME_OVERHEAD
    }

    /// 왼쪽 테두리와 세로 구분선을 건너뛴 캔버스 칸.
    fn canvas_column(&self, column: usize) -> usize {
        1 + column + usize::from(column >= self.half_columns())
    }

    /// 위쪽 테두리와 가로 구분선을 건너뛴 캔버스 줄.
    fn canvas_row(&self, row: usize) -> usize {
        1 + row + usize::from(row >= self.half_rows())
    }

    fn is_free(&self, row: usize, start: usize, span: usize) -> bool {
        let Some(cells) = self.taken.get(row) else {
            return false;
        };
        start + span <= cells.len() && cells[start..start + span].iter().all(|cell| !cell)
    }

    fn mark(&mut self, row: usize, start: usize, span: usize) {
        if let Some(cells) = self.taken.get_mut(row) {
            for cell in cells.iter_mut().skip(start).take(span) {
                *cell = true;
            }
        }
    }
}

/// 사분면 차트를 그린다. 폭이 모자라거나 축을 만들 수 없으면 `None`(코드블록으로 대체된다).
pub fn render(quadrant: &Quadrant, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if width < MIN_QUADRANT_WIDTH {
        return None;
    }
    let mut plot = Plot::new(width)?;
    let (frame_width, frame_height) = (plot.frame_width(), plot.frame_height());

    let mut canvas = Canvas::new(frame_width, frame_height);
    canvas.rect(0, 0, frame_width, frame_height, LineKind::Solid, theme.diagram_line, false);
    canvas.vline(plot.half_columns() + 1, 0, frame_height - 1, LineKind::Solid, theme.rule);
    canvas.hline(0, frame_width - 1, plot.half_rows() + 1, LineKind::Solid, theme.rule);

    draw_quadrant_labels(quadrant, &mut canvas, &mut plot, theme);
    let hidden = draw_points(quadrant, &mut canvas, &mut plot, theme);

    let mut lines = Vec::new();
    if !quadrant.title.is_empty() {
        lines.push(centered(&quadrant.title, frame_width, theme.diagram_label));
    }
    if !quadrant.y_labels.high.is_empty() {
        lines.push(centered(&format!("▲ {}", quadrant.y_labels.high), frame_width, theme.diagram_caption));
    }
    lines.extend(canvas.into_lines());
    if !quadrant.y_labels.low.is_empty() {
        lines.push(centered(&format!("▼ {}", quadrant.y_labels.low), frame_width, theme.diagram_caption));
    }
    if let Some(line) = axis_ends_line(&quadrant.x_labels.low, &quadrant.x_labels.high, frame_width, theme) {
        lines.push(line);
    }
    if hidden > 0 {
        lines.push(Line::single(truncate(&overflow_notice(hidden), frame_width), theme.diagram_caption));
    }
    Some(lines)
}

/// 구역 라벨을 네 귀퉁이 줄(위 두 구역은 첫 줄, 아래 두 구역은 마지막 줄) 가운데에 적는다.
///
/// 라벨이 점밭 한가운데를 가로막지 않도록 바깥쪽 줄에 붙인다. 적은 자리는 점이 못 쓰도록 막는다.
fn draw_quadrant_labels(quadrant: &Quadrant, canvas: &mut Canvas, plot: &mut Plot, theme: &Theme) {
    let left = (0, plot.half_columns());
    let right = (plot.half_columns(), plot.columns());
    let bottom_row = plot.rows() - 1;
    // quadrant-1은 오른쪽 위, 거기서 반시계 방향이다.
    let regions = [(right, 0usize), (left, 0), (left, bottom_row), (right, bottom_row)];
    for (label, ((start, end), row)) in quadrant.quadrant_labels.iter().zip(regions) {
        if label.is_empty() || end <= start {
            continue;
        }
        let region_width = end - start;
        let text = truncate(label, region_width);
        let text_width = width_of(&text);
        let column = start + (region_width - text_width) / 2;
        canvas.text(plot.canvas_column(column), plot.canvas_row(row), &text, theme.diagram_group);
        plot.mark(row, column, text_width);
    }
}

/// 점을 놓고 자리가 없어 못 놓은 개수를 돌려준다.
fn draw_points(quadrant: &Quadrant, canvas: &mut Canvas, plot: &mut Plot, theme: &Theme) -> usize {
    let mut hidden = 0;
    for point in &quadrant.points {
        let column = plot.x_axis.position_of(point.x);
        let row = plot.rows() - 1 - plot.y_axis.position_of(point.y);
        let candidates = placements_for(&point.name, column, plot.half_columns(), plot.columns());
        let Some((row, placement)) = choose_row(&candidates, plot, row) else {
            hidden += 1;
            continue;
        };
        let canvas_row = plot.canvas_row(row);
        canvas.put(plot.canvas_column(placement.marker), canvas_row, POINT_MARKER, theme.diagram_accent);
        if let Some((column, name)) = &placement.name {
            canvas.text(plot.canvas_column(*column), canvas_row, name, theme.diagram_text);
        }
        let (start, span) = (placement.start, placement.span);
        plot.mark(row, start, span);
    }
    hidden
}

/// 점 하나를 놓을 수 있는 자리 후보. 이름이 덜 잘리는 쪽을 앞세우고, 같으면 오른쪽, 끝으로 점만.
///
/// 어느 쪽이든 점 표시는 좌표가 가리키는 칸 그대로이고, 이름은 점과 같은 절반 안에만 적는다(세로
/// 구분선을 넘어가면 다른 사분면 글자처럼 보인다). 옆자리가 좁으면 이름을 `…`로 줄인다.
fn placements_for(name: &str, column: usize, half_columns: usize, columns: usize) -> Vec<Placement> {
    let (half_start, half_end) = if column < half_columns { (0, half_columns) } else { (half_columns, columns) };
    let mut candidates = Vec::new();
    if !name.is_empty() {
        let right_room = half_end.saturating_sub(column + 2);
        if right_room >= MIN_NAME_WIDTH {
            let text = truncate(name, right_room);
            let lost_width = width_of(name).saturating_sub(width_of(&text));
            let span = 2 + width_of(&text);
            candidates.push(Placement { start: column, span, marker: column, name: Some((column + 2, text)), lost_width });
        }
        let left_room = column.saturating_sub(half_start + 1);
        if left_room >= MIN_NAME_WIDTH {
            let text = truncate(name, left_room);
            let lost_width = width_of(name).saturating_sub(width_of(&text));
            let span = width_of(&text) + 2;
            let start = column - 1 - width_of(&text);
            candidates.push(Placement { start, span, marker: column, name: Some((start, text)), lost_width });
        }
        candidates.sort_by_key(|candidate| candidate.lost_width);
    }
    candidates.push(Placement { start: column, span: 1, marker: column, name: None, lost_width: usize::MAX });
    candidates
}

/// 후보들을 좌표가 가리키는 줄부터 위아래로 훑어 빈 자리를 고른다.
///
/// 이름을 덜 잃는 후보를 *모든* 줄에서 먼저 시도한다. 줄 하나 밀리는 것보다 이름이 잘리는 쪽이 더
/// 큰 손해이기 때문이다. 줄을 옮겨도 제 절반을 벗어나지 않으므로 점이 다른 사분면으로 새지 않는다.
/// 어느 줄에도 자리가 없으면 `None`이다.
fn choose_row<'a>(candidates: &'a [Placement], plot: &Plot, row: usize) -> Option<(usize, &'a Placement)> {
    let (first, last) = if row < plot.half_rows() { (0, plot.half_rows() - 1) } else { (plot.half_rows(), plot.rows() - 1) };
    let rows = nearby_rows(row, first, last);
    let mut group_start = 0;
    while let Some(lost_width) = candidates.get(group_start).map(|candidate| candidate.lost_width) {
        let group_end = group_start + candidates[group_start..].partition_point(|candidate| candidate.lost_width == lost_width);
        for row in &rows {
            for candidate in &candidates[group_start..group_end] {
                if plot.is_free(*row, candidate.start, candidate.span) {
                    return Some((*row, candidate));
                }
            }
        }
        group_start = group_end;
    }
    None
}

/// `row`에서 가까운 줄부터 차례로. 아래쪽을 먼저 봐서 이름이 아래로 쌓이게 한다.
fn nearby_rows(row: usize, first: usize, last: usize) -> Vec<usize> {
    let mut rows = Vec::new();
    if row < first || row > last {
        return rows;
    }
    rows.push(row);
    for distance in 1..=(last - first) {
        if row + distance <= last {
            rows.push(row + distance);
        }
        if row.checked_sub(distance).is_some_and(|up| up >= first) {
            rows.push(row - distance);
        }
    }
    rows
}

fn centered(text: &str, width: usize, style: Style) -> Line {
    let text = truncate(text, width);
    let mut line = Line::empty();
    line.push_str(&" ".repeat((width - width_of(&text)) / 2), Style::PLAIN);
    line.push(Span::new(text, style));
    line
}

/// 가로축 양끝 라벨을 한 줄에 좌우로 벌려 놓는다. 둘 다 비었으면 `None`.
fn axis_ends_line(low: &str, high: &str, width: usize, theme: &Theme) -> Option<Line> {
    let mut left = if low.is_empty() { String::new() } else { format!("◀ {low}") };
    let mut right = if high.is_empty() { String::new() } else { format!("{high} ▶") };
    if left.is_empty() && right.is_empty() {
        return None;
    }
    if width_of(&left) + width_of(&right) + 1 > width {
        left = truncate(&left, width.saturating_sub(1) / 2);
        right = truncate(&right, width.saturating_sub(width_of(&left) + 1));
    }
    let mut line = Line::empty();
    line.push(Span::new(left.clone(), theme.diagram_caption));
    if !right.is_empty() {
        let gap = width - width_of(&left) - width_of(&right);
        line.push_str(&" ".repeat(gap), Style::PLAIN);
        line.push(Span::new(right, theme.diagram_caption));
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::mermaid::quadrant::parse;

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

    fn rows(source: &str, width: usize) -> Vec<String> {
        render(&parse(source), &Theme::none(), width).unwrap().iter().map(Line::plain).collect()
    }

    fn row_of(rows: &[String], needle: &str) -> usize {
        rows.iter().position(|row| row.contains(needle)).unwrap_or_else(|| panic!("{needle} 없음: {rows:#?}"))
    }

    /// 글자가 놓인 칸 번호. `str::find`는 바이트 자리라 한글이 섞이면 칸 번호와 다르다.
    fn column_of(row: &str, needle: &str) -> usize {
        let byte = row.find(needle).unwrap_or_else(|| panic!("{needle} 없음: {row}"));
        width_of(&row[..byte])
    }

    /// `column`번 칸에 놓인 글자. 넓은 글자가 그 칸에 걸쳐 있으면 `None`.
    fn char_at_column(row: &str, column: usize) -> Option<char> {
        let mut used = 0;
        for c in row.chars() {
            if used == column {
                return Some(c);
            }
            used += crate::text::char_width(c);
            if used > column {
                break;
            }
        }
        None
    }

    // 4.1 제목이 차트 위에 온다. 2.2 축 라벨이 양 끝에 온다.
    #[test]
    fn title_and_axis_labels_sit_outside_the_frame() {
        let rows = rows(SAMPLE, 60);
        assert_eq!(rows[0].trim(), "This is a sample example");
        assert_eq!(rows[1].trim(), "▲ High Engagement");
        assert_eq!(rows[2].chars().next(), Some('┌'));
        let bottom = row_of(&rows, "└");
        assert_eq!(rows[bottom + 1].trim(), "▼ Low Engagement");
        assert!(rows[bottom + 2].starts_with("◀ Low Reach"));
        assert!(rows[bottom + 2].ends_with("High Reach ▶"));
        assert!(rows.iter().all(|row| width_of(row) <= 60));
    }

    // 2.1 네 구역에 제 라벨이 하나씩 놓인다.
    #[test]
    fn four_quadrant_labels_land_in_their_own_regions() {
        let rows = rows(SAMPLE, 60);
        let cross = row_of(&rows, "┼");
        let top = row_of(&rows, "Need to promote");
        assert_eq!(top, row_of(&rows, "We should expand"));
        assert!(top < cross);
        let bottom = row_of(&rows, "Re-evaluate");
        assert_eq!(bottom, row_of(&rows, "May be improved"));
        assert!(bottom > cross);
        let divider = column_of(&rows[cross], "┼");
        assert!(column_of(&rows[top], "Need to promote") < divider);
        assert!(column_of(&rows[top], "We should expand") > divider);
        assert!(column_of(&rows[bottom], "Re-evaluate") < divider);
        assert!(column_of(&rows[bottom], "May be improved") > divider);
    }

    // 2.3 라벨을 뺀 구역은 비어 있고 나머지는 그대로 나온다.
    #[test]
    fn omitted_quadrant_labels_leave_their_region_empty() {
        let rows = rows("quadrantChart\n quadrant-1 오직 하나\n", 60);
        assert_eq!(rows.iter().filter(|row| row.contains("오직 하나")).count(), 1);
        let label = row_of(&rows, "오직 하나");
        let cross = row_of(&rows, "┼");
        let divider = column_of(&rows[cross], "┼");
        assert!(column_of(&rows[label], "오직 하나") > divider);
        // 그 줄에 있는 글자는 라벨 하나뿐이다: 나머지 구역은 테두리·구분선만 남는다.
        assert_eq!(rows[label].replace('│', " ").trim(), "오직 하나");
    }

    // 3.1 좌표가 가리키는 자리에 점과 이름이 놓인다.
    #[test]
    fn points_land_on_their_coordinates() {
        let rows = rows("quadrantChart\n 왼쪽아래: [0.0, 0.0]\n 오른쪽위: [1.0, 1.0]\n", 60);
        let low = row_of(&rows, "왼쪽아래");
        let high = row_of(&rows, "오른쪽위");
        assert!(high < low, "y가 큰 점이 위에 있어야 한다: {rows:#?}");
        assert_eq!(column_of(&rows[low], &POINT_MARKER.to_string()), 1);
        // 오른쪽 끝 점은 이름을 왼쪽에 달아 틀 밖으로 나가지 않는다.
        assert!(rows[high].trim_end().ends_with(&format!("{POINT_MARKER}│")));
        assert!(rows.iter().all(|row| width_of(row) <= 60));
    }

    // 3.2 좌표가 같아도 서로 겹치지 않고 둘 다 보인다.
    #[test]
    fn identical_coordinates_stack_without_overlapping() {
        let rows = rows("quadrantChart\n 하나: [0.5, 0.2]\n 둘: [0.5, 0.2]\n 셋: [0.51, 0.21]\n", 60);
        let (first, second, third) = (row_of(&rows, "하나"), row_of(&rows, "둘"), row_of(&rows, "셋"));
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
    }

    // 3.2 줄을 옮겨도 십자선을 넘지 않아 사분면이 바뀌지 않는다.
    #[test]
    fn crowded_points_stay_inside_their_own_half() {
        let source: String = (0..12).map(|index| format!(" 항목{index}: [0.5, 0.9]\n")).collect();
        let rows = rows(&format!("quadrantChart\n{source}"), 60);
        let cross = row_of(&rows, "┼");
        for index in 0..12 {
            let name = format!("항목{index}");
            if let Some(row) = rows.iter().position(|row| row.contains(&name)) {
                assert!(row < cross, "{name}이 아래쪽 절반으로 샜다: {rows:#?}");
            }
        }
    }

    // 2.1·3.1 점과 이름이 십자선을 지우지 않아 네 구역의 경계가 그대로 남는다.
    #[test]
    fn points_never_break_the_divider_lines() {
        let source: String = (0..40)
            .map(|index| format!(" 이름이 제법 긴 항목 {index}: [{}, {}]\n", index as f64 / 39.0, (index % 7) as f64 / 6.0))
            .collect();
        let rows = rows(&format!("quadrantChart\n{source}"), 80);
        let top = row_of(&rows, "┌");
        let divider = column_of(&rows[top], "┬");
        for row in rows.iter().filter(|row| row.starts_with(['┌', '│', '├', '└'])) {
            assert!(matches!(char_at_column(row, divider), Some('┬' | '│' | '┼' | '┴')), "{divider}칸이 지워졌다: {row}");
        }
    }

    // 5.1 범위 밖 좌표는 안쪽 끝으로 붙는다.
    #[test]
    fn out_of_range_coordinates_clamp_to_the_edge() {
        let clamped = rows("quadrantChart\n 밖: [9.0, -9.0]\n", 60);
        let inside = rows("quadrantChart\n 밖: [1.0, 0.0]\n", 60);
        assert_eq!(clamped, inside);
        assert!(clamped.iter().all(|row| width_of(row) <= 60));
    }

    // 5.2 자리를 넘는 항목은 몇 개가 빠졌는지 알린다.
    #[test]
    fn overflowing_points_report_how_many_were_dropped() {
        let source: String = (0..40).map(|index| format!(" 아주 긴 항목 이름 {index}: [0.5, 0.9]\n")).collect();
        let rows = rows(&format!("quadrantChart\n{source}"), 60);
        let notice = rows.last().unwrap();
        assert!(notice.contains("more"), "{rows:#?}");
    }

    // 5.2 폭이 모자라면 코드블록으로 물러난다.
    #[test]
    fn too_narrow_width_falls_back() {
        assert!(render(&parse(SAMPLE), &Theme::none(), MIN_QUADRANT_WIDTH - 1).is_none());
        assert!(render(&parse(SAMPLE), &Theme::none(), MIN_QUADRANT_WIDTH).is_some());
    }

    // 5.3 어떤 입력·폭에도 패닉하지 않고 폭을 지킨다.
    #[test]
    fn never_panics_across_edge_cases() {
        let long = "아주 길고 긴 항목 이름".repeat(20);
        let cases = [
            "quadrantChart".to_string(),
            "quadrantChart\n x-axis\n y-axis\n title\n".to_string(),
            "quadrantChart\n quadrant-1\n quadrant-4 \n".to_string(),
            format!("quadrantChart\n title {long}\n x-axis {long} --> {long}\n y-axis {long}\n quadrant-2 {long}\n {long}: [0.5, 0.5]\n"),
            "quadrantChart\n a: [0,0]\n b: [1,1]\n c: [0.5,0.5]\n d: [-1e9, 1e9]\n".to_string(),
            (0..200).map(|index| format!(" p{index}: [{}, {}]\n", index as f64 / 199.0, 1.0 - index as f64 / 199.0)).collect::<String>(),
        ];
        for theme in [Theme::none(), Theme::dark()] {
            for case in &cases {
                for width in [0usize, 1, 8, 23, 24, 40, 80, 200] {
                    if let Some(lines) = render(&parse(case), &theme, width) {
                        assert!(lines.iter().all(|line| line.width() <= width), "폭 {width}: {case}");
                    }
                }
            }
        }
    }
}
