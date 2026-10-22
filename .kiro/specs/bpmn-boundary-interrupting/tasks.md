# 구현 계획 - bpmn-boundary-interrupting

## 정의
설계서의 컴포넌트를 기반, 핵심, 검증 순서로 구현해 경계 이벤트의 방해와 비방해를 읽고 다르게 그리는 계획이다.

- [ ] 1. 기반
- [x] 1.1 경계 이벤트 호스트 참조에 방해 여부 묶기
  - 호스트 참조를 호스트와 방해 여부를 함께 가진 값으로 바꾸고, 기존 생성 지점(XML, YAML, bizprocess)은 모두 방해로 채운다.
  - 호스트 참조를 읽는 검사, 컨테이너 상속, 경계 간선 생성은 호스트만 읽도록 맞춘다.
  - _DoneWhen: 컴파일 통과, 기존 BPMN 테스트 전부 무수정 통과(동작 변화 없음), bizprocess 오류 경계 이벤트 단위 테스트가 방해를 확인_
  - _Requirements: 1.1, 1.3, 4.1_
  - _Difficulty: mid_
  - _Boundary: bpmn::model_
- [ ] 1.2 이벤트 도형에 방해 여부와 비방해 글자 더하기
  - 이벤트 도형이 위치와 방해 여부를 함께 갖게 하고, 모든 생성 지점은 방해로 채운다.
  - 글자 표: 중간 + 비방해는 점선 원, 그 밖은 기존 위치 글자.
  - 커버리지 허용 목록에 점선 원을 더하고 목록 설명의 확인 기준 폰트에 D2Coding을 더한다.
  - _DoneWhen: 도형 단위 테스트에서 중간 + 비방해 이벤트가 `◌ Go`, 중간 + 방해가 `◎ Go`로 그려짐, 기존 도형, 그래프 테스트 무수정 통과_
  - _Requirements: 2.1, 2.2, 4.2_
  - _Difficulty: mid_
  - _Boundary: diagram::ir, layout::shape_

- [ ] 2. 핵심
- [ ] 2.1 (P) XML `cancelActivity` 읽기
  - 속성 없음, `"true"`는 방해, `"false"`는 비방해, 그 밖 값은 새 파서 오류.
  - _DoneWhen: 네 경우(없음, true, false, no) 단위 테스트 통과, `"no"` 입력의 `bpmn::render` 결과가 원문 물러남_
  - _Requirements: 1.1, 1.2, 1.5_
  - _Difficulty: low_
  - _Boundary: bpmn::parse_xml_
- [ ] 2.2 (P) YAML `cancel_activity` 읽기
  - 노드 키 목록에 더하고, 스칼라 `true`, `false`만 허용, 그 밖은 새 불리언 오류, 경계 아닌 노드에 있으면 기존 허용 안 됨 오류.
  - _DoneWhen: 네 경우와 경계 아닌 노드 단위 테스트 통과, 잘못된 값 입력의 `bpmn::render` 결과가 원문 물러남_
  - _Requirements: 1.3, 1.4, 1.6_
  - _Difficulty: low_
  - _Boundary: bpmn::parse_yaml_
- [ ] 2.3 (P) 오류, 취소 트리거의 비방해 거부
  - 경계 이벤트 검사에서 비방해 + 오류 또는 취소 트리거를 새 모델 오류로.
  - _DoneWhen: 트리거(오류, 취소, 타이머, 없음) x 방해 여부 단위 테스트 통과, 비방해 오류 경계 문서의 `bpmn::render` 결과가 원문 물러남_
  - _Requirements: 3.1, 3.2_
  - _Difficulty: low_
  - _Boundary: bpmn::validate_
- [ ] 2.4 경계 이벤트 방해 여부를 도형으로 전달
  - 경계 이벤트는 호스트 참조의 방해 여부, 그 밖 이벤트는 방해로 이벤트 도형을 만든다. 경계 간선은 그대로 실선.
  - _DoneWhen: 하강 단위 테스트에서 비방해 경계 이벤트 노드 도형이 비방해이고 호스트에서 오는 간선이 실선_
  - _Requirements: 2.2, 2.4_
  - _Difficulty: low_
  - _Boundary: bpmn::lower_

- [ ] 3. 검증
- [ ] 3.1 방해와 비방해 혼합 렌더링 통합 테스트
  - XML, YAML 각각 방해 타이머 경계와 비방해 타이머 경계를 한 호스트 또는 두 호스트에 둔 문서를 LR, TB로 그린다.
  - _DoneWhen: 네 렌더링 출력에서 방해 이벤트 칸이 `◎`, 비방해 이벤트 칸이 `◌`, 두 이벤트의 들어오고 나가는 선 칸이 실선 글자(`─` `│` 꺾임)이고 `╌` `╎` `┈` `┊`가 없음, `◌` 바로 앞 칸이 공백 또는 선 글자_
  - _Requirements: 2.3, 2.4, 2.5_
  - _Difficulty: mid_
- [ ] 3.2 속성 기반 테스트 P1, P2
  - P1: 비방해 경계가 없는 seed 고정 무작위 YAML 문서 렌더링이 기능 전 기준 출력과 같다.
  - P2: 비방해 경계 이벤트를 넣은 무작위 문서의 렌더링 글자가 모두 허용 목록 안이다.
  - _DoneWhen: 두 속성 테스트 통과, 실패 시 축소된 반례를 출력_
  - _Requirements: 4.1, 4.2_
  - _Difficulty: mid_
- [ ] 3.3 예제와 인수 확인
  - BPMN 예제 문서에 비방해 경계 이벤트 예제를 더한다.
  - _DoneWhen: `dg -P -s none examples/bpmn.md` 출력에 새 예제의 `◌`가 보이고, 기존 예제 구간 출력이 기능 전과 바이트 단위로 같음, `make test`와 `make lint` 통과_
  - _Requirements: 4.1_
  - _Difficulty: low_
