# Research & Design Decisions — bpmn-shapes

## Summary
- **Feature**: `bpmn-shapes`
- **Discovery Scope**: Extension(기존 `ir::Shape`/`ir::Marker` 어휘와 `layout::shape`·
  `layout::graph::marker_glyphs()` 확장) — 매핑표는 `../bpmn-support/research.md`
  §"BPMN 도형·표식 어휘"가 SSoT, 여기는 현재 코드로 재확인한 결과와 그 표에서
  벗어난 두 지점만 적는다.
- **Key Findings**:
  - `shape.rs::line_style()`은 도형과 무관하게 `«`로 시작하는 모든 본문 줄을 흐리고
    기울인다(`draw_sections()`·`Plain` 분기 둘 다 거침). 지금까지 호출자는 class·
    component 파서와 시퀀스 참여자뿐이었을 뿐, 새 도형에 신규 로직은 필요 없다 —
    스타일 단정 테스트만 없다(`shape.rs` 테스트 헬퍼가 `Line::plain()`으로 스타일을
    버린다).
  - `Shape::Circle`은 `Shape::Round`와 완전히 같은 글자로 그려진다(둘 다 `rect(...,
    round=true)`, `(tw+4, th+2)`) — 이벤트를 `Circle`로 대신할 수 없고, 새 변형이 맞다.
  - `Canvas::rect()`는 `LineKind::Heavy`를 지원하지만 `line_char()`가 `round`를
    `heavy`보다 먼저 보므로 굵은 선 + 둥근 모서리는 가는 `╭╮╰╯` 모서리에 굵은
    변이 붙는 어색한 모양이 된다(유니코드에 굵은 둥근 모서리가 없음).
  - `marker_glyphs()`의 단일 글자 표식(`Circle`·`Cross`·`DiamondFilled`)은 방향과
    무관하게 한 글자를 돌려주고, 배치기의 `gap_tail_rows`/LR 라벨 오프셋이 글자 수를
    자동으로 반영한다 — `Slash`도 같은 패턴으로 한 줄이면 끝난다.
  - 간선 선은 `segment_span()` 기준으로 노드 테두리 **바깥** 칸(`node_along +
    along_size`)에서 시작하고 `add_line()`은 글자 칸을 덮어쓰지 않으므로, 아래
    테두리에 놓은 `[+]`는 TB 출력 간선에 지워지지 않는다.
  - 기본 CJK 모노 폰트 커버리지 실측(아래) — `⬠`(U+2B20)·`∗`(U+2217)는 커버리지
    밖, `○◎●◉╱×+*`는 안.

## Research Log

### 스테레오타입 흐림·기울임의 범용성
- **Context**: 매핑표가 "`line_style()`이 이미 처리하므로 신규 로직 불필요"라고
  주장 — 검증 필요.
- **Sources Consulted**: `src/diagram/layout/shape.rs`(`line_style`, `draw_sections`,
  `Plain` 분기), `grep '«' src/`.
- **Findings**: `line_style(section, text, theme)`는 `text.starts_with('«')`만 보고
  `theme.diagram_text.dim().italic()`을 돌려준다. 호출 지점은 `draw_sections()`(Rect·
  Round·Circle·Note·Stadium·Diamond·Hexagon·Subroutine·Cylinder·Actor·Interface 공용)와
  `Plain` 분기 — 도형 분기별 예외 없음. 생산자는 `mermaid::class`(`«interface»`),
  `plantuml::class`/`component`(`<<x>>`→`«x»`), `layout::sequence`(참여자 종류)뿐.
- **Implications**: 이벤트·서브프로세스가 `draw_sections()`를 거치기만 하면 3.1~3.3은
  공짜. 필요한 건 스타일까지 보는 테스트 헬퍼(`Line::runs()` 사용) 하나.

### 이벤트 도형의 기하와 굵은 테두리
- **Context**: "둥근 상자 + 테두리 안 왼쪽 위치 글자 + 종료는 Heavy"를 현재
  `Canvas`로 그릴 수 있는지.
- **Findings**: `rect(x,y,w,h,Heavy,style,round)` 가능. 모서리는 `round`가 우선해
  `╭━━╮`처럼 섞인다. `round=false`면 `┏━━┓ ┃ ┃ ┗━━┛`로 일관됨. 위치 글자는
  `put()`으로 본문 첫 줄 왼쪽 칸에 놓고 본문은 `draw_sections()`를 오른쪽으로 두 칸
  밀어 호출하면 된다(`draw_sections`의 `x+1`/`w-2` 가운데 정렬 관례 유지).
- **Implications**: design §Key Decisions "종료 이벤트는 각진 굵은 모서리".

### 글자 커버리지 실측(`fc-list ":charset=U+XXXX:family=…"`, 2026-09-25, 이 머신)
- **Context**: `../bpmn-support/research.md` Risks의 `⬠` 폰트 우려 + `diagram-crow-foot-
  orientation`의 "커버리지 밖 글자 → 폰트 폴백 → 스크롤 지연" 전례.
- **Findings**(2 = 커버, 0 = 미커버):

  | 글자 | 코드 | Noto Sans Mono CJK KR(`fc-match monospace`) | D2Coding ligature | DejaVu Sans Mono |
  |---|---|---|---|---|
  | `⬠` 이벤트기반 게이트웨이 | U+2B20 | 0 | 0 | 0 |
  | `∗` 복합 게이트웨이 | U+2217 | 0 | 2 | 4 |
  | `◎` | U+25CE | 2 | 2 | 4 |
  | `○` `●` `◉` | U+25CB/CF/C9 | 2 | 2 | 4 |
  | `╱` | U+2571 | 2 | 2 | 4 |
  | `×` `*` `+` | U+00D7/2A/2B | 2 | 2 | 4 |
  | `⟨` `⟩`(기존 Diamond 옆면) | U+27E8/E9 | 0 | 2 | 4 |

  `⬠`는 설치된 어떤 모노 폰트에도 없다(DejaVu Sans·Noto Sans Symbols2 같은 비모노
  폰트로만 폴백). `∗`도 기본 CJK 모노 폰트에는 없다.
- **Implications**: 매핑표에서 두 글자 교체(§Design Decisions). 기존 `Diamond`의
  `⟨⟩`가 같은 문제를 이미 안고 있음은 이 스펙 범위 밖(Risks에 기록).

### `[+]`와 간선 접점
- **Context**: TB에서 서브프로세스 아래 가운데로 간선이 나가면 `[+]`와 겹치는지.
- **Findings**: `segment_span()`의 `top`은 `node_along + along_size`(테두리 다음 칸).
  `draw_segment()`는 `top..channel`만 긋고 테두리 칸에는 아무것도 쓰지 않는다.
  `Canvas::add_line()`은 `is_text()` 칸을 건너뛴다. 표식(`▼` 등)은 도착 쪽 `bottom`
  칸에만 놓인다.
- **Implications**: 요구사항 2.2는 추가 로직 없이 성립 — 테스트로만 못박는다.

## Architecture Pattern Evaluation
`../bpmn-support/research.md` §Architecture Pattern Evaluation 참고(기존 어휘 재사용
최대화 채택). 이 스펙 안의 선택지는 아래 결정 두 건뿐.

## Design Decisions

### Decision: 게이트웨이 기호 두 개 교체 — `⬠`→`◎`, `∗`→`*`
- **Context**: 실측 결과 두 글자가 기본 CJK 모노 폰트 커버리지 밖.
- **Alternatives Considered**:
  1. `⬠` 유지 + ASCII `E` 대체를 병기하고 실측 후 결정(매핑표 원안)
  2. 이벤트기반 = `◎`(BPMN 원 기호의 바깥 이중원과 같은 모양, 이미 중간 이벤트에
     쓰는 글자라 커버리지 검증됨), 복합 = ASCII `*`
- **Selected Approach**: 2. 실측이 끝났으므로 병기·보류할 이유가 없다. `◎`는 마름모
  안(게이트웨이)과 둥근 상자 안(이벤트)이라는 문맥이 달라 혼동되지 않는다.
- **Rationale**: `diagram-crow-foot-orientation`이 세운 규칙("커버리지 밖 글자 대신
  이미 쓰는 글자 재사용")을 그대로 따른다.
- **Trade-offs**: `E`보다 `◎`가 BPMN 원 도형에 가깝고 한 글자 폭이 같다. `*`는 `∗`보다
  낮게 찍히지만 모노 폰트 폴백이 없다.

### Decision: 게이트웨이는 코드 변경 없는 라벨 관례
- **Context**: `Shape::Diamond`가 이미 임의 라벨을 그리므로 새 변형·함수가 필요한지.
- **Alternatives Considered**:
  1. `bpmn/glyphs.rs`에 `GatewayKind → 기호` 표를 이 스펙에서 신설(로드맵 원안)
  2. 기호 표는 design.md의 "`lower()`가 만들어야 할 라벨" 계약으로만 두고, 코드
     표는 `GatewayKind` 등 어휘 enum을 소유하는 `bpmn-model`에서 만든다
- **Selected Approach**: 2. 표의 키(`GatewayKind`·`TaskKind`·`EventTrigger`)가 전부
  `bpmn::Model` 어휘라 이 스펙이 만들면 `bpmn-model`의 소유를 선점한다(설계 규칙:
  하류 전제를 상류에 심지 않음). 이 스펙은 다섯 라벨이 마름모 안에 그대로 그려짐과
  글자 커버리지만 테스트로 고정한다(요구사항 5.1·5.2).
- **Trade-offs**: 로드맵의 "`bpmn/glyphs.rs` 표" 문구가 `bpmn-model`로 넘어간다 —
  로드맵 갱신은 이 스펙 완료 시 한 줄.

### Decision: `Shape::Event(EventPosition)` 하나 + `Subprocess` 하나, 나머지는 재사용
- **Context**: 이벤트 세 종류를 변형 셋으로 펼칠지, 태스크에 새 변형을 둘지.
- **Selected Approach**: `Event(EventPosition { Start, Intermediate, End })` 한 변형 —
  세 위치가 글자·테두리만 다르고 기하가 같다. 태스크·호출 활동·데이터 객체·주석·
  저장소는 기존 `Round`/`Rect`/`Note`/`Cylinder` + `«…»` 둘째 줄 그대로.
- **Rationale**: `Shape::Start`/`Shape::End`(상태도 점) 이름과 부딪히지 않게 위치를
  별도 enum으로 묶는다. 이름은 기존 `ParticipantKind`/`GroupKind`처럼 "무엇의 어떤
  속성"인지 드러내는 `EventPosition`.

## Risks & Mitigations
- 기존 `Shape::Diamond` 옆면 `⟨⟩`(U+27E8/E9)가 Noto Sans Mono CJK KR에 없다 —
  게이트웨이가 마름모를 쓰므로 BPMN 그림에 폰트 폴백이 생길 수 있다. 이 스펙 밖
  (`c7785c8`에서 도입된 기존 동작) → 별도 bugfix 스펙 후보로 기록만 한다.
- 굵은 테두리 이벤트가 레인 테두리(가는 선)와 한 칸을 공유하는 경우는 없다(노드는
  그룹 안쪽 pad 2 안에 놓임) — 회귀 테스트 `node_outside_a_lane_is_drawn_outside_the_lane_box`
  등 레인 테스트 무수정 통과로 확인.
- `[+]`가 TB 다중 출력 포트와 겹칠 가능성: 포트는 테두리 다음 칸에서 시작하므로
  겹치지 않음(위 Research Log). 단 `[+]` 바로 아래로 선이 내려가면 `+`와 `│`가
  이어져 보이는데, BPMN에서도 `[+]`는 아래 가운데라 허용.

## References
- `../bpmn-support/research.md` §"BPMN 도형·표식 어휘" — 매핑표 원본
- `../diagram-crow-foot-orientation/design.md` — 커버리지 밖 글자 회피 규칙
- `../plantuml-wbs/design.md` — `Shape::Plain` 추가 선례(측정·그리기 분기 + `draw_rows` 테스트)
- OMG BPMN 2.0 §10.5(Events), §10.5.4(Gateways), §10.2(Sub-Process marker), §10.4(Default flow)
