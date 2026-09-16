//! `block-beta` 격자 배치.
//!
//! 묶음마다 `columns N` 격자를 만들고 선언 순서대로 왼쪽→오른쪽, 위→아래로 채운다.
//! 칸 넓이는 그 열에 놓인 상자의 최대 넓이, 여러 칸을 먹는 상자(`b:2`)는 모자란 만큼을
//! 먹는 열들에 나눠 더한다. 중첩 묶음은 제 격자를 재귀로 재고 테두리 상자로 감싼다.

use crate::diagram::canvas::{Canvas, LineKind, NORTH, SOUTH};
use crate::diagram::layout::shape;
use crate::diagram::mermaid::block::{Arrow, BlockDiagram, Group, Item};
use crate::line::Line;
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of, wrap_plain};

/// 열 사이 최소 간격. `──▶`가 들어갈 만큼이다.
const MIN_COLUMN_GAP: usize = 3;
/// 행 사이 간격. 꺾임 줄 하나와 화살촉 줄 하나.
const ROW_GAP: usize = 2;
/// 라벨 폭을 줄여 가며 다시 시도할 후보들.
const LABEL_CAPS: [usize; 5] = [48, 32, 24, 16, 8];
/// 터무니없는 입력에 거대한 격자를 할당하지 않게 막는 높이 상한.
const MAX_HEIGHT: usize = 4096;

pub fn render(diagram: &BlockDiagram, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if diagram.root.items.is_empty() {
        return None;
    }
    for cap in LABEL_CAPS {
        if let Some(lines) = build(diagram, theme, cap, width) {
            return Some(lines);
        }
    }
    None
}

#[derive(Clone, Copy)]
struct Gaps {
    column: usize,
    row: usize,
}

#[derive(Clone, Copy)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// 격자 한 칸에 놓인 항목의 잰 결과.
struct GridCell {
    /// (행, 열, 가로 칸 수)
    place: (usize, usize, usize),
    size: (usize, usize),
    /// 중첩 묶음이면 그 안쪽 배치.
    nested: Option<Box<Measured>>,
    sections: Vec<Vec<String>>,
}

struct Measured {
    width: usize,
    height: usize,
    column_widths: Vec<usize>,
    row_heights: Vec<usize>,
    cells: Vec<GridCell>,
}

fn build(diagram: &BlockDiagram, theme: &Theme, cap: usize, width: usize) -> Option<Vec<Line>> {
    let gaps = gaps_for(diagram, cap);
    let measured = measure(&diagram.root, cap, gaps);
    if measured.width == 0 || measured.height == 0 || measured.width > width || measured.height > MAX_HEIGHT {
        return None;
    }
    let mut canvas = Canvas::new(measured.width, measured.height);
    let mut rects: Vec<(String, Rect)> = Vec::new();
    draw(&mut canvas, 0, 0, &diagram.root, &measured, theme, gaps, 0, &mut rects);
    canvas.set_edge_mode(true);
    for arrow in &diagram.arrows {
        route(&mut canvas, &rects, arrow, theme, gaps, cap, measured.width);
    }
    let mut lines = canvas.into_lines();
    while lines.last().is_some_and(Line::is_blank) {
        lines.pop();
    }
    Some(lines)
}

/// 격자에 채운 자리: 항목마다 (행, 열, 가로 칸 수)와 열 수.
fn place(group: &Group) -> (Vec<(usize, usize, usize)>, usize) {
    let total: usize = group.items.iter().map(Item::span).sum();
    // `columns`가 없으면 한 줄에 다 늘어놓는 것이 기본이다.
    let columns = group.columns.unwrap_or(total).clamp(1, total.max(1));
    let mut places = Vec::with_capacity(group.items.len());
    let (mut row, mut column) = (0usize, 0usize);
    for item in &group.items {
        let span = item.span().clamp(1, columns);
        if column + span > columns && column > 0 {
            row += 1;
            column = 0;
        }
        places.push((row, column, span));
        column += span;
        if column >= columns {
            row += 1;
            column = 0;
        }
    }
    (places, columns)
}

fn measure(group: &Group, cap: usize, gaps: Gaps) -> Measured {
    let (places, columns) = place(group);
    let mut cells: Vec<GridCell> = Vec::with_capacity(group.items.len());
    for (item, place) in group.items.iter().zip(places.iter().copied()) {
        let cell = match item {
            Item::Space(_) => GridCell { place, size: (2, 1), nested: None, sections: Vec::new() },
            Item::Leaf(block) => {
                let sections = vec![wrap_plain(&block.label, cap)];
                let size = shape::measure(block.shape, &sections);
                GridCell { place, size, nested: None, sections }
            }
            Item::Nested(inner) => {
                let inner_measured = measure(inner, cap, gaps);
                // 테두리 한 칸 + 안쪽 여백 한 칸씩. 위 테두리에 얹는 제목도 들어가야 한다.
                let size = ((inner_measured.width + 4).max(title_width(inner) + 3), inner_measured.height + 2);
                GridCell { place, size, nested: Some(Box::new(inner_measured)), sections: Vec::new() }
            }
        };
        cells.push(cell);
    }
    let row_count = places.last().map(|(row, _, _)| row + 1).unwrap_or(0);
    let mut column_widths = vec![0usize; columns];
    for cell in cells.iter().filter(|cell| cell.place.2 == 1) {
        column_widths[cell.place.1] = column_widths[cell.place.1].max(cell.size.0);
    }
    // 여러 칸을 먹는 상자는 좁은 것부터 처리해야 넓힌 결과가 겹쳐 세지지 않는다.
    let mut spanning: Vec<usize> = (0..cells.len()).filter(|&index| cells[index].place.2 > 1).collect();
    spanning.sort_by_key(|&index| cells[index].place.2);
    for index in spanning {
        let (_, column, span) = cells[index].place;
        let available: usize = column_widths[column..column + span].iter().sum::<usize>() + (span - 1) * gaps.column;
        if cells[index].size.0 > available {
            let extra = cells[index].size.0 - available;
            for offset in 0..span {
                column_widths[column + offset] += extra / span + usize::from(offset < extra % span);
            }
        }
    }
    let mut row_heights = vec![0usize; row_count];
    for cell in &cells {
        row_heights[cell.place.0] = row_heights[cell.place.0].max(cell.size.1);
    }
    let width = column_widths.iter().sum::<usize>() + gaps.column * columns.saturating_sub(1);
    let height = row_heights.iter().sum::<usize>() + gaps.row * row_count.saturating_sub(1);
    Measured { width, height, column_widths, row_heights, cells }
}

/// `index`번째 칸이 시작하는 자리(앞 칸들과 간격의 합).
fn offset_of(sizes: &[usize], index: usize, gap: usize) -> usize {
    let index = index.min(sizes.len());
    sizes[..index].iter().sum::<usize>() + gap * index
}

/// `column`부터 `span`칸이 차지하는 넓이(사이 간격 포함).
fn span_width(widths: &[usize], column: usize, span: usize, gap: usize) -> usize {
    let start = column.min(widths.len());
    let end = (column + span).min(widths.len());
    widths[start..end].iter().sum::<usize>() + gap * (end - start).saturating_sub(1)
}

/// 묶음 위 테두리에 얹는 제목(` 아이디 `)의 폭. 이름 없는 묶음은 0.
fn title_width(group: &Group) -> usize {
    if group.id.is_empty() { 0 } else { width_of(&group.id) + 2 }
}

/// 중첩 깊이마다 테두리 선을 달리해 층을 구분한다.
fn border_kind(depth: usize) -> LineKind {
    match depth % 3 {
        1 => LineKind::Solid,
        2 => LineKind::Dashed,
        _ => LineKind::Heavy,
    }
}

fn draw(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    group: &Group,
    measured: &Measured,
    theme: &Theme,
    gaps: Gaps,
    depth: usize,
    rects: &mut Vec<(String, Rect)>,
) {
    for (item, cell) in group.items.iter().zip(measured.cells.iter()) {
        let (row, column, span) = cell.place;
        let rect = Rect {
            x: x + offset_of(&measured.column_widths, column, gaps.column),
            y: y + offset_of(&measured.row_heights, row, gaps.row),
            width: span_width(&measured.column_widths, column, span, gaps.column),
            height: measured.row_heights.get(row).copied().unwrap_or(0),
        };
        match item {
            Item::Space(_) => {}
            Item::Leaf(block) => {
                shape::draw(canvas, rect.x, rect.y, block.shape, &cell.sections, theme, rect.width, rect.height);
                rects.push((block.id.clone(), rect));
            }
            Item::Nested(inner) => {
                let Some(inner_measured) = &cell.nested else { continue };
                canvas.rect(rect.x, rect.y, rect.width, rect.height, border_kind(depth + 1), theme.diagram_group, false);
                if title_width(inner) > 0 && rect.width > 3 {
                    let title = truncate(&format!(" {} ", inner.id), rect.width - 3);
                    canvas.text(rect.x + 2, rect.y, &title, theme.diagram_caption);
                }
                let slack = rect.width.saturating_sub(4).saturating_sub(inner_measured.width);
                draw(canvas, rect.x + 2 + slack / 2, rect.y + 1, inner, inner_measured, theme, gaps, depth + 1, rects);
                rects.push((inner.id.clone(), rect));
            }
        }
    }
}

/// 화살표 라벨: 여러 줄이면 첫 줄만, 폭은 `cap`까지.
fn arrow_label(text: &str, cap: usize) -> String {
    match text.lines().next() {
        Some(first) if !first.is_empty() => truncate(first, cap),
        _ => String::new(),
    }
}

/// 같은 묶음·같은 행에 있는 화살표는 열 사이를 가로지르므로 라벨만큼 간격을 벌린다.
fn gaps_for(diagram: &BlockDiagram, cap: usize) -> Gaps {
    let mut positions: Vec<(String, usize, usize)> = Vec::new();
    collect_positions(&diagram.root, 0, &mut 0, &mut positions);
    let mut column = MIN_COLUMN_GAP;
    for arrow in &diagram.arrows {
        let text = arrow_label(&arrow.label, cap);
        if text.is_empty() {
            continue;
        }
        let from = positions.iter().find(|(id, _, _)| *id == arrow.from);
        let to = positions.iter().find(|(id, _, _)| *id == arrow.to);
        if let (Some(from), Some(to)) = (from, to)
            && from.1 == to.1
            && from.2 == to.2
        {
            column = column.max(width_of(&text) + 2);
        }
    }
    Gaps { column, row: ROW_GAP }
}

/// 아이디마다 (묶음 번호, 행)을 모은다. 크기를 재기 전에 화살표 방향을 가르는 데 쓴다.
fn collect_positions(group: &Group, key: usize, next_key: &mut usize, out: &mut Vec<(String, usize, usize)>) {
    let (places, _) = place(group);
    for (item, (row, _, _)) in group.items.iter().zip(places) {
        match item {
            Item::Space(_) => {}
            Item::Leaf(block) => out.push((block.id.clone(), key, row)),
            Item::Nested(inner) => {
                out.push((inner.id.clone(), key, row));
                *next_key += 1;
                let child = *next_key;
                collect_positions(inner, child, next_key, out);
            }
        }
    }
}

fn route(canvas: &mut Canvas, rects: &[(String, Rect)], arrow: &Arrow, theme: &Theme, gaps: Gaps, cap: usize, width: usize) {
    let find = |id: &str| rects.iter().find(|(name, _)| name == id).map(|(_, rect)| *rect);
    let (Some(from), Some(to)) = (find(&arrow.from), find(&arrow.to)) else { return };
    let text = arrow_label(&arrow.label, cap);
    let style = theme.diagram_line;
    // 세로로 겹치면서 좌우로 떨어져 있으면 두 상자 사이를 곧장 가로지른다.
    let overlaps = from.y.max(to.y) < (from.y + from.height).min(to.y + to.height);
    let sideways = if overlaps && to.x > from.x + from.width {
        Some((from.x + from.width, to.x - 1, true))
    } else if overlaps && from.x > to.x + to.width {
        Some((to.x + to.width, from.x - 1, false))
    } else {
        None
    };
    if let Some((start, end, rightward)) = sideways {
        let top = from.y.max(to.y);
        let row = (top + (from.y + from.height).min(to.y + to.height)) / 2;
        canvas.hline(start, end, row, LineKind::Solid, style);
        canvas.put(if rightward { end } else { start }, row, if rightward { '▶' } else { '◀' }, style);
        if !text.is_empty() && row > top {
            canvas.text_centered(start, row - 1, end - start + 1, &text, theme.diagram_label);
        }
        return;
    }
    let source_x = from.x + from.width / 2;
    let target_x = to.x + to.width / 2;
    let downward = to.y >= from.y + from.height;
    let upward = from.y >= to.y + to.height;
    if !downward && !upward {
        return;
    }
    // 화살촉 줄, 가로로 꺾는 줄, 그리고 양쪽 세로 토막의 구간.
    // 꺾는 줄은 목표 상자 바로 앞의 빈 띠 안에 둔다.
    let (head_row, band_row, source_stem, target_stem) = if downward {
        let head = to.y.saturating_sub(1);
        let band = to.y.saturating_sub(gaps.row).max(from.y + from.height).min(head);
        (head, band, (from.y + from.height, band), (band, head.saturating_sub(1)))
    } else {
        let head = to.y + to.height;
        let band = from.y.saturating_sub(gaps.row).max(head + 1).min(from.y.saturating_sub(1));
        (head, band, (band, from.y.saturating_sub(1)), (head + 1, band))
    };
    if source_x == target_x {
        let (low, high) = (source_stem.0.min(target_stem.0), source_stem.1.max(target_stem.1));
        if low <= high {
            canvas.vline(source_x, low, high, LineKind::Solid, style);
        }
    } else {
        draw_stem(canvas, source_x, source_stem, band_row, style);
        draw_stem(canvas, target_x, target_stem, band_row, style);
        canvas.hline(source_x, target_x, band_row, LineKind::Solid, style);
        canvas.join(source_x, band_row, if downward { NORTH } else { SOUTH }, LineKind::Solid, style, false);
        canvas.join(target_x, band_row, if downward { SOUTH } else { NORTH }, LineKind::Solid, style, false);
    }
    canvas.put(target_x, head_row, if downward { '▼' } else { '▲' }, style);
    if !text.is_empty() {
        // 꺾임 줄은 빈 띠라 세로선 옆이 비어 있다. 오른쪽이 모자라면 왼쪽에 붙인다.
        let label_width = width_of(&text);
        let x = if target_x + 3 + label_width <= width { target_x + 2 } else { target_x.saturating_sub(label_width + 2) };
        canvas.text(x, band_row, &text, theme.diagram_label);
    }
}

/// 꺾임 줄에 닿는 칸은 비워 두고 세로 토막만 긋는다. 그 칸은 방향 비트로 모서리를 만든다.
fn draw_stem(canvas: &mut Canvas, x: usize, span: (usize, usize), band: usize, style: Style) {
    let (mut low, mut high) = span;
    if low == band {
        low += 1;
    }
    if high == band {
        high = high.saturating_sub(1);
    }
    if low <= high {
        canvas.vline(x, low, high, LineKind::Solid, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::mermaid::block::parse;

    fn rows(source: &str, width: usize) -> Vec<String> {
        let lines = render(&parse(source), &Theme::none(), width).expect("배치 실패");
        lines.iter().map(Line::plain).collect()
    }

    #[test]
    fn columns_split_blocks_into_rows() {
        let out = rows("block-beta\n columns 2\n a b c d\n", 80);
        let tops = out.iter().filter(|line| line.contains('┌')).count();
        assert_eq!(tops, 2, "{out:#?}");
        assert!(out[1].contains('a') && out[1].contains('b'));
        assert!(out[1 + 3 + ROW_GAP].contains('c'));
    }

    #[test]
    fn without_columns_everything_sits_on_one_row() {
        let out = rows("block-beta\n a b c\n", 80);
        assert_eq!(out.len(), 3);
        assert!(out[1].contains('a') && out[1].contains('b') && out[1].contains('c'));
    }

    #[test]
    fn label_and_shape_are_drawn() {
        let out = rows("block-beta\n a[\"Block A\"] b((round))\n", 80);
        assert!(out[1].contains("Block A"), "{out:#?}");
        assert!(out.iter().any(|line| line.contains('╭')), "{out:#?}");
    }

    #[test]
    fn span_widens_the_block_over_two_columns() {
        let narrow = rows("block-beta\n columns 3\n a b c\n x y z\n", 80);
        let spanned = rows("block-beta\n columns 3\n a b:2\n x y z\n", 80);
        let cell = narrow[1].chars().filter(|c| *c == '│').count();
        assert_eq!(cell, 6);
        // b가 두 칸을 먹으면 첫 줄의 상자는 둘뿐이고 폭은 그대로다.
        assert_eq!(spanned[1].chars().filter(|c| *c == '│').count(), 4);
        assert_eq!(narrow.iter().map(|line| width_of(line)).max(), spanned.iter().map(|line| width_of(line)).max());
    }

    #[test]
    fn arrow_connects_two_blocks() {
        let out = rows("block-beta\n columns 2\n a b\n a --> b\n", 80);
        assert!(out.iter().any(|line| line.contains('▶')), "{out:#?}");
        let down = rows("block-beta\n columns 1\n a\n b\n a --> b\n", 80);
        assert!(down.iter().any(|line| line.contains('▼')), "{down:#?}");
    }

    #[test]
    fn labeled_arrow_shows_its_text() {
        let out = rows("block-beta\n columns 2\n a b\n a -- \"보냄\" --> b\n", 80);
        assert!(out.iter().any(|line| line.contains("보냄")), "{out:#?}");
        let down = rows("block-beta\n columns 1\n a\n b\n a -- \"내림\" --> b\n", 80);
        assert!(down.iter().any(|line| line.contains("내림")), "{down:#?}");
    }

    #[test]
    fn nested_group_wraps_children_in_one_box() {
        let out = rows("block-beta\n columns 1\n block:g\n  x y\n end\n z\n", 80);
        assert!(out.iter().any(|line| line.contains(" g ")), "{out:#?}");
        let group_row = out.iter().position(|line| line.contains(" g ")).unwrap();
        // 묶음 테두리 안쪽에 두 자식이 나란히 있다.
        assert!(out[group_row + 2].contains('x') && out[group_row + 2].contains('y'), "{out:#?}");
    }

    #[test]
    fn nesting_levels_use_different_borders() {
        let out = rows("block-beta\n block:outer\n  block:inner\n   x\n  end\n end\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains(" outer ") && joined.contains(" inner "), "{out:#?}");
        // 1단계는 실선, 2단계는 점선이라 층이 눈에 구분된다.
        assert!(joined.contains('┌'), "{out:#?}");
        assert!(joined.contains('╌') && joined.contains('╎'), "{out:#?}");
    }

    #[test]
    fn too_wide_returns_none() {
        let diagram = parse("block-beta\n columns 3\n aaaaaaaaaa bbbbbbbbbb cccccccccc\n");
        assert!(render(&diagram, &Theme::none(), 12).is_none());
    }

    #[test]
    fn empty_diagram_returns_none() {
        assert!(render(&parse("block-beta\n"), &Theme::none(), 80).is_none());
    }
}
