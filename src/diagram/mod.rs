//! 코드블록의 다이어그램 언어를 판별해 그림 줄로 바꾼다.

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
}

/// 코드 펜스의 언어 이름으로 다이어그램 언어를 고른다.
pub fn language_of_fence(lang: &str) -> Option<Language> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "mermaid" | "mmd" => Some(Language::Mermaid),
        "plantuml" | "puml" | "uml" => Some(Language::PlantUml),
        _ => None,
    }
}

/// 파일 확장자나 본문으로 다이어그램 언어를 추정한다.
pub fn language_of_source(path: Option<&str>, source: &str) -> Option<Language> {
    if let Some(p) = path {
        let lower = p.to_ascii_lowercase();
        if lower.ends_with(".puml") || lower.ends_with(".plantuml") || lower.ends_with(".pu") || lower.ends_with(".iuml") {
            return Some(Language::PlantUml);
        }
        if lower.ends_with(".mmd") || lower.ends_with(".mermaid") {
            return Some(Language::Mermaid);
        }
    }
    let trimmed = source.trim_start();
    if trimmed.starts_with("@start") {
        return Some(Language::PlantUml);
    }
    if mermaid::kind_of(source).is_some() {
        return Some(Language::Mermaid);
    }
    if plantuml::kind_of(source).is_some() {
        return Some(Language::PlantUml);
    }
    None
}

/// 다이어그램을 캡션(`◈ mermaid · flowchart ───`)과 함께 그린다. 지원하지 않거나 폭에 맞지 않으면 `None`.
pub fn render(language: Language, source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<Vec<Line>> {
    let (kind, body) = render_body(language, source, theme, width, options)?;
    let name = match language {
        Language::Mermaid => "mermaid",
        Language::PlantUml => "plantuml",
    };
    let mut out = vec![caption(name, kind, theme, width)];
    out.extend(body);
    Some(out)
}

/// 캡션 없이 그림 줄만 돌려준다: (종류 이름, 줄들). 다른 뷰어에 엔진으로 끼울 때 쓴다.
pub fn render_body(language: Language, source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let options = options.with_source(source);
    let (kind, mut body) = match language {
        Language::Mermaid => mermaid::render(source, theme, width, options)?,
        Language::PlantUml => plantuml::render(source, theme, width, options)?,
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
    }
}
