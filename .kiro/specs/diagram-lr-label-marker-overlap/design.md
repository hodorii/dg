# Design — diagram-lr-label-marker-overlap

## 정의
LR 방향에서 관계 라벨이 두 글자짜리 from측 표식을 덮어써 지우는 결함과, tail_label·
head_label이 표식과 달리 노드 경계에 바로 붙지 않던 불일치를 함께 고친다.

## 원인 (Root Cause)
`src/diagram/layout/graph.rs`의 `draw_segment_decorations()`는 `label`을
`top + 1`에, `tail_label`도 같은 `top + 1`에, `head_label`은 `bottom - width`에
그렸다. 셋의 실제 처지가 다른데도 표식 칸을 고려하지 않은 고정식을 썼던 것이 두
결함(글자 손실·경계 불일치)의 공통 원인이다.

- **`label`**(관계 이름)은 표식(`top_marker`)과 **같은 줄**(`segment.exit`)을 쓴다.
  표식은 1~2글자를 `top`부터 채우므로(1786~1797행 부근), 표식이 2글자
  (`CrowZeroOne`/`CrowMany`/`CrowZeroMany`)면 `label`이 시작하는 `top+1`이 표식의
  두 번째 글자 자리와 정확히 겹쳐 그 글자를 덮어쓴다(1.1).
- **`tail_label`**(카디널리티 등)과 **`head_label`**(다중성 등)은 표식보다 **한 줄
  위/아래**(`segment.exit - 1`/`segment.entry - 1`)에 그려진다 — 표식은 언제나
  `segment.exit`/`segment.entry` 한 줄에만 그려지므로(여러 글자라도 같은 줄에서
  가로로 늘어선다) 이 둘은 애초에 표식과 칸을 다툴 일이 없다. 그런데도 `top+1`
  (`tail_label`)·`bottom - width`(`head_label`, 마지막 글자가 `bottom-1`에서 끝남)를
  써서, 표식이 경계에 바로 붙는 것과 달리 두 라벨 모두 경계와 한 칸 떨어져 보였다
  (1.2, 1.3) — 표식 유무·글자 수와 무관하게 필요 없던 여백이었다.

## 수정 방식
`Direction::LeftRight` 분기에서 세 라벨의 x좌표를 각자의 처지에 맞게 분리한다.
- `label`: `top + marker_glyphs(top_marker, self.direction, true).len().max(1)` —
  표식이 실제로 차지하는 칸 수만큼만 건너뛴다(표식이 1글자면 종전과 같은 `top+1`).
- `tail_label`: `top` 그대로 — 표식 글자 수를 볼 필요가 없으므로 노드 경계 바로 다음
  칸에 항상 붙인다.
- `head_label`: `(bottom + 1) - width` — 마지막 글자가 `bottom`(표식의 마지막 글자와
  같은 자리)에서 끝나 to측 노드 경계에 바로 붙는다. `tail_label`과 대칭이다.

표식 자체 계산, `label`이 표식과 같은 줄에 붙어 보이는 배치 자체는 건드리지 않는다.

### 검토한 대안(기각)
사용자가 함께 검토를 요청한 두 안 — (A) 표식 줄엔 표식만 남기고 카디널리티+관계
라벨을 위 줄에 합치는 안(`1 nodes` / `♦────`), (B) 표식 줄엔 표식만, 관계 라벨은
그 아래 별도 줄로 내리는 안(`♦────` / `nodes`) — 은 기각한다.
- 관계 라벨은 이 코드베이스 전체(TopDown 중간 라벨 포함)에서 일관되게 "선 위에
  붙여 그 선이 무엇인지 바로 보여주는" 관례를 따른다. A·B 모두 라벨을 연결선이
  지나가는 줄에서 떼어내는 것이라, 이 관례를 이 경로에서만 깨고 라우팅·채널 계산
  (`channel_position`, `gap_label_room` 등)까지 다시 손봐야 하는 큰 변경이 된다.
- A는 카디널리티와 관계 이름이 한 줄에 섞여(`1 nodes`) 정작 표식 줄(`♦────`)엔
  아무 설명이 안 남아 오히려 "이 줄이 무슨 관계인지"를 표식만으로는 알 수 없게
  된다. B는 관계 라벨이 표식 줄 아래 세 번째 줄로 떨어져, 바로 아래에 다른 개체의
  내용이 있으면 그 개체 설명처럼 보일 위험이 있다.
- 지금 구조(표식 줄에 표식+라벨, 그 위/아래 줄에 tail_label/head_label만)는 ER의
  기존 카디널리티 표시(`diagram-lr-tail-label-position`에서 이미 확인)와도 같은
  모양이라 일관적이다. 이번 결함은 "붙는 정도"가 줄마다·양끝마다 달랐던 것뿐이라,
  구조를 바꾸지 않고 간격만 맞추는 쪽이 가장 작은 변경이다.

## 검증 속성
- (a) 결함 재현: 2글자 tail 표식(`CrowZeroMany`)과 라벨이 있는 그래프로 표식 두
  글자가 모두 남아 있는지 확인하는 테스트, tail_label·head_label이 각자의 경계
  바로 옆 칸에 붙는지 확인하는 테스트를 각각 추가하면, 수정 전 코드에서는
  **실패**함(1.1, 1.2, 1.3)을 확인.
- (b) 기대 동작: 같은 테스트들이 수정 후 통과(2.1, 2.2, 2.3).
- (c) 불변 동작: 기존 `left_right_tail_label_hugs_its_own_node_not_the_widest_
  sibling` 등 LR/TB 라벨 위치 테스트가 수정 후에도 그대로 통과(3.1, 3.3). `label`이
  표식 바로 다음 칸에서 시작하는 배치는 육안으로 재확인(3.2).

## 영향 범위
- `src/diagram/layout/graph.rs`: `draw_segment_decorations()`의 `Direction::LeftRight`
  분기(`label`·`tail_label`·`head_label` x좌표 각각 조정), 테스트 3건.
- 다른 파일 없음.
