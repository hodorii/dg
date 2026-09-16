# dg

가벼운 터미널 마크다운 뷰어. mermaid와 PlantUML 코드블록을 **문자 그림**으로 그린다.
아키텍처(컴포넌트)·클래스·ERD·시퀀스·흐름도·상태도를 이미지 프로토콜이나 외부 도구(Java, 브라우저) 없이
SSH·tmux 안에서 그대로 볼 수 있다.

- 의존성 4개(pulldown-cmark, crossterm, unicode-width, clap), 단일 정적 바이너리 약 1.2MB
- 다이어그램은 공통 IR 하나로 모여 배치기 두 개(계층 그래프, 시퀀스)만 유지한다
- 폭에 맞지 않으면 방향(LR↔TB)과 라벨 폭을 줄여 가며 다시 시도하고, 그래도 안 되면 원문 코드블록으로 보여 준다

## 설치

```sh
make install                  # ~/.local/bin/dg (PREFIX=... 로 변경, rustc 1.88 이상)
cargo install --path .        # 또는 ~/.cargo/bin
```

`make test`, `make lint`, `make uninstall`도 있다. `make deploy-mac`은 소스를 ssh 호스트 `mac`(`MAC_HOST=...`로 변경)으로
보내 그쪽 cargo로 빌드·설치한다(`~/.cargo/bin/dg`). macOS Terminal.app·iTerm2는 `COLORFGBG`를 내보내지 않으므로
밝은 배경이면 `export DG_STYLE=light`를 셸 설정에 넣는다.

## 사용법

```sh
dg README.md                  # 페이저로 열기 (터미널이면 기본)
dg -P README.md               # 페이저 없이 출력
cat doc.md | dg               # 표준입력
dg diagram.puml               # .puml/.mmd 는 다이어그램 단독 렌더링
dg -d -l mermaid graph.txt    # 확장자가 없을 때 언어 지정
dg -s light -w 80 doc.md      # 밝은 테마, 폭 80
```

| 옵션 | 설명 |
|------|------|
| `-w, --width N` | 폭. 기본은 문단은 터미널 폭(최대 120)에서 접고 다이어그램·표·코드블록은 터미널 폭을 다 씀. 주면 둘 다 이 값 |
| `-s, --style auto\|dark\|light\|none` | 색 테마. auto는 TTY면 dark, `COLORFGBG`로 밝은 배경 감지 |
| `-P, --print` | 페이저 없이 stdout으로 (파이프면 자동) |
| `-d, --diagram` | 입력을 다이어그램 소스로 취급 |
| `-l, --lang mermaid\|plantuml` | 다이어그램 언어 지정 (기본 자동 판별) |
| `--er-notation crow\|text\|both` | ERD 카디널리티 표기: 까치발(기본)·글자(`1`, `0..N`)·둘 다. 환경변수 `DG_ER_NOTATION` |
| `--direction tb\|lr` | 그래프(흐름도·클래스·ER·상태·컴포넌트) 배치 방향 강제. 폭에 안 들어가면 반대 방향으로 재시도. 환경변수 `DG_DIRECTION` |

환경변수: `DG_STYLE=dark|light|none|auto`, `DG_ER_NOTATION=crow|text|both`, `DG_DIRECTION=tb|lr`, `NO_COLOR`.

### 소스 안 지시자

문서마다 표기를 바꾸고 싶으면 각 언어의 관례대로 지시자를 넣는다(명령줄 옵션보다 우선). 지원 키: `erNotation`(crow·text·both), `direction`(tb·lr). 키는 `er_notation`, `er-notation`처럼 써도 같다.

```
%%{init: {"dg": {"erNotation": "text"}}}%%     mermaid init 지시자 (다른 항목과 함께 써도 됨)
%% dg: erNotation=both direction=lr             mermaid 주석 꼴 (여러 키를 한 줄에)
!pragma dg erNotation=text                      PlantUML pragma
' dg: erNotation=both                           PlantUML 주석 꼴
```

### 페이저 키

| 키 | 동작 |
|----|------|
| `j`/`k`, 방향키, 마우스 휠 | 한 줄 |
| `Space`, `PgDn`/`PgUp`, `Ctrl-f`/`Ctrl-b` | 한 화면 |
| `Ctrl-d`/`Ctrl-u` | 반 화면 |
| `g`/`G` | 처음/끝 |
| `/` 입력 후 `Enter`, `n`/`N` | 검색(대소문자 무시), 다음/이전. `Esc`로 해제 |
| `o`, 다이어그램 클릭 | 다이어그램 원문(mermaid/PlantUML 코드)을 아래에 펼치거나 접기 — 검증용 |
| `q` | 종료 |

창 크기가 바뀌면 다시 렌더링한다.

## 지원 문법

### 마크다운
제목(H1 `━`, H2 `─` 밑줄), 문단 줄바꿈(한글 어절 단위, CJK 폭 계산), 강조·굵게·취소·인라인 코드,
링크 `텍스트 (URL)`, 이미지, 목록(중첩·번호·체크박스), 인용, 코드블록, 표(정렬·셀 줄바꿈·폭 축소), 수평선,
각주, YAML/TOML 프론트매터 생략, HTML 흐리게.

### mermaid (` ```mermaid `)

| 종류 | 지원 |
|------|------|
| `flowchart`/`graph` | `TB`/`LR` 방향, 노드 모양 `[ ]` `( )` `([ ])` `[[ ]]` `[( )]` `(( ))` `{ }` `{{ }}`, `-->` `---` `-.->` `==>` `<-->` `--x` `--o`, `-- 글 -->`, `-->\|글\|`, `A & B --> C`, `subgraph`(중첩, `subgraph Id["제목"]`), 서브그래프 아이디로 향하는 간선(상자 위로 들어옴), 자기 간선, 되돌아가는 간선 |
| `sequenceDiagram` | `participant`/`actor`(`as` 별칭), `->>` `-->>` `-x` `-)` `->`, `+`/`-` 활성화, `activate`/`deactivate`, `Note left/right of/over`, `alt`/`else`/`opt`/`loop`/`par`/`critical`/`break`/`rect`, `box`, `autonumber`, `title` |
| `classDiagram` | `class X { }`, `X : 멤버`, `<<interface>>`, 제네릭 `~T~`, `<\|--` `--\|>` `<\|..` `..\|>` `*--` `o--` `-->` `..>` `--`, 다중성 `"1" --> "*"`, 라벨 `: 글`, `namespace`, `direction` |
| `erDiagram` | 엔터티 속성(`type name PK "설명"` → `name : type [PK]`), `\|\|--o{` 계열 카디널리티를 까치발 표식으로(`╪` 하나, `○` 없음, `⋏`/`⋎` 여럿), 식별(실선)/비식별(점선) |
| `stateDiagram(-v2)` | `[*]` 시작(●)/끝(◉), `-->` 전이 라벨, `state "설명" as X`, 합성 상태 `state X { }`, `<<choice>>` |

### PlantUML (` ```plantuml `, ` ```puml `)

종류는 본문에서 자동 판별한다(`participant`·`->`면 시퀀스, `class`/`<|--`면 클래스, `entity`만 있으면 ER, `[컴포넌트]`·`package`면 컴포넌트).

| 종류 | 지원 |
|------|------|
| 시퀀스 | `participant`/`actor`/`boundary`/`control`/`entity`/`database`/`collections`/`queue`(`"긴 이름" as X`, `X as "긴 이름"`), `->` `-->` `->>` `->x` `->o` `<-` `<--`, `++`/`--` 활성화, `activate`/`deactivate`/`return`, `note left/right of/over` (한 줄·`end note`), `alt`/`else`/`opt`/`loop`/`par`/`break`/`critical`/`group`/`end`, `== 구분 ==`, `...지연...`, `box`, `autonumber`, `title` |
| 클래스 | `class`/`abstract`/`interface`/`enum`/`annotation`/`object`(`{ }` 본문, `--`·`..`·`==` 칸 구분, `{static}` 등 제거), `extends`/`implements`, `X : 멤버`, `<\|--` `..\|>` `*--` `o--` `-->` `..>` `--`, `-down->` 같은 방향 힌트와 `[hidden]`·`[#색]` 무시, 다중성 `"1" -- "*"`, `package`/`namespace`, `left to right direction`, `title` |
| ER | `entity X { *id : int <<PK>> \n -- \n name }`, `\|\|--o{` `}o--\|\|` `\|o--o\|` `}\|--\|{` 카디널리티(까치발 표식) |
| 컴포넌트·배치·유스케이스 | `[이름]`, `[이름] as 별칭`, `component`/`interface`/`()`/`database`/`node`/`cloud`/`folder`/`frame`/`rectangle`/`storage`/`queue`/`actor`/`:액터:`/`usecase`/`(유스케이스)` 등, `package … { }` 중첩 그룹, `-->` `..>` `--` 와 `: 라벨`, `left to right direction`, `title` |

`skinparam`, `hide/show`, `!전처리`, `'주석`, `/' 블록 '/`, `legend`, `header/footer`는 무시한다.

## 라이브러리로 쓰기

렌더링 부분은 라이브러리 크레이트(`dg`)로도 쓸 수 있다. 페이저·명령줄은 `cli` 피처 뒤에 있으므로
끄면 clap·crossterm 없이 pulldown-cmark와 unicode-width만 딸려온다.

```toml
[dependencies]
dg = { path = "../dg", default-features = false }   # 또는 git = "..."
```

```rust
use dg::{render_markdown, render_diagram, to_ansi, to_text, Language, RenderOptions, Theme};

let options = RenderOptions { width: 80, theme: Theme::dark(), ..RenderOptions::default() };
let lines = render_markdown(source, &options);        // Vec<Line>
print!("{}", to_ansi(&lines, &options.theme));         // ANSI 문자열
let plain = to_text(&lines);                           // 색 없는 평문

let diagram = render_diagram(puml_source, Some(Language::PlantUml), &options); // Option<Vec<Line>>
```

`Line`은 문자열 하나와 (길이, 스타일) 구간 목록이라 `runs()`로 구간을 돌며 다른 렌더러(예: ratatui 스팬)로 옮길 수 있다.
다이어그램 블록의 줄 범위와 원문이 필요하면 `dg::markdown::render_document`를 쓴다. 예제: `cargo run --example lib_usage`.

## 배치 방식

그래프 계열(흐름도·클래스·ER·상태·컴포넌트)은 `src/diagram/layout/graph.rs` 하나가 그린다.

1. **층 나누기** — DFS로 되돌아가는 간선을 찾아 뒤집고 최장 경로로 층을 매긴다. 여러 층을 건너뛰는 간선은 가상 노드로 쪼갠다.
2. **블록 배치** — 그룹(subgraph/package)을 블록으로 보고, 블록 안에서 자식들을 무게중심(barycenter) 순서로 4회 정렬한 뒤
   층이 겹치는 앞 자식의 오른쪽에 차례로 놓는다. 블록은 걸친 모든 층에서 같은 띠를 차지하므로 그룹 테두리가 포개지지 않는다.
3. **직선화** — 노드를 자기 블록 안에서만 움직여 이웃의 중앙값 위치에 맞춘다.
4. **배선** — 층 사이 통로에 가로 구간을 넣고 겹치는 구간은 다른 줄에 배정한다. 라벨은 구간이 넉넉하면 가운데, 아니면 다른 세로선과 붙지 않는 쪽에 둔다.
   라벨이 그룹 밖으로 나가면 그 블록 폭을 늘려 한 번 더 배치한다.
5. **방향** — 배치는 along/across 좌표계로 하고 캔버스로 옮길 때만 TB/LR을 적용한다. LR에서는 간선마다 라벨 줄이 필요하므로 상자를 두 줄 간격으로 키운다.
6. **폭 맞추기** — 원하는 방향·반대 방향·라벨 폭 축소 순으로 시도하고, 그래도 넓으면 위→아래 배치에서
   가장 넓은 줄(한 그룹의 한 층에 나란히 놓인 것들)의 맨 오른쪽 자식을 한 층 아래로 내리는 **층 접기**를 반복한다
   (하위 그룹이면 통째로 옮긴다). 어떤 그룹 안에서만 간선이 들어오는 바깥 노드는 처음부터 그 그룹 아래에 둔다.
   실제 운영 문서의 20~30노드짜리 아키텍처 도면이 110칸 안에 들어간다.
7. **교차 처리** — 간선끼리 직교하면 `┼` 대신 `◠`로 건너뛴다. 한 간선의 출발 접점과 다른 간선의 도착 접점이 같은 열이면
   통로 순서를 강제해 세로선이 겹치지 않게 하고, 서로 열을 맞바꾸는 X자 교차는 실제 노드 쪽 접점을 한 칸 옮겨 푼다.
   무게중심 정렬 뒤에는 자식(가상 노드 사슬은 한 묶음)을 같은 층의 앞뒤 자식 너머로 옮겨 보고, 직선화까지 한 자리에서
   "간선 길이 합 + 교차 수×12"가 줄면 받아들인다(폭이 넘치면 교환 전으로). 직선화에서 가상 노드는 같은 블록의
   이웃을 밀어내서라도 위아래 접점과 열을 맞춘다.
   `DG_DEBUG=1`이면 노드 층·접점·통로 배정을 stderr에 적는다.

시퀀스는 `layout/sequence.rs`가 참여자 간격을 제약(라벨·노트·프레임 폭)으로 풀어 정한다.

## 개발

```sh
cargo test            # 단위·강건성 테스트
cargo clippy
dg -P -s none examples/architecture.md   # 예제 문서
```

## 라이선스

MIT
