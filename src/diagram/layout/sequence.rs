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
    /// 참여자별 최대 동시 활성화 겹침 수(겹치지 않으면 1). `position_participants()`가
    /// 간격 확보에 쓴다. 자기 메시지(`from == to`)의 활성화는 집계하지 않는다
    /// (sequence-nested-activation-offset — 그 경로는 activate/deactivate 자체를
    /// 읽지 않는 기존 결함이라 범위 밖).
    max_activation_depth: Vec<usize>,
    centers: Vec<usize>,
    total_width: usize,
}

/// 겹치는 활성화 막대를 옆으로 어긋나게 그릴 때 한 칸씩 더하는 상한. 이보다 깊게
/// 겹쳐도 더 벌어지지 않고 이 칸에 겹쳐 그린다(다이어그램 폭이 무한정 늘지 않게).
const ACTIVATION_OFFSET_CAP: usize = 3;

/// 깊이 인덱스(0부터, 그 활성화가 열릴 때 이미 열려 있던 구간 수)를 실제 오프셋
/// 칸 수로 바꾼다.
fn activation_offset(depth_index: usize) -> usize {
    depth_index.min(ACTIVATION_OFFSET_CAP)
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
            max_activation_depth: vec![1; sequence.participants.len()],
            centers: Vec::new(),
            total_width: 0,
        };
        layout.collect_fragments();
        layout.collect_activation_depths();
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

    /// 참여자별로 동시에 열려 있던 활성화 구간 수의 최댓값을 미리 구해 둔다 —
    /// `position_participants()`가 간격에 반영해야 하므로 그리기 전에 필요하다.
    /// `draw()`가 실제로 막대를 그릴 때 다시 훑는 것과 같은 열고/닫기 규칙(LIFO)을
    /// 쓰지만, 여기서는 칸 위치가 아니라 깊이 카운터만 본다.
    fn collect_activation_depths(&mut self) {
        let mut depth = vec![0usize; self.sequence.participants.len()];
        for item in &self.sequence.items {
            match item {
                SequenceItem::Message { from, to, activate_target, deactivate_source, .. } if from != to => {
                    if *activate_target {
                        depth[*to] += 1;
                        self.max_activation_depth[*to] = self.max_activation_depth[*to].max(depth[*to]);
                    }
                    if *deactivate_source {
                        depth[*from] = depth[*from].saturating_sub(1);
                    }
                }
                SequenceItem::Activate(p) => {
                    depth[*p] += 1;
                    self.max_activation_depth[*p] = self.max_activation_depth[*p].max(depth[*p]);
                }
                SequenceItem::Deactivate(p) => {
                    depth[*p] = depth[*p].saturating_sub(1);
                }
                _ => {}
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
        // 겹치는 활성화 막대는 참여자 칸에서 오른쪽으로 어긋나 그려지므로(오프셋 칸
        // 수만큼) 그 옆 간격이 최소한 그만큼 더 있어야 옆 참여자의 생명선과 겹치지
        // 않는다(sequence-nested-activation-offset). 겹치지 않는 참여자(깊이 1)는
        // 손대지 않는다.
        for (p, &depth) in self.max_activation_depth.iter().enumerate() {
            if depth <= 1 {
                continue;
            }
            let extra = activation_offset(depth - 1);
            if p + 1 < count {
                constraints.push((p, p + 1, gaps[p] + extra));
            } else {
                right_margin += extra;
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
        // 세 번째 필드는 이 구간이 열릴 때의 깊이 인덱스(0부터) — 오프셋 칸 계산에 쓴다.
        let mut active: Vec<Vec<(usize, Option<usize>, usize)>> = vec![Vec::new(); count];
        let mut active_depth = vec![0usize; count];
        let mut last_arrow_row = row;
        let mut fragment_stack: Vec<usize> = Vec::new();
        let mut delays: Vec<usize> = Vec::new();
        // 활성화를 열거나 닫는 메시지의 화살표 쪽 끝을 그 구간의 오프셋 칸까지
        // 연장하기 위한 항목별 (참여자와 무관하게, from/to 어느 쪽인지는
        // draw_message가 이미 안다) 오프셋 칸 수.
        let mut open_offset: Vec<Option<usize>> = vec![None; self.sequence.items.len()];
        let mut close_offset: Vec<Option<usize>> = vec![None; self.sequence.items.len()];
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
                        let depth_index = active_depth[*to];
                        active_depth[*to] += 1;
                        active[*to].push((last_arrow_row, None, depth_index));
                        open_offset[index] = Some(activation_offset(depth_index));
                    }
                    if *deactivate_source {
                        let closing_depth_index = active_depth[*from].saturating_sub(1);
                        Self::close_activation(&mut active[*from], last_arrow_row);
                        active_depth[*from] = closing_depth_index;
                        close_offset[index] = Some(activation_offset(closing_depth_index));
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
                    let depth_index = active_depth[*p];
                    active_depth[*p] += 1;
                    active[*p].push((last_arrow_row, None, depth_index));
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
            for (start, end, depth_index) in &active[p] {
                let end = end.unwrap_or(body_end);
                if *start < end {
                    canvas.vline(x + activation_offset(*depth_index), *start, end, LineKind::Heavy, theme.diagram_accent);
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
                    let from_offset = close_offset[*index].unwrap_or(0);
                    let to_offset = open_offset[*index].unwrap_or(0);
                    self.draw_message(canvas, *index, *from, *to, *start_row, *kind, *head, from_offset, to_offset);
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
            // 메시지·생명선(Solid)·노트(Dashed)와 겹치지 않도록 프레임 테두리만 Heavy로 그린다
            // (sequence-fragment-frame-distinction).
            canvas.rect(x_lo, fragment.start_row, x_hi - x_lo + 1, height, LineKind::Heavy, theme.diagram_group, false);
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

    fn close_activation(intervals: &mut [(usize, Option<usize>, usize)], row: usize) {
        if let Some(open) = intervals.iter_mut().rev().find(|(_, end, _)| end.is_none()) {
            open.1 = Some(row);
        }
    }

    /// `from_offset`/`to_offset`: 이 메시지가 그 쪽에서 활성화를 열거나(안쪽으로
    /// 새로 겹치는 구간) 닫을 때(가장 안쪽 구간이 닫힘), 그 구간이 그려지는 오프셋
    /// 칸 수. 겹침이 없으면 둘 다 0이라 기존과 동일하게 동작한다
    /// (sequence-nested-activation-offset).
    fn draw_message(&self, canvas: &mut Canvas, index: usize, from: usize, to: usize, start_row: usize, kind: LineKind, head: Marker, from_offset: usize, to_offset: usize) {
        let theme = self.theme;
        let (x_from, x_to) = (self.centers[from], self.centers[to]);
        let lines = &self.labels[index];
        let arrow_row = start_row + lines.len();
        // 라벨 가운데 정렬은 기준 생명선 칸으로 그대로 한다 — 화살표 자체(아래)만
        // 오프셋 칸까지 연장한다.
        let (label_lo, label_hi) = (x_from.min(x_to) + 1, x_from.max(x_to) - 1);
        for (k, line) in lines.iter().enumerate() {
            canvas.text_centered(label_lo, start_row + k, label_hi - label_lo + 1, line, theme.diagram_text);
        }
        let (eff_from, eff_to) = (x_from + from_offset, x_to + to_offset);
        let (lo, hi) = (eff_from.min(eff_to) + 1, eff_from.max(eff_to) - 1);
        canvas.hline(lo, hi, arrow_row, kind, theme.diagram_line);
        let rightward = eff_to > eff_from;
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
        canvas.join(eff_from, arrow_row, origin_bits, LineKind::Solid, theme.diagram_line, false);
    }

    fn draw_self_message(&self, canvas: &mut Canvas, index: usize, participant: usize, start_row: usize, kind: LineKind, head: Marker) {
        let theme = self.theme;
        let x = self.centers[participant];
        // 루프 폭 3칸(x+1..x+3). 화살촉은 도착 지점인 생명선(x)에 바로 붙어야 한다(다른 메시지가
        // 상대 생명선에 바로 닿는 것과 같은 관례). 여백이 필요한 곳은 화살촉과 루프의 세로
        // 연결선(x+3, 모서리) 사이(x+2)다 — 화살촉과 연결선이 붙어 `<|`처럼 보이던 것을
        // `<-|`처럼 여백이 있게 고친다.
        canvas.join(x, start_row, EAST, LineKind::Solid, theme.diagram_line, false);
        canvas.hline(x + 1, x + 3, start_row, kind, theme.diagram_line);
        canvas.vline(x + 3, start_row, start_row + 2, kind, theme.diagram_line);
        canvas.join(x + 3, start_row, 0, kind, theme.diagram_line, true);
        canvas.hline(x + 2, x + 3, start_row + 2, kind, theme.diagram_line);
        canvas.join(x + 3, start_row + 2, 0, kind, theme.diagram_line, true);
        let glyph = match head {
            Marker::OpenArrow => '<',
            Marker::Cross => '×',
            _ => '◀',
        };
        canvas.put(x + 1, start_row + 2, glyph, theme.diagram_line);
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

    /// 1.1/1.2 프래그먼트 바깥 테두리는 굵은선이라 메시지·생명선(가는 실선)과 겹치지 않는
    /// 글자로 그려진다. 1.3 `else` 구분선은 여전히 파선이고, 생명선과 만나는 지점은 굵은선이
    /// 아닌 평범한 교차(`┼`)라 프레임 테두리(굵은 교차 `╋`)와 헷갈리지 않는다.
    #[test]
    fn fragment_border_uses_heavy_line_distinct_from_flow() {
        let out = render(&sample(), &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        assert!(joined.contains(['┏', '┓', '┗', '┛', '━', '┃']), "테두리는 굵은선이어야 한다: {joined}");
        // else 구분선(파선)이 생명선과 만나는 지점은 가는 교차(┼)여야 한다 — 굵은 교차(╋)와 다름.
        let else_row = text.iter().find(|l| l.contains("[fail]")).expect("else 구분선이 있어야 한다");
        assert!(else_row.contains('┼'), "else 구분선-생명선 교차는 가는 교차여야 한다: {else_row}");
        assert!(else_row.contains('╌'), "else 구분선은 파선이어야 한다: {else_row}");
    }

    /// 2.2 프래그먼트가 중첩돼도(예: `loop` 안에 `alt`) 깊이와 무관하게 모두 같은 굵은선으로
    /// 그려진다(block-beta처럼 깊이별로 패턴이 바뀌지 않음).
    #[test]
    fn nested_fragments_share_the_same_heavy_border() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        let b = s.intern("B", "B", ParticipantKind::Box);
        s.items.push(SequenceItem::FragmentStart { kind: "loop".into(), label: "재시도".into() });
        s.items.push(SequenceItem::FragmentStart { kind: "alt".into(), label: "조건".into() });
        s.items.push(SequenceItem::Message { from: a, to: b, label: "hi".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        s.items.push(SequenceItem::FragmentEnd);
        s.items.push(SequenceItem::FragmentEnd);
        let out = render(&s, &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        assert!(joined.contains("loop [재시도]"), "{joined}");
        assert!(joined.contains("alt [조건]"), "{joined}");
        // 두 프레임 다 같은 굵은선 문자 집합만 쓴다(깊이별로 다른 LineKind로 순환하지 않음).
        let heavy_rows = text.iter().filter(|l| l.contains(['┏', '┓', '┗', '┛', '━', '┃'])).count();
        assert!(heavy_rows >= 2, "안쪽·바깥쪽 프레임 모두 굵은선 줄이 있어야 한다: {joined}");
    }

    /// 2.1 메시지 화살표·생명선이 프래그먼트 경계(위/아래 테두리)를 지나는 교차 지점이 패닉 없이
    /// 유효한 문자로 그려진다.
    #[test]
    fn messages_crossing_fragment_border_do_not_panic() {
        let out = render(&sample(), &Theme::none(), 80);
        assert!(out.is_some(), "패닉 없이 렌더링돼야 한다");
        let text: Vec<String> = out.unwrap().iter().map(Line::plain).collect();
        let joined = text.join("\n");
        // 프레임 위/아래 테두리와 생명선이 만나는 지점은 굵은 십자(╋)로 이어진다.
        assert!(joined.contains('╋'), "생명선-테두리 교차가 있어야 한다: {joined}");
    }

    #[test]
    fn too_narrow_returns_none() {
        assert!(render(&sample(), &Theme::none(), 10).is_none());
    }

    /// self-message(재귀)의 화살촉은 도착 지점인 생명선에 바로 붙어야 하고(다른 메시지가 상대
    /// 생명선에 바로 닿는 것과 같은 관례), 대신 화살촉과 루프의 세로 연결선(모서리) 사이에 최소
    /// 한 칸의 여백이 있어야 굽은 모서리 모양이 화살촉에 눌리지 않고 드러난다(`<|`처럼 붙어
    /// 보이던 것을 `<-|`처럼 고친 회귀). 교차 메시지의 도착 화살촉과 헷갈리지 않도록 참가자
    /// 하나에 self-message 하나만 있는 최소 픽스처를 쓴다.
    #[test]
    fn self_message_arrowhead_touches_lifeline_but_not_the_corner() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        s.items.push(SequenceItem::Message { from: a, to: a, label: "retry".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        let out = render(&s, &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let arrow_row = text.iter().find(|l| l.contains('◀')).expect("화살촉 줄이 있어야 한다");
        let chars: Vec<char> = arrow_row.chars().collect();
        let arrow_col = chars.iter().position(|&c| c == '◀').expect("화살촉 문자가 있어야 한다");
        let lifeline_col = chars[..arrow_col].iter().rposition(|&c| c == '│').expect("생명선 문자가 있어야 한다");
        assert_eq!(arrow_col, lifeline_col + 1, "화살촉은 생명선에 바로 붙어야 한다: {arrow_row}");
        let corner_col = chars[arrow_col + 1..].iter().position(|&c| c == '╯' || c == '┘').map(|i| arrow_col + 1 + i).expect("모서리 문자가 있어야 한다");
        assert!(corner_col > arrow_col + 1, "화살촉과 모서리 사이에 최소 한 칸이 있어야 한다: {arrow_row}");
        let between = &chars[arrow_col + 1..corner_col];
        assert!(between.iter().all(|&c| c == '─' || c == '╌'), "화살촉과 모서리 사이는 실선/점선이어야 한다: {arrow_row}");
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

    /// A와 C가 둘 다 B를 활성화한 채 겹치는 시나리오. LIFO 규칙(close_activation)이
    /// 전제이므로 `-`(deactivate) 접미사가 반드시 여는 쪽과 같은 메시지에 실려야
    /// 한다 — mermaid `-->>-` 접미사 파싱 자체는 이 스펙 밖의 별도 결함이라 파서를
    /// 거치지 않고 IR을 직접 구성한다.
    fn overlapping_activation_sequence() -> Sequence {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        let b = s.intern("B", "B", ParticipantKind::Box);
        let c = s.intern("C", "C", ParticipantKind::Box);
        s.items.push(SequenceItem::Message { from: a, to: b, label: "call1".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        s.items.push(SequenceItem::Message { from: c, to: b, label: "call2".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        s.items.push(SequenceItem::Message { from: b, to: c, label: "return2".into(), kind: LineKind::Dashed, head: Marker::Arrow, activate_target: false, deactivate_source: true });
        s.items.push(SequenceItem::Message { from: b, to: a, label: "return1".into(), kind: LineKind::Dashed, head: Marker::Arrow, activate_target: false, deactivate_source: true });
        s
    }

    /// 활성화 막대 글자('┃')만 남기고 나머지 칸은 공백으로 바꿔, 어느 열에 막대가
    /// 있는지 행마다 확인하기 쉽게 만든다.
    fn bar_columns(row: &str) -> Vec<usize> {
        row.chars().enumerate().filter(|(_, c)| *c == '┃').map(|(i, _)| i).collect()
    }

    #[test]
    fn overlapping_activations_draw_at_different_columns() {
        let out = render(&overlapping_activation_sequence(), &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        // "return2" 라벨이 있는 줄은 두 활성화가 모두 열려 있는 유일한 구간이다 —
        // 그 줄에서 활성화 막대 글자가 서로 다른 두 칸에 있어야 한다(1.1).
        let row = text.iter().find(|l| l.contains("return2")).unwrap_or_else(|| panic!("return2 줄을 못 찾음: {joined}"));
        let cols = bar_columns(row);
        assert_eq!(cols.len(), 2, "겹치는 두 활성화 막대가 같은 줄에 둘 다 보여야 한다: {row:?}\n{joined}");
        assert_ne!(cols[0], cols[1], "두 활성화 막대가 같은 칸에 겹쳐 그려지면 안 된다: {row:?}");
        assert_eq!(cols[1] - cols[0], 1, "두 번째(안쪽) 막대는 첫 번째보다 한 칸 오른쪽에 있어야 한다: {row:?}");
    }

    #[test]
    fn non_overlapping_activation_bar_stays_on_lifeline() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        let b = s.intern("B", "B", ParticipantKind::Box);
        s.items.push(SequenceItem::Message { from: a, to: b, label: "hi".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        s.items.push(SequenceItem::Message { from: b, to: a, label: "bye".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: true });
        let out = render(&s, &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        let row = text.iter().find(|l| l.contains('┃')).unwrap_or_else(|| panic!("활성화 막대 줄을 못 찾음: {joined}"));
        assert_eq!(bar_columns(row).len(), 1, "겹치지 않는 활성화는 지금처럼 막대가 한 칸에만 있어야 한다(1.3): {row:?}");
    }

    #[test]
    fn three_overlapping_activations_step_one_column_each() {
        let mut s = Sequence::default();
        let a = s.intern("A", "A", ParticipantKind::Box);
        let b = s.intern("B", "B", ParticipantKind::Box);
        let c = s.intern("C", "C", ParticipantKind::Box);
        let d = s.intern("D", "D", ParticipantKind::Box);
        for (from, label) in [(a, "1"), (c, "2"), (d, "3")] {
            s.items.push(SequenceItem::Message { from, to: b, label: label.into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        }
        s.items.push(SequenceItem::Message { from: b, to: a, label: "mark".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: false, deactivate_source: false });
        let out = render(&s, &Theme::none(), 100).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        let row = text.iter().find(|l| l.contains("mark")).unwrap_or_else(|| panic!("mark 줄을 못 찾음: {joined}"));
        let cols = bar_columns(row);
        assert_eq!(cols, vec![cols[0], cols[0] + 1, cols[0] + 2], "3겹이면 칸이 하나씩 더 어긋나야 한다(1.2): {row:?}\n{joined}");
    }

    #[test]
    fn activation_offset_caps_and_never_panics() {
        let mut s = Sequence::default();
        let b = s.intern("B", "B", ParticipantKind::Box);
        let callers: Vec<usize> = (0..6).map(|i| s.intern(&format!("P{i}"), &format!("P{i}"), ParticipantKind::Box)).collect();
        for &p in &callers {
            s.items.push(SequenceItem::Message { from: p, to: b, label: "go".into(), kind: LineKind::Solid, head: Marker::Arrow, activate_target: true, deactivate_source: false });
        }
        // 6겹(상한 3보다 훨씬 깊음)이어도 패닉 없이 끝까지 렌더링돼야 한다(4.1).
        let out = render(&s, &Theme::none(), 160);
        assert!(out.is_some(), "폭 안에서 렌더링이 실패하면 안 된다");
        let text: Vec<String> = out.unwrap().iter().map(Line::plain).collect();
        let last_row = text.iter().rev().find(|l| l.contains('┃')).unwrap_or_else(|| panic!("활성화 막대 줄을 못 찾음"));
        let cols = bar_columns(last_row);
        let span = cols.iter().max().unwrap() - cols.iter().min().unwrap();
        assert!(span <= ACTIVATION_OFFSET_CAP, "상한을 넘어 칸이 계속 벌어지면 안 된다(4.1): span={span}");
    }

    #[test]
    fn message_arrow_reaches_the_offset_column_it_opens_or_closes() {
        let out = render(&overlapping_activation_sequence(), &Theme::none(), 80).unwrap();
        let text: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        // call2가 여는 활성화(오프셋 1칸)의 화살촉은 두 번째 막대 칸 바로 다음(오른쪽)에 와야 한다.
        let call2_row = text.iter().find(|l| l.contains("call2")).unwrap_or_else(|| panic!("call2 줄을 못 찾음: {joined}"));
        // call2 라벨 줄이 아니라 화살표 줄(그다음 줄)에서 확인한다.
        let call2_arrow_row = text[text.iter().position(|l| l == call2_row).unwrap() + 1].clone();
        let cols = bar_columns(&call2_arrow_row);
        assert_eq!(cols.len(), 2, "call2 화살표 줄엔 두 활성화 막대가 다 보여야 한다: {call2_arrow_row:?}\n{joined}");
        let arrowhead_col = call2_arrow_row.chars().position(|c| c == '◀').unwrap_or_else(|| panic!("화살촉(◀)을 못 찾음: {call2_arrow_row:?}"));
        assert_eq!(arrowhead_col, cols[1] + 1, "call2가 새로 여는 활성화 칸 바로 옆에 화살촉이 와야 한다(3.1): {call2_arrow_row:?}");
    }
}

