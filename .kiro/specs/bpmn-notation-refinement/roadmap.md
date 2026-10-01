# Roadmap

## 상태: 폐기(2026-10-01, 사용자 결정)
`diagram-diamond-side-glyph-coverage`(아래 1번, 폰트 커버리지 버그픽스)는 이미 구현·커밋
완료된 상태로 **그대로 유지**한다 — 이건 "노테이션 재검토"가 아니라 독립된 버그픽스였다.
나머지 다섯 항목(`bpmn-gateway-fixed-size` 이하)은 사용자 지시로 전부 폐기한다. 구현은
진행하지 않는다. `bpmn-gateway-fixed-size`가 만든 실물 비교 자료(`render-comparison*.md`·
`decision-log.md` — bpmn.io 실측 포함)는 향후 재론의될 경우를 위해 기록으로만 남긴다.

## Overview
bpmn.io 스타일(크기 고정, 텍스트 배치, 기호 구분)을 참고해 BPMN 표기를 다듬는다. 여섯 중
하나(다이아몬드 옆면 글자 폰트 커버리지)는 원인·제약이 이미 `bpmn-shapes/research.md`에
실측돼 있는 채로 방치된 버그픽스라 바로 착수 가능하고, 나머지 다섯(게이트웨이 크기 고정,
이벤트 트리거 아이콘화, 태스크 아이콘 배치, 경계 이벤트 실제 부착, 태스크 최소 폭)은
문자 격자 제약 안에서 실제로 그려 비교해야 사용자가 채택 여부를 판단할 수 있는 설계
트랙이다.

## Approach Decision
- **Chosen**: 4개 스펙으로 분리, 버그픽스 하나를 먼저 독립 진행하고 나머지 셋은 각자
  requirements 단계에서 후보 렌더링을 실제로 보여준 뒤 사용자 승인을 받아 design으로
  넘어간다
- **Why**: 버그픽스는 이미 조사가 끝나 있어 더 미룰 이유가 없다. 나머지 셋은 "이 글자가
  이 문맥에서 알아보기 쉬운가"라는 판단이 핵심이라 문서 형태 스펙만으로는 결정할 수
  없다 — 실물 렌더링 비교가 requirements 산출물의 일부가 돼야 한다
- **Rejected alternatives**: (1) 넷을 한 스펙으로 — 버그픽스가 설계 결정 셋의 승인
  지연에 묶여 미뤄진다. (2) 전부 설계 트랙으로(버그픽스도 "재검토") — 이미 실측·기각
  사유가 확정된 걸 다시 여는 것은 낭비

## Scope
- **In**: `brief.md`가 정한 네 갈래 전부
- **Out**: `bpmn-support`의 게이트웨이 기호(`×+○*◎`) 자체 재작업(이미 완료), mermaid/
  PlantUML BPMN 네이티브 문법

## Constraints
- 의존성 4개·단일 정적 바이너리 유지
- 새 글자는 `fc-list`로 CJK 모노 폰트 커버리지 실측 후에만 채택
- 게이트웨이·다이아몬드 없는 기존 다이어그램 렌더링 결과 불변(바이트 동일 회귀 기준)

## Boundary Strategy
- **Why this split**: `diagram-diamond-side-glyph-coverage`는 `layout::shape` 그리기
  한 곳(글자 두 개 교체)만 건드리는 순수 버그픽스라 다른 셋과 독립적으로 끝난다.
  `bpmn-gateway-fixed-size`는 `Shape::is_point_anchored`(이벤트가 이미 쓰는 배치기
  훅)를 게이트웨이로 확장할지가 핵심이라 그 이벤트 구현을 그대로 참고할 수 있다.
  `bpmn-event-trigger-icons`·`bpmn-activity-icon-review`는 둘 다 "격자 문자로 아이콘을
  표현할 수 있는가"라는 같은 질문을 다른 도형에 묻는 것이라 순서 의존은 없지만, 이벤트
  트리거 쪽이 후보 글자(`▲`·`⊗`)가 이미 있어 먼저 결론 내기 쉽다
- **Shared seams to watch**: `bpmn/vocabulary.rs`(게이트웨이·트리거 라벨 표 SSoT),
  `layout::shape::draw()`의 `Shape::Diamond|Hexagon`·`Shape::Event` 분기, `layout::graph`
  의 `Shape::is_point_anchored` 및 그 소비 지점 4곳(이벤트 전용 → 게이트웨이까지
  넓히면 이 술어의 전제를 다시 확인해야 함 — `ir.rs`의 "상태도 Start/End는 이미 1×1"
  같은 예외 추론이 게이트웨이에도 똑같이 성립하는지 재검증 필요)

## Specs (dependency order)
- [x] diagram-diamond-side-glyph-coverage -- `Shape::Diamond`(`⟨⟩`, U+27E8/27E9)를
  CJK 모노 폰트 커버리지 안의 글자로 교체하는 버그픽스. `Shape::Hexagon`과 계속
  구분돼야 함(`c7785c8` 회귀 금지). `bpmn-shapes/research.md`의 실측·기각 대안이 이미
  있어 `$kiro-bugfix`로 바로 시작 가능.
  Dependencies: none
- [폐기] bpmn-gateway-fixed-size -- 게이트웨이 마름모를 라벨 길이에 안 맞춰 자라는 고정
  크기로, 이름을 도형 밖(이벤트처럼 오른쪽 또는 아래)에 두도록 재검토. `Shape::
  is_point_anchored`를 게이트웨이까지 넓힐지가 핵심 설계 질문. requirements 단계에서
  현재 방식과 후보 방식을 나란히 렌더링해 비교.
  Dependencies: diagram-diamond-side-glyph-coverage(같은 도형을 다루므로 글자 교체가
  먼저 끝나야 크기 정책 변경의 전후 비교가 깨끗함)
- [폐기] bpmn-event-trigger-icons -- 이벤트 트리거(message·timer·error 등 13종)를 `«이름»`
  텍스트에서 커버리지 통과 글자(`▲`·`⊗` 등)로 일부라도 아이콘화할지 결정. 13종 중
  커버리지를 통과하는 게 소수라 "부분 아이콘화(불일치한 시각 언어)" vs "전부 텍스트
  유지"를 사용자가 실물로 보고 골라야 함. requirements 단계에서 두 안 다 렌더링.
  Dependencies: none
- [폐기] bpmn-activity-icon-review -- 태스크/액티비티 종류(user·service·script 등)를
  상자 왼쪽 위 아이콘으로 옮길지 검토. 문자 격자에서 작은 아이콘과 테두리·본문이
  안 겹치는 배치가 마땅치 않아 "현행(둘째 줄 스테레오타입) 유지"로 결론 날 가능성이
  높음 — requirements 단계에서 후보 배치 1~2개를 실제로 그려보고 판단, 후보가 전부
  안 되면 이 스펙은 "현행 유지" 결론과 근거만 남기고 종료(코드 변경 없음).
  Dependencies: none
- [폐기] bpmn-boundary-event-attachment -- 경계 이벤트를 호스트와 같은 그룹의 별도 노드
  +무표식 점선(v1 근사)이 아니라, 호스트 도형의 테두리 위 실제 좌표에 이벤트 글자를
  겹쳐 그리는 방식으로 바꿀지 검토. `bpmn-support` 원 리서치가 "배치기에 노드-노드
  부착 개념이 없어 후순위"로 이미 미뤄둔 항목 — 지금 배치기가 자란 뒤에도(점 고정
  도형·`group_anchor`·`Element.parent`) 여전히 새 그리기 프리미티브(다른 노드의 렌더된
  테두리 좌표 위에 글자를 찍는 것)가 필요한지부터 설계 단계에서 확인해야 한다. 사용자
  확인(2026-09-29): "boundary event ... 개념은?"으로 제기.
  Dependencies: none(다만 `bpmn-gateway-fixed-size`가 점 고정 개념을 더 다듬으면 같은
  좌표 계산을 재사용할 여지가 있어 그 스펙 뒤가 자연스러움)
- [폐기] bpmn-task-min-width -- `Shape::Round`(태스크/액티비티가 쓰는 도형)가 라벨 길이에만
  맞춰 자라 "OK" 같은 짧은 라벨은 6×3, 긴 라벨은 28×4로 극단적으로 다른 비율이 되는
  문제를 검토(직접 렌더링해 확인함). bpmn.io는 태스크 상자 크기가 훨씬 균일하다. 단
  `Shape::Round`는 BPMN 전용이 아니라 모든 mermaid/PlantUML 흐름도가 공유하므로, 최소
  폭을 넣으면 BPMN 아닌 기존 다이어그램도 전부 영향을 받아 바이트 동일 회귀가 깨진다 —
  범위를 BPMN 태스크만으로 좁히려면 새 `Shape` 변형이 필요할 수 있다는 게 핵심 설계
  질문. 사용자 확인(2026-09-29): "round rect 비율/사이즈 고정 개념은?"으로 제기.
  Dependencies: none

<!-- 이후(이 로드맵 밖):
게이트웨이·이벤트·태스크 다섯 갈래가 전부 결론 나면, "점 고정 도형" 개념이 여럿으로
늘어날 수 있으니 그때 `Shape::is_point_anchored`의 이름·문서 주석을 한 번에 정리할
가치가 있는지 재검토(지금은 미정, 별도 스펙 후보로만 기록).
-->
