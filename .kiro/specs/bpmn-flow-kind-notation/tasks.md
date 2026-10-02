# 구현 계획 - bpmn-flow-kind-notation

## 정의
설계서의 컴포넌트를 기반, 핵심, 검증 순서로 구현하는 계획이다.

- [x] 1. 기반
- [x] 1.1 (P) 캔버스 잔 점선 무늬
  - _DoneWhen: 캔버스 테스트 출력이 `┈┈╮`, `┊`, `──┼──`이고 같은 칸 대시선 겹침은 `╌`, 기존 캔버스 테스트 무수정 통과_
  - _Requirements: 1.1, 4.1_
  - _Difficulty: mid_
  - _Boundary: canvas_
- [x] 1.2 (P) 테두리 상자 도형 판정
  - _DoneWhen: 모든 도형의 판정 표 테스트 통과_
  - _Requirements: 3.1_
  - _Difficulty: low_
  - _Boundary: ir_
- [x] 1.3 (P) 흐름 끝 소속 풀 판정 공개화와 검증 위임
  - _DoneWhen: 판정 단위 테스트(참여자, 레인, 하위 레인, 풀 없음, 모르는 id) 통과, 기존 검증 테스트 무수정 통과_
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: bpmn::model, bpmn::validate_

- [x] 2. 핵심
- [x] 2.1 BPMN 대응표 변경
  - _DoneWhen: 대응표 단위 테스트 통과, 메시지 흐름 렌더링에 `○`, `╌`, `▷`가 보임_
  - _Requirements: 1.1_
  - _Difficulty: low_
  - _Boundary: bpmn::lower_
  - _Depends: 1.1_
- [x] 2.2 (P) YAML 흐름 종류 자동 선택
  - _DoneWhen: 판정 표 단위 테스트 통과, 풀을 넘는 `-->` 문서가 메시지로 렌더링, `==>`는 원문_
  - _Requirements: 2.1, 2.2_
  - _Difficulty: mid_
  - _Boundary: bpmn::parse_yaml_
  - _Depends: 1.3_
- [x] 2.3 (P) LR 글자 없는 상자 노드 접점 분리
  - _DoneWhen: mermaid `flowchart LR`의 `A[" "]`에서 셋이 나가고 둘이 들어올 때 노드 옆 칸에 `┬` `┴` `┼`가 없고 흐름이 서로 다른 줄_
  - _Requirements: 3.1_
  - _Difficulty: high_
  - _Boundary: layout::graph_
  - _Depends: 1.2_

- [x] 3. 검증
- [x] 3.1 렌더링 통합 테스트
  - _DoneWhen: 네 흐름 YAML의 TB, LR 곧은 구간이 세 갈래, 풀 면 메시지 양방향 표식 위치, 자동 선택 문서와 위반 문서 결과, 글자 없는 상자 노드(mermaid, BPMN `task`, 주석)와 점 모양 노드 대조가 모두 출력 단언으로 통과_
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1_
  - _Difficulty: mid_
  - _Depends: 2.1, 2.2, 2.3_
- [x] 3.2 (P) 속성 테스트 P2, P3, P4
  - _DoneWhen: 세 속성 테스트가 정해진 반복 횟수만큼 통과하고, 실패 시 시드와 입력을 출력_
  - _Requirements: 2.1, 3.1, 4.2_
  - _Difficulty: mid_
  - _Boundary: bpmn 테스트, layout::graph 테스트, layout::shape 테스트_
  - _Depends: 2.1, 2.2, 2.3_
- [x] 3.3 예제와 회귀(P1)
  - _DoneWhen: `examples/bpmn.md` 네 흐름 블록을 TB와 LR로 렌더링한 출력에서 시퀀스는 `─`/`│`에 `▶`(LR) 또는 `▼`(TB), 메시지는 `╌`/`╎`에 풀 면의 `○`와 끝의 `◁`/`△`, 데이터 연결은 `┈`/`┊`에 `>`(LR) 또는 `∨`(TB), 연결은 `┈`/`┊`에 머리 없음이 보이고, `git diff examples/architecture.txt` 비어 있으며, 전체 테스트(해시 테스트 포함)와 clippy 통과 (원래 문구는 가로 배치의 글자 인접을 가정해 세로에서 반증 불가였음, 3.3 검토에서 정정)_
  - _Requirements: 1.2, 4.1_
  - _Difficulty: low_

- [x] 4. 재작업: 블랙박스 풀 면 메시지 (3.1 검증에서 발견, 3.3보다 먼저 실행)
- [x] 4.1 노드에서 블랙박스 풀로 들어가는 메시지의 바깥 통로 방향
  - _DoneWhen: 노드에서 블랙박스 풀로 가는 메시지가 TB, LR 모두 풀 면을 표식 칸 말고는 가로지르지 않고 풀 바깥 통로를 따라 풀 면 표식에 닿음(재현 테스트가 수정 전 실패, 수정 후 통과)_
  - _Requirements: 1.1_
  - _Difficulty: mid_
  - _Boundary: layout::graph_
- [x] 4.2 한 블랙박스 풀 면에 메시지가 둘 이상 붙을 때 표식 분리
  - _DoneWhen: 한 블랙박스 풀이 보내고 받는 메시지가 함께 있는 문서의 TB, LR 출력에서 풀 면에 `○`와 빈 삼각형이 모두 보이고 서로 다른 칸에 있음(재현 테스트가 수정 전 실패, 수정 후 통과)_
  - _Requirements: 1.1_
  - _Difficulty: high_
  - _Boundary: layout::graph_
