# Design — sequence-nested-activation-offset

## 정의
시퀀스 다이어그램에서 한 참여자에게 겹치는 활성화 구간이 있을 때, 각 구간을 UML
관례대로 옆으로 어긋나게(계단식) 그려 겹침 정도를 구분되게 보여주는
`diagram::layout::sequence::SequenceLayout`의 확장이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- **참여자별 최대 동시 활성화 깊이 계산**: `SequenceLayout`이 `draw()` 이전에 한 번
  계산해 보관(현재 `draw()` 안에서만 지역적으로 굴리는 `active_depth`/`active`를
  구조체 필드로 승격)
- **활성화 막대 x좌표 오프셋**: `draw()`의 활성화 막대 그리기(`sequence.rs:436` 부근)가
  각 구간의 깊이 인덱스만큼 오른쪽으로 어긋난 칸에 그리도록 변경
- **참여자 간격에 오프셋 반영**: `position_participants()`의 기존 제약(constraint)
  목록에 "이 참여자의 최대 동시 깊이만큼 오른쪽 간격을 더 확보" 제약 추가
- **메시지 화살표 끝점을 오프셋 칸에 맞춤**: `draw_message()`가 활성화를 열거나(가장
  최근에 연 구간) 닫을(가장 안쪽 구간) 때, 화살표의 해당 쪽 끝을 그 구간의 오프셋
  칸까지 연장

### Out-of-Scope
- **자기 메시지(`from == to`)의 활성화 처리**: `position_participants`/`draw` 양쪽
  모두 self-message 분기가 `activate_target`/`deactivate_source`를 애초에 읽지
  않는 기존 결함 — 이 스펙은 손대지 않는다(요구사항 Boundary Context)
- **프래그먼트 테두리·생명선 선패턴**: 이미 검토를 끝내고 현재 상태 유지로 결론
- **활성화 막대 자체를 2칸짜리 상자로 바꾸는 것**: dg의 기존 활성화 막대는 1칸짜리
  세로선(`LineKind::Heavy` vline)이다 — 폭을 넓히는 게 아니라 그 1칸짜리 선을
  깊이만큼 옆으로 어긋나게 여러 개 그리는 것만 다룬다(기존 시각 언어 유지)

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음)
- 내부 의존 방향: `position_participants()`(간격 계산) → `draw()`(그리기)는 기존과
  동일한 방향. 새 깊이 계산은 두 함수 모두보다 먼저 실행되는 공유 준비 단계로 추가한다
  (`collect_fragments()`와 같은 위치·순서)

### Revalidation Triggers
- `active`/`active_depth`의 자료구조나 `close_activation`의 LIFO(가장 최근 것부터
  닫힘) 전제가 바뀌면 이 스펙의 "깊이 인덱스 = 그 시점에 이미 열려 있던 구간 수"
  계산이 깨진다
- self-message의 활성화 처리가 다른 스펙에서 고쳐지면, 그 경로도 이 스펙의 오프셋
  로직을 타야 하는지 재검토 필요

## Architecture

### Key Decisions
- **오프셋은 오른쪽(참여자 번호가 커지는 쪽)으로만 준다** — 이유: 기존 간격 제약
  시스템(`constraints` 리스트 + `gaps` 배열)이 이미 "구간 (a, a+1)에 최소 거리 필요"
  형태로만 동작해서, 오른쪽 한 방향으로 통일하면 기존 인프라를 그대로 재사용해
  새 제약 하나만 추가하면 된다. 양쪽으로 분산하면 왼쪽 이웃 간격까지 건드려야 해서
  변경 범위가 커진다.
- **깊이 인덱스는 칸 수로 그대로 오프셋에 쓰되, 3칸에서 멈춘다(요구사항 4.1)** —
  이유: 실제로 4겹 이상 동시에 겹치는 시퀀스는 드물고, 상한 없이 깊이만큼 계속
  오프셋을 주면 재귀·다자 호출이 섞인 병리적 다이어그램에서 참여자 간격이
  무한정 벌어질 수 있다. 상한 이후 구간은 같은(가장 오른쪽) 칸에 겹쳐 그린다 —
  "몇 겹인지 정확히 다 구분"보다 "적어도 겹쳐 있다는 것 자체는 알 수 있고 폭이
  안전하다"를 우선한다.
- **깊이 인덱스는 활성화가 열릴 때 "그 시점에 이미 열려 있던 구간 수"로 정한다** —
  이유: `close_activation`이 이미 LIFO(가장 최근에 연 것부터 닫힘)로 동작하므로,
  push 시점의 열린 구간 수를 그대로 그 구간의 깊이로 고정해 두면 나중에 닫힐
  때도(가장 안쪽부터 닫히므로) 값을 다시 계산할 필요가 없다.
- **화살표는 "활성화를 열거나 닫는 그 끝"만 오프셋 칸까지 연장한다** — 이유: 화살표의
  반대쪽 끝(발신자)이나, 활성화와 무관한 일반 메시지는 지금처럼 기준 생명선 칸을
  그대로 쓴다. 라벨 가운데 정렬 기준(`lo`/`hi`)은 기준 생명선 칸으로 그대로 두고,
  화살촉과 마지막 가로선 구간만 연장해서 기존 라벨 배치 로직을 건드리지 않는다.

## Components and Interfaces

### SequenceLayout (신규 필드·메서드)
- Intent: 참여자별 최대 동시 활성화 깊이를 미리 계산해 간격 계산과 그리기 양쪽에서
  쓴다
- Requirements: 1.1, 1.2, 1.3, 2.1, 2.2

```rust
struct SequenceLayout<'a> {
    // 기존 필드 유지 …
    /// 참여자별 최대 동시 활성화 깊이(겹치지 않으면 1). 오프셋 칸 수 = (깊이-1).min(ACTIVATION_OFFSET_CAP).
    max_activation_depth: Vec<usize>,
    /// 항목 인덱스 → 그 메시지가 여닫는 활성화의 (참여자, 오프셋 칸 수).
    /// 활성화를 열거나 닫지 않는 메시지는 None.
    activation_offset: Vec<Option<(usize, usize)>>,
}

const ACTIVATION_OFFSET_CAP: usize = 3;

fn offset_of(depth_index: usize) -> usize {
    depth_index.min(ACTIVATION_OFFSET_CAP)
}
```
- 계약: `max_activation_depth`·`activation_offset`은 `collect_fragments()`와 같은
  시점에(빌드 초반, `position_participants()`보다 먼저) 한 번 계산해 채운다.
  self-message(`from == to`) 항목은 `activation_offset`에서 항상 `None`(out-of-scope).

### position_participants (기존 함수 확장)
- Intent: 참여자 간격 제약에 활성화 오프셋만큼의 여유를 추가한다
- Requirements: 2.1, 2.2
- 계약: 기존 `constraints` 리스트에 `max_activation_depth[p] > 1`인 각 참여자 `p`마다
  `(p, p+1, offset_of(max_activation_depth[p]-1) + 기존 최소 여백)` 제약을 추가한다
  (`p+1`이 없으면 `right_margin`에 반영). 겹치지 않는 참여자(깊이 1)는 제약이 추가되지
  않아 간격이 지금과 동일하다(2.2).

### draw (기존 함수 확장)
- Intent: 활성화 막대를 깊이 인덱스만큼 어긋난 칸에 그린다
- Requirements: 1.1, 1.2, 1.3
- 계약: 활성화 막대를 그리는 루프가 각 구간의 깊이 인덱스를 함께 들고 있어야 하므로
  `active[p]: Vec<(usize, Option<usize>)>`에 깊이 인덱스를 더해
  `Vec<(usize, Option<usize>, usize)>`로 확장한다. 그리는 칸은
  `self.centers[p] + offset_of(depth_index)`.

### draw_message (기존 함수 확장)
- Intent: 활성화를 열거나 닫는 메시지의 화살표 끝을 오프셋 칸까지 연장한다
- Requirements: 3.1, 3.2
- 계약: `activation_offset[index]`가 `Some((p, offset))`이고 `p`가 `from`이면
  발신 쪽 끝을, `p`가 `to`이면 수신 쪽 끝을 기준 칸에서 `offset`만큼(오른쪽 참여자
  방향으로) 더 연장해 가로선·화살촉을 그린다. `None`이면 기존과 동일하게 동작한다
  (1.3, 기존 동작 불변).

## Data Models
신규 도메인 타입 없음 — 위 인터페이스 명세로 충분하다.

## Error Handling
- **사용자 입력 오류**: 해당 없음(파싱은 mermaid/plantuml sequence 파서 소관, 변경 없음)
- **시스템 오류(패닉)**: `offset_of()`가 상한을 `.min()`으로 강제하므로 깊이가 아무리
  커도 오프셋 칸 수는 항상 `ACTIVATION_OFFSET_CAP` 이하 — 배열 인덱스 초과 위험 없음(4.1)
- **기능 강등**: 없음 — 폭 초과 시 기존 `render()`의 캡 축소 재시도·`None` 반환 계약
  그대로(변경 없음)

## Testing Strategy
- **Depth**: Standard — 기존 로직 확장(간격 계산 1곳, 그리기 2곳)이지만 새 상태
  (참여자별 최대 깊이)와 좌표 계산 분기가 늘어난다.
- **Unit(L6)**:
  - 겹치는 활성화 2개가 서로 다른 칸에 그려지는지(1.1)
  - 3개 이상일 때 칸이 하나씩 더 어긋나는지(1.2)
  - 겹치지 않는 기존 픽스처(`nested_fragments_track_inner_depth` 등 기존 테스트)가
    그대로 통과하는지(1.3, 회귀)
  - 겹침이 있는 참여자의 간격이 넓어지고, 없는 참여자 간격은 그대로인지(2.1, 2.2)
  - 활성화를 열고 닫는 메시지의 화살표 끝이 오프셋 칸과 같은 열에 오는지(3.1, 3.2)
  - 깊이가 상한을 넘어도 패닉 없이 렌더링이 끝나는지, 상한 칸에서 더 벌어지지
    않는지(4.1)
- **Integration**: `examples/sequence.md`의 기존 겹침 없는 alt/opt/loop 픽스처를
  release 바이너리로 렌더링해 외형이 그대로인지 육안 확인(1.3 회귀).
