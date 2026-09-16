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

/// 다이어그램을 그린다. 지원하지 않거나 폭에 맞지 않으면 `None`.
pub fn render(language: Language, source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<Vec<Line>> {
    let options = options.with_source(source);
    let (kind, body) = match language {
        Language::Mermaid => mermaid::render(source, theme, width, options)?,
        Language::PlantUml => plantuml::render(source, theme, width, options)?,
    };
    if body.iter().all(Line::is_blank) {
        return None;
    }
    if body.iter().map(Line::width).max().unwrap_or(0) > width {
        return None;
    }
    let name = match language {
        Language::Mermaid => "mermaid",
        Language::PlantUml => "plantuml",
    };
    let mut out = vec![caption(name, kind, theme, width)];
    out.extend(body);
    while out.last().is_some_and(Line::is_blank) {
        out.pop();
    }
    Some(out)
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
