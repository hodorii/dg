# Brief: bpmn-support

## Problem
BPMN 2.0 프로세스를 문서화하는 사용자, 그리고 이 저장소의 Kiro 방법론으로
`biz-process.md`를 쓰는 사용자가 대상이다. 지금 dg는 mermaid·PlantUML의 기존
다이어그램 종류만 그리고, BPMN 표기법은 전혀 지원하지 않는다. `biz-process.md`는
이미 프로세스를 L1~L5 계층으로 구조화하고 있지만 문자 그림으로 볼 방법이 없다.

## Current State
- 조사 결과(mermaid 이슈 #8160/PR #8166·#8313, PlantUML 포럼) mermaid·PlantUML
  둘 다 아직 확정된 BPMN 문법이 없다 — mermaid는 경쟁하는 두 베타 PR이 서로
  충돌 중이고, PlantUML은 네이티브 지원 자체가 없다.
- `nwdiag`(PlantUML 네트워크 다이어그램)도 별도로 검토했으나, 사용 빈도가
  낮다고 판단해 **BPMN을 먼저** 진행하기로 했다(`.kiro/specs/plantuml-wbs-nwdiag/`
  참고, 그쪽은 후순위로 보류).
- `biz-process.md` 실제 예시 7개(`.kiro/specs/*/biz-process.md`)를 직접 확인:
  `## L1 Process` → `### L2 Activity` → `### L3 FunctionGroup/UI` →
  `### L4 Step` → `### L5 DetailStep`(+ `Logic(AST)` IF/THEN/ELSE/THROW
  의사코드) 구조. L2:L3은 거의 1:1, 분기는 L5의 Logic(AST)에서만 나온다.
- dg의 그래프 배치기(`ir::Graph`+`layout::graph::render()`)는 중첩 그룹(subgraph)을
  이미 지원하지만, 그룹 박스가 구성원 범위에 맞춰 줄어드는 내용 적응형이라
  BPMN 레인(다이어그램/부모 전체를 관통하는 고정 띠)에는 그대로 못 쓴다 —
  다만 확장은 크지 않다(그룹 종류에 "레인" 모드 하나 추가 수준, 상세는
  research.md).

## Desired Outcome
세 가지 입력 문법이 하나의 BPMN 렌더링 능력으로 모인다:
1. BPMN 2.0 XML(공식 표준, 안정적 — 지금 바로 착수 가능) **+ 같은 내용을
   담는 YAML 표현도 지원**(사용자 결정, 2026-09-25 갱신 — 코드펜스에 손으로
   쓰기엔 XML이 너무 장황해서 YAML을 동급 입력으로 추가한다. 표준 BPMN YAML
   스키마는 없으므로 이 프로젝트가 `bpmn::Model`을 그대로 옮긴 자체 YAML
   스키마를 정의한다. 4-의존성 제약과 손으로 만든 파서 관례 안에서 YAML을
   얼마나/어떻게 지원할지는 `bpmn-xml` 스펙 착수 전 별도 리서치 필요 —
   research.md 참고)
2. `biz-process.md`(이 프로젝트 자체 산출물) — Sub-Process 접기/펼치기로
   Process/Activity/Step 3단 드릴다운 표현
3. 향후 mermaid가 BPMN 문법을 확정하면 그것도 같은 백엔드로 얹는다(지금은
   설계만, 구현 안 함)

## Approach
독립 리서치 2회(비판적 검토 1회 + 백지 리서치 1회, 상호 검증됨)를 거쳐 합의된
방향: `[세 파서] → bpmn::Model(공유 의미 계층) → validate → lower → ir::Graph
→ 기존 layout::graph::render()`. 새 레이아웃 엔진을 통째로 만들지 않고, 기존
그래프 배치기에 "레인" 그룹 모드 하나를 얹는다. 상세 설계·근거·대안 비교는
`research.md`(fable 리포트 2건 원문 보존)에 있다.

주요 결정(이미 확정, research.md에 근거):
- 태그 이름은 `participant:`(역할/조직/행위자를 포괄하는 상위 개념으로 채택,
  BPMN 메타모델의 엄밀한 Pool/Lane 구분 용어보다 이 프로젝트 편의를 우선)
- `participant:` 태그가 없으면 레인 없이 지금과 동일하게 렌더링(폴백 레인 없음)
- L3(FunctionGroup/UI)은 흐름 단위가 아니라 구조 단위 — 파선 그룹 박스(2개
  이상일 때)로 표현, 제어 흐름 계층(Pool/Lane/Sub-Process)에는 안 넣는다
- 기본 렌더링은 "Process 다이어그램 1장 + L2별 Activity 다이어그램 여러 장"
  (한 장에 전부 펼치기는 폭·중첩 레인 문제로 v1 제외, `all` 옵션은 역할
  전환이 없을 때만 안전하게 허용)
- 방법론 문서(`{{TEMPLATES}}/specs/biz-process.md`, `biz-process-rules.md`)
  갱신은 이번 트랙에서 하지 않는다 — dg 구현을 먼저 진행하고, 방법론 반영은
  별도로 나중에 결정

## Scope
- **In**: `ir::Graph`/`layout::graph`에 레인 그룹 모드 추가, BPMN 도형·표식
  어휘(이벤트/게이트웨이/태스크 종류, 시퀀스·메시지 흐름 구분), BPMN XML
  파서(OMG Descriptive 적합성 수준 하위집합), `biz-process.md` → BPMN 파서
  (Sub-Process 드릴다운 매핑)
- **Out**: BPMN YAML(표준 스키마 없음), mermaid BPMN 문법 실제 구현(문법
  미확정), 풀을 넘는 Group 아티팩트, 경계 이벤트의 진짜 테두리 부착 렌더링,
  이중선(중간 이벤트) 등 세부 도형, `nwdiag`(후순위로 이미 보류)

## Boundary Candidates
- `src/diagram/bpmn/` 신규 모듈군(model/validate/lower/glyphs/xml/parse_xml/
  bizprocess/mod) — mermaid·PlantUML 파서와 달리 `ir::Graph`를 직접 안 만들고
  `bpmn::Model`이라는 공유 의미 계층을 하나 더 거친다(세 파서가 BPMN 어휘
  규칙을 중복 구현하지 않도록)
- `layout::graph`의 레인 확장은 기존 그룹/블록 메커니즘 안에서 이뤄지고,
  gitgraph·sequence·기존 subgraph 동작은 건드리지 않는다(회귀 기준: 레인
  없는 기존 fixture 전부 바이트 동일)

## Out of Boundary
- `biz-process.md` 템플릿·규칙 파일 자체의 수정(방법론 트랙, 이번엔 보류)
- `nwdiag`(별도 스펙, 후순위)

## Upstream / Downstream
- **Upstream**: 없음(신규 다이어그램 종류)
- **Downstream**: `plantuml-wbs-nwdiag`(nwdiag 부분, 이 작업 이후 재개).
  방법론 트랙(`biz-process.md` 태그 문법 공식화)은 이 작업 결과를 참고해
  나중에 별도로 결정

## Existing Spec Touchpoints
- **Extends**: 없음
- **Adjacent**: `plantuml-wbs`(같은 그래프 배치기 재사용 선례), `plantuml-wbs-nwdiag`
  (레인 개념을 먼저 검토했던 곳, 결론이 이 브리프로 옮겨옴)

## Constraints
- 의존성 4개·단일 정적 바이너리 유지(XML은 새 크레이트 없이 손으로 토큰화)
- 레인 없는 기존 다이어그램(subgraph 포함)의 렌더링 결과가 바뀌면 안 됨
- 폭 초과 시 기존 규약(반대 방향 재시도 → 라벨 축소 → 원문 코드블록) 그대로
