# Bugfix — diagram-er-standalone-alias

## 정의
mermaid `erDiagram`에서 관계선·속성 블록 없이 **개체 하나만 단독으로 선언하는 줄**
(`ID["표시 이름"]`)에 붙인 대괄호 별칭이 전혀 반영되지 않는 결함을 고치는 수정이다.

## 재현 절차
1. 환경: `dg` 릴리스 빌드(`cargo build --release`), `./target/release/dg --print --diagram
   --lang mermaid <파일>`로 렌더링 확인.
2. 입력 A (별칭 있는 단독 선언 줄 + 별칭 없는 관계):
   ```
   erDiagram
       CUSTOMER["Customer"]
       CUSTOMER ||--o{ ORDER : places
   ```
3. 관찰 결과 A: 렌더된 상자 제목이 `CUSTOMER`로 남는다. mermaid 원문 규칙상 `Customer`가
   표시돼야 한다.
4. 입력 B (실사용 사례 — 라벨에 공백이 섞인 경우, 실제 프로젝트 참조 문서에서 재현됨):
   ```
   erDiagram
       mbr_ams_score_hst["회원입학선발점수이력 (mbr_ams_score_hst)"]
       mbr_role_rel["회원역활관계 (mbr_role_rel)"]
       mbr_role_rel ||--o{ mbr_ams_score_hst : "inst_cd,mbr_id,role_cd"
   ```
5. 관찰 결과 B: 두 상자 모두 원래 아이디(`mbr_ams_score_hst`, `mbr_role_rel`)가 그대로 표시된다.
6. 대조(이미 정상): 같은 별칭을 관계선에 직접 붙이면(`CUSTOMER["Customer"] ||--o{ ORDER :
   places`) 정상적으로 `Customer`가 표시된다. 속성 블록 헤더(`CUSTOMER["Customer"] { ... }`)도
   정상이다. `diagram-alias-label` 스펙이 고친 "같은 아이디가 별칭 없이 먼저 등장한 뒤 별칭
   있는 참조가 나중에 오는" 순서 의존 결함과는 다른 원인 — 이번 건은 순서와 무관하게, 단독
   선언 줄 자체가 파서에 아예 반영되지 않는다.
7. 다른 다이어그램 타입(mermaid flowchart의 단독 노드 선언 `A["표시 이름"]`은 정상 재현 확인
   — 결함 없음)에서는 재현되지 않는다 — mermaid `erDiagram`의 `src/diagram/mermaid/er.rs`
   `parse()`에 한정된 결함.

## Boundary Context
- **In scope**: `src/diagram/mermaid/er.rs`의 `parse()`가 한 줄이 "관계선"인지 "단독 개체
  선언"인지 판정하는 로직.
- **Out of scope**: `declare()` 자체의 라벨 결정 규칙(이미 `diagram-alias-label`에서 고쳐짐,
  이번엔 그 함수가 아예 호출되지 않는 게 문제). 속성 블록(`{ ... }`) 파싱. 다른 다이어그램
  타입(전부 재조사로 정상 확인).

## Behaviors

### 1. 현재 동작 (결함)
- 1.1: 관계선·속성 블록 없이 `ID["표시 이름"]` 단독 선언 줄만 있으면 → 그 개체가 나중에
  관계선에 별칭 없이 참조될 때 상자 제목이 아이디 그대로 남는다(별칭이 전혀 반영 안 됨).
- 1.2: 라벨에 공백이 포함된 단독 선언 줄(`ID["여러 단어 라벨"]`)도 → 동일하게 별칭이
  반영되지 않는다.

### 2. 기대 동작
- 2.1: 위 1.1과 같은 입력에서 → 상자 제목이 별칭(`Customer`)이어야 한다.
- 2.2: 위 1.2와 같은 입력에서 → 상자 제목이 공백 포함 별칭 전체여야 한다.

### 3. 불변 동작 (회귀 방지)
- 3.1: 관계선에 직접 붙인 별칭(`CUSTOMER["Customer"] ||--o{ ORDER : places`)은 계속
  정상 반영돼야 한다.
- 3.2: 속성 블록 헤더에 붙인 별칭(`CUSTOMER["Customer"] { ... }`)은 계속 정상 반영돼야 한다.
- 3.3: 별칭이 전혀 없는 관계선(`CUSTOMER ||--o{ ORDER : places`)은 계속 아이디를 그대로
  표시해야 한다.
- 3.4: 관계선의 카디널리티·라벨 표시와 속성 블록의 칸 표시는 이번 수정으로 달라지지 않아야
  한다.
