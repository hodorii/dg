//! 검증된 `Model`을 기존 `ir::Graph` 어휘로 옮긴다. BPMN 의미는 여기서 끝난다.

use super::model::{ElementKind, FlowKind, Lane, Model, Orientation};
use super::vocabulary::{CALL_ACTIVITY_LABEL, DATA_OBJECT_LABEL};
use crate::diagram::ir::{Direction, Edge, Graph, LineKind, Marker, Shape};
use std::collections::HashMap;

/// 검증된 모델 전제. 해석 안 되는 참조는 건너뛴다(패닉 없음).
pub fn lower(model: &Model) -> Graph {
    let mut graph = Graph { direction: Some(orientation_to_direction(model.orientation)), title: model.title.clone(), ..Graph::default() };

    let group_of = add_pools_and_lanes(&mut graph, model);
    add_elements(&mut graph, model, &group_of);
    add_flows(&mut graph, model, &group_of);
    add_placeholders_for_empty_groups(&mut graph);
    add_boundary_edges(&mut graph, model);

    graph
}

fn orientation_to_direction(orientation: Orientation) -> Direction {
    match orientation {
        Orientation::Horizontal => Direction::LeftRight,
        Orientation::Vertical => Direction::TopDown,
    }
}

/// 풀·(중첩) 레인을 선언 순서대로 레인 그룹으로 만든다. 돌려주는 표는 모델 id(참여자·레인) →
/// 그래프 그룹 인덱스.
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

fn add_elements(graph: &mut Graph, model: &Model, group_of: &HashMap<String, usize>) {
    for element in &model.elements {
        // 경계 이벤트의 소속이 비어 있으면 호스트의 그룹을 따른다.
        let container = element.container.as_deref().or_else(|| {
            let host = model.element(element.attached_to.as_deref()?)?;
            host.container.as_deref()
        });
        let group = container.and_then(|id| group_of.get(id)).copied();
        let index = graph.intern(&element.id, &element.name, shape_of(element.kind), group);
        graph.set_label(index, &section_text(element.kind, &element.name));
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

/// 끝을 노드에서 못 찾으면(참여자 자신 — 요소가 아니다) 메시지 흐름이고 최상위 참여자일 때만 그
/// 참여자 그룹의 닻(`Graph::group_anchor`)으로 대신 해석한다. 그 외(모르는 id 등)는 건너뛴다.
fn resolve_endpoint(graph: &mut Graph, model: &Model, group_of: &HashMap<String, usize>, kind: FlowKind, id: &str) -> Option<usize> {
    if let Some(index) = graph.find(id) {
        return Some(index);
    }
    if matches!(kind, FlowKind::Message) && model.participant_index(id).is_some() {
        return Some(graph.group_anchor(group_of[id]));
    }
    None
}

fn add_flows(graph: &mut Graph, model: &Model, group_of: &HashMap<String, usize>) {
    for flow in &model.flows {
        let (Some(from), Some(to)) = (resolve_endpoint(graph, model, group_of, flow.kind, &flow.source), resolve_endpoint(graph, model, group_of, flow.kind, &flow.target)) else { continue };
        let (kind, tail, head) = line_for(flow.kind);
        graph.add_edge(Edge { from, to, label: flow.label.clone(), kind, tail, head, ..Edge::default() });
    }
}

/// 경계 이벤트마다 호스트 → 이벤트로 표식·라벨 없는 점선 간선 하나(v1 근사).
fn add_boundary_edges(graph: &mut Graph, model: &Model) {
    for element in &model.elements {
        let Some(host_id) = &element.attached_to else { continue };
        let (Some(from), Some(to)) = (graph.find(host_id), graph.find(&element.id)) else { continue };
        graph.add_edge(Edge { from, to, kind: LineKind::Dashed, ..Edge::default() });
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
        FlowKind::Message => (LineKind::Dashed, Marker::Circle, Marker::OpenArrow),
        FlowKind::Association => (LineKind::Dashed, Marker::None, Marker::None),
        FlowKind::DataAssociation => (LineKind::Dashed, Marker::None, Marker::OpenArrow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{Element, EventPosition, EventTrigger, Flow, GatewayKind, Participant, TaskKind};
    use crate::diagram::ir::GroupKind;

    fn element(id: &str, kind: ElementKind) -> Element {
        Element { id: id.into(), name: id.into(), kind, container: None, attached_to: None }
    }

    #[test]
    fn node_kinds_map_to_shapes_and_sections_per_the_table() {
        let model = Model {
            elements: vec![
                Element { id: "e1".into(), name: "타이머".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: None },
                Element { id: "e2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: None, attached_to: None },
                Element { id: "e3".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: None },
                Element { id: "t1".into(), name: "검토".into(), kind: ElementKind::Task(TaskKind::User), container: None, attached_to: None },
                Element { id: "t2".into(), name: "처리".into(), kind: ElementKind::Task(TaskKind::None), container: None, attached_to: None },
                element("sp", ElementKind::Subprocess),
                element("ca", ElementKind::CallActivity),
                Element { id: "g1".into(), name: "재고 있음?".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: None, attached_to: None },
                Element { id: "g2".into(), name: "".into(), kind: ElementKind::Gateway(GatewayKind::Parallel), container: None, attached_to: None },
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
        assert_eq!((graph.edges[2].kind, graph.edges[2].tail, graph.edges[2].head), (LineKind::Dashed, Marker::Circle, Marker::OpenArrow));
        assert_eq!((graph.edges[3].kind, graph.edges[3].tail, graph.edges[3].head), (LineKind::Dashed, Marker::None, Marker::None));
        assert_eq!((graph.edges[4].kind, graph.edges[4].tail, graph.edges[4].head), (LineKind::Dashed, Marker::None, Marker::OpenArrow));
    }

    #[test]
    fn boundary_event_gets_an_extra_unmarked_dashed_edge_from_host_and_shares_its_group() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![
                Element { id: "host".into(), name: "작업".into(), kind: ElementKind::Task(TaskKind::None), container: Some("p1".into()), attached_to: None },
                Element { id: "boundary".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: None, attached_to: Some("host".into()) },
            ],
            ..Model::default()
        };
        let graph = lower(&model);
        let host_group = graph.nodes[graph.find("host").unwrap()].group;
        let boundary_group = graph.nodes[graph.find("boundary").unwrap()].group;
        assert_eq!(host_group, boundary_group);
        let extra_edges: Vec<_> = graph.edges.iter().filter(|e| e.from == graph.find("host").unwrap() && e.to == graph.find("boundary").unwrap()).collect();
        assert_eq!(extra_edges.len(), 1);
        assert_eq!(extra_edges[0].kind, LineKind::Dashed);
        assert_eq!(extra_edges[0].tail, Marker::None);
        assert_eq!(extra_edges[0].head, Marker::None);
        assert!(extra_edges[0].label.is_empty());
    }

    #[test]
    fn empty_pool_gets_one_anchor_placeholder_while_populated_pool_gets_none() {
        let model = Model {
            participants: vec![Participant { id: "empty".into(), name: "빈 풀".into(), lanes: Vec::new() }, Participant { id: "full".into(), name: "찬 풀".into(), lanes: Vec::new() }],
            elements: vec![Element { id: "a".into(), name: "a".into(), kind: ElementKind::Task(TaskKind::None), container: Some("full".into()), attached_to: None }],
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
                Element { id: "start".into(), name: "start".into(), kind: ElementKind::Task(TaskKind::None), container: Some("sales".into()), attached_to: None },
                Element { id: "review".into(), name: "review".into(), kind: ElementKind::Task(TaskKind::None), container: Some("sales".into()), attached_to: None },
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
        assert_eq!((message_edge.kind, message_edge.tail, message_edge.head), (LineKind::Dashed, Marker::Circle, Marker::OpenArrow));

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
}
