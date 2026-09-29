# Bugfix — diagram-diamond-side-glyph-coverage

## 정의
기본 CJK 모노 폰트 터미널에서 마름모(판단·게이트웨이) 도형이 폰트 폴백 없이 그려지게 하기
위해, 마름모 옆면 글자 `⟨`(U+27E8)·`⟩`(U+27E9)를 폰트 커버리지 안의 글자로 바꾸는 결함
수정이다.

## 재현 절차
1. 환경: 이 머신(2026-09-29), `fc-match monospace` → `NotoSansCJK-Regular.ttc: "Noto Sans
   Mono CJK JP"`(KR/JP/SC/TC/HK 다섯 지역 변형이 같은 파일이며 커버리지 동일). `dg` 릴리스
   빌드(`cargo build --release`), `./target/release/dg -P -s none -d <파일>`로 렌더링.
2. 커버리지 실측(`fc-list "<family>:charset=<hex>" | wc -l`, 0 = 미커버):
   ```
   U+27e8 ⟨  Noto Sans Mono CJK KR=0  Noto Sans Mono CJK JP=0  D2Coding=2  DejaVu Sans Mono=4
   U+27e9 ⟩  Noto Sans Mono CJK KR=0  Noto Sans Mono CJK JP=0  D2Coding=2  DejaVu Sans Mono=4
   ```
   `fc-list ':charset=27e8:spacing=100' family` → `DejaVu Sans Mono`, `FreeMono` 두 모노
   폰트에만 있다. 같은 도형이 함께 쓰는 빗금 `╱`(U+2571)·세로선 `│`(U+2502)는 Noto CJK에
   있다(각 2). `bpmn-shapes/research.md`의 2026-09-25 실측과 결과가 같다.
3. 입력 A(mermaid flowchart, 판단 `{}`와 예비 단계 `{{}}`를 나란히):
   ```
   flowchart TD
     A[start] --> B{ok?}
     B --> C{{prep}}
   ```
   관찰 결과 A(관련 부분):
   ```
    ╱─────╲
   ⟨  ok?  ⟩
    ╲─────╱
    ╱──────╲
   │  prep  │
    ╲──────╱
   ```
   출력 전체의 비ASCII 코드포인트 집합: `U+00B7 U+2500 U+2502 U+250C U+2510 U+2514 U+2518
   U+256E U+2570 U+2571 U+2572 U+25BC U+25C8 U+27E8 U+27E9` — 이 중 Noto Sans Mono CJK
   커버리지 밖인 것은 `U+27E8`·`U+27E9` 둘뿐이다(`grep -o '⟨' | wc -l` = 1, `⟩` = 1).
4. 입력 B(가로 배치): `flowchart LR / A[start] --> B{ok?} / B --> C[end]` → 
   `│ start │──▶⟨  ok?  ⟩──▶│ end │`. 간선 표식 `▶`이 옆면 글자 `⟨` 바로 왼쪽 칸에 닿는다.
5. 마름모를 공유하는 다른 문법에서도 같은 글자가 나온다(각각 `⟨` 1개·`⟩` 1개 확인):
   - mermaid `stateDiagram-v2`의 `state c <<choice>>`
   - mermaid `block-beta`의 `A{"ok?"}`
   - PlantUML 클래스 다이어그램의 `diamond d1`
   - BPMN XML `exclusiveGateway`(`examples/bpmn.md`를 `-w 140`으로 렌더링한 24~26·63행:
     `⟨  × 재고 있음?  ⟩`, `⟨  승인 여부  ⟩`)
6. 대조: 육각형(`{{}}`, PlantUML component `hexagon`)은 옆면이 `│`라 이 결함과 무관하다.
   `examples/architecture.txt`(README가 링크하는 골든 렌더링, `make examples`로 생성)의
   138·151·170행에 판단 노드 세 개가 `⟨ … ⟩`로 들어 있다 — 이 파일에만 저장소 안 `⟨⟩`가
   남아 있다(`grep -rl '⟨' src examples` → `src/diagram/layout/shape.rs`, 
   `examples/architecture.txt`).
7. 전례: `src/diagram/layout/graph.rs` `marker_glyphs()` 주석과 `diagram-crow-foot-orientation`
   스펙이 "기본 Noto Sans Mono CJK 커버리지 밖 글자(`⋎⋏≻≺`) → 터미널이 줄마다 폰트 폴백
   셰이핑 → 다이어그램이 화면에 있을 때 스크롤 지연"을 실측·수정한 기록. 이번 글자도 같은
   경로다(스크롤 지연 자체의 재측정은 이 세션에서 하지 않았다 — 커버리지 부재만 실측).

## Boundary Context
- **In scope**: `src/diagram/layout/shape.rs` `draw()`의 `Shape::Diamond | Shape::Hexagon`
  분기에서 `Shape::Diamond`가 쓰는 옆면 글자 두 개(`⟨`/`⟩`)와, 그 글자를 그대로 적어 둔
  테스트·골든 파일(`examples/architecture.txt`)의 갱신.
- **Out of scope**: 마름모의 크기 공식(`measure()`의 `(tw+6, th+2)`)·모서리 빗금·본문 배치,
  `Shape::Hexagon`의 옆면 `│`, 게이트웨이 기호(`×+○*◎`, `bpmn-shapes` 완료), 간선 표식
  글자(`marker_glyphs()`), 파서가 어떤 문법을 `Shape::Diamond`로 넘기는지, 게이트웨이 크기
  고정·이름 바깥 배치(로드맵의 `bpmn-gateway-fixed-size`).

## Behaviors

### 1. 현재 동작 (결함)
- 1.1: [`Shape::Diamond` 노드 렌더링] → 본문 줄마다 왼쪽 옆면 칸에 `⟨`(U+27E8), 오른쪽
  옆면 칸에 `⟩`(U+27E9)가 찍힌다 — 둘 다 `fc-match monospace`가 고르는 Noto Sans Mono
  CJK 커버리지 밖이다(`fc-list … charset=27e8` = 0).
- 1.2: [mermaid `flowchart` `{}` + `stateDiagram` `<<choice>>` + `block-beta` `{}` + PlantUML
  class `diamond` + BPMN 게이트웨이] → 문법을 가리지 않고 같은 두 글자가 나온다(도형을
  공유하므로 결함도 공유).
- 1.3: [`examples/architecture.txt` 골든 파일] → 138·151·170행에 `⟨`/`⟩`가 저장돼 있다.

### 2. 기대 동작
- 2.1: [`Shape::Diamond` 노드 렌더링] → 옆면 두 칸의 글자가 Noto Sans Mono CJK KR
  커버리지 안(`fc-list` ≥ 1)이고, 터미널 한 칸(unicode-width 1, 동아시아 폭 W/F 아님)을
  차지하며, 출력 전체에 U+27E8·U+27E9가 나오지 않는다.
- 2.2: [1.2와 같은 다섯 문법] → 전부 2.1의 새 글자로 그려진다(한 곳만 고쳐 전부 반영).
- 2.3: [`examples/architecture.txt`] → 새 글자로 재생성되며, 판단 노드 세 개의 옆면 글자
  여섯 칸 외에 이번 수정으로 달라지는 바이트가 없다(`git diff`로 확인).

### 3. 불변 동작 (회귀 방지)
- 3.1: [`Shape::Diamond`와 `Shape::Hexagon`을 같은 라벨로 렌더링] → 옆면 글자가 서로
  다르다(`c7785c8`가 고친 "판단과 예비 단계가 같은 모양" 결함이 되돌아오지 않음, 기존
  테스트 `diamond_and_hexagon_render_differently` 유지 — 마름모 쪽 기대 문자열만 새 글자로
  갱신).
- 3.2: [`Shape::Hexagon` 렌더링] → 옆면 `│`, 빗금 모서리, 크기 그대로(바이트 동일).
- 3.3: [`Shape::Diamond` 렌더링의 크기·모서리·본문] → `measure()` 결과 `(tw+6, th+2)`,
  모서리 `╱╲`, 본문 가운데 정렬은 그대로 — 옆면 두 칸의 글자만 바뀐다.
- 3.4: [BPMN 게이트웨이 다섯 라벨(`× 승인?`, `+ 병렬`, `○ 선택`, `* 복합`, `◎ 이벤트`)] →
  라벨 원문 그대로·흐려지지 않게 그려진다(기존 테스트
  `gateway_symbol_labels_render_verbatim_and_symbols_are_not_dimmed` 무수정 통과).
- 3.5: [BPMN 게이트웨이·이벤트 글자 허용 목록] → 기존 테스트
  `bpmn_glyph_codepoints_are_within_cjk_mono_coverage` 무수정 통과.
- 3.6: [간선 표식 글자(`▲▼◀▶`, `∧∨<>`, `△▽◁▷`, `╪╫`, `○`, `×`)] → `marker_glyphs()`는
  건드리지 않으며 LR 배치에서 표식이 마름모 옆면에 닿을 때도 그대로 찍힌다.
- 3.7: [마름모가 없는 다이어그램(`examples/architecture.txt`의 나머지 부분 포함)] → 렌더링
  결과 바이트 동일.
