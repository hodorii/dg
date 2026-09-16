//! 계층 배치(Sugiyama)로 그래프를 그린다.
//!
//! 1. 되돌아가는 간선을 DFS로 뒤집어 DAG를 만들고 최장 경로로 층을 매긴다.
//! 2. 그룹을 블록으로 보고, 블록 안에서 자식들을 무게중심 순서로 겹치지 않게 늘어놓는다.
//!    블록은 여러 층에 걸쳐 같은 띠를 차지하므로 그룹 테두리가 서로 포개지지 않는다.
//! 3. 층 사이 "통로"에 간선의 가로 구간을 넣고, 겹치는 구간은 다른 줄을 쓴다.
//! 4. 방향(TB/LR)은 배치 좌표계(along/across)를 캔버스 좌표로 옮길 때만 관여한다.

use crate::diagram::canvas::{Canvas, EAST, LineKind, NORTH, SOUTH, WEST};
use crate::diagram::ir::{Direction, Graph, Marker};
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
    // 1) 있는 그대로: 원하는 방향 → 반대 방향, 라벨 폭을 줄여 가며
    // 2) 그래도 넓으면 위→아래 배치에서 넓은 층을 여러 줄로 접는다
    let attempts = caps
        .iter()
        .flat_map(|&cap| [(cap, preferred, false), (cap, preferred.other(), false)])
        .chain(caps.iter().map(|&cap| (cap, Direction::TopDown, true)));
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
}

const GAP_ALONG_MIN: usize = 3;
/// 층 접기 최대 횟수.
const MAX_FOLDS: usize = 40;

impl<'a> Layout<'a> {
    fn build(graph: &'a Graph, theme: &'a Theme, direction: Direction, cap: usize, width: usize, allow_fold: bool) -> Option<Canvas> {
        let mut layout = Layout::new(graph, theme, direction, cap);
        layout.assign_layers();
        layout.push_outputs_below_groups();
        let mut folds = 0;
        loop {
            layout.arrange();
            let total_across = layout.blocks[0].width.max(layout.extra_across);
            let total_along = layout.total_along();
            let (canvas_width, canvas_height) = match direction {
                Direction::TopDown => (total_across, total_along),
                Direction::LeftRight => (total_along, total_across),
            };
            if canvas_width == 0 {
                return None;
            }
            if canvas_width <= width {
                let mut canvas = Canvas::new(canvas_width, canvas_height);
                layout.draw(&mut canvas);
                return Some(canvas);
            }
            if !allow_fold || direction != Direction::TopDown || folds >= MAX_FOLDS || !layout.fold_widest_row() {
                return None;
            }
            folds += 1;
            layout.reset_arrangement();
        }
    }

    /// 층이 정해진 뒤의 배치 전 과정.
    fn arrange(&mut self) {
        self.make_segments();
        self.build_blocks();
        for _ in 0..4 {
            self.place();
            self.reorder();
        }
        self.place();
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

    /// 가장 넓은 "줄"(한 블록의 한 층에 나란히 놓인 자식들)의 맨 오른쪽 자식을 아래로 내린다.
    /// 노드면 한 층 아래로, 하위 그룹이면 통째로 나머지 자식들 아래로 옮긴다. 나눌 줄이 없으면 false.
    fn fold_widest_row(&mut self) -> bool {
        let mut widest: Option<(usize, usize, usize)> = None; // (폭, 블록, 층)
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
                if widest.is_none_or(|(w, _, _)| end - start > w) {
                    widest = Some((end - start, block, layer));
                }
            }
        }
        let Some((_, block, layer)) = widest else { return false };
        let mut row = self.row_children(block, layer);
        row.sort_by_key(|&c| self.child_span(c).0);
        let Some(&target) = row.iter().rev().find(|&&c| self.is_movable(c)) else { return false };
        match target {
            Child::Node(i) => self.min_layer[i] = self.min_layer[i].max(layer + 1),
            Child::Block(moved) => {
                let others_max = row
                    .iter()
                    .filter(|&&c| !matches!(c, Child::Block(b) if b == moved))
                    .map(|&c| self.child_layer_max(c))
                    .max()
                    .unwrap_or(layer);
                let offset = (others_max + 1).saturating_sub(self.blocks[moved].layer_min).max(1);
                for member in self.child_members(Child::Block(moved)) {
                    if self.lnodes[member].node.is_some() {
                        self.min_layer[member] = self.min_layer[member].max(self.lnodes[member].layer + offset);
                    }
                }
            }
        }
        self.assign_layers();
        true
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
            Child::Node(i) => self.lnodes[i].node.is_some(),
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
                let (along_size, across_size) = match direction {
                    Direction::TopDown => (h, w),
                    Direction::LeftRight => {
                        // 왼쪽→오른쪽에서는 간선이 나가는 줄마다 라벨이 붙으므로 두 줄 간격이 필요하다.
                        let (incoming, outgoing) = degree[index];
                        let needed = 2 * incoming.max(outgoing) + 1;
                        (w, if h >= 3 { h.max(needed) } else { h })
                    }
                };
                LayoutNode { node: Some(0), layer: 0, group: node.group, along_size, across_size, extra_across: 0, across: 0, sections }
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
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue;
            }
            let (from, to) = if self.reversed[i] { (edge.to, edge.from) } else { (edge.from, edge.to) };
            directed[from].push(to);
            incoming_count[to] += 1;
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
        self.layer_count = layer.iter().max().map_or(0, |m| m + 1);
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
            self.blocks[block].children = keyed.into_iter().map(|(_, c)| c).collect();
        }
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
        for s in 0..self.segments.len() {
            let (from, to) = (self.segments[s].from, self.segments[s].to);
            self.segments[s].exit = self.lnodes[from].across + self.segments[s].exit_offset;
            self.segments[s].entry = self.lnodes[to].across + self.segments[s].entry_offset;
        }

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
            intervals.sort();
            let mut channel_ends: Vec<usize> = Vec::new();
            for (low, high, s) in intervals {
                let channel = match channel_ends.iter().position(|&end| end + 2 <= low) {
                    Some(c) => c,
                    None => {
                        channel_ends.push(0);
                        channel_ends.len() - 1
                    }
                };
                channel_ends[channel] = high;
                self.segments[s].channel = Some(channel);
            }
            let channels = channel_ends.len();
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
        self.draw_groups(canvas);
        for (i, node) in self.lnodes.iter().enumerate() {
            let along = self.node_along(i);
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
        for s in 0..self.segments.len() {
            self.draw_segment(canvas, s);
        }
        for s in 0..self.segments.len() {
            self.draw_segment_decorations(canvas, s);
        }
        self.draw_self_loops(canvas);
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
