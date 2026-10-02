//! 검증된 `Model`을 기존 `ir::Graph` 어휘로 옮긴다. BPMN 의미는 여기서 끝난다.
//!
//! `lower_with`가 유일한 변환 본체다: 정책(`ExpandPolicy`)에 따라 어떤 `Subprocess`를 펼치고
//! 접을지 정하고(요소 깊이는 `Model::depth_of` — L2 0·L4 1·L5 2), 장(chapter)마다
//! `ir::Graph` 하나를 만든다. `lower(model)`은 `lower_with(model, Depth(0))`의 첫 장과 같다 —
//! `parent`가 없는 모델(XML·YAML)은 모든 요소가 깊이 0이라 펼칠 것이 없고, 이 스펙 이전과 바이트
//! 단위로 같은 그래프가 된다(7.6).

use super::model::{Element, ElementKind, FlowKind, Lane, Model, Orientation};
use super::vocabulary::{CALL_ACTIVITY_LABEL, DATA_OBJECT_LABEL};
use crate::diagram::ir::{Direction, Edge, Graph, LineKind, Marker, Shape};
use crate::diagram::options::ExpandPolicy;
use std::collections::{HashMap, HashSet};

/// 여러 장(chapter) 가운데 하나의 성격. 캡션 종류 문자열의 SSoT(`caption_kind`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagramKind {
    Process,
    Activity(String),
    Step(String),
}

impl DiagramKind {
    pub fn caption_kind(&self) -> String {
        match self {
            DiagramKind::Process => "process".to_string(),
            DiagramKind::Activity(name) => format!("activity: {name}"),
            DiagramKind::Step(name) => format!("step: {name}"),
        }
    }
}

pub struct LoweredDiagram {
    pub kind: DiagramKind,
    pub graph: Graph,
}

/// 검증된 모델 전제. 해석 안 되는 참조는 건너뛴다(패닉 없음). `lower_with(model, Depth(0))`의
/// 첫 장과 같다 — `parent` 없는 모델(XML·YAML)은 이 스펙 이전과 같은 그래프.
pub fn lower(model: &Model) -> Graph {
    lower_with(model, ExpandPolicy::Depth(0)).into_iter().next().expect("항상 한 장 이상 돌려준다").graph
}

/// 검증된 모델 전제. 정책 허용 여부(`validate::participant_transitions`)는 호출자가 먼저 정한다.
pub fn lower_with(model: &Model, policy: ExpandPolicy) -> Vec<LoweredDiagram> {
    match policy {
        ExpandPolicy::Depth(n) => vec![LoweredDiagram { kind: DiagramKind::Process, graph: build_graph(model, None, Some(n), &HashSet::new()) }],
        ExpandPolicy::All => vec![LoweredDiagram { kind: DiagramKind::Process, graph: build_graph(model, None, None, &HashSet::new()) }],
        ExpandPolicy::PerActivity => {
            let mut out = vec![LoweredDiagram { kind: DiagramKind::Process, graph: build_graph(model, None, Some(0), &HashSet::new()) }];
            let transitioning: HashSet<String> = super::validate::participant_transitions(model).into_iter().collect();
            for l2 in &model.elements {
                if !matches!(l2.kind, ElementKind::Subprocess) || l2.parent.is_some() {
                    continue;
                }
                if !has_step_children(model, &l2.id) {
                    continue;
                }
                let name = first_line(&l2.name).to_string();
                lower_chapter(model, &l2.id, DiagramKind::Activity(name), &transitioning, &mut out);
            }
            out
        }
    }
}

fn has_step_children(model: &Model, l2_id: &str) -> bool {
    model.children_of(l2_id).any(|e| matches!(e.kind, ElementKind::Task(_) | ElementKind::Subprocess))
}

fn first_line(name: &str) -> &str {
    name.lines().next().unwrap_or("")
}

/// `root_id`를 뿌리로(제 몸은 상자 없이) 한 장을 만들고, 그 안에서 접힌(참여자 전환) 자식
/// `Subprocess`마다 `Step` 장을 재귀로 뒤에 붙인다(6.5).
fn lower_chapter(model: &Model, root_id: &str, kind: DiagramKind, transitioning: &HashSet<String>, out: &mut Vec<LoweredDiagram>) {
    let graph = build_graph(model, Some(root_id), None, transitioning);
    out.push(LoweredDiagram { kind, graph });
    for element in &model.elements {
        if matches!(element.kind, ElementKind::Subprocess) && element.parent.as_deref() == Some(root_id) && transitioning.contains(&element.id) {
            let name = first_line(&element.name).to_string();
            lower_chapter(model, &element.id, DiagramKind::Step(name), transitioning, out);
        }
    }
}

fn orientation_to_direction(orientation: Orientation) -> Direction {
    match orientation {
        Orientation::Horizontal => Direction::LeftRight,
        Orientation::Vertical => Direction::TopDown,
    }
}

/// 풀·(중첩) 레인을 선언 순서대로 레인 그룹으로 만든다(정책 무관, 항상 전부). 돌려주는 표는
/// 모델 id(참여자·레인) → 그래프 그룹 인덱스.
fn add_pools_and_lanes(graph: &mut Graph, model: &Model) -> HashMap<String, usize> {
    let mut group_of = HashMap::new();
    for participant in &model.participants {
        let group = graph.add_lane(&participant.name, None);
        group_of.insert(participant.id.clone(), group);
        add_lanes(graph, &participant.lanes, Some(group), &mut group_of);
    }
    group_of
}

fn add_lanes(graph: &mut Graph, lanes: &[Lane], parent: Option<usize>, group_of: &mut HashMap<String, usize>) {
    for lane in lanes {
        let group = graph.add_lane(&lane.name, parent);
        group_of.insert(lane.id.clone(), group);
        add_lanes(graph, &lane.sub_lanes, Some(group), group_of);
    }
}

/// `id`가 `root`(그 자신 제외)의 자손인지 — `parent` 사슬을 타고 올라가며 확인한다(상한 64).
fn is_descendant_of(model: &Model, id: &str, root: &str) -> bool {
    let mut cursor = model.element(id).and_then(|e| e.parent.clone());
    for _ in 0..64 {
        match cursor {
            Some(pid) if pid == root => return true,
            Some(pid) => cursor = model.element(&pid).and_then(|e| e.parent.clone()),
            None => return false,
        }
    }
    false
}

/// `id`가 이 장(chapter)에 보여야 하는지 — 범위(`subtree_root`) 안이고, 깊이(`max_depth`,
/// `subtree_root`가 없을 때만) 또는 사이에 접힌 조상이 없어야 한다.
fn is_visible(model: &Model, id: &str, subtree_root: Option<&str>, max_depth: Option<usize>, force_fold: &HashSet<String>) -> bool {
    if Some(id) == subtree_root {
        return false;
    }
    match subtree_root {
        Some(root) => {
            if !is_descendant_of(model, id, root) {
                return false;
            }
        }
        None => {
            if let Some(n) = max_depth
                && model.depth_of(id) > n
            {
                return false;
            }
        }
    }
    let mut cursor = model.element(id).and_then(|e| e.parent.clone());
    while let Some(pid) = cursor {
        if Some(pid.as_str()) == subtree_root {
            break;
        }
        if is_folded(model, &pid, max_depth, force_fold) {
            return false;
        }
        cursor = model.element(&pid).and_then(|e| e.parent.clone());
    }
    true
}

/// `id`(Sub-Process)가 접힌 채(상자 아님, 접힌 노드) 그려지는지.
fn is_folded(model: &Model, id: &str, max_depth: Option<usize>, force_fold: &HashSet<String>) -> bool {
    if force_fold.contains(id) {
        return true;
    }
    if !matches!(model.element(id).map(|e| e.kind), Some(ElementKind::Subprocess)) {
        return false;
    }
    match max_depth {
        Some(n) => model.depth_of(id) == n,
        None => false,
    }
}

/// 요소·그룹의 `parent`(Sub-Process id 또는 `None`)가 가리키는 그래프 그룹: 펼친 상자 → 그 상자,
/// `subtree_root`와 같음(자기 몸이 지워진 장의 뿌리) → 상자 없음(레인/루트로 대체), 그 외 →
/// `container`(레인) 또는 루트.
fn resolve_parent_group(parent_id: Option<&str>, subtree_root: Option<&str>, box_of: &HashMap<String, usize>, lane_group_of: &HashMap<String, usize>, container: Option<&str>) -> Option<usize> {
    if let Some(pid) = parent_id
        && Some(pid) != subtree_root
        && let Some(&g) = box_of.get(pid)
    {
        return Some(g);
    }
    container.and_then(|c| lane_group_of.get(c)).copied()
}

/// 흐름 끝을 그래프 노드로. 이미 인턴돼 있으면 그것, 펼친 상자를 가리키면 그 상자의 닻
/// (`Graph::group_anchor`), 메시지 흐름이 최상위 참여자를 가리키면 그 참여자 그룹의 닻. 그 외
/// (모르는 id 등)는 `None`(간선 생략).
fn resolve_flow_node(graph: &mut Graph, model: &Model, box_of: &HashMap<String, usize>, lane_group_of: &HashMap<String, usize>, kind: FlowKind, id: &str) -> Option<usize> {
    if let Some(index) = graph.find(id) {
        return Some(index);
    }
    if let Some(&g) = box_of.get(id) {
        return Some(graph.group_anchor(g));
    }
    if matches!(kind, FlowKind::Message) && model.participant_index(id).is_some() {
        return Some(graph.group_anchor(lane_group_of[id]));
    }
    None
}

/// 장 하나를 만든다. `subtree_root = None`이면 모델 전체(Process 정책들), `Some(id)`면 그 요소의
/// 자손만(Activity·Step — 자기 몸은 상자 없이 지워진다). `max_depth = None`이면 깊이 무제한
/// (`subtree_root` 있을 때, 또는 `All`) — `force_fold`만으로 접는다.
fn build_graph(model: &Model, subtree_root: Option<&str>, max_depth: Option<usize>, force_fold: &HashSet<String>) -> Graph {
    let mut graph = Graph { direction: Some(orientation_to_direction(model.orientation)), title: if subtree_root.is_none() { model.title.clone() } else { String::new() }, ..Graph::default() };

    let lane_group_of = add_pools_and_lanes(&mut graph, model);
    let is_vis = |id: &str| is_visible(model, id, subtree_root, max_depth, force_fold);
    let is_fold = |id: &str| is_folded(model, id, max_depth, force_fold);

    // 펼칠 Sub-Process 후보를 깊이 순으로: 부모 상자가 먼저 만들어져야 자식이 그 상자를
    // `parent`로 참조할 수 있다. L3 그룹은 자신을 품은 Sub-Process와 같은 깊이 직후에 만든다
    // (그 Sub-Process가 펼쳐졌거나 `subtree_root`로 지워졌을 때만 멤버가 보이므로).
    let mut expandable: Vec<&Element> = model.elements.iter().filter(|e| matches!(e.kind, ElementKind::Subprocess) && is_vis(&e.id) && !is_fold(&e.id)).collect();
    expandable.sort_by_key(|e| model.depth_of(&e.id));

    let mut box_of: HashMap<String, usize> = HashMap::new();
    let mut member_group_of: HashMap<String, usize> = HashMap::new();
    let mut processed_group_depths: HashSet<usize> = HashSet::new();

    let mut i = 0;
    while i < expandable.len() {
        let depth = model.depth_of(&expandable[i].id);
        while i < expandable.len() && model.depth_of(&expandable[i].id) == depth {
            let element = expandable[i];
            let parent_group = member_group_of.get(&element.id).copied().or_else(|| resolve_parent_group(element.parent.as_deref(), subtree_root, &box_of, &lane_group_of, element.container.as_deref()));
            let title = first_line(&element.name);
            let group = graph.add_group(title, parent_group);
            box_of.insert(element.id.clone(), group);
            i += 1;
        }
        add_l3_groups_at_depth(&mut graph, model, depth, subtree_root, &is_vis, &box_of, &lane_group_of, &mut member_group_of);
        processed_group_depths.insert(depth);
    }
    // 펼친 Sub-Process가 하나도 없어도(예: `Depth(0)`) L2(깊이 0) 바로 아래 L3 그룹은 여전히
    // `subtree_root`로 자기 몸이 지워진 장(Activity)에서 보일 수 있다.
    if !processed_group_depths.contains(&0) {
        add_l3_groups_at_depth(&mut graph, model, 0, subtree_root, &is_vis, &box_of, &lane_group_of, &mut member_group_of);
    }

    for element in &model.elements {
        if !is_vis(&element.id) {
            continue;
        }
        if matches!(element.kind, ElementKind::Subprocess) && !is_fold(&element.id) {
            continue; // 펼친 상자 자신은 노드가 아니다.
        }
        // 경계 이벤트의 소속이 비어 있으면 호스트의 그룹을 따른다(기존 규약).
        let effective_container = element.container.as_deref().or_else(|| {
            let host = model.element(element.attached_to.as_deref()?)?;
            host.container.as_deref()
        });
        let group = member_group_of
            .get(&element.id)
            .copied()
            .or_else(|| element.parent.as_deref().filter(|p| Some(*p) != subtree_root).and_then(|p| box_of.get(p)).copied())
            .or_else(|| effective_container.and_then(|c| lane_group_of.get(c)).copied());
        let index = graph.intern(&element.id, &element.name, shape_of(element.kind), group);
        graph.set_label(index, &section_text(element.kind, &element.name));
    }

    for flow in &model.flows {
        let (Some(from), Some(to)) = (resolve_flow_node(&mut graph, model, &box_of, &lane_group_of, flow.kind, &flow.source), resolve_flow_node(&mut graph, model, &box_of, &lane_group_of, flow.kind, &flow.target)) else { continue };
        let (kind, tail, head) = line_for(flow.kind);
        graph.add_edge(Edge { from, to, label: flow.label.clone(), kind, tail, head, ..Edge::default() });
    }

    add_placeholders_for_empty_groups(&mut graph);
    add_boundary_edges(&mut graph, model, &box_of);

    graph
}

/// `depth`에 있는 Sub-Process를 부모로 둔 `Model.groups`(L3) 가운데 가시 멤버가 하나 이상이고
/// 멤버의 `container`가 전부 같은 것만 파선 상자로 만든다(6.8: 레인이 갈리면 그룹 생략). 멤버
/// id → 그래프 그룹 인덱스를 `member_group_of`에 채운다.
#[allow(clippy::too_many_arguments)]
fn add_l3_groups_at_depth(graph: &mut Graph, model: &Model, depth: usize, subtree_root: Option<&str>, is_vis: &impl Fn(&str) -> bool, box_of: &HashMap<String, usize>, lane_group_of: &HashMap<String, usize>, member_group_of: &mut HashMap<String, usize>) {
    for group in &model.groups {
        let owner_depth = group.parent.as_deref().map(|p| model.depth_of(p)).unwrap_or(usize::MAX);
        if owner_depth != depth {
            continue;
        }
        let visible_members: Vec<&String> = group.members.iter().filter(|m| is_vis(m)).collect();
        if visible_members.is_empty() {
            continue;
        }
        let containers: HashSet<Option<&str>> = visible_members.iter().filter_map(|m| model.element(m)).map(|e| e.container.as_deref()).collect();
        if containers.len() != 1 {
            continue; // 멤버 레인이 갈리면 그룹 상자를 생략한다(6.8).
        }
        // 부모(L2)가 펼친 상자면 그 상자, 아니면(접힘·`subtree_root`로 지워짐) 멤버가 공유하는
        // 레인 — 부모도 레인도 없으면 루트.
        let shared_container = containers.into_iter().next().flatten();
        let parent_group = resolve_parent_group(group.parent.as_deref(), subtree_root, box_of, lane_group_of, shared_container);
        let gi = graph.add_group_with_id(&group.id, &group.name, parent_group);
        graph.set_group_line(gi, LineKind::Dashed);
        for member in visible_members {
            member_group_of.insert(member.clone(), gi);
        }
    }
}

/// 노드가 하나도 없는(자기 자신 + 하위 레인 전부) 풀·레인마다 보이지 않는 자리표시 노드 하나.
fn add_placeholders_for_empty_groups(graph: &mut Graph) {
    let mut has_element = vec![false; graph.groups.len()];
    for node in &graph.nodes {
        if let Some(group) = node.group {
            for ancestor in graph.ancestors(Some(group)) {
                has_element[ancestor] = true;
            }
        }
    }
    for (group, &populated) in has_element.iter().enumerate() {
        if !populated {
            let id = format!("@bpmn-placeholder:{group}");
            let index = graph.intern(&id, "", Shape::Anchor, Some(group));
            graph.set_label(index, "");
        }
    }
}

/// 경계 이벤트마다 호스트 → 이벤트로 표식·라벨 없는 실선 간선 하나(시퀀스 흐름과 같은 선). 호스트가 펼친 상자면 그
/// 상자의 닻에서(`Graph::group_anchor`), 접혀 있으면(또는 상자가 아니면, 기존 규약) 호스트
/// 노드에서 바로 잇는다.
fn add_boundary_edges(graph: &mut Graph, model: &Model, box_of: &HashMap<String, usize>) {
    for element in &model.elements {
        let Some(host_id) = &element.attached_to else { continue };
        let Some(to) = graph.find(&element.id) else { continue };
        let from = match box_of.get(host_id) {
            Some(&g) => graph.group_anchor(g),
            None => match graph.find(host_id) {
                Some(i) => i,
                None => continue,
            },
        };
        graph.add_edge(Edge { from, to, kind: LineKind::Solid, ..Edge::default() });
    }
}

fn shape_of(kind: ElementKind) -> Shape {
    match kind {
        ElementKind::Event { position, .. } => Shape::Event(position),
        ElementKind::Task(_) => Shape::Round,
        ElementKind::Subprocess => Shape::Subprocess,
        ElementKind::CallActivity => Shape::Round,
        ElementKind::Gateway(_) => Shape::Diamond,
        ElementKind::DataObject => Shape::Rect,
        ElementKind::DataStore => Shape::Cylinder,
        ElementKind::TextAnnotation => Shape::Note,
    }
}

/// 노드 본문(섹션 0)의 줄들을 `\n`으로 이은 문자열. `Graph::set_label`이 그대로 줄로 쪼갠다.
fn section_text(kind: ElementKind, name: &str) -> String {
    match kind {
        ElementKind::Event { trigger, .. } => {
            let mut lines: Vec<&str> = Vec::new();
            if !name.is_empty() {
                lines.push(name);
            }
            let trigger_label = trigger.map(super::model::EventTrigger::label);
            if let Some(label) = &trigger_label {
                lines.push(label);
            }
            lines.join("\n")
        }
        ElementKind::Task(task_kind) => match task_kind.label() {
            Some(label) => format!("{name}\n{label}"),
            None => name.to_string(),
        },
        ElementKind::Subprocess | ElementKind::DataStore | ElementKind::TextAnnotation => name.to_string(),
        ElementKind::CallActivity => format!("{name}\n{CALL_ACTIVITY_LABEL}"),
        ElementKind::Gateway(gateway_kind) => format!("{} {name}", gateway_kind.label()).trim_end().to_string(),
        ElementKind::DataObject => format!("{name}\n{DATA_OBJECT_LABEL}"),
    }
}

fn line_for(kind: FlowKind) -> (LineKind, Marker, Marker) {
    match kind {
        FlowKind::Sequence { is_default: false } => (LineKind::Solid, Marker::None, Marker::Arrow),
        FlowKind::Sequence { is_default: true } => (LineKind::Solid, Marker::Slash, Marker::Arrow),
        FlowKind::Message => (LineKind::Dashed, Marker::Circle, Marker::Triangle),
        FlowKind::Association => (LineKind::Dotted, Marker::None, Marker::None),
        FlowKind::DataAssociation => (LineKind::Dotted, Marker::None, Marker::OpenArrow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{Element, EventPosition, EventTrigger, Flow, GatewayKind, Group, Participant, TaskKind};
    use crate::diagram::ir::GroupKind;

    fn element(id: &str, kind: ElementKind) -> Element {
        Element { id: id.into(), name: id.into(), kind, container: None, attached_to: None, parent: None }
    }

    #[test]
    fn node_kinds_map_to_shapes_and_sections_per_the_table() {
        let model = Model {
            elements: vec![
                Element { id: "e1".into(), name: "타이머".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: None, parent: None },
                Element { id: "e2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: None, attached_to: None, parent: None },
                Element { id: "e3".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: None, parent: None },
                Element { id: "t1".into(), name: "검토".into(), kind: ElementKind::Task(TaskKind::User), container: None, attached_to: None, parent: None },
                Element { id: "t2".into(), name: "처리".into(), kind: ElementKind::Task(TaskKind::None), container: None, attached_to: None, parent: None },
                element("sp", ElementKind::Subprocess),
                element("ca", ElementKind::CallActivity),
                Element { id: "g1".into(), name: "재고 있음?".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: None, attached_to: None, parent: None },
                Element { id: "g2".into(), name: "".into(), kind: ElementKind::Gateway(GatewayKind::Parallel), container: None, attached_to: None, parent: None },
                element("do", ElementKind::DataObject),
                element("ds", ElementKind::DataStore),
                element("ta", ElementKind::TextAnnotation),
            ],
            ..Model::default()
        };
        let graph = lower(&model);
        let node = |id: &str| &graph.nodes[graph.find(id).unwrap()];

        assert_eq!(node("e1").shape, Shape::Event(EventPosition::Start));
        assert_eq!(node("e1").sections, vec![vec!["타이머".to_string(), "«timer»".to_string()]]);
        assert_eq!(node("e2").sections, vec![Vec::<String>::new()]);
        assert_eq!(node("e3").sections, vec![vec!["«timer»".to_string()]]);
        assert_eq!(node("t1").shape, Shape::Round);
        assert_eq!(node("t1").sections, vec![vec!["검토".to_string(), "«user»".to_string()]]);
        assert_eq!(node("t2").sections, vec![vec!["처리".to_string()]]);
        assert_eq!(node("sp").shape, Shape::Subprocess);
        assert_eq!(node("sp").sections, vec![vec!["sp".to_string()]]);
        assert_eq!(node("ca").shape, Shape::Round);
        assert_eq!(node("ca").sections, vec![vec!["ca".to_string(), "«call»".to_string()]]);
        assert_eq!(node("g1").shape, Shape::Diamond);
        assert_eq!(node("g1").sections, vec![vec!["× 재고 있음?".to_string()]]);
        assert_eq!(node("g2").sections, vec![vec!["+".to_string()]]);
        assert_eq!(node("do").shape, Shape::Rect);
        assert_eq!(node("do").sections, vec![vec!["do".to_string(), "«data»".to_string()]]);
        assert_eq!(node("ds").shape, Shape::Cylinder);
        assert_eq!(node("ds").sections, vec![vec!["ds".to_string()]]);
        assert_eq!(node("ta").shape, Shape::Note);
        assert_eq!(node("ta").sections, vec![vec!["ta".to_string()]]);
    }

    #[test]
    fn pools_and_lanes_become_lane_groups_in_declaration_order() {
        let model = Model {
            participants: vec![
                Participant { id: "p1".into(), name: "고객".into(), lanes: Vec::new() },
                Participant {
                    id: "p2".into(),
                    name: "판매사".into(),
                    lanes: vec![Lane { id: "l1".into(), name: "영업".into(), sub_lanes: Vec::new() }, Lane { id: "l2".into(), name: "창고".into(), sub_lanes: Vec::new() }],
                },
            ],
            elements: vec![element("a", ElementKind::Task(TaskKind::None))],
            ..Model::default()
        };
        let graph = lower(&model);
        assert_eq!(graph.groups.len(), 4);
        assert!(graph.groups.iter().all(|g| g.kind == GroupKind::Lane));
        assert_eq!(graph.groups[0].title, "고객");
        assert_eq!(graph.groups[0].parent, None);
        assert_eq!(graph.groups[1].title, "판매사");
        assert_eq!(graph.groups[1].parent, None);
        assert_eq!(graph.groups[2].title, "영업");
        assert_eq!(graph.groups[2].parent, Some(1));
        assert_eq!(graph.groups[3].title, "창고");
        assert_eq!(graph.groups[3].parent, Some(1));
    }

    #[test]
    fn model_without_pools_has_no_groups() {
        let model = Model { elements: vec![element("a", ElementKind::Task(TaskKind::None))], ..Model::default() };
        assert!(lower(&model).groups.is_empty());
    }

    #[test]
    fn orientation_maps_to_direction_and_title_is_carried() {
        let mut model = Model { title: "주문 처리".into(), elements: vec![element("a", ElementKind::Task(TaskKind::None))], ..Model::default() };
        assert_eq!(lower(&model).direction, Some(Direction::LeftRight));
        assert_eq!(lower(&model).title, "주문 처리");
        model.orientation = Orientation::Vertical;
        assert_eq!(lower(&model).direction, Some(Direction::TopDown));
    }

    #[test]
    fn flow_kinds_map_to_lines_per_the_table() {
        let model = Model {
            elements: vec![element("a", ElementKind::Task(TaskKind::None)), element("b", ElementKind::Task(TaskKind::None))],
            flows: vec![
                Flow { id: "f1".into(), source: "a".into(), target: "b".into(), label: "조건".into(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f2".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Sequence { is_default: true } },
                Flow { id: "f3".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Message },
                Flow { id: "f4".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Association },
                Flow { id: "f5".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::DataAssociation },
            ],
            ..Model::default()
        };
        let graph = lower(&model);
        assert_eq!((graph.edges[0].kind, graph.edges[0].tail, graph.edges[0].head, graph.edges[0].label.as_str()), (LineKind::Solid, Marker::None, Marker::Arrow, "조건"));
        assert_eq!((graph.edges[1].kind, graph.edges[1].tail, graph.edges[1].head), (LineKind::Solid, Marker::Slash, Marker::Arrow));
        assert_eq!((graph.edges[2].kind, graph.edges[2].tail, graph.edges[2].head), (LineKind::Dashed, Marker::Circle, Marker::Triangle));
        assert_eq!((graph.edges[3].kind, graph.edges[3].tail, graph.edges[3].head), (LineKind::Dotted, Marker::None, Marker::None));
        assert_eq!((graph.edges[4].kind, graph.edges[4].tail, graph.edges[4].head), (LineKind::Dotted, Marker::None, Marker::OpenArrow));
    }

    #[test]
    fn boundary_event_gets_an_extra_unmarked_solid_edge_from_host_and_shares_its_group() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![
                Element { id: "host".into(), name: "작업".into(), kind: ElementKind::Task(TaskKind::None), container: Some("p1".into()), attached_to: None, parent: None },
                Element { id: "boundary".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: None, attached_to: Some("host".into()), parent: None },
            ],
            ..Model::default()
        };
        let graph = lower(&model);
        let host_group = graph.nodes[graph.find("host").unwrap()].group;
        let boundary_group = graph.nodes[graph.find("boundary").unwrap()].group;
        assert_eq!(host_group, boundary_group);
        let extra_edges: Vec<_> = graph.edges.iter().filter(|e| e.from == graph.find("host").unwrap() && e.to == graph.find("boundary").unwrap()).collect();
        assert_eq!(extra_edges.len(), 1);
        assert_eq!(extra_edges[0].kind, LineKind::Solid);
        assert_eq!(extra_edges[0].tail, Marker::None);
        assert_eq!(extra_edges[0].head, Marker::None);
        assert!(extra_edges[0].label.is_empty());
    }

    #[test]
    fn empty_pool_gets_one_anchor_placeholder_while_populated_pool_gets_none() {
        let model = Model {
            participants: vec![Participant { id: "empty".into(), name: "빈 풀".into(), lanes: Vec::new() }, Participant { id: "full".into(), name: "찬 풀".into(), lanes: Vec::new() }],
            elements: vec![Element { id: "a".into(), name: "a".into(), kind: ElementKind::Task(TaskKind::None), container: Some("full".into()), attached_to: None, parent: None }],
            ..Model::default()
        };
        let graph = lower(&model);
        let empty_group = 0;
        let full_group = 1;
        let anchors_in = |group: usize| graph.nodes.iter().filter(|n| n.shape == Shape::Anchor && n.group == Some(group)).count();
        assert_eq!(anchors_in(empty_group), 1);
        assert_eq!(anchors_in(full_group), 0);
    }

    /// bugfix.md 재현 절차 2 — 검증을 거치지 않고 그대로 변환한다.
    fn blackbox_pool_message_flow_model() -> Model {
        Model {
            participants: vec![
                Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() },
                Participant {
                    id: "seller".into(),
                    name: "판매사".into(),
                    lanes: vec![Lane { id: "sales".into(), name: "영업".into(), sub_lanes: Vec::new() }],
                },
            ],
            elements: vec![
                Element { id: "start".into(), name: "start".into(), kind: ElementKind::Task(TaskKind::None), container: Some("sales".into()), attached_to: None, parent: None },
                Element { id: "review".into(), name: "review".into(), kind: ElementKind::Task(TaskKind::None), container: Some("sales".into()), attached_to: None, parent: None },
            ],
            flows: vec![
                Flow { id: "s1".into(), source: "start".into(), target: "review".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "m1".into(), source: "customer".into(), target: "start".into(), label: "주문".into(), kind: FlowKind::Message },
            ],
            ..Model::default()
        }
    }

    #[test]
    fn message_flow_ending_at_a_participant_id_becomes_an_edge_to_the_group_anchor() {
        let model = blackbox_pool_message_flow_model();
        let graph = lower(&model);
        assert_eq!(graph.edges.len(), 2, "시퀀스 흐름과 메시지 흐름 둘 다 간선으로 남아야 한다");

        let customer_group = graph.groups.iter().position(|g| g.title == "고객").expect("고객 풀 그룹이 있어야 한다");
        let message_edge = graph.edges.iter().find(|e| e.label == "주문").expect("메시지 흐름 간선이 있어야 한다");
        assert_eq!((message_edge.kind, message_edge.tail, message_edge.head), (LineKind::Dashed, Marker::Circle, Marker::Triangle));

        let anchor_node = &graph.nodes[message_edge.from];
        assert_eq!(anchor_node.shape, Shape::Anchor);
        assert_eq!(anchor_node.group, Some(customer_group));
        assert_eq!(graph.nodes[message_edge.to].id, "start");

        let anchors_in_customer_group = graph.nodes.iter().filter(|n| n.shape == Shape::Anchor && n.group == Some(customer_group)).count();
        assert_eq!(anchors_in_customer_group, 1, "블랙박스 풀 안 보이지 않는 노드는 하나뿐이어야 한다");
    }

    #[test]
    fn unresolved_flow_endpoints_are_skipped_without_panicking() {
        let model = Model {
            elements: vec![element("a", ElementKind::Task(TaskKind::None))],
            flows: vec![Flow { id: "f1".into(), source: "a".into(), target: "ghost".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } }],
            ..Model::default()
        };
        let graph = lower(&model);
        assert!(graph.edges.is_empty());
    }

    // ── bizprocess-bpmn: lower_with 정책별 여러 장 ──────────────────────

    fn nested_element(id: &str, kind: ElementKind, parent: Option<&str>, container: Option<&str>) -> Element {
        Element { id: id.into(), name: id.into(), kind, container: container.map(str::to_string), attached_to: None, parent: parent.map(str::to_string) }
    }

    /// L2(a1, 레인 A) → L4(a1.s1, 같은 레인) → L5(a1.s1.d1, 같은 레인) 3단 중첩.
    fn three_level_model() -> Model {
        Model {
            participants: vec![Participant { id: "pool".into(), name: "P".into(), lanes: vec![Lane { id: "laneA".into(), name: "A".into(), sub_lanes: Vec::new() }] }],
            elements: vec![
                nested_element("a1", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a1#start", ElementKind::Event { position: super::super::model::EventPosition::Start, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1#end", ElementKind::Event { position: super::super::model::EventPosition::End, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1.s1", ElementKind::Subprocess, Some("a1"), Some("laneA")),
                nested_element("a1.s1.d1", ElementKind::Task(TaskKind::None), Some("a1.s1"), Some("laneA")),
            ],
            flows: vec![
                Flow { id: "f1".into(), source: "a1#start".into(), target: "a1.s1".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f2".into(), source: "a1.s1".into(), target: "a1#end".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
            ],
            ..Model::default()
        }
    }

    #[test]
    fn depth_0_folds_l2_and_shows_nothing_deeper() {
        let model = three_level_model();
        let diagrams = lower_with(&model, ExpandPolicy::Depth(0));
        assert_eq!(diagrams.len(), 1);
        let graph = &diagrams[0].graph;
        assert!(graph.find("a1").is_some());
        assert_eq!(graph.nodes[graph.find("a1").unwrap()].shape, Shape::Subprocess);
        assert!(graph.find("a1.s1").is_none());
        assert!(graph.find("a1.s1.d1").is_none());
        assert!(graph.groups.iter().all(|g| g.kind == GroupKind::Lane), "깊이 0에서는 상자가 하나도 없어야 한다");
    }

    #[test]
    fn depth_1_expands_l2_into_a_box_and_folds_l4() {
        let model = three_level_model();
        let graph = &lower_with(&model, ExpandPolicy::Depth(1))[0].graph;
        assert!(graph.groups.iter().any(|g| g.kind == GroupKind::Box && g.title == "a1"));
        assert_eq!(graph.nodes[graph.find("a1.s1").unwrap()].shape, Shape::Subprocess);
        assert!(graph.find("a1").is_none(), "펼친 Sub-Process 자신은 노드가 아니라 상자다");
        assert!(graph.find("a1.s1.d1").is_none());
    }

    #[test]
    fn depth_2_and_all_expand_down_to_l5() {
        for policy in [ExpandPolicy::Depth(2), ExpandPolicy::All] {
            let model = three_level_model();
            let graph = &lower_with(&model, policy)[0].graph;
            assert!(graph.groups.iter().any(|g| g.title == "a1"), "{policy:?}");
            assert!(graph.groups.iter().any(|g| g.title == "a1.s1"), "{policy:?}");
            assert!(graph.find("a1.s1.d1").is_some(), "{policy:?}");
        }
    }

    #[test]
    fn lower_of_a_nested_model_is_the_first_chapter_of_depth_zero() {
        let model = three_level_model();
        let a = lower(&model);
        let b = lower_with(&model, ExpandPolicy::Depth(0)).into_iter().next().unwrap().graph;
        let ids = |g: &Graph| g.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b));
        assert_eq!(a.groups.len(), b.groups.len());
    }

    #[test]
    fn l3_group_becomes_a_dashed_box_only_when_every_visible_member_shares_one_lane() {
        let same_lane = Model {
            participants: vec![Participant { id: "pool".into(), name: "P".into(), lanes: vec![Lane { id: "laneA".into(), name: "A".into(), sub_lanes: Vec::new() }] }],
            elements: vec![
                nested_element("a1", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a1.s1", ElementKind::Task(TaskKind::None), Some("a1"), Some("laneA")),
                nested_element("a1.s2", ElementKind::Task(TaskKind::None), Some("a1"), Some("laneA")),
            ],
            groups: vec![Group { id: "a1.g1".into(), name: "그룹".into(), parent: Some("a1".into()), members: vec!["a1.s1".into(), "a1.s2".into()] }],
            ..Model::default()
        };
        let graph = &lower_with(&same_lane, ExpandPolicy::Depth(1))[0].graph;
        let dashed = graph.groups.iter().find(|g| g.id == "a1.g1").expect("가시 멤버가 있으면 그룹이 만들어져야 한다");
        assert_eq!(dashed.line, LineKind::Dashed);
        assert_eq!(graph.nodes[graph.find("a1.s1").unwrap()].group, Some(graph.groups.iter().position(|g| g.id == "a1.g1").unwrap()));

        let mut split_lane = same_lane;
        split_lane.participants[0].lanes.push(Lane { id: "laneB".into(), name: "B".into(), sub_lanes: Vec::new() });
        split_lane.elements[2].container = Some("laneB".into());
        let graph2 = &lower_with(&split_lane, ExpandPolicy::Depth(1))[0].graph;
        assert!(!graph2.groups.iter().any(|g| g.id == "a1.g1"), "멤버 레인이 갈리면 그룹 상자가 생략돼야 한다(6.8)");
        assert!(graph2.find("a1.s1").is_some());
        assert!(graph2.find("a1.s2").is_some());
    }

    #[test]
    fn boundary_event_on_an_expanded_subprocess_host_is_solid_from_the_box_anchor() {
        let mut model = three_level_model();
        model.elements.push(Element {
            id: "a1#error1".into(),
            name: "오류".into(),
            kind: ElementKind::Event { position: super::super::model::EventPosition::Intermediate, trigger: Some(EventTrigger::Error) },
            container: Some("laneA".into()),
            attached_to: Some("a1".into()),
            parent: None,
        });
        let graph = &lower_with(&model, ExpandPolicy::Depth(1))[0].graph;
        let event_index = graph.find("a1#error1").expect("경계 이벤트는 노드로 인턴돼야 한다");
        let box_index = graph.groups.iter().position(|g| g.title == "a1").expect("a1은 펼친 상자여야 한다");
        let boundary_edge = graph.edges.iter().find(|e| e.to == event_index).expect("경계 이벤트로 가는 간선이 있어야 한다");
        assert_eq!(boundary_edge.kind, LineKind::Solid);
        assert_eq!(graph.nodes[boundary_edge.from].shape, Shape::Anchor);
        assert_eq!(graph.nodes[boundary_edge.from].group, Some(box_index));
    }

    /// 3.5절(task 4.2) — L2 3개(하나는 빈 L2) → Process + 자식 있는 L2마다 Activity(2장).
    #[test]
    fn per_activity_makes_one_process_chapter_and_one_activity_chapter_per_non_empty_l2() {
        let model = Model {
            elements: vec![
                nested_element("a1", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a1#start", ElementKind::Event { position: super::super::model::EventPosition::Start, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1#end", ElementKind::Event { position: super::super::model::EventPosition::End, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1.s1", ElementKind::Task(TaskKind::None), Some("a1"), Some("laneA")),
                nested_element("a2", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a2#start", ElementKind::Event { position: super::super::model::EventPosition::Start, trigger: None }, Some("a2"), Some("laneA")),
                nested_element("a2#end", ElementKind::Event { position: super::super::model::EventPosition::End, trigger: None }, Some("a2"), Some("laneA")),
                nested_element("a3", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a3#start", ElementKind::Event { position: super::super::model::EventPosition::Start, trigger: None }, Some("a3"), Some("laneA")),
                nested_element("a3#end", ElementKind::Event { position: super::super::model::EventPosition::End, trigger: None }, Some("a3"), Some("laneA")),
                nested_element("a3.s1", ElementKind::Subprocess, Some("a3"), Some("laneA")),
                nested_element("a3.s1.d1", ElementKind::Task(TaskKind::None), Some("a3.s1"), Some("laneA")),
            ],
            participants: vec![Participant { id: "pool".into(), name: "P".into(), lanes: vec![Lane { id: "laneA".into(), name: "A".into(), sub_lanes: Vec::new() }] }],
            title: "제목".into(),
            ..Model::default()
        };
        let diagrams = lower_with(&model, ExpandPolicy::PerActivity);
        assert_eq!(diagrams.len(), 3, "빈 a2는 자기 Activity 장을 만들지 않는다");
        assert_eq!(diagrams[0].kind, DiagramKind::Process);
        assert_eq!(diagrams[0].graph.title, "제목");
        assert_eq!(diagrams[1].kind, DiagramKind::Activity("a1".into()));
        assert_eq!(diagrams[2].kind, DiagramKind::Activity("a3".into()));
        assert!(diagrams[1].graph.title.is_empty());
        // Activity 장의 최상위: L2 시작·L4·L2 종료가 보이고, L4 Sub-Process(a3.s1)는 펼친 상자다(4.6).
        let a3_activity = &diagrams[2].graph;
        assert!(a3_activity.find("a3#start").is_some());
        assert!(a3_activity.find("a3#end").is_some());
        assert!(a3_activity.groups.iter().any(|g| g.title == "a3.s1"), "자손 전부 펼침이므로 a3.s1도 상자여야 한다");
        assert!(a3_activity.find("a3.s1.d1").is_some());
    }

    /// L4 안에서 참여자가 바뀌면 그 L4는 접히고 별도 `Step` 장이 재귀로 뒤에 붙는다(6.5).
    #[test]
    fn an_internal_participant_transition_folds_its_l4_and_appends_a_step_chapter() {
        let model = Model {
            participants: vec![Participant {
                id: "pool".into(),
                name: "P".into(),
                lanes: vec![Lane { id: "laneA".into(), name: "A".into(), sub_lanes: Vec::new() }, Lane { id: "laneB".into(), name: "B".into(), sub_lanes: Vec::new() }],
            }],
            elements: vec![
                nested_element("a1", ElementKind::Subprocess, None, Some("laneA")),
                nested_element("a1#start", ElementKind::Event { position: super::super::model::EventPosition::Start, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1#end", ElementKind::Event { position: super::super::model::EventPosition::End, trigger: None }, Some("a1"), Some("laneA")),
                nested_element("a1.s1", ElementKind::Subprocess, Some("a1"), Some("laneA")),
                nested_element("a1.s1.d1", ElementKind::Task(TaskKind::None), Some("a1.s1"), Some("laneA")),
                nested_element("a1.s1.d2", ElementKind::Task(TaskKind::None), Some("a1.s1"), Some("laneB")),
            ],
            ..Model::default()
        };
        let diagrams = lower_with(&model, ExpandPolicy::PerActivity);
        assert_eq!(diagrams.len(), 3);
        assert_eq!(diagrams[1].kind, DiagramKind::Activity("a1".into()));
        assert_eq!(diagrams[2].kind, DiagramKind::Step("a1.s1".into()));
        let activity = &diagrams[1].graph;
        assert_eq!(activity.nodes[activity.find("a1.s1").unwrap()].shape, Shape::Subprocess, "전환이 있는 L4는 접혀야 한다");
        assert!(activity.find("a1.s1.d1").is_none(), "접힌 L4 안은 Activity 장에 보이지 않는다");
        let step = &diagrams[2].graph;
        assert!(step.find("a1.s1.d1").is_some());
        assert!(step.find("a1.s1.d2").is_some());
        assert!(step.title.is_empty());
    }

    #[test]
    fn diagram_kind_caption_strings_match_the_three_shapes() {
        assert_eq!(DiagramKind::Process.caption_kind(), "process");
        assert_eq!(DiagramKind::Activity("이름".into()).caption_kind(), "activity: 이름");
        assert_eq!(DiagramKind::Step("이름".into()).caption_kind(), "step: 이름");
    }
}
