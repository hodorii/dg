# Implementation Plan — gitgraph-junction-polish

## 정의
gitGraph를 읽는 사용자를 위해, 분기·병합 연결선의 모서리를 둥글게 그리고 세로 모드에서 분기
연결선과 커밋 id 글자 사이에 최소 간격을 두는 기능이다.

- [x] 1. 모서리 둥글기 — 가로 모드
- [x] 1.1 `build()`의 분기·병합 연결선에 `join(round: true)` 추가
  - DONE: 분기는 자식 끝점, 병합은 source 끝점에 각각 `join(x, y, 0, pattern, color, true)`를
    추가하고, 분기 1개·병합 1개짜리 픽스처를 `Theme::none()`으로 렌더링해 해당 모서리가
    `╰`/`╮`/`╯`/`╭` 중 하나로 나오는지, 3갈래 분기 픽스처의 T자 지점(`┳`/`┣` 등)은 둥글게
    바뀌지 않고 기존과 같은지 확인하는 테스트가 통과함
  - _Requirements: 1.1, 1.2, 1.5, 1.6_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러(가로)_

- [x] 2. 모서리 둥글기 — 세로 모드
- [x] 2.1 `build_vertical()`의 분기·병합 연결선에 `join(round: true)` 추가
  - DONE: 분기는 자식 끝점, 병합은 source 끝점에 각각 `join` 호출을 추가하고, 같은 구조를
    세로 모드로 렌더링해 모서리가 둥글게 나오는지 확인하는 테스트가 통과함
  - _Requirements: 1.3, 1.4, 1.5, 1.6_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러(세로)_

- [x] 3. 세로 모드 커밋 id 간격
- [x] 3.1 `fork_connector_start()` 구현 및 유닛 테스트
  - DONE: `parent_id_width == 0`이면 `parent_x`를 그대로, 아니면 `parent_x + 1 + width + 1`을
    돌려주는 순수 함수가 구현되고 경계값(0, 1, 긴 폭) 테스트가 통과함
  - _Requirements: 2.1_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_
- [x] 3.2 `build_vertical()`의 분기 연결선 시작 좌표에 적용
  - DONE: 세로 모드에서 부모의 가장 최근 커밋에 긴 id를 준 픽스처를 렌더링해 그 커밋 id
    글자와 분기 연결선 사이에 공백 문자가 최소 한 칸 있는지, 빈 id(`commit`만 있는 경우)
    픽스처는 기존처럼 공백 없이 이어지는지(회귀) 확인하는 테스트가 통과함
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러(세로)_
  - _Depends: 3.1, 2.1_

- [x] 4. 검증
- [x] 4.1 회귀 확인: 기존 테스트 스위트 + clippy
  - DONE: `cargo test` 전량 통과(기존 gitgraph 테스트 포함, 모서리 문자를 직접 비교하던 테스트는
    둥근 문자 기대값으로 조정), `cargo clippy --all-targets` 클린
  - _Requirements: 3.1, 3.2_
  - _Difficulty: low_
  - _Boundary: 전체_
  - _Depends: 1.1, 2.1, 3.2_
- [x] 4.2 실물 검증: 예제 문서 렌더링
  - DONE: 릴리스 바이너리로 `examples/gitgraph-vertical.md`·`examples/architecture.md`를
    실행해 패닉 없이 끝나고, 둥근 모서리와 세로 모드 간격이 육안으로 확인됨.
    `examples/architecture.txt`를 `make examples`로 재생성
  - _Requirements: 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 4.1_
