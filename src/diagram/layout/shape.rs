//! 노드 도형의 크기 계산과 그리기.

use crate::diagram::canvas::{Canvas, LineKind, NORTH, SOUTH};
use crate::diagram::ir::{Node, Shape};
use crate::style::{Style, Theme};
use crate::text::{width_of, wrap_plain};

/// 라벨 폭 상한에 맞춰 줄바꿈한 본문.
pub fn wrapped_sections(node: &Node, cap: usize) -> Vec<Vec<String>> {
    node.sections
        .iter()
        .enumerate()
        .filter(|(index, section)| *index == 0 || !section.is_empty())
        .map(|(_, section)| section.iter().flat_map(|line| wrap_plain(line, cap)).collect())
        .collect()
}

fn text_size(sections: &[Vec<String>]) -> (usize, usize) {
    let width = sections.iter().flatten().map(|s| width_of(s)).max().unwrap_or(0);
    let lines: usize = sections.iter().map(Vec::len).sum();
    let separators = sections.len().saturating_sub(1);
    (width, lines + separators)
}

/// (폭, 높이)
pub fn measure(shape: Shape, sections: &[Vec<String>]) -> (usize, usize) {
    let (tw, th) = text_size(sections);
    match shape {
        Shape::Start | Shape::End | Shape::Anchor => (1, 1),
        Shape::Rect | Shape::Round | Shape::Note | Shape::Circle => (tw + 4, th + 2),
        Shape::Stadium | Shape::Diamond | Shape::Hexagon | Shape::Subroutine => (tw + 6, th + 2),
        Shape::Cylinder => (tw + 4, th + 3),
        Shape::Actor => (tw.max(3), th + 3),
        Shape::Interface => (tw.max(1), th + 1),
    }
}

/// `min_height`보다 낮으면 위아래를 늘려 본문을 세로 가운데에 둔다.
/// `min_width`·`min_height`보다 작으면 늘려 그린다(접점·라벨 자리 확보용). 본문은 가운데에 둔다.
pub fn draw(canvas: &mut Canvas, x: usize, y: usize, shape: Shape, sections: &[Vec<String>], theme: &Theme, min_width: usize, min_height: usize) {
    let (measured_width, measured) = measure(shape, sections);
    let w = if matches!(shape, Shape::Start | Shape::End | Shape::Anchor) { measured_width } else { measured_width.max(min_width) };
    let h = measured.max(min_height);
    let extra_top = (h - measured) / 2;
    canvas.clear_rect(x, y, w, h);
    let border = theme.diagram_box;
    match shape {
        Shape::Anchor => {}
        Shape::Start => canvas.put(x, y, '●', theme.diagram_accent),
        Shape::End => canvas.put(x, y, '◉', theme.diagram_accent),
        Shape::Rect | Shape::Round | Shape::Circle | Shape::Note => {
            let kind = if shape == Shape::Note { LineKind::Dashed } else { LineKind::Solid };
            canvas.rect(x, y, w, h, kind, border, shape != Shape::Rect && shape != Shape::Note);
            draw_sections(canvas, x, y + 1 + extra_top, w, sections, theme, true);
        }
        Shape::Stadium => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, true);
            draw_sections(canvas, x, y + 1 + extra_top, w, sections, theme, true);
        }
        Shape::Diamond | Shape::Hexagon => {
            canvas.hline(x + 2, x + w - 3, y, LineKind::Solid, border);
            canvas.hline(x + 2, x + w - 3, y + h - 1, LineKind::Solid, border);
            canvas.put(x + 1, y, '╱', border);
            canvas.put(x + w - 2, y, '╲', border);
            canvas.put(x + 1, y + h - 1, '╲', border);
            canvas.put(x + w - 2, y + h - 1, '╱', border);
            for row in y + 1..y + h - 1 {
                canvas.put(x, row, '⟨', border);
                canvas.put(x + w - 1, row, '⟩', border);
            }
            draw_sections(canvas, x + 1, y + 1 + extra_top, w - 2, sections, theme, false);
        }
        Shape::Subroutine => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, false);
            for row in y + 1..y + h - 1 {
                canvas.put(x + 1, row, '│', border);
                canvas.put(x + w - 2, row, '│', border);
            }
            draw_sections(canvas, x + 1, y + 1 + extra_top, w - 2, sections, theme, false);
        }
        Shape::Cylinder => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, true);
            canvas.hline(x, x + w - 1, y + 1, LineKind::Solid, border);
            draw_sections(canvas, x, y + 2 + extra_top, w, sections, theme, true);
        }
        Shape::Actor => {
            let mid = x + w / 2;
            canvas.put(mid, y, '○', border);
            canvas.put(mid.saturating_sub(1), y + 1, '╱', border);
            canvas.put(mid, y + 1, '│', border);
            canvas.put(mid + 1, y + 1, '╲', border);
            canvas.put(mid.saturating_sub(1), y + 2, '╱', border);
            canvas.put(mid + 1, y + 2, '╲', border);
            draw_sections(canvas, x, y + 3, w, sections, theme, false);
        }
        Shape::Interface => {
            canvas.put(x + w / 2, y, '○', border);
            draw_sections(canvas, x, y + 1, w, sections, theme, false);
        }
    }
}

/// 칸 사이에 구분선을 넣어 가며 본문을 쓴다. `x`는 테두리 칸, 내용은 `x+2`부터.
fn draw_sections(canvas: &mut Canvas, x: usize, y: usize, w: usize, sections: &[Vec<String>], theme: &Theme, separators: bool) {
    let mut row = y;
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            if separators {
                canvas.hline(x, x + w - 1, row, LineKind::Solid, theme.diagram_box);
                canvas.join(x, row, NORTH | SOUTH, LineKind::Solid, theme.diagram_box, false);
                canvas.join(x + w - 1, row, NORTH | SOUTH, LineKind::Solid, theme.diagram_box, false);
            }
            row += 1;
        }
        for text in section {
            let style = line_style(index, text, theme);
            if index == 0 {
                canvas.text_centered(x + 1, row, w - 2, text, style);
            } else {
                canvas.text(x + 2, row, text, style);
            }
            row += 1;
        }
    }
}

fn line_style(section: usize, text: &str, theme: &Theme) -> Style {
    if text.starts_with('«') {
        theme.diagram_text.dim().italic()
    } else if section == 0 {
        theme.diagram_text.bold()
    } else {
        theme.diagram_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;

    fn draw_rows(shape: Shape, sections: Vec<Vec<String>>) -> Vec<String> {
        let (w, h) = measure(shape, &sections);
        let mut canvas = Canvas::new(w, h);
        draw(&mut canvas, 0, 0, shape, &sections, &Theme::none(), 0, 0);
        canvas.into_lines().iter().map(Line::plain).collect()
    }

    #[test]
    fn rect_with_two_sections() {
        let rows = draw_rows(Shape::Rect, vec![vec!["User".into()], vec!["+id".into(), "+name".into()]]);
        assert_eq!(rows, vec!["┌───────┐", "│ User  │", "├───────┤", "│ +id   │", "│ +name │", "└───────┘"]);
    }

    #[test]
    fn cylinder_has_lid() {
        let rows = draw_rows(Shape::Cylinder, vec![vec!["db".into()]]);
        assert_eq!(rows, vec!["╭────╮", "├────┤", "│ db │", "╰────╯"]);
    }
}
