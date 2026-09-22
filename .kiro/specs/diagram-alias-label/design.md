# Design — diagram-alias-label

## 정의
mermaid `erDiagram` 개체 별칭이 등장 순서와 무관하게 항상 반영되도록, `src/diagram/mermaid/er.rs`의 아이디↔라벨 결정 지점을 고치는 최소 수정이다.

## 원인 (Root Cause)
`src/diagram/mermaid/er.rs`의 `declare(graph, raw)`(현재 32~39행)는 아이디와 별칭 텍스트를 나눈 뒤 `graph.intern(id, &text, Shape::Rect, None)` 한 번만 호출한다. `Graph::intern`(`src/diagram/ir.rs` 119~131행)은 해당 아이디의 노드가 **처음 생성될 때만** 전달받은 라벨을 쓰고, 이미 존재하는 노드에는 라벨을 건드리지 않는다. 그래서 같은 아이디가 별칭 없이 먼저 등장(관계 한쪽 끝 또는 별칭 없는 속성 블록)하면 라벨이 아이디 그대로 굳어지고, 그 뒤 별칭 붙은 등장(`CUSTOMER["고객"]`)이 와도 `declare()`가 라벨을 다시 써 넣는 절차가 없어 무시된다 (1.1, 1.2).

다른 파서(`flow.rs`, `class.rs`, `state.rs`, 양쪽 `sequence.rs`, plantuml `class.rs`/`component.rs`)는 모두 별칭이 적힌 줄을 만날 때마다 `graph.set_label`/`sequence.participants[i].label` 등을 **무조건** 다시 쓴다 — `intern`의 최초 생성 시 라벨에만 의존하지 않는다. `er.rs`만 이 패턴을 빠뜨렸다.

## 수정 방식
`declare()`에서 대괄호 별칭이 있는 호출일 때는 `graph.intern`이 새로 만들었는지 여부와 무관하게 `graph.set_label(index, &text)`을 명시적으로 호출한다(별칭이 없는 호출은 지금처럼 아이디를 라벨로 써서 최초 생성 시에만 반영되게 둔다 — 이미 있는 노드의 라벨을 별칭 없는 재등장이 지우면 안 되므로).

```rust
fn declare(graph: &mut Graph, raw: &str) -> usize {
    let (id, alias) = match raw.find('[') {
        Some(p) => (raw[..p].trim(), Some(label(raw[p + 1..].trim_end_matches(']')))),
        None => (raw.trim(), None),
    };
    let index = graph.intern(id, alias.as_deref().unwrap_or(id), Shape::Rect, None);
    if let Some(text) = &alias {
        graph.set_label(index, text);
    }
    index
}
```

기각한 대안: `Graph::intern` 자체를 "라벨이 비어있지 않으면 항상 덮어쓴다"로 바꾸는 방안 — `flow.rs`/`class.rs`/`state.rs`가 별칭 없는 재등장에서 빈 문자열(`""`)을 라벨로 넘기며 "덮어쓰지 않음"에 의존하는 기존 계약을 깨뜨려 다른 다이어그램 타입에 회귀를 낼 위험이 있어 기각. `er.rs`만 고치는 쪽이 영향 범위가 좁다.

## 검증 속성
- (a) 결함 재현: `parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER[\"고객\"] {\n string name\n }\n")` → 수정 전 `nodes[..].sections[0] == ["CUSTOMER"]`로 실패(1.1).
- (b) 기대 동작: 같은 입력에서 수정 후 `sections[0] == ["고객"]`(2.1); 관계 두 개짜리 케이스(1.2/2.2)도 같은 방식으로 `고객` 확인.
- (c) 불변 동작: 별칭이 첫 등장인 기존 케이스(`parses_entities_and_relations`의 `CUSTOMER` — 별칭 없음, 3.2)와 속성 칸·카디널리티 표시(3.3)가 수정 후에도 그대로인지 기존 테스트 + 신규 순서역전 테스트로 확인.

## 영향 범위
- 파일: `src/diagram/mermaid/er.rs`(`declare` 함수)만 수정.
- 이 저장소는 기능별 소유 스펙이 없는 파서 모듈이라 침해할 Boundary Commitment 없음. `ir.rs`의 `Graph::intern`/`set_label` 공개 계약은 그대로 유지(새 호출 패턴만 추가).
