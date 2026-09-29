# Research — diagram-diamond-side-glyph-coverage

## Summary
- **Feature**: `diagram-diamond-side-glyph-coverage`
- **Discovery Scope**: Simple Addition (글자 두 개 교체, 배치·크기 불변)
- **Key Findings**:
  - 코드포인트 커버리지(`fc-list`)만 보면 후보 여섯 쌍이 전부 통과하지만, **동아시아 폭**을
    함께 보면 두 쌍(`〈〉`, `＜＞`)이 2칸 글자라 격자를 깨고, 두 쌍(`<>`, `◁▷`)은 이미
    간선 표식 글자라 LR 배치에서 옆면과 표식이 같은 글자로 붙는다.
  - 남는 것은 `‹`(U+2039)·`›`(U+203A) 한 쌍 — 커버·1칸(`width`·`width_cjk` 모두 1)·
    표식 미사용·원래 `⟨⟩`와 가장 닮은 꺾쇠.
  - 이 프로젝트는 A(모호) 폭 글자(`│─╱╲○◎×▲▼`)를 이미 도형·표식 전반에 쓰므로 A 등급
    자체는 탈락 사유가 아니다 — 탈락은 W/F(항상 2칸)와 표식 충돌로만 판정한다.

## Research Log

### 후보 글자 실측(2026-09-29, 이 머신)
- **Context**: `bugfix.md` 재현 절차 2의 `⟨⟩` 미커버 사실을 대체할 글자 선정.
- **Sources Consulted**: `fc-list "<family>:charset=<hex>"`(폰트 커버리지), Python 3
  `unicodedata.east_asian_width`(Unicode 16.0.0), `unicode-width` 0.2.2(`Cargo.lock`의
  실제 버전, `UnicodeWidthChar::width`/`width_cjk`)를 스크래치 크레이트로 실행,
  `src/diagram/layout/graph.rs` `marker_glyphs()`(표식 글자 목록).
- **Findings**(폰트 열: 매칭 파일 수, 0 = 미커버):

  | 글자 | 코드 | Noto Mono CJK KR | Noto Mono CJK JP | D2Coding | DejaVu Sans Mono | EAW | `width` / `width_cjk` | 표식 사용 | 판정 |
  |---|---|---|---|---|---|---|---|---|---|
  | `⟨` `⟩` (현재) | U+27E8/27E9 | 0 | 0 | 2 | 4 | Na | 1 / 1 | 없음 | 결함 원인(미커버) |
  | `〈` `〉` | U+3008/3009 | 2 | 2 | 2 | 0 | **W** | **2 / 2** | 없음 | 탈락 — 2칸 |
  | `＜` `＞` | U+FF1C/FF1E | 2 | 2 | 2 | 0 | **F** | **2 / 2** | 없음 | 탈락 — 2칸 |
  | `<` `>` | U+003C/003E | 2 | 2 | 2 | 4 | Na | 1 / 1 | `OpenArrow` LR, 까치발 many LR | 탈락 — 표식 충돌 |
  | `◁` `▷` | U+25C1/25B7 | 2 | 2 | 2 | 4 | A | 1 / **2** | `Triangle` LR | 탈락 — 표식 충돌(+A) |
  | `‹` `›` | U+2039/203A | 2 | 2 | 2 | 4 | N | 1 / 1 | 없음 | **채택** |

  참고로 같은 도형이 이미 쓰는 `│`(U+2502)·`╱`(U+2571)·`╲`(U+2572)와 게이트웨이·이벤트
  글자 `○◎×▲▼◀▶∧∨`는 전부 EAW A(`width` 1, `width_cjk` 2)다.
- **Implications**:
  - **W/F는 코드 수준에서 확정 탈락**: `measure()`가 옆면에 1칸씩만 배정(`tw+6`)하는데
    `Canvas::put()`은 `char_width(ch)`가 2면 `x+1`을 연속 칸으로 덧씌우고, `x+w > width`면
    아무것도 안 찍고 돌아간다 — 왼쪽 옆면은 본문 첫 공백 칸을 잡아먹고 오른쪽 옆면
    (`x+w-1`)은 **조용히 사라진다**. 터미널도 2칸으로 그리므로 폭 계산과 실제 렌더가 어긋난다.
  - **표식 충돌**: LR 배치에서 표식은 노드 옆면 바로 옆 칸에 놓인다(`│ start │──▶⟨  ok?  ⟩`).
    `<>`를 쓰면 `-->`가 아닌 `--o`/`--x`류 열린 화살표(`OpenArrow`, LR 글자 `>`)와 ER
    까치발이 옆면과 같은 글자로 붙고(`──>>  ok?  >`), `◁▷`는 `Triangle` 표식(LR 글자 `◁▷`)과
    같은 글자다. 표식 글자는 이 스펙 밖(`marker_glyphs()`)이라 옆면 쪽이 피해야 한다.
  - **A 등급 판정 기준**: 터미널이 모호 폭을 2칸으로 설정(`ambiwidth=double`류)하면 이
    프로젝트의 상자 테두리 전체가 이미 깨지므로, A는 "프로젝트가 감수 중인 위험"이고
    후보 개별 탈락 사유로는 삼지 않는다. 다만 `‹›`는 N이라 그 설정에서도 1칸이다 —
    현재 `⟨⟩`(Na)와 같은 등급으로, 폭 위험을 새로 들이지 않는다.
  - **미검증**: 폰트 파일 안 글리프의 실제 advance 폭(half-width인지)은 `hb-shape`·
    fontTools·`wcwidth`가 이 머신에 없어 재지 못했다. 커버리지(`fc-list`)와 폭 표(EAW·
    `unicode-width`)까지가 이 세션의 증거 범위다.

### 새 글자 적용 모습(코드 수정 없이 출력 치환으로 미리 봄)
- **Context**: 채택 글자가 마름모 인상을 유지하는지, 골든 파일 변화가 여섯 칸뿐인지.
- **Findings**:
  ```
   ╱─────╲                 ┌───────┐    ╱─────╲    ┌─────┐
  ‹  ok?  ›                │ start │──▶‹  ok?  ›──▶│ end │
   ╲─────╱                 └───────┘    ╲─────╱    └─────┘
  ```
  `examples/architecture.txt` 138·151·170행: `‹  언어 판별 가능?  ›`, `‹  종류 판별됨?  ›`,
  `‹  폭 안에 들어감?  ›` — 각 행 두 칸씩, 합계 여섯 칸.
- **Implications**: 크기·모서리·본문 위치 불변(bugfix 3.3), 골든 diff 범위(bugfix 2.3).

### 골든 파일의 기존 어긋남
- **Context**: `make examples`가 `examples/architecture.txt`를 재생성할 때 이번 수정 외의
  diff가 섞이는지.
- **Findings**: 현재 HEAD에서 `dg -P -s none -w 140 examples/architecture.md`와
  `examples/architecture.txt`를 비교하면 이미 766행 이후에 "모듈 의존 그래프(서브그래프로
  묶은 버전)" 절(`eade414`에서 architecture.md에 추가, txt 미갱신)이 통째로 다르다.
- **Implications**: 재생성하면 그 절이 함께 들어온다 — 이번 스펙의 변화(여섯 칸)와 별개로
  구분해 검증해야 한다(design §영향 범위).

## Design Decisions

### Decision: 옆면 글자 `⟨⟩` → `‹›`(U+2039/U+203A)
- **Context**: bugfix 1.1 — 기본 CJK 모노 폰트 커버리지 밖 글자.
- **Alternatives Considered**: 위 표의 다섯 쌍. 그 밖에 "완전히 뾰족한 마름모(여러 줄
  빗금)"는 `c7785c8`이 폭·줄 수 비용으로 이미 기각했고, "육각형과 같은 `│`"는 `c7785c8`이
  고친 결함으로 되돌아간다(bugfix 3.1 위반).
- **Selected Approach**: `draw()`의 `('⟨', '⟩')` 한 쌍만 `('‹', '›')`로 바꾼다.
- **Rationale**: 커버(Noto KR/JP 2, D2Coding 2, DejaVu 4)·1칸(`width`·`width_cjk` 1, EAW N)·
  표식 미사용·꺾쇠 모양 유지. `diagram-crow-foot-orientation`의 규칙("커버리지 밖 글자 대신
  기본 폰트에 있는 글자")과 `bpmn-shapes`의 판단 기준(Noto Sans Mono CJK KR 기준)을 그대로
  따른다.
- **Trade-offs**: `‹›`가 `⟨⟩`보다 작아 마름모 인상이 조금 약해진다. 대신 육각형 `│`·간선
  표식과 헷갈리지 않고 폰트 폴백이 사라진다.
- **Follow-up**: 구현 시 커버리지 허용 목록 테스트에 `‹›`를 추가해 재발을 막는다(design
  §검증 속성 (b)).

## Risks & Mitigations
- `‹›`가 특정 터미널 폰트에서 지나치게 작게 보일 수 있다 — 라벨 양옆 공백 두 칸이 그대로
  남아 옆면 위치는 식별된다; 인상 문제는 로드맵 `bpmn-gateway-fixed-size`의 기하 재검토에서
  다시 볼 수 있다.
- 골든 재생성에 무관한 절이 섞인다 — 위 Research Log; 커밋 전 diff를 두 부분으로 나눠 확인.

## References
- `.kiro/specs/bpmn-shapes/research.md` §글자 커버리지 실측 — 2026-09-25 선행 실측, 판단 기준 출처
- `.kiro/specs/diagram-crow-foot-orientation/design.md` — 폰트 폴백 회피 규칙의 전례
- `src/diagram/layout/graph.rs` `marker_glyphs()` — 표식 글자 목록과 폴백 성능 실측 주석
