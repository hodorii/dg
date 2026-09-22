# Implementation Plan — markdown-link-navigation

## 정의
마크다운 문서를 보는 사용자를 위해, 문서 안 링크를 따라 문서 내 헤딩으로 점프하거나 외부
브라우저를 열거나 다른 `.md` 파일로 이동(뒤로/앞으로 히스토리 포함)할 수 있게 하는 기능이다.

- [x] 1. links 모듈 — 분류·위치 인식·슬러그
- [x] 1.1 `LinkKind`/`classify()` 구현
  - DONE: `classify()`가 `#slug`→`Anchor`, `http(s)://`/`mailto:`→`External`, 그 외(빈 문자열
    포함)→`File`로 분류하는 유닛 테스트가 통과함
  - _Requirements: 2.1, 3.1, 4.1_
  - _Difficulty: low_
  - _Boundary: links 모듈_
- [x] 1.2 `LinkPosition`/`locate_links()` 구현
  - DONE: 밑줄(`underline`) 스타일 구간이 있는/없는 `Line` 픽스처(굵게+링크가 섞인 경우 포함)로
    `locate_links()`를 호출해 구간이 등장 순서대로 `link_index`를 받는지, 구간이 없으면 빈
    벡터를 돌려주는지 확인하는 테스트가 통과함
  - _Requirements: 1.1_
  - _Difficulty: mid_
  - _Boundary: links 모듈_
- [x] 1.3 `Slugger` 구현
  - DONE: 같은 제목 헤딩 여러 개를 순서대로 `slug()`에 넣어 GitHub 스타일(소문자·하이픈,
    중복 시 `-1`/`-2`)이 나오는 유닛 테스트가 통과함
  - _Requirements: 2.2, 2.3_
  - _Difficulty: low_
  - _Boundary: links 모듈_

- [x] 2. links 모듈 — 히스토리·외부 열기·경로 해석
- [x] 2.1 `History`(`record`/`go_back`/`go_forward`) 구현
  - DONE: 빈 히스토리에서 `go_back`/`go_forward`가 `None`을 돌려주고(4.7), 여러 항목을
    `record`한 뒤 `go_back` → `go_forward`가 왕복되는지, `go_back` 후 `record`하면 forward가
    비워지는지(4.8) 확인하는 유닛 테스트가 통과함
  - _Requirements: 4.5, 4.6, 4.7, 4.8_
  - _Difficulty: mid_
  - _Boundary: links 모듈_
- [x] 2.2 `open_external()`/`resolve_file_path()` 구현
  - DONE: `resolve_file_path()`가 상대 경로를 `base_dir` 기준으로 올바르게 합치는 유닛 테스트,
    `open_external()`이 실제 OS 명령을 셸아웃해(플랫폼별 분기) `Result`를 돌려주는 것을 확인
  - _Requirements: 3.1, 3.2, 4.1_
  - _Difficulty: low_
  - _Boundary: links 모듈_

- [x] 3. Document 확장
- [x] 3.1 `Document.links`/`headings` 채우기
  - DONE: `Tag::Link` 시작 시 `dest_url`을 `links`에 순서대로 push, `TagEnd::Heading`에서
    `Slugger`로 슬러그를 만들어 `(슬러그, 원본 줄 번호)`를 `headings`에 push하도록 수정해,
    링크·헤딩이 섞인 문서를 렌더링하면 두 목록이 실제 등장 순서와 일치함을 확인하는 테스트가
    통과함
  - _Requirements: 1.1, 2.1, 2.2, 2.3_
  - _Difficulty: mid_
  - _Boundary: markdown 렌더러_
  - _Depends: 1.3_

- [x] 4. pager — 위치 인식 배선
- [x] 4.1 `rebuild_lines()`에서 `link_positions`/`heading_lines` 갱신
  - DONE: `rebuild_lines()` 끝에서 `links::locate_links(&self.lines)`로 `link_positions`를,
    원본→최종 줄 매핑으로 보정한 `heading_lines`를 채우도록 수정해, 다이어그램 원문을 펼친
    뒤(`o` 토글)에도 두 목록의 줄 번호가 실제 화면과 일치함을 확인하는 테스트가 통과함
  - _Requirements: 1.1_
  - _Difficulty: high_
  - _Boundary: pager 상태_
  - _Depends: 3.1, 1.2_

- [x] 5. pager — 키보드 포커스·확정
- [x] 5.1 Tab/Shift-Tab 포커스 이동 + 시각 강조
  - DONE: `KeyCode::Tab`/`BackTab`이 현재 뷰포트 안의 링크 사이에서 `focused_link`를
    옮기고(포커스 없을 때 첫 링크부터), 화면에 링크가 없으면 무동작임을 확인하는 테스트가
    통과함. `draw()`가 포커스된 구간에 역상을 덧씌우는 것을 렌더된 스타일로 확인
  - _Requirements: 1.2, 1.3, 1.4_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 4.1_
- [x] 5.2 Enter 조건부 분기(포커스 있으면 링크 따라가기, 없으면 기존 스크롤)
  - DONE: 포커스된 링크가 있을 때 Enter가 `follow_link`를 호출하고 포커스를 지우는지, 포커스가
    없을 때는 기존처럼 `top`이 1 줄 내려가는지(회귀) 확인하는 테스트가 통과함
  - _Requirements: 1.5, 5.1_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 5.1_

- [x] 6. pager — 마우스 클릭 확장
- [x] 6.1 클릭 좌표 → 링크 우선 판정
  - DONE: 마우스 왼쪽 클릭 좌표가 `link_positions` 범위 안이면 `follow_link`를 호출하고, 범위
    밖이면 기존 `toggle_block_at`으로 넘어가는지 확인하는 테스트가 통과함(다이어그램 클릭
    토글 회귀 없음)
  - _Requirements: 1.1, 1.5, 5.1_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 4.1_

- [x] 7. pager — 링크 따라가기(종류별 동작)
- [x] 7.1 앵커 점프
  - DONE: `follow_link`가 `Anchor(slug)`를 `heading_lines`에서 찾아 `top`을 그 줄로 옮기고,
    못 찾으면 상태 메시지를 남기고 위치를 유지하는 테스트가 통과함
  - _Requirements: 2.1, 2.4_
  - _Difficulty: mid_
  - _Boundary: pager 링크 따라가기_
  - _Depends: 5.2_
- [x] 7.2 외부 URL 열기
  - DONE: `follow_link`가 `External(url)`을 `links::open_external`로 넘기고, 실패 시 상태
    메시지를 남기며 페이저가 계속 동작하는 테스트가 통과함
  - _Requirements: 3.1, 3.2_
  - _Difficulty: low_
  - _Boundary: pager 링크 따라가기_
  - _Depends: 7.1_
- [x] 7.3 다른 파일로 이동(+ 감시 모드 연동)
  - DONE: `follow_link`가 `File(path)`를 `base_dir` 유무로 분기해(4.2), 성공 시
    `history.record` 후 `source`/`title`/`base_dir`/`current_path`/`top=0`을 갱신하고
    감시 중이면 `Watcher`를 새 경로로 재생성하며(4.4), 실패 시 상태 메시지를 남기고 기존
    화면을 유지하는(4.3) 테스트가 통과함
  - _Requirements: 4.1, 4.2, 4.3, 4.4_
  - _Difficulty: high_
  - _Boundary: pager 링크 따라가기_
  - _Depends: 7.2, 2.1_
- [x] 7.4 `[`/`]` 뒤로·앞으로 키
  - DONE: `[`/`]`가 `history.go_back`/`go_forward`로 이전/다음 파일·위치를 복원하는지(4.5,
    4.6), 히스토리가 없을 때 무동작인지(4.7) 확인하는 테스트가 통과함
  - _Requirements: 4.5, 4.6, 4.7_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 7.3_

- [x] 8. main.rs 배선
- [x] 8.1 `base_dir` 계산 및 `Pager::new` 전달
  - DONE: 파일 경로 실행이면 그 파일의 부모 디렉터리를, 표준입력이면 `None`을 `Pager::new`에
    넘기도록 수정되어, `dg 파일.md`로 열었을 때 상대 링크가 그 파일 기준으로 풀림을 확인하는
    테스트가 통과함
  - _Requirements: 4.1, 4.2_
  - _Difficulty: low_
  - _Boundary: main.rs_
  - _Depends: 7.3_

- [x] 9. 검증
- [x] 9.1 회귀 확인: 기존 테스트 스위트 + clippy
  - DONE: `cargo test` 전량 통과(기존 페이저·마크다운 테스트 포함), `cargo clippy --all-targets`
    클린
  - _Requirements: 5.1, 5.2_
  - _Difficulty: low_
  - _Boundary: 전체_
  - _Depends: 8.1_
- [x] 9.2 실물 검증: 실제 파일 2개로 링크 이동 확인
  - DONE: 서로 링크로 연결된 마크다운 파일 2개(앵커·외부·파일 링크 섞음)를 만들어 릴리스
    바이너리로 렌더링 결과를 확인(이 세션엔 실제 터미널이 없어 로직 단위 자동 테스트로
    대체 가능한 부분은 그렇게 하고, 육안 확인이 꼭 필요한 부분은 미검증으로 명시)
  - _Requirements: 1.1~1.5, 2.1~2.4, 3.1~3.2, 4.1~4.8_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 9.1_
