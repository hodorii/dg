# Implementation Plan — sequence-nested-activation-offset

## 정의
시퀀스 다이어그램에서 한 참여자에게 겹치는 활성화 구간이 있을 때 UML 관례대로
옆으로 어긋나게(계단식) 그려 겹침 정도를 구분되게 보여주는 기능이다.

- [x] 1. 참여자별 최대 동시 활성화 깊이·메시지별 오프셋 계산(기반)
- [x] 1.1 `max_activation_depth`·`activation_offset` 필드와 계산 로직 추가
  - DONE: `SequenceLayout`에 `max_activation_depth: Vec<usize>`(참여자별)와
    `activation_offset: Vec<Option<(usize, usize)>>`(항목 인덱스별, self-message는
    항상 `None`)를 채우는 준비 단계를 `collect_fragments()`와 같은 시점에 추가하고,
    `ACTIVATION_OFFSET_CAP = 3`·`offset_of()` 상수·헬퍼를 둔다. 깊이 인덱스는
    LIFO(`close_activation`이 가장 최근에 연 것부터 닫는 것)를 전제로, 활성화가
    열리는 시점에 그 참여자에서 이미 열려 있는 구간 수로 고정한다. 단위 테스트로
    겹치는 활성화 2개·3개 픽스처를 넣었을 때 `max_activation_depth`·
    `activation_offset` 값이 기대대로 나옴을 확인
  - _Requirements: 1.1, 1.2_
  - _Difficulty: mid_
  - _Boundary: sequence 레이아웃(준비 단계)_

- [x] 2. 참여자 간격에 오프셋 반영
- [x] 2.1 `position_participants()`에 오프셋 제약 추가
  - DONE: `max_activation_depth[p] > 1`인 참여자마다 `(p, p+1)` 간격(또는 `p`가
    마지막이면 `right_margin`)에 `offset_of(max_activation_depth[p]-1)`만큼의
    여유를 더하는 제약을 기존 `constraints` 리스트에 추가. 겹치는 참여자가 있는
    픽스처는 그 옆 간격이 넓어지고, 겹치지 않는 기존 픽스처(`sample()` 등)는
    간격이 지금과 동일함을 확인하는 단위 테스트 추가
  - _Requirements: 2.1, 2.2_
  - _Difficulty: low_
  - _Boundary: sequence 레이아웃(간격 계산)_
  - _Depends: 1.1_

- [x] 3. 활성화 막대·화살표 그리기
- [x] 3.1 활성화 막대를 깊이 인덱스만큼 어긋난 칸에 그린다
  - DONE: `draw()`의 활성화 막대 루프가 쓰는 `active[p]`에 깊이 인덱스를 더해
    `Vec<(usize, Option<usize>, usize)>`로 확장하고, 그리는 칸을
    `self.centers[p] + offset_of(depth_index)`로 바꾼다. 겹치는 활성화 2개짜리
    렌더 결과에서 두 막대가 서로 다른 칸(`┃`)에 나오고, 겹치지 않는 기존 렌더
    결과(`nested_fragments_track_inner_depth` 등)는 막대 칸이 그대로임을 테스트로
    확인(1.1, 1.3)
  - _Requirements: 1.1, 1.3_
  - _Difficulty: mid_
  - _Boundary: sequence 레이아웃(그리기)_
  - _Depends: 1.1_

- [x] 3.2 3겹 이상일 때 칸이 하나씩 더 어긋나는지, 상한에서 멈추는지 확인
  - DONE: 활성화 3겹·5겹(상한 초과) 픽스처를 렌더링해, 3겹은 칸이 각각 다르게,
    5겹은 `ACTIVATION_OFFSET_CAP`(3칸)에서 더 벌어지지 않고 겹쳐 그려지며 패닉
    없이 끝까지 렌더됨을 테스트로 확인
  - _Requirements: 1.2, 4.1_
  - _Difficulty: low_
  - _Boundary: sequence 레이아웃(그리기)_
  - _Depends: 3.1_

- [x] 3.3 `draw_message()`가 활성화 여닫는 쪽 화살표 끝을 오프셋 칸까지 연장
  - DONE: `activation_offset[index]`가 `Some((p, offset))`이면 `p`가 `from`인지
    `to`인지에 따라 그쪽 화살표 끝(가로선·화살촉)만 `offset`만큼 연장하도록
    `draw_message()`를 바꾼다. 활성화를 여는 메시지의 화살촉이 새로 열리는 막대의
    칸과 같은 열에 오는지, 닫는 메시지의 화살표 출발점이 닫히는 막대의 칸과 같은
    열에 오는지 테스트로 확인. `None`인 일반 메시지는 기존 렌더 결과와 동일함을
    확인(회귀)
  - _Requirements: 3.1, 3.2_
  - _Difficulty: mid_
  - _Boundary: sequence 레이아웃(그리기)_
  - _Depends: 1.1, 3.1_

- [x] 4. 검증
- [x] 4.1 전체 스위트 + clippy + 실물 검증
  - DONE: `cargo test` 전량 통과(신규 테스트 포함), `cargo clippy --all-targets --
    -D warnings` 클린, `examples/sequence.md`의 기존 alt/opt/loop(겹침 없음)
    픽스처를 release 바이너리로 렌더링해 외형이 이전과 동일함을 육안 확인, 겹치는
    활성화가 있는 새 예시(2겹·3겹)를 렌더링해 막대가 계단식으로 어긋나 보이고
    화살표가 정확한 칸에 붙음을 육안 확인
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2, 3.1, 3.2, 4.1_
  - _Difficulty: low_
  - _Boundary: 검증_
  - _Depends: 2.1, 3.2, 3.3_
