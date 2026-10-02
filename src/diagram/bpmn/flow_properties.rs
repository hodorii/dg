//! YAML 흐름 표기와 BPMN 렌더링 글자에 대한 속성 테스트. 입력은 seed로 고정한 무작위 문서다.

use crate::diagram::bpmn::render;
use crate::diagram::ir::Direction;
use crate::diagram::layout::graph::Xorshift64;
use crate::diagram::layout::shape::CJK_MONO_COVERED_GLYPHS;
use crate::diagram::options::DiagramOptions;
use crate::line::Line;
use crate::style::Theme;

const ACCEPTED_ARROWS: [&str; 3] = ["-->", "-.->", "-.-"];
const NODE_KINDS: [&str; 7] = ["task", "userTask", "startEvent", "endEvent", "exclusiveGateway", "dataObjectReference", "textAnnotation"];
const NODE_NAMES: [&str; 4] = ["검토", "Ship order", "승인 요청", "X"];
const RENDER_WIDTH: usize = 100;
const ITERATIONS: u64 = 200;
const BASE_SEED: u64 = 0x5EED_F10E;
/// 간선 교차 건너뛰기 글자(`canvas::HOP`)는 커버리지 밖이지만, 모든 그래프 다이어그램이 함께 쓰는 기존
/// 규칙이고 이 기능의 설계가 그대로 둔다(건너뛰기 글자는 선 무늬와 무관). 커버리지 판정에서 이것만 뺀다.
const PREEXISTING_UNCOVERED_HOP: char = '◠';

struct GeneratedNode {
    id: String,
    kind: &'static str,
    name: Option<&'static str>,
    participant: Option<usize>,
}

/// 참여자 0~2, 노드 2~5, 흐름 1~2개. 흐름 끝은 서로 다른 두 대상(노드, 참여자가 있으면 가끔 풀 자체)이다.
struct GeneratedDocument {
    participant_count: usize,
    nodes: Vec<GeneratedNode>,
    flow_ends: Vec<(String, String)>,
}

impl GeneratedDocument {
    fn generate(rng: &mut Xorshift64) -> Self {
        let participant_count = rng.below(3);
        let node_count = 2 + rng.below(4);
        let nodes = (0..node_count)
            .map(|i| GeneratedNode {
                id: format!("n{i}"),
                kind: NODE_KINDS[rng.below(NODE_KINDS.len())],
                name: if rng.below(2) == 0 { None } else { Some(NODE_NAMES[rng.below(NODE_NAMES.len())]) },
                participant: match rng.below(participant_count + 1) {
                    index if index < participant_count => Some(index),
                    _ => None,
                },
            })
            .collect::<Vec<_>>();
        let mut endpoints: Vec<String> = nodes.iter().map(|node| node.id.clone()).collect();
        if participant_count > 0 && rng.below(4) == 0 {
            endpoints.extend((0..participant_count).map(|p| format!("p{p}")));
        }
        let flow_count = 1 + rng.below(2);
        let flow_ends = (0..flow_count)
            .map(|_| {
                let source = rng.below(endpoints.len());
                let target = (source + 1 + rng.below(endpoints.len() - 1)) % endpoints.len();
                (endpoints[source].clone(), endpoints[target].clone())
            })
            .collect();
        GeneratedDocument { participant_count, nodes, flow_ends }
    }

    fn node_line(node: &GeneratedNode, indent: &str) -> String {
        match node.name {
            Some(name) => format!("{indent}- {}: {} {name}\n", node.id, node.kind),
            None => format!("{indent}- {}: {}\n", node.id, node.kind),
        }
    }

    /// 흐름마다 `arrows[i]`를 화살 표기로 쓴 YAML 원문.
    fn to_yaml(&self, arrows: &[&str]) -> String {
        let mut yaml = String::new();
        if self.participant_count > 0 {
            yaml.push_str("participants:\n");
            for p in 0..self.participant_count {
                let members: Vec<&GeneratedNode> = self.nodes.iter().filter(|node| node.participant == Some(p)).collect();
                if members.is_empty() {
                    yaml.push_str(&format!("  - p{p}: Pool {p}\n"));
                } else {
                    yaml.push_str(&format!("  - p{p}:\n      nodes:\n"));
                    for node in members {
                        yaml.push_str(&Self::node_line(node, "        "));
                    }
                }
            }
        }
        let loose: Vec<&GeneratedNode> = self.nodes.iter().filter(|node| node.participant.is_none()).collect();
        if !loose.is_empty() {
            yaml.push_str("nodes:\n");
            for node in loose {
                yaml.push_str(&Self::node_line(node, "  "));
            }
        }
        yaml.push_str("flows:\n");
        for ((source, target), arrow) in self.flow_ends.iter().zip(arrows) {
            yaml.push_str(&format!("  - {source} {arrow} {target}\n"));
        }
        yaml
    }

    fn uniform_arrows(&self, arrow: &'static str) -> Vec<&'static str> {
        vec![arrow; self.flow_ends.len()]
    }
}

fn render_text(source: &str, direction: Option<(Direction, bool)>) -> Option<(&'static str, Vec<Line>)> {
    render(source, &Theme::none(), RENDER_WIDTH, DiagramOptions { direction, ..DiagramOptions::default() })
}

fn describe(rendered: &Option<(&'static str, Vec<Line>)>) -> String {
    match rendered {
        None => "None(원문 코드블록으로 물러남)".to_string(),
        Some((kind, lines)) => format!("{kind}\n{}", lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n")),
    }
}

/// 속성이 공허하지 않도록 생성한 문서의 절반 이상은 실제로 그려져야 한다.
fn assert_mostly_rendered(rendered_count: u64, property: &str) {
    assert!(rendered_count * 2 >= ITERATIONS, "{property}: 생성 문서 {ITERATIONS}개 중 {rendered_count}개만 렌더링됐다 — 생성기가 너무 많은 문서를 규칙 위반으로 만든다");
}

#[test]
fn the_three_accepted_arrows_render_the_same_diagram_for_the_same_ends() {
    let mut rendered_count = 0;
    let mut smallest_failure: Option<(usize, String)> = None;
    for iteration in 0..ITERATIONS {
        let seed = BASE_SEED + iteration;
        let mut rng = Xorshift64::new(seed);
        let document = GeneratedDocument::generate(&mut rng);
        let direction = [None, Some((Direction::TopDown, false)), Some((Direction::LeftRight, false))][rng.below(3)];
        let mixed: Vec<&str> = document.flow_ends.iter().map(|_| ACCEPTED_ARROWS[rng.below(ACCEPTED_ARROWS.len())]).collect();
        let baseline_source = document.to_yaml(&document.uniform_arrows(ACCEPTED_ARROWS[0]));
        let baseline = render_text(&baseline_source, direction);
        if baseline.is_some() {
            rendered_count += 1;
        }
        let variants = ACCEPTED_ARROWS[1..].iter().map(|&arrow| document.uniform_arrows(arrow)).chain([mixed]);
        for arrows in variants {
            let variant_source = document.to_yaml(&arrows);
            let variant = render_text(&variant_source, direction);
            if variant != baseline && smallest_failure.as_ref().is_none_or(|(size, _)| baseline_source.len() < *size) {
                let report = format!(
                    "seed={seed:#x} iteration={iteration} direction={direction:?}\n--- 기준 YAML\n{baseline_source}--- 기준 렌더링\n{}\n--- 변형 YAML\n{variant_source}--- 변형 렌더링\n{}",
                    describe(&baseline),
                    describe(&variant)
                );
                smallest_failure = Some((baseline_source.len(), report));
            }
        }
    }
    if let Some((_, report)) = smallest_failure {
        panic!("같은 두 끝을 다른 화살 표기로 쓴 YAML의 렌더링이 다르다(가장 작은 실패):\n{report}");
    }
    assert_mostly_rendered(rendered_count, "화살 표기 무관");
}

fn is_hangul(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
}

#[test]
fn bpmn_rendering_draws_only_glyphs_within_cjk_mono_coverage() {
    let mut rendered_count = 0;
    let mut smallest_failure: Option<(usize, String)> = None;
    for iteration in 0..ITERATIONS {
        let seed = BASE_SEED + iteration;
        let mut rng = Xorshift64::new(seed);
        let document = GeneratedDocument::generate(&mut rng);
        let arrows: Vec<&str> = document.flow_ends.iter().map(|_| ACCEPTED_ARROWS[rng.below(ACCEPTED_ARROWS.len())]).collect();
        let source = document.to_yaml(&arrows);
        let mut is_rendered = false;
        for direction in [Direction::TopDown, Direction::LeftRight] {
            let rendered = render_text(&source, Some((direction, false)));
            let Some((_, lines)) = &rendered else { continue };
            is_rendered = true;
            let mut uncovered: Vec<char> = lines.iter().flat_map(|line| line.text().chars().collect::<Vec<_>>()).filter(|&c| !c.is_ascii() && !is_hangul(c) && c != PREEXISTING_UNCOVERED_HOP && !CJK_MONO_COVERED_GLYPHS.contains(&c)).collect();
            uncovered.sort_unstable();
            uncovered.dedup();
            if !uncovered.is_empty() && smallest_failure.as_ref().is_none_or(|(size, _)| source.len() < *size) {
                let codepoints = uncovered.iter().map(|c| format!("{c:?}(U+{:04X})", *c as u32)).collect::<Vec<_>>().join(", ");
                let report = format!("seed={seed:#x} iteration={iteration} direction={direction:?}\n허용 목록 밖 글자: {codepoints}\n--- YAML\n{source}--- 렌더링\n{}", describe(&rendered));
                smallest_failure = Some((source.len(), report));
            }
        }
        if is_rendered {
            rendered_count += 1;
        }
    }
    if let Some((_, report)) = smallest_failure {
        panic!("BPMN 렌더링에 커버리지 허용 목록 밖 글자가 있다(가장 작은 실패):\n{report}");
    }
    assert_mostly_rendered(rendered_count, "커버리지 허용 목록");
}
