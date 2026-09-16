# Implementation Plan — mermaid-gantt

## 정의
일정과 작업 기간을 mermaid `gantt` 또는 PlantUML `@startgantt` 문법으로 표현하려는 사용자를 위해, 코드펜스 안 소스를 섹션·작업별 타임라인 막대로 이루어진 터미널 텍스트 차트로 그려주는 기능이다.

- [x] 1. 종류 판별과 파싱
- [x] 1.1 gantt 지시어 인식 및 dispatch 연결
  - DONE: 첫 지시어가 gantt면 판별 결과가 "gantt"로 나옴
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: mermaid 종류 판별_
- [x] 1.2 title/dateFormat/section 파싱
  - DONE: title 텍스트, dateFormat 지정, section 이름별 작업 그룹 경계가 각각 추출됨
  - _Requirements: 2.1, 2.3, 4.1_
  - _Difficulty: mid_
  - _Boundary: gantt 파서_
  - _Depends: 1.1_
- [x] 1.3 작업 줄 파싱(이름/id/시작일 또는 after/기간)
  - DONE: 작업 줄에서 이름, id, 시작일 또는 `after <id>` 참조, 기간이 dateFormat에 맞춰 해석되어 추출됨
  - _Requirements: 2.2, 3.1, 3.2, 3.3_
  - _Difficulty: high_
  - _Boundary: gantt 파서_
  - _Depends: 1.2_

- [x] 2. 타임라인 렌더링
- [x] 2.1 타임라인 축과 섹션 배치
  - DONE: 전체 일정 범위가 폭에 맞춰 축소 비례한 타임라인 위에 섹션별로 작업들이 그룹 구분되어 배치됨
  - _Requirements: 2.1, 5.1_
  - _Difficulty: high_
  - _Boundary: gantt 렌더러_
  - _Depends: 1.3_
- [x] 2.2 작업 막대 렌더링과 선행 관계 반영
  - DONE: 각 작업이 이름과 기간에 비례한 막대로 표시되고, after로 참조된 작업은 선행 작업이 끝나는 시점부터 이어서 시작됨
  - _Requirements: 2.2, 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: gantt 렌더러_
  - _Depends: 2.1_

- [x] 3. 경계·오류 대체 처리
- [x] 3.1 잘못된 참조·항목 초과 대체 처리
  - DONE: 존재하지 않는 작업id를 after로 참조하면 기준일부터 시작하는 것으로 처리되거나 일반 코드블록으로 대체되고, 작업 수가 세로 공간을 초과하면 생략 표시 또는 코드블록으로 대체됨
  - _Requirements: 3.3, 5.2_
  - _Difficulty: mid_
  - _Boundary: gantt 렌더러_
  - _Depends: 2.2_

- [x] 4. PlantUML `@startgantt` 지원
- [x] 4.1 `@startgantt` 판별 및 dispatch 연결
  - DONE: 코드펜스가 `@startgantt`로 시작하면 gantt 종류로 판별되어 렌더링 시도됨
  - _Requirements: 6.1_
  - _Difficulty: low_
  - _Boundary: plantuml 종류 판별_
- [x] 4.2 `Project starts`/작업 선언/기간 파싱
  - DONE: `Project starts <날짜>` 기준일과 `[작업명] requires N days`(`week`/`and` 결합 포함) 기간이 mermaid gantt와 같은 내부 표현으로 추출됨
  - _Requirements: 6.2, 6.5_
  - _Difficulty: mid_
  - _Boundary: plantuml gantt 파서_
  - _Depends: 4.1_
- [x] 4.3 시작일·순차 연결(`then`)·제약(`starts at ...'s end`) 파싱
  - DONE: 절대/상대(`D+N`) 시작일, `then`으로 이어진 작업, `starts at [다른 작업]'s end`류 제약이 이전 작업 종료 시점 기준으로 해석됨
  - _Requirements: 6.3, 6.4_
  - _Difficulty: high_
  - _Boundary: plantuml gantt 파서_
  - _Depends: 4.2_
- [x] 4.4 mermaid gantt와 렌더러 공유
  - DONE: PlantUML gantt 파서가 만든 내부 표현이 mermaid gantt와 같은 타임라인 렌더러를 그대로 통과해, 같은 일정이면 두 언어의 출력이 동일하게 나옴
  - _Requirements: 6.6_
  - _Difficulty: mid_
  - _Boundary: gantt 렌더러_
  - _Depends: 4.3, 2.2_

- [x] 5. 검증
- [x] 5.1 견고성 회귀 테스트
  - DONE: 순환 참조·존재하지 않는 id·다양한 폭 등 대표 입력(mermaid·PlantUML 둘 다)에서 비정상 종료 없이 렌더링이 끝남
  - _Requirements: 5.3_
  - _Difficulty: low_
  - _Boundary: gantt 파서, plantuml gantt 파서, gantt 렌더러_
  - _Depends: 3.1, 4.4_
- [x]* 5.2 시각적 회귀 스냅샷
  - DONE: 섹션·after 연쇄를 포함한 mermaid 예제와, 동등한 PlantUML `@startgantt` 예제의 렌더링 결과가 각각 스냅샷과 일치함
  - _Requirements: 2.1, 3.1, 6.6_
