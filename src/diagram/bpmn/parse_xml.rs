//! BPMN 2.0.2 Descriptive 하위집합의 의미 트리 → [`Model`].
//!
//! `xml::parse_document`가 만든 요소 트리를 걸으며 협업·참여자·프로세스·레인·흐름 노드·아티팩트·
//! 흐름을 채운다. 검증은 하지 않는다(`render_model`이 `validate`를 부른다). 어휘는
//! `super::vocabulary`의 `from_xml_name`만 호출 — 새 매핑표를 두지 않는다.

use super::model::{Element, ElementKind, EventTrigger, Flow, FlowKind, Lane, Model, Participant};
use super::xml::{self, XmlElement, XmlError};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Xml(XmlError),
    /// 루트 로컬 이름이 `definitions`·`process`·`collaboration`이 아님.
    UnexpectedRoot(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Xml(err) => write!(f, "{err}"),
            ParseError::UnexpectedRoot(name) => write!(f, "루트 요소 {name:?}는 definitions·process·collaboration이 아니다"),
        }
    }
}

/// 트리로 만들지 않는 구획(BPMNDI·vendor 확장).
pub const OPAQUE_ELEMENTS: &[&str] = &["BPMNDiagram", "extensionElements"];

/// 첫 시작 태그만 보고 BPMN XML인지 판정한다(design §Key Decisions 스니핑 규칙). 로컬 이름이
/// `definitions`이면 `xmlns*` 속성 중 하나가 BPMN 네임스페이스를 담거나 `xmlns*` 속성이 하나도
/// 없어야 하고, `process`·`collaboration`이면 무조건 참이다.
pub fn looks_like_bpmn(source: &str) -> bool {
    let Some(tag) = xml::first_start_tag(source) else { return false };
    match tag.name.as_str() {
        "process" | "collaboration" => true,
        "definitions" => {
            let namespace_decls: Vec<&str> = tag.attributes.iter().filter(|(name, _)| name == "xmlns" || name.starts_with("xmlns:")).map(|(_, value)| value.as_str()).collect();
            namespace_decls.is_empty() || namespace_decls.iter().any(|uri| uri.contains("omg.org/spec/BPMN"))
        }
        _ => false,
    }
}

/// 문서 → 검증 전 `Model`. `orientation`은 기본값.
pub fn parse(source: &str) -> Result<Model, ParseError> {
    let root = xml::parse_document(source, OPAQUE_ELEMENTS).map_err(ParseError::Xml)?;
    build_model(&root)
}

fn build_model(root: &XmlElement) -> Result<Model, ParseError> {
    let (definitions_name, collaboration, processes): (Option<&str>, Option<&XmlElement>, Vec<&XmlElement>) = match root.name.as_str() {
        "definitions" => (root.attribute("name"), root.children_named("collaboration").next(), root.children.iter().filter(|c| c.name == "process").collect()),
        "collaboration" => (None, Some(root), Vec::new()),
        "process" => (None, None, vec![root]),
        other => return Err(ParseError::UnexpectedRoot(other.to_string())),
    };

    let mut counter = 0usize;
    let mut participants: Vec<Participant> = Vec::new();
    // processRef 값 → 그 참여자의 인덱스.
    let mut process_ref_to_participant: HashMap<String, usize> = HashMap::new();

    if let Some(collaboration) = collaboration {
        for participant_elem in collaboration.children_named("participant") {
            let id = participant_elem.attribute("id").map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
            let name = trimmed_attribute(participant_elem, "name");
            if let Some(process_ref) = participant_elem.attribute("processRef") {
                process_ref_to_participant.insert(process_ref.to_string(), participants.len());
            }
            participants.push(Participant { id, name, lanes: Vec::new() });
        }
    }

    // 각 프로세스의 소속 참여자(있으면)와 레인 트리·노드→레인 대응표.
    struct ProcessContext<'a> {
        element: &'a XmlElement,
        participant_index: Option<usize>,
    }
    let mut contexts: Vec<ProcessContext> = Vec::new();
    for process in &processes {
        let process_id = process.attribute("id").unwrap_or("");
        if let Some(&index) = process_ref_to_participant.get(process_id) {
            contexts.push(ProcessContext { element: process, participant_index: Some(index) });
            continue;
        }
        let has_lane_set = process.children_named("laneSet").next().is_some();
        if has_lane_set {
            let id = process.attribute("id").map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
            let name = trimmed_attribute(process, "name");
            participants.push(Participant { id, name, lanes: Vec::new() });
            contexts.push(ProcessContext { element: process, participant_index: Some(participants.len() - 1) });
        } else {
            contexts.push(ProcessContext { element: process, participant_index: None });
        }
    }

    let mut elements: Vec<Element> = Vec::new();
    let mut flows: Vec<Flow> = Vec::new();
    let mut default_flow_ids: HashSet<String> = HashSet::new();
    for ctx in &contexts {
        for child in &ctx.element.children {
            if let Some(default_id) = child.attribute("default") {
                default_flow_ids.insert(default_id.to_string());
            }
        }
    }

    for ctx in &contexts {
        let mut node_lane_map: HashMap<String, String> = HashMap::new();
        let lanes: Vec<Lane> = ctx
            .element
            .children_named("laneSet")
            .flat_map(|lane_set| lane_set.children_named("lane"))
            .map(|lane_elem| build_lane(lane_elem, &mut counter, &mut node_lane_map))
            .collect();
        if let Some(index) = ctx.participant_index {
            participants[index].lanes = lanes;
        }
        let container_id: Option<String> = ctx.participant_index.map(|index| participants[index].id.clone());

        for child in &ctx.element.children {
            if child.name == "laneSet" {
                continue;
            }
            let literal_id = child.attribute("id");
            if let Some(kind) = element_kind_of(&child.name, child) {
                let id = literal_id.map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
                let name = element_name(&child.name, child);
                let attached_to = if child.name == "boundaryEvent" { child.attribute("attachedToRef").map(str::to_string) } else { None };
                let container = literal_id.and_then(|lid| node_lane_map.get(lid).cloned()).or_else(|| container_id.clone());
                elements.push(Element { id, name, kind, container, attached_to, parent: None });
            }
            if let Some(activity_id) = literal_id {
                for association in child.children_named("dataInputAssociation") {
                    for source_ref in association.children_named("sourceRef") {
                        let source = source_ref.text.trim();
                        if !source.is_empty() {
                            flows.push(Flow { id: synth_id(&mut counter), source: source.to_string(), target: activity_id.to_string(), label: String::new(), kind: FlowKind::DataAssociation });
                        }
                    }
                }
                for association in child.children_named("dataOutputAssociation") {
                    for target_ref in association.children_named("targetRef") {
                        let target = target_ref.text.trim();
                        if !target.is_empty() {
                            flows.push(Flow { id: synth_id(&mut counter), source: activity_id.to_string(), target: target.to_string(), label: String::new(), kind: FlowKind::DataAssociation });
                        }
                    }
                }
            }
        }

        for flow_elem in ctx.element.children_named("sequenceFlow") {
            let literal_id = flow_elem.attribute("id");
            let id = literal_id.map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
            let is_default = literal_id.is_some_and(|lid| default_flow_ids.contains(lid));
            let source = flow_elem.attribute("sourceRef").unwrap_or("").to_string();
            let target = flow_elem.attribute("targetRef").unwrap_or("").to_string();
            let label = trimmed_attribute(flow_elem, "name");
            flows.push(Flow { id, source, target, label, kind: FlowKind::Sequence { is_default } });
        }
        for assoc_elem in ctx.element.children_named("association") {
            let id = assoc_elem.attribute("id").map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
            let source = assoc_elem.attribute("sourceRef").unwrap_or("").to_string();
            let target = assoc_elem.attribute("targetRef").unwrap_or("").to_string();
            flows.push(Flow { id, source, target, label: String::new(), kind: FlowKind::Association });
        }
    }

    if let Some(collaboration) = collaboration {
        for mf in collaboration.children_named("messageFlow") {
            let id = mf.attribute("id").map(str::to_string).unwrap_or_else(|| synth_id(&mut counter));
            let source = mf.attribute("sourceRef").unwrap_or("").to_string();
            let target = mf.attribute("targetRef").unwrap_or("").to_string();
            let label = trimmed_attribute(mf, "name");
            flows.push(Flow { id, source, target, label, kind: FlowKind::Message });
        }
    }

    // 경계 이벤트 container := 호스트 container(전부 만든 뒤).
    let host_containers: HashMap<String, Option<String>> = elements.iter().map(|e| (e.id.clone(), e.container.clone())).collect();
    for element in &mut elements {
        if let Some(host_id) = element.attached_to.clone()
            && let Some(host_container) = host_containers.get(&host_id)
        {
            element.container = host_container.clone();
        }
    }

    // 문서에 있지만 모델 요소도 참여자도 되지 않은 id(레인·dataInput·property·dataObject 정의 등).
    let mut all_ids: HashSet<String> = HashSet::new();
    collect_ids(root, &mut all_ids);
    let element_ids: HashSet<&str> = elements.iter().map(|e| e.id.as_str()).collect();
    let participant_ids: HashSet<&str> = participants.iter().map(|p| p.id.as_str()).collect();
    let not_a_node_ids: HashSet<&str> = all_ids.iter().map(String::as_str).filter(|id| !element_ids.contains(id) && !participant_ids.contains(id)).collect();
    flows.retain(|flow| !not_a_node_ids.contains(flow.source.as_str()) && !not_a_node_ids.contains(flow.target.as_str()));

    let title = collaboration
        .and_then(|c| c.attribute("name"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| if processes.len() == 1 { processes[0].attribute("name").map(str::trim).filter(|s| !s.is_empty()) } else { None })
        .or_else(|| definitions_name.map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or("")
        .to_string();

    Ok(Model { title, orientation: super::model::Orientation::default(), participants, elements, flows, groups: Vec::new() })
}

fn build_lane(lane_elem: &XmlElement, counter: &mut usize, node_lane_map: &mut HashMap<String, String>) -> Lane {
    let id = lane_elem.attribute("id").map(str::to_string).unwrap_or_else(|| synth_id(counter));
    let name = trimmed_attribute(lane_elem, "name");
    for node_ref in lane_elem.children_named("flowNodeRef") {
        let node_id = node_ref.text.trim();
        if !node_id.is_empty() {
            node_lane_map.insert(node_id.to_string(), id.clone());
        }
    }
    let sub_lanes = lane_elem.children_named("childLaneSet").flat_map(|cls| cls.children_named("lane")).map(|l| build_lane(l, counter, node_lane_map)).collect();
    Lane { id, name, sub_lanes }
}

/// `vocabulary::element_kind_from_xml_name`의 결과에 이벤트 트리거만 얹는다(동작은 이 승격
/// 이전과 완전히 같다 — design §Key Decisions, 7.2).
fn element_kind_of(local_name: &str, elem: &XmlElement) -> Option<ElementKind> {
    let kind = super::vocabulary::element_kind_from_xml_name(local_name)?;
    Some(match kind {
        ElementKind::Event { position, .. } => ElementKind::Event { position, trigger: event_trigger_of(elem) },
        other => other,
    })
}

/// 해석되는 `*EventDefinition` 자식 0개 → `None`, 1개 → 그것, 2개 이상 → `parallelMultiple="true"`면
/// `ParallelMultiple` 아니면 `Multiple`.
fn event_trigger_of(elem: &XmlElement) -> Option<EventTrigger> {
    let triggers: Vec<EventTrigger> = elem.children.iter().filter_map(|c| EventTrigger::from_xml_name(&c.name)).collect();
    match triggers.len() {
        0 => None,
        1 => Some(triggers[0]),
        _ => Some(if elem.attribute("parallelMultiple") == Some("true") { EventTrigger::ParallelMultiple } else { EventTrigger::Multiple }),
    }
}

fn element_name(local_name: &str, elem: &XmlElement) -> String {
    if local_name == "textAnnotation" {
        return elem.child_text("text").unwrap_or("").trim().to_string();
    }
    trimmed_attribute(elem, "name")
}

fn trimmed_attribute(elem: &XmlElement, name: &str) -> String {
    elem.attribute(name).map(str::trim).unwrap_or("").to_string()
}

fn synth_id(counter: &mut usize) -> String {
    let id = format!("@bpmn-xml:{counter}");
    *counter += 1;
    id
}

fn collect_ids(elem: &XmlElement, ids: &mut HashSet<String>) {
    if let Some(id) = elem.attribute("id") {
        ids.insert(id.to_string());
    }
    for child in &elem.children {
        collect_ids(child, ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{EventPosition, GatewayKind, TaskKind};

    // --- looks_like_bpmn ---

    #[test]
    fn bpmn_namespaced_and_bare_definitions_process_and_collaboration_roots_are_recognized() {
        assert!(looks_like_bpmn(r#"<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"></definitions>"#));
        assert!(looks_like_bpmn("<definitions></definitions>"));
        assert!(looks_like_bpmn(r#"<process id="p1"></process>"#));
        assert!(looks_like_bpmn(r#"<collaboration id="c1"></collaboration>"#));
    }

    #[test]
    fn wsdl_html_and_maven_roots_are_rejected() {
        assert!(!looks_like_bpmn(r#"<definitions xmlns="http://schemas.xmlsoap.org/wsdl/"></definitions>"#));
        assert!(!looks_like_bpmn("<html><body></body></html>"));
        assert!(!looks_like_bpmn(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"></project>"#));
    }

    #[test]
    fn yaml_markdown_and_plain_text_are_rejected() {
        assert!(!looks_like_bpmn("process:\n  id: p1\n"));
        assert!(!looks_like_bpmn("## L1 프로세스\n- 값사슬: 판매"));
        assert!(!looks_like_bpmn("임의의 한글 문장입니다"));
        assert!(!looks_like_bpmn(""));
    }

    // --- 구조: 협업·참여자·프로세스·레인·소속·제목 ---

    #[test]
    fn two_participants_with_process_ref_keep_declaration_order_and_names() {
        let model = parse(
            r#"<definitions>
                <collaboration id="c1">
                    <participant id="p1" name="고객" processRef="proc1"/>
                    <participant id="p2" name="판매사" processRef="proc2"/>
                </collaboration>
                <process id="proc1"><startEvent id="s1"/></process>
                <process id="proc2"><startEvent id="s2"/></process>
            </definitions>"#,
        )
        .unwrap();
        assert_eq!(model.participants.iter().map(|p| (p.id.as_str(), p.name.as_str())).collect::<Vec<_>>(), vec![("p1", "고객"), ("p2", "판매사")]);
        assert_eq!(model.element("s1").unwrap().container.as_deref(), Some("p1"));
        assert_eq!(model.element("s2").unwrap().container.as_deref(), Some("p2"));
    }

    #[test]
    fn participant_without_process_ref_is_a_blackbox_pool_with_no_lanes() {
        let model = parse(r#"<definitions><collaboration id="c1"><participant id="p1" name="블랙박스"/></collaboration></definitions>"#).unwrap();
        assert_eq!(model.participants.len(), 1);
        assert!(model.participants[0].lanes.is_empty());
        assert!(model.elements.is_empty());
    }

    #[test]
    fn nested_lanes_place_nodes_in_the_deepest_listing_lane() {
        let model = parse(
            r#"<process id="p1">
                <laneSet>
                    <lane id="l1" name="상위">
                        <flowNodeRef>a</flowNodeRef>
                        <childLaneSet>
                            <lane id="l1-1" name="하위">
                                <flowNodeRef>a</flowNodeRef>
                            </lane>
                        </childLaneSet>
                    </lane>
                </laneSet>
                <task id="a"/>
                <task id="b"/>
            </process>"#,
        )
        .unwrap();
        assert_eq!(model.participants[0].lanes[0].id, "l1");
        assert_eq!(model.participants[0].lanes[0].sub_lanes[0].id, "l1-1");
        // `a`는 상위·하위 레인 둘 다에 나열됐으니 더 안쪽인 l1-1이 이긴다.
        assert_eq!(model.element("a").unwrap().container.as_deref(), Some("l1-1"));
        // `b`는 어떤 flowNodeRef에도 없으니 풀(합성 참여자) 직속.
        assert_eq!(model.element("b").unwrap().container.as_deref(), Some("p1"));
    }

    #[test]
    fn single_process_without_collaboration_has_no_participants_and_flat_containers() {
        let model = parse(r#"<process id="p1"><startEvent id="s"/></process>"#).unwrap();
        assert!(model.participants.is_empty());
        assert_eq!(model.element("s").unwrap().container, None);
    }

    #[test]
    fn single_process_with_lane_set_and_no_collaboration_synthesizes_one_participant() {
        let model = parse(r#"<process id="p1" name="프로세스"><laneSet><lane id="l1" name="레인"><flowNodeRef>s</flowNodeRef></lane></laneSet><startEvent id="s"/></process>"#).unwrap();
        assert_eq!(model.participants.len(), 1);
        assert_eq!(model.participants[0].id, "p1");
        assert_eq!(model.participants[0].name, "프로세스");
        assert_eq!(model.element("s").unwrap().container.as_deref(), Some("l1"));
    }

    #[test]
    fn title_prefers_collaboration_then_sole_process_then_definitions_then_nothing() {
        assert_eq!(parse(r#"<definitions name="정의"><collaboration id="c1" name="협업"/><process id="p1" name="프로세스"/></definitions>"#).unwrap().title, "협업");
        assert_eq!(parse(r#"<definitions name="정의"><process id="p1" name="프로세스"/></definitions>"#).unwrap().title, "프로세스");
        assert_eq!(parse(r#"<definitions name="정의"><process id="p1"/></definitions>"#).unwrap().title, "정의");
        assert_eq!(parse(r#"<definitions><process id="p1"/></definitions>"#).unwrap().title, "");
    }

    #[test]
    fn bpmndi_section_present_or_absent_yields_the_same_model() {
        let with_diagram = parse(r#"<definitions><process id="p1"><startEvent id="s"/></process><bpmndi:BPMNDiagram id="d"><bpmndi:BPMNPlane bpmnElement="p1"/></bpmndi:BPMNDiagram></definitions>"#).unwrap();
        let without_diagram = parse(r#"<definitions><process id="p1"><startEvent id="s"/></process></definitions>"#).unwrap();
        assert_eq!(with_diagram.elements.len(), without_diagram.elements.len());
        assert_eq!(with_diagram.elements[0].id, without_diagram.elements[0].id);
    }

    // --- 노드: 요소 표·트리거·경계 이벤트·서브프로세스·데이터·주석·무시 ---

    #[test]
    fn every_element_table_row_maps_to_its_kind() {
        let xml = r#"<process id="p1">
            <startEvent id="e1"/>
            <intermediateCatchEvent id="e2"/>
            <intermediateThrowEvent id="e3"/>
            <endEvent id="e4"/>
            <task id="t0"/>
            <userTask id="t1"/>
            <serviceTask id="t2"/>
            <scriptTask id="t3"/>
            <manualTask id="t4"/>
            <businessRuleTask id="t5"/>
            <sendTask id="t6"/>
            <receiveTask id="t7"/>
            <subProcess id="sp1"><task id="inner"/></subProcess>
            <adHocSubProcess id="sp2"/>
            <transaction id="sp3"/>
            <callActivity id="ca1"/>
            <exclusiveGateway id="g1"/>
            <parallelGateway id="g2"/>
            <inclusiveGateway id="g3"/>
            <complexGateway id="g4"/>
            <eventBasedGateway id="g5"/>
            <dataObjectReference id="do1" name="주문서"/>
            <dataObject id="do1-def"/>
            <dataStoreReference id="ds1" name="창고DB"/>
            <textAnnotation id="ta1"><text>메모</text></textAnnotation>
        </process>"#;
        let model = parse(xml).unwrap();
        assert_eq!(model.element("e1").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: None });
        assert_eq!(model.element("e2").unwrap().kind, ElementKind::Event { position: EventPosition::Intermediate, trigger: None });
        assert_eq!(model.element("e3").unwrap().kind, ElementKind::Event { position: EventPosition::Intermediate, trigger: None });
        assert_eq!(model.element("e4").unwrap().kind, ElementKind::Event { position: EventPosition::End, trigger: None });
        assert_eq!(model.element("t0").unwrap().kind, ElementKind::Task(TaskKind::None));
        assert_eq!(model.element("t1").unwrap().kind, ElementKind::Task(TaskKind::User));
        assert_eq!(model.element("t2").unwrap().kind, ElementKind::Task(TaskKind::Service));
        assert_eq!(model.element("t3").unwrap().kind, ElementKind::Task(TaskKind::Script));
        assert_eq!(model.element("t4").unwrap().kind, ElementKind::Task(TaskKind::Manual));
        assert_eq!(model.element("t5").unwrap().kind, ElementKind::Task(TaskKind::BusinessRule));
        assert_eq!(model.element("t6").unwrap().kind, ElementKind::Task(TaskKind::Send));
        assert_eq!(model.element("t7").unwrap().kind, ElementKind::Task(TaskKind::Receive));
        assert_eq!(model.element("sp1").unwrap().kind, ElementKind::Subprocess);
        assert_eq!(model.element("sp2").unwrap().kind, ElementKind::Subprocess);
        assert_eq!(model.element("sp3").unwrap().kind, ElementKind::Subprocess);
        assert_eq!(model.element("ca1").unwrap().kind, ElementKind::CallActivity);
        assert_eq!(model.element("g1").unwrap().kind, ElementKind::Gateway(GatewayKind::Exclusive));
        assert_eq!(model.element("g2").unwrap().kind, ElementKind::Gateway(GatewayKind::Parallel));
        assert_eq!(model.element("g3").unwrap().kind, ElementKind::Gateway(GatewayKind::Inclusive));
        assert_eq!(model.element("g4").unwrap().kind, ElementKind::Gateway(GatewayKind::Complex));
        assert_eq!(model.element("g5").unwrap().kind, ElementKind::Gateway(GatewayKind::EventBased));
        assert_eq!(model.element("do1").unwrap().kind, ElementKind::DataObject);
        assert_eq!(model.element("do1").unwrap().name, "주문서");
        assert_eq!(model.element("ds1").unwrap().kind, ElementKind::DataStore);
        assert_eq!(model.element("ta1").unwrap().kind, ElementKind::TextAnnotation);
        assert_eq!(model.element("ta1").unwrap().name, "메모");
        // `subProcess` 안의 노드는 결과에 없다(4.5).
        assert!(model.element("inner").is_none());
        // `dataObject` 정의는 노드가 아니다(4.8).
        assert!(model.element("do1-def").is_none());
    }

    #[test]
    fn event_definition_count_decides_the_trigger() {
        let model = parse(
            r#"<process id="p1">
                <startEvent id="none"/>
                <startEvent id="one"><messageEventDefinition/></startEvent>
                <startEvent id="two"><messageEventDefinition/><timerEventDefinition/></startEvent>
                <startEvent id="two-parallel" parallelMultiple="true"><messageEventDefinition/><timerEventDefinition/></startEvent>
            </process>"#,
        )
        .unwrap();
        assert_eq!(model.element("none").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: None });
        assert_eq!(model.element("one").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Message) });
        assert_eq!(model.element("two").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Multiple) });
        assert_eq!(model.element("two-parallel").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::ParallelMultiple) });
    }

    #[test]
    fn boundary_event_attaches_and_inherits_the_hosts_container_regardless_of_its_own_lane_listing() {
        let model = parse(
            r#"<process id="p1">
                <laneSet>
                    <lane id="host-lane"><flowNodeRef>host</flowNodeRef></lane>
                    <lane id="other-lane"><flowNodeRef>boundary</flowNodeRef></lane>
                </laneSet>
                <task id="host"/>
                <boundaryEvent id="boundary" attachedToRef="host"><errorEventDefinition/></boundaryEvent>
            </process>"#,
        )
        .unwrap();
        let boundary = model.element("boundary").unwrap();
        assert_eq!(boundary.attached_to.as_deref(), Some("host"));
        assert_eq!(boundary.container.as_deref(), Some("host-lane"));
    }

    #[test]
    fn out_of_scope_elements_are_ignored_and_the_rest_still_renders() {
        let model = parse(
            r#"<process id="p1">
                <group id="g1"/>
                <ioSpecification id="io1"/>
                <property id="prop1"/>
                <documentation id="doc1">설명</documentation>
                <camunda:executionListener id="ext1"/>
                <task id="t1"/>
            </process>"#,
        )
        .unwrap();
        assert_eq!(model.elements.len(), 1);
        assert_eq!(model.elements[0].id, "t1");
    }

    #[test]
    fn missing_name_attribute_yields_an_empty_name() {
        let model = parse(r#"<process id="p1"><task id="t1"/></process>"#).unwrap();
        assert_eq!(model.element("t1").unwrap().name, "");
    }

    // --- 흐름: 시퀀스·default·메시지·연관·데이터 연관·미리 거르기 ---

    #[test]
    fn named_sequence_flow_has_a_label() {
        let model = parse(r#"<process id="p1"><task id="a"/><task id="b"/><sequenceFlow id="f1" sourceRef="a" targetRef="b" name="예"/></process>"#).unwrap();
        assert_eq!(model.flows[0].label, "예");
    }

    #[test]
    fn only_the_flow_named_by_default_is_marked_default_on_both_gateways_and_activities() {
        let model = parse(
            r#"<process id="p1">
                <exclusiveGateway id="gw" default="f1"/>
                <task id="a" default="f3"/>
                <task id="b"/>
                <task id="c"/>
                <sequenceFlow id="f1" sourceRef="gw" targetRef="b"/>
                <sequenceFlow id="f2" sourceRef="gw" targetRef="c"/>
                <sequenceFlow id="f3" sourceRef="a" targetRef="b"/>
            </process>"#,
        )
        .unwrap();
        let is_default = |id: &str| matches!(model.flows.iter().find(|f| f.id == id).unwrap().kind, FlowKind::Sequence { is_default: true });
        assert!(is_default("f1"));
        assert!(!is_default("f2"));
        assert!(is_default("f3"));
    }

    #[test]
    fn condition_expression_without_a_name_has_no_label() {
        let model = parse(r#"<process id="p1"><task id="a"/><task id="b"/><sequenceFlow id="f1" sourceRef="a" targetRef="b"><conditionExpression>${x}</conditionExpression></sequenceFlow></process>"#).unwrap();
        assert_eq!(model.flows[0].label, "");
    }

    #[test]
    fn message_flow_becomes_a_message_flow_and_a_participant_endpoint_survives() {
        let model = parse(
            r#"<definitions>
                <collaboration id="c1">
                    <participant id="customer" name="고객"/>
                    <participant id="seller" name="판매사" processRef="proc1"/>
                    <messageFlow id="m1" sourceRef="customer" targetRef="start"/>
                </collaboration>
                <process id="proc1"><startEvent id="start"/></process>
            </definitions>"#,
        )
        .unwrap();
        let flow = model.flows.iter().find(|f| f.id == "m1").unwrap();
        assert_eq!(flow.kind, FlowKind::Message);
        assert_eq!(flow.source, "customer");
        assert_eq!(flow.target, "start");
    }

    #[test]
    fn message_flow_ending_at_a_lane_is_dropped_but_the_rest_survives() {
        let model = parse(
            r#"<definitions>
                <collaboration id="c1">
                    <participant id="p1" name="고객" processRef="proc-a"/>
                    <participant id="p2" name="판매사" processRef="proc-b"/>
                    <messageFlow id="m1" sourceRef="a" targetRef="lane-b"/>
                    <messageFlow id="m2" sourceRef="a" targetRef="b"/>
                </collaboration>
                <process id="proc-a"><task id="a"/></process>
                <process id="proc-b">
                    <laneSet><lane id="lane-b"><flowNodeRef>b</flowNodeRef></lane></laneSet>
                    <task id="b"/>
                </process>
            </definitions>"#,
        )
        .unwrap();
        assert!(!model.flows.iter().any(|f| f.id == "m1"));
        assert!(model.flows.iter().any(|f| f.id == "m2"));
    }

    #[test]
    fn association_and_data_associations_point_the_expected_direction() {
        let model = parse(
            r#"<process id="p1">
                <task id="a"/>
                <textAnnotation id="note"><text>메모</text></textAnnotation>
                <association id="as1" sourceRef="a" targetRef="note"/>
                <dataObjectReference id="obj" name="주문"/>
                <task id="b">
                    <dataInputAssociation id="dia1"><sourceRef>obj</sourceRef></dataInputAssociation>
                    <dataOutputAssociation id="doa1"><targetRef>obj</targetRef></dataOutputAssociation>
                </task>
            </process>"#,
        )
        .unwrap();
        assert!(model.flows.iter().any(|f| f.kind == FlowKind::Association && f.source == "a" && f.target == "note"));
        assert!(model.flows.iter().any(|f| f.kind == FlowKind::DataAssociation && f.source == "obj" && f.target == "b"));
        assert!(model.flows.iter().any(|f| f.kind == FlowKind::DataAssociation && f.source == "b" && f.target == "obj"));
    }

    #[test]
    fn data_association_ending_at_a_property_id_is_dropped() {
        let model = parse(
            r#"<process id="p1">
                <task id="a">
                    <property id="prop1"/>
                    <dataOutputAssociation id="doa1"><targetRef>prop1</targetRef></dataOutputAssociation>
                </task>
            </process>"#,
        )
        .unwrap();
        assert!(model.flows.is_empty());
    }

    #[test]
    fn flows_referencing_ids_absent_from_the_document_are_kept_for_validate_to_reject() {
        let model = parse(r#"<process id="p1"><task id="a"/><sequenceFlow id="f1" sourceRef="a" targetRef="ghost"/></process>"#).unwrap();
        assert!(model.flows.iter().any(|f| f.id == "f1" && f.target == "ghost"));
    }

    #[test]
    fn two_id_less_flows_get_distinct_synthesized_ids() {
        let model = parse(r#"<process id="p1"><task id="a"/><task id="b"/><task id="c"/><sequenceFlow sourceRef="a" targetRef="b"/><sequenceFlow sourceRef="b" targetRef="c"/></process>"#).unwrap();
        assert_ne!(model.flows[0].id, model.flows[1].id);
    }
}
