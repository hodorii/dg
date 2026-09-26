# verification-mapping

V-모델 우측: biz-process 계층(L1~L6)별 테스트 레벨과 통과 기준. `kiro-spec-design`이 Testing Strategy 작성 시 적용한다.

## 1. 계층 ↔ 테스트 레벨 ↔ 통과 기준

| L-level | biz-process 계층 | 테스트 레벨 | 통과 기준 |
|---|---|---|---|
| L1 | Process | Acceptance | requirements.md 수용 기준 전량 충족 + 실행 산출물 실물 실행 확인 |
| L2 | Activity | E2E | 사용자 시나리오 단위 흐름 성공 |
| L3 | FunctionGroup/UI | Integration (UI-API) | 화면-API 연동 성공, 실패 경로 포함 |
| L4 | Step | Integration (API) | 단일 API/서비스 경계 계약 충족 |
| L5 | DetailStep | Integration (Service) | 내부 서비스 협력 동작 충족 |
| L6 | Logic (AST) | Unit | 함수/메서드 단위 로직 정확성 |

도구·프레임워크는 steering `tech.md`. 비중은 Unit 최다 → Acceptance 최소.

## 2. 검증 깊이(Depth) — 전 계층 강제 금지

| Depth | 판정 | 필수 레벨 |
|---|---|---|
| Trivial | 단순 CRUD·설정 | Unit + Acceptance 스모크 |
| Standard | 신규 로직·다화면 | Unit ~ E2E |
| Complex | 도메인 규칙·통합·상태기계 | 전 레벨(Unit ~ Acceptance) |

Depth는 requirements.md 근거로 판정해 Testing Strategy 첫 줄에 명시한다.

## 3. 회귀 판단
- 하위 계층 변경 시 해당 레벨 + 상위 계층 레벨을 함께 재확인한다(예: L6 Logic 변경 → Unit 재확인 + 이를 호출하는 L5/L4 Integration 재확인).
