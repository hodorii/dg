# ASCII BPMN 2.0.2 표기법

BPMN 2.0.2 프로세스 다이어그램 부분집합을 7-bit ASCII로 옮긴 표기법. mermaid(이미지·박스 의존)와 보완 관계 — diff 용인, 인라인 삽입, dg 없는 환경에서 사용.

## 설계 원칙

1. **모양 운율** — BPMN 도형의 시각 특징을 문자 특징으로 번역: 원=괄호, 사각=대괄호, 마름모=`<>`, 두꺼운 테두리=괄호 겹침, 점선=점/쉘시.
2. **7-bit ASCII 전용** — dg 렌더러·git diff·PR 리뷰·LLM 컨텍스트 어디서든 동일. 유니코드 박스문자·mermaid 없이도 흐름이 복구된다.
3. **위치 문법** — 같은 문자도 위치(컨테이너/연결부)로 역할이 결정되어 파서 가능. 모호성은 직교 규칙으로 제거.

## 1. 이벤트 — 괄호 = 원

| BPMN 2.0.2 | ASCII | 근거 |
|---|---|---|
| Start Event | `(name)` | 얇은 원 1겹 → 괄호 1쌍 |
| End Event | `((name))`, 관례 `((end))` | 굵은 테두리 → 괄호 2겹 |
| Intermediate | `((T:2w))`, `((@:approval))` | 2겹 + 시그일(`X:`) 필수 — 시그일 없는 2겹은 종료로 해석 |
| Throw 표시 | `((@!:sent))` | 시그일 뒤 `!` = 채워진 아이콘(발사) |
| Boundary (방해) | `[review]^((T:2w))` | `^` = 작업 테두리에 붙박이 |
| Boundary (비방해) | `[review]~((T:5m))` | `~` = 점선 원(중단 안 함) |

이벤트 시그일(BPMN 아이콘 → ASCII):

| 시그일 | 종류 | 예 |
|---|---|---|
| `@:` | Message | `(@:issue-opened)` |
| `T:` | Timer | `(T:9am)` |
| `!:` | Error | `((!:deploy-fail))` |
| `^:` | Escalation | `((^:manager))` |
| `S:` | Signal | `((S:abort))` |
| `C:` | Conditional | `((C:form-received))` |
| `L>` / `L<` | Link (throw/catch) | `((L>a))` / `((L<a))` |
| `<:` | Compensation | `((<:undo-deploy))` |

## 2. 액티비티 — 대괄호 = 사각형

| BPMN 2.0.2 | ASCII | 근거 |
|---|---|---|
| Task | `[name]` | 둥근 사각 → 대괄호 |
| Task 유형(선택) | `[user:approve]` | 접두 — user/service/script/manual/send/receive |
| Subprocess (접힘) | `[[name]]` | 상자 안 상자 |
| Subprocess (펼침) | 이름 행 아래 들여쓰기 `->` | 펼침 내용은 하위 흐름 |
| Call Activity | `[>name]` | 호출 화살표 |
| Loop | `[name*]` | `*` = 반복 회전 화살표 |
| MI 병렬 | `[name=]` | 수평 막대 3개 → `=` |
| MI 순차 | `[name||]` | 수직 막대 3개 → `||` |
| Ad-hoc | `[[name~]]` | BPMN 틸데 마커 그대로 |
| Transaction | `[[[name]]]` | 이중 테두리 → 3겹 |

## 3. 게이트웨이 — `<>` = 마름모

| BPMN 2.0.2 | ASCII | 근거 |
|---|---|---|
| Exclusive (XOR) | `<x>`, `<name>` | 미지정 시 x; X 마커 생략 가능 |
| Parallel (AND) | `<+>`, `<+ name>` | + 마커 |
| Inclusive (OR) | `<o name>` | 원 마커 |
| Event-based | `<?>` | "어느 이벤트가 올까" |
| Complex | `<*>` | 별 마커 |

## 4. 흐름

| BPMN 2.0.2 | ASCII | 근거 |
|---|---|---|
| Sequence Flow | `->` | 실선 |
| Named Flow | `-:label->` | 라벨 |
| Conditional Flow | `-<cond>->` | 미니 마름모(다이아몬드 마커) |
| Default Flow | `-/->` | 슬래시 마커 |
| Message Flow | `-->` | 점선 (풀 간) |
| Association | `.->` | 점선 (데이터·주석) |

`<x>` 게이트웨이에서는 조건 라벨이 흐름 이름이 됨: `-:pass->`, `\-/->`.

## 5. 데이터·주석·그룹

| BPMN 2.0.2 | ASCII |
|---|---|
| Data Object | `{name}` |
| Data Store (DB) | `{=name=}` — 원기둥 평행선 |
| Annotation | `<<note>>` (UML 스테레오타입 관례), `.->`로 연결 |
| Group | 점선 상자 `.--- ... ---'` |
| 작성자 주석 | `# ...` (BPMN 요소 아님, 해석 무시) |

**직교 규칙** (모호성 제거): `{x}`는 데이터, `-:label->`는 흐름 라벨, `-<cond>->`는 조건 — 대괄호 `[ ]`는 오직 액티비티. 꺾쇠 `< >`는 게이트웨이 또는 조건 위치.

## 6. 레인·풀·세로 목록 규칙

- `이름 |` 접두 = 레인(같은 풀 내 시퀀스 흐름은 레인 경계를 자유로이 넘음)
- `+-- name --+` 상자 = 풀(협업)
- 세로 목록에서 행이 `->`로 시작하면 직전 주축 노드에서 이어짐
- 게이트웨이 행 뒤 들여쓰기 `-+->` / `\+->` = 분기(게이트웨이 마커 반복)
- 분기 뒤 첫 게이트웨이 행 = 합류점. **첫 분기 = 주축** 계속, 이후 분기 = 곁가지
- `\^->` / `\~->` = 경계 이벤트 발생 흐름(주축과 별개)
- `.->` = 데이터·주석 연관(주축 무관)

## 7. 루프

### 7.1 액티비티 자체 루프 (Loop Marker)

BPMN 표준 루프 마커 — 액티비티 자체가 반복된다:

```
[send-reminder*]          # 조건 만족까지 반복
[retry-payment*]
```

### 7.2 흐름 되돌리기 (Back-edge)

이전 노드로 시퀀스 흐름을 되돌린다. 표기법에서는 **이전 노드를 다시 지목**하는 것으로 표현:

```
       |   -> <x qa>
       |      -:pass-> [[release&deploy]]
       |      \-/-> [impl]        # [impl]로 되돌아감 = 재작업 루프
```

BPMN에서 루프는 "왼쪽으로 화살표"가 아니라 **같은 방향으로 이전 노드를 다시 지목**하는 것. 텍스트라서 기하학적 방향 없이 출발·도착 노드로 의미가 결정된다.

### 7.3 장거리 루프/점프 (Link Event)

루프가 멀리 있거나 여러 페이지에 걸치면 `L>` (throw) / `L<` (catch)로 연결:

```
       |   -> [tasks] -> ((L>rework))   # 흐름 여기서 끊고
...
((L<rework)) -> [impl]                    # 이 지점에서 재시작
```

### 7.4 직교 규칙

- 루프백 흐름이 다른 분기와 겹치면 `\` 분기 행에 명시적 대상 노드를 쓴다: `\-/-> [impl]`
- `[name*]` = 반복 자체가 액티비티 의미(시작마다 실행), `\-/-> [x]` = 프로세스 제어 흐름(게이트웨이 분기에서 기본 경로)
- BPMN 순수주의: 병렬 분기(`<+>`)는 반드시 게이트웨이에서 시작하고 게이트웨이에서 합류. 암묵적 분기·합류 없음.

## 8. 예시

### 8.1 원라인 — 기본 흐름

```
(start) -> [discovery] -> [spec-init] -> [requirements] -> [biz-process] -> [data-model] -> [ui-model] -> [design] -> [tasks] -> [impl] -> [qa] -> [[release&deploy]] -> ((end))
```

`[requirements.md]` 같은 산출물명 작업은 데이터 연관으로 분리 가능: `[requirements] .-> {requirements.md}`.

### 8.2 풀 버전 — 레인·게이트웨이·경계 타이머·병렬·서브프로세스 전개

```
user   | (start)
       |   -> [approve-idea]
agent  |   -> [discovery] .-> {research.md}
       |   -> [spec-init]
       |   -> [requirements] .-> {requirements.md}
       |   -> [biz-process] .-> {biz-process.md}
       |   -> <+>
       |      -+-> [data-model] .-> {data-model.md}
       |      \+-> [ui-model]   .-> {ui-model.md}
       |   -> <+>                        # 병렬 합류
       |   -> [design] .-> {design.md}
       |   -> [tasks] .-> {tasks.md}
user   |   -> [approve-tasks] ~((T:3d))  # 3일 경계 타이머(비방해)
       |      \~-> ((@!:remind-user))   # 시간 초과 시 리마인더 발송
agent  |   -> [impl]
       |   -> [qa]
       |   -> <x qa>
       |      -:pass-> [[release&deploy]]
       |      \-/-> [impl]               # 기본 흐름: 재작업 루프
user   |   -> [confirm-release]
       |   -> ((end))
```

서브프로세스 전개:

```
[[release&deploy]]
  -> [build] -> [tag]
  -> [deploy] .-> {release-note.md}
```

### 8.3 협업 — 풀 간 메시지 흐름

```
+-- agent ---------------------------------+
| [requirements] .-> {requirements.md}     |
|   -> ((@!:review-request))               |
+------------------------------------------+
              |
              v  @:review-request
+-- user ----------------------------------+
| ((@:review-request))                     |
|   -> [review]                            |
|   -> ((@!:approved))                     |
+------------------------------------------+
              ^
              |  @:approved
+-- agent ---------------------------------+
| ((@:approved)) -> [[release&deploy]]     |
+------------------------------------------+
```

## 9. 문법 스케치 (EBNF)

```ebnf
flow       := row (NL row)*
row        := [lane] [branch] node [tail]
lane       := name "|"
branch     := ("-" | "\\" | "\\^" | "\\~") [gwmark] "->"
node       := event | activity | gateway | data | annotation
event      := "(" inner ")" | "((" inner "))"
inner      := [sigil ":"] text
activity   := "[" name [marker] "]" | "[[" name [marker] "]]" | "[>" name "]"
gateway    := "<" [gwmark] [name] ">"
data       := "{" name "}" | "{=" name "=}"
annotation := "<<" text ">>"
tail       := (".->" data) | ("\^->" node) | ("\~->" node)
sigil      := "@" | "T" | "!" | "^" | "S" | "C" | "L>" | "L<" | "<:"
gwmark     := "+" | "o" | "x" | "?" | "*"
marker     := "*" | "=" | "||" | "~"
```

## 범위

BPMN 2.0.2 **기술(descriptive) 부분집합** 커버: 이벤트 3종 + 9개 정의, 액티비티 5형 + 마커, 게이트웨이 5종, 흐름 6종, 데이터 2종, 어노테이션/그룹, 풀/레인. 코레오그래피·컨버세이션·보상 핸들러 세부·이벤트 서브프로세스는 제외(확장 슬롯). `((<:...))` 보상 시그일과 이벤트 서브프로세스(`[[^:name]]`)는 필요 시 같은 운율로 확장 가능하다.

## 관련 자료

- 조사: ASCII BPMN 표기법의 기존 사례 — `{{REFERENCE}}/ascii-bpmn-survey.md`
