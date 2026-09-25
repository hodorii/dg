# Implementation Plan — plantuml-wbs

## 정의
PlantUML `@startwbs` 코드펜스를 기존 계층 그래프 배치기로 그리는 기능이다.

- [x] 1. `Shape::Plain`(테두리 없는 노드) 추가
- [x] 1.1 `ir::Shape::Plain` 변형과 `layout::shape`의 측정·그리기 분기 추가
  - DONE: `Shape` enum에 `Plain` 추가, `measure()`는 `(tw, th)`(여백 없음 —
    설계 때는 `tw+2`로 잡았으나 `draw_sections()`의 테두리 한 칸 전제 관례를
    그대로 쓰면 앞뒤 여백이 비대칭으로 남아 직접 그리는 쪽으로 바꿈),
    `draw()`는 테두리 없이 텍스트만 직접 채운다. 한 줄·여러 줄 본문 각각에
    대해 `draw_rows(Shape::Plain, ...)`가 테두리 문자 없이 텍스트만 그리는지
    확인하는 단위 테스트 추가
  - _Requirements: 2.3_
  - _Difficulty: low_
  - _Boundary: layout::shape_

- [x] 2. WBS 파서 작성
- [x] 2.1 (P) 깊이 스택 기반 트리 파싱(`*`/`+`/`-` 공통 깊이 글자)
  - DONE: `diagram::plantuml::wbs::parse()`를 새로 만들어, 줄 앞의 `*`/`+`/`-`
    개수로 깊이를 세고 "새 깊이보다 크거나 같은 스택은 pop, 남은 스택 맨
    위가 부모"인 표준 아웃라인 파싱으로 `ir::Graph`를 만든다. 스택이 비면
    새 뿌리(여러 뿌리 허용). `*`/`**`/`***` 단계별 부모-자식 관계, 최상위가
    둘 이상일 때 각각 독립 뿌리, 단계를 건너뛴 표기가 직전 항목의 자식이
    되는 경우를 확인하는 단위 테스트 추가
  - _Requirements: 1.2, 1.3, 1.4_
  - _Difficulty: mid_
  - _Boundary: plantuml::wbs_

- [x] 2.2 (P) 본문 표기(한 줄/여러 줄/`_` 접미사) 처리
  - DONE: 깊이 글자 뒤 `_`가 붙으면 `Shape::Plain`, 아니면 `Shape::Rect`로
    노드를 만든다. 텍스트가 `:`로 시작하면 `;`로 끝나는 줄까지 이어서
    `\n`으로 합친 뒤 `Graph::intern()`에 넘긴다(한 줄이면 그대로). 한 줄
    본문, `:`~`;` 여러 줄 본문, `_` 접미사 노드가 각각 올바르게 만들어지는지
    확인하는 단위 테스트 추가
  - _Requirements: 2.1, 2.2, 2.3_
  - _Difficulty: mid_
  - _Boundary: plantuml::wbs_
  - _Depends: 1.1_

- [x] 2.3 범위 밖 구문은 무시하고 구조만 살린다
  - DONE: 산술 방향(`+`/`-`의 좌우 의미), 노드 간 화살표, 인라인 색상
    (`[#색]`), `<style>` 블록, 별칭(`as`)이 섞인 입력을 파싱해 패닉 없이
    트리 구조(부모-자식 관계)가 정상적으로 나오는지 확인하는 단위 테스트
    추가(값 자체는 반영 안 해도 됨)
  - _Requirements: 3.3_
  - _Difficulty: low_
  - _Boundary: plantuml::wbs_
  - _Depends: 2.1, 2.2_

- [x] 3. 판별·배선 연결
- [x] 3.1 `kind_of_start_tag()`·`render()`에 `wbs` 연결
  - DONE: `diagram::plantuml::mod::kind_of_start_tag()`에
    `"wbs" => Some("wbs")` 추가(휴리스틱 점수 매기기 없이 바로 분기),
    `render()` 매치문에 `"wbs"` 분기(`wbs::parse` → `options.apply_to_graph`
    → `layout::graph::render`) 추가. `kind_of("@startwbs\n...\n@endwbs")`가
    다른 종류로 오판별되지 않는지, 항목이 하나도 없을 때 `render()`가
    `None`을 돌려주는지 확인하는 테스트 추가
  - _Requirements: 1.1, 3.1_
  - _Difficulty: low_
  - _Boundary: plantuml::mod_
  - _Depends: 2.1, 2.2_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과, `cargo clippy --all-targets -- -D warnings`
    클린, 여러 뿌리·깊이 3단 이상·`_` 노드·여러 줄 본문이 섞인 실제 WBS
    예시를 release 바이너리로 렌더링해 트리 구조가 올바르게 보이고 폭 초과
    시 원문 코드블록으로 대체되는 기존 규약도 유지됨을 육안 확인(3.2)
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
