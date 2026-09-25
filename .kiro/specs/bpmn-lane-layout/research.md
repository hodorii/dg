# Research & Design Decisions — bpmn-lane-layout

## Summary
- **Feature**: `bpmn-lane-layout`
- **Discovery Scope**: Extension(기존 `layout::graph` 그룹 메커니즘 확장) — 배경·대안
  비교는 `../bpmn-support/research.md` §"레인은 graph.rs의 그룹 확장으로"가 SSoT,
  여기는 그 7개 지점을 현재 코드로 재확인한 결과와 추가로 드러난 지점만 적는다.
- **Key Findings**:
  - 7개 지점은 모두 현재 코드에 그대로 있다(함수 이름 불변, `push_outputs_below_groups`
    포함). 다만 층 범위는 `group_layer_ranges()`(가상 노드 소속용)와
    `build_blocks()`(블록 범위용) 두 곳에서 따로 계산되므로 "유효 범위"는 둘 다에
    적용해야 한다.
  - 형제 레인 간격 **-1**은 맞다: `place_block()`은 `end`(배타적 끝) + `gap_between()`
    (실제 자식끼리 TB 3·LR 1)을 다음 시작으로 쓰고, `Canvas::add_line()`이 같은 칸의
    선 비트를 `|=`로 합치므로 다음 시작을 `end - 1`로 두면 두 사각형이 경계 한 칸을
    공유해 `┬`/`┴` 분기로 그려진다. 0이면 `││` 두 줄이 붙는다.
  - 연구 문서의 7개 밖에서 세 지점이 더 필요하다: `is_movable()`(층 접기가 레인
    블록을 통째로 아래로 내림), `route()`의 `top_levels`/`bottom_levels` 집계(
    `group_top()`/`inner_rank()`와 같은 규칙으로 세지 않으면 테두리와 내용이 겹침),
    교차축 여백(`block_pad` 2가 부모 레인 테두리와 자식 레인 테두리 사이에 빈 칸을
    남겨 BPMN의 밀착 레인이 안 됨).

## Research Log

### 7개 변경 지점 재확인 (`src/diagram/layout/graph.rs`, 2236줄 기준)
- **Context**: 연구 문서의 줄 번호가 밀렸을 수 있어 함수 이름으로 재탐색.
- **Sources Consulted**: `graph.rs` 전문, `ir.rs`, `canvas.rs::add_line/rect`.
- **Findings**:
  - `group_layer_ranges()`(689) — `make_segments()`가 `dummy_group()`에만 넘김.
    `build_blocks()`(713)는 `lnodes` 층으로 `Block.layer_min/max`를 따로 누적.
  - `place_block()`(764) — 층 범위가 겹치는 앞 자식들의 `end + gap_between`의 최댓값이
    시작. 블록 폭 = `max_end + 2*pad`(`title_min`, `block_extra` 가산).
  - `reorder()`(845)·`improve_by_swaps()`(884) — 블록마다 자식 전체를 재정렬/이동.
  - `push_outputs_below_groups()`(353) — 도착 노드의 원천이 모두 한 그룹이면 그 그룹의
    마지막 층 뒤로 민다. 레인 A→B 간선에서 `outer` = A(레인)이라 B의 노드가 A 전체
    뒤로 밀린다(연구 문서가 지적한 결함 재현 확인).
  - `group_top()`(1687) 랭크·`inner_rank()`(1711)·`route()`(1236~1244)의 층별
    레벨 집계 — 세 곳이 같은 "같은 층에서 시작/끝나는 조상 사슬 길이"를 각자 센다.
  - `layer_start[0]`(1320)은 0 고정. `total_along()`·`node_along()`·`segment_span()`이
    전부 `layer_start`에서 파생되므로 여기에 오프셋을 더하면 캔버스 크기까지 따라온다.
  - `render()`(16~36)의 방향·라벨 폭 재시도 루프는 `graph.direction`만 본다 — 변경 불필요.
- **Implications**: design.md §Key Decisions의 기하 규칙 표.

### 형제 레인 순서 고정의 범위
- **Context**: `reorder()`가 블록 안 자식 전체를 정렬하므로 "레인만 고정"하려면
  레인·비레인 혼재 시 규칙이 필요.
- **Findings**: BPMN에서는 한 그룹의 직계 자식이 레인이면 전부 레인이다(laneSet은
  분할). 최상위(블록 0)에 풀과 풀 밖 노드가 섞이는 경우만 실제로 있다.
- **Implications**: "레인 자식이 하나라도 있는 블록은 통째로 선언 순서 유지"로 단순화.
  혼재 블록의 비레인 자식도 선언 순서로 놓인다(요구사항 3.3은 패닉 없음·상자
  밖 배치만 요구).

## Architecture Pattern Evaluation
`../bpmn-support/research.md` §Architecture Pattern Evaluation 참고(그룹 확장 채택).

## Design Decisions

### Decision: 배너는 흐름축 앞쪽 접두 공간, 구분선은 기존 그룹 테두리 줄을 재사용
- **Context**: 레인 테두리가 부모와 같은 칸이라 제목을 테두리 선 위에 쓸 수 없다.
- **Alternatives Considered**:
  1. 제목을 기존처럼 테두리 선 위에 쓰기 — 형제·부모와 경계를 공유하므로 겹침
  2. LR에서 세로쓰기 제목 — 문자 폭·CJK 처리 복잡, 가독성 낮음
  3. 흐름축 앞에 깊이별 접두 공간(TB 상단 띠 / LR 왼쪽 열)을 두고 가로쓰기
- **Selected Approach**: 3. `layer_start[0]`에 깊이별 배너 크기 합을 더하고, 레인
  사각형은 자기 깊이의 배너 시작에서 시작한다. 배너와 내용 사이 구분선은 기존
  그룹 "내용 상자" 테두리 줄(`layer_start[0] + 2*rank`)을 그대로 쓴다 — 레인 안
  레인은 랭크를 올리지 않으므로 사슬 전체가 같은 줄에 구분선을 그린다.
- **Rationale**: 새 좌표 개념 없이 오프셋 하나와 그리기 두 줄로 끝난다.
- **Trade-offs**: 같은 깊이의 배너 크기는 전역 최댓값 하나(LR에서 제목 길이 차이만큼
  짧은 제목 옆이 빈다). 레인 없는 풀과 레인 있는 풀이 섞이면 레인 없는 풀의 배너
  아래가 한 단 빈다 — v1 허용.
- **Follow-up**: 실제 `biz-process.md` 규모(제목 6~8자 × 깊이 2)로 LR 폭 예산 실측.

### Decision: 교차축은 부모와 밀착(pad 0), 형제는 -1
- **Context**: `block_pad` 2를 그대로 쓰면 부모 테두리와 자식 레인 테두리 사이에
  빈 칸 한 줄이 남아 BPMN 풀/레인 그림과 다르다.
- **Selected Approach**: 부모가 레인일 때 레인 자식만 pad 0 + 형제 간격 -1, 노드·
  일반 그룹 자식은 기존 pad 2 유지. 부모 레인의 `block_extra`(TB 라벨 여유)는
  마지막 레인 자식에게 넘겨 밀착을 유지한다.
- **Trade-offs**: `place_block()`의 pad가 블록 단위에서 자식 단위로 바뀐다(한 줄).

## Risks & Mitigations
- 레인 있는 그래프에서 기존 `straighten()`/`push_members()`가 `block_pad`를 노드
  컨테이너 기준으로만 쓰므로 영향 없음 — 단, 회귀 테스트로 레인 없는 출력 바이트
  동일을 못박는다(요구사항 6.1).
- 배너 오프셋이 `segment_span()`의 `layer_start[layer].saturating_sub(1)`(가상 노드
  선 시작)과 만나는 경우는 층 0에 가상 노드가 없으므로 발생하지 않는다.

## References
- `../bpmn-support/research.md` — 배경·대안·7개 지점 원본
- OMG BPMN 2.0 §9.2(Pool "entire length of the Diagram"), §10.7(Lane)
