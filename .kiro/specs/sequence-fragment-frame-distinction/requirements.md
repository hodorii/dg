# Requirements — sequence-fragment-frame-distinction

## 정의
시퀀스 다이어그램을 읽는 사용자를 위해, `alt`/`opt`/`loop` 등 프래그먼트 테두리를 메시지·
생명선(흐름)과 다른 선패턴으로 그려 `--style none`에서도 구분 가능하게 하는 기능이다.

## Boundary Context
- **In scope**: `alt`/`opt`/`loop`/`par`/`critical`/`break`/`group`/`rect` 프래그먼트 바깥
  테두리의 `LineKind` 변경, 메시지·생명선과의 교차 지점 정합성, 기존 골든 출력
  (`examples/architecture.txt`) 갱신
- **Out of scope**: `else`/`option`/`and` 구분선(기존 파선 유지), 프래그먼트 제목 텍스트
  스타일(이미 강조색+굵게로 구분됨, 변경 없음), 중첩 프래그먼트끼리의 깊이별 구분(모든
  프래그먼트가 동일한 굵은선 — `block-beta`처럼 깊이마다 다른 패턴으로 순환하지 않음, 프레임의
  중첩은 이미 들여쓰기로 보임), `graph.rs` 공유 레이아웃(서브그래프·합성 상태 등)의 그룹 테두리
  (별도 검토 대상, 색상으로 이미 구분됨)

## Acceptance Criteria

### 1. 프래그먼트 테두리 선패턴
- 1.1: [`alt`/`opt`/`loop`/`par`/`critical`/`break`/`group`/`rect` 프래그먼트] → 바깥 테두리가
  메시지·생명선과 다른 선패턴(굵은선)으로 그려짐
- 1.2: [`--style none`] → 프래그먼트 테두리 글자(`┏`/`┓`/`┗`/`┛`/`━`/`┃`)가 메시지·생명선 글자
  (`┌`/`─`/`│` 등)와 겹치지 않음
- 1.3: [`else`/`option`/`and` 구분선] → 기존과 동일하게 파선으로 그려짐(변경 없음)

### 2. 겹치는 요소와의 정합성
- 2.1: [메시지 화살표나 생명선이 프래그먼트 테두리와 만남] → 교차 지점이 패닉 없이 올바른
  교차/모서리 문자로 그려짐
- 2.2: [프래그먼트가 중첩됨(예: `loop` 안에 `alt`)] → 중첩 깊이와 무관하게 모든 프래그먼트
  테두리가 동일한 굵은선으로 그려짐

### 3. 회귀
- 3.1: [기존 `sequence.rs`·시퀀스 관련 테스트] → 전량 통과
- 3.2: [항상] → 어떤 입력에도 패닉하지 않음
- 3.3: [`examples/architecture.txt`] → 바뀐 렌더링 결과로 재생성됨(원문 `examples/architecture.md`는
  변경 없음)
