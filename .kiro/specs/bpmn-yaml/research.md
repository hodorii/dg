# Research & Design Decisions — bpmn-yaml

## Summary
- **Feature**: `bpmn-yaml`
- **Discovery Scope**: Extension(`bpmn-model`의 `Model`·`render_model` 위에 `bpmn-xml`과 나란한 두 번째 파서
  한 겹). 파싱 방식(YAML 블록 스타일 하위집합 손파싱, 크레이트 없음)·문법 범위·노드 두 표기·흐름 한 줄
  표기·`read_link` 재사용·어휘 표 공유·파일 구성(`yaml.rs` + `parse_yaml.rs`)의 SSoT는
  `../bpmn-support/research.md` §"[2026-09-25 리서치 완료, fable]" — 여기는 현재 코드로 재확인한 결과와 그
  문서가 비워 둔 결정만 적는다.
- **Key Findings**(2026-09-27, `src/diagram/{mod,bpmn/*,mermaid/{flow,text}}.rs` 직접 확인):
  - `mermaid/flow.rs::read_link(chars: &[char], cursor: &mut usize) -> Option<Link>`는 비공개 함수이고
    `Link { label, kind: LineKind, head: Marker, tail: Marker }`도 비공개 구조체다. 재사용은 둘의 가시성을
    `pub(crate)`로 여는 것만으로 되고 동작 변경이 없다. `read_link`는 라벨에 `mermaid/text.rs::label()`을
    적용한다(따옴표 제거·`<br>`·`\n`·HTML 실체) — YAML 흐름 라벨도 같은 정리를 받는다.
  - 노드 로컬 이름 → `ElementKind` 대응(`startEvent`·`subProcess`·`callActivity`·`dataObjectReference`…
    12개)은 `bpmn/vocabulary.rs`가 아니라 `parse_xml.rs::element_kind_of`의 `match` 안에 있다. 태스크·게이트
    웨이만 어휘 표를 거친다. YAML이 같은 토큰을 쓰려면 이 대응을 `vocabulary`로 올려야 SSoT가 지켜진다.
  - `diagram::language_of_source`는 이미 `bpmn::kind_of`를 PlantUML 앞에서 부른다(`bpmn-xml` 3.2) — YAML
    스니핑을 `bpmn::kind_of` 안에 더하면 판별 순서 변경이 필요 없다. `language_of_path`는 `.yaml`·`.yml`을
    모른다(그대로 둔다 — 범용 YAML을 BPMN으로 오판할 수 없다).
  - 저장소에 `yq`는 없고 `python3` + PyYAML 6.0.3이 있다. 초안 YAML(주문 처리 협업)을 PyYAML `safe_load`로
    읽어 유효함을 확인했다 — `start-cust --> order-task`·`order-task -.-> msg-start`·`재고 있음?`는 모두 유효한
    평문 스칼라다. 단 `safe_load`(core 스키마)는 `true`·`007`·`~`를 bool·int·null로 바꾸므로, dg의 "암묵 타입
    변환 없음"과 구조를 비교할 때는 모든 스칼라를 문자열로 읽는 `yaml.BaseLoader`를 써야 한다.
  - 테스트 기준선 434개(`cargo test` 전량 통과). 기존 `bpmn/mod.rs`·`diagram/mod.rs` 테스트가
    `kind_of("process:\n  id: p1\n") == None`을 못박아 두었다 — 스니핑 규칙이 스키마 밖 YAML을 계속 거부해야
    이 테스트가 무수정 통과한다.

## Research Log

### 스니핑 규칙 — 스키마 밖 YAML·평문·PlantUML과의 충돌
- **Context**: 요구사항 1.1·1.3·1.4·1.6. `bpmn::kind_of`는 PlantUML 포괄 판별기 앞에 있으므로 스니핑이
  느슨하면 확장자 없는 평문이 BPMN으로 잡힌다.
- **Findings**: 스키마 최상위 키는 다섯(`title`·`orientation`·`participants`·`nodes`·`flows`)뿐이다. 들여쓰기
  0인 내용 줄이 전부 이 다섯 중 하나의 `키:` 줄이고 구조 키(`participants`·`nodes`·`flows`)가 하나 이상이면
  스키마 밖 YAML(`process:`)·`biz-process.md`(`## L1`)·평문(`키:` 아님)·PlantUML(`class A`·`@startuml`)·mermaid
  (`flowchart`)는 모두 거짓이 된다. `title:` 한 줄만 있는 문서도 거짓(구조 키 없음).
- **Implications**: 스니핑은 열 0 줄만 훑고 문서를 파싱하지 않는다 — `parse_xml::looks_like_bpmn`이 첫 시작
  태그만 렉싱하는 것과 같은 수위.

### 컨테이너 표현 — 중첩 vs 리터럴 `container:` 필드
- **Context**: "Model을 그대로 옮긴다"를 문자 그대로 하면 노드마다 `container: lane-sales`를 쓰게 된다.
- **Findings**: XML도 소속을 리터럴로 쓰지 않고 `laneSet/lane/flowNodeRef`에서 파생한다(`bpmn-xml` 3.3). 중첩
  (`participants[].lanes[].nodes[]`)은 XML의 파생 규칙과 같은 정보를 더 짧게 담고, 손으로 쓸 때 소속 오타를
  구조적으로 막는다.
- **Implications**: 노드 위치가 `container`를 정한다(참여자 직속 → 참여자 id, 레인 안 → 그 레인 id, 최상위
  `nodes:` → `None`). 경계 이벤트는 XML과 같이 호스트의 `container`를 따른다.

### default 흐름 지정 — 흐름 id가 없다
- **Context**: XML은 `default="f1"`로 흐름 id를 가리키지만 YAML 흐름은 문자열 한 줄이라 id가 없다.
- **Findings**: 한 노드에서 같은 대상으로 가는 시퀀스 흐름이 둘인 문서는 BPMN 의미상 무의미하므로 "대상 노드
  id"로 흐름을 유일하게 지목할 수 있다. 그 흐름이 없으면 오타이므로 오류.
- **Implications**: 확장형 노드의 `default: <대상 노드 id>` → 그 노드 → 대상 시퀀스 흐름의 `is_default = true`.

### `-.->`의 의미 — 메시지 흐름과 데이터 연관
- **Context**: mermaid 점선 화살은 하나인데 `Model`의 점선 화살 흐름은 `Message`·`DataAssociation` 둘이다.
- **Findings**: 데이터 연관은 한쪽 끝이 반드시 `dataObjectReference`/`dataStoreReference` 노드이고, 메시지 흐름
  끝은 흐름 노드 또는 참여자다 — 끝의 종류로 완전히 구분된다. `-.-`(머리 없음)는 `Association`.
- **Implications**: 파서가 문서 안 노드 종류만 보고 갈래를 정한다(검증은 하지 않음). 세 꼴 밖 화살(`==>`·
  `<-->`·`o--o`·`--x`)은 BPMN에 대응이 없어 오류.

### `read_link` 가시성 — 이 스펙 안인가, 별도 리팩터 스펙인가
- **Context**: orchestrator 지시 — design 단계에서 판단해 명시.
- **Findings**: 필요한 변경은 `struct Link`·필드 4개·`fn read_link`의 `pub(crate)` 승격뿐이다. 동작·테스트 변경
  0, mermaid 렌더링 바이트 불변(7.7). 별도 스펙으로 떼면 검증할 산출물이 없는 승인 게이트만 늘어난다.
- **Implications**: **이 스펙 안**에서 수행한다(`bpmn-xml`이 `diagram::mod` 판별 순서를 자기 안에서 바꾼 것과
  같은 수위). `parse_yaml`이 `mermaid::flow`를 import하는 방향만 허용하고 역방향은 금지.

### 외부 YAML 파서 검증 방법
- **Context**: `bpmn-support/research.md` 미검증 항목 — 스키마가 유효한 YAML인지 기계 검증.
- **Findings**: 문자열 리터럴 fixture는 외부 도구가 읽을 수 없다. 받아들여야 하는(긍정) fixture를 `include_str!`
  파일로 두면 같은 바이트를 dg 테스트와 PyYAML이 함께 읽는다. 거부해야 하는(부정) 입력은 유효한 YAML이어도
  dg가 거부하는 것이 설계이므로(앵커 등) 외부 검증 대상이 아니다.
- **Implications**: 긍정 fixture는 `src/diagram/bpmn/fixtures/*.yaml`(`examples/` 밖, 테스트 전용), 검증 스크립트는
  스크래치 디렉터리에서 `yaml.BaseLoader`로 읽어 같은 사실(최상위 키 순서·참여자 수·노드 수·흐름 수)을 단정한다.
  Python은 빌드·테스트 의존성이 아니라 수용 단계의 수동 확인이다(4-의존성 유지).

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| YAML 크레이트(`serde_yaml`·`yaml-rust2`) | 검증된 파서 채택 | 완전한 YAML | 전이 의존 추가로 4-의존성 원칙 위반, `serde_yaml`은 유지 종료 | **기각**(`bpmn-support/research.md` 결정 재확인) |
| dg 전용 유사 문법 | 들여쓰기 목록만 받는 자체 문법 | 파서 가장 짧음 | 아무 도구도 못 읽는 4번째 문법, "미래 mermaid" 자리 선점 | 기각(같은 문서) |
| 줄 기반 리더 → `YamlValue` 트리 → 모델 | 두 단계: 문법(YAML)과 의미(BPMN) 분리 | `bpmn-xml`과 같은 구조, 각 단계 독립 테스트, 리더는 BPMN을 모름 | 파일 둘 | **채택** |
| 리더가 바로 모델 생성(한 단계) | 줄을 읽으며 `Element` 생성 | 트리 메모리 없음 | `default` ↔ 흐름처럼 문서 순서를 넘는 참조가 있어 어차피 두 번 지남, 리더에 BPMN 지식이 섞임 | 기각 |
| 리터럴 `container:` 필드 | 노드마다 소속 id를 씀 | `Model` 필드와 1:1 | 장황, 소속 오타가 구조적으로 안 막힘, XML도 파생값 | 기각 |
| 중첩으로 소속 파생 | `participants[].lanes[].nodes[]` | 짧고 오타 불가, XML 파생 규칙과 동형 | 노드 위치 = 소속이라 풀 밖 노드는 최상위 `nodes:`로만 | **채택** |

## Design Decisions

### Decision: 스니핑 = 열 0 줄이 전부 최상위 키이고 구조 키가 하나 이상
- **Context**: 요구사항 1.1·1.3·1.4·1.6(위 Research Log).
- **Alternatives Considered**: 1) 첫 줄이 다섯 키 중 하나 — `title: 아무 문서`로 시작하는 평문 오발. 2) 본문에
  `participants:` 포함 — 다른 YAML(k8s 등)에 우연히 있을 수 있고 `biz-process.md`도 `participant:`를 쓴다. 3) 열
  0 줄 전부 검사 + 구조 키 필수.
- **Selected Approach**: (3). 주석·빈 줄·선두 `---`는 건너뛴다. 문서 파싱은 하지 않는다.
- **Rationale**: 스키마 최상위가 닫힌 집합이라 가능한 가장 엄격한 규칙이며 비용은 줄 훑기 한 번.
- **Trade-offs**: 최상위에 오타(`participant:`)가 있는 문서는 코드블록 — 허용(어차피 파서가 거부).

### Decision: 구조도 어휘도 엄격(모르는 키·종류·트리거·화살은 오류)
- **Context**: `bpmn-xml`은 "구조 엄격·어휘 관대"(모르는 엔티티는 원문 유지)였다.
- **Alternatives Considered**: 1) 모르는 종류 토큰의 노드를 건너뜀 — 그 노드로 가는 흐름이 끊겨 `validate`가
  어차피 문서 전체를 거부하고, 흐름이 없으면 노드가 조용히 사라진다. 2) 모르는 키 무시 — `nmae:` 오타가 이름
  없는 노드로 조용히 그려진다. 3) 전부 오류.
- **Selected Approach**: (3). YAML 문법 오류(`YamlError`)와 스키마 오류(`ParseError`) 모두 `Err` → 코드블록.
  스칼라 안 내용(이름 본문)에는 어떤 검사도 없다(관대함이 필요한 곳은 여기뿐).
- **Rationale**: XML은 기계가 쓰고 YAML은 사람이 쓴다 — 사람의 오타를 조용히 넘기면 틀린 그림이 맞는 그림처럼
  보인다. 진단 UI가 없어 코드블록 폴백이 유일한 신호이므로 신호를 없애면 안 된다.
- **Trade-offs**: 진단 문자열이 화면에 안 나온다(기존 규약). **사람이 확인할 결정(tasks 게이트)**.
- **Follow-up**: `ParseError`는 `Display`까지 갖춰 두어 진단 UI 스펙이 오면 그대로 노출할 수 있게.

### Decision: 노드 종류 표를 `vocabulary`로 올려 두 파서가 공유
- **Context**: Key Findings — 13개 로컬 이름 대응이 `parse_xml` 안 `match`에 있다.
- **Selected Approach**: `vocabulary::element_kind_from_xml_name(token) -> Option<ElementKind>`(이벤트는
  `trigger: None`으로 돌려주고 호출자가 채움; 태스크·게이트웨이는 기존 두 표를 거침). `parse_xml::element_kind_of`는
  이 함수를 부르도록 바꾸되 결과는 동일(기존 테스트 무수정 통과, XML fixture 출력 바이트 동일 — 7.2).
- **Rationale**: `bpmn-support/research.md` "두 파서가 같은 표 하나를 보게 한다"를 노드 종류에도 적용. `match`가
  둘이면 어긋난다(`vocabulary.rs` 모듈 주석의 원칙 그대로).
- **Trade-offs**: `bpmn-xml` 소유 파일을 한 함수 수정 — 동작 불변 리팩터로 한정하고 7.2로 못박는다.

### Decision: 흐름 항목 = `id 공백 화살 공백 id`, 화살은 `read_link` 그대로
- **Context**: `read_link`는 mermaid 문장 안 커서 위치에서 화살을 읽는다. id 읽기(`read_node`)는 mermaid 도형
  괄호·`:::` 꾸밈까지 읽어 YAML에는 과하다.
- **Alternatives Considered**: 1) `read_node`도 열어 재사용 — 도형 괄호를 받아들이게 되어 스키마가 흔들림.
  2) id를 mermaid와 같은 문자 집합으로 다시 정의 — 규칙 중복. 3) id = 공백으로 끊는 토큰, 그 사이를 `read_link`가
  읽음.
- **Selected Approach**: (3). 첫 공백 토큰 → `read_link` → 다음 공백 토큰 → 남은 글이 있으면 오류. 화살 앞뒤 공백
  필수(`a-->b`는 오류, 5.7). id에 공백 외 어떤 문자든 허용(한글 id 포함).
- **Rationale**: 규칙이 한 줄로 설명되고 `read_link`의 라벨 두 형태(`-- 글 -->`·`-->|글|`)가 공짜로 따라온다.
- **Trade-offs**: `a --> b --> c` 사슬은 "남은 글" 규칙으로 오류(범위 밖 명시). `-- 글 -->`의 글은 mermaid
  `label()` 정리를 받는다(따옴표 제거·`<br>` 줄바꿈).

### Decision: `orientation:`은 받되 `.yaml` 확장자는 판별에 쓰지 않음
- **Selected Approach**: `orientation: horizontal|vertical`(정확 일치) → `Model.orientation`; 명령줄 방향 옵션은
  기존 `apply_to_graph`가 덮는다(3.8). `language_of_path`에 `.yaml`·`.yml`을 더하지 않는다(1.5).
- **Rationale**: `Model`에 있는 필드는 옮긴다(대안 직렬화). 확장자는 범용 YAML을 BPMN으로 오판하므로 본문
  스니핑만 — `.bpmn`이 XML 전용 확장자인 것과 달리 `.yaml`은 BPMN 전용이 아니다.

### Decision: 합성 id `@bpmn-yaml:{n}`, 미리 거르는 흐름 없음
- **Selected Approach**: 흐름 id는 문서 순서 번호로 합성(`bpmn-xml`의 `@bpmn-xml:{n}` 관례). XML과 달리 흐름을
  미리 거르지 않는다 — YAML에는 `dataInput`·`property` 같은 "문서에 있지만 노드가 아닌" 항목이 없다. 레인 id를
  끝으로 쓴 흐름은 그대로 두어 `validate`가 거부(5.9), 최상위 참여자 id 끝은 `Model`이 받아들인다(5.10).

## Risks & Mitigations
- 들여쓰기 규칙(시퀀스 항목 안 매핑의 열 = 첫 키의 열)을 잘못 구현하면 PyYAML과 트리가 어긋남 → 7.5의 외부
  검증을 긍정 fixture 전부에 돌리고, 폭 2/4/혼합 fixture(2.1)로 못박기
- 스니핑이 PlantUML `class A` 같은 소스를 가로챌 위험 → `diagram::mod` 기존 robustness PlantUML·mermaid 배열
  전부에 `language_of_source` 불변 확인(1.6)
- `read_link`가 mermaid 쪽 요구로 바뀌면 YAML 흐름 해석이 따라 바뀜 → Revalidation Trigger로 명시, 흐름 fixture
  가 세 꼴 + 라벨 두 형태를 전부 덮음
- `element_kind_from_xml_name` 승격 중 XML 매핑이 어긋날 위험 → `parse_xml` 기존 테스트 무수정 + XML fixture
  출력 바이트 동일(7.2)

## References
- `../bpmn-support/research.md` §"[2026-09-25 리서치 완료, fable]"(파싱 방식·문법 범위·표기·파일 구성 SSoT),
  §"[2026-09-26 갱신]"(XML 먼저·YAML은 대안 직렬화)
- `../bpmn-xml/{design,research}.md` — 두 단계 파서 구조, 합성 id·경계 이벤트 소속·캡션 종류 관례
- `../bpmn-model/design.md` — `Model` 계약, `vocabulary` 표 원칙
- YAML 1.2.2 §6.1(들여쓰기 공간)·§6.6(주석 — 공백 뒤 `#`)·§7.3(흐름 스칼라 — 따옴표 이스케이프)·§8.2(블록
  컬렉션) — 받아들이는 하위집합의 근거; §6.9(노드 속성 — 앵커·태그)·§7.4(흐름 컬렉션)·§8.1(블록 스칼라)·§9.1
  (문서 — `---`·`...`)은 거부 범위
- `src/diagram/mermaid/flow.rs:13-18, 176-261`(`Link`·`read_link`), `src/diagram/mermaid/text.rs:39-60`(`label`),
  `src/diagram/bpmn/parse_xml.rs:225-245`(`element_kind_of`)
