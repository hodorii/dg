# Design — diagram-diamond-side-glyph-coverage

## 정의
마름모 옆면 글자를 기본 CJK 모노 폰트 커버리지 안의 1칸 글자 `‹›`로 바꿔 폰트 폴백을 없애는
결함 수정이다.

## 원인 (Root Cause)
`src/diagram/layout/shape.rs` `draw()`의 `Shape::Diamond | Shape::Hexagon` 분기가 옆면 글자를
`if shape == Shape::Hexagon { ('│', '│') } else { ('⟨', '⟩') }`로 고른다. `⟨`(U+27E8)·`⟩`(U+27E9)는
`c7785c8`이 "육각형과 구분되는 더 각진 꺾쇠"로 택했을 뿐, 이 저장소가 `marker_glyphs()`에서
세운 "기본 Noto Sans Mono CJK 커버리지 안 글자만" 규칙에 대조되지 않았다(bugfix 1.1). 도형을
mermaid flowchart/state/block·PlantUML class `diamond`·BPMN 게이트웨이가 공유하므로 한 곳의
글자 선택이 다섯 문법에 그대로 드러난다(1.2). 골든 파일은 그 출력을 저장한 것이다(1.3).

## 수정 방식
같은 줄의 글자 한 쌍만 `('‹', '›')`(U+2039/U+203A)로 바꾼다. 크기 공식·모서리·본문 배치·
`Hexagon` 분기는 건드리지 않는다. 주석의 "꺾쇠" 설명은 유지하고 커버리지 근거 한 줄을 덧붙인다.
- **선정 근거**(실측 표·절차는 `research.md`): Noto Sans Mono CJK KR/JP 커버(`fc-list` 2),
  EAW N, `unicode-width` 0.2.2 `width`·`width_cjk` 모두 1 → `measure()`가 옆면에 배정한 1칸과
  실제 렌더 폭이 같다. 간선 표식 글자(`marker_glyphs()`)에 쓰이지 않는다.
- **기각 — 2칸 글자 `〈〉`(U+3008/9, W)·`＜＞`(U+FF1C/E, F)**: 커버리지는 있지만 `width` 2.
  `Canvas::put()`이 왼쪽 옆면에서 본문 첫 칸을 연속 칸으로 덧씌우고, 오른쪽 옆면(`x+w-1`)은
  `x+w > width`라 아무것도 찍지 않아 글자가 사라진다. **코드포인트 커버리지만 보고 동아시아
  폭을 안 보면 걸리는 함정** — 이후 글자 선정은 커버리지와 폭을 반드시 함께 본다.
- **기각 — `<>`(Na)**: `Marker::OpenArrow`와 ER 까치발의 LR 글자와 동일. LR 배치에서 표식이
  옆면 바로 옆 칸에 놓이므로 `──>>  ok?  >`처럼 표식과 옆면이 같은 글자로 붙는다.
- **기각 — `◁▷`(A)**: `Marker::Triangle`의 LR 글자와 동일(같은 충돌). 덧붙여 `width_cjk` 2 —
  단, A 등급은 이 프로젝트가 `│─╱╲○◎×`에서 이미 감수 중이라 단독 탈락 사유는 아니다.
- **기각 — `│`(육각형과 동일)**: `c7785c8`이 고친 결함으로 회귀(bugfix 3.1).

## 검증 속성
- (a) 결함 재현: `Shape::Diamond` 한 노드를 그려 옆면 두 칸이 `‹`/`›`인지, 그리고 렌더 결과에
  U+27E8/U+27E9가 없는지 확인하는 테스트 — 수정 전 코드에서 **실패**(1.1, 1.2).
- (b) 기대 동작: 같은 테스트가 수정 후 통과(2.1, 2.2). 커버리지 허용 목록 테스트
  `bpmn_glyph_codepoints_are_within_cjk_mono_coverage` 곁에 마름모 옆면 글자용 허용 목록
  (`‹›`)과 제외 목록(`⟨⟩`)을 같은 형식으로 추가해 재발을 막는다. `examples/architecture.txt`를
  `make examples`로 재생성해 판단 노드 세 개의 여섯 칸이 바뀌었는지 `git diff`로 확인(2.3).
- (c) 불변 동작: `diamond_and_hexagon_render_differently`는 마름모 기대 문자열의 옆면 두 글자만
  `‹›`로 갱신하고 `assert_ne!`·육각형 문자열은 그대로 — 수정 전(현재 글자 기준)·후 모두 통과
  (3.1~3.3). `gateway_symbol_labels_render_verbatim_and_symbols_are_not_dimmed`·
  `bpmn_glyph_codepoints_are_within_cjk_mono_coverage`·`marker_glyphs()` 관련 테스트·파서의
  `Shape::Diamond` 판별 테스트(`flow.rs`·`state.rs`·`block.rs`·`class.rs`·`lower.rs`)는 무수정
  통과(3.4~3.6). 마름모 없는 예제 출력 바이트 동일(3.7).

## 영향 범위
- `src/diagram/layout/shape.rs`: `draw()` 한 줄(글자 쌍)·주석 한 줄, 테스트 한 건 갱신·한 건 추가.
- `examples/architecture.txt`: `make examples` 재생성. 주의 — 현재 HEAD에서 이 파일은 이미
  `eade414`(architecture.md에 서브그래프 절 추가)와 어긋나 있어 재생성 diff에 그 절이 함께
  들어온다. 이번 스펙 몫은 138·151·170행 여섯 칸이며, 나머지 diff는 별개 사유로 구분해 기록한다.
- 다른 파일 없음. 파서(`mermaid/*`, `plantuml/*`, `bpmn/lower.rs`)는 `Shape::Diamond`를 넘기기만
  하고 글자를 모른다. `bpmn-shapes`의 Boundary(게이트웨이 = 코드 변경 없는 라벨 관례,
  기호 `×+○*◎`)와 `bpmn-event-*`의 이벤트 글자는 건드리지 않는다.
