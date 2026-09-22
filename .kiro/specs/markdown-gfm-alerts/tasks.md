# Implementation Plan — markdown-gfm-alerts

## 정의
GitHub 스타일 문서를 보는 사용자를 위해, GFM 알림 블록을 종류별로 구분되는 라벨·색으로 보여주는
기능이다.

- [x] 1. GFM 알림 렌더링
- [x] 1.1 `ENABLE_GFM` 활성화 + `alert_style()` 구현
  - DONE: `render_document`의 `Options`에 `ENABLE_GFM`을 추가하고, `alert_style(theme, kind)`가
    5개 variant 전부에 대해 라벨 문자열과 스타일을 반환하는 유닛 테스트가 통과함(Note와
    Caution만 같은 색이고 Caution만 굵음을 확인)
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_
  - _Difficulty: low_
  - _Boundary: markdown 렌더러_
- [x] 1.2 `Tag::BlockQuote` 분기에 라벨 삽입 적용
  - DONE: `Tag::BlockQuote(_)`가 `kind`를 받도록 바뀌고, `Some(kind)`면 인용문 첫 줄에
    `alert_style`의 라벨을 그리도록 수정되어, 다섯 종류 알림을 렌더링하면 라벨이 보이고
    원본 `[!NOTE]` 마커가 본문에 안 남는 것을 확인하는 테스트가 통과함
  - _Requirements: 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: markdown 렌더러_
  - _Depends: 1.1_

- [x] 2. 검증
- [x] 2.1 회귀 테스트: 일반 인용문 · 미지원 마커
  - DONE: 마커 없는 일반 `>` 인용문과 `[!UNKNOWN]` 마커 픽스처가 기존과 동일한 골든 텍스트로
    렌더링됨을 확인하는 테스트가 통과함
  - _Requirements: 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: markdown 렌더러_
  - _Depends: 1.2_
- [x] 2.2 `--style none` 라벨 식별성 테스트
  - DONE: `Theme::none()`으로 다섯 종류를 렌더링해 색이 빠져도 라벨 텍스트만으로 다섯 종류가
    서로 구분됨(텍스트가 서로 다름)을 확인하는 테스트가 통과함
  - _Requirements: 1.7_
  - _Difficulty: low_
  - _Boundary: markdown 렌더러_
  - _Depends: 1.2_
- [x] 2.3 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(기존 마크다운 테스트 포함) — 최초 1회 무관한 테스트에서 일시적
    실패가 있었으나(이번 변경과 무관, `markdown::*` 테스트는 그 실행에서도 전부 통과) 연속
    3회 재실행에서 재현 안 됨(227개 전량 통과), `cargo clippy --all-targets` 클린, 릴리스
    바이너리로 다섯 종류 알림이 섞인 실제 문서를 `dg -P -s dark`로 렌더링해 패닉 없이 끝나고
    라벨·색이 육안으로 확인됨
  - _Requirements: 4.1, 4.2_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 2.1, 2.2_
