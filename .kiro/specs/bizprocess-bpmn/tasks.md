# Implementation Plan — bizprocess-bpmn

## 정의
`biz-process.md`를 쓰는 사용자가 그 문서를 BPMN 드릴다운 그림으로 보게 하기 위해, 헤딩 계층·괄호 태그·`Logic(AST)`를
읽는 개요 리더와 그것을 중첩 정보가 있는 BPMN 의미 모델로 옮기는 빌더, 정책에 따라 한 모델을 여러 장으로 내리는
변환, `bizprocess` 언어 배선을 만드는 기능이다. 회귀 픽스처는 저장소의 실제 `biz-process.md` 7개(원본 경로
`include_str!`)와 테스트 전용 합성 fixture이며 `examples/` 파일은 추가하지 않는다.

- [x] 1. 기반: 회귀 기준선·그룹 선 종류·모델 중첩·정책 옵션·검증 규칙
- [x] 1.1 (P) 기존 출력의 바이트 기준선 채취
  - 코드를 손대기 전에 `examples/*.md` 전부와 BPMN XML·YAML fixture를 `dg -P --width 100`으로 렌더링해 스크래치
    디렉터리에 저장(6.5의 diff 기준). 기준선 테스트 수(488 + doc 2)도 기록
  - DONE: 입력마다 기준 출력 파일이 하나씩 있고 같은 명령을 다시 돌려도 같은 내용
  - _Requirements: 7.6_
  - _Difficulty: low_
  - _Boundary: 검증_

- [x] 1.2 (P) 그룹 테두리 선 종류 + 레인 안 상자 중첩 spike
  - design §"ir · layout::graph"대로 그래프 그룹에 선 종류(기본 실선)를 두고 설정 함수를 더하며, 그룹 상자 그리기가
    그 선 종류를 쓰게 한다(레인은 실선 그대로). 손으로 만든 그래프로 레인 > 파선 상자 > 실선 상자 > 노드 2단 중첩과
    레인 > 파선 상자 > 노드를 LR·TB로 렌더링해 상자가 레인 안에 놓이고 파선 글자로 그려지는지 실측한다. 어긋나면
    배치기를 최소 수정하되 레인 없는 기존 테스트가 전부 무수정 통과해야 한다
  - DONE: 단위 테스트에서 파선 그룹 상자의 테두리에 대시 글자(`╌`/`╎`)가 있고 실선 그룹은 이전과 같은 글자(4.3);
    레인 안 2단 중첩 상자의 노드가 레인 띠 안쪽 행·열에 있음; 기존 `layout::graph` 테스트 전부 무수정 통과(7.6)
  - _Requirements: 4.3, 7.6_
  - _Difficulty: high_
  - _Boundary: ir, layout::graph_

- [x] 1.3 (P) 모델 중첩·그룹 확장(기존 파서 동작 불변)
  - design §"bpmn::model"대로 요소에 부모(품은 Sub-Process id) 필드, 모델에 Group 아티팩트 목록(id·이름·부모·멤버)을
    더하고 자식 조회·깊이 조회 도우미를 만든다. XML·YAML 파서와 기존 테스트의 요소·모델 리터럴은 부모 없음·그룹
    없음으로 채운다
  - DONE: 단위 테스트에서 자식 조회가 선언 순서를 지키고 깊이 조회가 부모 사슬 길이(상한 64)를 돌려줌; XML·YAML
    파서의 기존 단위 테스트 전부 무수정 통과, 두 fixture의 모델이 부모 없음·그룹 없음(7.6)
  - _Requirements: 4.1, 4.3, 4.4, 7.6_
  - _Difficulty: mid_
  - _Boundary: bpmn::model, bpmn::parse_xml, bpmn::parse_yaml_

- [x] 1.4 (P) 정책 옵션 `depth`
  - design §"diagram::options"대로 펼침 정책 값(`Depth(n)`·`PerActivity`(기본)·`All`)과 문자열 판별(`all`·`activity`·
    음이 아닌 정수, 공백·대소문자 무시), 옵션 필드, 지시자 키 `depth`, 마크다운 주석 꼴 지시자(`<!-- dg: … -->`)를
    더한다. 기존 지시자 꼴은 그대로
  - DONE: 단위 테스트에서 `all`/`activity`/`0`/`2`/` All `이 각 값, `deep`·`-1`·빈 문자열이 없음(6.6); `<!-- dg:
    depth=all -->`과 `%% dg: depth=1`이 옵션을 덮음; 기존 지시자 테스트 무수정 통과
  - _Requirements: 6.1, 6.2, 6.3, 6.6_
  - _Difficulty: low_
  - _Boundary: diagram::options_

- [x] 1.5 중첩·그룹 검증 규칙과 참여자 전환 조회
  - design §"bpmn::validate"의 새 위반 6종(부모가 Sub-Process 아님·부모 사이클·Sub-Process 경계를 넘는 시퀀스 흐름·
    경계 이벤트 부모 불일치·그룹 멤버 부모 불일치·두 그룹에 든 요소)을 기존 규칙 순서 뒤에 더하고, 그룹 id는 중복
    검사에, 멤버 참조는 참조 해석에 포함한다. 안에서 소속(레인)이 바뀌는 Sub-Process id 목록을 돌려주는 조회를 만든다
  - DONE: 단위 테스트에서 위반 6종마다 모델 하나 → 해당 변형 + id; 부모·그룹 없는 기존 모델은 새 규칙 전부 통과(기존
    검증 테스트 무수정); 전환 조회가 전환 없음 → 빈 목록, L2 안 전환 → 그 L2, L4 안 전환 → 그 L4(6.3, 6.4, 6.5)
  - _Requirements: 4.3, 5.4, 6.3, 6.4, 6.5_
  - _Difficulty: mid_
  - _Boundary: bpmn::validate_
  - _Depends: 1.3_

- [x] 2. 핵심: 개요 리더
- [x] 2.1 괄호 태그 분리와 문서 스니핑
  - design §"bpmn::bizprocess"의 태그 분리: 텍스트 끝 괄호를 뒤에서부터 벗기되 ID 목록(`1.1`·`1.1~1.5`를 쉼표로)은
    버리고 `키: 값` 목록은 태그로(값은 앞뒤 공백만 제거), 둘 다 아니면 그 괄호부터 이름; `키:` 목록의 조각에 `:`이 없으면
    값 쉼표 오류. 스니핑은 어떤 줄이든 `#`+공백을 벗기면 `L1` + 공백으로 시작하고 `:`를 포함할 때 참
  - DONE: 단위 테스트에서 `(1.1, 2.3)`·`(1.1~1.5, 2.1)` 제거(3.1); `(valueChainRef: VC-X)`·`(participant: 학습자)` 태그
    (3.2); `감시 루프(백그라운드)`·`(표준입력이거나 파일 미지정)` 이름 유지(3.3); `(participant: 멘토) (2.1, 2.3)` 둘 다
    태그, `(백그라운드) (2.1)`은 ID만 태그(3.4); `(participant: 학습자, 멘토)` → 쉼표 오류(3.5); `(participant:  학습자 )`
    → `학습자`, `시스템운영자`와 `시스템 운영자`가 다른 값(3.6); 스니핑이 7개 실제 문서 참, mermaid·PlantUML·XML·YAML·
    평문·`## 정의`만 있는 문서 거짓(1.2, 1.4)
  - _Requirements: 1.2, 1.4, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_
  - _Difficulty: high_
  - _Boundary: bpmn::bizprocess_

- [x] 2.2 헤딩 트리·이어지는 줄·게이트 줄·Logic 항목
  - 줄 분류(헤딩 `L<n>`·게이트·`Logic(AST):`·`- ` 항목·이어지는 줄·그 외 무시)와 명시적 스택으로 트리를 쌓는다. 레벨은
    `L<n>` 토큰만(`#` 개수·들여쓰기·종류 낱말 무시), 부모 = 레벨이 더 작은 가장 가까운 열린 헤딩, 게이트 줄은 현재
    L1을 닫고 다음 L1까지 무시. 항목은 `IF c THEN r`·`ELSE IF`·`ELSE r`·`ELSE (m) THEN r`·`THROW e`·평문(`항상:` 포함)으로
    읽고 `IF`만 있고 `THEN` 없으면 주석 + 진단. 헤딩·항목 이름은 2.1의 태그 분리를 거친다. L1 없음·L2 없는 L1은 오류
  - DONE: 단위 테스트에서 들여쓰기 0/2/4/6·`##`/`###`·종류 낱말 변형이 같은 트리(2.1); 두 줄 헤딩(`markdown-gfm-alerts`
    L4 원문)·두 줄 항목이 한 이름(2.2); 게이트 두 줄·`## 정의`·가치사슬 표·평문이 트리에 없음(2.3); 항목 5꼴이 각
    `LogicItem`이고 `IF` 없는 `THEN` 항목은 주석 + 진단(2.4, 5.7); L1 둘 → 프로세스 둘(2.5); L1 없음·L2 없음 → 오류
    (2.6); L2 → L4 직속·L5 없는 L4·빈 L2가 트리에 그대로(2.7); 헤딩 중간 잘림·`Logic(AST):`만·`- IF`만 → 오류 또는
    트리, 패닉 없음(7.3)
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 5.7, 7.3_
  - _Difficulty: high_
  - _Boundary: bpmn::bizprocess_

- [x] 3. 핵심: 개요 → 모델
- [x] 3.1 구조 매핑 — 참여자 상속·레인·L1~L5 요소·흐름·그룹·id
  - design §Key Decisions 드릴다운 표와 §"bpmn::parse_bizprocess" id 규약대로: L1마다 모델 하나(제목 = L1 이름), 태그가
    하나라도 있으면 풀(이름 = L1) + 첫 등장 순서 레인, 없으면 참여자 없음; 유효 참여자는 자기 태그 또는 가장 가까운
    조상의 것 → 소속 레인; L2 = Sub-Process(부모 없음) + 안쪽 시작/종료, L3 하나면 L2 이름 둘째 줄·둘 이상이면 그룹
    (멤버 = 그 L3의 L4들), L4 = L5 있으면 Sub-Process·없으면 태스크, L5 = 태스크; 형제는 문서 순서 시퀀스 흐름(L2:
    루트 시작→…→종료, L4: L2 시작→…→L2 종료, L5: 사슬만); 헤딩 이름 끝 ID 괄호 제거
  - DONE: 단위 테스트에서 태그 없는 문서 → 참여자 없음·소속 없음(3.9); L1만 태그 → 레인 하나·전부 상속(3.7); L2 태그 +
    L4 다른 태그 → L4·L5는 L4 참여자, 나머지 L2 참여자(3.7); 역할 재등장 → 레인 하나·첫 등장 순서(3.8); 풀 이름 = L1
    (3.10); L2 Sub-Process + 시작/종료 흐름(4.1); L3 하나 → 둘째 줄, 둘 → 그룹·멤버(4.2, 4.3); L4 종류(4.4); 형제 흐름
    순서(4.5); L1 둘 → 모델 둘·제목(2.5); L2 → L4 직속·빈 L2 패닉 없음(2.7); 모든 id 고유
  - _Requirements: 2.5, 2.7, 3.1, 3.7, 3.8, 3.9, 3.10, 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Difficulty: high_
  - _Boundary: bpmn::parse_bizprocess_
  - _Depends: 1.3, 2.2_

- [x] 3.2 Logic 매핑 — 게이트웨이·default·THROW·주석·다음 L5
  - design §Key Decisions Logic 표대로: 갈래 항목이 하나라도 있으면 소유 태스크 뒤 이름 없는 배타 게이트웨이, `IF`/`ELSE
    IF`는 조건 라벨 흐름 → 결과 태스크, `ELSE`는 default 흐름(라벨 = 메모 또는 없음), `THROW e`는 갈래 끝 오류 종료
    이벤트 + 그 L5를 품은 L2에 경계 오류 이벤트(소속 = L2의 것, 부모 없음), `IF` 없는 항목은 소유 태스크당 주석 하나에
    줄로 쌓아 연관으로 잇고, 다음 L5 형제는 소유 태스크에서 직접 이어진다. 조건·결과·주석 줄 끝 ID 괄호 제거. 모든
    합성 요소의 부모·소속 = 소유 태스크의 것
  - DONE: 단위 테스트에서 `IF`·`ELSE IF` → 게이트웨이 하나·라벨 흐름 둘(5.1, 5.2); `ELSE a`·`ELSE (m) THEN a` → default
    흐름·라벨(5.3); `ELSE THROW e`·`IF c THEN THROW e` → 오류 종료 이벤트 + L2 경계 이벤트(5.4); `항상:`·평문 둘 → 주석
    하나 두 줄 + 연관(5.5); 다음 L5가 소유 태스크에서 직접, 합류 없음(5.6); `IF`만 있는 항목 → 주석 줄(5.7); `(6.1)`
    제거(5.8); 부모·소속 상속
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8_
  - _Difficulty: high_
  - _Boundary: bpmn::parse_bizprocess_
  - _Depends: 3.1_

- [x] 4. 핵심: 정책별 여러 장
- [x] 4.1 한 장 정책 — `Depth(n)`·`All`과 기존 변환의 동일성
  - design §"bpmn::lower" 장 순서대로: 깊이 ≤ n 가시, 깊이 n의 자식 있는 Sub-Process는 접힘, 더 얕으면 실선 상자
    (부모 = 감싸는 상자 또는 레인, 제목 = 이름 첫 줄); 가시 그룹은 파선 상자(멤버 소속이 하나일 때만, 아니면 생략);
    가시 요소가 있는 레인만; 양끝 가시인 흐름만; 경계 이벤트 점선은 호스트 접힘 → 노드, 펼침 → 상자 닻. `All` = 전부
    펼침. 기존 한 장 변환은 `Depth(0)` 첫 장으로 정의
  - DONE: 단위 테스트에서 부모 없는 XML·YAML fixture 모델의 기존 변환 결과가 이 태스크 전과 노드·간선·그룹 순서까지
    동일(7.6); 3단 중첩 손 모델에서 `Depth(0)` → L2 접힘·자손 없음, `Depth(1)` → L2 실선 상자 + L4 접힘, `Depth(2)`·
    `All` → L5까지(6.2, 6.3); L3 그룹 파선 상자와 멤버 레인 갈림 시 생략(4.3, 6.8); 안쪽 시작/종료가 상자 안(4.1);
    펼친 호스트의 경계 이벤트가 상자 닻에서 점선(5.4); 사용 없는 레인 없음
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.4, 6.2, 6.3, 6.8, 7.6_
  - _Difficulty: high_
  - _Boundary: bpmn::lower_
  - _Depends: 1.2, 1.3, 1.5_

- [x] 4.2 여러 장 정책 — `PerActivity`와 `step` 재귀·제목
  - `PerActivity` = `Depth(0)` Process 장 + 자식 있는 L2마다 Activity 장(루트 = L2, 상자 없음, 자손 전부 펼침, 종류에
    L2 이름) — 단 안에서 소속이 바뀌는 L4는 접고 그 뒤에 Step 장(루트 = L4)을 재귀로 붙인다. Process 장만 제목 = 모델
    제목, 나머지는 빈 제목. 캡션 종류 문자열(`process`·`activity: 이름`·`step: 이름`)
  - DONE: 단위 테스트에서 L2 3개(하나는 빈 L2) 모델 → 장 3개(Process + Activity 2), Activity 장의 최상위가 L2 시작·L4·
    L2 종료이고 L4 Sub-Process가 펼친 상자(6.1, 4.6); L4 안 소속 전환 모델 → Activity 장에 그 L4 접힘 + Step 장 추가
    (6.5); Process 장만 제목(2.5); 캡션 종류 문자열 3꼴
  - _Requirements: 2.5, 4.6, 6.1, 6.5_
  - _Difficulty: high_
  - _Boundary: bpmn::lower_

- [x] 5. 배선
- [x] 5.1 `bizprocess` 렌더링 진입점과 fixture
  - design §"bpmn::mod"대로: 파싱 → L1마다 검증 → 정책 확정(`All`/`Depth(n ≥ 1)`인데 전환 있음 → `PerActivity` + 첫 줄
    안내 `※ depth=all 불가(참여자 전환) → activity`) → 여러 장 → 장마다 방향 옵션 적용·배치기 렌더링(하나라도 실패면
    전체 없음) → 한 본문(첫 장 종류 `process`, 둘째 장부터 빈 줄 + 캡션 줄; 캡션 함수는 언어 진입점의 것을 크레이트
    안에 공개해 재사용). 실제 문서 7개를 원본 경로로 `include_str!`한 상수 배열과 합성 fixture 두 개(태그·THROW·`ELSE IF`·
    L3 둘·L4 Sub-Process·이어지는 줄·괄호 이름 / L4 안 참여자 전환)를 테스트 전용으로 노출
  - DONE: 통합 테스트에서 태그 없는 문서 → `Some(("process", _))`이고 본문에 `◈ bizprocess · activity: ` 캡션이 L2 수만큼
    (6.1, 3.9); `--depth 0` → 캡션 없음, `all` → 한 장(6.2, 6.3); 합성 fixture + `all` → 안내 줄 + Activity 캡션(6.4);
    L4 전환 fixture → `step: ` 캡션(6.5); 두 L1 문서 → 본문 안 `process` 캡션 하나 더(2.5); 폭 8 → 없음(4.7); 파싱 오류
    문서 → 없음
  - _Requirements: 1.1, 2.5, 3.9, 4.7, 6.1, 6.2, 6.3, 6.4, 6.5_
  - _Difficulty: high_
  - _Boundary: bpmn::mod, diagram::mod(caption 가시성)_
  - _Depends: 1.4, 3.2, 4.2_

- [x] 5.2 언어·펜스·판별 순서·명령줄 옵션
  - design §"diagram::mod · cli · main"대로 `bizprocess` 언어(이름 `bizprocess`, 펜스 `bizprocess`/`biz-process`)를 더하고
    본문 판별 순서를 확장자 → `@start` → mermaid → BPMN → bizprocess → PlantUML로, 렌더 팔을 5.1의 진입점으로 잇는다.
    명령줄에 `--lang bizprocess`와 `--depth <all|activity|N>`(값 오류는 명령줄 오류)·환경변수 `DG_DEPTH`를 더한다. `.md`
    확장자는 판별에 넣지 않는다
  - DONE: 단위 테스트에서 펜스 두 이름 → 언어(1.1); 확장자·펜스 없는 실제 문서 → `bizprocess`(1.2); `x.md` 경로 판별
    없음(1.3); robustness의 PlantUML·mermaid 배열 + XML·YAML fixture가 이전과 같은 언어·갈래(1.4); `bpmn` 펜스의 `## L1`
    본문이 여전히 갈래 없음(1.5); `dg -d --depth deep <파일>`이 비제로 종료·`--depth all`은 정상(6.6, 수동)
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 6.6_
  - _Difficulty: mid_
  - _Boundary: diagram::mod, cli, main_

- [x] 6. 검증
- [x] 6.1 실제 `biz-process.md` 7개 회귀 통합 렌더링
  - 5.1의 실제 문서 상수 7개를 폭 100·80으로 렌더링해 대조한다; 드러나는 차이는 이 태스크 안에서 고친다(리더·빌더면
    2.x/3.x, 변환이면 4.x, 폭 초과면 원인 기록 후 재검토)
  - DONE: 통합 테스트에서 7개 모두 `Some`, Activity 캡션 수 = L2 헤딩 수, `dg-watch-mode`에 `감시 루프(백그라운드)`
    포함, 7개 어디에도 `검토 요청`·`valueChainRef`·`VC-DG-`·`(1.1`류 ID 괄호 없음(7.1); `dg-watch-mode` Process 장에
    `[+]` 5개, "감시 모드를 켠다" Activity 장에 `×`·`╱`·`--watch` 라벨(7.2); 두 줄 헤딩이 한 이름(2.2); `{text}` 육안 확인
  - _Requirements: 2.2, 2.3, 3.3, 3.9, 7.1, 7.2_
  - _Difficulty: mid_
  - _Boundary: bpmn::mod_
  - _Depends: 5.1_

- [x] 6.2 (P) 합성 fixture의 풀·레인·그룹·경계 이벤트 렌더링
  - 5.1의 합성 fixture를 폭 100으로 `activity`·`all`·`1`·`0`으로 렌더링해 레인·파선 그룹·경계 오류 이벤트·오류 종료·
    default 꼬리·폴백 안내·step 장이 실제 글자로 보이는지 못박는다
  - DONE: 통합 테스트에서 풀 제목 = L1 이름과 역할 레인 제목들이 첫 등장 순서(3.10, 3.8); Activity 장에 파선 테두리
    글자와 L3 제목 둘(4.3); Process 장 L2 상자 옆 `«error»`와 Activity 장 `◉ «error»`(5.4); `╱`와 조건 라벨(5.1~5.3);
    `all` → 안내 줄 + Activity 캡션, `1` → 안내 줄에 `depth=1`(6.4); 전환 fixture → `◈ bizprocess · step: `(6.5);
    `--direction tb` → 풀 제목들이 같은 줄(6.7); `{text}` 육안 확인
  - _Requirements: 3.8, 3.10, 4.3, 5.1, 5.2, 5.3, 5.4, 6.4, 6.5, 6.7_
  - _Difficulty: mid_
  - _Boundary: bpmn::mod_
  - _Depends: 5.2_

- [x] 6.3 (P) 손상 입력 강건성·마크다운 펜스 폴백
  - robustness에 bizprocess "반드시 없음" 배열(빈 문자열·공백·헤딩 없는 평문·L1만·헤딩 중간 잘림·`Logic(AST):`만·
    `- IF`만·태그 값 쉼표)과 "패닉만 없음" 배열(실제 문서 7개·합성 fixture 둘·잘린 실제 문서)을 폭 8·20·40·80·200 ×
    정책 `0`·`1`·`2`·`all`·`activity`로 돈다. 마크다운 테스트에 `bizprocess` 펜스 성공·실패 두 경우를 더한다
  - DONE: 반드시-없음 배열 전부 × 폭 × 정책이 없음(7.3); 패닉만-없음 배열 통과(7.4); 마크다운 문서의 펜스가 성공 시
    `◈ bizprocess · process` 캡션, 실패 시 `╭─ bizprocess ` 코드블록이고 앞뒤 문단은 그대로(7.5)
  - _Requirements: 7.3, 7.4, 7.5_
  - _Difficulty: mid_
  - _Boundary: diagram::mod, markdown 테스트_
  - _Depends: 5.2_

- [x] 6.4 전체 스위트 + clippy + 바이트 동일 회귀 + 의존성 수 + 실물 실행 + README
  - README 지원 문법 표에 `bizprocess` 행(헤딩 규칙·`participant:` 태그·Logic 매핑·정책 요약), 옵션 표에 `--depth`·
    `DG_DEPTH`, `-l` 값 목록 `mermaid|plantuml|bpmn|bizprocess`
  - DONE: `cargo test` 전량 통과(기준선 488 + doc 2 무수정 통과 + 신규), `cargo clippy --all-targets -- -D warnings` 클린,
    `examples/*.md`·XML·YAML fixture를 1.1과 같은 명령으로 렌더링해 기준선과 `diff` 무차이(7.6), `Cargo.toml` 의존성
    4개(7.7), `dg -d .kiro/specs/dg-watch-mode/biz-process.md`·`cat <같은 파일> | dg -d`·`dg -d -l bizprocess <같은 파일>`
    셋 다 `◈ bizprocess · process`로 시작(7.8), `dg -P .kiro/specs/dg-watch-mode/biz-process.md`는 마크다운 그대로(1.3),
    README 세 항목(7.9), 7개 문서 `{text}` 육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 1.1, 6.1, 6.2, 6.3_
