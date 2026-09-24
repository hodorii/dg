# Implementation Plan — diagram-lr-tail-label-position

## 정의
가로(LR) 방향 그래프형 다이어그램에서 관계의 from측 텍스트 라벨(카디널리티·다중성·관계
이름)이 자신의 개체가 아니라 같은 레이어의 가장 넓은 개체 기준 위치에 붙는 결함을
바로잡는 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 그래프 배치기 단위 테스트로 LR 방향 from측 라벨 위치 재현
  - DONE: 같은 레이어에 폭이 다른 두 노드를 만들어 관계 라벨과 from측 카디널리티/다중성
    라벨(`tail_label`)의 렌더된 칸 위치를 확인하는 테스트를 추가하고, 수정 전 코드로
    실행해 좁은 노드 쪽 라벨이 그 노드 상자 바로 옆이 아니라 떨어진 칸에 렌더됨을
    **실패**로 확인(1.1, 1.2)
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 1.2 classDiagram LR 다중성으로 같은 결함 재현
  - DONE: mermaid classDiagram 소스(폭이 다른 두 클래스 쌍, `"1" --> "many"` 다중성
    포함)를 `--direction lr`로 실제 파싱·렌더링하는 테스트를 추가하고, 수정 전 코드로
    실행해 좁은 클래스 쪽 다중성 라벨이 상자에서 떨어져 렌더됨을 **실패**로 확인(1.3)
  - _Requirements: 1.3, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 기존 LR/TB 라벨·마커 테스트 확인
  - DONE: `renders_left_right`, `right_left_flips_left_right_without_mirroring_text`,
    TopDown 방향 라벨 위치를 다루는 기존 테스트, to측(`head_label`) 위치를 다루는
    부분이 수정 전 코드에서 이미 전량 통과함을 확인(3.1, 3.2, 3.3)
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 3. 수정 적용
- [x] 3.1 LeftRight 분기의 from측 라벨 x좌표를 세그먼트 자신의 위치 기준으로 바꾼다
  - DONE: `draw_segment_decorations()`의 `Direction::LeftRight` 분기에서 `label`·
    `tail_label`의 x좌표를 레이어 공용 값(`gap_start + 1`) 대신 세그먼트 자신의
    from측 위치(`top + 1`)로 바꿔 1.1·1.2의 재현 테스트가 통과로 바뀜, `head_label`·
    TopDown 분기·`segment_span()`/`channel_position()`은 그대로 둠
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_
  - _Depends: 1.1, 1.2, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(1.1·1.2의 재현 테스트가 통과로 바뀌고 2.1의 불변
    동작 테스트도 계속 통과), `cargo clippy --all-targets -- -D warnings` 클린,
    폭이 다른 개체/클래스가 섞인 ER·classDiagram 예시를 release 바이너리로
    `--direction lr` 렌더링해 모든 from측 라벨이 자기 상자 옆에 붙어 보임을 육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
