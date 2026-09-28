# Research & Design Decisions — bizprocess-bpmn

## Summary
- **Feature**: `bizprocess-bpmn`
- **Discovery Scope**: Extension — `bpmn-model`의 `Model`·`render_model` 위에 XML·YAML과 나란한 세 번째 파서 한 겹
  + 드릴다운(중첩)을 위한 `Model`·`lower`·`ir` 소폭 확장. 태그 문법·드릴다운 매핑·기본 정책의 SSoT는
  `../bpmn-support/research.md` §Design Decisions("`participant:` 태그", "L1~L5 → Sub-Process 드릴다운", "기본
  렌더링 = 레벨별 별도 다이어그램") — 여기는 현재 코드로 재확인한 결과와 그 문서가 비워 둔 결정만 적는다.
- **Key Findings**(2026-09-28, `src/diagram/{mod,options,ir}.rs`·`src/diagram/bpmn/*.rs`·`src/diagram/layout/graph.rs`·
  `src/{cli,main,lib}.rs`·`.kiro/specs/*/biz-process.md` 7개·`biz-process-rules.md` 직접 확인):
  - `bpmn-support/research.md` 115~127행의 타입 스케치는 `bpmn-model` 구현 전 초안이라 실제와 다르다(아래 정정표).
  - `bpmn::Model`에는 중첩 개념이 없다(`Subprocess`는 접힌 것만, `Element`에 `parent` 없음, 그룹 아티팩트 없음).
    `bpmn-model/design.md` Revalidation Trigger가 "하류가 펼친 서브프로세스를 요구하면 그때 `parent`/`groups`
    도입"을 예약해 두었다 — 이 스펙이 그 결정을 내린다.
  - `ExpandPolicy`는 어디에도 없다. `DiagramOptions`는 `Copy` 구조체(`er_notation`, `direction`)이고 옵션 enum
    (`ErNotation`)은 `diagram::options`에 산다.
  - `ir::Group`은 `kind: GroupKind::{Box, Lane}`뿐이고 `layout::graph::draw_groups`는 항상 `LineKind::Solid`로
    그린다 — 파선 그룹 상자는 선 종류 필드 하나가 필요하다. `Canvas::rect`는 이미 `LineKind`를 받아 `Dashed`를 그린다.
  - `diagram::render_body`는 `(&'static str, Vec<Line>)` 한 장을 돌려주고 `caption()`은 비공개다. 여러 장 출력은
    본문 안에 캡션 줄을 직접 넣는 방식이 시그니처 변경 없이 가능하다.
  - `layout::graph::render`는 `graph.title`을 첫 줄에 굵게 넣는다(28~30행).
  - 테스트 기준선 488개 + doc test 2개(`cargo test` 2회 연속 전량 통과; 첫 실행의 1개 실패는 재현되지 않는
    비결정 실패로 기록만).

## Research Log

### `bpmn-support/research.md` 타입 이름 정정표(실제 코드 기준)
- **Context**: orchestrator 지시 — 초안 타입 이름을 베끼지 말고 실제 소스로 바로잡는다.
- **Findings**:

| research.md(115~127행) | 실제(`src/diagram/bpmn/model.rs` 등) |
|---|---|
| `Model{orientation,pools,nodes,flows,title}` | `Model{title, orientation, participants: Vec<Participant>, elements: Vec<Element>, flows: Vec<Flow>}` |
| `Pool{lanes}` | `Participant{id, name, lanes: Vec<Lane>}` |
| `Lane{sub_lanes}` | `Lane{id, name, sub_lanes}` (같음) |
| `FlowNode{kind,pool,lane,attached_to}` | `Element{id, name, kind: ElementKind, container: Option<String>, attached_to: Option<String>}` — 풀·레인 대신 `container`(참여자 id 또는 가장 안쪽 레인 id) 하나 |
| `Flow::{Sequence,Message,Association,DataAssociation}` | `Flow{id, source, target, label, kind: FlowKind}` + `FlowKind::{Sequence{is_default}, Message, Association, DataAssociation}` |
| `glyphs.rs` | `vocabulary.rs`(`TaskKind`/`EventTrigger`/`GatewayKind` 표 + `element_kind_from_xml_name`, `bpmn-yaml`이 승격) |
| `bpmn/{…,bizprocess,mod}.rs` 단일 파일 | 선례는 두 단계(`xml.rs`+`parse_xml.rs`, `yaml.rs`+`parse_yaml.rs`) — 이 스펙도 `bizprocess.rs`(문법) + `parse_bizprocess.rs`(의미) |
| "`ExpandPolicy`를 `lower`에 인자로" | `lower(model: &Model) -> Graph`는 그대로 두고 `lower_with(model, policy) -> Vec<LoweredDiagram>`을 더한다(기존 호출 불변) |

- **Implications**: design.md의 모든 시그니처는 위 실제 이름을 쓴다. `Element`에 `parent`, `Model`에 `groups`를 더하되
  기존 파서(XML·YAML)는 `None`/빈 값으로 채워 동작 불변.

### `biz-process.md` 실제 문법 재확인(7개 원문 + 템플릿 + 규칙)
- **Findings**:
  - 헤딩은 `## L1 Process: 이름 (valueChainRef: …)` → `### L2 Activity: 이름 (1.1, 1.2)` → `  ### L3 FunctionGroup/UI:
    이름` → `    ### L4 Step: 이름` → `      ### L5 DetailStep: 이름 (3.1)`. L2 이하는 전부 `###`이고 들여쓰기(0/2/4/6칸)로
    깊이를 표현한다 — 4칸 이상은 CommonMark상 코드블록이라 마크다운 파서로는 못 읽는다. 줄 기반으로 `L<n>` 토큰만
    본다(`#` 개수·들여쓰기 무시).
  - 헤딩이 다음 줄로 이어지는 경우 있음(`markdown-gfm-alerts` L4, `gitgraph-junction-polish` L5 등) — 이어지는 줄은
    더 깊이 들여쓴 평문.
  - `Logic(AST):` 아래 `- ` 항목: `IF c THEN a (id)`, `ELSE IF c THEN a`, `ELSE a`, `ELSE (메모) THEN a`, `항상: …`,
    `기존 테스트가 전량 통과함 (3.1)`(IF 없는 평문). 항목도 다음 줄로 이어진다. **실제 문서 7개에 `THROW`는 없다**
    (템플릿에만) — THROW 경로는 합성 fixture로만 검증한다.
  - 이름 속 괄호: `감시 루프(백그라운드)`(L3), `(표준입력이거나 파일 미지정)`(ELSE 메모). 게이트 줄
    `### ✅ 검토 요청 (L1: 이름)` + 다음 줄 `승인(✓) 또는 수정 사항을 입력하세요.`. 모든 문서가 L1 하나, L2마다 L3 정확히
    하나, L4 하나(드물게 둘), L5 1~3개.
  - 태그 `participant:`는 아직 어떤 문서에도 없다 → 7개 문서는 전부 "태그 없음 → 레인 없음" 경로.
- **Implications**: 파서는 라인 지향 상태기계; 레벨 건너뜀(L2 → L4)·이어지는 줄·게이트 줄을 규칙으로 명시(요구사항 2.x).

### 계층을 `Model`에 담는 방법
- **Context**: 드릴다운(접기/펼치기)은 요소가 어느 Sub-Process 안에 있는지 알아야 한다.
- **Alternatives**: 1) `ElementKind::Subprocess{ children: Vec<Element> }` 중첩 — 메타모델(`FlowElementsContainer`)에
  가장 충실하지만 `validate`·`lower`의 평면 순회(`model.elements.iter()`, `model.element(id)`)가 전부 재귀로 바뀜.
  2) `Element.parent: Option<String>` 평면 + 부모 참조 — 순회·조회 불변, `Element` 리터럴 46곳(대부분 테스트)에
  `parent: None` 한 줄. 3) 파서가 `Model` 여러 개를 투영(모델은 불변) — `lower`에 정책을 줄 수 없고 `All`·`Depth(n)`을
  파서가 구현하게 돼 사용자 결정("`ExpandPolicy`를 `lower`에")과 어긋남.
- **Selected**: (2). L3 그룹은 `Model.groups: Vec<Group{id, name, parent, members}>`(BPMN Group 아티팩트, 흐름 계층
  밖) — `Element`에 두 번째 필드를 더하지 않는다.

### 여러 장 출력 통로
- **Findings**: `render_body`가 한 장의 `(kind, lines)`를 돌려주고 `diagram::render`가 캡션을 붙인다. 마크다운·
  `main`·`lib`이 이 계약을 쓴다.
- **Selected**: 시그니처 불변. 첫 장의 종류는 `"process"`(정적), 둘째 장부터는 본문 안에 빈 줄 + 캡션 줄
  (`◈ bizprocess · activity: <이름>`)을 `bpmn::render_bizprocess`가 직접 넣는다. `diagram::caption`을 `pub(crate)`로
  열어 캡션 모양의 SSoT를 지킨다. 폭 검사(`render_body`)는 전체 줄에 그대로 적용된다. `RenderOptions.diagram_caption =
  false`여도 본문 안 캡션은 남는다(문서화).
- **Trade-off**: 한 장이라도 폭에 안 들어가면 전체가 `None`(기존 규약) — Activity 장은 노드 6개 안팎이라 위험 낮음.

### 파선 그룹 상자
- **Selected**: `ir::Group.line: LineKind`(기본 `Solid`) + `Graph::set_group_line`. `draw_groups`가 `block.line`을 쓴다.
  기존 그룹은 전부 `Solid`라 바이트 동일. `GroupKind` 변형 추가(`DashedBox`)는 레인/상자 판정 `match`를 전부 건드려
  기각.

### 정책 옵션의 위치와 이름
- **Selected**: `diagram::options::ExpandPolicy{Depth(usize), PerActivity, All}`(`Copy`, 기본 `PerActivity`),
  `DiagramOptions.expand_policy`, 옵션 키는 사용자 결정대로 `depth`(`--depth`, `DG_DEPTH`, 지시자 `depth=`). `bpmn`이
  `options`를 import하는 기존 방향을 유지(역방향 금지). 지시자에 `<!-- dg: depth=all -->` 꼴(마크다운 주석)을 더한다.

### 게이트웨이 매핑에서 비워져 있던 세부
- 다음 L5 형제는 소유 태스크에서 직접 이어진다(갈래는 안에서 끝남, 합류 없음) — 소유 태스크가 두 출력(다음 L5·게이트
  웨이)을 갖는 암묵 분기는 BPMN이 허용.
- `THROW`는 갈래 끝 `Event{End, Error}` + 그 L5를 품은 **L2**에 경계 `Event{Intermediate, Error}` 하나 — Process 그림
  (L2 접힘)에서 보이게. L4 상자에는 안 붙인다(중복).
- IF 없는 항목은 소유 태스크당 텍스트 주석 **하나**에 줄로 쌓는다(노드 수 억제, `Shape::Note`는 다중 줄 지원).
- 게이트웨이 이름 없음(`×`만), 조건은 흐름 라벨. `ELSE (메모)`의 메모는 default 흐름 라벨.
- 시작/종료 이벤트는 L1 루트와 각 L2 안에만(L4 안에는 없음).

### 회귀 픽스처 경로
- **Selected**: 실제 문서 7개를 `#[cfg(test)]`에서 `include_str!("../../../.kiro/specs/<spec>/biz-process.md")`로 읽는다
  (복사본 없음 — SSoT; 문서가 바뀌어도 여전히 파싱돼야 한다는 회귀 성질이 그대로 테스트). 태그·THROW·다중 L3·L4
  Sub-Process를 담은 합성 fixture는 `src/diagram/bpmn/fixtures/*.md`(테스트 전용, `examples/` 밖).

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 문법·의미 두 단계(`bizprocess` + `parse_bizprocess`) | 개요 트리(헤딩·태그·Logic) → `Model` | XML·YAML과 동형, 개요 리더는 BPMN을 모름, 각각 단위 테스트 | 파일 둘 | **채택** |
| 한 단계(줄을 읽으며 `Element` 생성) | 리더에 BPMN 지식 | 파일 하나 | 태그 상속·L3 하나/둘 판정처럼 형제 전체를 봐야 하는 규칙이 있어 어차피 두 번 지남 | 기각 |
| `Element.parent` 평면 중첩 | 부모 참조 | 순회 불변, 46곳 `parent: None` | 부모 사이클은 `validate`가 검사 | **채택** |
| `Subprocess{children}` 재귀 중첩 | 메타모델 그대로 | 충실도 | 검증·변환 전부 재귀화, XML·YAML 파서까지 영향 | 기각 |
| 파서가 장마다 `Model` 투영 | 모델 불변 | `Model` 수정 없음 | 정책이 파서에 갇힘, `lower` 인자 결정 위반 | 기각 |

## Design Decisions
(design.md §Key Decisions가 결정 본문의 SSoT — 여기는 대안·근거만.)

### Decision: 정책별 가시 집합은 `lower_with` 한 곳에서
- **Rationale**: `Depth(n)`·`All`·`PerActivity`(+ `step` 재귀)가 전부 "루트 + 가시 요소 + 펼칠 Sub-Process 집합"의
  변형이라 함수 하나로 일반화된다. 기존 `lower(model)`은 `Depth(0)` 첫 장으로 정의해 XML·YAML 출력 바이트 동일.
- **Trade-offs**: `lower.rs`가 커짐(+150~200줄) — 섹션 주석으로 완화.

### Decision: `All`/`Depth(n≥1)` 허용 조건은 `validate::participant_transitions`
- **Rationale**: 사용자 결정 "`validate`가 확인". `ModelError`가 아니라 조회 함수(모델은 유효, 정책만 불가) —
  `render_bizprocess`가 폴백을 결정하고 본문 첫 줄에 안내를 넣는다.

### Decision: L4 안 참여자 전환은 `step` 그림으로 재귀
- **Rationale**: 상자는 레인을 못 넘으므로(블록 트리 부모 하나) 펼칠 수 없는 L4는 접고 그 안을 별도 장으로 — brief의
  "Process/Activity/Step 3단"과 일치, 같은 함수의 루트만 바뀜. 실제 문서엔 태그가 없어 v1에서 경로만 검증.

## Risks & Mitigations
- 레인 안 Box 그룹(2단 중첩 포함) 배치가 `place_block`/`effective_group_ranges`에서 어긋날 수 있음 → tasks 첫 묶음에
  손으로 만든 `Graph` spike(레인 > 파선 상자 > 실선 상자 > 노드), 실패 시 `graph.rs` 최소 수정 + 레인 없는 fixture
  바이트 동일 유지
- `lower_with`로 일반화하며 XML·YAML 그래프의 노드·간선 순서가 바뀔 위험 → XML·YAML fixture 렌더링 바이트 동일 테스트(7.6)
- 태그 파서가 이름 속 괄호를 태그로 오인 → 7개 실제 문서 회귀(7.1) + `감시 루프(백그라운드)` 단정
- Activity 장이 폭 100을 넘어 전체가 코드블록으로 떨어질 위험 → 7개 문서 전부 폭 100·80 실측(7.1, 7.4), 넘으면 이름
  줄바꿈 규칙은 후속

## References
- `../bpmn-support/research.md` §"`participant:` 태그", §"L1~L5 → Sub-Process 드릴다운", §"기본 렌더링 = 레벨별
  별도 다이어그램", §Risks(태그 충돌·방법론 문서 미갱신)
- `../bpmn-model/design.md`(`Model` 계약, Revalidation Trigger "parent/groups"), `../bpmn-yaml/design.md`(두 단계
  파서·엄격 어휘·fixture 관례)
- `.agents/skills/kiro-biz-process/rules/biz-process-rules.md` §5(드릴다운 표현), `.kiro/settings/templates/specs/
  biz-process.md`(`ELSE THROW` 템플릿)
- `src/diagram/bpmn/model.rs:104-139`, `lower.rs:9-19`, `mod.rs:25-57`, `src/diagram/ir.rs:124-197`,
  `src/diagram/layout/graph.rs:708-745, 1825-1842`, `src/diagram/options.rs:32-95`, `src/diagram/mod.rs:16-122`
