//! `diagram::mod`가 mermaid·PlantUML과 같은 모양으로 부르는 BPMN 언어 진입점.
//!
//! `kind_of`는 `parse_xml::looks_like_bpmn`으로 BPMN XML을 스니핑해 갈래 `"xml"`을 돌려준다
//! (하류 `bpmn-yaml`·`bizprocess-bpmn` 스펙이 다른 갈래를 더한다). `render`는 그 갈래를 파서에
//! 넘기고 결과 `Model`을 [`render_model`]로 그린다. 손으로 만든 `Model`을 직접 그리려면
//! `render_model`을 바로 부른다.

pub mod bizprocess;
pub mod lower;
pub mod model;
pub mod parse_bizprocess;
pub mod parse_xml;
pub mod parse_yaml;
pub mod validate;
pub mod vocabulary;
pub mod xml;
pub mod yaml;

use crate::diagram::layout;
use crate::diagram::options::{DiagramOptions, ExpandPolicy};
use crate::line::Line;
use crate::style::Theme;
use model::Model;

/// 코드펜스 본문의 문법 갈래: BPMN XML이면 `Some("xml")`, 아니면 YAML 스니핑이 참이면
/// `Some("yaml")`(하류 `bizprocess-bpmn` 스펙이 다른 갈래를 더한다). XML 스니핑이 먼저다.
pub fn kind_of(source: &str) -> Option<&'static str> {
    if parse_xml::looks_like_bpmn(source) {
        Some("xml")
    } else if parse_yaml::looks_like_bpmn_yaml(source) {
        Some("yaml")
    } else {
        None
    }
}

/// `kind_of` → 갈래별 파서 → `render_model`. 파서 실패는 `None`.
pub fn render(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let model = match kind_of(source)? {
        "xml" => parse_xml::parse(source).ok()?,
        "yaml" => parse_yaml::parse(source).ok()?,
        _ => return None,
    };
    render_model(&model, theme, width, options)
}

/// `elements`가 비면 `None` → 검증 → 변환 → 방향 옵션 덮어쓰기 → 그래프 배치기 렌더링.
/// 종류 이름은 참여자 2개 이상이면 `"collaboration"`, 아니면 `"process"`.
pub fn render_model(model: &Model, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    if model.elements.is_empty() {
        return None;
    }
    validate::validate(model).ok()?;
    let mut graph = lower::lower(model);
    options.apply_to_graph(&mut graph);
    let lines = layout::graph::render(&graph, theme, width)?;
    let kind = if model.participants.len() >= 2 { "collaboration" } else { "process" };
    Some((kind, lines))
}

/// `biz-process.md` 개요 문법 진입점 — `parse_bizprocess::parse` → L1마다 검증·정책 확정·
/// `lower_with` → 장마다 배치기 렌더링. 파싱 실패·검증 실패·어느 한 장이라도 폭 초과 → `None`.
/// 종류는 항상 `"process"`(정적) — 둘째 장부터는 본문 안에 빈 줄 + 캡션을 직접 넣는다(design
/// §Key Decisions "여러 장 = 한 본문"). `All`/`Depth(n ≥ 1)`인데 참여자 전환이 있으면 `PerActivity`로
/// 자동 폴백하고 본문 첫 줄에 안내를 남긴다(6.4).
pub fn render_bizprocess(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let parsed = parse_bizprocess::parse(source).ok()?;
    if parsed.models.is_empty() {
        return None;
    }
    let mut body: Vec<Line> = Vec::new();
    let mut fallback_notice: Option<String> = None;
    let mut is_first_chapter = true;

    for model in &parsed.models {
        validate::validate(model).ok()?;
        let transitioning = validate::participant_transitions(model);
        let (policy, fell_back) = resolve_expand_policy(options.expand_policy, &transitioning);
        if fell_back && fallback_notice.is_none() {
            fallback_notice = Some(fallback_notice_text(options.expand_policy));
        }
        for diagram in lower::lower_with(model, policy) {
            let mut graph = diagram.graph;
            options.apply_to_graph(&mut graph);
            let lines = layout::graph::render(&graph, theme, width)?;
            if !is_first_chapter {
                body.push(Line::empty());
                body.push(crate::diagram::caption(crate::diagram::Language::BizProcess.name(), &diagram.kind.caption_kind(), theme, width));
            }
            body.extend(lines);
            is_first_chapter = false;
        }
    }

    if let Some(message) = fallback_notice {
        body.insert(0, Line::single(message, theme.diagram_note));
    }
    Some(("process", body))
}

/// `All`/`Depth(n ≥ 1)`이고 `participant_transitions`가 비지 않으면 `PerActivity`로 자동
/// 폴백한다(`Depth(0)`은 어차피 참여자 전환을 넘나들 일이 없어 폴백하지 않는다). 두 번째 값은
/// 폴백이 일어났는지.
fn resolve_expand_policy(requested: ExpandPolicy, transitioning: &[String]) -> (ExpandPolicy, bool) {
    let needs_full_expansion = matches!(requested, ExpandPolicy::All) || matches!(requested, ExpandPolicy::Depth(n) if n >= 1);
    if needs_full_expansion && !transitioning.is_empty() { (ExpandPolicy::PerActivity, true) } else { (requested, false) }
}

fn fallback_notice_text(requested: ExpandPolicy) -> String {
    let value = match requested {
        ExpandPolicy::All => "all".to_string(),
        ExpandPolicy::Depth(n) => n.to_string(),
        ExpandPolicy::PerActivity => "activity".to_string(),
    };
    format!("※ depth={value} 불가(참여자 전환) → activity")
}

/// bpmn.io 형식 그대로 쓴 공용 XML fixture(테스트 전용). `examples/`에는 두지 않는다(design §Out-of-Scope).
#[cfg(test)]
pub(crate) mod fixtures {
    /// 주문 처리 협업 — 풀 `고객`(레인 없음)·`판매사`(레인 `영업`·`창고`), `«message»` 시작,
    /// `«user»`/`«service»` 태스크, `×` 게이트웨이 + default 흐름, 경계 `«error»` 이벤트, 메시지
    /// 흐름, `<?xml` 선언·`bpmn:` 접두사·`xmlns:*` 5개·`incoming`/`outgoing`·`&#10;` 이름·
    /// `conditionExpression`·`bpmndi:BPMNDiagram` 구획을 모두 담는다(requirements 7.3).
    const ORDER_PROCESSING_HEADER: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "\n",
        r#"<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL" xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI" xmlns:dc="http://www.omg.org/spec/DD/20100524/DC" xmlns:di="http://www.omg.org/spec/DD/20100524/DI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" id="Definitions_1" targetNamespace="http://bpmn.io/schema/bpmn">"#,
    );

    pub fn order_processing_collaboration() -> String {
        format!("{ORDER_PROCESSING_HEADER}{ORDER_PROCESSING_BODY}{ORDER_PROCESSING_DIAGRAM}</bpmn:definitions>\n")
    }

    /// 위와 같은 문서에서 `bpmndi:BPMNDiagram` 구획만 지운 것 — 출력 바이트 동일 확인용(2.5).
    pub fn order_processing_collaboration_without_diagram() -> String {
        format!("{ORDER_PROCESSING_HEADER}{ORDER_PROCESSING_BODY}</bpmn:definitions>\n")
    }

    const ORDER_PROCESSING_BODY: &str = r#"
  <bpmn:collaboration id="Collaboration_1">
    <bpmn:participant id="Participant_Customer" name="고객" processRef="Process_Customer"/>
    <bpmn:participant id="Participant_Vendor" name="판매사" processRef="Process_Vendor"/>
    <bpmn:messageFlow id="Flow_Message" sourceRef="Task_CreateOrder" targetRef="StartEvent_OrderReceived"/>
  </bpmn:collaboration>
  <bpmn:process id="Process_Customer" isExecutable="false">
    <bpmn:startEvent id="StartEvent_Customer" name="시작">
      <bpmn:outgoing>Flow_1</bpmn:outgoing>
    </bpmn:startEvent>
    <bpmn:userTask id="Task_CreateOrder" name="주문서&#10;작성">
      <bpmn:incoming>Flow_1</bpmn:incoming>
      <bpmn:outgoing>Flow_2</bpmn:outgoing>
    </bpmn:userTask>
    <bpmn:endEvent id="EndEvent_CustomerDone" name="완료">
      <bpmn:incoming>Flow_2</bpmn:incoming>
    </bpmn:endEvent>
    <bpmn:sequenceFlow id="Flow_1" sourceRef="StartEvent_Customer" targetRef="Task_CreateOrder"/>
    <bpmn:sequenceFlow id="Flow_2" sourceRef="Task_CreateOrder" targetRef="EndEvent_CustomerDone"/>
  </bpmn:process>
  <bpmn:process id="Process_Vendor" isExecutable="false">
    <bpmn:laneSet id="LaneSet_1">
      <bpmn:lane id="Lane_Sales" name="영업">
        <bpmn:flowNodeRef>StartEvent_OrderReceived</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>Task_ReviewOrder</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>BoundaryEvent_Timeout</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>EndEvent_Cancelled</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>Task_NotifyOOS</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>EndEvent_OutOfStock</bpmn:flowNodeRef>
      </bpmn:lane>
      <bpmn:lane id="Lane_Warehouse" name="창고">
        <bpmn:flowNodeRef>Gateway_StockCheck</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>Task_PrepareShipment</bpmn:flowNodeRef>
        <bpmn:flowNodeRef>EndEvent_Shipped</bpmn:flowNodeRef>
      </bpmn:lane>
    </bpmn:laneSet>
    <bpmn:startEvent id="StartEvent_OrderReceived">
      <bpmn:messageEventDefinition id="MessageEventDefinition_1"/>
      <bpmn:outgoing>Flow_Sales1</bpmn:outgoing>
    </bpmn:startEvent>
    <bpmn:userTask id="Task_ReviewOrder" name="주문 검토">
      <bpmn:incoming>Flow_Sales1</bpmn:incoming>
      <bpmn:outgoing>Flow_Cross1</bpmn:outgoing>
    </bpmn:userTask>
    <bpmn:boundaryEvent id="BoundaryEvent_Timeout" name="시한 초과" attachedToRef="Task_ReviewOrder">
      <bpmn:errorEventDefinition id="ErrorEventDefinition_1"/>
      <bpmn:outgoing>Flow_Sales2</bpmn:outgoing>
    </bpmn:boundaryEvent>
    <bpmn:endEvent id="EndEvent_Cancelled" name="취소">
      <bpmn:incoming>Flow_Sales2</bpmn:incoming>
    </bpmn:endEvent>
    <bpmn:serviceTask id="Task_NotifyOOS" name="품절 안내">
      <bpmn:incoming>Flow_Cross2</bpmn:incoming>
      <bpmn:outgoing>Flow_Sales3</bpmn:outgoing>
    </bpmn:serviceTask>
    <bpmn:endEvent id="EndEvent_OutOfStock" name="품절 취소">
      <bpmn:incoming>Flow_Sales3</bpmn:incoming>
    </bpmn:endEvent>
    <bpmn:exclusiveGateway id="Gateway_StockCheck" name="재고 있음?" default="Flow_Cross2">
      <bpmn:incoming>Flow_Cross1</bpmn:incoming>
      <bpmn:outgoing>Flow_StockYes</bpmn:outgoing>
      <bpmn:outgoing>Flow_Cross2</bpmn:outgoing>
    </bpmn:exclusiveGateway>
    <bpmn:userTask id="Task_PrepareShipment" name="출고 준비">
      <bpmn:incoming>Flow_StockYes</bpmn:incoming>
      <bpmn:outgoing>Flow_Wh2</bpmn:outgoing>
    </bpmn:userTask>
    <bpmn:endEvent id="EndEvent_Shipped" name="완료">
      <bpmn:incoming>Flow_Wh2</bpmn:incoming>
    </bpmn:endEvent>
    <bpmn:sequenceFlow id="Flow_Sales1" sourceRef="StartEvent_OrderReceived" targetRef="Task_ReviewOrder"/>
    <bpmn:sequenceFlow id="Flow_Cross1" sourceRef="Task_ReviewOrder" targetRef="Gateway_StockCheck">
      <bpmn:conditionExpression xsi:type="bpmn:tFormalExpression">${true}</bpmn:conditionExpression>
    </bpmn:sequenceFlow>
    <bpmn:sequenceFlow id="Flow_StockYes" name="예" sourceRef="Gateway_StockCheck" targetRef="Task_PrepareShipment"/>
    <bpmn:sequenceFlow id="Flow_Cross2" sourceRef="Gateway_StockCheck" targetRef="Task_NotifyOOS"/>
    <bpmn:sequenceFlow id="Flow_Sales2" sourceRef="BoundaryEvent_Timeout" targetRef="EndEvent_Cancelled"/>
    <bpmn:sequenceFlow id="Flow_Sales3" sourceRef="Task_NotifyOOS" targetRef="EndEvent_OutOfStock"/>
    <bpmn:sequenceFlow id="Flow_Wh2" sourceRef="Task_PrepareShipment" targetRef="EndEvent_Shipped"/>
  </bpmn:process>
"#;

    const ORDER_PROCESSING_DIAGRAM: &str = r#"
  <bpmndi:BPMNDiagram id="BPMNDiagram_1">
    <!-- 좌표는 읽지 않는다 — BPMNDI는 영구 제외(design §Out-of-Scope) -->
    <bpmndi:BPMNPlane id="BPMNPlane_1" bpmnElement="Collaboration_1">
      <bpmndi:BPMNShape id="Shape_Customer" bpmnElement="Participant_Customer" isHorizontal="true">
        <dc:Bounds x="160" y="80" width="600" height="150"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNShape id="Shape_Vendor" bpmnElement="Participant_Vendor" isHorizontal="true">
        <dc:Bounds x="160" y="260" width="600" height="300"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNEdge id="Edge_Message" bpmnElement="Flow_Message">
        <![CDATA[ waypoints are not read by dg ]]>
        <di:waypoint x="300" y="230"/>
        <di:waypoint x="300" y="300"/>
      </bpmndi:BPMNEdge>
    </bpmndi:BPMNPlane>
  </bpmndi:BPMNDiagram>
"#;

    /// 주문 처리 협업 — 위 XML fixture와 같은 참여자·레인·노드·흐름 순서로 옮긴 YAML(7.4). 통합
    /// 테스트가 XML fixture 렌더링과 줄 단위로 비교한다.
    pub const ORDER_PROCESSING_YAML: &str = include_str!("fixtures/order_processing.yaml");
    /// 같은 문서를 들여쓰기 폭 2/4/혼합으로 쓴 것(2.1) — 셋 다 같은 그림이어야 한다.
    pub const WIDTH_TWO_YAML: &str = include_str!("fixtures/width_two.yaml");
    pub const WIDTH_FOUR_YAML: &str = include_str!("fixtures/width_four.yaml");
    pub const WIDTH_MIXED_YAML: &str = include_str!("fixtures/width_mixed.yaml");
    /// 홑따옴표·겹따옴표 이스케이프와 타입처럼 보이는 평문(2.2, 2.4, 2.5).
    pub const QUOTING_AND_TYPES_YAML: &str = include_str!("fixtures/quoting_and_types.yaml");
    /// `participants:` 없이 최상위 `nodes:`만 있는 평면 문서(3.5).
    pub const FLAT_NODES_YAML: &str = include_str!("fixtures/flat_nodes.yaml");
    /// 참여자 안 레인 안 하위 레인(3.3).
    pub const NESTED_LANES_YAML: &str = include_str!("fixtures/nested_lanes.yaml");

    // --- bizprocess-bpmn: 회귀 픽스처 ---

    /// 저장소의 실제 `biz-process.md` 7개(원본 경로 `include_str!` — 복사본 없음, SSoT). (스펙 이름,
    /// 본문) 쌍, 7.1의 회귀 렌더링이 쓴다.
    pub const BIZPROCESS_REAL: &[(&str, &str)] = &[
        ("dg-watch-mode", include_str!("../../../.kiro/specs/dg-watch-mode/biz-process.md")),
        ("gitgraph-branch-distinction", include_str!("../../../.kiro/specs/gitgraph-branch-distinction/biz-process.md")),
        ("gitgraph-junction-polish", include_str!("../../../.kiro/specs/gitgraph-junction-polish/biz-process.md")),
        ("markdown-gfm-alerts", include_str!("../../../.kiro/specs/markdown-gfm-alerts/biz-process.md")),
        ("markdown-link-navigation", include_str!("../../../.kiro/specs/markdown-link-navigation/biz-process.md")),
        ("markdown-source-view", include_str!("../../../.kiro/specs/markdown-source-view/biz-process.md")),
        ("sequence-fragment-frame-distinction", include_str!("../../../.kiro/specs/sequence-fragment-frame-distinction/biz-process.md")),
    ];
    /// 태그·`THROW`·`ELSE IF`·L3 둘·L4 Sub-Process·이어지는 줄·괄호 이름을 담은 합성 fixture
    /// (테스트 전용, `examples/` 밖).
    pub const TAGGED_PROCESS_MD: &str = include_str!("fixtures/tagged_process.md");
    /// L4 안 참여자 전환(6.5) — `step` 장 재귀 확인용.
    pub const TRANSITION_IN_STEP_MD: &str = include_str!("fixtures/transition_in_step.md");
}

#[cfg(test)]
mod tests {
    use super::model::{Element, ElementKind, EventPosition, EventTrigger, Flow, FlowKind, GatewayKind, Lane, Participant, TaskKind};
    use super::*;
    use crate::diagram::ir::Direction;
    use crate::diagram::options::DiagramOptions;

    fn task(id: &str, name: &str, container: Option<&str>) -> Element {
        Element { id: id.into(), name: name.into(), kind: ElementKind::Task(TaskKind::None), container: container.map(str::to_string), attached_to: None, parent: None }
    }

    fn one_pool_model() -> Model {
        Model {
            participants: vec![Participant { id: "p1".into(), name: "프로세스".into(), lanes: Vec::new() }],
            elements: vec![
                Element { id: "start".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: Some("p1".into()), attached_to: None, parent: None },
                task("t1", "처리", Some("p1")),
                Element { id: "end".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("p1".into()), attached_to: None, parent: None },
            ],
            flows: vec![
                Flow { id: "f1".into(), source: "start".into(), target: "t1".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f2".into(), source: "t1".into(), target: "end".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
            ],
            ..Model::default()
        }
    }

    #[test]
    fn kind_name_is_process_for_one_pool_and_collaboration_for_two_or_more() {
        let model = one_pool_model();
        let (kind, _) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "process");

        let mut two_pools = model.clone();
        two_pools.participants.push(Participant { id: "p2".into(), name: "상대".into(), lanes: Vec::new() });
        two_pools.elements.push(task("t2", "상대 작업", Some("p2")));
        let (kind, _) = render_model(&two_pools, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
    }

    #[test]
    fn model_without_elements_or_invalid_model_renders_nothing() {
        assert_eq!(render_model(&Model::default(), &Theme::none(), 100, DiagramOptions::default()), None);

        let invalid = Model {
            elements: vec![task("a", "a", Some("ghost-pool"))],
            ..Model::default()
        };
        assert_eq!(render_model(&invalid, &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn node_less_definitions_sniffs_as_xml_but_renders_nothing() {
        assert_eq!(kind_of("<definitions></definitions>"), Some("xml"));
        assert_eq!(render("<definitions></definitions>", &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn non_xml_bodies_have_no_grammar_kind() {
        assert_eq!(kind_of("process:\n  id: p1\n"), None);
        assert_eq!(kind_of("임의의 한글 문장입니다"), None);
    }

    #[test]
    fn prefix_less_single_process_renders_as_process_without_a_pool_band() {
        let source = r#"<definitions><process id="p1"><startEvent id="s"/><task id="t"/><endEvent id="e"/><sequenceFlow id="f1" sourceRef="s" targetRef="t"/><sequenceFlow id="f2" sourceRef="t" targetRef="e"/></process></definitions>"#;
        // bpmn-event-shape-notation 이후 종료 이벤트는 테두리 없이 겹원 글자(◉)만 그리므로
        // `┃`는 더 이상 어디에서도 나타나지 않는다. "풀 띠 없음"은 모델 자체에 참여자가 없다는
        // 사실로 직접 확인한다(풀 띠는 참여자별로만 그려지므로 참여자가 없으면 띠도 없다).
        let model = parse_xml::parse(source).unwrap();
        assert!(model.participants.is_empty());
        let (kind, _) = render(source, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "process");
    }

    #[test]
    fn order_processing_fixture_renders_as_collaboration() {
        let fixture = fixtures::order_processing_collaboration();
        let (kind, _) = render(&fixture, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
    }

    #[test]
    fn a_truncated_fixture_fails_to_parse_and_renders_nothing() {
        let fixture = fixtures::order_processing_collaboration();
        // 아스키 문자열 경계에서 자르므로 문자 경계 걱정 없이 안전하게 잘린다 — 협업·고객 프로세스만
        // 열린 채 남고 나머지가 통째로 사라져 반드시 파싱에 실패한다.
        let cut_at = fixture.find(r#"<bpmn:process id="Process_Vendor""#).expect("fixture에 있어야 한다");
        assert_eq!(render(&fixture[..cut_at], &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn empty_pool_renders_band_and_title_alongside_the_populated_pool() {
        let model = Model {
            title: "빈 협업".into(),
            participants: vec![Participant { id: "empty".into(), name: "미확정 참여자".into(), lanes: Vec::new() }, Participant { id: "full".into(), name: "우리".into(), lanes: Vec::new() }],
            elements: vec![task("a", "작업", Some("full"))],
            ..Model::default()
        };
        let (kind, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("빈 협업"));
        assert!(rendered.contains("미확정 참여자"));
        assert!(rendered.contains("우리"));
        assert!(rendered.contains("작업"));
    }

    #[test]
    fn nodes_outside_any_pool_mixed_with_pools_do_not_panic() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![task("in", "안", Some("p1")), task("out", "밖", None)],
            flows: vec![Flow { id: "f1".into(), source: "out".into(), target: "in".into(), label: String::new(), kind: FlowKind::Association }],
            ..Model::default()
        };
        let _ = render_model(&model, &Theme::none(), 100, DiagramOptions::default());
    }

    #[test]
    fn boundary_event_sits_right_of_host_in_the_same_lane_with_dashed_link_then_solid_sequence() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![
                task("host", "작업", Some("p1")),
                // container를 호스트와 같은 값으로 명시(1.7 "소속이 없거나 호스트와 같음") — 검증의
                // 풀 판정은 리터럴 container만 보므로(호스트 상속은 lower 몫), 풀 안쪽으로 나가는
                // 시퀀스 흐름(5.2)이 통과하려면 여기서 명시적으로 맞춰 둔다.
                Element { id: "boundary".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: Some("p1".into()), attached_to: Some("host".into()), parent: None },
                Element { id: "end".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("p1".into()), attached_to: None, parent: None },
            ],
            flows: vec![Flow { id: "f1".into(), source: "boundary".into(), target: "end".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } }],
            ..Model::default()
        };
        let (_, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains('╌'));
        assert!(rendered.contains("«error»"));

        // LR: 경계 이벤트가 호스트 오른쪽 다음 층·같은 레인 안(비슷한 줄 대역)에 있고, 그 뒤
        // 시퀀스 흐름은 실선(대시가 아닌 가로줄)으로 이어진다(5.1, 5.2).
        let host_row = lines.iter().position(|l| l.text().contains("작업")).expect("호스트 상자가 있어야 한다");
        let host_col = lines[host_row].text().find("작업").unwrap();
        let boundary_row = lines.iter().position(|l| l.text().contains("«error»")).expect("경계 이벤트 상자가 있어야 한다");
        let boundary_col = lines[boundary_row].text().find("«error»").unwrap();
        assert!(boundary_col > host_col, "경계 이벤트는 호스트보다 오른쪽 칸(다음 층)에 있어야 한다");
        assert!(boundary_row.abs_diff(host_row) <= 4, "경계 이벤트는 호스트와 같은 레인 대역 안에 있어야 한다");
    }

    #[test]
    fn vertical_stacks_pool_bands_left_right_and_horizontal_stacks_top_down() {
        let mut model = one_pool_model();
        model.participants.push(Participant { id: "p2".into(), name: "P2".into(), lanes: Vec::new() });
        model.elements.push(task("t2", "다른 작업", Some("p2")));

        let row_of = |lines: &[Line], needle: &str| lines.iter().position(|l| l.text().contains(needle)).expect("풀 제목이 있어야 한다");
        let col_of = |lines: &[Line], row: usize, needle: &str| lines[row].text().find(needle).expect("풀 제목이 있어야 한다");

        model.orientation = model::Orientation::Horizontal;
        let (_, horizontal) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        // 가로(흐름 왼쪽→오른쪽): 풀 띠가 위아래로 쌓여 두 풀 제목이 서로 다른 줄에 있다.
        assert_ne!(row_of(&horizontal, "프로세스"), row_of(&horizontal, "P2"));

        model.orientation = model::Orientation::Vertical;
        let (_, vertical) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        // 세로(흐름 위→아래): 풀 띠가 좌우로 나란해 두 풀 제목이 같은 줄의 서로 다른 칸에 있다.
        let p1_row = row_of(&vertical, "프로세스");
        assert_eq!(p1_row, row_of(&vertical, "P2"));
        assert_ne!(col_of(&vertical, p1_row, "프로세스"), col_of(&vertical, p1_row, "P2"));
    }

    #[test]
    fn direction_option_overrides_horizontal_model_orientation() {
        let model = one_pool_model();
        let options = DiagramOptions { direction: Some((Direction::TopDown, false)), ..DiagramOptions::default() };
        // 방향 옵션이 lower()가 만든 LeftRight를 덮어써도 패닉 없이 렌더링된다.
        assert!(render_model(&model, &Theme::none(), 100, options).is_some());
    }

    #[test]
    fn width_below_minimum_yields_none() {
        let model = one_pool_model();
        assert_eq!(render_model(&model, &Theme::none(), 8, DiagramOptions::default()), None);
    }

    #[test]
    fn odd_shapes_do_not_panic_self_loop_cycle_unnamed_three_level_lanes_multiple_boundaries() {
        let model = Model {
            participants: vec![Participant {
                id: "p1".into(),
                name: "".into(),
                lanes: vec![Lane { id: "l1".into(), name: "".into(), sub_lanes: vec![Lane { id: "l1-1".into(), name: "".into(), sub_lanes: Vec::new() }] }],
            }],
            elements: vec![
                task("a", "", Some("l1-1")),
                task("b", "", Some("l1-1")),
                Element { id: "boundary1".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: Some("a".into()), parent: None },
                Element { id: "boundary2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: None, attached_to: Some("a".into()), parent: None },
                Element { id: "gw".into(), name: "".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: Some("l1-1".into()), attached_to: None, parent: None },
            ],
            flows: vec![
                Flow { id: "self".into(), source: "a".into(), target: "a".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "ab".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "ba".into(), source: "b".into(), target: "a".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "a_gw".into(), source: "a".into(), target: "gw".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "gw_b".into(), source: "gw".into(), target: "b".into(), label: String::new(), kind: FlowKind::Sequence { is_default: true } },
            ],
            ..Model::default()
        };
        for orientation in [model::Orientation::Horizontal, model::Orientation::Vertical] {
            let mut model = model.clone();
            model.orientation = orientation;
            let _ = render_model(&model, &Theme::none(), 100, DiagramOptions::default());
        }
    }

    /// design §Testing Strategy 8.3 — 풀 `고객`, 풀 `판매사`(레인 `영업`·`창고`)의 주문 처리
    /// 협업. 파일을 파싱하지 않고 손으로 만든 `Model`로만 검증한다.
    fn order_processing_collaboration_model() -> Model {
        let user_task = |id: &str, name: &str, container: &str| Element { id: id.into(), name: name.into(), kind: ElementKind::Task(TaskKind::User), container: Some(container.into()), attached_to: None, parent: None };
        Model {
            title: "주문 처리".into(),
            orientation: model::Orientation::Horizontal,
            participants: vec![
                Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() },
                Participant {
                    id: "vendor".into(),
                    name: "판매사".into(),
                    lanes: vec![Lane { id: "lane-sales".into(), name: "영업".into(), sub_lanes: Vec::new() }, Lane { id: "lane-warehouse".into(), name: "창고".into(), sub_lanes: Vec::new() }],
                },
            ],
            elements: vec![
                // 고객
                Element { id: "start-cust".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: Some("customer".into()), attached_to: None, parent: None },
                user_task("order-task", "주문서 작성", "customer"),
                Element { id: "end-cust".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("customer".into()), attached_to: None, parent: None },
                // 판매사 · 영업
                Element { id: "msg-start".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Message) }, container: Some("lane-sales".into()), attached_to: None, parent: None },
                user_task("review-task", "주문 검토", "lane-sales"),
                // container를 호스트(review-task)와 같은 값으로 명시 — 검증의 풀 판정은 리터럴
                // container만 보므로(호스트 상속은 lower 몫), 뒤이은 시퀀스 흐름(5.2)이 통과하려면
                // 여기서 맞춰 둔다.
                Element {
                    id: "boundary-error".into(),
                    name: "시한 초과".into(),
                    kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) },
                    container: Some("lane-sales".into()),
                    attached_to: Some("review-task".into()),
                    parent: None,
                },
                Element { id: "end-cancel".into(), name: "취소".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-sales".into()), attached_to: None, parent: None },
                Element { id: "notify-oos".into(), name: "품절 안내".into(), kind: ElementKind::Task(TaskKind::Service), container: Some("lane-sales".into()), attached_to: None, parent: None },
                Element { id: "end-oos".into(), name: "품절 취소".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-sales".into()), attached_to: None, parent: None },
                // 판매사 · 창고
                Element { id: "gateway-stock".into(), name: "재고 있음?".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: Some("lane-warehouse".into()), attached_to: None, parent: None },
                user_task("prepare-task", "출고 준비", "lane-warehouse"),
                Element { id: "end-shipped".into(), name: "완료".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-warehouse".into()), attached_to: None, parent: None },
            ],
            flows: vec![
                // 고객
                Flow { id: "f-cust-1".into(), source: "start-cust".into(), target: "order-task".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-cust-2".into(), source: "order-task".into(), target: "end-cust".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 풀 사이 메시지 흐름
                Flow { id: "f-msg".into(), source: "order-task".into(), target: "msg-start".into(), label: String::new(), kind: FlowKind::Message },
                // 영업
                Flow { id: "f-sales-1".into(), source: "msg-start".into(), target: "review-task".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-sales-2".into(), source: "boundary-error".into(), target: "end-cancel".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-sales-3".into(), source: "notify-oos".into(), target: "end-oos".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 영업 → 창고(같은 풀, 다른 레인)
                Flow { id: "f-cross-1".into(), source: "review-task".into(), target: "gateway-stock".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 창고
                Flow { id: "f-wh-1".into(), source: "gateway-stock".into(), target: "prepare-task".into(), label: "예".into(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-wh-2".into(), source: "prepare-task".into(), target: "end-shipped".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 창고 → 영업(같은 풀, 다른 레인) — default 흐름
                Flow { id: "f-cross-2".into(), source: "gateway-stock".into(), target: "notify-oos".into(), label: String::new(), kind: FlowKind::Sequence { is_default: true } },
            ],
            groups: Vec::new(),
        }
    }

    #[test]
    fn order_processing_collaboration_renders_every_convention_in_one_picture() {
        let model = order_processing_collaboration_model();
        assert_eq!(validate::validate(&model), Ok(()));
        let (kind, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("폭 100 안에서 렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        for glyph in ["○", "◉", "◎", "«user»", "«service»", "«message»", "«error»", "× 재고 있음?", "╱", "╌"] {
            assert!(rendered.contains(glyph), "렌더링 결과에 {glyph:?}가 있어야 한다:\n{rendered}");
        }
        // `cargo test -- --nocapture`로 육안 확인(design §Key Decisions 표대로 보이는지).
        eprintln!("{rendered}");
    }

    /// bpmn-message-flow-participant-endpoint 4.1 — 블랙박스 풀(요소 없는 참여자 `customer`)이
    /// 판매사 풀 안 `start`로 메시지를 보내는 협업. bugfix.md 재현 절차 2와 같은 모델.
    fn blackbox_pool_message_flow_model() -> Model {
        Model {
            title: "블랙박스 협업".into(),
            participants: vec![
                Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() },
                Participant {
                    id: "seller".into(),
                    name: "판매사".into(),
                    lanes: vec![Lane { id: "sales".into(), name: "영업".into(), sub_lanes: Vec::new() }],
                },
            ],
            elements: vec![
                Element { id: "start".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Message) }, container: Some("sales".into()), attached_to: None, parent: None },
                task("review", "주문 검토", Some("sales")),
            ],
            flows: vec![
                Flow { id: "s1".into(), source: "start".into(), target: "review".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "m1".into(), source: "customer".into(), target: "start".into(), label: "주문".into(), kind: FlowKind::Message },
            ],
            ..Model::default()
        }
    }

    #[test]
    fn blackbox_pool_message_flow_renders_as_collaboration_without_panicking() {
        for orientation in [model::Orientation::Horizontal, model::Orientation::Vertical] {
            let mut model = blackbox_pool_message_flow_model();
            model.orientation = orientation;
            assert_eq!(validate::validate(&model), Ok(()));
            let (kind, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
            assert_eq!(kind, "collaboration");
            let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
            assert!(rendered.contains("고객"), "블랙박스 풀 제목이 있어야 한다:\n{rendered}");
            assert!(rendered.contains('╌'), "메시지 흐름의 점선이 있어야 한다:\n{rendered}");
            if orientation == model::Orientation::Horizontal {
                eprintln!("--- LR ---\n{rendered}");
            } else {
                eprintln!("--- TD ---\n{rendered}");
            }
        }
    }

    #[test]
    fn message_flow_between_two_participant_ids_and_element_to_participant_do_not_panic() {
        let mut model = blackbox_pool_message_flow_model();
        // 양끝이 참여자인 메시지 흐름(닻 ↔ 닻)을 요소 → 참여자 메시지 흐름과 섞는다.
        model.flows.push(Flow { id: "m2".into(), source: "customer".into(), target: "seller".into(), label: String::new(), kind: FlowKind::Message });
        for orientation in [model::Orientation::Horizontal, model::Orientation::Vertical] {
            let mut model = model.clone();
            model.orientation = orientation;
            assert_eq!(validate::validate(&model), Ok(()));
            let (kind, _) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
            assert_eq!(kind, "collaboration");
        }
    }

    /// tasks 4.1 — bpmn.io 형식 fixture(3.1)를 실제로 파싱·렌더링해, 손 모델(8.3
    /// `order_processing_collaboration_model`)과 노드·흐름 수가 같은지, 도형 어휘 11종이 전부
    /// 나오는지, `bpmndi` 구획 유무가 출력에 영향을 주지 않는지 확인한다.
    #[test]
    fn order_processing_fixture_matches_the_hand_model_and_ignores_the_diagram_section() {
        let fixture = fixtures::order_processing_collaboration();
        let model = parse_xml::parse(&fixture).expect("fixture는 파싱돼야 한다");
        let hand_model = order_processing_collaboration_model();
        assert_eq!(model.elements.len(), hand_model.elements.len());
        assert_eq!(model.flows.len(), hand_model.flows.len());

        let (kind, lines) = render(&fixture, &Theme::none(), 100, DiagramOptions::default()).expect("폭 100 안에서 렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        for glyph in ["○", "◉", "◎", "«user»", "«service»", "«message»", "«error»", "× 재고 있음?", "╱", "╌"] {
            assert!(rendered.contains(glyph), "렌더링 결과에 {glyph:?}가 있어야 한다:\n{rendered}");
        }
        // `&#10;`로 인코딩한 두 줄 이름이 실제로 두 줄로 보인다(2.2).
        assert!(rendered.contains("주문서"));
        assert!(rendered.contains("작성"));

        // `bpmndi:BPMNDiagram` 구획을 지운 같은 fixture와 출력 바이트가 같다(2.5).
        let fixture_without_diagram = fixtures::order_processing_collaboration_without_diagram();
        let without_diagram = render(&fixture_without_diagram, &Theme::none(), 100, DiagramOptions::default()).expect("구획 없이도 렌더링돼야 한다");
        assert_eq!(without_diagram.1.iter().map(Line::text).collect::<Vec<_>>(), lines.iter().map(Line::text).collect::<Vec<_>>());

        eprintln!("{rendered}");
    }

    // --- bpmn-yaml: 갈래 배선(3.1) ---

    #[test]
    fn kind_of_recognizes_xml_first_then_yaml_then_neither() {
        assert_eq!(kind_of(&fixtures::order_processing_collaboration()), Some("xml"));
        assert_eq!(kind_of(fixtures::ORDER_PROCESSING_YAML), Some("yaml"));
        assert_eq!(kind_of("process:\n  id: p1\n"), None);
    }

    #[test]
    fn flat_yaml_nodes_render_as_process_without_participants() {
        let (kind, model_participants) = {
            let model = parse_yaml::parse(fixtures::FLAT_NODES_YAML).expect("평면 문서는 파싱돼야 한다");
            (render(fixtures::FLAT_NODES_YAML, &Theme::none(), 100, DiagramOptions::default()).map(|(k, _)| k), model.participants.len())
        };
        assert_eq!(kind, Some("process"));
        assert_eq!(model_participants, 0);
    }

    #[test]
    fn order_processing_yaml_renders_as_collaboration() {
        let (kind, _) = render(fixtures::ORDER_PROCESSING_YAML, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
    }

    #[test]
    fn a_yaml_document_with_a_schema_error_renders_nothing() {
        // 최상위 키 목록에 없는 `nmae:`(6.3) — 스니핑은 통과하지 못하므로(구조 키가 없다)
        // `nodes:`를 곁들여 스니핑은 통과시키고 스키마 오류만 남긴다.
        assert_eq!(render("nodes:\n  - a:\n      nmae: 오타\n      kind: task\n", &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn participants_without_any_nodes_render_nothing() {
        assert_eq!(render("participants:\n  - p1: 이름만\n", &Theme::none(), 100, DiagramOptions::default()), None);
    }

    // --- bpmn-yaml: YAML fixture ≡ XML fixture 렌더링(4.1) ---

    #[test]
    fn yaml_fixture_matches_the_xml_fixture_element_and_flow_counts_and_rendering() {
        let xml_fixture = fixtures::order_processing_collaboration();
        let xml_model = parse_xml::parse(&xml_fixture).expect("XML fixture는 파싱돼야 한다");
        let yaml_model = parse_yaml::parse(fixtures::ORDER_PROCESSING_YAML).expect("YAML fixture는 파싱돼야 한다");
        assert_eq!(yaml_model.elements.len(), xml_model.elements.len());
        assert_eq!(yaml_model.flows.len(), xml_model.flows.len());

        let (xml_kind, xml_lines) = render(&xml_fixture, &Theme::none(), 100, DiagramOptions::default()).expect("XML fixture는 렌더링돼야 한다");
        let (yaml_kind, yaml_lines) = render(fixtures::ORDER_PROCESSING_YAML, &Theme::none(), 100, DiagramOptions::default()).expect("YAML fixture는 렌더링돼야 한다");
        assert_eq!(xml_kind, "collaboration");
        assert_eq!(yaml_kind, "collaboration");
        let xml_text = xml_lines.iter().map(Line::text).collect::<Vec<_>>();
        let yaml_text = yaml_lines.iter().map(Line::text).collect::<Vec<_>>();
        assert_eq!(yaml_text, xml_text, "YAML fixture 렌더링이 XML fixture 렌더링과 줄 단위로 같아야 한다");

        let rendered = yaml_text.join("\n");
        for glyph in ["○", "◉", "◎", "«user»", "«service»", "«message»", "«error»", "× 재고 있음?", "╱", "╌"] {
            assert!(rendered.contains(glyph), "렌더링 결과에 {glyph:?}가 있어야 한다:\n{rendered}");
        }
        // `{text}` 육안 확인용(cargo test -- --nocapture): 풀 두 띠·레인 두 띠·경계 점선이 보이는지.
        eprintln!("{rendered}");
    }

    #[test]
    fn xml_fixture_rendering_is_unchanged_by_the_vocabulary_promotion() {
        // 1.4의 어휘 승격 회귀(7.2) — XML fixture 렌더링에 이전과 같은 글자들이 그대로 있다.
        let fixture = fixtures::order_processing_collaboration();
        let (_, lines) = render(&fixture, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("× 재고 있음?"));
        assert!(rendered.contains('╱'));
        assert!(rendered.contains("«error»"));
    }

    #[test]
    fn vertical_orientation_in_yaml_stacks_pool_titles_on_one_line_and_the_direction_option_overrides_it() {
        // 참여자만 있고 노드가 없으면 `render_model`이 `None`이므로 노드를 하나씩 둔다.
        let source = "orientation: vertical\nparticipants:\n  - p1:\n      name: 참여자1\n      nodes:\n        - a: task\n  - p2:\n      name: 참여자2\n      nodes:\n        - b: task\n";
        let (_, lines) = render(source, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        let row_of = |needle: &str| lines.iter().position(|l| l.text().contains(needle)).expect("있어야 한다");
        assert_eq!(row_of("참여자1"), row_of("참여자2"), "orientation: vertical은 풀 제목을 한 줄에 나란히 둔다:\n{rendered}");

        // 명령줄 방향 옵션(문서와 반대: LeftRight)이 문서의 `orientation: vertical`(TopDown)을 덮는다(3.8).
        let options = DiagramOptions { direction: Some((crate::diagram::ir::Direction::LeftRight, false)), ..DiagramOptions::default() };
        let (_, overridden) = render(source, &Theme::none(), 100, options).expect("렌더링돼야 한다");
        let overridden_row_of = |needle: &str| overridden.iter().position(|l| l.text().contains(needle)).expect("있어야 한다");
        assert_ne!(overridden_row_of("참여자1"), overridden_row_of("참여자2"), "덮어쓴 방향에서는 풀 제목이 다른 줄에 있어야 한다");
    }
}

/// `render_bizprocess` 통합 테스트(tasks 5.1, 6.1, 6.2, 6.3) — 실제 문서 7개 + 합성 fixture.
#[cfg(test)]
mod bizprocess_render_tests {
    use super::*;
    use crate::diagram::options::ExpandPolicy;
    use crate::style::Theme;

    fn render(source: &str, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
        render_bizprocess(source, &Theme::none(), width, options)
    }

    fn text_of(lines: &[Line]) -> String {
        lines.iter().map(Line::text).collect::<Vec<_>>().join("\n")
    }

    fn options_with_depth(policy: ExpandPolicy) -> DiagramOptions {
        DiagramOptions { expand_policy: policy, ..DiagramOptions::default() }
    }

    // --- 7.1: 실제 문서 7개 회귀 ---

    #[test]
    fn every_real_document_renders_with_one_activity_caption_per_l2_and_no_leaked_structure() {
        for (name, source) in fixtures::BIZPROCESS_REAL {
            let (kind, lines) = render(source, 100, DiagramOptions::default()).unwrap_or_else(|| panic!("{name}은 렌더링돼야 한다"));
            assert_eq!(kind, "process");
            let rendered = text_of(&lines);
            let model = &parse_bizprocess::parse(source).unwrap().models[0];
            let l2_count = model.elements.iter().filter(|e| matches!(e.kind, model::ElementKind::Subprocess) && e.parent.is_none()).count();
            let activity_captions = rendered.matches("◈ bizprocess · activity: ").count();
            assert_eq!(activity_captions, l2_count, "{name}: Activity 캡션 수가 L2 수와 같아야 한다\n{rendered}");
            for leaked in ["검토 요청", "valueChainRef", "VC-DG-", "(1.1", "(2.1", "(3.1"] {
                assert!(!rendered.contains(leaked), "{name}: {leaked:?}가 그림에 남으면 안 된다");
            }
        }
    }

    #[test]
    fn dg_watch_mode_shows_five_collapsed_l2_boxes_and_the_gateway_glyphs() {
        let source = fixtures::BIZPROCESS_REAL.iter().find(|(name, _)| *name == "dg-watch-mode").unwrap().1;
        let (_, process_lines) = render(source, 100, DiagramOptions::default()).unwrap();
        let process_text = text_of(&process_lines);
        let process_only = process_text.split("◈ bizprocess · activity").next().unwrap_or(&process_text);
        assert_eq!(process_only.matches("[+]").count(), 5, "Process 장에 접힌 상자 5개:\n{process_only}");

        let activity_text = process_text.split("activity: 감시 모드를 켠다").nth(1).unwrap();
        let this_activity = activity_text.split("◈ bizprocess").next().unwrap();
        assert!(this_activity.contains('×'));
        assert!(this_activity.contains('╱'));
        assert!(this_activity.contains("--watch"));
    }

    // --- 6.1~6.4: 정책 ---

    #[test]
    fn default_policy_is_activity_with_one_caption_per_non_empty_l2() {
        let source = fixtures::BIZPROCESS_REAL[0].1;
        let (_, lines) = render(source, 100, DiagramOptions::default()).unwrap();
        assert_eq!(text_of(&lines).matches("◈ bizprocess · activity: ").count(), 5);
    }

    #[test]
    fn depth_0_has_no_in_body_caption_and_all_is_one_chapter() {
        let source = fixtures::BIZPROCESS_REAL[0].1;
        let (_, lines0) = render(source, 100, options_with_depth(ExpandPolicy::Depth(0))).unwrap();
        assert!(!text_of(&lines0).contains("◈ bizprocess"), "Depth(0)은 장이 하나라 본문 안 캡션이 없어야 한다");

        let (_, lines_all) = render(source, 100, options_with_depth(ExpandPolicy::All)).unwrap();
        assert!(!text_of(&lines_all).contains("◈ bizprocess"), "실제 문서는 참여자 태그가 없어 all이 그대로 한 장이어야 한다");
    }

    #[test]
    fn tagged_fixture_with_all_falls_back_with_a_notice_and_activity_captions() {
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 100, options_with_depth(ExpandPolicy::All)).expect("폴백 후 렌더링돼야 한다");
        let text = text_of(&lines);
        assert!(text.starts_with("※ depth=all 불가(참여자 전환) → activity"), "{text}");
        assert!(text.contains("◈ bizprocess · activity: "));
    }

    #[test]
    fn transition_fixture_produces_a_step_caption() {
        let (_, lines) = render(fixtures::TRANSITION_IN_STEP_MD, 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert!(text_of(&lines).contains("◈ bizprocess · step: "));
    }

    #[test]
    fn two_l1_headings_add_one_more_in_body_process_caption() {
        let source = "## L1 Process: A\n### L2 Activity: X\n## L1 Process: B\n### L2 Activity: Y\n";
        let (kind, lines) = render(source, 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "process");
        assert_eq!(text_of(&lines).matches("◈ bizprocess · process ").count(), 1, "본문 안 process 캡션이 하나 더 있어야 한다(첫 장 캡션은 diagram::render가 바깥에서 붙인다)");
    }

    #[test]
    fn width_eight_yields_none() {
        let source = fixtures::BIZPROCESS_REAL[0].1;
        assert_eq!(render(source, 8, DiagramOptions::default()), None);
    }

    #[test]
    fn a_parse_error_document_yields_none() {
        assert_eq!(render("그냥 평문입니다\n", 100, DiagramOptions::default()), None);
        assert_eq!(render("## L1 Process: A만 있고 L2 없음\n", 100, DiagramOptions::default()), None);
    }

    // --- 6.2: 합성 fixture의 풀·레인·그룹·경계 이벤트 렌더링 ---

    #[test]
    fn tagged_fixture_shows_pool_title_and_lanes_in_first_appearance_order() {
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 100, DiagramOptions::default()).unwrap();
        let text = text_of(&lines);
        assert!(text.contains("신청 처리"), "풀 제목(L1 이름):\n{text}");
        let row_of = |needle: &str| lines.iter().position(|l| l.text().contains(needle));
        let gap_row = row_of("갑").expect("갑 레인이 있어야 한다");
        let eul_row = row_of("을").expect("을 레인이 있어야 한다");
        assert!(gap_row < eul_row, "첫 등장 순서(갑이 먼저)대로 레인이 위에서 아래로 있어야 한다");
    }

    #[test]
    fn tagged_fixture_activity_chapter_shows_dashed_group_borders_and_both_l3_titles() {
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 100, DiagramOptions::default()).unwrap();
        let text = text_of(&lines);
        let activity = text.split("activity: 분류한다").nth(1).unwrap().split("◈ bizprocess").next().unwrap();
        assert!(activity.contains('╌') || activity.contains('╎'), "파선 그룹 테두리:\n{activity}");
        assert!(activity.contains("창구"));
        assert!(activity.contains("확인"));
    }

    #[test]
    fn tagged_fixture_shows_boundary_error_on_the_process_chapter_and_the_error_end_event_on_the_activity_chapter() {
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 100, DiagramOptions::default()).unwrap();
        let text = text_of(&lines);
        let process_only = text.split("◈ bizprocess · activity").next().unwrap();
        assert!(process_only.contains("«error»"), "Process 장 L2 옆 경계 오류 이벤트:\n{process_only}");
        let activity = text.split("activity: 심사한다").nth(1).unwrap();
        assert!(activity.contains("◉ 용량초과") || activity.contains("용량초과"), "{activity}");
        assert!(activity.contains("«error»"));
        assert!(activity.contains('╱'), "default 흐름의 빗금 꼬리");
        assert!(activity.contains("정상") && activity.contains("서류초과") && activity.contains("형식 아님"), "조건 라벨 셋:\n{activity}");
    }

    #[test]
    fn depth_1_notice_mentions_the_numeric_depth() {
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 100, options_with_depth(ExpandPolicy::Depth(1))).unwrap();
        assert!(text_of(&lines).starts_with("※ depth=1 불가(참여자 전환) → activity"));
    }

    /// 회귀(독립 검증에서 발견) — L3 하나·L4 하나·L5 하나(갈래 없음)처럼 상자 안 내용이 아주
    /// 단순한 Activity 장에서, 상자를 우회하는 시작→끝 통과선 때문에 상자 높이가 내용과 무관하게
    /// 부풀지 않아야 한다(`layout::graph::place_block`의 제목-폭 오프셋 규칙이 위→아래 배치
    /// 전용이어야 했는데 왼쪽→오른쪽에도 적용되던 결함).
    #[test]
    fn a_single_leaf_activity_chapter_does_not_inflate_its_box_height() {
        let source = "## L1 Process: 테스트\n### L2 Activity: 하나\n  ### L3 FunctionGroup/UI: 그룹\n    ### L4 Step: 단계\n      ### L5 DetailStep: 상세1\n";
        let (_, lines) = render(source, 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        let text = text_of(&lines);
        let activity = text.split("activity: 하나").nth(1).unwrap();
        let box_lines = activity.lines().take_while(|l| l.contains('│') || l.contains('┌') || l.contains('└')).count();
        assert!(box_lines <= 12, "상자가 내용(노드 하나)에 비해 지나치게 부풀면 안 된다({box_lines}줄):\n{activity}");
    }

    #[test]
    fn direction_tb_option_puts_pool_titles_on_one_line() {
        let options = DiagramOptions { direction: Some((crate::diagram::ir::Direction::TopDown, false)), ..DiagramOptions::default() };
        let (_, lines) = render(fixtures::TAGGED_PROCESS_MD, 200, options).unwrap();
        let row_of = |needle: &str| lines.iter().position(|l| l.text().contains(needle));
        assert_eq!(row_of("갑"), row_of("을"), "세로 배치에서는 레인 제목들이 한 줄에 나란해야 한다");
    }
}



