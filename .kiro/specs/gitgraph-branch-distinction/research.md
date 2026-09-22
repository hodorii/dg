# Research & Design Decisions — gitgraph-branch-distinction

## Summary
- **Feature**: `gitgraph-branch-distinction`
- **Discovery Scope**: Extension (`diagram::layout::gitgraph` 기존 렌더 함수 수정)
- **Key Findings**:
  - `xychart.rs`가 이미 4개 테마 색상 역할을 순환하는 팔레트 패턴을 쓰고 있다(`SeriesStyle`).
  - `block.rs`의 `border_kind(depth)`가 이미 `Solid → Dashed → Heavy`를 `% 3`으로 순환하는 선패턴
    패턴을 쓰고 있다(중첩 깊이 기준).
  - 두 패턴 모두 신규 의존성·신규 `canvas.rs` 변경 없이 `gitgraph.rs`에 그대로 옮겨 쓸 수 있다.

## Research Log

### gitGraph 현재 스타일 적용 지점
- **Context**: 어디를 트랙별로 나눠야 하는지 파악
- **Sources Consulted**: `src/diagram/layout/gitgraph.rs`(`build`/`build_vertical`)
- **Findings**: 트랙 선(`hline`/`vline`)·이름 라벨·분기 연결선·병합 연결선은 전부
  `theme.diagram_line` 단일 스타일. 커밋/병합 점은 `theme.diagram_accent` 단일 스타일. 병합 커밋의
  `Dot.row`는 이미 target 트랙으로 채워져 있어(`plan()`의 `GitEvent::Merge` 처리) 별도 분기 없이도
  점 색 배정이 자동으로 target 트랙을 따른다.
- **Implications**: 수정 지점은 정확히 4곳(트랙 선, 라벨, 점, 두 종류 연결선)이며 새 구조체 필드
  없이 기존 `row`/`column` 인덱스만으로 스타일을 계산할 수 있다.

### 기존 순환 패턴 선례
- **Context**: 팔레트·선패턴을 새로 설계할지, 기존 것을 재사용할지 판단
- **Sources Consulted**: `src/diagram/layout/xychart.rs:183-198`(팔레트), `src/diagram/layout/block.rs:176-181`
  (`border_kind`)
- **Findings**: `xychart.rs`는 `[theme.diagram_accent, theme.diagram_box, theme.diagram_group,
  theme.diagram_note]` 4색을 `ordinal % len`으로 순환한다. `block.rs`는 중첩 깊이를 `depth % 3`으로
  `Solid`/`Dashed`/`Heavy`에 매핑한다. 둘 다 `canvas.rs`의 기존 타입(`Style`, `LineKind`)만 쓴다.
- **Implications**: 새 색상·새 `LineKind` 변형을 만들 필요가 없다. `gitgraph.rs`에 같은 두 규칙을
  트랙 인덱스 기준으로 재구현하면 된다.

## Design Decisions

### Decision: 팔레트를 공유 모듈로 추출하지 않고 `gitgraph.rs`에 재구현
- **Context**: `xychart.rs`와 같은 4색 팔레트, `block.rs`와 같은 3패턴 순환을 쓰게 됐으니 공통 헬퍼로
  뽑을지 결정 필요
- **Alternatives Considered**:
  1. `style.rs`나 `canvas.rs`에 `diagram_palette()`/`cycle_line_kind()` 공유 함수를 만들고 세 다이어그램
     타입(xychart, block, gitgraph)이 함께 쓰도록 리팩터링
  2. `gitgraph.rs` 안에 `branch_style()` 하나를 새로 만들고, 팔레트 배열과 패턴 순환 로직만 같은
     방식으로 재구현(중복 허용)
- **Selected Approach**: 2번. `gitgraph.rs`에 로컬 `branch_style(theme, track)` 함수를 추가한다.
- **Rationale**: 공유 추출은 `xychart.rs`(이미 배포된, 테스트로 고정된 기능)의 파일을 건드리게 되어
  스펙 경계(`gitgraph.rs`만 소유)를 넘는다. 세 곳의 "숫자 하나를 배열 인덱스로 순환"이라는 로직은
  3줄짜리라 추상화 비용이 재사용 이득보다 크다(design-synthesis의 Simplification 기준).
- **Trade-offs**: 팔레트 색상표나 패턴 규칙이 나중에 바뀌면 세 파일을 따로 고쳐야 한다. 그 비용은
  현재 규모(각 파일 3~5줄)에서는 낮다고 판단.
- **Follow-up**: 다이어그램 타입이 하나 더 같은 순환 팔레트를 필요로 하게 되면(4번째 사례) 그때
  공유 헬퍼 추출을 재검토한다.

## Risks & Mitigations
- 트랙 수가 12개(색 4 × 패턴 3의 최소공배수)를 넘으면 같은 (색, 패턴) 조합이 재등장 — 완화: 요구사항
  1.2/3.2가 이미 "순환 재사용, 패닉 없음"을 명시적으로 허용하므로 설계상 결함 아님(가독성 저하는
  실사용 트랙 수가 12를 넘길 때만 의미 있는 후속 개선 대상)
- 색 팔레트(4)와 패턴(3)이 서로 다른 길이라 조합이 어긋나 보일 수 있음 — 완화: `track % 4`와
  `track % 3`을 각각 독립 계산하므로 트랙 0은 항상 동일 조합(`accent`+`Solid`)로 고정되어 회귀가
  자연히 보장됨(design.md Key Decisions)

## References
- `src/diagram/layout/xychart.rs:183-198` — 팔레트 순환 선례
- `src/diagram/layout/block.rs:176-181` — 선패턴 순환 선례(`border_kind`)
- `src/diagram/canvas.rs:244-278` — `LineKind`별 실제 글자(`─`/`╌`/`━`, `│`/`╎`/`┃`)
