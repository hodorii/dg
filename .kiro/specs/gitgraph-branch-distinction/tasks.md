# Implementation Plan — gitgraph-branch-distinction

## 정의
gitGraph를 읽는 사용자를 위해, 트랙(브랜치)마다 색상+선패턴을 독립적으로 순환 배정해 라벨 텍스트
없이도 브랜치를 구분 가능하게 하는 기능이다.

- [x] 1. 스타일 계산
- [x] 1.1 `branch_style()` 함수 구현
  - DONE: 트랙 순번을 받아 팔레트(4색, `% 4`)와 선패턴(`Solid`/`Dashed`/`Heavy`, `% 3`)을 각각 독립
    순환해 `(Style, LineKind)`를 반환하고, 트랙 0은 항상 `(theme.diagram_accent, LineKind::Solid)`를
    반환함
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_

- [x] 2. 가로 모드 적용
- [x] 2.1 `build()`의 트랙 선·이름 라벨·커밋/병합 점에 `branch_style` 적용
  - DONE: 트랙 2개 이상인 가로 모드 렌더링에서 트랙 선·브랜치 이름·커밋(및 병합) 점이 모두 같은
    트랙 색으로 묶여 나오고, `Theme::none()`에서는 트랙마다 다른 선패턴 글자(`─`/`╌`/`━`)로 나옴
  - _Requirements: 1.1, 2.1, 3.1_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.1_
- [x] 2.2 `build()`의 분기·병합 연결선에 `branch_style` 적용
  - DONE: 분기 연결선이 새로 생기는(child) 트랙 스타일로, 병합 연결선이 합류해 들어오는(source)
    트랙 스타일로 그려짐
  - _Requirements: 4.1, 4.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.1_

- [x] 3. 세로 모드 적용
- [x] 3.1 `build_vertical()`의 트랙 선·이름 라벨·커밋/병합 점에 `branch_style` 적용
  - DONE: 세로 모드(`gitGraph TB:`)에서도 가로 모드와 동일한 규칙으로 트랙별 색·선패턴(`│`/`╎`/`┃`)이
    적용됨
  - _Requirements: 1.3, 2.1, 3.1_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.1_
- [x] 3.2 `build_vertical()`의 분기·병합 연결선에 `branch_style` 적용
  - DONE: 세로 모드 분기·병합 가로 연결선이 각각 child/source 트랙 스타일로 그려짐
  - _Requirements: 4.1, 4.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 3.1_

- [x] 4. 검증
- [x] 4.1 유닛 테스트: `branch_style` 순환 경계값
  - DONE: `track = 0, 1, 2, 3, 4, 6, 7`에서 팔레트가 `% 4`, 패턴이 `% 3`로 순환하는지, `track == 0`이
    항상 `(diagram_accent, Solid)`인지 assert로 확인됨
  - _Requirements: 1.2, 2.2, 3.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.1_
- [x] 4.2 통합 테스트: 다중 트랙 색상·패턴 식별성
  - DONE: `Theme::dark()` 등 색 있는 테마에서 트랙별 ANSI 색 코드가 다름을, `Theme::none()`에서
    트랙별 선패턴 글자가 다름을 렌더링 결과 텍스트로 확인하는 테스트가 가로·세로 모드 각각 통과함
  - _Requirements: 1.1, 1.3, 2.1, 3.1_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.2, 3.2_
- [x] 4.3 회귀 확인: 기존 테스트 스위트 + 단일 트랙 골든 텍스트
  - DONE: `cargo test` 실행 시 기존 gitgraph 테스트(단일 트랙 `"main  ●───●───●"` 포함) 전량 통과,
    전체 스위트·clippy 클린
  - _Requirements: 5.1, 5.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 4.2_
- [x] 4.4 실물 검증: 예제 문서 렌더링
  - DONE: `examples/architecture.md`의 gitGraph 예제를 `dg -P -s none`으로 실행해 패닉 없이
    렌더링됨을 확인
  - _Requirements: 5.2_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 4.3_
