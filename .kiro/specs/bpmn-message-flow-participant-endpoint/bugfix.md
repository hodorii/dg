# Bugfix — bpmn-message-flow-participant-endpoint

## 정의
블랙박스 풀(내부 요소를 그리지 않는 외부 참여자)과 메시지를 주고받는 협업을 그리려는 사용자를
위해, 메시지 흐름의 끝이 참여자 id일 때 모델이 거부되고 간선이 사라지는 결함을 고치는 수정이다.

## 재현 절차
1. 환경: 커밋 `faae8e4`(bpmn-model 구현) 기준, 파서 없이 `bpmn::model::Model`을 손으로 만든다
   (`src/diagram/bpmn/lower.rs`에 임시 `#[cfg(test)]` 모듈을 붙여 `cargo test --lib`로 실행한 뒤
   `git checkout`으로 되돌림).
2. 입력: 참여자 `customer`(고객, 요소 없음)·`seller`(판매사, 레인 `sales`); 레인 `sales` 안에 시작
   이벤트 `start`와 `«user»` 태스크 `review`; 시퀀스 흐름 `s1: start → review`; 메시지 흐름
   `m1: customer → start`(라벨 `주문`, `FlowKind::Message`).
3. 관찰(검증): `validate(&model)` → `Err([UnknownReference { owner: "m1", reference: "customer" }])`.
   모델 전체가 거부돼 `render_model`은 `None`(원문 코드블록 폴백).
4. 관찰(변환): 검증을 건너뛰고 `lower(&model)`를 부르면 `graph.edges.len() == 1` — `m1`이 오류
   없이 조용히 사라진다.
5. 관찰(대조): `m1.source`를 레인 id `sales`로 바꾸면 `UnknownReference { owner: "m1",
   reference: "sales" }` — 레인 끝도 지금 거부되며, 이것은 유지해야 할 동작이다(BPMN 2.0.2에서
   `MessageFlow.sourceRef/targetRef`는 InteractionNode = 흐름 노드 또는 `Participant`; 레인은 아니다).
6. 관찰(배치 가능성): 4의 그래프에서 고객 풀의 자리표시 노드를 지우고 `Graph::group_anchor(고객
   그룹)` → `start` 점선 간선(`○` 꼬리·열린 화살촉·라벨 `주문`)을 손으로 넣어 `layout::graph::render`
   (폭 100)를 부르면 LR·TD 모두 패닉 없이 그려지고 고객 띠와 `start` 사이에 `╌` 점선이 이어진다.
   닻을 지우지 않고 더하면(같은 풀에 보이지 않는 노드 2개) 띠가 빈 줄 3개만큼 늘어난다. 풀 쪽 끝의
   `○`·열린 화살촉은 LR에서 풀 테두리에 놓이지 않는 경우가 있다(배치기의 레인 그룹 닻 처리).

## Boundary Context
- **In scope**: `FlowKind::Message` 흐름의 `source`/`target`이 **최상위 참여자 id**일 때 — 검증 통과
  (풀 사이 규칙은 그대로 적용), 변환 시 그 참여자 띠를 가리키는 간선(`ir::Graph::group_anchor`,
  이미 존재)으로 렌더링, 블랙박스 풀 띠·제목 유지, 풀 안 보이지 않는 노드는 하나만
- **Out of scope**: 시퀀스·연관·데이터 연관 흐름의 끝(요소만 — 지금 그대로), 레인 id를 메시지 흐름의
  끝으로 쓰는 것, 풀 테두리 위 표식·라벨 배치 품질(`layout::graph`의 레인 그룹 닻 처리 — 6의 관찰,
  별도 배치 스펙 몫), `bpmn-xml`의 `messageFlow` 처리(그 스펙이 이 수정을 반영), DI 제외·도형 어휘·
  레인 배치 논리(`bpmn-model`·`bpmn-shapes`·`bpmn-lane-layout` 확정분), 예제 문서

## Behaviors

### 1. 현재 동작 (결함)
- 1.1: [메시지 흐름의 한쪽 끝이 최상위 참여자 id(요소 없는 블랙박스 풀), 다른 끝은 다른 풀 안 요소]
  → [모델이 `UnknownReference`(흐름 id, 참여자 id)로 거부되고 그림이 나오지 않는다]
- 1.2: [1.1의 모델을 검증 없이 변환] → [그 메시지 흐름만 오류 없이 사라지고 나머지 간선만 남는다]
- 1.3: [메시지 흐름의 양끝이 서로 다른 최상위 참여자 id(블랙박스 풀 둘, 다른 요소는 있음)] → [두 끝
  모두 `UnknownReference`로 거부된다]
- 1.4: [메시지 흐름의 한쪽 끝이 참여자 id, 다른 끝이 그 참여자 안 요소] → [`UnknownReference`로
  거부된다 — 거부는 맞으나 이유가 다르게 보고된다]

### 2. 기대 동작
- 2.1: [메시지 흐름의 한쪽 끝이 최상위 참여자 id, 다른 끝은 다른 풀 안 요소] → [거부되지 않고 그림이
  나온다(캡션 `collaboration`); 블랙박스 풀 띠와 제목은 그대로 그려지고, 그 요소와 풀 띠 사이에 다른
  메시지 흐름과 같은 점선(`╌`)이 이어진다; 가로·세로 모두 패닉 없음]
- 2.2: [2.1의 모델을 변환] → [그 메시지 흐름이 다른 메시지 흐름과 같은 선 종류·꼬리·머리·라벨의
  간선으로 남고, 참여자 쪽 끝은 그 참여자 띠 안의 보이지 않는 닻이며, 블랙박스 풀 안 보이지 않는
  노드는 하나뿐이다]
- 2.3: [메시지 흐름의 양끝이 서로 다른 최상위 참여자 id] → [거부되지 않고 두 풀 띠 사이에 점선이
  이어진다; 패닉 없음]
- 2.4: [메시지 흐름의 한쪽 끝이 참여자 id, 다른 끝이 그 참여자 안 요소] → [`MessageFlowWithinParticipant`
  (흐름 id)로 거부된다]

### 3. 불변 동작 (회귀 방지)
- 3.1: [시퀀스·연관·데이터 연관 흐름의 끝이 참여자 id] → [지금처럼 `UnknownReference`로 거부된다
  (요소만 끝이 될 수 있다)]
- 3.2: [메시지 흐름의 끝이 레인 id(최상위 참여자 아님)] → [지금처럼 `UnknownReference`로 거부된다]
- 3.3: [메시지 흐름의 양끝이 요소이고 같은 풀, 또는 둘 다 풀 밖] → [`MessageFlowWithinParticipant`로
  거부된다(bpmn-model 1.3)]
- 3.4: [메시지 흐름의 양끝이 서로 다른 풀 안 요소] → [점선 + `○` 꼬리 + 열린 화살촉(bpmn-model 3.3)]
- 3.5: [메시지 흐름이 닿지 않는, 노드 없는 풀·레인] → [띠와 제목이 그대로 그려지고 보이지 않는
  자리표시가 하나만 있다(bpmn-model 4.3)]
- 3.6: [존재하지 않는 id를 끝으로 하는 흐름] → [`UnknownReference`로 거부되고 변환은 그 흐름을
  건너뛴다(bpmn-model 1.4)]
- 3.7: [노드가 하나도 없는 모델(참여자 사이 메시지 흐름만 있어도)] → [거부되지 않되 그림도 나오지
  않는다(bpmn-model 1.9)]
