# Brief: dg 정체성 확립과 다이어그램·마크다운 품질 개선

## Problem
dg의 정체성을 "다이어그램 렌더링 + 마크다운 완전 지원"의 두 축으로 재정의하려는데, 그 기준으로 1차 discovery(dg-mdview 구현분석)의 Out of Boundary 판단 일부가 재검토돼야 하고, 별도로 시퀀스·gitGraph 다이어그램 렌더링의 구체적 결함·개선 요구가 여러 건 확인됐다. 이걸 하나의 로드맵으로 정리할 근거가 없었다.

## Current State
- (1차 discovery 유지) dg는 IR 하나 + 배치기 2개로 mermaid+PlantUML을 처리하고, `dg-watch-mode`/`diagram-panic-audit`/`graph-swap-threshold-reference` 세 항목이 이미 로드맵에 있다.
- dg는 이미 lib+bin 이중 구조(`[lib]` + `cli` feature 게이팅)라 spec-viewer 같은 외부 소비자가 `default-features = false`로 순수 렌더러만 가져다 쓸 수 있다(`Cargo.toml`, `src/lib.rs`). mdview는 `[lib]`/`src/lib.rs`가 없는 bin 전용 크레이트라, spec-viewer의 `engine-mdview`가 실제 의존이 아니라 코드를 통째로 포팅해 쓴다(SSoT 위반 소지 — 다만 mdview 저장소 소관).
- dg 페이저의 `o` 토글은 다이어그램 블록 하나의 원문만 캡션 아래 펼쳐 보여주고(`markdown::render_source_block`), 마크다운 마크업(`#`/`##`/`**` 등) 자체를 하이라이팅하는 기능은 없다.
- 시퀀스 다이어그램 재귀(self-message) 루프(`layout/sequence.rs:556~573`, `draw_self_message`)는 폭이 정확히 2칸으로 고정돼 있고, 화살촉과 생명선 사이 여백이 0칸이다. 실제 재현(`sequenceDiagram` + `B->>B: retry`) 결과 `│◀╯`처럼 화살촉이 생명선에 간격 없이 바로 붙어 렌더링된다.
- gitGraph는 가로 모드(브랜치=행, 시간=열)만 있고 `--direction tb|lr`가 연결돼 있지 않다(`layout/gitgraph.rs`). 브랜치별 색 구분도 전혀 없어(`diagram_line`/`diagram_accent` 단일 스타일만 사용) 라벨 텍스트로만 브랜치를 구분한다.

## Desired Outcome
- dg의 정체성이 "다이어그램 렌더링"과 "마크다운 완전 지원" 두 축으로 명시되고, 각 축에 맞는 스펙이 로드맵에 배치된다.
- 마크다운 원문을 마크업 하이라이팅과 함께 보는 기능이 생긴다.
- 시퀀스 재귀 메시지의 화살촉과 생명선 사이에 시각적으로 구분되는 여백이 생긴다.
- gitGraph에 세로 모드가 생기고, 브랜치 구분이 색+선패턴 이중화로 `--style none`에서도 식별 가능해진다.
- mdview로 이식해야 할 항목·mdview 자체 구조 개선 필요성은 dg 스펙 시스템 밖에 기록만 되고 실행되지 않는다.

## Approach
1차 discovery의 세 항목(watch 모드, panic 감사, 문턱값 레퍼런스)은 그대로 유지하고, 이번에 나온 항목을 정체성 두 축에 맞춰 분류한다: "md full support" 축(마크다운 원문 하이라이팅)과 "diagram" 축(시퀀스 재귀 여백, gitGraph 세로 모드, gitGraph 브랜치 구분) 스펙 후보를 추가한다. 신규 의존성이 필요한 항목(syntect 기반 코드펜스 언어 하이라이팅, 이미지 프로토콜 등)은 여전히 배제한다 — "md full support"는 CommonMark/GFM 완성도와 dg 자체 마크업 표현의 문제이지, 서드파티 렌더러를 들이는 문제가 아니다.

## Scope
- **In**: 마크다운 원문 하이라이팅 모드, 시퀀스 재귀 여백 수정, gitGraph 세로 모드, gitGraph 브랜치 색+패턴 구분, (1차 유지) watch 모드·panic 감사·문턱값 레퍼런스.
- **Out**: mdview 리포지토리 수정 전반(PlantUML/방향재시도/ER표기법 이식, lib 분리), dg에 syntect 코드펜스 언어 하이라이팅·이미지 프로토콜·파일브라우저·GitHub 소스 fetch·스태시.

## Boundary Candidates
- 마크다운 원문 하이라이팅(신택스 컬러링)
- 시퀀스 다이어그램 재귀 메시지 레이아웃
- gitGraph 방향(세로/가로) 배치
- gitGraph 브랜치 시각 구분(색+패턴)

## Out of Boundary
- mdview 저장소 수정 전부(PlantUML/방향재시도/ER표기법 이식, lib 분리) — 다른 리포지토리, dg 스펙 시스템 소관 아님
- dg에 syntect 기반 코드펜스 "언어" 하이라이팅, 이미지 프로토콜(sixel), 파일브라우저, GitHub 소스 fetch, 스태시 — 의존성 최소화 원칙과 충돌(마크다운 마크업 자체 하이라이팅과는 다른 문제)

## Upstream / Downstream
- **Upstream**: `markdown::render_source_block`/`pager.rs`(원문 하이라이팅이 얹힐 기존 경로), `diagram::layout::sequence`(재귀 레이아웃), `diagram::layout::gitgraph`(방향·색 배치), `diagram::mermaid::gitgraph`(방향 지시자 파싱, 기존 `direction` 지시자 재사용), `style::Theme`(브랜치 색·`LineKind` 패턴).
- **Downstream**: README 지원 문법/옵션 표, robustness 테스트 패턴.

## Existing Spec Touchpoints
- **Extends**: 없음 — 기존 `mermaid-*` 7개 스펙은 완료 상태.
- **Adjacent**: `diagram/layout/sequence.rs`(`draw_self_message`), `diagram/layout/gitgraph.rs`, `diagram/mermaid/gitgraph.rs`, `markdown/mod.rs`(`render_source_block`), `pager.rs`(`o` 토글 패턴).

## Constraints
- 신규 의존성 최소화 원칙 유지(현재 4개: pulldown-cmark, crossterm, unicode-width, clap) — 원문 하이라이팅은 dg 자체 `Style`/`Theme`만으로 구현.
- panic 금지(`panic = "abort"` 프로필) 계약 유지.
- `edition = "2024"`, `rust-version = "1.88"` 유지.
- 기존 `--direction tb|lr` 지시자/CLI 플래그 메커니즘을 gitGraph에도 재사용(새 플래그 체계 만들지 않음).
- mdview 리포지토리에는 이번 스펙 시스템에서 아무 것도 쓰지 않는다(경계 밖).
