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

const TASK_KINDS: [&str; 2] = ["task", "userTask"];

/// 호스트 작업에 붙는 타이머 경계 이벤트. `cancel_activity`는 원문을 만들 때 정한다.
struct GeneratedBoundary {
    id: String,
    name: Option<&'static str>,
    host: String,
}

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
    boundaries: Vec<GeneratedBoundary>,
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
        GeneratedDocument { participant_count, nodes, flow_ends, boundaries: Vec::new() }
    }

    /// 기존 생성 결과에 경계 이벤트 1~2개를 얹는다. 호스트는 작업 노드이고(없으면 첫 노드를 작업으로 바꾼다),
    /// 나가는 흐름은 호스트와 같은 참여자의 다른 노드로만 이어 풀을 가로지르는 흐름 오류를 피한다.
    fn generate_with_boundaries(rng: &mut Xorshift64) -> Self {
        let mut document = Self::generate(rng);
        if !document.nodes.iter().any(|node| TASK_KINDS.contains(&node.kind)) {
            document.nodes[0].kind = TASK_KINDS[0];
        }
        let hosts: Vec<usize> = (0..document.nodes.len()).filter(|&i| TASK_KINDS.contains(&document.nodes[i].kind)).collect();
        for boundary_index in 0..1 + rng.below(2) {
            let host = &document.nodes[hosts[rng.below(hosts.len())]];
            let boundary = GeneratedBoundary {
                id: format!("b{boundary_index}"),
                name: if rng.below(2) == 0 { None } else { Some(NODE_NAMES[rng.below(NODE_NAMES.len())]) },
                host: host.id.clone(),
            };
            let targets: Vec<&GeneratedNode> = document.nodes.iter().filter(|node| node.id != host.id && node.participant == host.participant).collect();
            if !targets.is_empty() {
                let target = targets[rng.below(targets.len())].id.clone();
                document.flow_ends.push((boundary.id.clone(), target));
            }
            document.boundaries.push(boundary);
        }
        document
    }

    fn node_line(node: &GeneratedNode, indent: &str) -> String {
        match node.name {
            Some(name) => format!("{indent}- {}: {} {name}\n", node.id, node.kind),
            None => format!("{indent}- {}: {}\n", node.id, node.kind),
        }
    }

    /// 흐름마다 `arrows[i]`를 화살 표기로 쓴 YAML 원문.
    fn to_yaml(&self, arrows: &[&str]) -> String {
        self.to_yaml_with_cancel_activity(arrows, None)
    }

    /// `to_yaml`에 경계 이벤트마다 `cancel_activity`를 `cancel_activity` 값으로 적는다(`None`이면 키를 생략).
    fn to_yaml_with_cancel_activity(&self, arrows: &[&str], cancel_activity: Option<bool>) -> String {
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
        if !loose.is_empty() || !self.boundaries.is_empty() {
            yaml.push_str("nodes:\n");
            for node in loose {
                yaml.push_str(&Self::node_line(node, "  "));
            }
            for boundary in &self.boundaries {
                yaml.push_str(&format!("  - {}:\n      kind: boundaryEvent\n      trigger: timerEventDefinition\n      attached_to: {}\n", boundary.id, boundary.host));
                if let Some(name) = boundary.name {
                    yaml.push_str(&format!("      name: {name}\n"));
                }
                if let Some(is_interrupting) = cancel_activity {
                    yaml.push_str(&format!("      cancel_activity: {is_interrupting}\n"));
                }
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

/// 커버리지 허용 목록 밖 글자(ASCII·한글·기존 건너뛰기 글자 제외), 정렬·중복 제거.
fn uncovered_glyphs(lines: &[Line]) -> Vec<char> {
    let mut uncovered: Vec<char> = lines.iter().flat_map(|line| line.text().chars().collect::<Vec<_>>()).filter(|&c| !c.is_ascii() && !is_hangul(c) && c != PREEXISTING_UNCOVERED_HOP && !CJK_MONO_COVERED_GLYPHS.contains(&c)).collect();
    uncovered.sort_unstable();
    uncovered.dedup();
    uncovered
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
            let uncovered = uncovered_glyphs(lines);
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

fn plain_text_of(rendered: &Option<(&'static str, Vec<Line>)>) -> Option<String> {
    rendered.as_ref().map(|(_, lines)| lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n"))
}

#[test]
fn omitting_or_true_cancel_activity_renders_the_same_and_false_only_turns_the_boundary_glyph_into_dotted() {
    let mut rendered_count = 0;
    let mut smallest_failure: Option<(usize, String)> = None;
    for iteration in 0..ITERATIONS {
        let seed = BASE_SEED + iteration;
        let mut rng = Xorshift64::new(seed);
        let document = GeneratedDocument::generate_with_boundaries(&mut rng);
        let arrows = document.uniform_arrows(ACCEPTED_ARROWS[0]);
        let absent_source = document.to_yaml_with_cancel_activity(&arrows, None);
        let true_source = document.to_yaml_with_cancel_activity(&arrows, Some(true));
        let false_source = document.to_yaml_with_cancel_activity(&arrows, Some(false));
        let mut is_rendered = false;
        for direction in [Direction::TopDown, Direction::LeftRight] {
            let absent = render_text(&absent_source, Some((direction, false)));
            let explicit_true = render_text(&true_source, Some((direction, false)));
            let non_interrupting = render_text(&false_source, Some((direction, false)));
            is_rendered |= absent.is_some();
            let absent_text = plain_text_of(&absent);
            let non_interrupting_as_interrupting = plain_text_of(&non_interrupting).map(|text| text.replace('◌', "◎"));
            let has_dotted_glyph_per_boundary = absent_text.is_none() || plain_text_of(&non_interrupting).is_some_and(|text| text.matches('◌').count() == document.boundaries.len());
            let is_violated = explicit_true != absent || non_interrupting_as_interrupting != absent_text || !has_dotted_glyph_per_boundary;
            if is_violated && smallest_failure.as_ref().is_none_or(|(size, _)| absent_source.len() < *size) {
                let report = format!(
                    "seed={seed:#x} iteration={iteration} direction={direction:?}\n--- cancel_activity 생략 YAML\n{absent_source}--- 생략 렌더링\n{}\n--- cancel_activity: true YAML\n{true_source}--- true 렌더링\n{}\n--- cancel_activity: false YAML\n{false_source}--- false 렌더링\n{}",
                    describe(&absent),
                    describe(&explicit_true),
                    describe(&non_interrupting)
                );
                smallest_failure = Some((absent_source.len(), report));
            }
        }
        if is_rendered {
            rendered_count += 1;
        }
    }
    if let Some((_, report)) = smallest_failure {
        panic!("경계 이벤트의 cancel_activity 생략·true·false 렌더링 관계가 어긋났다(가장 작은 실패):\n{report}");
    }
    assert_mostly_rendered(rendered_count, "경계 이벤트 방해 여부");
}

#[test]
fn non_interrupting_boundary_rendering_draws_only_glyphs_within_cjk_mono_coverage() {
    let mut rendered_count = 0;
    let mut smallest_failure: Option<(usize, String)> = None;
    for iteration in 0..ITERATIONS {
        let seed = BASE_SEED + iteration;
        let mut rng = Xorshift64::new(seed);
        let document = GeneratedDocument::generate_with_boundaries(&mut rng);
        let source = document.to_yaml_with_cancel_activity(&document.uniform_arrows(ACCEPTED_ARROWS[0]), Some(false));
        let mut is_rendered = false;
        for direction in [Direction::TopDown, Direction::LeftRight] {
            let rendered = render_text(&source, Some((direction, false)));
            let Some((_, lines)) = &rendered else { continue };
            is_rendered = true;
            let uncovered = uncovered_glyphs(lines);
            let has_dotted_glyph = lines.iter().any(|line| line.text().contains('◌'));
            if (!uncovered.is_empty() || !has_dotted_glyph) && smallest_failure.as_ref().is_none_or(|(size, _)| source.len() < *size) {
                let codepoints = uncovered.iter().map(|c| format!("{c:?}(U+{:04X})", *c as u32)).collect::<Vec<_>>().join(", ");
                let report = format!("seed={seed:#x} iteration={iteration} direction={direction:?}\n허용 목록 밖 글자: {codepoints} / ◌ 있음: {has_dotted_glyph}\n--- YAML\n{source}--- 렌더링\n{}", describe(&rendered));
                smallest_failure = Some((source.len(), report));
            }
        }
        if is_rendered {
            rendered_count += 1;
        }
    }
    if let Some((_, report)) = smallest_failure {
        panic!("비방해 경계 이벤트가 있는 BPMN 렌더링에 커버리지 허용 목록 밖 글자가 있거나 ◌가 없다(가장 작은 실패):\n{report}");
    }
    assert_mostly_rendered(rendered_count, "비방해 경계 이벤트 커버리지");
}
