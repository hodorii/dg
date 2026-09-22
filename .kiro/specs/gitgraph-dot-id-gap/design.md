# Design — gitgraph-dot-id-gap

## 정의
gitGraph 커밋 점과 id 글자 사이에 최소 한 칸 공백을 두어, 점 글자의 실제 터미널 렌더 폭이
가정한 1칸을 넘는 환경에서도 id 첫 글자가 가려지지 않게 하는 결함 수정이다.

## 원인 (Root Cause)
`diagram::layout::gitgraph`의 두 렌더 함수 모두, 커밋 점을 찍은 바로 다음 칸에 공백 없이 id
텍스트를 그린다:
- `build()`(가로): `canvas.text(x(dot.column) + 1, dot.row, id, ...)` — 점 칸(`x(dot.column)`)
  바로 오른쪽 칸부터 시작
- `build_vertical()`(세로): `canvas.text(x(dot.row) + 1, top + dot.column, id, ...)` — 동일하게
  점 칸 바로 아래 칸(전치 좌표) 시작

`unicode-width` 기준으로 `●`/`◉`는 1칸이라 dg의 내부 텍스트 자체는 정확하지만(1.x, `od -c`로
확인됨), 실제 터미널 폰트가 그 원 글자를 시각적으로 1칸보다 넓게 그리면 완충 칸이 전혀 없어
바로 다음 글자가 가려진다.

## 수정 방식
텍스트 시작 x 좌표를 점 칸의 `+1`(점 바로 다음)에서 `+2`(점 다음 칸 하나를 비움)로 민다 —
가로·세로 두 곳 모두. 그에 맞춰 폭 계산도 같이 옮긴다:
- 가로 `build()`: `total`을 구하는 `x(dot.column) + 1 + width_of(id)` → `+ 2 + width_of(id)`
- 세로 `build_vertical()`: `col`을 구하는 `id_width + 1` → `id_width + 2`
- `fork_connector_start`(`gitgraph-junction-polish`가 추가한, "부모 커밋 id 뒤 → 분기 연결선"
  간격 계산)도 텍스트 시작점이 한 칸 밀렸으므로 `parent_x + 1 + width + 1` →
  `parent_x + 2 + width + 1`로 맞춘다(3.2 불변 동작 — 결과적으로 연결선 시작 위치가 한 칸 더
  밀리지만, 이는 "점 뒤 최소 한 칸"이라는 이번 수정과 정확히 일관된 결과이지 회귀가 아니다)

id가 비어 있으면(`if !id.is_empty()`) 텍스트를 아예 안 그리므로 점 다음 칸을 밀 필요가 없다 —
기존처럼 `+1` 자리를 그대로 두어 불필요한 여백을 만들지 않는다(3.1).

**기각한 대안**: 점 글자 자체를 폭 2로 재정의(`unicode-width` override) — 모든 다이어그램형이
공유하는 `canvas.rs`의 글자 폭 가정을 gitGraph 하나 때문에 바꾸는 건 영향 범위가 과함. 텍스트
시작 위치만 미는 쪽이 `gitgraph.rs` 안에서 끝나는 최소 수정이다.

## 검증 속성
- (a) 결함 재현: `x(dot.column)+1`/`x(dot.row)+1`에 텍스트가 시작되는 기존 코드로 렌더링한
  골든 텍스트에서 점과 id 사이 공백이 0칸임을 확인하는 테스트(수정 전에는 통과, 수정 후 실패
  하도록 작성 — 1.1/1.2)
- (b) 기대 동작: 수정 후 같은 렌더링에서 점과 id 사이에 공백이 최소 1칸 있음을 확인(2.1/2.2)
- (c) 불변 동작: 빈 id 커밋의 레이아웃이 이전과 동일(3.1), `gitgraph-junction-polish`의 세로
  모드 간격·`gitgraph-branch-distinction`의 색·선패턴·둥근 모서리가 유지됨(3.2/3.3), 기존
  `gitgraph.rs` 테스트 스위트가 전량 통과(3.4)

## 영향 범위
- `src/diagram/layout/gitgraph.rs`만 수정(`build`/`build_vertical`/`fork_connector_start`) —
  이 파일을 소유하는 세 스펙(`gitgraph-vertical-mode`/`gitgraph-branch-distinction`/
  `gitgraph-junction-polish`) 중 어느 것의 Boundary Commitment도 어기지 않는다(색상·패턴·모서리
  로직은 그대로, 텍스트 시작 좌표만 이동)
- `examples/architecture.txt` 재생성 필요(커밋 id가 있는 gitGraph 예제 포함)
