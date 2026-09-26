# Implementation Plan — bpmn-xml

## 정의
BPMN 도구가 내보낸 BPMN 2.0 XML을 새 크레이트 없이 읽어 이미 있는 BPMN 의미 모델·검증·레인 배치·도형
어휘로 그리기 위해, 문자 단위 XML 토크나이저·트리 빌더와 BPMN 의미 트리 → 모델 빌더를 만들고 스텁으로
남아 있던 BPMN 갈래 판별·렌더링 진입점을 실제로 채우는 기능이다. 검증용 문서는 전부 테스트 코드 안
문자열 리터럴이며 `examples/` 파일은 추가하지 않는다.

- [x] 1. 기반: 회귀 기준선과 XML 토크나이저
- [x] 1.1 (P) 기존 출력의 바이트 기준선 채취
  - 코드를 손대기 전에 `examples/*.md` 전부를 `dg -P --width 100`으로 렌더링해 스크래치 디렉터리에 저장
    (4.3의 diff 기준). 기준선 테스트 수(373)도 함께 기록
  - DONE: 예제 파일마다 기준 출력 파일이 하나씩 있고, 같은 명령을 다시 돌려도 같은 내용
  - _Requirements: 7.1_
  - _Difficulty: low_
  - _Boundary: 검증_

- [x] 1.2 (P) XML 토크나이저 1 — 요소 트리·텍스트·엔티티·접두사
  - design §Components "bpmn::xml"의 요소·시작 태그·오류 타입을 그대로 만들고, 문자 단위 렉서로 시작/끝/자기
    닫힘 태그, 홑·겹따옴표 속성(중복 이름은 첫 값), 직접 텍스트 + CDATA(문서 순서로 이어붙여 양끝 공백 제거,
    자식 요소 텍스트는 제외), `<?…?>` 선언·`<!DOCTYPE …>`·주석(안에 `<`·`>` 있어도) 건너뛰기를 구현한다.
    요소 이름은 첫 `:`까지 접두사를 벗기고, 속성 이름은 쓴 그대로 둔다(`xmlns:bpmn`·`xsi:type` 포함)
  - 엔티티는 `&lt; &gt; &amp; &quot; &apos;`와 `&#N;`·`&#xH;`만 디코딩(CDATA 밖 텍스트·속성 값), 그 외 이름
    (`&nbsp;`)과 잘못된 참조(`&#;`·`&#xD800;`·범위 초과)는 원문 그대로 남긴다. 트리 생성은 명시적 스택(재귀 없음),
    문자 경계는 `char_indices`·`str` 슬라이스만 사용
  - DONE: 단위 테스트에서 같은 문서를 접두사 없이/`bpmn:`/`bpmn2:`/`semantic:`으로 쓴 네 입력이 같은 트리
    (2.1); 홑·겹따옴표 속성의 `&amp;`·`&lt;`·`&gt;`·`&quot;`·`&apos;`·`&#10;`·`&#x2014;`가 `&`·`<`·`>`·`"`·`'`·
    줄바꿈·`—`로 디코딩(2.2); 선언·DOCTYPE·주석·요소 사이 개행이 있는 문서와 없는 문서의 트리가 같음(2.3);
    CDATA 안 `<`·`&amp;`가 그대로 텍스트로 남음(2.4); `&nbsp;`·`&#;`·`&#xD800;`·`&#99999999999;`가 원문 유지
    (2.7); `<a/>`와 `<a></a>`가 같은 트리(2.8); 속성·자식·텍스트 조회 도우미가 첫 일치를 돌려줌
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.7, 2.8_
  - _Difficulty: high_
  - _Boundary: bpmn::xml_

- [x] 1.3 XML 토크나이저 2 — 불투명 구획·깊이 상한·구조 오류·첫 시작 태그
  - 호출자가 준 로컬 이름 목록의 요소는 같은 렉서로 시작·끝 태그 균형만 세어(그 안의 주석·CDATA·같은 이름
    중첩도 정상 인식) 자식을 만들지 않고 건너뛴다. 중첩 깊이가 64를 넘으면 깊이 오류. 태그·따옴표·주석·CDATA·
    선언이 열린 채 끝남·요소 미닫힘은 끝 오류, 이름 없는 태그·값 없는 속성은 위치 포함 형식 오류, 닫는 태그
    불일치는 기대/실제 이름 포함 오류, 요소가 하나도 없으면 루트 없음 오류. 루트가 닫힌 뒤 내용은 무시
  - 프롤로그·주석·DOCTYPE만 건너뛰고 첫 시작 태그 하나(이름·속성)만 돌려주는 진입점을 따로 둔다(문서 전체를
    파싱하지 않음)
  - DONE: 단위 테스트에서 불투명 이름을 준 요소가 트리에 없고 그 안의 주석 속 가짜 닫는 태그·CDATA·같은 이름
    중첩에도 형제 요소가 올바르게 이어짐(2.5, 2.6); 태그 중간 잘림·요소 열린 채 끝·닫는 태그 불일치·따옴표/
    주석/CDATA/선언 미종결이 각각 해당 오류 변형(6.1~6.3); 깊이 65 → 깊이 오류, 64 → 성공(6.4); 빈 문자열·공백·
    `<` 한 글자 → 루트 없음 또는 끝 오류(6.5); 첫 시작 태그 진입점이 `<?xml …?><!-- c --><bpmn:definitions
    xmlns:bpmn="…">`에서 이름 `definitions`와 `xmlns:bpmn` 속성을 돌려주고, 태그가 없거나 망가지면 없음(1.1)
  - _Requirements: 1.1, 2.5, 2.6, 6.1, 6.2, 6.3, 6.4, 6.5_
  - _Difficulty: high_
  - _Boundary: bpmn::xml_
  - _Depends: 1.2_

- [x] 2. 핵심: BPMN 의미 트리 → 모델
- [x] 2.1 BPMN 스니핑 판정
  - 첫 시작 태그만 보고 판정: 로컬 이름이 `definitions`이면 `xmlns`로 시작하는 속성 중 값에 `omg.org/spec/BPMN`이
    있거나 `xmlns` 속성이 하나도 없어야 참, `process`·`collaboration`이면 참, 그 외 거짓
  - DONE: 단위 테스트에서 bpmn.io식 네임스페이스 선언 `definitions`·선언 없는 `definitions`·`process`·
    `collaboration` 루트 → 참(1.1); WSDL(`xmlns="http://schemas.xmlsoap.org/wsdl/"`) `definitions`·`<html>`·
    Maven `<project>` → 거짓(1.4); YAML 조각·`biz-process.md` 조각·평문·빈 문자열 → 거짓(1.5)
  - _Requirements: 1.1, 1.4, 1.5_
  - _Difficulty: mid_
  - _Boundary: bpmn::parse_xml_
  - _Depends: 1.3_

- [x] 2.2 (P) 구조 — 협업·참여자·프로세스·레인·소속·제목
  - design "bpmn::parse_xml" 빌드 순서 1~4·8: 루트(`definitions` 또는 `process`/`collaboration` 자체, 그 외는
    루트 오류) → 참여자(이름·`processRef` → 그 프로세스의 소속 = 참여자 id, `processRef` 없으면 레인 없는
    참여자) → 참여자가 가리키지 않는 프로세스는 레인 집합이 있으면 프로세스 id·이름으로 참여자 합성, 없으면
    소속 없음 → 레인 집합/레인/하위 레인 집합 재귀로 레인 트리, `flowNodeRef` 텍스트로 노드 id → 가장 안쪽
    레인 id 대응표 → 제목은 협업 이름 → 유일한 프로세스 이름 → 정의 이름 → 없음. 불투명 구획 이름 목록
    (`BPMNDiagram`·`extensionElements`)을 토크나이저에 넘긴다
  - DONE: 단위 테스트에서 참여자 2개 + `processRef` → 참여자 순서·이름 그대로(3.1); `processRef` 없는 참여자 →
    레인 없이 존재(3.2); `laneSet/lane/childLaneSet/lane` + `flowNodeRef` → 레인 트리와 노드 소속이 안쪽 레인
    (3.3), 미나열 노드 소속이 참여자 id(3.6); 협업 없는 단일 프로세스 → 참여자 없음·소속 없음(3.4), 레인
    있으면 합성 참여자 하나(3.5); 제목 우선순위 3단과 셋 다 없음(3.7); `bpmndi:BPMNDiagram`이 있는 문서와 없는
    문서의 모델이 같음(2.5)
  - _Requirements: 2.5, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_
  - _Difficulty: high_
  - _Boundary: bpmn::parse_xml_
  - _Depends: 1.3_

- [x] 2.3 (P) 노드 — 요소 표·트리거·경계 이벤트·서브프로세스·데이터·주석·무시
  - design "bpmn::parse_xml" 요소 표대로 프로세스 직접 자식을 요소로: 이벤트 4위치(트리거는 해석되는
    `*EventDefinition` 자식 0개 → 없음, 1개 → 그것, 2개 이상 → `parallelMultiple="true"`면 병렬 다중 아니면
    다중), 태스크(어휘 표 토큰 함수로), `subProcess`·`adHocSubProcess`·`transaction` → 접힌 서브프로세스(자식
    노드·흐름 무시), 호출 활동, 게이트웨이(어휘 표), `dataObjectReference`/`dataStoreReference`, 텍스트 주석
    (`text` 자식 본문). `boundaryEvent`의 `attachedToRef` → 부착, 전부 만든 뒤 경계 이벤트 소속을 호스트 소속으로
    덮어씀. 이름은 `@name` 디코딩 후 양끝 공백 제거. 그 외 자식은 노드로 만들지 않고 `id`가 있으면 "노드 아닌
    id" 집합에 모음. id 없는 노드는 `@bpmn-xml:{n}` 합성
  - DONE: 단위 테스트에서 요소 표 전 행이 해당 종류(4.1, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9); 트리거 0/1/2/2+
    `parallelMultiple` 네 경우(4.2); `boundaryEvent` → 부착 대상 설정·소속 = 호스트 소속(레인 나열이 달라도)
    (4.3); `subProcess` 안 노드·흐름이 결과에 없음(4.5); `dataObject`·`dataStore` 정의가 노드가 아니고 "노드 아닌
    id"에 있음(4.8); `group`·`ioSpecification`·`property`·`documentation`·vendor 요소가 무시되고 나머지 노드는
    있음(4.10); `@name` 없음 → 빈 이름(4.11)
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 4.11_
  - _Difficulty: high_
  - _Boundary: bpmn::parse_xml_
  - _Depends: 1.3_

- [x] 2.4 흐름 — 시퀀스·default·메시지·연관·데이터 연관·미리 거르기
  - `sequenceFlow`(라벨 = `@name`, default = 어떤 노드의 `@default` 값과 id 일치 — 게이트웨이·활동 모두),
    협업의 `messageFlow`, 프로세스의 `association`, 활동 자식 `dataInputAssociation`(`sourceRef` 텍스트 → 활동)·
    `dataOutputAssociation`(활동 → `targetRef` 텍스트)을 흐름으로. **끝이 최상위 참여자 id면 그대로 둔다**
    (`bpmn::Model`이 받아들임, `bpmn-message-flow-participant-endpoint`). 끝이 그 밖의 "노드 아닌 id"(레인·
    정의 요소·`dataInput`·`dataOutput`·`property` …)인 흐름만 버리고, 문서 어디에도 없는 id는 그대로 둔다
    (검증이 거부). id 없는 흐름은 `@bpmn-xml:{n}` 합성
  - DONE: 단위 테스트에서 이름 있는 시퀀스 흐름의 라벨(5.1); 게이트웨이·활동의 `default`가 가리킨 흐름만
    default(5.2); `conditionExpression`만 있는 흐름은 라벨 없음(5.3); `messageFlow` → 메시지 흐름(5.4), 끝이
    참여자 id면 그대로 `Flow`에 남아 정상 렌더링(5.5), 끝이 레인 id면 그 흐름만 없음(5.5a); `association` →
    연관, 입력/출력 데이터 연관의 방향(객체 → 활동 / 활동 →
    객체)(5.6), 끝이 `property` id면 그 연관만 없음(5.7); 문서에 없는 id를 가리키는 흐름은 남아 있음(5.8);
    id 없는 흐름 둘의 합성 id가 서로 다름
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.5a, 5.6, 5.7, 5.8_
  - _Difficulty: high_
  - _Boundary: bpmn::parse_xml_
  - _Depends: 2.2, 2.3_

- [x] 3. 배선
- [x] 3.1 BPMN 언어 진입점의 `xml` 갈래
  - 갈래 판별을 스니핑 판정으로(참이면 `"xml"`), 소스 렌더링을 `"xml"` 팔 → 파서 → 실패면 없음 → 모델 렌더링
    으로 채운다. 캡션 종류는 모델 렌더링이 준 `process`/`collaboration`. 공용 XML fixture(bpmn.io 형식 주문 처리
    협업 문서: `<?xml` 선언, `bpmn:` 접두사, `xmlns:*` 5개, `incoming`/`outgoing`, `&#10;` 이름, `conditionExpression`,
    `bpmndi:BPMNDiagram` 구획)를 테스트 전용 모듈의 문자열 리터럴로 둔다. 기존 "항상 None" 테스트는 새
    동작으로 갱신
  - DONE: 단위 테스트에서 `<definitions></definitions>` → 갈래 `"xml"`이되 렌더링은 없음(6.5); YAML 조각·평문 →
    갈래 없음(1.5); 접두사 없는 `definitions` + 단일 `process` 3노드 → `Some(("process", _))`이고 결과에 풀 띠
    글자(`┃`)가 없음(3.4); fixture → `Some(("collaboration", _))`(3.1); 파서 오류(잘린 fixture) → 없음(6.7)
  - _Requirements: 1.1, 1.5, 3.1, 3.4, 6.5, 6.7_
  - _Difficulty: mid_
  - _Boundary: bpmn::mod_
  - _Depends: 2.1, 2.4_

- [x] 3.2 언어 판별 순서 — BPMN 스니핑을 PlantUML 앞으로
  - 확장자·펜스 없는 소스의 언어 추정 순서를 확장자 → `@start` → mermaid → BPMN → PlantUML로 바꾼다(PlantUML
    판별이 점수 없으면 클래스로 보는 포괄 판별이라 뒤에 있어야 함). 확장자 `.bpmn`은 그대로 본문 무관
  - DONE: 테스트에서 fixture(`.bpmn` 경로 없음) → BPMN(1.3); 경로 `x.bpmn` + 임의 본문 → BPMN(1.2); WSDL·
    HTML 문서 → BPMN 아님(1.4); 기존 이상 입력 테스트의 PlantUML 배열 전부와 `@start` 없는 클래스 조각(`class A`)이
    이전과 같이 PlantUML, mermaid 배열 전부 mermaid(1.6)
  - _Requirements: 1.2, 1.3, 1.4, 1.6_
  - _Difficulty: mid_
  - _Boundary: diagram::mod_
  - _Depends: 3.1_

- [x] 4. 검증
- [x] 4.1 bpmn.io 형식 fixture 통합 렌더링
  - 3.1의 fixture를 폭 100으로 렌더링해 단정한다; 드러나는 결함은 이 태스크 안에서 고친다(파서 쪽이면 2.x,
    모델 이후는 재검토 후 `bpmn-model` 결함으로 기록)
  - DONE: 통합 테스트에서 fixture → 종류 `collaboration`, 결과 텍스트에 `○`·`●`·`◎`·`┃`·`«user»`·`«service»`·
    `«message»`·`«error»`·`× 재고 있음?`·`╱`·`╌`가 모두 있고, 파싱된 모델의 노드 수·흐름 수가 기존 손 모델
    (`bpmn-model` 8.3)과 같음(7.3, 3.1, 4.3, 5.2, 5.4); `bpmndi` 구획을 지운 같은 fixture와 출력 바이트 동일(2.5);
    `&#10;` 이름이 두 줄로 보임(2.2); `{text}` 육안 확인(design 표대로 풀 두 띠·레인 두 띠·경계 점선)
  - _Requirements: 2.2, 2.5, 3.1, 4.3, 5.2, 5.4, 7.3_
  - _Difficulty: mid_
  - _Boundary: bpmn::mod_
  - _Depends: 3.1_

- [x] 4.2 (P) 손상·비BPMN 입력 강건성과 마크다운 펜스 폴백
  - 기존 이상 입력 패닉 테스트의 BPMN 배열을 둘로 나눈다 — 반드시 없음: 빈 문자열·공백·`<`·`<definitions/>`·
    태그 중간 잘림·요소 열린 채 끝·닫는 태그 불일치·따옴표/주석/CDATA/선언 미종결·깊이 100 중첩·WSDL·YAML 조각·
    평문·풀을 넘는 시퀀스 흐름·중복 id; 패닉만 없음: fixture들·`&nbsp;` 이름·id 없는 흐름·이벤트 정의 2개·
    `subProcess` 안 노드. 폭 8·20·40·80·200. 마크다운의 기존 "파서 없는 bpmn 펜스" 테스트를 "노드 없는 definitions"·
    "손상 XML" 두 경우로 갱신
  - DONE: 반드시-없음 배열 전부 × 폭 5종이 `None`(6.1~6.5, 5.8, 1.4, 1.5); 패닉만-없음 배열 × 폭 5종 통과(6.6);
    마크다운 문서의 손상 `bpmn` 펜스가 코드블록(`╭─ bpmn `)이고 앞뒤 문단이 그대로(6.7)
  - _Requirements: 1.4, 1.5, 5.8, 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_
  - _Difficulty: mid_
  - _Boundary: diagram::mod, markdown 테스트_
  - _Depends: 3.2_

- [x] 4.3 전체 스위트 + clippy + 바이트 동일 회귀 + 의존성 수 + 실물 실행
  - DONE: `cargo test` 전량 통과(기준선 373개 중 의도적으로 갱신한 두 테스트 외 무수정 통과 + 신규), `cargo clippy
    --all-targets -- -D warnings` 클린, `examples/*.md`를 1.1과 같은 명령으로 렌더링해 기준선과 `diff` 무차이(7.1),
    `Cargo.toml` 의존성 4개 그대로(7.2), fixture를 스크래치 디렉터리의 `.bpmn` 파일로 써서 `dg <파일>`과
    `cat <파일> | dg -d` 둘 다 `◈ bpmn · collaboration` 캡션으로 시작하는 그림 출력(7.4, 1.2, 1.3), 4.1의 `{text}`
    육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 4.11, 5.1, 5.2, 5.3, 5.4, 5.5, 5.5a, 5.6, 5.7, 5.8, 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 7.1, 7.2, 7.3, 7.4_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 1.1, 4.1, 4.2_
