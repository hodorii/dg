//! 파서와 배치기 사이의 공통 중간 표현.
//! mermaid·PlantUML 파서는 모두 이 구조를 만들고, 배치기는 이것만 본다.

pub use crate::diagram::canvas::LineKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    TopDown,
    LeftRight,
}

impl Direction {
    pub fn other(self) -> Direction {
        match self {
            Direction::TopDown => Direction::LeftRight,
            Direction::LeftRight => Direction::TopDown,
        }
    }
}

/// BPMN 이벤트 노드가 흐름 안 어디에 있는지 — 위치 글자와 테두리 굵기만 바꾼다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPosition {
    Start,
    Intermediate,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Rect,
    Round,
    Stadium,
    Cylinder,
    Diamond,
    Hexagon,
    Subroutine,
    Circle,
    Actor,
    Interface,
    Note,
    /// 테두리 없이 글자만(PlantUML WBS의 `_` 접미사 등).
    Plain,
    /// 상태도의 시작점 `●`.
    Start,
    /// 상태도의 끝점 `◉`.
    End,
    /// 그룹을 가리키는 간선이 닿는 보이지 않는 점.
    Anchor,
    /// BPMN 이벤트: 테두리 없음, 위치 글자(○/◎/◉) 자체가 도형이고 이름이 그 오른쪽에 온다.
    Event(EventPosition),
    /// BPMN 접힌 서브프로세스: Round와 같은 테두리·본문에 아래 테두리 가운데 `[+]`.
    Subprocess,
}

impl Shape {
    /// 이 모양의 간선 접점·정렬 기준이 상자 가운데가 아니라 원점 칸(= 위치 글자)인지.
    /// `layout::graph`(접점 배정·정렬)와 `layout::shape`(그리기 오프셋)가 함께 참조하는
    /// SSoT — 이벤트는 "테두리 없이 글자가 곧 도형"이라 덩어리 가운데가 아니라 그 글자 칸에
    /// 선이 닿아야 한다(bpmn-event-notation-anchor). 상태도 `Start`/`End`는 이미 1×1이라
    /// 넣지 않는다 — 넣으면 `center()`가 0.5 이동해 기존 상태도 배치가 흔들릴 수 있다.
    pub fn is_point_anchored(self) -> bool {
        matches!(self, Shape::Event(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Marker {
    #[default]
    None,
    Arrow,
    OpenArrow,
    Triangle,
    DiamondFilled,
    DiamondOpen,
    Circle,
    Cross,
    /// 까치발(crow's foot) 표기: 정확히 하나 `||`.
    CrowOne,
    /// 없거나 하나 `|o`.
    CrowZeroOne,
    /// 하나 이상 `|{`.
    CrowMany,
    /// 없거나 여럿 `o{`.
    CrowZeroMany,
    /// BPMN default 시퀀스 흐름의 빗금 꼬리 `╱`.
    Slash,
}

/// ER 카디널리티 문자열(`1`, `0..1`, `1..N`, `0..N`)을 까치발 표식으로.
pub fn crow_marker(cardinality: &str) -> Marker {
    match cardinality {
        "1" => Marker::CrowOne,
        "0..1" => Marker::CrowZeroOne,
        "1..N" => Marker::CrowMany,
        "0..N" => Marker::CrowZeroMany,
        _ => Marker::None,
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    /// 칸으로 나뉜 본문. 첫 칸은 이름(굵게), 나머지는 속성·메서드 등.
    pub sections: Vec<Vec<String>>,
    pub shape: Shape,
    pub group: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: String,
    /// `from` 쪽 끝 라벨(다중성 등).
    pub tail_label: String,
    pub head_label: String,
    pub kind: LineKind,
    pub tail: Marker,
    pub head: Marker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GroupKind {
    /// 구성원 범위만 감싸는 내용 적응형 상자.
    #[default]
    Box,
    /// 소속 범위(다이어그램 전체 또는 부모 레인) 전체를 관통하는 띠.
    Lane,
}

#[derive(Clone, Debug)]
pub struct Group {
    /// 원문에서 그룹을 가리키는 아이디(간선의 끝으로 쓰일 수 있다).
    pub id: String,
    pub title: String,
    pub parent: Option<usize>,
    pub kind: GroupKind,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    pub direction: Option<Direction>,
    /// `direction`이 가리키는 축(TB·LR)을 따라 층 순서를 뒤집는다: BT는 TopDown 축을, RL은
    /// LeftRight 축을 뒤집어 만든다(`layout::graph`가 실제로 소비한다).
    pub direction_reversed: bool,
    pub title: String,
}

impl Graph {
    /// 아이디로 노드를 찾거나 새로 만든다. 새 노드는 `group`에 들어간다.
    pub fn intern(&mut self, id: &str, label: &str, shape: Shape, group: Option<usize>) -> usize {
        if let Some(i) = self.nodes.iter().position(|n| n.id == id) {
            return i;
        }
        let label = if label.is_empty() { id } else { label };
        self.nodes.push(Node {
            id: id.to_string(),
            sections: vec![label.lines().map(str::to_string).collect()],
            shape,
            group,
        });
        self.nodes.len() - 1
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    pub fn set_label(&mut self, index: usize, label: &str) {
        self.nodes[index].sections[0] = label.lines().map(str::to_string).collect();
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn add_group(&mut self, title: &str, parent: Option<usize>) -> usize {
        self.add_group_with_id(title, title, parent)
    }

    pub fn add_group_with_id(&mut self, id: &str, title: &str, parent: Option<usize>) -> usize {
        self.groups.push(Group { id: id.to_string(), title: title.to_string(), parent, kind: GroupKind::Box });
        self.groups.len() - 1
    }

    /// 레인 종류 그룹. id = title(기존 `add_group`과 같은 규약). 소속 범위(다이어그램 전체 또는
    /// 부모 레인) 전체를 관통하는 띠로 그려진다(`layout::graph`가 소비).
    pub fn add_lane(&mut self, title: &str, parent: Option<usize>) -> usize {
        let group = self.add_group_with_id(title, title, parent);
        self.groups[group].kind = GroupKind::Lane;
        group
    }

    /// 그룹 자체를 가리키는 간선 끝. 그룹 안에 보이지 않는 닻 노드를 두고 거기에 잇는다.
    pub fn group_anchor(&mut self, group: usize) -> usize {
        let id = format!("@group-anchor:{group}");
        let index = self.intern(&id, "", Shape::Anchor, Some(group));
        self.nodes[index].sections = vec![Vec::new()];
        index
    }

    /// 자기 자신을 포함한 조상 목록(안쪽부터).
    pub fn ancestors(&self, group: Option<usize>) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cursor = group;
        while let Some(g) = cursor {
            out.push(g);
            cursor = self.groups[g].parent;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_lane_marks_group_kind_lane_while_add_group_stays_box() {
        let mut g = Graph::default();
        let lane = g.add_lane("Sales", None);
        let boxed = g.add_group("Backend", None);
        assert_eq!(g.groups[lane].kind, GroupKind::Lane);
        assert_eq!(g.groups[boxed].kind, GroupKind::Box);
    }
}

/// 시퀀스 다이어그램.
#[derive(Clone, Debug, Default)]
pub struct Sequence {
    pub participants: Vec<Participant>,
    pub items: Vec<SequenceItem>,
    pub autonumber: bool,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct Participant {
    pub id: String,
    pub label: String,
    pub kind: ParticipantKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ParticipantKind {
    #[default]
    Box,
    Actor,
    Database,
    Boundary,
    Control,
    Entity,
    Collections,
    Queue,
}

#[derive(Clone, Debug)]
pub enum SequenceItem {
    Message {
        from: usize,
        to: usize,
        label: String,
        kind: LineKind,
        head: Marker,
        /// 받는 쪽을 활성화(`++`)·보내는 쪽을 비활성화(`--`).
        activate_target: bool,
        deactivate_source: bool,
    },
    Note {
        placement: NotePlacement,
        lines: Vec<String>,
    },
    FragmentStart {
        kind: String,
        label: String,
    },
    FragmentElse {
        label: String,
    },
    FragmentEnd,
    Divider {
        label: String,
    },
    Delay {
        label: String,
    },
    Activate(usize),
    Deactivate(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotePlacement {
    LeftOf(usize),
    RightOf(usize),
    Over(usize, usize),
}

impl Sequence {
    pub fn intern(&mut self, id: &str, label: &str, kind: ParticipantKind) -> usize {
        if let Some(i) = self.participants.iter().position(|p| p.id == id) {
            return i;
        }
        let label = if label.is_empty() { id } else { label };
        self.participants.push(Participant { id: id.to_string(), label: label.to_string(), kind });
        self.participants.len() - 1
    }
}
