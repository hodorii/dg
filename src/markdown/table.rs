//! 표 그리기.

use super::wrap::wrap_spans;
use crate::line::{Line, Span};
use crate::style::Theme;
use pulldown_cmark::Alignment;

pub fn render(rows: &[Vec<Vec<Span>>], has_header: bool, alignments: &[Alignment], width: usize, theme: &Theme) -> Vec<Line> {
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return Vec::new();
    }
    let mut natural: Vec<usize> = vec![1; column_count];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            let cell_width = wrap_spans(cell, usize::MAX / 4).iter().map(|l| l.iter().map(Span::width).sum::<usize>()).max().unwrap_or(0);
            natural[index] = natural[index].max(cell_width);
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
        let cells: Vec<Vec<Vec<Span>>> = (0..column_count)
            .map(|c| {
                let spans = row.get(c).cloned().unwrap_or_default();
                let spans: Vec<Span> = if is_header { spans.into_iter().map(|s| Span::new(s.text, s.style.merge(theme.table_head))).collect() } else { spans };
                wrap_spans(&spans, widths[c])
            })
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for line_index in 0..height {
            let mut line = Line::empty();
            line.push(Span::new("│", border));
            for c in 0..column_count {
                let content = cells[c].get(line_index).cloned().unwrap_or_default();
                let content_width: usize = content.iter().map(Span::width).sum();
                let slack = widths[c].saturating_sub(content_width);
                let (left_pad, right_pad) = match alignments.get(c) {
                    Some(Alignment::Right) => (slack, 0),
                    Some(Alignment::Center) => (slack / 2, slack - slack / 2),
                    _ => (0, slack),
                };
                line.push(Span::plain(" ".repeat(left_pad + 1)));
                for span in content {
                    line.push(span);
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
