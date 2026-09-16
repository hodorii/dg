# Implementation Plan — mermaid-xychart

## 정의
수치 추이를 mermaid `xychart-beta` 문법으로 표현하려는 사용자를 위해, 코드펜스 안 `xychart-beta` 소스를 X/Y 축 기반 막대·선 그래프로 이루어진 터미널 텍스트 차트로 그려주는 기능이다.

- [x] 1. 종류 판별과 파싱
- [x] 1.1 xychart-beta 지시어 인식 및 dispatch 연결
  - DONE: 첫 지시어가 xychart-beta면 판별 결과가 "xychart"로 나옴
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 title/x-axis/y-axis 파싱
  - DONE: title 텍스트, x-axis 카테고리 목록, y-axis 제목과 최소~최대 범위(또는 미지정 상태)가 각각 추출됨
  - _Requirements: 2.1, 2.2, 2.3, 4.1_
  - _Difficulty: mid_
  - _Boundary: xychart 파서_
  - _Depends: 1.1_

- [x] 2. 축·계열 렌더링
- [x] 2.1 축 렌더링 연동
  - DONE: 카테고리 라벨이 가로축에 순서대로, y-axis 범위(자동 계산 포함)가 세로축 눈금으로 공통 축 렌더러를 통해 표시됨
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: mid_
  - _Boundary: xychart 렌더러_
  - _Depends: 1.2_
- [x] 2.2 bar 계열 파싱과 렌더링
  - DONE: bar [...] 데이터가 각 카테고리 위치에 값 크기에 비례한 막대로 표시됨
  - _Requirements: 3.1_
  - _Difficulty: mid_
  - _Boundary: xychart 렌더러_
  - _Depends: 2.1_
- [x] 2.3 (P) line 계열 파싱과 렌더링
  - DONE: line [...] 데이터가 각 카테고리 지점을 잇는 선(또는 점 궤적)으로 표시됨
  - _Requirements: 3.2_
  - _Difficulty: high_
  - _Boundary: xychart 렌더러_
  - _Depends: 2.1_
- [x] 2.4 bar/line 동시 표시와 계열 길이 불일치 처리
  - DONE: bar와 line이 함께 정의되면 구분 가능한 형태로 같은 축 위에 표시되고, 계열 데이터 개수가 카테고리 수와 다르면 부족한 부분이 생략되어 오류 없이 표시됨
  - _Requirements: 3.3, 3.4_
  - _Difficulty: mid_
  - _Boundary: xychart 렌더러_
  - _Depends: 2.2, 2.3_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 폭 초과·음수 값 처리
  - DONE: 카테고리 수 증가로 가로 폭을 초과하면 일반 코드블록으로 대체 표시되고, 음수 값이 섞이면 기준선 아래로 표시되어 구분됨
  - _Requirements: 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: xychart 렌더러_
  - _Depends: 2.4_

- [x] 4. 검증
- [x] 4.1 견고성 회귀 테스트
  - DONE: 다양한 폭·계열 조합·비정상 입력에서 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: xychart 파서, xychart 렌더러_
  - _Depends: 3.1_
- [x]* 4.2 시각적 회귀 스냅샷
  - DONE: bar만/line만/bar+line 대표 예제의 렌더링 결과가 스냅샷과 일치함
  - _Requirements: 3.1, 3.2, 3.3_
