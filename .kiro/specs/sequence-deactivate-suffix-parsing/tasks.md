# Implementation Plan — sequence-deactivate-suffix-parsing

## 정의
mermaid 시퀀스 화살표에 `-`(deactivate) 접미사가 붙은 메시지가 비활성화·화살촉을
잃어버리는 결함을 고치는 수정이다.

- [x] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [x] 1.1 화살표 종류별 deactivate 접미사 파싱 재현
  - DONE: `->>-`, `-->>-`, `-x-`, `-)-`, `--)-` 각각에 deactivate 접미사가 붙은
    메시지를 `parse()`로 파싱해 `deactivate_source`·`head`를 확인하는 테스트를
    추가하고, 수정 전 코드로 실행해 `deactivate_source=false`·`head=None`으로
    **실패**함을 확인(1.1, 1.2)
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: mermaid 시퀀스 파서_

- [x] 2. 불변 동작 테스트 확인(수정 전에도 통과해야 함)
- [x] 2.1 기존 activate·무접미사·독립 구문 테스트 확인
  - DONE: `+` 접미사·접미사 없음·독립 `activate`/`deactivate` 구문을 다루는 기존
    `diagram::mermaid::sequence::tests::*`가 수정 전 코드에서 이미 전량 통과함을
    확인(3.1~3.3)
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: mermaid 시퀀스 파서_

- [x] 3. 수정 적용
- [x] 3.1 화살표 토큰 끝의 잘못 먹힌 `-`를 되돌린다
  - DONE: `parse_message()`에서 탐욕적으로 소비한 화살표 토큰의 마지막 글자가
    `-`면 한 글자 되돌려(`arrow_end -= 1`) `remainder`가 그 `-`를 다시 보게 해
    기존 `target_text.strip_prefix('-')` 로직이 정상 인식하게 한다. 1.1의 재현
    테스트가 통과로 바뀜을 확인
  - _Requirements: 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: mermaid 시퀀스 파서_
  - _Depends: 1.1, 2.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(1.1의 재현 테스트가 통과로 바뀌고 2.1의 불변
    동작 테스트도 계속 통과), `cargo clippy --all-targets -- -D warnings` 클린,
    `B-->>-C: text` 형태의 실제 mermaid 소스를 release 바이너리로 렌더링해
    활성화 막대가 그 메시지에서 정상적으로 닫히고 화살촉이 보임을 육안 확인
    (sequence-nested-activation-offset의 오프셋 표시도 이제 정상적으로 함께
    작동함을 확인)
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
