# Implementation Plan — gitgraph-vertical-mode

## 정의
좁은 터미널에서 gitGraph를 읽으려는 사용자를 위해, 기존 가로 모드(브랜치=행, 시간=열)에 더해
세로 모드(브랜치=열, 시간=행)를 추가하는 기능이다.

- [x] 1. 방향 파싱
- [x] 1.1 `gitGraph TB:`/`BT:`/`LR:`/`RL:` 헤더 파싱
  - DONE: `GitGraph`에 `direction: Option<(Direction, bool)>` 필드 추가, `parse()`가 헤더 첫 줄에서 기존 `options::parse_direction`으로 파싱해 채움
  - _Requirements: 1.1, 1.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 파서_
- [x] 1.2 `DiagramOptions.direction`(CLI/지시자)이 헤더보다 우선하도록 dispatch 연결
  - DONE: `mermaid::mod.rs`의 gitgraph 매치 갈래에서 `options.direction`이 있으면 파싱된 그래프의 방향을 덮어씀
  - _Requirements: 1.3_
  - _Difficulty: low_
  - _Boundary: mermaid dispatch_
  - _Depends: 1.1_

- [x] 2. 세로 배치 렌더링
- [x] 2.1 `build_vertical` 구현(가로 `build`의 전치)
  - DONE: 트랙=열, 커밋 순서=행으로 좌표를 매겨 `vline`(트랙)·`hline`(분기·병합 연결선)·머리글 이름·커밋 점을 그림
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: high_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.1_
- [x] 2.2 방향별 재시도 루프 연결
  - DONE: `render()`가 `[preferred, preferred.other()]` × 폭 캡 조합으로 시도, 방향에 따라 `build`/`build_vertical` 중 하나를 호출
  - _Requirements: 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.1_

- [x] 3. 검증
- [x] 3.1 기존 테스트 회귀 확인 및 신규 테스트 추가
  - DONE: 기존 17개 테스트(가로 모드) 그대로 통과(1개는 새 폭 재시도 계약에 맞춰 폭 값 조정), 세로 모드 전용 테스트 5개 추가(헤더 파싱·branch·merge·재시도·옵션 우선순위), robustness 퓨저에 `gitGraph TB:`/`BT:` 케이스 3개 추가, 전체 196개 테스트 + doctest 통과, clippy 클린
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 4.1, 4.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러, gitgraph 파서_
- [x] 3.2 검증용 샘플 문서 작성 및 README 갱신
  - DONE: `examples/gitgraph-vertical.md` 추가, `dg -P`로 세로/가로 모드를 나란히 렌더링해 확인. README `--direction` 옵션 설명에 gitGraph도 포함됨을 명시
  - _Requirements: 1.1, 1.2, 1.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 2.2_
