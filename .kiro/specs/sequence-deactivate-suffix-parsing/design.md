# Design — sequence-deactivate-suffix-parsing

## 정의
mermaid 시퀀스 화살표에 `-`(deactivate) 접미사가 붙은 메시지가 비활성화·화살촉을
잃어버리는 결함을 고친다.

## 원인 (Root Cause)
`src/diagram/mermaid/sequence.rs`의 `parse_message()`는 화살표 토큰을 찾을 때
`'-' | '>' | 'x' | ')' | '<'` 중 하나인 문자를 만나는 대로 탐욕적으로 계속 먹는다
(129~136행). `+`(activate)는 이 집합에 없어서 화살표 뒤에서 자연히 멈추지만, `-`는
화살표 몸통(`--`)에도 쓰이는 글자라 deactivate 접미사의 `-`까지 화살표 토큰에
그대로 먹혀 버린다 — 예를 들어 `->>-B`는 `arrow="->>-"`, `remainder="B: text"`가
되어 `target_text.strip_prefix('-')`가 볼 접두사 자체가 사라진다(1.1). 동시에
`arrow.ends_with(">>")`가 거짓이 되어(`"->>-"`는 `-`로 끝남) `head` 판별 분기가
전부 빗나가 `Marker::None`이 된다(1.2). `+`는 이 문자 집합에 없어 애초에 이 문제가
생기지 않는다(3.1).

## 수정 방식
탐욕적으로 소비한 뒤, 마지막 글자가 `-`면 한 글자 되돌린다 — mermaid의 화살표
문법은 `>`/`x`/`)`로 끝나지, 맨 끝이 `-`인 화살표는 없다(몸통의 `-`는 항상 뒤에
글자가 더 온다)는 전제를 그대로 코드로 옮긴 것뿐이다.
```rust
if arrow_end > 0 && after.as_bytes()[arrow_end - 1] == b'-' {
    arrow_end -= 1;
}
```
되돌려진 글자는 `remainder`의 맨 앞 `-`가 되어 기존 `target_text.strip_prefix('-')`
로직이 그대로 잡아낸다 — 그 아래 코드는 손대지 않는다.
- **대안 비교**: 화살표 문자 집합에서 `-`를 아예 빼고 별도로 `--`/`-` 반복 개수를
  세는 방법도 검토했으나, 화살표 몸통 길이를 세는 기존 로직(`arrow.starts_with("--")`
  등)까지 다시 짜야 해서 범위가 커진다. 마지막 글자 하나만 되돌리는 쪽이 기존 판별
  로직(`starts_with`/`ends_with`)을 그대로 재사용할 수 있어 최소 변경이다.

## 검증 속성
- (a) 결함 재현: `parse()`로 `->>-`/`-->>-`/`-x-`/`-)-`/`--)-` 각 화살표 뒤에
  deactivate 접미사를 붙인 메시지를 파싱해 `deactivate_source`·`head`를 확인하는
  테스트를 추가하면, 수정 전 코드에서는 **실패**함(1.1, 1.2 — `false`/`None`)을 확인.
- (b) 기대 동작: 같은 테스트가 수정 후 통과(2.1, 2.2 — `true`/접미사 없을 때와 같은
  `head`).
- (c) 불변 동작: `+` 접미사·접미사 없음·독립 `activate`/`deactivate` 구문을 다루는
  기존 테스트(`diagram::mermaid::sequence::tests::*`)가 수정 후에도 그대로
  통과(3.1~3.3).

## 영향 범위
- `src/diagram/mermaid/sequence.rs`: `parse_message()`의 화살표 토큰 경계 계산
  두 줄, 테스트 추가.
- 다른 파일 없음 — PlantUML 파서(`diagram::plantuml::sequence`)는 별도 코드라
  영향 없음(bugfix.md Out of scope).
