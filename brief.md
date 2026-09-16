# Brief: mermaid 블록 다이어그램·차트 지원

## Problem
dg 사용자가 마크다운에 mermaid `block-beta`(블록 다이어그램)나 `pie`/`xychart-beta`/`gantt`/`quadrantChart`/`gitGraph` 문법을 써도 dg가 이를 인식하지 못해 아무것도 렌더링되지 않는다.

## Current State
`src/diagram/mermaid/mod.rs`의 `kind_of`가 `flowchart/graph`, `sequenceDiagram`, `stateDiagram(-v2)`, `erDiagram`, `classDiagram(-v2)` 5개 키워드만 인식한다. 위 6개 신규 키워드는 매치에 없어 `None`을 반환하고, `language_of_source`도 이를 mermaid로 식별하지 못해 코드펜스가 그냥 원문으로 남는다. 리포지토리 전체에 block/pie/xychart/gantt/quadrant/gitgraph 관련 파서나 레이아웃 코드가 전혀 없다(그린필드).

## Desired Outcome
- `block-beta`, `pie`, `xychart-beta`, `gantt`, `quadrantChart`, `gitGraph` 6종이 각각 `kind_of`로 판별되고, 기존과 동일한 캡션(`◈ mermaid · <kind> ─`) 규칙과 폭 제한/실패 시 `None` 계약을 지키며 터미널 텍스트로 렌더링된다.
- 새 종류들도 `DiagramOptions`(테마·강제 방향 등)와 기존 robustness 테스트 패턴을 그대로 따른다.

## Approach
기존 아키텍처 패턴(파서 모듈 `src/diagram/mermaid/<kind>.rs` → 전용 IR/구조체 → `layout::*` 렌더러 → `mod.rs`의 `kind_of`/`render` 매치 암 1줄 추가)을 그대로 따른다. 6종은 렌더링 알고리즘 성격이 서로 달라 세 갈래로 나눈다:
- **그래프형**: `block-beta`(박스+그리드 배치, 기존 `layout::graph` 확장 여지 검토), `gitGraph`(브랜치 트랙 + 커밋 노드, `layout::sequence`의 레인 개념과 유사).
- **수치형 차트**: `pie`, `xychart-beta`, `gantt`, `quadrantChart` — 축·막대·범례·값 포맷팅을 공유할 공통 레이아웃 기반(가칭 `layout::chart`)이 없으면 4종이 각자 중복 구현하게 되어 SSoT를 어긴다. 공통 기반을 먼저 만들고 그 위에 얹는다.

## Scope
- **In**: `block-beta`, `pie`, `xychart-beta`, `gantt`, `quadrantChart`, `gitGraph` 6종의 파싱 + 터미널 렌더링, `kind_of`/`render` dispatch 확장, README 지원 표 갱신.
- **Out**: PlantUML 쪽 동등 차트 대응(별도 요청 시 후속 스펙), 색상/스타일 커스터마이징 확장(기존 `Theme` 그대로), 실제 그래픽/이미지 출력(순수 텍스트만), 애니메이션·인터랙션.

## Boundary Candidates
- 블록 다이어그램(그래프형) 렌더링
- 수치 차트 공통 레이아웃 기반(축·막대·범례·값 포맷)
- 개별 수치 차트 파서 4종(pie / xychart / gantt / quadrant)
- gitGraph(브랜치 그래프) 렌더링

## Out of Boundary
- PlantUML 차트 대응
- 실시간/애니메이션 렌더링
- 새 색상 테마 체계 도입(기존 `Theme::dark`/`Theme::none` 확장 없음)

## Upstream / Downstream
- **Upstream**: `diagram::mermaid::{kind_of, render}` dispatch, `diagram::layout::{graph, sequence}`, `diagram::ir::Graph`, `diagram::options::DiagramOptions`, `line::{Line, Span}` 렌더링 유틸.
- **Downstream**: README 다이어그램 지원 표, `examples/lib_usage.rs`, CLI 사용자 문서.

## Existing Spec Touchpoints
- **Extends**: 없음 — 이 리포지토리에 기존 spec 산출물이 없어 이번이 spec 시스템 첫 적용이다.
- **Adjacent**: `src/diagram/mermaid/{flow,er,class,state,sequence}.rs`(파서 패턴 참고), `src/diagram/layout/{graph,sequence,shape}.rs`(레이아웃 패턴 참고), `src/diagram/mod.rs`의 `robustness` 테스트(신규 종류도 이 패턴으로 견고성 테스트 추가).

## Constraints
- 순수 텍스트/유니코드 박스 문자 렌더링만 가능(이미지 불가).
- 폭을 초과하거나 렌더링 실패 시 `None`을 반환하는 기존 계약 유지, panic 금지(`panic = "abort"` 프로필 + robustness 테스트 통과).
- `Cargo.toml` 의존성 정책상 신규 크레이트 추가는 최소화(현재 `pulldown-cmark`, `unicode-width`, cli 피처의 `clap`/`crossterm`만 존재).
- `edition = "2024"`, `rust-version = "1.88"` 유지.
