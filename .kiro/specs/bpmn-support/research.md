# Research & Design Decisions — bpmn-support

## Summary
- **Feature**: `bpmn-support`
- **Discovery Scope**: Complex Integration(신규 다이어그램 종류 + 신규 입력 문법 3종 + 기존 그래프 배치기 확장)
- **Key Findings**:
  - BPMN 풀/레인은 `layout::graph`의 기존 그룹(Block) 메커니즘을 확장하는 것으로
    충분하다 — 새 레이아웃 엔진이 필요하지 않다(두 독립 리서치가 서로 검증).
  - mermaid·PlantUML 둘 다 BPMN 네이티브 문법이 없다 — BPMN XML(공식 표준)을
    첫 입력 문법으로 삼으면 이 문제를 우회할 수 있다.
  - `biz-process.md`의 L1~L5 계층은 BPMN의 Pool → Sub-Process(L2) →
    Sub-Process(L4) → Task(L5) 드릴다운 구조로 자연스럽게 매핑된다. L3만
    예외(구조 단위라 제어 흐름 계층에 안 들어감).

## Research Log

### 그룹/레인 구조 검증 (실측)
- **Context**: nwdiag와 BPMN 레인 둘 다에 쓸 "공유 레인 프리미티브"를 새로
  만들지, 기존 `graph.rs` 그룹을 확장할지 판단 필요.
- **Sources Consulted**: `src/diagram/layout/graph.rs`(직접 읽고 실제 렌더링
  실행), `src/diagram/layout/gitgraph.rs`, `src/diagram/layout/sequence.rs`.
- **Findings**:
  - `gitgraph.rs`의 트랙은 고정 전폭이 아니라 `extend()`로 넓어지는 내용
    적응형이다(`Plan.spans: Vec<Option<(usize,usize)>>`). 첫 검토에서 "고정
    전폭 밴드"로 오인했던 것은 틀렸다.
  - `graph.rs`의 `Block { children, start, width, layer_min, layer_max,
    registered }`는 이미 (층, 교차축) 사각형이고, `Child::Block(usize)`로
    중첩된다. `place_block()`은 층 범위가 겹치는 형제를 옆으로 미는 구간
    충돌 검사로 배치한다.
  - 중첩 subgraph를 실제로 LR 렌더링해 확인: 층 범위가 안 겹치는 형제
    그룹은 이미 나란히(띠처럼) 놓인다. "레인 = 그룹의 층 범위를 부모 전체
    범위로 강제"만 얹으면 띠가 만들어진다.
  - `push_outputs_below_groups()`가 그룹 뒤 노드를 그룹의 마지막 층 뒤로
    미는데, 레인 모드에서는 이게 다른 풀 전체를 밀어버려 꺼야 한다.
  - 형제 레인 사이 간격은 0이 아니라 **-1**이어야 한다(각 사각형이 테두리를
    따로 그리므로 0이면 두 줄로 겹쳐 보인다).
  - 풀 자체도 일반 그룹이 아니라 레인 종류 그룹이어야 한다 — 아니면 메시지
    흐름이 유발한 층 이동으로 풀 B가 짧게 그려져 OMG §9.2("entire length of
    the Diagram")를 어긴다.
- **Implications**: `GroupKind::{Box, Lane}` 필드 하나 추가 + `graph.rs`
  7곳 수정(§Design Decisions 참고)으로 충분. 새 모듈·새 좌표계 불필요.

### nwdiag와의 관계
- **Context**: nwdiag도 "가로 띠" 모양이라 같은 프리미티브를 공유할 수
  있는지 검토.
- **Findings**: 실제 PlantUML/blockdiag 구현을 대조한 결과, nwdiag의
  네트워크 버스선도 고정 전폭이 아니라 소속 노드 열 범위만큼만 그려지는
  내용 적응형이었다. 공유할 진짜 프리미티브가 없다(공통분모가
  `Canvas::hline`/`rect` 수준으로 줄어듦).
- **Implications**: nwdiag는 독립 소형 배치기(gantt/pie/xychart 전통)로
  별도 진행, 이번 스펙과 코드 공유 없음. 우선순위도 BPMN보다 낮음(사용자
  판단: "nwdiag는 필요성은 있으나 빈도가 낮고, BPMN은 빈도가 높음").

### mermaid/PlantUML의 BPMN 지원 현황
- **Context**: 목표 문법이 안정적인지 확인.
- **Sources Consulted**: mermaid GitHub 이슈 #8160, PR #8166·#8313(gh CLI로
  조회), PlantUML 포럼 스레드 다수.
- **Findings**: PlantUML은 BPMN 네이티브 지원이 전혀 없다. mermaid는 서로
  충돌하는 베타 PR 두 개가 병합 대기 중(유지관리자가 직접 충돌을 지적).
  안정된 문법이 없다.
- **Implications**: mermaid BPMN 포팅은 "향후" 트랙으로 보류. 대신 이미
  안정된 BPMN 2.0 XML(OMG 공식 표준)을 첫 입력 문법으로 채택해 "안정된
  타깃이 없다"는 병목을 우회한다.

### `biz-process.md` 실제 구조 (7개 파일 직접 확인)
- **Context**: 이 프로젝트 자체 산출물을 BPMN으로 그리는 매핑 설계.
- **Findings**:
  - 구조: `## L1 Process` → `### L2 Activity` → `### L3 FunctionGroup/UI` →
    `### L4 Step` → `### L5 DetailStep`(+ `Logic(AST)`). L2:L3은 거의 1:1,
    분기는 오직 L5의 `Logic(AST)`(`IF/ELSE IF/ELSE/THEN/THROW`)에서만 나온다.
  - 이름에 괄호가 섞여 있는 경우가 실제로 있다(예: "감시 루프(백그라운드)"),
    `### ✅ 검토 요청 (L1: …)` 게이트 줄도 있다 — 태그 파서가 이 둘을
    구조 태그와 구분해야 한다.
  - `biz-process-rules.md` §1: "주어는 항상 사용자/역할"은 서술 시점
    규칙일 뿐, 실제로는 학습자·멘토·운영자·시스템운영자 등 역할이 이미
    존재한다(사용자 확인).
- **Implications**: §Design Decisions의 `participant:` 태그·드릴다운 매핑.

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 공유 레인 프리미티브(신규) | gitgraph·sequence·nwdiag가 공유하는 독립 모듈 | 겉보기 재사용성 | 실제 소비자 2곳 다 전제가 틀림(내용 적응형) — 사변적 추상화 | **기각** |
| `graph.rs` 그룹 확장 | 기존 `Block`에 `GroupKind::Lane` 추가 | 기존 인프라 재사용, 회귀 위험 낮음, 코드 실측으로 검증됨 | `graph.rs` 자체가 커짐(+200~250줄) | **채택** |
| 세 파서가 각자 `ir::Graph` 직접 생성 | mermaid/PlantUML 파서와 같은 패턴 | 단순함 | "메시지 흐름=점선+○꼬리" 같은 BPMN 어휘 규칙을 세 파서가 중복 구현(SSoT 위반) | 기각 |
| `bpmn::Model` 공유 의미 계층 경유 | 세 파서 → Model → validate → lower → `ir::Graph` | 어휘 규칙 SSoT 하나, 검증 로직 한 곳 | 계층 하나 추가(작은 비용) | **채택** |

## Design Decisions

### Decision: 레인은 `graph.rs`의 그룹 확장으로
- **Context**: BPMN 풀/레인을 어떤 배치기로 그릴지
- **Alternatives Considered**: 1) 신규 공유 레인 프리미티브 2) `graph.rs` 그룹 확장
- **Selected Approach**: `Group`/`Block`에 `GroupKind::{Box, Lane}` 추가.
  레인 그룹의 층 범위는 "가장 가까운 상위 범위(부모가 없으면 다이어그램
  전체, 있으면 그 부모의 유효 범위)"로 강제. 풀 = 최상위 Lane 그룹, 레인 =
  그 안의 중첩 Lane 그룹.
- **Rationale**: 실제 코드 실측으로 최소 변경(7개 지점, 상세는 아래
  Follow-up)임을 확인. 새 레이아웃 개념·좌표계가 필요 없다.
- **Trade-offs**: `graph.rs`가 커지지만(섹션 분리로 완화 가능), 기존
  gitgraph·sequence·subgraph 코드는 전혀 안 건드린다.
- **Follow-up**(구현 시 반영할 `graph.rs` 변경 지점):
  1. `group_layer_ranges()` → `effective_group_range()`로 통합(Lane이면
     부모 유효 범위, 아니면 기존처럼 구성원 범위)
  2. `place_block()`: 부모·형제가 둘 다 Lane이면 간격 `-1`(테두리 공유)
  3. `reorder()`/`improve_by_swaps()`: Lane 블록은 순서 고정(선언 순서 =
     DI 세로 순서 보존)
  4. `push_outputs_below_groups()`: 대상 그룹이 Lane이면 건너뜀
  5. `group_top()`/`inner_rank()`: 부모가 Lane인 Lane 블록은 랭크를 안 올림
     (테두리가 부모와 한 칸에 겹치게)
  6. 배너 공간: `layer_start[0]`에 레인 깊이별 오프셋 추가, 제목은 테두리
     위 대신 배너 안에
  7. 방향: `Model.orientation` → `Graph.direction`(기존 LR/TB 재시도 폴백
     그대로 재사용)

### Decision: 세 파서는 `bpmn::Model`을 거쳐 `ir::Graph`로
- **Context**: BPMN XML·biz-process.md·(미래) mermaid 세 문법이 공유할 계층
- **Selected Approach**: `src/diagram/bpmn/{model,validate,lower,glyphs,xml,
  parse_xml,bizprocess,mod}.rs`. `Model{orientation,pools,nodes,flows,title}`,
  `Pool{lanes}`, `Lane{sub_lanes}`(중첩), `FlowNode{kind,pool,lane,
  attached_to}`, `Flow::{Sequence,Message,Association,DataAssociation}`.
- **Rationale**: "시퀀스 흐름은 풀을 못 넘는다", "메시지 흐름=점선+○꼬리"
  같은 BPMN 고유 규칙을 한 곳(`validate`/`glyphs`)에만 둔다. 미래 mermaid
  파서는 `fn parse(&str) -> Model` 하나만 구현하면 나머지(검증·도형·배치)를
  공짜로 얻는다.
- **Trade-offs**: 다른 다이어그램 종류보다 계층이 하나 더 있음 — BPMN
  고유의 교차 검증 요구(풀 경계 규칙 등)가 그래프 자체엔 없는 개념이라
  정당화됨.

### Decision: BPMN 도형·표식 어휘 (`bpmn-shapes` 스펙이 그대로 쓸 매핑표)
- **Context**: Gateway·Activity(Task)·Event 등 BPMN 표기를 dg의 문자 제약
  안에서 어떻게 그릴지. **이 표는 아직 어떤 스펙에도 구현되지 않았다** —
  `bpmn-lane-layout`(현재 진행 중)은 레인 배경만 다루고 도형은 다음 스펙
  (`bpmn-shapes`)의 범위다(로드맵 순서, brief.md Scope 참고).
- **Selected Approach**(기존 `Shape`/`Marker`/`LineKind` 재사용을 최대화):

  | BPMN 요소 | dg 렌더링 | 비고 |
  |---|---|---|
  | 시작/중간/종료 이벤트 | `Shape::Event(pos)`(신규): 둥근 상자, 테두리 안 왼쪽에 `○`/`◎`/`●`. 종료는 `LineKind::Heavy` 테두리 | 얇음/굵음 관례는 기존 선 종류로 해결. 중간 이벤트의 이중선(`╔═╗`)은 `LineKind::Double` 신규 추가가 필요해 후순위 |
  | 이벤트 트리거(message/timer/error/signal 등 13종) | 이름 아래 둘째 줄에 `«message»`·`«timer»`·`«error»`… | `shape.rs::line_style()`이 이미 `«`로 시작하는 줄을 흐리게·기울여 그린다(PlantUML 스테레오타입 관례 재사용, 신규 로직 불필요). 아이콘 글자(✉◷⚡)는 폰트 커버리지 문제(CJK 모노 폰트 폴백 이력)로 v1 제외 |
  | 태스크(user/service/script/manual/business-rule/send/receive) | `Shape::Round` + 둘째 줄 `«user»`·`«service»`… | 종류 none이면 둘째 줄 생략 |
  | 접힌 서브프로세스 | `Shape::Subprocess`(신규): Round 아래 테두리 가운데 `[+]` | |
  | 펼친 서브프로세스 | 레인 안 `Box` 종류 그룹(기존 중첩 그룹 그대로) | |
  | 호출 활동(Call Activity) | Round + `«call»` | |
  | 게이트웨이(XOR/AND/OR/이벤트기반/복합) | `Shape::Diamond`, 라벨 = 기호+이름(`×` XOR, `+` AND, `○` OR, `*` complex, `◎` 이벤트기반) | **해결됨(`bpmn-shapes` 구현, 2026-09-25)**: `⬠`(U+2B20)·`∗`(U+2217) 둘 다 기본 CJK 모노 폰트 커버리지 밖으로 실측돼 각각 `◎`·ASCII `*`로 교체. 최종 표는 `bpmn/vocabulary.rs`(`bpmn-model`)가 SSoT |
  | 데이터 저장소 | `Shape::Cylinder`(기존) | |
  | 데이터 객체 | `Shape::Rect` + `«data»` | 접힌 모서리 도형은 후순위 |
  | 텍스트 주석 | `Shape::Note`(기존 점선 상자) | |
  | 시퀀스 흐름 | `LineKind::Solid` + `Marker::Arrow`, 조건은 `label` | |
  | default 흐름(빗금 꼬리) | `Marker::Slash`(신규) | `marker_glyphs()` 표에 한 줄 추가 |
  | 메시지 흐름(풀 사이만) | `LineKind::Dashed` + 꼬리 `Marker::Circle` + 머리 `Marker::OpenArrow` | 전부 기존 마커 재사용 |
  | 연관 / 데이터 연관 | `Dashed` 무표식 / `Dashed` + `OpenArrow` | |
  | 경계 이벤트(Boundary Event) | v1 근사: 호스트 태스크 다음 층에 중간 이벤트 노드를 두고 무표식 `Dashed` 선으로 연결 | 테두리에 실제로 부착하는 렌더링은 레이아웃이 "노드-노드 부착" 개념이 없어 후순위 |
  | Group 아티팩트(풀을 넘는 상자) | 미지원 | 블록 트리가 부모 하나만 허용 — mermaid `bpmn-beta`도 같은 한계("a group cannot yet stretch across pools") |
- **Rationale**: 기존 `Shape`/`Marker`/`LineKind` 어휘를 최대한 재사용해
  신규 추가를 `Shape::Event`·`Shape::Subprocess`·`Marker::Slash` 세 개(+
  후순위 `LineKind::Double`)로 최소화. `«stereotype»` 둘째 줄 관례는
  PlantUML class 다이어그램에서 이미 검증됨.
- **Trade-offs**: 진짜 BPMN 아이콘(우편봉투·시계·번개 등)은 문자 제약상
  포기하고 `«이름»` 텍스트로 대체 — 정보는 보존되지만 시각적으로는
  간략화됨.

### Decision: `participant:` 태그, 레인 없으면 렌더링도 그대로
- **Context**: `biz-process.md`에 역할(학습자/멘토/운영자 등)을 어떻게
  표시할지
- **Alternatives Considered**: 1) `role:`(fable 권장, BPMN 메타모델 용어에
  더 정확) 2) `participant:`(사용자 채택, role/조직/actor를 포괄하는 상위
  개념)
- **Selected Approach**: `participant:` 채택(사용자 결정). 헤딩 끝 괄호에
  `(participant: 학습자)` 형태로, 기존 `(valueChainRef: …)`/`(1.1, 2.3)`
  괄호 관례를 그대로 일반화("ID 목록" 아니면 "key: value 목록"). 가장 가까운
  조상의 태그가 상속되고, **역할 이름당 레인 하나**(재등장 시 기존 레인으로
  복귀, 새 레인 안 만듦). 태그가 전혀 없으면 레인 없이 지금과 동일하게
  렌더링(폴백 레인 없음, 사용자 결정) — 기존 7개 문서 전부 호환.
- **Rationale**: 하나의 L1은 끊기지 않는 단일 흐름이라, 그 안에서 바뀌는
  주체는 BPMN 정의상 풀이 아니라 레인이다(시퀀스 흐름은 풀 경계를 못 넘음,
  OMG §9.2). 진짜 다중 풀(별도 조직, 메시지로만 통신)이 필요해지면 그건
  문서 구조 확장(L1을 분리 + 메시지 표기)이 필요한 별도 문제라 지금
  선반영하지 않는다(YAGNI).
- **Trade-offs**: 태그 이름이 BPMN 메타모델의 엄밀한 "Participant=Pool"
  구분과 다르게 쓰이지만, 이건 dg/Kiro 자체 문서 표기이지 BPMN 파일 포맷이
  아니므로 문제 없다고 판단.
- **파싱 함정**(실제 파일에서 확인, `bizprocess.rs` 구현 시 반영):
  - 이름 자체에 괄호가 있는 경우(예: "감시 루프(백그라운드)") — 괄호
    내용이 ID 목록이나 key:value 목록으로 완전히 파싱될 때만 태그로 인식,
    아니면 이름 텍스트로 둔다.
  - `### ✅ 검토 요청 (L1: …)` 게이트 줄은 접두사로 먼저 제외.
  - 헤딩 텍스트가 다음 줄로 이어지는 경우 합친 뒤 괄호를 본다.
  - 값에 쉼표 불허(진단), 앞뒤 공백만 제거하고 대소문자는 그대로("시스템운영자"
    ≠ "시스템 운영자", 진단으로 알림).

### Decision: `biz-process.md` L1~L5 → Sub-Process 드릴다운
- **Context**: L4/L5의 분기(`Logic(AST)`)를 BPMN으로 어떻게 표현할지, 여러
  L5가 순차인지 병렬인지
- **Alternatives Considered**: 1) 평면 매핑(L4→L5 사슬 + 인위적 합류
  게이트웨이) 2) Sub-Process 드릴다운(L2·L4를 접힌 Sub-Process로)
- **Selected Approach**: (2) 채택. L1=풀(+`participant:` 레인), L2=항상
  Sub-Process(펼치면 안에 L3 그룹+L4들), L3=파선 그룹 박스(2개 이상일 때만,
  하나면 L2 상자의 부제 — 흐름 계층에는 안 넣음), L4=자식이 있으면
  Sub-Process·없으면 태스크, L5=태스크, `Logic(AST)`는 소유 L5 뒤 게이트웨이
  (`IF`=조건 분기, `ELSE`=default 흐름, `THROW`=경계 오류 이벤트, IF 없는
  줄=텍스트 주석).
- **Rationale**: (a) `ELSE THROW`가 안쪽 종료 이벤트가 아니라 **바깥 접힌
  상자의 경계 오류 이벤트**로 자연스럽게 이어짐(실제 dg-watch-mode 문서의
  "오류 메시지와 함께 비정상 종료"와 정확히 일치). (b) 인위적 "합류
  게이트웨이"가 사라짐 — 분기는 안에서 종료되고 Sub-Process 완료 후 바깥
  흐름이 이어짐. 사용자가 원래 구상한 "Process/Activity/Step 드릴다운"
  개념과도 정확히 일치하며, BPMN 자체 기능(Sub-Process 접기/펼치기)이라
  새 개념이 필요 없다.
- **Trade-offs**: L3만 이 3단 모델에 안 맞음(제어 흐름이 아니라 구조
  단위) — 명시적 예외로 처리(사용자 결정: 파선 그룹 박스).
- **미해결로 남긴 것**: L5 형제가 순차인지 병렬인지는 텍스트만으로 알 수
  없다 — Sub-Process 안에서는 일단 순차로 두되(문서 등장 순서), 이후 실제
  병렬 표기가 `biz-process-rules.md`에 추가되면(예: `PAR:`) AND 게이트웨이로
  대응할 여지를 `Model`에 남겨 둔다.

### Decision: 기본 렌더링 = 레벨별 별도 다이어그램(전부 펼치기는 v1 제외)
- **Context**: 드릴다운을 실제로 화면에 어떻게 그릴지(dg는 코드펜스 하나당
  평면적인 한 페이지를 그리는 구조)
- **Alternatives Considered**: 1) 전부 펼쳐서 한 장(`All`) 2) 레벨별 여러
  장(`PerActivity`, 각 L2가 자기 다이어그램)
- **Selected Approach**: (2)를 기본값으로. `Model`을 넘길 때 `ExpandPolicy`
  (`Depth(n) | PerActivity | All`)를 `lower`에 인자로 주고, `DiagramOptions`에
  `depth` 옵션을 추가. 기본 출력은 "Process 다이어그램 1장(L2들을 접힌
  Sub-Process 상자로) + L2마다 그 안을 펼친 Activity 다이어그램 1장씩",
  캡션으로 구분(`◈ bizprocess · process` / `◈ bizprocess · activity: <이름>`).
  `All`은 `validate`가 "어떤 L2 안에서도 참여자 전환이 없음"을 확인했을 때만
  허용, 아니면 진단 후 묶음 방식으로 자동 폴백. 사용자 결정으로 이 기본값
  그대로 채택.
- **Rationale**: 실측 결과 전부 펼치면 실제 문서 규모(7개 L2 × 2~3 L5 ≈
  25노드 직렬)가 LR로 300칸을 넘어 TB 폴백 시 200행짜리 세로 다이어그램이
  되어 읽을 수 없다. 또한 펼친 Sub-Process 안에서 참여자가 바뀌면 그 안에
  또 레인이 필요한데, 그건 v1 레인 설계(다이어그램 최상단에만 배너) 범위
  밖이다. 반대로 "레벨별 별도 다이어그램"은 각 장의 폭이 예산 안에 들고,
  펼친 Sub-Process 안의 역할 전환이 그 장 자체의 최상위 레인이 되어 v1
  레인 설계로 그대로 그려진다 — 그리고 이건 실제 BPMN DI의 관례(펼친
  Sub-Process마다 별도 `BPMNDiagram`)와도 같다(dg만의 타협이 아님).
- **Follow-up**: 특정 노드만 골라 펼치는 기능은 `biz-process.md`에 안정된
  id가 없어 v1 제외. 페이저의 기존 다이어그램 블록 클릭-토글
  (`pager.rs::toggle_block_at()`)이 이후 자연스러운 확장 지점.

### Decision: BPMN XML v1 = OMG Descriptive 적합성(Level 1) 하위집합, 손으로 토큰화
- **Context**: 실제 BPMN XML은 방대함 — v1 범위와 파싱 방법
- **Selected Approach**: 새 크레이트 없이 손으로 쓴 XML 토크나이저
  (`bpmn/xml.rs`, 요소·속성·텍스트·주석/CDATA·엔티티 디코딩·네임스페이스
  접두사 제거). 지원 범위: `collaboration/participant`(풀)·`messageFlow`,
  `process`(레인 없으면 평면, 있으면 `laneSet`), 시작/중간/경계/종료
  이벤트(message·timer·error·signal 등 트리거), 태스크 7종·(접힌)
  서브프로세스·호출활동, 게이트웨이 5종(+`@default`), `sequenceFlow`,
  기본 아티팩트.

  **[2026-09-26 갱신, 사용자 결정으로 대체됨]**: `BPMNDI`(다이어그램 교환
  계층) 파싱은 `isHorizontal`을 포함해 전부 영구 제외한다 — 충실도 대상은
  BPMN 2.0.2 프로세스 모델(의미 계층: Process·Task·Event·Gateway…)이지
  다이어그램 노드가 아니라는 사용자 결정(`bpmn-model/design.md` Key
  Decisions). `bpmn-xml`은 `<bpmndi:*>` 구획을 통째로 건너뛰고, 방향은
  `bpmn::model::Orientation` 기본값 + CLI `--direction` 옵션으로만 정한다.
- **Rationale**: "의존성 4개·단일 정적 바이너리" 원칙 유지. 이 하위집합은
  mermaid #8313이 명시한 v1 목표이자 실제 도구(Camunda/bpmn.io) 샘플
  대부분을 커버한다.
- **Trade-offs**: YAML은 v1 제외(표준 스키마 없음, 세 번째 문법 자리가
  이미 있으므로 서두를 이유 없음).

  **[2026-09-25 갱신, 사용자 결정으로 뒤집힘]**: XML과 동급으로 YAML도
  지원한다(코드펜스에 손으로 쓰기엔 XML이 너무 장황함). 표준 BPMN YAML이
  없으므로 `bpmn::Model`을 그대로 옮긴 dg 자체 YAML 스키마를 정의한다.

  **[2026-09-25 리서치 완료, fable]** — 결론: **YAML 블록 스타일 하위집합을
  손으로 파싱한다**(크레이트 추가 없음, dg 전용 유사 문법 발명도 안 함).
  - 기존 파서들(`plantuml/wbs.rs`의 깊이 스택, `mermaid/text.rs`의 따옴표
    인식 분할·`label()` 이스케이프, `gitgraph.rs`/`gantt.rs`의 key:value
    스캔)이 이미 YAML 하위집합 파싱에 필요한 기법을 전부 갖고 있다 — 새
    알고리즘이 필요 없다.
  - `serde_yaml`(2024 유지 종료)·`yaml-rust2` 등 크레이트는 전이 의존이
    늘어나 4-의존성 원칙을 깨는 대가로 아끼는 코드(150~200줄)에 안 맞아
    기각. dg 전용 유사 문법도 파싱 난이도 절감이 미미한데 아무 도구도
    못 읽는 4번째 문법이 생기고, 로드맵이 일부러 비워 둔 "미래 mermaid"
    자리를 dg 방언이 선점해 버려 기각.
  - **받아들이는 것**: 블록 매핑(`key: value`)·블록 시퀀스(`- item`), 평문/
    홑·겹따옴표 스칼라, `#` 주석(따옴표 밖), 임의 폭의 공백 들여쓰기.
    **받아들이지 않는 것**: 앵커/별칭, 태그, 다중 문서, 흐름 매핑/시퀀스,
    블록 스칼라(`|`/`>`), 복합 키, 암묵 타입 변환 — 전부 BPMN처럼 깊이
    4~5의 트리+문자열 구조에는 애초에 불필요.
  - 노드는 압축형(`- id: kind label…`, 시퀀스 항목=단일 키 매핑)과 확장형
    (값이 매핑, `trigger`/`attached_to`/`default` 등 추가 필드 필요할 때)
    두 가지 — "압축형은 kind+label만"이라는 한 줄 규칙으로 모호성을 없앤다.
    흐름은 문자열 한 줄(`a --> b`, `a -.-> b`(메시지), `a -- 라벨 --> b`) —
    `mermaid/flow.rs::read_link`를 `pub(crate)`로 열어 재사용(SSoT, 새로
    안 만듦).
  - kind/trigger 어휘는 BPMN XML 로컬 요소명을 그대로 토큰으로 써서 XML·
    YAML 두 파서가 `bpmn/vocabulary.rs`의 같은 표 하나를 보게 한다 —
    `bpmn-model` 스펙이 이 표를 이미 확정했다(완료, 2026-09-26).
  - 파일 구성: `bpmn/yaml.rs`(하위집합 리더 → `YamlValue{Scalar,Seq,Map}`
    트리) + `bpmn/parse_yaml.rs`(트리 → `Model`), 각 150~250줄 — 기존
    파서 규모(129~372줄) 안.
  - **로드맵 순서 권고**(2026-09-25, 한때 반영됨): YAML이 XML보다 작고
    사람이 읽기 쉬워 `bpmn-model`의 테스트 fixture로도 좋으므로,
    `bpmn-xml`을 쪼개 `bpmn-yaml`(먼저)·`bpmn-xml`(나중)로 진행한다.

    **[2026-09-26 갱신, 사용자 결정으로 뒤집힘]**: 순서를 `bpmn-xml`(먼저)
    → `bpmn-yaml`(나중)로 되돌린다 — 이유: "모델 충실도 유지". XML은 공식
    표준이라 `bpmn::Model`의 충실도를 먼저 검증하는 기준이 될 수 있지만,
    YAML은 이 프로젝트가 손으로 만든 편의 문법이라 그것이 먼저 모델 형태를
    정하면 모델이 표준이 아니라 YAML에 맞춰질 위험이 있다. `bpmn-yaml`은
    `bpmn-xml`이 검증한 모델을 그대로 옮기는 대안 직렬화로 재배치됐다
    (`bpmn-support/roadmap.md`).
  - **미검증**: 실제 YAML 예시를 외부 YAML 파서(`yq` 등)로 돌려 유효성을
    기계 검증하지는 않음 — `bpmn-yaml` 스펙의 태스크로 넣을 것.

## Risks & Mitigations
- 레인 배너·펼친 Sub-Process 폭 예산 초과 → 실제 문서로 `bpmn-lane-layout`
  구현 시 조기 실측, 안 들어가면 TB 폴백 모양이 읽을 만한지 별도 확인
- ~~게이트웨이 글자(특히 이벤트 기반 게이트웨이 `⬠`) 폰트 커버리지 불확실~~
  **해결됨**: `bpmn-shapes` 구현에서 실측 완료, `◎`·ASCII `*`로 교체(위
  Decision 표 참고)
- `fold_widest_row()`/`improve_by_swaps()`가 Lane 블록을 건드리지 않는지
  회귀 테스트로 못박기(레인 순서가 흐트러지면 방법론적으로 틀린 그림이 됨)
- `biz-process.md` 태그 파싱이 실제 문서의 괄호 포함 이름·게이트 줄과
  충돌하지 않는지 7개 기존 파일 전부를 회귀 픽스처로 사용
- 방법론 문서(`biz-process-rules.md`) 갱신을 미뤄 뒀으므로, 이 스펙이 정한
  `participant:` 태그 문법이 나중에 방법론 쪽에서 다르게 확정되면 dg
  파서도 같이 바뀌어야 함 — 이번 스펙의 태그 문법을 dg 쪽에서만 잠정
  표준으로 문서화해 둘 것

## References
- OMG BPMN 2.0 Specification §9.2(Pool), §10.7(Lane), DI `isHorizontal`
- mermaid GitHub: issue #8160, PR #8166, PR #8313
- `.kiro/specs/*/biz-process.md`(7개), `.agents/skills/kiro-biz-process/rules/biz-process-rules.md`
- `.kiro/specs/plantuml-wbs-nwdiag/{brief,roadmap}.md`(nwdiag 쪽 선행 검토,
  이번 스펙으로 결론 이관)
