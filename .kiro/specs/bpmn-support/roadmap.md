# Roadmap

## Overview
BPMN 2.0 렌더링을 세 입력 문법(BPMN XML/biz-process.md/향후 mermaid)이 공유하는
하나의 능력으로 만든다. 새 레이아웃 엔진 없이 기존 `layout::graph`에 레인
그룹 모드를 얹고, 세 파서가 `bpmn::Model`이라는 공유 의미 계층을 거쳐
`ir::Graph`로 내려가는 구조다. `nwdiag`(`plantuml-wbs-nwdiag`)보다 우선한다.

## Approach Decision
- **Chosen**: 6개 스펙으로 분리(YAML을 XML의 대안 직렬화로 별도 스펙화하며
  2026-09-25 5개에서 늘어남), 각자 독립 검증 가능한 단위(파서 없이도
  테스트 가능한 것부터 시작)
- **Why**: `research.md`의 결정 사항이 이미 상당히 구체적이라(그래프 확장
  지점 7곳, 도형 어휘, XML v1 범위, biz-process 매핑까지) 한 스펙으로
  묶으면 승인 게이트가 너무 커진다. 파서 없이 손으로 만든 `Graph`로
  검증되는 레이아웃 부분을 가장 먼저 떼어 위험을 조기에 확인한다.
- **Rejected alternatives**: (1) 한 스펙으로 통짜 진행 — 검증 단위가 너무
  커져 회귀 원인 추적이 어려워짐. (2) 파서(XML)부터 시작 — 레이아웃이
  안 되면 파서 결과를 그릴 데가 없어 순서가 안 맞음.

## Scope
- **In**: `research.md`에 확정된 전체 범위(레인 레이아웃, BPMN 도형 어휘,
  XML v1 하위집합, YAML 대안 직렬화, biz-process.md 드릴다운 매핑)
- **Out**: `nwdiag`(별도 스펙, 후순위), mermaid BPMN 실제 구현(문법 미확정),
  방법론 문서(`biz-process-rules.md`) 갱신(별도 트랙)

## Constraints
- 의존성 4개·단일 정적 바이너리 유지
- 레인 없는 기존 다이어그램(subgraph 포함) 렌더링 결과 불변(바이트 동일 회귀 기준)

## Boundary Strategy
- **Why this split**: 레이아웃(1) → 도형(2) → 공유 계층+렌더 배선(3) → XML
  파서(4) → YAML 파서(5, XML이 검증한 모델의 대안 직렬화) → biz-process
  파서(6) 순으로, 각 단계가 이전 단계 없이는 테스트할 방법이 없는 순서를
  그대로 스펙 순서로 삼는다.
- **Shared seams to watch**: `layout::graph`의 `place_block`/`group_layer_ranges`
  /`push_outputs_below_groups`(레인 모드 분기점), `ir::Shape`/`ir::Marker`
  (도형·표식 추가 지점), `diagram::mod::Language`(신규 `Bpmn`/`BizProcess`
  판별 지점).

## Specs (dependency order)
- [x] bpmn-lane-layout -- `GroupKind::Lane` 추가 + `layout::graph` 7개 지점
  수정(간격 -1, 배너, 순서 고정 등). 파서 없이 손으로 만든 `Graph`로 검증.
  Dependencies: none
- [x] bpmn-shapes -- `Shape::Event(EventPosition)`·`Shape::Subprocess`·
  `Marker::Slash` 등 BPMN 도형·표식 어휘(`ir.rs`/`layout/shape.rs`/
  `layout/graph.rs`에 직접 추가 — 별도 `glyphs.rs` 파일 없이 완료).
  Dependencies: bpmn-lane-layout
- [ ] bpmn-model -- `bpmn::{Model, validate, lower}` + `render()` 배선,
  `Language::Bpmn` 캡션·판별, 토큰·라벨 어휘 표는 `bpmn/vocabulary.rs`.
  손으로 만든 `Model`로 end-to-end 테스트.
  Dependencies: bpmn-shapes
- [ ] bpmn-xml -- `bpmn/{xml, parse_xml}.rs`(OMG Descriptive Level 1
  하위집합). 실제 BPMN XML 샘플 fixture, 손상 입력 강건성 테스트. 공식
  표준 그대로 구현해 `bpmn::Model`이 표준 충실도를 먼저 검증받도록
  YAML보다 앞에 둔다(사용자 결정, 2026-09-26 로드맵 재정렬 — "모델 충실도
  유지": YAML이 손으로 만든 편의 문법이라 그것이 먼저 모델 형태를 정하면
  모델이 표준이 아니라 YAML에 맞춰질 위험이 있음).
  Dependencies: bpmn-model
- [ ] bpmn-yaml -- `bpmn/{yaml, parse_yaml}.rs`(YAML 블록 스타일 하위집합
  손파싱, 크레이트 추가 없음 — research.md의 fable 리서치로 확정된 문법
  · 예시 · 기존 파서 재사용 지점 그대로). `bpmn-xml`이 검증한 `bpmn::Model`을
  그대로 옮기는 대안 직렬화로 뒤에 둔다.
  Dependencies: bpmn-model, bpmn-xml
- [ ] bizprocess-bpmn -- `bpmn/bizprocess.rs`(L1~L5+Logic(AST) → Model,
  `participant:` 태그 파싱, `ExpandPolicy`/`depth` 옵션), 저장소의 기존
  `biz-process.md` 7개를 회귀 fixture로 사용.
  Dependencies: bpmn-model

<!-- 이후(이 로드맵 밖, 별도 스펙으로 재개):
plantuml-wbs-nwdiag (nwdiag) — 이 로드맵 전부 완료 후 재개
방법론 트랙(biz-process-rules.md에 participant: 태그 문법 공식 반영) — 별도 결정
-->
