# Research & Design Decisions — bpmn-model

## Summary
- **Feature**: `bpmn-model`
- **Discovery Scope**: Extension(`bpmn-lane-layout`·`bpmn-shapes`가 만든 `ir` 어휘 위에
  의미 계층 한 겹 + `diagram::mod` 배선). 아키텍처·도형 매핑표·검증 규칙의 SSoT는
  `../bpmn-support/research.md` — 여기는 현재 코드로 재확인한 결과와 그 문서가 비워 둔
  결정만 적는다.
- **Key Findings**(2026-09-26, `src/diagram/{ir,mod}.rs`·`layout/{graph,shape}.rs` 직접 확인):
  - `ir::EventPosition`·`Shape::{Event, Subprocess}`·`Marker::Slash`·`GroupKind::Lane`·
    `Graph::add_lane()`이 전부 구현돼 있고, `bpmn-shapes/design.md` §"lower()가 만들어야
    할 라벨" 계약이 그대로 유효하다 — 이 스펙은 `ir`·`layout`을 한 줄도 고치지 않는다.
  - `Language`는 `diagram::mod`의 두 변형 enum이고, 이름 문자열이 `render()`(`match`)와
    `main.rs:58`(`if == Mermaid … else "plantuml"`)에 중복돼 있다 — 변형을 더하면
    `main.rs` 쪽이 `Bpmn`을 `plantuml`로 잘못 적는다. 확장자 표도 `language_of_source()`와
    `main::looks_like_diagram_file()`에 중복돼 있다.
  - `layout::graph::render()`는 `graph.direction`을 선호 방향으로 삼아 반대 방향·층 접기로
    재시도한다 — 방향은 `Graph.direction`에 넣기만 하면 폴백 규약이 공짜(research.md
    Follow-up 7 확인).
  - 구성원 없는 레인은 `registered == false`라 그려지지 않는다(`bpmn-lane-layout` 결정,
    자리표시는 이 스펙 몫). `Shape::Anchor` 노드는 간선이 없어도 크기 `(3,1)`/`(1,3)`으로
    측정되고 그려지지 않으며, `is_movable() == false`·제목 오른쪽 배치 등 예외 처리가
    이미 있다(`graph.rs:331,427,873`).
  - `DiagramOptions::apply_to_graph()`가 방향 옵션을 `Graph`에 덮어쓴다 — 모든 그래프 계열
    파서가 `parse → apply_to_graph → render` 순서를 따르므로 BPMN도 같은 자리(lower 뒤).

## Research Log

### 노드 소속 표현 — `pool`+`lane` 두 필드 vs 하나
- **Context**: `bpmn-support/research.md`는 `FlowNode{kind, pool, lane, attached_to}`(이 스펙에서
  메타모델 이름 `Element`·`Participant`로 바뀜, 아래 Decision 참고).
- **Findings**: 레인은 항상 정확히 하나의 풀에 속하므로 `lane`이 있으면 `pool`은
  파생값이다. 두 필드를 두면 "레인이 그 풀에 속하는가" 검증 규칙과 불일치 상태가 생긴다.
- **Implications**: `container: Option<String>`(풀 id 또는 가장 안쪽 레인 id) 하나로.
  참여자는 `Model::participant_of_container()`로 조회. 파서는 레인 소속이면 레인 id, 아니면
  참여자 id를 넣는다.

### 빈 풀·레인의 자리표시
- **Context**: 협업 다이어그램의 블랙박스 풀(프로세스 없는 참여자)은 실제 XML에서 흔하다.
  v1은 풀 자체를 메시지 흐름 끝으로 못 쓰므로(그룹 닻 미보증) 노드 없는 풀이 생긴다.
- **Alternatives**: (1) 안 그림 — 참여자가 조용히 사라짐. (2) `Shape::Plain` 빈 노드 —
  측정 `(0,0)`이라 배치기 폭 0 블록 전제와 충돌 위험. (3) `Shape::Anchor` 노드 하나 —
  보이지 않고 예외 처리가 이미 있음.
- **Implications**: (3). 간선 없는 닻은 `assign_layers()`가 층 0에 두고 `group_members()`가
  비어 층 보정을 건너뛴다. **미검증** — 실제 렌더링은 태스크에서 테스트로 확인(Risks).

### 캡션 종류 이름
- **Findings**: 캡션 `◈ {언어} · {종류}`의 종류는 mermaid는 소스 키워드, PlantUML은 본문
  휴리스틱에서 나온다. BPMN은 모델이 있어야 종류를 안다(풀 수).
- **Implications**: 종류 = 풀 2개 이상 `collaboration`, 아니면 `process`(OMG 다이어그램
  종류 이름 그대로). `bpmn::kind_of(source)`는 파서가 생기기 전까지 `None`.

## Architecture Pattern Evaluation
`../bpmn-support/research.md` §Architecture Pattern Evaluation의 "`bpmn::Model` 공유 의미
계층" 채택을 그대로 따른다. 이 스펙 안의 선택지는 아래 결정 네 건.

## Design Decisions

### Decision: 충실도 대상은 BPMN 2.0.2 프로세스 모델(의미 계층), BPMNDI는 영구 제외 — 사용자 결정 2026-09-26
- **Context**: 로드맵이 `bpmn-xml`을 `bpmn-yaml` 앞에 둔 이유("모델 충실도")가 무엇에 대한
  충실도인지 확인 필요. 사용자 원문: "bpmn xml (bpmn 2.0.2 의 process model : 의미(다이어그램
  노드 아닌 process task event ...) 충실하게 하기를 기대한다. 프로세스 모델 이니까)".
- **Selected Approach**: `Model`은 `<process>`/`<collaboration>` 의미 트리(Participant·LaneSet/Lane·
  FlowElement·Artifact·SequenceFlow/MessageFlow/Association)만 담고, 다이어그램 교환 계층
  (`<bpmndi:*>`: 좌표·크기·waypoint·`BPMNShape`/`BPMNEdge`·`isHorizontal`) 개념은 하나도 넣지
  않는다 — "연기"가 아니라 영구 제외. 타입 이름도 메타모델 용어(`Participant`, `Element`,
  `ElementKind`)로 맞춘다("풀"은 그려진 띠를 가리키는 한국어 서술에만 남김).
- **Rationale**: dg는 자기 배치기(`layout::graph`)가 있어 다른 도구가 그린 좌표를 보존할 이유가
  없고, 의미 모델에 충실해야 세 문법이 같은 `Model`로 수렴한다.
- **Trade-offs**: `bpmn-support/research.md`의 XML v1 범위 문구 "`BPMNDI`의 `isHorizontal`만"은
  이 결정으로 대체된다 — `bpmn-xml`은 `<bpmndi:*>` 구획을 통째로 건너뛰고, 방향은 기본값
  (가로) + CLI 옵션으로만 정한다. 그 문서 갱신은 로드맵 소유자 몫(이 스펙은 기록만).
- **Follow-up**: `Orientation`은 DI 값이 아니라 dg 배치 힌트(mermaid `flowchart LR` 상당)임을
  타입 주석에 명시.

### Decision: 어휘 표는 `bpmn/vocabulary.rs` 한 파일, 토큰·라벨·종류를 한 튜플에
- **Context**: `bpmn-support/research.md`는 "kind/trigger 어휘는 BPMN XML 로컬 요소명을
  토큰으로, XML·YAML 파서가 같은 표 하나를 보게 — `bpmn-model`에서 확정".
- **Alternatives**: 1) `glyphs.rs`(로드맵 문구) — 이름이 "글자"만 담는 듯 읽힘. 2) 종류
  enum 옆 `model.rs`에 흩어진 `match` 셋 — 토큰과 라벨이 두 `match`로 갈라져 SSoT 약화.
- **Selected Approach**: `vocabulary.rs`에 종류별 `const` 튜플 표(`(종류, XML 로컬 이름,
  라벨)`) 하나씩 — `xml_name()`·`from_xml_name()`·`label()`이 같은 표를 읽는다.
- **Rationale**: 파일 이름이 내용(토큰+라벨 어휘)을 말한다. 로드맵의 `glyphs.rs` 문구는
  이 스펙 완료 시 한 줄 갱신.

### Decision: `Orientation`은 모델 고유 enum, `ir::EventPosition`은 재사용
- **Context**: 방향과 이벤트 위치를 `ir` 타입으로 직접 쓸지.
- **Selected Approach**: `Orientation::{Horizontal, Vertical}`(dg 배치 방향 힌트 — 위 결정대로
  BPMNDI `isHorizontal`을 읽는 자리가 아니라 YAML·CLI가 지정하는 값) → `lower()`가
  `Direction::{LeftRight, TopDown}`으로 한 번만 매핑. `EventPosition`은 같은 개념이 `ir`에 이미
  있으므로 재사용(`pub use`).
- **Rationale**: "가로 풀 = 흐름 왼쪽→오른쪽"은 렌더링 결정이라 세 파서가 각자 하면
  중복. 반면 이벤트 위치는 의미와 그림이 1:1이라 두 번 정의하면 SSoT 위반.

### Decision: 검증 결과는 `Result<(), Vec<ModelError>>`, 렌더 경로에선 `None`으로 강등
- **Context**: dg는 진단 채널이 없다(실패 = 원문 코드블록).
- **Selected Approach**: 위반 전부를 모아 돌려주고(요구사항 1.8), `render_model()`은
  `validate(..).ok()?`로 `None`. `ModelError`는 `Display`를 구현해 파서 스펙이 `DG_DEBUG`
  등으로 노출할 여지를 남긴다.
- **Rationale**: 첫 오류만 돌려주면 파서 fixture 디버깅이 반복적이다. 진단 UI는 이 스펙
  범위 밖.

### Decision: `render(source)`와 `render_model(model)` 분리, `kind_of`는 `None`
- **Context**: `Language::Bpmn` 배선은 소스 문자열을 받지만 이 스펙엔 파서가 없다.
- **Selected Approach**: `bpmn::render(source, …)`는 `kind_of(source)?`로 시작해 지금은
  항상 `None`(코드블록 폴백); 파서 스펙은 `kind_of`에 갈래(`<` → xml 등)와 `render`의
  `match` 팔 하나씩만 더한다. 모델 이후 경로(`validate → lower → apply_to_graph →
  layout::graph::render`)는 `render_model()` 하나에만 있다.
- **Rationale**: mermaid/PlantUML `render()`와 같은 모양(`kind_of` → `match kind`)이라
  `diagram::mod`가 세 언어를 똑같이 다룬다. 노드 없는 모델은 `render_model`이 먼저
  `None`(요구사항 1.9) — 자리표시 닻만 있는 그래프를 배치기에 넘기지 않는다.

## Risks & Mitigations
- 자리표시 `Anchor`가 간선 없이 레인 안에 홀로 있을 때의 배치는 코드 읽기로만 확인 —
  태스크에서 단위 테스트로 못박고, 안 되면 `Shape::Plain` + 공백 한 칸 본문으로 대체
- 풀 2개를 LR로 쌓으면 폭 100을 넘길 수 있음(8.3) — 기존 폴백(TB 재시도)이 `Some`을
  보장하므로 요구사항은 "렌더링됨"으로 두고 `{text}` 육안 확인만 추가
- `Language`에 변형을 더하면 외부 라이브러리 사용자의 `match`가 깨질 수 있음(`lib.rs`가
  `Language`를 재노출) — 0.x 버전이므로 수용, 변경 로그 한 줄

## References
- `../bpmn-support/research.md` §"세 파서는 `bpmn::Model`을 거쳐" · §"BPMN 도형·표식 어휘"
- `../bpmn-shapes/design.md` §Key Decisions "lower()가 만들어야 할 라벨" — 라벨 계약
- `../bpmn-lane-layout/design.md` §Out-of-Scope "구성원 없는 레인" — 자리표시 결정 위임
- OMG BPMN 2.0.2 §9.2(Participant/Pool·시퀀스 흐름 경계), §9.4(Message Flow), §10.2(Process·
  FlowElement·LaneSet), §10.5.6(Boundary Event); §12(BPMNDI)는 의도적으로 참조하지 않음
