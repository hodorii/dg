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
use crate::text::{char_width, truncate, width_of};
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Clone)]
struct LayoutNode {
    node: Option<usize>,
    /// 가상 노드가 속한 간선.
    edge: Option<usize>,
    layer: usize,
    group: Option<usize>,
    along_size: usize,
    /// 메시지 끝 칸 때문에 늘리기 전의 흐름축 크기. 배치를 다시 할 때마다 여기서부터 다시 늘린다.
    content_along_size: usize,
    across_size: usize,
    /// 자기 자신 간선처럼 상자 옆에 더 필요한 칸.
    extra_across: usize,
    across: usize,
    sections: Vec<Vec<String>>,
    /// 접점·정렬 기준이 상자 가운데가 아니라 원점 칸인지(`Shape::is_point_anchored()`,
    /// 가상 노드는 항상 false) — `center()`·`spread()`·크기 계산·`resolve_port_swaps()`가
    /// 함께 참조한다.
    point_anchored: bool,
}

impl LayoutNode {
    fn footprint(&self) -> usize {
        self.across_size + self.extra_across
    }
    fn center(&self) -> f64 {
        if self.point_anchored {
            return self.across as f64;
        }
        self.across as f64 + self.across_size as f64 / 2.0
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
struct Block {
    group: Option<usize>,
    children: Vec<Child>,
    start: usize,
    width: usize,
    layer_min: usize,
    layer_max: usize,
    registered: bool,
}

#[derive(Clone)]
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
    /// 노드별 나가는/들어오는 구간 번호(구간 색인 오름차순) — `make_segments()` 끝에서 만든다.
    /// `assign_ports()`·`straighten_layer()`가 전체 구간을 훑는 대신 이 목록을 읽는다(결과
    /// 동일 변환: 원래 `segments.iter().filter(...)`와 원소·순서가 같다).
    outgoing_of: Vec<Vec<usize>>,
    incoming_of: Vec<Vec<usize>>,
    /// (d) 층별 구성원 색인(노드 색인 오름차순) — `make_segments()` 끝에서 만든다.
    /// `straighten_layer()`가 매번 `0..lnodes.len()`을 훑어 층을 거르는 대신 이걸 복제해 쓴다
    /// (원소가 원래 필터와 같으므로 `across`로 다시 정렬해도 같은 결과).
    layer_members: Vec<Vec<usize>>,
    adjacency: Vec<Vec<usize>>,
    blocks: Vec<Block>,
    /// (d) 블록별 등록된 레인 자식 목록(자식 순서) — `build_blocks()` 끝에서 만든다. 레인 자식이
    /// 있는 블록은 `reorder()`·`improve_by_swaps()`가 건너뛰므로 `children` 순서가 안 바뀐다.
    lane_children: Vec<Vec<usize>>,
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
    /// 층 사이 간격마다 꼬리 표식 줄 바로 다음에 비워 두는 줄 수: 풀 안의 노드와 흐름 끝을 피할 자리가 없는 메시지
    /// 다리가 이 줄로 풀을 가로지른다. 배치를 다시 할 때 비운다.
    gap_detour_rows: Vec<usize>,
    /// 장애물에 걸린 메시지 다리를 비켜 놓을지. 비켜 놓은 그림이 폭을 넘으면 끄고 다시 배선한다.
    allows_leg_detours: bool,
    /// 이번 배치에서 메시지 다리를 비켜 놓거나 그 자리를 마련했는지.
    has_detoured_legs: bool,
    /// 걸린 다리를 비켜 놓기 직전(통로 깊이만큼 칸을 끼운 뒤) 상태. 비켜 놓은 그림이 폭을 넘으면 여기서 다시 배선한다.
    before_leg_detours: Option<Box<Layout<'a>>>,
    /// 통로 라벨이 블록 폭 밖으로 삐져나갈 때 필요한 가로 폭.
    extra_across: usize,
    /// 라벨이 그룹 밖으로 나가지 않도록 블록마다 더 주는 폭.
    block_extra: Vec<usize>,
    /// 이름표가 레인 테두리를 덮지 않게 옮기고 간격을 늘리며, 메시지 다리가 구간 이름표를 비키게 할지. 그렇게 한 그림이
    /// 폭을 넘으면 끄고 다시 놓는다.
    avoids_label_conflicts: bool,
    /// 이번 배치에서 이름표 때문에 무언가를 했는지: 어느 배선에서든 이름표를 레인 테두리 칸에서 옮겼거나, 레인 사이
    /// 간격을 늘렸거나, 구간 이름표를 알고 메시지 다리를 비켜 놓거나 그 자리를 마련했다. 중간 배선에서 옮긴 자리로 블록
    /// 폭을 늘렸다가 마지막 배선에서 되돌아와도 늘린 폭은 남으므로, 폭을 넘었을 때 이름표를 살피지 않고 다시 놓을지는
    /// 이것으로 정한다.
    has_made_room_for_labels: bool,
    /// 이번 배치에서 통로 칸을 끼운 그림이 폭을 넘어 늘린 노드나 비켜 놓은 다리를 덜어 내려 했는지.
    has_shed_extras: bool,
    /// `DG_DEBUG`가 켜져 있으면 배선 정보를 stderr에 적는다.
    debug: bool,
    /// 캔버스 가로 한도. 이웃 교환은 이 폭을 넘기는 후보를 받지 않는다.
    width_limit: usize,
    /// 칸 끼우기 단계에서 찾은 풀 간선 경로의 결정(`base_pool_edge_routes()` 순서). 그리기가 다시 찾지 않고 쓴다.
    pool_route_plans: Vec<PoolRoutePlan>,
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
/// 접기 재시도 한 시도 안에서 구간 수가 이 상한을 넘으면 그 시도를 접는다(`None`). 정체
/// 계산(`stalls`)·`FOLD_PATIENCE`·`MAX_FOLDS`·접기 후보 로직은 그대로 두고, 배치 한 번의
/// 크기(구간 수)에만 절대 상한을 건다 — design.md가 표본 실측(수정 전 12초 타임아웃 안에
/// 그림으로 끝난 건의 최종 구간 수 최대 992)으로 근거를 댄 값이라 조정하지 않는다. 12초 밖의
/// 입력(예: `labels_s3_c20`·`base_s9_c40`)은 접기 도중 구간 수가 수천~1만 대까지 갔다가도
/// 회복하는 사례가 있어 patience(연속 초과 허용)로 구제하려 했으나, streak 7·8~17까지도
/// 회복하지 않는 반례가 함께 나와 유한한 patience로는 "곧 회복"과 "영원히 회복 안 함"을
/// 가릴 수 없다는 게 확인됐다(research.md) — 그래서 원안대로 되돌렸다: 12초 밖 입력은
/// bugfix.md 2.4에 따라 결과가 바뀔 수 있고 그게 맞다.
const FOLD_SEGMENT_LIMIT: usize = 1500;
/// 메시지 이름표를 얹는 가로 조각이 이름표 폭보다 길어야 하는 칸 수. 양 끝 꺾임이나 표식과 이름표 사이에 선이 한 칸
/// 이상 보여야 이름표가 그 선의 것으로 읽힌다.
const POOL_LABEL_RUN_MARGIN: usize = 3;
/// 다리가 걸친 장애물을 셀 때 노드 상자, 흐름 선, 테두리 하나를 구간 이름표 몇 개로 치는지. 다리가 이름표를 지나면
/// 이름표 한 칸이 선에 끊길 뿐이지만 노드나 테두리를 지나면 그림을 잘못 읽게 하므로, 이름표를 비키려고 그쪽으로 옮기지
/// 않고 그쪽을 비키려고 이름표 위로 옮길 수는 있게 한다.
const LABEL_HITS_PER_OTHER_HIT: usize = 2;
/// 다리 비켜 놓기 탐색이 경로 하나당 받는 옮김 수의 상한.
const LEG_MOVES_PER_ROUTE: usize = 4;
/// 장애물에 걸린 다리를 옮겨 볼 자리 수(쪽마다). 앞뒤로 가장 가까운 자리만 본다 — 더 보면 메시지가 많은 문서에서
/// 시험할 옮김이 크게 는다.
const DETOURS_PER_SIDE: usize = 1;
/// 장애물에 걸린 다리를 옮겨 볼 흐름축 거리의 상한. 더 멀리 돌아가는 다리는 다른 메시지와 엇갈리기 쉽고, 메시지가
/// 많은 문서에서 막힌 다리마다 풀 전체를 훑으면 느려진다.
const DETOUR_REACH: usize = 8;

/// 지금 순서대로 놓되, 놓을 항목에 아직 놓이지 않은 보내는 항목(`precedence`의 앞)이 있으면 그것을
/// 지금 순서대로(재귀로) 먼저 놓는다. 제약이 없는 항목끼리의 앞뒤는 그대로다. 순환에 걸린 제약은
/// 어느 쪽을 지켜도 다른 쪽을 어기므로 버리고 지금 순서를 따른다. `items`에 없는 항목의 제약은 무시한다.
fn order_with_senders_pulled_forward(items: &[usize], precedence: &[(usize, usize)]) -> Vec<usize> {
    let position_of = |item: usize| items.iter().position(|&candidate| candidate == item);
    let mut senders_of: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    for &(sender, receiver) in precedence {
        if let (Some(sender), Some(receiver)) = (position_of(sender), position_of(receiver))
            && sender != receiver
        {
            senders_of[receiver].push(sender);
        }
    }
    let acyclic_senders_of: Vec<Vec<usize>> = (0..items.len())
        .map(|receiver| {
            let mut senders: Vec<usize> = senders_of[receiver].iter().copied().filter(|&sender| !is_reachable_through_senders(&senders_of, sender, receiver)).collect();
            senders.sort_unstable();
            senders.dedup();
            senders
        })
        .collect();
    let mut is_placed = vec![false; items.len()];
    let mut ordered = Vec::with_capacity(items.len());
    for position in 0..items.len() {
        place_after_senders(position, &acyclic_senders_of, &mut is_placed, &mut |placed| ordered.push(items[placed]));
    }
    ordered
}

/// `from`에서 보내는 항목 쪽으로 거슬러 가 `target`에 닿는지 — 닿으면 두 항목은 같은 순환에 있다.
fn is_reachable_through_senders(senders_of: &[Vec<usize>], from: usize, target: usize) -> bool {
    let mut is_seen = vec![false; senders_of.len()];
    let mut pending = vec![from];
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !std::mem::replace(&mut is_seen[current], true) {
            pending.extend(senders_of[current].iter().copied());
        }
    }
    false
}

/// `position`의 아직 놓이지 않은 보내는 항목을 지금 순서대로 먼저 놓고 자신을 놓는다(`senders_of`는 순환이 없다).
fn place_after_senders(position: usize, senders_of: &[Vec<usize>], is_placed: &mut [bool], place: &mut impl FnMut(usize)) {
    if std::mem::replace(&mut is_placed[position], true) {
        return;
    }
    for &sender in &senders_of[position] {
        place_after_senders(sender, senders_of, is_placed, place);
    }
    place(position);
}

impl<'a> Layout<'a> {
    fn build(graph: &'a Graph, theme: &'a Theme, direction: Direction, cap: usize, width: usize, allow_fold: bool) -> Option<Canvas> {
        let mut layout = Layout::new(graph, theme, direction, cap);
        layout.width_limit = width;
        layout.assign_layers();
        layout.push_outputs_below_groups();
        // 폭을 넘는 배치는 세로 배치에서 층을 접을 때만 쓰인다(접을 줄과 폭 비교). 그 밖에는 넘는지만 맞으면 된다.
        let too_wide_use = if allow_fold && direction == Direction::TopDown { TooWideUse::Folded } else { TooWideUse::Discarded };
        layout.arrange(false, too_wide_use);
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
                layout.arrange(true, TooWideUse::Discarded);
                if layout.canvas_size().0 > width {
                    layout.reset_arrangement();
                    layout.arrange(false, TooWideUse::Discarded);
                }
                return Some(layout.canvas_with_fewest_label_defects());
            }
            if !allow_fold || direction != Direction::TopDown || !layout.fold_widest_row() {
                return None;
            }
            layout.reset_arrangement();
            layout.arrange(false, too_wide_use);
            // (a) 접기 재시도의 구간 수 절대 상한: 접기 쳇바퀴(폭이 안 줄면서 층·가상 노드·구간
            // 수만 누적)에 빠진 시도를 배치 한 번의 크기가 커지기 전에 접는다.
            if layout.segments.len() > FOLD_SEGMENT_LIMIT {
                return None;
            }
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

    /// 그린 그림. 메시지 이름표를 다른 선, 글자, 레인 테두리를 비키는 자리로 옮겼는데 옮긴 이름표에 흠(`LabelDefects`)이
    /// 남으면, 옮기지 않고 가운데에 둔 그림을 다시 그려 그림 전체의 흠이 덜한 쪽을 쓴다. 깨끗한 자리가 없어 가장 가벼운
    /// 흠을 고른 이름표는 다른 이름표 바로 옆이거나 그 글자를 가릴 수 있어서다. 두 그림은 옮긴 이름표만 다르므로 옮긴
    /// 이름표에 흠이 없으면 다시 그리지 않는다. 두 그림은 메시지 이름표를 놓기 전까지 같으므로 캔버스 하나에 그 앞까지
    /// 그려 두고, 이름표와 그 뒤에 그리는 그룹 제목이 바꾼 칸만 적어 두었다가 되돌려 다른 방식으로 다시 놓는다 — 넓은
    /// 그림에서 캔버스를 둘 두면 메모리가 그림 하나만큼 는다.
    fn canvas_with_fewest_label_defects(&self) -> Canvas {
        let (canvas_width, canvas_height) = self.canvas_size();
        let mut canvas = Canvas::new(canvas_width, canvas_height);
        let before_labels = self.draw_until_pool_labels(&mut canvas);
        canvas.start_journal();
        let labels = self.draw_pool_labels_and_titles(&mut canvas, &before_labels, PoolLabelPlacement::AvoidingLines);
        if label_defects(&canvas, labels.iter().filter(|label| label.is_moved)) != LabelDefects::default() {
            let moved_defects = label_defects(&canvas, &labels);
            canvas.restore_journal();
            let centered_labels = self.draw_pool_labels_and_titles(&mut canvas, &before_labels, PoolLabelPlacement::Centered);
            if label_defects(&canvas, &centered_labels) >= moved_defects {
                canvas.restore_journal();
                self.draw_pool_labels_and_titles(&mut canvas, &before_labels, PoolLabelPlacement::AvoidingLines);
            }
        }
        canvas.stop_journal();
        canvas
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

    /// 층이 정해진 뒤의 배치 전 과정. `refine`이면 이웃 교환 탐색까지 한다. 이름표 때문에 무언가를 한 그림이 폭을 넘거나
    /// 폭에 맞추려고 늘린 노드나 비켜 놓은 다리를 덜어 냈으면 이름표를 살피지 않고 다시 놓는다 — 이름표가 테두리나 선에
    /// 걸리는 편이 메시지 끝 표식이 합쳐지거나 그림이 접히거나 다른 방향, 원문 코드로 떨어지는 것보다 낫고, 살피지 않은
    /// 배치는 접기 판단도 살피기 전과 같다. `too_wide_use`는 폭을 넘는 배치를 부른 쪽이 쓰는지다.
    ///
    /// 간격 늘리기와 통로 칸 끼우기는 칸을 끼우기만 해 폭을 줄이지 않으므로, 그 앞에서 이미 폭을 넘는 배치는 끝까지 해도
    /// 넘는다. 그때 이름표 때문에 무언가를 했으면 어차피 다시 놓으니 남은 단계를 하지 않는다. 폭을 넘는 배치를 버리는
    /// 쪽이면, 이름표를 살피지 않은 배치가 폭을 넘을 때 어느 쪽을 골라도 넘으므로 이름표를 살핀 배치를 끝내지 않는다.
    fn arrange(&mut self, refine: bool, too_wide_use: TooWideUse) {
        self.arrange_until_label_room(refine);
        if !self.fits_width() {
            if self.has_made_room_for_labels {
                self.rearrange_without_labels(refine);
                return;
            }
            // 메시지가 없으면 남은 단계가 싸서, 이름표를 살피지 않은 배치를 따로 하는 편이 오히려 비싸다.
            if self.may_rearrange_without_labels() && self.has_pool_edges() && self.discards_too_wide(too_wide_use) {
                self.finish_too_wide_arrangement(refine);
                return;
            }
        }
        self.finish_arrangement();
        if self.needs_rearrangement_without_labels() {
            self.rearrange_without_labels(refine);
        }
    }

    /// 폭을 넘는 배치를 버리는 쪽에서, 간격 늘리기 앞에서 이미 폭을 넘는 이름표를 살핀 배치를 마친다. 이름표를 살피지
    /// 않은 배치가 폭 안에 들 때만 이름표를 살핀 배치를 끝까지 해 둘 가운데 무엇을 쓸지 가린다.
    fn finish_too_wide_arrangement(&mut self, refine: bool) {
        let mut without_labels = self.clone();
        without_labels.rearrange_without_labels(refine);
        if without_labels.fits_width() {
            self.finish_arrangement();
            if !self.needs_rearrangement_without_labels() {
                return;
            }
        }
        *self = without_labels;
    }

    /// 이름표를 살핀 배치를 이름표를 살피지 않고 다시 놓아야 하는지(`arrange()`).
    fn needs_rearrangement_without_labels(&self) -> bool {
        (self.has_made_room_for_labels && !self.fits_width()) || (self.avoids_label_conflicts && self.has_shed_extras)
    }

    /// 이름표를 살피는 배치라 이름표를 살피지 않고 다시 놓을 수 있는지. 가로 배치는 이름표를 살펴도 옮기거나 늘리는
    /// 것이 없다.
    fn may_rearrange_without_labels(&self) -> bool {
        self.avoids_label_conflicts && self.direction == Direction::TopDown
    }

    /// 폭을 넘는 이 배치를 부른 쪽이 버리는지: 버린다고 했거나, 접을 줄이 없어 층 접기가 쓰지 않는다.
    fn discards_too_wide(&self, too_wide_use: TooWideUse) -> bool {
        too_wide_use == TooWideUse::Discarded || self.foldable_rows().is_empty()
    }

    fn rearrange_without_labels(&mut self, refine: bool) {
        self.reset_arrangement();
        self.avoids_label_conflicts = false;
        self.arrange_until_label_room(refine);
        self.finish_arrangement();
    }

    /// 배치의 앞 단계: 층 안 순서, 접점, 배선, 라벨 자리까지.
    fn arrange_until_label_room(&mut self, refine: bool) {
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
        self.widen_for_straight_lines();
        self.route_with_message_end_growth();
        if self.reserve_label_room() {
            self.place();
            self.assign_ports();
            self.straighten();
            self.route_with_message_end_growth();
        }
    }

    /// 배치의 뒤 단계: 이름표 자리 간격 늘리기와 통로 칸 끼우기, 폭을 넘으면 덜어 내기.
    fn finish_arrangement(&mut self) {
        self.widen_lane_gaps_for_labels();
        // 세로 배치에서는 늘린 노드가 통로 칸 끼우기를 거쳐서야 폭을 바꾸므로, 끼우기 전 상태를 두었다가 넘치면 덜어 낸다.
        // 비켜 놓은 메시지 다리도 폭을 바꾸지만, 그 전 상태는 걸린 다리가 있을 때만 칸 끼우기 단계가 둔다. 끼우기 전에
        // 이미 넘치면 끼운 뒤에도 넘치므로 덜어 낼 것이 없다.
        let fitted_before_channel_room = self.fits_width();
        let before_channel_room = (self.direction == Direction::TopDown && self.has_grown_nodes() && fitted_before_channel_room).then(|| self.clone());
        self.finish_with_pool_channel_room();
        if fitted_before_channel_room && !self.fits_width() {
            self.shed_extras_to_fit_width(before_channel_room);
            // 덜어 낸 배치는 칸 끼우기 전 상태의 복제라 그 뒤에 표시한다.
            self.has_shed_extras = true;
        }
    }

    fn has_pool_edges(&self) -> bool {
        (0..self.graph.edges.len()).any(|edge| self.is_pool_edge(edge))
    }

    /// 통로 칸을 끼운 그림이 폭을 넘으면 늘린 노드부터 덜고(세로 배치), 그래도 넘치면 메시지 다리를 비켜 놓지 않고 다시
    /// 배선한다. 그림이 원문 코드나 다른 배치로 떨어지는 것보다 다리가 다른 노드를 지나는 편이 낫다.
    fn shed_extras_to_fit_width(&mut self, before_channel_room: Option<Layout<'a>>) {
        if let Some(before_channel_room) = &before_channel_room {
            self.keep_only_growth_that_fits(before_channel_room.clone());
        }
        if self.fits_width() || !self.has_detoured_legs {
            return;
        }
        let without_detours = match before_channel_room {
            Some(before_channel_room) => Self::arranged_without_detours(before_channel_room, false),
            None => match self.before_leg_detours.take() {
                Some(before_leg_detours) => Self::arranged_without_detours(*before_leg_detours, true),
                None => return,
            },
        };
        if without_detours.fits_width() {
            *self = without_detours;
        }
    }

    /// `before`에서 메시지 다리를 비켜 놓지 않고 통로 칸을 끼워 배선을 마친 배치. 세로 배치에서 늘린 노드가 있어 그래도
    /// 넘치면 늘림을 덜어 본다. `is_after_channel_room`이면 `before`는 통로 깊이만큼 칸을 끼운 뒤라 배선을 다시 해야 한다.
    fn arranged_without_detours(mut before: Layout<'a>, is_after_channel_room: bool) -> Layout<'a> {
        before.allows_leg_detours = false;
        let mut layout = before.clone();
        layout.finish_with_pool_channel_room();
        if is_after_channel_room {
            layout.route();
        }
        if layout.direction == Direction::TopDown && !layout.fits_width() && before.has_grown_nodes() {
            layout.keep_only_growth_that_fits(before);
        }
        layout
    }

    /// 메시지 끝 수만큼 노드를 흐름축으로 늘려 배선한다. 세로 배치는 늘림이 높이라 그대로 늘리고, 가로 배치는 늘림이 곧
    /// 폭이라 늘리지 않은 배선의 폭에서 남는 만큼만 늘린다.
    fn route_with_message_end_growth(&mut self) {
        let wanted = self.along_sizes_for_message_ends();
        match self.direction {
            Direction::TopDown => {
                self.set_along_sizes(&wanted);
                self.route();
            }
            Direction::LeftRight => {
                self.set_along_sizes(&vec![0; wanted.len()]);
                self.route();
                let affordable = self.growth_within_width(&wanted);
                if self.set_along_sizes(&affordable) {
                    self.route();
                }
            }
        }
    }

    /// 가로 배치에서 폭 안에 드는 늘림만 남긴다. 폭은 층마다 가장 큰 노드의 흐름축 크기를 더한 것이라, 늘림이 작은
    /// 노드부터 그 층의 최대 크기가 커지는 만큼을 남은 폭에서 빼며 고른다 — 한 노드 때문에 다른 노드까지 늘리지 못하는
    /// 일이 없다. 늘리지 않은 배선이 끝난 뒤에 부른다.
    fn growth_within_width(&self, wanted: &[usize]) -> Vec<usize> {
        let mut room = self.width_limit.saturating_sub(self.canvas_size().0);
        let mut layer_content = self.layer_content.clone();
        let mut growth_order: Vec<usize> = (0..wanted.len()).filter(|&n| wanted[n] > self.lnodes[n].content_along_size).collect();
        growth_order.sort_by_key(|&n| (wanted[n] - self.lnodes[n].content_along_size, n));
        let mut affordable = vec![0; wanted.len()];
        for n in growth_order {
            let layer = self.lnodes[n].layer;
            let widening = wanted[n].saturating_sub(layer_content[layer]);
            if widening <= room {
                room -= widening;
                layer_content[layer] = layer_content[layer].max(wanted[n]);
                affordable[n] = wanted[n];
            }
        }
        affordable
    }

    /// 풀 간선 통로 칸을 끼우고 배선을 마친다.
    fn finish_with_pool_channel_room(&mut self) {
        if self.reserve_pool_channel_room() {
            self.route();
        }
    }

    /// 가상 노드·블록·배선 정보를 지우고 층 배정만 남긴다.
    fn reset_arrangement(&mut self) {
        let real_count = self.graph.nodes.len();
        self.lnodes.truncate(real_count);
        for node in &mut self.lnodes {
            node.across = 0;
            node.along_size = node.content_along_size;
        }
        self.segments.clear();
        self.outgoing_of.clear();
        self.incoming_of.clear();
        self.layer_members.clear();
        self.adjacency.clear();
        self.blocks.clear();
        self.lane_children.clear();
        self.lnode_block.clear();
        self.extra_across = 0;
        self.block_extra.clear();
        self.avoids_label_conflicts = true;
        self.has_made_room_for_labels = false;
        self.has_shed_extras = false;
        self.pool_route_plans.clear();
        self.gap_detour_rows.clear();
        self.has_detoured_legs = false;
        self.before_leg_detours = None;
    }

    /// 가장 넓은 줄의 맨 오른쪽 자식을 아래로 내린다. 나눌 줄이 없으면 false.
    fn fold_widest_row(&mut self) -> bool {
        let Some(candidate) = self.fold_candidates().into_iter().next() else { return false };
        self.apply_fold(candidate);
        true
    }

    /// 접기 후보: 넓은 "줄"(한 블록의 한 층에 나란히 놓인 자식들)부터, 줄 안에서는 오른쪽 자식부터.
    fn fold_candidates(&self) -> Vec<FoldCandidate> {
        let mut rows: Vec<(usize, usize, usize)> = self
            .foldable_rows()
            .into_iter()
            .map(|(block, layer, row)| {
                let start = row.iter().map(|&c| self.child_span(c).0).min().unwrap_or(0);
                let end = row.iter().map(|&c| self.child_span(c).1).max().unwrap_or(0);
                (end - start, block, layer)
            })
            .collect(); // (폭, 블록, 층)
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

    /// 접을 수 있는 줄(블록, 층, 그 줄의 자식들): 옮길 수 있는 자식이 둘 이상인 줄. 어느 줄이 있는지는 블록과 층에만
    /// 달렸고 자리에는 달리지 않는다.
    fn foldable_rows(&self) -> Vec<(usize, usize, Vec<Child>)> {
        (0..self.blocks.len())
            .filter(|&block| self.blocks[block].registered)
            .flat_map(|block| (self.blocks[block].layer_min..=self.blocks[block].layer_max).map(move |layer| (block, layer)))
            .map(|(block, layer)| (block, layer, self.row_children(block, layer)))
            .filter(|(_, _, row)| row.iter().filter(|&&c| self.is_movable(c)).count() >= 2)
            .collect()
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
                let point_anchored = node.shape.is_point_anchored();
                let (along_size, across_size) = match direction {
                    Direction::TopDown => {
                        // 접점마다 라벨이 붙을 수 있으면 상자를 그만큼 넓혀 접점 사이를 벌린다.
                        // 점 표기 노드는 접점이 모두 원점 칸 하나로 모이므로 벌릴 필요가 없다.
                        let (incoming, outgoing) = degree[index];
                        let ports = incoming.max(outgoing);
                        let spacing = port_spacing_for(widest_end_label[index]);
                        let needed = if !point_anchored && ports > 1 && w > 2 { (ports - 1) * spacing + 3 } else { 0 };
                        (h, w.max(needed))
                    }
                    Direction::LeftRight => {
                        // 왼쪽→오른쪽에서는 간선이 나가는 줄마다 라벨이 붙으므로 두 줄 간격이 필요하다.
                        // 점 표기 노드는 벌리지 않는다(위와 같은 이유).
                        let (incoming, outgoing) = degree[index];
                        let needed = 2 * incoming.max(outgoing) + 1;
                        // 글자 없는 상자(테두리 두 줄)도 한쪽에 흐름이 둘 이상이면 키워 접점이 한 점에 모이지 않게 한다.
                        let is_crowded_glyphless_box = h == 2 && node.shape.is_border_box() && incoming.max(outgoing) >= 2;
                        (w, if !point_anchored && (h >= 3 || is_crowded_glyphless_box) { h.max(needed) } else { h })
                    }
                };
                LayoutNode { node: Some(0), edge: None, layer: 0, group: node.group, along_size, content_along_size: along_size, across_size, extra_across: 0, across: 0, sections, point_anchored }
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
            outgoing_of: Vec::new(),
            incoming_of: Vec::new(),
            layer_members: Vec::new(),
            adjacency: Vec::new(),
            blocks: Vec::new(),
            lane_children: Vec::new(),
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
            gap_detour_rows: Vec::new(),
            allows_leg_detours: true,
            has_detoured_legs: false,
            before_leg_detours: None,
            extra_across: 0,
            block_extra: Vec::new(),
            avoids_label_conflicts: true,
            has_made_room_for_labels: false,
            has_shed_extras: false,
            debug: std::env::var_os("DG_DEBUG").is_some(),
            width_limit: usize::MAX,
            pool_route_plans: Vec::new(),
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
            if edge.from != edge.to && !self.is_pool_edge(i) {
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
            if edge.from == edge.to || self.is_pool_edge(i) {
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
                from == anchor && to != anchor && !self.is_pool_edge(i)
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
            if edge.from == edge.to || self.is_pool_edge(i) {
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
                    content_along_size: 0,
                    across_size: 1,
                    extra_across: 0,
                    across: 0,
                    sections: Vec::new(),
                    point_anchored: false,
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
        // (c) 노드별 구간 색인: 구간 색인 오름차순으로 쌓으므로 원소·순서가
        // `segments.iter().filter(|s| s.from == i)`(또는 `to == i`)와 정확히 같다.
        self.outgoing_of = vec![Vec::new(); self.lnodes.len()];
        self.incoming_of = vec![Vec::new(); self.lnodes.len()];
        for (s, segment) in self.segments.iter().enumerate() {
            self.outgoing_of[segment.from].push(s);
            self.incoming_of[segment.to].push(s);
        }
        // (d) 층별 구성원 색인: 노드 색인 오름차순으로 쌓으므로
        // `(0..lnodes.len()).filter(|&i| lnodes[i].layer == layer)`와 원소·순서가 같다.
        self.layer_members = vec![Vec::new(); self.layer_count];
        for i in 0..self.lnodes.len() {
            let layer = self.lnodes[i].layer;
            if layer < self.layer_members.len() {
                self.layer_members[layer].push(i);
            }
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

    /// 레인(풀) 그룹의 닻 노드인지 — 블랙박스 풀 등 참여자 자체를 가리키는 메시지 흐름의 끝.
    fn is_pool_anchor(&self, node: usize) -> bool {
        let node = &self.graph.nodes[node];
        node.shape == Shape::Anchor && node.group.is_some_and(|g| self.graph.groups[g].kind == GroupKind::Lane)
    }

    /// 풀 닻에 닿는 간선 — 층 배치·구간 배선에서 빠지고 `draw_pool_edge_lines()`가 풀 면으로 따로 긋는다.
    fn is_pool_edge(&self, edge: usize) -> bool {
        let edge = &self.graph.edges[edge];
        edge.from != edge.to && (self.is_pool_anchor(edge.from) || self.is_pool_anchor(edge.to))
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
        self.order_root_lanes_by_precedence();
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
        // (d) 블록별 등록된 레인 자식 목록: `children` 순서 그대로(레인 자식이 있는 블록은
        // `reorder()`·`improve_by_swaps()`가 건너뛰므로 이 순서는 이후 안 바뀐다).
        self.lane_children = vec![Vec::new(); self.blocks.len()];
        for block in 0..self.blocks.len() {
            for &child in &self.blocks[block].children {
                if let Child::Block(b) = child
                    && self.is_lane(b)
                    && self.blocks[b].registered
                {
                    self.lane_children[block].push(b);
                }
            }
        }
    }

    /// 루트의 레인 블록만 `lane_precedence`를 따르도록 다시 줄 세운다. 레인이 아닌 자식의 자리와
    /// 레인 안 하위 레인 순서는 그대로 둔다(레인 순서는 선언대로라는 규약은 하위 레인에만 걸린다).
    fn order_root_lanes_by_precedence(&mut self) {
        if self.graph.lane_precedence.is_empty() {
            return;
        }
        let lane_slots: Vec<usize> = (0..self.blocks[0].children.len()).filter(|&slot| self.is_lane_child(self.blocks[0].children[slot])).collect();
        let lane_groups: Vec<usize> = lane_slots
            .iter()
            .map(|&slot| match self.blocks[0].children[slot] {
                Child::Block(block) => block - 1,
                Child::Node(_) => unreachable!("레인 자리만 골랐다"),
            })
            .collect();
        let ordered_groups = order_with_senders_pulled_forward(&lane_groups, &self.graph.lane_precedence);
        for (slot, group) in lane_slots.into_iter().zip(ordered_groups) {
            self.blocks[0].children[slot] = Child::Block(group + 1);
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
            // 닻(그룹으로 드나드는 화살표 자리)은 테두리 제목 글자와 겹치지 않게 제목 오른쪽에
            // 둔다 — 위→아래 배치에서만 뜻이 있다: 제목이 상자 위 테두리를 가로로 차지하므로
            // 그 글자 폭만큼 `start`(가로 위치)를 밀어야 겹치지 않는다. 왼쪽→오른쪽 배치에서는
            // `start`가 세로 위치라 제목 "폭"과 무관하다 — 그대로 적용하면 제목이 길수록 닻이
            // 아래로 밀려 상자가 실제 내용과 무관하게 부풀어 오른다(bizprocess-bpmn 회귀).
            if self.direction == Direction::TopDown
                && let Child::Node(i) = child
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
        // (d) 층별 구성원 색인: 매번 `0..lnodes.len()`을 훑어 층을 거르는 대신 복제해 정렬한다
        // (원소는 원래 필터와 같으므로 안정 정렬 결과도 같다).
        let mut members: Vec<usize> = self.layer_members.get(layer).cloned().unwrap_or_default();
        members.sort_by_key(|&i| self.lnodes[i].across);
        for (k, &i) in members.iter().enumerate() {
            let has_upper = self.adjacency[i].iter().any(|&n| self.lnodes[n].layer + 1 == layer);
            if !use_upper && has_upper {
                continue;
            }
            // 접점끼리 맞아떨어지는 시작 위치들의 중앙값을 목표로 한다.
            // (c) 노드별 구간 색인: 전체 구간 대신 이 노드가 닿은 구간만 훑는다(원소·순서는
            // 원래 `segments.iter().filter(...)`와 같다 — 둘 다 구간 색인 오름차순).
            let mut targets: Vec<i64> = if use_upper {
                self.incoming_of[i]
                    .iter()
                    .filter_map(|&s| {
                        let segment = &self.segments[s];
                        (self.lnodes[segment.from].layer + 1 == layer)
                            .then(|| self.lnodes[segment.from].across as i64 + segment.exit_offset as i64 - segment.entry_offset as i64)
                    })
                    .collect()
            } else {
                self.outgoing_of[i]
                    .iter()
                    .filter_map(|&s| {
                        let segment = &self.segments[s];
                        (self.lnodes[segment.to].layer == layer + 1)
                            .then(|| self.lnodes[segment.to].across as i64 + segment.entry_offset as i64 - segment.exit_offset as i64)
                    })
                    .collect()
            };
            if targets.is_empty() {
                continue;
            }
            targets.sort_unstable();
            let desired = targets[targets.len() / 2].max(0) as usize;
            let container = self.lnode_block[i];
            let (mut low, mut high) = self.across_bounds(&members, k, layer);
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

    /// `members`(층 `layer`, 교차축 오름차순)의 `k`번째 노드가 놓일 수 있는 교차축 시작 위치 범위:
    /// 블록 안쪽 여백, 같은 층 이웃과의 간격, 그 층을 관통하는 형제 레인 상자를 모두 피한다.
    fn across_bounds(&self, members: &[usize], k: usize, layer: usize) -> (usize, usize) {
        let i = members[k];
        let footprint = self.lnodes[i].footprint();
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
        // (d) 블록별 등록된 레인 자식 목록: `is_lane(b)`·`registered`는 이미 걸러져 있으므로
        // 층 범위만 확인한다(이건 `layer`마다 달라 미리 걸러 둘 수 없다).
        for &b in self.lane_children.get(container).map(Vec::as_slice).unwrap_or_default() {
            if !(self.blocks[b].layer_min <= layer && layer <= self.blocks[b].layer_max) {
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
        (low, high)
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
        if node.point_anchored {
            // 접점 수와 무관하게 모두 원점 칸(= 위치 글자)으로 모인다 — 상태도 `●`/`◉`에
            // 간선 여럿이 닿을 때와 같은 결과.
            return node.across;
        }
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
            let mut outgoing: Vec<usize> = self.outgoing_of[i].clone();
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
            let mut incoming: Vec<usize> = self.incoming_of[i].clone();
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
        let lane_sides = self.lane_side_acrosses();
        for layer in 0..gap_count {
            let mut intervals: Vec<(usize, usize, usize)> = Vec::new();
            let mut label_room = 0;
            let mut head_room = 0;
            let mut placed_labels: Vec<(usize, usize)> = Vec::new();
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
                            let (label_x, is_moved_off_lane_side) = self.choose_label_x(s, layer, label_width, &lane_sides, &placed_labels);
                            self.has_made_room_for_labels |= is_moved_off_lane_side;
                            placed_labels.push((label_x, label_x + label_width));
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
            self.gap[layer] = GAP_ALONG_MIN.max(self.gap_tail_rows[layer] + self.detour_rows(layer) + self.gap_label_room[layer] + channels + self.gap_head_room[layer] + self.gap_head_rows[layer]);
        }

        self.layer_start = vec![0; self.layer_count];
        if self.layer_count > 0 {
            // 깊이별 배너 크기 합과 그 뒤 구분선 칸을 첫 층 시작 앞에 둔다 — 레인이 없으면 0이라
            // 기존 출력과 바이트 단위로 같다(요구사항 6.1).
            self.layer_start[0] = self.banner_along().iter().sum::<usize>() + self.lane_border_margin();
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

    /// 깊이마다 `banner_start`(마지막 칸은 모든 배너 크기 합).
    fn banner_starts(&self) -> Vec<usize> {
        let mut starts = vec![0];
        for size in self.banner_along() {
            starts.push(starts[starts.len() - 1] + size);
        }
        starts
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

    /// 모든 구간의 접점(exit/entry)을 다시 계산한다 — `refresh_ports_for()`의 전체 버전(SSoT).
    fn refresh_ports(&mut self) {
        self.refresh_ports_for(0..self.segments.len());
    }

    /// (b) 국소 갱신: 주어진 구간만 접점(exit/entry)을 다시 계산한다. 전체 `refresh_ports()`와
    /// 같은 값을 내는 이유는 design.md 불변식 ①②(아래 `resolve_port_swaps()` 주석) — 나머지
    /// 구간은 입력(출발/도착 노드·층·across·오프셋)이 그대로라 다시 계산해도 같은 값이 나온다.
    fn refresh_ports_for<I: IntoIterator<Item = usize>>(&mut self, segments: I) {
        for s in segments {
            let (from, to) = (self.segments[s].from, self.segments[s].to);
            self.segments[s].exit = self.lnodes[from].across + self.segments[s].exit_offset;
            self.segments[s].entry = self.lnodes[to].across + self.segments[s].entry_offset;
        }
    }

    /// 같은 통로에서 두 간선이 서로의 열을 맞바꾸는 X자 교차(i.exit == j.entry, j.exit == i.entry)는
    /// 줄 순서로는 풀 수 없다. 실제 노드에 붙은 접점을 한 칸 옮겨 열을 어긋나게 한다.
    /// (b) X자 교차 해소: 짝 후보를 "출발 층이 같은 구간"으로만 좁히고(층별 버킷), 접점 갱신을
    /// 전체가 아니라 바뀐 구간만 국소로 한다. 동치가 성립하는 불변식(후속 변경이 조용히
    /// 깨뜨리지 않도록 여기 적는다):
    /// ① `make_segments()` 이후 구간의 출발/도착 노드와 층은 바뀌지 않는다(이 함수 안에서
    ///    바뀌는 건 `across`·오프셋뿐) — 그래서 층별 버킷을 함수 시작에 한 번만 만들어 8회차
    ///    내내 그대로 써도 된다.
    /// ② 이 함수 안에서 `across`가 바뀌는 노드는 교환된 두 가상 노드뿐이고, 오프셋이 바뀌는
    ///    구간은 nudge된 구간뿐이다 — 그래서 그 둘에 닿은 구간만 다시 계산해도 전체
    ///    `refresh_ports()`와 값이 같다(나머지 구간은 입력이 그대로).
    /// ③ nudge된 구간 집합은 다음 교환 시점 또는 회차 끝 전체 갱신에서 반드시 비운다(drain).
    ///    교환 시에는 교환된 두 가상 노드의 `from`·`to` 양쪽 구간을 모두 갱신 대상에 넣는다.
    fn resolve_port_swaps(&mut self) {
        // 방문하는 (i, j) 쌍·순서는 원래 이중 루프(i×j 전체를 돌며 출발 층이 다르면 건너뜀)와
        // 정확히 같다: 바깥 i는 구간 색인 순서 그대로, 안쪽은 i와 같은 층 버킷 안에서 색인
        // 오름차순으로만 돈다(불변식 ①).
        let mut by_from_layer: Vec<Vec<usize>> = vec![Vec::new(); self.layer_count];
        for (s, segment) in self.segments.iter().enumerate() {
            let layer = self.lnodes[segment.from].layer;
            if layer < by_from_layer.len() {
                by_from_layer[layer].push(s);
            }
        }
        let mut nudged: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for _ in 0..8 {
            let mut any_nudged = false;
            for i in 0..self.segments.len() {
                let layer = self.lnodes[self.segments[i].from].layer;
                for &j in &by_from_layer[layer] {
                    if i == j {
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
                        // 같은 회차에서 반대 순서로 다시 만나 되돌리지 않도록 바로 갱신한다 — 교환된
                        // 두 가상 노드에 닿는 구간(출발·도착 양쪽) + 그때까지 밀어 둔 nudge 구간(③).
                        nudged.extend(self.outgoing_of[a_to].iter().copied());
                        nudged.extend(self.incoming_of[a_to].iter().copied());
                        nudged.extend(self.outgoing_of[b_to].iter().copied());
                        nudged.extend(self.incoming_of[b_to].iter().copied());
                        let drained: Vec<usize> = nudged.drain().collect();
                        self.refresh_ports_for(drained);
                        any_nudged = true;
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
                        self.lnodes[node].node.is_some() && self.lnodes[node].across_size >= 4 && !self.lnodes[node].point_anchored
                    }) else {
                        continue;
                    };
                    let node = if is_exit { self.segments[s].from } else { self.segments[s].to };
                    let limit = self.lnodes[node].across_size - 2;
                    let offset = if is_exit { &mut self.segments[s].exit_offset } else { &mut self.segments[s].entry_offset };
                    *offset = if *offset < limit { *offset + 1 } else { offset.saturating_sub(1).max(1) };
                    nudged.insert(s);
                    any_nudged = true;
                }
            }
            if !any_nudged {
                return;
            }
            let drained: Vec<usize> = nudged.drain().collect();
            self.refresh_ports_for(drained);
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

    /// TB 라벨 자리: 가로 구간이 넉넉하면 그 가운데, 아니면 다른 간선의 세로줄과 안 붙는 쪽. 가운데가 레인 테두리
    /// (`lane_sides`)를 덮으면 구간 안에서 테두리, 다른 간선의 세로줄, 이 층에 먼저 놓은 라벨(`placed_labels`)을 모두
    /// 비키는 자리로 옮긴다. 그런 자리가 없으면 선에서 떼어 놓지 않고 가운데에 두어 `widen_lane_gaps_for_labels()`가
    /// 간격을 늘리게 한다. 레인이 있으면 선 왼쪽 자리는 선이 지나는 블록의 테두리 안쪽에 들 때만 쓴다 — 블록은 오른쪽으로만
    /// 넓혀 주므로 왼쪽으로 나간 이름표는 그 블록 테두리나 이웃 풀 위에 놓인다. 테두리 때문에 옮겼는지도 돌려준다.
    fn choose_label_x(&self, s: usize, layer: usize, label_width: usize, lane_sides: &[usize], placed_labels: &[(usize, usize)]) -> (usize, bool) {
        let segment = &self.segments[s];
        let (low, high) = (segment.exit.min(segment.entry), segment.exit.max(segment.entry));
        let span = high - low;
        let center = (span >= label_width + 2).then(|| low + (span - label_width) / 2 + 1);
        // 가운데 자리가 쓰이면 다른 간선의 세로줄을 모으지 않는다 — 같은 층의 모든 구간을 훑는 일이라 배선마다 되풀이하면 느리다.
        if let Some(x) = center
            && !covers_any(x, label_width, lane_sides)
        {
            return (x, false);
        }
        let stubs = self.layer_stubs(s, layer);
        let is_free = |x: usize| stubs.iter().all(|&stub| stub + 1 < x || stub > x + label_width);
        if let Some(x) = center {
            let is_clear = |&x: &usize| is_free(x) && !placed_labels.iter().any(|&(start, end)| x < end && start < x + label_width);
            return starts_between_sides(low, high, label_width, lane_sides).find(is_clear).map_or((x, false), |inside| (inside, true));
        }
        let right = high + 2;
        let inner_start = if lane_sides.is_empty() { 0 } else { self.routing_block_inner_start(s) };
        let left = low.checked_sub(label_width + 2).filter(|&x| x >= inner_start);
        let beside = [Some(right), left, Some(right + 1), Some(right + 2), Some(right + 3)].into_iter().flatten().filter(|&x| is_free(x));
        let first_choice = beside.clone().next().unwrap_or(right);
        if !covers_any(first_choice, label_width, lane_sides) {
            return (first_choice, false);
        }
        beside.clone().find(|&x| !covers_any(x, label_width, lane_sides)).map_or((first_choice, false), |x| (x, true))
    }

    /// 구간이 지나는 블록의 테두리 바로 안쪽 첫 교차축 자리. 테두리가 없는 루트 블록은 0이다.
    fn routing_block_inner_start(&self, s: usize) -> usize {
        let block = self.routing_block(self.segments[s].from, self.segments[s].to);
        self.blocks[block].start + usize::from(block != 0)
    }

    /// 같은 층의 다른 구간들이 통로 줄을 세로로 지나는 교차축 자리(출발·도착 접점).
    fn layer_stubs(&self, s: usize, layer: usize) -> Vec<usize> {
        self.segments.iter().enumerate().filter(|(other, o)| *other != s && self.lnodes[o.from].layer == layer).flat_map(|(_, o)| [o.exit, o.entry]).collect()
    }

    /// 레인 사각형의 세로(TB)·가로(LR) 변이 놓인 교차축 자리, 오름차순. 레인은 소속 범위의 흐름축 전체를 관통하므로
    /// 층 사이 통로 줄과 노드 쪽 다리가 지나는 모든 흐름축 자리에서 이 변들을 만난다. 흐름도 subgraph는 레인이 아니라
    /// 빠진다. 테두리를 피하지 않을 때는 비어 있다.
    fn lane_side_acrosses(&self) -> Vec<usize> {
        if !self.avoids_label_conflicts {
            return Vec::new();
        }
        let mut sides: Vec<usize> = (1..self.blocks.len())
            .filter(|&b| self.blocks[b].registered && self.is_lane(b))
            .flat_map(|b| [self.blocks[b].start, self.blocks[b].start + self.blocks[b].width - 1])
            .collect();
        sides.sort_unstable();
        sides.dedup();
        sides
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

    /// 레인 테두리를 피할 자리가 없는 이름표와 가로 조각이 짧아 빠지는 메시지 이름표가 지나는 레인 사이 간격을 그
    /// 자리에서 늘리고 다시 배선한다. 블록 폭을 처음부터 다시 셈하면 앞서 일직선을 위해 끼운 칸이 사라져 배치가 통째로
    /// 바뀌므로, 간격에 칸만 끼워 나머지 배치를 그대로 둔다. 끼운 뒤 이름표 흠(`LaneLabelFaults`)이 줄지 않거나 폭을
    /// 넘으면 끼우기 전으로 되돌린다. 배치마다 한 번만 정한다.
    fn widen_lane_gaps_for_labels(&mut self) {
        if !self.avoids_label_conflicts || self.direction != Direction::TopDown {
            return;
        }
        let faults_before = self.lane_label_faults();
        let insertions = self.lane_gap_insertions(&faults_before);
        if insertions.is_empty() {
            return;
        }
        let before = self.clone();
        for &(at, count) in &insertions {
            self.insert_across(at, count);
        }
        self.has_made_room_for_labels = true;
        self.refresh_ports();
        self.route_with_message_end_growth();
        if !self.lane_label_faults().is_fewer_than(&faults_before) || (before.fits_width() && !self.fits_width()) {
            *self = before;
        }
    }

    /// 늘릴 레인 사이 간격마다 (끼울 교차축 자리, 칸 수), 자리가 뒤인 것부터 — 뒤에서부터 끼워야 앞 자리가 밀리지 않는다.
    /// 테두리 위 이름표는 제 폭만큼, 짧은 가로 조각은 모자란 만큼 늘리고, 한 간격에 여럿이 겹치면 가장 많이 필요한 만큼만
    /// 끼운다.
    fn lane_gap_insertions(&self, faults: &LaneLabelFaults) -> Vec<(usize, usize)> {
        let mut count_at: BTreeMap<usize, usize> = BTreeMap::new();
        for &(low, high, count) in faults.labels_on_lane_sides.iter().chain(&faults.short_label_runs) {
            if let Some(at) = self.lane_gap_crossed(low, high) {
                let wanted = count_at.entry(at).or_default();
                *wanted = (*wanted).max(count);
            }
        }
        count_at.into_iter().rev().collect()
    }

    /// 세로 배치의 이름표 흠: 레인 테두리를 덮은 채 남은 이름표와 가로 조각이 짧아 빠지는 메시지 이름표.
    fn lane_label_faults(&self) -> LaneLabelFaults {
        let lane_sides = self.lane_side_acrosses();
        let named_routes = self.named_pool_edge_routes();
        let mut labels_on_lane_sides = Vec::new();
        if !lane_sides.is_empty() {
            labels_on_lane_sides.extend(self.segment_labels_on_lane_sides(&lane_sides));
            labels_on_lane_sides.extend(self.blocked_pool_edge_label_runs(&named_routes, &lane_sides));
        }
        LaneLabelFaults { labels_on_lane_sides, short_label_runs: self.short_pool_edge_label_runs(&named_routes) }
    }

    /// 레인 테두리를 덮는 구간 이름표의 (구간 시작, 끝, 이름표 폭). 구간이 짧아 선 옆에 놓인 이름표도 간격을 늘리면
    /// 구간이 길어져 구간 안으로 들어올 수 있다.
    fn segment_labels_on_lane_sides(&self, lane_sides: &[usize]) -> Vec<(usize, usize, usize)> {
        (0..self.segments.len())
            .filter_map(|s| {
                let segment = &self.segments[s];
                let label_x = segment.label_x?;
                let label_width = width_of(&self.segment_labels(s).0);
                covers_any(label_x, label_width, lane_sides).then(|| (segment.exit.min(segment.entry), segment.exit.max(segment.entry), label_width))
            })
            .collect()
    }

    /// 메시지(`named_routes`) 이름표 가운데 가운데 자리가 레인 테두리를 덮고 테두리로 나뉜 조각 어디에도 들어가지 않는
    /// 것의 (조각 시작, 끝, 이름표 폭). 세로 배치에서 이름표를 얹는 가로 조각은 교차축 조각이라 캔버스 열이 교차축
    /// 자리와 같다.
    fn blocked_pool_edge_label_runs(&self, named_routes: &[(PoolEdgeRoute, usize)], lane_sides: &[usize]) -> Vec<(usize, usize, usize)> {
        named_routes
            .iter()
            .filter_map(|(route, label_width)| {
                let run = self.pool_edge_label_run(*label_width, &self.horizontal_runs(&route.waypoints()), lane_sides)?;
                (run.covers_lane_side && starts_between_sides(run.start, run.end, *label_width, lane_sides).next().is_none()).then_some((run.start, run.end, *label_width))
            })
            .collect()
    }

    /// 메시지(`named_routes`) 가운데 가장 긴 가로 조각도 이름표가 들어가기에 짧아 이름표를 그리지 못하는 것의 (그 조각
    /// 시작, 끝, 모자란 칸 수).
    fn short_pool_edge_label_runs(&self, named_routes: &[(PoolEdgeRoute, usize)]) -> Vec<(usize, usize, usize)> {
        named_routes
            .iter()
            .filter_map(|(route, label_width)| {
                let (start, end, _) = self.horizontal_runs(&route.waypoints()).into_iter().next()?;
                let wanted = label_width + POOL_LABEL_RUN_MARGIN;
                (end - start < wanted).then(|| (start, end, wanted - (end - start)))
            })
            .collect()
    }

    /// 이름 있는 풀 간선마다 통로 깊이까지 정한 경로와 이름표 폭. 깊이에 따라 통로와 노드 사이 가로 조각 길이가
    /// 달라진다. 다리를 비켜 놓는 탐색은 비싸서 하지 않으므로, 그린 경로와 꺾임이 다를 수 있다.
    fn named_pool_edge_routes(&self) -> Vec<(PoolEdgeRoute, usize)> {
        if !(0..self.graph.edges.len()).any(|edge| self.is_pool_edge(edge) && !self.graph.edges[edge].label.is_empty()) {
            return Vec::new();
        }
        self.depth_only_pool_edge_routes()
            .into_iter()
            .filter(|route| !self.graph.edges[route.edge].label.is_empty())
            .map(|route| {
                let label_width = width_of(&self.graph.edges[route.edge].label.replace('\n', " "));
                (route, label_width)
            })
            .collect()
    }

    /// 교차축 `low`~`high`를 지나는 선이 가로지르는 레인 사이 간격 가운데 가운데에 가장 가까운 것의 뒤쪽 레인 시작 자리.
    /// 그 자리에 칸을 끼우면 간격만 넓어진다. 이웃 레인과 테두리를 함께 쓰는 하위 레인 사이에는 간격이 없다.
    fn lane_gap_crossed(&self, low: usize, high: usize) -> Option<usize> {
        let lanes: Vec<usize> = (1..self.blocks.len()).filter(|&b| self.blocks[b].registered && self.is_lane(b)).collect();
        let gap_ends = lanes.iter().map(|&b| self.blocks[b].start + self.blocks[b].width - 1).filter_map(|right_side| {
            let next_start = lanes.iter().map(|&b| self.blocks[b].start).filter(|&start| start > right_side).min()?;
            (low < next_start && right_side < high && next_start > right_side + 1).then_some((right_side, next_start))
        });
        gap_ends.min_by_key(|&(right_side, next_start)| (right_side + next_start).abs_diff(low + high)).map(|(_, next_start)| next_start)
    }

    /// 구간 하나로만 이어진 레인 안 노드(도착 쪽은 들어오는 구간이, 출발 쪽은 나가는 구간이 하나뿐)가
    /// 레인 오른쪽 끝에 막혀 상대와 일직선이 못 되면, 레인 안쪽 오른쪽 여백 앞에 모자란 칸 수만큼 열을
    /// 끼워 넣고 노드를 일직선 자리로 옮긴다 — 노드는 레인 안에 온전히 남고, 나머지 배치는 그대로 둔
    /// 채 그 열 뒤쪽만 민다.
    fn widen_for_straight_lines(&mut self) {
        for s in 0..self.segments.len() {
            let segment = &self.segments[s];
            let (from, to) = (segment.from, segment.to);
            if self.lnodes[from].node.is_none() || self.lnodes[to].node.is_none() {
                continue;
            }
            let (exit_offset, entry_offset) = (segment.exit_offset, segment.entry_offset);
            if self.incoming_of[to].len() == 1
                && let Some(desired) = (self.lnodes[from].across + exit_offset).checked_sub(entry_offset)
            {
                self.widen_lane_to_place(to, desired);
            }
            if self.outgoing_of[from].len() == 1
                && let Some(desired) = (self.lnodes[to].across + entry_offset).checked_sub(exit_offset)
            {
                self.widen_lane_to_place(from, desired);
            }
        }
        self.refresh_ports();
    }

    /// 레인 안 노드 `node`를 교차축 `desired`(지금보다 오른쪽)로 옮기되 레인 끝을 넘으면 그만큼 레인을 넓힌다.
    fn widen_lane_to_place(&mut self, node: usize, desired: usize) {
        let block = self.lnode_block[node];
        if desired <= self.lnodes[node].across || !self.is_lane(block) {
            return;
        }
        let insert_at = self.blocks[block].start + self.blocks[block].width - self.block_pad(block);
        let end = desired + self.lnodes[node].footprint();
        if end <= insert_at || self.overlaps_layer_neighbor(node, desired, end) {
            return;
        }
        self.insert_across(insert_at, end - insert_at);
        self.lnodes[node].across = desired;
    }

    /// 노드 `node`를 교차축 `[start, end)`로 옮기면 같은 층의 다른 노드와 겹치는지.
    fn overlaps_layer_neighbor(&self, node: usize, start: usize, end: usize) -> bool {
        let layer = self.lnodes[node].layer;
        self.layer_members[layer].iter().any(|&other| other != node && self.lnodes[other].across < end && start < self.lnodes[other].across + self.lnodes[other].footprint())
    }

    /// 교차축 `at` 위치에 `count`칸을 끼워 넣는다: 그 뒤의 노드·블록은 밀고, `at`을 품은 블록은 넓힌다.
    fn insert_across(&mut self, at: usize, count: usize) {
        for node in &mut self.lnodes {
            if node.across >= at {
                node.across += count;
            }
        }
        for block in &mut self.blocks {
            if block.start >= at {
                block.start += count;
            } else if at < block.start + block.width {
                block.width += count;
            }
        }
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
        self.layer_start[last] + self.layer_total(last) + self.lane_border_margin()
    }

    /// 레인이 있을 때 흐름축 양 끝에서 내용과 레인 선(구분선·끝 테두리) 사이에 두는 칸 수 — 노드가
    /// 선 위에 겹치거나 맞붙지 않게 한다(가로 흐름은 칸이 좁아 한 칸 더). 레인이 없으면 0.
    fn lane_border_margin(&self) -> usize {
        if self.banner_along().is_empty() {
            return 0;
        }
        match self.direction {
            Direction::TopDown => 1,
            Direction::LeftRight => 2,
        }
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

    /// 메시지 이름표를 놓기 전까지 그린다: 그룹, 노드, 흐름과 그 이름표, 메시지 선과 표식.
    fn draw_until_pool_labels(&self, canvas: &mut Canvas) -> DrawnBeforePoolLabels {
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
                    // 흐름축 크기는 메시지 끝 칸 때문에 측정값보다 클 수 있다.
                    let (min_width, min_height) = match self.direction {
                        Direction::TopDown => (node.across_size, node.along_size),
                        Direction::LeftRight => (node.along_size, node.across_size),
                    };
                    shape::draw(canvas, x, y, self.graph.nodes[n].shape, &node.sections, self.theme, min_width, min_height);
                }
                None => {
                    let (start, end) = self.dummy_line_along(i);
                    let kind = self.dummy_kind(i);
                    self.line_along(canvas, start, end, node.across, kind, self.theme.diagram_line);
                }
            }
        }
        canvas.set_edge_mode(true);
        for s in 0..self.segments.len() {
            self.draw_segment(canvas, s);
        }
        let segment_labels: Vec<PlacedLabel> = (0..self.segments.len()).filter_map(|s| self.draw_segment_decorations(canvas, s)).collect();
        self.draw_self_loops(canvas);
        let pool_edge_routes = self.draw_pool_edge_lines(canvas, &segment_labels);
        DrawnBeforePoolLabels { pool_edge_routes, segment_labels }
    }

    /// 메시지 이름표를 `placement` 방식으로 놓고 그룹 제목을 그린다. 놓은 간선 이름표(흐름 이름표 포함)를 돌려준다.
    fn draw_pool_labels_and_titles(&self, canvas: &mut Canvas, before_labels: &DrawnBeforePoolLabels, placement: PoolLabelPlacement) -> Vec<PlacedLabel> {
        let mut placed = before_labels.segment_labels.clone();
        placed.extend(self.draw_pool_edge_labels(canvas, &before_labels.pool_edge_routes, placement));
        canvas.set_edge_mode(false);
        self.draw_group_titles(canvas);
        placed
    }

    /// 풀 간선의 한쪽 끝(흐름축 위치, 교차축 위치, 상대 끝 쪽으로 향하는 교차축 부호). 노드는 상대를
    /// 향한 면 바로 바깥 칸에서, 풀은 상대를 향한 테두리 칸(면)의 흐름축 중앙에서 끝난다. `face_alongs`는 풀 닻 블록마다
    /// 그 면의 흐름축 중앙(`pool_face_alongs()`)이다.
    fn pool_edge_end(&self, end: usize, other: usize, face_alongs: &BTreeMap<usize, usize>) -> (usize, usize, bool) {
        let (lo, hi) = self.pool_edge_end_across_range(end);
        let toward_higher = self.faces_higher_across(end, other);
        if self.is_pool_anchor(end) {
            let along = face_alongs[&self.lnode_block[end]];
            (along, if toward_higher { hi } else { lo }, toward_higher)
        } else {
            // 점 표기(이벤트)는 글자 칸, 상자는 흐름축 가운데에서 나간다.
            let along = self.node_along(end) + if self.lnodes[end].point_anchored { 0 } else { self.lnodes[end].along_size / 2 };
            (along, if toward_higher { hi + 1 } else { lo.saturating_sub(1) }, toward_higher)
        }
    }

    /// 풀 간선 끝이 차지하는 교차축 범위: 풀 닻이면 풀 블록, 노드면 상자.
    fn pool_edge_end_across_range(&self, end: usize) -> (usize, usize) {
        if self.is_pool_anchor(end) {
            let block = &self.blocks[self.lnode_block[end]];
            (block.start, block.start + block.width - 1)
        } else {
            (self.lnodes[end].across, self.lnodes[end].across + self.lnodes[end].across_size - 1)
        }
    }

    /// 풀 간선 끝 `end`가 상대 `other`를 교차축 큰 쪽에서 마주하는지.
    fn faces_higher_across(&self, end: usize, other: usize) -> bool {
        let (lo, hi) = self.pool_edge_end_across_range(end);
        let (other_lo, other_hi) = self.pool_edge_end_across_range(other);
        other_lo + other_hi > lo + hi
    }

    /// 풀 닻에 닿는 메시지 흐름: 끝에서 교차축으로 나가 풀 바로 바깥 통로까지, 통로를 따라 흐름축으로,
    /// 다시 교차축으로 상대 끝까지 — 풀 쪽은 면에 수직으로 닿는다. 메시지 선과 표식을 긋고 그린 경로를 돌려준다. 메시지
    /// 선이 앞서 놓은 구간 이름표(`segment_labels`) 줄을 지나면 이름표 글자 사이 빈칸이 선 글자로 바뀌므로 구간 이름표를
    /// 다시 쓴다(메시지 표식은 남긴다). 메시지 이름표는 모든 선과 표식을 그린 뒤 `draw_pool_edge_labels()`가 얹는다.
    fn draw_pool_edge_lines(&self, canvas: &mut Canvas, segment_labels: &[PlacedLabel]) -> Vec<PoolEdgeRoute> {
        let routes = self.planned_pool_edge_routes();
        for route in &routes {
            debug_assert!(self.channel_fits_its_gap(route), "통로 줄은 끼워 둔 사이 통로 칸 안에 놓여야 한다");
            self.draw_pool_edge_line(canvas, route);
        }
        for label in segment_labels {
            put_text_over_lines(canvas, label.x, label.row, &label.text, self.theme.diagram_label);
        }
        routes
    }

    /// 메시지(`routes`) 이름표를 `placement` 방식으로 얹고 놓은 이름표를 돌려준다. 모든 메시지 선과 표식을 그린 뒤에
    /// 얹어야 뒤에 그린 선이 앞서 놓은 이름표를 덮지 않고, 이름표가 다른 메시지의 선과 표식을 비킬 수 있다. 놓을 자리가
    /// 좁은 메시지부터 얹는다 — 자리가 넓은 이름표가 먼저 좁은 조각을 차지하면 좁은 쪽은 비킬 데가 없다.
    fn draw_pool_edge_labels(&self, canvas: &mut Canvas, routes: &[PoolEdgeRoute], placement: PoolLabelPlacement) -> Vec<PlacedLabel> {
        let mut by_label_room: Vec<&PoolEdgeRoute> = routes.iter().collect();
        by_label_room.sort_by_cached_key(|route| self.pool_edge_label_room(route));
        let lane_sides = self.lane_side_columns();
        by_label_room.into_iter().filter_map(|route| self.draw_pool_edge_label(canvas, route, placement, &lane_sides)).collect()
    }

    /// 메시지 선과 양 끝 표식을 긋는다.
    fn draw_pool_edge_line(&self, canvas: &mut Canvas, route: &PoolEdgeRoute) {
        let cross_direction = match self.direction {
            Direction::TopDown => Direction::LeftRight,
            Direction::LeftRight => Direction::TopDown,
        };
        let style = self.theme.diagram_line;
        let edge = &self.graph.edges[route.edge];
        let (from_along, from_across, from_toward_higher) = route.from;
        let (to_along, to_across, to_toward_higher) = route.to;
        let corners = corner_points(&route.waypoints());
        for piece in corners.windows(2) {
            let ((along0, across0), (along1, across1)) = (piece[0], piece[1]);
            if along0 == along1 {
                self.line_across(canvas, across0, across1, along0, edge.kind, style);
            } else {
                self.line_along(canvas, along0, along1, across0, edge.kind, style);
            }
        }
        for &(along, across) in corners.iter().skip(1).take(corners.len().saturating_sub(2)) {
            let (x, y) = self.to_canvas(along, across);
            canvas.join(x, y, 0, edge.kind, style, true);
        }
        for (marker, along, across, toward_higher) in [(edge.tail, from_along, from_across, from_toward_higher), (edge.head, to_along, to_across, to_toward_higher)] {
            let glyphs = marker_glyphs(marker, cross_direction, toward_higher);
            let count = glyphs.len();
            for (k, glyph) in glyphs.into_iter().enumerate() {
                let offset = count - 1 - k;
                let across = if toward_higher { across + offset } else { across.saturating_sub(offset) };
                let (x, y) = self.to_canvas(along, across);
                canvas.put(x, y, glyph, style);
            }
        }
    }

    /// 메시지 이름표를 얹을 가장 긴 가로 조각이 이름표보다 긴 칸 수. 이름표가 없거나 들어가지 않으면 0이다.
    fn pool_edge_label_room(&self, route: &PoolEdgeRoute) -> usize {
        let label = &self.graph.edges[route.edge].label;
        if label.is_empty() {
            return 0;
        }
        let longest = self.horizontal_runs(&route.waypoints()).into_iter().next().map_or(0, |(start, end, _)| end - start);
        longest.saturating_sub(width_of(&label.replace('\n', " ")))
    }

    /// 메시지 이름표를 경로의 가로 조각에 얹고(들어갈 때만) 놓은 이름표를 돌려준다. `lane_sides`는 가로 조각을 가로지르는
    /// 레인 변의 캔버스 열(`lane_side_columns()`)이다.
    fn draw_pool_edge_label(&self, canvas: &mut Canvas, route: &PoolEdgeRoute, placement: PoolLabelPlacement, lane_sides: &[usize]) -> Option<PlacedLabel> {
        let edge = &self.graph.edges[route.edge];
        if edge.label.is_empty() {
            return None;
        }
        let text = edge.label.replace('\n', " ");
        let width = width_of(&text);
        let runs = self.horizontal_runs(&route.waypoints());
        let run = self.pool_edge_label_run(width, &runs, lane_sides)?;
        let centered = (run.centered, run.row);
        let (x, row) = match placement {
            PoolLabelPlacement::AvoidingLines => self.pool_edge_label_x(canvas, width, &runs, lane_sides).unwrap_or(centered),
            PoolLabelPlacement::Centered => centered,
        };
        put_text_over_lines(canvas, x, row, &text, self.theme.diagram_label);
        Some(PlacedLabel { x, row, text, is_moved: (x, row) != centered })
    }

    /// 풀 간선마다 양 끝의 기본 자리. 한 노드의 한 쪽이나 한 풀의 한 면에 여럿이 붙으면 칸을 나눠 준다(표식이 서로
    /// 덮지 않게). 노드 쪽을 먼저 나눠야 면 칸이 나뉜 노드 쪽 칸 순서를 따라 통로 구간끼리 엇갈리지 않는다.
    fn base_pool_edge_routes(&self) -> Vec<PoolEdgeRoute> {
        let unspread = self.unspread_pool_edge_routes();
        let mut routes = unspread.clone();
        self.spread_shared_end_points(&mut routes, &unspread, false, false);
        self.spread_shared_end_points(&mut routes, &unspread, true, false);
        // 풀끼리 오가는 메시지의 상대 끝도 풀 면이라 처음엔 나누기 전 자리로 줄 선다. 노드 쪽 끝을 나눈 경로가 붙은 면은
        // 나눈 자리로 다시 줄 세워야 면 칸이 상대 끝 순서를 따라, 풀끼리 오가는 경로의 다리와 같은 칸에 놓이지 않는다.
        self.spread_shared_end_points(&mut routes, &unspread, true, true);
        routes
    }

    /// 풀 간선마다 양 끝을 `pool_edge_end()` 자리에 둔 경로(칸 나누기 전).
    fn unspread_pool_edge_routes(&self) -> Vec<PoolEdgeRoute> {
        let face_alongs = self.pool_face_alongs();
        (0..self.graph.edges.len())
            .filter(|&i| self.is_pool_edge(i))
            .map(|i| {
                let edge = &self.graph.edges[i];
                let is_pool_at_from = self.is_pool_anchor(edge.from);
                let (from, to) = (self.pool_edge_end(edge.from, edge.to, &face_alongs), self.pool_edge_end(edge.to, edge.from, &face_alongs));
                PoolEdgeRoute { edge: i, from, to, is_pool_at_from, depth: 1, node_side_jog: None, shares_node_side: false, is_detoured: false }
            })
            .collect()
    }

    /// 풀 닻 블록마다 그 면의 흐름축 중앙. 레인 제목 칸과 그림 끝을 셀 때마다 모든 블록을 훑으므로, 메시지 끝마다 세지
    /// 않고 블록마다 한 번 센다.
    fn pool_face_alongs(&self) -> BTreeMap<usize, usize> {
        (0..self.graph.nodes.len())
            .filter(|&node| self.is_pool_anchor(node))
            .map(|node| self.lnode_block[node])
            .map(|block| (block, (self.banner_start(self.lane_depth(block)) + self.lane_along_end(block)) / 2))
            .collect()
    }

    /// 풀 간선 경로를 찾는다: 통로를 따라 달리는 구간이 겹치면 면에서 더 먼 통로 줄을 주고, 그러고도 교차축 다리끼리
    /// 같은 칸을 같은 방향으로 쓰면 다리를 흐름축으로 비켜 놓는다.
    fn search_pool_edge_routes(&self) -> Vec<PoolEdgeRoute> {
        let mut routes = self.depth_only_pool_edge_routes();
        let _ = self.separate_colliding_legs(&mut routes, &self.leg_obstacles(), &[]);
        routes
    }

    /// 칸을 끼우기 전에 찾아 둔 결정(`pool_route_plans`)을 지금 배치의 기본 자리에 입혀 그릴 경로를 만든다. 결정은
    /// 기본 자리에서 옮긴 칸 수와 통로 깊이라 칸을 끼워 좌표가 밀려도 그대로 맞는다. 끼운 뒤에 다시 찾으면 끼운 칸보다
    /// 깊은 통로 줄이 나와 풀 테두리 위에 그어질 수 있다.
    fn planned_pool_edge_routes(&self) -> Vec<PoolEdgeRoute> {
        let mut routes = self.base_pool_edge_routes();
        if self.pool_route_plans.len() != routes.len() {
            debug_assert!(false, "풀 간선 경로는 칸 끼우기 단계에서 정해 둬야 한다");
            return self.search_pool_edge_routes();
        }
        for (route, plan) in routes.iter_mut().zip(&self.pool_route_plans) {
            plan.apply_to(route);
            if let Some(jog) = plan.jog {
                let return_across = self.jog_return_across(route, jog.return_kind);
                debug_assert!(return_across.is_some(), "칸을 끼워도 비켜 가기 자리는 노드와 통로 사이에 남는다");
                route.node_side_jog = return_across.map(|return_across| NodeSideJog { shifted_along: route.other_end().0.saturating_add_signed(jog.along_shift), return_across, return_kind: jog.return_kind });
            }
        }
        routes
    }

    /// 같은 풀 면 바깥 통로를 함께 쓰는 경로 묶음(경로 번호 오름차순).
    fn face_groups(&self, routes: &[PoolEdgeRoute]) -> Vec<Vec<usize>> {
        let mut channel_groups: BTreeMap<PoolFace, Vec<usize>> = BTreeMap::new();
        for (r, route) in routes.iter().enumerate() {
            channel_groups.entry((route.pool_anchor(self.graph), route.pool_end().2)).or_default().push(r);
        }
        channel_groups.into_values().collect()
    }

    /// 같은 풀 면 바깥 통로를 함께 쓰는 경로들의 통로 깊이를 정한다.
    fn assign_channel_depths(&self, routes: &mut [PoolEdgeRoute]) {
        for members in self.face_groups(routes) {
            let depths = face_group_depths(&members, |r| &routes[r]);
            for (&r, depth) in members.iter().zip(depths) {
                routes[r].depth = depth;
            }
        }
    }

    /// 다른 풀을 곧게 지나는 다리(막힌 칸)와 같은 칸을 같은 방향으로 쓰는 다리가 없어질 때까지, 충돌한 경로 하나의
    /// 끝이나 꺾임을 흐름축으로 한 칸씩 옮긴다. 메시지 선끼리 합쳐지지 않는 것이 먼저이고, 같으면 다리가 노드 상자나
    /// 시퀀스 흐름 끝 칸에 덜 걸리는 옮김을 고른다. 그 순위가 줄어드는 옮김만 받으므로 끝난다. 직교 교차는 충돌이
    /// 아니다(건너뛰기 글자가 맡는다). 그다음 아직 장애물에 걸린 다리를 비켜 놓고(허락될 때), 비켜 놓았으면 true.
    fn separate_colliding_legs(&self, routes: &mut Vec<PoolEdgeRoute>, obstacles: &LegObstacles, prepared_detours: &[Option<Vec<PoolEdgeRoute>>]) -> bool {
        let prepared_paths: Vec<Option<RoutePoints>> = prepared_detours.iter().zip(routes.iter()).map(|(prepared, route)| prepared.as_ref().map(|_| route.waypoints())).collect();
        let mut search = self.start_leg_search(std::mem::take(routes));
        search.count_obstacle_hits(|route| self.obstacle_hits(route, obstacles));
        // 순위가 줄어드는 옮김만 받아 끝나지만, 메시지가 아주 많은 문서에서 오래 돌지 않게 경로 수에 비례해 끊는다.
        for _ in 0..LEG_MOVES_PER_ROUTE * search.routes.len() {
            let Some(&first) = search.collisions.first() else { break };
            let candidates = first.movers_in_preference_order().into_iter().flat_map(|r| self.leg_moves(&search.routes, r).into_iter().map(move |moved| (r, moved)));
            match self.best_leg_move(&search, candidates, obstacles, None) {
                Some(trial) if trial.rank < search.rank() => self.accept_leg_move(&mut search, trial),
                _ => break,
            }
        }
        // 미리 찾아 둔 비켜 갈 후보는 그 뒤로 경로가 바뀌지 않았으면 다시 훑지 않고 쓴다.
        let still_prepared: Vec<Option<&Vec<PoolEdgeRoute>>> = prepared_paths.iter().zip(prepared_detours).enumerate().map(|(r, (path, prepared))| prepared.as_ref().filter(|_| path.as_ref() == Some(&search.routes[r].waypoints()))).collect();
        let has_detoured = self.allows_leg_detours && self.detour_blocked_legs(&mut search, obstacles, &still_prepared);
        *routes = search.routes;
        has_detoured
    }

    /// 후보 옮김 가운데 순위가 가장 낮은 것. 순위가 같으면 먼저 본 후보를 둔다. 비켜 놓기(`detour_bounds`가 있을 때)는
    /// 다른 경로의 통로 깊이를 바꾸는 후보를 받지 않고 — 한 다리를 비키려고 다른 메시지 통로를 뒤섞지 않는다 —, 순위가
    /// `detour_bounds` 이하인 후보가 나오면 더 보지 않는다. 후보는 가까운 자리부터 오므로 뒤에서 더 나은 것이 나올 일이
    /// 드물다.
    fn best_leg_move(&self, search: &LegSearch, candidates: impl Iterator<Item = (usize, PoolEdgeRoute)>, obstacles: &LegObstacles, detour_bounds: Option<(usize, usize)>) -> Option<LegMoveTrial> {
        let mut best: Option<LegMoveTrial> = None;
        for (r, moved) in candidates {
            let trial = self.try_leg_move(search, r, moved, obstacles, detour_bounds.is_some());
            if best.as_ref().is_none_or(|best| trial.rank < best.rank) {
                best = Some(trial);
            }
            if best.as_ref().zip(detour_bounds).is_some_and(|(best, good_enough)| best.rank <= good_enough) {
                break;
            }
        }
        best
    }

    /// 다리가 다른 노드 상자나 시퀀스 흐름 끝 칸에 걸린 경로마다, 그 다리를 장애물을 피하는 가장 가까운 흐름축 자리로
    /// 옮겨 본다. 메시지 선끼리 새로 합쳐지지 않고 걸린 수가 주는 옮김만 받는다. 걸리지 않은 경로는 그대로다.
    /// `prepared`는 경로마다 미리 찾아 둔 후보(있으면)다 — 다른 경로가 그 사이 끝 칸을 차지한 후보만 빼고 쓴다. 한 번만
    /// 훑는다 — 메시지가 많은 문서에서는 한 노드나 풀에 경로가 몰려 있어, 옮긴 경로 곁을 다시 훑으면 거의 모든 경로를
    /// 다시 훑는다. 옮긴 경로가 있으면 true.
    fn detour_blocked_legs(&self, search: &mut LegSearch, obstacles: &LegObstacles, prepared: &[Option<&Vec<PoolEdgeRoute>>]) -> bool {
        let mut has_detoured = false;
        let mut end_cells = EndCells::of(&search.routes);
        for r in 0..search.routes.len() {
            if search.obstacle_hits_of[r] == 0 {
                continue;
            }
            let moves = match prepared.get(r).copied().flatten() {
                Some(prepared) => {
                    let still_free: Vec<PoolEdgeRoute> = prepared.iter().filter(|moved| end_cells.keeps_ends_free(&search.routes[r], moved)).cloned().collect();
                    // 미리 찾은 후보는 구간 이름표도 비킨 자리뿐이라, 모두 막혔으면 이름표만 지나는 자리가 남았을 수 있다.
                    if still_free.is_empty() && obstacles.has_labels() { self.detour_moves(&DetourScan { routes: &search.routes, end_cells: &end_cells, obstacles }, r) } else { still_free }
                }
                None => self.detour_moves(&DetourScan { routes: &search.routes, end_cells: &end_cells, obstacles }, r),
            };
            let candidates = moves.into_iter().map(|moved| (r, moved));
            let good_enough = (search.collision_count, search.total_obstacle_hits - search.obstacle_hits_of[r]);
            if let Some(trial) = self.best_leg_move(search, candidates, obstacles, Some(good_enough)).filter(|trial| trial.rank < search.rank()) {
                self.accept_detour_move(search, trial);
                end_cells = EndCells::of(&search.routes);
                has_detoured = true;
            }
        }
        has_detoured
    }

    /// 경로 `r`의 상대 끝 쪽 다리를 장애물에 걸리지 않는 흐름축 자리로 옮긴 후보들: 지금 꺾임에서 쪽마다 가장 가까운
    /// `DETOURS_PER_SIDE`곳을, 가까운 자리부터. 메시지 선끼리의 충돌은 순위가 따로 가린다. 다리가 노드, 흐름, 테두리에
    /// 걸렸는데 그런 자리가 없으면 구간 이름표만 지나는 자리를 받는다 — 이름표 한 칸이 끊기는 편이 노드나 테두리를
    /// 지나는 것보다 낫다.
    fn detour_moves(&self, scan: &DetourScan, r: usize) -> Vec<PoolEdgeRoute> {
        let may_cross_labels = scan.obstacles.has_labels() && scan.routes[r].legs().any(|line| scan.obstacles.hits_besides_labels(&line) > 0);
        let places = self.detour_places(scan, r, may_cross_labels);
        if places.clear.is_empty() && may_cross_labels { places.crossing_labels } else { places.clear }
    }

    /// 장애물에 걸리지 않는 `detour_moves()` 후보. 구간 이름표만 지나는 자리는 받지 않는다.
    fn clear_detour_moves(&self, scan: &DetourScan, r: usize) -> Vec<PoolEdgeRoute> {
        self.detour_places(scan, r, false).clear
    }

    /// 다리를 옮겨 볼 자리를 꺾임 앞뒤로 한 번 훑어 모은 후보: 장애물에 걸리지 않는 것과, `wants_crossing_labels`면
    /// 구간 이름표만 지나는 것까지 받은 것. 이름표를 지나는 자리를 받는 쪽이 같은 자리에서 먼저 차므로, 걸리지 않는 후보를
    /// 다 찾을 때까지만 훑으면 둘 다 찾는다 — 따로 훑으면 걸리지 않는 후보가 없을 때 같은 자리들을 두 번 끝까지 훑는다.
    fn detour_places(&self, scan: &DetourScan, r: usize, wants_crossing_labels: bool) -> DetourCandidates {
        let (bend, lower, higher) = self.detour_reach(&scan.routes[r]);
        let mut places = self.detour_places_on_side(scan, r, lower, wants_crossing_labels);
        places.extend(self.detour_places_on_side(scan, r, higher, wants_crossing_labels));
        DetourCandidates { clear: nearest_places_first(places.clear, bend), crossing_labels: nearest_places_first(places.crossing_labels, bend) }
    }

    /// 다리를 옮겨 볼 흐름축 자리: 지금 꺾임과, 그 앞쪽(가까운 자리부터)과 뒤쪽. 풀 면 안쪽, 꺾임에서
    /// `DETOUR_REACH` 안이다.
    fn detour_reach(&self, route: &PoolEdgeRoute) -> (usize, std::iter::Rev<std::ops::Range<usize>>, std::ops::RangeInclusive<usize>) {
        let (low, high) = self.face_interior(route.pool_anchor(self.graph));
        let bend = route.other_bend_along();
        let (low, high) = (low.max(bend.saturating_sub(DETOUR_REACH)), high.min(bend + DETOUR_REACH));
        (bend, (low..bend.min(high + 1)).rev(), (bend + 1).max(low)..=high)
    }

    /// `alongs` 차례로 훑어 다리가 장애물에 걸리지 않는 후보가 있는 첫 `DETOURS_PER_SIDE`곳과 그 후보들, `wants_crossing_labels`면
    /// 구간 이름표만 지나는 후보까지 받은 첫 `DETOURS_PER_SIDE`곳과 그 후보들.
    fn detour_places_on_side(&self, scan: &DetourScan, r: usize, alongs: impl Iterator<Item = usize>, wants_crossing_labels: bool) -> DetourPlaces {
        let mut places = DetourPlaces::default();
        for along in alongs {
            if places.clear.len() == DETOURS_PER_SIDE && (!wants_crossing_labels || places.crossing_labels.len() == DETOURS_PER_SIDE) {
                break;
            }
            let (mut clear, mut crossing_labels) = (Vec::new(), Vec::new());
            for moved in self.detours_at(&scan.routes[r], scan.end_cells, along) {
                match scan.obstacles.clearance(&moved) {
                    LegClearance::Clear if wants_crossing_labels => {
                        crossing_labels.push(moved.clone());
                        clear.push(moved);
                    }
                    LegClearance::Clear => clear.push(moved),
                    LegClearance::CrossesLabels if wants_crossing_labels => crossing_labels.push(moved),
                    _ => {}
                }
            }
            places.add(along, clear, crossing_labels);
        }
        places
    }

    /// 경로 `r`의 상대 끝 쪽 다리를 흐름축 `along`에서 내는 경로들: 두 끝을 함께 옮겨 곧게 긋기(통로를 따라 달리지
    /// 않아 다른 메시지와 덜 엇갈린다), 상대 끝째로 옮기기, 옮길 수 없는 노드 쪽 끝이면 다리만 비켜 노드를 품은 풀 바깥이나 끝 노드 바로 앞에서 돌아오기.
    fn detours_at(&self, route: &PoolEdgeRoute, end_cells: &EndCells, along: usize) -> Vec<PoolEdgeRoute> {
        let mut unjogged = route.clone();
        unjogged.node_side_jog = None;
        unjogged.is_detoured = true;
        let mut detours: Vec<PoolEdgeRoute> = Vec::new();
        if unjogged.pool_end().0 != along {
            detours.extend(self.with_ends_moved(end_cells, route, unjogged.clone(), &[(true, along), (false, along)]));
        }
        detours.extend(self.with_ends_moved(end_cells, route, unjogged.clone(), &[(true, along)]));
        if unjogged.other_end().0 != along {
            for return_kind in [JogReturn::BesideNodePool, JogReturn::BesideEndNode] {
                if let Some(return_across) = self.jog_return_across(&unjogged, return_kind) {
                    let mut jogged = unjogged.clone();
                    jogged.node_side_jog = Some(NodeSideJog { shifted_along: along, return_across, return_kind });
                    detours.push(jogged);
                }
            }
        }
        detours
    }

    /// 경로들의 조각을 줄별로 색인하고 지금 충돌을 센 탐색 상태. 장애물 수는 충돌이 있을 때만 따로 센다.
    fn start_leg_search(&self, routes: Vec<PoolEdgeRoute>) -> LegSearch {
        let face_groups = self.face_groups(&routes);
        let mut face_group_of = vec![0; routes.len()];
        for (g, members) in face_groups.iter().enumerate() {
            for &r in members {
                face_group_of[r] = g;
            }
        }
        let pieces_of = routes.iter().enumerate().map(|(r, route)| self.route_pieces(route, r)).collect();
        let obstacle_hits_of = vec![0; routes.len()];
        let mut search = LegSearch { routes, face_groups, face_group_of, pieces_of, pieces_on_line: BTreeMap::new(), collisions: Vec::new(), collision_count: 0, obstacle_hits_of, total_obstacle_hits: 0 };
        search.reindex_pieces();
        search
    }

    /// 경로 `r`을 `moved`로 바꿨을 때의 순위. 같은 면 묶음의 통로 깊이만 다시 정하고, 옮긴 경로와 깊이가 바뀐 경로의
    /// 조각만 나머지 조각 색인과 견준다.
    fn try_leg_move(&self, search: &LegSearch, r: usize, moved: PoolEdgeRoute, obstacles: &LegObstacles, keeps_other_depths: bool) -> LegMoveTrial {
        let members = &search.face_groups[search.face_group_of[r]];
        let depths = face_group_depths(members, |m| if m == r { &moved } else { &search.routes[m] });
        if keeps_other_depths && members.iter().zip(&depths).any(|(&m, &depth)| m != r && depth != search.routes[m].depth) {
            return LegMoveTrial { changed: Vec::new(), obstacle_hits: Vec::new(), rank: (usize::MAX, usize::MAX) };
        }
        let changed: Vec<(usize, PoolEdgeRoute)> = members
            .iter()
            .zip(depths)
            .filter(|&(&m, depth)| m == r || depth != search.routes[m].depth)
            .map(|(&m, depth)| {
                let mut route = if m == r { moved.clone() } else { search.routes[m].clone() };
                route.depth = depth;
                (m, route)
            })
            .collect();
        let affected: BTreeSet<usize> = changed.iter().map(|(m, _)| *m).collect();
        let old_pieces: Vec<RoutePiece> = affected.iter().flat_map(|&m| search.pieces_of[m].iter().copied()).collect();
        let new_pieces: Vec<RoutePiece> = changed.iter().flat_map(|(m, route)| self.route_pieces(route, *m)).collect();
        let collisions = search.collision_count - search.collisions_involving(&old_pieces, &affected) + search.collisions_involving(&new_pieces, &affected);
        if collisions > search.collision_count {
            // 메시지 충돌이 늘면 받을 수 없으므로 장애물은 세지 않는다.
            return LegMoveTrial { changed, obstacle_hits: Vec::new(), rank: (collisions, usize::MAX) };
        }
        let obstacle_hits: Vec<usize> = changed.iter().map(|(_, route)| self.obstacle_hits(route, obstacles)).collect();
        let old_hits: usize = affected.iter().map(|&m| search.obstacle_hits_of[m]).sum();
        let total_hits = search.total_obstacle_hits - old_hits + obstacle_hits.iter().sum::<usize>();
        LegMoveTrial { changed, obstacle_hits, rank: (collisions, total_hits) }
    }

    fn accept_leg_move(&self, search: &mut LegSearch, trial: LegMoveTrial) {
        for ((m, route), hits) in trial.changed.into_iter().zip(trial.obstacle_hits) {
            search.pieces_of[m] = self.route_pieces(&route, m);
            search.total_obstacle_hits = search.total_obstacle_hits - search.obstacle_hits_of[m] + hits;
            search.obstacle_hits_of[m] = hits;
            search.routes[m] = route;
        }
        search.reindex_pieces();
    }

    /// 비켜 놓기 옮김을 받는다. 충돌 목록을 다시 만들지 않고 바뀐 경로의 조각만 줄 색인에서 바꾸며 충돌 수는 시험에서
    /// 센 값을 쓴다 — 비켜 놓기는 충돌 수만 보므로, 메시지가 많은 문서에서 옮길 때마다 모든 줄을 다시 짝짓지 않는다.
    fn accept_detour_move(&self, search: &mut LegSearch, trial: LegMoveTrial) {
        search.collision_count = trial.rank.0;
        search.collisions.clear();
        for ((m, route), hits) in trial.changed.into_iter().zip(trial.obstacle_hits) {
            search.replace_pieces(m, self.route_pieces(&route, m));
            search.total_obstacle_hits = search.total_obstacle_hits - search.obstacle_hits_of[m] + hits;
            search.obstacle_hits_of[m] = hits;
            search.routes[m] = route;
        }
    }

    /// 경로 조각이 장애물에 하나라도 걸리는지. 걸린 수를 다 세지 않고 처음 걸린 데서 멈춘다.
    fn hits_any_obstacle(&self, route: &PoolEdgeRoute, obstacles: &LegObstacles) -> bool {
        route.legs().any(|line| obstacles.hits(&line) > 0)
    }

    /// 경로의 다리가 걸린 장애물(노드 상자, 흐름 끝 칸, 흐름 선, 테두리, 배너) 수.
    fn obstacle_hits(&self, route: &PoolEdgeRoute, obstacles: &LegObstacles) -> usize {
        route.legs().map(|line| obstacles.hits(&line)).sum()
    }

    /// 메시지 다리가 지나면 안 되는 칸: 노드 상자(풀 닻 말고)와 시퀀스 흐름의 끝 칸, 끝 표식, 그리고 흐름 선. 선의 끝
    /// 칸은 건너뛰기 글자가 되지 못하고 이음 글자로 합쳐진다. 칸을 끼운 뒤 배선을 다시 하기 전에도 부르므로 흐름의
    /// 접점은 노드의 지금 자리에서 다시 센다.
    fn leg_obstacles(&self) -> LegObstacles {
        let mut obstacles = LegObstacles::default();
        for (i, lnode) in self.lnodes.iter().enumerate().filter(|(i, lnode)| lnode.node.is_some() && !self.is_pool_anchor(*i) && lnode.along_size > 0 && lnode.across_size > 0) {
            let along = self.node_along(i);
            obstacles.add(CellRect { along: (along, along + lnode.along_size - 1), across: (lnode.across, lnode.across + lnode.across_size - 1) });
        }
        let mut line_end_cells: Vec<(usize, usize)> = Vec::new();
        for (s, segment) in self.segments.iter().enumerate() {
            let (top, bottom, gap_start) = self.segment_span(s);
            let (exit, entry) = self.current_ports(s);
            line_end_cells.extend(segment.is_first.then_some((top, exit)));
            line_end_cells.extend(segment.is_last.then_some((bottom, entry)));
            line_end_cells.extend(self.segment_marker_glyphs_at(s, exit, entry).into_iter().map(|(along, across, _)| (along, across)));
            if exit == entry {
                obstacles.add_flow_line(false, exit, (top, bottom));
            } else if let Some(channel) = self.channel_position(s, gap_start) {
                obstacles.add_flow_line(false, exit, (top, channel));
                obstacles.add_flow_line(true, channel, (exit, entry));
                obstacles.add_flow_line(false, entry, (channel, bottom));
            }
            if let Some(label_cells) = self.segment_label_cells(s, gap_start, exit).filter(|_| self.avoids_label_conflicts) {
                obstacles.add_label(label_cells);
            }
        }
        line_end_cells.sort_unstable();
        line_end_cells.dedup();
        for (along, across) in line_end_cells {
            obstacles.add(CellRect { along: (along, along), across: (across, across) });
        }
        for (i, lnode) in self.lnodes.iter().enumerate().filter(|(_, lnode)| lnode.node.is_none()) {
            obstacles.add_flow_line(false, lnode.across, self.dummy_line_along(i));
        }
        self.add_group_frames(&mut obstacles);
        obstacles.index_cells();
        obstacles
    }

    /// 세로 배치에서 구간 이름표가 차지하는 통로 줄 칸. 메시지 선이 그 위를 지나면 이름표가 끊긴다. 배선 뒤 칸을 끼워
    /// 노드가 밀렸으면 이름표도 출발 접점(`current_exit`)과 함께 밀린 것으로 센다.
    fn segment_label_cells(&self, s: usize, gap_start: usize, current_exit: usize) -> Option<CellRect> {
        let segment = &self.segments[s];
        let label_x = segment.label_x?;
        let channel = self.channel_position(s, gap_start)?;
        let label_width = width_of(&self.segment_labels(s).0);
        let start = (label_x + current_exit).checked_sub(segment.exit)?;
        (label_width > 0).then(|| CellRect { along: (channel, channel), across: (start, start + label_width - 1) })
    }

    /// 풀, 레인, 그룹 테두리의 변과 레인 제목 칸(배너). 배너는 지나면 제목과 구분선이 끊긴다.
    fn add_group_frames(&self, obstacles: &mut LegObstacles) {
        // `banner_start`는 부를 때마다 모든 블록을 훑으므로 깊이별로 한 번만 센다.
        let banner_starts = self.banner_starts();
        let banner_start = |depth: usize| banner_starts[depth.min(banner_starts.len() - 1)];
        for b in (1..self.blocks.len()).filter(|&b| self.blocks[b].registered) {
            let block = &self.blocks[b];
            let across = (block.start, block.start + block.width - 1);
            let along = if self.is_lane(b) {
                let depth = self.lane_depth(b);
                obstacles.add(CellRect { along: (banner_start(depth), banner_start(depth + 1)), across });
                (banner_start(depth), self.lane_along_end(b))
            } else {
                (self.group_top(b), self.group_bottom(b))
            };
            for fixed in [along.0, along.1] {
                obstacles.add_frame_side(true, fixed, across);
            }
            for fixed in [across.0, across.1] {
                obstacles.add_frame_side(false, fixed, along);
            }
        }
    }

    /// 구간의 접점(출발, 도착 교차축)을 노드의 지금 자리에서 센 값. 배선 뒤 칸을 끼우면 `exit`, `entry`는 낡는다.
    fn current_ports(&self, s: usize) -> (usize, usize) {
        let segment = &self.segments[s];
        (self.lnodes[segment.from].across + segment.exit_offset, self.lnodes[segment.to].across + segment.entry_offset)
    }

    /// 가상 노드가 제 층을 곧게 지나는 선의 흐름축 범위. 층 양옆 통로까지 한 칸씩 물려 그려야 그룹 테두리와 만나는
    /// 칸이 `┼`가 된다.
    fn dummy_line_along(&self, lnode: usize) -> (usize, usize) {
        let layer = self.lnodes[lnode].layer;
        (self.layer_start[layer].saturating_sub(1), self.layer_start[layer] + self.layer_total(layer))
    }

    /// 경로의 곧은 조각들. 교차축 조각은 같은 부모 안의 다른 블록(풀)을 통째로 지나면 막힌 칸이고, 흐름축 조각은
    /// 노드 쪽 비켜 가기와 장애물을 피해 옮긴 경로의 통로 줄이 막힌 칸이다. 다른 통로 줄끼리는 통로 깊이가 이미
    /// 갈랐지만, 옮긴 경로의 통로 구간은 다른 경로의 면 칸을 넘어가 같은 깊이 줄에서 겹칠 수 있다.
    fn route_pieces(&self, route: &PoolEdgeRoute, r: usize) -> Vec<RoutePiece> {
        let jog_across = route.node_side_jog.map(|jog| jog.return_across);
        route
            .straight_lines()
            .map(|StraightLine { is_across, fixed, low, high }| {
                let is_blocking = if is_across { self.passes_through_sibling_block(route, low, high) } else { route.is_detoured || jog_across == Some(fixed) };
                RoutePiece { route: r, is_across, fixed, low, high, is_blocking }
            })
            .collect()
    }

    /// 교차축 구간 `low..=high`가 경로 풀 블록과 같은 부모 안의 다른 블록을 끝이 아닌 채로 통째로 지나는지. 칸을
    /// 끼우기 전에는 깊어진 통로 줄이 그 블록의 먼 테두리에 놓일 수 있는데, 칸을 끼우면 블록을 지나게 되므로 테두리에
    /// 닿기만 해도 지나는 것으로 센다.
    fn passes_through_sibling_block(&self, route: &PoolEdgeRoute, low: usize, high: usize) -> bool {
        let block = self.lnode_block[route.pool_anchor(self.graph)];
        let parent = self.block_parent(block).unwrap_or(0);
        self.blocks[parent].children.iter().any(|&child| match child {
            Child::Block(b) => low <= self.blocks[b].start && self.blocks[b].start + self.blocks[b].width - 1 <= high,
            Child::Node(_) => false,
        })
    }

    /// 경로 `r`을 흐름축으로 한 칸 비켜 놓은 후보들: 상대 끝, 풀 쪽 끝, 두 끝을 함께(곧은 경로가 곧게 남게) 옮기고,
    /// 옮길 수 없는 노드 쪽 끝은 꺾임만 옮겨 노드 앞에서 돌아오게 한다(풀 쪽 끝도 그 흐름축으로 옮겨 곧게 지나는 것까지).
    /// 저마다 상대 끝 쪽(통로를 덜 달리는 쪽)부터.
    fn leg_moves(&self, routes: &[PoolEdgeRoute], r: usize) -> Vec<PoolEdgeRoute> {
        let route = &routes[r];
        let (pool_along, other_along) = (route.pool_end().0, route.other_end().0);
        let toward_pool = steps_toward_first(other_along, pool_along);
        let mut end_sets: Vec<Vec<(bool, usize)>> = toward_pool.iter().map(|&along| vec![(true, along)]).collect();
        end_sets.extend(steps_toward_first(pool_along, other_along).into_iter().map(|along| vec![(false, along)]));
        if pool_along == other_along {
            end_sets.extend(toward_pool.iter().map(|&along| vec![(true, along), (false, along)]));
        }
        let end_cells = EndCells::of(routes);
        let mut moves: Vec<PoolEdgeRoute> = end_sets.iter().filter_map(|ends| self.with_ends_moved(&end_cells, route, route.clone(), ends)).collect();
        if let Some(return_across) = self.jog_return_across(route, JogReturn::BesideNodePool).filter(|_| route.node_side_jog.is_none()) {
            for shifted_along in toward_pool.into_iter().filter(|&along| self.is_in_face_interior(route, along)) {
                let mut jogged = route.clone();
                jogged.node_side_jog = Some(NodeSideJog { shifted_along, return_across, return_kind: JogReturn::BesideNodePool });
                moves.extend(self.with_ends_moved(&end_cells, route, jogged.clone(), &[(false, shifted_along)]));
                moves.push(jogged);
            }
        }
        moves
    }

    /// 경로 `own`을 바꾼 `moved`의 끝들을 `(상대 끝인지, 흐름축)`대로 옮긴 경로. 하나라도 옮길 수 없으면 `None`.
    fn with_ends_moved(&self, end_cells: &EndCells, own: &PoolEdgeRoute, mut moved: PoolEdgeRoute, ends: &[(bool, usize)]) -> Option<PoolEdgeRoute> {
        for &(is_other_end, along) in ends {
            if !self.can_move_end(end_cells, own, &moved, is_other_end, along) {
                return None;
            }
            moved.end_mut(is_other_end).0 = along;
        }
        Some(moved)
    }

    /// 경로 `own`(을 바꾼 `route`)의 한 끝을 흐름축 `along`으로 옮겨도 되는지: 풀 면이면 면 안쪽, 노드면 상자
    /// 안쪽(점 표기는 안 됨)에 머물고 다른 경로의 끝 칸과 겹치지 않아야 한다.
    fn can_move_end(&self, end_cells: &EndCells, own: &PoolEdgeRoute, route: &PoolEdgeRoute, is_other_end: bool, along: usize) -> bool {
        let (anchor, end) = if is_other_end { (route.other_anchor(self.graph), route.other_end()) } else { (route.pool_anchor(self.graph), route.pool_end()) };
        let stays_on_its_node = if self.is_pool_anchor(anchor) {
            let (low, high) = self.face_interior(anchor);
            (low..=high).contains(&along)
        } else {
            route.node_side_jog.is_none() && self.node_end_range(anchor).is_some_and(|(low, high)| (low..=high).contains(&along))
        };
        stays_on_its_node && end_cells.is_free_for(own, (along, end.1))
    }

    /// `along`이 경로 풀 면의 안쪽 흐름축 범위에 드는지. 최상위 풀들은 흐름축 범위가 같으므로 다리가 지나는 다른
    /// 풀의 배너와 모서리도 함께 피한다.
    fn is_in_face_interior(&self, route: &PoolEdgeRoute, along: usize) -> bool {
        let (low, high) = self.face_interior(route.pool_anchor(self.graph));
        (low..=high).contains(&along)
    }

    /// 풀 면에서 끝을 둘 수 있는 흐름축 범위: 배너 구분선과 아래 모서리를 뺀 면 안쪽.
    fn face_interior(&self, anchor: usize) -> (usize, usize) {
        let block = self.lnode_block[anchor];
        (self.banner_start(self.lane_depth(block) + 1) + 1, self.lane_along_end(block).saturating_sub(1))
    }

    /// 노드 쪽 끝을 흐름축으로 옮길 수 있는 범위: 상자 모서리를 뺀 안쪽. 점 표기 노드는 글자 칸에서만 나간다.
    fn node_end_range(&self, node: usize) -> Option<(usize, usize)> {
        let lnode = &self.lnodes[node];
        let along = self.node_along(node);
        (!lnode.point_anchored && lnode.along_size >= 3).then(|| (along + 1, along + lnode.along_size - 2))
    }

    /// 노드 쪽 다리를 비켜 놓았다가 노드 흐름축으로 되돌아올 교차축 칸(`kind` 자리). 노드 쪽 끝과 통로 사이에 들지
    /// 않으면 비켜 갈 수 없다.
    fn jog_return_across(&self, route: &PoolEdgeRoute, kind: JogReturn) -> Option<usize> {
        let node = route.other_anchor(self.graph);
        if self.is_pool_anchor(node) {
            return None;
        }
        let (_, end_across, _) = route.other_end();
        let channel = route.channel();
        let is_channel_higher = channel > end_across;
        let return_across = match kind {
            JogReturn::BesideNodePool => self.across_beside_node_pool(route, node, is_channel_higher)?,
            JogReturn::BesideEndNode => step_across(end_across, is_channel_higher)?,
        };
        let is_between = if is_channel_higher { end_across < return_across && return_across < channel } else { channel < return_across && return_across < end_across };
        is_between.then_some(return_across)
    }

    /// 노드를 품은 풀(경로 풀과 같은 부모 안의 블록) 바로 바깥, 통로 쪽 교차축 칸. 노드가 풀에 없으면 노드 쪽 끝 바로 옆.
    fn across_beside_node_pool(&self, route: &PoolEdgeRoute, node: usize, is_channel_higher: bool) -> Option<usize> {
        let pool_parent = self.block_parent(self.lnode_block[route.pool_anchor(self.graph)]).unwrap_or(0);
        let mut block = self.lnode_block[node];
        while block != 0 && self.block_parent(block) != Some(pool_parent) {
            block = self.block_parent(block).unwrap_or(0);
        }
        if block == 0 {
            return step_across(route.other_end().1, is_channel_higher);
        }
        if is_channel_higher { Some(self.blocks[block].start + self.blocks[block].width) } else { self.blocks[block].start.checked_sub(1) }
    }

    /// 경로의 통로 줄이 그 사이 통로 안(두 면 사이)에 드는지.
    fn channel_fits_its_gap(&self, route: &PoolEdgeRoute) -> bool {
        let gap = self.channel_gap_of(route);
        let channel = route.channel();
        gap.low_face.is_none_or(|face| face < channel) && gap.high_face.is_none_or(|face| channel < face)
    }

    /// 경로를 통로가 놓이는 사이 통로(풀 쪽 끝의 면과 그 면이 마주한 이웃 사이)별로 묶는다.
    fn routes_by_channel_gap(&self, routes: &[PoolEdgeRoute]) -> BTreeMap<ChannelGap, Vec<usize>> {
        let mut gaps: BTreeMap<ChannelGap, Vec<usize>> = BTreeMap::new();
        for (r, route) in routes.iter().enumerate() {
            gaps.entry(self.channel_gap_of(route)).or_default().push(r);
        }
        gaps
    }

    /// 경로의 통로가 놓이는 사이 통로. 풀 블록과 같은 부모 안의 형제 가운데 그 면이 마주한 가장 가까운 것이
    /// 맞은편이다 — 맞은편 풀의 면에서 센 통로도 같은 사이 통로로 묶인다.
    fn channel_gap_of(&self, route: &PoolEdgeRoute) -> ChannelGap {
        let (_, face, toward_higher) = route.pool_end();
        let block = self.lnode_block[route.pool_anchor(self.graph)];
        let parent = self.block_parent(block).unwrap_or(0);
        let sibling_extents = self.blocks[parent].children.iter().filter_map(|&child| match child {
            Child::Block(b) if b == block => None,
            Child::Block(b) => Some((self.blocks[b].start, self.blocks[b].start + self.blocks[b].width - 1)),
            Child::Node(n) => Some((self.lnodes[n].across, self.lnodes[n].across + self.lnodes[n].footprint().max(1) - 1)),
        });
        if toward_higher {
            ChannelGap { low_face: Some(face), high_face: sibling_extents.map(|(start, _)| start).filter(|&start| start > face).min() }
        } else {
            ChannelGap { low_face: sibling_extents.map(|(_, end)| end).filter(|&end| end < face).max(), high_face: Some(face) }
        }
    }

    /// 같은 끝 자리(`is_pool`이면 풀 닻의 면, 아니면 노드의 한 쪽)에 붙는 간선 끝 묶음.
    fn shared_end_groups(&self, routes: &[PoolEdgeRoute], is_pool: bool) -> BTreeMap<EndSide, Vec<EndAttachment>> {
        let mut groups: BTreeMap<EndSide, Vec<EndAttachment>> = BTreeMap::new();
        for (r, route) in routes.iter().enumerate() {
            let edge = &self.graph.edges[route.edge];
            for (anchor, end, other, is_from) in [(edge.from, route.from, route.to, true), (edge.to, route.to, route.from, false)] {
                if self.is_pool_anchor(anchor) == is_pool {
                    groups.entry((anchor, end.2)).or_default().push(EndAttachment { other_along: other.0, route: r, is_from });
                }
            }
        }
        groups
    }

    /// 한 풀 면이나 한 노드의 한 쪽에 붙는 끝이 둘 이상이면 나누기 전 자리(`unspread`)를 둘러 칸을 나눠 준다 — 상대
    /// 끝의 흐름축 순서대로 놓아 통로를 달리는 구간끼리 엇갈리지 않게 한다. 하나뿐이면 나누기 전 자리 그대로다.
    /// `only_beside_shared_node_sides`면 노드 쪽 끝을 나눠 받은 경로가 붙은 면만 다시 나눈다.
    fn spread_shared_end_points(&self, routes: &mut [PoolEdgeRoute], unspread: &[PoolEdgeRoute], is_pool: bool, only_beside_shared_node_sides: bool) {
        for ((anchor, _), mut attachments) in self.shared_end_groups(routes, is_pool) {
            if attachments.len() < 2 || (only_beside_shared_node_sides && !attachments.iter().any(|a| routes[a.route].shares_node_side)) {
                continue;
            }
            let range = if is_pool { self.face_interior(anchor) } else { self.node_end_spread_range(anchor) };
            // 폭 때문에 늘리지 못한 노드는 끝이 칸 밖으로 밀려나지 않게 지금 칸에 모아 둔다.
            if !is_pool && range.1 + 1 < range.0 + attachments.len() {
                continue;
            }
            attachments.sort_unstable_by_key(|a| (a.other_along, a.route, a.is_from));
            let first = &attachments[0];
            let center = if first.is_from { unspread[first.route].from.0 } else { unspread[first.route].to.0 };
            for (attachment, along) in attachments.iter().zip(spread_face_points(center, attachments.len(), range)) {
                let route = &mut routes[attachment.route];
                route.shares_node_side |= !is_pool;
                let end = if attachment.is_from { &mut route.from } else { &mut route.to };
                end.0 = along;
            }
        }
    }

    /// 노드 쪽 끝을 나눠 놓을 흐름축 범위: 상자는 모서리를 뺀 안쪽, 점 표기는 글자 칸부터 노드 흐름축 끝까지.
    fn node_end_spread_range(&self, node: usize) -> (usize, usize) {
        let along = self.node_along(node);
        self.node_end_range(node).unwrap_or((along, along + self.lnodes[node].along_size.max(1) - 1))
    }

    /// 노드마다 한 쪽에 붙는 메시지 끝이 저마다 한 칸씩 들어갈 흐름축 크기(끝이 하나뿐이면 0). 쪽은 교차축 자리만으로
    /// 정해지므로 배선 전에도 셀 수 있다 — 늘린 크기로 처음부터 배선해 배선을 한 번 더 하지 않는다.
    fn along_sizes_for_message_ends(&self) -> Vec<usize> {
        let mut end_counts: BTreeMap<EndSide, usize> = BTreeMap::new();
        for edge in self.graph.edges.iter().enumerate().filter(|&(i, _)| self.is_pool_edge(i)).map(|(_, edge)| edge) {
            for (end, other) in [(edge.from, edge.to), (edge.to, edge.from)] {
                if !self.is_pool_anchor(end) {
                    *end_counts.entry((end, self.faces_higher_across(end, other))).or_default() += 1;
                }
            }
        }
        let mut needed = vec![0; self.graph.nodes.len()];
        for ((node, _), count) in end_counts.into_iter().filter(|&(_, count)| count >= 2) {
            needed[node] = needed[node].max(along_size_for_spread_ends(self.lnodes[node].point_anchored, count));
        }
        needed
    }

    fn has_grown_nodes(&self) -> bool {
        self.lnodes.iter().any(|lnode| lnode.along_size > lnode.content_along_size)
    }

    /// 세로 배치에서 노드를 늘려 통로 칸을 끼운 그림이 폭을 넘었으면, 늘림이 작은 노드부터 몇 개까지 늘려도 폭 안에
    /// 드는지 찾아 그만큼만 늘린다. 그림 대신 원문이 나오는 것보다 넘치게 만든 노드의 끝이 한 칸에 모이는 편이 낫다.
    /// 하나도 늘리지 않아도 넘치면 다 늘린 채로 둔다(어차피 폭 안에 들지 않는다). `before_channel_room`은 통로 칸을
    /// 끼우기 전 상태라, 후보마다 거기서 배선과 칸 끼우기만 다시 한다. 이분 탐색이라 다시 하는 횟수는 로그로 는다.
    fn keep_only_growth_that_fits(&mut self, before_channel_room: Layout<'a>) {
        let mut growth_order: Vec<usize> = (0..self.graph.nodes.len()).filter(|&n| before_channel_room.lnodes[n].along_size > before_channel_room.lnodes[n].content_along_size).collect();
        growth_order.sort_by_key(|&n| (before_channel_room.lnodes[n].along_size - before_channel_room.lnodes[n].content_along_size, n));
        let mut best = before_channel_room.arranged_with_growth_of(&growth_order[..0]);
        if !best.fits_width() {
            return;
        }
        let (mut fitting, mut overflowing) = (0, growth_order.len());
        while overflowing - fitting > 1 {
            let middle = (fitting + overflowing) / 2;
            let layout = before_channel_room.arranged_with_growth_of(&growth_order[..middle]);
            if layout.fits_width() {
                best = layout;
                fitting = middle;
            } else {
                overflowing = middle;
            }
        }
        *self = best;
    }

    /// `grown` 노드만 늘리고 나머지는 본디 크기로 둔 채 배선과 통로 칸 끼우기를 다시 한 배치.
    fn arranged_with_growth_of(&self, grown: &[usize]) -> Layout<'a> {
        let mut layout = self.clone();
        let mut sizes = vec![0; self.graph.nodes.len()];
        for &n in grown {
            sizes[n] = self.lnodes[n].along_size;
        }
        layout.set_along_sizes(&sizes);
        layout.route();
        layout.finish_with_pool_channel_room();
        layout
    }

    fn fits_width(&self) -> bool {
        self.canvas_size().0 <= self.width_limit
    }

    /// 노드 흐름축 크기를 본디 크기와 `needed` 가운데 큰 쪽으로 둔다. 바뀐 노드가 있으면 true.
    fn set_along_sizes(&mut self, needed: &[usize]) -> bool {
        let mut has_changed = false;
        for (lnode, &needed) in self.lnodes.iter_mut().zip(needed) {
            let along_size = lnode.content_along_size.max(needed);
            has_changed |= along_size != lnode.along_size;
            lnode.along_size = along_size;
        }
        has_changed
    }

    /// 풀 간선 경로를 한 번 찾아 그 결정을 `pool_route_plans`에 남기고 그 경로가 쓸 통로 칸을 끼운다. 다리 비켜 놓기는
    /// 마주 보는 통로의 두 면 줄이 갈린 배치에서 찾아야 하므로, 먼저 통로 깊이만 정한 경로의 칸을 끼우고 그 위에서 찾은
    /// 뒤 더 깊어진 만큼만 더 끼운다. 그릴 때는 다시 찾지 않고 남긴 결정을 쓴다.
    fn reserve_pool_channel_room(&mut self) -> bool {
        if !self.has_pool_edges() {
            self.pool_route_plans.clear();
            return false;
        }
        let has_inserted = self.insert_depth_only_channel_room();
        let (depth_only, searched, has_made_room) = self.search_pool_edge_legs();
        self.pool_route_plans = depth_only.iter().zip(&searched).map(|(base, searched)| PoolRoutePlan::between(base, searched)).collect();
        let deeper_room = room_beyond(&self.channel_room_insertions(&searched), &self.channel_room_insertions(&depth_only));
        self.insert_channel_room(&deeper_room);
        has_inserted || has_made_room || !deeper_room.is_empty()
    }

    /// 통로 깊이만 정한 경로들과, 그 다리를 서로 비키고 장애물을 비켜 놓은 경로들. 비켜 갈 자리가 없는 다리가 있으면
    /// 먼저 자리를 마련하고(마련했으면 셋째 값이 true), 비켜 놓기 전 상태를 폭이 넘칠 때를 위해 둔다.
    fn search_pool_edge_legs(&mut self) -> (Vec<PoolEdgeRoute>, Vec<PoolEdgeRoute>, bool) {
        let mut obstacles = self.leg_obstacles();
        let mut depth_only = self.depth_only_pool_edge_routes();
        let mut prepared_detours = if self.allows_leg_detours { self.prepared_detours(&depth_only, &obstacles) } else { Vec::new() };
        self.before_leg_detours = None;
        if prepared_detours.iter().any(Option::is_some) {
            self.before_leg_detours = Some(Box::new(self.clone()));
        }
        let stranded: Vec<usize> = (0..prepared_detours.len()).filter(|&r| prepared_detours[r].as_ref().is_some_and(Vec::is_empty)).collect();
        let has_made_room = !stranded.is_empty() && self.make_room_for_stranded_legs(&depth_only, &stranded, &obstacles);
        if has_made_room {
            self.has_detoured_legs = true;
            obstacles = self.leg_obstacles();
            depth_only = self.depth_only_pool_edge_routes();
            prepared_detours.clear();
        }
        let mut searched = depth_only.clone();
        let has_detoured = self.separate_colliding_legs(&mut searched, &obstacles, &prepared_detours);
        self.has_detoured_legs |= has_detoured;
        self.has_made_room_for_labels |= obstacles.has_labels() && (has_made_room || has_detoured);
        (depth_only, searched, has_made_room)
    }

    /// 통로 깊이만 정한 경로들이 쓸 통로 칸을 끼운다. 끼웠으면 true.
    fn insert_depth_only_channel_room(&mut self) -> bool {
        let room = self.channel_room_insertions(&self.depth_only_pool_edge_routes());
        self.insert_channel_room(&room);
        !room.is_empty()
    }

    /// 통로 깊이만 정한 경로(`routes`)마다, 다리가 장애물에 걸렸으면 어떤 장애물(구간 이름표 포함)에도 걸리지 않는 비켜
    /// 갈 후보(없으면 빈 목록), 걸리지 않았으면 `None`. 빈 목록인 경로가 자리를 마련해야 할 경로이고, 자리를 마련하지
    /// 않으면 비켜 놓기가 이 후보를 그대로 쓴다.
    fn prepared_detours(&self, routes: &[PoolEdgeRoute], obstacles: &LegObstacles) -> Vec<Option<Vec<PoolEdgeRoute>>> {
        let end_cells = EndCells::of(routes);
        let scan = DetourScan { routes, end_cells: &end_cells, obstacles };
        (0..routes.len()).map(|r| self.hits_any_obstacle(&routes[r], obstacles).then(|| self.clear_detour_moves(&scan, r))).collect()
    }

    /// 비켜 갈 자리가 없는 경로(`stranded`)마다 자리를 마련한다: 끝 노드 바로 앞 칸이 막혔으면 그 앞에 칸을 끼우고,
    /// 아니면 층 사이 간격에 빈 줄을 비운다. 마련한 뒤에도 다리를 메시지 선과 겹치지 않게 비켜 놓을 수 없는 경로의
    /// 자리는 되돌린다 — 쓰이지 않는 빈 칸만 그림에 남기 때문이다. 통로 칸을 끼운 뒤에 불러야 다리가 아직 끼우지 않은
    /// 통로 줄 때문에 풀 안을 지나는 것으로 잘못 보지 않는다. 남긴 자리가 있으면 true.
    fn make_room_for_stranded_legs(&mut self, routes: &[PoolEdgeRoute], stranded: &[usize], obstacles: &LegObstacles) -> bool {
        let rooms: Vec<(usize, LegRoom)> = stranded.iter().filter_map(|&r| self.room_for_leg(&routes[r], obstacles).map(|room| (r, room))).collect();
        if rooms.is_empty() {
            return false;
        }
        self.apply_leg_rooms(rooms.iter().map(|(_, room)| room.clone()));
        let freed = self.freed_legs(&rooms.iter().map(|(r, _)| *r).collect::<Vec<_>>());
        if freed.iter().all(|&is_freed| is_freed) {
            return true;
        }
        // 자리를 마련하기 전 상태는 비켜 놓기 직전 상태로 이미 두었다.
        let Some(before_leg_detours) = self.before_leg_detours.take() else { return true };
        *self = (*before_leg_detours).clone();
        self.before_leg_detours = Some(before_leg_detours);
        let useful: Vec<LegRoom> = rooms.iter().zip(&freed).filter(|(_, is_freed)| **is_freed).map(|((_, room), _)| room.clone()).collect();
        if useful.is_empty() {
            return false;
        }
        self.apply_leg_rooms(useful.into_iter());
        true
    }

    /// 다리 하나를 위해 마련할 자리: 끝 노드 바로 앞 칸이 막혔으면 그 앞에 끼울 칸, 아니면 상대 끝에서 가장 가까운
    /// 층 사이 간격의 빈 줄. 층 사이 간격이 없으면 `None`.
    fn room_for_leg(&self, route: &PoolEdgeRoute, obstacles: &LegObstacles) -> Option<LegRoom> {
        if let Some(front_room) = self.front_room(route, obstacles) {
            return Some(LegRoom::FrontOfEndNode(front_room));
        }
        let gap_count = self.layer_count.saturating_sub(1);
        let other_along = route.other_end().0;
        (0..gap_count).min_by_key(|&layer| self.detour_row(layer).abs_diff(other_along)).map(LegRoom::GapRow)
    }

    /// 자리들을 마련하고 그만큼 통로 칸을 다시 맞춘다.
    fn apply_leg_rooms(&mut self, rooms: impl Iterator<Item = LegRoom>) {
        let mut front_rooms: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        let mut gap_rows = vec![0; self.layer_count.saturating_sub(1)];
        for room in rooms {
            match room {
                LegRoom::FrontOfEndNode(FrontRoom { at, also_shifted }) => front_rooms.entry(at).or_default().extend(also_shifted),
                LegRoom::GapRow(layer) => gap_rows[layer] += 1,
            }
        }
        // 뒤쪽부터 끼워야 앞쪽 자리와 함께 밀 노드가 밀리지 않는다.
        for (at, also_shifted) in front_rooms.into_iter().rev() {
            self.insert_across(at, 1);
            for node in also_shifted {
                self.lnodes[node].across += 1;
            }
        }
        if gap_rows.iter().any(|&count| count > 0) {
            self.gap_detour_rows = gap_rows.iter().enumerate().map(|(layer, count)| self.detour_rows(layer) + count).collect();
            self.route();
        }
        self.insert_depth_only_channel_room();
    }

    /// 경로 `legs`마다 다리를 장애물에 걸리지 않고 메시지 선끼리의 충돌을 늘리지 않는 자리로 옮길 수 있는지.
    fn freed_legs(&self, legs: &[usize]) -> Vec<bool> {
        let obstacles = self.leg_obstacles();
        let mut search = self.start_leg_search(self.depth_only_pool_edge_routes());
        search.count_obstacle_hits(|route| self.obstacle_hits(route, &obstacles));
        let end_cells = EndCells::of(&search.routes);
        let scan = DetourScan { routes: &search.routes, end_cells: &end_cells, obstacles: &obstacles };
        legs.iter()
            .map(|&r| {
                search.obstacle_hits_of[r] == 0 || self.clear_detour_moves(&scan, r).into_iter().any(|moved| self.try_leg_move(&search, r, moved, &obstacles, true).rank.0 <= search.collision_count)
            })
            .collect()
    }

    /// 층 `layer` 다음 간격에서 꼬리 표식 줄 바로 다음 줄(비켜 갈 줄이 놓이는 자리).
    fn detour_row(&self, layer: usize) -> usize {
        self.layer_start[layer] + self.layer_total(layer) + self.gap_tail_rows[layer]
    }

    /// 끝 노드 바로 앞 칸(노드 쪽 끝 다음, 통로 쪽)이 장애물에 막혔으면 그 칸을 비울 자리. 칸은 막은 것과 끝 노드 사이
    /// 어디에 끼워도 되는데, 곧은 흐름이 한쪽 끝만 밀려 꺾이면 안 되므로(흐름 배치는 이 기능이 바꿀 몫이 아니다) 그 흐름의
    /// 다른 끝도 함께 민다. 함께 밀다가 막은 것까지 밀거나 끝 노드를 못 밀게 되면, 또는 민 노드가 이웃 노드나 제 레인
    /// 테두리에 닿으면 그 자리는 쓸 수 없다. 끝 노드에 가까운 자리부터 본다. 쓸 자리가 없으면 `None`.
    fn front_room(&self, route: &PoolEdgeRoute, obstacles: &LegObstacles) -> Option<FrontRoom> {
        let node = route.other_anchor(self.graph);
        let (end_along, end_across, _) = route.other_end();
        let front = self.jog_return_across(route, JogReturn::BesideEndNode)?;
        if !obstacles.covers(end_along, front) {
            return None;
        }
        let blockers = self.nodes_covering(end_along, front);
        // 끼운 자리부터 교차축 큰 쪽이 밀리므로, 통로가 큰 쪽이면 막은 것만, 작은 쪽이면 끝 노드만 밀려야 한다.
        let (must_shift, must_stay): (Vec<usize>, Vec<usize>) = if front > end_across { (blockers.clone(), vec![node]) } else { (vec![node], blockers.clone()) };
        let first = if front > end_across { self.lnodes[node].across + 1 } else { blockers.iter().map(|&b| self.lnodes[b].across).min().unwrap_or(front) + 1 };
        let last = if front > end_across { front } else { self.lnodes[node].across };
        (first..=last).rev().find_map(|at| {
            let also_shifted = self.nodes_shifted_with_straight_flows(at)?;
            let is_shifted = |n: usize| self.lnodes[n].across >= at || also_shifted.contains(&n);
            (must_shift.iter().all(|&n| is_shifted(n)) && !must_stay.iter().any(|&n| is_shifted(n))).then_some(FrontRoom { at, also_shifted })
        })
    }

    /// 칸 `(along, across)`를 덮는 노드 상자들(풀 닻 말고).
    fn nodes_covering(&self, along: usize, across: usize) -> Vec<usize> {
        (0..self.lnodes.len())
            .filter(|&i| {
                let lnode = &self.lnodes[i];
                let node_along = self.node_along(i);
                lnode.node.is_some() && !self.is_pool_anchor(i) && (node_along..node_along + lnode.along_size).contains(&along) && (lnode.across..lnode.across + lnode.across_size).contains(&across)
            })
            .collect()
    }

    /// 교차축 `at`에 칸을 끼울 때 곧은 흐름 구간이 꺾이지 않으려면 함께 밀어야 하는, `at`보다 앞의 노드들. 곧은 구간의
    /// 한쪽 끝만 밀리면 다른 끝도 민다(이어서 따라간다). 민 노드가 같은 층의 밀리지 않는 노드와 겹치거나 `at`을 품지
    /// 않은 블록(레인) 테두리를 넘으면 `None`.
    fn nodes_shifted_with_straight_flows(&self, at: usize) -> Option<Vec<usize>> {
        let mut is_shifted: Vec<bool> = self.lnodes.iter().map(|lnode| lnode.across >= at).collect();
        let straight: Vec<(usize, usize)> = (0..self.segments.len()).filter(|&s| self.current_ports(s).0 == self.current_ports(s).1).map(|s| (self.segments[s].from, self.segments[s].to)).collect();
        let mut also_shifted = Vec::new();
        let mut has_grown = true;
        while has_grown {
            has_grown = false;
            for &(from, to) in &straight {
                for (moved, other) in [(from, to), (to, from)] {
                    if is_shifted[moved] && !is_shifted[other] {
                        is_shifted[other] = true;
                        also_shifted.push(other);
                        has_grown = true;
                    }
                }
            }
        }
        also_shifted.iter().all(|&n| self.fits_one_further(n, at, &is_shifted)).then_some(also_shifted)
    }

    /// `at`보다 앞의 노드 `n`을 교차축으로 한 칸 밀어도 같은 층의 밀리지 않는 노드와 겹치지 않고 제 블록 안에 남는지.
    fn fits_one_further(&self, n: usize, at: usize, is_shifted: &[bool]) -> bool {
        let lnode = &self.lnodes[n];
        let (low, high) = (lnode.across + 1, lnode.across + lnode.footprint().max(1));
        let is_clear_of_neighbors = self.layer_members[lnode.layer].iter().all(|&other| {
            let other_node = &self.lnodes[other];
            is_shifted[other] || other_node.across + other_node.footprint().max(1) <= low || high < other_node.across
        });
        let block = &self.blocks[self.lnode_block[n]];
        let block_grows = self.lnode_block[n] == 0 || (block.start < at && at < block.start + block.width);
        is_clear_of_neighbors && (block_grows || high + 1 < block.start + block.width)
    }

    /// 기본 자리에 통로 깊이만 정한 경로(다리 비켜 놓기 전).
    fn depth_only_pool_edge_routes(&self) -> Vec<PoolEdgeRoute> {
        let mut routes = self.base_pool_edge_routes();
        self.assign_channel_depths(&mut routes);
        routes
    }

    /// 경로들의 통로 깊이가 풀 사이 간격에 다 들어가도록 끼울 (위치, 칸 수).
    fn channel_room_insertions(&self, routes: &[PoolEdgeRoute]) -> Vec<(usize, usize)> {
        let mut insertions: Vec<(usize, usize)> = Vec::new();
        for (gap, members) in self.routes_by_channel_gap(routes) {
            if faces_share_channel_rows(routes, &members) {
                insertions.extend(facing_gap_insertion(gap, routes, &members));
            } else {
                insertions.extend(self.single_face_insertions(routes, &members));
            }
        }
        insertions
    }

    fn insert_channel_room(&mut self, insertions: &[(usize, usize)]) {
        let mut insertions = insertions.to_vec();
        // 뒤쪽부터 끼워야 앞쪽 끼울 자리가 밀리지 않는다.
        insertions.sort_unstable_by(|a, b| b.cmp(a));
        for &(at, count) in &insertions {
            self.insert_across(at, count);
        }
    }

    /// 한 면 쪽 통로만 쓰는 사이 통로에 끼울 (위치, 칸 수): 면마다 가장 깊은 통로가 남는 칸을 넘는 만큼 면 바깥에.
    fn single_face_insertions(&self, routes: &[PoolEdgeRoute], members: &[usize]) -> Vec<(usize, usize)> {
        // 바깥쪽 통로와 이웃 사이에 빈 칸 하나는 남긴다(간격이 한 칸뿐이면 그 칸이 곧 통로다).
        let room = self.gap_across().saturating_sub(1).max(1);
        let mut deepest: BTreeMap<(usize, bool), usize> = BTreeMap::new();
        for &r in members {
            let (_, face, toward_higher) = routes[r].pool_end();
            let depth = deepest.entry((face, toward_higher)).or_default();
            *depth = (*depth).max(routes[r].depth);
        }
        deepest
            .into_iter()
            .filter(|&(_, depth)| depth > room)
            .map(|((face, toward_higher), depth)| (if toward_higher { face + 1 } else { face }, depth - room))
            .collect()
    }

    /// 경로의 가로 조각(`horizontal_runs()`) 가운데 가장 긴 것과 그 가운데 자리(들어갈 때만). `lane_sides`는 가로 조각을
    /// 가로지르는 레인 변의 캔버스 열이다.
    fn pool_edge_label_run(&self, width: usize, runs: &[(usize, usize, usize)], lane_sides: &[usize]) -> Option<PoolEdgeLabelRun> {
        let &(start, end, row) = runs.first()?;
        if end - start < width + POOL_LABEL_RUN_MARGIN {
            return None;
        }
        let centered = centered_label_start(start, end, width);
        let covers_lane_side = covers_any(centered, width, lane_sides);
        Some(PoolEdgeLabelRun { start, end, row, centered, covers_lane_side })
    }

    /// 경로의 가로 조각들(캔버스 열 시작, 끝, 줄), 긴 것부터. 길이가 같으면 경로에서 뒤에 오는 조각이 먼저다 — 가장 긴
    /// 조각에 이름표를 얹던 때부터 같은 길이면 뒤쪽 조각을 골라 왔다.
    fn horizontal_runs(&self, waypoints: &[(usize, usize)]) -> Vec<(usize, usize, usize)> {
        let points: Vec<(usize, usize)> = waypoints.iter().map(|&(along, across)| self.to_canvas(along, across)).collect();
        let mut runs: Vec<(usize, usize, usize)> = points.windows(2).rev().filter(|p| p[0].1 == p[1].1).map(|p| (p[0].0.min(p[1].0), p[0].0.max(p[1].0), p[0].1)).collect();
        runs.sort_by_key(|&(start, end, _)| std::cmp::Reverse(end - start));
        runs
    }

    /// 메시지 이름표를 놓을 자리(첫 칸, 줄). 이름표가 들어가는 가로 조각을 긴 것부터 보며, 조각마다 가운데, 레인 테두리로
    /// 나뉜 조각 안 자리, 그 밖의 자리를 가운데에 가까운 것부터 본다. 흠(`LabelSpotFaults`)이 없는 첫 자리를 고른다 — 앞서
    /// 그린 노드 이름, 이름표, 표식, 레인 테두리, 다른 메시지의 세로선을 덮거나 글자에 붙지 않는다. 그런 자리가 없으면
    /// 흠이 가장 가벼운 자리(같으면 조각 가운데에 가까운 자리)를 고른다. 들어가는 조각이 없으면 `None`. `runs`는 경로의
    /// 가로 조각들(`horizontal_runs()`), `lane_sides`는 그 조각을 가로지르는 레인 변의 캔버스 열이다.
    fn pool_edge_label_x(&self, canvas: &Canvas, width: usize, runs: &[(usize, usize, usize)], lane_sides: &[usize]) -> Option<(usize, usize)> {
        let runs: Vec<(usize, usize, usize)> = runs.iter().copied().filter(|&(start, end, _)| end - start >= width + POOL_LABEL_RUN_MARGIN).collect();
        let faults_at = |x: usize, row: usize| label_spot_faults(canvas, x, row, width, lane_sides);
        let clean = runs.iter().find_map(|&(start, end, row)| label_starts_by_preference(start, end, width, lane_sides).find(|&x| faults_at(x, row) == LabelSpotFaults::default()).map(|x| (x, row)));
        clean.or_else(|| {
            runs.iter()
                .flat_map(|&(start, end, row)| {
                    let centered = centered_label_start(start, end, width);
                    (start + 1..=end - width).map(move |x| (x, row, x.abs_diff(centered)))
                })
                .min_by_key(|&(x, row, distance)| (faults_at(x, row), distance))
                .map(|(x, row, _)| (x, row))
        })
    }

    /// 가로 조각을 세로로 가로지르는 레인 변의 캔버스 열. 세로 배치에서는 레인의 교차축 변이다. 가로 배치의 가로 조각은
    /// 풀 사이 통로나 노드 줄에 놓여 레인의 세로 변(제목 칸 구분선, 양 끝)이 지나는 흐름축 자리에 닿지 않는다.
    fn lane_side_columns(&self) -> Vec<usize> {
        match self.direction {
            Direction::TopDown => self.lane_side_acrosses(),
            Direction::LeftRight => Vec::new(),
        }
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
            // 레인은 항상 실선 — 파선은 Box 그룹(BPMN L3 등)에만 의미가 있다.
            let line = if self.is_lane(b) { LineKind::Solid } else { self.graph.groups[block.group.unwrap()].line };
            canvas.rect(x0, y0, w, h, line, self.theme.diagram_group, false);
            // 구분선: 하위 레인이 없는 레인만 자기 깊이 배너 바로 뒤에 긋는다 — 제목 칸이 자기 깊이만큼만
            // 차지하고 나머지는 내용 영역이다. 하위 레인이 있으면 그 사각형들이 경계를 대신한다.
            if self.is_lane(b) && self.lane_children[b].is_empty() {
                let divider = self.banner_start(self.lane_depth(b) + 1);
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
            // 위→아래의 점 표기 노드(이벤트)는 둘째 줄이 글자 칸보다 들여 쓰여 있어 글자 바로 아래에서 나간다.
            Some(_) if self.direction == Direction::TopDown && self.lnodes[segment.from].point_anchored => self.node_along(segment.from) + 1,
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
        segment.channel.map(|c| gap_start + self.gap_tail_rows[layer] + self.detour_rows(layer) + self.gap_label_room[layer] + c)
    }

    fn detour_rows(&self, layer: usize) -> usize {
        self.gap_detour_rows.get(layer).copied().unwrap_or(0)
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

    /// 구간 양 끝 표식 글자와 그 자리(흐름축, 교차축). 여러 글자면 마지막 글자가 노드에 닿는다.
    fn segment_marker_glyphs(&self, s: usize) -> Vec<(usize, usize, char)> {
        self.segment_marker_glyphs_at(s, self.segments[s].exit, self.segments[s].entry)
    }

    /// 구간 양 끝 표식 글자와 그 자리. 접점을 교차축 `exit`, `entry`에 둔다.
    fn segment_marker_glyphs_at(&self, s: usize, exit: usize, entry: usize) -> Vec<(usize, usize, char)> {
        let segment = &self.segments[s];
        let edge = &self.graph.edges[segment.edge];
        let reversed = self.effective_reversed(segment.edge);
        let (top, bottom, _) = self.segment_span(s);
        let (top_marker, bottom_marker) = if reversed { (edge.head, edge.tail) } else { (edge.tail, edge.head) };
        let mut placed = Vec::new();
        if segment.is_first {
            // 위쪽 끝: 마지막 글자가 노드(top)에 닿고, 앞 글자들은 그 아래로 이어진다.
            let glyphs = marker_glyphs(top_marker, self.direction, true);
            let count = glyphs.len();
            for (k, glyph) in glyphs.into_iter().enumerate() {
                let along = top + (count - 1 - k);
                if along <= bottom {
                    placed.push((along, exit, glyph));
                }
            }
        }
        if segment.is_last {
            let glyphs = marker_glyphs(bottom_marker, self.direction, false);
            let count = glyphs.len();
            for (k, glyph) in glyphs.into_iter().enumerate() {
                if let Some(along) = bottom.checked_sub(count - 1 - k)
                    && along >= top
                {
                    placed.push((along, entry, glyph));
                }
            }
        }
        placed
    }

    /// 구간의 끝 표식과 이름표, 끝 라벨을 그리고 놓은 간선 이름표를 돌려준다.
    fn draw_segment_decorations(&self, canvas: &mut Canvas, s: usize) -> Option<PlacedLabel> {
        let segment = &self.segments[s];
        let edge = &self.graph.edges[segment.edge];
        let reversed = self.effective_reversed(segment.edge);
        let (top, bottom, gap_start) = self.segment_span(s);
        let (label, tail_label, head_label) = self.segment_labels(s);
        let top_marker = if reversed { edge.head } else { edge.tail };
        let marker_style = self.theme.diagram_line;
        for (along, across, glyph) in self.segment_marker_glyphs(s) {
            let (x, y) = self.to_canvas(along, across);
            canvas.put(x, y, glyph, marker_style);
        }
        let label_style = self.theme.diagram_label;
        let mut placed = None;
        match self.direction {
            Direction::TopDown => {
                if !label.is_empty()
                    && let Some(channel) = self.channel_position(s, gap_start)
                    && let Some(x) = segment.label_x
                {
                    canvas.text(x, channel, &label, label_style);
                    placed = Some(PlacedLabel { x, row: channel, text: label, is_moved: false });
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
                    placed = Some(PlacedLabel { x: top + tail_glyphs, row: segment.exit, text: label, is_moved: false });
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
        placed
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

/// 이름표를 `(x, y)`부터 쓰되 이미 글자(노드 이름, 다른 이름표, 표식)가 있는 칸에 걸리는 글자는 쓰지 않는다.
/// 이름표가 들어갈 깨끗한 자리가 없을 때 남의 글자를 지우기보다 이름표 글자를 빠뜨리는 편이 낫다 — 표식과 노드 이름은
/// 그림의 뜻을 지닌다.
fn put_text_over_lines(canvas: &mut Canvas, x: usize, y: usize, text: &str, style: Style) {
    let mut cursor = x;
    for glyph in text.chars() {
        let width = char_width(glyph);
        if width == 0 {
            continue;
        }
        if !(cursor..cursor + width).any(|col| canvas.is_text_cell(col, y)) {
            canvas.put(cursor, y, glyph, style);
        }
        cursor += width;
    }
}

/// 조각 `start`~`end`에 폭 `width` 이름표를 놓아 볼 첫 칸들, 먼저 볼 것부터: 가운데, `sides`(레인 테두리)로 나뉜 조각
/// 안 자리, 조각 안의 나머지 자리(가운데에 가까운 것부터). 앞 칸이 겹칠 수 있다.
fn label_starts_by_preference(start: usize, end: usize, width: usize, sides: &[usize]) -> impl Iterator<Item = usize> + use<> {
    let centered = centered_label_start(start, end, width);
    std::iter::once(centered).chain(starts_between_sides(start, end, width, sides)).chain(starts_nearest_first(start + 1, end - width, centered))
}

/// 캔버스 `row`줄 `x`칸부터 폭 `width` 이름표를 놓으면 생기는 흠. 앞 필드일수록 무거워 필드 순서대로 견준다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct LabelSpotFaults {
    /// 덮는 글자 칸 수(노드 이름, 다른 이름표, 표식).
    covered_text: usize,
    /// 덮는 레인 테두리 수.
    covered_sides: usize,
    /// 덮는 칸 가운데 다른 선과 엇갈리거나 선이 아닌 칸 수.
    crossed_cells: usize,
    /// 바로 옆에 글자가 붙는 끝 수.
    touching_text: usize,
}

/// 캔버스 `row`줄 `x`칸부터 폭 `width` 이름표를 놓을 때의 흠. `sides`는 피할 레인 테두리 열, 오름차순.
fn label_spot_faults(canvas: &Canvas, x: usize, row: usize, width: usize, sides: &[usize]) -> LabelSpotFaults {
    let cells = x..x + width;
    let first_side = sides.partition_point(|&side| side < x);
    LabelSpotFaults {
        covered_text: cells.clone().filter(|&col| canvas.is_text_cell(col, row)).count(),
        covered_sides: sides[first_side..].iter().take_while(|&&side| side < x + width).count(),
        crossed_cells: cells.filter(|&col| !canvas.is_line_run(col, row, 1)).count(),
        touching_text: usize::from(x.checked_sub(1).is_some_and(|left| canvas.is_text_cell(left, row))) + usize::from(canvas.is_text_cell(x + width, row)),
    }
}

/// 조각 `start`~`end`(양 끝은 꺾임이나 표식) 가운데에 맞춘 폭 `width` 이름표의 첫 칸.
fn centered_label_start(start: usize, end: usize, width: usize) -> usize {
    start + (end - start + 1 - width) / 2
}

/// 아래로 이어지는 선 글자와 위로 이어지는 선 글자. 위아래가 이어진 칸에 이름표 글자가 있으면 세로선을 끊은 것이다.
const LINE_GOING_DOWN: &str = "│╎┊├┤┼┬┌┐╭╮△▲";
const LINE_GOING_UP: &str = "│╎┊├┤┼┴└┘╰╯▽▼";

/// 그린 간선 이름표들의 흠. 앞 필드일수록 무거워 필드 순서대로 견준다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct LabelDefects {
    /// 제 칸에 보이지 않는 이름표 글자 수: 다른 글자에 막혀 쓰지 못했거나, 다른 선이나 글자에 덮였거나, 넓은 글자끼리
    /// 겹쳐 줄이 밀렸다.
    hidden_glyphs: usize,
    /// 바로 옆에 다른 글자가 붙은 이름표 끝 수. 붙으면 두 글자가 한 낱말로 읽힌다.
    touching_text: usize,
    /// 이름표 글자가 끊은 세로선(흐름, 메시지, 풀이나 레인 테두리) 칸 수.
    cut_lines: usize,
}

impl LabelDefects {
    fn add(&mut self, other: &LabelDefects) {
        self.hidden_glyphs += other.hidden_glyphs;
        self.touching_text += other.touching_text;
        self.cut_lines += other.cut_lines;
    }
}

/// 캔버스에 놓인 이름표들(`labels`)의 흠. 이름표 줄과 그 위아래 줄만 보며, 줄마다 한 번에 세 줄만 펼쳐 둔다 — 넓은
/// 캔버스에 이름표가 많아도 메모리가 그림 크기만큼 늘지 않는다.
fn label_defects<'l>(canvas: &Canvas, labels: impl IntoIterator<Item = &'l PlacedLabel>) -> LabelDefects {
    let mut labels_by_row: BTreeMap<usize, Vec<&PlacedLabel>> = BTreeMap::new();
    for label in labels {
        labels_by_row.entry(label.row).or_default().push(label);
    }
    let mut defects = LabelDefects::default();
    for (row, labels) in labels_by_row {
        let [above, here, below] = [row.checked_sub(1), Some(row), Some(row + 1)].map(|y| y.map(|y| canvas.aligned_glyphs(y)).unwrap_or_default());
        for label in labels {
            defects.add(&row_label_defects(label, &above, &here, &below));
        }
    }
    defects
}

/// `here`줄에 놓인 이름표 하나의 흠. 세 줄은 `Canvas::aligned_glyphs()`로 펼친 이름표 줄과 그 위아래 줄이다 — 바깥
/// `None`은 캔버스 밖, 안쪽 `None`은 넓은 글자의 둘째 칸이거나 밀려 제 열에 보이지 않는 칸이다.
fn row_label_defects(label: &PlacedLabel, above: &[Option<char>], here: &[Option<char>], below: &[Option<char>]) -> LabelDefects {
    let mut defects = LabelDefects::default();
    let mut column = label.x;
    for glyph in label.text.chars().filter(|&glyph| char_width(glyph) > 0) {
        defects.hidden_glyphs += usize::from(here.get(column).copied() != Some(Some(glyph)));
        column += char_width(glyph);
    }
    let is_text_beside = |x: Option<usize>| x.and_then(|x| here.get(x).copied()).is_some_and(|glyph| glyph.is_none_or(char::is_alphanumeric));
    defects.touching_text = usize::from(is_text_beside(label.x.checked_sub(1))) + usize::from(is_text_beside(Some(column)));
    let continues = |cells: &[Option<char>], x: usize, glyphs: &str| cells.get(x).copied().flatten().is_some_and(|glyph| glyphs.contains(glyph));
    defects.cut_lines = (label.x..column).filter(|&x| continues(above, x, LINE_GOING_DOWN) && continues(below, x, LINE_GOING_UP)).count();
    defects
}

/// `[x, x + width)`가 `sides` 칸을 하나라도 덮는지. `sides`는 오름차순.
fn covers_any(x: usize, width: usize, sides: &[usize]) -> bool {
    let first_at_or_after = sides.partition_point(|&side| side < x);
    sides.get(first_at_or_after).is_some_and(|&side| side < x + width)
}

/// `low`, `high`(선의 꺾임이나 끝) 사이를 `sides` 칸으로 나눈 조각 가운데 폭 `width`가 들어가는 조각 안의 시작 칸들.
/// 조각 가운데가 구간 가운데에 가까운 조각부터, 조각 안에서는 가운데에 맞춘 자리부터 바깥으로 늘어놓는다. 레인 안쪽이든
/// 두 레인 사이 간격이든 들어가는 쪽을 고를 수 있다.
fn starts_between_sides(low: usize, high: usize, width: usize, sides: &[usize]) -> impl Iterator<Item = usize> + use<> {
    let inner_sides = sides.iter().copied().filter(|&side| low < side && side < high);
    let cuts: Vec<usize> = std::iter::once(low).chain(inner_sides).chain(std::iter::once(high)).collect();
    let mut fitting: Vec<(usize, usize)> = cuts.windows(2).filter(|cut| cut[1] - cut[0] > width).map(|cut| (cut[0], cut[1])).collect();
    fitting.sort_by_key(|&(start, end)| (start + end).abs_diff(low + high));
    fitting.into_iter().flat_map(move |(start, end)| {
        let (first, last) = (start + 1, end - width);
        starts_nearest_first(first, last, first + (last - first) / 2)
    })
}

/// `first..=last` 칸들을 `centered`에 가까운 것부터(거리가 같으면 앞 칸부터) — 차례대로 늘어놓아 정렬한 것과 같은
/// 순서를, 긴 조각에서도 앞 몇 칸만 볼 때 모두 늘어놓지 않고 낸다.
fn starts_nearest_first(first: usize, last: usize, centered: usize) -> impl Iterator<Item = usize> {
    let reach = centered.abs_diff(first).max(centered.abs_diff(last));
    (0..=reach).flat_map(move |distance| {
        let before = centered.checked_sub(distance);
        let after = (distance > 0).then(|| centered + distance);
        [before, after].into_iter().flatten().filter(move |&x| first <= x && x <= last)
    })
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

/// 풀 닻 노드와 그 풀이 상대를 마주하는 면(교차축 큰 쪽이면 true).
type PoolFace = (usize, bool);

/// 간선 끝이 붙는 자리: 풀 닻이나 노드, 그리고 교차축 큰 쪽인지.
type EndSide = (usize, bool);

/// 풀 면이나 노드의 한 쪽에 붙는 간선 끝: 상대 끝의 흐름축 위치, 경로 번호, 출발 쪽 끝인지.
struct EndAttachment {
    other_along: usize,
    route: usize,
    is_from: bool,
}

/// 경로의 경유점이나 꺾임점. 많아야 여섯 개(풀 끝, 통로의 두 점, 비켜 가기의 두 점, 상대 끝)라 힙을 쓰지 않고
/// 담는다 — 비켜 갈 자리를 훑을 때 후보마다 다리를 다시 센다.
#[derive(Clone, Copy, Default)]
struct RoutePoints {
    points: [(usize, usize); 6],
    len: usize,
}

impl RoutePoints {
    fn push(&mut self, point: (usize, usize)) {
        self.points[self.len] = point;
        self.len += 1;
    }

    fn pop(&mut self) {
        self.len -= 1;
    }
}

impl PartialEq for RoutePoints {
    fn eq(&self, other: &RoutePoints) -> bool {
        **self == **other
    }
}

impl std::ops::Deref for RoutePoints {
    type Target = [(usize, usize)];

    fn deref(&self) -> &[(usize, usize)] {
        &self.points[..self.len]
    }
}

impl std::ops::DerefMut for RoutePoints {
    fn deref_mut(&mut self) -> &mut [(usize, usize)] {
        &mut self.points[..self.len]
    }
}

/// 풀 닻에 닿는 간선 하나의 경로. 끝은 `pool_edge_end()`와 같은 (흐름축, 교차축, 상대 쪽 부호).
#[derive(Clone)]
struct PoolEdgeRoute {
    edge: usize,
    from: (usize, usize, bool),
    to: (usize, usize, bool),
    /// 통로를 풀 쪽 끝(둘 다 풀이면 출발 쪽)의 면에 둔다.
    is_pool_at_from: bool,
    /// 통로가 면에서 떨어진 칸 수(1 = 면 바로 바깥).
    depth: usize,
    /// 노드 쪽 다리가 막힌 칸을 피해 흐름축으로 비켜 지나면 그 꺾음.
    node_side_jog: Option<NodeSideJog>,
    /// 노드 쪽 끝이 같은 노드 같은 쪽의 다른 끝과 칸을 나눠 받았는지.
    shares_node_side: bool,
    /// 장애물을 피해 다리를 옮겼는지. 옮긴 다리의 통로 구간은 다른 경로의 면 칸을 넘어갈 수 있다.
    is_detoured: bool,
}

/// 통로에서 노드 쪽으로 꺾여 나가는 다리를 흐름축 `shifted_along`에서 내고, 교차축 `return_across` 칸(`return_kind`가
/// 정한 자리)에서 노드 쪽 끝의 흐름축으로 되돌린다 — 노드 쪽 끝은 옮길 수 없고 다리만 비켜야 할 때 쓴다.
#[derive(Clone, Copy)]
struct NodeSideJog {
    shifted_along: usize,
    return_across: usize,
    return_kind: JogReturn,
}

/// 비켜 간 다리가 노드 쪽 끝의 흐름축으로 되돌아오는 교차축 자리.
#[derive(Clone, Copy, PartialEq, Eq)]
enum JogReturn {
    /// 노드를 품은 풀 바로 바깥, 통로 쪽 — 다른 메시지 다리만 비켜 가면 될 때 풀 안을 건드리지 않는다.
    BesideNodePool,
    /// 끝 노드 바로 앞, 통로 쪽 — 풀 안의 다른 노드나 화살촉이 다리 길목에 있을 때 그 앞에서야 노드 쪽으로 꺾는다.
    BesideEndNode,
}

/// 경로의 곧은 조각 하나. `is_across`면 흐름축 `fixed`에서 교차축 `low..=high`를, 아니면 교차축 `fixed`에서
/// 흐름축 `low..=high`를 쓴다.
#[derive(Clone, Copy)]
struct RoutePiece {
    route: usize,
    is_across: bool,
    fixed: usize,
    low: usize,
    high: usize,
    is_blocking: bool,
}

/// 세로 배치의 이름표 흠. 둘 다 (선 조각의 교차축 시작, 끝, 레인 사이 간격에 늘릴 칸 수)다.
struct LaneLabelFaults {
    /// 레인 테두리를 덮은 채 남은 구간, 메시지 이름표. 늘릴 칸 수는 이름표 폭이다.
    labels_on_lane_sides: Vec<(usize, usize, usize)>,
    /// 가장 긴 가로 조각도 짧아 그리지 못하는 메시지 이름표. 늘릴 칸 수는 모자란 칸 수다.
    short_label_runs: Vec<(usize, usize, usize)>,
}

impl LaneLabelFaults {
    /// 두 흠 모두 늘지 않고 하나라도 줄었는지.
    fn is_fewer_than(&self, before: &LaneLabelFaults) -> bool {
        let (sides, short) = (self.labels_on_lane_sides.len(), self.short_label_runs.len());
        let (sides_before, short_before) = (before.labels_on_lane_sides.len(), before.short_label_runs.len());
        sides <= sides_before && short <= short_before && (sides, short) != (sides_before, short_before)
    }
}

/// 폭을 넘는 배치를 부른 쪽이 어떻게 쓰는지.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TooWideUse {
    /// 버린다. 폭을 넘는지만 맞으면 된다.
    Discarded,
    /// 층 접기가 접을 줄을 고르고 폭을 견주는 데 쓴다.
    Folded,
}

/// 메시지 이름표를 놓기 전까지 그린 것: 그린 메시지 경로와 놓은 구간 이름표.
struct DrawnBeforePoolLabels {
    pool_edge_routes: Vec<PoolEdgeRoute>,
    segment_labels: Vec<PlacedLabel>,
}

/// 메시지 이름표를 놓는 방식.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PoolLabelPlacement {
    /// 다른 선, 글자, 레인 테두리를 비키는 자리로 옮긴다.
    AvoidingLines,
    /// 가장 긴 가로 조각 가운데에 둔다.
    Centered,
}

/// 캔버스에 놓은 간선 이름표 하나(첫 칸, 줄, 글자)와, 메시지 이름표를 가장 긴 가로 조각 가운데에서 옮겼는지.
#[derive(Clone)]
struct PlacedLabel {
    x: usize,
    row: usize,
    text: String,
    is_moved: bool,
}

/// 풀 간선 이름표를 얹을 가로 조각(캔버스 열 `start`~`end`, 줄 `row`)과 그 가운데 자리.
struct PoolEdgeLabelRun {
    start: usize,
    end: usize,
    row: usize,
    centered: usize,
    covers_lane_side: bool,
}

/// 곧은 선 하나: `is_across`면 흐름축 `fixed`에서 교차축 `low..=high`를, 아니면 교차축 `fixed`에서 흐름축
/// `low..=high`를 쓴다.
struct StraightLine {
    is_across: bool,
    fixed: usize,
    low: usize,
    high: usize,
}

/// 흐름축, 교차축 범위(양 끝 포함)로 잡은 칸 사각형.
struct CellRect {
    along: (usize, usize),
    across: (usize, usize),
}

/// 메시지 다리가 지나면 안 되는 칸 사각형들을 흐름축 칸별(교차축 범위), 교차축 칸별(흐름축 범위)로 색인하고, 다리와
/// 같은 칸을 같은 방향으로 쓰면 안 되는 흐름 선 조각을 줄별로 모은 것. 다리 조각은 한 줄을 쓰므로 그 줄의 색인만 보면
/// 되어, 비켜 갈 자리를 넓게 훑어도 장애물 수에 비례해 느려지지 않는다.
#[derive(Default)]
struct LegObstacles {
    /// 색인하기 전에 모은 칸 사각형.
    cell_rects: Vec<CellRect>,
    across_spans_at_along: SpansByLine,
    along_spans_at_across: SpansByLine,
    /// 색인하기 전에 모은 구간 이름표 칸. 다리가 지나면 이름표가 끊기지만, 다른 장애물과 달리 이것만 덮는 다리를 위해
    /// 칸이나 줄을 끼우지는 않는다.
    label_rects: Vec<CellRect>,
    label_spans_at_along: SpansByLine,
    label_spans_at_across: SpansByLine,
    flow_spans_on_line: SpansOnLines,
    frame_spans_on_line: SpansOnLines,
}

impl LegObstacles {
    fn add(&mut self, rect: CellRect) {
        self.cell_rects.push(rect);
    }

    fn add_label(&mut self, rect: CellRect) {
        self.label_rects.push(rect);
    }

    fn has_labels(&self) -> bool {
        !self.label_rects.is_empty()
    }

    /// 모은 칸 사각형을 줄별로 색인한다. 장애물을 다 모은 뒤 한 번 부른다.
    fn index_cells(&mut self) {
        self.flow_spans_on_line.sort();
        self.frame_spans_on_line.sort();
        self.across_spans_at_along = SpansByLine::of(self.cell_rects.iter().map(|rect| (rect.along, rect.across)));
        self.along_spans_at_across = SpansByLine::of(self.cell_rects.iter().map(|rect| (rect.across, rect.along)));
        self.label_spans_at_along = SpansByLine::of(self.label_rects.iter().map(|rect| (rect.along, rect.across)));
        self.label_spans_at_across = SpansByLine::of(self.label_rects.iter().map(|rect| (rect.across, rect.along)));
    }

    /// 흐름 선의 곧은 조각(`is_across`면 흐름축 `fixed`에서 교차축 `low..=high`, 아니면 그 반대).
    fn add_flow_line(&mut self, is_across: bool, fixed: usize, (low, high): (usize, usize)) {
        self.flow_spans_on_line.entries.push(((is_across, fixed), (low.min(high), low.max(high))));
    }

    /// 풀, 레인, 그룹 테두리의 곧은 변. 다리가 이 변을 따라 달리거나 그 위에서 꺾으면 테두리가 끊긴다. 변을 곧게
    /// 넘는 것(`┼`)과 풀 면에 닿는 끝은 괜찮다 — 그래서 흐름 선과 달리 조각 끝이 변 위에 있어도 세지 않는다.
    fn add_frame_side(&mut self, is_across: bool, fixed: usize, span: (usize, usize)) {
        self.frame_spans_on_line.entries.push(((is_across, fixed), span));
    }

    /// 조각이 걸친 장애물 수: 지나는 상자와 끝 칸, 같은 칸을 같은 방향으로 쓰는 흐름 선 조각과 테두리 변, 꺾이거나
    /// 끝나는 칸에 놓인 흐름 선. 흐름 선을 곧게 가로지르는 칸만 건너뛰기 글자가 되고, 나머지는 이음 글자로 합쳐진다.
    fn hits(&self, piece: &StraightLine) -> usize {
        LABEL_HITS_PER_OTHER_HIT * self.hits_besides_labels(piece) + self.label_hits(piece)
    }

    /// 조각이 걸친 장애물 가운데 구간 이름표가 아닌 것의 수.
    fn hits_besides_labels(&self, piece: &StraightLine) -> usize {
        self.obstacles_besides_labels(piece).count()
    }

    /// 조각이 걸친 장애물 가운데 구간 이름표가 아닌 것들, 하나씩. 걸렸는지만 볼 때 첫 장애물에서 멈출 수 있다.
    fn obstacles_besides_labels<'o>(&'o self, piece: &'o StraightLine) -> impl Iterator<Item = ()> + 'o {
        let spans_at = if piece.is_across { &self.across_spans_at_along } else { &self.along_spans_at_across };
        let meets = |&&(low, high): &&(usize, usize)| low <= piece.high && piece.low <= high;
        let cells = spans_at.at(piece.fixed).iter().filter(meets).map(|_| ());
        let parallel = move |lines: &'o SpansOnLines| lines.on((piece.is_across, piece.fixed)).iter().map(|(_, span)| span).filter(meets).map(|_| ());
        let flows_at_ends = [piece.low, piece.high].into_iter().filter(|&end| self.has_flow_crossing(!piece.is_across, end, piece.fixed)).map(|_| ());
        cells.chain(parallel(&self.flow_spans_on_line)).chain(parallel(&self.frame_spans_on_line)).chain(flows_at_ends)
    }

    /// 경로의 다리들이 장애물에 걸리는 정도.
    fn clearance(&self, route: &PoolEdgeRoute) -> LegClearance {
        let mut clearance = LegClearance::Clear;
        for line in route.legs() {
            if self.obstacles_besides_labels(&line).next().is_some() {
                return LegClearance::Blocked;
            }
            if self.labels_crossed(&line).next().is_some() {
                clearance = LegClearance::CrossesLabels;
            }
        }
        clearance
    }

    /// 조각이 지나는 구간 이름표 수.
    fn label_hits(&self, piece: &StraightLine) -> usize {
        self.labels_crossed(piece).count()
    }

    /// 조각이 지나는 구간 이름표들.
    fn labels_crossed<'o>(&'o self, piece: &'o StraightLine) -> impl Iterator<Item = &'o (usize, usize)> + 'o {
        let spans_at = if piece.is_across { &self.label_spans_at_along } else { &self.label_spans_at_across };
        spans_at.at(piece.fixed).iter().filter(|&&(low, high)| low <= piece.high && piece.low <= high)
    }

    /// 줄 `(is_across, fixed)`의 흐름 선이 그 줄 위 `at` 칸을 지나는지.
    fn has_flow_crossing(&self, is_across: bool, fixed: usize, at: usize) -> bool {
        self.flow_spans_on_line.on((is_across, fixed)).iter().any(|&(_, (low, high))| (low..=high).contains(&at))
    }

    fn covers(&self, along: usize, across: usize) -> bool {
        self.across_spans_at_along.at(along).iter().any(|&(low, high)| (low..=high).contains(&across))
    }
}

/// 줄(방향, 고정 좌표)마다 그 줄 위의 범위들. 줄 순서로 정렬해 두고 줄마다 시작 자리를 적어 두어 한 줄의 범위를 바로
/// 찾는다 — 다리 비켜 놓기가 후보 다리마다 여러 줄을 찾아 이분 탐색이면 그것만으로 배치 시간의 큰 몫을 쓴다.
#[derive(Default)]
struct SpansOnLines {
    entries: Vec<SpanOnLine>,
    /// 방향별(가로지르지 않는 줄, 가로지르는 줄)로 고정 좌표 `k`의 범위는 `entries[starts[k]..starts[k + 1]]`.
    line_starts: [Vec<usize>; 2],
}

/// 줄(방향, 고정 좌표)과 그 줄 위의 범위.
type SpanOnLine = ((bool, usize), (usize, usize));

impl SpansOnLines {
    /// 줄 순서로 정렬하고 줄마다 시작 자리를 적는다. 범위를 다 모은 뒤 한 번 부른다.
    fn sort(&mut self) {
        self.entries.sort_unstable();
        let across_start = self.entries.partition_point(|((is_across, _), _)| !is_across);
        for (is_across, (first, last)) in [(false, (0, across_start)), (true, (across_start, self.entries.len()))] {
            let block = &self.entries[first..last];
            let line_count = block.last().map_or(0, |((_, fixed), _)| fixed + 1);
            let mut starts = vec![0; line_count + 1];
            for ((_, fixed), _) in block {
                starts[fixed + 1] += 1;
            }
            starts[0] = first;
            for k in 0..line_count {
                starts[k + 1] += starts[k];
            }
            self.line_starts[usize::from(is_across)] = starts;
        }
    }

    fn on(&self, (is_across, fixed): (bool, usize)) -> &[SpanOnLine] {
        let starts = &self.line_starts[usize::from(is_across)];
        if fixed + 1 < starts.len() { &self.entries[starts[fixed]..starts[fixed + 1]] } else { &[] }
    }
}

/// 줄 번호마다 그 줄에 걸친 범위들. 한 덩어리 배열에 줄 순서로 담아, 줄마다 따로 할당하지 않는다.
#[derive(Default)]
struct SpansByLine {
    /// 줄 `k`의 범위는 `spans[starts[k]..starts[k + 1]]`.
    starts: Vec<usize>,
    spans: Vec<(usize, usize)>,
}

impl SpansByLine {
    /// `(줄 범위, 그 줄들에 걸친 범위)`들로 만든다.
    fn of(rects: impl Iterator<Item = ((usize, usize), (usize, usize))> + Clone) -> SpansByLine {
        let line_count = rects.clone().map(|((_, last), _)| last + 1).max().unwrap_or(0);
        let mut starts = vec![0; line_count + 1];
        for ((first, last), _) in rects.clone() {
            for line in first..=last {
                starts[line + 1] += 1;
            }
        }
        for line in 0..line_count {
            starts[line + 1] += starts[line];
        }
        let mut filled = starts.clone();
        let mut spans = vec![(0, 0); starts[line_count]];
        for ((first, last), span) in rects {
            for line in first..=last {
                spans[filled[line]] = span;
                filled[line] += 1;
            }
        }
        SpansByLine { starts, spans }
    }

    fn at(&self, line: usize) -> &[(usize, usize)] {
        if line + 1 < self.starts.len() { &self.spans[self.starts[line]..self.starts[line + 1]] } else { &[] }
    }
}

impl RoutePiece {
    /// 조각이 놓인 줄(방향, 고정 좌표). 같은 줄의 조각끼리만 같은 칸을 같은 방향으로 쓸 수 있다.
    fn line(&self) -> (bool, usize) {
        (self.is_across, self.fixed)
    }

    /// 같은 줄의 다른 경로 조각과 칸을 함께 쓰는 충돌인지. 적어도 한쪽이 막힌 칸이어야 센다 — 둘 다 한 노드 쪽 끝에
    /// 모이는 다리라면 노드 쪽 끝 나누기가 맡는다.
    fn collides_with(&self, other: &RoutePiece) -> bool {
        let shares_cells = self.low <= other.high && other.low <= self.high;
        self.route != other.route && shares_cells && (self.is_blocking || other.is_blocking)
    }
}

/// 탐색이 정한 경로 하나의 결정: 기본 자리에서 옮긴 끝의 흐름축 칸 수, 비켜 가기 꺾임이 상대 끝에서 떨어진 칸 수,
/// 통로 깊이. 칸을 끼워 좌표가 밀려도 그대로 쓸 수 있게 기본 자리에 대한 차이로 남긴다.
#[derive(Clone, Copy)]
struct PoolRoutePlan {
    from_shift: isize,
    to_shift: isize,
    jog: Option<PlannedJog>,
    depth: usize,
}

/// 비켜 가기 결정: 꺾임이 상대 끝에서 흐름축으로 떨어진 칸 수와 되돌아오는 자리.
#[derive(Clone, Copy)]
struct PlannedJog {
    along_shift: isize,
    return_kind: JogReturn,
}

impl PoolRoutePlan {
    fn between(base: &PoolEdgeRoute, searched: &PoolEdgeRoute) -> PoolRoutePlan {
        let shift = |from: usize, to: usize| to as isize - from as isize;
        PoolRoutePlan {
            from_shift: shift(base.from.0, searched.from.0),
            to_shift: shift(base.to.0, searched.to.0),
            jog: searched.node_side_jog.map(|jog| PlannedJog { along_shift: shift(searched.other_end().0, jog.shifted_along), return_kind: jog.return_kind }),
            depth: searched.depth,
        }
    }

    /// 끝 옮김과 통로 깊이를 기본 자리 `route`에 입힌다. 비켜 가기 교차축 칸은 배치에 달려 있어 부르는 쪽이 정한다.
    fn apply_to(&self, route: &mut PoolEdgeRoute) {
        route.from.0 = route.from.0.saturating_add_signed(self.from_shift);
        route.to.0 = route.to.0.saturating_add_signed(self.to_shift);
        route.depth = self.depth;
    }
}

/// 다리 비켜 놓기 탐색의 상태: 경로, 같은 면 묶음, 경로별 조각과 줄별 조각 색인, 지금 충돌, 경로별 장애물 수.
struct LegSearch {
    routes: Vec<PoolEdgeRoute>,
    face_groups: Vec<Vec<usize>>,
    face_group_of: Vec<usize>,
    pieces_of: Vec<Vec<RoutePiece>>,
    pieces_on_line: BTreeMap<(bool, usize), Vec<RoutePiece>>,
    collisions: Vec<LegCollision>,
    /// 충돌 수. 비켜 놓기 단계에서는 목록(`collisions`)을 다시 만들지 않고 이 수만 맞춘다.
    collision_count: usize,
    obstacle_hits_of: Vec<usize>,
    total_obstacle_hits: usize,
}

impl LegSearch {
    /// 작을수록 낫다: 메시지 선끼리의 충돌 수가 먼저, 장애물에 걸린 수가 다음.
    fn rank(&self) -> (usize, usize) {
        (self.collision_count, self.total_obstacle_hits)
    }

    /// 경로 `route`의 조각을 `pieces`로 바꾸고 줄 색인에서도 바꾼다.
    fn replace_pieces(&mut self, route: usize, pieces: Vec<RoutePiece>) {
        for old in &self.pieces_of[route] {
            if let Some(line) = self.pieces_on_line.get_mut(&old.line()) {
                line.retain(|piece| piece.route != route);
            }
        }
        for piece in &pieces {
            self.pieces_on_line.entry(piece.line()).or_default().push(*piece);
        }
        self.pieces_of[route] = pieces;
    }

    fn count_obstacle_hits(&mut self, hits_of: impl Fn(&PoolEdgeRoute) -> usize) {
        self.obstacle_hits_of = self.routes.iter().map(hits_of).collect();
        self.total_obstacle_hits = self.obstacle_hits_of.iter().sum();
    }

    /// 경로별 조각에서 줄별 색인과 충돌 목록을 다시 만든다. 줄 순서, 경로 순서로 훑어 충돌 순서가 늘 같다.
    fn reindex_pieces(&mut self) {
        self.pieces_on_line.clear();
        for piece in self.pieces_of.iter().flatten() {
            self.pieces_on_line.entry(piece.line()).or_default().push(*piece);
        }
        self.collisions.clear();
        for pieces in self.pieces_on_line.values() {
            for (k, first) in pieces.iter().enumerate() {
                for second in &pieces[k + 1..] {
                    if first.collides_with(second) {
                        self.collisions.push(LegCollision { first_route: first.route, second_route: second.route, is_first_blocking: first.is_blocking, is_second_blocking: second.is_blocking });
                    }
                }
            }
        }
        self.collision_count = self.collisions.len();
    }

    /// `pieces`(경로 `affected`의 조각)가 일으키는 충돌 수: 색인의 다른 경로 조각과, `pieces`끼리.
    fn collisions_involving(&self, pieces: &[RoutePiece], affected: &BTreeSet<usize>) -> usize {
        let with_others: usize = pieces.iter().map(|piece| self.pieces_on_line.get(&piece.line()).map_or(0, |line| line.iter().filter(|other| !affected.contains(&other.route) && piece.collides_with(other)).count())).sum();
        let mut affected_on_line: BTreeMap<(bool, usize), Vec<&RoutePiece>> = BTreeMap::new();
        for piece in pieces {
            affected_on_line.entry(piece.line()).or_default().push(piece);
        }
        let among_themselves: usize = affected_on_line.values().map(|line| line.iter().enumerate().map(|(k, first)| line[k + 1..].iter().filter(|second| first.collides_with(second)).count()).sum::<usize>()).sum();
        with_others + among_themselves
    }
}

/// 후보 옮김 하나의 결과: 바뀐 경로들(번호, 새 경로)과 그 장애물 수, 옮긴 뒤의 순위.
struct LegMoveTrial {
    changed: Vec<(usize, PoolEdgeRoute)>,
    obstacle_hits: Vec<usize>,
    rank: (usize, usize),
}

/// 한 면 묶음 `members`의 통로 깊이. `route_of`로 경로를 바꿔 끼워 볼 수 있다.
fn face_group_depths<'r>(members: &[usize], route_of: impl Fn(usize) -> &'r PoolEdgeRoute) -> Vec<usize> {
    let spans: Vec<ChannelSpan> = members.iter().map(|&r| ChannelSpan { bend_along: route_of(r).other_bend_along(), face_along: route_of(r).pool_end().0, is_detoured: route_of(r).is_detoured }).collect();
    channel_depths(&spans)
}

/// 두 경로의 조각이 같은 칸을 같은 방향으로 쓰는 충돌.
#[derive(Clone, Copy)]
struct LegCollision {
    first_route: usize,
    second_route: usize,
    is_first_blocking: bool,
    is_second_blocking: bool,
}

impl LegCollision {
    /// 비켜 놓을 경로 순서: 한쪽만 막힌 칸이면 그 칸에 꺾임이나 끝이 걸린 다른 쪽부터, 둘 다면 나중 경로부터.
    fn movers_in_preference_order(&self) -> [usize; 2] {
        let later_first = [self.first_route.max(self.second_route), self.first_route.min(self.second_route)];
        match (self.is_first_blocking, self.is_second_blocking) {
            (true, false) => [self.second_route, self.first_route],
            (false, true) => [self.first_route, self.second_route],
            _ => later_first,
        }
    }
}

/// `across`에서 교차축으로 한 칸 옮긴 자리: `is_higher`면 큰 쪽, 아니면 작은 쪽.
fn step_across(across: usize, is_higher: bool) -> Option<usize> {
    if is_higher { Some(across + 1) } else { across.checked_sub(1) }
}

/// `from`에서 한 칸 옮긴 두 자리를 `toward` 쪽부터. 같으면 큰 쪽부터.
fn steps_toward_first(from: usize, toward: usize) -> Vec<usize> {
    let (near, far) = if toward < from { (from.checked_sub(1), Some(from + 1)) } else { (Some(from + 1), from.checked_sub(1)) };
    near.into_iter().chain(far).collect()
}

/// 경유점에서 겹친 점과 곧게 이어지는 가운데 점을 뺀, 실제로 꺾이는 점들(양 끝 포함).
fn corner_points(waypoints: &[(usize, usize)]) -> RoutePoints {
    let mut corners = RoutePoints::default();
    for &point in waypoints {
        if corners.last() == Some(&point) {
            continue;
        }
        if let [.., before, last] = *corners {
            let is_straight_through = (before.0 == last.0 && last.0 == point.0) || (before.1 == last.1 && last.1 == point.1);
            if is_straight_through {
                corners.pop();
            }
        }
        corners.push(point);
    }
    corners
}

impl PoolEdgeRoute {
    fn pool_end(&self) -> (usize, usize, bool) {
        if self.is_pool_at_from { self.from } else { self.to }
    }

    fn pool_anchor(&self, graph: &Graph) -> usize {
        let edge = &graph.edges[self.edge];
        if self.is_pool_at_from { edge.from } else { edge.to }
    }

    fn other_anchor(&self, graph: &Graph) -> usize {
        let edge = &graph.edges[self.edge];
        if self.is_pool_at_from { edge.to } else { edge.from }
    }

    fn end_mut(&mut self, is_other_end: bool) -> &mut (usize, usize, bool) {
        if is_other_end == self.is_pool_at_from { &mut self.to } else { &mut self.from }
    }

    /// 상대 끝 쪽 다리가 통로에서 꺾여 나가는 흐름축 위치.
    fn other_bend_along(&self) -> usize {
        self.node_side_jog.map_or(self.other_end().0, |jog| jog.shifted_along)
    }

    /// 두 꺾임이 흐름축으로 어긋나 통로 줄을 따라 달리는지. 어긋나지 않으면 교차축으로 곧게 그어 줄을 쓰지 않는다.
    fn runs_along_channel(&self) -> bool {
        self.pool_end().0 != self.other_bend_along()
    }

    /// 통로 줄을 세는 면이 사이 통로의 낮은 쪽(교차축 작은 쪽)인지.
    fn counts_from_low_face(&self) -> bool {
        self.pool_end().2
    }

    /// 통로를 따라 달리는 흐름축 구간(양 끝 포함).
    fn along_span(&self) -> (usize, usize) {
        let (pool_along, bend_along) = (self.pool_end().0, self.other_bend_along());
        (pool_along.min(bend_along), pool_along.max(bend_along))
    }

    /// 출발 끝에서 도착 끝까지의 경유점(흐름축, 교차축): 풀 쪽 끝, 통로 줄의 두 꺾임, 비켜 가기가 있으면 그 두
    /// 꺾임, 상대 끝. 통로를 따라갈 구간이 없으면 두 통로 꺾임이 한 점에 겹친다.
    fn waypoints(&self) -> RoutePoints {
        let channel = self.channel();
        let ((pool_along, pool_across, _), (other_along, other_across, _)) = (self.pool_end(), self.other_end());
        let bend_along = self.other_bend_along();
        let mut points = RoutePoints::default();
        for point in [(pool_along, pool_across), (pool_along, channel), (bend_along, channel)] {
            points.push(point);
        }
        if let Some(jog) = self.node_side_jog {
            points.push((jog.shifted_along, jog.return_across));
            points.push((other_along, jog.return_across));
        }
        points.push((other_along, other_across));
        if !self.is_pool_at_from {
            points.reverse();
        }
        points
    }

    /// 경유점에서 실제로 꺾이는 점 사이의 곧은 선들.
    fn straight_lines(&self) -> impl Iterator<Item = StraightLine> {
        let corners = corner_points(&self.waypoints());
        (1..corners.len()).map(move |k| (corners[k - 1], corners[k])).map(|piece| {
            let ((along0, across0), (along1, across1)) = piece;
            if along0 == along1 {
                StraightLine { is_across: true, fixed: along0, low: across0.min(across1), high: across0.max(across1) }
            } else {
                StraightLine { is_across: false, fixed: across0, low: along0.min(along1), high: along0.max(along1) }
            }
        })
    }

    /// 통로 줄을 뺀 곧은 선들(다리와 비켜 가기). 통로 줄은 풀 사이 간격에만 놓이고, 칸을 끼우기 전에는 깊은 줄이
    /// 풀 테두리에 겹쳐 보이지만 끼운 뒤에는 그렇지 않으므로 장애물로 세지 않는다.
    fn legs(&self) -> impl Iterator<Item = StraightLine> {
        let channel = self.channel();
        self.straight_lines().filter(move |line| line.is_across || line.fixed != channel)
    }

    fn other_end(&self) -> (usize, usize, bool) {
        if self.is_pool_at_from { self.to } else { self.from }
    }

    fn channel(&self) -> usize {
        let (_, face, toward_higher) = self.pool_end();
        if toward_higher { face + self.depth } else { face.saturating_sub(self.depth) }
    }
}

/// 비켜 갈 자리가 없는 다리를 위해 마련하는 자리: 끝 노드 앞 칸, 또는 층 사이 간격(층 번호)의 빈 줄.
#[derive(Clone)]
enum LegRoom {
    FrontOfEndNode(FrontRoom),
    GapRow(usize),
}

/// 끝 노드 앞 칸: 교차축 `at`에 칸을 끼우고, 곧은 흐름이 꺾이지 않게 `at`보다 앞의 `also_shifted` 노드도 한 칸 민다.
#[derive(Clone)]
struct FrontRoom {
    at: usize,
    also_shifted: Vec<usize>,
}

/// 다리 비켜 놓기 후보가 장애물에 걸리는 정도.
enum LegClearance {
    /// 노드 상자, 흐름 선, 테두리에 걸린다.
    Blocked,
    /// 구간 이름표만 지난다.
    CrossesLabels,
    /// 아무 장애물에도 걸리지 않는다.
    Clear,
}

/// 비켜 갈 후보 자리들(흐름축, 그 자리의 후보): 장애물에 걸리지 않는 것과 구간 이름표만 지나는 것까지 받은 것.
#[derive(Default)]
struct DetourPlaces {
    clear: Vec<(usize, Vec<PoolEdgeRoute>)>,
    crossing_labels: Vec<(usize, Vec<PoolEdgeRoute>)>,
}

impl DetourPlaces {
    /// 흐름축 `along`의 후보를 아직 `DETOURS_PER_SIDE`곳이 차지 않은 쪽에 더한다.
    fn add(&mut self, along: usize, clear: Vec<PoolEdgeRoute>, crossing_labels: Vec<PoolEdgeRoute>) {
        for (places, moves) in [(&mut self.clear, clear), (&mut self.crossing_labels, crossing_labels)] {
            if !moves.is_empty() && places.len() < DETOURS_PER_SIDE {
                places.push((along, moves));
            }
        }
    }

    fn extend(&mut self, other: DetourPlaces) {
        self.clear.extend(other.clear);
        self.crossing_labels.extend(other.crossing_labels);
    }
}

/// 비켜 갈 후보들, 가까운 자리부터: 장애물에 걸리지 않는 것과 구간 이름표만 지나는 것까지 받은 것.
struct DetourCandidates {
    clear: Vec<PoolEdgeRoute>,
    crossing_labels: Vec<PoolEdgeRoute>,
}

/// 자리마다 모은 후보를 꺾임 `bend`에 가까운 자리부터 잇는다. 같은 순위면 먼저 본 후보를 두므로 가까운 자리부터
/// 둔다(같은 거리면 먼저 훑은 앞쪽).
fn nearest_places_first(mut places: Vec<(usize, Vec<PoolEdgeRoute>)>, bend: usize) -> Vec<PoolEdgeRoute> {
    places.sort_by_key(|&(along, _)| along.abs_diff(bend));
    places.into_iter().flat_map(|(_, moves)| moves).collect()
}

/// 다리를 비켜 놓을 자리를 훑을 때 함께 보는 것: 지금 경로들, 그 끝 칸, 장애물.
struct DetourScan<'s> {
    routes: &'s [PoolEdgeRoute],
    end_cells: &'s EndCells,
    obstacles: &'s LegObstacles,
}

/// 경로들의 끝 칸(흐름축, 교차축)마다 그 칸에 놓인 끝 수. 끝 하나를 옮겨 볼 때마다 모든 경로를 훑지 않게 한 번 센다.
struct EndCells {
    counts: BTreeMap<(usize, usize), usize>,
}

impl EndCells {
    fn of(routes: &[PoolEdgeRoute]) -> EndCells {
        let mut counts = BTreeMap::new();
        for end in routes.iter().flat_map(|route| [route.from, route.to]) {
            *counts.entry((end.0, end.1)).or_default() += 1;
        }
        EndCells { counts }
    }

    /// 경로 `own`을 `moved`로 바꿔도 그 끝들이 다른 경로의 끝 칸에 놓이지 않는지.
    fn keeps_ends_free(&self, own: &PoolEdgeRoute, moved: &PoolEdgeRoute) -> bool {
        [moved.from, moved.to].iter().all(|end| self.is_free_for(own, (end.0, end.1)))
    }

    /// `cell`에 경로 `own`의 끝 말고 다른 경로의 끝이 없는지.
    fn is_free_for(&self, own: &PoolEdgeRoute, cell: (usize, usize)) -> bool {
        let own_ends = [own.from, own.to].iter().filter(|end| (end.0, end.1) == cell).count();
        self.counts.get(&cell).copied().unwrap_or(0) <= own_ends
    }
}

/// 형제 블록 사이 간격 하나: 낮은 쪽 블록의 면과 높은 쪽 블록의 면(교차축). 그쪽에 형제가 없으면 `None`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ChannelGap {
    low_face: Option<usize>,
    high_face: Option<usize>,
}

/// 사이 통로의 두 면에서 각각 센 통로 줄이 같은 칸에 놓여 선이 합쳐질 수 있는지: 두 면 쪽 경로 가운데 통로를
/// 따라 달리는 구간이 겹치는 짝이 있으면 그렇다. 겹치지 않으면 같은 줄을 나눠 써도 선이 닿지 않는다.
fn faces_share_channel_rows(routes: &[PoolEdgeRoute], members: &[usize]) -> bool {
    let row_users: Vec<&PoolEdgeRoute> = members.iter().map(|&r| &routes[r]).filter(|route| route.runs_along_channel()).collect();
    let (low_face_users, high_face_users): (Vec<&PoolEdgeRoute>, Vec<&PoolEdgeRoute>) = row_users.into_iter().partition(|route| route.counts_from_low_face());
    low_face_users.iter().any(|low| high_face_users.iter().any(|high| low.along_span().0 <= high.along_span().1 && high.along_span().0 <= low.along_span().1))
}

/// 마주 보는 사이 통로의 두 면에서 센 줄 수 합이 간격보다 크면 높은 쪽 면 앞에 끼울 (위치, 칸 수).
fn facing_gap_insertion(gap: ChannelGap, routes: &[PoolEdgeRoute], members: &[usize]) -> Option<(usize, usize)> {
    let (low_face, high_face) = (gap.low_face?, gap.high_face?);
    let deepest_from = |is_low_face: bool| members.iter().map(|&r| &routes[r]).filter(|route| route.runs_along_channel() && route.counts_from_low_face() == is_low_face).map(|route| route.depth).max().unwrap_or(0);
    let rows_needed = deepest_from(true) + deepest_from(false);
    let rows_available = high_face.saturating_sub(low_face + 1);
    (rows_needed > rows_available).then(|| (high_face, rows_needed - rows_available))
}

/// `wanted` 칸 끼우기 가운데 `granted`(같은 배치에서 이미 끼운 몫으로 센 것)를 넘는 만큼만, 자리별로.
fn room_beyond(wanted: &[(usize, usize)], granted: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let total_at = |insertions: &[(usize, usize)]| {
        let mut totals: BTreeMap<usize, usize> = BTreeMap::new();
        for &(at, count) in insertions {
            *totals.entry(at).or_default() += count;
        }
        totals
    };
    let granted = total_at(granted);
    total_at(wanted).into_iter().filter_map(|(at, count)| count.checked_sub(granted.get(&at).copied().unwrap_or(0)).filter(|&extra| extra > 0).map(|extra| (at, extra))).collect()
}

/// 한 쪽에 끝 `count`개가 저마다 한 칸씩 들어갈 노드 흐름축 크기. 상자는 모서리 두 칸에는 끝을 두지 않고, 점 표기는
/// 글자 칸부터 끝을 둔다. 끝 사이 빈 칸까지 잡으면 메시지가 많은 문서에서 노드가 두 배로 커져 폭 안에 들지 않는다.
fn along_size_for_spread_ends(is_point_anchored: bool, count: usize) -> usize {
    if is_point_anchored { count } else { count + 2 }
}

/// 가운데(면 중앙이나 노드 쪽 기본 칸)를 둘러 `count`칸을 사이에 빈 칸 하나씩 두고(범위가 짧으면 붙여서) `interior` 안에 놓는다.
fn spread_face_points(center: usize, count: usize, (low, high): (usize, usize)) -> Vec<usize> {
    let room = high.saturating_sub(low) + 1;
    let spacing = if 2 * (count - 1) < room { 2 } else { 1 };
    let span = spacing * (count - 1);
    let first = center.saturating_sub(span / 2).min(high.saturating_sub(span)).max(low);
    (0..count).map(|k| first + k * spacing).collect()
}

/// 한 면 바깥 통로를 달리는 구간: 상대 끝 쪽 꺾임과 면 칸의 흐름축, 장애물을 피해 옮긴 경로인지.
#[derive(Clone, Copy)]
struct ChannelSpan {
    bend_along: usize,
    face_along: usize,
    is_detoured: bool,
}

impl ChannelSpan {
    fn range(&self) -> (usize, usize) {
        (self.bend_along.min(self.face_along), self.bend_along.max(self.face_along))
    }
}

/// 한 면 바깥 통로를 함께 쓰는 간선들의 통로 깊이.
/// 통로 구간이 겹치는 둘 가운데 상대 끝이 면 칸에서 더 먼 쪽(면 칸 앞쪽이면 더 앞, 뒤쪽이면 더
/// 뒤)이 면에 가까운 통로를 쓰면, 그 상대 끝 다리는 더 깊은 통로에 닿기 전에 꺾이고 다른 쪽의
/// 면 다리도 얕은 통로까지만 가므로 서로 가로지르지 않는다. 면 칸 앞쪽 구간과 뒤쪽 구간은 면 칸이 상대 끝 순서로
/// 놓이면 겹치지 않지만, 장애물을 피해 옮긴 다리의 구간은 겹칠 수 있어 그때는 뒤쪽 구간이 더 깊은 통로를 쓴다.
fn channel_depths(spans: &[ChannelSpan]) -> Vec<usize> {
    let ranges: Vec<(usize, usize)> = spans.iter().map(ChannelSpan::range).collect();
    let overlaps = |i: usize, j: usize| ranges[j].0 <= ranges[i].1 && ranges[i].0 <= ranges[j].1;
    let mut depths = vec![1; spans.len()];
    let mut before_face_point: Vec<usize> = Vec::new();
    for is_before_face_point in [true, false] {
        let mut order: Vec<usize> = (0..spans.len()).filter(|&i| if is_before_face_point { spans[i].bend_along < spans[i].face_along } else { spans[i].bend_along > spans[i].face_along }).collect();
        order.sort_unstable_by_key(|&i| (spans[i].bend_along, i));
        if !is_before_face_point {
            order.reverse();
        }
        for (k, &i) in order.iter().enumerate() {
            let mut overlapped = order[..k].iter().filter(|&&j| overlaps(i, j)).map(|&j| depths[j]).max();
            if !is_before_face_point {
                let detoured_other_side = before_face_point.iter().filter(|&&j| (spans[i].is_detoured || spans[j].is_detoured) && overlaps(i, j)).map(|&j| depths[j]).max();
                overlapped = overlapped.max(detoured_other_side);
            }
            depths[i] = overlapped.map_or(1, |depth| depth + 1);
        }
        if is_before_face_point {
            before_face_point = order;
        }
    }
    depths
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

/// 결정적 의사난수(xorshift64) — 외부 crate 없이 seed로 재현 가능한 테스트 입력을 만든다
/// (bugfix.md 재현 4의 파이썬 생성기와 같은 모양). 배치기 테스트와 BPMN 속성 테스트가 함께 쓴다.
#[cfg(test)]
pub(crate) struct Xorshift64(u64);

#[cfg(test)]
impl Xorshift64 {
    pub(crate) fn new(seed: u64) -> Self {
        // 0이면 xorshift가 0에 갇히므로 홀수로 섞는다.
        Xorshift64((seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1)
    }
    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// 안정적인 해시(FNV-1a, 외부 crate 없이) — 큰 렌더링 결과를 바이트 그대로 담는 대신 쓴다.
/// 배치기 테스트와 BPMN 속성 테스트가 함께 쓴다.
#[cfg(test)]
pub(crate) fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
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

    #[test]
    fn senders_are_pulled_forward_while_unconstrained_items_keep_their_current_order() {
        let (a, b, c) = (4, 1, 7);
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[]), vec![a, b, c], "제약이 없으면 지금 순서 그대로");
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[(c, a)]), vec![c, a, b], "보내는 항목만 받는 항목 바로 앞으로 끌어온다");
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[(a, c)]), vec![a, b, c], "이미 만족한 제약은 순서를 바꾸지 않는다");
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[(c, b), (b, a)]), vec![c, b, a], "보내는 항목의 보내는 항목도 재귀로 끌어온다");
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[(a, b), (b, a)]), vec![a, b, c], "두 항목 순환은 지금 순서를 따른다");
        assert_eq!(order_with_senders_pulled_forward(&[a, b, c], &[(a, b), (b, c), (c, a)]), vec![a, b, c], "세 항목 순환도 지금 순서를 따른다");
        assert_eq!(order_with_senders_pulled_forward(&[a, b], &[(9, a)]), vec![a, b], "목록에 없는 항목의 제약은 무시한다");
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
    /// 배너 다음 구분선이 한 줄만 있으며, 노드는 구분선·아랫변에 겹치지 않고 바로 안쪽에 놓인다.
    #[test]
    fn three_level_lane_nesting_has_no_doubled_divider() {
        let mut g = Graph::default();
        let pool = g.add_lane("Pool", None);
        let lane = g.add_lane("Lane", Some(pool));
        let sub = g.add_lane("SubLane", Some(lane));
        g.intern("s1", "s1", Shape::Rect, Some(sub));
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let lane_bottom_lines = out.iter().filter(|l| l.starts_with('└')).count();
        assert_eq!(lane_bottom_lines, 1, "3단 중첩의 아랫변이 모두 같은 줄에 있어야 한다: {text}");
        let sub_title_row = out.iter().position(|l| l.contains("SubLane")).unwrap_or_else(|| panic!("SubLane not found: {text}"));
        assert!(out[sub_title_row + 1].starts_with('├'), "SubLane 제목 줄 바로 다음이 구분선이어야 한다: {text}");
        assert!(out[sub_title_row + 2].contains('┌'), "구분선 바로 다음 줄이 s1 상자 윗변이어야 한다(이중 구분선 없음): {text}");
        let box_bottom_row = out.iter().position(|l| l.contains("└────┘")).unwrap_or_else(|| panic!("s1 bottom not found: {text}"));
        assert!(out[box_bottom_row + 1].starts_with('└'), "s1 상자 아랫변 바로 다음 줄이 레인 아랫변이어야 한다: {text}");
    }

    // ── 그룹 테두리 선 종류(bizprocess-bpmn 1.2) ─────────────────────────

    /// 레인 > 파선 Box 그룹 > 실선 Box 그룹 > 노드의 2단 중첩. 파선 그룹은 대시 글자로, 그
    /// 안의 실선 그룹은 이전과 같은 실선 글자로 그려지고, 노드는 레인 띠 안쪽에 있다.
    #[test]
    fn dashed_box_group_renders_with_dash_glyphs_nested_inside_a_solid_box_inside_a_lane() {
        for direction in [Direction::TopDown, Direction::LeftRight] {
            let mut g = Graph { direction: Some(direction), ..Graph::default() };
            let lane = g.add_lane("Lane", None);
            let dashed = g.add_group_with_id("dashed", "Dashed", Some(lane));
            g.set_group_line(dashed, LineKind::Dashed);
            let solid = g.add_group_with_id("solid", "Solid", Some(dashed));
            let n = g.intern("n", "n", Shape::Rect, Some(solid));
            let m = g.intern("m", "m", Shape::Rect, Some(lane));
            g.add_edge(Edge { from: n, to: m, head: Marker::Arrow, ..Edge::default() });
            let out = rows(render(&g, &Theme::none(), 100).unwrap());
            let text = out.join("\n");
            assert!(text.contains('╌') || text.contains('╎'), "파선 그룹 테두리에 대시 글자가 있어야 한다({direction:?}): {text}");
            assert!(text.contains('┌') && text.contains('└'), "실선 그룹 테두리는 이전과 같은 실선 글자여야 한다({direction:?}): {text}");
            let lane_left = char_col(&out[0], '┌');
            let lane_right = char_rcol(&out[0], '┐');
            let n_row = out.iter().position(|l| l.contains('n')).unwrap_or_else(|| panic!("n not found({direction:?}): {text}"));
            let n_col = out[n_row].find('n').unwrap();
            assert!(n_col > lane_left && n_col < lane_right, "노드가 레인 띠 안쪽 칸에 있어야 한다({direction:?}): {text}");
        }
    }

    /// 레인 > 파선 Box 그룹 > 노드(1단 중첩)도 대시 글자로 그려지고 기존 실선 그룹·레인 전용
    /// 테스트는 전부 무수정으로 통과한다(회귀 없음은 이 파일의 나머지 테스트가 증명한다).
    #[test]
    fn dashed_box_group_directly_inside_a_lane_renders_with_dash_glyphs() {
        let mut g = Graph::default();
        let lane = g.add_lane("Lane", None);
        let dashed = g.add_group_with_id("dashed", "Dashed", Some(lane));
        g.set_group_line(dashed, LineKind::Dashed);
        g.intern("n", "n", Shape::Rect, Some(dashed));
        let out = rows(render(&g, &Theme::none(), 100).unwrap());
        let text = out.join("\n");
        assert!(text.contains('╌') || text.contains('╎'), "파선 그룹 테두리에 대시 글자가 있어야 한다: {text}");
    }

    /// 기본 Box 그룹은 `line` 필드를 손대지 않으면 실선 그대로다(바이트 동일 보장).
    #[test]
    fn plain_box_group_defaults_to_solid_line() {
        let mut g = Graph::default();
        let group = g.add_group("Backend", None);
        assert_eq!(g.groups[group].line, LineKind::Solid);
    }

    /// bizprocess-bpmn 회귀 — `place_block`의 "닻은 제목 글자 폭만큼 오른쪽에 둔다" 규칙은
    /// 위→아래 배치에서만 뜻이 있다(제목이 상자 위 테두리를 가로로 차지하므로). 왼쪽→오른쪽
    /// 배치에서 그 규칙을 그대로 적용하면 제목 길이만큼 닻(그리고 층을 공유하는 형제 내용)이
    /// 세로로 밀려 상자가 내용과 무관하게 부풀어 오른다 — 이 latent 결함은 `layout::graph`
    /// 자체(그룹 id를 간선 끝으로 쓰는 모든 곳, 예: mermaid `subgraph` id를 향한 간선)에 있었고
    /// bizprocess의 L2/L4 Sub-Process 상자가 처음으로 크게 드러냈을 뿐이다.
    #[test]
    fn a_titled_box_groups_height_does_not_grow_with_its_title_length_in_left_to_right_layout() {
        let mut g = Graph { direction: Some(Direction::LeftRight), ..Graph::default() };
        let group = g.add_group_with_id("box", "아주 길고 긴 상자 제목입니다 정말로 깁니다", None);
        g.intern("inner", "inner", Shape::Rect, Some(group));
        let start = g.intern("start", "start", Shape::Rect, None);
        let end = g.intern("end", "end", Shape::Rect, None);
        let anchor_in = g.group_anchor(group);
        g.add_edge(Edge { from: start, to: anchor_in, head: Marker::Arrow, ..Edge::default() });
        let anchor_out = g.group_anchor(group);
        g.add_edge(Edge { from: anchor_out, to: end, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 200).unwrap());
        assert!(out.len() <= 12, "제목이 길어도 상자 높이는 내용(노드 하나)에 비례해야 한다({}줄):\n{}", out.len(), out.join("\n"));
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

    /// 요구사항 2.1, 2.6(bpmn-event-notation-anchor로 강화): TB에서 가는 선 노드(`Round`)는
    /// 기존 접점 규칙(테두리 바로 위 줄)을 그대로 따르고, 이벤트 노드(상자 없음)는 원 글자와
    /// 정확히 같은 열, 바로 위 줄에 화살촉이 붙는다 — "이벤트가 차지한 열 범위 안"이 아니라
    /// "원 글자와 같은 열"로 강화(수정 전엔 이름이 있으면 덩어리 가운데 열에 떨어져 어긋난다).
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
        // 종료 이벤트는 테두리가 없다 — `◉` 글자와 정확히 같은 열, 바로 위 줄에 화살촉이 온다.
        let end_row = row_of(&out, "◉");
        let glyph_col = char_col(&out[end_row], '◉');
        assert_eq!(out[end_row - 1].chars().nth(glyph_col), Some('▼'), "화살촉이 원 글자(◉)와 정확히 같은 열에 있어야 한다: {text}");
    }

    /// 요구사항 2.1(bpmn-event-notation-anchor로 강화): LR에서 이벤트 노드는 상자 테두리가
    /// 아니라 위치 글자 바로 왼쪽 칸에 화살촉이 붙는다(글자는 `◉`).
    #[test]
    fn event_shapes_attach_edges_using_existing_contact_rules_left_right() {
        let mut g = Graph::default();
        let start = g.intern("start", "Go", Shape::Event(EventPosition::Start), None);
        let end = g.intern("end", "Done", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: start, to: end, head: Marker::Arrow, ..Edge::default() });
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let end_row = row_of(&out, "◉");
        let end_col = char_col(&out[end_row], '◉');
        assert_eq!(out[end_row].chars().nth(end_col - 1), Some('▶'), "화살촉이 종료 이벤트의 위치 글자(◉) 바로 왼쪽에 있어야 한다: {text}");
    }

    /// bugfix bpmn-event-notation-anchor 재현 1.1·1.5(수정 전 실패, 4열 vs 1열): 위→아래,
    /// 이름 있는 종료 이벤트로 들어오는 간선은 이름 길이와 무관하게 원 글자(◉) 바로 위,
    /// 정확히 같은 열에 닿아야 한다.
    #[test]
    fn end_event_incoming_edge_lands_directly_above_the_glyph_regardless_of_name_length() {
        for name in ["", "Done", "주문 취소"] {
            let mut g = Graph::default();
            let task = g.intern("task", "Task", Shape::Round, None);
            let end = g.intern("end", "end", Shape::Event(EventPosition::End), None);
            g.set_label(end, name);
            g.add_edge(Edge { from: task, to: end, head: Marker::Arrow, ..Edge::default() });
            let out = rows(render(&g, &Theme::none(), 80).unwrap());
            let text = out.join("\n");
            let end_row = row_of(&out, "◉");
            let glyph_col = char_col(&out[end_row], '◉');
            assert_eq!(out[end_row - 1].chars().nth(glyph_col), Some('▼'), "이름 {name:?}: 화살촉이 원 글자(◉)와 정확히 같은 열에 있어야 한다: {text}");
        }
    }

    /// bugfix bpmn-event-notation-anchor 재현 1.2(수정 전 실패): 위→아래, 이름 있는 시작
    /// 이벤트에서 나가는 간선은 이름 길이와 무관하게 원 글자(○) 바로 아래, 같은 열에서
    /// 나가야 한다(수정 전엔 이름 위 열에서 나간다).
    #[test]
    fn start_event_outgoing_edge_leaves_directly_below_the_glyph_regardless_of_name_length() {
        for name in ["", "Go", "주문 접수 «message»"] {
            let mut g = Graph::default();
            let start = g.intern("start", "start", Shape::Event(EventPosition::Start), None);
            g.set_label(start, name);
            let task = g.intern("task", "Task", Shape::Round, None);
            g.add_edge(Edge { from: start, to: task, head: Marker::Arrow, ..Edge::default() });
            let out = rows(render(&g, &Theme::none(), 80).unwrap());
            let text = out.join("\n");
            let start_row = row_of(&out, "○");
            let glyph_col = char_col(&out[start_row], '○');
            // 바로 아래 줄은 화살촉이 아니라 선의 첫 칸(│)이지만, 그 열이 원 글자와 같아야
            // 한다 — 화살촉 자체는 도착 노드 바로 위에서만 그려진다.
            let exit_col = out[start_row + 1].chars().position(|c| c != ' ');
            assert_eq!(exit_col, Some(glyph_col), "이름 {name:?}: 선이 원 글자(○)와 정확히 같은 열에서 나가야 한다: {text}");
        }
    }

    /// bugfix bpmn-event-notation-anchor 재현 1.3(수정 전 실패): 위→아래, 종료 이벤트로
    /// 들어오는 간선이 둘이면 화살촉 둘 다 원 글자 바로 위, 같은 열로 모여야 한다(수정
    /// 전엔 이름 위에 벌려 놓여 어느 것도 원 글자에 닿지 않는다).
    #[test]
    fn end_event_two_incoming_edges_converge_directly_above_the_glyph() {
        let mut g = Graph::default();
        let a = g.intern("a", "A", Shape::Round, None);
        let b = g.intern("b", "B", Shape::Round, None);
        let end = g.intern("end", "Done", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: a, to: end, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: b, to: end, head: Marker::Arrow, ..Edge::default() });
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let end_row = row_of(&out, "◉");
        let glyph_col = char_col(&out[end_row], '◉');
        let arrow_cols: Vec<usize> = out[end_row - 1].chars().enumerate().filter(|&(_, c)| c == '▼').map(|(i, _)| i).collect();
        assert!(!arrow_cols.is_empty(), "화살촉이 있어야 한다: {text}");
        for col in arrow_cols {
            assert_eq!(col, glyph_col, "모든 화살촉이 원 글자(◉) 바로 위, 같은 열로 모여야 한다: {text}");
        }
    }

    /// bugfix bpmn-event-notation-anchor 재현 1.4·1.7(수정 전 실패): 왼쪽→오른쪽, 본문
    /// 세 줄인 종료 이벤트로 들어오는 간선이 둘이면 화살촉 모두 원 글자 바로 왼쪽, 같은
    /// 줄로 모여야 한다(수정 전엔 원 글자가 접점 벌림으로 밀려 다른 줄에 있다).
    #[test]
    fn left_right_three_line_event_two_incoming_edges_converge_directly_left_of_the_glyph() {
        let mut g = Graph::default();
        let a = g.intern("a", "A", Shape::Round, None);
        let b = g.intern("b", "B", Shape::Round, None);
        let end = g.intern("end", "one\ntwo\nthree", Shape::Event(EventPosition::End), None);
        g.add_edge(Edge { from: a, to: end, head: Marker::Arrow, ..Edge::default() });
        g.add_edge(Edge { from: b, to: end, head: Marker::Arrow, ..Edge::default() });
        g.direction = Some(Direction::LeftRight);
        let out = rows(render(&g, &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        let end_row = row_of(&out, "◉");
        let glyph_col = char_col(&out[end_row], '◉');
        for (i, row) in out.iter().enumerate() {
            if let Some(col) = row.chars().position(|c| c == '▶') {
                assert_eq!((i, col), (end_row, glyph_col - 1), "화살촉은 원 글자(◉)와 같은 줄, 바로 왼쪽 칸에 있어야 한다: {text}");
            }
        }
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
        for needle in ["○", "◉", "«user»", "«service»", "× 재고 있음?", "예", "아니오"] {
            assert!(text.contains(needle), "{needle:?}가 결과에 있어야 한다:\n{text}");
        }
    }

    // ── layout-crossgroup-edge-blowup: 재현·회귀 테스트 ──────────────────

    /// bugfix.md 재현 4의 생성기와 같은 모양: subgraph 8개(그룹당 6~7노드 사슬) + 두 그룹
    /// 사이를 잇는 무작위 간선 `cross_edges`개. `chain_groups`면 그룹 0→1→…→7을 먼저 이어(실사용
    /// 펜스처럼 그룹이 사슬이 되는 구조) 어느 시도로도 폭에 안 들어가는 그래프를 만든다.
    fn cross_group_flowchart(seed: u64, cross_edges: usize, chain_groups: bool, hub: bool, wide_labels: bool) -> Graph {
        let mut g = Graph::default();
        let mut rng = Xorshift64::new(seed);
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for gi in 0..8 {
            let size = if gi < 4 { 6 } else { 7 };
            let group = g.add_group(&format!("G{gi}"), None);
            let mut members = Vec::new();
            for j in 0..size {
                let id = format!("S{gi}_{j}");
                // 불균등 라벨형(bugfix.md 재현 4): 노드 30%에 긴 라벨을 줘 상자 폭을 들쭉날쭉하게 한다.
                let label = if wide_labels && rng.below(10) < 3 {
                    format!("{id} very long label text very long label text")
                } else {
                    id.clone()
                };
                members.push(g.intern(&id, &label, Shape::Rect, Some(group)));
            }
            for pair in members.windows(2) {
                g.add_edge(Edge { from: pair[0], to: pair[1], head: Marker::Arrow, ..Edge::default() });
            }
            groups.push(members);
        }
        if chain_groups {
            for gi in 0..7 {
                let from = *groups[gi].last().unwrap();
                let to = groups[gi + 1][0];
                g.add_edge(Edge { from, to, head: Marker::Arrow, ..Edge::default() });
            }
        }
        for _ in 0..cross_edges {
            let (g1, g2) = if hub {
                // 허브형(bugfix.md 재현 4): 교차 간선이 모두 그룹 0을 거친다 — 그룹 0이
                // 매 층마다 여러 그룹과 동시에 얽혀 접어도 좀처럼 풀리지 않는다.
                let other = 1 + rng.below(7);
                if rng.below(2) == 0 { (0, other) } else { (other, 0) }
            } else {
                let a = rng.below(8);
                let mut b = rng.below(8);
                while b == a {
                    b = rng.below(8);
                }
                (a, b)
            };
            let a = groups[g1][rng.below(groups[g1].len())];
            let b = groups[g2][rng.below(groups[g2].len())];
            g.add_edge(Edge { from: a, to: b, head: Marker::Arrow, ..Edge::default() });
        }
        g
    }

    /// 1.1: 교차 그룹 간선이 늘면 위→아래 층 접기 재시도가 쳇바퀴에 빠져(폭이 안 줄면서 층·
    /// 가상 노드·구간 수만 누적) 배치 한 번의 비용이 폭발한다(1.2, 1.3). 수정 전에는 이 상한을
    /// 훌쩍 넘겨 실패해야 하고, 수정 후(2.2, 2.3)에는 2초 안에 끝나야 한다(그림이든 폴백이든).
    /// 디버그 빌드는 release보다 여러 배 느리므로(`large_graph_renders_within_a_bounded_time`과
    /// 같은 이유) 상한은 release 실측(design.md)보다 넉넉하다.
    #[test]
    fn crossgroup_fold_retry_finishes_within_two_seconds() {
        let g = cross_group_flowchart(2, 20, false, false, false);
        let start = std::time::Instant::now();
        let _ = render(&g, &Theme::none(), 118);
        let elapsed = start.elapsed();
        assert!(elapsed < std::time::Duration::from_secs(2), "교차 그룹 간선 접기 재시도가 {elapsed:?}나 걸림(2초 상한)");
    }

    /// 1.2: 실사용 펜스처럼 그룹이 교차 간선으로 사슬이 되어 방향·라벨 폭 어느 시도로도 폭에
    /// 안 들어가는 그래프는 폴백(`None`) 판정까지 오래 걸리지 않아야 한다(1.1, 2.1). 수정 전엔
    /// 접기 쳇바퀴 때문에 상한을 넘겨 실패해야 한다.
    #[test]
    fn crossgroup_chain_gives_up_quickly() {
        let g = cross_group_flowchart(1, 60, true, true, false);
        let start = std::time::Instant::now();
        let out = render(&g, &Theme::none(), 118);
        let elapsed = start.elapsed();
        assert!(out.is_none(), "이 그래프는 폭 118 어디에도 들어가지 않아야 한다");
        // 수정 전(release 프로파일 없는 이 디버그 빌드) 14.68초 — 3초는 그래도 8배 가까운 여유를
        // 남긴다. release 실측(0.5초 상한, design.md 2.1)은 task 4.1에서 release 바이너리로 잰다.
        assert!(elapsed < std::time::Duration::from_secs(3), "폴백 판정이 {elapsed:?}나 걸림(3초 상한, 디버그 빌드 여유 포함)");
    }

    /// 2.1: 합성 가족(변형 4종 × seed 4개, 간선 적은 단계)의 결과 고정. `EXPECTED`는 수정 전
    /// 코드로 기록한 (줄 수, FNV-1a 해시) — 표 아래 diagnostic 테스트로 만들었다. 수정 후에도
    /// 그림으로 끝난 건은 바이트가 같아야 하고(3.1), 전 건이 1초 안에 끝나야 한다(2.2).
    // (설명, seed, cross_edges, chain, hub, wide_labels, 기대(줄 수, 해시) — None이면 미기록)
    type CrossgroupCase = (&'static str, u64, usize, bool, bool, bool, Option<(usize, u64)>);

    #[test]
    fn crossgroup_family_matches_recorded_results_and_stays_fast() {
        let cases: Vec<CrossgroupCase> = vec![
            ("base_s0_c5", 0, 5, false, false, false, Some((147, 0x1de81e2201b93dcf))),
            ("base_s0_c10", 0, 10, false, false, false, Some((154, 0x535c1bb5b35fe37f))),
            ("base_s1_c5", 1, 5, false, false, false, Some((88, 0xc493b82bebc5aa5b))),
            ("base_s1_c10", 1, 10, false, false, false, Some((115, 0x02c24620c2d32dc4))),
            ("base_s2_c5", 2, 5, false, false, false, Some((120, 0xb732327876e8d537))),
            ("base_s2_c10", 2, 10, false, false, false, Some((130, 0xdf8729fba82a2f20))),
            ("base_s3_c5", 3, 5, false, false, false, Some((125, 0xdadd30ce6fa5f092))),
            ("base_s3_c10", 3, 10, false, false, false, Some((147, 0x89300a615a3299b4))),
            ("chain_s0_c5", 0, 5, true, false, false, Some((353, 0x188bf37f4fe84187))),
            ("chain_s1_c5", 1, 5, true, false, false, Some((353, 0x4af265a59cda7f44))),
            ("chain_s2_c5", 2, 5, true, false, false, Some((355, 0x32b2b8672a5dc90d))),
            ("chain_s3_c5", 3, 5, true, false, false, Some((351, 0xd0c3b599224af277))),
            ("hub_s0_c5", 0, 5, false, true, false, Some((116, 0x4872a5c43bbbd101))),
            ("hub_s1_c5", 1, 5, false, true, false, Some((139, 0x319bdd4b77892b02))),
            ("hub_s2_c5", 2, 5, false, true, false, Some((142, 0x880afd64d24d5963))),
            ("hub_s3_c5", 3, 5, false, true, false, Some((77, 0x2264597b92e91ad7))),
            ("labels_s0_c5", 0, 5, false, false, true, Some((163, 0x1c07e64ffdaa22c0))),
            ("labels_s1_c5", 1, 5, false, false, true, Some((441, 0x1cb172b7e68ad19c))),
            ("labels_s2_c5", 2, 5, false, false, true, Some((319, 0xf9316dfcb158e337))),
            ("labels_s3_c5", 3, 5, false, false, true, Some((315, 0xcfcb625efc54043e))),
        ];
        let mut mismatches = Vec::new();
        let mut slow = Vec::new();
        for (name, seed, n, chain, hub, wide, expected) in cases {
            let g = cross_group_flowchart(seed, n, chain, hub, wide);
            let start = std::time::Instant::now();
            let out = render(&g, &Theme::none(), 118);
            let elapsed = start.elapsed();
            if elapsed >= std::time::Duration::from_secs(1) {
                slow.push(format!("{name}: {elapsed:?}"));
            }
            if let Some(lines) = out {
                let text = rows(lines).join("\n");
                let actual = (text.lines().count(), fnv1a(&text));
                eprintln!("CASE {name} lines={} hash={:#018x}", actual.0, actual.1);
                if let Some(exp) = expected
                    && actual != exp
                {
                    mismatches.push(format!("{name}: expected {exp:?}, got {actual:?}"));
                }
            } else {
                eprintln!("CASE {name} fallback(None)");
            }
        }
        assert!(mismatches.is_empty(), "결과가 바뀐 건: {mismatches:#?}");
        assert!(slow.is_empty(), "1초를 넘긴 건(디버그 빌드 여유 포함): {slow:#?}");
    }

    /// 가운데 노드 하나에 `incoming`개가 들어오고 `outgoing`개가 나가는 왼쪽→오른쪽 그래프.
    /// 가운데 노드는 본문이 비어 글자 없는 도형(테두리만)으로 그려진다.
    fn lr_hub_graph(shape: Shape, incoming: usize, outgoing: usize) -> Graph {
        lr_hub_graph_with_sections(shape, vec![Vec::new()], incoming, outgoing)
    }

    fn lr_hub_graph_with_sections(shape: Shape, sections: Vec<Vec<String>>, incoming: usize, outgoing: usize) -> Graph {
        let mut g = Graph { direction: Some(Direction::LeftRight), ..Graph::default() };
        let hub = g.intern("A", "A", shape, None);
        g.nodes[hub].sections = sections;
        for i in 0..incoming {
            let source = g.intern(&format!("I{i}"), &format!("i{i}"), Shape::Rect, None);
            g.add_edge(Edge { from: source, to: hub, head: Marker::Arrow, ..Edge::default() });
        }
        for i in 0..outgoing {
            let target = g.intern(&format!("O{i}"), &format!("o{i}"), Shape::Rect, None);
            g.add_edge(Edge { from: hub, to: target, head: Marker::Arrow, ..Edge::default() });
        }
        g
    }

    /// 들어오는 화살표는 모두 가운데 노드의 왼쪽 테두리 바로 앞 같은 열에 닿고,
    /// 나가는 흐름은 오른쪽 테두리 바로 뒤 칸에서 시작한다 — 그 줄 수를 센다.
    fn assert_glyphless_hub_ports_separate(shape: Shape) {
        let out = rows(render(&lr_hub_graph(shape, 2, 3), &Theme::none(), 80).unwrap());
        let text = out.join("\n");
        assert!(!text.contains(['┬', '┴', '┼']), "흐름이 한 점에 모였다:\n{text}");
        let cells: Vec<Vec<char>> = out.iter().map(|row| row.chars().collect()).collect();
        let arrow_col = cells.iter().filter_map(|row| row.iter().position(|&c| c == '▶')).min().unwrap();
        let (hub_width, _) = shape::measure(shape, &[Vec::new()]);
        let hub_right = arrow_col + hub_width;
        let incoming_rows = cells.iter().filter(|row| row.get(arrow_col) == Some(&'▶')).count();
        let outgoing_rows = cells.iter().filter(|row| row.get(hub_right + 1) == Some(&'─')).count();
        assert_eq!(incoming_rows, 2, "들어오는 흐름이 서로 다른 줄이 아니다:\n{text}");
        assert_eq!(outgoing_rows, 3, "나가는 흐름이 서로 다른 줄이 아니다:\n{text}");
    }

    #[test]
    fn left_right_glyphless_rect_separates_ports_per_flow() {
        assert_glyphless_hub_ports_separate(Shape::Rect);
    }

    #[test]
    fn left_right_glyphless_diamond_separates_ports_per_flow() {
        assert_glyphless_hub_ports_separate(Shape::Diamond);
    }

    #[test]
    fn left_right_glyphless_box_with_one_flow_per_side_keeps_two_rows() {
        let out = rows(render(&lr_hub_graph(Shape::Rect, 1, 1), &Theme::none(), 80).unwrap());
        assert_eq!(out, ["┌────┐          ┌────┐", "│ i0 │──▶┌──┐──▶│ o0 │", "└────┘   └──┘   └────┘"]);
        let out = rows(render(&lr_hub_graph(Shape::Diamond, 1, 1), &Theme::none(), 80).unwrap());
        assert_eq!(out, ["┌────┐            ┌────┐", "│ i0 │──▶ ╱──╲ ──▶│ o0 │", "└────┘    ╲──╱    └────┘"]);
    }

    #[test]
    fn left_right_point_anchored_node_still_converges_flows() {
        let out = rows(render(&lr_hub_graph(Shape::Start, 0, 3), &Theme::none(), 80).unwrap());
        assert_eq!(
            out,
            ["     ┌────┐", "  ╭─▶│ o0 │", "  │  └────┘", "  │", "  │  ┌────┐", "●─┴┬▶│ o1 │", "   │ └────┘", "   │", "   │ ┌────┐", "   ╰▶│ o2 │", "     └────┘"]
        );
        let out = rows(render(&lr_hub_graph(Shape::Event(EventPosition::Start), 0, 3), &Theme::none(), 80).unwrap());
        assert_eq!(
            out,
            ["     ┌────┐", "  ╭─▶│ o0 │", "  │  └────┘", "  │", "  │  ┌────┐", "○─┴┬▶│ o1 │", "   │ └────┘", "   │", "   │ ┌────┐", "   ╰▶│ o2 │", "     └────┘"]
        );
    }

    /// 같은 쪽 흐름이 가운데 노드 옆 칸에서 몇 줄로 드나드는지 센다. 들어오는 쪽은 가운데 노드 바로 앞
    /// 열의 `▶`, 나가는 쪽은 가운데 노드 바로 뒤 열에서 왼쪽(노드 쪽)으로 이어지는 선 글자다.
    /// 나가는 그래프에서 가운데 노드는 첫 층이라 0열에서 시작한다.
    fn hub_side_port_rows(out: &[String], hub_width: usize, is_incoming: bool) -> usize {
        let cells: Vec<Vec<char>> = out.iter().map(|row| row.chars().collect()).collect();
        if is_incoming {
            let Some(arrow_col) = cells.iter().filter_map(|row| row.iter().position(|&c| c == '▶')).min() else { return 0 };
            cells.iter().filter(|row| row.get(arrow_col) == Some(&'▶')).count()
        } else {
            const CONNECTS_LEFT: [char; 9] = ['─', '┬', '┴', '┼', '┤', '╮', '╯', '┐', '┘'];
            cells.iter().filter(|row| row.get(hub_width).is_some_and(|c| CONNECTS_LEFT.contains(c))).count()
        }
    }

    #[test]
    fn left_right_border_box_gives_each_same_side_flow_its_own_row() {
        const ALL_SHAPES: [Shape; 19] = [
            Shape::Rect,
            Shape::Round,
            Shape::Stadium,
            Shape::Cylinder,
            Shape::Diamond,
            Shape::Hexagon,
            Shape::Subroutine,
            Shape::Circle,
            Shape::Actor,
            Shape::Interface,
            Shape::Note,
            Shape::Plain,
            Shape::Start,
            Shape::End,
            Shape::Anchor,
            Shape::Event(EventPosition::Start),
            Shape::Event(EventPosition::Intermediate),
            Shape::Event(EventPosition::End),
            Shape::Subprocess,
        ];
        let border_boxes: Vec<Shape> = ALL_SHAPES.into_iter().filter(|shape| shape.is_border_box()).collect();
        assert_eq!(border_boxes.len(), 10, "테두리 상자 도형 10종 전부를 입력 공간으로 쓴다");
        let mut failures: Vec<String> = Vec::new();
        for shape in border_boxes {
            for is_named in [false, true] {
                let sections = if is_named { vec![vec!["Hub".to_string()]] } else { vec![Vec::new()] };
                let (hub_width, _) = shape::measure(shape, &sections);
                for flows in 2..=5 {
                    for is_incoming in [true, false] {
                        let (incoming, outgoing) = if is_incoming { (flows, 0) } else { (0, flows) };
                        let graph = lr_hub_graph_with_sections(shape, sections.clone(), incoming, outgoing);
                        let Some(lines) = render(&graph, &Theme::none(), 80) else {
                            failures.push(format!("shape={shape:?} named={is_named} k={flows} incoming={is_incoming}: 렌더링 실패"));
                            continue;
                        };
                        let out = rows(lines);
                        let port_rows = hub_side_port_rows(&out, hub_width, is_incoming);
                        if port_rows != flows {
                            failures.push(format!("shape={shape:?} named={is_named} k={flows} incoming={is_incoming}: 노드 옆 칸 {port_rows}줄\n{}", out.join("\n")));
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "같은 쪽 흐름이 노드 옆 칸에서 서로 다른 줄이 아니다({}건):\n{}", failures.len(), failures.join("\n\n"));
    }

    #[test]
    fn starts_nearest_first_lists_every_start_in_the_sorted_order() {
        // 이름표 자리 후보를 모두 늘어놓아 (가운데와의 거리, 칸)으로 정렬한 순서와 같아야 고르는 자리가 바뀌지 않는다.
        for (first, last) in [(0, 0), (1, 2), (3, 9), (5, 40)] {
            for centered in [0, first, (first + last) / 2, last, last + 3] {
                let mut sorted: Vec<usize> = (first..=last).collect();
                sorted.sort_by_key(|&x| (x.abs_diff(centered), x));
                assert_eq!(starts_nearest_first(first, last, centered).collect::<Vec<_>>(), sorted, "first={first} last={last} centered={centered}");
            }
        }
    }

    #[test]
    fn spans_on_lines_finds_the_spans_of_each_line() {
        let documents: [Vec<SpanOnLine>; 3] = [
            vec![((true, 4), (1, 3)), ((false, 0), (2, 5)), ((true, 0), (0, 9)), ((false, 7), (1, 1)), ((true, 4), (0, 2)), ((false, 7), (0, 4)), ((false, 2), (3, 3))],
            vec![((true, 3), (0, 1)), ((true, 3), (0, 1))],
            Vec::new(),
        ];
        for entries in documents {
            let mut lines = SpansOnLines { entries: entries.clone(), ..SpansOnLines::default() };
            lines.sort();
            for line in [false, true].into_iter().flat_map(|is_across| (0..10).map(move |fixed| (is_across, fixed))) {
                let mut expected: Vec<SpanOnLine> = entries.iter().copied().filter(|&(on, _)| on == line).collect();
                expected.sort_unstable();
                assert_eq!(lines.on(line), expected.as_slice(), "줄 {line:?}");
            }
        }
    }
}
