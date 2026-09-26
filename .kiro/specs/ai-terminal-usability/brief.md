# Brief: ai-terminal-usability

## Problem
dg 사용자, 그중에서도 Claude Code 같은 AI 코딩 에이전트와 터미널에서 함께 작업하는 개발자(herdr,
tmux 등으로 화면을 나눠 씀)가 대상이다. dg는 지금까지 "혼자 문서를 훑어보는 뷰어" 기준으로
설계·검증돼 왔고, "AI가 실시간으로 마크다운·다이어그램을 고치는 동안 사람이 옆 창에서 지켜보거나,
AI 자신이 렌더링 결과를 확인해야 하는" 워크플로우에서 겪는 구체적 마찰은 아직 조사된 적이 없다.

## Current State
- watch 모드(VC-DG-VIEW-WATCH, `dg-watch-mode` 스펙 completed)는 단일 파일 mtime 폴링으로
  변경을 감지해 스크롤 위치를 유지한 채 재렌더링한다.
- 페이저 키·마우스 휠·검색·원문 토글은 "혼자 보는" 시나리오 기준으로 검증돼 있다(README,
  `dg-watch-mode` requirements 6.1).
- herdr/tmux처럼 화면을 여러 창으로 나눠 쓰는 환경, 또는 AI 에이전트가 dg 출력을 스스로
  소비·검증해야 하는 시나리오에 대한 사용성 검증은 없다.

## Desired Outcome
AI와 함께 터미널에서 개발하는(herdr/tmux 등 분할 창 활용) 워크플로우에서 dg를 쓸 때 겪는 구체적
마찰이 목록화되고, 우선순위가 매겨진 개선 항목(승인 기준)으로 정리되어 있다.

## Approach
조사 선행형 단일 스펙. research.md에 실사용 시나리오별 마찰 지점을 먼저 기록하고, 검증된 항목만
requirements.md의 승인 기준으로 전환한다.
기각한 대안: (1) 구체적 문제 확인 전에 "AI 통합" 신규 기능부터 설계하는 것 — 근거 없는 확장.
(2) 지금 바로 다중 스펙으로 쪼개는 것 — 무엇이 문제인지 모르는 상태에서의 조기 분해.

## Scope
- **In**: 분할 화면 좁은 폭에서의 다이어그램·표 렌더링, watch 재렌더링 시 시각적 소음(깜빡임 등),
  dg 출력을 사람이 AI에게 복사해 다시 붙여넣는 흐름, dg 출력의 성공/실패(폴백 여부)를 에이전트가
  스스로 판별할 수단, 배색 자동판별(`COLORFGBG` 등)이 herdr 같은 멀티플렉서에서도 유효한지
- **Out**: 새 다이어그램 언어 지원, herdr/tmux 자체의 기능 변경, 이미 검증된 페이저 키 조작 재설계

## Boundary Candidates
- dg는 herdr/tmux가 물려주는 폭·배색 정보를 읽기만 한다 — 그 값 자체가 왜 틀렸는지는 dg 책임
  밖(필요하면 Out of Boundary로 기록)
- "사람이 보는 출력"과 "에이전트가 파싱하는 출력"은 서로 다른 소비자 — 후자를 위한 신호(종료
  코드, 폴백 여부 등)가 필요하면 기존 사람용 출력 포맷은 건드리지 않고 별도 표면으로 분리한다

## Out of Boundary
- herdr/tmux 자체 설정·버그 수정
- 렌더링 배치 알고리즘 자체의 품질 개선(기존 다이어그램 스펙들이 이미 다룬 영역)

## Upstream / Downstream
- **Upstream**: VC-DG-VIEW-WATCH(`dg-watch-mode`, completed), VC-DG-VIEW-PAGER
- **Downstream**: 조사에서 크고 독립적인 항목이 나오면 별도 스펙으로 분리될 수 있음(roadmap.md
  참고)

## Existing Spec Touchpoints
- **Extends**: `dg-watch-mode`(재렌더링 시 시각적 경험) — 단, completed·승인된 스펙 자체를
  재오픈하지 않고 새 스펙에서 다룬다
- **Adjacent**: `markdown-source-view`(원문 토글), 좁은 폭 재시도를 쓰는 다이어그램 계열 스펙들

## Constraints
- 의존성 4개·단일 정적 바이너리 원칙(README) 유지
- 기존 사람용 텍스트 출력 포맷(스크린샷 없이 붙여넣기 가능해야 함) 하위호환
