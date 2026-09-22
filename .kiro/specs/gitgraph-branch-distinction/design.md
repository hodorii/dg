# Design — gitgraph-branch-distinction

## 정의
gitGraph를 읽는 사용자를 위해, 트랙(브랜치)마다 색상+선패턴을 독립적으로 순환 배정해 라벨 텍스트
없이도 브랜치를 구분 가능하게 하는 `diagram::layout::gitgraph` 내부 렌더링 로직이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **트랙별 스타일 계산**: 트랙 순번 → (색, 선패턴) 매핑 함수
- **가로 모드 적용**: `build()`의 트랙 선·커밋/병합 점·브랜치 이름 라벨·분기/병합 연결선에 배정 반영
- **세로 모드 적용**: `build_vertical()`에 동일 규칙 반영
- **`--style none` 식별성**: 색상 비활성 상태에서도 선패턴 글자가 트랙마다 달라지는 것

### Out-of-Scope
- **커밋 ID 텍스트 색상**: 기존 `theme.diagram_text` 유지(요구사항에 없음, 변경 시 별도 요구사항 필요)
- **사용자 지정 팔레트/패턴 옵션**: CLI·소스 지시자로 색·패턴을 직접 고르는 기능 없음(자동 순환만)
- **`xychart.rs`/`block.rs`의 기존 팔레트·패턴 코드 리팩터링**: 같은 기법(테마 역할 순환, `LineKind` 순환)을
  참고해 `gitgraph.rs` 안에서 재구현하되, 공유 모듈로 추출하지 않는다 — 이 스펙은 `gitgraph.rs`만 소유
- **가로/세로 배치 로직 자체**: 트랙 배치·열/행 좌표 계산은 `gitgraph-vertical-mode` 소관, 변경 없음

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음)
- 내부 의존 방향: `style::Theme` → `diagram::canvas::{Canvas, LineKind}` → `diagram::layout::gitgraph`
  (기존과 동일, 역방향 import 없음)

### Revalidation Triggers
- `Theme`에서 `diagram_accent`/`diagram_box`/`diagram_group`/`diagram_note` 필드가 제거되거나
  의미(용도)가 바뀌면 팔레트 재정의 필요
- `canvas::LineKind`에 값이 추가/제거되면 패턴 순환 배열 재검토 필요
- 실무 gitGraph 트랙 수가 색·패턴 조합 수(12) 이상으로 흔해져 순환 재사용이 가독성 문제를 일으키면
  팔레트 확장 검토

## Architecture

### Technology Stack
| Layer | Choice | Role |
|-------|--------|------|
| 렌더링 | 기존 `style::Theme` / `diagram::canvas::{Canvas, LineKind}` 재사용 | 트랙별 색·선패턴 계산 및 적용 |

### Key Decisions
- **팔레트는 `xychart.rs`가 쓰는 4개 테마 색상 역할(`diagram_accent`/`diagram_box`/`diagram_group`/
  `diagram_note`)을 그대로 재사용** — 이유: 새 색 상수를 만들지 않고 dark/light/none 테마 전환에
  자동으로 맞춘다. 새 팔레트를 만드는 대안은 `research.md` 참조.
- **선패턴 순환은 `block.rs`의 `border_kind(depth)`와 동일한 `Solid → Dashed → Heavy` (`% 3`) 규칙을
  `gitgraph.rs` 안에 동일하게 재구현** — 이유: `canvas.rs`가 이미 지원하는 3종만으로 `--style none`
  식별성을 확보하고, `canvas.rs`에 새 로직을 추가하지 않는다.
- **색 팔레트 길이(4)와 패턴 길이(3)를 서로 다르게 유지하고 각각 `track % len`으로 독립 순환** —
  이유: 두 축이 맞물려 실질 반복 없는 조합 수가 12개로 늘고, 트랙 0(단일 브랜치)은 항상
  `(diagram_accent, Solid)`로 고정되어 2.2/5.1 회귀가 규칙에서 자연히 보장된다.
- **병합 커밋 점은 별도 분기 없이 `dot.row` 기준으로 스타일 배정** — 이유: `plan()`이 이미 병합
  커밋의 `Dot.row`를 target 트랙으로 채워 넣으므로(`plan.dots.push(Dot { row: *target, .. })`),
  커밋 점과 병합 점을 구분하는 조건문이 필요 없다(4.2 자동 충족).

## Components and Interfaces

### diagram::layout::gitgraph — branch_style
- Intent: 트랙(브랜치) 순번을 (색, 선패턴) 쌍으로 매핑하는 순수 함수
- Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.1, 3.2
```rust
/// 트랙 순번에 대응하는 (색, 선패턴)을 돌려준다. 팔레트(4)·패턴(3) 길이를 각각 독립적으로
/// 순환하므로 어떤 `track` 값에도 항상 값을 돌려주고 패닉하지 않는다. `track == 0`은 항상
/// `(theme.diagram_accent, LineKind::Solid)`— 단일 브랜치 회귀(2.2, 5.1)가 이 규칙에서 보장된다.
fn branch_style(theme: &Theme, track: usize) -> (Style, LineKind);
```
- 계약: `track`은 0-based 트랙 인덱스로, 가로 모드에서는 `Dot.row`/`plan.spans`의 행 인덱스, 세로
  모드에서는 같은 값이 열(트랙) 인덱스로 쓰인다. 입력 범위 제한 없음(배열 인덱스는 항상 `% len`).

### diagram::layout::gitgraph — build / build_vertical (기존 함수 수정)
- Intent: 트랙 선·커밋/병합 점·브랜치 이름 라벨·분기/병합 연결선 각각에 `branch_style`을 적용
- Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.1, 3.2, 4.1, 4.2, 5.1, 5.2
- 계약(적용 지점 4곳, 가로·세로 동일 규칙):
  - **트랙 선**(`plan.spans` 순회 hline/vline): `branch_style(theme, row)` — 해당 트랙 자신의 스타일
  - **브랜치 이름 라벨**(`names` 텍스트): `branch_style(theme, row).0`(색만) — 기존 `theme.diagram_label`
    대신 트랙 색 사용
  - **커밋/병합 점**(`plan.dots` 순회): `branch_style(theme, dot.row).0`(색만) — 기존 `theme.diagram_accent`
    대신 트랙 색 사용. 커밋 ID 텍스트는 기존 `theme.diagram_text` 그대로(Out-of-Scope)
  - **분기 연결선**(`plan.forks`의 `(parent, child, column)`): `branch_style(theme, child)` — 새로
    생기는 트랙 기준
  - **병합 연결선**(`plan.merges`의 `(source, target, column)`): `branch_style(theme, source)` —
    합류해 들어오는 트랙 기준

## Data Models
신규 타입 없음. 기존 `Dot`/`Plan` 구조체(`row`/`column`/`forks`/`merges`) 그대로 사용 — 위 인터페이스
명세로 충분하다.

## Error Handling
- **사용자 입력 오류**: 해당 없음(파싱은 `mermaid/gitgraph.rs` 소관, 이번 스펙에서 변경 없음)
- **외부 자원 오류**: 해당 없음(파일·네트워크 접근 없음)
- **시스템 오류(패닉)**: `branch_style`의 배열 인덱싱은 항상 `track % len`을 거치므로 out-of-bounds
  패닉 불가(5.2, 3.2, 1.2)
- **기능 강등**: 없음 — 스타일 배정은 실패 경로가 없는 순수 계산이며, 폭 초과로 인한 렌더링 포기는
  기존 `render()`의 재시도·`None` 반환 계약을 그대로 따른다(변경 없음)

## Testing Strategy
- **Depth**: Standard — 신규 순수 함수 하나 + 기존 렌더 함수 두 곳의 스타일 적용부 수정. 상태기계·
  외부 통합 없음
- **Unit(L6)**: `branch_style(theme, track)`을 `track = 0, 1, 2, 3, 4, 6, 7`(팔레트·패턴 경계 포함)로
  호출해 색·패턴 순환이 각각 `% 4`/`% 3` 규칙대로 나오는지, `track == 0`이 항상
  `(diagram_accent, Solid)`인지 검증 (1.2, 2.1, 2.2, 3.2)
- **Integration(L4~L5)**: `build`/`build_vertical`을 트랙 2개 이상 입력으로 호출해 `Theme::dark()`
  등 색이 있는 테마에서 트랙별 ANSI 색 코드가 다른지, `Theme::none()`에서 선패턴 글자
  (`─`/`╌`/`━`, 세로는 `│`/`╎`/`┃`)가 트랙마다 달라지는지 확인 (1.1, 1.3, 2.1, 3.1)
- **E2E(L2)**: 분기·병합이 섞인 gitGraph를 가로·세로 각각으로 렌더링해 분기선이 자식 트랙, 병합선이
  source 트랙, 병합 점이 target 트랙의 스타일을 따르는지 텍스트로 확인 (4.1, 4.2)
- **Acceptance(L1)**: `cargo test`(기존 gitgraph.rs 테스트 전량 통과, 특히 단일 트랙 골든 텍스트
  `"main  ●───●───●"` 불변 — 5.1) + `examples/architecture.md`의 gitGraph 예제를 `dg -P -s none`으로
  실행해 패닉 없이 렌더링되는지 실물 확인 (5.2)

## File Structure Plan
```
src/diagram/layout/gitgraph.rs   # branch_style() 추가, build()/build_vertical() 스타일 적용부 수정,
                                  # 신규 유닛/통합 테스트 추가 (신규 파일 없음)
```
