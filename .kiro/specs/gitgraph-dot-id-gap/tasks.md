# Implementation Plan — gitgraph-dot-id-gap

## 정의
gitGraph 커밋 점과 id 글자 사이에 최소 한 칸 공백을 두어 점 글자가 다음 글자를 가리지 않게
하는 결함 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 가로·세로 모드 "점-id 최소 한 칸 공백" 테스트 작성
  - DONE: `commit id: "root-long-id"` 같은 픽스처를 가로·세로 각각 `Theme::none()`으로
    렌더링해 점 글자와 id 사이에 공백 문자가 있는지 확인하는 테스트를 추가하고, 수정 전
    코드에서 실행해 **실패**함을 확인(공백 없음이 재현됨, 1.1/1.2)
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 빈 id·간격·색상·모서리 회귀 테스트 확인
  - DONE: 빈 id 커밋(`commit`만) 픽스처가 기존과 동일한 레이아웃인지(3.1), 기존
    `gitgraph.rs` 테스트 스위트(트랙 색·선패턴 `gitgraph-branch-distinction`, 둥근 모서리·
    세로 간격 `gitgraph-junction-polish` 포함) 전량이 수정 전 코드에서 이미 통과함을 확인
    (3.2, 3.3, 3.4)
  - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_

- [x] 3. 수정 적용
- [x] 3.1 텍스트 시작 좌표와 폭 계산을 한 칸씩 민다
  - DONE: `build()`의 `total` 계산과 텍스트 draw 호출, `build_vertical()`의 `col` 계산과 텍스트
    draw 호출, `fork_connector_start()`의 오프셋을 각각 설계대로 `+1` → `+2`(또는 그에 맞는
    보정)로 수정해 1.1의 재현 테스트가 통과로 바뀜. 구현 중 발견: 좌표만 밀면 트랙 선(대시)이
    이미 그 칸을 지나가 "점-대시-글자"가 되는 경우가 있어(design.md엔 없던 디테일),
    `canvas.clear_rect`로 그 칸을 명시적으로 비워 항상 진짜 공백이 되도록 보강함
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.1, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(1.1의 재현 테스트가 이제 통과로 바뀌고, 2.1의 불변 동작
    테스트도 계속 통과), `cargo clippy --all-targets` 클린, 릴리스 바이너리로
    `examples/gitgraph-vertical.md`·`examples/architecture.md`를 실행해 패닉 없이 끝나고
    점-id 간격이 육안으로 확인됨, `examples/architecture.txt`를 `make examples`로 재생성
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3, 3.4_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
