# Design — bpmn-model

## 정의
세 입력 문법(XML·YAML·`biz-process.md`)이 공유할 BPMN 의미 모델 `bpmn::Model`을 정의하고,
`validate`(BPMN 구조 규칙) → `lower`(`ir::Graph`, 기존 레인·도형·표식 어휘) →
`layout::graph::render` 경로와 `Language::Bpmn` 배선을 만드는 계층이다. 파서는 없고,
손으로 만든 `Model`로 end-to-end 검증한다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **의미 모델**: `bpmn::model::{Model, Participant, Lane, Element, ElementKind, TaskKind, EventTrigger,
  GatewayKind, Flow, FlowKind, Orientation}` — 세 파서가 만들어야 할 유일한 산출 형태. BPMN 2.0.2
  프로세스 모델(의미 계층)만 담는다
- **어휘 표**: `bpmn::vocabulary` — 종류 ↔ BPMN XML 로컬 이름 토큰 ↔ 도형 라벨(`«user»`·`×`…)
- **검증**: `bpmn::validate::{validate, ModelError}` — 참조·중복·풀 경계·경계 이벤트 규칙
- **변환**: `bpmn::lower::lower` — 모델 → `ir::Graph`(레인 그룹·도형·선·경계 이벤트 근사·
  빈 풀 자리표시·방향)
- **배선**: `bpmn::{kind_of, render, render_model}`, `diagram::Language::Bpmn`, `Language::name()`,
  `diagram::language_of_path()`, `cli::LangArg::Bpmn`
- **회귀 기준**: mermaid·PlantUML 출력 바이트 동일(8.1)

### Out-of-Scope
- **파서·코드펜스 본문 문법**: `bpmn-xml`·`bpmn-yaml`·`bizprocess-bpmn`. 이 스펙의 `kind_of`는
  항상 `None` — 파서 스펙이 갈래를 더한다
- **BPMNDI(다이어그램 교환 계층) 전부 — 영구 제외(사용자 결정 2026-09-26)**: 좌표·크기·
  waypoint·`BPMNShape`/`BPMNEdge` 참조·`isHorizontal`을 `Model`에 두지 않는다. 배치는 `lower` 이후
  dg 자신의 배치기 몫. 하류 `bpmn-xml`은 `<bpmndi:*>` 구획을 읽지 않고 건너뛴다(`bpmn-support/
  research.md`의 "`BPMNDI`의 `isHorizontal`만" 문구는 이 결정으로 대체됨)
- **펼친 서브프로세스·노드 중첩·`ExpandPolicy`/`depth`**: `bizprocess-bpmn`. `ElementKind::Subprocess`는
  접힌 것만
- **풀·레인을 흐름 끝으로(그룹 닻)·풀을 넘는 Group·경계 이벤트 테두리 부착·이중선**: 미지원
  (`bpmn-support/research.md`)
- **`ir`·`layout` 변경**: 없음. 새 도형·표식·레인 규칙은 `bpmn-shapes`/`bpmn-lane-layout` 재개
- **진단 UI**: `ModelError`는 `Display`까지; 화면 노출은 파서 스펙 몫

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음, Rust 2024 edition)
- 내부 의존 방향: `ir` ← `bpmn::model`(`EventPosition` 재사용만) ← `bpmn::vocabulary` ←
  `bpmn::validate` ← `bpmn::lower`(`ir::Graph` 생성) ← `bpmn::mod`(`layout::graph`·`options` 사용)
  ← `diagram::mod` ← `main`/`cli`. `model`·`vocabulary`·`validate`가 `layout`·`canvas`를 import하거나
  `ir`/`layout`이 `bpmn`을 알면 설계 위반 — BPMN 의미(풀·경계 이벤트)는 `bpmn` 안에서 끝난다

### Revalidation Triggers
- `layout::graph::render()`의 재시도 순서(선호 → 반대 → TB 접기)가 바뀌면 `Orientation` 매핑·6.4 재검토
- `bpmn-lane-layout`이 구성원 없는 레인을 그리게 바뀌면 자리표시 닻 제거
- `bpmn-shapes/design.md` "lower()가 만들어야 할 라벨" 계약이 바뀌면 `vocabulary` 라벨 열 동기화
- 하류 스펙이 펼친 서브프로세스·풀 안 Group·풀을 끝으로 하는 메시지 흐름을 실제로 요구하면
  `Model`에 `parent`/`groups` 도입을 그때 결정(지금은 넣지 않음)

## Architecture

### Boundary Map
```mermaid
flowchart LR
  tests[hand built Model tests] --> model[bpmn model types]
  model --> vocabulary[bpmn vocabulary tokens labels]
  model --> validate[bpmn validate ModelError]
  model --> lower[bpmn lower to ir Graph]
  vocabulary --> lower
  validate --> bpmnmod[bpmn mod render_model]
  lower --> bpmnmod
  bpmnmod --> graph[layout graph render]
  bpmnmod --> options[DiagramOptions apply_to_graph]
  diagrammod[diagram mod Language Bpmn] --> bpmnmod
  main[main cli LangArg] --> diagrammod
```

### Technology Stack
| Layer | Choice | Role |
|-------|--------|------|
| 의미 계층·배선 | Rust 1.96, 표준 라이브러리만 | 타입·검증·변환, 기존 배치기 재사용 |

### Key Decisions
- **충실도 대상은 BPMN 2.0.2 프로세스 모델, DI 아님(사용자 결정 2026-09-26)**: `Model`의 타입·이름은
  메타모델 용어(`Participant`·`Lane`·`Element`(FlowElement+Artifact)·`SequenceFlow`/`MessageFlow`…)를
  따르고, 다이어그램 계층 개념은 하나도 넣지 않는다 — 사용자 말 그대로 "process model: 의미
  (다이어그램 노드 아닌 process task event …) 충실하게, 프로세스 모델이니까". dg는 자기 배치기가
  있어 다른 도구가 그린 좌표를 보존할 이유가 없다. `bpmn-support/research.md`의 `Pool{lanes}`·
  `FlowNode{…}` 스케치는 이미 의미 전용이었고, 이 결정으로 이름만 메타모델에 맞춘다.
- **소속은 `container` 하나**(참여자 id 또는 가장 안쪽 레인 id) — 이유: 레인은 참여자(프로세스)에
  종속돼 `pool`은 파생값(research.md). 조회는 `Model::participant_of_container()` 한 곳.
- **`Orientation`은 dg 배치 방향 힌트, 매핑은 `lower` 한 곳**: `Horizontal → LeftRight`(기본, 6.1),
  `Vertical → TopDown` — mermaid `flowchart LR`과 같은 성격의 렌더링 지시이며 BPMNDI
  `isHorizontal`에서 읽어 오는 값이 아니다(XML 파서는 기본값을 두고 CLI 옵션이 덮는다). 이유:
  렌더링 결정을 파서 셋이 반복하지 않게.
- **`EventPosition`은 `ir` 재사용** — 의미와 그림이 1:1, 두 번 정의하면 SSoT 위반.
- **어휘 표는 종류별 튜플 `const` 하나** `(종류, XML 로컬 이름, 라벨)`; 토큰·라벨 함수가 같은 표를
  읽는다(7.3) — 이유: `match` 둘로 갈라지면 어긋난다. 파일 이름 `vocabulary.rs`(research.md).
- **검증은 전부 모아 `Vec<ModelError>`**(1.8); 렌더 경로는 `None`으로 강등(코드블록 폴백).
- **빈 풀·레인 자리표시 = `Shape::Anchor` 노드**(4.3) — 이유: 보이지 않고 배치기 예외 처리가 이미
  있음. 노드 없는 모델은 자리표시 전에 `None`(1.9).
- **경계 이벤트 = 호스트 → 이벤트 무표식 `Dashed` 간선**(5.1, `bpmn-support/research.md` v1 근사) —
  간선이 다음 층을 강제하고 그룹은 호스트 것을 따른다.
- **캡션 종류 = 풀 수**: 2개 이상 `collaboration`, 아니면 `process`(6.7).
- **이름·확장자 표 SSoT**: `Language::name()`·`language_of_path()`를 `render()`·`main.rs`가 함께
  쓴다 — 이유: 지금 두 곳에 중복돼 변형 추가 시 `main.rs`가 틀린 이름을 쓴다(research.md).
- **`kind_of(source)`는 `None`, `render(source)`는 `kind_of`로 시작**(6.8) — mermaid/PlantUML과 같은
  모양이라 파서 스펙이 `match` 팔만 더한다.
- **요소 → 도형**(`bpmn-shapes/design.md` 라벨 계약 그대로; `sections = vec![섹션0]`):

| 모델 요소 | `Shape` | 섹션0 | 요구사항 |
|---|---|---|---|
| `Event{position, trigger}` | `Event(position)` | `[name]` + `trigger.label()`; 이름 없으면 트리거 줄만, 둘 다 없으면 `[]` | 2.1, 2.2 |
| `Task(kind)` | `Round` | `[name]` + `kind.label()`(`None`이면 생략) | 2.3 |
| `Subprocess` / `CallActivity` | `Subprocess` / `Round` | `[name]` / `[name, "«call»"]` | 2.4, 2.5 |
| `Gateway(kind)` | `Diamond` | `["{kind.label()} {name}".trim_end()]` | 2.6 |
| `DataObject` / `DataStore` / `TextAnnotation` | `Rect` / `Cylinder` / `Note` | `[name, "«data»"]` / `[name]` / `[name]` | 2.7 |

- **흐름 → 선**(`LineKind`, `tail`, `head`; 라벨은 모두 `flow.label`):

| `FlowKind` | 선 | 꼬리 | 머리 | 요구사항 |
|---|---|---|---|---|
| `Sequence{is_default:false}` / `{true}` | `Solid` | `None` / `Slash` | `Arrow` | 3.1, 3.2 |
| `Message` | `Dashed` | `Circle` | `OpenArrow` | 3.3 |
| `Association` / `DataAssociation` | `Dashed` | `None` | `None` / `OpenArrow` | 3.4, 3.5 |
| 경계 부착(흐름 아님, 라벨 없음) | `Dashed` | `None` | `None` | 5.1 |

## Components and Interfaces

### bpmn::model — 의미 모델 (신규)
- Intent: 세 파서의 유일한 산출 형태 — BPMN 2.0.2 프로세스 모델의 하위집합. 검증·변환 논리 없음
  (조회 도우미만)
- Requirements: 1.2, 1.9, 2.1~2.7, 3.1~3.5, 4.1~4.5, 5.1, 6.1, 6.2, 6.5
```rust
pub use crate::diagram::ir::EventPosition;
// 아래 enum은 모두 Clone, Copy, Debug, PartialEq, Eq; struct는 Clone, Debug, Default.
pub enum Orientation { #[default] Horizontal, Vertical }                       // dg 배치 방향 힌트(DI 아님)
pub enum TaskKind { #[default] None, User, Service, Script, Manual, BusinessRule, Send, Receive }
pub enum EventTrigger { Message, Timer, Error, Escalation, Cancel, Compensation, Conditional, Link, Signal, Terminate, Multiple, ParallelMultiple }
pub enum GatewayKind { Exclusive, Parallel, Inclusive, Complex, EventBased }
/// FlowElement(FlowNode·DataObject·DataStore) + Artifact(TextAnnotation)의 종류.
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
impl Default for ElementKind { fn default() -> Self { ElementKind::Task(TaskKind::None) } }
impl ElementKind {
    pub fn is_activity(self) -> bool;   // Task·Subprocess·CallActivity — 경계 이벤트를 붙일 수 있는 것
    pub fn is_flow_node(self) -> bool;  // BPMN FlowNode(활동·이벤트·게이트웨이) — 시퀀스 흐름의 끝이 될 수 있는 것
}
/// 협업의 참여자(= 그려질 때 풀). 프로세스의 레인 집합을 품는다.
pub struct Participant { pub id: String, pub name: String, pub lanes: Vec<Lane> }
pub struct Lane { pub id: String, pub name: String, pub sub_lanes: Vec<Lane> }
pub struct Element {
    pub id: String,
    pub name: String,
    pub kind: ElementKind,
    /// 소속 참여자 id 또는 가장 안쪽 레인 id(laneSet/flowNodeRef). None = 참여자 밖.
    pub container: Option<String>,
    /// 경계 이벤트의 호스트 활동 id.
    pub attached_to: Option<String>,
}
pub enum FlowKind { Sequence { is_default: bool }, Message, Association, DataAssociation }
pub struct Flow { pub id: String, pub source: String, pub target: String, pub label: String, pub kind: FlowKind }
pub struct Model { pub title: String, pub orientation: Orientation, pub participants: Vec<Participant>, pub elements: Vec<Element>, pub flows: Vec<Flow> }
impl Model {
    /// 참여자 id 또는 (중첩) 레인 id → 그 참여자의 인덱스. 모르는 id면 None.
    pub fn participant_of_container(&self, container: &str) -> Option<usize>;
    pub fn element(&self, id: &str) -> Option<&Element>;
}
```
- 계약: 타입은 임의 값을 허용하고 불변식은 `validate`가 검사한다(파서는 채우기만). id는 풀·레인·
  노드·흐름 전체에서 고유(1.5). `Flow`는 `Default` 없음(`kind` 필수).

### bpmn::vocabulary — 토큰·라벨 표 (신규)
- Intent: 종류 ↔ XML 로컬 이름 ↔ 도형 라벨의 단일 출처
- Requirements: 2.2, 2.3, 2.5, 2.6, 2.7, 7.1~7.3
```rust
impl TaskKind     { pub fn xml_name(self) -> &'static str; pub fn from_xml_name(t: &str) -> Option<Self>; pub fn label(self) -> Option<&'static str>; }
impl EventTrigger { pub fn xml_name(self) -> &'static str; pub fn from_xml_name(t: &str) -> Option<Self>; pub fn label(self) -> &'static str; }
impl GatewayKind  { pub fn xml_name(self) -> &'static str; pub fn from_xml_name(t: &str) -> Option<Self>; pub fn label(self) -> &'static str; }
pub const CALL_ACTIVITY_LABEL: &str = "«call»";
pub const DATA_OBJECT_LABEL: &str = "«data»";
```
- 표: `TaskKind` — `task`/없음, `userTask`/`«user»`, `serviceTask`/`«service»`, `scriptTask`/`«script»`,
  `manualTask`/`«manual»`, `businessRuleTask`/`«businessRule»`, `sendTask`/`«send»`, `receiveTask`/
  `«receive»`. `EventTrigger` — `{trigger}EventDefinition`(`messageEventDefinition` …
  `parallelMultipleEventDefinition`)/`«{trigger}»`(`«message»` … `«parallelMultiple»`, 첫 글자 소문자
  camelCase). `GatewayKind` — `exclusiveGateway`/`×`, `parallelGateway`/`+`, `inclusiveGateway`/`○`,
  `complexGateway`/`*`, `eventBasedGateway`/`◎`.
- 계약: `from_xml_name`은 대소문자 그대로 정확 일치만(7.2); `xml_name(from_xml_name(t)) == t`(7.1).

### bpmn::validate — 구조 규칙 (신규)
- Intent: BPMN 고유 규칙을 한 곳에서 검사
- Requirements: 1.1~1.9
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    DuplicateId(String),
    UnknownReference { owner: String, reference: String },   // owner(흐름·노드 id)가 가리킨 reference가 없다
    SequenceFlowCrossesParticipants(String),
    SequenceFlowEndsAtNonFlowNode(String),
    MessageFlowWithinParticipant(String),
    BoundaryEventNotIntermediate(String),
    BoundaryHostNotActivity(String),
    BoundaryContainerMismatch(String),
}
impl std::fmt::Display for ModelError { /* 변형 이름 + 관련 id */ }
/// 발견한 위반 전부. 노드 없는 모델은 Ok.
pub fn validate(model: &Model) -> Result<(), Vec<ModelError>>;
```
- 규칙 순서(앞 규칙이 실패한 참조는 뒤 규칙에서 건너뜀): 중복 id(1.5) → 참조 해석(`container`·
  `attached_to`·`source`·`target`, 1.4) → 시퀀스 흐름: 양끝 `is_flow_node`(1.6)·같은 풀(풀 밖끼리도
  같은 풀로 봄, 1.1·1.2) → 메시지 흐름: 양끝 풀이 둘 다 `Some`이고 서로 다름(1.3) → 경계 이벤트:
  `Event{Intermediate,..}`·호스트 `is_activity`·`container`가 `None`이거나 호스트와 동일(1.7).
  연관·데이터 연관은 참조 해석만.

### bpmn::lower — 모델 → `ir::Graph` (신규)
- Intent: 검증된 모델을 기존 어휘로 옮김. BPMN 의미는 여기서 끝난다
- Requirements: 2.1~2.7, 3.1~3.5, 4.1~4.5, 5.1, 5.2, 6.1, 6.2, 6.5
```rust
/// 검증된 모델 전제. 해석 안 되는 참조는 건너뛴다(패닉 없음).
pub fn lower(model: &Model) -> Graph;
```
- 순서: `direction`(`Orientation` 매핑)·`title` → 풀·레인을 선언 순서로 `add_lane`(부모 = 바깥 그룹,
  `HashMap<모델 id, 그룹 index>`) → 노드 `intern(id, name, shape, group)` 후 `sections[0]`을 §Key
  Decisions 표대로 교체(경계 이벤트의 그룹은 `container`가 `None`이면 호스트 그룹) → 노드가 하나도
  없는 풀·레인마다 `Shape::Anchor`·빈 섹션 노드 하나(id `@bpmn-placeholder:{id}`) → 흐름을 표대로
  `add_edge` → 경계 이벤트마다 호스트 → 이벤트 `Dashed` 무표식 간선.

### bpmn::mod — 언어 진입점 (신규)
- Intent: `diagram::mod`가 mermaid·PlantUML과 같은 모양으로 부르는 진입점
- Requirements: 1.9, 6.3, 6.4, 6.7, 6.8
```rust
pub mod model; pub mod vocabulary; pub mod validate; pub mod lower;
/// 코드펜스 본문의 문법 갈래. 이 스펙에서는 항상 None(파서 없음).
pub fn kind_of(source: &str) -> Option<&'static str>;
/// kind_of → 파서 → render_model. 지금은 항상 None.
pub fn render(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)>;
/// elements 비면 None → validate → lower → options.apply_to_graph → layout::graph::render. 종류 = participants 2개 이상 "collaboration", 아니면 "process".
pub fn render_model(model: &Model, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)>;
```

### diagram::mod · main · cli — `Language::Bpmn` 배선 (기존 파일 확장)
- Intent: 코드펜스·확장자·`--lang`으로 `bpmn`을 고르고 캡션 이름을 한 곳에서 얻는다
- Requirements: 6.6, 6.7, 6.8, 8.1
```rust
pub enum Language { Mermaid, PlantUml, Bpmn }
impl Language { pub fn name(self) -> &'static str; }            // "mermaid" | "plantuml" | "bpmn"
/// 확장자 표의 단일 출처(.puml/.plantuml/.pu/.iuml · .mmd/.mermaid · .bpmn).
pub fn language_of_path(path: &str) -> Option<Language>;
pub fn language_of_fence(lang: &str) -> Option<Language>;       // "bpmn" 추가
pub fn language_of_source(path: Option<&str>, source: &str) -> Option<Language>; // language_of_path + bpmn::kind_of 스니핑
// render_body / kind_of: Language::Bpmn => bpmn::render / bpmn::kind_of
// cli.rs: enum LangArg { Mermaid, Plantuml, Bpmn }
// main.rs: language.name(); looks_like_diagram_file → diagram::language_of_path(p).is_some()
```
- 계약: `render()`의 이름 `match`와 `main.rs:58`의 `if`는 `Language::name()`으로 교체(중복 제거).
  mermaid·PlantUML 경로는 이동만, 동작 불변(8.1).

## Data Models
`bpmn::model` 인터페이스 블록이 전부. 불변식(`validate`가 보장): id 전역 고유, 모든 참조 해석 가능,
시퀀스 흐름은 풀 안, 메시지 흐름은 풀 사이, 경계 이벤트는 활동에 붙은 중간 이벤트. `Graph` 쪽:
모델 노드 id = `Node.id`(파서 스펙이 진단에 되짚을 수 있게), 자리표시 id는 `@bpmn-placeholder:`
접두(그룹 닻 `@group-anchor:`와 같은 관례).

## Error Handling
- **사용자 입력 오류**: 규칙 위반 → `Err(Vec<ModelError>)`(1.1~1.8), 렌더 경로 `None` → 원문
  코드블록. 파서 없는 `bpmn` 코드펜스 → `kind_of` `None` → 코드블록(6.8)
- **외부 자원 오류**: 해당 없음
- **시스템 오류(패닉)**: `lower`는 해석 안 되는 참조를 건너뜀; 자기 자신 흐름·순환·이름 없음·3단
  레인·경계 이벤트 여럿·양방향을 단위 테스트로 패닉 없음 확인(8.2)
- **기능 강등**: 폭 초과 → `layout::graph::render` 규약(반대 방향 → 라벨 축소 → `None`) 그대로(6.4);
  방향 옵션은 `apply_to_graph`가 모델 방향을 덮어씀(6.3)

## Testing Strategy
- **Depth**: Complex — 도메인 규칙(검증) + 계층 통합(모델 → 배치기 → 언어 배선). 코드펜스 E2E는
  파서 부재로 폴백(6.8)만, 그림 E2E는 `render_model` 손 모델
- **Unit(L6)**:
  - `vocabulary`: 세 표 전 항목 `xml_name(from_xml_name(t)) == t`(7.1); `"UserTask"`·`"foo"` → `None`(7.2);
    라벨이 `bpmn-shapes` 계약 문자열과 일치(2.2, 2.3, 2.6, 7.3)
  - `validate`: 규칙마다 위반 모델 하나 → 해당 변형 + id(1.1, 1.3~1.7); 레인 간 시퀀스 → `Ok`(1.2);
    위반 둘 → 둘 다 보고(1.8); 노드 없음 → `Ok`(1.9)
  - `lower`: 노드 종류 8가지 → `shape`·`sections`(2.1~2.7; 이름 없는 이벤트 `[]`, 트리거만 `["«timer»"]`);
    흐름 5종 + 경계 간선 → `kind`·`tail`·`head`·`label`(3.1~3.5, 5.1); 풀·레인 → `GroupKind::Lane`·
    `parent`·선언 순서(4.1, 4.2); 빈 풀 → `Anchor` 자리표시 한 개(4.3); 풀 없음 → `groups` 빔(4.4);
    `Horizontal`/`Vertical` → `LeftRight`/`TopDown`(6.1, 6.2); `title`(6.5)
- **Integration(L5, `bpmn::tests`, `render_model` + `Line::plain`)**:
  - 빈 풀 자리표시가 실제로 띠·제목을 그리고 노드는 안 보임(4.3); 풀 밖 노드 혼재 패닉 없음(4.5)
  - LR에서 경계 이벤트가 호스트 오른쪽 다음 층·같은 레인 안, `╌` 점선으로 이어지고 그 뒤는 실선
    화살(5.1, 5.2)
  - `Vertical` → 풀 띠 좌우, `Horizontal` → 위아래(6.1, 6.2); `DiagramOptions.direction = TopDown`이
    `Horizontal`을 덮음(6.3); 폭 8 → `None`(6.4); 종류 이름 풀 1개 `process`·2개 `collaboration`(6.7)
  - 8.2 패닉 없음 조합; 8.3 손 모델 — 풀 `고객`(시작 → `«user»` 주문서 작성 → 종료), 풀 `판매사`
    레인 `영업`(`«message»` 시작 ← 메시지 흐름, `«user»` 주문 검토 + 경계 `«error»` 시한 초과 →
    종료 취소, `«service»` 품절 안내 → 종료 품절 취소)·레인 `창고`(`× 재고 있음?` 예 → `«user»`
    출고 준비 → 종료, default → 품절 안내) — 폭 100 → `Some`, `○ ● ◎ ┃ «user» «service» «message»
    «error» × ╱ ╌` 전부 포함, `{text}` 육안 확인
- **Integration(L4, `diagram::mod`·`markdown` 테스트)**: `language_of_fence("bpmn")`·
  `language_of_path("x.bpmn")`·`Language::Bpmn.name()`(6.6); `caption("bpmn", kind)` 평문이
  `◈ bpmn · collaboration `로 시작(6.7); `robustness::odd_inputs_do_not_panic`에 `Language::Bpmn`
  루프(빈 문자열·`<definitions>`·YAML 조각·한글) → 전부 `None`(6.8); 마크다운 문서의 `bpmn` 펜스가
  코드블록으로 남고 앞뒤 문단은 그대로(6.8)
- **Acceptance(L1)**: `cargo test` 전량(기준선 334개 무수정 통과 + 신규) + `cargo clippy --all-targets
  -- -D warnings` + `examples/*.md`를 수정 전후 `dg -P --width 100`으로 렌더링해 `diff` 무차이(8.1) +
  `dg -d -l bpmn <임의 파일>` → 코드블록 출력, 패닉 없음(6.6, 6.8)

## File Structure Plan
```
src/diagram/bpmn/mod.rs         # kind_of(None) · render(source) · render_model · 통합 테스트
src/diagram/bpmn/model.rs       # Model·Participant·Lane·Element·ElementKind·Flow·FlowKind·Orientation + 조회 도우미
src/diagram/bpmn/vocabulary.rs  # TaskKind·EventTrigger·GatewayKind 토큰·라벨 표, CALL_ACTIVITY_LABEL·DATA_OBJECT_LABEL
src/diagram/bpmn/validate.rs    # ModelError · validate
src/diagram/bpmn/lower.rs       # lower(Model) -> Graph
src/diagram/mod.rs              # Language::Bpmn · name() · language_of_path · 배선 (기존 파일 확장)
src/cli.rs                      # LangArg::Bpmn (기존 파일 확장)
src/main.rs                     # Language::name() · language_of_path 사용 (기존 파일 확장)
```
