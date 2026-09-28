//! 종류 ↔ BPMN XML 로컬 이름 토큰 ↔ 도형 라벨의 단일 출처.
//!
//! 종류별로 튜플 표 하나씩 두고, `xml_name()`·`from_xml_name()`·`label()`이 모두 같은 표를
//! 읽는다 — `match`가 둘로 갈라지면 토큰과 라벨이 어긋난다.

use super::model::{ElementKind, EventPosition, EventTrigger, GatewayKind, TaskKind};

pub const CALL_ACTIVITY_LABEL: &str = "«call»";
pub const DATA_OBJECT_LABEL: &str = "«data»";

/// `startEvent` … `textAnnotation`(태스크·게이트웨이 제외 12행) — 이벤트는 항상 `trigger: None`으로
/// 두고 호출자(XML은 `event_trigger_of`, YAML은 `trigger:` 필드)가 채운다.
const ELEMENT_KIND_TABLE: &[(&str, ElementKind)] = &[
    ("startEvent", ElementKind::Event { position: EventPosition::Start, trigger: None }),
    ("intermediateCatchEvent", ElementKind::Event { position: EventPosition::Intermediate, trigger: None }),
    ("intermediateThrowEvent", ElementKind::Event { position: EventPosition::Intermediate, trigger: None }),
    ("boundaryEvent", ElementKind::Event { position: EventPosition::Intermediate, trigger: None }),
    ("endEvent", ElementKind::Event { position: EventPosition::End, trigger: None }),
    ("subProcess", ElementKind::Subprocess),
    ("adHocSubProcess", ElementKind::Subprocess),
    ("transaction", ElementKind::Subprocess),
    ("callActivity", ElementKind::CallActivity),
    ("dataObjectReference", ElementKind::DataObject),
    ("dataStoreReference", ElementKind::DataStore),
    ("textAnnotation", ElementKind::TextAnnotation),
];

/// 로컬 요소명 → 종류. 이벤트는 `trigger: None`으로 돌려주고 호출자가 채운다. `boundaryEvent`는
/// `Intermediate`. 표에 없으면 태스크·게이트웨이 표를 차례로 본다. 대소문자 그대로 정확 일치만.
pub fn element_kind_from_xml_name(token: &str) -> Option<ElementKind> {
    if let Some((_, kind)) = ELEMENT_KIND_TABLE.iter().find(|(name, _)| *name == token) {
        return Some(*kind);
    }
    if let Some(kind) = TaskKind::from_xml_name(token) {
        return Some(ElementKind::Task(kind));
    }
    if let Some(kind) = GatewayKind::from_xml_name(token) {
        return Some(ElementKind::Gateway(kind));
    }
    None
}

const TASK_KIND_TABLE: &[(TaskKind, &str, Option<&str>)] = &[
    (TaskKind::None, "task", None),
    (TaskKind::User, "userTask", Some("«user»")),
    (TaskKind::Service, "serviceTask", Some("«service»")),
    (TaskKind::Script, "scriptTask", Some("«script»")),
    (TaskKind::Manual, "manualTask", Some("«manual»")),
    (TaskKind::BusinessRule, "businessRuleTask", Some("«businessRule»")),
    (TaskKind::Send, "sendTask", Some("«send»")),
    (TaskKind::Receive, "receiveTask", Some("«receive»")),
];

const EVENT_TRIGGER_TABLE: &[(EventTrigger, &str, &str)] = &[
    (EventTrigger::Message, "messageEventDefinition", "«message»"),
    (EventTrigger::Timer, "timerEventDefinition", "«timer»"),
    (EventTrigger::Error, "errorEventDefinition", "«error»"),
    (EventTrigger::Escalation, "escalationEventDefinition", "«escalation»"),
    (EventTrigger::Cancel, "cancelEventDefinition", "«cancel»"),
    (EventTrigger::Compensation, "compensationEventDefinition", "«compensation»"),
    (EventTrigger::Conditional, "conditionalEventDefinition", "«conditional»"),
    (EventTrigger::Link, "linkEventDefinition", "«link»"),
    (EventTrigger::Signal, "signalEventDefinition", "«signal»"),
    (EventTrigger::Terminate, "terminateEventDefinition", "«terminate»"),
    (EventTrigger::Multiple, "multipleEventDefinition", "«multiple»"),
    (EventTrigger::ParallelMultiple, "parallelMultipleEventDefinition", "«parallelMultiple»"),
];

const GATEWAY_KIND_TABLE: &[(GatewayKind, &str, &str)] = &[
    (GatewayKind::Exclusive, "exclusiveGateway", "×"),
    (GatewayKind::Parallel, "parallelGateway", "+"),
    (GatewayKind::Inclusive, "inclusiveGateway", "○"),
    (GatewayKind::Complex, "complexGateway", "*"),
    (GatewayKind::EventBased, "eventBasedGateway", "◎"),
];

impl TaskKind {
    pub fn xml_name(self) -> &'static str {
        TASK_KIND_TABLE.iter().find(|(kind, _, _)| *kind == self).map(|(_, name, _)| *name).expect("모든 TaskKind는 표에 있다")
    }

    pub fn from_xml_name(token: &str) -> Option<Self> {
        TASK_KIND_TABLE.iter().find(|(_, name, _)| *name == token).map(|(kind, _, _)| *kind)
    }

    pub fn label(self) -> Option<&'static str> {
        TASK_KIND_TABLE.iter().find(|(kind, _, _)| *kind == self).and_then(|(_, _, label)| *label)
    }
}

impl EventTrigger {
    pub fn xml_name(self) -> &'static str {
        EVENT_TRIGGER_TABLE.iter().find(|(kind, _, _)| *kind == self).map(|(_, name, _)| *name).expect("모든 EventTrigger는 표에 있다")
    }

    pub fn from_xml_name(token: &str) -> Option<Self> {
        EVENT_TRIGGER_TABLE.iter().find(|(_, name, _)| *name == token).map(|(kind, _, _)| *kind)
    }

    pub fn label(self) -> &'static str {
        EVENT_TRIGGER_TABLE.iter().find(|(kind, _, _)| *kind == self).map(|(_, _, label)| *label).expect("모든 EventTrigger는 표에 있다")
    }
}

impl GatewayKind {
    pub fn xml_name(self) -> &'static str {
        GATEWAY_KIND_TABLE.iter().find(|(kind, _, _)| *kind == self).map(|(_, name, _)| *name).expect("모든 GatewayKind는 표에 있다")
    }

    pub fn from_xml_name(token: &str) -> Option<Self> {
        GATEWAY_KIND_TABLE.iter().find(|(_, name, _)| *name == token).map(|(kind, _, _)| *kind)
    }

    pub fn label(self) -> &'static str {
        GATEWAY_KIND_TABLE.iter().find(|(kind, _, _)| *kind == self).map(|(_, _, label)| *label).expect("모든 GatewayKind는 표에 있다")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_kind_tokens_round_trip_and_label_matches_contract() {
        for &(kind, token, label) in TASK_KIND_TABLE {
            assert_eq!(TaskKind::from_xml_name(token), Some(kind));
            assert_eq!(kind.xml_name(), token);
            assert_eq!(kind.label(), label);
        }
        assert_eq!(TaskKind::from_xml_name("UserTask"), None);
        assert_eq!(TaskKind::from_xml_name("foo"), None);
    }

    #[test]
    fn event_trigger_tokens_round_trip_and_label_matches_contract() {
        for &(kind, token, label) in EVENT_TRIGGER_TABLE {
            assert_eq!(EventTrigger::from_xml_name(token), Some(kind));
            assert_eq!(kind.xml_name(), token);
            assert_eq!(kind.label(), label);
        }
        assert_eq!(EventTrigger::from_xml_name("MessageEventDefinition"), None);
        assert_eq!(EventTrigger::from_xml_name("foo"), None);
    }

    #[test]
    fn gateway_kind_tokens_round_trip_and_label_matches_contract() {
        for &(kind, token, label) in GATEWAY_KIND_TABLE {
            assert_eq!(GatewayKind::from_xml_name(token), Some(kind));
            assert_eq!(kind.xml_name(), token);
            assert_eq!(kind.label(), label);
        }
        assert_eq!(GatewayKind::from_xml_name("ExclusiveGateway"), None);
        assert_eq!(GatewayKind::from_xml_name("foo"), None);
    }

    #[test]
    fn kindless_task_has_no_label() {
        assert_eq!(TaskKind::None.label(), None);
    }

    #[test]
    fn element_kind_table_covers_its_twelve_rows_plus_task_and_gateway_lookup() {
        for &(token, kind) in ELEMENT_KIND_TABLE {
            assert_eq!(element_kind_from_xml_name(token), Some(kind), "{token}");
        }
        assert_eq!(ELEMENT_KIND_TABLE.len(), 12);
        assert_eq!(element_kind_from_xml_name("task"), Some(ElementKind::Task(TaskKind::None)));
        assert_eq!(element_kind_from_xml_name("userTask"), Some(ElementKind::Task(TaskKind::User)));
        assert_eq!(element_kind_from_xml_name("exclusiveGateway"), Some(ElementKind::Gateway(GatewayKind::Exclusive)));
    }

    #[test]
    fn boundary_event_is_intermediate_and_unknown_tokens_are_none() {
        assert_eq!(element_kind_from_xml_name("boundaryEvent"), Some(ElementKind::Event { position: EventPosition::Intermediate, trigger: None }));
        assert_eq!(element_kind_from_xml_name("StartEvent"), None);
        assert_eq!(element_kind_from_xml_name("foo"), None);
    }
}
