//! YAML 값 트리(`super::yaml`) → 검증 전 `Model`.
//!
//! 최상위 키·참여자/레인 중첩(소속)·노드 두 표기·종류/트리거 토큰·경계 부착·`default`·흐름 한 줄
//! 표기를 스키마대로 걷는다. 검증은 하지 않는다(`render_model`이 `validate`를 부른다). 어휘는
//! `super::vocabulary`만 부른다 — BPMN 이름 표를 여기 두지 않는다.

use super::model::{Element, ElementKind, EventTrigger, Flow, FlowKind, Lane, Model, Orientation, Participant};
use super::vocabulary;
use super::yaml::{self, YamlError, YamlValue};
use crate::diagram::ir::{LineKind, Marker};
use crate::diagram::mermaid::flow::{Link, read_link};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Yaml(YamlError),
    /// 값의 모양이 스키마와 다름. `path`는 `participants[1].lanes[0].nodes[2]` 꼴, `expected`는
    /// "매핑"·"시퀀스"·"스칼라"·"단일 키 매핑"·"문자열 항목".
    UnexpectedShape { path: String, expected: &'static str },
    UnknownKey { path: String, key: String },
    /// 확장형의 `kind`, `boundaryEvent`의 `attached_to`.
    MissingKey { path: String, key: &'static str },
    /// 이벤트 아닌 노드의 `trigger`, `boundaryEvent` 아닌 노드의 `attached_to`.
    FieldNotAllowed { path: String, key: &'static str },
    UnknownKind { path: String, token: String },
    UnknownTrigger { path: String, token: String },
    UnknownOrientation(String),
    DefaultFlowMissing { node: String, target: String },
    /// 항목 원문: 화살 없음·공백 없음·남는 글.
    MalformedFlow(String),
    /// 세 꼴 밖 화살.
    UnsupportedFlow(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Yaml(err) => write!(f, "{err}"),
            ParseError::UnexpectedShape { path, expected } => write!(f, "{path}은(는) {expected}이어야 한다"),
            ParseError::UnknownKey { path, key } => write!(f, "{path}에 스키마 밖 키 {key:?}가 있다"),
            ParseError::MissingKey { path, key } => write!(f, "{path}에 필수 키 {key:?}가 없다"),
            ParseError::FieldNotAllowed { path, key } => write!(f, "{path}에는 {key:?}를 쓸 수 없다"),
            ParseError::UnknownKind { path, token } => write!(f, "{path}의 종류 토큰 {token:?}를 모른다"),
            ParseError::UnknownTrigger { path, token } => write!(f, "{path}의 트리거 토큰 {token:?}를 모른다"),
            ParseError::UnknownOrientation(value) => write!(f, "방향 {value:?}를 모른다"),
            ParseError::DefaultFlowMissing { node, target } => write!(f, "{node}에서 {target}로 가는 시퀀스 흐름이 없어 default를 걸 수 없다"),
            ParseError::MalformedFlow(text) => write!(f, "흐름 항목 형식이 올바르지 않다: {text:?}"),
            ParseError::UnsupportedFlow(text) => write!(f, "지원하지 않는 화살 표기다: {text:?}"),
        }
    }
}

/// 스니핑·최상위 검사의 단일 출처.
pub const TOP_LEVEL_KEYS: &[&str] = &["title", "orientation", "participants", "nodes", "flows"];

const PARTICIPANT_OR_LANE_KEYS: &[&str] = &["name", "lanes", "nodes"];
const NODE_KEYS: &[&str] = &["kind", "name", "trigger", "attached_to", "default"];

/// 열 0 내용 줄(주석·빈 줄·선두 `---` 제외)이 전부 `TOP_LEVEL_KEYS`의 `키:` 줄이고
/// `participants`·`nodes`·`flows` 중 하나 이상이 있으면 참. 문서를 파싱하지 않는다.
pub fn looks_like_bpmn_yaml(source: &str) -> bool {
    let mut seen_content = false;
    let mut has_structural_key = false;
    for raw in source.lines() {
        let Some(content) = column_zero_content(raw) else { continue };
        if !seen_content {
            seen_content = true;
            if content == "---" {
                continue;
            }
        }
        let Some(key) = top_level_key_of(content) else { return false };
        if matches!(key, "participants" | "nodes" | "flows") {
            has_structural_key = true;
        }
    }
    has_structural_key
}

/// 들여쓰기 없고(공백·탭으로 시작하지 않고) 주석·트레일링 공백을 지운 뒤에도 내용이 남는 줄만.
/// `bpmn::yaml`의 완전한 트리 빌더는 부르지 않는다(스니핑 전용, 문서를 파싱하지 않는다).
fn column_zero_content(raw: &str) -> Option<&str> {
    if raw.starts_with(' ') || raw.starts_with('\t') {
        return None;
    }
    let stripped = strip_comment_loosely(raw);
    let trimmed = stripped.trim_end();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

/// 따옴표 밖의, 줄 시작 또는 공백 뒤에 오는 첫 `#`부터 잘라낸다(홑따옴표 이스케이프까지는 안 본다 —
/// 스니핑은 열 0의 `키:` 모양만 보면 충분하다).
fn strip_comment_loosely(raw: &str) -> &str {
    let mut in_quote: Option<char> = None;
    let mut prev_is_space = true;
    for (i, c) in raw.char_indices() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    in_quote = Some(c);
                } else if c == '#' && prev_is_space {
                    return &raw[..i];
                }
            }
        }
        prev_is_space = c.is_whitespace();
    }
    raw
}

fn top_level_key_of(content: &str) -> Option<&'static str> {
    TOP_LEVEL_KEYS.iter().copied().find(|key| content.strip_prefix(key).and_then(|rest| rest.strip_prefix(':')).is_some())
}

/// 문서 → 검증 전 `Model`.
pub fn parse(source: &str) -> Result<Model, ParseError> {
    let root = yaml::parse_document(source).map_err(ParseError::Yaml)?;
    build_model(&root)
}

/// 참여자/레인 중첩을 걷는 동안 모은, 아직 해석하지 않은 노드 항목 하나.
struct RawNode {
    id: String,
    path: String,
    container: Option<String>,
    value: YamlValue,
}

fn build_model(root: &YamlValue) -> Result<Model, ParseError> {
    let root_entries = root.as_map().ok_or_else(|| ParseError::UnexpectedShape { path: "$".into(), expected: "매핑" })?;
    check_allowed_keys(root_entries, "$", TOP_LEVEL_KEYS)?;

    let title = optional_scalar_field(root, "$", "title")?.map(|s| s.trim().to_string()).unwrap_or_default();
    let orientation = match optional_scalar_field(root, "$", "orientation")? {
        None => Orientation::default(),
        Some(token) => match token.as_str() {
            "horizontal" => Orientation::Horizontal,
            "vertical" => Orientation::Vertical,
            _ => return Err(ParseError::UnknownOrientation(token)),
        },
    };

    let mut raw_nodes: Vec<RawNode> = Vec::new();
    let participants = match root.get("participants") {
        Some(value) => parse_container_seq(value, "participants", &mut raw_nodes)?.into_iter().map(|c| Participant { id: c.id, name: c.name, lanes: c.children }).collect(),
        None => Vec::new(),
    };
    if let Some(nodes_value) = root.get("nodes") {
        collect_raw_nodes(nodes_value, "nodes", None, &mut raw_nodes)?;
    }

    let mut default_pairs: Vec<(String, String)> = Vec::new();
    let mut elements: Vec<Element> = raw_nodes.iter().map(|raw| build_element(raw, &mut default_pairs)).collect::<Result<_, _>>()?;
    apply_boundary_containers(&mut elements);

    let mut flows: Vec<Flow> = Vec::new();
    if let Some(flows_value) = root.get("flows") {
        let items = flows_value.as_seq().ok_or_else(|| ParseError::UnexpectedShape { path: "flows".into(), expected: "시퀀스" })?;
        for (i, item) in items.iter().enumerate() {
            let item_path = format!("flows[{i}]");
            let text = item.as_scalar().ok_or_else(|| ParseError::UnexpectedShape { path: item_path.clone(), expected: "문자열 항목" })?;
            let id = format!("@bpmn-yaml:{i}");
            flows.push(parse_flow_line(text, id, &elements)?);
        }
    }
    apply_default_flows(&mut flows, default_pairs)?;

    Ok(Model { title, orientation, participants, elements, flows })
}

fn check_allowed_keys(entries: &[(String, YamlValue)], path: &str, allowed: &[&str]) -> Result<(), ParseError> {
    for (key, _) in entries {
        if !allowed.contains(&key.as_str()) {
            return Err(ParseError::UnknownKey { path: path.to_string(), key: key.clone() });
        }
    }
    Ok(())
}

fn optional_scalar_field(value: &YamlValue, path: &str, key: &str) -> Result<Option<String>, ParseError> {
    match value.get(key) {
        None => Ok(None),
        Some(v) => match v.as_scalar() {
            Some(s) => Ok(Some(s.to_string())),
            None => Err(ParseError::UnexpectedShape { path: format!("{path}.{key}"), expected: "스칼라" }),
        },
    }
}

fn single_key_mapping<'a>(value: &'a YamlValue, path: &str) -> Result<(String, &'a YamlValue), ParseError> {
    match value.as_map() {
        Some([(key, body)]) => Ok((key.clone(), body)),
        _ => Err(ParseError::UnexpectedShape { path: path.to_string(), expected: "단일 키 매핑" }),
    }
}

// --- 참여자 · 레인(같은 모양의 재귀) ---

struct ContainerNode {
    id: String,
    name: String,
    children: Vec<Lane>,
}

fn parse_container_seq(value: &YamlValue, path: &str, raw_nodes: &mut Vec<RawNode>) -> Result<Vec<ContainerNode>, ParseError> {
    let items = value.as_seq().ok_or_else(|| ParseError::UnexpectedShape { path: path.to_string(), expected: "시퀀스" })?;
    items.iter().enumerate().map(|(i, item)| parse_container_item(item, &format!("{path}[{i}]"), raw_nodes)).collect()
}

/// 참여자 항목과 레인 항목은 같은 모양이다: 단일 키 매핑(키 = id), 값이 스칼라면 이름만 있는
/// 컨테이너, 매핑이면 키 ⊂ {`name`, `lanes`, `nodes`}.
fn parse_container_item(item: &YamlValue, path: &str, raw_nodes: &mut Vec<RawNode>) -> Result<ContainerNode, ParseError> {
    let (id, body) = single_key_mapping(item, path)?;
    match body {
        YamlValue::Scalar(name) => Ok(ContainerNode { id, name: name.clone(), children: Vec::new() }),
        YamlValue::Map(entries) => {
            check_allowed_keys(entries, path, PARTICIPANT_OR_LANE_KEYS)?;
            let name = optional_scalar_field(body, path, "name")?.unwrap_or_default();
            let children = match body.get("lanes") {
                Some(lanes_value) => parse_container_seq(lanes_value, &format!("{path}.lanes"), raw_nodes)?.into_iter().map(|c| Lane { id: c.id, name: c.name, sub_lanes: c.children }).collect(),
                None => Vec::new(),
            };
            if let Some(nodes_value) = body.get("nodes") {
                collect_raw_nodes(nodes_value, &format!("{path}.nodes"), Some(id.clone()), raw_nodes)?;
            }
            Ok(ContainerNode { id, name, children })
        }
        YamlValue::Seq(_) => Err(ParseError::UnexpectedShape { path: path.to_string(), expected: "매핑 또는 스칼라" }),
    }
}

fn collect_raw_nodes(value: &YamlValue, path: &str, container: Option<String>, raw_nodes: &mut Vec<RawNode>) -> Result<(), ParseError> {
    let items = value.as_seq().ok_or_else(|| ParseError::UnexpectedShape { path: path.to_string(), expected: "시퀀스" })?;
    for (i, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{i}]");
        let (id, body) = single_key_mapping(item, &item_path)?;
        raw_nodes.push(RawNode { id, path: item_path, container: container.clone(), value: body.clone() });
    }
    Ok(())
}

// --- 노드: 압축형 · 확장형 → Element ---

fn build_element(raw: &RawNode, default_pairs: &mut Vec<(String, String)>) -> Result<Element, ParseError> {
    let (kind_token, name, trigger_token, attached_to_field, default_target) = match &raw.value {
        YamlValue::Scalar(s) => match s.split_once(' ') {
            Some((token, rest)) => (token.to_string(), rest.trim().to_string(), None, None, None),
            None => (s.clone(), String::new(), None, None, None),
        },
        YamlValue::Map(entries) => {
            check_allowed_keys(entries, &raw.path, NODE_KEYS)?;
            let kind_token = optional_scalar_field(&raw.value, &raw.path, "kind")?.ok_or_else(|| ParseError::MissingKey { path: raw.path.clone(), key: "kind" })?;
            let name = optional_scalar_field(&raw.value, &raw.path, "name")?.unwrap_or_default();
            let trigger = optional_scalar_field(&raw.value, &raw.path, "trigger")?;
            let attached_to = optional_scalar_field(&raw.value, &raw.path, "attached_to")?;
            let default_target = optional_scalar_field(&raw.value, &raw.path, "default")?;
            (kind_token, name, trigger, attached_to, default_target)
        }
        YamlValue::Seq(_) => return Err(ParseError::UnexpectedShape { path: raw.path.clone(), expected: "매핑 또는 스칼라" }),
    };

    let kind = vocabulary::element_kind_from_xml_name(&kind_token).ok_or_else(|| ParseError::UnknownKind { path: raw.path.clone(), token: kind_token.clone() })?;

    let kind = match trigger_token {
        None => kind,
        Some(trigger_token) => match kind {
            ElementKind::Event { position, .. } => {
                let trigger = EventTrigger::from_xml_name(&trigger_token).ok_or_else(|| ParseError::UnknownTrigger { path: raw.path.clone(), token: trigger_token.clone() })?;
                ElementKind::Event { position, trigger: Some(trigger) }
            }
            _ => return Err(ParseError::FieldNotAllowed { path: raw.path.clone(), key: "trigger" }),
        },
    };

    let attached_to = match (kind_token == "boundaryEvent", attached_to_field) {
        (true, Some(host)) => Some(host),
        (true, None) => return Err(ParseError::MissingKey { path: raw.path.clone(), key: "attached_to" }),
        (false, Some(_)) => return Err(ParseError::FieldNotAllowed { path: raw.path.clone(), key: "attached_to" }),
        (false, None) => None,
    };

    if let Some(target) = default_target {
        default_pairs.push((raw.id.clone(), target));
    }

    Ok(Element { id: raw.id.clone(), name, kind, container: raw.container.clone(), attached_to })
}

/// 경계 이벤트의 `container` := 호스트의 `container`(전부 만든 뒤, `parse_xml`과 같은 규칙).
fn apply_boundary_containers(elements: &mut [Element]) {
    let host_containers: HashMap<String, Option<String>> = elements.iter().map(|e| (e.id.clone(), e.container.clone())).collect();
    for element in elements.iter_mut() {
        if let Some(host_id) = element.attached_to.clone()
            && let Some(host_container) = host_containers.get(&host_id)
        {
            element.container = host_container.clone();
        }
    }
}

// --- 흐름: 한 줄 표기 → FlowKind ---

fn parse_flow_line(text: &str, id: String, elements: &[Element]) -> Result<Flow, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    let mut cursor = 0usize;

    let source_start = cursor;
    while cursor < chars.len() && !chars[cursor].is_whitespace() {
        cursor += 1;
    }
    if cursor == source_start {
        return Err(ParseError::MalformedFlow(text.to_string()));
    }
    let source: String = chars[source_start..cursor].iter().collect();

    let before_link = cursor;
    skip_spaces(&chars, &mut cursor);
    if cursor == before_link {
        return Err(ParseError::MalformedFlow(text.to_string()));
    }
    let Some(link) = read_link(&chars, &mut cursor) else {
        return Err(ParseError::MalformedFlow(text.to_string()));
    };

    let before_target = cursor;
    skip_spaces(&chars, &mut cursor);
    if cursor == before_target {
        return Err(ParseError::MalformedFlow(text.to_string()));
    }
    let target_start = cursor;
    while cursor < chars.len() && !chars[cursor].is_whitespace() {
        cursor += 1;
    }
    if cursor == target_start {
        return Err(ParseError::MalformedFlow(text.to_string()));
    }
    let target: String = chars[target_start..cursor].iter().collect();

    skip_spaces(&chars, &mut cursor);
    if cursor != chars.len() {
        return Err(ParseError::MalformedFlow(text.to_string()));
    }

    let kind = flow_kind_of(&link, &source, &target, elements, text)?;
    Ok(Flow { id, source, target, label: link.label, kind })
}

fn skip_spaces(chars: &[char], cursor: &mut usize) {
    while *cursor < chars.len() && chars[*cursor].is_whitespace() {
        *cursor += 1;
    }
}

/// design §Key Decisions의 `Link` → `FlowKind` 표.
fn flow_kind_of(link: &Link, source: &str, target: &str, elements: &[Element], raw_text: &str) -> Result<FlowKind, ParseError> {
    let is_data_end = |id: &str| elements.iter().any(|e| e.id == id && matches!(e.kind, ElementKind::DataObject | ElementKind::DataStore));
    match (link.kind, link.head, link.tail) {
        (LineKind::Solid, Marker::Arrow, Marker::None) => Ok(FlowKind::Sequence { is_default: false }),
        (LineKind::Dashed, Marker::Arrow, Marker::None) => {
            if is_data_end(source) || is_data_end(target) { Ok(FlowKind::DataAssociation) } else { Ok(FlowKind::Message) }
        }
        (LineKind::Dashed, Marker::None, Marker::None) => Ok(FlowKind::Association),
        _ => Err(ParseError::UnsupportedFlow(raw_text.to_string())),
    }
}

fn apply_default_flows(flows: &mut [Flow], default_pairs: Vec<(String, String)>) -> Result<(), ParseError> {
    for (node, target) in default_pairs {
        let flow = flows.iter_mut().find(|f| f.source == node && f.target == target && matches!(f.kind, FlowKind::Sequence { .. }));
        match flow {
            Some(flow) => flow.kind = FlowKind::Sequence { is_default: true },
            None => return Err(ParseError::DefaultFlowMissing { node, target }),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::bpmn::model::EventPosition;

    // --- 1.1, 1.3 looks_like_bpmn_yaml ---

    const ORDER_PROCESSING_LIKE: &str = "title: 주문 처리\nparticipants:\n  - customer: 고객\nnodes:\n  - a: startEvent\nflows:\n  - a --> a\n";

    #[test]
    fn documents_with_a_structural_top_level_key_and_nothing_else_sniff_true() {
        for source in [ORDER_PROCESSING_LIKE, "nodes:\n  - a: startEvent\nflows:\n  - a --> a\n", "# 머리말\n---\nnodes:\n  - a: startEvent\n"] {
            assert!(looks_like_bpmn_yaml(source), "{source:?}");
        }
    }

    #[test]
    fn documents_without_a_recognized_structural_key_sniff_false() {
        for source in [
            "process:\n  id: p1\n",
            "title: 문서만\n",
            "## L1 프로세스\n- 값사슬: 판매\n",
            "임의의 한글 문장입니다",
            "",
            r#"<bpmn:definitions></bpmn:definitions>"#,
            "class A",
            "flowchart LR\n A --> B\n",
        ] {
            assert!(!looks_like_bpmn_yaml(source), "{source:?}");
        }
    }

    // --- 3.1~3.9 구조 ---

    #[test]
    fn two_participants_with_direct_nodes_keep_order_name_and_container() {
        let source = "participants:\n  - customer: 고객\n  - vendor:\n      name: 판매사\n      nodes:\n        - a: startEvent\n";
        let model = parse(source).unwrap();
        assert_eq!(model.participants.iter().map(|p| (p.id.as_str(), p.name.as_str())).collect::<Vec<_>>(), vec![("customer", "고객"), ("vendor", "판매사")]);
        assert!(model.participants[0].lanes.is_empty());
        assert_eq!(model.element("a").unwrap().container.as_deref(), Some("vendor"));
    }

    #[test]
    fn scalar_participant_has_no_lanes_or_nodes() {
        let model = parse("participants:\n  - customer: 고객\nnodes:\n  - a: startEvent\n").unwrap();
        assert!(model.participants[0].lanes.is_empty());
        assert_eq!(model.element("a").unwrap().container, None);
    }

    #[test]
    fn nested_lanes_place_nodes_in_the_innermost_lane() {
        let source = "participants:\n  - vendor:\n      lanes:\n        - sales:\n            lanes:\n              - senior:\n                  nodes:\n                    - a: startEvent\n";
        let model = parse(source).unwrap();
        assert_eq!(model.participants[0].lanes[0].id, "sales");
        assert_eq!(model.participants[0].lanes[0].sub_lanes[0].id, "senior");
        assert_eq!(model.element("a").unwrap().container.as_deref(), Some("senior"));
    }

    #[test]
    fn participant_with_both_lanes_and_nodes_keeps_nodes_directly_in_the_pool() {
        let source = "participants:\n  - vendor:\n      lanes:\n        - sales: 영업\n      nodes:\n        - a: startEvent\n";
        let model = parse(source).unwrap();
        assert_eq!(model.element("a").unwrap().container.as_deref(), Some("vendor"));
    }

    #[test]
    fn top_level_nodes_without_participants_have_no_container() {
        let model = parse("nodes:\n  - a: startEvent\n").unwrap();
        assert!(model.participants.is_empty());
        assert_eq!(model.element("a").unwrap().container, None);
    }

    #[test]
    fn title_present_or_absent() {
        assert_eq!(parse("title: 주문 처리\nnodes:\n  - a: startEvent\n").unwrap().title, "주문 처리");
        assert_eq!(parse("nodes:\n  - a: startEvent\n").unwrap().title, "");
    }

    #[test]
    fn orientation_recognizes_horizontal_vertical_default_and_rejects_unknown() {
        assert_eq!(parse("orientation: horizontal\nnodes:\n  - a: startEvent\n").unwrap().orientation, Orientation::Horizontal);
        assert_eq!(parse("orientation: vertical\nnodes:\n  - a: startEvent\n").unwrap().orientation, Orientation::Vertical);
        assert_eq!(parse("nodes:\n  - a: startEvent\n").unwrap().orientation, Orientation::Horizontal);
        assert_eq!(parse("orientation: Horizontal\nnodes:\n  - a: startEvent\n").unwrap_err(), ParseError::UnknownOrientation("Horizontal".into()));
    }

    #[test]
    fn out_of_schema_keys_at_top_level_participant_and_lane_are_unknown_key() {
        assert!(matches!(parse("elements:\n  - a: startEvent\n"), Err(ParseError::UnknownKey { .. })));
        assert!(matches!(parse("participants:\n  - p1:\n      label: x\n"), Err(ParseError::UnknownKey { .. })));
        assert!(matches!(parse("participants:\n  - p1:\n      lanes:\n        - l1:\n            label: x\n"), Err(ParseError::UnknownKey { .. })));
    }

    #[test]
    fn a_participant_item_that_is_not_a_single_key_mapping_is_unexpected_shape() {
        assert!(matches!(parse("participants:\n  - a\n  - b\n"), Err(ParseError::UnexpectedShape { .. })));
    }

    // --- 4.1~4.9 노드 ---

    #[test]
    fn compact_form_with_and_without_a_name() {
        let model = parse("nodes:\n  - review: userTask 주문 검토\n  - start: startEvent\n").unwrap();
        assert_eq!(model.element("review").unwrap().name, "주문 검토");
        assert_eq!(model.element("review").unwrap().kind, ElementKind::Task(super::super::model::TaskKind::User));
        assert_eq!(model.element("start").unwrap().name, "");
    }

    #[test]
    fn expanded_form_reflects_every_field_and_requires_kind() {
        let model = parse("nodes:\n  - review:\n      kind: userTask\n      name: 주문 검토\n").unwrap();
        assert_eq!(model.element("review").unwrap().name, "주문 검토");
        assert!(matches!(parse("nodes:\n  - review:\n      name: 주문 검토\n"), Err(ParseError::MissingKey { key: "kind", .. })));
    }

    #[test]
    fn every_vocabulary_row_maps_to_the_same_kind_as_xml() {
        for token in ["startEvent", "intermediateCatchEvent", "intermediateThrowEvent", "endEvent", "task", "userTask", "serviceTask", "scriptTask", "manualTask", "businessRuleTask", "sendTask", "receiveTask", "subProcess", "adHocSubProcess", "transaction", "callActivity", "exclusiveGateway", "parallelGateway", "inclusiveGateway", "complexGateway", "eventBasedGateway", "dataObjectReference", "dataStoreReference", "textAnnotation"] {
            let source = format!("nodes:\n  - a: {token}\n");
            let model = parse(&source).unwrap_or_else(|e| panic!("{token} 파싱 실패: {e}"));
            assert_eq!(model.element("a").unwrap().kind, vocabulary::element_kind_from_xml_name(token).unwrap(), "{token}");
        }
    }

    #[test]
    fn trigger_is_allowed_only_on_events() {
        let model = parse("nodes:\n  - start:\n      kind: startEvent\n      trigger: messageEventDefinition\n").unwrap();
        assert_eq!(model.element("start").unwrap().kind, ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Message) });
        assert!(matches!(parse("nodes:\n  - a:\n      kind: task\n      trigger: messageEventDefinition\n"), Err(ParseError::FieldNotAllowed { key: "trigger", .. })));
    }

    #[test]
    fn boundary_event_requires_attached_to_and_inherits_the_hosts_container() {
        let source = "nodes:\n  - host: task 작업\n  - b:\n      kind: boundaryEvent\n      trigger: errorEventDefinition\n      attached_to: host\n";
        let model = parse(source).unwrap();
        assert_eq!(model.element("b").unwrap().attached_to.as_deref(), Some("host"));
        assert!(matches!(parse("nodes:\n  - b:\n      kind: boundaryEvent\n"), Err(ParseError::MissingKey { key: "attached_to", .. })));
        assert!(matches!(parse("nodes:\n  - b:\n      kind: task\n      attached_to: host\n"), Err(ParseError::FieldNotAllowed { key: "attached_to", .. })));
    }

    #[test]
    fn unknown_kind_or_trigger_tokens_are_rejected_case_sensitively() {
        assert!(matches!(parse("nodes:\n  - a: UserTask\n"), Err(ParseError::UnknownKind { .. })));
        assert!(matches!(parse("nodes:\n  - a: user\n"), Err(ParseError::UnknownKind { .. })));
        assert!(matches!(parse("nodes:\n  - a:\n      kind: startEvent\n      trigger: message\n"), Err(ParseError::UnknownTrigger { .. })));
    }

    // --- 5.1~5.10 흐름 ---

    fn two_task_model_source(flow_line: &str) -> String {
        format!("nodes:\n  - a: task\n  - b: task\nflows:\n  - {flow_line}\n")
    }

    #[test]
    fn plain_arrow_is_a_sequence_flow_without_a_label() {
        let model = parse(&two_task_model_source("a --> b")).unwrap();
        assert_eq!(model.flows[0].kind, FlowKind::Sequence { is_default: false });
        assert_eq!(model.flows[0].label, "");
    }

    #[test]
    fn both_label_forms_are_read() {
        for line in ["a -- 예 --> b", "a -->|예| b"] {
            let model = parse(&two_task_model_source(line)).unwrap();
            assert_eq!(model.flows[0].label, "예");
        }
    }

    #[test]
    fn dashed_arrow_between_flow_nodes_is_a_message_flow() {
        let model = parse(&two_task_model_source("a -.-> b")).unwrap();
        assert_eq!(model.flows[0].kind, FlowKind::Message);
    }

    #[test]
    fn dashed_arrow_touching_a_data_object_or_store_is_a_data_association() {
        let source = "nodes:\n  - a: task\n  - d: dataObjectReference\nflows:\n  - a -.-> d\n  - d -.-> a\n";
        let model = parse(source).unwrap();
        assert_eq!(model.flows[0].kind, FlowKind::DataAssociation);
        assert_eq!(model.flows[1].kind, FlowKind::DataAssociation);
    }

    #[test]
    fn dashed_line_without_a_head_is_an_association() {
        let model = parse(&two_task_model_source("a -.- b")).unwrap();
        assert_eq!(model.flows[0].kind, FlowKind::Association);
    }

    #[test]
    fn arrows_outside_the_three_supported_shapes_are_unsupported() {
        for line in ["a ==> b", "a <--> b", "a o--o b", "a --x b", "a --- b"] {
            assert!(matches!(parse(&two_task_model_source(line)), Err(ParseError::UnsupportedFlow(_))), "{line}");
        }
    }

    #[test]
    fn missing_or_misplaced_arrows_are_malformed() {
        for line in ["a b", "a-->b", "a --> b --> c", "a --> b 뒤글"] {
            assert!(matches!(parse(&two_task_model_source(line)), Err(ParseError::MalformedFlow(_))), "{line}");
        }
    }

    #[test]
    fn a_non_scalar_flow_item_is_unexpected_shape() {
        assert!(matches!(parse("nodes:\n  - a: task\nflows:\n  - source: a\n"), Err(ParseError::UnexpectedShape { .. })));
    }

    // --- 4.9 노드 항목이 단일 키 매핑이 아님 ---

    #[test]
    fn a_bare_scalar_node_item_is_unexpected_shape() {
        assert!(matches!(parse("nodes:\n  - startEvent\n"), Err(ParseError::UnexpectedShape { .. })));
    }

    #[test]
    fn a_two_key_node_item_is_unexpected_shape() {
        assert!(matches!(parse("nodes:\n  - a: x\n    b: y\n"), Err(ParseError::UnexpectedShape { .. })));
    }

    #[test]
    fn flow_ends_may_name_ids_that_do_not_exist_in_the_document() {
        let model = parse(&two_task_model_source("a --> ghost")).unwrap();
        assert_eq!(model.flows[0].target, "ghost");
    }

    #[test]
    fn default_marks_the_named_sequence_flow_and_errors_when_missing() {
        let source = "nodes:\n  - gw:\n      kind: exclusiveGateway\n      default: b\n  - a: task\n  - b: task\nflows:\n  - gw --> a\n  - gw --> b\n";
        let model = parse(source).unwrap();
        let is_default = |target: &str| model.flows.iter().find(|f| f.target == target).map(|f| matches!(f.kind, FlowKind::Sequence { is_default: true })).unwrap_or(false);
        assert!(!is_default("a"));
        assert!(is_default("b"));

        assert!(matches!(parse("nodes:\n  - gw:\n      kind: exclusiveGateway\n      default: ghost\n"), Err(ParseError::DefaultFlowMissing { .. })));
    }

    #[test]
    fn synthesized_flow_ids_are_distinct() {
        let model = parse("nodes:\n  - a: task\n  - b: task\nflows:\n  - a --> b\n  - b --> a\n").unwrap();
        assert_ne!(model.flows[0].id, model.flows[1].id);
    }
}
