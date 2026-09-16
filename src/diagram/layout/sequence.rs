//! 시퀀스 다이어그램 배치.
//!
//! 참여자는 위쪽 상자, 아래로 생명선. 메시지는 라벨 줄 + 화살표 줄을 차지한다.
//! 참여자 사이 간격은 그 사이를 지나는 라벨·노트·프레임이 들어갈 만큼 벌린다.

use crate::diagram::canvas::{Canvas, EAST, LineKind, NORTH, WEST};
use crate::diagram::ir::{Marker, NotePlacement, ParticipantKind, Sequence, SequenceItem, Shape};
use crate::diagram::layout::shape;
use crate::line::Line;
use crate::text::{width_of, wrap_plain};
use crate::style::Theme;

pub fn render(sequence: &Sequence, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if sequence.participants.is_empty() {
        return None;
    }
    for cap in [48usize, 32, 24, 16] {
        if let Some(canvas) = SequenceLayout::build(sequence, theme, cap, width) {
            return Some(canvas.into_lines());
        }
    }
    None
}

struct Fragment {
    kind: String,
    label: String,
    lo: usize,
    hi: usize,
    inner_depth: usize,
    start_row: usize,
    else_rows: Vec<(usize, String)>,
    end_row: usize,
    x_lo: usize,
    x_hi: usize,
}

struct ParticipantBox {
    sections: Vec<Vec<String>>,
    shape: Shape,
    width: usize,
    height: usize,
}

struct SequenceLayout<'a> {
    sequence: &'a Sequence,
    theme: &'a Theme,
    boxes: Vec<ParticipantBox>,
    /// 항목별로 미리 줄바꿈한 메시지 라벨.
    labels: Vec<Vec<String>>,
    fragments: Vec<Fragment>,
    /// 항목 번호 → 프레임 번호 (FragmentStart 항목만).
    fragment_of_item: Vec<Option<usize>>,
    centers: Vec<usize>,
    total_width: usize,
}

impl<'a> SequenceLayout<'a> {
    fn build(sequence: &'a Sequence, theme: &'a Theme, cap: usize, width: usize) -> Option<Canvas> {
        let boxes = sequence
            .participants
            .iter()
            .map(|p| {
                let (shape, sections) = match p.kind {
                    ParticipantKind::Box => (Shape::Rect, vec![vec![p.label.clone()]]),
                    ParticipantKind::Actor => (Shape::Actor, vec![vec![p.label.clone()]]),
                    ParticipantKind::Database => (Shape::Cylinder, vec![vec![p.label.clone()]]),
                    other => {
                        let stereotype = format!("«{}»", format!("{other:?}").to_lowercase());
                        (Shape::Rect, vec![vec![stereotype, p.label.clone()]])
                    }
                };
                let sections: Vec<Vec<String>> =
                    sections.into_iter().map(|s| s.iter().flat_map(|l| wrap_plain(l, cap)).collect()).collect();
                let (w, h) = shape::measure(shape, &sections);
                ParticipantBox { sections, shape, width: w, height: h }
            })
            .collect();
        let mut counter = 0;
        let labels = sequence
            .items
            .iter()
            .map(|item| match item {
                SequenceItem::Message { label, .. } => {
                    let text = if sequence.autonumber {
                        counter += 1;
                        format!("{counter}. {label}")
                    } else {
                        label.clone()
                    };
                    if text.trim().is_empty() { Vec::new() } else { wrap_plain(&text, cap) }
                }
                _ => Vec::new(),
            })
            .collect();
        let mut layout = SequenceLayout {
            sequence,
            theme,
            boxes,
            labels,
            fragments: Vec::new(),
            fragment_of_item: vec![None; sequence.items.len()],
            centers: Vec::new(),
            total_width: 0,
        };
        layout.collect_fragments();
        layout.position_participants();
        if layout.total_width > width {
            return None;
        }
        let mut canvas = Canvas::new(layout.total_width, 1);
        layout.draw(&mut canvas);
        Some(canvas)
    }

    fn collect_fragments(&mut self) {
        let mut stack: Vec<usize> = Vec::new();
        let participant_count = self.sequence.participants.len();
        for (index, item) in self.sequence.items.iter().enumerate() {
            match item {
                SequenceItem::FragmentStart { kind, label } => {
                    self.fragments.push(Fragment {
                        kind: kind.clone(),
                        label: label.clone(),
                        lo: usize::MAX,
                        hi: 0,
                        inner_depth: 0,
                        start_row: 0,
                        else_rows: Vec::new(),
                        end_row: 0,
                        x_lo: usize::MAX,
                        x_hi: 0,
                    });
                    let id = self.fragments.len() - 1;
                    self.fragment_of_item[index] = Some(id);
                    stack.push(id);
                }
                SequenceItem::FragmentEnd => {
                    if let Some(closed) = stack.pop() {
                        let depth = self.fragments[closed].inner_depth + 1;
                        if let Some(&parent) = stack.last() {
                            self.fragments[parent].inner_depth = self.fragments[parent].inner_depth.max(depth);
                        }
                    }
                }
                SequenceItem::Message { from, to, .. } => {
                    for &f in &stack {
                        let fragment = &mut self.fragments[f];
                        fragment.lo = fragment.lo.min(*from).min(*to);
                        fragment.hi = fragment.hi.max(*from).max(*to);
                    }
                }
                SequenceItem::Note { placement, .. } => {
                    let (a, b) = match placement {
                        NotePlacement::LeftOf(p) | NotePlacement::RightOf(p) => (*p, *p),
                        NotePlacement::Over(a, b) => (*a.min(b), *a.max(b)),
                    };
                    for &f in &stack {
                        let fragment = &mut self.fragments[f];
                        fragment.lo = fragment.lo.min(a);
                        fragment.hi = fragment.hi.max(b);
                    }
                }
                _ => {}
            }
        }
        for fragment in &mut self.fragments {
            if fragment.lo == usize::MAX {
                fragment.lo = 0;
                fragment.hi = participant_count - 1;
            }
        }
    }

    fn label_width(&self, index: usize) -> usize {
        self.labels[index].iter().map(|l| width_of(l)).max().unwrap_or(0)
    }

    fn note_width(lines: &[String]) -> usize {
        lines.iter().map(|l| width_of(l)).max().unwrap_or(0) + 4
    }

    /// 참여자 생명선의 x 좌표를 정한다.
    fn position_participants(&mut self) {
        let count = self.sequence.participants.len();
        let mut gaps: Vec<usize> = (0..count.saturating_sub(1))
            .map(|i| self.boxes[i].width.div_ceil(2) + self.boxes[i + 1].width.div_ceil(2) + 3)
            .collect();
        let mut left_margin = self.boxes[0].width / 2;
        let mut right_margin = self.boxes[count - 1].width.div_ceil(2);
        // (i, j, 최소 거리) — i < j
        let mut constraints: Vec<(usize, usize, usize)> = Vec::new();
        for (index, item) in self.sequence.items.iter().enumerate() {
            match item {
                SequenceItem::Message { from, to, .. } if from == to => {
                    let need = self.label_width(index) + 6;
                    if *from + 1 < count {
                        constraints.push((*from, from + 1, need));
                    } else {
                        right_margin = right_margin.max(need);
                    }
                }
                SequenceItem::Message { from, to, .. } => {
                    let (a, b) = (*from.min(to), *from.max(to));
                    constraints.push((a, b, self.label_width(index) + 3));
                }
                SequenceItem::Note { placement, lines } => {
                    let note_width = Self::note_width(lines);
                    match placement {
                        NotePlacement::RightOf(p) => {
                            if *p + 1 < count {
                                constraints.push((*p, p + 1, note_width + 3));
                            } else {
                                right_margin = right_margin.max(note_width + 2);
                            }
                        }
                        NotePlacement::LeftOf(p) => {
                            if *p > 0 {
                                constraints.push((p - 1, *p, note_width + 3));
                            } else {
                                left_margin = left_margin.max(note_width + 2);
                            }
                        }
                        NotePlacement::Over(a, b) => {
                            let (a, b) = (*a.min(b), *a.max(b));
                            let half = note_width / 2 + 2;
                            if a == b {
                                if a > 0 {
                                    constraints.push((a - 1, a, half));
                                } else {
                                    left_margin = left_margin.max(half);
                                }
                                if a + 1 < count {
                                    constraints.push((a, a + 1, half));
                                } else {
                                    right_margin = right_margin.max(half);
                                }
                            } else {
                                constraints.push((a, b, note_width.saturating_sub(2)));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for fragment in &self.fragments {
            let need = 5 + 2 * fragment.inner_depth + width_of(&fragment.kind);
            if fragment.lo > 0 {
                constraints.push((fragment.lo - 1, fragment.lo, need));
            } else {
                left_margin = left_margin.max(need);
            }
            if fragment.hi + 1 < count {
                constraints.push((fragment.hi, fragment.hi + 1, 5 + 2 * fragment.inner_depth));
            } else {
                right_margin = right_margin.max(5 + 2 * fragment.inner_depth);
            }
        }
        constraints.sort_by_key(|&(a, b, _)| (b - a, a));
        for (a, b, need) in constraints {
            let current: usize = gaps[a..b].iter().sum();
            if current >= need {
                continue;
            }
            let deficit = need - current;
            let span = b - a;
            for (k, gap) in gaps[a..b].iter_mut().enumerate() {
                *gap += deficit / span + usize::from(k < deficit % span);
            }
        }
        let mut centers = vec![left_margin];
        for gap in &gaps {
            centers.push(centers.last().unwrap() + gap);
        }
        self.total_width = centers[count - 1] + right_margin + 1;
        self.centers = centers;
    }

    fn draw(&mut self, canvas: &mut Canvas) {
        let theme = self.theme;
        let header_height = self.boxes.iter().map(|b| b.height).max().unwrap_or(3);
        let mut row = header_height;
        if !self.sequence.title.is_empty() {
            canvas.text_centered(0, 0, self.total_width, &self.sequence.title, theme.diagram_text.bold());
            row += 2;
        }
        let header_top = row - header_height;
        let body_start = row;
        let count = self.sequence.participants.len();
        let mut active: Vec<Vec<(usize, Option<usize>)>> = vec![Vec::new(); count];
        let mut active_depth = vec![0usize; count];
        let mut last_arrow_row = row;
        let mut fragment_stack: Vec<usize> = Vec::new();
        let mut delays: Vec<usize> = Vec::new();
        // 항목마다 (행 범위, x 범위)를 기록해 프레임 크기를 잡는다.
        let items: Vec<SequenceItem> = self.sequence.items.clone();
        let mut pending: Vec<(usize, usize, usize, usize, SequenceItem)> = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let start_row = row;
            let (x_lo, x_hi) = match &item {
                SequenceItem::Message { from, to, .. } if from == to => {
                    let lines = self.labels[index].len();
                    row += lines.max(3);
                    let x = self.centers[*from];
                    last_arrow_row = start_row + 2;
                    (x, x + 4 + self.label_width(index))
                }
                SequenceItem::Message { from, to, activate_target, deactivate_source, .. } => {
                    row += self.labels[index].len() + 1;
                    last_arrow_row = row - 1;
                    if *activate_target {
                        active_depth[*to] += 1;
                        active[*to].push((last_arrow_row, None));
                    }
                    if *deactivate_source {
                        Self::close_activation(&mut active[*from], last_arrow_row);
                        active_depth[*from] = active_depth[*from].saturating_sub(1);
                    }
                    let (a, b) = (self.centers[*from.min(to)], self.centers[*from.max(to)]);
                    (a, b)
                }
                SequenceItem::Note { placement, lines } => {
                    let note_width = Self::note_width(lines);
                    row += lines.len() + 2;
                    let x = match placement {
                        NotePlacement::RightOf(p) => self.centers[*p] + 2,
                        NotePlacement::LeftOf(p) => self.centers[*p].saturating_sub(2 + note_width),
                        NotePlacement::Over(a, b) => {
                            let center = (self.centers[*a] + self.centers[*b]) / 2;
                            center.saturating_sub(note_width / 2)
                        }
                    };
                    (x, x + note_width)
                }
                SequenceItem::FragmentStart { .. } => {
                    let id = self.fragment_of_item[index].unwrap();
                    self.fragments[id].start_row = row;
                    fragment_stack.push(id);
                    row += 1;
                    (usize::MAX, 0)
                }
                SequenceItem::FragmentElse { label } => {
                    if let Some(&id) = fragment_stack.last() {
                        self.fragments[id].else_rows.push((row, label.clone()));
                    }
                    row += 1;
                    (usize::MAX, 0)
                }
                SequenceItem::FragmentEnd => {
                    if let Some(id) = fragment_stack.pop() {
                        self.fragments[id].end_row = row;
                        let (lo, hi) = (self.fragments[id].x_lo, self.fragments[id].x_hi);
                        if let Some(&parent) = fragment_stack.last() {
                            self.fragments[parent].x_lo = self.fragments[parent].x_lo.min(lo);
                            self.fragments[parent].x_hi = self.fragments[parent].x_hi.max(hi);
                        }
                    }
                    row += 1;
                    (usize::MAX, 0)
                }
                SequenceItem::Divider { .. } => {
                    row += 1;
                    (usize::MAX, 0)
                }
                SequenceItem::Delay { .. } => {
                    delays.push(row);
                    row += 1;
                    (usize::MAX, 0)
                }
                SequenceItem::Activate(p) => {
                    active_depth[*p] += 1;
                    active[*p].push((last_arrow_row, None));
                    (usize::MAX, 0)
                }
                SequenceItem::Deactivate(p) => {
                    Self::close_activation(&mut active[*p], last_arrow_row);
                    active_depth[*p] = active_depth[*p].saturating_sub(1);
                    (usize::MAX, 0)
                }
            };
            for &f in &fragment_stack {
                self.fragments[f].x_lo = self.fragments[f].x_lo.min(x_lo);
                self.fragments[f].x_hi = self.fragments[f].x_hi.max(x_hi);
            }
            pending.push((index, start_row, x_lo, x_hi, item));
        }
        while let Some(id) = fragment_stack.pop() {
            self.fragments[id].end_row = row;
            row += 1;
        }
        let body_end = row;
        let footer_top = body_end + 1;
        let bottom = footer_top + header_height;

        // 생명선
        for (p, &x) in self.centers.iter().enumerate() {
            canvas.vline(x, header_top + self.boxes[p].height.max(1) - 1 + (header_height - self.boxes[p].height), footer_top, LineKind::Solid, theme.diagram_line);
            for (start, end) in &active[p] {
                let end = end.unwrap_or(body_end);
                if *start < end {
                    canvas.vline(x, *start, end, LineKind::Heavy, theme.diagram_accent);
                }
            }
        }
        let _ = body_start;
        for &delay_row in &delays {
            for &x in &self.centers {
                canvas.put(x, delay_row, '⋮', theme.diagram_line);
            }
        }

        // 항목
        for (index, start_row, _, _, item) in &pending {
            match item {
                SequenceItem::Message { from, to, kind, head, .. } if from == to => {
                    self.draw_self_message(canvas, *index, *from, *start_row, *kind, *head);
                }
                SequenceItem::Message { from, to, kind, head, .. } => {
                    self.draw_message(canvas, *index, *from, *to, *start_row, *kind, *head);
                }
                SequenceItem::Note { placement, lines } => {
                    let note_width = Self::note_width(lines);
                    let x = match placement {
                        NotePlacement::RightOf(p) => self.centers[*p] + 2,
                        NotePlacement::LeftOf(p) => self.centers[*p].saturating_sub(2 + note_width),
                        NotePlacement::Over(a, b) => {
                            let center = (self.centers[*a] + self.centers[*b]) / 2;
                            center.saturating_sub(note_width / 2)
                        }
                    };
                    canvas.clear_rect(x, *start_row, note_width, lines.len() + 2);
                    canvas.rect(x, *start_row, note_width, lines.len() + 2, LineKind::Dashed, theme.diagram_note, false);
                    for (k, line) in lines.iter().enumerate() {
                        canvas.text(x + 2, start_row + 1 + k, line, theme.diagram_note);
                    }
                }
                SequenceItem::Divider { label } => {
                    canvas.hline(0, self.total_width - 1, *start_row, LineKind::Heavy, theme.diagram_group);
                    let text = format!(" {label} ");
                    canvas.text_centered(0, *start_row, self.total_width, &text, theme.diagram_text.bold());
                }
                SequenceItem::Delay { label } if !label.is_empty() => {
                    canvas.text_centered(0, *start_row, self.total_width, &format!(" {label} "), theme.diagram_caption);
                }
                _ => {}
            }
        }

        // 프레임(안쪽 것을 나중에 그려 테두리가 위에 남게 한다)
        let mut order: Vec<usize> = (0..self.fragments.len()).collect();
        order.sort_by_key(|&f| std::cmp::Reverse(self.fragments[f].inner_depth));
        for f in order {
            let fragment = &self.fragments[f];
            let pad = 1 + 2 * fragment.inner_depth;
            let x_lo = fragment.x_lo.min(self.centers[fragment.lo]).saturating_sub(pad + 1);
            let x_hi = (fragment.x_hi.max(self.centers[fragment.hi]) + pad + 1).min(self.total_width - 1);
            let height = fragment.end_row + 1 - fragment.start_row;
            canvas.rect(x_lo, fragment.start_row, x_hi - x_lo + 1, height, LineKind::Solid, theme.diagram_group, false);
            let title = if fragment.label.is_empty() {
                format!(" {} ", fragment.kind)
            } else {
                format!(" {} [{}] ", fragment.kind, fragment.label)
            };
            canvas.text(x_lo + 1, fragment.start_row, &title, theme.diagram_accent.bold());
            for (else_row, label) in &fragment.else_rows {
                canvas.hline(x_lo + 1, x_hi - 1, *else_row, LineKind::Dashed, theme.diagram_group);
                if !label.is_empty() {
                    canvas.text(x_lo + 2, *else_row, &format!(" [{label}] "), theme.diagram_accent);
                }
            }
        }

        // 참여자 상자(머리·발)
        for (p, participant_box) in self.boxes.iter().enumerate() {
            let x = self.centers[p].saturating_sub(participant_box.width / 2);
            let y = header_top + header_height - participant_box.height;
            shape::draw(canvas, x, y, participant_box.shape, &participant_box.sections, theme, 0);
            shape::draw(canvas, x, footer_top, participant_box.shape, &participant_box.sections, theme, 0);
        }
        let _ = bottom;
    }

    fn close_activation(intervals: &mut [(usize, Option<usize>)], row: usize) {
        if let Some(open) = intervals.iter_mut().rev().find(|(_, end)| end.is_none()) {
            open.1 = Some(row);
        }
    }

    fn draw_message(&self, canvas: &mut Canvas, index: usize, from: usize, to: usize, start_row: usize, kind: LineKind, head: Marker) {
        let theme = self.theme;
        let (x_from, x_to) = (self.centers[from], self.centers[to]);
        let lines = &self.labels[index];
        let arrow_row = start_row + lines.len();
        let (lo, hi) = (x_from.min(x_to) + 1, x_from.max(x_to) - 1);
        for (k, line) in lines.iter().enumerate() {
            canvas.text_centered(lo, start_row + k, hi - lo + 1, line, theme.diagram_text);
        }
        canvas.hline(lo, hi, arrow_row, kind, theme.diagram_line);
        let rightward = x_to > x_from;
        let glyph = match (head, rightward) {
            (Marker::OpenArrow, true) => '>',
            (Marker::OpenArrow, false) => '<',
            (Marker::Cross, _) => '✕',
            (Marker::Circle, _) => '○',
            (Marker::None, _) => ' ',
            (_, true) => '▶',
            (_, false) => '◀',
        };
        if glyph != ' ' {
            let x = if rightward { hi } else { lo };
            canvas.put(x, arrow_row, glyph, theme.diagram_line);
        }
        let origin_bits = if rightward { EAST } else { WEST };
        canvas.join(x_from, arrow_row, origin_bits, LineKind::Solid, theme.diagram_line, false);
    }

    fn draw_self_message(&self, canvas: &mut Canvas, index: usize, participant: usize, start_row: usize, kind: LineKind, head: Marker) {
        let theme = self.theme;
        let x = self.centers[participant];
        canvas.join(x, start_row, EAST, LineKind::Solid, theme.diagram_line, false);
        canvas.hline(x + 1, x + 2, start_row, kind, theme.diagram_line);
        canvas.vline(x + 2, start_row, start_row + 2, kind, theme.diagram_line);
        canvas.join(x + 2, start_row, 0, kind, theme.diagram_line, true);
        canvas.join(x + 2, start_row + 2, WEST | NORTH, kind, theme.diagram_line, true);
        let glyph = match head {
            Marker::OpenArrow => '<',
            Marker::Cross => '✕',
            _ => '◀',
        };
        canvas.put(x + 1, start_row + 2, glyph, theme.diagram_line);
        for (k, line) in self.labels[index].iter().enumerate() {
            canvas.text(x + 4, start_row + k, line, theme.diagram_text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Sequence {
        let mut s = Sequence::default();
        let a = s.intern("A", "Alice", ParticipantKind::Actor);
        let b = s.intern("B", "Bob", ParticipantKind::Box);
        s.items.push(SequenceItem::FragmentStart { kind: "alt".into(), label: "ok".into() });
        s.items.push(SequenceItem::Message { from: a, to: b, label: "hello".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        s.items.push(SequenceItem::Message { from: b, to: a, label: "hi".into(), kind: LineKind::Dashed, head: Marker::Arrow, activate_target: false, deactivate_source: true });
        s.items.push(SequenceItem::FragmentElse { label: "fail".into() });
        s.items.push(SequenceItem::Message { from: b, to: b, label: "retry".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        s.items.push(SequenceItem::FragmentEnd);
        s.items.push(SequenceItem::Note { placement: NotePlacement::Over(a, b), lines: vec!["done".into()] });
        s
    }

    #[test]
    fn renders_messages_and_fragment() {
        let out = render(&sample(), &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        assert!(joined.contains("hello"), "{joined}");
        assert!(joined.contains("▶"), "{joined}");
        assert!(joined.contains("◀"), "{joined}");
        assert!(joined.contains("alt [ok]"), "{joined}");
        assert!(joined.contains("[fail]"), "{joined}");
        assert!(joined.contains("retry"), "{joined}");
        assert!(joined.contains("done"), "{joined}");
    }

    #[test]
    fn too_narrow_returns_none() {
        assert!(render(&sample(), &Theme::none(), 10).is_none());
    }
}
