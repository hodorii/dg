# IETF ASCII-art 다이어그램 관례 조사

조사 일시: 2026-09-28
조사 동기: ASCII BPMN 표기법 설계 시 IETF의 ASCII-art 관례를 벤치마킹

## 1. 개요

IETF(Internet Engineering Task Force)는 RFC(Request for Comments) 문서에서 다이어그램을 **ASCII-art**로 작성하는 공식 관례를 가지고 있다. 2013년 RFC 6949에서 공식화되어 현재까지 모든 RFC에 적용 중. 핵심 철학: **이미지 없이도 프로토콜 명세가 완전해야 한다** — 접근성, 단색 인쇄, 터미널 환경, 텍스트 검색을 우선한다.

## 2. 공식 규칙

### 2.1 법적 근거

| RFC | 내용 |
|---|---|
| **RFC 6949** (2013) | ASCII art와 SVG 병행 허용 공식화. "Graphics may include ASCII art and a more complex form to be defined, such as SVG line art" |
| **RFC 7996** (2016) | SVG 1.2 RFC 프로파일 정의. SVG Tiny 1.2 기반, 멀티미디어·인터랙티비티·스크립팅·애니메이션·메타데이터·확장성 제외 |
| **RFC 7994** | Plain-text RFC 요구사항. ASCII-art는 `type='ascii-art'`로 태깅, plain-text 출력에만 포함 |

### 2.2 작성 규칙

| 규칙 | 세부사항 |
|---|---|
| **폭 제한** | 72자 이내 (plaintext 렌더링에서 잘리지 않도록) |
| **폰트** | 고정폭(fixed-width) 필수. 비가변 폰트 금지 |
| **색상** | 단색(흑백)만. 컬러·그레이스케일 불가 |
| **접근성** | 시각 장애인 지원, 단색 디스플레이·인쇄 대응 |
| **병행 권장** | SVG + ASCII-art 함께 제출. SVG는 HTML/PDF, ASCII-art는 plaintext에 사용 |
| **단독 가능** | ASCII-art만 제출 가능 (RFC 9139가 예시). 이 경우 모든 렌더링에 ASCII-art 사용 |

### 2.3 XML 구조 (RFC 7991 기준)

```xml
<figure>
  <name>Box diagram</name>
  <artset>
    <artwork type="svg" name="box.svg">
      <svg xmlns="http://www.w3.org/2000/svg" version="1.1" viewBox="0 0 71 40">
        <g>
          <title>Layer 1</title>
          <rect x="4.5" y="6.5" width="61.0" height="27.0"
                stroke="black" stroke-width="1.0" fill="none" />
          <text x="33.883" text-anchor="middle" y="26.559">
            <tspan fill="black" font-size="13.0">A box</tspan>
          </text>
        </g>
      </svg>
    </artwork>
    <artwork type="ascii-art" name="box.txt">
      <![CDATA[
 +--------+
 | A box  |
 +--------+
      ]]>
    </artwork>
  </artset>
</figure>
```

`<artset>`이 SVG와 ASCII-art 두 개의 `<artwork>`을 포함. `type` 속성으로 구분.

## 3. 도구 생태계

| 도구 | 유형 | 용도 |
|---|---|---|
| **asciiflow** | 웹 편집기 | 인터랙티브 ASCII 다이어그램 편집. ASCII/유니코드 박스 문자 지원. 텍스트만 내보내기 |
| **aasvg** | CLI | ASCII-art → SVG 변환. IETF 공식 권장 도구. `xml2rfc`와 통합 |
| **mscgen** | CLI | 시퀀스 다이어그램 전용 DSL. ASCII/이미지 출력 |
| **PlantUML** | CLI | 다이어그램 범용. `-tutxt` 옵션으로 ASCII 출력 |
| **rfc-draw** | CLI | SVG + ASCII-art 동시 생성. RFC 7996 준수 검사 포함 |
| **Textik** | 웹 편집기 | 순수 ASCII만 지원. 오픈소스 |
| **Monodraw** | 앱 | macOS 전유료. SVG 내보내기 가능 |

### aasvg 사용법

```bash
# kramdown-rfc에서 블록 시작
~~~ aasvg
 +--------+
 | A box  |
 +--------+
~~~

# 결과: ASCII-art와 거의 동일한 SVG 생성
```

## 4. 실제 RFC 예시

### 4.1 RFC 9139 — 순수 ASCII-art만 사용

ICN LoWPAN 명세. SVG 없이 ASCII-art만으로 모든 다이어그램 표현 (Figure 2):

```
,------------------,
|                  |
|   ICN LoWPAN     |
|  Convergence     |
|     Layer        |
|                  |
+------------------+
```

### 4.2 RFC 9115 — 시퀀스 다이어그램 (ASCII-art)

ACME STAR 위임 프로토콜. 참여자 간 메시지 흐름을 ASCII-art로 표현 (Figure 1 일부):

```
       |                    |       |   Signature        |
       |                    |       o------------------->|
       |                    |       |                    |
       |                    |       |<~~~~Validation~~~~>|
       |                    |       |                    |
       |                    |       |   CSR              |
       |                    |       |   Signature        |
       |                    |       o------------------->|
       |                    |       |   Acknowledgement  |
       |                    |       |<-------------------o
       |                    |       |                    |
       |<~~Await issuance~->|       |<~~Await issuance~~>|
       |                                                 |
       |     (unauthenticated) GET STAR certificate      |
       o------------------------------------------------>|
       |                 Certificate #1                  |
       |<------------------------------------------------o
       |     (unauthenticated) GET STAR certificate      |
       o------------------------------------------------>|
       |                 Certificate #2                  |
       |<------------------------------------------------o
       |                     [...]                       |
```

### 4.3 RFC 9115 — 컴포넌트 다이어그램 (ASCII-art)

CDNI 2단계 위임 토폴로지 (Figure 12 일부):

```
.--------------------.
|      .------.------. |
|      | STAR | ACME |<-------------.
|  CP  | dele | STAR | |             |
|      | srv  | cli  +-----.         |
|      '---+--'------' |    |        6
 '---------|------^---'     5        |
           |      |         |     .--|-------.
           |      |         |    | .-+----.   |
           7      |          '---->| ACME |   |
           |      |              | | STAR | C |
           |      4              | +------| A |
           |      |              | | HTTP |   |
           |      |              | '----+-'   |
           |   .-'                '--^--|----'
    .--------------v--|--.                   |  |
   |      .------.----+-. |                  |  10
   |      |      | STAR | |                  |  |
   | uCDN | CDNI | dele | |                  |  |
   |      |      | fwd  | |                  |  |
   |      '----+-'-+----' |                  |  |
    '-------^--|---|--^--'                   |  |
            |  |   |  |                      |  |
            |  2   8  |                      |  |
            1  |   |  3                      |  |
            |  |   |  |                      9  |
    .-------|--v---v--|---------.            |  |
   |      .-+----.----+-.------. |           |  |
   |      |      | STAR |      +------------'   |
   | dCDN | CDNI | dele | HTTP | |              |
   |      |      | cli  |      |<--------------'
   |      '------'------'------' |
    '---------------------------'
```

### 4.4 RFC 793 — 패킷 헤더 다이어그램 (TCP)

IETF에서 가장 상징적인 ASCII-art. 비트 단위 필드 표현:

```
:    0                   1                   2                   3
:    0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |          Source Port          |       Destination Port        |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |                        Sequence Number                        |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |                    Acknowledgment Number                      |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |  Data |           |U|A|P|R|S|F|                               |
:   | Offset| Reserved  |R|C|S|S|Y|I|            Window             |
:   |       |           |G|K|H|T|N|N|                               |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |           Checksum            |         Urgent Pointer        |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |                    Options                    |    Padding    |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
:   |                             data                              |
:   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 4.5 RFC 9100 — QUIC 타이밍 다이어그램

QUIC 프로토콜의 타이밍 다이어그램도 ASCII-art로 표현. 시간축과 이벤트를 텍스트 그리드로 배치.

## 5. 패킷 헤더 다이어그램 관례

IETF에서 가장 널리 사용되는 ASCII-art 형식. 공식 표준은 없지만 광범위한 관례가 존재한다.

### 5.1 구조 규칙

1. **비트 번호 행** — 상단에 0-31 비트 번호 표시
2. **경계선** — `+`와 `-`로 32비트 경계 표시
3. **필드** — `|`로 구분, 필드명은 중앙 정렬
4. **설명 목록** — 다이어그램 바로 아래에 필드 설명을 구조화된 텍스트로 나열

### 5.2 Augmented ASCII Packet Header Diagrams

draft-mcquistin-augmented-ascii-diagrams에서 기존 관례를 기계 판독 가능하게 확장하는 방법을 제안:
- 각 PDU를 별도 섹션으로 분리
- 섹션 번호로 상호 참조
- ABNF 문법 정의 (draft 단계)

## 6. 우리 프로젝트와의 연관성

| IETF 관례 | ASCII BPMN 표기법 | 공통점 |
|---|---|---|
| 72자 폭 제한 | 터미널 폭 120자 이내 | 둘 다 고정폭 텍스트 환경 전제 |
| 고정폭 폰트 | 7-bit ASCII만 | 렌더링 일관성 |
| 단색 | 색상 없는 텍스트 | 접근성·단순성 |
| SVG + ASCII 병행 | mermaid + ASCII 병행 | 이미지 없는 환경에서도 읽을 수 있게 |
| aasvg 변환 | dg 렌더러 | 텍스트 → 시각적 표현 변환 |
| 패킷 헤더 관례 | 위치 문법 | 텍스트 그리드로 구조 표현 |

## 7. 시사점

1. **검증된 관례** — IETF는 10년 이상 ASCII-art를 RFC 표준 포맷으로 사용. 텍스트 기반 다이어그램 표기의 실용적 가치가 국제 표준 기구에서 검증됨.
2. **도구 생태계** — asciiflow, aasvg, mscgen 등 성숙한 도구체계가 존재. 우리 표기법도 dg 렌더러와 통합 가능.
3. **SVG 병행 전략** — IETF는 ASCII-art를 주력으로 하되 SVG를 병행. 우리 프로젝트의 mermaid(이미지) + ASCII(텍스트) 병행 전략과 동일한 철학.
4. **접근성 우선** — IETF의 ASCII-art 규칙은 접근성과 단순성을 최우선. 우리 표기법의 7-bit ASCII 제한도 같은 맥락.
5. **패킷 헤더 관례의 교훈** — 공식 표준 없이도 관례가 형성되면 널리 채택됨. 우리 ASCII BPMN 표기법도 관례로 시작할 수 있음.

## 8. 참고 링크

- IETF 다이어그램 가이드: https://authors.ietf.org/diagrams
- RFC 6949 (RFC 시리즈 포맷 요구사항): https://www.rfc-editor.org/rfc/rfc6949.html
- RFC 7996 (SVG 1.2 RFC): https://www.rfc-editor.org/rfc/rfc7996.html
- RFC 7994 (Plain-text RFC 요구사항): https://greenbytes.de/tech/specs/rfc7994.html
- RFC 9139 (ASCII-art만 사용 예시): https://www.rfc-editor.org/rfc/rfc9139.html
- RFC 9115 (시퀀스 다이어그램 예시): https://www.rfc-editor.org/rfc/rfc9115.html
- asciiflow: https://asciiflow.com/
- aasvg: https://github.com/martinthomson/aasvg
- rfc-draw: https://github.com/nevil-brownlee/rfc-draw
- Augmented ASCII Packet Header Diagrams: https://www.ietf.org/archive/id/draft-mcquistin-augmented-ascii-diagrams-01.html
