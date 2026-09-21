//! gitGraph 배치.
//!
//! 가로 모드(기본): 브랜치마다 가로 트랙을 한 줄씩 잡고, 커밋을 등장 순서대로 왼쪽에서 오른쪽으로
//! 찍는다. 분기·병합은 두 트랙 줄을 잇는 세로 연결선으로 그린다.
//! 세로 모드(`direction: tb`): 브랜치마다 세로 트랙을 한 칸씩 잡고, 커밋을 등장 순서대로 위에서
//! 아래로 찍는다. 분기·병합은 두 트랙 칸을 잇는 가로 연결선으로 그린다 — 가로 모드를 그대로 전치(transpose)한 것.

use crate::diagram::canvas::{Canvas, LineKind};
use crate::diagram::ir::Direction;
use crate::diagram::mermaid::gitgraph::{GitEvent, GitGraph};
use crate::line::Line;
use crate::style::Theme;
use crate::text::{truncate, width_of};

const COMMIT_DOT: char = '●';
const MERGE_DOT: char = '◉';
/// 커밋 한 칸이 점과 아이디 말고 따로 차지하는 폭(`●───`).
const SLOT_PADDING: usize = 4;

pub fn render(graph: &GitGraph, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if graph.tracks.is_empty() || graph.events.is_empty() {
        return None;
    }
    let plan = plan(graph);
    let preferred = graph.direction.map(|(direction, _)| direction).unwrap_or(Direction::LeftRight);
    // 지정한 방향으로 먼저 시도하고, 폭에 안 들어가면 반대 방향으로도 시도한다(다른 그래프형과 같은 계약).
    // 각 방향 안에서는 이름·아이디를 더 짧게 잘라 다시 시도한다.
    for direction in [preferred, preferred.other()] {
        for cap in [24usize, 16, 12, 8] {
            let built = match direction {
                Direction::LeftRight => build(graph, &plan, theme, cap, width),
                Direction::TopDown => build_vertical(graph, &plan, theme, cap, width),
            };
            if let Some(canvas) = built {
                return Some(canvas.into_lines());
            }
        }
    }
    None
}

struct Dot {
    row: usize,
    column: usize,
    id: String,
    is_merge: bool,
}

/// 열·줄 좌표로 옮긴 그래프. 실제 칸 좌표는 `build`가 붙인다.
struct Plan {
    dots: Vec<Dot>,
    /// (부모 줄, 자식 줄, 갈라지는 열)
    forks: Vec<(usize, usize, usize)>,
    /// (가져오는 쪽 줄, 받는 쪽 줄, 병합 커밋 열)
    merges: Vec<(usize, usize, usize)>,
    /// 트랙별 (첫 열, 끝 열).
    spans: Vec<Option<(usize, usize)>>,
}

fn plan(graph: &GitGraph) -> Plan {
    let count = graph.tracks.len();
    let mut plan = Plan { dots: Vec::new(), forks: Vec::new(), merges: Vec::new(), spans: vec![None; count] };
    let mut tips: Vec<Option<usize>> = vec![None; count];
    let mut next_column = 0usize;
    extend(&mut plan.spans, 0, 0);
    for event in &graph.events {
        match event {
            GitEvent::Commit { track, id } if *track < count => {
                let column = next_column;
                next_column += 1;
                extend(&mut plan.spans, *track, column);
                tips[*track] = Some(column);
                plan.dots.push(Dot { row: *track, column, id: id.clone(), is_merge: false });
            }
            GitEvent::Fork { track, parent } if *track < count && *parent < count => {
                let anchor = tips[*parent].unwrap_or_else(|| next_column.saturating_sub(1));
                extend(&mut plan.spans, *parent, anchor);
                extend(&mut plan.spans, *track, anchor);
                if track != parent {
                    plan.forks.push((*parent, *track, anchor));
                }
            }
            GitEvent::Merge { target, source } if *target < count => {
                let column = next_column;
                next_column += 1;
                extend(&mut plan.spans, *target, column);
                tips[*target] = Some(column);
                plan.dots.push(Dot { row: *target, column, id: String::new(), is_merge: true });
                if let Some(source) = source.filter(|s| *s < count && s != target) {
                    extend(&mut plan.spans, source, column);
                    plan.merges.push((source, *target, column));
                }
            }
            _ => {}
        }
    }
    plan
}

fn extend(spans: &mut [Option<(usize, usize)>], track: usize, column: usize) {
    match spans.get_mut(track) {
        Some(Some((first, last))) => {
            *first = (*first).min(column);
            *last = (*last).max(column);
        }
        Some(slot) => *slot = Some((column, column)),
        None => {}
    }
}

fn build(graph: &GitGraph, plan: &Plan, theme: &Theme, cap: usize, width: usize) -> Option<Canvas> {
    let names: Vec<String> = graph.tracks.iter().map(|name| truncate(name, cap)).collect();
    let ids: Vec<String> = plan.dots.iter().map(|dot| truncate(&dot.id, cap)).collect();
    let left = names.iter().map(|name| width_of(name)).max().unwrap_or(0) + 2;
    let slot = ids.iter().map(|id| width_of(id)).max().unwrap_or(0).saturating_add(SLOT_PADDING);
    let last_column = plan.spans.iter().flatten().map(|(_, last)| *last).max().unwrap_or(0);
    // 폭이 넘칠 게 뻔하면 칸 좌표를 만들기 전에 접는다. 곱셈이 넘치는 것도 여기서 막는다.
    let span = last_column.checked_mul(slot);
    if span.is_none_or(|span| left.saturating_add(span).saturating_add(1) > width) {
        return None;
    }
    let x = |column: usize| left + column * slot;
    let mut total = left + 1;
    for (_, last) in plan.spans.iter().flatten() {
        total = total.max(x(*last) + 1);
    }
    for (dot, id) in plan.dots.iter().zip(&ids) {
        total = total.max(x(dot.column) + 1 + width_of(id));
    }
    if total > width {
        return None;
    }

    let mut canvas = Canvas::new(total, names.len());
    // 트랙과 연결선은 간선 모드로 그려 서로 직교하는 칸이 건너뛰기로 표시되게 한다.
    canvas.set_edge_mode(true);
    for (row, span) in plan.spans.iter().enumerate() {
        if let Some((first, last)) = span
            && last > first
        {
            canvas.hline(x(*first), x(*last), row, LineKind::Solid, theme.diagram_line);
        }
    }
    for (parent, child, column) in &plan.forks {
        canvas.vline(x(*column), *parent, *child, LineKind::Solid, theme.diagram_line);
    }
    for (source, target, column) in &plan.merges {
        canvas.vline(x(*column), *source, *target, LineKind::Solid, theme.diagram_line);
    }
    canvas.set_edge_mode(false);
    for (row, name) in names.iter().enumerate() {
        canvas.text(0, row, name, theme.diagram_label);
    }
    for (dot, id) in plan.dots.iter().zip(&ids) {
        let glyph = if dot.is_merge { MERGE_DOT } else { COMMIT_DOT };
        canvas.put(x(dot.column), dot.row, glyph, theme.diagram_accent);
        if !id.is_empty() {
            canvas.text(x(dot.column) + 1, dot.row, id, theme.diagram_text);
        }
    }
    Some(canvas)
}

/// `build`를 세로로 전치한 것: 트랙=칸(열), 커밋 순서=줄(행). 좁은 터미널에서 가로 모드가
/// 옆으로 잘리는 대신, 세로로 스크롤해서 읽을 수 있게 한다.
fn build_vertical(graph: &GitGraph, plan: &Plan, theme: &Theme, cap: usize, width: usize) -> Option<Canvas> {
    let names: Vec<String> = graph.tracks.iter().map(|name| truncate(name, cap)).collect();
    let ids: Vec<String> = plan.dots.iter().map(|dot| truncate(&dot.id, cap)).collect();
    if names.is_empty() {
        return None;
    }
    let top = 1; // 브랜치 이름을 적는 머리글 줄.
    let name_width = names.iter().map(|name| width_of(name)).max().unwrap_or(0);
    let id_width = ids.iter().map(|id| width_of(id)).max().unwrap_or(0);
    // 트랙(열) 사이 간격: 이름과 "점+아이디" 중 더 넓은 쪽 + 여백.
    let col = name_width.max(id_width + 1).saturating_add(SLOT_PADDING);
    let last_track = names.len() - 1;
    // 폭이 넘칠 게 뻔하면 칸 좌표를 만들기 전에 접는다. 곱셈이 넘치는 것도 여기서 막는다.
    let span = last_track.checked_mul(col);
    if span.is_none_or(|span| span.saturating_add(col) > width) {
        return None;
    }
    let x = |track: usize| track * col;
    let total_width = x(last_track) + col;
    if total_width > width {
        return None;
    }
    let last_row = plan.spans.iter().flatten().map(|(_, last)| *last).max().unwrap_or(0);
    let total_height = top + last_row + 1;

    let mut canvas = Canvas::new(total_width, total_height);
    // 트랙과 연결선은 간선 모드로 그려 서로 직교하는 칸이 건너뛰기로 표시되게 한다.
    canvas.set_edge_mode(true);
    for (track, span) in plan.spans.iter().enumerate() {
        if let Some((first, last)) = span
            && last > first
        {
            canvas.vline(x(track), top + first, top + last, LineKind::Solid, theme.diagram_line);
        }
    }
    for (parent, child, step) in &plan.forks {
        canvas.hline(x(*parent), x(*child), top + step, LineKind::Solid, theme.diagram_line);
    }
    for (source, target, step) in &plan.merges {
        canvas.hline(x(*source), x(*target), top + step, LineKind::Solid, theme.diagram_line);
    }
    canvas.set_edge_mode(false);
    for (track, name) in names.iter().enumerate() {
        canvas.text(x(track), 0, name, theme.diagram_label);
    }
    for (dot, id) in plan.dots.iter().zip(&ids) {
        let glyph = if dot.is_merge { MERGE_DOT } else { COMMIT_DOT };
        canvas.put(x(dot.row), top + dot.column, glyph, theme.diagram_accent);
        if !id.is_empty() {
            canvas.text(x(dot.row) + 1, top + dot.column, id, theme.diagram_text);
        }
    }
    Some(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::mermaid::gitgraph::parse;
    use crate::diagram::{DiagramOptions, Language};

    fn rows(source: &str, width: usize) -> Vec<String> {
        render(&parse(source), &Theme::none(), width).unwrap().iter().map(Line::plain).collect()
    }

    /// 2.1 `commit`만 있으면 한 트랙에 왼쪽에서 오른쪽으로 늘어선다.
    #[test]
    fn commits_only_share_one_track() {
        let out = rows("gitGraph\n commit\n commit\n commit\n", 80);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], "main  ●───●───●");
    }

    /// 2.2 `branch`는 새 트랙 줄을 만들고, 4.1 이름을 트랙 옆에 적는다.
    #[test]
    fn branch_adds_a_named_track() {
        let out = rows("gitGraph\n commit\n branch develop\n commit\n", 80);
        assert_eq!(out.len(), 2);
        assert!(out[0].starts_with("main"), "{out:?}");
        assert!(out[1].starts_with("develop"), "{out:?}");
    }

    /// 2.3 `checkout`/`switch` 뒤의 커밋은 그 트랙 줄에 찍힌다.
    #[test]
    fn checkout_moves_following_commits_to_that_track() {
        let out = rows("gitGraph\n commit\n branch develop\n checkout main\n commit\n switch develop\n commit\n", 80);
        assert_eq!(out[0].matches(COMMIT_DOT).count(), 2);
        assert_eq!(out[1].matches(COMMIT_DOT).count(), 1);
    }

    /// 3.1 `merge`는 두 트랙 사이에 연결선을 긋고 병합 커밋을 남긴다.
    #[test]
    fn merge_draws_a_connector_between_tracks() {
        let out = rows("gitGraph\n commit\n branch develop\n commit\n checkout main\n merge develop\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains(MERGE_DOT), "{joined}");
        assert!(out[1].contains('└'), "{joined}");
        assert!(out[1].contains('┘'), "{joined}");
    }

    /// 3.2 없는 브랜치를 병합해도 연결선만 빠지고 나머지는 그대로 그린다.
    #[test]
    fn merging_an_undeclared_branch_omits_only_the_connector() {
        let out = rows("gitGraph\n commit\n merge ghost\n commit\n", 80);
        assert_eq!(out.len(), 1);
        assert!(out[0].contains(MERGE_DOT), "{out:?}");
        assert_eq!(out[0].matches(COMMIT_DOT).count(), 2);
    }

    /// 5.2 선언되지 않은 브랜치로 `checkout` 하면 그 자리에서 갈라진 새 트랙이 된다.
    #[test]
    fn checkout_of_an_undeclared_branch_opens_a_track() {
        let out = rows("gitGraph\n commit\n checkout phantom\n commit\n", 80);
        assert_eq!(out.len(), 2);
        assert!(out[1].starts_with("phantom"), "{out:?}");
        assert_eq!(out[1].matches(COMMIT_DOT).count(), 1);
    }

    /// 4.2 `commit id: "..."`의 글은 커밋 점 옆에 붙는다.
    #[test]
    fn commit_id_is_shown_next_to_its_dot() {
        let out = rows("gitGraph\n commit id: \"init\"\n commit id: \"next\"\n", 80);
        assert_eq!(out[0], "main  ●init───●next");
    }

    /// 5.1 폭에 들어가지 않으면 `None`을 돌려 평범한 코드블록으로 물러난다.
    #[test]
    fn too_narrow_returns_none() {
        // 세로 모드로도 못 들어갈 만큼 좁아야 한다 — 가로가 좁아도 세로로 들어가면 렌더링된다
        // (아래 `falls_back_to_vertical_when_horizontal_is_too_narrow`).
        let graph = parse("gitGraph\n commit\n commit\n commit\n commit\n commit\n");
        assert!(render(&graph, &Theme::none(), 4).is_none());
    }

    /// 가로로는 안 들어가도(커밋이 많아 옆으로 길어짐) 세로로는 들어가는 폭이면, 반대 방향으로
    /// 재시도해 세로 모드로 렌더링한다(다른 그래프형과 같은 "안 맞으면 반대 방향" 계약).
    #[test]
    fn falls_back_to_vertical_when_horizontal_is_too_narrow() {
        let graph = parse("gitGraph\n commit\n commit\n commit\n commit\n commit\n");
        let out = render(&graph, &Theme::none(), 12).expect("세로 모드로는 들어가야 한다");
        let lines: Vec<String> = out.iter().map(Line::plain).collect();
        assert!(lines.len() > 1, "세로 모드는 커밋마다 줄이 늘어나야 한다: {lines:?}");
        let total_dots: usize = lines.iter().map(|l| l.matches(COMMIT_DOT).count()).sum();
        assert_eq!(total_dots, 5, "커밋 5개가 다 나와야 한다: {lines:?}");
    }

    #[test]
    fn empty_graph_returns_none() {
        assert!(render(&parse("gitGraph"), &Theme::none(), 80).is_none());
    }

    /// 트랙을 건너뛰는 병합 연결선은 사이에 낀 트랙 선을 건너뛰기 표시로 지나간다.
    #[test]
    fn connector_hops_over_an_intervening_track() {
        let source = "gitGraph\n commit\n branch one\n branch two\n commit\n checkout main\n merge two\n checkout one\n commit\n";
        let out = rows(source, 80);
        assert_eq!(out.len(), 3);
        assert!(out[1].contains('◠'), "{out:?}");
    }

    #[test]
    fn end_to_end_through_diagram_render() {
        let source = "gitGraph\n   commit id: \"init\"\n   branch develop\n   checkout develop\n   commit\n   checkout main\n   merge develop\n   commit\n";
        let out = crate::diagram::render(Language::Mermaid, source, &Theme::none(), 80, DiagramOptions::default()).unwrap();
        let joined: Vec<String> = out.iter().map(Line::plain).collect();
        let joined = joined.join("\n");
        assert!(joined.contains("mermaid · gitgraph"), "{joined}");
        assert!(joined.contains("init"), "{joined}");
        assert!(joined.contains("develop"), "{joined}");
        assert!(joined.contains(MERGE_DOT), "{joined}");
    }

    /// `gitGraph TB:` 헤더가 세로 모드를 고른다: 트랙마다 열 하나, 커밋은 위에서 아래로.
    #[test]
    fn header_tb_renders_vertically() {
        let out = rows("gitGraph TB:\n commit\n commit\n commit\n", 80);
        assert_eq!(out.len(), 4, "머리글 줄 1 + 커밋 3줄: {out:?}"); // 이름 줄 + 커밋 3개(줄마다 하나)
        assert!(out[0].starts_with("main"), "{out:?}");
        assert_eq!(out.iter().map(|l| l.matches(COMMIT_DOT).count()).sum::<usize>(), 3, "{out:?}");
    }

    /// 세로 모드에서 `branch`는 새 열(트랙 칸)을 만들고, 이름은 그 열 위 머리글 줄에 적힌다.
    #[test]
    fn vertical_branch_adds_a_named_column() {
        let out = rows("gitGraph TB:\n commit\n branch develop\n commit\n", 80);
        assert!(out[0].starts_with("main"), "{out:?}");
        assert!(out[0].contains("develop"), "머리글 줄에 두 트랙 이름이 다 있어야 한다: {out:?}");
    }

    /// 세로 모드에서 `merge`는 두 열 사이에 가로 연결선을 긋는다.
    #[test]
    fn vertical_merge_draws_a_horizontal_connector() {
        let out = rows("gitGraph TB:\n commit\n branch develop\n commit\n checkout main\n merge develop\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains(MERGE_DOT), "{joined}");
        assert!(joined.contains('─'), "가로 연결선이 있어야 한다: {joined}");
    }

    /// `DiagramOptions.direction`(CLI·소스 지시자)이 헤더보다 우선한다 — 다른 그래프형과 같은 계약.
    #[test]
    fn options_direction_overrides_header() {
        let source = "gitGraph LR:\n commit\n commit\n commit\n";
        let horizontal = crate::diagram::render(Language::Mermaid, source, &Theme::none(), 80, DiagramOptions::default()).unwrap();
        let forced_vertical = crate::diagram::render(
            Language::Mermaid,
            source,
            &Theme::none(),
            80,
            DiagramOptions { direction: Some((Direction::TopDown, false)), ..DiagramOptions::default() },
        )
        .unwrap();
        let h_joined: Vec<String> = horizontal.iter().map(Line::plain).collect();
        assert!(h_joined.join("\n").contains("●───●───●"), "{h_joined:?}");
        assert!(forced_vertical.len() > horizontal.len(), "강제 세로 모드는 줄이 더 많아야 한다");
    }
}
