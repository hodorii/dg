# ASCII BPMN 표기법 조사 — 기존 사례

조사 일시: 2026-09-28
조사 동기: `.kiro/steering/ascii-bpmn.md` 설계 시 기존 유사 사례 확인

## 기존 사례

| 프로젝트 | 방향 | 이벤트 기호 | 게이트웨이 기호 | 특징 |
|---|---|---|---|---|
| **@bpmnkit/ascii** | XML → ASCII (렌더링) | `○` start, `●` end, `◎` intermediate, `◉` throw, `◈` boundary | `×` XOR, `+` AND, `◇` OR, `?` event-based, `✱` complex | 액티비티에 3글자 유형 태그 (`[svc]`, `[usr]`). **핵심 동기: LLM 프롬프트 비용 절감** — "XML보다 수백 토큰, 스크린샷보다 읽기 쉬움" |
| **@bpmn-sdk/ascii** | XML → ASCII (렌더링) | 동일 체계 | 동일 체계 | Sugiyama 레이아웃 엔진 사용. 터미널·CI·스냅샷용 |
| **bpmn-txt** (cwygoda) | 텍스트 DSL → BPMN XML | DSL 문법 별도 | XOR/AND/OR 지원 | YAML-like 문법. VS Code 확장 (문법 강조, 자동완성, 라이브 미리보기) |
| **bpl (BPMN-Lite)** | 텍스트 → Mermaid/BPMN XML | `!` start/end | `?` XOR, `+` positive, `-` negative, `~` OR | `:` process, `@` lane, `#` data object, `$` data store, `^` message flow. 접사 기반 문법 |
| **BPMN Sketch Miner** | 텍스트 DSL → BPMN (마이닝) | 프로세스 마이닝으로 추론 | XOR/event-based 등 자동 추론 | 자연어에 가까운 DSL. 레이아웃 알고리즘 포함 |
| **IETF/Internet-Draft** | ASCII-art ↔ SVG 병행 | — | — | RFC 표준 관례. asciiflow·Textik·Monodraw 같은 도구체계 존재 |
| **PlantBPMN** | 텍스트 DSL → BPMN XML | Xtext 기반 | Eclipse 기반 편집기 | BPMN XML 스키마로 모델 변환 |

## 공통 패턴

1. **이벤트 원 안에 기호** — 모두 이벤트를 원 안에 기호로 구분 (start=○/thin, end=●/thick, intermediate=◎, throw=◉, boundary=◈). 우리 표기법의 괄호 겹침 + 시그일과 같은 발상.
2. **게이트웨이 전용 기호 집합** — XOR, AND, OR, event-based, complex에 각기 다른 문자/모양.
3. **텍스트 ↔ 시각 모델 변환** — 방향이 렌더링이든 역생성이든, 텍스트를 1차 표기로 삼는 것은 동일.
4. **동기 일치** — 터미널, CI 로그, PR 코멘트, LLM 프롬프트 등 이미지 없는 환경.

## 우리 설계와의 차이

| 차이점 | 기존 사례 | 우리 표기법 |
|---|---|---|
| **ASCII의 위치** | 대부분 XML → ASCII 렌더링 산출물 (ASCII가 목적) | ASCII가 소스 표기 자체 (사람이 직접 작성) |
| **문자 집합** | Unicode 박스 문자 (`┌─┐`, `→`) | 7-bit ASCII만 |
| **문법 스타일** | 별도 DSL (bpmn-txt, bpl) 또는 좌표 기반 렌더링 | 위치 문법 — 괄호·대괄호·꺾쇠의 용도 규칙 |
| **이벤트 구분** | 유니코드 기호 (○●◎◉◈) | 괄호 겹침 + 시그일 (`((T:2w))`) |
| **루프** | 대부분 미명시 | 3형태 명시 (액티비티 루프, 백엣지, 링크 이벤트) |

## 시사점

- **@bpmnkit/ascii의 LLM 동기**가 우리 설계 의도와 정확히 일치 — 이 표기법의 존재 근거가 이미 검증됨.
- 기존 사례가 모두 "렌더링 산출물"인 반면, 우리 표기법은 **사람이 직접 작성하는 1차 표기**라는 점에서 차별화됨. bpmn-txt·bpl이 텍스트 소스이지만 별도 DSL이고, 우리는 순수 ASCII 문자만으로 위치 문법을 구성.
- IETF의 ASCII-art ↔ SVG 병행 관례는 우리 프로젝트의 dg(ASCII 렌더러)와 같은 맥락 — 텍스트 표기의 실용적 가치가 표준 기구에서도 인정됨.

## 참고 링크

- @bpmnkit/ascii: https://bpmnkit.com/docs/packages/ascii
- @bpmn-sdk/ascii: https://www.npmjs.com/package/@bpmn-sdk/ascii
- bpmn-txt: https://github.com/cwygoda/bpmn-txt
- bpl (BPMN-Lite): https://github.com/oisee/bpl
- BPMN Sketch Miner: https://link.springer.com/content/pdf/10.1007/s10270-022-01009-w
- IETF ASCII-art 관례: https://authors.ietf.org/diagrams
