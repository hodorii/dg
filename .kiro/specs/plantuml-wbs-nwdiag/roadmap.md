# Roadmap

## Overview
PlantUML WBS(`@startwbs`)와 네트워크 구성도(`@startnwdiag`) 지원을 추가한다. 둘의
구현 규모가 크게 달라(WBS는 기존 배치기 재사용, nwdiag는 새 배치기 필요) 별도
스펙으로 분리하고, 규모가 큰 쪽(nwdiag)은 착수 전 사용자 확인을 한 번 더 거친다.

## Approach Decision
- **Chosen**: 2개 스펙으로 분리 — `plantuml-wbs`(즉시 진행 가능), `plantuml-nwdiag`
  (새 배치기 여부부터 확인 필요)
- **Why**: WBS는 순수 트리라 기존 `ir::Graph`+`layout::graph`를 그대로 타므로
  위험이 낮고 결정할 것도 적다. nwdiag는 "가로 버스 선 + 교차하는 세로 노드 열"
  구조가 기존 두 배치기 어디에도 안 맞아, 새 배치기(세 번째)를 만들지 여부부터
  결정해야 다음 단계(요구사항·설계)가 의미 있다. 하나로 묶으면 WBS까지 그 결정을
  기다려야 해서 불필요하게 느려진다.
- **Rejected alternatives**: (1) 둘을 한 스펙으로 묶기 — 위 이유로 기각. (2) nwdiag를
  기존 `ir::Graph`에 억지로 끼워 맞추기 — "네트워크 소속"·"주소 라벨"·"버스 선"
  개념이 그래프의 노드/간선 모델과 안 맞아 나중에 되돌리기 어려운 설계 부채가
  된다고 판단해 기각(brief.md Boundary Candidates).

## Scope
- **In**: `@startwbs` 파서(그래프 IR 재사용), `@startnwdiag` 최소 실행 가능
  하위집합(network 블록·노드·address·다중 소속 점프선) 설계까지
- **Out**: nwdiag 고급 옵션(elink, 커스텀 아이콘 등), mermaid mindmap(요청에
  없었음)

## Constraints
- 의존성 4개·단일 정적 바이너리 유지
- nwdiag를 진행하면 README의 "배치기 두 개만 유지" 문구도 함께 갱신

## Boundary Strategy
- **Why this split**: 위험·결정 사항의 크기가 다른 두 기능을 같은 속도로 묶지
  않기 위해서다 — WBS는 지금 바로 `$kiro-spec-init`으로 들어가도 되지만, nwdiag는
  "새 배치기를 셋으로 늘릴지" 자체가 이 프로젝트의 설계 원칙(README)을 건드리는
  결정이라 사용자 확인이 선행돼야 한다.
- **Shared seams to watch**: `diagram::plantuml::mod::kind_of_start_tag()`(확정
  태그 분기 지점, 둘 다 여기에 한 줄씩 추가), `diagram::ir::Graph`(WBS는 재사용,
  nwdiag는 별도 IR 검토 대상이라 여기서 갈림)

## Specs (dependency order)
- [ ] plantuml-wbs -- `@startwbs` PlantUML WBS를 기존 그래프 배치기로 렌더링한다.
  Dependencies: none
- [ ] plantuml-nwdiag -- `@startnwdiag` PlantUML 네트워크 구성도를 렌더링한다(새
  배치기 필요 — 착수 전 사용자 확인 필요). Dependencies: none(WBS와 독립, 순서
  무관하지만 새 배치기 여부 확인이 먼저)
