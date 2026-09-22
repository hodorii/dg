# Bugfix — diagram-alias-label

## 정의
mermaid `erDiagram`에서 개체(entity) 아이디에 대괄호 별칭(`ID["표시 이름"]`)을 붙였을 때, 그 아이디가 별칭 없이 먼저 등장한 적이 있으면 별칭이 무시되고 아이디가 그대로 표시되는 결함을 고치는 수정이다.

## 재현 절차
1. 환경: `dg` 릴리스 빌드(`cargo build --release`), `./target/release/dg --print --diagram <파일>`로 렌더링 확인.
2. 입력 A (관계가 먼저, 별칭 있는 개체 블록이 나중):
   ```
   erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER["고객"] {
      string name
    }
   ```
3. 관찰 결과 A: 렌더된 개체 상자에 `CUSTOMER`가 표시된다. mermaid 원문 규칙상 `고객`이 표시되어야 한다.
4. 입력 B (별칭 없는 관계가 먼저, 별칭 있는 관계가 나중, 같은 아이디를 두 번 참조):
   ```
   erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER["고객"] ||--o{ INVOICE : has
   ```
5. 관찰 결과 B: 두 관계 모두에서 `CUSTOMER` 상자에 여전히 `CUSTOMER`가 표시된다(고객으로 바뀌지 않는다).
6. 대조: `CUSTOMER["고객"] { ... }` 블록이 관계보다 먼저 오거나, 별칭이 있는 관계가 먼저 오는 경우(별칭 없는 참조가 나중에 나오는 경우)는 이미 정상적으로 `고객`이 표시된다 — 즉 결함은 "같은 아이디의 별칭 없는 첫 등장이 별칭을 가리는" 순서 의존성이다.
7. 다른 다이어그램 타입(mermaid flowchart/class/state/sequence, plantuml class·entity·component·sequence)에서는 동일한 순서로 재현되지 않는다 — 각 파서가 별칭이 적힌 줄을 만날 때마다 라벨을 무조건 다시 써 넣기 때문이다. 오직 mermaid `erDiagram`의 `declare()`(`src/diagram/mermaid/er.rs`)만 최초 생성 시점에만 라벨을 정하고, 이후 별칭이 있는 줄을 만나도 라벨을 갱신하지 않는다.

## Boundary Context
- **In scope**: `src/diagram/mermaid/er.rs`의 개체 아이디 ↔ 표시 라벨 결정 로직(`declare` 함수). 개체가 여러 번(관계 양 끝, 속성 블록 헤더) 참조될 때 대괄호 별칭이 등장 순서와 무관하게 항상 반영되도록 한다.
- **Out of scope**: 다른 mermaid/PlantUML 다이어그램 파서(이미 별칭을 순서 무관하게 반영함 — 재조사로 확인). `render_source_block`(원문 그대로 보여주는 기능, 다이어그램 렌더링과 무관). ER 카디널리티·관계선 표기 로직.

## Behaviors

### 1. 현재 동작 (결함)
- 1.1: `CUSTOMER` 아이디가 별칭 없이 먼저 등장(관계 한쪽 끝)하고, 그 뒤에 `CUSTOMER["고객"] { ... }` 속성 블록이 나오면 → 렌더된 상자 제목이 `CUSTOMER`로 남는다(별칭 `고객`이 반영되지 않음).
- 1.2: `CUSTOMER` 아이디가 별칭 없이 먼저 등장(관계 한쪽 끝)하고, 그 뒤에 같은 아이디를 별칭과 함께 쓴 또 다른 관계(`CUSTOMER["고객"] ||--o{ INVOICE : has`)가 나오면 → 두 관계 모두에서 상자 제목이 `CUSTOMER`로 남는다.

### 2. 기대 동작
- 2.1: 위 1.1과 같은 입력에서 → 렌더된 상자 제목이 `고객`이어야 한다(별칭이 등장 순서와 무관하게 반영).
- 2.2: 위 1.2와 같은 입력에서 → 두 관계 모두에서 상자 제목이 `고객`이어야 한다.

### 3. 불변 동작 (회귀 방지)
- 3.1: 별칭이 있는 개체 선언(`CUSTOMER["고객"]`)이 해당 아이디의 첫 등장인 입력(현재도 정상 동작)은 계속 `고객`을 표시해야 한다.
- 3.2: 별칭이 전혀 없는 입력(`CUSTOMER ||--o{ ORDER : places`만 있는 경우)은 계속 아이디 `CUSTOMER`를 그대로 표시해야 한다.
- 3.3: 속성 블록의 칸(`string name` 등)과 관계선의 카디널리티·라벨(`places` 등) 표시는 이번 수정으로 달라지지 않아야 한다.
