//! 시퀀스 다이어그램 배치.
//!
//! 참여자는 위쪽 상자, 아래로 생명선. 메시지는 라벨 줄 + 화살표 줄을 차지한다.
//! 참여자 사이 간격은 그 사이를 지나는 라벨·노트·프레임이 들어갈 만큼 벌린다.

use crate::diagram::canvas::{Canvas, EAST, LineKind, WEST};
use crate::diagram::ir::{Marker, NotePlacement, ParticipantKind, Sequence, SequenceItem, Shape};
use crate::diagram::layout::shape;
use crate::line::Line;
use crate::text::{truncate, width_of, wrap_plain};
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
    /// `else`/`option`/`and` 구분선의 라벨(폭 계산·그리기에 모두 씀).
    else_labels: Vec<String>,
    else_rows: Vec<(usize, String)>,
    end_row: usize,
    x_lo: usize,
    x_hi: usize,
}

/// 프레임 위쪽 테두리에 얹는 제목(`kind [label]`). 폭 계산과 그리기가 같은 문자열을 쓰게 한다.
fn frame_title(kind: &str, label: &str) -> String {
    if label.is_empty() { format!(" {kind} ") } else { format!(" {kind} [{label}] ") }
}

/// `else`/`option`/`and` 구분선의 라벨 표시. 비어 있으면 그리지 않는다.
fn frame_else_title(label: &str) -> String {
    format!(" [{label}] ")
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
                        else_labels: Vec::new(),
                        else_rows: Vec::new(),
                        end_row: 0,
                        x_lo: usize::MAX,
                        x_hi: 0,
                    });
                    let id = self.fragments.len() - 1;
                    self.fragment_of_item[index] = Some(id);
                    stack.push(id);
                }
                SequenceItem::FragmentElse { label } => {
                    if let Some(&id) = stack.last() {
                        self.fragments[id].else_labels.push(label.clone());
                    }
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
                    // 재귀 루프 폭(3칸) + 라벨 앞 여백만큼: draw_self_message의 x+5(라벨 시작)에 2칸 버퍼.
                    let need = self.label_width(index) + 7;
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
            // 위쪽 테두리에 얹는 제목(`kind [label]`, `[else 라벨]`)이 다 들어갈 만큼 안쪽 폭도 확보한다.
            let pad = 1 + 2 * fragment.inner_depth;
            let title_width = std::iter::once(width_of(&frame_title(&fragment.kind, &fragment.label)))
                .chain(fragment.else_labels.iter().map(|l| width_of(&frame_else_title(l))))
                .max()
                .unwrap_or(0);
            let interior_need = title_width.saturating_sub(2 * pad);
            if interior_need == 0 {
                continue;
            }
            if fragment.hi > fragment.lo {
                constraints.push((fragment.lo, fragment.hi, interior_need));
            } else if fragment.lo > 0 {
                constraints.push((fragment.lo - 1, fragment.lo, interior_need));
            } else {
                left_margin = left_margin.max(interior_need);
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

        // 생명선·활성 막대·메시지는 간선 모드로 그려 서로 직교하면 건너뛰기로 표시한다.
        canvas.set_edge_mode(true);
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

        canvas.set_edge_mode(false);
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
            // 폭 계산이 빗나가도(둥근 폭·희귀 경로) 테두리를 뚫고 나가지 않도록 안쪽 폭에 맞춰 자른다.
            let inner_width = x_hi.saturating_sub(x_lo + 1);
            let title = truncate(&frame_title(&fragment.kind, &fragment.label), inner_width);
            canvas.text(x_lo + 1, fragment.start_row, &title, theme.diagram_accent.bold());
            for (else_row, label) in &fragment.else_rows {
                canvas.hline(x_lo + 1, x_hi - 1, *else_row, LineKind::Dashed, theme.diagram_group);
                if !label.is_empty() {
                    let else_inner_width = x_hi.saturating_sub(x_lo + 2);
                    let else_title = truncate(&frame_else_title(label), else_inner_width);
                    canvas.text(x_lo + 2, *else_row, &else_title, theme.diagram_accent);
                }
            }
        }

        // 참여자 상자(머리·발)
        for (p, participant_box) in self.boxes.iter().enumerate() {
            let x = self.centers[p].saturating_sub(participant_box.width / 2);
            let y = header_top + header_height - participant_box.height;
            shape::draw(canvas, x, y, participant_box.shape, &participant_box.sections, theme, 0, 0);
            shape::draw(canvas, x, footer_top, participant_box.shape, &participant_box.sections, theme, 0, 0);
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
            // graph.rs의 CrowMany 폰트 폴백 수정과 같은 이유로 ×(U+00D7)로.
            (Marker::Cross, _) => '×',
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
        // 루프 폭 3칸(x+1..x+3): 돌아오는 줄(start_row+2)도 나가는 줄과 대칭으로 실선을 그려서
        // 화살촉(x+2)과 생명선(x) 사이에 실제 선 한 칸(x+1)이 남도록 한다 — 화살촉이 생명선에 바로
        // 붙어 `<|`처럼 보이던 것을 `<-`처럼 여백이 있게 고친다.
        canvas.join(x, start_row, EAST, LineKind::Solid, theme.diagram_line, false);
        canvas.hline(x + 1, x + 3, start_row, kind, theme.diagram_line);
        canvas.vline(x + 3, start_row, start_row + 2, kind, theme.diagram_line);
        canvas.join(x + 3, start_row, 0, kind, theme.diagram_line, true);
        canvas.hline(x + 1, x + 3, start_row + 2, kind, theme.diagram_line);
        canvas.join(x + 3, start_row + 2, 0, kind, theme.diagram_line, true);
        let glyph = match head {
            Marker::OpenArrow => '<',
            Marker::Cross => '×',
            _ => '◀',
        };
        canvas.put(x + 2, start_row + 2, glyph, theme.diagram_line);
        for (k, line) in self.labels[index].iter().enumerate() {
            canvas.text(x + 5, start_row + k, line, theme.diagram_text);
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

    /// self-message(재귀)의 화살촉이 생명선에 바로 붙지 않고, 사이에 실선 한 칸이 있어야 한다
    /// (`<|`처럼 보이던 것을 `<-`처럼 여백 있게 고친 회귀). 교차 메시지의 도착 화살촉(정상적으로
    /// 상대 생명선에 바로 닿아야 함)과 헷갈리지 않도록 참가자 하나에 self-message 하나만 있는
    /// 최소 픽스처를 쓴다.
    #[test]
    fn self_message_arrowhead_does_not_touch_lifeline() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        s.items.push(SequenceItem::Message { from: a, to: a, label: "retry".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        let out = render(&s, &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let arrow_row = text.iter().find(|l| l.contains('◀')).expect("화살촉 줄이 있어야 한다");
        let chars: Vec<char> = arrow_row.chars().collect();
        let arrow_col = chars.iter().position(|&c| c == '◀').expect("화살촉 문자가 있어야 한다");
        let lifeline_col = chars[..arrow_col].iter().rposition(|&c| c == '│').expect("생명선 문자가 있어야 한다");
        assert!(arrow_col > lifeline_col + 1, "화살촉과 생명선 사이에 최소 한 칸이 있어야 한다: {arrow_row}");
        let between = &chars[lifeline_col + 1..arrow_col];
        assert!(between.iter().all(|&c| c == '─' || c == '╌'), "화살촉 앞은 실선/점선이어야 한다: {arrow_row}");
    }

    /// 프레임 조건이 참여자 간격보다 훨씬 길면, 잘려서 대괄호가 닫히지 않은 채 테두리를 뚫고
    /// 나가는 대신 간격을 늘려서라도 제목이 온전히 들어가야 한다.
    #[test]
    fn long_fragment_label_does_not_overflow_border() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        let b = s.intern("B", "B", ParticipantKind::Box);
        let label = "매우 긴 조건 텍스트가 참여자 두 개 사이 간격보다 훨씬 길게 이어집니다";
        s.items.push(SequenceItem::FragmentStart { kind: "alt".into(), label: label.into() });
        s.items.push(SequenceItem::Message { from: a, to: b, label: "hi".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        s.items.push(SequenceItem::FragmentEnd);
        let out = render(&s, &Theme::none(), 200).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        assert!(joined.contains(&format!("[{label}]")), "{joined}");
    }
}
