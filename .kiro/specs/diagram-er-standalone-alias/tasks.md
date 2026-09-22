# Implementation Plan — diagram-er-standalone-alias

## 정의
mermaid `erDiagram`의 단독 개체 선언 줄(`ID["라벨"]`, 관계선·속성 블록 없음)에 붙인 대괄호
별칭이 반영되도록 고치는 결함 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 단독 선언 줄 별칭 반영 테스트 작성
  - DONE: `CUSTOMER["Customer"]`만 단독으로 있고 나중에 별칭 없이 관계선에서 참조되는
    픽스처, 라벨에 공백이 섞인 픽스처(`mbr_ams_score_hst["회원입학선발점수이력 (...)"]`류)
    두 가지를 추가하고, 수정 전 코드로 실행해 **실패**함을 확인(상자 라벨이 아이디로 남음,
    1.1/1.2)
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: mermaid er 파서_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 기존 관계선/속성 블록 별칭·무별칭 회귀 테스트 확인
  - DONE: `parses_entities_and_relations`, `diagram-alias-label`이 추가한 순서 의존 회귀
    테스트(관계선 인라인 별칭, 속성 블록 헤더 별칭, 별칭 없는 관계선)가 수정 전 코드에서
    이미 전량 통과함을 확인(3.1~3.3)
  - _Requirements: 3.1, 3.2, 3.3, 3.4_
  - _Difficulty: low_
  - _Boundary: mermaid er 파서_

- [x] 3. 수정 적용
- [x] 3.1 관계선 판정 후 단독 선언 줄을 declare()로 디스패치
  - DONE: `parse()`가 각 줄을 `parse_relation()`으로 넘기기 전에 대괄호 내용을 제외한
    나머지에 관계 연산자(`--`/`..`)가 있는지로 관계선 여부를 판정하는 헬퍼를 두고, 아니면
    `declare()`를 직접 호출하도록 고쳐 1.1의 재현 테스트가 통과로 바뀜
  - _Requirements: 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: mermaid er 파서_
  - _Depends: 1.1, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(1.1의 재현 테스트가 통과로 바뀌고 2.1의 불변 동작 테스트도
    계속 통과), `cargo clippy --all-targets -- -D warnings` 클린, 실제 참조 문서
    (`/home/hs/w/.kiro/reference/schemas_diagram.md`)의 회원 도메인 ERD를 릴리스 바이너리로
    렌더링해 `mbr_ams_score_hst` 등 한글 별칭이 실제로 표시됨을 육안 확인
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3, 3.4_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
