# Design Review

> 핵심 지표: 설계가 구현으로 넘어가기 안전한가. GO/NO-GO 판정.

## Review Philosophy
- 품질 보증 (완벽 추구 아님). 핵심 이슈 ≤3. 대화형. 장·단점 균형. 명확한 GO/NO-GO.

## Scope & Non-Goals
- Scope: 설계 문서 품질 → GO/NO-GO.
- Non-Goals: 구현급 설계, 기술 조사, 기술 선택 확정 (설계 단계 반복에 위임).

## Core Review Criteria
1. **기존 아키텍처 정합** — 경계·레이어·의존 방향·결합·모듈 조직
2. **설계 일관성·표준** — 명명·에러 처리·로깅·설정·데이터 모델링
3. **확장·유지보수성** — 관심사 분리·SRP·테스트 가능성·복잡도 적정
4. **타입·인터페이스** — 타입 정의·`any` 회피·API 경계·입력 검증

## Review Process
1. **Analyze** — 4 기준 대조, 핵심 이슈에 집중
2. **Critical Issues (≤3)** — 각: Issue/Impact/Suggestion/Traceability(요구사항 ID)/Evidence(설계 섹션)
3. **Strengths** — 장점 1-2개
4. **Decide** — GO(치명적 불일치 없음·요구사항 충족·구현 경로 명확) / NO-GO(근본 충돌·치명적 갭·과도 복잡)

## Output Format
```
### Design Review Summary — 2-3문장 (품질·준비도)
### Critical Issues (≤3) — Issue/Impact/Recommendation/Traceability/Evidence
### Design Strengths — 1-2개
### Final Assessment — GO/NO-GO + 근거(1-2문장) + 다음 단계
```

## Length
- 요약 2-3문장. 이슈 각 5-7줄. 전체 ~400단어.

## Checklist
- 핵심 이슈 ≤3, 각에 Impact+Recommendation
- Traceability: 각 이슈에 요구사항 ID
- Evidence: 각 이슈에 설계 문서 위치
- Decision: GO/NO-GO + 근거 + 다음 단계