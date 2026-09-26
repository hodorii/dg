# Implementation Plan — bpmn-event-shape-notation

## 정의
BPMN 이벤트를 활동(둥근 상자)과 한눈에 가르기 위해, 이벤트 노드를 상자 없이 원 글자 + 오른쪽 이름으로
그리게 도형 그리기 분기만 바꾸는 수정이다. 손으로 만든 도형·그래프 단위 테스트와 기존 예제 렌더링으로
검증한다.

- [x] 1. 재현 테스트 갱신(수정 전 실패해야 함)
- [x] 1.1 (P) 도형 테스트의 이벤트 기대 출력을 원 표기로
  - 시작·중간·종료 "Go" → `["○ Go"]`·`["◎ Go"]`·`["● Go"]`; 본문 없음 → `["○"]`; `["Go", "«timer»"]` →
    `["○ Go", "  «timer»"]`(높이 2), 한 줄 본문 높이 1로 기대 리터럴을 바꾼다
  - "Round와 높이 같음" 단정은 "Round `["╭────╮", "│ Go │", "╰────╯"]`와 출력이 다르고 테두리 글자
    `╭╮╰╯─│┏┓┗┛━┃`가 하나도 없다"로 바꾼다
  - 스타일 단정: 위치 글자 run이 `theme.diagram_accent`, 이름 run `bold`, 둘째 줄 run `dim && italic`
  - DONE: 바뀐 세 테스트가 수정 전 코드에서 리터럴 불일치로 **실패**함을 확인(1.1~1.5)
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5_
  - _Difficulty: low_
  - _Boundary: layout::shape 테스트_

- [x] 1.2 (P) 배치 테스트의 이벤트 접점 기대를 원 글자 기준으로
  - 가로: `▶`의 바로 오른쪽 칸이 종료 이벤트의 `●`; 세로: `●`가 있는 줄 바로 위 줄에 `▼`가 있고 그 열이
    이벤트가 차지한 열 범위 안. 가운데 `Round` 노드의 "텍스트 줄에서 두 줄 위" 단정은 그대로
  - DONE: 가로 테스트가 수정 전 `▶` 오른쪽이 `┃`라 **실패**, 세로 테스트가 수정 전 `●` 위 줄이 `┏━━┓`라
    **실패**함을 확인(1.6)
  - _Requirements: 1.6, 2.6_
  - _Difficulty: mid_
  - _Boundary: layout::graph 테스트_

- [x] 2. 불변 동작 테스트(수정 전에도 통과해야 함)
- [x] 2.1 기대 목록의 `┃` 제거와 기존 스위트 기준선
  - 주문 처리 협업 손 모델·bpmn.io fixture(`bpmn::tests`)와 손 그래프(`layout::graph::tests`)의 기대 글자
    목록에서 `┃`만 빼고 나머지(`○●◎«user»«service»«message»«error»× 재고 있음?╱╌`)는 그대로; "종료
    이벤트의 굵은 테두리도 `┃`를 쓴다"는 주석을 현재 사실로 갱신
  - 도형 코드를 손대기 전에 `examples/*.md` 전부를 `dg -P --width 100`으로 렌더링해 스크래치 디렉터리에
    저장(3.6 diff 기준)
  - DONE: 바뀐 세 테스트와 나머지 스위트 전량이 수정 전 코드에서 통과(1.1·1.2에서 바꾼 다섯 테스트만
    실패); 예제 파일마다 기준 출력이 하나씩 있다(3.1~3.7)
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_
  - _Difficulty: low_
  - _Boundary: bpmn::mod 테스트, layout::graph 테스트, 검증_

- [x] 3. 수정 적용
- [x] 3.1 이벤트 도형을 상자 없는 원 글자 + 이름으로
  - design §수정 방식: 측정은 "위치 글자·공백·본문 폭 × 본문 줄 수(최소 1)", 본문 없으면 1×1; 그리기는
    상자 테두리 호출을 없애고, 배치기가 늘린 폭 안에서 덩어리를 가운데 두어 위치 글자(강조색)를 놓고 그
    오른쪽 두 칸부터 모든 줄을 왼쪽 맞춤으로, 기존 줄 스타일 규칙(이름 굵게·`«…»` 흐림·기울임) 그대로
  - 어휘의 `Event` 변형 주석을 "원 글자 + 오른쪽 이름, 테두리 없음"으로
  - 다른 모양 분기(`Round`·`Subprocess`·`Circle`·`Stadium` 등)·`Canvas`·배치기는 손대지 않는다
  - DONE: 1.1·1.2의 다섯 테스트가 통과로 바뀌고 2.1의 스위트가 계속 통과
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1, 3.3, 3.4_
  - _Difficulty: mid_
  - _Boundary: layout::shape, diagram::ir_
  - _Depends: 1.1, 1.2, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 예제 회귀 + 실물 확인
  - `cargo test` 전량 통과, `cargo clippy --all-targets -- -D warnings` 클린
  - `examples/*.md`를 2.1과 같은 명령으로 렌더링해 `bpmn.md` 외 전부 기준선과 `diff` 무차이; `bpmn.md`는
    이벤트 노드 자리만 다르고 `○ 주문 접수`·`● 완료`·`◎ 시한 초과`/`«timer»`가 상자 없이 보이며
    태스크·서브프로세스는 둥근 상자 그대로임을 `{text}` 출력으로 육안 확인
  - `git diff --stat`에 `shape.rs`·`graph.rs`(테스트)·`bpmn/mod.rs`(테스트)·`ir.rs` 외 소스 변경 없음
  - design §영향 범위대로 `bpmn-shapes/design.md` Key Decisions·`bpmn-xml/requirements.md` 4.1·
    `bpmn-support/research.md` 이벤트 행에 이 스펙을 가리키는 한 줄씩
  - DONE: 위 네 항목 모두 확인되고 접점 한계 (a)(b)(design §수정 방식)가 실물에서 예상대로만 나타남
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
