//! BPMN 구조 규칙 검증 — 참조·중복·풀 경계·경계 이벤트·중첩(Sub-Process `parent`)·그룹.
//!
//! 규칙 순서(앞 규칙이 실패한 참조는 뒤 규칙에서 건너뜀): 중복 id(그룹 id 포함) → 참조 해석
//! (`container`·`attached_to`·`parent`·그룹 `members`·`source`·`target`) → 시퀀스 흐름(양끝 flow
//! node·같은 풀·같은 `parent`) → 메시지 흐름(양끝 풀이 둘 다 있고 서로 다름) → 경계 이벤트(중간
//! 이벤트·호스트가 활동·소속이 없거나 호스트와 같음·`parent`가 호스트와 같음) → 중첩(`parent`가
//! `Subprocess`·사이클 없음) → 그룹(멤버가 그룹과 같은 `parent`·한 그룹에만 소속).

use super::model::{ElementKind, EventPosition, FlowKind, Model};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    DuplicateId(String),
    /// owner(흐름·노드·그룹 id)가 가리킨 reference가 없다.
    UnknownReference { owner: String, reference: String },
    SequenceFlowCrossesParticipants(String),
    SequenceFlowEndsAtNonFlowNode(String),
    MessageFlowWithinParticipant(String),
    BoundaryEventNotIntermediate(String),
    BoundaryHostNotActivity(String),
    BoundaryContainerMismatch(String),
    /// 요소 id — `parent`가 가리키는 요소가 `Subprocess`가 아니다.
    ParentNotSubprocess(String),
    /// 요소 id — `parent` 사슬을 따라가면 자기 자신으로 되돌아온다.
    ParentCycle(String),
    /// 시퀀스 흐름 id — 양끝의 `parent`가 다르다(Sub-Process 경계를 넘어간다).
    SequenceFlowCrossesSubprocess(String),
    /// 경계 이벤트 id — `parent`가 호스트의 `parent`와 다르다.
    BoundaryParentMismatch(String),
    /// 그룹 id — 멤버의 `parent`가 그룹의 `parent`와 다르다.
    GroupMemberOutsideParent(String),
    /// 요소 id — 두 그룹의 `members`에 함께 들어 있다.
    GroupMembersOverlap(String),
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
            ModelError::ParentNotSubprocess(id) => write!(f, "ParentNotSubprocess({id})"),
            ModelError::ParentCycle(id) => write!(f, "ParentCycle({id})"),
            ModelError::SequenceFlowCrossesSubprocess(id) => write!(f, "SequenceFlowCrossesSubprocess({id})"),
            ModelError::BoundaryParentMismatch(id) => write!(f, "BoundaryParentMismatch({id})"),
            ModelError::GroupMemberOutsideParent(id) => write!(f, "GroupMemberOutsideParent({id})"),
            ModelError::GroupMembersOverlap(id) => write!(f, "GroupMembersOverlap({id})"),
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
    let mut broken_parent: HashSet<&str> = HashSet::new();
    let mut broken_flow: HashSet<&str> = HashSet::new();
    let mut broken_group: HashSet<&str> = HashSet::new();

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
        if let Some(parent) = &element.parent
            && !element_ids.contains(parent.as_str())
        {
            errors.push(ModelError::UnknownReference { owner: element.id.clone(), reference: parent.clone() });
            broken_parent.insert(&element.id);
        }
    }
    for group in &model.groups {
        for member in &group.members {
            if !element_ids.contains(member.as_str()) {
                errors.push(ModelError::UnknownReference { owner: group.id.clone(), reference: member.clone() });
                broken_group.insert(&group.id);
            }
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

    check_sequence_flows(model, &broken_flow, &broken_container, &broken_parent, &mut errors);
    check_message_flows(model, &broken_flow, &broken_container, &mut errors);
    check_boundary_events(model, &broken_attach, &broken_container, &broken_parent, &mut errors);
    check_parent_rules(model, &broken_parent, &mut errors);
    check_groups(model, &broken_group, &mut errors);

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
    for group in &model.groups {
        *counts.entry(group.id.as_str()).or_default() += 1;
    }
    let mut duplicates: Vec<&str> = counts.into_iter().filter(|(_, count)| *count > 1).map(|(id, _)| id).collect();
    duplicates.sort_unstable();
    errors.extend(duplicates.into_iter().map(|id| ModelError::DuplicateId(id.to_string())));
}

/// `parent`가 `Subprocess`를 가리키는지, 사슬이 사이클 없이 끝나는지. 참조가 이미 깨졌으면
/// 건너뛴다(앞 규칙에서 이미 보고됨).
fn check_parent_rules(model: &Model, broken_parent: &HashSet<&str>, errors: &mut Vec<ModelError>) {
    for element in &model.elements {
        if broken_parent.contains(element.id.as_str()) {
            continue;
        }
        if let Some(parent_id) = &element.parent {
            let parent = model.element(parent_id).expect("참조 해석 검사를 통과했다");
            if !matches!(parent.kind, ElementKind::Subprocess) {
                errors.push(ModelError::ParentNotSubprocess(element.id.clone()));
            }
        }
        if has_parent_cycle(model, &element.id) {
            errors.push(ModelError::ParentCycle(element.id.clone()));
        }
    }
}

/// `element_id`에서 시작해 `parent` 사슬을 올라가다 자기 자신을 다시 만나면 참(상한 64에서 끊는다
/// — `validate` 통과 전이라 끊긴 사슬·모르는 참조가 섞여 있을 수 있다).
fn has_parent_cycle(model: &Model, element_id: &str) -> bool {
    let mut cursor = model.element(element_id).and_then(|e| e.parent.clone());
    for _ in 0..64 {
        match cursor {
            Some(id) if id == element_id => return true,
            Some(id) => cursor = model.element(&id).and_then(|e| e.parent.clone()),
            None => return false,
        }
    }
    false
}

/// 그룹 멤버가 그룹과 같은 `parent`인지, 한 그룹에만 속하는지. 멤버 참조가 이미 깨졌으면 그
/// 그룹은 건너뛴다(앞 규칙에서 이미 보고됨).
fn check_groups(model: &Model, broken_group: &HashSet<&str>, errors: &mut Vec<ModelError>) {
    let mut owner_of: HashMap<&str, &str> = HashMap::new();
    for group in &model.groups {
        if broken_group.contains(group.id.as_str()) {
            continue;
        }
        for member_id in &group.members {
            let member = model.element(member_id).expect("참조 해석 검사를 통과했다");
            if member.parent.as_deref() != group.parent.as_deref() {
                errors.push(ModelError::GroupMemberOutsideParent(group.id.clone()));
            }
            if let Some(&other_owner) = owner_of.get(member_id.as_str())
                && other_owner != group.id
            {
                errors.push(ModelError::GroupMembersOverlap(member_id.clone()));
            } else {
                owner_of.insert(member_id.as_str(), group.id.as_str());
            }
        }
    }
}

/// 안에서 참여자(container)가 바뀌는 `Subprocess` 요소 id들(자손 중 `container`가 자기와 다른
/// 것이 있음). 빈 `Vec` = 어디든 펼칠 수 있음. 모델은 유효(검증 통과)하다고 전제 — 사이클·모르는
/// 참조는 상한 4096에서 끊어 패닉 없이 건너뛴다.
pub fn participant_transitions(model: &Model) -> Vec<String> {
    let mut transitioning = Vec::new();
    for element in &model.elements {
        if !matches!(element.kind, ElementKind::Subprocess) {
            continue;
        }
        let mut stack: Vec<&str> = model.children_of(&element.id).map(|e| e.id.as_str()).collect();
        let mut seen: HashSet<&str> = HashSet::new();
        let mut transitions = false;
        while let Some(id) = stack.pop() {
            if !seen.insert(id) || seen.len() > 4096 {
                continue;
            }
            let Some(descendant) = model.element(id) else { continue };
            if descendant.container != element.container {
                transitions = true;
            }
            stack.extend(model.children_of(id).map(|e| e.id.as_str()));
        }
        if transitions {
            transitioning.push(element.id.clone());
        }
    }
    transitioning
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
    if model.participant_index(element_id).is_none() {
        let element = model.element(element_id)?;
        if broken_container.contains(element.id.as_str()) {
            return None;
        }
    }
    Some(model.participant_of_endpoint(element_id))
}

fn check_sequence_flows(model: &Model, broken_flow: &HashSet<&str>, broken_container: &HashSet<&str>, broken_parent: &HashSet<&str>, errors: &mut Vec<ModelError>) {
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
        if !broken_parent.contains(flow.source.as_str()) && !broken_parent.contains(flow.target.as_str()) && source.parent != target.parent {
            errors.push(ModelError::SequenceFlowCrossesSubprocess(flow.id.clone()));
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

fn check_boundary_events(model: &Model, broken_attach: &HashSet<&str>, broken_container: &HashSet<&str>, broken_parent: &HashSet<&str>, errors: &mut Vec<ModelError>) {
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
        if !broken_container.contains(element.id.as_str())
            && let Some(container) = &element.container
            && host.container.as_deref() != Some(container.as_str())
        {
            errors.push(ModelError::BoundaryContainerMismatch(element.id.clone()));
        }
        if !broken_parent.contains(element.id.as_str()) && element.parent != host.parent {
            errors.push(ModelError::BoundaryParentMismatch(element.id.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{Element, Flow, GatewayKind, Lane, Participant, TaskKind};

    fn flow_node(id: &str, container: Option<&str>) -> Element {
        Element { id: id.into(), name: id.into(), kind: ElementKind::Task(TaskKind::None), container: container.map(str::to_string), attached_to: None, parent: None }
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
        let not_intermediate = Element { id: "e1".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: None, attached_to: Some("host".into()), parent: None };
        let host = flow_node("host", None);
        let model = Model { elements: vec![host, not_intermediate], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryEventNotIntermediate("e1".into())]));

        let mut gateway_host = flow_node("g", None);
        gateway_host.kind = ElementKind::Gateway(GatewayKind::Exclusive);
        let boundary = Element { id: "e2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: None }, container: None, attached_to: Some("g".into()), parent: None };
        let model = Model { elements: vec![gateway_host, boundary], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryHostNotActivity("e2".into())]));

        let host_in_pool = flow_node("host2", Some("p1"));
        let mismatched = Element { id: "e3".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: None }, container: Some("p2".into()), attached_to: Some("host2".into()), parent: None };
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

    // --- bizprocess-bpmn: 중첩(parent)·그룹 규칙, 참여자 전환 조회 ---

    fn subprocess(id: &str, parent: Option<&str>) -> Element {
        Element { id: id.into(), name: id.into(), kind: ElementKind::Subprocess, container: None, attached_to: None, parent: parent.map(str::to_string) }
    }

    fn child_of(id: &str, parent: &str) -> Element {
        Element { id: id.into(), name: id.into(), kind: ElementKind::Task(TaskKind::None), container: None, attached_to: None, parent: Some(parent.into()) }
    }

    #[test]
    fn parent_pointing_at_a_non_subprocess_element_is_rejected() {
        let model = Model { elements: vec![flow_node("host", None), child_of("a", "host")], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::ParentNotSubprocess("a".into())]));
    }

    #[test]
    fn a_parent_chain_that_loops_back_to_itself_is_a_cycle() {
        let model = Model { elements: vec![subprocess("a", Some("a"))], ..Model::default() };
        assert_eq!(validate(&model), Err(vec![ModelError::ParentCycle("a".into())]));
    }

    #[test]
    fn sequence_flow_between_elements_with_different_parents_crosses_a_subprocess_boundary() {
        let model = Model {
            elements: vec![subprocess("sp1", None), subprocess("sp2", None), child_of("a", "sp1"), child_of("b", "sp2")],
            flows: vec![sequence("f1", "a", "b")],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::SequenceFlowCrossesSubprocess("f1".into())]));
    }

    #[test]
    fn boundary_event_parent_must_match_its_host_parent() {
        let model = Model {
            elements: vec![
                subprocess("sp1", None),
                subprocess("sp2", None),
                child_of("host", "sp1"),
                Element { id: "boundary".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: None }, container: None, attached_to: Some("host".into()), parent: Some("sp2".into()) },
            ],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::BoundaryParentMismatch("boundary".into())]));
    }

    #[test]
    fn group_member_outside_the_group_parent_is_rejected() {
        let model = Model {
            elements: vec![subprocess("sp1", None), child_of("a", "sp1")],
            groups: vec![super::super::model::Group { id: "g1".into(), name: "G1".into(), parent: None, members: vec!["a".into()] }],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::GroupMemberOutsideParent("g1".into())]));
    }

    #[test]
    fn an_element_belonging_to_two_groups_at_once_is_rejected() {
        let model = Model {
            elements: vec![flow_node("a", None)],
            groups: vec![
                super::super::model::Group { id: "g1".into(), name: "G1".into(), parent: None, members: vec!["a".into()] },
                super::super::model::Group { id: "g2".into(), name: "G2".into(), parent: None, members: vec!["a".into()] },
            ],
            ..Model::default()
        };
        assert_eq!(validate(&model), Err(vec![ModelError::GroupMembersOverlap("a".into())]));
    }

    #[test]
    fn existing_parentless_groupless_models_pass_every_new_rule() {
        assert_eq!(validate(&blackbox_pool_message_flow_model()), Ok(()));
    }

    #[test]
    fn participant_transitions_is_empty_when_no_subprocess_contains_a_different_container() {
        let model = Model {
            elements: vec![
                Element { id: "a1".into(), name: "a1".into(), kind: ElementKind::Subprocess, container: Some("laneA".into()), attached_to: None, parent: None },
                Element { id: "a1.s1".into(), name: "a1.s1".into(), kind: ElementKind::Task(TaskKind::None), container: Some("laneA".into()), attached_to: None, parent: Some("a1".into()) },
            ],
            ..Model::default()
        };
        assert_eq!(participant_transitions(&model), Vec::<String>::new());
    }

    #[test]
    fn participant_transitions_flags_a_subprocess_whose_descendant_container_differs() {
        let model = Model {
            elements: vec![
                Element { id: "a1".into(), name: "a1".into(), kind: ElementKind::Subprocess, container: Some("laneA".into()), attached_to: None, parent: None },
                Element { id: "a1.s1".into(), name: "a1.s1".into(), kind: ElementKind::Task(TaskKind::None), container: Some("laneB".into()), attached_to: None, parent: Some("a1".into()) },
            ],
            ..Model::default()
        };
        assert_eq!(participant_transitions(&model), vec!["a1".to_string()]);
    }

    #[test]
    fn participant_transitions_flags_an_inner_subprocess_the_same_way_as_an_outer_one() {
        // 구조적으로 L2·L4를 구분하지 않는 조회 자체의 동작 확인 — "L4 안 전환"은 조회 대상
        // Subprocess가 문서 트리 몇 층에 있든 같은 규칙(자손 container 불일치)으로 잡힌다.
        let model = Model {
            elements: vec![
                Element { id: "a1.s1".into(), name: "a1.s1".into(), kind: ElementKind::Subprocess, container: Some("laneA".into()), attached_to: None, parent: None },
                Element { id: "a1.s1.d1".into(), name: "a1.s1.d1".into(), kind: ElementKind::Task(TaskKind::None), container: Some("laneB".into()), attached_to: None, parent: Some("a1.s1".into()) },
            ],
            ..Model::default()
        };
        assert_eq!(participant_transitions(&model), vec!["a1.s1".to_string()]);
    }
}
