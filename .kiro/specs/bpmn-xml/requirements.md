# Requirements — bpmn-xml

## 정의
BPMN 도구(Camunda Modeler·bpmn.io·Flowable 등)가 내보낸 BPMN 2.0 XML 파일이나 `bpmn`
코드펜스 본문을 dg가 새 외부 의존성 없이 읽어, 이미 구현된 BPMN 의미 모델·검증·
레인 배치·도형 어휘(`bpmn-model`)로 문자 그림을 그리는 첫 번째 BPMN 입력 문법
기능이다. OMG BPMN 2.0.2 Descriptive 적합성(Level 1) 하위집합을 표준 그대로 읽어
의미 모델의 표준 충실도를 검증하는 기준이 된다.

## Boundary Context
- **In scope**: BPMN XML 문서의 판별(코드펜스·확장자·본문 스니핑), 네임스페이스
  접두사·주석·CDATA·엔티티·XML 선언이 섞인 실제 도구 출력 읽기, 협업(참여자·메시지
  흐름)과 프로세스(레인 집합·중첩 레인·노드 소속), Descriptive 하위집합의 흐름 노드
  (이벤트 4위치 + 트리거, 태스크 8종, 접힌 서브프로세스, 호출 활동, 게이트웨이 5종),
  데이터 객체·데이터 저장소·텍스트 주석, 시퀀스 흐름(라벨·default)·메시지 흐름·연관·
  데이터 연관, 경계 이벤트 부착, 다이어그램 교환 구획과 도구 확장 구획의 무시, 손상·
  비BPMN 입력에 대한 패닉 없는 원문 코드블록 폴백, 기존 언어(mermaid·PlantUML)
  판별·렌더링 불변
- **Out of scope**: 다이어그램 교환 계층(BPMNDI — 좌표·크기·경로·`isHorizontal`;
  사용자 결정 2026-09-26, 영구 제외 — 방향은 기본값과 기존 방향 옵션으로만 정함),
  BPMN YAML(`bpmn-yaml`)과 `biz-process.md`(`bizprocess-bpmn`) 문법 — 이 스펙은 두
  문법의 본문을 BPMN XML로 오판하지 않을 책임만 진다, 펼친 서브프로세스 내부 노드
  렌더링(접힌 상자로만), **레인 자체를 끝으로 하는 메시지 흐름의 렌더링**(레인은
  `MessageFlow`의 끝점이 될 수 없음 — 최상위 참여자(블랙박스 풀 포함)는
  `bpmn-message-flow-participant-endpoint`가 지원하므로 이 스펙이 `sourceRef`/
  `targetRef`를 참여자 id 그대로 `Flow`에 넣으면 렌더링된다), 조건식
  (`conditionExpression`)·문서화(`documentation`)·실행 속성
  (`isExecutable`·vendor 속성)의 표시, 범용 XML 파서(스키마·DTD·외부 엔티티·
  네임스페이스 해석), 새 도형·표식·검증 규칙, 진단 메시지 UI(실패는 기존과 같이
  원문 코드블록), `examples/` 예제 문서 추가(검증용 문서는 테스트 코드 안 문자열로만)

## Acceptance Criteria

### 1. 입력 판별
- 1.1: [`bpmn` 코드펜스 본문이 XML 선언·주석 뒤 첫 요소가 `definitions`(BPMN 네임
  스페이스 URI `omg.org/spec/BPMN`를 선언하거나 네임스페이스 선언이 전혀 없음)·
  `process`·`collaboration` 중 하나인 XML] → [BPMN 그림으로 렌더링된다]
- 1.2: [확장자 `.bpmn` 파일] → [본문 스니핑 없이 BPMN으로 판별된다(기존 동작 유지)]
- 1.3: [확장자·코드펜스 언어가 없는 입력(표준 입력·`.txt`)이 1.1의 XML] → [BPMN으로
  판별돼 그림이 나오고, PlantUML 등 다른 언어로 오판되지 않는다]
- 1.4: [BPMN이 아닌 XML(`<html>`, WSDL `<definitions xmlns="http://schemas.xmlsoap.org/wsdl/">`,
  Maven `<project>`)] → [BPMN으로 판별되지 않고 이전과 같은 결과(코드블록 또는 기존
  언어 판별)다]
- 1.5: [`bpmn` 코드펜스 본문이 XML이 아님(YAML 조각·`biz-process.md` 조각·평문·빈
  문자열)] → [원문 코드블록으로 물러난다(후속 문법 스펙의 자리)]
- 1.6: [항상 + mermaid·PlantUML 소스] → [언어 판별 결과가 이 기능 이전과 동일하다]

### 2. 실제 도구 출력 읽기(XML 표면 문법)
- 2.1: [같은 내용을 접두사 없이 / `bpmn:` / `bpmn2:` / `semantic:` 접두사로 쓴 문서] →
  [네 문서의 그림이 동일하다]
- 2.2: [속성 값이 홑따옴표 또는 겹따옴표, 안에 `&amp;`·`&lt;`·`&gt;`·`&quot;`·`&apos;`·
  `&#10;`·`&#x2014;` 참조 포함] → [노드 이름에 `&`·`<`·`>`·`"`·`'`·줄바꿈·`—`로 디코딩되어
  보이고, `&#10;`는 라벨의 둘째 줄이 된다]
- 2.3: [`<?xml …?>` 선언·`<!DOCTYPE …>`·주석(`<!-- -->`, 안에 `<`·`>` 포함)·요소 사이
  공백/개행이 있는 문서] → [없는 문서와 그림이 동일하다]
- 2.4: [텍스트 주석의 `<text>` 본문이 평문 또는 CDATA(`<![CDATA[ … ]]>`, 안에 `<`
  포함)] → [본문이 그대로 점선 상자 안에 보인다]
- 2.5: [`bpmndi:BPMNDiagram` 구획(좌표·waypoint 수백 줄, 안에 주석·CDATA 포함)이
  있는 문서] → [구획을 지운 문서와 그림이 동일하고, 그림 어디에도 좌표 값이
  나타나지 않는다]
- 2.6: [`extensionElements` 안 vendor 요소(`camunda:`·`flowable:`·`zeebe:` 접두사)와
  vendor 속성] → [무시되고 그림은 동일하다]
- 2.7: [정의되지 않은 엔티티(`&nbsp;`)나 잘못된 문자 참조(`&#;`·`&#xD800;`·
  `&#99999999999;`)가 이름에 있음] → [참조 원문이 그대로 이름에 남고 문서는 정상
  렌더링된다]
- 2.8: [자기 닫힘 태그(`<startEvent id="s"/>`)와 열고 닫는 태그(`<startEvent id="s">
  </startEvent>`)] → [같은 결과다]

### 3. 협업·프로세스 구조 → 풀·레인
- 3.1: [`collaboration`에 `participant`가 둘 이상, 각각 `processRef`로 `process`를 가리킴]
  → [참여자 이름의 풀 띠가 선언 순서대로 그려지고 캡션이 `◈ bpmn · collaboration`이다]
- 3.2: [`participant`에 `processRef`가 없음(블랙박스 풀)] → [노드 없는 빈 띠와 제목이
  그려진다]
- 3.3: [`process`에 `laneSet/lane`이 있고 `lane/childLaneSet/lane`으로 하위 레인, 각
  `lane/flowNodeRef`가 노드 id를 나열] → [부모 띠 안에 자식 띠가 선언 순서대로
  중첩되고, 노드는 자기를 나열한 가장 안쪽 레인 안에 놓인다]
- 3.4: [`collaboration` 없이 `process` 하나, 레인 없음] → [띠 없는 평면 그림이고 캡션이
  `◈ bpmn · process`다]
- 3.5: [`collaboration` 없이 `process`에 `laneSet`이 있음] → [프로세스 이름의 풀 하나 안에
  레인이 그려진다]
- 3.6: [레인이 있는 프로세스에서 어떤 `flowNodeRef`에도 나열되지 않은 노드] → [풀
  직속(레인 밖)에 놓인다]
- 3.7: [제목] → [`collaboration/@name` → (그것이 없으면) 유일한 `process/@name` →
  `definitions/@name` 순서로 첫 비어 있지 않은 값이 그림 제목이 되고, 셋 다 없으면
  제목이 없다]

### 4. 흐름 노드 → 도형(기존 어휘 표 그대로)
- 4.1: [`startEvent` / `intermediateCatchEvent`·`intermediateThrowEvent` / `endEvent`] →
  [각각 `○`/`◎`/`●` 이벤트 상자, 종료는 굵은 테두리]("상자·굵은 테두리" 렌더링은
  `bpmn-event-shape-notation`이 대체함 — 매핑(`ElementKind::Event → Shape::Event(position)`)은
  그대로, 그리기만 테두리 없는 위치 글자 + 이름으로 바뀜)
- 4.2: [이벤트 안 `*EventDefinition` 자식이 하나(`messageEventDefinition` … 12종)] →
  [둘째 줄 `«message»`처럼 해당 트리거 라벨; 자식이 둘 이상이면 `«multiple»`,
  `parallelMultiple="true"`이면 `«parallelMultiple»`; 없으면 트리거 줄 없음]
- 4.3: [`boundaryEvent attachedToRef="…"`] → [호스트 활동 옆 다음 층에 중간 이벤트로
  놓이고 점선으로 이어지며(기존 근사), 레인 나열과 무관하게 호스트와 같은 레인에
  있다]
- 4.4: [`task`·`userTask`·`serviceTask`·`scriptTask`·`manualTask`·`businessRuleTask`·
  `sendTask`·`receiveTask`] → [둥근 상자, `task`는 이름만, 나머지는 둘째 줄 `«user»` …
  `«receive»`]
- 4.5: [`subProcess`·`adHocSubProcess`·`transaction`(안에 자식 노드·흐름 포함)] → [접힌
  서브프로세스 상자(`[+]`) 하나로 그려지고 안쪽 노드·흐름은 그려지지 않는다]
- 4.6: [`callActivity`] → [둥근 상자 + `«call»`]
- 4.7: [`exclusiveGateway`·`parallelGateway`·`inclusiveGateway`·`complexGateway`·
  `eventBasedGateway`] → [마름모 안 `×`·`+`·`○`·`*`·`◎` + 이름]
- 4.8: [`dataObjectReference` / `dataStoreReference`] → [`«data»` 직사각형 / 원통이고,
  이름은 참조 요소의 `@name`; 정의 요소 `dataObject`·`dataStore` 자체는 노드로 그려
  지지 않는다]
- 4.9: [`textAnnotation` + `text` 자식] → [점선 상자, 본문은 `text`의 내용]
- 4.10: [프로세스 안 이 스펙 밖 요소(`group`·`category`·`ioSpecification`·`property`·
  `documentation`·`dataInput`·`dataOutput`·vendor 요소)] → [무시되고 나머지 노드는 정상
  렌더링된다]
- 4.11: [`@name`이 없는 노드] → [이름 없이(이벤트는 위치 글자만·게이트웨이는 기호만)
  그려진다]

### 5. 흐름 → 선
- 5.1: [`sequenceFlow sourceRef targetRef name="예"`] → [실선 화살에 라벨 `예`]
- 5.2: [출발 노드(게이트웨이 또는 활동)의 `default="f1"`이 시퀀스 흐름 `f1`을 가리킴] →
  [그 흐름의 꼬리에 `╱`가 그려진다]
- 5.3: [`name` 없이 `conditionExpression`만 있는 시퀀스 흐름] → [라벨 없는 실선 화살
  (조건식은 표시하지 않음)]
- 5.4: [`messageFlow`의 양끝이 서로 다른 풀 안의 노드] → [점선 + `○` 꼬리 + 열린
  화살촉]
- 5.5: [`messageFlow`의 한쪽 끝이 최상위 `participant` id(블랙박스 풀 자체)] → [그
  참여자 id를 `sourceRef`/`targetRef` 그대로 흐름 끝으로 두어 정상 렌더링된다(점선
  + `○` 꼬리 + 열린 화살촉, 참여자 쪽 끝은 그 풀 띠의 보이지 않는 닻) — `bpmn-model`
  단독으로는 거부됐으나 `bpmn-message-flow-participant-endpoint`가 해결함]
- 5.5a: [`messageFlow`의 한쪽 끝이 `lane` id] → [레인은 `MessageFlow`의 끝점이 될 수
  없으므로(BPMN 2.0.2) 그 흐름만 걸러 생략하고 두 풀과 나머지 흐름은 정상
  렌더링된다]
- 5.6: [`association` / 활동 안 `dataInputAssociation`(`sourceRef` 자식 = 데이터 객체 id)·
  `dataOutputAssociation`(`targetRef` 자식 = 데이터 객체 id)] → [점선 무표식 / 데이터
  객체와 활동 사이 점선 + 열린 화살촉(입력은 객체 → 활동, 출력은 활동 → 객체)]
- 5.7: [데이터 연관의 끝이 노드가 아닌 항목(`dataInput`·`dataOutput`·`property` id)] →
  [그 연관만 그려지지 않고 나머지는 정상이다]
- 5.8: [문서가 BPMN 구조 규칙을 어김(풀을 넘는 시퀀스 흐름·없는 id 참조·중복 id)] →
  [기존 검증 규칙대로 거부돼 원문 코드블록으로 물러난다]

### 6. 손상 입력 강건성
- 6.1: [태그 중간이나 요소가 열린 채로 잘린 문서] → [패닉 없이 원문 코드블록]
- 6.2: [닫는 태그 이름이 여는 태그와 다름] → [패닉 없이 원문 코드블록]
- 6.3: [속성 따옴표·주석·CDATA·`<?xml` 선언이 닫히지 않음] → [패닉 없이 원문 코드블록]
- 6.4: [요소 중첩 깊이가 64를 넘는 문서] → [패닉·스택 넘침 없이 원문 코드블록]
- 6.5: [빈 문자열·공백만·`<` 한 글자·`<definitions/>`처럼 노드가 하나도 없는 유효 문서]
  → [패닉 없이 원문 코드블록]
- 6.6: [1.1~6.5의 어떤 입력 + 폭 8·20·40·80·200] → [패닉이 없다]
- 6.7: [마크다운 문서 안 `bpmn` 코드펜스의 파싱이 실패함] → [그 펜스만 코드블록이 되고
  앞뒤 문단은 정상 렌더링된다]

### 7. 회귀·실물 확인
- 7.1: [항상 + mermaid·PlantUML 코드펜스] → [렌더링 결과가 이 기능 이전과 바이트
  단위로 동일하다]
- 7.2: [항상] → [외부 의존성이 늘지 않는다(`Cargo.toml`의 의존성 4개 그대로)]
- 7.3: [bpmn.io 형식 그대로 쓴 주문 처리 협업 문서(풀 `고객`·`판매사`, 레인 `영업`·
  `창고`, `«message»` 시작, `«user»`/`«service»` 태스크, `×` 게이트웨이 + default 흐름,
  경계 `«error»` 이벤트, 메시지 흐름, `bpmndi:BPMNDiagram` 구획 포함)] → [폭 100
  안에서 렌더링되고 `○ ● ◎ ┃ «user» «service» «message» «error» × ╱ ╌`가 모두 보이며,
  같은 내용의 손 모델(`bpmn-model` 8.3)과 노드·흐름 수가 같다]
- 7.4: [`dg <파일>.bpmn` 및 `cat <파일>.bpmn | dg -d`] → [둘 다 그림이 나오고 캡션이
  `◈ bpmn · …`으로 시작한다]
