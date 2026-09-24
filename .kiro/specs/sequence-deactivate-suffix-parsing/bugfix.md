# Bugfix — sequence-deactivate-suffix-parsing

## 정의
mermaid 시퀀스 다이어그램을 작성하는 사용자를 위해, 화살표에 `-`(deactivate) 접미사를
붙인 메시지(`B->>-A: text`, `B-->>-A: text` 등)가 실제로 비활성화·화살촉을 잃어버리지
않도록 `diagram::mermaid::sequence::parse_message`의 파싱 결함을 고친다.

## 재현 절차
1. 환경: `cargo test --lib`(단위 테스트로 파서 출력 직접 확인), 또는
   `dg -P -s none -d -l mermaid <파일>`로 실제 렌더링 확인.
2. 입력 A(가장 흔한 형태, dashed 리턴 + deactivate):
   ```
   sequenceDiagram
       A->>+B: open
       B-->>-A: close
   ```
3. 관찰(`crate::diagram::mermaid::sequence::parse`의 결과를 직접 출력):
   ```
   0: Message { from: 0, to: 1, label: "open", kind: Solid, head: Arrow, activate_target: true, deactivate_source: false }
   1: Message { from: 1, to: 0, label: "close", kind: Dashed, head: None, activate_target: false, deactivate_source: false }
   ```
   `+`(activate) 접미사가 붙은 메시지(0번)는 `activate_target: true`로 정상 파싱된다.
   `-`(deactivate) 접미사가 붙은 메시지(1번)는 `deactivate_source: false`로
   **인식되지 않고, 화살촉(`head`)까지 `None`으로 사라진다**(원래는 `Arrow`여야 함).
4. 입력 B(solid 화살표에서도 동일하게 재현): `B->>-A: text` → `deactivate_source: false`,
   `head: None`.
5. 입력 C(비동기 화살표에서도 동일하게 재현): `B--)-A: text` → `deactivate_source: false`,
   `head: None`.
6. 대조(문제 없는 경우): `-` 대신 `+`(activate)를 붙이면(`B->>+A: text`) 어떤 화살표
   종류든 정상적으로 `activate_target: true`와 올바른 `head`가 나온다.

## Boundary Context
- **In scope**: mermaid 시퀀스 파서(`diagram::mermaid::sequence::parse_message`)의
  화살표 뒤 `-`(deactivate) 접미사 인식
- **Out of scope**: PlantUML 시퀀스 파서(`diagram::plantuml::sequence`) — PlantUML은
  `++`/`--`라는 다른 표기를 쓰고 별도 파서라 이번 재현·수정 대상이 아니다(재확인
  필요하면 별도 이슈). mermaid의 `activate`/`deactivate` 독립 구문(접미사가 아닌
  별도 줄)은 이미 정상 동작하며 이 결함과 무관.

## Behaviors

### 1. 현재 동작 (결함)
- 1.1: [화살표 뒤에 `-`(deactivate) 접미사가 붙은 메시지를 파싱할 때, 화살표 종류가
  무엇이든(`->>`, `-->>`, `-)`, `--)` 등)] → [그 메시지의 `deactivate_source`가
  `false`로 남아 비활성화가 인식되지 않는다]
- 1.2: [같은 조건에서] → [그 메시지의 화살촉(`head`)도 `None`이 되어 원래 화살표
  종류가 가리키던 마커(`Arrow` 등)가 사라진다]

### 2. 기대 동작
- 2.1: [화살표 뒤에 `-`(deactivate) 접미사가 붙은 메시지를 파싱할 때, 화살표 종류가
  무엇이든] → [그 메시지의 `deactivate_source`가 `true`로 인식된다]
- 2.2: [같은 조건에서] → [화살촉(`head`)이 접미사 없이 같은 화살표를 썼을 때와 같은
  값으로 정상 인식된다]

### 3. 불변 동작 (회귀 방지)
- 3.1: [`+`(activate) 접미사가 붙은 메시지] → [지금처럼 `activate_target: true`와
  올바른 `head`로 정상 파싱된다]
- 3.2: [접미사가 없는 메시지] → [지금처럼 `activate_target`·`deactivate_source`
  둘 다 `false`, `head`는 화살표 종류에 맞게 정상 파싱된다]
- 3.3: [독립적인 `activate X`/`deactivate X` 구문(접미사가 아닌 별도 줄)] → [지금처럼
  정상 동작한다]
