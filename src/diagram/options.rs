//! 다이어그램 렌더링 옵션과 소스 안 지시자.
//!
//! 명령줄 옵션이 기본값이 되고, 소스 안 지시자가 그것을 덮어쓴다. 지시자는 각 언어의 관례를 따른다:
//! - mermaid: `%%{init: {"dg": {"erNotation": "text"}}}%%` 또는 `%% dg: erNotation=text`
//! - PlantUML: `!pragma dg erNotation=text` 또는 `' dg: erNotation=text`

use crate::diagram::ir::{Graph, Marker};

/// ER 관계의 카디널리티 표기.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ErNotation {
    /// 까치발 표식만.
    #[default]
    Crow,
    /// `1`, `0..N` 같은 글자만.
    Text,
    /// 둘 다.
    Both,
}

impl ErNotation {
    pub fn parse(value: &str) -> Option<ErNotation> {
        match value.trim().to_ascii_lowercase().as_str() {
            "crow" | "crowsfoot" | "crows-foot" | "ie" => Some(ErNotation::Crow),
            "text" | "cardinality" | "label" => Some(ErNotation::Text),
            "both" => Some(ErNotation::Both),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DiagramOptions {
    pub er_notation: ErNotation,
}

impl DiagramOptions {
    /// 소스 안 지시자를 읽어 옵션을 덮어쓴 사본.
    pub fn with_source(self, source: &str) -> DiagramOptions {
        let mut options = self;
        for line in source.lines() {
            let trimmed = line.trim();
            let body = if let Some(rest) = trimmed.strip_prefix("%%{init:") {
                // mermaid init 지시자: JSON 안의 "dg" 항목만 본다.
                match rest.find("\"dg\"") {
                    Some(p) => json_pairs(&rest[p..]),
                    None => continue,
                }
            } else if let Some(rest) = trimmed.strip_prefix("%%").and_then(|r| r.trim().strip_prefix("dg:")) {
                key_value_pairs(rest)
            } else if let Some(rest) = trimmed.strip_prefix("!pragma").and_then(|r| r.trim().strip_prefix("dg")) {
                key_value_pairs(rest)
            } else if let Some(rest) = trimmed.strip_prefix('\'').and_then(|r| r.trim().strip_prefix("dg:")) {
                key_value_pairs(rest)
            } else {
                continue;
            };
            for (key, value) in body {
                options.apply(&key, &value);
            }
        }
        options
    }

    fn apply(&mut self, key: &str, value: &str) {
        if normalize(key) == "ernotation"
            && let Some(notation) = ErNotation::parse(value)
        {
            self.er_notation = notation;
        }
    }

    /// 파서가 만든 그래프에 표기 옵션을 적용한다. 파서는 까치발 표식과 글자를 둘 다 만들어 둔다.
    pub fn apply_to_graph(&self, graph: &mut Graph) {
        let is_crow = |m: Marker| matches!(m, Marker::CrowOne | Marker::CrowZeroOne | Marker::CrowMany | Marker::CrowZeroMany);
        for edge in &mut graph.edges {
            match self.er_notation {
                ErNotation::Both => {}
                ErNotation::Crow => {
                    if is_crow(edge.tail) && is_cardinality(&edge.tail_label) {
                        edge.tail_label.clear();
                    }
                    if is_crow(edge.head) && is_cardinality(&edge.head_label) {
                        edge.head_label.clear();
                    }
                }
                ErNotation::Text => {
                    if is_crow(edge.tail) {
                        edge.tail = Marker::None;
                    }
                    if is_crow(edge.head) {
                        edge.head = Marker::None;
                    }
                }
            }
        }
    }
}

fn is_cardinality(text: &str) -> bool {
    matches!(text, "1" | "0..1" | "1..N" | "0..N")
}

/// `erNotation`, `er_notation`, `er-notation`을 같은 키로 본다.
fn normalize(key: &str) -> String {
    key.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// `key=value key2=value2` 꼴.
fn key_value_pairs(text: &str) -> Vec<(String, String)> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter_map(|pair| pair.split_once('=').or_else(|| pair.split_once(':')))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches(['"', '\'']).to_string()))
        .collect()
}

/// `"key": "value"` 쌍을 JSON 조각에서 대충 뽑는다(dg 항목만 보므로 완전한 파서는 필요 없다).
fn json_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let Some(end) = rest[start + 1..].find('"') else { break };
        let key = &rest[start + 1..start + 1 + end];
        rest = &rest[start + 1 + end + 1..];
        let after = rest.trim_start();
        if let Some(value_text) = after.strip_prefix(':') {
            let value_text = value_text.trim_start();
            if let Some(quoted) = value_text.strip_prefix('"')
                && let Some(close) = quoted.find('"')
            {
                pairs.push((key.to_string(), quoted[..close].to_string()));
                rest = &quoted[close + 1..];
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_directives_in_each_syntax() {
        let base = DiagramOptions::default();
        assert_eq!(base.with_source("erDiagram\n%%{init: {\"theme\": \"dark\", \"dg\": {\"erNotation\": \"text\"}}}%%\n").er_notation, ErNotation::Text);
        assert_eq!(base.with_source("%% dg: er_notation=both\nerDiagram").er_notation, ErNotation::Both);
        assert_eq!(base.with_source("@startuml\n!pragma dg erNotation=text\n@enduml").er_notation, ErNotation::Text);
        assert_eq!(base.with_source("' dg: er-notation=crow").er_notation, ErNotation::Crow);
        assert_eq!(base.with_source("erDiagram\nA ||--o{ B : x").er_notation, ErNotation::Crow);
    }
}
