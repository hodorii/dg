# Implementation Plan — diagram-diamond-side-glyph-coverage

## 정의
마름모 옆면 글자를 기본 CJK 모노 폰트 커버리지 안의 1칸 글자 `‹›`로 바꿔 폰트 폴백을 없애는
결함 수정이다.

- [ ] 1. 재현 테스트 작성(수정 전 실패해야 함)
- [ ] 1.1 마름모 옆면 글자·코드포인트 테스트 작성
  - 마름모 한 노드를 그려 본문 줄의 첫 칸·끝 칸이 `‹`/`›`인지, 렌더 결과 어느 칸에도
    U+27E8·U+27E9가 없는지 확인
  - 마름모 옆면 글자 허용 목록(`‹›`)·제외 목록(`⟨⟩`) 테스트를 기존 게이트웨이 커버리지
    허용 목록 테스트와 같은 형식으로 추가
  - DONE: 두 테스트가 수정 전 코드에서 **실패**함을 `cargo test` 출력으로 확인(옆면이
    `⟨⟩`이고 U+27E8/E9가 존재)
  - _Requirements: 1.1, 1.2, 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: layout::shape 그리기_

- [ ] 2. 불변 동작 테스트 확인·갱신(수정 전에도 통과해야 함)
- [ ] 2.1 마름모·육각형 구분 테스트의 기대 문자열 확인
  - `diamond_and_hexagon_render_differently`가 육각형 문자열·`assert_ne!`로 두 도형의
    구분을 고정하고 있음을 확인(마름모 기대 문자열은 3.1에서 수정과 함께 갱신)
  - 마름모 크기 `(tw+6, th+2)`·모서리 `╱╲`·본문 가운데 정렬을 옆면 글자와 무관하게
    확인하는 단언이 없으면 한 건 추가(옆면 두 칸을 제외한 나머지 칸 비교)
  - DONE: 수정 전 코드에서 해당 테스트 전량 통과
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: low_
  - _Boundary: layout::shape 그리기_
- [ ] 2.2 (P) 게이트웨이·표식·파서 회귀 테스트 통과 확인
  - `gateway_symbol_labels_render_verbatim_and_symbols_are_not_dimmed`,
    `bpmn_glyph_codepoints_are_within_cjk_mono_coverage`, 간선 표식 글자 테스트, 파서의
    마름모 판별 테스트(mermaid flowchart·state·block, PlantUML class, BPMN lower)가 수정
    전 코드에서 전량 통과함을 확인 — 이 태스크는 수정하지 않는다
  - DONE: 위 테스트 목록이 `cargo test` 출력에서 전부 `ok`
  - _Requirements: 3.4, 3.5, 3.6_
  - _Difficulty: low_
  - _Boundary: layout::graph 표식, 파서(mermaid·plantuml·bpmn)_

- [ ] 3. 수정 적용
- [ ] 3.1 옆면 글자 쌍 교체
  - 마름모 분기의 `('⟨', '⟩')`를 `('‹', '›')`로 바꾸고, 주석에 커버리지 근거(Noto Sans
    Mono CJK 커버·EAW N·1칸) 한 줄을 덧붙인다
  - `diamond_and_hexagon_render_differently`의 마름모 기대 문자열 옆면 두 글자만 `‹›`로
    갱신(육각형 문자열·`assert_ne!`는 그대로)
  - DONE: 1.1의 재현 테스트가 통과로 바뀌고 2.1·2.2의 불변 테스트가 계속 통과
  - _Requirements: 2.1, 2.2, 3.1_
  - _Difficulty: low_
  - _Boundary: layout::shape 그리기_
  - _Depends: 1.1, 2.1, 2.2_

- [ ] 4. 검증
- [ ] 4.1 전체 스위트·clippy·실물 렌더링
  - `cargo test` 전량 통과, `cargo clippy --all-targets -- -D warnings` 클린
  - 릴리스 바이너리로 mermaid flowchart(TD·LR) `{}`·stateDiagram `<<choice>>`·block-beta
    `{}`·PlantUML class `diamond`·`examples/bpmn.md` 게이트웨이를 렌더링해 출력에
    U+27E8/U+27E9가 0개, `‹›`가 옆면에 있음을 코드포인트로 확인
  - LR 배치에서 표식(`▶`, `○`)이 옆면 `‹` 옆 칸에 그대로 찍힘을 확인
  - DONE: 다섯 문법 출력 모두 `grep -o '⟨' | wc -l` = 0이고 옆면이 `‹`/`›`
  - _Requirements: 2.1, 2.2, 3.6_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 3.1_
- [ ] 4.2 골든 파일 재생성·diff 분리 확인
  - `make examples`로 `examples/architecture.txt` 재생성
  - `git diff`에서 138·151·170행의 옆면 여섯 칸 변화와, 별개 사유(`eade414`의 서브그래프
    절 미갱신)로 들어오는 나머지 diff를 구분해 확인 — 마름모 없는 부분은 그 절 외에
    바이트 변화가 없어야 한다
  - DONE: diff가 "여섯 칸 + 서브그래프 절" 두 덩이로만 설명되고, 그 외 행 변화 0
  - _Requirements: 1.3, 2.3, 3.7_
  - _Difficulty: low_
  - _Boundary: 골든 파일(examples/architecture.txt)_
  - _Depends: 3.1_
