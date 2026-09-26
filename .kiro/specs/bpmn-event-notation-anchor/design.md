# Design — bpmn-event-notation-anchor

## 정의
BPMN 이벤트를 텍스트가 아니라 표기(원 글자)로 다루기 위해, 배치기가 `Shape::Event` 노드의 접점·정렬 기준을
"덩어리 가운데"에서 "원점 칸(= 원 글자)"으로 바꾸고, 그리기가 원 글자를 노드 원점에 고정하며, 종료 글자를
상태도 끝점과 같은 `◉`로 맞추는 수정이다. `bpmn-event-shape-notation`이 "받아들이는 한계"로 남긴 접점 어긋남
(a)(b)를 해소한다.

## 원인 (Root Cause)
- **접점 = 덩어리 가운데** — `src/diagram/layout/graph.rs` `spread()`(1278~1295행): `across_size <= 2`면
  `across`, 아니면 `across + across_size / 2`. 이름 있는 이벤트는 `measure()`가 `tw + 2 ≥ 3`을 주므로 접점이
  "원 글자 + 이름" 덩어리의 가운데 열(위→아래) 또는 가운데 줄(왼쪽→오른쪽)이 된다. 원 글자는
  `src/diagram/layout/shape.rs` `Event` 분기(71~78행)에서 덩어리 맨 앞 칸(`block_x`, `row0`)에 있으므로 둘의
  차이가 `across_size / 2` — 이름이 길수록 벌어진다(1.1·1.2·1.5). 왼쪽→오른쪽은 본문 두 줄까지 `across_size ≤ 2`
  예외에 걸려 우연히 맞고, 세 줄부터 어긋난다(1.4).
- **정렬 기준도 덩어리 가운데** — `LayoutNode::center()`(56~58행) `across + across_size / 2.0`: 무게중심 정렬
  (`reorder`)·교차 수·접점 순서 정렬이 모두 선이 닿지 않는 지점을 노드 위치로 쓴다.
- **다중 접점 벌림** — 노드 크기 계산(438~452행): 위→아래 `ports > 1 && w > 2`면 `across_size`를
  `(ports-1)*spacing+3`으로, 왼쪽→오른쪽 `h ≥ 3`이면 `2*ports+1`로 늘리고, `spread()`가 그 안에 접점을
  벌려 놓는다(1.3·1.4 후반). 늘어난 값이 `Layout::draw()`(1774~1778행)의 `min_width`/`min_height`로 전달되고
  `shape::draw()`의 `block_x = x + (w - measured_width)/2`·`row0 = y + extra_top`(48·71·72행)이 원 글자를 여분
  안에서 다시 가운데로 밀어 접점과 또 어긋난다(재현 7: 접점 3·5번 줄, 글자 3번 줄). 접점 하나인 노드는
  `across_size == measured`라 이 가운데 정렬 항이 0이다 — 형제 노드 정렬로 노드가 늘어나는 경로는 없다(늘림은
  자기 접점 수에서만 온다).
- **접점 밀기** — `resolve_port_swaps()`(1532~1541행): `across_size >= 4`인 실제 노드의 접점을 ±1칸 옮겨 X자
  교차를 푼다. 이벤트 `"Done"`(폭 6)도 대상이라 접점이 원 글자를 떠날 수 있다.
- **종료 글자** — `shape.rs` 76행 `EventPosition::End => '●'`. 같은 파일 53~54행의 상태도 관례는 `Start = ●`,
  `End = ◉`(1.6).

## 수정 방식
- **`ir::Shape` — 점 표기 술어(SSoT)**: `pub fn is_point_anchored(self) -> bool { matches!(self, Shape::Event(_)) }`
  — "접점·정렬 기준이 상자 가운데가 아니라 원점 칸(원 글자)인 모양". 상태도 `Start`/`End`는 크기 1×1이라
  이미 점처럼 동작하므로 목록에 넣지 않는다(넣으면 `center()`가 0.5 이동해 상태도 배치가 흔들릴 수 있다 — 3.1·3.2
  보호).
- **`layout::graph` — 네 곳, 모두 술어 분기**: `LayoutNode`에 `point_anchored: bool`(노드 생성 시
  `shape.is_point_anchored()`, 가상 노드 `false`).
  - `center()`: `point_anchored`면 `across as f64`(원점 칸).
  - `spread()`: 첫 줄에 `if node.point_anchored { return node.across; }` — 접점 수와 무관하게 모두 원점 칸으로
    모인다(상태도 `●`/`◉`에 간선 여럿이 닿는 것과 같은 결과, 2.3·2.4).
  - 크기 계산: `point_anchored`면 두 방향 모두 접점 벌림 없이 `(h, w)`/`(w, h)` 그대로 — 접점이 한 칸으로 모이므로
    벌릴 폭이 필요 없다.
  - `resolve_port_swaps()` 후보 조건에 `&& !point_anchored` — 원 글자를 떠나는 접점 이동 금지. 대신 남는 교차는
    기존 규칙대로 `◠` 건너뛰기로 그려진다.
  - `Layout::draw()`·통로·라벨·닻 규칙 무변경.
- **`layout::shape` — `Event` 분기**: `block_x`/`row0` 계산을 지우고 `x`/`y` 사용 — 배치기가 여분 폭·높이를 주더라도
  원 글자는 항상 노드 원점, 여분은 이름 뒤·아래로만 남는다(2.5; 술어와 짝을 이루는 그리기 쪽 약속). 글자 표
  `End => '◉'`(2.6). `measure()`·다른 분기 무변경.
- **새 출력**(위→아래 `Round "Task"` → `End "Done"`; 왼쪽→오른쪽 세 줄 본문; 도출값, 열·줄 일치가 단정 대상):
  ```
  ╭──────╮            ╭──────╮   ◉ one
  │ Task │            │ Task │──▶  two
  ╰──────╯            ╰──────╯     three
      │
      ▼
      ◉ Done
  ```
  `examples/bpmn.md` 첫 그림은 `──▶◉ 완료`로 글자만 바뀌고, 둘째 협업 그림은 `○ «message»`·`◎ 시한 초과`·
  `◉ 주문 취소`·`◉ 출고 완료`·`◉ 안내 완료`의 선이 모두 원 글자 열에서 드나든다.
- **기각한 대안**: (A) 측정을 대칭으로(`2*tw + 3`, 원 글자를 가운데 칸에) — 배치기 무변경이지만 노드 폭이 두 배가
  되고 왼쪽→오른쪽에서는 빈 줄까지 필요하다. (B) 그리기만 원점 고정(`block_x = x`, `row0 = y`) — 접점 하나인
  노드에서는 이미 `block_x == x`라 재현 3·4·6이 그대로 남는다(작업 지시의 가설을 재현으로 반증); 이 스펙에서는
  다중 접점 보조 원인을 막는 짝 수정으로만 채택. (C) 원 글자를 접점 열에 그리고 이름을 그 오른쪽으로 — 이름이
  노드 폭 밖으로 넘치거나 (A)와 같은 대칭 측정이 필요하다.

## 검증 속성
- (a) 결함 재현(수정 전 **실패**): `layout::graph::tests` — 위→아래 `Round → End "Done"`에서 `▼` 열 == `◉` 열
  (수정 전 4 vs 1, 1.1); `Start "Go" → Round`에서 `○` 아래 칸이 선(1.2); `A, B → End "Done"`에서 모든 `▼`가 `◉`
  바로 위(1.3); 왼쪽→오른쪽 세 줄 본문·접점 둘에서 모든 `▶`가 `◉` 바로 왼쪽(수정 전 1 vs 0, 1.4); 이름 `""`와
  `"주문 취소"`의 `◉`가 `▼` 열과 같음(1.5). `layout::shape::tests` — `min_width = measured + 4`,
  `min_height = measured + 2`로 그려도 원 글자가 `(0, 0)`(2.5의 그리기 몫); `End "Go"` → `["◉ Go"]`(1.6).
- (b) 기대 동작: 위 테스트 통과(2.1~2.6); 기존 두 접점 테스트를 "열 범위 안" → "같은 열", `●` → `◉`로 강화.
- (c) 불변 동작: 나머지 스위트 무수정 통과(3.1·3.3~3.5·3.8); `bpmn_glyph_codepoints…` 허용 목록에 `◉` 추가·
  이벤트 글자 목록 `['○','◎','◉']`(3.6, 근거 `bpmn-shapes/research.md` 실측표 U+25C9 2/2); `examples/*.md`를 수정
  전후 `dg -P --width 100`으로 렌더링해 `bpmn.md` 외 `diff` 무차이(3.2); `bpmn.md`는 이벤트 글자·선 위치만 다름;
  `bpmn::tests`·손 그래프 기대 글자 목록 `●` → `◉`(3.7).

## 영향 범위
- `src/diagram/ir.rs`: `Shape::is_point_anchored()` 추가, `Event` 주석 `○/◎/◉`.
- `src/diagram/layout/graph.rs`: `LayoutNode.point_anchored`, `center()`, `spread()`, 크기 계산, `resolve_port_swaps()`;
  테스트 `event_shapes_attach_edges_…_top_down`/`…_left_right` 강화 + 위 (a)의 새 테스트, `hand_built_…` 목록 `◉`.
- `src/diagram/layout/shape.rs`: `Event` 분기(원점 고정·`◉`·주석); 테스트 `event_start_intermediate_end_…`(`◉ Go`),
  `bpmn_glyph_codepoints…`, 새 원점 고정 테스트.
- `src/diagram/bpmn/mod.rs`: 테스트만 — 239행 주석, 458·537행 목록 `●` → `◉`.
- 변경 없음: `canvas.rs`, `bpmn/{model,validate,lower,parse_xml,vocabulary}.rs`(`●` 리터럴 없음 — grep 확인),
  `examples/`(파일은 그대로, 렌더링만 바뀜), README(BPMN 글자 표 없음).
- 이전 스펙 정정(`bpmn-message-flow-participant-endpoint` 선례대로 한 줄 포인터만): `bpmn-event-shape-notation/design.md`
  "받아들이는 한계 (a)(b)"·잔여 항목 → 이 스펙이 해소(그 스펙의 "도형이 방향을 모르는 현 계약 안에서는 풀 수
  없다"는 배치기 쪽 술어로 풀림); `bpmn-xml/requirements.md` 4.1·`bpmn-support/research.md` 138행·
  `bpmn-shapes/design.md` 95행의 `●` 종료 글자 → `◉` 포인터.
- 경계 확인: `bpmn-shapes` Allowed Dependencies(`ir` → `shape` ← `graph`) 그대로 — `graph`가 `ir::Shape` 술어를
  읽는 방향. `bpmn-model` Boundary "`ir`·`layout` 무변경"은 그 스펙의 자기 제약이라 충돌 없음. 술어를 `ir`에 두어
  "이벤트는 점 표기"라는 사실이 그리기(`shape`)와 배치(`graph`) 두 소비자에 한 곳에서 공급된다.
