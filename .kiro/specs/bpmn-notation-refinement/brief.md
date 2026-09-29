# Brief: bpmn-notation-refinement

## Problem
`bpmn-support` 로드맵(6개 스펙, 전부 완료·커밋됨)이 만든 BPMN 표기 어휘를 bpmn.io의
실제 시각 관례(크기 고정, 텍스트 배치 위치, 게이트웨이 기호 구분)와 다시 맞춰보려는
사용자를 위한 개선안이다. 사용자 요청 원문: "bpmn notation 개선 안(bpmn.io 의 노테이션
스타일: 크기고정, 텍스트 입력위치 등 참고) gateway ×+○* 등 표시에 의한 구분, 다이아몬드
형상, 이벤트(○◉⊗◎ 상태 ✉️✉️➔⏰▲⚡) activity(task) notation ...".

## Current State
현재 코드(직접 확인)와 과거 스펙이 이미 남긴 관련 기록:
- **게이트웨이 기호**(`bpmn/vocabulary.rs` `GATEWAY_KIND_TABLE`, `bpmn-shapes` 완료):
  Exclusive `×`, Parallel `+`, Inclusive `○`, Complex `*`, EventBased `◎` — 사용자가 요청한
  "×+○*" 구분은 이미 구현돼 있다. 다만 `Shape::Diamond`(아래) 자체의 기하가 bpmn.io와
  다르다.
- **다이아몬드 형상**(`layout/shape.rs::draw`, `Shape::Diamond|Hexagon` 분기): 옆면 글자가
  `⟨`/`⟩`(U+27E8/27E9)인데, `bpmn-shapes/research.md`가 2026-09-25에 이미 실측·기록해
  둔 대로 **Noto Sans Mono CJK KR에 커버리지가 없다**(이 세션에서 `fc-list`로 재확인,
  D2Coding·DejaVu Sans Mono엔 있음) — 폰트 폴백 위험이 있는 채로 방치돼 있다
  (`bpmn-shapes/research.md` Risks: "이 스펙 밖(`c7785c8`에서 도입된 기존 동작) → 별도
  bugfix 스펙 후보로 기록만 한다", 아직 스펙을 안 엶). 크기도 `(tw+6, th+2)`로 라벨
  길이에 맞춰 자라난다 — bpmn.io의 "작은 고정 크기 마름모 + 기호만 안, 이름은 밖"과
  다르다.
- **이벤트**(`layout/shape.rs::draw` `Shape::Event`, `bpmn-event-notation-anchor` 완료):
  `○`(시작)/`◎`(중간)/`◉`(종료) 원 글자가 이미 "크기 고정 + 이름은 오른쪽 바깥"으로
  그려진다 — bpmn.io 관례와 기하상 이미 가장 가깝다. 다만 이벤트 트리거(message·timer·
  error 등 13종)는 아이콘이 아니라 `«message»`/`«timer»`류 스테레오타입 텍스트 둘째
  줄이다(`bpmn-support/research.md`: "아이콘 글자(✉◷⚡)는 폰트 커버리지 문제로 v1
  제외"). 사용자가 이번에 다시 요청한 아이콘 집합(✉️ ✉️➔ ⏰ ▲ ⚡)을 이 세션에서 `fc-list`로
  재실측한 결과:

  | 글자 | 코드 | Noto Sans Mono CJK KR |
  |---|---|---|
  | ✉️ | U+2709 | 0 (미커버) |
  | ⏰ | U+23F0 | 0 (D2Coding·DejaVu에도 없음 — 완전 이모지 전용) |
  | ⚡ | U+26A1 | 0 (미커버) |
  | ▲ | U+25B2 | 2 (커버) |
  | ⊗ | U+2297 | 2 (커버) |

  즉 요청 집합 중 `▲`·`⊗`만 이 프로젝트의 "커버리지 밖 글자 금지" 규칙을 통과한다 —
  나머지는 과거 `✉◷⚡` 기각과 같은 사유로 다시 기각될 후보다.
- **태스크/액티비티**(`Shape::Round` + `«user»`류 둘째 줄): 라벨은 상자 가운데, 종류는
  아이콘이 아니라 스테레오타입 텍스트 — bpmn.io는 아이콘을 상자 왼쪽 위 모서리에 작게
  찍는다. 문자 격자에서 "작은 아이콘 + 본문 겹치지 않기"를 표현할 방법이 마땅치 않다
  (한 글자를 모서리에 겹쳐 찍으면 테두리 글자와 자리다툼).

## Desired Outcome
BPMN 표기가 bpmn.io 스타일에 더 가까워지되, 이 프로젝트의 고정 제약(4-의존성, CJK 모노
폰트 커버리지 안의 글자만, 패닉 금지)을 하나도 깨지 않는 상태 — 특히 이미 알려진 채
방치된 `⟨⟩` 폰트 위험이 해소되고, 크기 고정·텍스트 위치 정책이 이벤트뿐 아니라
게이트웨이에도 일관되게 적용되는지(또는 적용 안 하기로 결정하는지) 명시적 결론이 남는다.

## Approach
네 갈래로 쪼갠다 — 하나는 이미 원인·제약이 다 밝혀진 채 열리지 않은 버그픽스라 바로
착수 가능하고, 나머지 셋은 시각 정책 자체를 사용자가 정해야 하는 설계 트랙이라 requirements
단계에서 실제 비교(before/after 렌더링)를 먼저 보여준 뒤 결정해야 한다(자세한 분해·순서는
roadmap.md).

## Scope
- **In**: `Shape::Diamond`(`⟨⟩`) 폰트 대체(mermaid·PlantUML·BPMN 게이트웨이 공유 도형이라
  BPMN 전용이 아님), 게이트웨이 크기 고정 여부 재검토, 이벤트 트리거 아이콘화(커버리지
  통과분만) 여부 재검토, 태스크/액티비티 아이콘 배치 재검토
- **Out**: 이미 완료된 `bpmn-support` 6개 스펙의 재작업(게이트웨이 기호 자체는 이미 구현
  됨 — 이번 스코프는 "기호"가 아니라 "기하·아이콘화" 문제), mermaid/PlantUML BPMN 네이티브
  문법 변경, 색상·테마 정책

## Boundary Candidates
- `Shape::Diamond`는 BPMN 전용이 아니라 mermaid `flowchart`의 판단 노드(`{}`)·PlantUML
  등 다른 다이어그램 종류와 공유된다 — 옆면 글자를 바꾸면 그 전부가 영향권. 회귀 기준은
  `Shape::Hexagon`과 시각적으로 계속 구분돼야 한다는 것(`c7785c8`가 원래 고친 문제를
  되돌리면 안 됨)
- 이벤트 아이콘화·게이트웨이 크기 고정은 `layout::shape`(그리기)와 `layout::graph`
  (배치·접점 기준, `Shape::is_point_anchored` 확장 여지)에 걸친다 — 이벤트가 이미 이
  패턴(점 고정+바깥 텍스트)을 쓰고 있으므로 게이트웨이에 확장할지가 핵심 설계 질문

## Out of Boundary
- 태스크 아이콘의 구체적 모양 확정(격자 문자로 표현 가능한 후보가 마땅치 않아, 이 트랙의
  결론이 "현행 유지"일 가능성이 높다 — requirements 단계에서 후보를 실제로 그려 보고
  판단)

## Upstream / Downstream
- **Upstream**: `bpmn-support`(완료) — 이 개선안이 다루는 어휘·기하 전부 그 로드맵이
  만든 것
- **Downstream**: 없음(현재 계획 없음)

## Existing Spec Touchpoints
- **Extends**: `bpmn-shapes`(게이트웨이·이벤트 도형 원안), `bpmn-event-notation-anchor`
  (점 고정+바깥 텍스트 패턴의 최초 구현체 — 게이트웨이로 확장 시 그대로 참고)
- **Adjacent**: `diagram-crow-foot-orientation`(글자 커버리지 밖 회피 규칙의 선례),
  `bpmn-support`(글자 표 SSoT 원칙)

## Constraints
- 의존성 4개·단일 정적 바이너리 유지
- 새 글자는 반드시 `fc-list ":charset=U+XXXX:family=Noto Sans Mono CJK KR"`로 커버리지
  실측 후 채택(이 저장소의 확립된 관례 — 커버리지 밖 글자는 폰트 폴백으로 스크롤 지연을
  일으킨 전례가 있음)
- 레인 없는·게이트웨이 없는 기존 다이어그램(mermaid·PlantUML·BPMN 전부)의 렌더링 결과가
  이번 트랙으로 바뀌면 안 됨(바이트 동일 회귀 기준)
