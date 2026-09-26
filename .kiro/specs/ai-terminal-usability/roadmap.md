# Roadmap

## Overview
AI와 함께 터미널에서(herdr, tmux 등) 개발하는 워크플로우를 전제로 dg의 사용성 마찰을 조사해 개선
항목을 도출한다. 대상이 아직 구체적 항목으로 확정되지 않았으므로 조사(research.md)를 먼저 하는
단일 스펙으로 시작하고, 조사 결과 서로 독립적인 큰 항목이 나오면 그때 분리한다.

## Approach Decision
- **Chosen**: 조사 선행형 단일 스펙(`ai-terminal-usability`) — research.md로 마찰 지점을 먼저
  확인한 뒤 requirements.md로 전환
- **Why**: 무엇이 문제인지 확정되지 않은 상태에서 여러 스펙으로 미리 쪼개면 근거 없는 범위 확장이
  된다. 분할 화면 폭·watch 재렌더링·에이전트 소비 가능성은 서로 얽혀 있어 한 경계 안에서 함께
  보는 편이 낫다.
- **Rejected alternatives**: (1) 즉시 다중 스펙으로 분해 — 근거 부족. (2) 기존 `dg-watch-mode`
  스펙을 재오픈해 추가 — 이미 completed·승인된 스펙의 경계를 사후에 넓히는 것은 SRP 위반(새
  관심사는 새 스펙).

## Scope
- **In**: 분할 화면 폭 대응, watch 재렌더링 시 시각 경험, 사람↔AI 간 결과 전달(복사/재입력),
  에이전트가 렌더 성공 여부를 스스로 판별하는 수단, 멀티플렉서 환경에서의 배색 자동판별
- **Out**: herdr/tmux 자체 변경, 신규 다이어그램 언어, 이미 검증된 페이저 키 조작

## Constraints
- 의존성 4개·단일 정적 바이너리 유지
- 기존 사람용 출력 포맷 하위호환

## Boundary Strategy
- **Why this split**: 지금은 "조사"와 "구현"을 한 스펙 안에 두되, research.md 단계에서
  독립적으로 크고 분리 가능한 항목(예: 에이전트 전용 출력 모드처럼 완전히 새로운 표면)이 나오면
  그 항목만 별도 스펙으로 뗀다
- **Shared seams to watch**: `watch.rs`(재렌더링) · `pager.rs`(표시/키) · 좁은 폭 재시도 로직
  (다이어그램 배치기) — 이 스펙의 변경이 이 세 지점과 만나면 기존 스펙들과의 경계를 재확인한다

## Specs (dependency order)
- [ ] ai-terminal-usability -- herdr/tmux 등 분할 터미널 + AI 협업 워크플로우에서 dg 사용성을
  조사하고 개선한다. Dependencies: none

<!-- 조사 결과에 따라 분리될 수 있음(확정 아님, research.md 완료 후 채움):
## Existing Spec Updates
(해당 없음)

## Direct Implementation Candidates
(해당 없음)
-->
