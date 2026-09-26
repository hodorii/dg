# biz-process-rules

> `kiro-biz-process` 스킬이 준수해야 할 작성/포맷 규칙. 원칙: ko / SSoT / SRP / Descriptable Name.

## 1. 사용자 시각 작성 규칙 (User-Perspective)
- 주어는 반드시 **사용자/역할** (예: "고객", "관리자", "신규 가입자"). 시스템은 주어로 쓰지 않음.
- 동사는 사용자 행위 중심: "~을 입력한다", "~을 확인한다", "~을 선택한다".
- 시스템 응답은 "시스템이 ~한다" 가 아니라 "사용자는 ~ 결과를 본다/받는다" 로 서술.
- 화면/UX 용어는 사용자가 인지하는 단위(화면, 버튼, 알림)로 표현.

## 2. BPMN 흐름 원칙
- **순차(Sequence)**: 단계적 진행
- **분기(Gateway)**: 조건에 따른 갈림 (IF/ELSE, ERROR)
- **병렬(Parallel)**: 독립 단위 동시 진행 표시
- **이벤트(Event)**: 타임아웃, 외부 콜백, 예외

## 3. 가치사슬 링크 규칙 (SSoT)
- L1 Process.id 는 `BP-<의미있는명칭>` 형식 (Descriptable).
- `valueChainRef` 는 `value-chain.md` 의 Unit.id 와 정확히 일치 (대소문자 포함).
- Unit 의 `value`/`validation` 은 복제하지 않고 **참조 링크**만 둔다.
- 양방향 점검(권장): biz-process 에서는 Unit.id 를 `valueChainRef` 로, value-chain 에서는 `bizProcessRef` 로 맞춤.
- `value-chain.md` 는 제품/비즈니스 owner 가 소유하는 SSoT 이므로 본 스킬은 **자동 수정하지 않는다**. `bizProcessRef` 가 없으면 링크는 일방향(`valueChainRef` 단독)으로 유효하며, owner 에게 추가를 요청한다.

## 4. V-모델 좌측 전개 (Progressive Unfold)

- BizProcess 드릴다운 = V-모델 **좌측(전개)**. 검증(우측)은 biz-process.md에 기록 않음.
- 검증 레벨 매핑은 `kiro-spec-design/rules/verification-mapping.md`(L1~L6 ↔ 테스트 레벨 ↔ Depth). `design.md` Testing Strategy가 이를 적용해 기록하고, 소비 스킬(verify-completion, validate-impl, impl)은 그 Testing Strategy를 따른다.
- 레벨: Process → Activity → FunctionGroup/UI → Step → DetailStep → Logic(AST)

## 5. 드릴다운 표현 규칙
- 계층은 YAML 또는 들여쓰기 목록. 최하위 L6 는 의사코드(pseudo-code)로 표현("AST 노드" 는 의사코드로 충분).
- 각 **L1 Process** 는 승인 게이트 주석(`### ✅ 검토 요청`)으로 구분. 비대화형(-y) 실행 시에는 전체 드릴다운 후 단일 게이트로 일괄 승인.
- 드릴다운 L2/L3 블록에는 관련 요구사항 ID를 태그하여 `requirements.md` 와 추적성 유지 (매핑 테이블에도 기록).

## 6. 승인 게이트 (Informed Consent)
- 각 L1 Process 생성 직후 사용자 검토 요청. 단순 확인이 아닌 "이해 기반 동의".
- `-y` 플래그: 전 레벨 자동 승인 (요건/가치사슬은 사전 승인 전제).

## 7. 부트스트랩 가이드 (value-chain.md 생성)
`value-chain.md` 가 없으면 다음 최소 골격을 제품/비즈니스 owner 에게 제시하고 생성을 요청한다 (본 스킬이 자동 생성하지는 않는다).

```markdown
# 가치사슬 (Value Chain) — SSoT

## Mega (가치사슬 정의서)
- id: VC-<domain>
- name: <도메인 가치사슬>
- value: <최상위 고객 가치>

## Main (주요 가치 흐름)
- id: VC-<domain>-<main>
- name: <주요 흐름>
- parent: VC-<domain>

## Unit (단위 프로세스)
- id: VC-<domain>-<unit>
- name: <단위 프로세스명>
- parent: VC-<domain>-<main>
- value: <이 단위가 전달하는 가치>
- validation: <가치 충족 조건>
- bizProcessRef: BP-<의미있는명칭>   # 선택: 해당 BizProcess 와 양방향 연결
```

참고: `bizProcessRef` 는 선택 항목. 미작성 시 BizProcess → Unit 링크(`valueChainRef`)만으로도 유효하다.
