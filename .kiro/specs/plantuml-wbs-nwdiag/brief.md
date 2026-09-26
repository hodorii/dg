# Brief: plantuml-wbs-nwdiag

## Problem
dg 사용자, 그중에서도 프로젝트 문서에 WBS(작업 분류 체계)나 네트워크 구성도를
PlantUML로 그려 두고 터미널에서 그대로 보고 싶은 사람이 대상이다. 지금 dg는
PlantUML 5종(시퀀스·클래스·ER·컴포넌트·간트)만 지원해, `@startwbs`나
`@startnwdiag` 코드펜스를 만나면 판별에 실패해 원문 코드블록으로 물러난다.

## Current State
- PlantUML 판별·배치는 `diagram::plantuml::mod::kind_of_start_tag()`가 확정 태그
  (`@startgantt`)를 먼저 보고, 그 외에는 본문 키워드 점수로 시퀀스/클래스/ER/
  컴포넌트 중 하나로 추정한다.
- 그래프형 4종(시퀀스 제외)은 전부 `diagram::ir::Graph` 하나로 모여
  `layout::graph::render()` 배치기 하나를 공유한다(SSoT, README에 명시된 설계
  원칙: "배치기 두 개(계층 그래프, 시퀀스)만 유지").
- WBS(`@startwbs`)는 PlantUML 공식 문법으로, `*`/`**`/`***`(OrgMode) 또는
  `+`/`-`(좌우 방향 지정) 기호로 깊이를 나타내는 **순수 트리**다 — 노드 하나에
  부모 하나, 형태상 이미 dg가 지원하는 flowchart/class 트리와 다르지 않다.
- nwdiag(`@startnwdiag`)는 PlantUML이 blockdiag 계열 문법을 그대로 받아 그리는
  **네트워크 구성도**다 — `network 이름 { ... }` 블록(가로 "버스" 선 하나)에
  노드가 속하고, 한 노드가 여러 network에 동시에 속하면 그 사이를 세로
  "점프선"으로 잇는다. 주소(`address = "..."`)가 각 교차점 라벨로 붙는다.
  루트 하나짜리 트리가 아니라 "가로 선 여러 개 + 그 선들을 가로지르는 세로
  노드 열"이라는, 기존 계층 그래프·시퀀스 배치기 어느 쪽에도 맞지 않는 별도
  구조다.

## Desired Outcome
`@startwbs`/`@startnwdiag` PlantUML 코드펜스가 원문 코드블록으로 물러나지 않고
문자 그림으로 렌더링된다.

## Approach
WBS와 nwdiag는 필요한 작업 규모가 크게 달라 하나로 묶지 않는다.
- **WBS**: 기존 `ir::Graph` + `layout::graph::render()`를 그대로 재사용하는
  새 파서 하나로 끝난다 — `@startgantt`가 이미 쓰는 "확정 태그는 점수 매기기
  없이 바로 분기" 패턴을 그대로 따라 `kind_of_start_tag()`에 `"wbs"`만 추가하면
  된다. 새 배치기가 필요 없다.
- **nwdiag**: 기존 두 배치기 중 어느 것도 "가로 버스 선 + 교차하는 세로 노드
  열 + 다중 소속 점프선" 구조에 맞지 않는다. 새 배치기(세 번째)가 필요하고,
  이는 README가 못박은 "배치기 두 개만 유지"라는 설계 원칙을 깨는 선택이라
  단순 구현 결정이 아니라 확인이 필요한 아키텍처 결정이다.

## Scope
- **In**: `@startwbs` 파서 + 기존 그래프 배치기 연결(WBS). nwdiag는 조사·설계까지만
  이 브리프에 포함하고, 실제 새 배치기 구현 여부는 사용자 확인 후 별도 결정.
- **Out**: mermaid 쪽 mindmap(비슷한 트리형이지만 mermaid엔 없는 문법 — 요청에
  없었음), nwdiag의 PlantUML 고급 옵션(예: `elink`, 커스텀 shape 아이콘) 전체
  구현은 1차 범위 밖 — 최소 실행 가능한 하위집합(네트워크·노드·주소·다중 소속
  점프선)만 우선.

## Boundary Candidates
- WBS 파서는 `diagram::plantuml::wbs`(가칭) 모듈 하나, `class.rs`/`component.rs`와
  같은 층위 — `ir::Graph`를 만들어 넘기기만 하고 배치는 몰라도 된다.
- nwdiag는 만약 진행하면 `diagram::plantuml::nwdiag`(파서) +
  `diagram::layout::nwdiag`(새 배치기) 두 모듈이 필요 — 기존 `ir::Graph`를
  억지로 재사용하면 "네트워크 소속"·"주소 라벨"·"교차 버스 선" 같은 개념이
  안 맞아 나중에 되돌리기 어려운 설계 부채가 된다. 처음부터 별도 IR
  (`ir::Network` 가칭)을 검토해야 한다.

## Out of Boundary
- 이번 브리프는 nwdiag의 "새 배치기 만들지 여부" 자체를 결정하지 않는다 — 그건
  사용자 확인 사항으로 남겨 둔다.

## Upstream / Downstream
- **Upstream**: 없음(신규 다이어그램 종류 추가라 기존 스펙에 의존하지 않음)
- **Downstream**: WBS 스펙이 먼저 끝나면 nwdiag 설계 때 "새 IR을 만들 가치가
  있는지"를 판단하는 참고 사례(비교 대상)가 된다

## Existing Spec Touchpoints
- **Extends**: 없음(완전히 새 다이어그램 종류)
- **Adjacent**: `diagram::plantuml::gantt`(확정 태그로 바로 분기하는 선례),
  `diagram::plantuml::class`/`component`(그래프 IR을 만들어 넘기는 선례 —
  WBS가 그대로 따라 할 패턴)

## Constraints
- 의존성 4개·단일 정적 바이너리 원칙(README) 유지 — 새 크레이트 없이 기존
  파서·배치기 인프라만으로
- nwdiag를 진행한다면 "배치기 두 개만 유지"라는 기존 설계 원칙을 명시적으로
  깨는 것이므로, README의 그 문구도 같이 갱신해야 함
