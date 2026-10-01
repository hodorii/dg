# Research & Design Decisions — bpmn-gateway-fixed-size

## Summary
- **Feature**: `bpmn-gateway-fixed-size`
- **Discovery Scope**: Extension — `bpmn-event-notation-anchor`가 만든 "점 고정 + 바깥 텍스트" 패턴을
  게이트웨이로 넓힐지 판단. requirements 단계 산출물(로드맵 지시: "현재 방식과 후보 방식을 나란히
  렌더링해 비교"). 실물 렌더링 원문은 `render-comparison.md`(표본 1~5).
- **Key Findings**(2026-09-29, `cbfd774` 기준 현행 바이너리 + 격리 worktree 프로토타입, 이 머신):
  - 현행 마름모는 `(tw+6, th+2)`로 이름에 비례해 자란다. 긴 이름(`+ 모든 사전 준비가 끝난 뒤 병합`)은
    30×4, 이름 없는 `○`는 7×3, 왼쪽→오른쪽에서 출구 3개인 `+ 병렬 분기`는 접점 벌림으로 17×7이 된다
    (표본 1·4 현행) — 같은 그림 안에서 게이트웨이 크기가 5×3~30×4로 흩어진다.
  - **후보 A(기호 한 글자, 이벤트 패턴 그대로)는 기각 사유가 명확하다**: 이름 없는 포괄 게이트웨이 `○`이
    시작 이벤트 `○`과 글자·크기·접점까지 완전히 같아 구분 불가(표본 1 후보 A 끝부분 `○` 단독), 이벤트기반
    `◎`도 중간 이벤트와 같다. `+`는 선 교차 글자처럼 보인다(표본 3 후보 A `+` 단독).
  - **후보 B(고정 5×3 마름모 + 기호 안 + 이름 오른쪽)**는 bpmn.io 기하에 가장 가깝고 위→아래에서
    깨끗하다: 접점이 마름모 중심 열 하나로 모여 `├`로 갈라지고 라벨 `예`·`아니오`가 다 보인다(표본 3·5).
    폭도 준다(협업 그림 89→87, 기본 LR 111→105, 기본 TD 32→30).
  - **후보 C(이름 아래)**는 위→아래에서 이름이 마름모와 흐름선 사이에 끼고(표본 1·3 후보 C), 왼쪽→오른쪽
    에서는 노드 폭이 이름 폭이 되어 마름모와 출구 선 사이에 이름 폭만큼 빈틈이 생긴다(표본 2 후보 C
    `‹ + ›                   ─╮`).
  - **점 고정 공통 결함(왼쪽→오른쪽)**: 라벨 붙은 출구가 둘이면 같은 줄에서 나가며 라벨 하나가 사라진다
    — 표본 4 후보 A·B·C 모두 `─아니오─┬▶`만 남고 `예`가 없다(현행은 `›─예──╮`/`›─아니오─╮` 둘 다 보임).
    이벤트는 출구가 보통 하나라 드러나지 않던 배치기 한계. 채택 시 design이 풀어야 할 결함.
  - **점 고정 공통 특성(위→아래)**: 이름이 길어 층에서 가장 넓은 점 고정 노드는 원점이 0열에 놓여 흐름선이
    왼쪽으로 꺾인다(`╭──╯`). 현행 이벤트도 같다(`◎ 모든 사전 준비가 끝난 뒤 병합` 실측, 아래 Research Log)
    — 이 스펙의 신규 결함이 아니라 기존 특성.
  - 프로토타입은 `Shape::Diamond`를 건드리지 않고 `Shape::Gateway` 변형을 더했다 → mermaid `{}` 판단 노드·
    상태도 `<<choice>>`·PlantUML diamond 출력은 바이트 동일(대조 입력 `cmp` 일치). `Diamond` 자체를 바꾸면
    그 셋이 전부 바뀐다 — 범위 결정 사항.
  - `is_point_anchored: bool`만으로는 후보 B·C를 못 만든다. 이벤트는 접점 = 원점 칸이지만 5×3 마름모의
    접점은 원점에서 (2, 1) 떨어진 중심 칸이라, 배치기 `center()`·`spread()`가 방향별 오프셋을 알아야 한다
    (프로토타입에서 `point_anchor() -> Option<(dx, dy)>`로 일반화, 나머지 소비 지점 둘 — 접점 벌림 제거·
    접점 밀기 제외 — 은 그대로 성립).

## Research Log

### 현행 렌더링 실측
- **Context**: 크기·접점이 이름 길이·출구 수에 따라 어떻게 변하는지 원문 확보.
- **Sources Consulted**: `target/release/dg -P -s none`(`cbfd774`), 입력 세 가지(스크래치, 아래).
- **Findings**: `render-comparison.md` 표본 1~5 "현행". 이름 없는 `○` 7×3, `× OK?` 9×3, 긴 이름 30×4(자동
  줄바꿈으로 두 줄), LR 출구 3개 `+ 병렬 분기` 17×7·입구 3개 합류 `+` 7×7. 협업 그림(`examples/bpmn.md`
  HEAD 둘째 블록)은 폭 89. `gw-split.md`는 `-w 100`에서 LR로 접혀 TD 비교는 `-w 140 --direction tb`로 채취.
- **Implications**: 현행은 "같은 종류 도형이 같은 크기"라는 bpmn.io 관례와 가장 멀지만, 이름·라벨이 어떤
  방향에서도 하나도 소실되지 않는다.

### 입력 표본(재현용)
- `gw-basic`: 시작 → 검토(`userTask`) → `exclusiveGateway OK?` → 처리 → `parallelGateway 모든 사전 준비가
  끝난 뒤 병합` → 마무리 → `inclusiveGateway`(이름 없음) → 끝. 위→아래 `-w 140`·왼쪽→오른쪽 `--direction lr`.
- `gw-split`: 주문 → `exclusiveGateway 재고 있음?` ─예→ `parallelGateway 병렬 분기` → 포장·송장 발행·결제 확인
  → `parallelGateway`(이름 없음) → 출고; ─아니오→ 품절 안내 → 취소.
- `collab`: `git show cbfd774:examples/bpmn.md` 둘째 코드펜스(작업 트리 사본은 다른 세션이 수정 중이라 HEAD 사용).
- 대조군 `control`: mermaid `flowchart` `{승인?}` + `stateDiagram-v2` `<<choice>>`.

### 프로토타입(격리 worktree, 폐기)
- **Context**: 후보를 예측이 아니라 실물로 보기 위해. 저장소 본체 무변경(`git status` 확인).
- **Method**: `git worktree add --detach <scratch>/proto cbfd774` → `ir.rs` `Shape::Gateway` + `point_anchor()`
  `Option<(usize, usize)>`(Event `(0,0)`, Gateway A `(0,0)`/B·C `(2,1)`), `graph.rs` `LayoutNode.anchor_offset`
  (방향별 dx 또는 dy)를 `center()`·`spread()`에 더함, `shape.rs` `measure`/`draw` Gateway 분기(환경 변수
  `DG_GW_MODE=a|b|c`), `lower.rs` `Gateway → Shape::Gateway`. 라벨 `"× 이름"`을 그리기 쪽에서 첫 글자·나머지로
  갈랐다(프로토타입 편의; 실제 설계는 `lower()`가 기호·이름을 따로 넘기는 편이 맞다). 4파일 +83/−5줄.
  worktree·빌드 산출물 삭제 후 `git worktree list`에 본체만 남음.
- **Findings**: Key Findings 그대로. 크기: B 5×3 고정(이름은 오른쪽 6열부터, 두 줄 이름은 둘째 줄이 마름모
  아래 줄 옆에 옴 — 표본 1 후보 B `╲─╱  병합`), C 5×3 + 이름 줄 수.
- **Implications**: 후보 B·C의 배치기 파급은 "술어를 오프셋으로 일반화" 수준 — `bpmn-event-notation-anchor`
  design의 소비 지점 4곳 중 `center()`·`spread()` 둘이 바뀌고 크기 계산·`resolve_port_swaps()` 제외는 그대로.
  LR 라벨 소실은 그 4곳 밖(라벨 배치)이라 design에서 별도 원인 조사 필요.

### 점 고정 노드의 왼쪽 꺾임이 기존 특성인지
- **Context**: 표본 1 후보 A·B·C에서 긴 이름 게이트웨이 앞 흐름선이 `╭──╯`로 꺾임.
- **Findings**: 같은 입력에서 그 게이트웨이를 `intermediateThrowEvent`(같은 긴 이름)로 바꿔 현행 바이너리로
  그리면 `╭────╯` / `◎ 모든 사전 준비가 끝난 뒤 병합`(0열)로 똑같이 꺾인다.
- **Implications**: 이 스펙 범위 밖의 기존 특성. 게이트웨이를 점 고정으로 바꾸면 게이트웨이에도 같은 특성이
  생긴다는 것만 requirements에 "알려진 특성"으로 적는다.

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations |
|---|---|---|---|
| 현행 유지 | `(tw+6, th+2)` 내용 적응형 마름모, 기호+이름 안 | 코드 0, 라벨 소실 없음, 어떤 방향도 안정 | bpmn.io 기하와 가장 멂, 같은 그림에서 5×3~30×4 |
| A 기호 한 글자 | 이벤트 패턴 그대로(`is_point_anchored` 확장만) | 코드 최소 | `○`·`◎`가 이벤트와 구분 불가 — 실물로 기각 근거 확보 |
| B 고정 마름모 + 이름 오른쪽 | 5×3, 기호 안, 접점 중심 | bpmn.io에 가장 가깝고 TD 깨끗, 폭 감소 | 배치기 오프셋 일반화, LR 라벨 소실 해결 필요, 왼쪽 꺾임(기존 특성) |
| C 고정 마름모 + 이름 아래 | B와 같되 이름 아래 | bpmn.io 라벨 위치 그대로 | TD 이름이 흐름선 사이에 낌, LR 마름모–선 빈틈, LR 라벨 소실 |

## Design Decisions
이 단계에서는 결정하지 않는다 — 실물 비교를 근거로 오케스트레이터·사용자가 고른다(`requirements.md`
갈래 (a)/(b)). design 단계로 넘길 질문:
- 갈래 (a) 채택 시: `Shape::Diamond` 자체를 바꾸지 않고 BPMN 전용 변형을 두는지(대조군 바이트 동일 유지 경로).
- 갈래 (a) 채택 시: 왼쪽→오른쪽 라벨 소실의 원인과 해법(점 고정 노드의 다중 출구 라벨).
- 갈래 (a) 채택 시: 이름 위치 B(오른쪽) vs C(아래) — 실물상 B가 두 방향 모두 무난, C는 TD만 bpmn.io에 가깝다.

## Risks & Mitigations
- 갈래 (a)에서 LR 라벨 소실을 못 풀면 현행보다 정보가 준다 — requirements가 "라벨 소실 없음"을 수용 기준으로
  고정해 design이 반드시 다루게 한다.
- 이름 없는 `○`/`◎` 게이트웨이와 이벤트 글자 혼동 — 마름모 테두리가 있는 B·C에서만 해소(A 기각 근거).
- `Shape::Diamond` 공유 도형 회귀 — 대조군(`control.md`)을 회귀 기준으로 requirements에 명시.

## References
- `../bpmn-notation-refinement/roadmap.md` 2번 항목 — 이 스펙의 지시 원문
- `../bpmn-event-notation-anchor/design.md` — 점 고정 술어와 배치기 소비 지점 4곳
- `../bpmn-shapes/research.md` §Decision "게이트웨이는 코드 변경 없는 라벨 관례" — 현행 방식의 결정 근거
- `../diagram-diamond-side-glyph-coverage/` — 옆면 `‹›`(선행 완료, `cbfd774`)
- `render-comparison.md` — 실물 렌더링 원문 5표본 × (현행·A·B·C)
