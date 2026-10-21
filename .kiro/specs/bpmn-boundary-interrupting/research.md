# 조사와 설계 결정

## 요약
- **기능**: `bpmn-boundary-interrupting`
- **조사 범위**: Extension
- **핵심 발견**:
  - `cancelActivity`는 XML, YAML 어느 파서도 읽지 않는다. 경계 이벤트는 `◎`와 실선 간선 하나로만 그려진다(`lower.rs` `add_boundary_edges`).
  - BPMN 흐름 선 무늬는 이미 종류마다 주인이 있다: 실선 시퀀스, 파선 메시지, 점선 연결과 데이터 연결(`lower.rs` 384~388).
  - `Element` 구조체 리터럴 77곳 중 `attached_to: Some`은 11곳뿐이다.

## 조사 기록

### 비방해 표기 수단
- **배경**: 사용자 결정 "식별성 좋은 것, 아니면 점선 간선".
- **참고 출처**: BPMN 2.0.2 명세 10.5.4(비방해 경계 이벤트 = 점선 이중 원), `fc-list ":family=Noto Sans Mono CJK KR:charset=<코드>"` 실측, Python `unicodedata.east_asian_width`.
- **발견**: 점선 간선은 메시지 흐름의 파선, 연결의 점선과 겹쳐 오히려 식별성을 떨어뜨린다. 글자 후보 실측 결과는 아래 표.
- **시사점**: 선은 그대로 두고 이벤트 글자만 바꾼다.

| 글자 | 이름 | CJK 모노 커버리지 | 폭 분류 |
|---|---|---|---|
| U+25CC | DOTTED CIRCLE | 있음 | N |
| U+2299 | CIRCLED DOT OPERATOR | 있음 | A |
| U+25D0 | CIRCLE WITH LEFT HALF BLACK | 있음 | A |
| U+25CD | CIRCLE WITH VERTICAL FILL | 없음 | N |
| U+229A | CIRCLED RING OPERATOR | 없음 | N |

## 아키텍처 패턴 평가

| 선택지 | 설명 | 장점 | 위험과 한계 | 비고 |
|---|---|---|---|---|
| 점선 간선 | 호스트에서 이벤트로 가는 선을 점선으로 | 그리기 변경 최소 | 연결, 메시지 흐름과 혼동 | 사용자 차선책, 기각 |
| 글자 U+25CC | 점선 원 | BPMN 점선 원과 같은 운율, 한 칸 폭 | 일부 폰트에서 결합 기호 자리표로 그려질 수 있음 | 채택 |
| 글자 U+2299 | 점 박힌 원 | 커버리지 있음 | 폭 분류 A라 터미널 설정에 따라 두 칸, BPMN 운율 없음 | 기각 |
| 글자와 점선 간선 둘 다 | | 가장 눈에 띔 | 선 무늬 충돌은 그대로 | 기각 |

## 설계 결정

### 결정: `방해 여부를 둘 자리`
- **배경**: 모델과 IR 둘 다 방해 여부를 알아야 한다.
- **검토한 대안**:
  1. `Element`에 `is_interrupting: bool` 추가 - 리터럴 77곳 수정, 경계가 아닌 요소에도 값이 생김
  2. `EventPosition`에 비방해 변형 추가 - 모델의 위치 검사와 어휘표 match까지 번짐
  3. `attached_to: Option<Attachment>` + `Shape::Event { position, is_interrupting }` - 경계에만 값이 있음
- **선택안**: 3
- **근거**: 잘못된 상태를 타입으로 막고 수정 범위가 가장 작다(`Some` 11곳, `Shape::Event` 생성 지점 약 15곳).
- **득실**: `Shape::Event`의 방해 여부는 지금은 경계 이벤트에만 쓰인다. 이벤트 서브프로세스 비방해 시작 이벤트가 오면 그대로 재사용할 수 있다.
- **후속 확인**: 구현 중 `Shape::Event` 생성 지점 수 실측.

## 위험과 완화
- U+25CC가 결합 기호 자리표라 앞 글자와 겹쳐 그려지는 터미널 - 글자 앞이 항상 공백이거나 줄 시작인지 통합 테스트로 확인, 문제면 U+2299로 바꾸고 사용자 확인.
- `Shape::Event` 필드형 변경이 다른 다이어그램 테스트에 번짐 - 컴파일러가 모든 지점을 알려 주므로 누락 위험 없음.

## 참고 자료
- [BPMN 2.0.2](https://www.omg.org/spec/BPMN/2.0.2/) - 10.5.4 경계 이벤트, `cancelActivity` 기본값 true, 오류와 취소는 항상 방해
