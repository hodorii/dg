# Implementation Plan — sequence-fragment-frame-distinction

## 정의
시퀀스 다이어그램을 읽는 사용자를 위해, 프래그먼트(`alt`/`opt`/`loop` 등) 바깥 테두리를
메시지·생명선과 다른 선패턴(굵은선)으로 그리는 기능이다.

- [x] 1. 프래그먼트 테두리 선패턴 변경
- [x] 1.1 `LineKind::Solid` → `Heavy` 변경 및 유닛 테스트
  - DONE: 프래그먼트 바깥 테두리를 그리는 `rect()` 호출의 `LineKind`를 `Heavy`로 바꾸고, `alt`
    (단독)와 `loop` 안에 `alt`가 중첩된 픽스처를 `Theme::none()`으로 렌더링해 테두리 글자가
    `┏`/`┓`/`┗`/`┛`/`━`/`┃` 중 하나이고 메시지·생명선 글자(`─`/`│`)와 겹치지 않으며, 중첩
    깊이와 무관하게 두 프레임 모두 같은 굵은선인지, `else` 구분선은 여전히 파선(`╌`)인지
    확인하는 테스트가 통과함
  - _Requirements: 1.1, 1.2, 1.3, 2.2_
  - _Difficulty: low_
  - _Boundary: sequence 렌더러_

- [x] 2. 검증
- [x] 2.1 통합 테스트: 메시지가 프래그먼트 경계를 지나는 교차 지점
  - DONE: 프래그먼트 안팎을 오가는 메시지가 있는 픽스처를 렌더링해 패닉 없이 끝나고, 테두리와
    메시지 화살표가 만나는 지점이 유효한 교차/모서리 문자로 그려짐을 확인하는 테스트가 통과함
  - _Requirements: 2.1_
  - _Difficulty: low_
  - _Boundary: sequence 렌더러_
  - _Depends: 1.1_
- [x] 2.2 회귀 확인: 기존 테스트 스위트 + clippy
  - DONE: `cargo test` 전량 통과(기존 시퀀스 테스트 포함), `cargo clippy --all-targets` 클린
  - _Requirements: 3.1, 3.2_
  - _Difficulty: low_
  - _Boundary: 전체_
  - _Depends: 2.1_
- [x] 2.3 실물 검증: `examples/architecture.txt` 재생성
  - DONE: `make examples`(Makefile이 지정하는 `dg -P -s none -w 140`, 시퀀스 참여자 7개가 실제로
    다이어그램으로 그려지려면 폭 140 이상 필요)로 `examples/architecture.txt`를 갱신. diff는
    28줄만 바뀜 — (1) 이 스펙의 alt 프레임이 굵은선(┏━┓┃┗┛)으로 바뀐 부분, (2) 직전에 머지된
    `gitgraph-branch-distinction`이 그 스펙에서는 이 골든 파일을 갱신하지 않아 밀려 있던
    브랜치별 색·선패턴 변경분. 둘 다 현재 `dg` 출력과 일치시키는 정당한 변경이라 함께 커밋
  - _Requirements: 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 2.2_
