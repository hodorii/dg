# Implementation Plan — diagram-alias-label

## 정의
mermaid `erDiagram` 개체 별칭이 등장 순서와 무관하게 항상 반영되도록 `declare()`를 고치고, 결함 재현·기대 동작·불변 동작을 유닛 테스트로 고정하는 작업이다.

- [x] 1. 결함을 유닛 테스트로 고정
- [x] 1.1 별칭이 나중에 등장하는 두 케이스를 재현하는 테스트를 추가한다
  - DONE: `src/diagram/mermaid/er.rs`의 `mod tests`에 `CUSTOMER ||--o{ ORDER : places` 뒤에 `CUSTOMER["고객"] { string name }`가 오는 케이스와, `CUSTOMER ||--o{ ORDER : places` 뒤에 `CUSTOMER["고객"] ||--o{ INVOICE : has`가 오는 케이스를 각각 파싱해 `CUSTOMER` 노드의 `sections[0] == vec!["고객"]`을 단언하는 테스트 함수 두 개(또는 하나로 통합)가 존재하고, 수정 전 코드에서는 `cargo test --lib mermaid::er` 실행 시 이 테스트가 실패한다.
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: src/diagram/mermaid/er.rs_

- [x] 2. 불변 동작을 지키는 테스트를 확인·보강한다
- [x] 2.1 별칭이 첫 등장인 기존 케이스와 별칭 없는 케이스가 계속 통과하는지 확인한다
  - DONE: 기존 `parses_entities_and_relations` 테스트(별칭 없는 `CUSTOMER`가 그대로 `CUSTOMER`로 표시됨, 3.2)가 수정 전후 모두 통과하고, 별칭이 첫 등장인 케이스(`CUSTOMER["고객"] { ... }`가 관계보다 먼저 오는 입력)를 검증하는 테스트가 하나 이상 존재해 `sections[0] == vec!["고객"]`을 확인한다.
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: src/diagram/mermaid/er.rs_
  - _Depends: 1.1_

- [x] 3. `declare()`가 별칭을 등장 순서와 무관하게 반영하도록 고친다
- [x] 3.1 대괄호 별칭이 있는 호출마다 라벨을 무조건 다시 쓴다
  - DONE: `src/diagram/mermaid/er.rs`의 `declare(graph, raw)`가 대괄호 별칭을 분리한 뒤 `graph.intern`으로 노드를 확보하고, 별칭이 있으면 노드가 이미 존재했는지와 무관하게 `graph.set_label(index, &text)`를 호출한다. 별칭이 없는 호출은 기존처럼 최초 생성 시에만 아이디가 라벨이 된다. `cargo test --lib mermaid::er`가 태스크 1·2의 테스트를 모두 통과시킨다.
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: src/diagram/mermaid/er.rs_
  - _Depends: 1.1, 2.1_

- [x] 4. 전체 검증
- [x] 4.1 라이브러리 전체 테스트와 clippy를 통과시킨다
  - DONE: `cargo test --lib`가 모두 통과하고 `cargo clippy --all-targets --all-features`가 경고 없이 끝난다(`-D warnings` 기준 클린).
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Depends: 3.1_
