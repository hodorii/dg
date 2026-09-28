//! 코드블록의 다이어그램 언어를 판별해 그림 줄로 바꾼다.

pub mod bpmn;
pub mod canvas;
pub mod ir;
pub mod layout;
pub mod mermaid;
pub mod options;
pub mod plantuml;

use crate::line::{Line, Span};
use crate::style::Theme;
use crate::text::width_of;
pub use options::{DiagramOptions, ErNotation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Mermaid,
    PlantUml,
    Bpmn,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Language::Mermaid => "mermaid",
            Language::PlantUml => "plantuml",
            Language::Bpmn => "bpmn",
        }
    }
}

/// 코드 펜스의 언어 이름으로 다이어그램 언어를 고른다.
pub fn language_of_fence(lang: &str) -> Option<Language> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "mermaid" | "mmd" => Some(Language::Mermaid),
        "plantuml" | "puml" | "uml" => Some(Language::PlantUml),
        "bpmn" => Some(Language::Bpmn),
        _ => None,
    }
}

/// 파일 확장자 표의 단일 출처. 소스 언어 추정과 명령줄의 "다이어그램 파일인가" 판정이 함께 쓴다.
pub fn language_of_path(path: &str) -> Option<Language> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".puml") || lower.ends_with(".plantuml") || lower.ends_with(".pu") || lower.ends_with(".iuml") {
        return Some(Language::PlantUml);
    }
    if lower.ends_with(".mmd") || lower.ends_with(".mermaid") {
        return Some(Language::Mermaid);
    }
    if lower.ends_with(".bpmn") {
        return Some(Language::Bpmn);
    }
    None
}

/// 파일 확장자나 본문으로 다이어그램 언어를 추정한다.
pub fn language_of_source(path: Option<&str>, source: &str) -> Option<Language> {
    if let Some(language) = path.and_then(language_of_path) {
        return Some(language);
    }
    let trimmed = source.trim_start();
    if trimmed.starts_with("@start") {
        return Some(Language::PlantUml);
    }
    if mermaid::kind_of(source).is_some() {
        return Some(Language::Mermaid);
    }
    // BPMN을 PlantUML보다 먼저 본다 — PlantUML `kind_of`는 점수가 없으면 `Some("class")`를 돌려주는
    // 포괄 판별기라 뒤에 두면 확장자 없는 BPMN XML을 영영 못 본다(design §Key Decisions).
    if bpmn::kind_of(source).is_some() {
        return Some(Language::Bpmn);
    }
    if plantuml::kind_of(source).is_some() {
        return Some(Language::PlantUml);
    }
    None
}

/// 다이어그램을 캡션(`◈ mermaid · flowchart ───`)과 함께 그린다. 지원하지 않거나 폭에 맞지 않으면 `None`.
pub fn render(language: Language, source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<Vec<Line>> {
    let (kind, body) = render_body(language, source, theme, width, options)?;
    let mut out = vec![caption(language.name(), kind, theme, width)];
    out.extend(body);
    Some(out)
}

/// 캡션 없이 그림 줄만 돌려준다: (종류 이름, 줄들). 다른 뷰어에 엔진으로 끼울 때 쓴다.
pub fn render_body(language: Language, source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let options = options.with_source(source);
    let (kind, mut body) = match language {
        Language::Mermaid => mermaid::render(source, theme, width, options)?,
        Language::PlantUml => plantuml::render(source, theme, width, options)?,
        Language::Bpmn => bpmn::render(source, theme, width, options)?,
    };
    if body.iter().all(Line::is_blank) {
        return None;
    }
    if body.iter().map(Line::width).max().unwrap_or(0) > width {
        return None;
    }
    while body.last().is_some_and(Line::is_blank) {
        body.pop();
    }
    Some((kind, body))
}

/// 소스만 보고 종류 이름(`flowchart`, `sequence`, `class`, `er`, `state`, `block`, `component`)을 알려준다.
pub fn kind_of(language: Language, source: &str) -> Option<&'static str> {
    match language {
        Language::Mermaid => mermaid::kind_of(source),
        Language::PlantUml => plantuml::kind_of(source),
        Language::Bpmn => bpmn::kind_of(source),
    }
}

fn caption(language: &str, kind: &str, theme: &Theme, width: usize) -> Line {
    let text = format!("◈ {language} · {kind} ");
    let pad = width.saturating_sub(width_of(&text)).min(40);
    Line::from_spans(vec![Span::new(text, theme.diagram_caption), Span::new("─".repeat(pad), theme.rule)])
}

#[cfg(test)]
mod robustness {
    use super::*;

    /// 잘리거나 이상한 입력에도 패닉 없이 결과 또는 `None`을 돌려줘야 한다.
    #[test]
    fn odd_inputs_do_not_panic() {
        let mermaid = [
            "flowchart TB",
            "flowchart TB\n A",
            "flowchart LR\n A --> A",
            "flowchart TB\n A --> B\n B --> A\n",
            "flowchart TB\n subgraph S\n end\n A --> B",
            "flowchart TB\n A[\"unclosed",
            "flowchart TB\n A -- --> B",
            "graph TD\n A-->B-->C-->A",
            "graph LR\n A & B & C --> D & E\n E --> A\n D -.->|x| B",
            "sequenceDiagram",
            "sequenceDiagram\n A->>A: self\n alt\n end\n",
            "sequenceDiagram\n Note over A: alone",
            "sequenceDiagram\n A->>B: 아주 긴 메시지 라벨이 여기에 들어가면 어떻게 될까요 한번 봅시다 정말로 길게 써 봅니다\n B-->>A: ok",
            "stateDiagram-v2\n [*] --> [*]",
            "stateDiagram-v2\n state X {\n }\n",
            "erDiagram\n A {\n }\n",
            "erDiagram\n A ||--|| B",
            "classDiagram\n class A\n",
            "classDiagram\n A <|-- B : \n",
            "classDiagram\n A \"1\" --> \"*\" B\n B --> C\n C --> A\n A --> A : loop",
            "gitGraph",
            "gitGraph:\n commit\n commit\n",
            "gitGraph\n branch\n checkout\n merge\n commit id:\n",
            "gitGraph\n commit\n merge ghost\n commit\n",
            "gitGraph\n commit\n checkout phantom\n commit\n merge main\n",
            "gitGraph\n commit id: \"아주 긴 커밋 아이디가 여기에 들어가면 어떻게 될까요\"\n branch feature\n commit\n",
            "gitGraph\n commit\n branch a\n commit\n branch b\n commit\n branch c\n commit\n checkout main\n merge c\n merge b\n merge a\n commit\n",
            "gitGraph TB:\n commit\n commit\n commit\n",
            "gitGraph BT:\n commit\n branch feature\n commit\n checkout main\n merge feature\n",
            "gitGraph TB:\n commit id: \"아주 긴 커밋 아이디가 여기에 들어가면 어떻게 될까요\"\n branch a\n commit\n branch b\n commit\n branch c\n commit\n checkout main\n merge c\n merge b\n merge a\n",
            "block-beta",
            "block-beta\n columns 3\n a[\"Block A\"] b:2\n c d e\n a --> c",
            "block-beta\n columns 2\n block:group1\n  columns 1\n  x y\n end\n z",
            "block-beta\n columns 0\n a:99 space:3\n a --> a\n a --> zzz",
            "block-beta\n block:g\n  block:h\n   block:i\n    q\n",
            "block-beta\n columns 1\n a[\"아주 긴 블록 이름을 넣어 보면 어떻게 될까요 정말 길게 써 봅니다\"]\n b\n a -- \"긴 라벨도 붙여 본다\" --> b",
            "pie",
            "pie showData",
            "pie title 제목만 있고 항목은 없다",
            "pie\n \"A\" : 0\n \"B\" : 0\n",
            "pie\n \"A\" : 1e400\n \"B\" : NaN\n \"C\" : -5\n",
            "pie\n \"값이 없는 항목\" : \n \"구분자가 없는 줄\"\n \"A\" : 3\n",
            "pie\n \"이스케이프된 #quot;따옴표#quot;\" : 3\n \"백슬래시 \\\"따옴표\\\"\" : 2\n",
            "pie showData title 아주 길고 긴 제목을 붙이면 폭을 넘길지도 모른다 과연 어떨까\n \"정말 길고 긴 항목 이름을 하나 넣어 봅니다 어떻게 되나\" : 1234567\n \"B\" : 1\n",
            "xychart-beta",
            "xychart-beta\n bar [1, 2, 3]",
            "xychart-beta\n x-axis [a, b]\n bar [1]\n line [1, 2, 3, 4]",
            "xychart-beta\n title \"제목\"\n x-axis \"달\" [1월, 2월]\n y-axis \"권\" -5 --> 5\n bar [3, -4]\n line [-5, 5]",
            "xychart-beta\n x-axis 0 --> 10\n line [1 \"a\", 2 \"b\"]",
            "xychart\n y-axis 1e300 --> -1e300\n bar [\n line []",
            "quadrantChart",
            "quadrantChart\n title\n x-axis\n y-axis\n",
            "quadrantChart\n x-axis 하나뿐\n quadrant-3 왼쪽 아래\n",
            "quadrantChart\n Campaign A: [0.3, 0.6]\n Campaign A: [0.3, 0.6]\n",
            "quadrantChart\n 밖으로: [12, -7]\n 이상함: [x, y]\n 반쪽: [0.5]\n",
            "quadrantChart\n 아주 길고 긴 항목 이름을 가진 무언가: [0.99, 0.99]\n",
            "quadrantChart\n A:::hot: [0.2, 0.2] radius: 5, color: #f00\n classDef hot color: #f00\n",
            "gantt",
            "gantt\n title T\n section S",
            "gantt\n a :",
            "gantt\n dateFormat YYYY-MM-DD\n section S\n a :a1, 2024-01-01, 5d\n b :done, after a1, 2w",
            "gantt\n a :a1, after a1, 1d\n b :b1, after nope, 1d",
            "gantt\n a :a1, 2024-99-99, 5x\n b :b1, 9999-12-31, 100000w",
            "gantt\n dateFormat DD-MM-YYYY\n 아주 긴 작업 이름을 여기에 적어 봅니다 :x1, 01-01-2024, 3d",
        ];
        let plantuml = [
            "@startuml\n@enduml",
            "@startuml\nA -> B\n@enduml",
            "@startuml\nA -> A : self\nalt x\nelse\nend\nend\n@enduml",
            "@startuml\nclass A {\n@enduml",
            "@startuml\nA <|-- B\nB <|-- A\n@enduml",
            "@startuml\npackage P {\n[X]\n}\n[X] --> [Y]\n[Y] --> [X]\n@enduml",
            "@startuml\nparticipant \"긴 이름 참여자\" as L\nL -> L ++ : go\nreturn done\nnote left : n\n@enduml",
            "@startuml\nentity E {\n *id\n}\nE }o--o{ E\n@enduml",
            "@startuml\nleft to right direction\nactor A\nA --> (UC)\n(UC) --> :B:\n@enduml",
            "@startgantt\n@endgantt",
            "@startgantt\n[a]\n@endgantt",
            "@startgantt\nProject starts 2024-01-01\n[a] requires 10 days then [b] requires 1 week and 2 days\n@endgantt",
            "@startgantt\n[a] -> [b]\n[b] -> [a]\n[a] requires 3 days\n@endgantt",
            "@startgantt\n[a] starts D+14\n[a] starts at [b]'s end\n[b] starts at [a]'s start\n@endgantt",
            "@startgantt\n<style>\n[x] requires 1 day\n@endgantt",
            "@startgantt\ntitle 긴 제목을 여기에 아주 길게 적어 봅니다\n[아주 긴 작업 이름입니다] requires 500 weeks\n@endgantt",
        ];
        for source in mermaid {
            for width in [8usize, 20, 40, 80, 200] {
                let _ = render(Language::Mermaid, source, &Theme::none(), width, DiagramOptions::default());
                let _ = render(Language::Mermaid, source, &Theme::dark(), width, DiagramOptions::default());
            }
        }
        for source in plantuml {
            for width in [8usize, 20, 40, 80, 200] {
                let _ = render(Language::PlantUml, source, &Theme::none(), width, DiagramOptions::default());
            }
        }
        // BPMN(6.1~6.5, 5.8, 1.4, 1.5) — 반드시 `None`인 입력들. 손상된 구조·범위 밖 문법·구조
        // 규칙 위반은 패닉 없이 코드블록으로 물러나야 한다.
        let bpmn_must_be_none: Vec<String> = vec![
            "".into(),
            "   \n  ".into(),
            "<".into(),
            "<definitions/>".into(),
            "<bpmn:definitio".into(),                                   // 태그 중간 잘림
            "<definitions><process>".into(),                            // 요소가 열린 채 끝남
            "<definitions><process></definitions>".into(),              // 닫는 태그 불일치
            r#"<definitions x="unterminated>"#.into(),                  // 속성 따옴표 미종결
            "<!-- unterminated".into(),                                 // 주석 미종결
            "<definitions><![CDATA[unterminated".into(),                // CDATA 미종결
            "<?xml unterminated".into(),                                // 선언 미종결
            format!("{}{}", "<a>".repeat(100), "</a>".repeat(100)),     // 깊이 64 초과(6.4)
            r#"<definitions xmlns="http://schemas.xmlsoap.org/wsdl/"></definitions>"#.into(), // WSDL
            "process:\n  id: p1\n".into(),                              // YAML 조각
            "임의의 한글 문장입니다".into(),                                  // 평문
            // 풀을 넘는 시퀀스 흐름(5.8) — validate가 거부.
            r#"<definitions><collaboration id="c1"><participant id="p1" processRef="pr1"/><participant id="p2" processRef="pr2"/></collaboration><process id="pr1"><task id="a"/></process><process id="pr2"><task id="b"/><sequenceFlow id="f1" sourceRef="a" targetRef="b"/></process></definitions>"#.into(),
            // 중복 id(5.8) — validate가 거부.
            r#"<definitions><process id="p1"><task id="dup"/><task id="dup"/></process></definitions>"#.into(),
            // --- bpmn-yaml(6.1~6.5, 4.10, 5.9) ---
            // 줄 중간에서 잘림 — 아스키 문자열 경계에서 잘라 안전하다.
            {
                let fixture = crate::diagram::bpmn::fixtures::ORDER_PROCESSING_YAML;
                let cut_at = fixture.find("boundary-error").expect("fixture에 있어야 한다");
                fixture[..cut_at + 5].to_string()
            },
            // 블록 중간에서 잘림 — `lane-warehouse:` 블록 전체가 사라진다.
            {
                let fixture = crate::diagram::bpmn::fixtures::ORDER_PROCESSING_YAML;
                let cut_at = fixture.find("lane-warehouse").expect("fixture에 있어야 한다");
                fixture[..cut_at].to_string()
            },
            "# 주석만 있는 문서\n".into(),
            "---\n".into(),
            "nodes:\n  - a:\n      kind: task\n      name: &anchor\n".into(), // 앵커(2.8)
            "nodes:\n\t- a: task\n".into(),                                  // 탭 들여쓰기(2.9)
            "nodes:\n  - a: UnknownKind\n".into(),                           // 모르는 종류(4.8)
            "nodes:\n  - a: task\n  - b: task\nflows:\n  - a-->b\n".into(),  // 화살 앞뒤 공백 없음(5.7)
            "nodes:\n  - a: task\n  - b: task\nflows:\n  - a ==> b\n".into(), // 지원하지 않는 화살(5.6)
            "nodes:\n  - gw:\n      kind: exclusiveGateway\n      default: ghost\n".into(), // default 대상 흐름 없음(4.7)
            // 레인 끝 메시지 흐름(5.9) — validate가 거부.
            "participants:\n  - p1:\n      lanes:\n        - lane1: 레인\n  - p2:\n      nodes:\n        - a: task\nflows:\n  - lane1 -.-> a\n".into(),
            "nodes:\n  - a: task\n  - a: task\n".into(), // 중복 id(4.10) — validate가 거부.
        ];
        for source in &bpmn_must_be_none {
            for width in [8usize, 20, 40, 80, 200] {
                assert_eq!(render(Language::Bpmn, source, &Theme::none(), width, DiagramOptions::default()), None, "{source:?} (width {width})는 None이어야 한다");
            }
        }

        // 패닉만 없으면 되는 입력들(6.6) — 성공·실패 둘 다 허용.
        let bpmn_may_render: Vec<String> = vec![
            crate::diagram::bpmn::fixtures::order_processing_collaboration(),
            crate::diagram::bpmn::fixtures::order_processing_collaboration_without_diagram(),
            r#"<process id="p1"><task id="a" name="&nbsp; 이름"/></process>"#.into(),
            r#"<process id="p1"><task id="a"/><task id="b"/><sequenceFlow sourceRef="a" targetRef="b"/></process>"#.into(), // id 없는 흐름
            r#"<process id="p1"><startEvent id="s"><messageEventDefinition/><timerEventDefinition/></startEvent></process>"#.into(), // 이벤트 정의 2개
            r#"<process id="p1"><startEvent id="s"/><subProcess id="sp"><task id="inner"/><startEvent id="inner-s"/></subProcess><endEvent id="e"/><sequenceFlow id="f1" sourceRef="s" targetRef="sp"/><sequenceFlow id="f2" sourceRef="sp" targetRef="e"/></process>"#.into(), // subProcess 안 노드
            // --- bpmn-yaml fixture 전부(6.6) ---
            crate::diagram::bpmn::fixtures::ORDER_PROCESSING_YAML.into(),
            crate::diagram::bpmn::fixtures::WIDTH_TWO_YAML.into(),
            crate::diagram::bpmn::fixtures::WIDTH_FOUR_YAML.into(),
            crate::diagram::bpmn::fixtures::WIDTH_MIXED_YAML.into(),
            crate::diagram::bpmn::fixtures::QUOTING_AND_TYPES_YAML.into(),
            crate::diagram::bpmn::fixtures::FLAT_NODES_YAML.into(),
            crate::diagram::bpmn::fixtures::NESTED_LANES_YAML.into(),
        ];
        for source in &bpmn_may_render {
            for width in [8usize, 20, 40, 80, 200] {
                let _ = render(Language::Bpmn, source, &Theme::none(), width, DiagramOptions::default());
            }
        }
    }

    #[test]
    fn caption_uses_language_name_and_kind() {
        let line = caption("bpmn", "collaboration", &Theme::none(), 40);
        assert!(line.text().starts_with("◈ bpmn · collaboration "));
    }

    /// tasks 3.2 — BPMN 스니핑이 PlantUML 포괄 판별보다 먼저 온다(design §Key Decisions).
    #[test]
    fn language_of_source_prefers_bpmn_sniffing_over_plantumls_catch_all() {
        let fixture = crate::diagram::bpmn::fixtures::order_processing_collaboration();
        // 확장자·펜스 없는 fixture(1.3).
        assert_eq!(language_of_source(None, &fixture), Some(Language::Bpmn));
        // 확장자 `.bpmn`은 본문 무관(1.2).
        assert_eq!(language_of_source(Some("x.bpmn"), "아무 내용"), Some(Language::Bpmn));
        // WSDL·HTML은 BPMN으로 오판되지 않는다 — 이전과 같은 결과(1.4): PlantUML `kind_of`가
        // 점수 없으면 `Some("class")`를 돌려주는 포괄 판별기라, `bpmn::kind_of`가 늘 `None`이던
        // 이 스펙 이전에도 결국 PlantUML(class)로 잡혔다(이 순서 변경이 새로 만든 결과가 아니다).
        assert_eq!(language_of_source(None, r#"<definitions xmlns="http://schemas.xmlsoap.org/wsdl/"></definitions>"#), Some(Language::PlantUml));
        assert_eq!(language_of_source(None, "<html><body></body></html>"), Some(Language::PlantUml));
    }

    /// tasks 4.2 — 확장자·펜스 없는 BPMN YAML도 BPMN으로 판별되고(1.4), `.yaml`/`.yml` 확장자는
    /// 그 자체로 BPMN을 뜻하지 않는다(1.5, 본문 스니핑을 따른다).
    #[test]
    fn yaml_fixture_without_extension_or_fence_is_recognized_as_bpmn_and_yaml_extensions_defer_to_sniffing() {
        assert_eq!(language_of_source(None, crate::diagram::bpmn::fixtures::ORDER_PROCESSING_YAML), Some(Language::Bpmn));
        assert_eq!(language_of_path("x.yaml"), None);
        assert_eq!(language_of_path("x.yml"), None);
        // 확장자가 판별에 끼어들지 않으므로, 경로가 `.yaml`이어도 본문 스니핑 결과를 따른다.
        assert_eq!(language_of_source(Some("x.yaml"), crate::diagram::bpmn::fixtures::ORDER_PROCESSING_YAML), Some(Language::Bpmn));
        // `.yml` 확장자는 그 자체로 아무 언어도 뜻하지 않으므로, 구조 키 없는 본문은 PlantUML의
        // 포괄 판별(점수 없으면 `Some("class")`)로 떨어진다 — 이 기능 이전과 같은 결과.
        assert_eq!(language_of_source(Some("x.yml"), "그냥 평문"), Some(Language::PlantUml));
    }

    /// tasks 3.2 — 기존 PlantUML·mermaid 판별은 이 스펙 이후에도 그대로다(1.6).
    #[test]
    fn language_of_source_leaves_existing_plantuml_and_mermaid_detection_unchanged() {
        assert_eq!(language_of_source(None, "class A"), Some(Language::PlantUml));
        assert_eq!(language_of_source(None, "flowchart TB\n A --> B"), Some(Language::Mermaid));
        // `title:`만 있고 구조 키(participants·nodes·flows)가 없는 평문은 BPMN YAML로 스니핑되지
        // 않으므로(1.3) 이 기능 이전과 같은 판별 결과로 남는다.
        assert_eq!(language_of_source(None, "title: 문서\n본문"), Some(Language::PlantUml));
        for source in [
            "@startuml\n@enduml",
            "@startuml\nA -> B\n@enduml",
            "@startgantt\n@endgantt",
            "@startuml\nclass A {\n@enduml",
        ] {
            assert_eq!(language_of_source(None, source), Some(Language::PlantUml), "{source:?}는 그대로 PlantUML이어야 한다");
        }
    }
}
