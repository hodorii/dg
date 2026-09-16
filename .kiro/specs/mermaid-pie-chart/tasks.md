# Implementation Plan — mermaid-pie-chart

## 정의
비율 데이터를 mermaid `pie` 문법으로 표현하려는 사용자를 위해, 코드펜스 안 `pie` 소스를 항목별 막대와 백분율로 이루어진 터미널 텍스트 차트로 그려주는 기능이다.

- [x] 1. 종류 판별과 파싱
- [x] 1.1 pie 지시어 인식 및 dispatch 연결
  - DONE: 첫 지시어가 pie(또는 pie showData)면 판별 결과가 "pie"로 나옴
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 "항목" : 값 목록과 title/showData 옵션 파싱
  - DONE: `"이름" : 값` 줄들에서 이름·값 쌍 목록이 추출되고, title과 showData 옵션 유무가 함께 인식되며, 이스케이프된 큰따옴표가 원래 텍스트로 풀림
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 4.2_
  - _Difficulty: mid_
  - _Boundary: pie 파서_
  - _Depends: 1.1_

- [x] 2. 차트 렌더링
- [x] 2.1 백분율 계산과 공통 렌더러 연동
  - DONE: 항목별 값으로부터 백분율이 계산되고, 공통 막대·범례 렌더러를 통해 항목 이름·비례 막대·(showData 여부에 따라)값 또는 백분율이 함께 표시됨
  - _Requirements: 2.1, 2.3, 2.4, 3.1_
  - _Difficulty: mid_
  - _Boundary: pie 렌더러_
  - _Depends: 1.2_
- [x] 2.2 제목 표시
  - DONE: pie title 텍스트가 차트 위 또는 캡션 옆에 표시됨
  - _Requirements: 2.2_
  - _Difficulty: low_
  - _Boundary: pie 렌더러_
  - _Depends: 2.1_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 빈 데이터·폭 초과 대체 처리
  - DONE: 값이 모두 0이거나 항목이 없으면 빈 목록 또는 일반 코드블록으로, 렌더링 폭 초과 시 일반 코드블록으로 대체 표시됨
  - _Requirements: 4.1, 4.3_
  - _Difficulty: low_
  - _Boundary: pie 렌더러_
  - _Depends: 2.1_

- [x] 4. 검증
- [x] 4.1 견고성 회귀 테스트
  - DONE: 다양한 폭과 비정상 pie 입력(빈 값, 이스케이프 문자 등)에서 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 4.4_
  - _Difficulty: low_
  - _Boundary: pie 파서, pie 렌더러_
  - _Depends: 3.1_
- [x]* 4.2 시각적 회귀 스냅샷
  - DONE: showData 유무·다항목 대표 예제의 렌더링 결과가 스냅샷과 일치함
  - _Requirements: 2.1, 2.4_
