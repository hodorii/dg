//! BPMN 2.0.2 프로세스 모델(의미 계층)의 공유 표현.
//!
//! XML·YAML·`biz-process.md` 세 파서가 만들어야 할 유일한 산출 형태. 다이어그램 교환 계층
//! (BPMNDI — 좌표·크기·waypoint)은 담지 않는다: 배치는 dg 자신의 배치기(`layout::graph`) 몫이다.
//! 이 모듈은 조회 도우미만 두고, 구조 규칙 검증은 `validate`, 도형·선 변환은 `lower`가 맡는다.

pub use crate::diagram::ir::EventPosition;

/// dg 배치 방향 힌트(mermaid `flowchart LR`과 같은 성격) — BPMNDI `isHorizontal`을 읽어 오는
/// 값이 아니다. 매핑은 `lower` 한 곳: `Horizontal → LeftRight`, `Vertical → TopDown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TaskKind {
    #[default]
    None,
    User,
    Service,
    Script,
    Manual,
    BusinessRule,
    Send,
    Receive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventTrigger {
    Message,
    Timer,
    Error,
    Escalation,
    Cancel,
    Compensation,
    Conditional,
    Link,
    Signal,
    Terminate,
    Multiple,
    ParallelMultiple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayKind {
    Exclusive,
    Parallel,
    Inclusive,
    Complex,
    EventBased,
}

/// FlowElement(FlowNode·DataObject·DataStore) + Artifact(TextAnnotation)의 종류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    /// Start/Intermediate(Catch·Throw·Boundary 공통)/End + EventDefinition 하나.
    Event { position: EventPosition, trigger: Option<EventTrigger> },
    Task(TaskKind),
    /// 접힌 서브프로세스만.
    Subprocess,
    CallActivity,
    Gateway(GatewayKind),
    DataObject,
    DataStore,
    TextAnnotation,
}

impl Default for ElementKind {
    fn default() -> Self {
        ElementKind::Task(TaskKind::None)
    }
}

impl ElementKind {
    /// Task·Subprocess·CallActivity — 경계 이벤트를 붙일 수 있는 것.
    pub fn is_activity(self) -> bool {
        matches!(self, ElementKind::Task(_) | ElementKind::Subprocess | ElementKind::CallActivity)
    }

    /// BPMN FlowNode(활동·이벤트·게이트웨이) — 시퀀스 흐름의 끝이 될 수 있는 것.
    pub fn is_flow_node(self) -> bool {
        matches!(self, ElementKind::Event { .. } | ElementKind::Task(_) | ElementKind::Subprocess | ElementKind::CallActivity | ElementKind::Gateway(_))
    }
}

/// 협업의 참여자(= 그려질 때 풀). 프로세스의 레인 집합을 품는다.
#[derive(Clone, Debug, Default)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub lanes: Vec<Lane>,
}

#[derive(Clone, Debug, Default)]
pub struct Lane {
    pub id: String,
    pub name: String,
    pub sub_lanes: Vec<Lane>,
}

#[derive(Clone, Debug, Default)]
pub struct Element {
    pub id: String,
    pub name: String,
    pub kind: ElementKind,
    /// 소속 참여자 id 또는 가장 안쪽 레인 id(laneSet/flowNodeRef). `None` = 참여자 밖.
    pub container: Option<String>,
    /// 경계 이벤트의 호스트 활동 id.
    pub attached_to: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowKind {
    Sequence { is_default: bool },
    Message,
    Association,
    DataAssociation,
}

#[derive(Clone, Debug)]
pub struct Flow {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
    pub kind: FlowKind,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: String,
    pub orientation: Orientation,
    pub participants: Vec<Participant>,
    pub elements: Vec<Element>,
    pub flows: Vec<Flow>,
}

impl Model {
    /// 참여자 id 또는 (중첩) 레인 id → 그 참여자의 인덱스. 모르는 id면 `None`.
    pub fn participant_of_container(&self, container: &str) -> Option<usize> {
        self.participants.iter().position(|p| p.id == container || lane_tree_contains(&p.lanes, container))
    }

    pub fn element(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    /// 최상위 참여자 id 정확 일치 → 인덱스. 레인 id는 `None`(레인은 InteractionNode가 아니다 —
    /// `MessageFlow.sourceRef`/`targetRef`로 쓸 수 있는 건 흐름 노드와 `Participant`뿐이다).
    pub fn participant_index(&self, id: &str) -> Option<usize> {
        self.participants.iter().position(|p| p.id == id)
    }
}

fn lane_tree_contains(lanes: &[Lane], id: &str) -> bool {
    lanes.iter().any(|lane| lane.id == id || lane_tree_contains(&lane.sub_lanes, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_model() -> Model {
        Model {
            participants: vec![Participant {
                id: "pool-1".into(),
                name: "판매사".into(),
                lanes: vec![Lane { id: "lane-1".into(), name: "영업".into(), sub_lanes: vec![Lane { id: "lane-1-1".into(), name: "선임".into(), sub_lanes: Vec::new() }] }],
            }],
            ..Model::default()
        }
    }

    #[test]
    fn participant_of_container_resolves_pool_lane_and_sub_lane() {
        let model = nested_model();
        assert_eq!(model.participant_of_container("pool-1"), Some(0));
        assert_eq!(model.participant_of_container("lane-1"), Some(0));
        assert_eq!(model.participant_of_container("lane-1-1"), Some(0));
        assert_eq!(model.participant_of_container("unknown"), None);
    }

    #[test]
    fn participant_index_resolves_only_top_level_participant_ids() {
        let model = nested_model();
        assert_eq!(model.participant_index("pool-1"), Some(0));
        assert_eq!(model.participant_index("lane-1"), None);
        assert_eq!(model.participant_index("lane-1-1"), None);
        assert_eq!(model.participant_index("unknown"), None);
    }

    #[test]
    fn defaults_are_horizontal_orientation_and_kindless_task() {
        let model = Model::default();
        assert_eq!(model.orientation, Orientation::Horizontal);
        assert_eq!(ElementKind::default(), ElementKind::Task(TaskKind::None));
    }

    #[test]
    fn only_task_subprocess_call_activity_are_activities() {
        assert!(ElementKind::Task(TaskKind::None).is_activity());
        assert!(ElementKind::Subprocess.is_activity());
        assert!(ElementKind::CallActivity.is_activity());
        assert!(!ElementKind::Gateway(GatewayKind::Exclusive).is_activity());
        assert!(!ElementKind::Event { position: EventPosition::Start, trigger: None }.is_activity());
        assert!(!ElementKind::DataObject.is_activity());
    }

    #[test]
    fn data_object_store_and_annotation_are_not_flow_nodes() {
        assert!(!ElementKind::DataObject.is_flow_node());
        assert!(!ElementKind::DataStore.is_flow_node());
        assert!(!ElementKind::TextAnnotation.is_flow_node());
        assert!(ElementKind::Task(TaskKind::None).is_flow_node());
        assert!(ElementKind::Event { position: EventPosition::Start, trigger: None }.is_flow_node());
        assert!(ElementKind::Gateway(GatewayKind::Exclusive).is_flow_node());
    }
}
