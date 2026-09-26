# Research & Design Decisions — bpmn-xml

## Summary
- **Feature**: `bpmn-xml`
- **Discovery Scope**: Extension(`bpmn-model`이 만든 `Model`·`render_model`·`Language::Bpmn` 배선 위에
  첫 파서 한 겹). 범위·접근(손으로 쓴 XML 토크나이저, Descriptive Level 1 하위집합, BPMNDI 영구
  제외)의 SSoT는 `../bpmn-support/research.md` §"BPMN XML v1" — 여기는 현재 코드로 재확인한
  결과와 그 문서가 비워 둔 결정만 적는다.
- **Key Findings**(2026-09-26, `src/diagram/{mod,bpmn/*,mermaid/mod,plantuml/mod}.rs` 직접 확인):
  - `bpmn::vocabulary`의 토큰이 이미 BPMN XML 로컬 요소명 그대로(`userTask`·`messageEventDefinition`·
    `exclusiveGateway`)라 파서는 `from_xml_name(local_name)` 호출만 하면 된다 — 새 매핑표 불필요.
  - `bpmn::kind_of`·`bpmn::render`는 항상 `None`인 스텁이고, `render_model`이 `validate → lower →
    apply_to_graph → layout::graph::render`를 이미 갖고 있다 — 이 스펙은 `Model`을 채우는 일만 한다.
  - **`plantuml::kind_of`는 점수가 없으면 `Some("class")`를 돌려주는 포괄 판별기**다(`plantuml/mod.rs:88`).
    `diagram::language_of_source()`가 mermaid → PlantUML → BPMN 순서라, 확장자 없는 BPMN XML은
    BPMN 스니핑에 닿기 전에 PlantUML로 잡힌다. BPMN 스니핑을 PlantUML 앞으로 옮겨야 한다(요구사항 1.3).
  - 저장소에 XML 토크나이저·트리 타입이 전혀 없다(`mermaid/text.rs`·`plantuml/text.rs`는 줄 단위
    정리기). 이 스펙이 첫 문자 단위 토크나이저다.
  - `Graph::set_label`이 `\n`으로 본문을 여러 줄로 쪼갠다 — bpmn.io가 이름 줄바꿈을 `&#10;`으로
    내보내므로 문자 참조 디코딩만 하면 다중 줄 라벨이 공짜다.
  - `validate`는 참조가 하나라도 끊기면 모델 전체를 거부한다 — 실제 도구 출력에서 흔한 "메시지 흐름의
    끝이 블랙박스 풀(참여자 id)"는 이제 `bpmn::Model`이 직접 받아들인다
    (`bpmn-message-flow-participant-endpoint`, 2026-09-26); 파서가 미리 걸러야 하는 건 레인·
    `dataInput`·`property` 등 여전히 범위 밖인 id뿐이다(아래 Decision 갱신 참고).

## Research Log

### 실제 도구 출력의 표면 형태(bpmn.io·Camunda 7·Flowable, 기억 기반 — 실물 파일 미확보)
- **Context**: 토크나이저 범위가 실제 내보내기 파일을 덮는지.
- **Findings**:
  - 접두사: bpmn.io `bpmn:`, Camunda 7 `bpmn:`(구형 `bpmn2:`), Flowable 접두사 없음(기본 네임스페이스)
    + `flowable:` vendor. Signavio `semantic:`. 속성은 전부 접두사 없음(BPMN 스키마
    `attributeFormDefault="unqualified"`); 예외는 `xsi:type`·`xmlns:*`·vendor 속성(`camunda:assignee`).
  - 노드 자식: `incoming`/`outgoing`(무시), `*EventDefinition`, `extensionElements`(vendor 트리, 클 수
    있음), `dataInputAssociation`/`dataOutputAssociation`(`sourceRef`/`targetRef` **자식 요소 텍스트**로
    참조 — 속성이 아님), `conditionExpression`(`xsi:type`, 텍스트 = 표현식), `documentation`.
  - 데이터: bpmn.io는 `dataObjectReference`(이름·프로세스 안) + `dataObject`(정의, 이름 없음) 쌍,
    `dataStoreReference`만 두고 `dataStore`는 생략.
  - 줄바꿈 이름: `name="주문&#10;검토"`. 특수문자: `&amp;`·`&lt;`·`&quot;`.
  - `bpmndi:BPMNDiagram`은 `definitions` 마지막 자식으로 문서의 절반 이상을 차지한다.
  - 다이어그램 하나에 협업 없이 `process` 하나만 있는 게 기본 새 문서 형태.
- **Implications**: 요구사항 2.x·4.8·5.6의 근거. 토크나이저는 텍스트 자식(`flowNodeRef`·`sourceRef`·
  `text`)을 반드시 돌려줘야 하고, `extensionElements`는 `BPMNDiagram`처럼 통째로 건너뛰어도 된다.
- **미검증**: 실물 내보내기 파일로 대조하지 않았다 — 태스크에서 bpmn.io 형식 fixture를 테스트 문자열로
  써서 확인(요구사항 7.3). 실물 파일이 다르면 이 절을 갱신.

### 언어 스니핑 순서와 오판 위험
- **Context**: 요구사항 1.3·1.4·1.6.
- **Findings**: mermaid `kind_of`는 첫 낱말이 키워드여야 하므로 `<?xml`·`<definitions`에는 `None`.
  PlantUML `kind_of`는 위 Key Findings대로 포괄적. `language_of_source`는 확장자·펜스가 없을
  때(표준 입력·`.txt`, `lib.rs::render_diagram(language=None)`)만 불린다.
- **Implications**: `bpmn::kind_of`를 `plantuml::kind_of` 앞에 둔다. BPMN 스니핑은 "첫 요소가
  `definitions`(BPMN 네임스페이스 또는 네임스페이스 선언 없음)·`process`·`collaboration`"으로 엄격하므로
  PlantUML·mermaid 소스를 가로챌 수 없다(둘 다 `<`로 시작하는 첫 요소가 없다). WSDL도 루트가
  `definitions`지만 `xmlns="http://schemas.xmlsoap.org/wsdl/"`를 선언하므로 제외된다.

### 코드 크기 기준
- **Findings**: 기존 파서 129~372줄(`plantuml/gantt.rs` 최대). 토크나이저(요소·속성·텍스트·주석·CDATA·
  엔티티·접두사·불투명 구획·깊이 제한) 추정 250~300줄 + 테스트, 모델 빌더 추정 250~300줄 + 테스트.
- **Implications**: 파일 둘(`xml.rs`·`parse_xml.rs`)로 나누고 태스크도 둘로 나눈다(각각 독립 테스트 가능).

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| XML 크레이트(`roxmltree`·`quick-xml`) | 검증된 파서 채택 | 표준 적합성, 코드 0줄 | 전이 의존 추가로 "의존성 4개·단일 정적 바이너리" 원칙 위반 | **기각**(`bpmn-support/research.md` 결정 재확인) |
| 정규식·줄 단위 스캔 | 기존 파서처럼 줄마다 `<tag …>` 매칭 | 가장 짧음 | 속성이 여러 줄로 갈라진 태그·주석 안 `<`·CDATA에 깨짐 — 실제 내보내기가 정확히 그렇게 생김 | 기각 |
| 문자 단위 토크나이저 → 요소 트리 → 모델 | 두 단계: 문법(XML)과 의미(BPMN) 분리 | 각 단계 독립 테스트, BPMN 지식은 `parse_xml`에만 | 코드 두 파일 | **채택** |
| 스트리밍(SAX식) 한 단계 | 토큰을 바로 모델로 | 트리 메모리 없음 | `flowNodeRef`(레인) ↔ 노드, `default` ↔ 흐름처럼 문서 순서를 넘는 참조가 많아 어차피 두 번 지나야 함 | 기각 |

## Design Decisions

### Decision: BPMN 스니핑 규칙 = 첫 요소 로컬 이름 + 네임스페이스 선언 검사
- **Context**: `kind_of`가 범용 XML·YAML·`biz-process.md`에 오발하지 않으면서 손으로 쓴 접두사 없는 코드펜스도 받아야 한다.
- **Alternatives Considered**: 1) 본문 어디든 `omg.org/spec/BPMN` 포함 — 접두사·네임스페이스 없이 손으로 쓴 펜스(`<definitions><process>…`)를 놓침. 2) 첫 요소가 `definitions`면 전부 — WSDL 오발. 3) 첫 요소 로컬 이름이 `definitions`이고 (`xmlns*` 속성 중 하나가 `omg.org/spec/BPMN`를 담거나 `xmlns*` 속성이 하나도 없음), 또는 `process`/`collaboration`.
- **Selected Approach**: (3). 프롤로그(`<?…?>`·주석·`<!DOCTYPE`)만 건너뛰고 첫 시작 태그 하나만 렉싱한다 — 문서 전체를 파싱하지 않는다.
- **Rationale**: 실제 도구 출력은 항상 BPMN 네임스페이스를 선언하고, 손으로 쓴 조각은 보통 선언이 없다. 둘 사이(다른 네임스페이스만 선언한 `definitions`)는 BPMN이 아니다.
- **Trade-offs**: 네임스페이스를 선언하되 BPMN URI를 오타 낸 문서는 코드블록 — 허용.

### Decision: 토크나이저 관용도 = 구조는 엄격, 어휘는 관대
- **Context**: `bpmn-support/research.md`는 관용도를 정하지 않았다(orchestrator가 지목한 열린 결정).
- **Alternatives Considered**: 1) HTML식 복구(닫히지 않은 요소 자동 닫기) — 잘린 문서의 절반만 그려 사용자가 잘림을 못 알아챔. 2) 전부 엄격(모르는 엔티티도 실패) — `&nbsp;` 하나로 문서 전체가 코드블록.
- **Selected Approach**: 구조 오류(잘림·닫는 태그 불일치·따옴표/주석/CDATA 미종결·깊이 64 초과·루트 없음)는 `Err` → 코드블록. 어휘 오류(모르는 엔티티·잘못된 문자 참조)는 원문 그대로 텍스트로 남긴다. 루트 닫힘 뒤의 내용은 무시. 중복 속성은 첫 값.
- **Rationale**: BPMN 파일은 기계가 쓰므로 구조 오류 = 잘림·손상이라 숨기지 않는 편이 낫고, 어휘 오류는 이름 한 칸의 문제라 문서를 버릴 이유가 없다.
- **Trade-offs**: 잘린 문서의 부분 렌더링 없음 — 로드맵의 "손상 입력 강건성"은 패닉 없음 + 폴백이지 복구가 아니다.
- **Follow-up**: 사람이 확인할 결정 — tasks 승인 게이트에서 재확인.

### Decision: 불투명 구획(`BPMNDiagram`·`extensionElements`)은 토크나이저가 균형만 세고 트리를 만들지 않음
- **Context**: BPMNDI는 영구 제외(사용자 결정 2026-09-26)이고 문서의 절반 이상이다.
- **Alternatives Considered**: 1) 트리로 만들고 `parse_xml`이 무시 — 메모리·시간 낭비, 안 쓸 좌표를 디코딩. 2) 이름을 보고 텍스트 검색으로 `</bpmndi:BPMNDiagram>`까지 점프 — 주석·CDATA 안의 가짜 닫는 태그에 깨짐. 3) 같은 태그 렉서로 시작/끝 태그만 세며 건너뛰고(주석·CDATA는 정상 처리) 자식을 만들지 않음.
- **Selected Approach**: (3). 불투명 로컬 이름 목록은 BPMN 지식이라 `parse_xml`이 소유하고 `xml::parse_document(source, opaque)` 인자로 넘긴다.
- **Rationale**: 정확성(주석·CDATA)을 잃지 않고 트리 생성만 아낀다. 토크나이저는 BPMN 이름을 모른다.

### Decision: 요소 접두사는 벗기고, 속성 이름은 쓴 그대로
- **Context**: orchestrator 지시는 "요소·속성 접두사 제거". BPMN 스키마는 속성이 항상 비한정(unqualified)이다.
- **Alternatives Considered**: 1) 속성도 벗김 — `camunda:name` 같은 vendor 속성이 `name`과 충돌할 수 있고, 스니핑에 필요한 `xmlns:bpmn` 이름이 사라짐. 2) 쓴 그대로.
- **Selected Approach**: (2). 읽는 BPMN 속성(`id`·`name`·`processRef`·`sourceRef`·`targetRef`·`attachedToRef`·`default`·`parallelMultiple`)은 원문이 접두사 없이 쓰이므로 결과가 같고, vendor 속성과 충돌하지 않는다.
- **Rationale**: 표준이 보장하는 성질을 그대로 쓴다(지시와의 차이는 결과 동일·안전성 향상이므로 보고만).

### Decision: 깊이 상한 64, 트리 생성은 명시적 스택
- **Context**: 요구사항 6.4. Rust `Vec<Element>` 재귀 drop과 재귀 하강 파서는 깊은 중첩에서 스택 넘침(패닉이 아닌 abort)을 낸다.
- **Selected Approach**: 시작 태그 스택 길이가 64를 넘으면 `Err(TooDeep)`. 실제 BPMN 문서 깊이는 10 미만.
- **Rationale**: 상한이 있으면 재귀 드롭·재귀 레인 순회(`childLaneSet`) 모두 안전하다.

### Decision: 파서가 미리 거르는 흐름 = 끝이 "문서에 있지만 노드가 아닌 id"인 것
- **Context**: `validate`는 끊긴 참조 하나로 문서 전체를 거부한다(1.4). 실제 출력에는 참여자(블랙박스 풀)로 가는 메시지 흐름, `dataInput`/`property`로 가는 데이터 연관이 흔하다.
- **Alternatives Considered**: 1) 전부 통과 → 흔한 문서가 코드블록. 2) 끝이 모델 요소가 아니면 전부 버림 → 진짜 오타 참조(1.4·5.8)도 조용히 사라져 검증이 무력화. 3) 문서 안 어떤 요소의 `id`이긴 하지만 모델 요소가 되지 않은 id(참여자·레인·`dataInput`·`dataOutput`·`property`·`dataObject`·`dataStore`·`ioSpecification` 등)를 끝으로 가진 흐름만 버리고, 문서 어디에도 없는 id는 그대로 두어 `validate`가 거부한다.
- **Selected Approach**: (3). `parse_xml`이 의미 트리를 걷는 동안 "노드가 되지 않은 id" 집합을 모은다.
- **Rationale**: 범위 밖 항목으로의 흐름(v1 미지원)과 손상된 참조(거부)를 구분한다 — `bpmn-model` 1.4·1.9와 `bpmn-model/requirements.md` "블랙박스 풀은 띠만" 결정 그대로.
- **[2026-09-26 갱신, `bpmn-message-flow-participant-endpoint`로 대체]**: 참여자(블랙박스 풀) 끝은
  더는 "노드가 되지 않은 id"로 걸러지지 않는다 — `bpmn::Model::participant_index`가 최상위 참여자
  id를 `FlowKind::Message`의 정당한 끝으로 인정하고 `lower()`가 `Graph::group_anchor()`로 그 풀
  띠에 잇는다(사용자가 정보 손실을 지적해 별도 버그픽스로 해결). 이 결정은 이제 레인·`dataInput`·
  `property` 등 **참여자가 아닌** "노드 아닌 id"에만 적용된다.

### Decision: 경계 이벤트의 소속은 호스트를 따름
- **Context**: `validate`의 `BoundaryContainerMismatch`(1.7) — 도구가 경계 이벤트를 호스트와 다른 레인의 `flowNodeRef`에 넣거나 어디에도 넣지 않는 경우가 있다.
- **Selected Approach**: `attachedToRef`가 해석되면 `container`를 호스트의 것으로 덮어쓴다(레인 나열 무시). 호스트가 없으면 나열대로 두어 `validate`가 거부.
- **Rationale**: BPMN 의미상 경계 이벤트는 호스트 테두리 위에 있으므로 레인은 호스트의 레인이다(`bpmn/mod.rs` 테스트 주석의 관례와 동일).

### Decision: 참여자가 가리키지 않는 `process`의 처리
- **Context**: `bpmn-support/research.md` "process(레인 없으면 평면, 있으면 laneSet)".
- **Selected Approach**: 협업의 `participant/@processRef`가 가리키는 프로세스 → 그 참여자의 레인·노드. 아무 참여자도 가리키지 않는 프로세스 → `laneSet`이 있으면 프로세스 id·이름으로 참여자를 합성(레인이 풀 없이는 그려질 곳이 없음), 없으면 노드의 `container = None`(평면). 제목은 `collaboration/@name` → 유일한 `process/@name` → `definitions/@name`.
- **Rationale**: 협업 없는 단일 프로세스(가장 흔한 새 문서)가 띠 없이 그려지고(3.4), 레인 있는 단일 프로세스는 풀 하나로 그려진다(3.5) — 둘 다 BPMN 도구의 화면과 같다.

### Decision: `kind_of`는 문법 갈래 `"xml"`, 캡션 종류는 `render_model`의 것
- **Context**: `diagram::kind_of` 문서는 "종류 이름(flowchart …)"이라 하지만 BPMN 종류(process/collaboration)는 모델을 만들어야 안다(`bpmn-model/research.md`).
- **Selected Approach**: `bpmn::kind_of(source) → Some("xml")`(갈래), `bpmn::render`는 `render_model`이 준 `"process"`/`"collaboration"`을 캡션 종류로 돌려준다 — `bpmn-model/design.md` 6.7 그대로. 후속 `bpmn-yaml`은 `"yaml"` 갈래를 더한다.
- **Rationale**: `kind_of`의 역할이 `render`의 `match` 팔 선택이라는 `bpmn-model` 설계를 유지.

### Decision: id 없는 노드·흐름은 `@bpmn-xml:{n}` 합성 id
- **Context**: `validate`는 id 중복을 거부하므로 빈 id가 둘이면 문서 전체가 떨어진다. 손으로 쓴 코드펜스는 흐름 id를 생략하기 쉽다.
- **Selected Approach**: 문서 순서 번호로 합성. `@` 접두는 `@bpmn-placeholder:`·`@group-anchor:` 관례.

## Risks & Mitigations
- 실물 내보내기 파일 미확보 → bpmn.io 형식을 기억으로 재현한 fixture(7.3)로 검증; 실제 파일이 생기면 fixture 교체
- 접두사 없는 손 코드펜스와 `definitions` 루트의 다른 XML 구분이 "네임스페이스 선언 없음"에 의존 → 1.4 fixture에 WSDL·Maven·HTML을 포함해 못박기
- `language_of_source` 순서 변경이 PlantUML 판별을 바꿀 위험 → 기존 `robustness` PlantUML 배열 전부에 `language_of_source(None, …) == Some(PlantUml)`인 항목(`@start` 없는 것 포함) 확인(1.6)
- 이벤트 정의가 둘 이상인데 `parallelMultiple`이 없는 경우 `«multiple»` — 표준(§10.5.3 Multiple)과 일치, 도구 화면과도 같음

## References
- `../bpmn-support/research.md` §"BPMN XML v1 = OMG Descriptive 적합성(Level 1) 하위집합" · §"세 파서는 `bpmn::Model`을 거쳐"
- `../bpmn-model/{design,research}.md` — `Model` 계약, BPMNDI 영구 제외, 캡션 종류, `kind_of`/`render` 분리
- OMG BPMN 2.0.2 §8.3(Definitions), §9.3(Participant·`processRef`), §9.4(MessageFlow), §10.2(Process·LaneSet·`flowNodeRef`), §10.3.3(Activity `default`), §10.5(Event·EventDefinition·`attachedToRef`·`parallelMultiple`), §10.6(Gateway `default`), §10.4(DataObjectReference·DataStoreReference·DataAssociation `sourceRef`/`targetRef` 자식), §8.4(Artifact — TextAnnotation·Association); §12(BPMNDI)는 참조하지 않음
- XML 1.0 §2.4(문자 참조)·§4.6(미리 정의된 엔티티 5개) — 이 다섯과 숫자 참조만 디코딩
