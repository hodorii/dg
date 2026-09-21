# Implementation Plan — sequence-self-message-clearance

## 정의
시퀀스 다이어그램에서 자기 자신에게 보내는 재귀 메시지(self-message)를 읽는 사용자를 위해, 화살촉과 루프의 세로 연결선(모서리) 사이에 시각적으로 구분되는 여백을 두어 렌더링하는 기능이다.

- [x] 1. 재귀 루프 여백 확보
- [x] 1.1 `draw_self_message` 루프 폭 확장 및 화살촉 위치 조정
  - DONE: self-message 루프가 2칸→3칸으로 넓어지고(x+1..x+3). 화살촉은 도착 지점인 생명선(x)에 바로 붙고(x+1, 다른 메시지가 상대 생명선에 바로 닿는 관례와 동일), 화살촉과 루프 모서리(x+3) 사이(x+2)에 실선 한 칸을 그려 모서리의 굽은 모양이 화살촉에 눌리지 않게 함(`draw_self_message`)
  - _Requirements: 1.1, 1.2_
  - _Difficulty: mid_
  - _Boundary: 시퀀스 렌더러_
- [x] 1.2 참가자 간 폭 계산에 재귀 루프 폭 반영
  - DONE: `position_participants`의 self-message 폭 상수를 `label_width + 6` → `label_width + 7`로 올려 넓어진 루프·라벨 시작 위치(x+5)를 반영, 옆 생명선과 겹치지 않음
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: 시퀀스 렌더러_
  - _Depends: 1.1_

- [x] 2. 경계 처리
- [x] 2.1 폭 초과 시 기존 재시도 계약 유지
  - DONE: 폭 계산이 기존 `constraints`/`gaps` 메커니즘을 그대로 타므로 레이블 폭 축소·재시도·코드블록 대체 계약이 그대로 유지됨(`too_narrow_returns_none` 테스트로 확인)
  - _Requirements: 2.3_
  - _Difficulty: low_
  - _Boundary: 시퀀스 렌더러_
  - _Depends: 1.2_

- [x] 3. 검증
- [x] 3.1 기존 테스트 갱신 및 견고성 회귀
  - DONE: 전용 회귀 테스트 `self_message_arrowhead_touches_lifeline_but_not_the_corner` 추가, 기존 `renders_messages_and_fragment` 등 7개 테스트 전부 통과, 전체 스위트 196개 + doctest 2개 통과, clippy(`-D warnings`) 클린
  - _Requirements: 3.1, 3.2_
  - _Difficulty: low_
  - _Boundary: 시퀀스 렌더러_
  - _Depends: 2.1_
- [x] 3.2 검증용 샘플 문서 작성
  - DONE: `examples/sequence-self-message.md` 추가, `dg -P -s none`으로 렌더링해 `│◀─╯` 형태(화살촉이 생명선에 붙고 모서리와는 여백)가 실제로 나오는지 확인함
  - _Requirements: 1.1, 1.2_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 1.1_
