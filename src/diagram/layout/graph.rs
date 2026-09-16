//! 계층 배치(Sugiyama)로 그래프를 그린다.
//!
//! 1. 되돌아가는 간선을 DFS로 뒤집어 DAG를 만들고 최장 경로로 층을 매긴다.
//! 2. 그룹을 블록으로 보고, 블록 안에서 자식들을 무게중심 순서로 겹치지 않게 늘어놓는다.
//!    블록은 여러 층에 걸쳐 같은 띠를 차지하므로 그룹 테두리가 서로 포개지지 않는다.
//! 3. 층 사이 "통로"에 간선의 가로 구간을 넣고, 겹치는 구간은 다른 줄을 쓴다.
//! 4. 방향(TB/LR)은 배치 좌표계(along/across)를 캔버스 좌표로 옮길 때만 관여한다.

use crate::diagram::canvas::{Canvas, EAST, LineKind, NORTH, SOUTH, WEST};
use crate::diagram::ir::{Direction, Graph, Marker, Shape};
use crate::diagram::layout::shape;
use crate::line::Line;
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

pub fn render(graph: &Graph, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if graph.nodes.is_empty() {
        return None;
    }
    let preferred = graph.direction.unwrap_or(Direction::TopDown);
    let caps = [30usize, 22, 16, 12];
    // 라벨 폭마다: 원하는 방향 → 반대 방향 → 위→아래로 층 접기. 라벨을 잘게 접는 것보다
    // 층을 접어 세로로 길어지는 쪽이 읽기 낫다.
    let attempts = caps.iter().flat_map(|&cap| [(cap, preferred, false), (cap, preferred.other(), false), (cap, Direction::TopDown, true)]);
    for (cap, direction, allow_fold) in attempts {
        if let Some(canvas) = Layout::build(graph, theme, direction, cap, width, allow_fold) {
            let mut lines = canvas.into_lines();
            if !graph.title.is_empty() {
                lines.insert(0, Line::empty());
                lines.insert(0, Line::single(graph.title.clone(), theme.diagram_text.bold()));
            }
            return Some(lines);
        }
    }
    None
}

struct LayoutNode {
    node: Option<usize>,
    /// 가상 노드가 속한 간선.
    edge: Option<usize>,
    layer: usize,
    group: Option<usize>,
    along_size: usize,
    across_size: usize,
    /// 자기 자신 간선처럼 상자 옆에 더 필요한 칸.
    extra_across: usize,
    across: usize,
    sections: Vec<Vec<String>>,
}

impl LayoutNode {
    fn footprint(&self) -> usize {
        self.across_size + self.extra_across
    }
    fn center(&self) -> f64 {
        self.across as f64 + self.across_size as f64 / 2.0
    }
}

struct Segment {
    edge: usize,
    from: usize,
    to: usize,
    is_first: bool,
    is_last: bool,
    exit: usize,
    entry: usize,
    /// 노드 시작점 기준 접점 위치. 노드가 움직여도 유지되므로 직선화가 접점끼리 맞출 수 있다.
    exit_offset: usize,
    entry_offset: usize,
    /// 출발 노드의 나가는 간선 중 몇 번째/총 몇 개인지(라벨을 어느 쪽에 둘지 정한다).
    exit_rank: (usize, usize),
    entry_rank: (usize, usize),
    channel: Option<usize>,
    /// TB에서 라벨의 가로 시작 위치(통로 줄 위).
    label_x: Option<usize>,
}

#[derive(Clone, Copy)]
enum Child {
    Node(usize),
    Block(usize),
}

fn same_child(a: Child, b: Child) -> bool {
    matches!((a, b), (Child::Node(x), Child::Node(y)) if x == y) || matches!((a, b), (Child::Block(x), Child::Block(y)) if x == y)
}

#[derive(Clone, Copy)]
struct FoldCandidate {
    target: Child,
    layer: usize,
    /// 같은 줄의 다른 자식들이 걸친 마지막 층(블록을 그 아래로 내릴 때 쓴다).
    others_max: usize,
}

struct Block {
    group: Option<usize>,
    children: Vec<Child>,
    start: usize,
    width: usize,
    layer_min: usize,
    layer_max: usize,
    registered: bool,
}

struct Layout<'a> {
    graph: &'a Graph,
    theme: &'a Theme,
    direction: Direction,
    lnodes: Vec<LayoutNode>,
    /// 층 접기로 정해진 노드별 최소 층.
    min_layer: Vec<usize>,
    reversed: Vec<bool>,
    segments: Vec<Segment>,
    adjacency: Vec<Vec<usize>>,
    blocks: Vec<Block>,
    lnode_block: Vec<usize>,
    layer_count: usize,
    layer_start: Vec<usize>,
    layer_content: Vec<usize>,
    top_levels: Vec<usize>,
    bottom_levels: Vec<usize>,
    gap: Vec<usize>,
    gap_label_room: Vec<usize>,
    gap_head_room: Vec<usize>,
    /// 통로 라벨이 블록 폭 밖으로 삐져나갈 때 필요한 가로 폭.
    extra_across: usize,
    /// 라벨이 그룹 밖으로 나가지 않도록 블록마다 더 주는 폭.
    block_extra: Vec<usize>,
    /// `DG_DEBUG`가 켜져 있으면 배선 정보를 stderr에 적는다.
    debug: bool,
}

const GAP_ALONG_MIN: usize = 3;
/// 층 접기 최대 횟수.
const MAX_FOLDS: usize = 40;
/// 폭이 줄지 않는 접기를 연속으로 참아 주는 횟수.
const FOLD_PATIENCE: usize = 12;

impl<'a> Layout<'a> {
    fn build(graph: &'a Graph, theme: &'a Theme, direction: Direction, cap: usize, width: usize, allow_fold: bool) -> Option<Canvas> {
        let mut layout = Layout::new(graph, theme, direction, cap);
        layout.assign_layers();
        layout.push_outputs_below_groups();
        layout.arrange(false);
        let mut best_width = layout.canvas_size().0;
        let mut stalls = 0;
        for _ in 0..MAX_FOLDS {
            let (canvas_width, _) = layout.canvas_size();
            if canvas_width == 0 {
                return None;
            }
            if layout.debug {
                eprintln!("attempt cap {cap} {direction:?} fold {allow_fold}: width {canvas_width} / {width}");
            }
            if canvas_width <= width {
                // 최종 배치에서만 이웃 교환으로 간선을 짧게 한다. 폭이 넘치면 교환 전으로 되돌린다.
                layout.reset_arrangement();
                layout.arrange(true);
                if layout.canvas_size().0 > width {
                    layout.reset_arrangement();
                    layout.arrange(false);
                }
                let (canvas_width, canvas_height) = layout.canvas_size();
                let mut canvas = Canvas::new(canvas_width, canvas_height);
                layout.draw(&mut canvas);
                return Some(canvas);
            }
            if !allow_fold || direction != Direction::TopDown || !layout.fold_widest_row() {
                return None;
            }
            layout.reset_arrangement();
            layout.arrange(false);
            let folded_width = layout.canvas_size().0;
            if folded_width < best_width {
                best_width = folded_width;
                stalls = 0;
            } else {
                stalls += 1;
                if stalls > FOLD_PATIENCE {
                    return None;
                }
            }
        }
        None
    }

    /// (가로, 세로) 캔버스 크기.
    fn canvas_size(&self) -> (usize, usize) {
        let total_across = self.blocks[0].width.max(self.extra_across);
        let total_along = self.total_along();
        match self.direction {
            Direction::TopDown => (total_across, total_along),
            Direction::LeftRight => (total_along, total_across),
        }
    }

    /// 층이 정해진 뒤의 배치 전 과정. `refine`이면 이웃 교환 탐색까지 한다.
    fn arrange(&mut self, refine: bool) {
        self.make_segments();
        self.build_blocks();
        for _ in 0..4 {
            self.place();
            self.reorder();
        }
        self.place();
        if refine {
            self.improve_by_swaps();
        }
        self.assign_ports();
        self.straighten();
        self.route();
        if self.reserve_label_room() {
            self.place();
            self.assign_ports();
            self.straighten();
            self.route();
        }
    }

    /// 가상 노드·블록·배선 정보를 지우고 층 배정만 남긴다.
    fn reset_arrangement(&mut self) {
        let real_count = self.graph.nodes.len();
        self.lnodes.truncate(real_count);
        for node in &mut self.lnodes {
            node.across = 0;
        }
        self.segments.clear();
        self.adjacency.clear();
        self.blocks.clear();
        self.lnode_block.clear();
        self.extra_across = 0;
        self.block_extra.clear();
    }

    /// 가장 넓은 줄의 맨 오른쪽 자식을 아래로 내린다. 나눌 줄이 없으면 false.
    fn fold_widest_row(&mut self) -> bool {
        let Some(candidate) = self.fold_candidates().into_iter().next() else { return false };
        self.apply_fold(candidate);
        true
    }

    /// 접기 후보: 넓은 "줄"(한 블록의 한 층에 나란히 놓인 자식들)부터, 줄 안에서는 오른쪽 자식부터.
    fn fold_candidates(&self) -> Vec<FoldCandidate> {
        let mut rows: Vec<(usize, usize, usize)> = Vec::new(); // (폭, 블록, 층)
        for block in 0..self.blocks.len() {
            if !self.blocks[block].registered {
                continue;
            }
            for layer in self.blocks[block].layer_min..=self.blocks[block].layer_max {
                let row = self.row_children(block, layer);
                if row.iter().filter(|&&c| self.is_movable(c)).count() < 2 {
                    continue;
                }
                let start = row.iter().map(|&c| self.child_span(c).0).min().unwrap_or(0);
                let end = row.iter().map(|&c| self.child_span(c).1).max().unwrap_or(0);
                rows.push((end - start, block, layer));
            }
        }
        rows.sort_by_key(|row| std::cmp::Reverse(row.0));
        let mut candidates = Vec::new();
        for (_, block, layer) in rows.into_iter().take(1) {
            let mut row = self.row_children(block, layer);
            row.sort_by_key(|&c| self.child_span(c).0);
            let others_max = |target: Child| {
                row.iter()
                    .filter(|&&c| !same_child(c, target))
                    .map(|&c| self.child_layer_max(c))
                    .max()
                    .unwrap_or(layer)
            };
            for &target in row.iter().rev().filter(|&&c| self.is_movable(c)).take(1) {
                candidates.push(FoldCandidate { target, layer, others_max: others_max(target) });
            }
        }
        candidates
    }

    /// 후보를 적용한다: 노드면 한 층 아래로, 하위 그룹이면 통째로 나머지 자식들 아래로(최소 층 제약).
    fn apply_fold(&mut self, candidate: FoldCandidate) {
        match candidate.target {
            Child::Node(i) => self.min_layer[i] = self.min_layer[i].max(candidate.layer + 1),
            Child::Block(moved) => {
                let offset = (candidate.others_max + 1).saturating_sub(self.blocks[moved].layer_min).max(1);
                for member in self.child_members(Child::Block(moved)) {
                    if self.lnodes[member].node.is_some() {
                        self.min_layer[member] = self.min_layer[member].max(self.lnodes[member].layer + offset);
                    }
                }
            }
        }
        self.assign_layers();
    }

    /// 블록 `block`의 자식 가운데 층 `layer`에 걸친 것들.
    fn row_children(&self, block: usize, layer: usize) -> Vec<Child> {
        self.blocks[block]
            .children
            .iter()
            .copied()
            .filter(|&child| match child {
                Child::Node(i) => self.lnodes[i].layer == layer,
                Child::Block(b) => self.blocks[b].registered && self.blocks[b].layer_min <= layer && layer <= self.blocks[b].layer_max,
            })
            .collect()
    }

    fn is_movable(&self, child: Child) -> bool {
        match child {
            // 가상 노드는 배선의 산물이고, 닻은 그룹 첫 층에 붙어 있어야 한다.
            Child::Node(i) => self.lnodes[i].node.is_some_and(|n| self.graph.nodes[n].shape != Shape::Anchor),
            Child::Block(_) => true,
        }
    }

    /// 자식의 가로 [시작, 끝) 구간.
    fn child_span(&self, child: Child) -> (usize, usize) {
        match child {
            Child::Node(i) => (self.lnodes[i].across, self.lnodes[i].across + self.lnodes[i].footprint()),
            Child::Block(b) => (self.blocks[b].start, self.blocks[b].start + self.blocks[b].width),
        }
    }

    fn child_layer_max(&self, child: Child) -> usize {
        match child {
            Child::Node(i) => self.lnodes[i].layer,
            Child::Block(b) => self.blocks[b].layer_max,
        }
    }

    /// 어떤 그룹 안의 노드들에서만 간선이 들어오는 바깥 노드는 그 그룹 아래에 둔다
    /// (상자에서 나오는 출력은 상자 밑으로).
    fn push_outputs_below_groups(&mut self) {
        let n = self.graph.nodes.len();
        let mut changed = false;
        for node in 0..n {
            let node_groups = self.graph.ancestors(self.graph.nodes[node].group);
            let mut source_group: Option<Option<usize>> = None;
            let mut has_successor_inside = false;
            for (e, edge) in self.graph.edges.iter().enumerate() {
                if edge.from == edge.to {
                    continue;
                }
                let (from, to) = if self.reversed[e] { (edge.to, edge.from) } else { (edge.from, edge.to) };
                if to == node {
                    // from이 속한 그룹 가운데 node를 품지 않는 가장 바깥 것
                    let outer = self
                        .graph
                        .ancestors(self.graph.nodes[from].group)
                        .into_iter()
                        .rfind(|g| !node_groups.contains(g));
                    match source_group {
                        None => source_group = Some(outer),
                        Some(existing) if existing == outer => {}
                        Some(_) => source_group = Some(None),
                    }
                }
                if from == node && self.graph.ancestors(self.graph.nodes[to].group).iter().any(|g| !node_groups.contains(g)) {
                    has_successor_inside = true;
                }
            }
            let Some(Some(group)) = source_group else { continue };
            if has_successor_inside {
                continue;
            }
            let group_max = (0..n)
                .filter(|&i| self.graph.ancestors(self.graph.nodes[i].group).contains(&group))
                .map(|i| self.lnodes[i].layer)
                .max()
                .unwrap_or(0);
            if self.min_layer[node] < group_max + 1 {
                self.min_layer[node] = group_max + 1;
                changed = true;
            }
        }
        if changed {
            self.assign_layers();
        }
    }

    fn new(graph: &'a Graph, theme: &'a Theme, direction: Direction, cap: usize) -> Layout<'a> {
        let mut degree = vec![(0usize, 0usize); graph.nodes.len()];
        for edge in &graph.edges {
            if edge.from != edge.to {
                degree[edge.from].1 += 1;
                degree[edge.to].0 += 1;
            }
        }
        let lnodes = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let sections = shape::wrapped_sections(node, cap);
                let (w, h) = shape::measure(node.shape, &sections);
                let (w, h) = if node.shape == Shape::Anchor {
                    // 보이지 않는 닻: 간선마다 접점이 두 칸씩 떨어지도록 폭만 확보한다.
                    let (incoming, outgoing) = degree[index];
                    let span = 2 * incoming.max(outgoing).max(1) + 1;
                    match direction {
                        Direction::TopDown => (span, 1),
                        Direction::LeftRight => (1, span),
                    }
                } else {
                    (w, h)
                };
                let (along_size, across_size) = match direction {
                    Direction::TopDown => (h, w),
                    Direction::LeftRight => {
                        // 왼쪽→오른쪽에서는 간선이 나가는 줄마다 라벨이 붙으므로 두 줄 간격이 필요하다.
                        let (incoming, outgoing) = degree[index];
                        let needed = 2 * incoming.max(outgoing) + 1;
                        (w, if h >= 3 { h.max(needed) } else { h })
                    }
                };
                LayoutNode { node: Some(0), edge: None, layer: 0, group: node.group, along_size, across_size, extra_across: 0, across: 0, sections }
            })
            .enumerate()
            .map(|(i, mut n)| {
                n.node = Some(i);
                n
            })
            .collect();
        let _ = &degree;
        let mut layout = Layout {
            graph,
            theme,
            direction,
            lnodes,
            min_layer: vec![0; graph.nodes.len()],
            reversed: vec![false; graph.edges.len()],
            segments: Vec::new(),
            adjacency: Vec::new(),
            blocks: Vec::new(),
            lnode_block: Vec::new(),
            layer_count: 0,
            layer_start: Vec::new(),
            layer_content: Vec::new(),
            top_levels: Vec::new(),
            bottom_levels: Vec::new(),
            gap: Vec::new(),
            gap_label_room: Vec::new(),
            gap_head_room: Vec::new(),
            extra_across: 0,
            block_extra: Vec::new(),
            debug: std::env::var_os("DG_DEBUG").is_some(),
        };
        for edge in &graph.edges {
            if edge.from == edge.to {
                let label_width = width_of(&edge.label);
                let extra = match direction {
                    Direction::TopDown => 3 + label_width + usize::from(label_width > 0),
                    Direction::LeftRight => 2,
                };
                let node = &mut layout.lnodes[edge.from];
                node.extra_across = node.extra_across.max(extra);
            }
        }
        layout
    }

    fn gap_across(&self) -> usize {
        match self.direction {
            Direction::TopDown => 3,
            Direction::LeftRight => 1,
        }
    }

    /// 이웃 사이 간격. 가상 노드(선 하나)끼리는 붙여도 된다.
    fn gap_between(&self, a: Child, b: Child) -> usize {
        let is_dummy = |child: Child| matches!(child, Child::Node(i) if self.lnodes[i].node.is_none());
        match (is_dummy(a), is_dummy(b)) {
            (true, true) => 1,
            (true, false) | (false, true) => 2.min(self.gap_across()),
            (false, false) => self.gap_across(),
        }
    }

    // ── 1. 층 ──────────────────────────────────────────────────────────

    fn assign_layers(&mut self) {
        let n = self.graph.nodes.len();
        let mut outgoing: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if edge.from != edge.to {
                outgoing[edge.from].push((edge.to, i));
            }
        }
        let mut state = vec![0u8; n];
        for start in 0..n {
            if state[start] == 0 {
                self.mark_back_edges(start, &outgoing, &mut state);
            }
        }
        let mut incoming_count = vec![0usize; n];
        let mut directed: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut add = |from: usize, to: usize| {
            directed[from].push(to);
            incoming_count[to] += 1;
        };
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue;
            }
            let (from, to) = if self.reversed[i] { (edge.to, edge.from) } else { (edge.from, edge.to) };
            add(from, to);
            // 그룹 닻으로 드나드는 간선은 층 계산에서 그룹 구성원 전체와 잇는 것으로 본다:
            // 그룹으로 들어오는 화살표는 상자 위에서, 나가는 화살표는 상자 아래에서 나온다.
            if self.graph.nodes[to].shape == Shape::Anchor {
                for member in self.group_members(to) {
                    add(from, member);
                }
            }
            if self.graph.nodes[from].shape == Shape::Anchor {
                for member in self.group_members(from) {
                    add(member, to);
                }
            }
        }
        let mut queue: Vec<usize> = (0..n).filter(|&i| incoming_count[i] == 0).collect();
        let mut layer = self.min_layer.clone();
        let mut head = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            for &v in &directed[u] {
                layer[v] = layer[v].max(layer[u] + 1);
                incoming_count[v] -= 1;
                if incoming_count[v] == 0 {
                    queue.push(v);
                }
            }
        }
        for (node, &assigned) in self.lnodes.iter_mut().zip(&layer) {
            node.layer = assigned;
        }
        // 접기·닻 제약으로 맨 위 층이 비면 전체를 끌어올린다.
        let lowest = self.lnodes.iter().take(n).map(|node| node.layer).min().unwrap_or(0);
        for node in self.lnodes.iter_mut().take(n) {
            node.layer -= lowest;
        }
        // 닻은 그룹의 첫 층(나가는 간선이 있으면 마지막 층)에 붙인다.
        for anchor in 0..n {
            if self.graph.nodes[anchor].shape != Shape::Anchor {
                continue;
            }
            let members = self.group_members(anchor);
            let has_outgoing = self.graph.edges.iter().any(|e| e.from == anchor && e.to != anchor);
            let layers = members.iter().map(|&m| self.lnodes[m].layer);
            let target = if has_outgoing { layers.max() } else { layers.min() };
            if let Some(target) = target {
                self.lnodes[anchor].layer = target;
            }
        }
        self.layer_count = self.lnodes.iter().take(n).map(|node| node.layer + 1).max().unwrap_or(0);
    }

    /// 닻이 속한 그룹(하위 그룹 포함)의 실제 구성원.
    fn group_members(&self, anchor: usize) -> Vec<usize> {
        let Some(group) = self.graph.nodes[anchor].group else { return Vec::new() };
        (0..self.graph.nodes.len())
            .filter(|&i| i != anchor && self.graph.nodes[i].shape != Shape::Anchor && self.graph.ancestors(self.graph.nodes[i].group).contains(&group))
            .collect()
    }

    fn mark_back_edges(&mut self, start: usize, outgoing: &[Vec<(usize, usize)>], state: &mut [u8]) {
        // 명시적 스택으로 DFS: (노드, 다음에 볼 간선 번호)
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = 1;
        while let Some(&mut (u, ref mut next)) = stack.last_mut() {
            if *next < outgoing[u].len() {
                let (v, edge) = outgoing[u][*next];
                *next += 1;
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => self.reversed[edge] = true,
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }

    // ── 2. 가상 노드와 구간 ─────────────────────────────────────────────

    fn make_segments(&mut self) {
        let group_ranges = self.group_layer_ranges();
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue;
            }
            let (from, to) = if self.reversed[i] { (edge.to, edge.from) } else { (edge.from, edge.to) };
            let (layer_from, layer_to) = (self.lnodes[from].layer, self.lnodes[to].layer);
            let mut chain = vec![from];
            for layer in layer_from + 1..layer_to {
                let group = self.dummy_group(from, to, layer, &group_ranges);
                self.lnodes.push(LayoutNode {
                    node: None,
                    edge: Some(i),
                    layer,
                    group,
                    along_size: 0,
                    across_size: 1,
                    extra_across: 0,
                    across: 0,
                    sections: Vec::new(),
                });
                chain.push(self.lnodes.len() - 1);
            }
            chain.push(to);
            let last = chain.len() - 2;
            for (k, pair) in chain.windows(2).enumerate() {
                self.segments.push(Segment {
                    edge: i,
                    from: pair[0],
                    to: pair[1],
                    is_first: k == 0,
                    is_last: k == last,
                    exit: 0,
                    entry: 0,
                    exit_offset: 0,
                    entry_offset: 0,
                    exit_rank: (0, 1),
                    entry_rank: (0, 1),
                    channel: None,
                    label_x: None,
                });
            }
        }
        self.adjacency = vec![Vec::new(); self.lnodes.len()];
        for segment in &self.segments {
            self.adjacency[segment.from].push(segment.to);
            self.adjacency[segment.to].push(segment.from);
        }
    }

    /// 그룹마다 (구성원이 있는) 층 범위.
    fn group_layer_ranges(&self) -> Vec<(usize, usize)> {
        let mut ranges = vec![(usize::MAX, 0); self.graph.groups.len()];
        for (i, node) in self.graph.nodes.iter().enumerate() {
            for g in self.graph.ancestors(node.group) {
                ranges[g].0 = ranges[g].0.min(self.lnodes[i].layer);
                ranges[g].1 = ranges[g].1.max(self.lnodes[i].layer);
            }
        }
        ranges
    }

    /// 가상 노드가 들어갈 그룹: 그 층에 걸쳐 있는 출발 쪽(없으면 도착 쪽) 그룹 가운데 가장 안쪽.
    /// 선이 그룹 상자를 아래(또는 위)로 곧게 빠져나가게 한다.
    fn dummy_group(&self, from: usize, to: usize, layer: usize, ranges: &[(usize, usize)]) -> Option<usize> {
        let contains = |g: &usize| ranges[*g].0 <= layer && layer <= ranges[*g].1;
        self.graph
            .ancestors(self.graph.nodes[from].group)
            .into_iter()
            .find(contains)
            .or_else(|| self.graph.ancestors(self.graph.nodes[to].group).into_iter().find(contains))
    }

    // ── 3. 블록(그룹) 트리 ─────────────────────────────────────────────

    fn build_blocks(&mut self) {
        self.blocks.push(Block { group: None, children: Vec::new(), start: 0, width: 0, layer_min: 0, layer_max: 0, registered: true });
        for (g, _) in self.graph.groups.iter().enumerate() {
            self.blocks.push(Block { group: Some(g), children: Vec::new(), start: 0, width: 0, layer_min: usize::MAX, layer_max: 0, registered: false });
        }
        self.lnode_block = self.lnodes.iter().map(|n| n.group.map_or(0, |g| g + 1)).collect();
        self.block_extra = vec![0; self.blocks.len()];
        for i in 0..self.lnodes.len() {
            let block = self.lnode_block[i];
            self.register_block(block);
            self.blocks[block].children.push(Child::Node(i));
            let layer = self.lnodes[i].layer;
            let mut cursor = Some(block);
            while let Some(b) = cursor {
                self.blocks[b].layer_min = self.blocks[b].layer_min.min(layer);
                self.blocks[b].layer_max = self.blocks[b].layer_max.max(layer);
                cursor = self.block_parent(b);
            }
        }
    }

    fn register_block(&mut self, block: usize) {
        if self.blocks[block].registered {
            return;
        }
        self.blocks[block].registered = true;
        let parent = self.block_parent(block).unwrap_or(0);
        self.register_block(parent);
        self.blocks[parent].children.push(Child::Block(block));
    }

    fn block_parent(&self, block: usize) -> Option<usize> {
        if block == 0 {
            return None;
        }
        Some(self.graph.groups[block - 1].parent.map_or(0, |p| p + 1))
    }

    fn block_pad(&self, block: usize) -> usize {
        if block == 0 { 0 } else { 2 }
    }

    // ── 4. 배치 ────────────────────────────────────────────────────────

    fn place(&mut self) {
        self.place_block(0);
        self.blocks[0].start = 0;
        self.assign_absolute(0, 0);
    }

    /// 자식들을 순서대로, 층이 겹치는 앞 자식의 오른쪽에 놓는다. 블록 폭을 돌려준다.
    fn place_block(&mut self, block: usize) -> usize {
        let pad = self.block_pad(block);
        let children = self.blocks[block].children.clone();
        let mut placed: Vec<(usize, usize, usize, usize, Child)> = Vec::new();
        let mut max_end = 0;
        for child in children {
            let (child_width, layer_min, layer_max) = match child {
                Child::Node(i) => (self.lnodes[i].footprint(), self.lnodes[i].layer, self.lnodes[i].layer),
                Child::Block(b) => {
                    let w = self.place_block(b);
                    (w, self.blocks[b].layer_min, self.blocks[b].layer_max)
                }
            };
            let start = placed
                .iter()
                .filter(|&&(_, _, lo, hi, _)| lo <= layer_max && layer_min <= hi)
                .map(|&(_, end, _, _, earlier)| end + self.gap_between(earlier, child))
                .max()
                .unwrap_or(0);
            let end = start + child_width;
            placed.push((start, end, layer_min, layer_max, child));
            match child {
                Child::Node(i) => self.lnodes[i].across = start + pad,
                Child::Block(b) => self.blocks[b].start = start + pad,
            }
            max_end = max_end.max(end);
        }
        // 제목은 가로로 쓰므로 위→아래 배치에서만 가로(across) 폭을 차지한다.
        let title_min = match (self.direction, self.blocks[block].group) {
            (Direction::TopDown, Some(g)) => width_of(&self.graph.groups[g].title) + 4,
            _ => 0,
        };
        let width = (max_end + 2 * pad).max(title_min) + self.block_extra.get(block).copied().unwrap_or(0);
        self.blocks[block].width = width;
        width
    }

    fn assign_absolute(&mut self, block: usize, base: usize) {
        self.blocks[block].start += base;
        let origin = self.blocks[block].start;
        for child in self.blocks[block].children.clone() {
            match child {
                Child::Node(i) => self.lnodes[i].across += origin,
                Child::Block(b) => self.assign_absolute(b, origin),
            }
        }
    }

    fn child_members(&self, child: Child) -> Vec<usize> {
        match child {
            Child::Node(i) => vec![i],
            Child::Block(b) => (0..self.lnodes.len())
                .filter(|&i| {
                    let mut cursor = Some(self.lnode_block[i]);
                    while let Some(x) = cursor {
                        if x == b {
                            return true;
                        }
                        cursor = self.block_parent(x);
                    }
                    false
                })
                .collect(),
        }
    }

    fn child_center(&self, child: Child) -> f64 {
        match child {
            Child::Node(i) => self.lnodes[i].center(),
            Child::Block(b) => self.blocks[b].start as f64 + self.blocks[b].width as f64 / 2.0,
        }
    }

    /// 블록마다 자식들을 이웃의 무게중심 순서로 다시 늘어놓는다.
    fn reorder(&mut self) {
        for block in 0..self.blocks.len() {
            let children = self.blocks[block].children.clone();
            if children.len() < 2 {
                continue;
            }
            let mut keyed: Vec<(f64, Child)> = children
                .into_iter()
                .map(|child| {
                    let members = self.child_members(child);
                    let mut sum = 0.0;
                    let mut count = 0;
                    for &m in &members {
                        for &neighbor in &self.adjacency[m] {
                            if !members.contains(&neighbor) {
                                sum += self.lnodes[neighbor].center();
                                count += 1;
                            }
                        }
                    }
                    let key = if count > 0 { sum / count as f64 } else { self.child_center(child) };
                    (key, child)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            if self.debug {
                let describe = |c: &Child| match c {
                    Child::Node(i) => self.lnodes[*i].node.map_or(format!("dummy{i}(L{})", self.lnodes[*i].layer), |n| format!("{}(L{})", self.graph.nodes[n].id, self.lnodes[*i].layer)),
                    Child::Block(b) => format!("block{b}"),
                };
                eprintln!("reorder block {block}: {}", keyed.iter().map(|(k, c)| format!("{}={k:.0}", describe(c))).collect::<Vec<_>>().join(" "));
            }
            self.blocks[block].children = keyed.into_iter().map(|(_, c)| c).collect();
        }
    }

    /// 무게중심 정렬은 자리가 매번 바뀌어 흔들릴 수 있다. 최종 자리를 기준으로
    /// 자식(가상 노드는 같은 간선의 사슬을 한 묶음으로)을 같은 층을 공유하는 앞 자식 앞으로
    /// 옮겨 보고, 간선 길이 합이 줄면 받아들인다.
    fn improve_by_swaps(&mut self) {
        let mut current = self.total_edge_length();
        for _ in 0..3 {
            let mut improved = false;
            for block in 0..self.blocks.len() {
                let mut k = 1;
                while k < self.blocks[block].children.len() {
                    let unit = self.unit_of(block, k);
                    let first = unit[0];
                    let leader = self.blocks[block].children[first];
                    let candidates: Vec<usize> = (0..first)
                        .rev()
                        .filter(|&j| {
                            let other = self.blocks[block].children[j];
                            unit.iter().any(|&u| self.children_share_layer(other, self.blocks[block].children[u]))
                        })
                        .take(4)
                        .collect();
                    let _ = leader;
                    let mut accepted = false;
                    for j in candidates {
                        let before = self.blocks[block].children.clone();
                        let moving: Vec<Child> = unit.iter().map(|&u| self.blocks[block].children[u]).collect();
                        for &u in unit.iter().rev() {
                            self.blocks[block].children.remove(u);
                        }
                        for (offset, child) in moving.into_iter().enumerate() {
                            self.blocks[block].children.insert(j + offset, child);
                        }
                        self.place();
                        let candidate = self.total_edge_length();
                        if candidate < current {
                            current = candidate;
                            improved = true;
                            accepted = true;
                            break;
                        }
                        self.blocks[block].children = before;
                    }
                    if !accepted {
                        k = unit.last().copied().unwrap_or(k) + 1;
                    } else {
                        k += 1;
                    }
                }
            }
            self.place();
            if !improved {
                break;
            }
        }
    }

    /// 자식 `k`가 가상 노드면 같은 간선의 가상 노드 자리들(오름차순), 아니면 `[k]`.
    fn unit_of(&self, block: usize, k: usize) -> Vec<usize> {
        let children = &self.blocks[block].children;
        let edge = match children[k] {
            Child::Node(i) => self.lnodes[i].edge,
            Child::Block(_) => None,
        };
        let Some(edge) = edge else { return vec![k] };
        (0..children.len())
            .filter(|&j| matches!(children[j], Child::Node(i) if self.lnodes[i].edge == Some(edge)))
            .collect()
    }

    fn children_share_layer(&self, a: Child, b: Child) -> bool {
        let range = |child: Child| match child {
            Child::Node(i) => (self.lnodes[i].layer, self.lnodes[i].layer),
            Child::Block(b) => (self.blocks[b].layer_min, self.blocks[b].layer_max),
        };
        let (a_min, a_max) = range(a);
        let (b_min, b_max) = range(b);
        a_min <= b_max && b_min <= a_max
    }

    /// 모든 구간의 가로 이동량 합(중심 기준).
    fn total_edge_length(&self) -> f64 {
        self.segments.iter().map(|s| (self.lnodes[s.from].center() - self.lnodes[s.to].center()).abs()).sum()
    }

    // ── 5. 직선화 ──────────────────────────────────────────────────────

    fn straighten(&mut self) {
        for _ in 0..2 {
            for layer in 1..self.layer_count {
                self.straighten_layer(layer, true);
            }
            for layer in (0..self.layer_count.saturating_sub(1)).rev() {
                self.straighten_layer(layer, false);
            }
        }
    }

    fn straighten_layer(&mut self, layer: usize, use_upper: bool) {
        let mut members: Vec<usize> = (0..self.lnodes.len()).filter(|&i| self.lnodes[i].layer == layer).collect();
        members.sort_by_key(|&i| self.lnodes[i].across);
        for (k, &i) in members.iter().enumerate() {
            let has_upper = self.adjacency[i].iter().any(|&n| self.lnodes[n].layer + 1 == layer);
            if !use_upper && has_upper {
                continue;
            }
            // 접점끼리 맞아떨어지는 시작 위치들의 중앙값을 목표로 한다.
            let mut targets: Vec<i64> = self
                .segments
                .iter()
                .filter_map(|segment| {
                    if use_upper && segment.to == i && self.lnodes[segment.from].layer + 1 == layer {
                        Some(self.lnodes[segment.from].across as i64 + segment.exit_offset as i64 - segment.entry_offset as i64)
                    } else if !use_upper && segment.from == i && self.lnodes[segment.to].layer == layer + 1 {
                        Some(self.lnodes[segment.to].across as i64 + segment.entry_offset as i64 - segment.exit_offset as i64)
                    } else {
                        None
                    }
                })
                .collect();
            if targets.is_empty() {
                continue;
            }
            targets.sort_unstable();
            let footprint = self.lnodes[i].footprint();
            let desired = targets[targets.len() / 2].max(0) as usize;
            let container = self.lnode_block[i];
            let pad = self.block_pad(container);
            let mut low = self.blocks[container].start + pad;
            let mut high = (self.blocks[container].start + self.blocks[container].width).saturating_sub(pad + footprint);
            if k > 0
                && let Some((_, end)) = self.bound_of(container, members[k - 1])
            {
                let gap = self.gap_between(Child::Node(members[k - 1]), Child::Node(i));
                low = low.max(end + gap);
            }
            if k + 1 < members.len()
                && let Some((start, _)) = self.bound_of(container, members[k + 1])
            {
                let gap = self.gap_between(Child::Node(i), Child::Node(members[k + 1]));
                high = high.min(start.saturating_sub(gap + footprint));
            }
            if low > high {
                continue;
            }
            self.lnodes[i].across = desired.clamp(low, high);
        }
    }

    /// `container`의 직계 자식 가운데 `other`를 품은 것의 [시작, 끝) 구간.
    fn bound_of(&self, container: usize, other: usize) -> Option<(usize, usize)> {
        let mut block = self.lnode_block[other];
        if block == container {
            let node = &self.lnodes[other];
            return Some((node.across, node.across + node.footprint()));
        }
        loop {
            let parent = self.block_parent(block)?;
            if parent == container {
                return Some((self.blocks[block].start, self.blocks[block].start + self.blocks[block].width));
            }
            block = parent;
        }
    }

    // ── 6. 배선 ────────────────────────────────────────────────────────

    /// 노드의 안쪽 칸에 `count`개 접점을 둘 때 `index`번째 위치. 가운데를 기준으로 두 칸씩 벌린다.
    fn spread(&self, lnode: usize, index: usize, count: usize) -> usize {
        let node = &self.lnodes[lnode];
        if node.across_size <= 2 {
            return node.across;
        }
        let usable = node.across_size - 2;
        let center = node.across + node.across_size / 2;
        if count <= 1 {
            return center;
        }
        if 2 * (count - 1) < usable {
            return center + 2 * index - (count - 1);
        }
        (node.across + 1 + ((index + 1) * usable) / (count + 1)).min(node.across + node.across_size - 2)
    }

    /// 노드마다 나가는/들어오는 접점을 이웃 위치 순서로 배정한다(노드 기준 상대 위치).
    fn assign_ports(&mut self) {
        let count = self.lnodes.len();
        for i in 0..count {
            let base = self.lnodes[i].across;
            let mut outgoing: Vec<usize> = (0..self.segments.len()).filter(|&s| self.segments[s].from == i).collect();
            outgoing.sort_by(|&a, &b| {
                let ca = self.lnodes[self.segments[a].to].center();
                let cb = self.lnodes[self.segments[b].to].center();
                ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let total = outgoing.len();
            for (k, s) in outgoing.into_iter().enumerate() {
                self.segments[s].exit_offset = self.spread(i, k, total) - base;
                self.segments[s].exit_rank = (k, total);
            }
            let mut incoming: Vec<usize> = (0..self.segments.len()).filter(|&s| self.segments[s].to == i).collect();
            incoming.sort_by(|&a, &b| {
                let ca = self.lnodes[self.segments[a].from].center();
                let cb = self.lnodes[self.segments[b].from].center();
                ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let total = incoming.len();
            for (k, s) in incoming.into_iter().enumerate() {
                self.segments[s].entry_offset = self.spread(i, k, total) - base;
                self.segments[s].entry_rank = (k, total);
            }
        }
    }

    fn route(&mut self) {
        self.refresh_ports();
        self.resolve_port_swaps();

        self.layer_content = vec![0; self.layer_count];
        self.top_levels = vec![0; self.layer_count];
        self.bottom_levels = vec![0; self.layer_count];
        for i in 0..self.lnodes.len() {
            let node = &self.lnodes[i];
            let layer = node.layer;
            self.layer_content[layer] = self.layer_content[layer].max(node.along_size);
            let mut top = 0;
            let mut bottom = 0;
            let mut cursor = Some(self.lnode_block[i]);
            while let Some(b) = cursor {
                if b != 0 {
                    top += usize::from(self.blocks[b].layer_min == layer);
                    bottom += usize::from(self.blocks[b].layer_max == layer);
                }
                cursor = self.block_parent(b);
            }
            self.top_levels[layer] = self.top_levels[layer].max(top);
            self.bottom_levels[layer] = self.bottom_levels[layer].max(bottom);
        }

        let gap_count = self.layer_count.saturating_sub(1);
        self.gap = vec![GAP_ALONG_MIN; gap_count];
        self.gap_label_room = vec![0; gap_count];
        self.gap_head_room = vec![0; gap_count];
        for layer in 0..gap_count {
            let mut intervals: Vec<(usize, usize, usize)> = Vec::new();
            let mut label_room = 0;
            let mut head_room = 0;
            for s in 0..self.segments.len() {
                let segment = &self.segments[s];
                if self.lnodes[segment.from].layer != layer {
                    continue;
                }
                let (label, tail_label, head_label) = self.segment_labels(s);
                let label_width = width_of(&label);
                let bent = segment.exit != segment.entry;
                match self.direction {
                    Direction::TopDown => {
                        if !bent && label.is_empty() {
                            continue;
                        }
                        let (mut low, mut high) = (segment.exit.min(segment.entry), segment.exit.max(segment.entry));
                        if label_width > 0 {
                            let label_x = self.choose_label_x(s, layer, label_width);
                            low = low.min(label_x);
                            high = high.max(label_x + label_width - 1);
                            self.segments[s].label_x = Some(label_x);
                        }
                        self.extra_across = self.extra_across.max(high + 1);
                        intervals.push((low, high, s));
                    }
                    Direction::LeftRight => {
                        label_room = label_room.max(label_width.max(width_of(&tail_label)));
                        head_room = head_room.max(width_of(&head_label));
                        if bent {
                            intervals.push((segment.exit.min(segment.entry), segment.exit.max(segment.entry), s));
                        }
                    }
                }
            }
            let channels = self.assign_channels(intervals);
            let room = |w: usize| if w > 0 { w + 1 } else { 0 };
            self.gap_label_room[layer] = room(label_room);
            self.gap_head_room[layer] = room(head_room);
            self.gap[layer] = GAP_ALONG_MIN.max(2 + self.gap_label_room[layer] + channels + self.gap_head_room[layer]);
        }

        self.layer_start = vec![0; self.layer_count];
        for layer in 1..self.layer_count {
            self.layer_start[layer] = self.layer_start[layer - 1] + self.layer_total(layer - 1) + self.gap[layer - 1];
        }
    }

    fn refresh_ports(&mut self) {
        for s in 0..self.segments.len() {
            let (from, to) = (self.segments[s].from, self.segments[s].to);
            self.segments[s].exit = self.lnodes[from].across + self.segments[s].exit_offset;
            self.segments[s].entry = self.lnodes[to].across + self.segments[s].entry_offset;
        }
    }

    /// 같은 통로에서 두 간선이 서로의 열을 맞바꾸는 X자 교차(i.exit == j.entry, j.exit == i.entry)는
    /// 줄 순서로는 풀 수 없다. 실제 노드에 붙은 접점을 한 칸 옮겨 열을 어긋나게 한다.
    fn resolve_port_swaps(&mut self) {
        for _ in 0..8 {
            let mut nudged = false;
            for i in 0..self.segments.len() {
                for j in 0..self.segments.len() {
                    if i == j || self.lnodes[self.segments[i].from].layer != self.lnodes[self.segments[j].from].layer {
                        continue;
                    }
                    let (a, b) = (&self.segments[i], &self.segments[j]);
                    if a.exit != b.entry || b.exit != a.entry || a.exit == a.entry {
                        continue;
                    }
                    let candidates = [(j, false), (i, true), (i, false), (j, true)];
                    let Some(&(s, is_exit)) = candidates.iter().find(|&&(s, is_exit)| {
                        let node = if is_exit { self.segments[s].from } else { self.segments[s].to };
                        self.lnodes[node].node.is_some() && self.lnodes[node].across_size >= 4
                    }) else {
                        continue;
                    };
                    let node = if is_exit { self.segments[s].from } else { self.segments[s].to };
                    let limit = self.lnodes[node].across_size - 2;
                    let offset = if is_exit { &mut self.segments[s].exit_offset } else { &mut self.segments[s].entry_offset };
                    *offset = if *offset < limit { *offset + 1 } else { offset.saturating_sub(1).max(1) };
                    nudged = true;
                }
            }
            if !nudged {
                return;
            }
            self.refresh_ports();
        }
    }

    /// 통로 줄 배정. 구간이 겹치지 않는 간선은 한 줄을 나눠 쓴다.
    ///
    /// 한 간선의 출발 접점과 다른 간선의 도착 접점이 같은 열이면 두 간선의 세로선이 그 열을
    /// 나눠 쓰게 되므로, 출발 쪽 간선의 통로가 반드시 위에 오도록 순서를 강제한다
    /// (출발 쪽은 통로까지 내려오고 도착 쪽은 통로부터 내려가니 줄이 다르면 겹치지 않는다).
    /// 돌려주는 값은 쓴 줄 수.
    fn assign_channels(&mut self, mut intervals: Vec<(usize, usize, usize)>) -> usize {
        intervals.sort();
        let count = intervals.len();
        // must_precede[i]에 j가 있으면 i의 통로가 j보다 위여야 한다.
        let mut must_precede: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut pending: Vec<usize> = vec![0; count];
        for i in 0..count {
            for j in 0..count {
                if i != j && self.segments[intervals[i].2].exit == self.segments[intervals[j].2].entry {
                    must_precede[i].push(j);
                    pending[j] += 1;
                }
            }
        }
        // 제약을 지키는 순서(위상 정렬). 순환이면 남은 것을 그냥 이어 붙인다.
        let mut order: Vec<usize> = Vec::with_capacity(count);
        let mut ready: Vec<usize> = (0..count).filter(|&i| pending[i] == 0).collect();
        let mut placed = vec![false; count];
        while let Some(i) = ready.first().copied() {
            ready.remove(0);
            order.push(i);
            placed[i] = true;
            for &j in &must_precede[i] {
                pending[j] -= 1;
                if pending[j] == 0 {
                    ready.push(j);
                    ready.sort_unstable();
                }
            }
        }
        order.extend((0..count).filter(|&i| !placed[i]));
        let mut rows: Vec<Vec<(usize, usize)>> = Vec::new();
        let mut assigned: Vec<Option<usize>> = vec![None; count];
        for i in order {
            let (low, high, s) = intervals[i];
            let minimum = (0..count)
                .filter(|&k| must_precede[k].contains(&i))
                .filter_map(|k| assigned[k])
                .map(|row| row + 1)
                .max()
                .unwrap_or(0);
            let fits = |occupied: &Vec<(usize, usize)>| occupied.iter().all(|&(lo, hi)| hi + 2 <= low || high + 2 <= lo);
            let row = (minimum..rows.len()).find(|&r| fits(&rows[r])).unwrap_or_else(|| {
                rows.resize_with(minimum.max(rows.len()) + 1, Vec::new);
                rows.len() - 1
            });
            rows[row].push((low, high));
            assigned[i] = Some(row);
            self.segments[s].channel = Some(row);
        }
        rows.len()
    }

    /// TB 라벨 자리: 가로 구간이 넉넉하면 그 가운데, 아니면 다른 간선의 세로줄과 안 붙는 쪽.
    fn choose_label_x(&self, s: usize, layer: usize, label_width: usize) -> usize {
        let segment = &self.segments[s];
        let (low, high) = (segment.exit.min(segment.entry), segment.exit.max(segment.entry));
        let span = high - low;
        if span >= label_width + 2 {
            return low + (span - label_width) / 2 + 1;
        }
        let stubs: Vec<usize> = self
            .segments
            .iter()
            .enumerate()
            .filter(|(other, o)| *other != s && self.lnodes[o.from].layer == layer)
            .flat_map(|(_, o)| [o.exit, o.entry])
            .collect();
        let is_free = |x: usize| stubs.iter().all(|&stub| stub + 1 < x || stub > x + label_width);
        let right = high + 2;
        let left = if low >= label_width + 2 { Some(low - label_width - 2) } else { None };
        [Some(right), left, Some(right + 1), Some(right + 2), Some(right + 3)]
            .into_iter()
            .flatten()
            .find(|&x| is_free(x))
            .unwrap_or(right)
    }

    /// 라벨이 그룹 상자 밖으로 나가면 그 블록의 폭을 늘려 달라고 표시한다. 늘린 게 있으면 true.
    fn reserve_label_room(&mut self) -> bool {
        if self.direction != Direction::TopDown {
            return false;
        }
        let mut changed = false;
        for s in 0..self.segments.len() {
            let segment = &self.segments[s];
            let Some(label_x) = segment.label_x else { continue };
            let (label, _, _) = self.segment_labels(s);
            let end = label_x + width_of(&label);
            let block = self.routing_block(segment.from, segment.to);
            let pad = self.block_pad(block);
            let limit = self.blocks[block].start + self.blocks[block].width - pad;
            if end > limit {
                self.block_extra[block] = self.block_extra[block].max(end - limit);
                changed = true;
            }
        }
        changed
    }

    /// 두 노드를 모두 품는 가장 안쪽 블록.
    fn routing_block(&self, a: usize, b: usize) -> usize {
        let mut chain_a = Vec::new();
        let mut cursor = Some(self.lnode_block[a]);
        while let Some(x) = cursor {
            chain_a.push(x);
            cursor = self.block_parent(x);
        }
        let mut cursor = Some(self.lnode_block[b]);
        while let Some(x) = cursor {
            if chain_a.contains(&x) {
                return x;
            }
            cursor = self.block_parent(x);
        }
        0
    }

    fn layer_total(&self, layer: usize) -> usize {
        2 * self.top_levels[layer] + self.layer_content[layer] + 2 * self.bottom_levels[layer]
    }

    fn total_along(&self) -> usize {
        if self.layer_count == 0 {
            return 0;
        }
        let last = self.layer_count - 1;
        self.layer_start[last] + self.layer_total(last)
    }

    fn node_along(&self, lnode: usize) -> usize {
        let layer = self.lnodes[lnode].layer;
        self.layer_start[layer] + 2 * self.top_levels[layer]
    }

    /// (간선 라벨, 꼬리 라벨, 머리 라벨) — 이 구간이 맡은 것만.
    fn segment_labels(&self, s: usize) -> (String, String, String) {
        let segment = &self.segments[s];
        let edge = &self.graph.edges[segment.edge];
        let reversed = self.reversed[segment.edge];
        let label = if segment.is_first && self.shows_label(segment.edge) { edge.label.replace('\n', " ") } else { String::new() };
        let (top_label, bottom_label) = if reversed { (&edge.head_label, &edge.tail_label) } else { (&edge.tail_label, &edge.head_label) };
        let tail = if segment.is_first { top_label.clone() } else { String::new() };
        let head = if segment.is_last { bottom_label.clone() } else { String::new() };
        (label, tail, head)
    }

    /// 한 노드에서 같은 라벨의 간선이 셋 이상 나가면 첫 간선에만 라벨을 쓴다.
    fn shows_label(&self, edge_index: usize) -> bool {
        let edge = &self.graph.edges[edge_index];
        if edge.label.is_empty() {
            return false;
        }
        let mut same = self.graph.edges.iter().enumerate().filter(|(_, e)| e.from == edge.from && e.label == edge.label);
        let first = same.next().map(|(i, _)| i);
        let count = 1 + same.count();
        count < 3 || first == Some(edge_index)
    }

    // ── 7. 그리기 ──────────────────────────────────────────────────────

    fn to_canvas(&self, along: usize, across: usize) -> (usize, usize) {
        match self.direction {
            Direction::TopDown => (across, along),
            Direction::LeftRight => (along, across),
        }
    }

    fn line_along(&self, canvas: &mut Canvas, a0: usize, a1: usize, across: usize, kind: LineKind, style: Style) {
        match self.direction {
            Direction::TopDown => canvas.vline(across, a0, a1, kind, style),
            Direction::LeftRight => canvas.hline(a0, a1, across, kind, style),
        }
    }

    fn line_across(&self, canvas: &mut Canvas, c0: usize, c1: usize, along: usize, kind: LineKind, style: Style) {
        match self.direction {
            Direction::TopDown => canvas.hline(c0, c1, along, kind, style),
            Direction::LeftRight => canvas.vline(along, c0, c1, kind, style),
        }
    }

    fn draw(&self, canvas: &mut Canvas) {
        if self.debug {
            for (i, node) in self.lnodes.iter().enumerate() {
                if let Some(n) = node.node {
                    eprintln!("node {} layer {} across {} group {:?} min_layer {}", self.graph.nodes[n].id, node.layer, node.across, node.group, self.min_layer[i]);
                }
            }
        }
        self.draw_groups(canvas);
        for (i, node) in self.lnodes.iter().enumerate() {
            let along = self.node_along(i);
            canvas.set_edge_mode(node.node.is_none());
            match node.node {
                Some(n) => {
                    let (x, y) = self.to_canvas(along, node.across);
                    let min_height = match self.direction {
                        Direction::TopDown => 0,
                        Direction::LeftRight => node.across_size,
                    };
                    shape::draw(canvas, x, y, self.graph.nodes[n].shape, &node.sections, self.theme, min_height);
                }
                None => {
                    // 층 양옆 통로까지 한 칸씩 물려 그려야 그룹 테두리와 만나는 칸이 `┼`가 된다.
                    let layer = node.layer;
                    let start = self.layer_start[layer].saturating_sub(1);
                    let end = self.layer_start[layer] + self.layer_total(layer);
                    let kind = self.dummy_kind(i);
                    self.line_along(canvas, start, end, node.across, kind, self.theme.diagram_line);
                }
            }
        }
        canvas.set_edge_mode(true);
        for s in 0..self.segments.len() {
            self.draw_segment(canvas, s);
        }
        for s in 0..self.segments.len() {
            self.draw_segment_decorations(canvas, s);
        }
        self.draw_self_loops(canvas);
        canvas.set_edge_mode(false);
        self.draw_group_titles(canvas);
    }

    fn dummy_kind(&self, lnode: usize) -> LineKind {
        self.segments
            .iter()
            .find(|s| s.from == lnode || s.to == lnode)
            .map_or(LineKind::Solid, |s| self.graph.edges[s.edge].kind)
    }

    fn draw_groups(&self, canvas: &mut Canvas) {
        let mut order: Vec<usize> = (1..self.blocks.len()).filter(|&b| self.blocks[b].registered).collect();
        order.sort_by_key(|&b| self.block_depth(b));
        for b in order {
            let block = &self.blocks[b];
            let top = self.group_top(b);
            let bottom = self.group_bottom(b);
            let (x0, y0) = self.to_canvas(top, block.start);
            let (x1, y1) = self.to_canvas(bottom, block.start + block.width - 1);
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            canvas.rect(x0, y0, w, h, LineKind::Solid, self.theme.diagram_group, false);
        }
    }

    /// 그룹 제목은 선에 잘리지 않도록 맨 나중에 쓴다.
    fn draw_group_titles(&self, canvas: &mut Canvas) {
        for b in 1..self.blocks.len() {
            let block = &self.blocks[b];
            let Some(g) = block.group else { continue };
            let title = &self.graph.groups[g].title;
            if !block.registered || title.is_empty() {
                continue;
            }
            let top = self.group_top(b);
            let (x0, y0) = self.to_canvas(top, block.start);
            let w = match self.direction {
                Direction::TopDown => block.width,
                Direction::LeftRight => self.group_bottom(b) - top + 1,
            };
            let text = format!(" {} ", truncate(title, w.saturating_sub(4)));
            canvas.text(x0 + 1, y0, &text, self.theme.diagram_group.bold());
        }
    }

    fn group_bottom(&self, b: usize) -> usize {
        let block = &self.blocks[b];
        self.layer_start[block.layer_max] + 2 * self.top_levels[block.layer_max] + self.layer_content[block.layer_max] + 2 * self.inner_rank(b) + 1
    }

    fn group_top(&self, b: usize) -> usize {
        let block = &self.blocks[b];
        let mut rank = 0;
        let mut cursor = self.block_parent(b);
        while let Some(p) = cursor {
            if p != 0 && self.blocks[p].layer_min == block.layer_min {
                rank += 1;
            }
            cursor = self.block_parent(p);
        }
        self.layer_start[block.layer_min] + 2 * rank
    }

    fn block_depth(&self, block: usize) -> usize {
        let mut depth = 0;
        let mut cursor = self.block_parent(block);
        while let Some(p) = cursor {
            depth += 1;
            cursor = self.block_parent(p);
        }
        depth
    }

    /// 같은 층에서 끝나는 하위 블록 사슬의 길이.
    fn inner_rank(&self, block: usize) -> usize {
        let layer_max = self.blocks[block].layer_max;
        self.blocks[block]
            .children
            .iter()
            .filter_map(|child| match child {
                Child::Block(c) if self.blocks[*c].layer_max == layer_max => Some(1 + self.inner_rank(*c)),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// 구간의 위쪽 끝(출발 상자 바로 다음 칸)과 아래쪽 끝(도착 상자 바로 앞 칸).
    fn segment_span(&self, s: usize) -> (usize, usize, usize) {
        let segment = &self.segments[s];
        let layer = self.lnodes[segment.from].layer;
        let gap_start = self.layer_start[layer] + self.layer_total(layer);
        let top = match self.lnodes[segment.from].node {
            Some(_) => self.node_along(segment.from) + self.lnodes[segment.from].along_size,
            None => gap_start,
        };
        let bottom = match self.lnodes[segment.to].node {
            Some(_) => self.node_along(segment.to) - 1,
            None => self.layer_start[layer + 1] - 1,
        };
        (top, bottom, gap_start)
    }

    fn channel_position(&self, s: usize, gap_start: usize) -> Option<usize> {
        let segment = &self.segments[s];
        let layer = self.lnodes[segment.from].layer;
        segment.channel.map(|c| gap_start + 1 + self.gap_label_room[layer] + c)
    }

    fn draw_segment(&self, canvas: &mut Canvas, s: usize) {
        let segment = &self.segments[s];
        if self.debug {
            // 배선 디버깅: DG_DEBUG=1 이면 구간마다 접점·통로를 stderr에 적는다.
            let name = |l: usize| self.lnodes[l].node.map_or(format!("dummy{l}@{}", self.lnodes[l].across), |n| self.graph.nodes[n].id.clone());
            let (top, bottom, gap_start) = self.segment_span(s);
            eprintln!(
                "seg {s} edge {} {} -> {} exit {} entry {} channel {:?} rows {top}..{bottom} gap {gap_start} layer {}",
                segment.edge, name(segment.from), name(segment.to), segment.exit, segment.entry, segment.channel, self.lnodes[segment.from].layer
            );
        }
        let kind = self.graph.edges[segment.edge].kind;
        let style = self.theme.diagram_line;
        let (top, bottom, gap_start) = self.segment_span(s);
        if segment.exit == segment.entry {
            self.line_along(canvas, top, bottom, segment.exit, kind, style);
            return;
        }
        let Some(channel) = self.channel_position(s, gap_start) else { return };
        self.line_along(canvas, top, channel, segment.exit, kind, style);
        self.line_across(canvas, segment.exit, segment.entry, channel, kind, style);
        self.line_along(canvas, channel, bottom, segment.entry, kind, style);
        for across in [segment.exit, segment.entry] {
            let (x, y) = self.to_canvas(channel, across);
            canvas.join(x, y, 0, kind, style, true);
        }
    }

    fn draw_segment_decorations(&self, canvas: &mut Canvas, s: usize) {
        let segment = &self.segments[s];
        let edge = &self.graph.edges[segment.edge];
        let reversed = self.reversed[segment.edge];
        let (top, bottom, gap_start) = self.segment_span(s);
        let (label, tail_label, head_label) = self.segment_labels(s);
        let (top_marker, bottom_marker) = if reversed { (edge.head, edge.tail) } else { (edge.tail, edge.head) };
        let marker_style = self.theme.diagram_line;
        if segment.is_first
            && let Some(glyph) = marker_glyph(top_marker, self.direction, true)
        {
            let (x, y) = self.to_canvas(top, segment.exit);
            canvas.put(x, y, glyph, marker_style);
        }
        if segment.is_last
            && let Some(glyph) = marker_glyph(bottom_marker, self.direction, false)
        {
            let (x, y) = self.to_canvas(bottom, segment.entry);
            canvas.put(x, y, glyph, marker_style);
        }
        let label_style = self.theme.diagram_label;
        match self.direction {
            Direction::TopDown => {
                if !label.is_empty()
                    && let Some(channel) = self.channel_position(s, gap_start)
                    && let Some(x) = segment.label_x
                {
                    canvas.text(x, channel, &label, label_style);
                }
                if !tail_label.is_empty() {
                    let x = side_label_x(segment.exit, segment.exit_rank, width_of(&tail_label));
                    canvas.text(x, top, &tail_label, label_style);
                }
                if !head_label.is_empty() {
                    let x = side_label_x(segment.entry, segment.entry_rank, width_of(&head_label));
                    canvas.text(x, bottom, &head_label, label_style);
                }
            }
            Direction::LeftRight => {
                if !label.is_empty() {
                    canvas.text(gap_start + 1, segment.exit, &label, label_style);
                }
                if !tail_label.is_empty() {
                    canvas.text(gap_start + 1, segment.exit.saturating_sub(1), &tail_label, label_style);
                }
                if !head_label.is_empty() {
                    let x = bottom.saturating_sub(width_of(&head_label));
                    canvas.text(x, segment.entry.saturating_sub(1), &head_label, label_style);
                }
            }
        }
    }

    fn draw_self_loops(&self, canvas: &mut Canvas) {
        let style = self.theme.diagram_line;
        for edge in &self.graph.edges {
            if edge.from != edge.to {
                continue;
            }
            let node = &self.lnodes[edge.from];
            let (x, y) = self.to_canvas(self.node_along(edge.from), node.across);
            let (w, h) = match self.direction {
                Direction::TopDown => (node.across_size, node.along_size),
                Direction::LeftRight => (node.along_size, node.across_size),
            };
            let label = edge.label.replace('\n', " ");
            match self.direction {
                Direction::TopDown if h >= 3 => {
                    canvas.join(x + w - 1, y + 1, EAST, edge.kind, style, false);
                    canvas.hline(x + w, x + w + 1, y + 1, edge.kind, style);
                    canvas.vline(x + w + 1, y + 1, y + 2, edge.kind, style);
                    canvas.join(x + w + 1, y + 1, 0, edge.kind, style, true);
                    canvas.join(x + w + 1, y + 2, WEST, edge.kind, style, true);
                    canvas.put(x + w, y + 2, '◀', style);
                    canvas.text(x + w + 3, y + 1, &label, self.theme.diagram_label);
                }
                Direction::LeftRight if w >= 5 => {
                    canvas.join(x + 1, y + h - 1, SOUTH, edge.kind, style, false);
                    canvas.join(x + w - 2, y + h - 1, SOUTH, edge.kind, style, false);
                    canvas.vline(x + 1, y + h, y + h + 1, edge.kind, style);
                    canvas.hline(x + 1, x + w - 2, y + h + 1, edge.kind, style);
                    canvas.join(x + 1, y + h + 1, 0, edge.kind, style, true);
                    canvas.join(x + w - 2, y + h + 1, NORTH, edge.kind, style, true);
                    canvas.put(x + w - 2, y + h, '▲', style);
                    canvas.text(x + w, y + h + 1, &label, self.theme.diagram_label);
                }
                _ => {}
            }
        }
    }
}

/// 끝 라벨을 선의 어느 쪽에 둘지: 여러 간선이 한 노드에 모이면 왼쪽 절반은 선 왼쪽에 둔다.
fn side_label_x(line_x: usize, rank: (usize, usize), label_width: usize) -> usize {
    let (index, count) = rank;
    if count > 1 && index * 2 < count { line_x.saturating_sub(label_width) } else { line_x + 1 }
}

/// 끝 표식 글자. `at_top`이면 위(또는 왼쪽) 노드에 붙는 쪽이다.
fn marker_glyph(marker: Marker, direction: Direction, at_top: bool) -> Option<char> {
    let index = match (direction, at_top) {
        (Direction::TopDown, true) => 0,
        (Direction::TopDown, false) => 1,
        (Direction::LeftRight, true) => 2,
        (Direction::LeftRight, false) => 3,
    };
    let glyphs: [char; 4] = match marker {
        Marker::None => return None,
        Marker::Arrow => ['▲', '▼', '◀', '▶'],
        Marker::OpenArrow => ['∧', '∨', '<', '>'],
        Marker::Triangle => ['△', '▽', '◁', '▷'],
        Marker::DiamondFilled => ['◆'; 4],
        Marker::DiamondOpen => ['◇'; 4],
        Marker::Circle => ['○'; 4],
        Marker::Cross => ['✕'; 4],
    };
    Some(glyphs[index])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::ir::{Edge, Shape};

    fn simple_graph() -> Graph {
        let mut g = Graph::default();
        let a = g.intern("A", "Start", Shape::Round, None);
        let b = g.intern("B", "Middle", Shape::Rect, None);
        let c = g.intern("C", "End", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, head: Marker::Arrow, label: "go".into(), ..Edge::default() });
        g.add_edge(Edge { from: b, to: c, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: c, to: a, head: Marker::Arrow, kind: LineKind::Dashed, ..Edge::default() });
        g
    }

    fn rows(lines: Vec<Line>) -> Vec<String> {
        lines.iter().map(Line::plain).collect()
    }

    #[test]
    fn renders_top_down_chain_with_back_edge() {
        let out = rows(render(&simple_graph(), &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert!(text.contains("Start"), "{text}");
        assert!(text.contains("▼"), "{text}");
        assert!(text.contains("▲"), "{text}");
        assert!(text.contains("go"), "{text}");
    }

    #[test]
    fn renders_left_right() {
        let mut g = simple_graph();
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert!(text.contains("▶"), "{text}");
        assert!(out.len() < 12, "{text}");
    }

    #[test]
    fn groups_enclose_members() {
        let mut g = Graph::default();
        let group = g.add_group("Backend", None);
        let a = g.intern("A", "api", Shape::Rect, Some(group));
        let b = g.intern("B", "db", Shape::Cylinder, Some(group));
        let c = g.intern("C", "client", Shape::Rect, None);
        g.add_edge(Edge { from: c, to: a, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: a, to: b, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert!(text.contains("Backend"), "{text}");
    }

    #[test]
    fn too_wide_returns_none() {
        let mut g = Graph::default();
        for i in 0..6 {
            g.intern(&format!("N{i}"), "wide node label here", Shape::Rect, None);
        }
        assert!(render(&g, &Theme::none(), 8).is_none());
    }
}
