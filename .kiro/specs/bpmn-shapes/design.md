# Design — bpmn-shapes

## 정의
`diagram::ir`의 노드 모양·간선 표식 어휘에 `Shape::Event(EventPosition)`·
`Shape::Subprocess`·`Marker::Slash`를 더하고 `layout::shape`/`layout::graph`가 그것을
그리게 하는 확장이다. 게이트웨이·태스크·`«…»` 둘째 줄은 기존 어휘의 라벨 관례로
확정한다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **어휘**: `ir::Shape::{Event(EventPosition), Subprocess}`, `ir::EventPosition`,
  `ir::Marker::Slash`
- **도형 기하**: `layout::shape::{measure, draw}`의 새 분기(§Key Decisions 표)
- **표식 글자**: `layout::graph::marker_glyphs()`의 `Slash` 한 줄
- **라벨 관례(코드 없음, 테스트로 고정)**: 게이트웨이 = `Shape::Diamond` + `기호 이름`,
  태스크 = `Shape::Round` + 둘째 줄 `«종류»`, 이벤트 트리거 = 둘째 줄 `«트리거»`
- **회귀 기준**: 새 어휘를 쓰지 않는 그래프의 출력 바이트 동일(요구사항 6.1)

### Out-of-Scope
- **`LineKind::Double`(중간 이벤트 이중선)**: `bpmn-support/research.md`가 후순위로
  미룸 — `◎` 글자가 위치를 구분하므로 v1 불필요
- **경계 이벤트 테두리 부착·데이터 객체 접힌 모서리·트리거 아이콘 글자**: 미지원. v1
  근사(인접 노드 + 점선, `Rect`+`«data»`, `«이름»`)는 `bpmn-model`의 `lower()`가 만든다
- **`bpmn::Model`·`GatewayKind`/`TaskKind`/`EventTrigger` 어휘 enum·`bpmn/glyphs.rs` 표**:
  `bpmn-model` 소유. 이 스펙은 그 표가 만들어야 할 **라벨 문자열**만 계약으로 정한다
  (§Key Decisions "lower()가 만들어야 할 라벨")
- **파서·코드펜스 언어·CLI·실제 BPMN 문서 렌더링**: `bpmn-yaml`·`bpmn-xml`·
  `bizprocess-bpmn`. 이 스펙은 손으로 만든 `Graph`로만 검증
- **레인 논리**: `bpmn-lane-layout`(완료). 변경 없음 — 새 도형이 레인 안에 놓여도
  패닉 없음만 확인

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음, Rust 2024 edition 그대로)
- 내부 의존 방향: `diagram::ir`(어휘) → `layout::shape`(도형 그리기, `canvas` 사용) ←
  `layout::graph`(노드 배치·간선 표식, `shape`·`canvas` 사용). `layout::shape`가
  `layout::graph`를, `ir`가 `layout`을 import하면 설계 위반. BPMN 이름(`Event`·
  `Subprocess`·`Slash`)은 `ir`에만 등장하고 `layout`은 모양·글자만 안다

### Revalidation Triggers
- `Canvas::line_char()`가 굵은 둥근 모서리를 지원하게 되면 종료 이벤트 모서리 재검토
  (이벤트가 더 이상 상자로 그려지지 않으므로 `bpmn-event-shape-notation` 이후 소멸)
- `layout::graph::segment_span()`이 노드 테두리 칸에서 선을 시작하게 바뀌면 `[+]`
  보존(2.2) 재검토
- `shape::draw_sections()`의 `x+1`/`w-2` 가운데 정렬 관례가 바뀌면 이벤트 위치 글자
  열 재검토
- 기본 모노 폰트 커버리지 재실측 결과가 달라지면(§Key Decisions 글자 표) 기호 재검토

## Architecture

### Boundary Map
```mermaid
flowchart LR
  test[shape.rs and graph.rs tests hand built Graph] --> ir[ir Shape Marker EventPosition]
  ir --> shape[layout shape measure draw]
  ir --> graph[layout graph marker_glyphs]
  graph --> shape
  shape --> canvas[canvas rect put text_centered]
  graph --> canvas
```

### Technology Stack
| Layer | Choice | Role |
|-------|--------|------|
| 도형·표식 | Rust 1.96, 기존 `Canvas` 박스 문자 그리기 | 새 분기 3개, 새 좌표계 없음 |

### Key Decisions
- **이벤트는 변형 하나 + 위치 enum**: `Shape::Event(EventPosition)` — 이유: 세 위치의
  기하가 같고 글자·선 종류만 다르다. 상태도의 `Shape::Start`/`End`와 이름 충돌 회피.
- **종료 이벤트는 각진 굵은 모서리 `┏━┓`**: `rect(..., Heavy, round=false)` — 이유:
  `line_char()`가 `round`를 `heavy`보다 우선해 둥근+굵은은 가는 모서리에 굵은 변이
  붙는 어색한 모양(research.md). (이벤트를 테두리 상자로 그리는 이 결정 전체는
  `bpmn-event-shape-notation`이 대체함 — 테두리 없는 위치 글자 + 이름으로 바뀜.)
- **태스크는 전부 `Shape::Round`(사용자 확정)**: 하위 종류별 새 모양 없음, 둘째 줄
  `«종류»`만 — 이유: 사용자 결정(2026-09-25, 구현상 문제가 없는 한 라운드렉트
  유지); 현재 코드에서 폭·글자 겹침 문제가 없음을 6.3 실물 렌더링으로 확인한다.
  벗어나려면 그 문제를 research.md에 기록해야 한다.
- **게이트웨이는 코드 변경 없음**: `Shape::Diamond`가 임의 라벨을 그리고, 기호는
  `«`로 시작하지 않아 흐려지지 않는다 — 이유: 새 변형이 줄 정보가 없다. 기호 표의
  키는 `bpmn-model` 어휘라 코드 표도 그쪽 소유(research.md).
- **글자 교체 두 건**: `⬠`→`◎`, `∗`→`*` — 이유: 기본 CJK 모노 폰트 커버리지 실측
  (research.md). `crow-foot-orientation`의 "이미 쓰는 글자 재사용" 규칙.
- **`«…»` 흐림·기울임은 신규 로직 없음**: `line_style()`이 도형과 무관하게 처리함을
  코드로 확인 — 이 스펙은 스타일까지 보는 테스트만 더한다.
- **`Slash`는 방향 무관 한 글자**: `Circle`/`Cross`와 같은 패턴 — 이유: `╱`는 세로선
  위에서도 가로선 위에서도 빗금으로 읽히고, 글자 수 1이라 LR 라벨 오프셋·
  `gap_tail_rows`가 기존 규칙으로 맞는다.
- **기하 규칙**(`tw`·`th` = 본문 최대 폭·줄 수, `x`·`y`·`w`·`h` = `draw()` 인자·최종 크기,
  `row0 = y + 1 + extra_top`):

| 규칙 | 내용 | 요구사항 |
|---|---|---|
| Event 크기 | `measure = (tw + 6, th.max(1) + 2)` — 테두리·공백·글자·공백·본문·공백·테두리 | 1.4, 1.6 |
| Event 테두리 | Start·Intermediate: `rect(Solid, round=true)`; End: `rect(Heavy, round=false)`, 색 `theme.diagram_box` | 1.1~1.3 |
| Event 위치 글자 | `(x+2, row0)`에 `○`/`◎`/`●`, 색 `theme.diagram_accent`(상태도 `Start`/`End`와 동일) — 종료 글자는 `bpmn-event-notation-anchor`가 `●` → `◉`로 대체(위치도 원점 고정으로 수정) | 1.1~1.3, 1.6 |
| Event 본문 | `draw_sections(x+2, row0, w-2, sections, separators=false)` → 이름이 `x+4..` 가운데 정렬, 둘째 줄은 `line_style()` 그대로 | 1.1, 3.1, 3.3 |
| Subprocess 크기·테두리·본문 | `Round`와 동일(`(tw+4, th+2)`, `rect(Solid, round=true)`, `draw_sections(x, row0, w, …, true)`) | 2.1, 3.2 |
| Subprocess `[+]` | 본문 뒤 `text_centered(x, y+h-1, w, "[+]", theme.diagram_box)` — 테두리 줄의 `─` 세 칸을 글자 칸으로 바꿈; 이후 `add_line()`은 글자 칸을 건너뜀 | 2.1, 2.2, 2.3 |
| Slash 글자 | `marker_glyphs(Slash, _, _) = ['╱']` | 4.1~4.3 |
| 접점 | 변경 없음 — `draw()`의 `min_width`/`min_height` 처리와 `segment_span()`이 그대로 적용 | 1.5 |

- **`lower()`가 만들어야 할 라벨(계약, `bpmn-model`이 소비)**:

| BPMN 요소 | `Shape` | `sections[0]` |
|---|---|---|
| 시작/중간/종료 이벤트 | `Event(Start/Intermediate/End)` | `[이름]` 또는 `[이름, "«trigger»"]`; 이름 없으면 `[]` |
| 태스크(7종)·호출 활동 | `Round` | `[이름]`(종류 none) 또는 `[이름, "«user»"]`… `«call»` |
| 접힌 서브프로세스 | `Subprocess` | `[이름]` |
| 게이트웨이 XOR/AND/OR/복합/이벤트기반 | `Diamond` | `["× 이름"]`/`["+ 이름"]`/`["○ 이름"]`/`["* 이름"]`/`["◎ 이름"]` |
| default 흐름 | 간선 `tail: Marker::Slash` | — |

## Components and Interfaces

### diagram::ir — Shape·Marker 어휘 (기존 파일 확장)
- Intent: BPMN 이벤트·서브프로세스·default 흐름을 배치기에 전달하는 이름
- Requirements: 1.1~1.6, 2.1, 4.1
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPosition { Start, Intermediate, End }

pub enum Shape {
    /* 기존 변형 … */
    /// BPMN 이벤트: 둥근 상자, 본문 왼쪽에 위치 글자(○/◎/●). End는 굵은 테두리.
    Event(EventPosition),
    /// BPMN 접힌 서브프로세스: Round + 아래 테두리 가운데 `[+]`.
    Subprocess,
}

pub enum Marker {
    /* 기존 변형 … */
    /// BPMN default 시퀀스 흐름의 빗금 꼬리 `╱`.
    Slash,
}
```
- 계약: `Shape`·`Marker`의 `Default`·`Copy`·`Eq` 파생 유지. 기존 변형 순서·이름 불변.
  `Shape::Anchor` 판정(`!= Shape::Anchor`)만 쓰는 `layout::graph`·`layout::block`은
  새 변형에 영향 없음(컴파일러가 `measure`/`draw`/`marker_glyphs`의 빠진 분기를 잡는다).

### diagram::layout::shape — Event·Subprocess 그리기 (기존 파일 확장)
- Intent: §Key Decisions 기하 규칙의 `measure`/`draw` 분기
- Requirements: 1.1~1.4, 1.6, 2.1, 2.3, 3.1~3.4, 5.1, 5.2, 6.2
```rust
pub fn measure(shape: Shape, sections: &[Vec<String>]) -> (usize, usize); // Event·Subprocess 분기 추가
pub fn draw(canvas: &mut Canvas, x: usize, y: usize, shape: Shape, sections: &[Vec<String>],
            theme: &Theme, min_width: usize, min_height: usize);           // 동일
```
- 계약: 다른 모양의 분기 순서·출력 불변. `Event`는 `draw_sections()`를 재사용(스타일
  규칙 한 곳). 테스트 헬퍼 `draw_runs(shape, sections) -> Vec<Vec<(String, Style)>>`
  (`Line::runs()` 기반)를 더해 흐림·기울임·굵음을 단정한다 — 기존 `draw_rows()`는
  `Line::plain()`이라 스타일을 못 본다.

### diagram::layout::graph — marker_glyphs (기존 파일 확장)
- Intent: `Marker::Slash` 글자
- Requirements: 4.1~4.3, 1.5, 2.2, 6.1~6.3
- 계약: `marker_glyphs()` 표에 `Marker::Slash => vec!['╱']` 한 줄. 다른 함수 무변경 —
  1.5·2.2·6.3은 기존 배치 규칙이 새 도형에도 성립함을 `render()` 테스트로 고정한다.

## Data Models
`EventPosition`·`Shape`·`Marker` 변형이 전부(위 인터페이스 블록). 불변식: `layout`은
BPMN 의미를 모른다 — `Event`는 "글자 하나 붙은 둥근 상자", `Slash`는 "빗금 한 글자".

## Error Handling
- **사용자 입력 오류**: 없음(파서 없음). 본문이 빈 `Event`는 `th.max(1)`로 한 줄
  본문을 확보(1.6)
- **외부 자원 오류**: 해당 없음
- **시스템 오류(패닉)**: `Subprocess` 폭은 `tw+4 ≥ 5`라 `[+]`가 모서리를 덮지 않음;
  `Event`의 `x+2`·`x+4`는 `w ≥ 6` 안. 이름 없음·한 글자·여러 줄·레인 안·양방향 조합을
  단위 테스트로 패닉 없음 확인(6.2)
- **기능 강등**: 폭 초과 시 기존 `render()` 규약(반대 방향 → 라벨 축소 → `None`)
  그대로. 6.3의 예시가 폭 100을 넘으면 라벨 축소 전 단계에서 통과해야 한다(넘으면
  태스크 모양 결정의 "구체적 구현 문제"에 해당 — research.md에 기록 후 재검토)

## Testing Strategy
- **Depth**: Standard — 신규 파서·화면 없음, 기존 도형 그리기의 분기 확장. E2E는 파서
  부재로 해당 없음(`bpmn-model`에서 수행)
- **Unit(L6, `layout::shape::tests`, `draw_rows`/`draw_runs`)**:
  - 회귀(무수정 통과): `rect_with_two_sections`, `cylinder_has_lid`,
    `diamond_and_hexagon_render_differently`, `plain_has_no_border`,
    `plain_supports_multiple_lines`(6.1)
  - Event Start "Go" → `["╭──────╮", "│ ○ Go │", "╰──────╯"]`(1.1, 1.4); Intermediate →
    `◎`(1.2); End → `["┏━━━━━━┓", "┃ ● Go ┃", "┗━━━━━━┛"]`(1.3); 본문 `[]` → `["╭────╮",
    "│ ○  │", "╰────╯"]` 패닉 없음(1.6)
  - Event `["Go", "«timer»"]` → 둘째 줄 run이 `dim && italic`, 첫 줄 run이 `bold`, 높이
    4(3.1); Subprocess·Round 같은 본문 → 같은 스타일(3.2, 3.4); 한 줄 본문 → 높이 3(3.3)
  - Subprocess "Sub" → `["╭─────╮", "│ Sub │", "╰─[+]─╯"]`(2.1); `min_width = 측정+4`
    → `[+]`가 늘어난 아래 줄 가운데(2.3)
  - Diamond `× 승인?`·`+ 병렬`·`○ 선택`·`* 복합`·`◎ 이벤트` → 가운데 줄에 라벨 원문,
    기호 run이 `!dim && !italic`(5.1); 다섯 기호·`○◎●`·`╱`의 코드포인트가 커버리지
    허용 목록(연구 실측 표) 안(5.2 — 상수 배열 대조 테스트)
- **Unit(L6, `layout::graph::tests`, 손으로 만든 `Graph` + `render()`)**:
  - 회귀(무수정 통과): `renders_top_down_chain_with_back_edge`(Round), `renders_left_right`,
    `crow_one_marker_is_a_single_glyph_not_doubled`, `crow_many_marker_opens_toward_the_node_it_touches`
    (`marker_glyphs` 직접), `left_right_label_does_not_overwrite_a_two_glyph_tail_marker`,
    레인 테스트 16개(`top_level_lanes_span_…` ~ `deep_nesting_empty_lane_…`), 파서 테스트
    `mermaid::flow::parses_shapes_and_links`, `mermaid::block::parses_columns_labels_shapes_and_spans`,
    `plantuml::class::parses_classes_and_relations`, `mermaid::class::parses_classes_members_and_relations`(6.1)
  - `marker_glyphs(Slash, TB/LR, true/false) == ['╱']`(4.1, 4.2)
  - TB·LR `Event(Start)`→`Round`→`Event(End)`: 화살촉이 각 노드 테두리 바로 바깥 줄,
    굵은 테두리 노드에도 선이 붙음(1.5)
  - TB `Subprocess`→`Rect`: `[+]` 줄이 출력 후에도 남아 있고 그 아래 줄에 선(2.2)
  - TB `a --Slash--> b`: a 아래 첫 줄 exit 열이 `╱`(4.1); LR: a 오른쪽 첫 칸이 `╱`(4.2);
    LR + label "no" + head Arrow: 라벨이 `╱` 다음 칸부터, 화살촉 `▶`(4.3)
  - 패닉 없음: 이름 없는 Event·한 글자·세 줄 본문·레인 안 Event/Subprocess·양방향·
    Slash 양끝(6.2)
  - 손으로 만든 주문 처리 그래프(레인 2개 — `add_lane` 2개 — 안에 시작/종료
    이벤트·`«user»`/`«service»` 태스크·`× 재고 있음?` 게이트웨이·`예`/`아니오`
    조건 라벨 흐름을 배치)를 LR 폭 100으로 `render()` → `Some`, 결과에
    `○`·`●`·`┃`·`«user»`·`«service»`·`× 재고 있음?`·`예`·`아니오`가 모두 있음(6.3)
- **Integration**: 없음(`ir` → `layout` 한 방향, 파서 없음)
- **Acceptance**: `cargo test` 전량(기준선 313개) + `cargo clippy --all-targets -- -D
  warnings` + `examples/*.md`를 수정 전후 `dg -P --width 100`으로 렌더링해 `diff`
  무차이(6.1 실물) + 6.3 테스트의 `{text}` 출력 육안 확인(태스크 `Round` 결정에
  폭·겹침 문제가 없음)

## File Structure Plan
```
src/diagram/ir.rs                 # EventPosition, Shape::{Event, Subprocess}, Marker::Slash (기존 파일 확장)
src/diagram/layout/shape.rs       # measure/draw 분기 + draw_runs 헬퍼 + 도형 테스트 (기존 파일 확장)
src/diagram/layout/graph.rs       # marker_glyphs Slash + render() 테스트 (기존 파일 확장)
```
