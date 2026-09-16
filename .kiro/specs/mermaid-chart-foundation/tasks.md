# Implementation Plan — mermaid-chart-foundation

## 정의
`pie`/`xychart-beta`/`gantt`/`quadrantChart` 등 여러 수치 차트를 다루는 사람을 위해, 축·막대·범례·수치 표기를 모든 차트에서 동일한 규칙으로 그려주는 공통 렌더링 규칙이다.

- [x] 1. 축 계산과 표시
- [x] 1.1 수치 축 눈금·라벨 계산
  - DONE: 최소~최대 범위와 표시 폭이 주어지면 일정 간격의 눈금 값과 위치가 계산되어 축 라인으로 그려지고, 범위가 지정되지 않으면 데이터 값 범위로부터 자동 계산됨
  - _Requirements: 1.1, 1.3_
  - _Difficulty: high_
  - _Boundary: 차트 축 렌더러_
- [x] 1.2 축 제목 배치
  - DONE: 축 제목 텍스트가 잘리지 않고 축 근처에 표시됨
  - _Requirements: 1.2_
  - _Difficulty: low_
  - _Boundary: 차트 축 렌더러_
  - _Depends: 1.1_

- [x] 2. 막대·범례·값 표기
- [x] 2.1 값 비례 막대 렌더링
  - DONE: 항목별 값 목록이 값 크기에 비례한 길이의 막대로 그려지고, 모든 값이 0이면 막대 없이 항목명만 표시됨
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: 차트 막대 렌더러_
  - _Depends: 1.1_
- [x] 2.2 (P) 음수 값 방향 표시
  - DONE: 음수 값이 섞인 목록에서 기준선을 기준으로 방향(좌/우 또는 상/하)이 구분되어 표시됨
  - _Requirements: 2.3_
  - _Difficulty: mid_
  - _Boundary: 차트 막대 렌더러_
  - _Depends: 2.1_
- [x] 2.3 (P) 범례와 긴 이름 처리
  - DONE: 항목명과 값이 함께 표시되고, 항목명이 표시 폭보다 길면 잘리거나 줄바꿈되어도 레이아웃이 깨지지 않음
  - _Requirements: 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: 차트 범례 렌더러_
  - _Depends: 1.1_
- [x] 2.4 (P) 수치 포맷팅
  - DONE: 정수 값은 소수점 없이, 소수 값은 일정한 자리수로 표시되고, 비율 표기가 필요한 경우 총합 대비 백분율이 표시됨
  - _Requirements: 4.1, 4.2, 4.3_
  - _Difficulty: low_
  - _Boundary: 차트 값 포맷터_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 항목 초과·빈 데이터 대체 처리
  - DONE: 항목 수가 표시 가능한 공간을 초과하면 생략 표시(예: "외 N개") 또는 전체가 일반 코드블록으로 대체되고, 항목이 하나도 없으면 빈 차트 틀 또는 코드블록으로 대체됨
  - _Requirements: 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: 차트 공통 렌더러_
  - _Depends: 2.1, 2.3, 2.4_

- [x] 4. 검증
- [x] 4.1 견고성 회귀 테스트
  - DONE: 빈 목록·0값·음수·긴 이름·범위 초과 등 대표 입력 조합에 대해 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: 차트 공통 렌더러_
  - _Depends: 3.1_
