# 구현 계획 - bpmn-pool-message-layout

## 정의
버그픽스 순서(재현 테스트, 불변 테스트, 수정, 양쪽 통과)로 설계의 네 수정을 구현하는 계획이다.

- [x] 1. 재현 테스트 (수정 전 실패)
- [x] 1.1 입력 A~D 재현 테스트
  - _DoneWhen: 입력 A~D를 TB, LR로 렌더링한 출력 단언(A: 받는 풀이 앞, B: 메시지 선끼리 이음 글자, C, D: 표식 수 부족을 뒤집은 기대 단언)이 수정 전 코드에서 실패하는 것을 출력으로 확인_
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 2.4_
  - _Difficulty: mid_
  - _Boundary: bpmn 렌더링 테스트_
- [x] 1.2 입력 E 재현 테스트
  - _DoneWhen: 입력 E를 TB, LR로 렌더링해 메시지 선이 끝 노드 말고 다른 노드 상자를 지나지 않고 `기록` 테두리가 그대로임을 단언하는 테스트가 수정 전 코드에서 실패하는 것을 출력으로 확인_
  - _Requirements: 1.5, 2.5_
  - _Difficulty: mid_
  - _Boundary: bpmn 렌더링 테스트_
- [x] 1.3 입력 F 재현 테스트
  - _DoneWhen: 입력 F를 TB, LR로 렌더링해 메시지 이름표 글자가 모두 보이고 풀 테두리가 끊기지 않음을 단언하는 테스트가 수정 전 코드에서 실패하는 것을 출력으로 확인_
  - _Requirements: 1.6, 2.6_
  - _Difficulty: mid_
  - _Boundary: bpmn 렌더링 테스트_

- [x] 2. 불변 테스트 (수정 전 통과)
- [x] 2.1 (P) 메시지 없음, 순방향, 끝 하나씩인 문서의 0.4.0 결과 고정 속성 테스트
  - _DoneWhen: 무작위 문서(참여자 0~3, 노드 2~6, TB, LR) 렌더링 해시를 수정 전 코드로 기록하고 그 기록과 같음을 단언하는 테스트가 수정 전에 통과, 실패 시 시드와 문서 출력_
  - _Requirements: 3.1, 3.2, 3.3, 3.5_
  - _Difficulty: mid_
  - _Boundary: bpmn 렌더링 테스트_

- [ ] 3. 수정
- [x] 3.1 풀 순서에 메시지 방향 제약 반영
  - _DoneWhen: 입력 A의 TB, LR 출력에서 고객 풀이 판매사 앞에 놓이고 메시지가 위에서 아래(왼쪽에서 오른쪽)로 흐름, 2.1 테스트 통과_
  - _Requirements: 1.1, 2.1, 3.1_
  - _Difficulty: mid_
  - _Boundary: ir, bpmn::lower, layout::graph_
  - _Depends: 1.1, 2.1_
- [x] 3.2 사이 통로 단위 줄 배정과 칸 끼우기
  - _DoneWhen: 입력 B2 가로 배치의 마주 보는 두 풀 사이에서 두 면의 선이 서로 다른 줄을 쓰고 이음 글자로 합쳐지지 않음, 통로에 붙은 풀이 보낸 메시지는 그 면 쪽 줄, 지나가는 메시지는 받는 면 쪽 줄임이 출력에서 확인됨(3.1 이후 입력 B에는 마주 보는 통로가 없음)_
  - _Requirements: 1.2, 2.2, 3.2_
  - _Difficulty: high_
  - _Boundary: layout::graph_
  - _Depends: 3.1_
- [ ] 3.3 사이 통로를 곧게 지나는 다리의 충돌 회피
  - _DoneWhen: 입력 B의 TB, LR에서 다른 풀을 곧게 지나는 메시지가 다른 메시지 선과 같은 칸을 같은 방향으로 쓰지 않고 직교 교차만 남음, 입력 G(마주 보는 통로에서 두 면의 노드 쪽 다리가 같은 흐름축 칸)의 TB, LR에서도 이음 글자로 합쳐지지 않음_
  - _Requirements: 1.3, 2.3_
  - _Difficulty: high_
  - _Boundary: layout::graph_
  - _Depends: 3.2_
- [ ] 3.4 노드 쪽 끝 칸 나누기
  - _DoneWhen: 입력 C, D의 TB, LR 출력에서 모든 메시지의 `○`와 빈 삼각형이 노드 쪽에 따로 보이고 선이 합쳐지지 않음_
  - _Requirements: 1.4, 2.4_
  - _Difficulty: high_
  - _Boundary: layout::graph_
  - _Depends: 3.3_
- [ ] 3.5 노드 쪽 다리의 다른 노드 피하기
  - _DoneWhen: 입력 E의 TB, LR 출력과 `examples/bpmn.md`에서 메시지 선이 끝 노드 말고 다른 노드 상자를 지나지 않고 노드 테두리 글자가 그대로임, 1.2 테스트 통과_
  - _Requirements: 1.5, 2.5, 3.2_
  - _Difficulty: high_
  - _Boundary: layout::graph_
  - _Depends: 1.2, 3.4_
- [ ] 3.6 메시지 이름표의 풀 테두리 피하기
  - _DoneWhen: 입력 F의 TB, LR 출력과 `examples/bpmn.md`에서 메시지 이름표가 풀, 레인 테두리와 다른 선, 노드를 덮지 않고 테두리가 끊기지 않음, 1.3 테스트 통과, BPMN이 아닌 `examples/*.md` 바이트 동일_
  - _Requirements: 1.6, 2.6, 3.4, 3.5_
  - _Difficulty: mid_
  - _Boundary: layout::graph_
  - _Depends: 1.3, 3.5_

- [ ] 4. 검증
- [ ] 4.1 다중 메시지 속성 테스트와 회귀 확인
  - _DoneWhen: 무작위 문서(블랙박스 풀 1~3, 메시지 2~6, TB, LR)에서 메시지마다 `○`와 빈 삼각형이 하나씩 서로 다른 칸에 있고 메시지 선끼리 같은 칸을 같은 방향으로 쓰지 않고 다른 노드 상자를 지나지 않으며 이름표가 테두리를 덮지 않음을 단언하는 속성 테스트 통과, 1.1~1.3, 2.1 통과, `examples/*.md`는 2.1에 따른 풀 순서 변화 말고 바이트 동일, 전체 테스트와 clippy 통과_
  - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 3.4, 3.5_
  - _Difficulty: mid_
- [ ] 4.2 성능과 메모리 점검 (최종, 필수)
  - _DoneWhen: 0.4.0 바이너리와 수정 바이너리로 `examples/*.md`와 부하 문서(메시지 N개와 2N개, 블랙박스 풀 여럿)를 렌더링해 실행 시간(여러 번 측정의 중앙값)과 최대 RSS(`/usr/bin/time -v`)를 표로 비교하고, 예제는 10% 이내 증가, 부하 문서는 2배 입력에 4배 미만 시간임을 확인_
  - _Requirements: 3.6_
  - _Difficulty: mid_
  - _Depends: 4.1_

