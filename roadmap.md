# Roadmap

## Overview
dg 터미널 렌더러에 mermaid `block-beta`(블록 다이어그램)와 5종 차트(`pie`, `xychart-beta`, `gantt`, `quadrantChart`, `gitGraph`)를 추가한다. 기존 mermaid 서브모듈 패턴(파서 → 전용 구조체 → `layout::*` 렌더러 → `mod.rs` dispatch 1줄 등록)을 그대로 따르되, 렌더링 알고리즘이 서로 다른 세 갈래(그래프형/수치 차트형/브랜치형)로 나누어 독립 스펙으로 분해한다.

## Approach Decision
- **Chosen**: 종류별 독립 스펙 분해 + 수치 차트 4종(`pie`/`xychart-beta`/`gantt`/`quadrantChart`)이 공유할 공통 차트 레이아웃 기반(가칭 `layout::chart`)을 먼저 만들고 그 위에 개별 차트를 얹는 순서.
- **Why**: 코드베이스가 이미 "다이어그램 종류 = 독립 모듈 + `mod.rs` 1줄 등록" 패턴을 확립해뒀고(SRP), 그래프형·수치형·브랜치형은 레이아웃 알고리즘 자체가 다르다. 수치 차트 4종은 축·막대·범례·값 포맷팅이 겹치므로 공통 기반 없이 각자 구현하면 SSoT를 어긴다.
- **Rejected alternatives**: (1) 6종을 한 스펙에 몰아넣기 — 범위가 너무 커서 리뷰·테스트 단위가 흐려짐. (2) 공통 차트 기반 없이 4종을 각자 독립 구현 — 값 포맷·범례·캡션 규칙이 미묘하게 갈라질 위험(SSoT 위반).

## Scope
- **In**: `block-beta`, `pie`, `xychart-beta`, `gantt`, `quadrantChart`, `gitGraph` 파싱 + 렌더링, dispatch 확장, README 표 갱신.
- **Out**: PlantUML 대응 차트, 색상 테마 확장, 이미지/애니메이션 출력.

## Constraints
순수 텍스트 렌더링만, 폭 초과·실패 시 `None` 반환 계약 유지, panic 금지, 신규 크레이트 추가 최소화, `edition 2024`/`rust 1.88` 유지. (상세는 brief.md 참조)

## Boundary Strategy
- **Why this split**: 그래프형(노드+엣지 — `block-beta`, 기존 `layout::graph` 확장 여지)과 브랜치형(트랙+커밋 — `gitGraph`, `layout::sequence`의 레인 개념과 유사)과 수치형(축+값 — `pie`/`xychart`/`gantt`/`quadrant`, 완전히 새 레이아웃)은 렌더링 알고리즘 자체가 갈라지므로 인프라 레이어에서부터 나눈다.
- **Shared seams to watch**: `mermaid::mod.rs`의 `kind_of`/`render` 매치 암이 스펙마다 한 줄씩만 늘어나도록 규율할 것, 신규 타입도 `DiagramOptions`/`Theme`을 동일하게 존중하는지, 캡션 포맷(`◈ mermaid · <kind> ─`)과 README/examples 동기화, `layout::chart` 공통 기반의 API를 pie 스펙에서 확정한 뒤 xychart/gantt/quadrant가 그대로 재사용(중간에 API를 바꾸면 이미 구현된 스펙까지 흔들림).

## Specs (dependency order)
- [ ] mermaid-block-diagram -- `block-beta` 파싱과 그리드/박스 레이아웃 렌더링. Dependencies: none
- [ ] mermaid-gitgraph -- `gitGraph` 파싱과 브랜치 트랙·커밋 렌더링. Dependencies: none
- [ ] mermaid-chart-foundation -- pie/xychart/gantt/quadrant가 공유할 축·막대·범례·값 포맷팅 공통 레이아웃(`layout::chart`) 마련. Dependencies: none
- [ ] mermaid-pie-chart -- `pie` 파싱 + 비율 막대·범례 렌더링(공통 기반의 첫 소비자, API 확정). Dependencies: mermaid-chart-foundation
- [ ] mermaid-xychart -- `xychart-beta` 파싱 + X/Y 축 막대·선 렌더링. Dependencies: mermaid-chart-foundation, mermaid-pie-chart
- [ ] mermaid-quadrant-chart -- `quadrantChart` 파싱 + 4분면 좌표 배치 렌더링(xychart의 좌표축 관례 재사용). Dependencies: mermaid-xychart
- [ ] mermaid-gantt -- `gantt` 파싱(날짜/기간) + 타임라인 바 렌더링. Dependencies: mermaid-chart-foundation
