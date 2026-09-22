# Implementation Plan — diagram-crow-foot-orientation

## 정의
ER "many" 까치발 표식의 방향(벌어진 끝이 개체 쪽을 향하도록)을 바로잡는 결함 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 세로·가로 방향 "many" 표식 방향 테스트 작성
  - DONE: top-down(개체가 아래/위 각각인 경우)·left-right 세 가지 배치로 "many" 쪽
    표식 글자를 확인하는 테스트를 추가하고, 수정 전 코드로 실행해 **실패**함을 확인
    (뾰족한 끝이 개체 쪽, 1.1~1.3)
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 one 표식·화살표류 방향 회귀 테스트 확인
  - DONE: `crow_one_marker_is_a_single_glyph_not_doubled`와 화살표(`Marker::Arrow` 등)
    방향을 다루는 기존 테스트가 수정 전 코드에서 이미 전량 통과함을 확인(3.1~3.3)
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_

- [x] 3. 수정 적용
- [x] 3.1 many 글자 인덱스 매핑을 뒤집는다
  - DONE: `marker_glyphs()`의 `many` 배열을 `['∧','∨','<','>']`에서
    `['∨','∧','>','<']`로 바꿔 1.1의 재현 테스트가 통과로 바뀜, `OpenArrow` 자신의
    배열은 그대로 둠
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: low_
  - _Boundary: graph 레이아웃 렌더러_
  - _Depends: 1.1, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(1.1의 재현 테스트가 통과로 바뀌고 2.1의 불변 동작
    테스트도 계속 통과), `cargo clippy --all-targets -- -D warnings` 클린, 실제 참조
    문서(`/home/hs/w/.kiro/reference/schemas_diagram.md`)의 ERD를 릴리스 바이너리로
    렌더링해 까치발이 개체 쪽으로 벌어져 보임을 육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
