//! mermaid `erDiagram` 파서.

use super::text::{clean_lines, label};
use crate::diagram::ir::{crow_marker, Edge, Graph, LineKind, Shape};

pub fn parse(source: &str) -> Graph {
    let mut graph = Graph::default();
    let mut open_entity: Option<usize> = None;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        if let Some(index) = open_entity {
            if trimmed.starts_with('}') {
                open_entity = None;
            } else {
                add_attribute(&mut graph, index, trimmed);
            }
            continue;
        }
        if trimmed.to_ascii_lowercase().starts_with("erdiagram") || trimmed.starts_with("direction") {
            continue;
        }
        if let Some(head) = trimmed.strip_suffix('{') {
            let index = declare(&mut graph, head.trim());
            open_entity = Some(index);
            continue;
        }
        // 관계선(`--`/`..` 포함)이 아니면 관계 없이 개체 하나만 선언하는 줄이다
        // (`ID["라벨"]` 단독) — parse_relation은 관계 연산자를 전제로 하므로 여기서 먼저
        // declare()로 보낸다(diagram-er-standalone-alias). 대괄호 라벨은 공백을 포함할 수
        // 있어(예: "회원입학선발점수이력 (mbr_ams_score_hst)") 판정에서 제외한다.
        if is_relation_line(trimmed) {
            parse_relation(&mut graph, trimmed);
        } else {
            declare(&mut graph, trimmed);
        }
    }
    graph
}

/// 대괄호 `[...]` 안의 내용을 지우고 남은 부분에 관계 연산자(`--`/`..`)가 있으면 관계선으로
/// 판정한다. 라벨 안에 우연히 그 문자열이 들어 있어도(드묾) 대괄호 밖만 보므로 오판하지 않는다.
fn is_relation_line(line: &str) -> bool {
    let mut outside_brackets = String::with_capacity(line.len());
    let mut depth = 0u32;
    for c in line.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => outside_brackets.push(c),
            _ => {}
        }
    }
    outside_brackets.contains("--") || outside_brackets.contains("..")
}

fn declare(graph: &mut Graph, raw: &str) -> usize {
    // `CUSTOMER["Customer"]`
    let (id, alias) = match raw.find('[') {
        Some(p) => (raw[..p].trim(), Some(label(raw[p + 1..].trim_end_matches(']')))),
        None => (raw.trim(), None),
    };
    let index = graph.intern(id, alias.as_deref().unwrap_or(id), Shape::Rect, None);
    // `intern`은 노드를 처음 만들 때만 라벨을 쓴다. 같은 아이디가 별칭 없이 먼저 나온 뒤에도
    // 별칭이 항상 반영되도록, 별칭이 있는 호출은 기존 노드 여부와 무관하게 라벨을 다시 쓴다.
    if let Some(text) = &alias {
        graph.set_label(index, text);
    }
    index
}

/// `string name PK "comment"` → `name : string [PK]`
fn add_attribute(graph: &mut Graph, index: usize, raw: &str) {
    let without_comment = match raw.find('"') {
        Some(p) => raw[..p].trim(),
        None => raw.trim(),
    };
    let mut words = without_comment.split_whitespace();
    let Some(type_name) = words.next() else { return };
    let Some(name) = words.next() else { return };
    let keys: Vec<&str> = words.collect();
    let mut text = format!("{name} : {type_name}");
    if !keys.is_empty() {
        text.push_str(&format!(" [{}]", keys.join(",")));
    }
    let node = &mut graph.nodes[index];
    if node.sections.len() < 2 {
        node.sections.push(Vec::new());
    }
    node.sections[1].push(text);
}

fn cardinality(token: &str) -> &'static str {
    match token {
        "||" => "1",
        "|o" | "o|" => "0..1",
        "}|" | "|{" => "1..N",
        "}o" | "o{" => "0..N",
        _ => "",
    }
}

fn parse_relation(graph: &mut Graph, line: &str) {
    let (body, text) = match line.split_once(':') {
        Some((b, t)) => (b, label(t)),
        None => (line, String::new()),
    };
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.len() < 3 {
        return;
    }
    let relation = words[1];
    let Some(line_start) = relation.find("--").or_else(|| relation.find("..")) else { return };
    let dashed = relation[line_start..].starts_with("..");
    let (left, right) = (&relation[..line_start], &relation[line_start + 2..]);
    let a = declare(graph, words[0]);
    let b = declare(graph, &words[2..].join(" "));
    graph.add_edge(Edge {
        from: a,
        to: b,
        label: text,
        // 표식과 글자를 둘 다 만들어 두고, 표기 옵션이 하나를 고른다.
        tail_label: cardinality(left).to_string(),
        head_label: cardinality(right).to_string(),
        kind: if dashed { LineKind::Dashed } else { LineKind::Solid },
        tail: crow_marker(cardinality(left)),
        head: crow_marker(cardinality(right)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities_and_relations() {
        let g = parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER {\n  string name PK\n  int age\n }\n");
        assert_eq!(g.nodes[0].sections[1], vec!["name : string [PK]", "age : int"]);
        assert_eq!(g.edges[0].tail, crate::diagram::ir::Marker::CrowOne);
        assert_eq!(g.edges[0].head, crate::diagram::ir::Marker::CrowZeroMany);
        assert_eq!(g.edges[0].label, "places");
        // 3.2: 별칭이 전혀 없으면 아이디가 그대로 라벨이다.
        assert_eq!(g.nodes[0].sections[0], vec!["CUSTOMER"]);
    }

    // 2.1: 별칭이 첫 등장인 경우는 이미 정상 동작한다(회귀 방지 대조군).
    #[test]
    fn alias_on_first_appearance_is_shown() {
        let g = parse("erDiagram\n CUSTOMER[\"고객\"] ||--o{ ORDER : places\n");
        assert_eq!(g.nodes[0].sections[0], vec!["고객"]);
    }

    // 1.1/2.1: 별칭 없는 관계가 먼저 오고, 별칭 있는 속성 블록이 나중에 와도 별칭이 반영돼야 한다.
    #[test]
    fn alias_in_later_attribute_block_overrides_earlier_bare_reference() {
        let g = parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER[\"고객\"] {\n  string name\n }\n");
        assert_eq!(g.nodes[0].sections[0], vec!["고객"]);
    }

    // 1.2/2.2: 별칭 없는 관계가 먼저 오고, 별칭 있는 관계가 나중에 같은 아이디를 참조해도 반영돼야 한다.
    #[test]
    fn alias_in_later_relation_overrides_earlier_bare_reference() {
        let g = parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n CUSTOMER[\"고객\"] ||--o{ INVOICE : has\n");
        let customer = g.find("CUSTOMER").unwrap();
        assert_eq!(g.nodes[customer].sections[0], vec!["고객"]);
    }

    // 1.1/2.1(diagram-er-standalone-alias): 관계선·속성 블록 없이 별칭만 단독으로 선언한
    // 줄도 반영돼야 한다 — 그 줄이 parse_relation으로 잘못 넘어가 통째로 무시되던 결함.
    #[test]
    fn standalone_declaration_alias_is_shown() {
        let g = parse("erDiagram\n CUSTOMER[\"Customer\"]\n CUSTOMER ||--o{ ORDER : places\n");
        let customer = g.find("CUSTOMER").unwrap();
        assert_eq!(g.nodes[customer].sections[0], vec!["Customer"]);
    }

    // 1.2/2.2: 라벨에 공백이 섞인 단독 선언도 반영돼야 한다(실제 재현 사례:
    // `/home/hs/w/.kiro/reference/schemas_diagram.md`의 회원 도메인 ERD).
    #[test]
    fn standalone_declaration_alias_with_spaces_is_shown() {
        let g = parse(
            "erDiagram\n mbr_ams_score_hst[\"회원입학선발점수이력 (mbr_ams_score_hst)\"]\n mbr_role_rel[\"회원역활관계\"]\n mbr_role_rel ||--o{ mbr_ams_score_hst : \"rel\"\n",
        );
        let hst = g.find("mbr_ams_score_hst").unwrap();
        assert_eq!(g.nodes[hst].sections[0], vec!["회원입학선발점수이력 (mbr_ams_score_hst)"]);
        let role = g.find("mbr_role_rel").unwrap();
        assert_eq!(g.nodes[role].sections[0], vec!["회원역활관계"]);
    }

    // 3.1~3.3(불변): 관계선 인라인 별칭·속성 블록 별칭·별칭 없는 관계선은 여전히 그대로다.
    #[test]
    fn inline_relation_and_attribute_block_aliases_still_work_without_regression() {
        let g = parse("erDiagram\n CUSTOMER[\"Customer\"] ||--o{ ORDER : places\n ORDER[\"Order\"] {\n  string id\n }\n");
        assert_eq!(g.nodes[0].sections[0], vec!["Customer"]);
        let order = g.find("ORDER").unwrap();
        assert_eq!(g.nodes[order].sections[0], vec!["Order"]);

        let g2 = parse("erDiagram\n CUSTOMER ||--o{ ORDER : places\n");
        assert_eq!(g2.nodes[0].sections[0], vec!["CUSTOMER"]);
    }
}
