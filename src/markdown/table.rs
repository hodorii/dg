//! 표 그리기.

use super::wrap::wrap_spans;
use crate::line::{Line, Span};
use crate::style::Theme;
use crate::text::width_of;
use pulldown_cmark::Alignment;

pub fn render(rows: &[Vec<Vec<Span>>], has_header: bool, alignments: &[Alignment], width: usize, theme: &Theme) -> Vec<Line> {
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return Vec::new();
    }
    let mut natural: Vec<usize> = vec![1; column_count];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            // 강제 줄바꿈(`<br>`)만 나누고 자연 폭을 잰다.
            let mut line_width = 0;
            let mut cell_width = 0;
            for span in cell {
                for (k, piece) in span.text.split('\n').enumerate() {
                    if k > 0 {
                        cell_width = cell_width.max(line_width);
                        line_width = 0;
                    }
                    line_width += width_of(piece);
                }
            }
            natural[index] = natural[index].max(cell_width.max(line_width));
        }
    }
    let frame = column_count * 3 + 1;
    let mut widths = natural.clone();
    while widths.iter().sum::<usize>() + frame > width {
        let Some((widest, _)) = widths.iter().enumerate().filter(|(_, w)| **w > 3).max_by_key(|(_, w)| **w) else { break };
        widths[widest] -= 1;
    }
    let border = theme.table_border;
    let mut lines = Vec::new();
    let rule = |left: &str, fill: &str, middle: &str, right: &str, style| -> Line {
        let mut text = String::from(left);
        for (index, w) in widths.iter().enumerate() {
            text.push_str(&fill.repeat(w + 2));
            text.push_str(if index + 1 == widths.len() { right } else { middle });
        }
        Line::single(text, style)
    };
    lines.push(rule("┌", "─", "┬", "┐", border));
    for (row_index, row) in rows.iter().enumerate() {
        let is_header = has_header && row_index == 0;
        let empty: Vec<Span> = Vec::new();
        let cells: Vec<Vec<Vec<Span>>> = (0..column_count)
            .map(|c| {
                let spans = row.get(c).unwrap_or(&empty);
                if is_header {
                    let styled: Vec<Span> = spans.iter().map(|s| Span::new(s.text.clone(), s.style.merge(theme.table_head))).collect();
                    wrap_spans(&styled, widths[c])
                } else {
                    wrap_spans(spans, widths[c])
                }
            })
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for line_index in 0..height {
            let mut line = Line::empty();
            line.push(Span::new("│", border));
            for c in 0..column_count {
                let content: &[Span] = cells[c].get(line_index).map_or(&empty, Vec::as_slice);
                let content_width: usize = content.iter().map(Span::width).sum();
                let slack = widths[c].saturating_sub(content_width);
                let (left_pad, right_pad) = match alignments.get(c) {
                    Some(Alignment::Right) => (slack, 0),
                    Some(Alignment::Center) => (slack / 2, slack - slack / 2),
                    _ => (0, slack),
                };
                line.push(Span::plain(" ".repeat(left_pad + 1)));
                for span in content {
                    line.push_str(&span.text, span.style);
                }
                line.push(Span::plain(" ".repeat(right_pad + 1)));
                line.push(Span::new("│", border));
            }
            lines.push(line);
        }
        if is_header && rows.len() > 1 {
            lines.push(rule("├", "─", "┼", "┤", border));
        } else if row_index + 1 < rows.len() {
            lines.push(rule("├", "─", "┼", "┤", theme.table_row_rule));
        }
    }
    lines.push(rule("└", "─", "┴", "┘", border));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_bordered_table() {
        let rows = vec![vec![vec![Span::plain("a")], vec![Span::plain("bb")]], vec![vec![Span::plain("1")], vec![Span::plain("2")]]];
        let out: Vec<String> = render(&rows, true, &[Alignment::None, Alignment::Right], 40, &Theme::none()).iter().map(Line::plain).collect();
        assert_eq!(out, vec!["┌───┬────┐", "│ a │ bb │", "├───┼────┤", "│ 1 │  2 │", "└───┴────┘"]);
        let rows = vec![vec![vec![Span::plain("a")]], vec![vec![Span::plain("1")]], vec![vec![Span::plain("2")]]];
        let out: Vec<String> = render(&rows, true, &[Alignment::None], 40, &Theme::none()).iter().map(Line::plain).collect();
        assert_eq!(out, vec!["┌───┐", "│ a │", "├───┤", "│ 1 │", "├───┤", "│ 2 │", "└───┘"]);
    }
}
