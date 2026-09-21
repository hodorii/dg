---
inclusion: always
---

# Git Workflow

## Feature 브랜치
- 기능 추가·동작 변경 작업은 `main`에 직접 커밋하지 않는다. 브랜치를 만들어 그 위에서 작업한다.
- 브랜치 이름: `{{SPECS}}` 스펙 기반 작업은 `feat/<spec-name>`(예: `feat/gitgraph-branch-distinction`), 스펙 없는 버그 수정은 `fix/<short-desc>`, 그 외 유지보수성 변경은 `chore/<short-desc>`.
- 작업은 로컬에서 커밋을 쌓아 가며 진행한다. push는 커밋마다가 아니라 **브랜치 단위**로 한다(작업이 끝났을 때 그 브랜치를 통째로 origin에 push).
- README 오타 수정처럼 사소한 편집은 브랜치 없이 바로 `main`에 커밋해도 된다 — 실제 동작·기능이 바뀌는 작업에 이 규칙을 적용한다.

## 기록: 의도와 결과가 브랜치 기록으로 남아야 함
- 브랜치를 열 때(첫 커밋 또는 그 직후) 무엇을·왜 하려는지(의도)를 커밋 메시지에 남긴다.
- 작업이 끝나면 브랜치를 origin에 push하고, `main`으로 merge하되 `--no-ff`로 병합 커밋을 남긴다(브랜치가 fast-forward로 흡수돼 사라지지 않게).
- 병합 커밋 메시지에는 이 브랜치의 의도(왜 시작했는지)와 결과(무엇을 실제로 이뤘는지, 처음 의도와 달라진 점이 있다면 그것도)를 함께 적는다 — 나중에 `git log --merges`만 봐도 브랜치 단위로 무슨 일이 있었는지 알 수 있어야 한다. 개별 커밋들은 이 merge 커밋을 통해 main 히스토리에 그대로 남는다.
- merge 후 원격 브랜치는 삭제한다(로컬 브랜치도 정리) — 기록은 브랜치 ref가 아니라 `--no-ff` 병합 커밋과 그 아래 개별 커밋들로 남기 때문에, 브랜치를 지워도 히스토리는 남는다.

## 적용 범위
- 이 규칙은 dg 리포지토리에 한정된다.
- 이 규칙이 생기기 전(`main`에 직접 커밋된) `sequence-self-message-clearance`·`gitgraph-vertical-mode` 두 기능은
  히스토리를 다시 쓰지 않고, 완료 시점 커밋을 가리키는 라벨 브랜치(`feat/sequence-self-message-clearance`,
  `feat/gitgraph-vertical-mode`)만 만들어 origin에 남겼다. 이 두 라벨 브랜치는 병합 커밋이 없어 "기록"을
  브랜치 ref 자체가 대신하므로, 위 "merge 후 삭제" 규칙과 달리 지우지 않는다.
