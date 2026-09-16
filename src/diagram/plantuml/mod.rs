//! PlantUML 문법 파서 모음.

pub mod class;
pub mod component;
pub mod gantt;
pub mod relation;
pub mod sequence;
pub mod text;

use crate::diagram::layout;
use crate::diagram::options::DiagramOptions;
use crate::line::Line;
use crate::style::Theme;

const SEQUENCE_ONLY: &[&str] = &[
    "participant", "activate", "deactivate", "return", "alt", "else", "opt", "loop", "par", "break", "critical", "group",
    "autonumber", "ref", "hnote", "rnote", "destroy", "create", "box",
];
const CLASS_ONLY: &[&str] = &["class", "abstract", "enum", "annotation", "object", "struct", "protocol", "namespace", "exception"];
const COMPONENT_ONLY: &[&str] = &[
    "component", "package", "node", "cloud", "folder", "frame", "rectangle", "usecase", "storage", "card", "file", "artifact",
    "hexagon", "agent", "stack", "label", "person", "together",
];

/// 본문을 훑어 종류를 추정한다: gantt · sequence · class · er · component. 뚜렷한 신호가 없으면
/// class로 본다(모르는 키워드는 무시하고 클래스 다이어그램 취급).
pub fn kind_of(source: &str) -> Option<&'static str> {
    if let Some(kind) = kind_of_start_tag(source) {
        return Some(kind);
    }
    let lines = text::clean_lines(source);
    let mut sequence_score = 0;
    let mut class_score = 0;
    let mut component_score = 0;
    let mut entity_count = 0;
    let mut class_count = 0;
    for line in &lines {
        let trimmed = line.trim();
        let first = text::keyword(trimmed).to_ascii_lowercase();
        let first = first.as_str();
        let has_brace = trimmed.ends_with('{');
        if SEQUENCE_ONLY.contains(&first) {
            sequence_score += 2;
        } else if CLASS_ONLY.contains(&first) || (first == "entity" && has_brace) {
            class_score += 3;
            if first == "entity" {
                entity_count += 1;
            } else {
                class_count += 1;
            }
        } else if COMPONENT_ONLY.contains(&first) {
            component_score += 3;
        } else if first == "interface" {
            class_score += 1;
            component_score += 1;
            class_count += 1;
        }
        if trimmed.starts_with("==") || trimmed.starts_with("...") {
            sequence_score += 2;
        }
        if trimmed.starts_with('[') || (trimmed.contains("[") && trimmed.contains("]") && !trimmed.contains("-[")) {
            component_score += 2;
        }
        if sequence::looks_like_message(trimmed) {
            let is_single_dash = !trimmed.contains("--") && !trimmed.contains("..") && trimmed.contains("->") || trimmed.contains("<-") && !trimmed.contains("<--");
            if is_single_dash {
                sequence_score += 3;
            } else {
                sequence_score += 1;
                class_score += 1;
                component_score += 1;
            }
        }
        if relation::contains_relation(trimmed) {
            let has_class_marker = ["|>", "<|", "*-", "-*", " o-", "-o ", "||", "}|", "|{", "}o", "o{"].iter().any(|m| trimmed.contains(m));
            if has_class_marker {
                class_score += 3;
            }
        }
    }
    if sequence_score > class_score && sequence_score > component_score {
        return Some("sequence");
    }
    if component_score > class_score {
        return Some("component");
    }
    // 어떤 점수도 없으면(모르는 키워드뿐이라 아무 갈래로도 못 잡히면) 클래스로 본다. 클래스 파서는
    // 알아보지 못하는 줄을 조용히 건너뛰므로, 진짜 빈 내용이면 결국 layout::graph가 빈 그래프를
    // 보고 `None`을 돌려줘 코드블록으로 물러난다 — 억지로 클래스를 지어내지 않는다.
    Some(if entity_count > 0 && class_count == 0 { "er" } else { "class" })
}

/// 여는 태그 하나로 종류가 정해지는 다이어그램.
///
/// PlantUML은 다이어그램 갈래마다 `@start<종류>` 태그가 따로 있고, 그중 `@startuml`만 여러 갈래를
/// 담는 일반 태그라 위 휴리스틱 점수 매기기가 필요하다. `@startgantt`처럼 갈래를 못 박는 태그는
/// 점수를 매기면 오히려 틀린다. 간트의 작업 줄 `[작업] requires 10 days`가 `[`로 시작해 컴포넌트
/// 점수를 올리기 때문이다. 그래서 태그를 먼저 보고 곧장 끊는다.
fn kind_of_start_tag(source: &str) -> Option<&'static str> {
    let first = source.lines().map(str::trim).find(|line| !line.is_empty() && !line.starts_with('\''))?;
    let tag: String = first.strip_prefix("@start")?.chars().take_while(char::is_ascii_alphanumeric).collect();
    match tag.to_ascii_lowercase().as_str() {
        "gantt" => Some("gantt"),
        _ => None,
    }
}

pub fn render(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let kind = kind_of(source)?;
    let body = match kind {
        "gantt" => layout::gantt::render(&gantt::parse(source), theme, width)?,
        "sequence" => layout::sequence::render(&sequence::parse(source), theme, width)?,
        "component" => {
            let mut graph = component::parse(source);
            options.apply_to_graph(&mut graph);
            layout::graph::render(&graph, theme, width)?
        }
        _ => {
            let mut graph = class::parse(source);
            options.apply_to_graph(&mut graph);
            layout::graph::render(&graph, theme, width)?
        }
    };
    Some((kind, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        assert_eq!(kind_of("@startuml\nAlice -> Bob : hi\n@enduml"), Some("sequence"));
        assert_eq!(kind_of("@startuml\nclass A\nA <|-- B\n@enduml"), Some("class"));
        assert_eq!(kind_of("@startuml\nentity E {\n *id\n}\nE ||--o{ F\n@enduml"), Some("er"));
        assert_eq!(kind_of("@startuml\n[Web] --> [API]\ndatabase DB\n[API] --> DB\n@enduml"), Some("component"));
        // 뚜렷한 신호가 없으면 class로 본다(모르는 키워드 취급). 그래도 내용이 진짜 비어 있으면
        // class::parse가 빈 그래프를 만들고, layout::graph::render가 그걸 보고 None을 돌려주므로
        // render()는 여전히 코드블록으로 물러난다 — kind_of만 보면 이 차이가 안 드러난다.
        assert_eq!(kind_of("@startuml\n@enduml"), Some("class"));
        assert!(render("@startuml\n@enduml", &Theme::none(), 80, DiagramOptions::default()).is_none());
    }

    // 모르는 다이어그램 갈래(스윔레인 등)를 만나도 코드블록으로 포기하지 않고 클래스로 시도한다.
    #[test]
    fn unrecognized_content_falls_back_to_class_instead_of_giving_up() {
        assert_eq!(kind_of("@startuml\nAlice : does something\n@enduml"), Some("class"));
        let (kind, _) = render("@startuml\nAlice : does something\n@enduml", &Theme::none(), 80, DiagramOptions::default()).unwrap();
        assert_eq!(kind, "class");
    }

    // 6.1 `@startgantt`는 본문이 `[`로 시작하는 줄 투성이라도 휴리스틱을 거치지 않고 gantt다.
    #[test]
    fn start_gantt_tag_wins_over_the_heuristic() {
        let source = "@startgantt\nProject starts 2020-07-01\n[Task1] requires 10 days\n[Task1] -> [Task2]\n@endgantt";
        assert_eq!(kind_of(source), Some("gantt"));
        // 같은 본문을 일반 태그로 감싸면 예전대로 휴리스틱이 컴포넌트로 읽는다(태그만 보고 끊는다).
        let generic = source.replace("gantt", "uml");
        assert_eq!(kind_of(&generic), Some("component"));
        // 태그 앞에 주석·빈 줄이 있어도 찾는다.
        assert_eq!(kind_of("\n' 메모\n@startgantt\n[T] requires 1 day\n@endgantt"), Some("gantt"));
    }
}
