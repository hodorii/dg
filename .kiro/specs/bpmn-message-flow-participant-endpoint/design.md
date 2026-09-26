# Design — bpmn-message-flow-participant-endpoint

## 정의
블랙박스 풀과 메시지를 주고받는 협업을 그리기 위해, `FlowKind::Message` 흐름의 끝으로 최상위
참여자 id를 허용하고 그 끝을 참여자 띠의 그룹 닻(`ir::Graph::group_anchor`)에 잇는 수정이다.
BPMN 2.0.2 `MessageFlow.sourceRef/targetRef`의 타입은 InteractionNode(흐름 노드 + `Participant`)다.

## 원인 (Root Cause)
- `src/diagram/bpmn/validate.rs` `validate()` 참조 해석 루프(67~80행): `flow.source`/`flow.target`를
  `element_ids`(= `model.elements`의 id)에만 대조한다. 참여자 id는 `container_ids`에만 있고 흐름 끝
  해석에는 쓰이지 않으므로 참여자 끝은 `UnknownReference`가 되고(1.1, 1.3) `broken_flow`에 들어가
  `check_message_flows`가 그 흐름을 건너뛴다 — 1.4에서 `MessageFlowWithinParticipant` 대신
  `UnknownReference`가 나오는 이유.
- 같은 파일 `pool_of()`(131~137행): `model.element(id)?`로 시작해 요소가 아니면 `None`(건너뜀). 참조
  해석이 통과하더라도 참여자 끝의 풀 인덱스를 알 수 없다.
- `src/diagram/bpmn/lower.rs` `add_flows()`(80~86행): 양끝을 `graph.find(id)`(노드만)로 찾고 하나라도
  `None`이면 `continue` → 간선이 조용히 사라진다(1.2). `add_pools_and_lanes()`가 만드는 참여자·레인 id
  → 그룹 인덱스 표 `group_of`는 `add_flows`에 전달되지 않고, `Graph::group_anchor()`는 `bpmn::lower`
  어디에서도 호출되지 않는다.
- 배경: bpmn-model design.md Out-of-Scope "풀·레인을 흐름 끝으로(그룹 닻) — 미지원"과 Revalidation
  Trigger "하류 스펙이 풀을 끝으로 하는 메시지 흐름을 실제로 요구하면 그때 결정"의 조건이 `bpmn-xml`
  (`<messageFlow sourceRef="참여자">` 블랙박스 관례)로 충족됐다.

## 수정 방식
- **`model.rs`** — 조회 도우미 하나 추가. 정확 일치만, 레인 id는 `None`(3.2):
  ```rust
  impl Model {
      /// 최상위 참여자 id 정확 일치 → 인덱스. 레인 id는 None(레인은 InteractionNode가 아니다).
      pub fn participant_index(&self, id: &str) -> Option<usize>;
  }
  ```
- **`validate.rs`** — 두 곳.
  - 참조 해석 루프: 끝 id가 유효한 조건을 `element_ids.contains(id) || (matches!(flow.kind,
    FlowKind::Message) && model.participant_index(id).is_some())`로. 시퀀스·연관·데이터 연관은 조건이
    바뀌지 않아 참여자 끝이 그대로 `UnknownReference`(3.1).
  - `pool_of(model, broken_container, endpoint_id)`: 맨 앞에 `if let Some(i) = model.participant_index(
    endpoint_id) { return Some(Some(i)); }`. 이후 `check_message_flows`는 무변경 — 양끝 풀이 둘 다
    `Some`이고 다르면 통과(2.1, 2.3), 같으면 `MessageFlowWithinParticipant`(2.4). 시퀀스 흐름은 참여자
    끝이 `broken_flow`로 먼저 걸러져 이 분기에 닿지 않는다.
- **`lower.rs`** — 두 곳.
  - `add_flows(graph, model, group_of)`: 끝 해석을 `graph.find(id)` → 실패 시 `FlowKind::Message`이고
    `model.participant_index(id)`가 `Some`이면 `graph.group_anchor(group_of[id])` → 둘 다 아니면 건너뜀
    (3.6). 선 종류·표식·라벨은 기존 `line_for` 표 그대로(2.2).
  - `lower()` 호출 순서: `add_flows`를 `add_placeholders_for_empty_groups` **앞**으로. 닻(`@group-anchor:
    {group}`, `Shape::Anchor`)이 그 그룹의 노드로 집계돼 자리표시가 추가되지 않는다(`has_element`는
    도형을 가리지 않고 모든 노드를 세므로 변경 없음) — 재현 6: 닻 2개면 띠가 빈 줄 3개만큼 늘어난다.
    흐름이 닿지 않는 빈 풀은 종전대로 `@bpmn-placeholder:` 하나(3.5).
- 기각한 대안: (a) `Flow`의 끝을 `enum FlowEnd { Element(String), Participant(String) }`로 — id가
  참여자·레인·요소·흐름 전체에서 고유(`check_duplicate_ids`)하므로 평문 id로 이미 결정되며, 세 파서와
  모든 픽스처가 바뀌는 대가만 있다. (b) `participant_of_container`로 해석 — 레인 id까지 허용돼 3.2
  위반. (c) 검증은 그대로 두고 `bpmn-xml`이 그런 흐름을 버림(현 5.5) — 사용자가 지적한 프로세스 정보
  손실.

## 검증 속성
- (a) 결함 재현: `validate` 단위 테스트 — 재현 절차 2의 모델이 `Ok`, 양끝 참여자 모델이 `Ok`, 참여자 +
  자기 요소가 `Err([MessageFlowWithinParticipant])`; `lower` 단위 테스트 — 간선 2개. 수정 전 코드에서
  `UnknownReference`·간선 1개로 **실패**함을 확인(1.1~1.4).
- (b) 기대 동작: 위 테스트 통과(2.1, 2.3, 2.4); `lower` — 간선 `(Dashed, Circle, OpenArrow, "주문")`,
  한쪽 끝 노드가 `shape == Anchor`·`group == 고객 그룹`, 그 그룹의 `Anchor` 노드 수 1(2.2);
  `bpmn::tests` 통합 — `render_model` `Some(("collaboration", _))`, 평문에 `고객`·`╌` 포함, LR·TD,
  양끝 참여자(닻 ↔ 닻)도 패닉 없음(2.1, 2.3).
- (c) 불변 동작: 기존 `validate`·`lower`·`bpmn::tests` 전량 무수정 통과(3.3~3.7); 신규 — 시퀀스 흐름의
  참여자 끝 → `UnknownReference`(3.1), 메시지 흐름의 레인 끝 → `UnknownReference`(3.2). 수정 전에도
  통과.

## 영향 범위
- `src/diagram/bpmn/model.rs`: `Model::participant_index` + 단위 테스트(레인 id → `None`).
- `src/diagram/bpmn/validate.rs`: 흐름 끝 해석 조건, `pool_of` 참여자 분기, 테스트.
- `src/diagram/bpmn/lower.rs`: `add_flows` 시그니처·끝 해석, `lower()` 호출 순서, 테스트.
- `src/diagram/bpmn/mod.rs`: 통합 테스트 추가만.
- 변경 없음: `ir.rs`·`layout/*`·`vocabulary.rs`·`diagram/mod.rs`. bpmn-model Boundary Commitments 유지 —
  `ir`·`layout` 무변경, 의존 방향 `ir` ← `model` ← `validate` ← `lower`. bpmn-model design.md Out-of-Scope의
  "풀·레인을 흐름 끝으로 — 미지원"은 이 스펙으로 "참여자만 지원, 레인 미지원"이 된다(해당 줄에 포인터).
- 하류: `bpmn-xml` requirements 5.5("참여자 끝 → 그 흐름만 그려지지 않음")·design "참여자 끝 → 제거"·
  Out-of-scope "풀·레인 자체를 끝으로 하는 메시지 흐름"이 이 수정과 충돌 — 그 스펙이 "참여자 끝 유지,
  레인 끝만 제거"로 바꿔야 한다(이 스펙은 편집하지 않음).
- 잔여(별도 스펙 후보): 레인 그룹 닻 간선의 풀 쪽 `○`·열린 화살촉이 LR에서 테두리에 놓이지 않을 수
  있음 — `layout::graph` 몫, 이 수정으로 도입되는 것이 아니라 드러나는 것.
