# Implementation Plan — mermaid-gitgraph

## 정의
커밋/브랜치 흐름을 mermaid `gitGraph` 문법으로 표현하려는 사용자를 위해, 코드펜스 안 `gitGraph` 소스를 브랜치 트랙과 커밋 점으로 이루어진 터미널 텍스트 다이어그램으로 그려주는 기능이다.

- [x] 1. 종류 판별과 기본 파싱
- [x] 1.1 gitGraph 지시어 인식 및 dispatch 연결
  - DONE: 첫 지시어가 gitGraph(콜론 포함 여부 무관)면 판별 결과가 "gitgraph"로 나옴
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 commit/branch/checkout/merge 문 파싱
  - DONE: commit, branch <이름>, checkout(또는 switch) <이름>, merge <이름> 문이 각각 순서를 유지한 이벤트 목록으로 추출됨
  - _Requirements: 2.1, 2.2, 2.3, 3.1_
  - _Difficulty: mid_
  - _Boundary: gitgraph 파서_
  - _Depends: 1.1_

- [x] 2. 트랙·커밋 렌더링
- [x] 2.1 트랙 배치와 커밋 점 렌더링
  - DONE: 각 브랜치가 고유한 가로 트랙으로 배치되고, commit마다 해당 트랙 위에 순서대로 점이 좌에서 우로 표시됨
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: high_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 1.2_
- [x] 2.2 (P) 병합선 렌더링
  - DONE: merge 문이 있으면 두 트랙 사이에 병합선이 그려지고, 대상 브랜치가 정의되지 않았으면 병합선 없이 나머지가 정상 표시됨
  - _Requirements: 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.1_
- [x] 2.3 (P) 브랜치·커밋 라벨 표시
  - DONE: 브랜치 이름이 트랙 옆에, `commit id: "..."`로 지정한 id가 커밋 점 옆에 표시됨
  - _Requirements: 4.1, 4.2_
  - _Difficulty: low_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.1_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 표시 실패·잘못된 참조 대체 처리
  - DONE: 트랙 수 증가로 표시 폭을 초과하면 일반 코드블록으로 대체 표시되고, 정의되지 않은 브랜치로 checkout하면 오류 없이 새 트랙으로 처리되거나 코드블록으로 대체됨
  - _Requirements: 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: gitgraph 렌더러_
  - _Depends: 2.1, 2.2_

- [x] 4. 검증
- [x] 4.1 견고성 회귀 테스트
  - DONE: 다양한 폭과 비정상 gitGraph 입력(빈 브랜치, 잘못된 참조 등)에서 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: gitgraph 파서, gitgraph 렌더러_
  - _Depends: 3.1_
- [x]* 4.2 시각적 회귀 스냅샷
  - DONE: branch/checkout/merge 조합 대표 예제의 렌더링 결과가 스냅샷과 일치함
  - _Requirements: 2.2, 3.1, 4.1_
