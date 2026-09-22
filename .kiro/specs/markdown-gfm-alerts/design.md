# Design — markdown-gfm-alerts

## 정의
GitHub 스타일 문서를 보는 사용자를 위해, `pulldown-cmark`가 이미 파싱해 주는 GFM 알림 블록
종류를 라벨+기존 카테고리 팔레트로 렌더링하는 `markdown::Renderer`의 기능이다.

## Boundary Commitments

### In-Scope (This Spec Owns)
- `markdown::render_document`가 여는 `pulldown_cmark::Options`에 `ENABLE_GFM` 추가
- `Renderer::handle`의 `Tag::BlockQuote(kind)` 처리 — `kind: Some(BlockQuoteKind)`일 때
  라벨 줄 삽입 + 종류별 스타일 적용, `None`일 때 기존 동작 그대로

### Out-of-Scope
- `ENABLE_GFM`이 다루는 alert 외 다른 항목(현재 버전 기준 alert 블록쿼트가 유일하게 이
  플래그로 활성화되는 기능 — `research.md` 참조 없음, pulldown-cmark 문서로 확인됨)
- 새 `Theme` 필드 추가(기존 `diagram_*` 카테고리 팔레트 재사용 — 아래 Key Decisions)
- 알림 종류·라벨 문구를 사용자가 바꾸는 옵션(CLI 플래그·지시자 없음)

### Allowed Dependencies
- 외부: 없음(신규 크레이트 없음 — `pulldown-cmark`에 이미 있는 `ENABLE_GFM`만 켠다)
- 내부 의존 방향: `style::Theme`(기존 필드만 읽음) → `markdown::Renderer`(기존과 동일)

### Revalidation Triggers
- `pulldown-cmark`가 `ENABLE_GFM`에 alert 외 다른 기능을 추가하면(현재 0.13.4 기준 alert만
  해당) 이 스펙의 "In-Scope는 alert뿐" 전제 재검토 필요
- `Theme`의 `diagram_accent`/`diagram_box`/`diagram_group`/`diagram_note` 역할이 바뀌면(색
  재배정) 알림 팔레트도 같이 바뀜 — 의도된 연동(SSoT)이지 재검토 대상 아님

## Architecture

### Key Decisions
- **새 `Theme` 필드를 추가하지 않고 기존 4색 카테고리 팔레트를 재사용한다** — 이유: dg의 색상
  철학은 "제목·링크는 파랑 계열 하나로 통일하고 나머지는 회색 명도로만 구분"(`style.rs`
  주석)이라, 알림 5종에 새로 5가지 원색(파랑/초록/보라/노랑/빨강)을 넣으면 이 철학과
  충돌한다. 반면 `xychart.rs`의 계열 팔레트, `gitgraph.rs`의 `branch_style`이 이미 "카테고리
  N개를 구분해야 할 때" `[diagram_accent, diagram_box, diagram_group, diagram_note]` 4색을
  순환하는 선례를 만들어 뒀다 — 알림 종류 구분도 정확히 같은 문제(카테고리 N개 구분)라 같은
  팔레트를 재사용한다.
- **다섯 번째 종류(Caution)는 팔레트를 한 바퀴 돌려 Note와 같은 색을 쓰되 굵게 한다** —
  이유: 팔레트가 4색뿐이라 5종을 전부 다른 색으로 채울 수 없다. `Note`(정보)와 `Caution`
  (가장 심각)이 같은 색이면 혼동될 수 있으니 `Caution`만 `.bold()`를 더해 최소한
  Note와는 다르게 보이게 한다. 참 구분 기준은 항상 라벨 텍스트이므로(요구사항 1.7) 색이
  겹쳐도 기능상 문제는 없다.
- **라벨은 GitHub 원문 그대로 영문 단어를 쓴다**(`Note`/`Tip`/`Important`/`Warning`/`Caution`,
  번역하지 않음) — 이유: 시퀀스 다이어그램의 `alt`/`loop`/`opt`처럼, dg는 이미 소스 키워드를
  그대로 노출하는 관례가 있고, GitHub 알림은 그 자체로 전 세계적으로 통용되는 고유 명칭이다.
- **`pulldown-cmark`가 이미 `[!NOTE]` 마커를 본문에서 제거해 준다** — 실제 파싱 결과(`Start(
  BlockQuote(Some(Note))), Start(Paragraph), Text("본문")`)로 확인함, 별도 마커 스트리핑
  로직이 필요 없다(요구사항 3.1은 dg가 아무 것도 안 해도 이미 충족).

## Components and Interfaces

### markdown — alert_style (신규 헬퍼)
- Intent: GFM 알림 종류를 라벨 문자열과 스타일로 매핑한다
- Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7
```rust
/// GFM 알림 종류별 (라벨, 스타일). 4색 카테고리 팔레트(`xychart`/`gitgraph`와 동일한 인프라)를
/// 재사용하고, 팔레트를 한 바퀴 도는 다섯 번째(Caution)만 굵게 더해 Note와 구분한다.
fn alert_style(theme: &Theme, kind: BlockQuoteKind) -> (&'static str, Style) {
    match kind {
        BlockQuoteKind::Note => ("Note", theme.diagram_accent),
        BlockQuoteKind::Tip => ("Tip", theme.diagram_box),
        BlockQuoteKind::Important => ("Important", theme.diagram_group),
        BlockQuoteKind::Warning => ("Warning", theme.diagram_note),
        BlockQuoteKind::Caution => ("Caution", theme.diagram_accent.bold()),
    }
}
```
- 계약: 순수 함수, 패닉 없음(`match`가 `BlockQuoteKind`의 5개 variant를 전부 다룸 — 이후
  pulldown-cmark가 새 variant를 추가하면 컴파일 타임에 non-exhaustive 경고로 드러남).

### markdown — Renderer::handle의 `Tag::BlockQuote` 분기 (기존 코드 수정)
- Intent: 알림 종류가 있으면 라벨 줄을 추가로 그리고, 없으면 기존 인용문 그대로 그린다
- Requirements: 2.1, 2.2, 3.1, 3.2
- 계약: `Tag::BlockQuote(_)`의 `_`를 `kind`로 받는다. 기존 들여쓰기(`│ `)·스타일 push 로직은
  `kind` 값과 무관하게 그대로 실행한다(2.1, 2.2 — 회귀 없음, 일반 인용문·미지원 마커는 지금과
  동일). `kind`가 `Some(k)`이면 `alert_style(theme, k)`로 라벨을 구해 인용문 첫 줄로
  삽입한다(3.1). 이후 본문(문단)은 `pulldown-cmark`가 이미 마커를 제거한 텍스트를 기존 문단
  처리 경로 그대로 흘려보낸다(3.2 — 줄바꿈·긴 줄 접기는 손대지 않음).

## Data Models
신규 영속 타입 없음. `pulldown_cmark::BlockQuoteKind`(외부 크레이트 타입)를 그대로 매치키로
쓴다 — 위 인터페이스 명세로 충분하다.

## Error Handling
- **사용자 입력 오류**: 해당 없음(알 수 없는 마커는 `kind: None`으로 자연히 일반 인용문 처리,
  2.2)
- **외부 자원 오류**: 해당 없음
- **시스템 오류(패닉)**: `alert_style`의 `match`가 전 variant를 다뤄 패닉 경로 없음(4.1)
- **기능 강등**: 없음

## Testing Strategy
- **Depth**: Trivial — 옵션 플래그 하나 추가 + 기존 함수 한 곳의 분기 확장, 새 상태·통합
  지점 없음
- **Unit(L6)**: `alert_style`을 5개 variant 전부로 호출해 라벨 문자열이 맞는지, Note와
  Caution만 같은 색이고 나머지는 서로 다른지, Caution만 `bold`인지 확인 (1.1~1.6)
- **Integration(L4~L5)**: `render_document`를 다섯 종류 알림 + 일반 인용문 + `[!UNKNOWN]`
  마커로 렌더링해 각 라벨이 보이고 원본 `[!...]` 마커가 본문에 안 남는지(3.1), 일반
  인용문·미지원 마커는 기존 골든 텍스트와 같은지(2.1, 2.2), `Theme::none()`에서도 라벨
  텍스트만으로 다섯 종류가 구분되는지(1.7) 확인
- **E2E**: 없음(단일 렌더 함수 호출로 충분)
- **Acceptance(L1)**: `cargo test` 전체 통과(4.2) + 실제 GitHub 스타일 문서를 `dg -P`로
  렌더링해 패닉 없이 끝나고 육안으로 확인(4.1)

## File Structure Plan
```
src/markdown/mod.rs   # ENABLE_GFM 추가, alert_style() 추가, Tag::BlockQuote 분기 확장,
                       # 테스트 추가
```
