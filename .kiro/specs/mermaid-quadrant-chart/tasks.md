# Implementation Plan — mermaid-quadrant-chart

## 정의
두 축 기준으로 항목을 4개 영역에 배치해 비교하려는 사용자를 위해, 코드펜스 안 `quadrantChart` 소스를 4분면 좌표 배치 형태의 터미널 텍스트 차트로 그려주는 기능이다.

- [x] 1. 종류 판별과 파싱
- [x] 1.1 quadrantChart 지시어 인식 및 dispatch 연결
  - DONE: 첫 지시어가 quadrantChart면 판별 결과가 "quadrant"로 나옴
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 title/quadrant 라벨/축 라벨/좌표 데이터 파싱
  - DONE: title, quadrant-1~4 라벨(일부 생략 허용), x-axis/y-axis 양끝 라벨, `"이름": [x, y]` 좌표 목록이 각각 추출됨
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 4.1_
  - _Difficulty: mid_
  - _Boundary: quadrant 파서_
  - _Depends: 1.1_

- [x] 2. 사분면 렌더링
- [x] 2.1 사분면 틀 렌더링
  - DONE: 4분면 영역과 각 영역 라벨(생략된 영역은 빈 상태), 축 양끝 라벨이 표시됨
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: mid_
  - _Boundary: quadrant 렌더러_
  - _Depends: 1.2_
- [x] 2.2 좌표 배치와 겹침 처리
  - DONE: 0~1 범위 좌표에 맞춰 항목 이름(또는 점)이 배치되고, 좌표가 같거나 가까운 두 항목은 겹치지 않고 구분 가능하게 표시됨
  - _Requirements: 3.1, 3.2_
  - _Difficulty: high_
  - _Boundary: quadrant 렌더러_
  - _Depends: 2.1_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 좌표 범위·항목 초과 대체 처리
  - DONE: 0~1 범위를 벗어난 좌표는 보정되거나 일반 코드블록으로 대체되고, 항목 수가 표시 공간을 초과하면 생략 표시 또는 코드블록으로 대체됨
  - _Requirements: 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: quadrant 렌더러_
  - _Depends: 2.2_

- [x] 4. 검증
- [x] 4.1 견고성 회귀 테스트
  - DONE: 범위 밖 좌표·다항목·빈 라벨 등 대표 입력에서 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: quadrant 파서, quadrant 렌더러_
  - _Depends: 3.1_
- [x]* 4.2 시각적 회귀 스냅샷
  - DONE: 4분면 라벨·다항목 대표 예제의 렌더링 결과가 스냅샷과 일치함
  - _Requirements: 2.1, 3.1_
