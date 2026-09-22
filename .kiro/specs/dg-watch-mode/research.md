# Research & Design Decisions — dg-watch-mode

## Summary
- **Feature**: `dg-watch-mode`
- **Discovery Scope**: Extension(`main.rs`/`pager.rs`/`cli.rs` 기존 진입점 확장 + 신규 `watch.rs`)
- **Key Findings**:
  - dg는 의존성 4개·단일 정적 바이너리 ~1.2MB가 핵심 원칙이라 `notify` 등 OS 이벤트 감시 크레이트는
    처음부터 배제 대상이었다(사용자 확인: "oss 기반한 철학은 유지").
  - `pager.rs`의 이벤트 루프는 이미 `crossterm::event::poll`로 타임아웃 대기가 가능해, 감시 폴링을
    별도 스레드 없이 같은 루프에 끼워 넣을 수 있다.
  - 현재 `render` 클로저(`main.rs`)는 `source: String`을 `move`로 캡처해 고정값을 그린다 — 감시
    모드에서 최신 내용을 반영하려면 이 캡처 방식 자체를 바꿔야 한다.

## Research Log

### 현재 렌더링 경로에서 소스가 어떻게 흐르는지
- **Context**: 감시로 갱신된 내용을 어디에 반영해야 하는지 파악
- **Sources Consulted**: `src/main.rs`(`read_input`, `render` 클로저, `use_pager` 분기),
  `src/pager.rs`(`Pager::new`, `event_loop`의 렌더 트리거 조건)
- **Findings**: `source`는 `read_input()`에서 한 번 읽혀 `render` 클로저에 캡처된다. 페이저는
  `width`/`columns`가 바뀔 때만 `(self.render)(width, columns)`를 다시 부른다. print 모드는
  한 번 호출하고 끝난다.
- **Implications**: 감시를 붙이려면 (a) 소스를 갱신 가능한 상태로 바꾸고 (b) 페이저의 재렌더링
  트리거 조건에 "파일 변경"을 추가해야 한다.

### 페이저 이벤트 루프의 대기 방식
- **Context**: 감시 폴링이 키 입력 반응성(3.2, 입력 지연 없음)을 해치지 않는지 확인
- **Sources Consulted**: `src/pager.rs:74-118`(`event_loop`), crossterm 0.29 `event::poll` 문서
  (버전은 `Cargo.toml`에 고정)
- **Findings**: `event::read()`는 블로킹이지만 `event::poll(Duration)`은 타임아웃 안에 이벤트가
  없으면 `false`를 돌려주고 즉시 리턴한다. 타임아웃 값을 300ms처럼 짧게 잡으면 키 입력 체감
  지연 없이 주기적으로 파일을 확인할 수 있다.
- **Implications**: 별도 스레드·채널 없이 `watcher.is_some()`일 때만 `event::poll(timeout)`
  경로로 바꾸면 되고, 감시하지 않을 때는 기존 `event::read()` 블로킹 경로를 그대로 둘 수 있어
  회귀(6.1) 위험이 낮다.

## Design Decisions

### Decision: 파일 변경 감지 — mtime 폴링 vs OS 이벤트 API
- **Context**: 파일이 바뀐 걸 어떻게 알아챌지
- **Alternatives Considered**:
  1. `notify` 크레이트(inotify/FSEvents/ReadDirectoryChangesW 래퍼)로 즉시 이벤트 수신
  2. `std::fs::metadata(path).modified()`를 주기적으로 비교하는 폴링
- **Selected Approach**: 2번(폴링). `POLL_INTERVAL = 300ms`.
- **Rationale**: dg는 "의존성 4개, 단일 바이너리" 원칙을 README 첫 줄에 내세우는 프로젝트고,
  사용자도 이 세션에서 "OSS 기반 철학 유지"를 명시적으로 재확인했다. `notify`는 플랫폼별 백엔드를
  끌고 오는 무거운 의존성이라 원칙과 정면으로 충돌한다. 폴링은 표준 라이브러리만으로 요구사항의
  "1초 이내" 기준을 넉넉히 만족한다.
- **Trade-offs**: 매우 짧은 시간 안에 여러 번 저장하면 중간 상태를 건너뛸 수 있다(마지막 상태만
  반영) — 미리보기 용도에서는 문제되지 않는다. CPU를 아주 약간 더 쓴다(300ms마다 `stat` 1회,
  무시할 수준).
- **Follow-up**: 실사용에서 300ms가 체감상 느리면(2.1의 "1초 이내"는 여유 있게 통과하지만) 값만
  조정하면 된다 — `POLL_INTERVAL` 상수 하나로 SSoT.

### Decision: 갱신된 소스 전달 — 클로저 재캡처 vs 인자화 vs 공유 셀
- **Context**: `render` 클로저가 감시로 갱신된 최신 소스를 그리게 하는 방법
- **Alternatives Considered**:
  1. `Rc<RefCell<String>>`(또는 `Arc<Mutex<String>>`)를 클로저가 캡처하고, 감시 쪽에서 그 셀을
     갱신
  2. `render` 클로저 시그니처를 `Fn(&str, usize, usize) -> Document`로 바꿔 소스를 인자로 받고,
     `Pager`/print 루프가 소스를 소유·갱신
- **Selected Approach**: 2번(인자화).
- **Rationale**: 공유 셀은 "누가 언제 쓰는지"가 타입에 드러나지 않아(내부 가변성) 나중에 읽는
  사람이 클로저 내부를 봐야 갱신 시점을 알 수 있다. 인자화는 `Pager::source`/print 루프의 지역
  변수 한 곳에서만 소스를 갱신한다는 게 시그니처로 드러난다. 호출부(`main.rs` 한 곳)만 고치면
  되는 낮은 리스크의 변경이라 굳이 내부 가변성을 들일 이유가 없다.
- **Trade-offs**: `Pager::new`의 인자가 하나(`source: String`) 늘어난다 — 단일 호출부라 부담 적음.
- **Follow-up**: 없음.

### Decision: 폴링 위치 — 같은 스레드 vs 백그라운드 스레드+채널
- **Context**: 감시 폴링을 어디서 돌릴지
- **Alternatives Considered**:
  1. 별도 스레드에서 감시하고 채널로 메인 루프에 알림
  2. 페이저 이벤트 루프/print 루프와 같은 스레드에서 타임아웃 폴링
- **Selected Approach**: 2번(동일 스레드).
- **Rationale**: 페이저는 이미 `event::poll(timeout)`로 타임아웃 대기가 가능해(연구 로그 참조)
  같은 루프 반복 안에 `watcher.poll()`을 끼워 넣는 것으로 충분하다. print 모드도 단순 sleep
  루프면 된다. 스레드+채널은 동기화 코드만 늘어나고 이 규모(파일 하나, 단일 프로세스)에서 얻는
  이득이 없다.
- **Trade-offs**: 폴링 주기(300ms) 동안은 파일 변경과 키 입력이 정확히 동시에 오면 키 입력이
  먼저 처리된 뒤 다음 루프에서 파일 변경이 반영된다 — 체감 차이 없음.
- **Follow-up**: 없음.

## Risks & Mitigations
- 폴링 주기 동안 여러 번 저장되면 중간 상태를 놓칠 수 있음 — 완화: 미리보기 목적상 마지막 상태만
  보이면 충분, 요구사항에도 "모든 중간 상태"를 요구하지 않음
- 에디터가 원자적 저장(임시 파일 쓰고 rename)을 쓰면 그 찰나에 `stat`이 실패할 수 있음 — 완화:
  `Watcher`가 이를 `ReadError`(짧게 스칠 가능성 높음) → 다음 폴에서 새 mtime 감지 → `Changed`로
  자연 복구(5.2)하도록 설계됨, 사용자 개입 불필요

## References
- `src/pager.rs:74-118` — 기존 이벤트 루프(재렌더링 트리거 조건, 블로킹 `event::read`)
- `src/main.rs:20-76` — 기존 `run()`(소스 캡처, 페이저/print 분기)
- `Cargo.toml` — 의존성 4개 원칙, `cli` feature 게이팅
