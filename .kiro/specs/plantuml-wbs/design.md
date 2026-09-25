# Design — plantuml-wbs

## 정의
PlantUML `@startwbs` 코드펜스를 기존 `diagram::ir::Graph` + `layout::graph`
배치기로 그리는 새 파서(`diagram::plantuml::wbs`)다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **판별**: `diagram::plantuml::mod::kind_of_start_tag()`에 `"wbs" => Some("wbs")`
  한 줄 추가(`@startgantt`와 같은 패턴 — 휴리스틱 점수 매기기를 거치지 않음)
- **파서**: `diagram::plantuml::wbs::parse(source: &str) -> Graph` 신규 모듈
- **`render()` 연결**: `diagram::plantuml::mod::render()`의 매치문에 `"wbs"` 분기
  추가 — `class`/`component`와 같은 패턴(`options.apply_to_graph` 뒤
  `layout::graph::render`)
- **상자 없는 노드 모양**: `diagram::ir::Shape`에 `Plain`(테두리 없이 글자만)
  변형 추가 + `layout::shape`의 `measure`/`draw`에 그 처리 추가(요구사항 2.3)

### Out-of-Scope
- **좌우 동시 분기(산술 표기 `+`/`-`, `<`/`>`)**: 뿌리 하나에서 양쪽으로 갈라지는
  배치는 `layout::graph`가 지금 못 하는 새 능력이라 별도 스펙 대상(brief.md)
- **노드 간 화살표, 인라인 색상, `<style>` 블록, 별칭(`as`)**: 요구사항 3.3에 따라
  구문만 건너뛰고 값은 반영하지 않는다 — 별도 처리 로직을 만들지 않는다

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음)
- 내부 의존 방향: `plantuml::wbs` → `diagram::ir`(`Graph`/`Shape` 사용,
  `Shape::Plain` 추가) → `layout::shape`/`layout::graph`(기존 배치기, 변경은
  `Shape::Plain` 그리기 분기 추가뿐)

### Revalidation Triggers
- `layout::graph`가 노드 모양별 접점 계산 방식을 바꾸면 `Shape::Plain`(테두리
  없음)의 접점 위치도 같이 재검토
- 이 스펙 이후 좌우 분기(mindmap류) 스펙이 진행되면 `Shape::Plain`을 그쪽에서도
  재사용할 수 있는지 확인

## Architecture

### Boundary Map
```mermaid
flowchart LR
  tag[kind_of_start_tag] --> render[plantuml::render]
  render --> parse[plantuml::wbs::parse]
  parse --> graph[ir::Graph]
  graph --> layout[layout::graph::render]
```

### Key Decisions
- **깊이는 절대 단계가 아니라 "직전 스택"으로 판정한다** — 이유: 요구사항 1.4
  (단계를 건너뛴 표기)를 만족하려면 `depth`가 스택에 정확히 맞물릴 필요가
  없다. 새 항목의 `depth`보다 크거나 같은 스택 항목을 전부 pop한 뒤 남은
  스택의 맨 위를 부모로 삼는 표준 아웃라인 파싱 방식(마크다운 목록·mermaid
  mindmap과 같은 부류)을 그대로 쓴다. 스택이 비면 새 뿌리(요구사항 1.3).
- **`*`/`+`/`-` 모두 깊이 표시 글자로 센다** — 이유: 산술 표기(`+`/`-`)는
  방향까지 겸하므로 그 의미는 무시하지만(out-of-scope), 글자 자체를 깊이
  계산에서 빼면 그 줄이 그냥 텍스트로 오인돼 트리가 깨진다. "구문은 건너뛰되
  구조는 안 깨짐"(요구사항 3.3)을 만족하는 가장 단순한 방법이다.
- **여러 줄 본문(`:`~`;`)은 `\n`으로 이어 붙여 기존 `Graph::intern`에 그대로
  넘긴다** — 이유: `intern`이 이미 `label.lines()`로 라벨을 여러 줄 섹션으로
  쪼개므로(예: mermaid `<br/>` 변환과 같은 자리), 새 자료구조 없이 문자열만
  맞추면 된다.
- **`Shape::Plain` 신규 추가(상자 없음)** — 이유: 기존 모양 중 "테두리 없이
  여러 줄 글자만"에 맞는 게 없다(`Interface`는 동그라미+글자 전용).
  `measure()`는 `(tw, th)`(여백 없음), `draw()`는 테두리를 그리지 않고 본문을
  직접 채운다(`draw_sections()`의 "테두리 한 칸" 전제 관례를 그대로 쓰면
  앞뒤 여백이 비대칭으로 남아 재사용하지 않는다 — 구현 중 확인).

## Components and Interfaces

### diagram::plantuml::wbs (신규 모듈)
- Intent: `@startwbs` 본문을 파싱해 `ir::Graph`를 만든다
- Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 3.1, 3.3
```rust
pub fn parse(source: &str) -> Graph
```
- 계약: 반환된 `Graph`는 노드마다 부모 하나(또는 뿌리)로 향하는 간선만 가지며
  마커(화살촉)가 없다(`Edge::default()`, 조직도 스타일 — 요구사항 In-Scope에
  화살표 없음이 이미 전제). 유효한 항목이 하나도 없으면(3.1) 노드 없는 빈
  `Graph`를 돌려주고, `diagram::render_body()`의 기존 "본문이 전부 빈 줄이면
  `None`" 계약이 그대로 원문 대체를 처리한다(신규 처리 불필요).

### diagram::plantuml::mod (기존 파일 확장)
- Intent: `wbs` 종류를 판별·배선한다
- Requirements: 1.1
- 계약: `kind_of_start_tag()`에 `"wbs"` 한 케이스, `render()` 매치문에 `"wbs"`
  한 분기 — 둘 다 `"gantt"`/`"component"` 기존 케이스와 같은 모양

### diagram::ir::Shape / diagram::layout::shape (기존 파일 확장)
- Intent: 테두리 없는 노드 모양을 추가한다
- Requirements: 2.3
- 계약: `Shape::Plain` 변형 추가. `measure()`: `(tw, th)`. `draw()`: 테두리
  없이 본문을 직접 채운다(다른 모양의 기존 분기 순서를 바꾸지 않음).

## Data Models
신규 도메인 타입 없음(`Shape::Plain`은 기존 enum에 변형 추가). `ir::Graph`를
그대로 쓴다.

## Error Handling
- **사용자 입력 오류**: 잘못된 표기(범위 밖 구문)는 무시하고 구조만 살린다(3.3)
- **시스템 오류(패닉)**: 항목이 하나도 없으면 빈 `Graph` → 기존
  `render_body()`의 "본문 전부 빔 → `None`" 경로로 흡수, 새 패닉 경로 없음(3.1)
- **기능 강등**: 폭 초과 시 기존 `layout::graph::render()`의 방향 재시도·실패
  시 `None` 계약 그대로(3.2, 변경 없음)

## Testing Strategy
- **Depth**: Standard — 새 파서 모듈 하나 + 기존 배치기 재사용, 새 상태기계
  없음
- **Unit(L6)**:
  - `*`/`**`/`***` 단계별 부모-자식 간선이 맞는지(1.2)
  - 최상위 `*`가 둘 이상일 때 각각 독립 뿌리로 나오는지(1.3)
  - 단계를 건너뛴 표기가 직전 항목의 자식이 되는지(1.4)
  - `:`~`;` 여러 줄 본문이 한 노드에 여러 줄로 들어가는지(2.2)
  - `_` 접미사 노드가 테두리 없이 그려지는지(2.3, `Shape::Plain` 렌더 테스트)
  - 항목이 없을 때 렌더링이 `None`을 돌려주는지(3.1)
  - 범위 밖 구문(`+`/`-` 방향, 화살표, `[#색]`)이 섞여도 패닉 없이 트리가
    그려지는지(3.3)
- **Integration**: `kind_of("@startwbs\n...\n@endwbs")`가 다른 PlantUML 종류로
  오판별되지 않는지(1.1), 실제 WBS 예시를 release 바이너리로 렌더링해 육안 확인
