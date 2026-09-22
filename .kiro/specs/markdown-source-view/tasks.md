# Implementation Plan — markdown-source-view

## 정의
마크다운 문서를 보는 사용자를 위해, 렌더된 화면과 마크다운 문법이 색으로 구분된 원문 화면을
문서 전체·블록 단위로 오가며 보고, 화면의 텍스트를 드래그해 선택·복사할 수 있게 하는 기능이다.

- [x] 1. Foundation — 파싱 기반·독립 유틸리티
- [x] 1.1 오프셋 기반 파싱으로 전환
  - DONE: `Parser::new_ext(...).into_offset_iter()`로 바꿔도 기존 렌더 결과가 그대로라 기존
    227개+ 렌더링 테스트가 수정 없이 통과함
  - _Requirements: 2.1_
  - _Difficulty: mid_
  - _Boundary: markdown::Renderer_
- [x] 1.2 (P) 클립보드 OSC 52 시퀀스 생성
  - DONE: `clipboard_sequence()`가 텍스트를 base64로 감싼 OSC 52 이스케이프 문자열을 돌려주는
    유닛 테스트(빈 문자열·유니코드 텍스트 포함)가 통과함
  - _Requirements: 4.1, 4.5_
  - _Difficulty: low_
  - _Boundary: links 모듈_
- [x] 1.3 (P) Line 열→바이트 변환 공유 헬퍼 추출
  - DONE: 기존 `highlight_span` 테스트가 리팩터 후에도 그대로 통과하고, 새 열 기반 부분 문자열
    추출 함수가 같은 헬퍼로 CJK 폭이 섞인 줄에서도 올바른 구간을 돌려주는 테스트가 통과함
  - _Requirements: 4.1_
  - _Difficulty: low_
  - _Boundary: line 모듈_
- [x] 1.4 비다이어그램 블록 오프셋 수집
  - DONE: 중첩 리스트를 포함한 문서를 렌더링하면 `Document.text_blocks`가 서로 겹치지 않는
    구간들로 채워지고, 각 블록의 원문이 원본 문자(마크업 포함) 그대로 슬라이스됨을 확인하는
    테스트가 통과함
  - _Requirements: 2.1, 2.3, 2.4_
  - _Difficulty: high_
  - _Boundary: markdown::Renderer_
  - _Depends: 1.1_

- [x] 2. Core — 원문 하이라이팅과 블록 토글
- [x] 2.1 마크다운 문법 하이라이팅 렌더링
  - DONE: 헤딩·강조·링크·인용·목록·코드펜스가 섞인 원문을 렌더링하면 각 문법 마커 위치에
    해당 테마 색이 덧씌워지고 원문 문자는 그대로 보존됨을 확인하는 테스트, `--style none`에서도
    문자 구성이 안 바뀜을 확인하는 테스트가 통과함
  - _Requirements: 3.1, 3.2, 3.3_
  - _Difficulty: high_
  - _Boundary: markdown 렌더러_
  - _Depends: 1.1_
- [x] 2.2 pager 블록 토글을 텍스트 블록까지 일반화
  - DONE: 다이어그램과 텍스트 블록이 섞인 문서에서 `o` 키·클릭이 블록 종류를 가리지 않고
    화면에 보이는 첫 블록을 토글하며, 다이어그램은 기존처럼 원문을 캡션 아래 덧붙이고 텍스트
    블록은 그 구간의 렌더 줄을 원문으로 대체함을 확인하는 테스트, 다이어그램 단독 기존 회귀
    테스트가 그대로 통과함
  - _Requirements: 2.1, 2.2, 2.3, 2.4_
  - _Difficulty: high_
  - _Boundary: pager 상태_
  - _Depends: 1.4, 2.1_
- [x] 2.3 전역 원문 토글 키
  - DONE: `s` 키를 누르면 화면 전체가 원문 하이라이팅 결과로 바뀌고, 다시 누르면 스크롤
    위치를 유지한 채 기존 렌더 화면으로 돌아오며, 그 사이 블록별 펼침 상태가 그대로 유지됨을
    확인하는 테스트, 감시 모드에서 전역 원문 내용이 갱신됨을 확인하는 테스트가 통과함
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_
  - _Difficulty: mid_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 2.1_

- [x] 3. Core — 드래그 선택과 클립보드
- [x] 3.1 마우스 프로토콜을 button-event 추적으로 전환
  - DONE: 마우스 모드 이스케이프 시퀀스가 button-event 추적(+SGR)으로 바뀌어도 기존 휠 스크롤
    테스트가 그대로 통과함(회귀 없음)
  - _Requirements: 4.1_
  - _Difficulty: low_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 2.3_
- [x] 3.2 Down/Drag/Up 상태 기계로 드래그·클릭 구분
  - DONE: Down 후 Drag 없이 Up이 오면 기존 `click_at`이 그대로 실행되고(링크 따라가기·블록
    토글 회귀 없음), Down-Drag-Up이 오면 클릭 액션 대신 선택 상태로 전환됨을 확인하는 테스트가
    통과함
  - _Requirements: 4.2_
  - _Difficulty: high_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 3.1_
- [x] 3.3 선택 구간 텍스트 추출과 클립보드 전달
  - DONE: 여러 줄에 걸친 드래그 구간에서 스트림(읽기) 순서로 텍스트가 정확히 추출돼 대기
    중인 클립보드 상태에 저장되고, 다음 화면 그리기에서 클립보드 시퀀스가 출력된 뒤 비워짐을
    확인하는 테스트, 전역 원문 화면에서도 동일하게 동작함을 확인하는 테스트가 통과함
  - _Requirements: 4.1, 4.3, 4.5_
  - _Difficulty: high_
  - _Boundary: pager 그리기_
  - _Depends: 1.2, 1.3, 3.2_
- [x] 3.4 화면 그리기에서 선택 구간 시각 강조
  - DONE: 드래그 진행·완료 상태에서 화면 그리기가 해당 구간에 역상 스타일을 덧씌운 줄을
    렌더함을 렌더된 스타일 기준으로 확인하는 테스트, 마우스 휠 스크롤이 선택 상태를 건드리지
    않음을 확인하는 테스트가 통과함
  - _Requirements: 4.1, 4.4_
  - _Difficulty: mid_
  - _Boundary: pager 그리기_
  - _Depends: 3.2_

- [x] 4. Integration
- [x] 4.1 새 키 입력 시 진행 중인 선택 강조 해제
  - DONE: 드래그 강조가 남아 있는 상태에서 임의의 키를 누르면 강조가 사라짐을 확인하는
    테스트가 통과함
  - _Requirements: 4.1_
  - _Difficulty: low_
  - _Boundary: pager 이벤트 루프_
  - _Depends: 3.4_
- [x] 4.2 상태 표시줄 힌트 갱신
  - DONE: 원문 보기·블록 토글이 가능한 문서를 연 상태 표시줄에 전역 토글 키 힌트가 보임을
    확인하는 테스트가 통과함
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: pager 그리기_
  - _Depends: 2.3_

- [x] 5. Validation
- [x] 5.1 회귀 확인: 기존 테스트 스위트 + clippy
  - DONE: `cargo test` 전량 통과(기존 페이저·마크다운·링크 테스트 포함), `cargo clippy
    --all-targets` 클린
  - _Requirements: 5.1, 5.2_
  - _Difficulty: low_
  - _Boundary: 전체_
  - _Depends: 4.1, 4.2_
- [x] 5.2 실물 검증: 실제 파일로 전역/블록 토글·하이라이팅 확인
  - DONE: 헤딩·강조·링크·목록·다이어그램이 섞인 실제 마크다운 파일을 릴리스 바이너리로
    렌더링해 원문 하이라이팅·전역 토글 결과를 확인(이 세션엔 실제 터미널이 없어 로직 단위
    자동 테스트로 대체 가능한 부분은 그렇게 하고, 드래그·클립보드처럼 육안 확인이 꼭 필요한
    부분은 미검증으로 명시)
  - _Requirements: 1.1~1.5, 2.1~2.4, 3.1~3.3, 4.1~4.5_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 5.1_
