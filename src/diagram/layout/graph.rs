//! 계층 배치(Sugiyama)로 그래프를 그린다.
//!
//! 1. 되돌아가는 간선을 DFS로 뒤집어 DAG를 만들고 최장 경로로 층을 매긴다.
//! 2. 그룹을 블록으로 보고, 블록 안에서 자식들을 무게중심 순서로 겹치지 않게 늘어놓는다.
//!    블록은 여러 층에 걸쳐 같은 띠를 차지하므로 그룹 테두리가 서로 포개지지 않는다.
//! 3. 층 사이 "통로"에 간선의 가로 구간을 넣고, 겹치는 구간은 다른 줄을 쓴다.
//! 4. 방향(TB/LR)은 배치 좌표계(along/across)를 캔버스 좌표로 옮길 때만 관여한다.

use crate::diagram::canvas::{Canvas, EAST, LineKind, NORTH, SOUTH, WEST};
use crate::diagram::ir::{Direction, Graph, GroupKind, Marker, Shape};
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
    /// BT·RL처럼 흐름 방향을 뒤집는지: 켜지면 모든 간선을 `effective_reversed`로 본다(층 계산·배선이
    /// 원천을 마지막 층에, 도착을 첫 층에 두게 되어 그 뒤로는 보통 TB/LR과 똑같이 그려진다).
    reverse_along: bool,
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
    /// 통로 위쪽에서 꼬리 표식이 차지하는 줄 수(최소 1), 아래쪽에서 머리 표식이 차지하는 줄 수.
    gap_tail_rows: Vec<usize>,
    gap_head_rows: Vec<usize>,
    /// 통로 라벨이 블록 폭 밖으로 삐져나갈 때 필요한 가로 폭.
    extra_across: usize,
    /// 라벨이 그룹 밖으로 나가지 않도록 블록마다 더 주는 폭.
    block_extra: Vec<usize>,
    /// `DG_DEBUG`가 켜져 있으면 배선 정보를 stderr에 적는다.
    debug: bool,
    /// 캔버스 가로 한도. 이웃 교환은 이 폭을 넘기는 후보를 받지 않는다.
    width_limit: usize,
}

const GAP_ALONG_MIN: usize = 3;
/// 층 접기 최대 횟수.
const MAX_FOLDS: usize = 40;
/// 이웃 교환에서 앞뒤로 몇 자식까지 옮겨 볼지.
const SWAP_REACH: usize = 8;
/// 노드가 이보다 많은 그래프(자동 생성된 대형 그래프 등)는 이웃 교환 탐색 폭·되풀이 횟수를 줄인다.
/// 보통 손으로 쓰는 다이어그램은 이 문턱을 넘지 않으므로 동작이 그대로다.
const LARGE_GRAPH_NODES: usize = 80;
/// 큰 그래프에서 쓰는 좁힌 탐색 폭.
const SWAP_REACH_LARGE: usize = 2;
/// 교차 하나를 가로 이동 몇 칸으로 칠지.
const CROSSING_PENALTY: f64 = 12.0;
/// 폭이 줄지 않는 접기를 연속으로 참아 주는 횟수.
const FOLD_PATIENCE: usize = 12;

impl<'a> Layout<'a> {
    fn build(graph: &'a Graph, theme: &'a Theme, direction: Direction, cap: usize, width: usize, allow_fold: bool) -> Option<Canvas> {
        let mut layout = Layout::new(graph, theme, direction, cap);
        layout.width_limit = width;
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
            // 레인은 폭 초과로 층을 접어도 나란한 띠를 유지한다 — 접기 후보에서 제외.
            Child::Block(b) => !self.is_lane(b),
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
                let (from, to) = if self.effective_reversed(e) { (edge.to, edge.from) } else { (edge.from, edge.to) };
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
            // 레인은 이미 다이어그램 전체를 관통하므로 "상자 밑으로 밀기"가 무의미하다 — 오히려
            // 레인을 벗어나는 첫 노드를 레인의 마지막 층 뒤로 밀어 버려 요구사항 3.1을 어긴다.
            if self.graph.groups[group].kind == GroupKind::Lane {
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
        // 노드에 붙는 끝 라벨(다중성) 가운데 가장 넓은 것: 접점 간격을 정한다.
        let mut widest_end_label = vec![0usize; graph.nodes.len()];
        for edge in &graph.edges {
            if edge.from != edge.to {
                degree[edge.from].1 += 1;
                degree[edge.to].0 += 1;
                let widest = width_of(&edge.tail_label).max(width_of(&edge.head_label));
                widest_end_label[edge.from] = widest_end_label[edge.from].max(widest);
                widest_end_label[edge.to] = widest_end_label[edge.to].max(widest);
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
                    Direction::TopDown => {
                        // 접점마다 라벨이 붙을 수 있으면 상자를 그만큼 넓혀 접점 사이를 벌린다.
                        let (incoming, outgoing) = degree[index];
                        let ports = incoming.max(outgoing);
                        let spacing = port_spacing_for(widest_end_label[index]);
                        let needed = if ports > 1 && w > 2 { (ports - 1) * spacing + 3 } else { 0 };
                        (h, w.max(needed))
                    }
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
            reverse_along: graph.direction_reversed,
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
            gap_tail_rows: Vec::new(),
            gap_head_rows: Vec::new(),
            extra_across: 0,
            block_extra: Vec::new(),
            debug: std::env::var_os("DG_DEBUG").is_some(),
            width_limit: usize::MAX,
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
            let (from, to) = if self.effective_reversed(i) { (edge.to, edge.from) } else { (edge.from, edge.to) };
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
            let has_outgoing = self.graph.edges.iter().enumerate().any(|(i, e)| {
                let (from, to) = if self.effective_reversed(i) { (e.to, e.from) } else { (e.from, e.to) };
                from == anchor && to != anchor
            });
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

    /// 층 계산·배선에서 실제로 쓸 방향: 되돌아가는 간선 뒤집기(`reversed`)에 전체 흐름 뒤집기
    /// (`reverse_along`, BT·RL)를 더한다. 사이클을 끊는 것과 방향을 뒤집는 것은 서로 다른
    /// 문제라 각각 독립적으로 계산되지만, 둘 다 "이 간선을 층 계산에서 뒤집어 볼지"로 합쳐진다.
    fn effective_reversed(&self, edge: usize) -> bool {
        self.reversed[edge] != self.reverse_along
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
        let group_ranges = self.effective_group_ranges();
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue;
            }
            let (from, to) = if self.effective_reversed(i) { (edge.to, edge.from) } else { (edge.from, edge.to) };
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

    /// 그룹마다 유효 층 범위 — Lane은 부모의 유효 범위(부모 없으면 다이어그램 전체), Box는
    /// (구성원이 있는) 실제 범위. `dummy_group()`(가상 노드 소속 판정)과 `build_blocks()`(블록
    /// 층 범위) 양쪽의 단일 출처라 서로 다른 값을 쓰는 일이 없다.
    fn effective_group_ranges(&self) -> Vec<(usize, usize)> {
        let mut member_ranges = vec![(usize::MAX, 0); self.graph.groups.len()];
        for (i, node) in self.graph.nodes.iter().enumerate() {
            for g in self.graph.ancestors(node.group) {
                member_ranges[g].0 = member_ranges[g].0.min(self.lnodes[i].layer);
                member_ranges[g].1 = member_ranges[g].1.max(self.lnodes[i].layer);
            }
        }
        let full_range = (0, self.layer_count.saturating_sub(1));
        (0..self.graph.groups.len())
            .map(|g| {
                let mut cursor = g;
                loop {
                    if self.graph.groups[cursor].kind != GroupKind::Lane {
                        break member_ranges[cursor];
                    }
                    match self.graph.groups[cursor].parent {
                        Some(p) => cursor = p,
                        None => break full_range,
                    }
                }
            })
            .collect()
    }

    /// 블록 0(루트)은 false.
    fn is_lane(&self, block: usize) -> bool {
        block != 0 && self.graph.groups[block - 1].kind == GroupKind::Lane
    }

    /// 레인 블록은 테두리가 부모와 한 줄이라 `group_top()` 랭크·`inner_rank()`·`route()`의
    /// `top_levels`/`bottom_levels` 어디에도 세지 않는다(중첩 레인 사슬 전체가 배너만큼만
    /// 밀리고 상자 나눔마다 한 단씩 더 밀리지 않는다 — 그 밀림은 `banner_start()`가 대신 맡는다).
    /// Box 블록은 항상 셈.
    fn counts_toward_rank(&self, block: usize) -> bool {
        !self.is_lane(block)
    }

    /// Lane 조상 수(최상위 Lane = 0). Box 블록에는 호출하지 않는다.
    fn lane_depth(&self, block: usize) -> usize {
        let mut depth = 0;
        let mut cursor = self.block_parent(block);
        while let Some(p) = cursor {
            if self.is_lane(p) {
                depth += 1;
            }
            cursor = self.block_parent(p);
        }
        depth
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
        // 레인 블록의 층 범위는 소속 범위(부모 레인 또는 다이어그램 전체) 전체로 강제한다 —
        // dummy_group()과 같은 출처(effective_group_ranges)라 두 판정이 어긋나지 않는다.
        let effective = self.effective_group_ranges();
        for block in 1..self.blocks.len() {
            if self.blocks[block].registered && self.is_lane(block) {
                let g = self.blocks[block].group.unwrap();
                self.blocks[block].layer_min = effective[g].0;
                self.blocks[block].layer_max = effective[g].1;
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

    /// `child`가 레인 블록인지(`Child::Node`는 항상 false).
    fn is_lane_child(&self, child: Child) -> bool {
        matches!(child, Child::Block(b) if self.is_lane(b))
    }

    /// `child`를 `block` 안에 놓을 때 좌우로 남길 여백. 부모·자식이 모두 레인이면 0(밀착),
    /// 그 밖(노드·일반 그룹 자식, 또는 부모가 레인이 아님)은 기존 `block_pad` 그대로.
    fn lane_pad(&self, block: usize, child: Child) -> usize {
        if self.is_lane(block) && self.is_lane_child(child) { 0 } else { self.block_pad(block) }
    }

    /// 자식들을 순서대로, 층이 겹치는 앞 자식의 오른쪽에 놓는다. 블록 폭을 돌려준다.
    fn place_block(&mut self, block: usize) -> usize {
        // 부모 레인의 라벨 여유(block_extra)는 마지막 레인 자식에게 넘기고 자기 폭에는 더하지
        // 않는다 — 밀착 배치를 유지하면서도 라벨이 잘리지 않게 마지막 레인이 넓어진다.
        if self.is_lane(block) {
            let last_lane_child = self.blocks[block].children.iter().rev().find_map(|&c| match c {
                Child::Block(b) if self.is_lane(b) => Some(b),
                _ => None,
            });
            if let Some(last_lane_block) = last_lane_child {
                let extra = std::mem::take(&mut self.block_extra[block]);
                self.block_extra[last_lane_block] += extra;
            }
        }
        let default_pad = self.block_pad(block);
        let children = self.blocks[block].children.clone();
        let mut placed: Vec<(usize, usize, usize, usize, Child)> = Vec::new();
        let mut max_end = 0;
        for child in children.iter().copied() {
            let (child_width, layer_min, layer_max) = match child {
                Child::Node(i) => (self.lnodes[i].footprint(), self.lnodes[i].layer, self.lnodes[i].layer),
                Child::Block(b) => {
                    let w = self.place_block(b);
                    (w, self.blocks[b].layer_min, self.blocks[b].layer_max)
                }
            };
            let mut start = placed
                .iter()
                .filter(|&&(_, _, lo, hi, _)| lo <= layer_max && layer_min <= hi)
                .map(|&(_, end, _, _, earlier)| {
                    if self.is_lane(block) && self.is_lane_child(earlier) && self.is_lane_child(child) {
                        // 형제 레인: 경계 칸을 공유해 두 테두리가 한 줄로 합쳐지게 한 칸 앞에서 시작한다.
                        end.saturating_sub(1)
                    } else {
                        end + self.gap_between(earlier, child)
                    }
                })
                .max()
                .unwrap_or(0);
            // 닻(그룹으로 드나드는 화살표 자리)은 테두리 제목 글자와 겹치지 않게 제목 오른쪽에 둔다.
            if let Child::Node(i) = child
                && self.lnodes[i].node.is_some_and(|n| self.graph.nodes[n].shape == Shape::Anchor)
                && let Some(g) = self.blocks[block].group
            {
                start = start.max(width_of(&self.graph.groups[g].title) + 2);
            }
            let end = start + child_width;
            placed.push((start, end, layer_min, layer_max, child));
            let pad = self.lane_pad(block, child);
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
        // 레인 밀착: 첫 자식의 왼쪽·마지막 자식의 오른쪽 여백은 각자의 `lane_pad`를 따른다(보통
        // 그룹은 둘 다 `default_pad`라 기존 `2 * pad`와 같은 값).
        let left_pad = children.first().map(|&c| self.lane_pad(block, c)).unwrap_or(default_pad);
        let right_pad = children.last().map(|&c| self.lane_pad(block, c)).unwrap_or(default_pad);
        let width = (max_end + left_pad + right_pad).max(title_min) + self.block_extra.get(block).copied().unwrap_or(0);
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
            // 레인 자식이 하나라도 있으면 통째로 선언 순서를 지킨다(BPMN은 한 그룹의 직계
            // 자식이 레인이면 전부 레인이므로 부분 고정은 사변적 — research.md).
            if children.iter().any(|&c| self.is_lane_child(c)) {
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
        self.place_and_straighten();
        let mut current = self.total_edge_length();
        // 노드가 많으면 후보 하나 평가할 때마다 드는 재배치·교차 계산 비용이 커진다. 탐색 폭·되풀이
        // 횟수를 줄여 전체 시간을 억제한다 — 문턱 아래(보통 손으로 쓰는 다이어그램)는 원래 그대로다.
        let large = self.lnodes.len() > LARGE_GRAPH_NODES;
        let swap_reach = if large { SWAP_REACH_LARGE } else { SWAP_REACH };
        let outer_iterations = if large { 1 } else { 3 };
        for _ in 0..outer_iterations {
            let mut improved = false;
            for block in 0..self.blocks.len() {
                // reorder()와 같은 이유로 레인 자식이 있는 블록은 이웃 교환도 건너뛴다.
                if self.blocks[block].children.iter().any(|&c| self.is_lane_child(c)) {
                    continue;
                }
                let mut k = 1;
                while k < self.blocks[block].children.len() {
                    let unit = self.unit_of(block, k);
                    let first = unit[0];
                    let last = *unit.last().unwrap_or(&first);
                    let shares = |j: usize| {
                        let other = self.blocks[block].children[j];
                        unit.iter().any(|&u| self.children_share_layer(other, self.blocks[block].children[u]))
                    };
                    // 앞 자식 앞으로, 그리고 뒤 자식 뒤로 옮겨 보는 후보(가까운 것부터).
                    let earlier: Vec<usize> = (0..first).rev().filter(|&j| shares(j)).take(swap_reach).collect();
                    let later: Vec<usize> = (last + 1..self.blocks[block].children.len()).filter(|&j| shares(j)).take(swap_reach).collect();
                    let candidates: Vec<usize> = earlier.into_iter().chain(later).collect();
                    let mut accepted = false;
                    for j in candidates {
                        let before = self.blocks[block].children.clone();
                        let moving: Vec<Child> = unit.iter().map(|&u| self.blocks[block].children[u]).collect();
                        for &u in unit.iter().rev() {
                            self.blocks[block].children.remove(u);
                        }
                        // 뒤로 옮길 때는 빠진 만큼 자리가 당겨진다.
                        let insert_at = if j > last { j + 1 - unit.len() } else { j };
                        for (offset, child) in moving.into_iter().enumerate() {
                            self.blocks[block].children.insert(insert_at + offset, child);
                        }
                        self.place_and_straighten();
                        let fits = self.canvas_size().0 <= self.width_limit;
                        let candidate = if fits { self.total_edge_length() } else { f64::INFINITY };
                        if self.debug {
                            let describe = |c: Child| match c {
                                Child::Node(i) => self.lnodes[i].node.map_or(format!("dummy{i}"), |n| self.graph.nodes[n].id.clone()),
                                Child::Block(b) => format!("block{b}"),
                            };
                            eprintln!("swap block {block}: {} -> before {} : cost {current:.0} -> {candidate:.0} crossings {}", describe(self.blocks[block].children[insert_at]), describe(before[j]), self.count_crossings());
                        }
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

    /// 후보 평가용: 배치 뒤 직선화까지 해서 실제 그려질 자리에 가깝게 잰다.
    fn place_and_straighten(&mut self) {
        self.place();
        self.assign_ports();
        self.straighten();
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

    /// 배치 품질: 구간의 가로 이동량 합(중심 기준)에 교차 수를 무겁게 더한 값.
    fn total_edge_length(&self) -> f64 {
        let length: f64 = self.segments.iter().map(|s| (self.lnodes[s.from].center() - self.lnodes[s.to].center()).abs()).sum();
        length + CROSSING_PENALTY * self.count_crossings() as f64
    }

    /// 같은 통로를 지나는 구간 쌍 가운데 교차하는 것의 수(중심 기준 근사).
    ///
    /// 같은 층(`layer`)에서 출발하는 구간끼리만 겨루므로, 먼저 층별로 묶고 그 안에서만 짝을 짓는다.
    /// 원래는 전체 구간을 두 겹으로 훑으며 층이 다르면 건너뛰었는데(구간 수의 제곱), 이 함수가 이웃
    /// 교환 탐색의 후보 하나마다 다시 불리다 보니 층 수가 많은 큰 그래프에서 비용이 크게 불었다.
    /// 층별로 나누면 훑는 범위가 총 구간 수가 아니라 각 층 구간 수의 제곱들의 합으로 줄어든다.
    fn count_crossings(&self) -> usize {
        // `self.layer_count`에 기대지 않고 지금 구간들의 실제 층 범위로 크기를 정한다 — 이 함수는
        // 배치가 계속 바뀌는 도중에도 불리므로, 어긋난 캐시값으로 색인이 벗어나는 일을 원천적으로 막는다.
        let layers = self.segments.iter().map(|s| self.lnodes[s.from].layer).max().map_or(0, |max| max + 1);
        let mut by_layer: Vec<Vec<usize>> = vec![Vec::new(); layers];
        for (index, segment) in self.segments.iter().enumerate() {
            by_layer[self.lnodes[segment.from].layer].push(index);
        }
        let mut crossings = 0;
        for indices in &by_layer {
            for (position, &a_index) in indices.iter().enumerate() {
                let a = &self.segments[a_index];
                let (ea, na) = (self.lnodes[a.from].center(), self.lnodes[a.to].center());
                for &b_index in &indices[position + 1..] {
                    let b = &self.segments[b_index];
                    let (eb, nb) = (self.lnodes[b.from].center(), self.lnodes[b.to].center());
                    let inverted = (ea < eb && na > nb) || (ea > eb && na < nb);
                    let (a_low, a_high) = (ea.min(na), ea.max(na));
                    let (b_low, b_high) = (eb.min(nb), eb.max(nb));
                    let a_contains_b = a_low < b_low && b_high < a_high;
                    let b_contains_a = b_low < a_low && a_high < b_high;
                    if inverted || a_contains_b || b_contains_a {
                        crossings += 1;
                    }
                }
            }
        }
        crossings
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
            // 레인 형제 블록은 실제 구성원이 없는 층에도 (다이어그램 전체를 관통하므로) 자리를
            // 차지한다 — 그 층에 같은 층 이웃이 없어 위 두 검사로 못 잡는다. 레인 상자 밖에
            // 그려져야 하는 노드(요구사항 3.3)가 침범하지 않게 별도로 막는다.
            for child in self.blocks[container].children.clone() {
                let Child::Block(b) = child else { continue };
                if !(self.is_lane(b) && self.blocks[b].registered && self.blocks[b].layer_min <= layer && layer <= self.blocks[b].layer_max) {
                    continue;
                }
                let (bstart, bend) = (self.blocks[b].start, self.blocks[b].start + self.blocks[b].width);
                let node_center = self.lnodes[i].across as f64 + footprint as f64 / 2.0;
                let lane_center = bstart as f64 + (bend - bstart) as f64 / 2.0;
                if node_center < lane_center {
                    high = high.min(bstart.saturating_sub(self.gap_across()));
                } else {
                    low = low.max(bend + self.gap_across());
                }
            }
            // 가상 노드는 선을 곧게 펴는 것이 우선이므로, 막고 있는 같은 블록의 이웃을 밀어내 자리를 만든다.
            if self.lnodes[i].node.is_none() {
                if desired > high {
                    high += self.push_members(&members, k + 1, desired - high, container, true);
                } else if desired < low {
                    low -= self.push_members(&members, k, low - desired, container, false);
                }
            }
            if low > high {
                continue;
            }
            self.lnodes[i].across = desired.clamp(low, high);
        }
    }

    /// `members[from..]`(오른쪽으로) 또는 `members[..from]`(왼쪽으로)를 `delta`만큼 밀어 본다.
    /// 같은 블록의 이웃만 밀고, 다른 블록이나 블록 경계는 벽으로 본다. 실제로 확보한 칸 수를 돌려준다.
    fn push_members(&mut self, members: &[usize], from: usize, delta: usize, container: usize, rightward: bool) -> usize {
        let index = if rightward { from } else { from.checked_sub(1).unwrap_or(usize::MAX) };
        let Some(&j) = members.get(index) else { return delta };
        if self.lnode_block[j] != container {
            return 0;
        }
        let footprint = self.lnodes[j].footprint();
        let pad = self.block_pad(container);
        let (block_start, block_end) = (self.blocks[container].start, self.blocks[container].start + self.blocks[container].width);
        if rightward {
            let mut high = block_end.saturating_sub(pad + footprint);
            if let Some(&next) = members.get(index + 1) {
                let gap = self.gap_between(Child::Node(j), Child::Node(next));
                if let Some((start, _)) = self.bound_of(container, next) {
                    let wanted_end = self.lnodes[j].across + delta + footprint + gap;
                    let gained = if wanted_end > start { self.push_members(members, index + 1, wanted_end - start, container, true) } else { 0 };
                    high = high.min((start + gained).saturating_sub(gap + footprint));
                }
            }
            let target = (self.lnodes[j].across + delta).min(high);
            let achieved = target.saturating_sub(self.lnodes[j].across);
            self.lnodes[j].across = target;
            achieved
        } else {
            let mut low = block_start + pad;
            if index > 0 {
                let previous = members[index - 1];
                let gap = self.gap_between(Child::Node(previous), Child::Node(j));
                if let Some((_, end)) = self.bound_of(container, previous) {
                    let wanted_start = self.lnodes[j].across.saturating_sub(delta);
                    let gained = if end + gap > wanted_start { self.push_members(members, index, end + gap - wanted_start, container, false) } else { 0 };
                    low = low.max((end - gained) + gap);
                }
            }
            let target = self.lnodes[j].across.saturating_sub(delta).max(low);
            let achieved = self.lnodes[j].across.saturating_sub(target);
            self.lnodes[j].across = target;
            achieved
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

    /// 노드의 안쪽 칸에 `count`개 접점을 둘 때 `index`번째 위치. 가운데를 기준으로 `spacing`칸씩 벌리되,
    /// 들어가지 않으면 두 칸, 그것도 안 되면 고르게 나눈다.
    fn spread(&self, lnode: usize, index: usize, count: usize, spacing: usize) -> usize {
        let node = &self.lnodes[lnode];
        if node.across_size <= 2 {
            return node.across;
        }
        let usable = node.across_size - 2;
        let center = node.across + node.across_size / 2;
        if count <= 1 {
            return center;
        }
        for step in [spacing.max(2), 2] {
            if step * (count - 1) < usable {
                let start = center - (step * (count - 1)) / 2;
                return start + step * index;
            }
        }
        (node.across + 1 + ((index + 1) * usable) / (count + 1)).min(node.across + node.across_size - 2)
    }

    /// 끝 라벨(다중성)이 이웃 접점과 겹치지 않게 하려면 접점 사이가 얼마나 벌어져야 하는지.
    fn port_spacing(&self, segments: &[usize], tail_side: bool) -> usize {
        segments
            .iter()
            .map(|&s| {
                let (_, tail, head) = self.segment_labels(s);
                width_of(if tail_side { &tail } else { &head })
            })
            .max()
            .map_or(2, port_spacing_for)
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
            let spacing = self.port_spacing(&outgoing, true);
            for (k, s) in outgoing.into_iter().enumerate() {
                self.segments[s].exit_offset = self.spread(i, k, total, spacing) - base;
                self.segments[s].exit_rank = (k, total);
            }
            let mut incoming: Vec<usize> = (0..self.segments.len()).filter(|&s| self.segments[s].to == i).collect();
            incoming.sort_by(|&a, &b| {
                let ca = self.lnodes[self.segments[a].from].center();
                let cb = self.lnodes[self.segments[b].from].center();
                ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let total = incoming.len();
            let spacing = self.port_spacing(&incoming, false);
            for (k, s) in incoming.into_iter().enumerate() {
                self.segments[s].entry_offset = self.spread(i, k, total, spacing) - base;
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
                if b != 0 && self.counts_toward_rank(b) {
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
        self.gap_tail_rows = vec![1; gap_count];
        self.gap_head_rows = vec![1; gap_count];
        for s in 0..self.segments.len() {
            let segment = &self.segments[s];
            let layer = self.lnodes[segment.from].layer;
            if layer >= gap_count {
                continue;
            }
            let edge = &self.graph.edges[segment.edge];
            let (top_marker, bottom_marker) = if self.effective_reversed(segment.edge) { (edge.head, edge.tail) } else { (edge.tail, edge.head) };
            if segment.is_first {
                self.gap_tail_rows[layer] = self.gap_tail_rows[layer].max(marker_glyphs(top_marker, self.direction, true).len());
            }
            if segment.is_last {
                self.gap_head_rows[layer] = self.gap_head_rows[layer].max(marker_glyphs(bottom_marker, self.direction, false).len());
            }
        }
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
                if self.direction == Direction::TopDown {
                    // 끝 라벨은 선 오른쪽 두 칸부터 쓰므로 캔버스가 그만큼 넓어야 한다.
                    for (across, text) in [(segment.exit, &tail_label), (segment.entry, &head_label)] {
                        if !text.is_empty() {
                            self.extra_across = self.extra_across.max(across + 2 + width_of(text));
                        }
                    }
                }
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
            self.gap[layer] = GAP_ALONG_MIN
                .max(self.gap_tail_rows[layer] + self.gap_label_room[layer] + channels + self.gap_head_room[layer] + self.gap_head_rows[layer]);
        }

        self.layer_start = vec![0; self.layer_count];
        if self.layer_count > 0 {
            // 깊이별 배너 크기 합을 첫 층 시작 앞에 둔다 — 레인이 없으면 0이라 기존 출력과
            // 바이트 단위로 같다(요구사항 6.1).
            self.layer_start[0] = self.banner_along().iter().sum();
        }
        for layer in 1..self.layer_count {
            self.layer_start[layer] = self.layer_start[layer - 1] + self.layer_total(layer - 1) + self.gap[layer - 1];
        }
    }

    /// 깊이별 배너 크기(흐름축). TB는 깊이마다 고정 2(테두리 줄 + 제목 줄), LR은 그 깊이에서
    /// 가장 넓은 레인 제목 폭에 맞춘 3(테두리 칸 + 제목 앞뒤 한 칸씩)을 더한 값 — 잘리지 않는다.
    fn banner_along(&self) -> Vec<usize> {
        let max_depth = (1..self.blocks.len())
            .filter(|&b| self.blocks[b].registered && self.is_lane(b))
            .map(|b| self.lane_depth(b))
            .max();
        let Some(max_depth) = max_depth else { return Vec::new() };
        (0..=max_depth)
            .map(|depth| match self.direction {
                Direction::TopDown => 2,
                Direction::LeftRight => {
                    let widest_title = (1..self.blocks.len())
                        .filter(|&b| self.blocks[b].registered && self.is_lane(b) && self.lane_depth(b) == depth)
                        .map(|b| width_of(&self.graph.groups[self.blocks[b].group.unwrap()].title))
                        .max()
                        .unwrap_or(0);
                    3 + widest_title
                }
            })
            .collect()
    }

    /// 깊이 `depth`의 배너가 시작하는 흐름축 위치(그보다 얕은 깊이의 배너 크기 합).
    fn banner_start(&self, depth: usize) -> usize {
        self.banner_along().iter().take(depth).sum()
    }

    /// 레인 사각형의 흐름축 끝: 최상위(부모가 레인이 아님)면 캔버스 끝, 부모가 레인이면 부모와
    /// 같은 끝(사슬 전체가 다이어그램 끝까지 함께 닿는다).
    fn lane_along_end(&self, block: usize) -> usize {
        match self.block_parent(block) {
            Some(p) if self.is_lane(p) => self.lane_along_end(p),
            _ => self.total_along().saturating_sub(1),
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
                    let inverted = (a.exit < b.exit && a.entry > b.entry) || (a.exit > b.exit && a.entry < b.entry);
                    let mutual = a.exit == b.entry && b.exit == a.entry && a.exit != a.entry;
                    if !inverted && !mutual {
                        continue;
                    }
                    // 뒤집힌 쌍의 두 도착점이 모두 가상 노드면 자리를 맞바꾸는 것으로 교차가 사라진다.
                    let (a_to, b_to) = (a.to, b.to);
                    let is_dummy = |l: usize| self.lnodes[l].node.is_none();
                    if is_dummy(a_to) && is_dummy(b_to) && self.lnodes[a_to].layer == self.lnodes[b_to].layer {
                        if self.debug {
                            eprintln!("swap targets: seg {i} ({}->{}) x seg {j} ({}->{})", a.exit, a.entry, b.exit, b.entry);
                        }
                        let (x, y) = (self.lnodes[a_to].across, self.lnodes[b_to].across);
                        self.lnodes[a_to].across = y;
                        self.lnodes[b_to].across = x;
                        // 같은 회차에서 반대 순서로 다시 만나 되돌리지 않도록 바로 갱신한다.
                        self.refresh_ports();
                        nudged = true;
                        continue;
                    }
                    // 출발점 쪽은 바꾸지 않는다: 교차를 아래로만 밀어 실제 노드 앞에서 멈추게 해야
                    // 위아래로 왕복하지 않는다. 남는 교차는 긴 가로선 위의 건너뛰기(◠)로 그려진다.
                    if !mutual {
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
        let span_of = |k: usize| {
            let segment = &self.segments[intervals[k].2];
            (segment.exit.min(segment.entry), segment.exit.max(segment.entry))
        };
        for i in 0..count {
            for j in 0..count {
                if i == j {
                    continue;
                }
                let (a, b) = (&self.segments[intervals[i].2], &self.segments[intervals[j].2]);
                let (low, high) = span_of(i);
                let strictly_inside = |x: usize| low < x && x < high;
                // i의 가로 구간 안에서 j가 내려오면(출발 접점) j의 통로가 위여야 교차하지 않고,
                // j가 그 안으로 내려가면(도착 접점) j의 통로가 아래여야 한다.
                // 출발 열 = 도착 열이면 세로선을 나눠 쓰므로 출발 쪽이 반드시 위.
                let j_above_i = strictly_inside(b.exit) && !strictly_inside(b.entry);
                let j_below_i = strictly_inside(b.entry) && !strictly_inside(b.exit);
                let shares_column = a.exit == b.entry;
                if shares_column || j_below_i {
                    must_precede[i].push(j);
                    pending[j] += 1;
                } else if j_above_i && a.entry != b.exit {
                    must_precede[j].push(i);
                    pending[i] += 1;
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
        let reversed = self.effective_reversed(segment.edge);
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
                    let (min_width, min_height) = match self.direction {
                        Direction::TopDown => (node.across_size, 0),
                        Direction::LeftRight => (0, node.across_size),
                    };
                    shape::draw(canvas, x, y, self.graph.nodes[n].shape, &node.sections, self.theme, min_width, min_height);
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
            let (top, bottom) = if self.is_lane(b) { (self.banner_start(self.lane_depth(b)), self.lane_along_end(b)) } else { (self.group_top(b), self.group_bottom(b)) };
            let (x0, y0) = self.to_canvas(top, block.start);
            let (x1, y1) = self.to_canvas(bottom, block.start + block.width - 1);
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            canvas.rect(x0, y0, w, h, LineKind::Solid, self.theme.diagram_group, false);
            if self.is_lane(b) {
                // 구분선: 배너 전체와 내용 상자를 가르는 교차선. `group_top(b)`가 이미
                // `layer_start[0] + 2*rank`(레인은 rank 0이라 사슬 전체가 같은 줄)를 준다.
                let divider = self.group_top(b);
                self.line_across(canvas, block.start, block.start + block.width - 1, divider, LineKind::Solid, self.theme.diagram_group);
            }
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
            if self.is_lane(b) {
                let banner_top = self.banner_start(self.lane_depth(b));
                let (x0, y0) = self.to_canvas(banner_top + 1, block.start + 1);
                let text = match self.direction {
                    Direction::TopDown => truncate(title, block.width.saturating_sub(4)).to_string(),
                    Direction::LeftRight => title.clone(),
                };
                canvas.text(x0, y0, &text, self.theme.diagram_group.bold());
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
            if p != 0 && self.blocks[p].layer_min == block.layer_min && self.counts_toward_rank(p) {
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

    /// 같은 층에서 끝나는 하위 블록 사슬의 길이. 레인 자식은 부모와 테두리가 한 줄이라 세지
    /// 않는다(`counts_toward_rank`) — 사슬 자체는 그 안의 Box 자식을 찾도록 계속 내려간다.
    fn inner_rank(&self, block: usize) -> usize {
        let layer_max = self.blocks[block].layer_max;
        self.blocks[block]
            .children
            .iter()
            .filter_map(|child| match child {
                Child::Block(c) if self.blocks[*c].layer_max == layer_max => {
                    Some(usize::from(self.counts_toward_rank(*c)) + self.inner_rank(*c))
                }
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
        // 그룹 닻으로 드나드는 선은 노드와 같은 규칙으로 그룹 테두리 바로 바깥 칸에서 끝나고(시작하고),
        // 표식도 그 칸에 놓인다.
        let top = match self.lnodes[segment.from].node {
            Some(n) if self.graph.nodes[n].shape == Shape::Anchor => self.group_bottom(self.lnode_block[segment.from]) + 1,
            Some(_) => self.node_along(segment.from) + self.lnodes[segment.from].along_size,
            None => gap_start,
        };
        let bottom = match self.lnodes[segment.to].node {
            Some(n) if self.graph.nodes[n].shape == Shape::Anchor => self.group_top(self.lnode_block[segment.to]).saturating_sub(1),
            Some(_) => self.node_along(segment.to) - 1,
            None => self.layer_start[layer + 1] - 1,
        };
        (top, bottom, gap_start)
    }

    fn channel_position(&self, s: usize, gap_start: usize) -> Option<usize> {
        let segment = &self.segments[s];
        let layer = self.lnodes[segment.from].layer;
        segment.channel.map(|c| gap_start + self.gap_tail_rows[layer] + self.gap_label_room[layer] + c)
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
        let reversed = self.effective_reversed(segment.edge);
        let (top, bottom, gap_start) = self.segment_span(s);
        let (label, tail_label, head_label) = self.segment_labels(s);
        let (top_marker, bottom_marker) = if reversed { (edge.head, edge.tail) } else { (edge.tail, edge.head) };
        let marker_style = self.theme.diagram_line;
        if segment.is_first {
            // 위쪽 끝: 마지막 글자가 노드(top)에 닿고, 앞 글자들은 그 아래로 이어진다.
            let glyphs = marker_glyphs(top_marker, self.direction, true);
            let count = glyphs.len();
            for (k, glyph) in glyphs.into_iter().enumerate() {
                let along = top + (count - 1 - k);
                if along > bottom {
                    continue;
                }
                let (x, y) = self.to_canvas(along, segment.exit);
                canvas.put(x, y, glyph, marker_style);
            }
        }
        if segment.is_last {
            let glyphs = marker_glyphs(bottom_marker, self.direction, false);
            let count = glyphs.len();
            for (k, glyph) in glyphs.into_iter().enumerate() {
                let Some(along) = bottom.checked_sub(count - 1 - k) else { continue };
                if along < top {
                    continue;
                }
                let (x, y) = self.to_canvas(along, segment.entry);
                canvas.put(x, y, glyph, marker_style);
            }
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
                // from측 라벨(관계 이름·tail_label)은 이 세그먼트가 실제로 시작하는 자기 노드의
                // 오른쪽 바로 다음 칸(top)에 붙인다 — 레이어 공용 통로 시작점(gap_start)을 쓰면
                // 같은 레이어의 더 넓은 노드 기준으로 밀려, 좁은 노드에서 시작하는 라벨이 자기
                // 노드에서 멀리 떨어져 보인다(diagram-lr-tail-label-position). to측(head_label)은
                // 이미 세그먼트 자신의 bottom을 쓴다 — 그것과 대칭을 맞춘다(단, head_label도
                // tail_label과 같은 이유로 표식과 다른 줄을 쓰므로 to측 노드 경계에 바로
                // 붙여야 한다 — 아래 head_label 계산 참고).
                //
                // `label`(관계 이름)은 표식과 같은 segment.exit 줄을 쓴다. top 바로 다음
                // 칸부터 쓰면 표식이 두 글자(까치발 `CrowZeroMany` 등, 예: `╫>`)인 경우 그
                // 두 번째 글자를 그대로 덮어써 지운다 — 표식이 실제로 차지하는 칸 수만큼만
                // 건너뛴다(diagram-lr-label-marker-overlap).
                //
                // `tail_label`(카디널리티 등)은 표식보다 한 줄 위(exit-1)에 그려 표식과 절대
                // 같은 칸을 다투지 않는다 — 표식 글자 수와 무관하게 노드 경계 바로 다음 칸(top)에
                // 붙여, 표식이 노드 테두리에 맞닿는 것과 같은 간격으로 맞춘다(다이어그램마다
                // 표식 글자 수가 달라도 카디널리티 위치가 흔들리지 않는다).
                let tail_glyphs = marker_glyphs(top_marker, self.direction, true).len().max(1);
                if !label.is_empty() {
                    canvas.text(top + tail_glyphs, segment.exit, &label, label_style);
                }
                if !tail_label.is_empty() {
                    canvas.text(top, segment.exit.saturating_sub(1), &tail_label, label_style);
                }
                if !head_label.is_empty() {
                    // tail_label과 대칭: head_label도 표식과 다른 줄(entry-1)에 그려 표식
                    // 글자 수를 다툴 일이 없다 — 마지막 글자가 to측 노드 경계 바로 앞 칸
                    // (bottom, 표식의 마지막 글자와 같은 자리)에서 끝나도록 오른쪽으로
                    // 한 칸 더 붙인다(diagram-lr-label-marker-overlap).
                    let x = (bottom + 1).saturating_sub(width_of(&head_label));
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

/// 끝 라벨을 선의 어느 쪽에 둘지: 접점이 둘이면 왼쪽 것은 선 왼쪽에, 그 밖에는 모두 오른쪽에 둔다
/// (셋 이상은 접점 간격을 라벨 폭만큼 벌려 두었다).
fn side_label_x(line_x: usize, rank: (usize, usize), label_width: usize) -> usize {
    let (index, count) = rank;
    // 선·표식과 라벨 사이에 한 칸을 띄운다.
    if count == 2 && index == 0 { line_x.saturating_sub(label_width + 1) } else { line_x + 2 }
}

/// 끝 라벨 폭에 따른 접점 간격: 표식, 한 칸, 라벨, 한 칸.
fn port_spacing_for(label_width: usize) -> usize {
    if label_width == 0 { 2 } else { label_width + 3 }
}

/// 끝 표식 글자들. `at_top`이면 위(또는 왼쪽) 노드에 붙는 쪽이다.
/// 여러 글자면 선에서 노드 쪽으로 차례로 놓는다(마지막 글자가 노드에 닿는다).
fn marker_glyphs(marker: Marker, direction: Direction, at_top: bool) -> Vec<char> {
    let index = match (direction, at_top) {
        (Direction::TopDown, true) => 0,
        (Direction::TopDown, false) => 1,
        (Direction::LeftRight, true) => 2,
        (Direction::LeftRight, false) => 3,
    };
    // 까치발: 발가락이 노드 쪽으로 벌어진다. 한 개는 가로막대(╪/╫), 없음은 ○.
    //
    // "many"는 원래 집합론 기호(⋎/⋏/≻/≺)를 빌려 썼는데, 이 넷은 흔한 CJK
    // 모노스페이스 폰트(예: 기본 Noto Sans Mono CJK)의 커버리지 밖이라
    // 터미널이 매번 다른 폰트로 폴백해서 셰이핑해야 했다 — 실측: 이 폴백이
    // 다이어그램이 화면에 있을 때만 스크롤이 눈에 띄게 느려지는 원인이었다
    // (한 줄 안에서 폰트를 여러 번 갈아타야 함). `OpenArrow`가 이미 쓰는
    // ∧/∨/</> 로 바꿨다 — 넷 다 기본 폰트에 있고, `one`(╪/╫) 뒤에만 붙어
    // 나오므로 단독 `OpenArrow`와 헷갈리지 않는다.
    let one = ['╪', '╪', '╫', '╫'][index];
    // 까치발("many")은 노드 쪽으로 벌어지고 선 쪽으로 좁아져야 한다(OpenArrow와 반대 방향
    // 관례 — OpenArrow는 뾰족한 끝이 노드에 닿아야 하고, 까치발은 벌어진 끝이 노드에 닿아야
    // 한다). 문자 네 개는 OpenArrow와 같은 걸 재사용하되(폰트 폴백 이유는 위와 동일), 위/아래
    // 그리고 왼쪽/오른쪽을 서로 맞바꿔 벌어진 쪽이 노드에 닿게 한다(diagram-crow-foot-orientation).
    let many = ['∨', '∧', '>', '<'][index];
    let glyphs: Vec<char> = match marker {
        Marker::None => return Vec::new(),
        Marker::Arrow => vec![['▲', '▼', '◀', '▶'][index]],
        Marker::OpenArrow => vec![['∧', '∨', '<', '>'][index]],
        Marker::Triangle => vec![['△', '▽', '◁', '▷'][index]],
        Marker::DiamondFilled => vec!['♦'],
        Marker::DiamondOpen => vec!['♢'],
        Marker::Circle => vec!['○'],
        // ✕(U+2715)도 같은 이유로 폴백 대상이라 어디서나 커버되는 ×(U+00D7)로.
        Marker::Cross => vec!['×'],
        // `one`(╪/╫) 자체가 이미 선을 가로지르는 두 짧은 표식(까치발 "1" 카디널리티) 모양이라
        // 한 글자면 충분하다 — 예전엔 두 번 그려서 표식이 네 줄로 겹쳐 보였다.
        Marker::CrowOne => vec![one],
        Marker::CrowZeroOne => vec!['○', one],
        Marker::CrowMany => vec![one, many],
        Marker::CrowZeroMany => vec!['○', many],
        // BPMN default 흐름 꼬리: 방향과 무관한 한 글자(Circle/Cross와 같은 패턴) — 세로선
        // 위에서도 가로선 위에서도 빗금으로 읽힌다.
        Marker::Slash => vec!['╱'],
    };
    glyphs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::ir::{Edge, EventPosition, Shape};

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

    /// 되돌아가는 간선 없는 사슬: BT·RL 검증에서 순수하게 방향만 본다.
    fn chain_graph() -> Graph {
        let mut g = Graph::default();
        let a = g.intern("A", "Start", Shape::Rect, None);
        let b = g.intern("B", "Middle", Shape::Rect, None);
        let c = g.intern("C", "End", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: b, to: c, head: Marker::Arrow, ..Edge::default() });
        g
    }

    /// 텍스트가 나오는 줄 번호(첫 매치).
    fn row_of(lines: &[String], needle: &str) -> usize {
        lines.iter().position(|line| line.contains(needle)).unwrap_or_else(|| panic!("{needle} not found in {lines:?}"))
    }

    /// 문자 열(칸) 기준 위치. 상자 그림 문자는 UTF-8 바이트로 3바이트라 `str::find`(바이트
    /// 오프셋)를 그대로 쓰면 칸 수와 어긋난다 — 서로 다른 줄의 위치를 비교할 때는 항상 이걸 쓴다.
    fn char_col(row: &str, needle: char) -> usize {
        row.chars().position(|c| c == needle).unwrap_or_else(|| panic!("{needle:?} not found in row {row:?}"))
    }

    fn char_rcol(row: &str, needle: char) -> usize {
        row.chars().enumerate().filter(|&(_, c)| c == needle).last().map(|(i, _)| i).unwrap_or_else(|| panic!("{needle:?} not found in row {row:?}"))
    }

    #[test]
    fn bottom_up_flips_top_down_without_mirroring_text() {
        let mut g = chain_graph();
        g.direction = Some(Direction::TopDown);
        g.direction_reversed = true;
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        // 시작(A)이 물리적으로 아래, 끝(C)이 위 — 일반 TD와 반대.
        assert!(row_of(&out, "End") < row_of(&out, "Start"), "{text}");
        assert!(text.contains("▲"), "{text}");
        assert!(!text.contains("▼"), "{text}");
        // 텍스트 자체는 뒤집히지 않는다(글자 순서 보존).
        assert!(text.contains("Start"), "{text}");
        assert!(text.contains("Middle"), "{text}");
        assert!(text.contains("End"), "{text}");
    }

    #[test]
    fn right_left_flips_left_right_without_mirroring_text() {
        let mut g = chain_graph();
        g.direction = Some(Direction::LeftRight);
        g.direction_reversed = true;
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let row = out.iter().find(|line| line.contains("Start")).unwrap();
        // 시작(A)이 오른쪽, 끝(C)이 왼쪽 — 일반 LR과 반대.
        assert!(row.find("End").unwrap() < row.find("Start").unwrap(), "{text}");
        assert!(text.contains("◀"), "{text}");
        assert!(!text.contains("▶"), "{text}");
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

    /// 자동 생성된 대형 그래프(모듈 의존성 덤프 등)를 그릴 때 이웃 교환 탐색이 노드 수에 비례해
    /// 느려지지 않아야 한다. 한때 100여 개 노드짜리 그래프 하나를 접어 넣는 데 2초 넘게 걸렸는데
    /// (`improve_by_swaps`가 후보마다 전체 재배치+교차 계산을 반복), `LARGE_GRAPH_NODES` 문턱
    /// 위에서는 탐색 폭·되풀이 횟수를 줄이도록 고쳤다. 이 테스트는 그 회귀를 막는다.
    #[test]
    fn large_graph_renders_within_a_bounded_time() {
        // 실제 모듈 의존성 그래프처럼: 모듈 대부분이 몇 안 되는 공통 유틸로 모이고(폭이 넓은 층 하나),
        // 층 사이를 멀리 건너뛰는 간선은 없게 한다(그런 간선은 이 테스트가 아니라 층 매김 쪽 문제라
        // 여기서 같이 재현하면 원인이 섞인다).
        let mut g = Graph::default();
        let utils: Vec<usize> = (0..5).map(|i| g.intern(&format!("util{i}"), &format!("유틸 {i}"), Shape::Rect, None)).collect();
        for i in 0..100 {
            let module = g.intern(&format!("mod{i}"), &format!("module {i}"), Shape::Rect, None);
            g.add_edge(Edge { from: module, to: utils[i % utils.len()], head: Marker::Arrow, ..Edge::default() });
            if i % 7 == 0 && i > 0 {
                let sibling = g.find(&format!("mod{}", i - 1)).unwrap();
                g.add_edge(Edge { from: module, to: sibling, head: Marker::Arrow, ..Edge::default() });
            }
        }
        let start = std::time::Instant::now();
        let out = render(&g, &Theme::none(), 140);
        // 실사용 환경보다 넉넉한 상한(개발/디버그 빌드 기준). 대형 그래프라 아예 못 그릴(`None`) 수도
        // 있지만, 어느 쪽이든 이 시간 안에 끝나야 한다 — 패닉도, 무한정 느려지는 것도 없어야 한다.
        assert!(start.elapsed().as_secs() < 10, "대형 그래프 렌더링이 {:?}나 걸림", start.elapsed());
        if let Some(lines) = out {
            assert!(!lines.is_empty());
        }
    }

    /// ER "정확히 1" 카디널리티(`CrowOne`)는 `╪`/`╫` 한 글자만 찍는다 — 그 글자 자체가 이미 선을
    /// 가로지르는 두 짧은 표식 모양이라, 두 번 찍으면 네 줄로 겹쳐 보였다(회귀).
    #[test]
    fn crow_one_marker_is_a_single_glyph_not_doubled() {
        let mut g = Graph::default();
        let a = g.intern("A", "A", Shape::Rect, None);
        let b = g.intern("B", "B", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, tail: Marker::CrowOne, head: Marker::CrowOne, ..Edge::default() });
        let lines = render(&g, &Theme::none(), 80).unwrap();
        let text: Vec<String> = lines.iter().map(Line::plain).collect();
        let joined = text.join("\n");
        let crow_lines = text.iter().filter(|l| l.trim() == "╪" || l.trim() == "╫").count();
        assert_eq!(crow_lines, 2, "간선 양 끝에 각각 한 줄씩만 있어야 한다(합쳐서 2줄): {joined}");
    }

    /// (diagram-crow-foot-orientation) "many" 까치발은 벌어진 끝이 그 표식이 닿는 개체
    /// 쪽을 향해야 한다 — 뾰족한 끝이 개체를 향하면 방향이 뒤집힌 것(화살표와 반대 관례).
    #[test]
    fn crow_many_marker_opens_toward_the_node_it_touches() {
        // top-down: 아래 개체 쪽(at_top=false)은 아래로 벌어지는 `∧`, 위 개체 쪽
        // (at_top=true)은 위로 벌어지는 `∨`.
        assert_eq!(marker_glyphs(Marker::CrowMany, Direction::TopDown, false), vec!['╪', '∧']);
        assert_eq!(marker_glyphs(Marker::CrowMany, Direction::TopDown, true), vec!['╪', '∨']);
        // left-right: 왼쪽 개체 쪽(at_top=true)은 왼쪽으로 벌어지는 `>`, 오른쪽 개체 쪽
        // (at_top=false)은 오른쪽으로 벌어지는 `<`.
        assert_eq!(marker_glyphs(Marker::CrowMany, Direction::LeftRight, true), vec!['╫', '>']);
        assert_eq!(marker_glyphs(Marker::CrowMany, Direction::LeftRight, false), vec!['╫', '<']);
    }

    /// (diagram-lr-tail-label-position) 가로(LR) 방향에서 같은 레이어에 폭이 다른 두 개체가
    /// 있을 때, from측 라벨(관계 이름·카디널리티/다중성)은 각자 자기 노드 경계에서 같은
    /// 간격만큼 떨어져야 한다 — 레이어에서 가장 넓은 노드 기준 위치를 공유해 좁은 노드 쪽
    /// 라벨만 멀리 떨어지면 안 된다.
    #[test]
    fn left_right_tail_label_hugs_its_own_node_not_the_widest_sibling() {
        let mut g = Graph::default();
        let a = g.intern("A", "A", Shape::Rect, None);
        let b = g.intern("B", "B", Shape::Rect, None);
        let long = g.intern("LONG", "LONG_ENTITY_NAME_HERE", Shape::Rect, None);
        let c = g.intern("C", "C", Shape::Rect, None);
        g.direction = Some(Direction::LeftRight);
        g.add_edge(Edge { from: a, to: b, head: Marker::Arrow, label: "aaa".into(), tail_label: "1".into(), head_label: "9".into(), ..Edge::default() });
        g.add_edge(Edge { from: long, to: c, head: Marker::Arrow, label: "bbb".into(), tail_label: "1".into(), head_label: "9".into(), ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 120).unwrap());
        let text = out.join("\n");

        // 문자 열(칸) 기준으로 비교한다 — 상자 그림 문자는 UTF-8 바이트로는 3바이트라
        // `str::find`(바이트 오프셋)를 그대로 쓰면 칸 수와 어긋난다.
        let char_col = |row: &str, needle: char| row.chars().position(|c| c == needle).unwrap_or_else(|| panic!("{needle:?} not found in row {row:?}\n{text}"));
        let nth_border_col = |row: &str, n: usize| {
            row.chars().enumerate().filter(|(_, c)| *c == '│').nth(n).map(|(i, _)| i).unwrap_or_else(|| panic!("{n}번째 │ not found in row {row:?}\n{text}"))
        };

        let a_row = out.iter().position(|l| l.contains(" A ")).unwrap_or_else(|| panic!("A not found: {text}"));
        let a_border = nth_border_col(&out[a_row], 1);
        let a_tail_col = char_col(&out[a_row - 1], '1');

        let long_row = out.iter().position(|l| l.contains("LONG_ENTITY_NAME_HERE")).unwrap_or_else(|| panic!("LONG not found: {text}"));
        let long_border = nth_border_col(&out[long_row], 1);
        let long_tail_col = char_col(&out[long_row - 1], '1');

        let a_gap = a_tail_col as isize - a_border as isize;
        let long_gap = long_tail_col as isize - long_border as isize;
        assert_eq!(a_gap, long_gap, "from측 라벨은 자기 노드 경계에서 같은 간격만큼 떨어져야 한다(A={a_gap}, LONG={long_gap}):\n{text}");
        assert!(a_gap <= 3, "from측 라벨이 노드 경계에서 너무 멀리 떨어져 있다(gap={a_gap}):\n{text}");
    }

    /// (diagram-lr-label-marker-overlap) LR에서 from측 표식이 두 글자(까치발 `CrowZeroMany`
    /// 등)일 때, 관계 라벨(`label`)이 표식의 두 번째 글자를 덮어써 지우면 안 된다 — 둘 다 같은
    /// `segment.exit` 줄을 쓰기 때문에 라벨 시작 칸이 표식 폭을 건너뛰어야 한다. 표식이 한
    /// 글자인 흔한 경우(카디널리티 간격)는 건드리지 않는다 — 사용자가 그 간격은 기존(top+1)이
    /// 맞다고 확인했다.
    #[test]
    fn left_right_label_does_not_overwrite_a_two_glyph_tail_marker() {
        let mut g = Graph::default();
        let a = g.intern("A", "A", Shape::Rect, None);
        let b = g.intern("B", "B", Shape::Rect, None);
        g.direction = Some(Direction::LeftRight);
        // CrowZeroMany 꼬리 표식은 두 글자(╫/○, > 계열)를 쓴다 — 둘 다 살아 있어야 한다.
        g.add_edge(Edge { from: a, to: b, tail: Marker::CrowZeroMany, label: "places".into(), ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");

        let expected: Vec<char> = marker_glyphs(Marker::CrowZeroMany, Direction::LeftRight, true);
        assert_eq!(expected.len(), 2, "이 테스트는 두 글자짜리 표식을 전제로 한다: {expected:?}");
        let a_row = out.iter().position(|l| l.contains(" A ")).unwrap_or_else(|| panic!("A not found: {text}"));
        for glyph in &expected {
            assert!(out[a_row].contains(*glyph), "표식 글자 {glyph:?}가 라벨에 덮여 사라졌다: {text}");
        }
        assert!(text.contains("places"), "관계 라벨 자체도 그대로 보여야 한다: {text}");
    }

    /// (diagram-lr-tail-label-border-gap) tail_label(카디널리티 등)은 표식보다 한 줄 위에
    /// 그려져 표식과 같은 칸을 다투지 않으므로, 표식 글자 수와 무관하게 노드 경계 바로
    /// 다음 칸에 붙어야 한다(사용자 확인: 표식이 노드 테두리에 맞닿는 것과 같은 간격).
    #[test]
    fn left_right_tail_label_hugs_the_border_directly() {
        let mut g = Graph::default();
        let a = g.intern("A", "A", Shape::Rect, None);
        let b = g.intern("B", "B", Shape::Rect, None);
        g.direction = Some(Direction::LeftRight);
        g.add_edge(Edge { from: a, to: b, tail: Marker::CrowOne, tail_label: "1".into(), ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");

        let a_row = out.iter().position(|l| l.contains(" A ")).unwrap_or_else(|| panic!("A not found: {text}"));
        let a_border = out[a_row].chars().enumerate().filter(|(_, c)| *c == '│').nth(1).map(|(i, _)| i).unwrap_or_else(|| panic!("A border not found: {text}"));
        let tail_col = out[a_row - 1].chars().position(|c| c == '1').unwrap_or_else(|| panic!("tail label not found above its row: {text}"));
        // 경계 칸 바로 다음 칸(빈 칸 없이)에 붙어야 한다 — 표식이 몇 글자든 상관없다.
        assert_eq!(tail_col as isize - a_border as isize, 1, "tail_label이 노드 경계에 바로 붙지 않았다: {text}");
    }

    /// (diagram-lr-tail-label-border-gap) head_label(카디널리티 등)도 tail_label과
    /// 대칭이다 — 표식과 다른 줄에 그려지므로 to측 노드 경계 바로 앞 칸에 붙어야 한다.
    #[test]
    fn left_right_head_label_hugs_the_border_directly() {
        let mut g = Graph::default();
        let a = g.intern("A", "A", Shape::Rect, None);
        let b = g.intern("B", "B", Shape::Rect, None);
        g.direction = Some(Direction::LeftRight);
        g.add_edge(Edge { from: a, to: b, head: Marker::CrowOne, head_label: "9".into(), ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");

        let b_row = out.iter().position(|l| l.contains(" B ")).unwrap_or_else(|| panic!("B not found: {text}"));
        // " B " 바로 앞 칸이 B의 왼쪽 경계(│)다 — A도 같은 줄에 있을 수 있어 첫 │는 A의
        // 것일 수 있으므로, B의 것을 " B " 위치로부터 역산한다.
        let chars: Vec<char> = out[b_row].chars().collect();
        let b_text_at = chars.windows(3).position(|w| w == [' ', 'B', ' ']).unwrap_or_else(|| panic!("\" B \" not found: {text}"));
        let b_border = b_text_at - 1;
        assert_eq!(chars[b_border], '│', "B 왼쪽 경계 위치 계산이 틀렸다: {text}");
        let head_col = out[b_row - 1].chars().position(|c| c == '9').unwrap_or_else(|| panic!("head label not found above its row: {text}"));
        // 경계 칸 바로 앞 칸(빈 칸 없이)에 붙어야 한다.
        assert_eq!(b_border as isize - head_col as isize, 1, "head_label이 노드 경계에 바로 붙지 않았다: {text}");
    }

    // ── 레인(bpmn-lane-layout) ────────────────────────────────────────

    /// 요구사항 1.1, 1.4: 최상위 레인 A(노드가 층 0에만)와 레인 B(층 0~2)가 흐름 방향으로
    /// 같은 범위를 관통하며 좌우로 나란히 그려진다.
    #[test]
    fn top_level_lanes_span_the_full_diagram_depth_side_by_side() {
        let mut g = Graph::default();
        let lane_a = g.add_lane("A", None);
        let lane_b = g.add_lane("B", None);
        g.intern("a1", "a1", Shape::Rect, Some(lane_a));
        let b1 = g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        let b2 = g.intern("b2", "b2", Shape::Rect, Some(lane_b));
        let b3 = g.intern("b3", "b3", Shape::Rect, Some(lane_b));
        g.add_edge(Edge { from: b1, to: b2, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: b2, to: b3, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert_eq!(out[0].matches('┌').count(), 2, "두 레인의 위 테두리가 같은 줄에서 시작해야 한다: {text}");
        // A(왼쪽)는 노드가 층 0에만 있어도 B(층 0~2)와 같은 범위까지 관통해야 하므로, 맨 아래
        // 줄 맨 왼쪽 칸이 A 자신의 아래 테두리(└)여야 한다.
        assert!(out.last().unwrap().starts_with('└'), "A의 아래 테두리가 캔버스 맨 아래 줄까지 이어져야 한다: {text}");
    }

    /// 요구사항 1.3: 레인 안에 중첩된 레인 상자는 부모 레인의 흐름 방향 범위 전체를 관통한다
    /// (부모·자식 레인의 아래 테두리가 같은 줄에 있다 — 자식의 실제 구성원은 첫 층뿐이어도).
    #[test]
    fn nested_lane_covers_the_same_range_as_its_parent() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let sub = g.add_lane("Sub", Some(pool));
        let s1 = g.intern("s1", "s1", Shape::Rect, Some(sub));
        let s2 = g.intern("s2", "s2", Shape::Rect, Some(pool));
        g.add_edge(Edge { from: s1, to: s2, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        // Sub의 테두리가 Pool의 아래 테두리와 정확히 같은 줄에서 만나면 그 칸이 분기 문자(┴)로
        // 이어진다 — 다른 줄에서 따로 닫혔다면 이 문자가 나오지 않는다.
        assert!(out.last().unwrap().contains('┴'), "Sub의 아래 테두리가 Pool과 같은 줄에서 만나야 한다(┴): {text}");
    }

    /// 요구사항 2.1, 2.2, 2.3: 부모 레인 안의 형제 레인은 경계 한 열을 공유하고(││ 없음),
    /// 부모 테두리와 만나는 칸이 분기 문자(┬·┴)이며, 첫/마지막 자식의 바깥 테두리가 부모
    /// 테두리와 같은 열이다.
    #[test]
    fn sibling_lanes_under_a_pool_share_one_border_with_branch_glyphs() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane_a = g.add_lane("A", Some(pool));
        let lane_b = g.add_lane("B", Some(pool));
        g.intern("a1", "a1", Shape::Rect, Some(lane_a));
        g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        assert!(!text.contains("││"), "형제 레인 사이 경계가 두 줄(││)이면 안 된다: {text}");
        // 형제 레인의 경계가 부모 테두리와 만나는 칸이 분기 문자로 이어진다(부모 자신의 맨
        // 윗줄이 아니라, A·B 자신의 배너 테두리가 Pool 테두리와 만나는 줄에서).
        assert!(text.contains('┬'), "부모 테두리와 만나는 칸에 ┬가 있어야 한다: {text}");
        assert!(text.contains('┴'), "부모 테두리와 만나는 칸에 ┴가 있어야 한다: {text}");
        let pool_left = char_col(&out[0], '┌');
        let pool_right = char_col(&out[0], '┐');
        let a_title_row = out.iter().position(|l| l.contains('A')).unwrap_or_else(|| panic!("A not found: {text}"));
        let a_left = char_col(&out[a_title_row], '│');
        assert_eq!(a_left, pool_left, "첫 자식(A)의 왼쪽 테두리가 부모 테두리와 같은 열이어야 한다: {text}");
        let b_title_row = out.iter().position(|l| l.contains('B')).unwrap_or_else(|| panic!("B not found: {text}"));
        let b_right = char_rcol(&out[b_title_row], '│');
        assert_eq!(b_right, pool_right, "마지막 자식(B)의 오른쪽 테두리가 부모 테두리와 같은 열이어야 한다: {text}");
    }

    /// 요구사항 1.3, 2.3: 풀 > 레인 > 하위 레인 3단 중첩의 아랫변이 모두 같은 줄에 있고,
    /// 배너 다음 구분선이 한 줄만 있다(중첩 단계마다 가로선이 늘어나는 이중 구분선 없음).
    #[test]
    fn three_level_lane_nesting_has_no_doubled_divider() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane = g.add_lane("Lane", Some(pool));
        let sub = g.add_lane("SubLane", Some(lane));
        g.intern("s1", "s1", Shape::Rect, Some(sub));
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let bottom_lines = out.iter().filter(|l| l.contains('└')).count();
        assert_eq!(bottom_lines, 1, "3단 중첩의 아랫변이 모두 같은 줄에 있어야 한다: {text}");
        let sub_title_row = out.iter().position(|l| l.contains("SubLane")).unwrap_or_else(|| panic!("SubLane not found: {text}"));
        assert!(
            out[sub_title_row + 1].contains('┌'),
            "SubLane 제목 줄 바로 다음이 구분선 겸 s1 상자 윗변이어야 한다(이중 구분선 없음): {text}"
        );
    }

    /// 요구사항 4.1, 4.6: TB에서 레인 제목 줄이 첫 노드 줄보다 위이고 테두리 선(─)과 겹치지
    /// 않는다. 빈 제목 레인은 배너 자리는 유지하되 글자를 쓰지 않는다(패닉도 없다).
    #[test]
    fn tb_lane_title_gets_its_own_banner_row_without_dashes() {
        let mut g = Graph::default();
        let lane = g.add_lane("Sales", None);
        let empty_lane = g.add_lane("", None);
        g.intern("n1", "n1", Shape::Rect, Some(lane));
        g.intern("n2", "n2", Shape::Rect, Some(empty_lane));
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let title_row = out.iter().position(|l| l.contains("Sales")).unwrap_or_else(|| panic!("title not found: {text}"));
        let node_row = out.iter().position(|l| l.contains(" n1 ")).unwrap_or_else(|| panic!("node not found: {text}"));
        assert!(title_row < node_row, "제목 줄이 노드 줄보다 위여야 한다: {text}");
        assert!(!out[title_row].contains('─'), "제목 줄에 테두리 선이 없어야 한다: {text}");
    }

    /// 요구사항 4.2: LR에서 제목이 노드 내용 왼쪽의 전용 열에 가로쓰기로 쓰이고 잘리지 않는다.
    #[test]
    fn left_right_lane_title_gets_its_own_banner_column_without_truncation() {
        let mut g = Graph::default();
        let lane = g.add_lane("Longish Lane Title", None);
        g.intern("n", "n", Shape::Rect, Some(lane));
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        assert!(text.contains("Longish Lane Title"), "LR 레인 제목은 잘리지 않아야 한다: {text}");
        let title_row = out.iter().position(|l| l.contains("Longish Lane Title")).unwrap();
        let node_row = out.iter().position(|l| l.contains(" n ")).unwrap_or_else(|| panic!("node not found: {text}"));
        let node_top_row = out[..node_row].iter().rposition(|l| l.contains('┌')).unwrap_or_else(|| panic!("node top border not found: {text}"));
        let title_col = char_col(&out[title_row], 'L');
        let node_col = char_col(&out[node_top_row], '┌');
        assert!(title_col < node_col, "LR 제목 열이 노드 왼쪽 테두리보다 왼쪽이어야 한다: {text}");
    }

    /// 요구사항 4.3: 자식 레인의 배너가 부모 레인의 배너 다음에 오며 두 제목이 겹치지 않는다.
    #[test]
    fn nested_lane_banners_stack_without_overlapping_titles() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane = g.add_lane("Lane", Some(pool));
        g.intern("n", "n", Shape::Rect, Some(lane));
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let pool_row = out.iter().position(|l| l.contains("Pool")).unwrap_or_else(|| panic!("Pool not found: {text}"));
        let lane_row = out.iter().position(|l| l.contains("Lane")).unwrap_or_else(|| panic!("Lane not found: {text}"));
        assert!(pool_row < lane_row, "풀 제목이 레인 제목보다 앞 줄에 있어야 한다: {text}");
    }

    /// 요구사항 4.4: 같은 깊이의 형제 레인 제목 길이가 서로 달라도 모든 형제의 노드 내용이
    /// 같은 위치에서 시작한다.
    #[test]
    fn sibling_lane_titles_of_different_length_still_align_content_start() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let short = g.add_lane("X", Some(pool));
        let long = g.add_lane("A Very Long Title Indeed", Some(pool));
        g.intern("n1", "n1", Shape::Rect, Some(short));
        g.intern("n2", "n2", Shape::Rect, Some(long));
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        let row1 = out.iter().position(|l| l.contains(" n1 ")).unwrap_or_else(|| panic!("n1 not found: {text}"));
        let row2 = out.iter().position(|l| l.contains(" n2 ")).unwrap_or_else(|| panic!("n2 not found: {text}"));
        assert_eq!(row1, row2, "형제 레인의 첫 노드는 제목 길이와 무관하게 같은 줄에서 시작해야 한다: {text}");
    }

    /// 요구사항 4.5: 위→아래 방향 + 제목이 구성원 노드보다 넓으면 레인이 제목 폭만큼 넓어지고
    /// 제목이 잘리지 않는다(기존 그룹 제목과 같은 규약).
    #[test]
    fn tb_wide_lane_title_widens_the_lane_instead_of_truncating() {
        let mut g = Graph::default();
        let lane = g.add_lane("A Very Long Lane Title", None);
        g.intern("n", "n", Shape::Rect, Some(lane));
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert!(text.contains("A Very Long Lane Title"), "긴 제목이 잘리면 안 된다: {text}");
    }

    /// 요구사항 2.4: 형제 레인을 A, B, C 순서로 선언 + C→A 간선만 있어도(무게중심 정렬이
    /// 다른 순서를 선호할 수 있어도) A, B, C 선언 순서 그대로 그려진다.
    #[test]
    fn lane_children_keep_declaration_order_even_when_an_edge_pulls_otherwise() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane_a = g.add_lane("A", Some(pool));
        let lane_b = g.add_lane("B", Some(pool));
        let lane_c = g.add_lane("C", Some(pool));
        let a1 = g.intern("a1", "a1", Shape::Rect, Some(lane_a));
        g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        let c1 = g.intern("c1", "c1", Shape::Rect, Some(lane_c));
        g.add_edge(Edge { from: c1, to: a1, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        let title_row = out.iter().position(|l| l.contains('A') && l.contains('B') && l.contains('C')).unwrap_or_else(|| panic!("A, B, C 배너 줄을 못 찾음: {text}"));
        let pos_a = out[title_row].find('A').unwrap();
        let pos_b = out[title_row].find('B').unwrap();
        let pos_c = out[title_row].find('C').unwrap();
        assert!(pos_a < pos_b && pos_b < pos_c, "선언 순서 A, B, C가 유지돼야 한다: {text}");
    }

    /// 요구사항 2.5: 형제 레인 + 폭 초과로 층 접기 폴백이 일어나도 레인은 접히지 않고 나란한
    /// 띠를 유지하며, 레인 안의 노드만 접힌다.
    #[test]
    fn narrow_width_folds_nodes_inside_a_lane_without_stacking_the_lanes() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane_a = g.add_lane("A", Some(pool));
        let lane_b = g.add_lane("B", Some(pool));
        // A 안에 나란한 노드를 여럿 둬(간선 없음, 모두 층0) 위→아래에서 폭이 넘치게 한다.
        for i in 0..4 {
            g.intern(&format!("a{i}"), &format!("a{i}"), Shape::Rect, Some(lane_a));
        }
        // B 안에는 층이 깊은 사슬을 둬(왼쪽→오른쪽 재시도도 폭이 넘치게) `render()`가 반대
        // 방향으로 피하지 못하고 반드시 위→아래 + 층 접기로 떨어지게 만든다.
        let mut prev = g.intern("b0", "b0", Shape::Rect, Some(lane_b));
        for i in 1..6 {
            let next = g.intern(&format!("b{i}"), &format!("b{i}"), Shape::Rect, Some(lane_b));
            g.add_edge(Edge { from: prev, to: next, head: Marker::Arrow, ..Edge::default() });
            prev = next;
        }
        let out = rows(render(&g, &Theme::none(), 40).unwrap());
        let text = out.join("\n");
        // 접기가 실제로 일어났는지 확인: 간선 없이 원래 모두 층 0인 a0~a3가 좁은 폭 때문에
        // 서로 다른 줄(층)로 접혔어야 한다.
        let a_rows: std::collections::BTreeSet<usize> =
            (0..4).map(|i| out.iter().position(|l| l.contains(&format!(" a{i} "))).unwrap_or_else(|| panic!("a{i} not found: {text}"))).collect();
        assert!(a_rows.len() > 1, "레인 A 안의 노드들이 층 접기로 서로 다른 줄에 놓여야 한다: {text}");
        // 레인 A·B 자신은 접히지 않고 여전히 좌우로 나란해야 한다(배너가 같은 줄).
        let a_row = out.iter().position(|l| l.contains('A')).unwrap();
        let b_row = out.iter().position(|l| l.contains('B')).unwrap();
        assert_eq!(a_row, b_row, "A·B 배너가 여전히 같은 줄(좌우 나란)이어야 한다: {text}");
    }

    /// 요구사항 3.1: A 안에 a1 뒤로 a2, a3가 더 있어도, a1에서 B로 가는 간선의 도착 노드는
    /// a1의 바로 다음 층에 놓이고 A의 마지막 노드(a3) 뒤로 밀리지 않는다.
    #[test]
    fn cross_lane_successor_lands_right_after_its_source_layer() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane_a = g.add_lane("A", Some(pool));
        let lane_b = g.add_lane("B", Some(pool));
        let a1 = g.intern("a1", "a1", Shape::Rect, Some(lane_a));
        let a2 = g.intern("a2", "a2", Shape::Rect, Some(lane_a));
        let a3 = g.intern("a3", "a3", Shape::Rect, Some(lane_a));
        let b1 = g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        g.add_edge(Edge { from: a1, to: a2, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: a2, to: a3, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: a1, to: b1, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        let row_a2 = out.iter().position(|l| l.contains(" a2 ")).unwrap_or_else(|| panic!("a2 not found: {text}"));
        let row_b1 = out.iter().position(|l| l.contains(" b1 ")).unwrap_or_else(|| panic!("b1 not found: {text}"));
        assert_eq!(row_a2, row_b1, "b1이 a2와 같은 줄에 있어야 한다(A의 마지막 뒤로 밀리면 안 됨): {text}");
    }

    /// 요구사항 3.2: 레인 경계를 넘는 간선의 선이 끊기지 않고 화살촉이 도착 노드 바로 위에
    /// 붙는다.
    #[test]
    fn edge_crossing_lane_boundary_stays_unbroken_with_arrowhead_at_target() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane_a = g.add_lane("A", Some(pool));
        let lane_b = g.add_lane("B", Some(pool));
        let a1 = g.intern("a1", "a1", Shape::Rect, Some(lane_a));
        let b1 = g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        g.add_edge(Edge { from: a1, to: b1, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        assert!(text.contains('▼'), "화살촉이 있어야 한다: {text}");
        let b1_row = out.iter().position(|l| l.contains(" b1 ")).unwrap_or_else(|| panic!("b1 not found: {text}"));
        // 상자는 (윗변, 이름 줄, 아랫변) 3줄이므로 화살촉은 이름 줄에서 두 줄 위(윗변 바로 위)다.
        assert!(out[b1_row - 2].contains('▼'), "화살촉이 도착 노드 상자 바로 위 줄에 있어야 한다: {text}");
    }

    /// 요구사항 3.3: 레인 밖 노드(최상위)는 레인 상자 바깥에 그려지고 패닉이 없다.
    #[test]
    fn node_outside_a_lane_is_drawn_outside_the_lane_box() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane = g.add_lane("Lane", Some(pool));
        let sub = g.add_lane("SubLane", Some(lane));
        let c1 = g.intern("c1", "c1", Shape::Rect, Some(sub));
        let d1 = g.intern("d1", "d1", Shape::Rect, None);
        g.add_edge(Edge { from: c1, to: d1, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let pool_right = char_rcol(&out[0], '┐');
        let d1_row = out.iter().position(|l| l.contains(" d1 ")).unwrap_or_else(|| panic!("d1 not found: {text}"));
        let d1_col = char_col(&out[d1_row], 'd');
        assert!(d1_col > pool_right, "레인 밖 노드는 레인 상자 오른쪽 바깥에 그려져야 한다: {text}");
    }

    /// 요구사항 5.1, 5.2: 방향 지정 없음(기본 위→아래)이면 레인이 좌우로, `LeftRight`면
    /// 레인이 위아래로 쌓인다.
    #[test]
    fn unspecified_direction_places_lanes_side_by_side_left_right_stacks_them() {
        let build = |direction: Option<Direction>| {
            let mut g = Graph::default();
            let pool = g.add_lane("Pool", None);
            let lane_a = g.add_lane("A", Some(pool));
            let lane_b = g.add_lane("B", Some(pool));
            g.intern("a1", "a1", Shape::Rect, Some(lane_a));
            g.intern("b1", "b1", Shape::Rect, Some(lane_b));
            g.direction = direction;
            g
        };
        let tb = rows(render(&build(None), &Theme::none(), 100).unwrap());
        let tb_text = tb.join("\n");
        let a_row = tb.iter().position(|l| l.contains('A')).unwrap_or_else(|| panic!("A not found: {tb_text}"));
        let b_row = tb.iter().position(|l| l.contains('B')).unwrap_or_else(|| panic!("B not found: {tb_text}"));
        assert_eq!(a_row, b_row, "TB에서 A·B 배너가 같은 줄(좌우로 나란)이어야 한다: {tb_text}");

        let lr = rows(render(&build(Some(Direction::LeftRight)), &Theme::none(), 100).unwrap());
        let lr_text = lr.join("\n");
        let a_row_lr = lr.iter().position(|l| l.contains('A')).unwrap_or_else(|| panic!("A not found: {lr_text}"));
        let b_row_lr = lr.iter().position(|l| l.contains('B')).unwrap_or_else(|| panic!("B not found: {lr_text}"));
        assert!(a_row_lr < b_row_lr, "LR에서 A가 B보다 위(위아래로 쌓임)여야 한다: {lr_text}");
    }

    /// 요구사항 5.3: 지정 폭을 초과하면 기존 규약대로 `None`을 돌려준다.
    #[test]
    fn lane_graph_too_wide_returns_none() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool with a very long title indeed", None);
        let lane = g.add_lane("Lane with an equally long title", Some(pool));
        g.intern("n", "wide node label here", Shape::Rect, Some(lane));
        assert!(render(&g, &Theme::none(), 8).is_none());
    }

    /// 요구사항 6.2: 3단 중첩·구성원 없는 레인·레인 간 되돌아가는 간선이 섞여도 패닉이 없다.
    #[test]
    fn deep_nesting_empty_lane_and_backward_cross_lane_edge_do_not_panic() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane = g.add_lane("Lane", Some(pool));
        let sub = g.add_lane("SubLane", Some(lane));
        let _empty = g.add_lane("Empty", Some(pool));
        let s1 = g.intern("s1", "s1", Shape::Rect, Some(sub));
        let lane_b = g.add_lane("B", Some(pool));
        let b1 = g.intern("b1", "b1", Shape::Rect, Some(lane_b));
        g.add_edge(Edge { from: s1, to: b1, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: b1, to: s1, head: Marker::Arrow, kind: LineKind::Dashed, ..Edge::default() });
        let out = render(&g, &Theme::none(), 100);
        assert!(out.is_some(), "패닉 없이 렌더링돼야 한다");
    }

    // ── BPMN 도형·표식(bpmn-shapes) ──────────────────────────────────

    /// 요구사항 4.1, 4.2: `Marker::Slash`는 방향·위치와 무관하게 한 글자다
    /// (`Circle`/`Cross`와 같은 패턴 — `crow_one_marker_is_a_single_glyph_not_doubled`와 대응).
    #[test]
    fn slash_marker_is_a_single_glyph_regardless_of_direction() {
        for direction in [Direction::TopDown, Direction::LeftRight] {
            for at_top in [true, false] {
                assert_eq!(marker_glyphs(Marker::Slash, direction, at_top), vec!['╱']);
            }
        }
    }

    /// 요구사항 4.1: TB에서 default 흐름 꼬리 빗금이 출발 노드 테두리 바로 아래 줄, 같은
    /// 열에 놓인다.
    #[test]
    fn slash_tail_marker_sits_right_below_the_source_node_in_top_down() {
        let mut g = Graph::default();
        let a = g.intern("a", "a", Shape::Rect, None);
        let b = g.intern("b", "b", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, tail: Marker::Slash, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let a_text_row = row_of(&out, " a ");
        let a_col = char_col(&out[a_text_row], 'a');
        let exit_row = a_text_row + 2; // 테두리 바로 아래 줄(요구사항 4.1)
        assert_eq!(char_col(&out[exit_row], '╱'), a_col, "a 테두리 바로 아래, a와 같은 열에 빗금이 있어야 한다: {text}");
    }

    /// 요구사항 4.2: LR에서는 출발 노드 오른쪽 테두리 바로 다음 칸에 빗금이 놓인다.
    #[test]
    fn slash_tail_marker_sits_right_after_the_source_node_in_left_right() {
        let mut g = Graph::default();
        let a = g.intern("a", "a", Shape::Rect, None);
        let b = g.intern("b", "b", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, tail: Marker::Slash, head: Marker::Arrow, ..Edge::default() });
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let a_text_row = row_of(&out, " a ");
        let a_col = char_col(&out[a_text_row], 'a');
        // a letter 다음의 첫 '│'가 a 자신의 오른쪽 테두리다(char_rcol은 b의 오른쪽 테두리까지
        // 집어버려 못 쓴다 — 한 줄에 두 노드의 테두리가 같이 있다).
        let a_right_border = out[a_text_row].chars().enumerate().skip(a_col + 1).find(|&(_, c)| c == '│').map(|(i, _)| i).expect("a 오른쪽 테두리가 있어야 한다");
        assert_eq!(char_col(&out[a_text_row], '╱'), a_right_border + 1, "a 오른쪽 바로 다음 칸에 빗금이 있어야 한다: {text}");
    }

    /// 요구사항 4.3: 빗금 꼬리 + 화살촉 머리 + 라벨을 함께 쓰면 라벨이 빗금을 덮어쓰지 않고
    /// 그 다음 칸부터 시작하며, 화살촉은 도착 노드에 그대로 붙는다.
    #[test]
    fn slash_tail_does_not_get_overwritten_by_a_label_in_left_right() {
        let mut g = Graph::default();
        let a = g.intern("a", "a", Shape::Rect, None);
        let b = g.intern("b", "b", Shape::Rect, None);
        g.add_edge(Edge { from: a, to: b, tail: Marker::Slash, head: Marker::Arrow, label: "no".into(), ..Edge::default() });
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let a_text_row = row_of(&out, " a ");
        let slash_col = char_col(&out[a_text_row], '╱');
        let label_col = char_col(&out[a_text_row], 'n');
        assert_eq!(label_col, slash_col + 1, "라벨이 빗금 다음 칸부터 시작해야 한다: {text}");
        assert!(text.contains('▶'), "화살촉이 그대로 그려져야 한다: {text}");
    }

    /// 요구사항 2.6: TB에서 가는 선 노드(`Round`)는 기존 접점 규칙(테두리 바로 위 줄)을
    /// 그대로 따르고, 이벤트 노드(상자 없음)는 위치 글자 줄 바로 위 줄에 화살촉이 붙는다.
    #[test]
    fn event_shapes_attach_edges_using_existing_contact_rules_top_down() {
        let mut g = Graph::default();
        let start = g.intern("start", "Go", Shape::Event(EventPosition::Start), None);
        let mid = g.intern("mid", "Mid", Shape::Round, None);
        let end = g.intern("end", "Done", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: start, to: mid, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: mid, to: end, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        // Round는 테두리(위)·본문(텍스트 줄)·테두리(아래) 세 줄을 차지하므로, 화살촉은
        // 텍스트 줄에서 두 줄 위(테두리 바로 위 줄)에 있다.
        let mid_row = row_of(&out, "Mid");
        assert!(out[mid_row - 2].contains('▼'), "가는 선 노드 테두리 바로 위 줄에 화살촉이 붙어야 한다: {text}");
        // 종료 이벤트는 테두리가 없다 — `●` 글자 줄 바로 위 줄에, 이벤트가 차지한 열 범위 안에
        // 화살촉이 온다(design §수정 방식의 접점 한계 (a): 화살촉은 원 글자 열이 아니라 덩어리
        // 가운데 열 — 이름 위 — 에 올 수 있다).
        let end_row = row_of(&out, "●");
        let end_line: Vec<char> = out[end_row].chars().collect();
        let first_col = end_line.iter().position(|&c| c != ' ').expect("종료 이벤트 줄에 글자가 있어야 한다");
        let last_col = end_line.iter().rposition(|&c| c != ' ').expect("종료 이벤트 줄에 글자가 있어야 한다");
        let above: Vec<char> = out[end_row - 1].chars().collect();
        assert!((first_col..=last_col).any(|col| above.get(col) == Some(&'▼')), "이벤트가 차지한 열 범위 안, 바로 위 줄에 화살촉이 붙어야 한다: {text}");
    }

    /// 요구사항 2.6: LR에서 이벤트 노드는 상자 테두리가 아니라 위치 글자 바로 왼쪽 칸에
    /// 화살촉이 붙는다.
    #[test]
    fn event_shapes_attach_edges_using_existing_contact_rules_left_right() {
        let mut g = Graph::default();
        let start = g.intern("start", "Go", Shape::Event(EventPosition::Start), None);
        let end = g.intern("end", "Done", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: start, to: end, head: Marker::Arrow, ..Edge::default() });
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let end_row = row_of(&out, "●");
        let end_col = char_col(&out[end_row], '●');
        assert_eq!(out[end_row].chars().nth(end_col - 1), Some('▶'), "화살촉이 종료 이벤트의 위치 글자(●) 바로 왼쪽에 있어야 한다: {text}");
    }

    /// 요구사항 2.2: 서브프로세스 아래로 간선이 나가도 `[+]`가 지워지지 않고, 그 아래 줄에
    /// 선이 그려진다.
    #[test]
    fn subprocess_plus_marker_survives_an_outgoing_edge_below_it() {
        let mut g = Graph::default();
        let sub = g.intern("sub", "Sub", Shape::Subprocess, None);
        let next = g.intern("next", "Next", Shape::Rect, None);
        g.add_edge(Edge { from: sub, to: next, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let plus_row = row_of(&out, "[+]");
        let plus_col = char_col(&out[plus_row], '+');
        let line_row = &out[plus_row + 1];
        assert!(line_row.chars().nth(plus_col).is_some_and(|c| c == '│' || c == '▼'), "[+] 바로 아래 줄에 선이 있어야 한다: {text}");
    }

    /// 요구사항 6.2: 이름 없는 이벤트·한 글자·세 줄 본문·레인 안 배치·양방향·빗금 양끝
    /// 조합에 TB·LR 모두 패닉이 없다.
    #[test]
    fn new_shapes_and_slash_marker_do_not_panic_across_edge_cases() {
        let mut g = Graph::default();
        let lane = g.add_lane("Lane", None);
        let unnamed_start = g.intern("s0", "s0", Shape::Event(EventPosition::Start), Some(lane));
        g.set_label(unnamed_start, ""); // 이름 없는 이벤트: 본문 줄 없음
        let one_char = g.intern("s1", "X", Shape::Event(EventPosition::Intermediate), Some(lane));
        let three_line = g.intern("s2", "one\ntwo\nthree", Shape::Subprocess, Some(lane));
        let end = g.intern("s3", "Done", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: unnamed_start, to: one_char, tail: Marker::Slash, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: one_char, to: three_line, head: Marker::Arrow, ..Edge::default() });
        // 빗금 양끝: 꼬리·머리 둘 다 Slash.
        g.add_edge(Edge { from: three_line, to: end, tail: Marker::Slash, head: Marker::Slash, ..Edge::default() });
        // 양방향: 되돌아가는 간선.
        g.add_edge(Edge { from: end, to: unnamed_start, head: Marker::Arrow, kind: LineKind::Dashed, ..Edge::default() });
        assert!(render(&g, &Theme::none(), 100).is_some(), "TB에서 패닉 없이 렌더링돼야 한다");
        g.direction = Some(Direction::LeftRight);
        assert!(render(&g, &Theme::none(), 100).is_some(), "LR에서 패닉 없이 렌더링돼야 한다");
    }

    fn task_with_stereotype(g: &mut Graph, id: &str, name: &str, stereotype: &str, group: Option<usize>) -> usize {
        let index = g.intern(id, name, Shape::Round, group);
        g.nodes[index].sections[0] = vec![name.to_string(), format!("«{stereotype}»")];
        index
    }

    /// 요구사항 6.3: 손으로 만든 "주문 처리" 그래프(레인 2개, 시작/종료 이벤트, `«user»`/
    /// `«service»` 태스크, XOR 게이트웨이, 조건 라벨 흐름)가 LR 폭 100 안에서 렌더링되고,
    /// 1~5의 새 관례가 한 그림 안에 모두 드러난다. 파일을 파싱하지 않고 `Graph`를 직접 만든다.
    #[test]
    fn hand_built_order_processing_graph_renders_within_width_100() {
        let mut g = Graph { direction: Some(Direction::LeftRight), ..Graph::default() };
        let sales = g.add_lane("영업", None);
        let warehouse = g.add_lane("창고", None);

        let received = g.intern("received", "주문 접수", Shape::Event(EventPosition::Start), Some(sales));
        let review = task_with_stereotype(&mut g, "review", "주문 검토", "user", Some(sales));
        let check_stock = g.intern("check_stock", "× 재고 있음?", Shape::Diamond, Some(warehouse));
        let prepare = task_with_stereotype(&mut g, "prepare", "출고 준비", "user", Some(warehouse));
        let shipped_end = g.intern("shipped_end", "shipped_end", Shape::Event(EventPosition::End), Some(warehouse));
        g.set_label(shipped_end, ""); // 이름 없는 종료 이벤트
        let notify = task_with_stereotype(&mut g, "notify", "품절 안내 발송", "service", Some(sales));
        let cancel_end = g.intern("cancel_end", "품절 취소", Shape::Event(EventPosition::End), Some(sales));

        g.add_edge(Edge { from: received, to: review, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: review, to: check_stock, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: check_stock, to: prepare, head: Marker::Arrow, label: "예".into(), ..Edge::default() });
        g.add_edge(Edge { from: prepare, to: shipped_end, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: check_stock, to: notify, head: Marker::Arrow, label: "아니오".into(), ..Edge::default() });
        g.add_edge(Edge { from: notify, to: cancel_end, head: Marker::Arrow, ..Edge::default() });

        let lines = render(&g, &Theme::none(), 100);
        assert!(lines.is_some(), "폭 100 안에서 렌더링돼야 한다");
        let out = rows(lines.unwrap());
        let text = out.join("\n");
        for needle in ["○", "●", "«user»", "«service»", "× 재고 있음?", "예", "아니오"] {
            assert!(text.contains(needle), "{needle:?}가 결과에 있어야 한다:\n{text}");
        }
    }
}
