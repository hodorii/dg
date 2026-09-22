# Brief: 다이어그램 표기 식별성 개선

## Problem
시퀀스 다이어그램에서 `alt`/`opt`/`loop` 등 프래그먼트 테두리가 메시지·생명선(흐름)과 똑같은
굵기·선 종류로 그려져 한눈에 구분되지 않는다는 사용자 보고가 있었다. 같은 문제(구조가 다른
요소인데 선패턴이 같아 식별이 어려움)가 다른 다이어그램에도 있는지 검토가 필요했다.

## Current State
- 실제 재현(`sequenceDiagram` + `alt`/`else`): 프래그먼트 테두리가 `LineKind::Solid` +
  `theme.diagram_group`(회색 계열)로 그려진다(`layout/sequence.rs:497`). 메시지·생명선도
  `LineKind::Solid`(또는 파싱된 화살표 종류) + `theme.diagram_group`과 아주 가까운 회색
  `theme.diagram_line`으로 그려진다 — 즉 프레임과 흐름이 **같은 선 종류**이고 색도 둘 다
  회색 계열이라 명도 차이가 작다. `--style none`에서는 색이 아예 빠지므로 완전히 동일한
  글자(`─`/`│`/`┌`)로만 보여 구분 수단이 없다.
- 같은 파일 안에 이미 반례가 있다: 노트(`Note`)는 `LineKind::Dashed` + `theme.diagram_note`로
  (line 470), 활성화 막대(activation bar)는 `LineKind::Heavy` + `theme.diagram_accent`로(line 439)
  이미 흐름과 다른 선패턴을 쓴다 — "구조가 다른 요소는 선패턴도 다르게" 원칙이 부분적으로만
  적용돼 있다.
- 같은 원칙의 선례가 두 군데 더 있다: `block-beta`의 중첩 그룹은 깊이별로
  `Solid→Dashed→Heavy`를 순환하는 `border_kind(depth)`(`layout/block.rs:176`)를 쓰고,
  방금 완료된 `gitgraph-branch-distinction` 스펙은 트랙(브랜치)별로 같은 3종 `LineKind`를
  순환하는 `branch_style()`을 `gitgraph.rs`에 추가했다 — 새 렌더링 로직 없이 기존
  `canvas.rs`의 `LineKind` 3종만 재배정하는 방식이 이미 두 번 검증됐다.
- 다른 다이어그램형도 검토했다: `flowchart`/`classDiagram`/`stateDiagram`/`erDiagram`/컴포넌트는
  전부 `layout/graph.rs` 하나를 공유한다(README). 여기서 서브그래프·합성 상태·패키지 같은
  "그룹" 테두리는 `LineKind::Solid` + `theme.diagram_group`(회색)을 쓰고, 일반 노드 테두리는
  `LineKind::Solid` + `theme.diagram_box`(파란 계열, `layout/shape.rs:46`)를 쓴다 — **선패턴은
  똑같이 Solid**지만 **색상 계열 자체가 다르다**(회색 대 파랑). 색이 있는 테마에서는 구분되고,
  `--style none`에서만 모호해진다 — 시퀀스의 "회색끼리 부딪히는" 상황보다는 덜 심각하다.
  `xychart`/`pie`/`quadrant`/`gantt`는 그룹·프레임 개념이 없어 해당 없음.

## Desired Outcome
- 시퀀스 `alt`/`opt`/`loop`/`par`/`critical`/`break` 프래그먼트 테두리가 메시지·생명선과
  선패턴으로 뚜렷이 구분되어, `--style none`에서도 "이건 흐름이고 이건 프레임 경계"를 글자
  모양만으로 알 수 있다.
- (검토 결과) `graph.rs` 공유 그룹 vs 노드 구분은 색상으로 이미 되고 있어 이번 라운드의
  실행 범위에는 넣지 않는다 — 심각도가 낮고, `graph.rs`는 5개 다이어그램형이 공유하는
  핵심 파일이라 건드리면 블라스트 반경이 크다. 후속 검토 항목으로만 기록한다.

## Approach
`block-beta`의 `border_kind(depth)`, `gitgraph-branch-distinction`의 `branch_style()`과 같은
패턴을 재사용해 시퀀스 프래그먼트 테두리에 아직 그 파일에서 안 쓰는 `LineKind::Heavy`를
적용한다(노트가 이미 `Dashed`를 쓰므로 재사용하면 "프레임인지 노트인지" 새 혼동이 생김 —
`Heavy`가 남는 선택지). 새 렌더링 로직을 만들지 않고 기존 `canvas.rs`의 `LineKind` 3종 안에서
재배정만 한다.

## Scope
- **In**: 시퀀스 프래그먼트(`alt`/`opt`/`loop`/`par`/`critical`/`break`) 바깥 테두리의
  `LineKind` 변경, `else`/`option`/`and` 구분선과의 관계 정리(프레임과 같은 패턴으로 통일할지
  기존 `Dashed`를 유지해 "프레임=Heavy, 분기=Dashed"로 이원화할지는 requirements에서 결정)
- **Out**: `graph.rs` 공유 레이아웃(서브그래프·합성 상태·패키지 그룹 vs 노드) 선패턴 변경,
  `theme.rs`의 색상 값 자체 변경(SSoT 유지 — `LineKind`만 바꾼다), `xychart`/`pie`/`quadrant`/
  `gantt`(그룹·프레임 개념 없음, 해당 없음)

## Boundary Candidates
- 시퀀스 프래그먼트 테두리 `LineKind`
- (참고, 이번 스코프 아님) `graph.rs` 그룹 테두리 vs 노드 테두리의 선패턴 구분 — 색상만으로도
  이미 구분되므로 우선순위 낮음

## Out of Boundary
- `graph.rs` 공유 레이아웃의 그룹/노드 선패턴 변경 — 심각도 낮고 블라스트 반경 큼, 별도 라운드에서
  다시 검토
- `theme.rs` 색상 값(RGB/인덱스) 자체를 바꾸는 것 — 이번 문제는 선패턴(`LineKind`) 재배정으로
  풀리고, 색상 SSoT는 건드릴 이유가 없음

## Upstream / Downstream
- **Upstream**: `diagram/layout/sequence.rs`(프래그먼트 렌더, `draw_message`/`draw_self_message`),
  `diagram/layout/block.rs`(`border_kind` 선례), `diagram/layout/gitgraph.rs`(`branch_style` 선례),
  `diagram/canvas.rs`(`LineKind` 정의)
- **Downstream**: 기존 시퀀스 테스트 중 프레임 테두리 글자(`┌`/`─`/`│` 등)를 골든 텍스트로 직접
  비교하는 테스트가 있으면 새 글자(`┏`/`━`/`┃`)에 맞춰 조정 필요

## Existing Spec Touchpoints
- **Extends**: 없음(신규 스펙)
- **Adjacent**: `sequence-self-message-clearance`(같은 `sequence.rs`, 다른 관심사 — 자기 메시지
  여백 vs 프래그먼트 테두리 선패턴, 서로 겹치지 않음), `gitgraph-branch-distinction`(같은
  `LineKind` 재배정 기법의 선례)

## Constraints
- 신규 의존성 없음(현재 4개 유지)
- panic 금지(`panic = "abort"`) 계약 유지
- `edition = "2024"`, `rust-version = "1.88"` 유지
- 기존 `canvas.rs`의 `LineKind`(Solid/Dashed/Heavy) 3종 안에서만 해결 — 새 선 종류를 추가하지 않음
