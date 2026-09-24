# Implementation Plan — diagram-lr-label-marker-overlap

## 정의
LR 방향에서 관계 라벨이 두 글자짜리 from측 표식을 덮어써 지우는 결함과, tail_label·
head_label이 표식과 달리 노드 경계에 바로 붙지 않던 불일치를 함께 고치는 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 2글자 from측 표식 + 라벨 조합으로 글자 손실 재현
  - DONE: `CrowZeroMany` tail 표식과 관계 라벨을 가진 그래프를 LR로 렌더링해 표식 두
    글자가 모두 보이는지 확인하는 테스트를 추가하고, 수정 전 코드로 실행해 `○` 글자가
    사라짐을 **실패**로 확인(1.1)
  - _Requirements: 1.1, 2.1_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 1.2 tail_label이 노드 경계에 바로 붙는지 재현
  - DONE: `CrowOne`(한 글자) tail 표식 + `tail_label`로 렌더링해 경계와 라벨 사이에
    빈 칸이 없는지(칸 차이 == 1) 확인하는 테스트를 추가하고, 수정 전 코드로 실행해
    빈 칸이 하나 남아(칸 차이 == 2) **실패**로 확인(1.2)
  - _Requirements: 1.2, 2.2_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 1.3 head_label이 노드 경계에 바로 붙는지 재현
  - DONE: `CrowOne`(한 글자) head 표식 + `head_label`로 렌더링해 to측 경계와 라벨
    사이에 빈 칸이 없는지(칸 차이 == 1) 확인하는 테스트를 추가하고, 수정 전 코드로
    실행해 빈 칸이 하나 남아(칸 차이 == 2) **실패**로 확인(1.3)
  - _Requirements: 1.3, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 기존 LR/TB 라벨 위치 테스트 확인
  - DONE: `left_right_tail_label_hugs_its_own_node_not_the_widest_sibling` 등 기존
    LR/TB 라벨 테스트가 수정 전 코드에서 이미 전량 통과함을 확인(3.1, 3.3)
  - _Requirements: 3.1, 3.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 3. 수정 적용
- [x] 3.1 `label`·`tail_label`·`head_label`의 x좌표를 각자의 처지에 맞게 분리한다
  - DONE: `draw_segment_decorations()`의 `Direction::LeftRight` 분기에서 `label`은
    `top + marker_glyphs(top_marker, ..).len().max(1)`(표식 폭만큼 건너뜀),
    `tail_label`은 `top` 그대로, `head_label`은 `(bottom + 1) - width`(마지막 글자가
    `bottom`에서 끝남)로 분리해 1.1·1.2·1.3의 재현 테스트가 모두 통과로 바뀜,
    TopDown 분기·표식 자체 계산은 그대로 둠
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_
  - _Depends: 1.1, 1.2, 1.3, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 278개 전량 통과(1.1·1.2·1.3의 재현 테스트가 통과로 바뀌고
    2.1의 불변 동작 테스트도 계속 통과), `cargo clippy --all-targets -- -D warnings`
    클린, 두 글자 까치발 표식(`CrowZeroMany`) + 라벨, ER 카디널리티 양쪽,
    `examples/architecture.md`의 classDiagram(`1`·`*` 양쪽)을 release 바이너리로
    실제 렌더링해 표식 글자 손실이 없고 from측·to측 라벨이 대칭으로 경계에 붙음을
    육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
