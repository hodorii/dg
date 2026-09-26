# Design — bpmn-event-shape-notation

## 정의
BPMN 이벤트를 활동(둥근 상자)과 한눈에 가르기 위해, `Shape::Event`를 상자 없이 "원 글자 + 오른쪽 이름"으로
그리게 `layout::shape`의 측정·그리기 분기만 바꾸는 수정이다. 활동 모양(`Round`·`Subprocess`)은 사용자
확정대로 둥근 상자 그대로다.

## 원인 (Root Cause)
- `src/diagram/layout/shape.rs` `draw()`의 `Shape::Event(position)` 분기(66~78행): 테두리를
  `canvas.rect(x, y, w, h, kind, border, round)`로 그린다 — `Rect | Round | Circle | Note`·`Stadium`·
  `Subprocess`가 쓰는 것과 같은 원시 함수. `Canvas::rect()`(`canvas.rs` 192~209행)는 네 변을 `hline`/`vline`
  직선으로 긋고 `round`가 참이면 모서리 네 칸의 글자만 `╭╮╰╯`로 바꾼다(`line_char` 269~272행). 변이 휘거나
  좁아지는 일은 없으므로 `round` 값과 무관하게 실루엣은 늘 직사각형이다(1.1~1.3). `End`의
  `LineKind::Heavy`도 같은 직사각형의 선 굵기만 바꾼다(1.3).
- 같은 파일 `measure()` 32행 `(tw + 6, th.max(1) + 2)`: Round의 상자 기하(`tw+4, th+2`)에 위치 글자 두 칸을
  더한 것 — 이름 없는 이벤트도 3×6 상자가 된다(1.4).
- 배경: `bpmn-support/research.md` 매핑표(138행)와 `bpmn-shapes` 요구사항 1.1이 이벤트를 "둥근 상자 +
  테두리 안 왼쪽 위치 글자"로 정했다. `bpmn-shapes/research.md`는 "`Shape::Circle`은 `Round`와 같은
  글자로 그려진다"고 확인했으면서도 새 변형에 다시 상자 테두리를 줬다 — 사용자 지적대로 설계 결정의
  문제이며, 코드는 그 결정을 충실히 구현했다.

## 수정 방식
- **`layout::shape` — `Event` 분기 두 곳만** (`Canvas`·`layout::graph` 무변경):
  - `measure`: `Shape::Event(_) => (if tw == 0 { 1 } else { tw + 2 }, th.max(1))` — 위치 글자·공백·본문 폭,
    높이는 본문 줄 수(최소 1).
  - `draw`: `rect()` 호출 제거. `block_x = x + (w - measured_width) / 2`(배치기가 `min_width`로 늘렸을 때
    덩어리를 가운데에), `row0 = y + extra_top`. `put(block_x, row0, glyph, theme.diagram_accent)`(글자 표
    `○`/`◎`/`●` 그대로), 이어서 모든 섹션의 줄을 순서대로 `text(block_x + 2, row0 + k, line,
    line_style(0, line, theme))` — 구분선 없음, 왼쪽 맞춤(이름이 트리거 줄보다 좁아도 원 글자 옆에 붙어 있게).
    `draw_sections()`의 `x+1`/`w-2` 가운데 정렬 관례는 테두리 한 칸을 전제하므로 쓰지 않는다(`Plain` 분기의
    같은 이유).
- **`ir.rs`** `Shape::Event` 주석: "둥근 상자 …" → "원 글자(○/◎/●) + 오른쪽 이름, 테두리 없음".
- **새 출력**(`draw_rows`, 본문별):
  | 본문 | Start | End | 이전(Start) |
  |---|---|---|---|
  | `["Go"]` | `["○ Go"]` | `["● Go"]` | `["╭──────╮", "│ ○ Go │", "╰──────╯"]` |
  | `[]` | `["○"]` | `["●"]` | `["╭────╮", "│ ○  │", "╰────╯"]` |
  | `["Go", "«timer»"]` | `["○ Go", "  «timer»"]` | — | 네 줄 상자 |
- **위치 글자는 그대로 셋 다 필요** — 상자를 빼면 글자 자체가 도형이다. BPMN의 가는 원/이중 원/굵은
  원이 `○`/`◎`/`●`에 1:1로 대응하고(중간 이벤트의 이중선은 `LineKind::Double` 없이 이 글자가 유일한
  구분), 셋 다 `bpmn-shapes` 커버리지 실측 안(Noto Sans Mono CJK 2/2, 2026-09-27 `fc-list` 재확인). 종료의
  "굵은 테두리"는 채운 원 `●`로 대체 — 굵은 둥근 모서리가 유니코드에 없다는 `bpmn-shapes` 제약도 함께
  사라진다.
- **접점(bugfix 2.6)** — `layout::graph` 규칙 그대로 성립함을 코드로 확인: 간선은 노드 경계 상자
  `(w, h)` 바깥 칸에서 시작·끝난다(`segment_span`). 위→아래: 접점 열 = `spread()` 가운데 `across + w/2`
  → 덩어리 가운데 열, 행은 `y-1`/`y+h`. 왼쪽→오른쪽: `h < 3`이면 `across_size = h` 그대로고 `spread()`가
  `across_size ≤ 2`에서 `across`(0번 줄)를 돌려주므로 `▶`가 원 글자 바로 왼쪽 칸(`x-1`, `row0`)에 온다.
  받아들이는 한계: (a) 위→아래 화살촉은 원 글자 열이 아니라 덩어리 가운데 열(이름 위)에 온다,
  (b) 본문 세 줄 이상이면 왼쪽→오른쪽 접점 줄이 `h/2`라 원 글자 줄과 어긋난다 — `Interface`·`Actor`(아이콘
  + 이름, 테두리 없음)가 이미 같은 방식이며, 도형이 방향을 모르는 현 계약 안에서는 풀 수 없다(별도 배치
  스펙 몫).
- **bpmn-shapes 결정 수정(이 스펙이 명시적으로 뒤집는 것)**: 요구사항 1.1~1.3 "둥근 상자 안 …/테두리
  전체가 굵은 선" → 상자 없음·`●`; 1.4 "Round와 높이 같음" → 높이 = 본문 줄 수(최소 1; 근거였던 "위치 글자가
  줄을 추가하지 않는다"는 그대로 성립); 1.6 "한 줄 높이 상자" → `○` 1×1; design Key Decision "종료 이벤트는
  각진 굵은 모서리"와 Revalidation Trigger "굵은 둥근 모서리 지원 시 재검토"는 소멸; 기하 규칙 표의 Event
  네 행은 위 `수정 방식`이 대체. `bpmn-xml` 요구사항 4.1 "`○`/`◎`/`●` 이벤트 상자, 종료는 굵은 테두리"의
  "상자·굵은 테두리" 문구도 함께 소멸(도형 매핑은 그대로).
- **기각한 대안**: (A) 원 글자 위·이름 아래(`Interface` 배치) — 기본 방향(가로)에서 본문 두 줄 이하 노드의
  간선이 원 글자 줄의 상자 오른쪽 끝(이름 폭만큼 떨어진 칸)에서 시작해 원과 끊어져 보인다.
  (B) 손으로 그린 타원 테두리 ` ╭────╮`/`│ ○ Go │`/` ╰────╯` — 모서리 네 칸만 안으로 들어간 상자라
  "rect 빼고"를 만족하지 못하고, 종료는 굵은 둥근 모서리가 없어 `┏━━┓` 팔각형이 되며, Start/Intermediate
  구분에 여전히 안쪽 글자가 필요하다. (C) `Shape::Event`를 고친 `Shape::Circle`의 별칭으로 — `Circle`은
  mermaid `(( ))`·PlantUML `circle`이 소비하는 "이름을 안에 넣는" 모양이라 그 사용자의 기대를 바꾸고, 세
  위치를 구분할 수 없다.

## 검증 속성
- (a) 결함 재현: `layout::shape::tests` 세 테스트의 기대 리터럴을 새 출력으로 바꾸면 수정 전 코드에서
  **실패**(1.1~1.5); `layout::graph::tests` 가로 접점 테스트를 "`▶` 바로 오른쪽이 `●`"로 바꾸면 수정 전
  `▶` 오른쪽이 `┃`라 **실패**(1.6); 세로 접점 테스트에 "`▼`가 `●` 줄 바로 위 줄"을 더하면 수정 전엔 그
  줄이 `┏━━┓`라 **실패**.
- (b) 기대 동작: 위 테스트 통과(2.1~2.6); `draw_runs`로 `○ Go`/`  «timer»`의 스타일 — 글자 accent, 이름
  `bold`, 둘째 줄 `dim && italic`, 높이 2·한 줄 본문 높이 1(2.5).
- (c) 불변 동작: 기존 도형·배치·bpmn 테스트 전량 무수정 통과(3.1~3.5; 다만 3.7의 `┃` 기대만 목록에서
  뺀다 — 수정 전에도 통과); `examples/*.md`를 수정 전후 `dg -P --width 100`으로 렌더링해 `bpmn.md` 외
  `diff` 무차이(3.6), `bpmn.md`는 이벤트 노드 자리만 다름.

## 영향 범위
- `src/diagram/layout/shape.rs`: `measure`/`draw`의 `Event` 분기; 테스트
  `event_start_intermediate_end_render_with_position_glyphs`(리터럴 + 높이 단정 → "Round와 다르다"),
  `event_with_empty_body_has_one_line_height_and_does_not_panic`(`["○"]`),
  `event_stereotype_second_line_is_dim_and_italic`(높이 4→2, 3→1). `bpmn_glyph_codepoints…`·다른 도형
  테스트 무변경.
- `src/diagram/layout/graph.rs`: 테스트만 — `event_shapes_attach_edges_…_top_down`(종료 이벤트 단정을
  `●` 줄 기준으로), `…_left_right`(`┃` → `●`), `hand_built_order_processing_graph_…`(`┃` 기대 제거).
  배치 코드 무변경.
- `src/diagram/bpmn/mod.rs`: 테스트만 — 두 기대 목록(459·538행)에서 `┃` 제거, 237~242행 주석("종료
  이벤트의 굵은 테두리도 `┃`를 쓴다") 갱신.
- `src/diagram/ir.rs`: `Shape::Event` 주석.
- 변경 없음: `canvas.rs`, `layout/graph.rs` 본문, `bpmn/{model,validate,lower,parse_xml,vocabulary}.rs`
  (렌더링 문자열을 단정하는 테스트 없음 — grep으로 확인), `examples/`.
- 경계 확인: `bpmn-shapes` Allowed Dependencies(`ir` → `shape` ← `graph`) 그대로; `bpmn-model` Boundary
  "`ir`·`layout` 무변경"은 그 스펙이 소유하지 않는 영역에 대한 자기 제약이라 이 스펙과 충돌하지 않는다.
  뒤집는 `bpmn-shapes` 결정은 위 `수정 방식`에 열거 — 구현 시 `bpmn-shapes/design.md` Key Decisions와
  `bpmn-xml/requirements.md` 4.1, `bpmn-support/research.md` 매핑표 이벤트 행에 이 스펙 이름을 가리키는
  한 줄씩만 더한다(`bpmn-message-flow-participant-endpoint` 선례).
- 잔여(별도 스펙 후보): `Shape::Circle`이 `Round`와 같은 글자로 그려지는 같은 문제; 접점 한계 (a)(b).
