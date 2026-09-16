//! PlantUML 문법 파서 모음.

pub mod class;
pub mod component;
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

/// 본문을 훑어 종류를 추정한다: sequence · class · er · component.
pub fn kind_of(source: &str) -> Option<&'static str> {
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
    if sequence_score == 0 && class_score == 0 && component_score == 0 {
        return None;
    }
    if sequence_score > class_score && sequence_score > component_score {
        return Some("sequence");
    }
    if component_score > class_score {
        return Some("component");
    }
    Some(if entity_count > 0 && class_count == 0 { "er" } else { "class" })
}

pub fn render(source: &str, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    let kind = kind_of(source)?;
    let body = match kind {
        "sequence" => layout::sequence::render(&sequence::parse(source), theme, width)?,
        "component" => layout::graph::render(&component::parse(source), theme, width)?,
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
        assert_eq!(kind_of("@startuml\n@enduml"), None);
    }
}
