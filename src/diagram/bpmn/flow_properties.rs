//! YAML 흐름 표기와 BPMN 렌더링 글자에 대한 속성 테스트. 입력은 seed로 고정한 무작위 문서다.

use crate::diagram::bpmn::render;
use crate::diagram::ir::Direction;
use crate::diagram::layout::graph::{Xorshift64, fnv1a};
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

/// 풀 하나의 모양. 레인은 하위 레인 수(0이면 그 레인 자체가 노드를 담는 잎)만 정한다.
#[derive(Clone, Copy, PartialEq)]
enum PoolShape {
    BlackBox,
    Plain,
    Laned { sub_lane_counts: [usize; 2], lane_count: usize },
}

/// 노드가 들어가는 가장 안쪽 칸: (풀, 레인, 하위 레인).
type LeafContainer = (usize, Option<usize>, Option<usize>);

/// 풀 순서나 메시지 통로를 바꿀 조건(받는 풀이 앞, 마주 보는 메시지, 풀을 가로지르는 메시지, 한 면이나
/// 한 노드에 메시지 여럿)을 하나도 만족하지 않는 문서. 노드는 YAML에 선언 순서대로 나오고 블랙박스 풀의
/// 닻은 흐름을 그릴 때 생기므로, 메시지를 받는 블랙박스 풀은 노드를 가진 모든 풀 뒤에 놓인다. 그래서
/// 마지막 노드 풀에서 그 블랙박스 풀로 가는 메시지 하나는 이미 순방향이고 사이 풀을 지나지 않는다.
struct UnchangedDomainDocument {
    pools: Vec<PoolShape>,
    nodes: Vec<(GeneratedNode, Option<LeafContainer>)>,
    flow_ends: Vec<(String, String)>,
}

fn is_flow_node_kind(kind: &str) -> bool {
    !matches!(kind, "dataObjectReference" | "textAnnotation")
}

impl UnchangedDomainDocument {
    fn generate(rng: &mut Xorshift64) -> Self {
        let pools: Vec<PoolShape> = (0..rng.below(4))
            .map(|_| match rng.below(3) {
                0 => PoolShape::BlackBox,
                1 => PoolShape::Plain,
                _ => PoolShape::Laned { lane_count: 1 + rng.below(2), sub_lane_counts: [rng.below(3), rng.below(3)] },
            })
            .collect();
        let leaves = Self::leaf_containers(&pools);
        let node_count = 2 + rng.below(5);
        let nodes: Vec<(GeneratedNode, Option<LeafContainer>)> = (0..node_count)
            .map(|i| {
                let leaf = if leaves.is_empty() { None } else { Some(leaves[rng.below(leaves.len())]) };
                let node = GeneratedNode {
                    id: format!("n{i}"),
                    kind: NODE_KINDS[rng.below(NODE_KINDS.len())],
                    name: if rng.below(2) == 0 { None } else { Some(NODE_NAMES[rng.below(NODE_NAMES.len())]) },
                    participant: leaf.map(|(pool, _, _)| pool),
                };
                (node, leaf)
            })
            .collect();
        let mut flow_ends = Vec::new();
        for _ in 0..1 + rng.below(node_count) {
            let source = rng.below(node_count);
            let target = (source + 1 + rng.below(node_count - 1)) % node_count;
            let (source_node, target_node) = (&nodes[source].0, &nodes[target].0);
            let is_same_pool = source_node.participant == target_node.participant;
            let has_flow_node_end = is_flow_node_kind(source_node.kind) || is_flow_node_kind(target_node.kind);
            if is_same_pool && has_flow_node_end {
                flow_ends.push((source_node.id.clone(), target_node.id.clone()));
            }
        }
        let black_boxes: Vec<usize> = (0..pools.len()).filter(|&p| pools[p] == PoolShape::BlackBox).collect();
        let last_node_pool = nodes.iter().filter_map(|(node, _)| node.participant).max();
        if let Some(last_node_pool) = last_node_pool
            && !black_boxes.is_empty()
            && rng.below(4) != 0
        {
            let senders: Vec<&GeneratedNode> = nodes.iter().map(|(node, _)| node).filter(|node| node.participant == Some(last_node_pool) && is_flow_node_kind(node.kind)).collect();
            if !senders.is_empty() {
                let sender = senders[rng.below(senders.len())].id.clone();
                let receiver = black_boxes[rng.below(black_boxes.len())];
                flow_ends.push((sender, format!("p{receiver}")));
            }
        }
        UnchangedDomainDocument { pools, nodes, flow_ends }
    }

    fn leaf_containers(pools: &[PoolShape]) -> Vec<LeafContainer> {
        let mut leaves = Vec::new();
        for (pool, shape) in pools.iter().enumerate() {
            match *shape {
                PoolShape::BlackBox => {}
                PoolShape::Plain => leaves.push((pool, None, None)),
                PoolShape::Laned { lane_count, sub_lane_counts } => {
                    for (lane, &sub_lane_count) in sub_lane_counts.iter().enumerate().take(lane_count) {
                        if sub_lane_count == 0 {
                            leaves.push((pool, Some(lane), None));
                        }
                        leaves.extend((0..sub_lane_count).map(|sub| (pool, Some(lane), Some(sub))));
                    }
                }
            }
        }
        leaves
    }

    fn has_message_flow(&self) -> bool {
        self.flow_ends.iter().any(|(_, target)| target.starts_with('p'))
    }

    fn push_container(&self, yaml: &mut String, id: &str, name: &str, leaf: LeafContainer, indent: &str) {
        let members: Vec<&GeneratedNode> = self.nodes.iter().filter(|(_, l)| *l == Some(leaf)).map(|(node, _)| node).collect();
        if members.is_empty() {
            yaml.push_str(&format!("{indent}- {id}: {name}\n"));
            return;
        }
        yaml.push_str(&format!("{indent}- {id}:\n{indent}    name: {name}\n{indent}    nodes:\n"));
        for node in members {
            yaml.push_str(&GeneratedDocument::node_line(node, &format!("{indent}      ")));
        }
    }

    fn to_yaml(&self) -> String {
        let mut yaml = String::new();
        if !self.pools.is_empty() {
            yaml.push_str("participants:\n");
        }
        for (pool, shape) in self.pools.iter().enumerate() {
            match *shape {
                PoolShape::BlackBox => yaml.push_str(&format!("  - p{pool}: Pool {pool}\n")),
                PoolShape::Plain => self.push_container(&mut yaml, &format!("p{pool}"), &format!("Pool {pool}"), (pool, None, None), "  "),
                PoolShape::Laned { lane_count, sub_lane_counts } => {
                    yaml.push_str(&format!("  - p{pool}:\n      name: Pool {pool}\n      lanes:\n"));
                    for (lane, &sub_lane_count) in sub_lane_counts.iter().enumerate().take(lane_count) {
                        let (lane_id, lane_name) = (format!("l{pool}_{lane}"), format!("Lane {pool}.{lane}"));
                        if sub_lane_count == 0 {
                            self.push_container(&mut yaml, &lane_id, &lane_name, (pool, Some(lane), None), "        ");
                            continue;
                        }
                        yaml.push_str(&format!("        - {lane_id}:\n            name: {lane_name}\n            lanes:\n"));
                        for sub in 0..sub_lane_count {
                            self.push_container(&mut yaml, &format!("s{pool}_{lane}_{sub}"), &format!("Sub {pool}.{lane}.{sub}"), (pool, Some(lane), Some(sub)), "              ");
                        }
                    }
                }
            }
        }
        let loose: Vec<&GeneratedNode> = self.nodes.iter().filter(|(_, leaf)| leaf.is_none()).map(|(node, _)| node).collect();
        if !loose.is_empty() {
            yaml.push_str("nodes:\n");
            for node in loose {
                yaml.push_str(&GeneratedDocument::node_line(node, "  "));
            }
        }
        if !self.flow_ends.is_empty() {
            yaml.push_str("flows:\n");
            for (source, target) in &self.flow_ends {
                yaml.push_str(&format!("  - {source} --> {target}\n"));
            }
        }
        yaml
    }
}

const UNCHANGED_DOMAIN_BASE_SEED: u64 = 0x0400_B9A1;
const UNCHANGED_DOMAIN_DIRECTIONS: [Option<(Direction, bool)>; 5] = [None, Some((Direction::TopDown, false)), Some((Direction::LeftRight, false)), Some((Direction::TopDown, true)), Some((Direction::LeftRight, true))];

/// 0.4.0 코드로 기록한 (줄 수, FNV-1a 해시). `None`은 원문 코드블록으로 물러난 문서다. 표 순서가
/// `UNCHANGED_DOMAIN_BASE_SEED`부터의 seed 순서다.
const UNCHANGED_DOMAIN_RECORDED: [Option<(usize, u64)>; 100] = [
    Some((11, 0x9ba9a954f843517b)),
    Some((39, 0x267d122b395cb1a1)),
    Some((28, 0xc8c0c5bccf0165d1)),
    Some((9, 0x701764a65b3b1275)),
    Some((8, 0xf1dd807edf80f1e2)),
    Some((14, 0xd6dae399cb5616b7)),
    Some((8, 0x0248b48690aacad2)),
    Some((8, 0x18bd42ad69f2d585)),
    Some((21, 0x5ff8b94bf175a4c2)),
    Some((21, 0x953cbbd887eaa2d4)),
    Some((13, 0x0f47cb402f2831f1)),
    Some((8, 0xcdcd04f016458776)),
    Some((31, 0xe7aef6b0071917b4)),
    Some((18, 0xa07e5af04997a864)),
    Some((13, 0x9fcf41cab9add195)),
    Some((13, 0x71da17fdd507e2d0)),
    Some((25, 0x547549283cf24125)),
    Some((4, 0x1965c44df5ab56e6)),
    Some((11, 0xabac60d505a8c1e9)),
    Some((27, 0xa08ccdaa505cdc64)),
    Some((32, 0x8c8efaea59dff7a3)),
    Some((8, 0x21cf9bae2cd59d58)),
    Some((14, 0x73386a990321fe77)),
    Some((7, 0xfa5530e9306491b9)),
    Some((19, 0x97add4aec649d665)),
    Some((30, 0xcaea2db150552b08)),
    Some((16, 0x553592a0d804cc9d)),
    Some((13, 0xc7ea542059efef44)),
    Some((17, 0x5adae5051cb6e90a)),
    Some((16, 0x23404a54a3839791)),
    Some((22, 0x88e399bee7f9db43)),
    Some((8, 0x6d521426b6010048)),
    Some((12, 0x3d19c4ed599656ca)),
    Some((20, 0x42b2f8fe2248b85a)),
    Some((19, 0xc9e455b1bd8922be)),
    Some((27, 0x313ba7f519da4bce)),
    Some((18, 0x3d830b79c7aff514)),
    Some((14, 0xff2d9c120a8bdfc8)),
    Some((12, 0xf73bbe17c10d06c4)),
    Some((12, 0x7370a88d411fc12f)),
    Some((47, 0xafdffd617e469ab7)),
    Some((5, 0x44606d032e68a676)),
    Some((5, 0xb2e2dae8dfa55f72)),
    Some((20, 0x8b2a0359e212ee6d)),
    Some((14, 0x96568a5d796ae630)),
    Some((3, 0x8cafa8649e983de8)),
    Some((13, 0xda9c05ef457d2743)),
    Some((22, 0x09637fc3872e640d)),
    Some((27, 0x5376daeb2acabb5f)),
    Some((48, 0x3255ace36660a268)),
    Some((12, 0xe9ba0200850cb383)),
    Some((17, 0x3d32733bd7877df9)),
    Some((27, 0xf7a650a068e53659)),
    Some((17, 0xcbf5ffb7e0618b00)),
    Some((21, 0x298c9be0b4e6689a)),
    Some((9, 0xc76db1bcd5a3de96)),
    Some((10, 0x5482f28c0c47cce7)),
    Some((23, 0xc094986a285de43b)),
    Some((14, 0xc5cc847de0304f55)),
    Some((43, 0x20e7974055bcb612)),
    Some((29, 0x081e3744900cfd46)),
    Some((23, 0xf02c0dd4f0d0a311)),
    Some((21, 0xf2d74117ff75c319)),
    Some((31, 0xe29af45c813a7827)),
    Some((35, 0x6936ed070a64d2c0)),
    Some((9, 0x373d120c384a1d19)),
    Some((11, 0xafaf8b52855cb2e2)),
    Some((24, 0x4c4a27f9690bab6c)),
    Some((13, 0x864f9bb372025de3)),
    Some((6, 0x6f6870d12e85e85e)),
    Some((9, 0xb2c8f3773b232b95)),
    Some((13, 0x08f37f569f0c6496)),
    Some((15, 0x94f89c9a2f9b1d71)),
    Some((42, 0x205c56f07a73d44e)),
    Some((22, 0x596f738c8d3bcb4f)),
    Some((14, 0xf8edaa63a247d40f)),
    Some((16, 0xfdb3cbb3fe6104ca)),
    Some((18, 0xc676a7741abb2f2c)),
    Some((25, 0xb701d73c661a9c0f)),
    Some((15, 0x79acdd05f5d14a75)),
    Some((7, 0x30361cff2cce6e68)),
    Some((38, 0x8d9beadcdf44f640)),
    Some((33, 0x318aca5a7f488ad9)),
    Some((68, 0x054cd66acea827df)),
    Some((46, 0xec65c6326776a279)),
    Some((11, 0xd765f9254edeba52)),
    Some((18, 0xb35ec09fc2a838da)),
    Some((8, 0x41a19baeda9244f0)),
    Some((27, 0xa59a2d05e573a4d2)),
    Some((9, 0x4a7f33266537f0ca)),
    Some((9, 0x42b7d6114cde9234)),
    Some((25, 0xfb8b48e4ab33a2f4)),
    Some((15, 0x26dbe4edc5804ff1)),
    Some((16, 0x979e8e448613664f)),
    Some((11, 0xaff3933e30dcf110)),
    Some((13, 0x0ccab7602485cea4)),
    Some((8, 0x684594657839af60)),
    Some((11, 0x9126e135c30d6cad)),
    Some((33, 0xa7a3cc45a9f3277c)),
    Some((19, 0x5c4351995b088b78)),
];

fn rendered_fingerprint(rendered: &Option<(&'static str, Vec<Line>)>) -> Option<(usize, u64)> {
    let (_, lines) = rendered.as_ref()?;
    let text = lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
    Some((lines.len(), fnv1a(&text)))
}

#[test]
fn documents_outside_pool_message_defects_render_as_recorded_in_0_4_0() {
    let mut mismatches = Vec::new();
    let mut recorded_table = String::new();
    let (mut rendered_count, mut message_count) = (0, 0);
    for (index, expected) in UNCHANGED_DOMAIN_RECORDED.iter().enumerate() {
        let seed = UNCHANGED_DOMAIN_BASE_SEED + index as u64;
        let mut rng = Xorshift64::new(seed);
        let document = UnchangedDomainDocument::generate(&mut rng);
        let direction = UNCHANGED_DOMAIN_DIRECTIONS[rng.below(UNCHANGED_DOMAIN_DIRECTIONS.len())];
        let source = document.to_yaml();
        let rendered = render_text(&source, direction);
        let actual = rendered_fingerprint(&rendered);
        rendered_count += usize::from(actual.is_some());
        message_count += usize::from(actual.is_some() && document.has_message_flow());
        recorded_table.push_str(&match actual {
            Some((line_count, hash)) => format!("    Some(({line_count}, {hash:#018x})),\n"),
            None => "    None,\n".to_string(),
        });
        if actual != *expected {
            mismatches.push(format!("seed={seed:#x} index={index} direction={direction:?} 기록 {expected:?}, 지금 {actual:?}\n--- YAML\n{source}--- 렌더링\n{}", describe(&rendered)));
        }
    }
    assert!(mismatches.is_empty(), "0.4.0에서 기록한 렌더링과 달라진 문서 {}건:\n{}\n--- 지금 코드의 기록 표\n{recorded_table}", mismatches.len(), mismatches.join("\n\n"));
    assert!(rendered_count * 4 >= UNCHANGED_DOMAIN_RECORDED.len() * 3, "생성 문서 {}개 중 {rendered_count}개만 렌더링됐다", UNCHANGED_DOMAIN_RECORDED.len());
    assert!(message_count * 5 >= UNCHANGED_DOMAIN_RECORDED.len(), "메시지 흐름이 있는 렌더링 문서가 {message_count}개뿐이다");
}
