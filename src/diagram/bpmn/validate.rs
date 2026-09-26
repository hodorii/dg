//! BPMN 구조 규칙 검증 — 참조·중복·풀 경계·경계 이벤트.
//!
//! 규칙 순서(앞 규칙이 실패한 참조는 뒤 규칙에서 건너뜀): 중복 id → 참조 해석(`container`·
//! `attached_to`·`source`·`target`) → 시퀀스 흐름(양끝 flow node·같은 풀) → 메시지 흐름(양끝 풀이
//! 둘 다 있고 서로 다름) → 경계 이벤트(중간 이벤트·호스트가 활동·소속이 없거나 호스트와 같음).

use super::model::{ElementKind, EventPosition, FlowKind, Model};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    DuplicateId(String),
    /// owner(흐름·노드 id)가 가리킨 reference가 없다.
    UnknownReference { owner: String, reference: String },
    SequenceFlowCrossesParticipants(String),
    SequenceFlowEndsAtNonFlowNode(String),
    MessageFlowWithinParticipant(String),
    BoundaryEventNotIntermediate(String),
    BoundaryHostNotActivity(String),
    BoundaryContainerMismatch(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::DuplicateId(id) => write!(f, "DuplicateId({id})"),
            ModelError::UnknownReference { owner, reference } => write!(f, "UnknownReference({owner} -> {reference})"),
            ModelError::SequenceFlowCrossesParticipants(id) => write!(f, "SequenceFlowCrossesParticipants({id})"),
            ModelError::SequenceFlowEndsAtNonFlowNode(id) => write!(f, "SequenceFlowEndsAtNonFlowNode({id})"),
            ModelError::MessageFlowWithinParticipant(id) => write!(f, "MessageFlowWithinParticipant({id})"),
            ModelError::BoundaryEventNotIntermediate(id) => write!(f, "BoundaryEventNotIntermediate({id})"),
            ModelError::BoundaryHostNotActivity(id) => write!(f, "BoundaryHostNotActivity({id})"),
            ModelError::BoundaryContainerMismatch(id) => write!(f, "BoundaryContainerMismatch({id})"),
        }
    }
}

/// 발견한 위반 전부. 노드 없는 모델은 `Ok`.
pub fn validate(model: &Model) -> Result<(), Vec<ModelError>> {
    if model.elements.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();

    check_duplicate_ids(model, &mut errors);

    let element_ids: HashSet<&str> = model.elements.iter().map(|e| e.id.as_str()).collect();
    let container_ids: HashSet<&str> = all_container_ids(model);
    let mut broken_container: HashSet<&str> = HashSet::new();
    let mut broken_attach: HashSet<&str> = HashSet::new();
    let mut broken_flow: HashSet<&str> = HashSet::new();

    for element in &model.elements {
        if let Some(container) = &element.container
            && !container_ids.contains(container.as_str())
        {
            errors.push(ModelError::UnknownReference { owner: element.id.clone(), reference: container.clone() });
            broken_container.insert(&element.id);
        }
        if let Some(host) = &element.attached_to
            && !element_ids.contains(host.as_str())
        {
            errors.push(ModelError::UnknownReference { owner: element.id.clone(), reference: host.clone() });
            broken_attach.insert(&element.id);
        }
    }
    for flow in &model.flows {
        let mut ok = true;
        let is_valid_endpoint = |id: &str| element_ids.contains(id) || (matches!(flow.kind, FlowKind::Message) && model.participant_index(id).is_some());
        if !is_valid_endpoint(flow.source.as_str()) {
            errors.push(ModelError::UnknownReference { owner: flow.id.clone(), reference: flow.source.clone() });
            ok = false;
        }
        if !is_valid_endpoint(flow.target.as_str()) {
            errors.push(ModelError::UnknownReference { owner: flow.id.clone(), reference: flow.target.clone() });
            ok = false;
        }
        if !ok {
            broken_flow.insert(&flow.id);
        }
    }

    check_sequence_flows(model, &broken_flow, &broken_container, &mut errors);
    check_message_flows(model, &broken_flow, &broken_container, &mut errors);
    check_boundary_events(model, &broken_attach, &broken_container, &mut errors);

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn check_duplicate_ids(model: &Model, errors: &mut Vec<ModelError>) {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for participant in &model.participants {
        *counts.entry(participant.id.as_str()).or_default() += 1;
        count_lane_ids(&participant.lanes, &mut counts);
    }
    for element in &model.elements {
        *counts.entry(element.id.as_str()).or_default() += 1;
    }
    for flow in &model.flows {
        *counts.entry(flow.id.as_str()).or_default() += 1;
    }
    let mut duplicates: Vec<&str> = counts.into_iter().filter(|(_, count)| *count > 1).map(|(id, _)| id).collect();
    duplicates.sort_unstable();
    errors.extend(duplicates.into_iter().map(|id| ModelError::DuplicateId(id.to_string())));
}

fn count_lane_ids<'a>(lanes: &'a [super::model::Lane], counts: &mut HashMap<&'a str, usize>) {
    for lane in lanes {
        *counts.entry(lane.id.as_str()).or_default() += 1;
        count_lane_ids(&lane.sub_lanes, counts);
    }
}

fn all_container_ids(model: &Model) -> HashSet<&str> {
    let mut ids = HashSet::new();
    for participant in &model.participants {
        ids.insert(participant.id.as_str());
        collect_lane_ids(&participant.lanes, &mut ids);
    }
    ids
}

fn collect_lane_ids<'a>(lanes: &'a [super::model::Lane], ids: &mut HashSet<&'a str>) {
    for lane in lanes {
        ids.insert(lane.id.as_str());
        collect_lane_ids(&lane.sub_lanes, ids);
    }
}

/// 흐름 끝의 소속 참여자 인덱스. 끝이 최상위 참여자 id면 그 참여자 자신. 요소를 못 찾거나
/// `container` 참조가 이미 깨졌으면 `None`(앞 규칙에서 이미 보고된 참조라 뒤 규칙은 건너뛴다).
fn pool_of(model: &Model, broken_container: &HashSet<&str>, element_id: &str) -> Option<Option<usize>> {
    if let Some(i) = model.participant_index(element_id) {
        return Some(Some(i));
    }
    let element = model.element(element_id)?;
    if broken_container.contains(element.id.as_str()) {
        return None;
    }
    Some(element.container.as_deref().and_then(|c| model.participant_of_container(c)))
}

fn check_sequence_flows(model: &Model, broken_flow: &HashSet<&str>, broken_container: &HashSet<&str>, errors: &mut Vec<ModelError>) {
    for flow in &model.flows {
        if broken_flow.contains(flow.id.as_str()) || !matches!(flow.kind, FlowKind::Sequence { .. }) {
            continue;
        }
        let source = model.element(&flow.source).expect("참조 해석 검사를 통과했다");
        let target = model.element(&flow.target).expect("참조 해석 검사를 통과했다");
        if !source.kind.is_flow_node() || !target.kind.is_flow_node() {
            errors.push(ModelError::SequenceFlowEndsAtNonFlowNode(flow.id.clone()));
            continue;
        }
        let (Some(source_pool), Some(target_pool)) = (pool_of(model, broken_container, &flow.source), pool_of(model, broken_container, &flow.target)) else { continue };
        if source_pool != target_pool {
            errors.push(ModelError::SequenceFlowCrossesParticipants(flow.id.clone()));
        }
    }
}

fn check_message_flows(model: &Model, broken_flow: &HashSet<&str>, broken_container: &HashSet<&str>, errors: &mut Vec<ModelError>) {
    for flow in &model.flows {
        if broken_flow.contains(flow.id.as_str()) || !matches!(flow.kind, FlowKind::Message) {
            continue;
        }
        let (Some(source_pool), Some(target_pool)) = (pool_of(model, broken_container, &flow.source), pool_of(model, broken_container, &flow.target)) else { continue };
        let both_in_pools = source_pool.is_some() && target_pool.is_some();
        if !both_in_pools || source_pool == target_pool {
            errors.push(ModelError::MessageFlowWithinParticipant(flow.id.clone()));
        }
    }
}

fn check_boundary_events(model: &Model, broken_attach: &HashSet<&str>, broken_container: &HashSet<&str>, errors: &mut Vec<ModelError>) {
    for element in &model.elements {
        let Some(host_id) = &element.attached_to else { continue };
        if broken_attach.contains(element.id.as_str()) {
            continue;
        }
        let host = model.element(host_id).expect("참조 해석 검사를 통과했다");
        if !matches!(element.kind, ElementKind::Event { position: EventPosition::Intermediate, .. }) {
            errors.push(ModelError::BoundaryEventNotIntermediate(element.id.clone()));
        }
        if !host.kind.is_activity() {
            errors.push(ModelError::BoundaryHostNotActivity(element.id.clone()));
        }
        if broken_container.contains(element.id.as_str()) {
            continue;
        }
        if let Some(container) = &element.container
            && host.container.as_deref() != Some(container.as_str())
        {
            errors.push(ModelError::BoundaryContainerMismatch(element.id.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{Element, Flow, GatewayKind, Lane, Participant, TaskKind};

    fn flow_node(id: &str, container: Option<&str>) -> Element {
        Element { id: id.into(), name: id.into(), kind: ElementKind::Task(TaskKind::None), container: container.map(str::to_string), attached_to: None }
    }

    fn sequence(id: &str, source: &str, target: &str) -> Flow {
        Flow { id: id.into(), source: source.into(), target: target.into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } }
    }

    fn message(id: &str, source: &str, target: &str, label: &str) -> Flow {
        Flow { id: id.into(), source: source.into(), target: target.into(), label: label.into(), kind: FlowKind::Message }
    }

    /// bugfix.md 재현 절차 2 — 블랙박스 풀 `customer`(요소 없음)가 레인 `sales`(참여자 `seller`
    /// 소속) 안 `start`로 메시지를 보낸다.
    fn blackbox_pool_message_flow_model() -> Model {
        Model {
            participants: vec![
                Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() },
                Participant { id: "seller".into(), name: "판매사".into(), lanes: vec![Lane { id: "sales".into(), name: "영업".into(), sub_lanes: Vec::new() }] },
            ],
            elements: vec![flow_node("start", Some("sales")), flow_node("review", Some("sales"))],
            flows: vec![sequence("s1", "start", "review"), message("m1", "customer", "start", "주문")],
            ..Model::default()
        }
    }

    #[test]
    fn model_without_nodes_is_ok() {
        assert_eq!(validate(&Model::default()), Ok(()));
    }

    #[test]
    fn sequence_flow_across_participants_is_rejected() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }, Participant { id: "p2".into(), name: "P2".into(), lanes: Vec::new() }],
            elements: vec![flow_node("a", Some("p1")), flow_node("b", Some("p2"))],
            flows: vec![sequence("f1", "a", "b")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::SequenceFlowCrossesParticipants("f1".into())]));
    }

    #[test]
    fn sequence_flow_across_lanes_of_same_pool_is_ok() {
        let model = Model {
            participants: vec![Participant {
                id: "p1".into(),
                name: "P1".into(),
                lanes: vec![Lane { id: "l1".into(), name: "L1".into(), sub_lanes: Vec::new() }, Lane { id: "l2".into(), name: "L2".into(), sub_lanes: Vec::new() }],
            }],
            elements: vec![flow_node("a", Some("l1")), flow_node("b", Some("l2"))],
            flows: vec![sequence("f1", "a", "b")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Ok(()));
    }

    #[test]
    fn message_flow_within_same_participant_is_rejected() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![flow_node("a", Some("p1")), flow_node("b", Some("p1"))],
            flows: vec![Flow { id: "f1".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Message }],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::MessageFlowWithinParticipant("f1".into())]));
    }

    #[test]
    fn message_flow_with_both_ends_outside_any_pool_is_rejected() {
        let model = Model {
            elements: vec![flow_node("a", None), flow_node("b", None)],
            flows: vec![Flow { id: "f1".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Message }],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::MessageFlowWithinParticipant("f1".into())]));
    }

    #[test]
    fn unknown_reference_is_reported_with_the_dangling_id() {
        let model = Model { elements: vec![flow_node("a", Some("ghost-pool"))], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::UnknownReference { owner: "a".into(), reference: "ghost-pool".into() }]));
    }

    #[test]
    fn duplicate_ids_are_reported() {
        let model = Model { elements: vec![flow_node("dup", None), flow_node("dup", None)], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::DuplicateId("dup".into())]));
    }

    #[test]
    fn sequence_flow_ending_at_data_object_is_rejected() {
        let mut target = flow_node("d", None);
        target.kind = ElementKind::DataObject;
        let model = Model { elements: vec![flow_node("a", None), target], flows: vec![sequence("f1", "a", "d")], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::SequenceFlowEndsAtNonFlowNode("f1".into())]));
    }

    #[test]
    fn boundary_event_must_be_intermediate_attached_to_an_activity_and_match_host_container() {
        let not_intermediate = Element { id: "e1".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: None, attached_to: Some("host".into()) };
        let host = flow_node("host", None);
        let model = Model { elements: vec![host, not_intermediate], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryEventNotIntermediate("e1".into())]));

        let mut gateway_host = flow_node("g", None);
        gateway_host.kind = ElementKind::Gateway(GatewayKind::Exclusive);
        let boundary = Element { id: "e2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: None }, container: None, attached_to: Some("g".into()) };
        let model = Model { elements: vec![gateway_host, boundary], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryHostNotActivity("e2".into())]));

        let host_in_pool = flow_node("host2", Some("p1"));
        let mismatched = Element { id: "e3".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: None }, container: Some("p2".into()), attached_to: Some("host2".into()) };
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }, Participant { id: "p2".into(), name: "P2".into(), lanes: Vec::new() }],
            elements: vec![host_in_pool, mismatched],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryContainerMismatch("e3".into())]));
    }

    // --- bpmn-message-flow-participant-endpoint ---

    #[test]
    fn message_flow_ending_at_a_top_level_participant_id_is_accepted() {
        let model = blackbox_pool_message_flow_model();
        assert_eq!(validate(&model), Ok(()));
    }

    #[test]
    fn message_flow_between_two_top_level_participant_ids_is_accepted() {
        let mut model = blackbox_pool_message_flow_model();
        model.flows.push(message("m2", "customer", "seller", ""));
        assert_eq!(validate(&model), Ok(()));
    }

    #[test]
    fn message_flow_from_a_participant_to_its_own_element_is_rejected_as_within_participant() {
        let model = Model {
            participants: vec![Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() }],
            elements: vec![flow_node("a", Some("customer"))],
            flows: vec![message("m1", "customer", "a", "")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::MessageFlowWithinParticipant("m1".into())]));
    }

    #[test]
    fn sequence_flow_ending_at_a_participant_id_is_still_unknown_reference() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![flow_node("a", Some("p1"))],
            flows: vec![sequence("f1", "p1", "a")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::UnknownReference { owner: "f1".into(), reference: "p1".into() }]));
    }

    #[test]
    fn message_flow_ending_at_a_lane_id_is_still_unknown_reference() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: vec![Lane { id: "l1".into(), name: "L1".into(), sub_lanes: Vec::new() }] }],
            elements: vec![flow_node("a", Some("l1"))],
            flows: vec![message("f1", "l1", "a", "")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::UnknownReference { owner: "f1".into(), reference: "l1".into() }]));
    }

    #[test]
    fn multiple_violations_are_all_reported() {
        let model = Model {
            elements: vec![flow_node("dup", None), flow_node("dup", None)],
            flows: vec![Flow { id: "f1".into(), source: "dup".into(), target: "ghost".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } }],
            ..Model::default()
        };
        let Err(errors) = validate(&model) else { panic!("모델이 거부돼야 한다") };
        assert!(errors.contains(&ModelError::DuplicateId("dup".into())));
        assert!(errors.contains(&ModelError::UnknownReference { owner: "f1".into(), reference: "ghost".into() }));
        assert!(errors.len() >= 2);
    }
}
