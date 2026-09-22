# Design — diagram-er-standalone-alias

## 정의
mermaid `erDiagram`의 단독 개체 선언 줄이 파서 디스패치에서 아예 무시되던 결함을 고친다.

## 원인 (Root Cause)
`src/diagram/mermaid/er.rs`의 `parse()` 메인 루프는 각 줄을 세 갈래로만 분기한다:
(1) `erDiagram`/`direction` 헤더, (2) `{`로 끝나는 속성 블록 오프너(`declare()` 호출),
(3) 그 외 전부 `parse_relation()`. 단독 선언 줄(`ID["라벨"]`, 뒤에 `{`도 관계 연산자도
없음)은 (2)에 안 걸려 (3)으로 떨어진다. `parse_relation()`은 `body.split_whitespace()`로
낱말을 나눈 뒤 `words[1]`에서 `--`/`..`를 찾는데, 라벨에 공백이 있으면 대괄호 안이 여러
낱말로 쪼개져 `words[1]`이 라벨 조각이 되고(1.2), 공백이 없어도 애초에 관계 연산자가 없어
`words.len() < 3`이거나 `--`/`..`를 못 찾아 바로 `return`한다(1.1) — 두 경우 다
`declare()`가 전혀 호출되지 않아 그 아이디는 나중에 관계선이 별칭 없이 참조할 때 비로소
`declare()`가 호출되며 라벨 없는 노드로 생성된다.

## 수정 방식
`parse()`가 매 줄을 `parse_relation()`으로 넘기기 전에, 그 줄이 실제 "관계선"인지 먼저
판정한다: 대괄호 `[...]` 안의 내용을 제외한 나머지에 `--`/`..`가 있으면 관계선(기존처럼
`parse_relation()`), 없으면 단독 선언 줄로 보고 `declare()`를 직접 호출한다. 대괄호를
제외하고 판정하는 이유는 라벨 문자열 안에 우연히 `-`가 반복돼 있어도(드묾) 관계선으로
오판하지 않기 위함이다.
- **대안 비교**: `words.len() < 3`일 때만 예외적으로 `declare()`로 폴백하는 방법도 검토했으나,
  1.2(라벨에 공백이 섞여 `words.len() >= 3`이 되면서도 관계선이 아닌 경우)를 못 잡아 기각.
  대괄호를 제외하고 관계 연산자 유무로 판정하는 쪽이 두 케이스를 한 규칙으로 다 잡는다.

## 검증 속성
- (a) 결함 재현: `parses_standalone_declaration_with_alias`류 테스트를 수정 전 코드에 대면
  실패함(1.1, 1.2 — 상자 제목이 아이디로 남음)을 확인.
- (b) 기대 동작: 같은 테스트가 수정 후 통과(2.1 — 단독 선언 별칭이 반영됨, 2.2 — 공백 포함
  라벨도 반영됨).
- (c) 불변 동작: 기존 `parses_entities_and_relations` 및 `diagram-alias-label`이 추가한
  회귀 테스트(관계선 인라인 별칭, 순서 의존 케이스)가 수정 후에도 그대로 통과(3.1~3.4).

## 영향 범위
- `src/diagram/mermaid/er.rs`: `parse()`(디스패치 로직에 관계선 판정 함수 추가), 신규 헬퍼
  함수 하나(`is_relation_line` 또는 동등), 테스트 추가.
- 다른 파일 없음 — `markdown-source-view`/`markdown-link-navigation` 등 다른 스펙의 Boundary
  Commitment와 무관(전혀 다른 모듈).
