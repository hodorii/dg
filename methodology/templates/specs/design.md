# Design — <feature>

## 정의
[A]를 위해 [B]를 하는 [C]이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **[책임 영역]**: [소유하는 동작·데이터 1줄]

### Out-of-Scope
- **[비소유 영역]**: [누가 소유하는지]

### Allowed Dependencies
- 외부: [라이브러리 + 버전]
- 내부 의존 방향: `a` → `b` → `c` (역방향 import = 설계 위반)

### Revalidation Triggers
- [이 설계의 전제가 깨져 재검토가 필요한 조건 — 계약·소유·의존 방향·런타임 전제·규모]

## Architecture

### Boundary Map
[Mermaid — 모듈/컴포넌트와 의존 방향. 복잡 기능 필수]

### Technology Stack
| Layer | Choice | Role |
|-------|--------|------|
| | | |

### Key Decisions
- **[결정]**: [내용] — 이유: [근거 1줄]. 대안 비교는 research.md.

## System Flows
[비자명 흐름만, Mermaid sequence/state. 없으면 절 생략]
- [흐름별 결정 사항 — 요구사항 ID 태그 (예: 7.2)]

## Components and Interfaces

### [module] — [Component]
- Intent: [책임 1줄]
- Requirements: [2.1~2.5, 3.1]
```[lang]
[공개 시그니처·타입 — 구현 언어 그대로. 오류 타입 포함]
```
- [계약 특이사항: 이벤트·상태·실패 모드]

## Data Models
[도메인 타입·영속 구조·불변식 — 위 인터페이스로 충분하면 그렇게 명시]

## Error Handling
- **사용자 입력 오류**: [처리 + 요구사항 ID]
- **외부 자원 오류** (파일·네트워크·권한): [격리 방식]
- **시스템 오류** (패닉·예외): [복구·종료 경로]
- **기능 강등**: [의존 실패 시 폴백]

## Testing Strategy
- **Depth**: [Trivial | Standard | Complex — 근거 1줄]
- **Unit**: [모듈별 핵심 케이스 + 요구사항 ID]
- **Integration**: [경계 횡단 시나리오]
- **E2E**: [biz-process L2 흐름 대응]
- **Acceptance**: [수용 기준 충족 + 실물 실행 확인]
- **Performance**: [필요 시 수치]

## File Structure Plan
```
[디렉터리 트리 + 책임 주석 — 모든 컴포넌트가 파일 경로를 가져야 함]
```

## Optional (필요 시만)
Security / Performance / Migration
