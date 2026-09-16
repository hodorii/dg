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
    /// 상태도의 시작점 `●`.
    Start,
    /// 상태도의 끝점 `◉`.
    End,
    /// 그룹을 가리키는 간선이 닿는 보이지 않는 점.
    Anchor,
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

#[derive(Clone, Debug)]
pub struct Group {
    /// 원문에서 그룹을 가리키는 아이디(간선의 끝으로 쓰일 수 있다).
    pub id: String,
    pub title: String,
    pub parent: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    pub direction: Option<Direction>,
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
        self.groups.push(Group { id: id.to_string(), title: title.to_string(), parent });
        self.groups.len() - 1
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
