# Roadmap

## Overview
1차 discovery(dg-mdview 구현분석)에서 나온 세 항목에 더해, dg 정체성을 "다이어그램 + 마크다운 완전 지원"으로 재확인하는 과정에서 나온 네 항목을 추가한다: 마크다운 원문 하이라이팅, 시퀀스 재귀 메시지 여백 수정, gitGraph 세로 모드, gitGraph 브랜치 시각 구분. 전부 dg 리포지토리 내에서 신규 의존성 없이 끝나는 항목들이다. 구현 순서는 그래프(다이어그램) 기능을 먼저 두고, 디자인 합의가 더 필요한 논란거리는 뒤로 미룬다.

**3차 추가**: "시퀀스 alt 박스가 흐름과 구분 안 됨" 사용자 보고를 계기로 다이어그램 표기 식별성을
전반적으로 재검토했다. 시퀀스 프래그먼트(`alt`/`opt`/`loop` 등) 테두리가 메시지·생명선과 같은
`LineKind::Solid` + 근접한 회색이라 `--style none`에서 완전히 동일해 보이는 게 확인된 결함 —
`block-beta`/`gitGraph`의 기존 `LineKind` 재배정 선례를 그대로 재사용해 고칠 수 있다(완료). 같은
맥락에서 `graph.rs`(흐름도·클래스·ER·상태·컴포넌트 공유) 그룹 vs 노드 테두리도 검토했으나, 색상
계열 자체가 달라(회색 대 파랑) 색 있는 테마에서는 이미 구분되고 `--style none`에서만 모호해 심각도가
낮다 — 게다가 5개 다이어그램형이 공유하는 핵심 파일이라 블라스트 반경이 커서, 이번 라운드 스펙에는
넣지 않고 참고 기록만 남긴다(자세한 근거는 이전 brief.md, git 히스토리 참조).

**4차 추가**: gitGraph 분기·병합 연결선 시각 정리 사용자 보고 — (1) 모서리를 둥글게, (2) 세로
모드에서 분기 연결선이 부모의 최근 커밋과 같은 행에 그려지면서 커밋 id 글자 바로 뒤에 공백 없이
선이 붙는 문제(실제 글자 훼손은 아니고 간격 없음). 둘 다 재현 확인, `layout/gitgraph.rs` 안에서
끝나는 작은 정리라 스펙 하나로 묶는다(자세한 근거는 git 히스토리 참조. 이후 사용자가 실제
터미널 스크린샷으로 "점 글자가 id 첫 글자를 가림"을 제보해 `gitgraph-dot-id-gap` 버그 스펙으로
근본 원인을 별도 수정함 — (2)는 유효했지만 실제 신고된 증상의 근본 원인은 아니었다).

**5차 추가**: `markdown-source-view`(원문 보기)를 시작하려다 그 전에 마크다운 지원 전반을 다시
점검했다. GitHub 스타일 알림 블록(`> [!NOTE]` 등)이 전혀 인식되지 않는 게 실제로 재현됐다 —
`pulldown-cmark`에 이미 구현된 `ENABLE_GFM` 옵션을 안 켜고 있었을 뿐이라 신규 의존성 없이 낮은
리스크로 고칠 수 있다. 나머지 미지원 확장(스마트 문장부호·정의 목록·위/아래 첨자·위키링크·
헤딩 속성·수식)은 실사용 빈도가 낮거나 의존성/표현 방식 문제가 있어 이번 라운드는 보류한다
(근거는 이전 brief.md, git 히스토리 참조). 재현된 결함 + 기존 라이브러리 기능 재사용이라 설계
이견이 없는 항목이므로 `markdown-source-view`보다 앞에 둔다.

**6차 추가**: "md full support" 축을 이어서, 링크 이동(문서 내 앵커 점프·외부 URL 브라우저
열기·다른 `.md` 파일 이동+back/forward 히스토리)을 검토했다. dg에 전혀 없던 기능임을 확인—
링크는 정적 텍스트일 뿐 상호작용 개념이 없다. `dg-watch-mode`가 이미 만들어 둔 `Pager`의
`source` 교체+`rerender()` 경로를 파일 이동에 재사용할 수 있어 생각보다 구조적 부담이 작다.
코드펜스 신택스 하이라이팅도 같이 논의됐으나 사용자가 이번 라운드는 보류하기로 확인해
`markdown-syntax-highlighting`(가칭)으로 이름만 남기고 스펙화하지 않는다. lazy/뷰포트
파싱으로의 구조 전환도 검토했으나, 다이어그램은 이미 블록 단위 독립 렌더링이고 앵커 점프·
검색은 오히려 전체 선(先)파싱이 유리해 채택하지 않는다(자세한 근거는 brief.md). 1차
discovery가 배제한 "파일브라우저"와의 관계도 정리했다 — 디렉터리 브라우징 UI는 여전히 배제,
이번에 들어가는 건 "문서 저자가 적어 둔 링크를 따라가는 것"뿐이라 배제 범위를 좁혀 재정의.

## Approach Decision
- **Chosen**: 혼합 분해(Path E) 유지 — 스펙 8건(신규 7건 + 1차 `dg-watch-mode`) + 직접구현 후보 2건(1차 유지) + 경계 밖 기록 확장. 스펙 순서는 "그래프 기능 우선, 논란거리는 뒤로" 원칙으로 재배열. 4차 추가로 `gitgraph-junction-polish`, 5차 추가로 `markdown-gfm-alerts`, 6차 추가로 `markdown-link-navigation`을 더한다(`markdown-gfm-alerts`/`markdown-link-navigation` 둘 다 md 축이지만 이번 discovery로 스코프가 좁혀져 `markdown-source-view`보다 앞).
- **Why**: 시퀀스 재귀 여백·gitGraph 세로 모드는 이미 재현된 결함이거나 기존 `--direction` 패턴을 그대로 확장하는 것이라 설계 이견이 거의 없다 — 먼저 구현해 빠르게 가치를 낸다. gitGraph 브랜치 색+패턴 구분은 같은 "그래프" 축이지만 "어떤 색을, 어떤 패턴으로"가 취향·가독성 판단이 들어가는 논란거리라 세로 모드 뒤로 민다. `dg-watch-mode`와 `markdown-source-view`는 다이어그램 배치 로직이 아니라 뷰어 UX/마크다운 축이라 그래프 기능들보다 뒤에 두고, 그중에서도 `markdown-source-view`는 "md full support" 정체성 자체가 이번에 막 재정의된 것이라 스코프 이견 여지가 가장 커서 맨 뒤에 둔다. `sequence-fragment-frame-distinction`은 재현된 결함 + 기존 선례 재사용이라 설계 이견이 없는 diagram 축 항목 — `markdown-source-view`보다 앞에 둔다. `gitgraph-junction-polish`도 마찬가지로 재현된 결함(간격) + 기존 `round` 메커니즘 재사용(모서리)이라 설계 이견이 없고, gitGraph 계열 스펙들과 같은 파일을 다루므로 그 바로 뒤(브랜치 구분 다음, 시퀀스 프레임 스펙과는 독립)에 둔다.
- **Rejected alternatives**: (1) 기존처럼 의존관계·모듈 인접성만으로 순서 정하기 — 이번엔 사용자가 "그래프 먼저, 논란거리는 뒤로"라는 별도 우선순위 기준을 명시했으므로 그 기준을 스펙 순서에 직접 반영하는 쪽을 택함. (2) 논란거리 항목을 아예 Out of Boundary로 빼기 — 배제가 아니라 순서 조정 요청이므로 스펙 목록에는 남기고 뒤로만 민다. (3) `graph.rs` 그룹 테두리 개선도 바로 스펙으로 추가 — 사용자 요청은 "검토"였지 "수정"이 아니었고, 실측 결과 심각도가 낮은 데다 블라스트 반경이 커서 별도 확인 없이 로드맵에 확정 항목으로 못 박지 않기로 함. (4) 모서리 둥글기·간격 문제를 스펙 2건으로 쪼개기 — 각각 몇 줄 안 되는 변경이라 스펙 게이트 두 번을 거치는 오버헤드가 과함, 하나로 묶되 requirements에서 최종 확인. (5) 스마트 문장부호 등 나머지 미지원 마크다운 확장도 같이 스펙으로 추가 — 실사용 빈도·표현 방식 문제로 가치 대비 리스크가 커서 이번 라운드는 보류, 근거만 기록. (6) lazy/뷰포트 파싱으로 구조 전환 — 다이어그램은 이미 블록 단위, 앵커 점프·검색은 전체 선파싱이 더 유리해 기각. (7) 코드펜스 신택스 하이라이팅을 이번 라운드에 포함 — 사용자가 명시적으로 보류 결정.

## Scope
- **In**: (1차) watch 모드, panic 감사, 문턱값 레퍼런스 승격. (2차) 마크다운 원문 하이라이팅, 시퀀스 재귀 여백, gitGraph 세로 모드, gitGraph 브랜치 시각 구분. (3차) 시퀀스 프래그먼트 테두리 선패턴 구분. (4차) gitGraph 분기·병합 연결선 모서리 둥글기 + 세로 모드 커밋 id 간격. (5차) GFM 알림 블록(`[!NOTE]` 등) 지원. (6차) 마크다운 링크 이동(앵커·외부 URL·파일 간 이동+히스토리).
- **Out**: mdview 리포지토리 수정 전부, dg에 syntect 코드펜스 언어 하이라이팅·이미지 프로토콜·파일브라우저(디렉터리 브라우징 UI)·GitHub 소스 fetch·스태시, `graph.rs` 그룹 vs 노드 선패턴 구분(색상으로 이미 구분됨, 참고 기록만), 다른 다이어그램형의 모서리 둥글기(요청 밖), 스마트 문장부호·정의 목록·위/아래 첨자·위키링크·헤딩 속성·수식(실사용 빈도 낮음/표현 방식 미정/의존성 충돌), 코드펜스 신택스 하이라이팅(보류), lazy/뷰포트 파싱 구조 전환(불필요 판단).

## Constraints
신규 의존성 추가 없음, panic 금지 계약 유지, `edition 2024`/`rust 1.88` 유지, 기존 `--direction` 메커니즘 재사용. (상세는 brief.md 참조)

## Boundary Strategy
- **Why this split**: "diagram" 축(재귀 여백, gitGraph 세로 모드, gitGraph 브랜치 구분)과 "md full support" 축(원문 하이라이팅)은 건드리는 모듈이 다르다(`diagram::layout::*` vs `markdown::*`/`pager.rs`) — 변경 이유가 갈려 있어 축별로 스펙을 나눈다. gitGraph 두 스펙(세로 모드/브랜치 구분)은 같은 파일(`layout/gitgraph.rs`)을 건드리지만 하나는 배치 로직, 하나는 스타일 로직이라 SRP상 분리하되 순서만 조율한다.
- **Priority rule applied**: "그래프 기능 먼저, 논란거리는 뒤로"(사용자 지시) — diagram 축 중 설계 이견 없는 항목(재귀 여백, 세로 모드) → diagram 축 중 논란거리(브랜치 구분) → 비-diagram 축(watch 모드) → 스코프 자체가 논란거리인 항목(원문 하이라이팅) 순.
- **Shared seams to watch**: gitGraph 두 스펙 다 `layout/gitgraph.rs`를 건드리므로 세로 모드를 먼저 구현 확정한 뒤 브랜치 구분을 얹을 것(의존관계로 명시). 재귀 여백 수정이 기존 채널/폭 재시도 로직을 깨지 않는지, 원문 하이라이팅이 기존 `render_source_block`/`o` 토글과 자연스럽게 이어지는지, `--style none`에서 gitGraph 브랜치 구분이 실제로 색 없이도 식별되는지(로버스트니스 테스트 추가).

## Specs (dependency order — 그래프 기능 우선, 논란거리는 뒤로)
- [x] sequence-self-message-clearance -- 재귀(self-message) 루프의 화살촉과 생명선 사이에 최소 1칸 여백 확보. 재현된 결함, 설계 이견 없음. Dependencies: none
- [x] gitgraph-vertical-mode -- gitGraph에 `--direction tb|lr` 세로/가로 배치 추가. 기존 패턴 확장, 설계 이견 없음. Dependencies: none
- [x] gitgraph-branch-distinction -- 브랜치별 색+선패턴 이중 구분(`--style none`에서도 식별 가능). 같은 그래프 축이지만 색·패턴 선택은 논란거리라 세로 모드 뒤로. Dependencies: gitgraph-vertical-mode
- [x] dg-watch-mode -- 파일 변경 감지 후 자동 재렌더링(페이저·print 양쪽 지원). 그래프 배치 로직이 아닌 뷰어 UX라 그래프 스펙들 뒤로. Dependencies: none
- [x] sequence-fragment-frame-distinction -- 시퀀스 `alt`/`opt`/`loop`/`par`/`critical`/`break` 프래그먼트 테두리를 메시지·생명선과 다른 `LineKind`(Heavy)로 구분. 재현된 결함 + `block-beta`/`gitGraph` 선례 재사용이라 설계 이견 없음. Dependencies: none
- [x] gitgraph-junction-polish -- gitGraph 분기·병합 연결선의 모서리를 둥글게(기존 `canvas.rect`/`join`의 `round` 메커니즘 재사용), 세로 모드에서 분기 연결선이 부모의 최근 커밋 행과 겹칠 때 커밋 id 글자와 최소 한 칸 간격 확보. 재현된 결함 + 기존 메커니즘 재사용이라 설계 이견 없음. Dependencies: gitgraph-branch-distinction
- [x] markdown-gfm-alerts -- GitHub 스타일 알림 블록(`> [!NOTE]`/`[!TIP]`/`[!IMPORTANT]`/`[!WARNING]`/`[!CAUTION]`)을 라벨+색으로 렌더링. `pulldown-cmark`의 `ENABLE_GFM`을 켜기만 하면 되는 기존 라이브러리 기능 재사용 — 재현된 결함, 설계 이견 없음. Dependencies: none
- [ ] markdown-link-navigation -- 문서 내 앵커(`#헤딩`) 점프, 외부 URL 브라우저 열기(`xdg-open`/`open` 셸아웃), 다른 `.md` 파일로 이동(`dg-watch-mode`의 `source`/`rerender()` 경로 재사용)+back/forward 히스토리. 마우스·키보드 양쪽 링크 선택. 이번 discovery로 스코프가 좁혀짐(디렉터리 브라우징 UI는 계속 배제). Dependencies: dg-watch-mode
- [ ] markdown-source-view -- 마크다운 원문을 마크업 신택스 하이라이팅과 함께 보여주는 모드(기존 `o` 다이어그램 원문 토글을 문서 전체로 일반화). "md full support" 축의 스코프 자체가 논란거리라 맨 뒤 — 트리거 방식(전역 토글/블록별 확장/둘 다) requirements 단계에서 결정 필요. Dependencies: none

## Existing Spec Updates
(없음)

## Direct Implementation Candidates
- [ ] diagram-panic-audit -- `layout/chart.rs`, `layout/sequence.rs`의 non-test `unwrap`/`expect` 3건을 점검해 `Option`/`None` 계약으로 대체하거나 안전성을 증명한다. 그래프 축에 속하고 논란거리 없음 — 스펙 게이트 없이 바로 진행 가능.
- [ ] graph-swap-threshold-reference -- `layout/graph.rs`의 `SWAP_REACH_LARGE` 등 성능 문턱값과 근거를 `{{REFERENCE}}`로 승격해 재측정 가능하게 문서화한다. 그래프 축, 논란거리 없음.

## Out of Boundary (cross-repo 또는 철학 충돌, 실행하지 않음)
- mdview에 PlantUML/방향 재시도/ER 표기법 이식 — mdview 자체 discovery에서 시작할 것
- mdview를 lib+bin 이중 구조로 분리(`[lib]`/`src/lib.rs` 추가) — spec-viewer의 `engine-mdview` 포팅 코드를 실제 의존성으로 바꾸는 데 필요하지만 mdview 저장소 소관
- dg에 syntect 기반 코드펜스 "언어" 하이라이팅, 이미지 프로토콜(sixel), 파일브라우저, GitHub 소스 fetch, 스태시 — 의존성 최소화 원칙과 충돌 또는 스코프 확장
