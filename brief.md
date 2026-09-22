# Brief: gitGraph 분기·병합 지점 시각 정리

## Problem
사용자가 gitGraph에서 두 가지를 보고했다: (1) 분기·병합 지점에 둥근 모서리를 적용해 달라,
(2) 커밋 id 글자가 분기·병합 지점과 겹쳐(정확히는 간격 없이 붙어) 읽기 어렵다. 둘 다 실제로
재현된다.

## Current State
- **모서리**: gitGraph의 분기·병합 연결선(`canvas.hline`/`vline`)은 `round` 인자를 받지 않는
  공개 함수만 쓴다(`layout/gitgraph.rs`). `canvas.rect()`/`canvas.join()`은 이미 `round: bool`을
  받아 모서리를 `╭╮╰╯`로 그릴 수 있지만(`canvas.rs:187,192`), gitGraph는 이 경로를 안 쓴다 —
  분기·병합 연결선이 트랙 선과 만나는 지점은 전부 각진 모서리(`┌┐└┘`, 굵은선이면 `┏┓┗┛`)다.
- **글자 겹침(정확히는 간격 없음)**: 세로 모드(`gitGraph TB:`)에서 재현된다. `branch`(분기)는
  새 스텝(행)을 쓰지 않고 부모의 **가장 최근 커밋과 같은 행**에서 갈라진다(`anchor =
  tips[parent]`, `layout/gitgraph.rs`의 `plan()`). 그 결과 분기 연결선(가로선)이 **부모 커밋의
  id 글자와 같은 행**에 그려지는데, 연결선이 글자보다 먼저 그려지고 글자가 나중에 그 위를
  덮어써서 실제 겹침(글자 훼손)은 없지만, 글자가 끝나자마자 간격 없이 바로 선(`╌`/`━`)이
  시작돼 시각적으로 붙어 보인다. 실제 재현:
  ```
  ●root-long-id╌╌╌╌┐     ← "id" 바로 뒤에 공백 없이 파선 시작
  │                ●dev1
  ◉╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘
  ```
  가로 모드(기본)는 이 문제가 없다 — 분기 연결선이 세로선이라 부모 행의 글자(가로로 이어짐)와
  겹치는 열이 다르다(글자는 열 anchor+1부터, 연결선은 열 anchor 하나뿐).
- 병합(`merge`)은 항상 새 스텝을 쓰므로(`next_column` 증가) 기존 커밋 행과 겹치지 않는다 —
  이 문제는 **분기** 시점에서만 재현된다. 다만 사용자 표현("머지 시")은 분기·병합을 아우르는
  "브랜치 구조가 바뀌는 지점" 정도의 넓은 의미로 보인다.
- 직전 스펙(`gitgraph-branch-distinction`)에서 트랙별 색+선패턴을 이미 넣었고, 이번 항목은 그
  위에 얹는 순수 시각 정리(모서리 둥글기, 간격)라 겹치는 로직은 없다.

## Desired Outcome
- gitGraph의 분기·병합 연결선이 트랙 선과 만나는 지점이 둥근 모서리로 그려진다(가로·세로 모드
  모두, 기존 `canvas.rect()`가 쓰는 것과 같은 `round` 메커니즘 재사용).
- 세로 모드에서 분기가 부모의 최근 커밋 행과 같은 행에 그려질 때, 커밋 id 글자와 분기 연결선
  사이에 최소 한 칸 공백이 생겨 글자와 선이 시각적으로 붙지 않는다.

## Approach
두 항목 다 `layout/gitgraph.rs` 안에서 끝나는 작은 렌더링 정리이고 신규 의존성이 없다. 서로
다른 메커니즘(모서리는 `canvas.join(..., round: true)` 호출 추가, 간격은 연결선의 시작 좌표
계산 조정)이지만 같은 파일·같은 "분기·병합 연결선" 영역을 다듬는 하나의 작업이라 스펙 하나로
묶는다(둘 다 따로 스펙 게이트를 거치기엔 각각 몇 줄 안 되는 변경). SRP상 분리할지는
requirements에서 최종 확인.

## Scope
- **In**: gitGraph 분기·병합 연결선의 모서리를 둥글게(가로·세로 모드 모두), 세로 모드에서
  분기 연결선과 부모 커밋 id 글자 사이 최소 간격 확보
- **Out**: 다른 다이어그램형(block-beta, sequence 등)의 모서리 둥글기(요청 밖), gitGraph
  배치·색상 로직 자체 변경(`gitgraph-vertical-mode`/`gitgraph-branch-distinction` 소관, 이미
  완료), 가로 모드의 간격 문제(재현 안 됨 — 애초에 해당 없음)

## Boundary Candidates
- gitGraph 분기·병합 연결선 모서리 둥글기
- 세로 모드 분기 연결선-커밋 id 글자 간격

## Out of Boundary
- 다른 다이어그램형의 모서리 스타일 일괄 변경 — 요청 범위 밖, 별도 검토 필요
- 가로 모드 간격 문제 — 재현되지 않아 대상 아님

## Upstream / Downstream
- **Upstream**: `diagram::layout::gitgraph`(`build`/`build_vertical`, `plan()`의 `anchor` 계산),
  `diagram::canvas`(`join`/`rect`의 기존 `round` 메커니즘, 신규 API 불필요할 가능성 높음)
- **Downstream**: 기존 gitgraph 테스트 중 정확한 모서리 글자(`┘`/`└` 등)를 비교하는 테스트가
  있으면 둥근 글자(`╯`/`╰`)로 조정 필요, `examples/architecture.txt`의 gitGraph 절 재생성
  가능성

## Existing Spec Touchpoints
- **Extends**: 없음(신규 스펙)
- **Adjacent**: `gitgraph-vertical-mode`(세로 모드 배치, 이번에 간격 문제가 나는 위치),
  `gitgraph-branch-distinction`(트랙별 색+선패턴, 이번 항목과 같은 파일이지만 독립적)

## Constraints
- 신규 의존성 없음
- panic 금지 계약 유지
- 기존 `canvas.rs`의 `round`/`LineKind` 메커니즘 재사용 — 새 셀 필드·새 글자 추가하지 않음
