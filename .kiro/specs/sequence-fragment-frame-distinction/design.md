# Design — sequence-fragment-frame-distinction

## 정의
시퀀스 다이어그램을 읽는 사용자를 위해, 프래그먼트(`alt`/`opt`/`loop` 등) 바깥 테두리를
메시지·생명선과 다른 `LineKind`(굵은선)로 그리는 `diagram::layout::sequence`의 렌더링 로직이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **프래그먼트 바깥 테두리 선패턴**: `alt`/`opt`/`loop`/`par`/`critical`/`break`/`group`/`rect`
  프래그먼트를 그리는 `canvas.rect(..)` 호출 하나의 `LineKind`
- **골든 출력 갱신**: `examples/architecture.txt` 재생성(원문 `examples/architecture.md`은
  변경 없음)

### Out-of-Scope
- **`else`/`option`/`and` 구분선**: 기존 `LineKind::Dashed` 그대로(요구사항 1.3), 코드 변경 없음
- **프래그먼트 제목 텍스트 스타일**: 이미 `theme.diagram_accent.bold()`로 구분됨, 변경 없음
- **중첩 프래그먼트의 깊이별 패턴 순환**: `block-beta`의 `border_kind(depth)`와 달리, 여기선
  모든 프래그먼트가 동일한 굵은선(중첩은 이미 들여쓰기로 보임) — 새 순환 로직 없음
- **`graph.rs` 공유 레이아웃의 그룹 테두리**: 별도 검토 대상(brief.md), 이 스펙이 손대지 않음

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음)
- 내부 의존 방향: `diagram::canvas::LineKind`(기존 값 재사용, 신규 변형 없음) →
  `diagram::layout::sequence`(기존과 동일, 역방향 없음)

### Revalidation Triggers
- `canvas::LineKind`에 값이 추가/제거되면 이 스펙의 "메시지·생명선과 다른 값" 전제 재검토 필요
- 시퀀스 레이아웃에 프래그먼트 외 다른 요소가 `LineKind::Heavy`를 새로 쓰게 되면(현재는
  활성화 막대만 씀) 프래그먼트 테두리와의 구분 여부 재검토 필요

## Architecture

### Key Decisions
- **프래그먼트 바깥 테두리를 `LineKind::Solid` → `LineKind::Heavy`로 바꾼다** — 이유: 메시지·
  생명선이 이미 `Solid`를 쓰고, 노트가 이미 `Dashed`를 쓰므로(`layout/sequence.rs:470`) 셋 중
  남는 값은 `Heavy` 하나뿐이다. 활성화 막대도 `Heavy`를 쓰지만(`layout/sequence.rs:439`) 폭
  1칸짜리 세로 막대와 프래그먼트의 넓은 사각 테두리는 모양 자체가 달라 서로 혼동되지 않는다.
- **`else`/`option`/`and` 구분선은 건드리지 않는다** — 이유: 이미 `Dashed`라 프레임(`Heavy`)·
  흐름(`Solid`)과 구분되는 세 번째 역할로 기능한다. 굳이 프레임과 통일할 필요가 없다
  (요구사항 1.3).
- **교차 지점 처리 로직을 새로 만들지 않는다** — 이유: `canvas.rs`의 `line_char`는 방향
  비트+`dashed`/`heavy` 플래그의 조합만으로 글자를 고르는 범용 테이블이라, 어떤 호출이 그
  비트를 세웠는지 모른다. 활성화 막대(`Heavy`)가 생명선(`Solid`)을 가로지르는 기존 코드가 이미
  같은 조합을 그려 왔으므로, 프래그먼트 테두리(`Heavy`)가 메시지(`Solid`)와 만나는 것도 기존
  인프라가 그대로 처리한다.

## Components and Interfaces

### diagram::layout::sequence — SequenceLayout::draw (기존 함수 내부 수정)
- Intent: 프래그먼트 바깥 테두리를 흐름과 다른 선패턴으로 그린다
- Requirements: 1.1, 1.2, 1.3, 2.1, 2.2
```rust
// 변경 전: canvas.rect(x_lo, fragment.start_row, x_hi - x_lo + 1, height, LineKind::Solid, theme.diagram_group, false);
// 변경 후:
canvas.rect(x_lo, fragment.start_row, x_hi - x_lo + 1, height, LineKind::Heavy, theme.diagram_group, false);
```
- 계약: 색상(`theme.diagram_group`)과 둥근 모서리 여부(`false`)는 그대로 — `LineKind`만
  바뀐다. `alt`/`opt`/`loop`/`par`/`critical`/`break`/`group`/`rect` 전부 이 호출 하나를
  공유하므로(파서가 전부 같은 `SequenceItem::FragmentStart`로 귀결) 별도 분기 없이 8종 모두에
  일괄 적용된다(1.1). `else_rows`를 그리는 다음 줄(`canvas.hline(.., LineKind::Dashed, ..)`)은
  변경하지 않는다(1.3).

## Data Models
신규 타입 없음. `Fragment` 구조체 그대로 사용 — 위 인터페이스 명세로 충분하다.

## Error Handling
- **사용자 입력 오류**: 해당 없음(파싱은 `mermaid/plantuml::sequence` 소관, 변경 없음)
- **외부 자원 오류**: 해당 없음
- **시스템 오류(패닉)**: `canvas.rect`/`line_char`는 기존에 이미 모든 방향 비트 조합을 처리하는
  전수 매치라 새 패닉 경로가 생기지 않음(2.1, 3.2)
- **기능 강등**: 없음 — 폭 초과 시 기존 `render()`의 캡 축소·`None` 반환 계약 그대로(변경 없음)

## Testing Strategy
- **Depth**: Trivial — 기존 함수 호출 한 곳의 인자(`LineKind`) 하나만 바뀌는 설정성 변경, 새
  상태·분기·통합 지점 없음
- **Unit(L6)**: `render()`를 `alt`/`loop` 중첩 픽스처로 호출해 `Theme::none()`에서 테두리 글자가
  `┏`/`┓`/`┗`/`┛`/`━`/`┃` 중 하나이고 메시지·생명선 글자(`─`/`│`)와 같은 셀에 나오지 않는지,
  `else` 구분선은 여전히 `╌`인지 확인 (1.1, 1.2, 1.3, 2.2)
- **Integration**: 메시지 화살표가 프래그먼트 경계를 지나는 픽스처(기존
  `renders_messages_and_fragment` 확장)로 교차 지점이 패닉 없이 그려지는지 확인 (2.1)
- **E2E**: 없음(단일 렌더 함수 호출로 충분, 별도 흐름 없음)
- **Acceptance**: `cargo test` 전체 통과(회귀, 3.1) + `examples/architecture.md`를
  `dg -P -s none`으로 재실행해 `examples/architecture.txt`를 갱신하고 패닉 없이 끝나는지 확인
  (3.2, 3.3)

## File Structure Plan
```
src/diagram/layout/sequence.rs   # 프래그먼트 rect() 호출의 LineKind 변경, 테스트 추가/보강
examples/architecture.txt        # 재생성(생성 스크립트 없음 — dg -P -s none으로 직접 갱신)
```
