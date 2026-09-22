# Implementation Plan — dg-watch-mode

## 정의
파일을 편집하며 dg로 미리보기를 확인하는 사용자를 위해, 파일이 바뀔 때마다 자동으로 다시
렌더링해 보여주는 기능이다.

- [x] 1. 감시 기반 모듈
- [x] 1.1 `watch.rs`: `Watcher`/`PollResult`/`POLL_INTERVAL` 구현
  - DONE: 임시 파일을 대상으로 `Watcher::new` 직후 `poll()`이 `Unchanged`, 내용을 바꾼 뒤 `poll()`이
    `Changed(새 내용)`, 파일을 지운 뒤 `poll()`이 `ReadError`(1회만, 다음 `poll()`부터는 오류가
    이어지는 동안 `Unchanged`), 파일을 복구하면 다시 `Changed`, `poll_forced()`가 mtime과 무관하게
    항상 다시 읽음을 확인. 모든 파일 접근이 `Result` 매칭이라 패닉하지 않음(`unwrap`/`expect` 없음)
  - _Requirements: 2.1, 2.2, 2.3, 5.1, 5.2, 5.3_
  - _Difficulty: mid_
  - _Boundary: watch 모듈_
- [x] 1.2 `lib.rs`에 `watch` 모듈을 `cli` 피처 게이트로 등록
  - DONE: `cargo build --no-default-features`(라이브러리 전용)에서 `watch` 모듈이 포함되지 않고,
    기본(`cli` 피처 켠) 빌드에서는 `dg::watch::{Watcher, PollResult}`를 외부에서 쓸 수 있음
  - _Requirements: 1.2_
  - _Difficulty: low_
  - _Boundary: lib 모듈 등록_
  - _Depends: 1.1_
- [x] 1.3 CLI `--watch`/`-W` 플래그 추가
  - DONE: `dg --help` 출력에 `--watch`/`-W`가 나타나고, `Cli.watch: bool`이 파싱됨
  - _Requirements: 1.2_
  - _Difficulty: low_
  - _Boundary: cli 파싱_

- [x] 2. main.rs 배선(print 모드)
- [x] 2.1 `render` 클로저가 `source: &str`를 인자로 받도록 변경
  - DONE: 기존 단일 렌더링 경로(`--watch` 없음)가 이전과 동일한 출력을 내며, `cargo test`의 기존
    회귀 스위트가 그대로 통과함(1.1)
  - _Requirements: 1.1_
  - _Difficulty: mid_
  - _Boundary: main.rs 렌더 경로_
  - _Depends: 1.3_
- [x] 2.2 `--watch` + 표준입력 조합의 조기 오류 처리
  - DONE: `echo x | dg --watch`가 표준입력을 읽으려 시도하지 않고(블로킹 없음) 오류 메시지와 함께
    0이 아닌 종료 코드로 즉시 끝남
  - _Requirements: 1.3_
  - _Difficulty: low_
  - _Boundary: main.rs 입력 검증_
  - _Depends: 2.1_
- [x] 2.3 print 모드 감시 루프
  - DONE: `dg -P --watch 파일`을 실행한 채 파일을 수정하면 1초 이내 새 렌더링이 표준출력에 다시
    쓰이고(2.1, 4.1), 파일을 지우면 stderr에 오류가 한 번만 찍히고 마지막 출력을 유지하다 파일이
    복구되면 자동으로 재개되며(5.1, 5.2), 내용이 안 바뀌면 아무 것도 다시 쓰이지 않고(2.2),
    `Ctrl-C`로 종료됨(4.2). `--watch` 없이는 기존과 동일한 단일 출력(1.1, 6.1)
  - _Requirements: 2.1, 2.2, 4.1, 4.2, 5.1, 5.2, 6.1_
  - _Difficulty: mid_
  - _Boundary: main.rs print 루프_
  - _Depends: 2.2_

- [x] 3. pager.rs 확장
- [x] 3.1 `Pager`가 `source: String`과 `Option<Watcher>`를 보유하도록 생성자·필드 확장
  - DONE: `Pager::new`가 초기 소스와 `Option<Watcher>`를 받고, `render: F`의 시그니처가
    `Fn(&str, usize, usize) -> Document`로 바뀐 상태로 기존 단위 테스트(`expanding_a_block_...`)가
    수정된 헬퍼로 그대로 통과함
  - _Requirements: 1.2_
  - _Difficulty: mid_
  - _Boundary: pager 구조체_
  - _Depends: 1.2_
- [x] 3.2 이벤트 루프에 감시 폴링과 타임아웃 대기 추가
  - DONE: `watcher`가 `None`이면 이전과 동일한 블로킹 `event::read()` 경로가 그대로 동작함(6.1,
    3.3의 `q` 종료 포함). `watcher`가 `Some`이면 루프마다 `watcher.poll()`을 먼저 확인해 `Changed`
    시 `source`를 갱신하고 폭 비교와 무관하게 재렌더링하되 `top`(스크롤 위치)은 건드리지 않아
    문서가 안 짧아지는 한 그대로 유지되고(3.1), `event::poll(POLL_INTERVAL)` 타임아웃 대기라 키
    입력은 감시 여부와 무관하게 즉시 반응함(3.2)
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.3, 5.1, 6.1_
  - _Difficulty: high_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 3.1_
- [x] 3.3 `r` 키 수동 갱신과 상태 표시줄 감시 상태 표시
  - DONE: 감시 모드에서 `r`을 누르면 mtime과 무관하게 즉시 `poll_forced()`가 적용되어 화면이
    갱신되고, 상태 표시줄이 평소엔 "감시 중", 읽기 오류 중엔 "감시 불가: …"를 보여줌(감시하지
    않을 때는 기존 상태 표시줄과 동일)
  - _Requirements: 3.4, 3.5, 5.1_
  - _Difficulty: low_
  - _Boundary: pager 상태 표시줄_
  - _Depends: 3.2_

- [x] 4. 통합
- [x] 4.1 main.rs 페이저 호출부를 새 `Pager::new` 시그니처에 연결
  - DONE: `dg --watch 파일`(TTY, `-P` 없음)이 페이저를 감시 모드로 열고, `dg 파일`(플래그 없음)은
    기존과 동일하게 `watcher: None`으로 열려 회귀 없음(1.2, 6.1)
  - _Requirements: 1.2, 6.1_
  - _Difficulty: low_
  - _Boundary: main.rs 페이저 호출부_
  - _Depends: 2.3, 3.3_

- [x] 5. 검증
- [x] 5.1 통합 테스트: 페이저 감시 동작
  - DONE: `Pager`에 필드를 직접 설정하는 기존 테스트 패턴으로 `PollResult` 시퀀스를 주입해
    `source`/`document`/`top`/상태 표시줄이 `Changed`·`ReadError`·`Unchanged` 각각에서 기대대로
    바뀌는지, `r` 키 처리가 `poll_forced` 경로를 타는지 확인하는 테스트가 통과함
  - _Requirements: 3.1, 3.2, 3.4, 3.5, 5.1, 5.2_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프, pager 상태 표시줄_
  - _Depends: 4.1_
- [x] 5.2 회귀 확인: 기존 테스트 스위트 + clippy
  - DONE: `cargo test`(기존 202개 + 신규 테스트 전량), `cargo clippy --all-targets` 클린
  - _Requirements: 1.1, 6.1_
  - _Difficulty: low_
  - _Boundary: 전체_
  - _Depends: 5.1_
- [x] 5.3 실물 검증: 릴리스 바이너리로 print 모드 감시 확인
  - DONE: 임시 마크다운 파일을 `dg -P -s none --watch`로 감시하며 수정해 1초 이내 재출력됨을
    확인(3회 연속 갱신), 파일을 지웠다 복구해 stderr 오류 1회 후 자동 재개됨을 확인,
    `--watch`+표준입력이 블로킹 없이 즉시 오류로 끝남을 확인, `Ctrl-C`(SIGINT)로 프로세스가
    종료됨을 확인. 페이저 감시 모드(TTY 필요)는 이 세션에 실제 터미널이 없어 육안 확인은 못
    했고, 스크롤 유지·`r`·상태 표시줄은 5.1의 통합 테스트로만 검증됨 — 실사용 환경에서의 육안
    확인은 별도로 필요
  - _Requirements: 1.3, 2.1, 4.1, 4.2, 5.1, 5.2_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 5.2_
