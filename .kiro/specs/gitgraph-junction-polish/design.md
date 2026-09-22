# Design — gitgraph-junction-polish

## 정의
gitGraph를 읽는 사용자를 위해, 분기·병합 연결선이 트랙 선과 만나는 모서리를 기존 `canvas`의
`round` 메커니즘으로 둥글게 그리고, 세로 모드에서 분기 연결선이 부모의 최근 커밋과 같은 행에
그려질 때 그 커밋 id 글자 뒤에 최소 한 칸 공백을 두는 `diagram::layout::gitgraph`의 렌더링
로직이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **모서리 둥글기**: `build()`(가로)·`build_vertical()`(세로)의 분기·병합 연결선 draw 호출
  직후 해당 연결선의 "자식"(분기) 또는 "source"(병합) 쪽 끝점 셀에 `canvas.join(..., round:
  true)`를 추가
- **세로 모드 간격**: `build_vertical()`의 분기 연결선(`hline`)이 부모의 최근 커밋 행과 같은
  행에서 시작될 때, 그 커밋의 id 글자 폭만큼(+공백 1칸) 시작 x 좌표를 민다

### Out-of-Scope
- **T자·교차 지점**: `canvas.rs`의 `line_char`가 3비트 이상 조합에는 애초에 `round`를 반영하지
  않음 — 이 스펙에서 그 로직을 바꾸지 않는다(요구사항 1.5)
- **병합 target 쪽**: 병합 커밋 점(`◉`)이 항상 그 자리를 덮어써 모서리가 안 보임 — 손댈 필요 없음
- **가로 모드 간격**: 재현되지 않음(분기 연결선이 세로선이라 글자와 겹치는 열이 다름) — 코드
  변경 없음
- **다른 다이어그램형의 모서리**: `graph.rs`/`block.rs`/`sequence.rs` 등은 건드리지 않음
- **둥근 모서리 on/off 옵션**: 새 CLI 플래그·소스 지시자 없음 — 항상 적용

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음)
- 내부 의존 방향: `diagram::canvas`(기존 `join`/`round` 메커니즘, 신규 API 없음) →
  `diagram::layout::gitgraph`(기존과 동일)

### Revalidation Triggers
- `canvas::line_char`가 3비트 이상 조합에도 `round`를 반영하도록 바뀌면 T자·교차 지점 관련
  요구사항(1.5) 재검토 필요
- `plan()`의 `column`(스텝) 값이 트랙 간에 재사용되도록 바뀌면(현재는 전역 유일) 간격 계산의
  "커밋 조회" 로직(아래 `fork_connector_start`) 재검토 필요

## Architecture

### Key Decisions
- **모서리는 각 연결선의 "끝점" 좌표에 `join(round: true)`를 추가하는 것으로 충분하다** — 이유:
  `canvas.rs`의 `hline`/`vline`은 이미 방향 비트를 셀에 누적하고, `rect()`가 이미 같은 방식으로
  (선을 다 그린 뒤 모서리 4곳만 `round=true`로 재계산) 둥근 모서리를 구현한 선례가 있다
  (`canvas.rs:201-207`). 분기의 "자식" 끝점, 병합의 "source" 끝점이 정확히 그 트랙 자신의
  span 선과 만나 2비트 코너를 이루는 지점이므로, 기존 span 그리기(먼저 실행됨) 뒤에 연결선을
  그리고 그 끝점에 `join(x, y, 0, kind, style, true)`를 호출하면 이미 쌓인 비트에 `round`만
  더해 다시 계산된다 — 새 좌표 계산이나 특수 케이스 분기가 필요 없다.
- **모서리 방향(`╰`/`╮`/`╯`/`╭`)을 코드에서 고정하지 않는다** — 이유: 어느 글자가 나오는지는
  트랙 인덱스 순서(부모/자식, source/target 중 어느 쪽이 화면상 위/아래인지)에 따라 달라진다
  (예: 가로 모드에서 일반적인 "기능 브랜치 → main" 병합은 `╯`류지만, 반대로 "main →
  나중에 만든 브랜치"로 병합하면 `╭`류가 나올 수 있다). `join`이 이미 쌓인 비트를 그대로
  둥글게 바꾸므로 어떤 조합이 오든 올바르게 처리된다 — 글자를 미리 판단해 분기하지 않는다.
- **세로 모드 간격은 "그 행의 커밋 id 폭"만큼만 민다(고정 칸 수 아님)** — 이유: 사용자 제안
  ("한 글자 비껴가면 될듯")은 최소 사례 기준이지만, id 길이는 커밋마다 다르므로 고정 오프셋은
  긴 id에서 여전히 붙어 보이는 문제를 완전히 없애지 못한다. `col`(트랙 폭)은 이미 전역 최대
  id 폭 기준으로 넉넉히 잡혀 있으므로(`SLOT_PADDING`), 실제 텍스트 폭만큼만 민 뒤 한 칸을
  더하면 항상 최소 한 칸 공백이 보장되고, 남는 여백은 `col`의 기존 패딩이 흡수한다. 빈
  id(`commit`만 있는 경우)는 밀지 않는다 — 점 글자 바로 뒤에 대시가 오는 건 이미 자연스럽고,
  괜히 밀면 불필요한 변경이 된다.

## Components and Interfaces

### diagram::layout::gitgraph — fork_connector_start (신규 헬퍼)
- Intent: 세로 모드 분기 연결선이 부모 커밋의 id 글자와 겹치지 않도록 시작 x 좌표를 구한다
- Requirements: 2.1
```rust
/// 부모 트랙의 x 좌표(`parent_x`)와 그 행에 있는 부모 커밋 id의 렌더 폭(`parent_id_width`)으로
/// 분기 연결선을 시작할 x 좌표를 구한다. id가 비어 있으면(`0`) 민지 않는다(기존과 동일 — 점
/// 글자 바로 뒤에 이어지는 대시는 자연스럽다). 비어 있지 않으면 텍스트 폭 + 공백 1칸만큼 민다.
fn fork_connector_start(parent_x: usize, parent_id_width: usize) -> usize {
    if parent_id_width == 0 { parent_x } else { parent_x + 1 + parent_id_width + 1 }
}
```
- 계약: 순수 함수, 패닉 없음(정수 덧셈만, 오버플로 우려 없는 범위 — 다이어그램 폭은 이미 다른
  경로에서 `width` 상한으로 제한됨).

### diagram::layout::gitgraph — build / build_vertical (기존 함수 수정)
- Intent: 분기·병합 연결선을 그린 뒤 모서리를 둥글게 하고, 세로 모드 분기는 시작 좌표를
  `fork_connector_start`로 구한다
- Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 2.1, 2.2
- 계약(4곳, 각각 기존 draw 호출 바로 뒤에 한 줄 추가):
  - **가로 분기**(`build`): `vline(x(column), parent, child, ...)` 뒤에
    `join(x(column), child, 0, pattern, color, true)` (1.1)
  - **가로 병합**(`build`): `vline(x(column), source, target, ...)` 뒤에
    `join(x(column), source, 0, pattern, color, true)` (1.2)
  - **세로 분기**(`build_vertical`): 시작 좌표를 `fork_connector_start(x(parent),
    id_width_of_parent_dot_at_this_step)`로 구해 `hline(start, x(child), top+step, ...)`를
    그린 뒤 `join(x(child), top+step, 0, pattern, color, true)` (1.3, 2.1) — 부모 커밋의 id
    폭은 `plan.dots`에서 `column == *step`인 항목을 찾아 대응하는(같은 순서의) `ids[]` 폭으로
    구한다(2.1 대상이 아닌 병합·가로 모드는 이 조회를 하지 않는다, 2.2)
  - **세로 병합**(`build_vertical`): `hline(x(source), x(target), top+step, ...)` 뒤에
    `join(x(source), top+step, 0, pattern, color, true)` (1.4)
  - T자·교차(3비트 이상) 지점은 `join`을 호출하지 않음(해당 좌표가 아님) — `line_char`가
    `round`를 애초에 안 보므로 자동으로 기존과 동일 (1.5)
  - 색상 비활성(`Theme::none()`)에서도 `round`는 `Style`과 무관한 셀 플래그라 그대로 적용됨
    (1.6)

## Data Models
신규 타입 없음. `fork_connector_start`는 순수 함수이고, `Plan`/`Dot` 구조체는 기존 그대로
사용한다(간격 계산에 필요한 `column`/`id` 필드가 이미 있음) — 위 인터페이스 명세로 충분하다.

## Error Handling
- **사용자 입력 오류**: 해당 없음(파싱 변경 없음)
- **외부 자원 오류**: 해당 없음
- **시스템 오류(패닉)**: `fork_connector_start`는 순수 정수 연산만(오버플로 없음), `plan.dots`
  조회는 `Option`으로 처리해 못 찾으면 `0`(민지 않음)으로 안전하게 대체 — 새 패닉 경로 없음
  (3.2)
- **기능 강등**: 없음(폭 초과 시 기존 재시도·`None` 반환 계약 그대로)

## Testing Strategy
- **Depth**: Standard — 신규 순수 함수 하나 + 기존 렌더 함수 두 곳에 draw 호출 추가(4곳), 상태
  기계·외부 통합 없음
- **Unit(L6)**: `fork_connector_start(parent_x, 0)`이 `parent_x`를 그대로 돌려주는지, 0이
  아닐 때 `parent_x + width + 2`를 돌려주는지 확인 (2.1)
- **Integration(L4~L5)**: 가로·세로 각 모드에서 분기 1개·병합 1개짜리 픽스처를 `Theme::none()`
  으로 렌더링해 해당 모서리 글자가 `╰`/`╮`/`╯`/`╭` 중 하나인지(1.1~1.4), 3갈래 분기 픽스처의
  T자 지점은 기존과 같은 문자(`┳`/`┣` 등, 둥글지 않음)인지(1.5) 확인. 세로 모드에서 부모
  커밋에 긴 id를 준 픽스처로 그 커밋의 id 글자와 분기선 사이에 공백 문자가 있는지, 빈 id
  픽스처는 기존처럼 공백 없이 이어지는지(2.1) 확인
- **E2E**: 없음(단일 렌더 함수 호출로 충분)
- **Acceptance(L1)**: `cargo test` 전체 통과(회귀, 3.1) + `examples/gitgraph-vertical.md`·
  `examples/architecture.md`를 실제 릴리스 바이너리로 렌더링해 패닉 없이 끝나고 육안으로 둥근
  모서리·간격을 확인 (3.2, 3.3)

## File Structure Plan
```
src/diagram/layout/gitgraph.rs   # fork_connector_start() 추가, build()/build_vertical()의
                                  # 분기·병합 draw 호출부 수정, 테스트 추가
```
