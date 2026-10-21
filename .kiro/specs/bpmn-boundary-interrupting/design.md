# 설계서 - bpmn-boundary-interrupting

## 정의
BPMN 다이어그램을 터미널에서 읽는 사용자가 경계 이벤트가 액티비티를 중단시키는지 아닌지를 그림만 보고 구분하도록, 경계 이벤트의 방해 여부를 입력에서 읽어 방해와 비방해를 다르게 그리는 경계 이벤트 표기 기능이다.

## 경계 확정

### 소유 범위 (이 스펙)
- **경계 이벤트 방해 여부 모델**: `Element.attached_to`가 호스트 id와 방해 여부를 함께 가진다.
- **XML `cancelActivity`, YAML `cancel_activity` 읽기**: 값 해석과 잘못된 값의 파서 오류.
- **방해 여부와 트리거 조합 검사**: 오류, 취소 트리거의 비방해를 모델 오류로.
- **비방해 이벤트 글자**: IR 이벤트 도형의 방해 여부와 그 글자 `◌`.

### 비소유 범위
- **흐름 선과 표식**: `bpmn-flow-kind-notation`.
- **이벤트 위치 글자 `○` `◎` `◉`와 트리거 둘째 줄**: `bpmn-event-shape-notation`.
- **ASCII 출력 글자 대응(비방해 글자 포함)**: `ascii-output-mode`.
- **bizprocess 입력**: 기존 동작 그대로(만드는 경계 이벤트는 항상 방해).
- **원문 코드블록 물러남 경로**: 기존 `bpmn::render`의 `parse -> validate -> ok()?`.

### 허용 의존
- External: 없음(새 크레이트 없음).
- Internal direction: `diagram::ir` -> `bpmn::model` -> `bpmn::parse_xml`, `bpmn::parse_yaml`, `bpmn::parse_bizprocess` -> `bpmn::validate` -> `bpmn::lower` -> `layout::shape` (역방향 import = 설계 위반).

### 재검증 조건
- 경계 이벤트를 호스트 테두리 위에 올리는 배치가 들어오면(이벤트 글자 위치 전제가 바뀜).
- 이벤트 서브프로세스의 비방해 시작 이벤트를 다루게 되면(`Shape::Event`의 방해 여부를 시작 이벤트에도 써야 함).
- 확인 기준 폰트(Noto Sans Mono CJK, D2Coding)가 바뀌거나 늘면(`◌` 커버리지와 폭 재측정).

## 아키텍처

### 핵심 결정
- **비방해 = 이벤트 글자 `◌`(DOTTED CIRCLE, U+25CC)**: BPMN 비방해 이벤트의 점선 원과 같은 운율이고, Noto Sans Mono CJK KR와 D2Coding 1.3.3 두 폰트에 있고(`fc-list ... charset=25CC` 실측), 두 폰트에서 한 칸 폭(D2Coding 글자 너비 실측 1.0, East Asian Width N)이다 - 이유: 사용자 결정 "식별성 우선", 선 무늬는 메시지(파선), 연결(점선)과 충돌. 대안은 research.md.
- **선은 바꾸지 않음**: 호스트 -> 이벤트 간선과 이벤트에서 나가는 시퀀스 흐름은 지금처럼 실선(2.4) - 이유: 선 무늬는 흐름 종류만 뜻한다는 `bpmn-flow-kind-notation` 규칙 유지.
- **`attached_to: Option<String>` -> `Option<Attachment>`**: 방해 여부는 경계 이벤트에만 의미가 있으므로 호스트 참조와 한 값으로 묶는다 - 이유: 경계가 아닌 요소에 방해 여부가 생기는 상태를 표현할 수 없게 하고, `attached_to: None`인 기존 생성 지점(약 66곳)을 건드리지 않는다.
- **`Shape::Event(EventPosition)` -> `Shape::Event { position, is_interrupting }`**: 그리기가 방해 여부를 알아야 하므로 IR 도형에 둔다 - 이유: `EventPosition`에 변형을 더하면 모델 쪽 exhaustive match(위치 검사, 어휘표)까지 번진다.
- **트리거 조합 검사는 `validate`**: 기존 경계 규칙(`BoundaryEventNotIntermediate` 등)과 같은 곳, 같은 물러남 경로(3.1) - 이유: XML, YAML 두 파서에 같은 규칙을 두 번 쓰지 않음(SSoT).

## 요구사항 추적
| 요구사항 ID | 구현 요소 |
|---|---|
| 1.1, 1.2, 1.5 | parse_xml - 경계 이벤트 읽기 |
| 1.3, 1.4, 1.6 | parse_yaml - 경계 이벤트 읽기 |
| 2.1~2.3 | layout::shape - 이벤트 글자 |
| 2.4 | lower - 경계 간선(변경 없음, 회귀 테스트로 고정) |
| 2.5 | layout::shape - 이벤트 글자(노드 원점 그리기, 방향 무관) |
| 3.1, 3.2 | validate - 경계 이벤트 검사 |
| 4.1 | 모든 컴포넌트 - 방해(기본값) 경로는 기존 출력 그대로 |
| 4.2 | layout::shape - 커버리지 허용 목록에 `◌` 추가 |

## 컴포넌트와 인터페이스

### bpmn::model - Attachment
- 의도: 경계 이벤트의 호스트와 방해 여부.
- 요구사항: 1.1~1.4
```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub host: String,
    pub is_interrupting: bool,
}

pub struct Element {
    // ... 기존 필드
    pub attached_to: Option<Attachment>,
}
```
- `parse_bizprocess`의 오류 경계 이벤트는 `is_interrupting: true`.

### bpmn::parse_xml - 경계 이벤트 읽기
- 의도: `boundaryEvent`의 `cancelActivity` 해석.
- 요구사항: 1.1, 1.2, 1.5
```rust
pub enum ParseError {
    Xml(XmlError),
    UnexpectedRoot(String),
    InvalidCancelActivity { element: String, value: String },
}
```
- 속성 없음 또는 `"true"` -> 방해, `"false"` -> 비방해, 그 밖 -> `InvalidCancelActivity`(파싱 실패 -> 원문 물러남).

### bpmn::parse_yaml - 경계 이벤트 읽기
- 의도: 노드 키 `cancel_activity` 해석.
- 요구사항: 1.3, 1.4, 1.6
```rust
const NODE_KEYS: &[&str] = &["kind", "name", "trigger", "attached_to", "cancel_activity", "default"];

pub enum ParseError {
    // ... 기존 변형
    InvalidBoolean { path: String, key: &'static str, token: String },
}
```
- 스칼라 `true`, `false`만 허용, 그 밖 -> `InvalidBoolean`. `boundaryEvent`가 아닌 노드에 있으면 기존 `FieldNotAllowed { key: "cancel_activity" }`.

### bpmn::validate - 경계 이벤트 검사
- 의도: 오류, 취소 트리거의 비방해 거부.
- 요구사항: 3.1, 3.2
```rust
pub enum ModelError {
    // ... 기존 변형
    BoundaryTriggerMustInterrupt(String),
}
```
- `check_boundary_events`에서 `!is_interrupting` + 트리거 `Error` 또는 `Cancel` -> 오류. 트리거 없음, 다른 트리거는 통과.

### diagram::ir - 이벤트 도형
- 의도: 그리기에 방해 여부 전달.
- 요구사항: 2.1~2.3
```rust
pub enum Shape {
    // ...
    Event { position: EventPosition, is_interrupting: bool },
}
```
- `lower::shape_of`가 경계 이벤트는 `attached_to.is_interrupting`, 그 밖 이벤트는 `true`로 채운다.

### layout::shape - 이벤트 글자
- 의도: 위치와 방해 여부 -> 글자.
- 요구사항: 2.1~2.3, 2.5, 4.2
- 글자 표(first match): `Intermediate` + 비방해 -> `◌`, `Start` -> `○`, `Intermediate` -> `◎`, `End` -> `◉`.
- 글자는 지금처럼 노드 원점에 그린다 - 가로, 세로 배치 모두 같은 자리(2.5).
- `CJK_MONO_COVERED_GLYPHS`에 `◌` 추가하고, 목록 설명에 확인 기준 폰트로 D2Coding을 더한다(목록 글자는 두 폰트 모두에 있고 한 칸 폭).

## 데이터 모델
위 인터페이스로 충분하다. 불변식: `attached_to`가 `Some`인 요소만 방해 여부를 가진다(타입으로 보장).

## 오류 처리
- **사용자 입력 오류**: 잘못된 `cancelActivity`, `cancel_activity` 값(1.5, 1.6)과 오류, 취소 트리거 비방해(3.1)는 파서 오류 또는 모델 오류 -> 기존 경로로 원문 코드블록.
- **외부 자원 오류**: 해당 없음.
- **시스템 오류**: 해당 없음(새 panic 경로 없음).
- **기능 강등**: 원문 코드블록 물러남(기존).

## 테스트 전략
- **깊이**: Standard - 파싱, 검사, 그리기 세 곳의 새 분기, 상태 기계나 외부 연동 없음.
- **단위**: parse_xml `cancelActivity` 없음, `"true"`, `"false"`, `"no"`(1.1, 1.2, 1.5); parse_yaml 같은 네 경우와 비경계 노드의 키(1.3, 1.4, 1.6); validate 오류, 취소, 타이머, 트리거 없음 x 방해, 비방해(3.1, 3.2); shape 글자 표(2.1, 2.2).
- **통합**: `bpmn::render`로 방해, 비방해 경계가 함께 있는 XML, YAML을 LR, TB 두 방향으로 그려 이벤트 글자가 `◎`, `◌`로 다르고 경계 간선과 나가는 흐름이 실선(2.3~2.5).
- **E2E**: 해당 없음(biz-process 없음).
- **인수**: `dg -P -s none`으로 `examples/bpmn.md`의 새 비방해 예제를 실제로 그려 확인, 기존 예제 출력 바이트 동일(4.1).
- **성능**: 해당 없음.
- **속성 기반**: P1: 비방해 경계가 없는 무작위 YAML 문서의 렌더링은 이 기능 전과 같다 - 4.1 - `flow_properties.rs`의 seed 고정 생성기 문서. P2: 비방해 경계 이벤트를 넣은 무작위 문서의 렌더링 글자는 모두 `CJK_MONO_COVERED_GLYPHS` 안이다 - 4.2 - 같은 생성기에 경계 이벤트 추가.

## 파일 구조 계획
```
src/diagram/ir.rs                     Shape::Event 필드형                                code
src/diagram/bpmn/model.rs             Attachment                                         code
src/diagram/bpmn/parse_xml.rs         cancelActivity 해석, InvalidCancelActivity        code
src/diagram/bpmn/parse_yaml.rs        cancel_activity 해석, InvalidBoolean              code
src/diagram/bpmn/parse_bizprocess.rs  Attachment 생성(항상 방해)                         code
src/diagram/bpmn/validate.rs          BoundaryTriggerMustInterrupt                       code
src/diagram/bpmn/lower.rs             shape_of 방해 여부 전달, attached_to.host 사용     code
src/diagram/layout/shape.rs           이벤트 글자 표, 커버리지 목록                      code
src/diagram/layout/graph.rs, src/diagram/ir.rs 테스트   Shape::Event 생성 지점 갱신      code
src/diagram/bpmn/flow_properties.rs   P1, P2                                             code
examples/bpmn.md                      비방해 경계 이벤트 예제                            document
```
