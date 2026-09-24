# Design — diagram-lr-tail-label-position

## 정의
가로(LR) 방향 그래프형 다이어그램에서 관계의 from측 텍스트 라벨(카디널리티·다중성·관계
이름)이 자신의 개체가 아니라 같은 레이어의 가장 넓은 개체 기준 위치에 붙는 결함을 고친다.

## 원인 (Root Cause)
`src/diagram/layout/graph.rs`의 `draw_segment_decorations()`는 `segment_span()`이
돌려주는 `(top, bottom, gap_start)` 중 `top`을 "이 세그먼트가 실제로 시작하는 from측
개체 상자의 오른쪽 바로 다음 칸"으로, `gap_start`는 "그 레이어에서 가장 넓은 개체를
기준으로 잡은 공용 배선 통로 시작 칸"으로 서로 다른 목적으로 계산한다
(`segment_span()`: `top`은 `node_along(from) + along_size`로 세그먼트별, `gap_start`는
`layer_start[layer] + layer_total(layer)`로 레이어 전체 공용).

까치발 기호(`marker_glyphs`, 1786~1810행 부근)는 이 `top`을 그대로 써서 세그먼트별
위치를 정확히 맞춘다. 그런데 바로 아래 `Direction::LeftRight` 분기(1829~1839행 부근)의
관계 라벨(`label`)과 from측 라벨(`tail_label`)만 `top` 대신 `gap_start + 1`을 x좌표로
쓴다. 레이어에서 자신이 가장 넓은 개체면 `top == gap_start`라 우연히 맞아 보이지만
(재현 절차의 `LONG_ENTITY_NAME_HERE` 쪽), 더 좁은 개체(`A` 쪽)는 `top < gap_start`가
되어 라벨이 자기 상자에서 멀리, 공용 통로 시작점 쪽으로 밀려난다(1.1~1.3). to측
`head_label`은 같은 함수에서 세그먼트별 값인 `bottom`을 그대로 쓰기 때문에 이 문제가
없다(3.2).

## 수정 방식
`Direction::LeftRight` 분기의 `label`·`tail_label` x좌표를 `gap_start + 1` 대신
`top + 1`로 바꾼다 — `head_label`이 이미 세그먼트별 `bottom`을 쓰는 것과 대칭을 맞추는
것뿐이라 관련 함수 서명이나 다른 분기(TopDown)는 건드리지 않는다.
- **대안 비교**: `gap_start` 자체(레이어별 공용 배선 통로 시작점) 계산을 세그먼트별로
  바꾸는 방법도 검토했으나, `gap_start`는 `channel_position()`이 배선 통로(채널) 열을
  잡는 데도 같이 쓰이는 값이라 세그먼트별로 바꾸면 같은 레이어의 여러 간선이 서로 다른
  통로 시작점을 갖게 돼 교차 회피(채널 배정) 로직과 충돌할 위험이 있어 기각 —
  라벨 앵커만 `top`으로 바꾸는 쪽이 배선 로직에 영향이 없다.

## 검증 속성
- (a) 결함 재현: `src/diagram/layout/graph.rs` 테스트 모듈에 같은 레이어에 폭이 다른
  두 개체 쌍을 만드는 그래프(재현 절차의 `A`/`LONG_ENTITY_NAME_HERE` 구성과 동등)로
  `tail_label`·`label`의 렌더된 칸 위치를 확인하는 테스트를 추가하면, 수정 전 코드에서는
  **실패**함(1.1~1.3 — 좁은 개체 쪽 라벨이 상자에서 떨어져 있음)을 확인.
- (b) 기대 동작: 같은 테스트가 수정 후 통과(2.1~2.3 — 두 개체 모두 자기 상자 바로 옆에
  라벨이 붙음).
- (c) 불변 동작: 기존 `renders_left_right`·`right_left_flips_left_right_without_mirroring_text`
  테스트, 그리고 TopDown 방향 라벨 위치를 확인하는 기존 테스트가 수정 후에도 그대로
  통과(3.1~3.3).

## 영향 범위
- `src/diagram/layout/graph.rs`: `draw_segment_decorations()`의 `Direction::LeftRight`
  분기 두 줄(`label`, `tail_label`의 x좌표), 테스트 추가.
- 다른 파일 없음 — `segment_span()`·`channel_position()`·마커 글리프 계산·TopDown 분기는
  변경하지 않으므로 mermaid/plantuml 파서 쪽 바운더리 커밋에 영향 없음.
