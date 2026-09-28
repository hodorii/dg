//! `Outline` → `Model` — design §Key Decisions 드릴다운 표·Logic 표·id 규약대로. 검증은 하지
//! 않는다(그건 `validate`의 몫). `bizprocess`(개요)만 알고 BPMN 어휘는 여기서 처음 등장한다.

use super::bizprocess::{self, Heading, LogicItem, Outcome, OutlineError};
use super::model::{Element, ElementKind, EventPosition, EventTrigger, Flow, FlowKind, GatewayKind, Group, Lane, Model, Participant, TaskKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Outline(OutlineError),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Outline(e) => write!(f, "Outline({e})"),
        }
    }
}

pub struct Parsed {
    pub models: Vec<Model>,
    pub diagnostics: Vec<bizprocess::Diagnostic>,
}

/// `bizprocess::parse_outline` → L1마다 `Model` 하나. 개요 오류는 그대로 옮긴다(`ParseError::Outline`).
pub fn parse(source: &str) -> Result<Parsed, ParseError> {
    let outline = bizprocess::parse_outline(source).map_err(ParseError::Outline)?;
    let models = outline.processes.iter().map(build_process).collect();
    Ok(Parsed { models, diagnostics: outline.diagnostics })
}

struct Ctx {
    model: Model,
    flow_counter: usize,
}

impl Ctx {
    fn next_flow_id(&mut self) -> String {
        self.flow_counter += 1;
        format!("@bizprocess-flow:{}", self.flow_counter)
    }

    fn add_flow(&mut self, source: &str, target: &str, label: &str, kind: FlowKind) {
        let id = self.next_flow_id();
        self.model.flows.push(Flow { id, source: source.to_string(), target: target.to_string(), label: label.to_string(), kind });
    }
}

/// 헤딩 자기 `participant` 태그 또는 `inherited`(가장 가까운 조상의 유효 참여자).
fn own_or_inherited_role(heading: &Heading, inherited: Option<&str>) -> Option<String> {
    heading.tags.iter().find(|t| t.key == "participant").map(|t| t.value.clone()).or_else(|| inherited.map(str::to_string))
}

fn container_of(heading: &Heading, inherited: Option<&str>, has_tags: bool) -> Option<String> {
    if !has_tags {
        return None;
    }
    own_or_inherited_role(heading, inherited).map(|role| format!("lane:{role}"))
}

/// L1 전체를 전위 순회(문서 순서)하며 `participant` 값을 첫 등장 순서로 모은다.
fn collect_participant_order(heading: &Heading, order: &mut Vec<String>) {
    if let Some(tag) = heading.tags.iter().find(|t| t.key == "participant")
        && !order.contains(&tag.value)
    {
        order.push(tag.value.clone());
    }
    for child in &heading.children {
        collect_participant_order(child, order);
    }
}

fn build_process(l1: &Heading) -> Model {
    let mut role_order = Vec::new();
    collect_participant_order(l1, &mut role_order);
    let has_tags = !role_order.is_empty();

    let mut model = Model { title: l1.name.clone(), ..Model::default() };
    if has_tags {
        model.participants.push(Participant {
            id: "pool".to_string(),
            name: l1.name.clone(),
            lanes: role_order.into_iter().map(|role| Lane { id: format!("lane:{role}"), name: role, sub_lanes: Vec::new() }).collect(),
        });
    }

    let mut ctx = Ctx { model, flow_counter: 0 };
    let root_role = own_or_inherited_role(l1, None);
    let root_container = root_role.as_deref().filter(|_| has_tags).map(|role| format!("lane:{role}"));
    ctx.model.elements.push(Element { id: "#start".into(), name: String::new(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: root_container.clone(), attached_to: None, parent: None });
    ctx.model.elements.push(Element { id: "#end".into(), name: String::new(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: root_container, attached_to: None, parent: None });

    let mut prev = "#start".to_string();
    for (j, l2) in l1.children.iter().filter(|c| c.level == 2).enumerate() {
        let l2_id = format!("a{}", j + 1);
        build_activity(&mut ctx, l2, &l2_id, has_tags, root_role.as_deref());
        ctx.add_flow(&prev, &l2_id, "", FlowKind::Sequence { is_default: false });
        prev = l2_id;
    }
    ctx.add_flow(&prev, "#end", "", FlowKind::Sequence { is_default: false });

    ctx.model
}

/// L2 하나(+ L3 그룹·L4·L5 전체 서브트리)를 모델에 채운다. `l2_id`는 이미 정해진 합성 id.
fn build_activity(ctx: &mut Ctx, l2: &Heading, l2_id: &str, has_tags: bool, inherited: Option<&str>) {
    let role = own_or_inherited_role(l2, inherited);
    let container = container_of(l2, inherited, has_tags);

    let l3s: Vec<&Heading> = l2.children.iter().filter(|c| c.level == 3).collect();
    let name = match l3s.as_slice() {
        [only] => format!("{}\n{}", l2.name, only.name),
        _ => l2.name.clone(),
    };
    ctx.model.elements.push(Element { id: l2_id.to_string(), name, kind: ElementKind::Subprocess, container: container.clone(), attached_to: None, parent: None });

    let l2_start = format!("{l2_id}#start");
    let l2_end = format!("{l2_id}#end");
    ctx.model.elements.push(Element { id: l2_start.clone(), name: String::new(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: container.clone(), attached_to: None, parent: Some(l2_id.to_string()) });
    ctx.model.elements.push(Element { id: l2_end.clone(), name: String::new(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: container.clone(), attached_to: None, parent: Some(l2_id.to_string()) });

    // L4 목록(+ 소속 L3 그룹 인덱스): L3 둘 이상 → 각 L3의 L4들(그룹 있음); L3 하나 → 그 L3의
    // L4들(그룹 없음, 이름은 이미 부제로 씀); L3 없음 → L2 바로 아래 L4들(2.7).
    let mut l4_entries: Vec<(&Heading, Option<usize>)> = Vec::new();
    match l3s.as_slice() {
        [] => l4_entries.extend(l2.children.iter().filter(|c| c.level == 4).map(|l4| (l4, None))),
        [only] => l4_entries.extend(only.children.iter().filter(|c| c.level == 4).map(|l4| (l4, None))),
        many => {
            for (gi, l3) in many.iter().enumerate() {
                l4_entries.extend(l3.children.iter().filter(|c| c.level == 4).map(move |l4| (l4, Some(gi))));
            }
        }
    }

    let mut group_members: Vec<Vec<String>> = vec![Vec::new(); l3s.len()];
    let mut error_counter = 0usize;
    let mut prev = l2_start.clone();
    for (k, (l4, group_index)) in l4_entries.iter().enumerate() {
        let l4_id = format!("{l2_id}.s{}", k + 1);
        build_step(ctx, l4, &l4_id, l2_id, role.as_deref(), container.clone(), has_tags, &mut error_counter);
        if let Some(gi) = group_index {
            group_members[*gi].push(l4_id.clone());
        }
        ctx.add_flow(&prev, &l4_id, "", FlowKind::Sequence { is_default: false });
        prev = l4_id;
    }
    ctx.add_flow(&prev, &l2_end, "", FlowKind::Sequence { is_default: false });

    if l3s.len() >= 2 {
        for (gi, l3) in l3s.iter().enumerate() {
            ctx.model.groups.push(Group { id: format!("{l2_id}.g{}", gi + 1), name: l3.name.clone(), parent: Some(l2_id.to_string()), members: group_members[gi].clone() });
        }
    }

    // L2 자신에 직접 달린 Logic(정상 문서에는 없음 — 방어적으로 지원, 값 손실 없음).
    build_logic(ctx, &l2.logic, l2_id, None, container.clone(), l2_id, container, &mut error_counter);
}

/// `l2_role`은 L4가 자기 태그가 없을 때 물려받을 값(L2의 유효 참여자), `l2_container`는 L2
/// 자신의 소속(THROW 경계 이벤트가 여기 붙는다).
#[allow(clippy::too_many_arguments)]
fn build_step(ctx: &mut Ctx, l4: &Heading, l4_id: &str, l2_id: &str, l2_role: Option<&str>, l2_container: Option<String>, has_tags: bool, error_counter: &mut usize) {
    let role = own_or_inherited_role(l4, l2_role);
    let container = container_of(l4, l2_role, has_tags);

    let l5s: Vec<&Heading> = l4.children.iter().filter(|c| c.level == 5).collect();
    let kind = if l5s.is_empty() { ElementKind::Task(TaskKind::None) } else { ElementKind::Subprocess };
    ctx.model.elements.push(Element { id: l4_id.to_string(), name: l4.name.clone(), kind, container: container.clone(), attached_to: None, parent: Some(l2_id.to_string()) });

    let mut prev: Option<String> = None;
    for (m, l5) in l5s.iter().enumerate() {
        let l5_id = format!("{l4_id}.d{}", m + 1);
        let l5_container = container_of(l5, role.as_deref(), has_tags);
        ctx.model.elements.push(Element { id: l5_id.clone(), name: l5.name.clone(), kind: ElementKind::Task(TaskKind::None), container: l5_container.clone(), attached_to: None, parent: Some(l4_id.to_string()) });
        if let Some(p) = &prev {
            ctx.add_flow(p, &l5_id, "", FlowKind::Sequence { is_default: false });
        }
        prev = Some(l5_id.clone());
        build_logic(ctx, &l5.logic, &l5_id, Some(l4_id.to_string()), l5_container, l2_id, l2_container.clone(), error_counter);
    }

    build_logic(ctx, &l4.logic, l4_id, Some(l2_id.to_string()), container, l2_id, l2_container, error_counter);
}

#[allow(clippy::too_many_arguments)]
fn build_logic(ctx: &mut Ctx, items: &[LogicItem], owner_id: &str, owner_parent: Option<String>, owner_container: Option<String>, l2_id: &str, l2_container: Option<String>, error_counter: &mut usize) {
    if items.is_empty() {
        return;
    }
    let mut notes: Vec<String> = Vec::new();
    let mut gateway_id: Option<String> = None;
    let mut branch_index = 0usize;

    for item in items {
        match item {
            LogicItem::Note(text) => notes.push(text.clone()),
            LogicItem::Branch { condition, note, outcome } => {
                branch_index += 1;
                let gw_id = match &gateway_id {
                    Some(id) => id.clone(),
                    None => {
                        let id = format!("{owner_id}#gw");
                        ctx.model.elements.push(Element { id: id.clone(), name: String::new(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: owner_container.clone(), attached_to: None, parent: owner_parent.clone() });
                        ctx.add_flow(owner_id, &id, "", FlowKind::Sequence { is_default: false });
                        gateway_id = Some(id.clone());
                        id
                    }
                };
                let (label, is_default) = match condition {
                    Some(c) => (c.clone(), false),
                    None => (note.clone().unwrap_or_default(), true),
                };
                match outcome {
                    Outcome::Action(name) => {
                        let then_id = format!("{owner_id}#then{branch_index}");
                        ctx.model.elements.push(Element { id: then_id.clone(), name: name.clone(), kind: ElementKind::Task(TaskKind::None), container: owner_container.clone(), attached_to: None, parent: owner_parent.clone() });
                        ctx.add_flow(&gw_id, &then_id, &label, FlowKind::Sequence { is_default });
                    }
                    Outcome::Throw(error_name) => {
                        let throw_id = format!("{owner_id}#throw{branch_index}");
                        ctx.model.elements.push(Element { id: throw_id.clone(), name: error_name.clone(), kind: ElementKind::Event { position: EventPosition::End, trigger: Some(EventTrigger::Error) }, container: owner_container.clone(), attached_to: None, parent: owner_parent.clone() });
                        ctx.add_flow(&gw_id, &throw_id, &label, FlowKind::Sequence { is_default });
                        *error_counter += 1;
                        let boundary_id = format!("{l2_id}#error{error_counter}");
                        ctx.model.elements.push(Element { id: boundary_id, name: error_name.clone(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: l2_container.clone(), attached_to: Some(l2_id.to_string()), parent: None });
                    }
                }
            }
        }
    }

    if !notes.is_empty() {
        let note_id = format!("{owner_id}#note");
        ctx.model.elements.push(Element { id: note_id.clone(), name: notes.join("\n"), kind: ElementKind::TextAnnotation, container: owner_container.clone(), attached_to: None, parent: owner_parent.clone() });
        ctx.add_flow(owner_id, &note_id, "", FlowKind::Association);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::bpmn::model::Orientation;

    fn only_model(source: &str) -> Model {
        let parsed = parse(source).unwrap_or_else(|e| panic!("파싱 실패: {e}"));
        assert_eq!(parsed.models.len(), 1);
        parsed.models.into_iter().next().unwrap()
    }

    fn find<'a>(model: &'a Model, id: &str) -> &'a Element {
        model.element(id).unwrap_or_else(|| panic!("요소 {id}가 있어야 한다: {:#?}", model.elements.iter().map(|e| &e.id).collect::<Vec<_>>()))
    }

    // ── 3.7~3.10: 참여자 상속·레인·풀 ──────────────────────────────────

    #[test]
    fn no_tags_anywhere_means_no_participants_and_no_containers() {
        let model = only_model("## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n");
        assert!(model.participants.is_empty());
        assert!(model.elements.iter().all(|e| e.container.is_none()));
    }

    #[test]
    fn a_tag_only_on_l1_is_inherited_by_everything_as_a_single_lane() {
        let model = only_model("## L1 Process: A (participant: 학습자)\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\n");
        assert_eq!(model.participants.len(), 1);
        assert_eq!(model.participants[0].id, "pool");
        assert_eq!(model.participants[0].lanes.len(), 1);
        assert_eq!(model.participants[0].lanes[0].id, "lane:학습자");
        for id in ["#start", "#end", "a1", "a1#start", "a1#end", "a1.s1", "a1.s1.d1"] {
            assert_eq!(find(&model, id).container.as_deref(), Some("lane:학습자"), "{id}");
        }
    }

    #[test]
    fn a_different_tag_on_an_l4_only_reassigns_that_l4_and_its_l5_descendants() {
        let model = only_model(concat!(
            "## L1 Process: A (participant: 담당자)\n",
            "### L2 Activity: B\n",
            "    ### L4 Step: C\n",
            "      ### L5 DetailStep: D\n",
            "    ### L4 Step: E (participant: 승인자)\n",
            "      ### L5 DetailStep: F\n",
        ));
        assert_eq!(find(&model, "a1").container.as_deref(), Some("lane:담당자"));
        assert_eq!(find(&model, "a1.s1").container.as_deref(), Some("lane:담당자"));
        assert_eq!(find(&model, "a1.s1.d1").container.as_deref(), Some("lane:담당자"));
        assert_eq!(find(&model, "a1.s2").container.as_deref(), Some("lane:승인자"));
        assert_eq!(find(&model, "a1.s2.d1").container.as_deref(), Some("lane:승인자"));
    }

    #[test]
    fn a_role_reappearing_gets_one_lane_in_first_appearance_order() {
        let model = only_model(concat!(
            "## L1 Process: A\n",
            "### L2 Activity: B (participant: 승인자)\n",
            "    ### L4 Step: C (participant: 담당자)\n",
            "### L2 Activity: D (participant: 승인자)\n",
            "    ### L4 Step: E\n",
        ));
        let lanes: Vec<&str> = model.participants[0].lanes.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(lanes, vec!["승인자", "담당자"]);
    }

    #[test]
    fn pool_name_is_the_l1_name() {
        let model = only_model("## L1 Process: 주문 처리 (participant: 담당자)\n### L2 Activity: B\n");
        assert_eq!(model.participants[0].name, "주문 처리");
        assert_eq!(model.title, "주문 처리");
    }

    // ── 4.1~4.5: 드릴다운 매핑 ──────────────────────────────────────────

    #[test]
    fn l2_becomes_a_subprocess_with_its_own_start_and_end_sequenced_from_the_root() {
        let model = only_model("## L1 Process: A\n### L2 Activity: B\n");
        assert_eq!(find(&model, "a1").kind, ElementKind::Subprocess);
        let seq = |s: &str, t: &str| model.flows.iter().any(|f| f.source == s && f.target == t && matches!(f.kind, FlowKind::Sequence { is_default: false }));
        assert!(seq("#start", "a1"));
        assert!(seq("a1", "#end"));
        assert!(seq("a1#start", "a1#end"), "L4 없는 L2는 내부 시작→종료가 직접 이어져야 한다");
    }

    #[test]
    fn a_single_l3_becomes_the_l2_subtitle_without_a_group() {
        let model = only_model("## L1 Process: A\n### L2 Activity: B\n  ### L3 FunctionGroup/UI: C\n    ### L4 Step: D\n");
        assert_eq!(find(&model, "a1").name, "B\nC");
        assert!(model.groups.is_empty());
    }

    #[test]
    fn two_or_more_l3s_become_groups_with_their_own_l4s_as_members() {
        let model = only_model(concat!(
            "## L1 Process: A\n",
            "### L2 Activity: B\n",
            "  ### L3 FunctionGroup/UI: C\n",
            "    ### L4 Step: D\n",
            "  ### L3 FunctionGroup/UI: E\n",
            "    ### L4 Step: F\n",
        ));
        assert_eq!(find(&model, "a1").name, "B");
        assert_eq!(model.groups.len(), 2);
        assert_eq!(model.groups[0].name, "C");
        assert_eq!(model.groups[0].members, vec!["a1.s1".to_string()]);
        assert_eq!(model.groups[0].parent.as_deref(), Some("a1"));
        assert_eq!(model.groups[1].name, "E");
        assert_eq!(model.groups[1].members, vec!["a1.s2".to_string()]);
    }

    #[test]
    fn l4_with_l5_children_is_a_subprocess_without_l5_is_a_task() {
        let model = only_model(concat!(
            "## L1 Process: A\n",
            "### L2 Activity: B\n",
            "    ### L4 Step: C\n",
            "      ### L5 DetailStep: D\n",
            "    ### L4 Step: E\n",
        ));
        assert_eq!(find(&model, "a1.s1").kind, ElementKind::Subprocess);
        assert_eq!(find(&model, "a1.s2").kind, ElementKind::Task(TaskKind::None));
    }

    #[test]
    fn siblings_are_sequenced_in_document_order_with_no_join_gateway() {
        let model = only_model(concat!(
            "## L1 Process: A\n",
            "### L2 Activity: B\n",
            "    ### L4 Step: C\n",
            "      ### L5 DetailStep: D\n",
            "      ### L5 DetailStep: E\n",
            "    ### L4 Step: F\n",
        ));
        let seq = |s: &str, t: &str| model.flows.iter().any(|f| f.source == s && f.target == t);
        assert!(seq("a1.s1", "a1.s2"));
        assert!(seq("a1.s1.d1", "a1.s1.d2"));
    }

    #[test]
    fn an_l2_directly_over_an_l4_and_an_l2_with_no_children_do_not_panic() {
        let model = only_model("## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n### L2 Activity: 빈 것\n");
        assert!(find(&model, "a1.s1").kind == ElementKind::Task(TaskKind::None));
        assert_eq!(find(&model, "a2").kind, ElementKind::Subprocess);
    }

    #[test]
    fn two_l1_headings_produce_two_models_with_their_own_titles() {
        let parsed = parse("## L1 Process: A\n### L2 Activity: X\n## L1 Process: B\n### L2 Activity: Y\n").unwrap();
        assert_eq!(parsed.models.len(), 2);
        assert_eq!(parsed.models[0].title, "A");
        assert_eq!(parsed.models[1].title, "B");
    }

    #[test]
    fn all_ids_in_a_model_are_unique() {
        let model = only_model(crate::diagram::bpmn::fixtures::TAGGED_PROCESS_MD);
        let mut ids: Vec<&str> = model.elements.iter().map(|e| e.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "요소 id가 모두 달라야 한다");
        let mut flow_ids: Vec<&str> = model.flows.iter().map(|f| f.id.as_str()).collect();
        let before = flow_ids.len();
        flow_ids.sort_unstable();
        flow_ids.dedup();
        assert_eq!(flow_ids.len(), before, "흐름 id가 모두 달라야 한다");
    }

    // ── 5.1~5.8: Logic → 게이트웨이 ──────────────────────────────────

    #[test]
    fn if_and_else_if_become_one_gateway_with_two_labeled_flows() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - IF 조건1 THEN 결과1\n        - ELSE IF 조건2 THEN 결과2\n",
        ));
        let owner = "a1.s1.d1";
        assert_eq!(find(&model, &format!("{owner}#gw")).kind, ElementKind::Gateway(GatewayKind::Exclusive));
        let gw_edges: Vec<&Flow> = model.flows.iter().filter(|f| f.source == format!("{owner}#gw")).collect();
        assert_eq!(gw_edges.len(), 2);
        assert!(gw_edges.iter().any(|f| f.label == "조건1" && f.target == format!("{owner}#then1")));
        assert!(gw_edges.iter().any(|f| f.label == "조건2" && f.target == format!("{owner}#then2")));
        assert_eq!(find(&model, &format!("{owner}#then1")).name, "결과1");
    }

    #[test]
    fn else_becomes_a_default_flow_with_or_without_a_memo_label() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - IF 조건 THEN 결과\n        - ELSE 기본결과\n",
        ));
        let owner = "a1.s1.d1";
        let default_flow = model.flows.iter().find(|f| f.source == format!("{owner}#gw") && matches!(f.kind, FlowKind::Sequence { is_default: true })).unwrap();
        assert_eq!(default_flow.target, format!("{owner}#then2"));
        assert!(default_flow.label.is_empty());

        let model2 = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - IF 조건 THEN 결과\n        - ELSE (표준입력이거나 파일 미지정) THEN 기본결과\n",
        ));
        let default_flow2 = model2.flows.iter().find(|f| f.source == format!("{owner}#gw") && matches!(f.kind, FlowKind::Sequence { is_default: true })).unwrap();
        assert_eq!(default_flow2.label, "표준입력이거나 파일 미지정");
    }

    #[test]
    fn throw_creates_an_error_end_event_and_a_boundary_error_event_on_the_enclosing_l2() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - ELSE THROW 문제발생\n",
        ));
        let throw = find(&model, "a1.s1.d1#throw1");
        assert_eq!(throw.kind, ElementKind::Event { position: EventPosition::End, trigger: Some(EventTrigger::Error) });
        assert_eq!(throw.name, "문제발생");
        let boundary = find(&model, "a1#error1");
        assert_eq!(boundary.kind, ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) });
        assert_eq!(boundary.attached_to.as_deref(), Some("a1"));
        assert_eq!(boundary.parent, None);
        assert!(!model.elements.iter().any(|e| e.id == "a1.s1#error1"), "경계 이벤트는 L4가 아니라 L2에만 붙는다");
    }

    #[test]
    fn items_without_if_become_one_multiline_text_annotation_linked_by_association() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - 항상: 첫째 줄\n        - 둘째 줄 설명\n",
        ));
        let note = find(&model, "a1.s1.d1#note");
        assert_eq!(note.kind, ElementKind::TextAnnotation);
        assert_eq!(note.name, "항상: 첫째 줄\n둘째 줄 설명");
        assert!(model.flows.iter().any(|f| f.source == "a1.s1.d1" && f.target == "a1.s1.d1#note" && matches!(f.kind, FlowKind::Association)));
    }

    #[test]
    fn the_next_l5_sibling_after_a_gated_l5_connects_directly_from_the_owning_task() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n",
            "      ### L5 DetailStep: D\nLogic(AST):\n        - IF 조건 THEN 결과\n",
            "      ### L5 DetailStep: E\n",
        ));
        assert!(model.flows.iter().any(|f| f.source == "a1.s1.d1" && f.target == "a1.s1.d2"));
        assert!(!model.elements.iter().any(|e| e.id.contains("join")));
    }

    #[test]
    fn an_if_without_then_still_renders_as_a_note_via_the_outline_diagnostic() {
        let parsed = parse(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - IF 조건만 있음\n",
        ))
        .unwrap();
        assert!(!parsed.diagnostics.is_empty());
        let model = &parsed.models[0];
        assert!(model.element("a1.s1.d1#note").is_some());
    }

    #[test]
    fn trailing_id_parens_are_stripped_from_condition_and_outcome_text() {
        let model = only_model(concat!(
            "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: C\n      ### L5 DetailStep: D\nLogic(AST):\n",
            "        - IF 조건 THEN 결과 (6.1)\n",
        ));
        assert_eq!(find(&model, "a1.s1.d1#then1").name, "결과");
    }

    #[test]
    fn a_transition_fixture_leaves_the_model_orientation_at_its_default() {
        // orientation은 이 스펙이 다루지 않는다(항상 기본값) — 회귀 확인만.
        let model = only_model("## L1 Process: A\n### L2 Activity: B\n");
        assert_eq!(model.orientation, Orientation::Horizontal);
    }
}

#[cfg(test)]
mod sanity_validate_real_docs {
    use super::*;
    use crate::diagram::bpmn::validate;

    #[test]
    fn all_real_docs_and_synthetic_fixtures_produce_valid_models() {
        let mut sources: Vec<(&str, &str)> = crate::diagram::bpmn::fixtures::BIZPROCESS_REAL.to_vec();
        sources.push(("tagged_process", crate::diagram::bpmn::fixtures::TAGGED_PROCESS_MD));
        sources.push(("transition_in_step", crate::diagram::bpmn::fixtures::TRANSITION_IN_STEP_MD));
        for (name, source) in sources {
            let parsed = parse(source).unwrap_or_else(|e| panic!("{name}: 파싱 실패: {e}"));
            for model in &parsed.models {
                if let Err(errors) = validate::validate(model) {
                    panic!("{name}: 검증 실패: {errors:?}\n모델: {model:#?}");
                }
            }
        }
    }

    #[test]
    fn transition_fixture_is_flagged_by_participant_transitions() {
        let parsed = parse(crate::diagram::bpmn::fixtures::TRANSITION_IN_STEP_MD).unwrap();
        let model = &parsed.models[0];
        let transitions = validate::participant_transitions(model);
        assert!(transitions.contains(&"a1.s1".to_string()), "transitions={transitions:?}");
    }
}
