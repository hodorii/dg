# Implementation Plan — mermaid-block-diagram

## 정의
마크다운에 mermaid `block-beta` 문법으로 박스/그리드 구조를 표현하려는 사용자를 위해, 코드펜스 안 `block-beta` 소스를 터미널 텍스트 다이어그램으로 그려주는 렌더링 기능이다.

- [x] 1. 종류 판별과 기본 파싱 골격
- [x] 1.1 block-beta 지시어 인식 및 dispatch 연결
  - DONE: mermaid 코드펜스 첫 줄이 block-beta면 판별 결과가 "block"으로 나오고, 아니면 기존 종류 판별 결과에 영향이 없음
  - _Requirements: 1.1, 1.2_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 블록 선언과 레이블 파싱
  - DONE: `id["레이블"]` 형태의 블록 선언에서 블록 하나당 아이디와 레이블 텍스트가 추출됨
  - _Requirements: 2.3_
  - _Difficulty: mid_
  - _Boundary: block 파서_
  - _Depends: 1.1_

- [x] 2. 배치 계산과 박스 렌더링
- [x] 2.1 columns 기반 그리드 배치 계산
  - DONE: columns N 지정 시 블록들이 N열 그리드 좌표로 배치되고, 미지정 시 기본 가로 배치 좌표가 계산됨
  - _Requirements: 2.1, 2.2_
  - _Difficulty: high_
  - _Boundary: block 레이아웃_
  - _Depends: 1.2_
- [x] 2.2 (P) 블록 span(가로 병합) 처리
  - DONE: `b:2`처럼 span이 지정된 블록이 그리드에서 지정한 열 수만큼 넓은 폭으로 배치됨
  - _Requirements: 2.4_
  - _Difficulty: mid_
  - _Boundary: block 레이아웃_
  - _Depends: 2.1_
- [x] 2.3 (P) 박스와 텍스트 렌더링
  - DONE: 각 그리드 칸이 테두리 박스로 그려지고 안에 레이블 텍스트가 표시됨
  - _Requirements: 2.3_
  - _Difficulty: mid_
  - _Boundary: block 렌더러_
  - _Depends: 2.1_

- [x] 3. 관계와 그룹 표현
- [x] 3.1 블록 간 화살표 렌더링
  - DONE: `-->`로 연결된 두 블록 사이에 연결선이 그려지고, 라벨이 있으면 함께 표시됨
  - _Requirements: 3.1, 3.2_
  - _Difficulty: high_
  - _Boundary: block 렌더러_
  - _Depends: 2.3_
- [x] 3.2 (P) 중첩 그룹(block...end) 렌더링
  - DONE: block...end로 묶인 블록들이 하나의 바깥 테두리 안에 표시되고, 2단 이상 중첩 시 각 단계가 구분되는 테두리로 표시됨
  - _Requirements: 4.1, 4.2_
  - _Difficulty: high_
  - _Boundary: block 파서, block 레이아웃_
  - _Depends: 2.1_

- [x] 4. 경계·오류 대체 처리
- [x] 4.1 표시 실패·빈 입력 대체 처리
  - DONE: 렌더링 폭 초과 또는 구문이 비어있거나 불완전한 입력에서 일반 코드블록 형태(╭─ mermaid ─...╰─)로 대체 표시됨
  - _Requirements: 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: block 렌더러_
  - _Depends: 3.1, 3.2_

- [x] 5. 검증
- [x] 5.1 견고성 회귀 테스트
  - DONE: 잘리거나 비정상적인 block-beta 입력 목록에 대해 다양한 폭에서 렌더링을 시도해도 비정상 종료 없이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: block 파서, block 렌더러_
  - _Depends: 4.1_
- [x]* 5.2 시각적 회귀 스냅샷 테스트
  - DONE: columns/span/화살표/중첩 그룹을 포함한 대표 예제들의 렌더링 결과가 스냅샷과 일치함
  - _Requirements: 2.1, 2.4, 3.1, 4.1_
