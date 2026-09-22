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
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

const COMMIT_DOT: char = '●';
const MERGE_DOT: char = '◉';
/// 커밋 한 칸이 점과 아이디 말고 따로 차지하는 폭(`●───`).
const SLOT_PADDING: usize = 4;

/// 트랙(브랜치) 순번에 대응하는 (색, 선패턴). 색은 `xychart.rs`와 같은 4개 테마 역할을, 선패턴은
/// `block.rs`의 `border_kind`와 같은 실선→파선→굵은선 3종을 각각 독립적으로 순환한다. 두 길이가
/// 서로 달라 조합 수가 늘고, 트랙 0은 항상 `(diagram_accent, Solid)`라 단일 브랜치는 기존과 같다.
fn branch_style(theme: &Theme, track: usize) -> (Style, LineKind) {
    let colors = [theme.diagram_accent, theme.diagram_box, theme.diagram_group, theme.diagram_note];
    let patterns = [LineKind::Solid, LineKind::Dashed, LineKind::Heavy];
    (colors[track % colors.len()], patterns[track % patterns.len()])
}

/// 세로 모드 분기 연결선을 시작할 x 좌표. 부모 트랙의 x 좌표(`parent_x`)와 그 행에 있는 부모
/// 커밋 id의 렌더 폭(`parent_id_width`)으로 구한다. id가 비어 있으면 밀지 않는다(점 글자
/// 바로 뒤에 이어지는 대시는 기존처럼 자연스럽다). 비어 있지 않으면 텍스트가 시작되는 자리
/// (점 뒤 2칸, `gitgraph-dot-id-gap`) + 텍스트 폭 + 공백 1칸만큼 밀어, 분기선이 커밋 id 글자에
/// 바로 붙어 보이지 않게 한다(gitgraph-junction-polish).
fn fork_connector_start(parent_x: usize, parent_id_width: usize) -> usize {
    if parent_id_width == 0 { parent_x } else { parent_x + 2 + parent_id_width + 1 }
}

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
        // 점과 id 사이에 한 칸 공백을 두므로(gitgraph-dot-id-gap) 텍스트는 +2에서 시작한다.
        total = total.max(x(dot.column) + 2 + width_of(id));
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
            let (color, pattern) = branch_style(theme, row);
            canvas.hline(x(*first), x(*last), row, pattern, color);
        }
    }
    for (parent, child, column) in &plan.forks {
        let (color, pattern) = branch_style(theme, *child);
        canvas.vline(x(*column), *parent, *child, pattern, color);
        // 자식 트랙이 시작되는 끝점을 둥글게(gitgraph-junction-polish).
        canvas.join(x(*column), *child, 0, pattern, color, true);
    }
    for (source, target, column) in &plan.merges {
        let (color, pattern) = branch_style(theme, *source);
        canvas.vline(x(*column), *source, *target, pattern, color);
        // 합류해 들어오는(source) 트랙 쪽 끝점을 둥글게.
        canvas.join(x(*column), *source, 0, pattern, color, true);
    }
    canvas.set_edge_mode(false);
    for (row, name) in names.iter().enumerate() {
        canvas.text(0, row, name, branch_style(theme, row).0);
    }
    for (dot, id) in plan.dots.iter().zip(&ids) {
        let glyph = if dot.is_merge { MERGE_DOT } else { COMMIT_DOT };
        canvas.put(x(dot.column), dot.row, glyph, branch_style(theme, dot.row).0);
        if !id.is_empty() {
            // 점 바로 다음 칸은 비워 둔다(gitgraph-dot-id-gap) — 점 글자의 실제 터미널 렌더
            // 폭이 1칸을 넘는 환경에서 id 첫 글자가 가려지는 것을 막는다. 트랙 선(대시)이 이미
            // 그 칸을 지나갈 수 있으므로 명시적으로 비워 진짜 공백을 보장한다.
            canvas.clear_rect(x(dot.column) + 1, dot.row, 1, 1);
            canvas.text(x(dot.column) + 2, dot.row, id, theme.diagram_text);
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
    // 점과 id 사이 한 칸 공백(gitgraph-dot-id-gap) — id가 어디에도 없으면 기존처럼 점 한 칸만
    // 예약해 불필요한 여백을 만들지 않는다.
    let id_reserve = if id_width == 0 { 1 } else { id_width + 2 };
    // 트랙(열) 사이 간격: 이름과 "점+공백+아이디" 중 더 넓은 쪽 + 여백.
    let col = name_width.max(id_reserve).saturating_add(SLOT_PADDING);
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
            let (color, pattern) = branch_style(theme, track);
            canvas.vline(x(track), top + first, top + last, pattern, color);
        }
    }
    for (parent, child, step) in &plan.forks {
        let (color, pattern) = branch_style(theme, *child);
        // 부모의 가장 최근 커밋(이 분기의 anchor 행)에 id가 있으면 그 글자에 바로 붙지 않도록
        // 시작 좌표를 밀어낸다(gitgraph-junction-polish).
        let parent_id_width = plan.dots.iter().zip(&ids).find(|(dot, _)| dot.column == *step).map(|(_, id)| width_of(id)).unwrap_or(0);
        let start = fork_connector_start(x(*parent), parent_id_width);
        canvas.hline(start, x(*child), top + step, pattern, color);
        // 자식 트랙이 시작되는 끝점을 둥글게.
        canvas.join(x(*child), top + step, 0, pattern, color, true);
    }
    for (source, target, step) in &plan.merges {
        let (color, pattern) = branch_style(theme, *source);
        canvas.hline(x(*source), x(*target), top + step, pattern, color);
        // 합류해 들어오는(source) 트랙 쪽 끝점을 둥글게.
        canvas.join(x(*source), top + step, 0, pattern, color, true);
    }
    canvas.set_edge_mode(false);
    for (track, name) in names.iter().enumerate() {
        canvas.text(x(track), 0, name, branch_style(theme, track).0);
    }
    for (dot, id) in plan.dots.iter().zip(&ids) {
        let glyph = if dot.is_merge { MERGE_DOT } else { COMMIT_DOT };
        canvas.put(x(dot.row), top + dot.column, glyph, branch_style(theme, dot.row).0);
        if !id.is_empty() {
            // 점 바로 다음 칸은 비워 둔다(가로 모드와 동일한 이유, gitgraph-dot-id-gap).
            canvas.clear_rect(x(dot.row) + 1, top + dot.column, 1, 1);
            canvas.text(x(dot.row) + 2, top + dot.column, id, theme.diagram_text);
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

    /// 3.1 `merge`는 두 트랙 사이에 연결선을 긋고 병합 커밋을 남긴다. 연결선 끝점 모서리는
    /// 둥글게 그려진다(gitgraph-junction-polish).
    #[test]
    fn merge_draws_a_connector_between_tracks() {
        let out = rows("gitGraph\n commit\n branch develop\n commit\n checkout main\n merge develop\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains(MERGE_DOT), "{joined}");
        assert!(out[1].contains('╰'), "{joined}");
        assert!(out[1].contains('╯'), "{joined}");
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
        assert_eq!(out[0], "main  ● init──● next");
    }

    /// 1.1/2.1(gitgraph-dot-id-gap) 가로 모드: 점 글자와 id 글자 사이에 최소 한 칸 공백이 있어야
    /// 한다 — 터미널 폰트에 따라 점 글자의 시각적 폭이 1칸을 넘어 id 첫 글자를 가릴 수 있기
    /// 때문(사용자 스크린샷으로 재현됨). 빈 id 커밋은 여백을 새로 만들지 않는다(3.1).
    #[test]
    fn horizontal_dot_and_id_have_a_gap() {
        let out = rows("gitGraph\n commit id: \"root-long-id\"\n", 80);
        assert_eq!(out[0].chars().nth(6), Some(COMMIT_DOT));
        assert_eq!(out[0].chars().nth(7), Some(' '), "점과 id 사이에 공백이 있어야 한다: {}", out[0]);
        assert_eq!(out[0].chars().nth(8), Some('r'), "공백 다음이 id 첫 글자여야 한다: {}", out[0]);

        // 빈 id는 기존처럼 여백 없이 붙는다(회귀 없음).
        let empty = rows("gitGraph\n commit\n commit\n commit\n", 80);
        assert_eq!(empty[0], "main  ●───●───●");
    }

    /// 1.2/2.2(gitgraph-dot-id-gap) 세로 모드: 점 글자와 id 글자 사이에 최소 한 칸 공백이 있어야
    /// 한다.
    #[test]
    fn vertical_dot_and_id_have_a_gap() {
        let out = rows("gitGraph TB:\n commit id: \"root-long-id\"\n", 80);
        let row = &out[1];
        assert_eq!(row.chars().next(), Some(COMMIT_DOT));
        assert_eq!(row.chars().nth(1), Some(' '), "점과 id 사이에 공백이 있어야 한다: {row}");
        assert_eq!(row.chars().nth(2), Some('r'), "공백 다음이 id 첫 글자여야 한다: {row}");
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
        // 연결선은 이제 트랙(브랜치)별 선패턴을 따르므로(gitgraph-branch-distinction), 실선/파선/
        // 굵은선 중 어느 것이든 가로 연결선 글자면 통과한다.
        assert!(joined.contains(['─', '╌', '━']), "가로 연결선이 있어야 한다: {joined}");
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

    /// 1.2/2.1/2.2/3.2 팔레트(4)·선패턴(3)이 각각 독립적으로 `track % len` 순환하고, 트랙 0은 항상
    /// 고정된 조합이라 단일 브랜치 회귀(2.2, 5.1)가 규칙에서 보장된다.
    #[test]
    fn branch_style_cycles_color_and_pattern_independently() {
        let theme = Theme::dark();
        let colors = [theme.diagram_accent, theme.diagram_box, theme.diagram_group, theme.diagram_note];
        let patterns = [LineKind::Solid, LineKind::Dashed, LineKind::Heavy];
        for track in [0usize, 1, 2, 3, 4, 6, 7] {
            let (color, pattern) = branch_style(&theme, track);
            assert_eq!(color, colors[track % 4], "track {track} 색");
            assert_eq!(pattern, patterns[track % 3], "track {track} 패턴");
        }
        assert_eq!(branch_style(&theme, 0), (theme.diagram_accent, LineKind::Solid), "트랙 0은 항상 고정값");
    }

    /// 1.1/1.3 트랙 선·커밋 점·브랜치 이름 라벨이 트랙마다 같은 색으로 묶여 나온다(가로 모드).
    #[test]
    fn tracks_get_distinct_colors_when_style_is_enabled() {
        let theme = Theme::dark();
        let lines = render(&parse("gitGraph\n commit\n branch develop\n commit\n"), &theme, 80).unwrap();
        let style_of = |line: &Line, needle: char| -> Style {
            line.runs().find(|(text, _)| text.contains(needle)).map(|(_, style)| style).unwrap()
        };
        let main_dot = style_of(&lines[0], COMMIT_DOT);
        let main_label = style_of(&lines[0], 'm');
        let develop_dot = style_of(&lines[1], COMMIT_DOT);
        let develop_label = style_of(&lines[1], 'd');
        assert_eq!(main_dot, theme.diagram_accent, "{lines:?}");
        assert_eq!(main_label, main_dot, "이름 라벨도 트랙 색을 따라야 한다: {lines:?}");
        assert_eq!(develop_dot, theme.diagram_box, "{lines:?}");
        assert_eq!(develop_label, develop_dot, "이름 라벨도 트랙 색을 따라야 한다: {lines:?}");
    }

    /// 2.1/3.1 트랙마다 선패턴이 달라 `--style none`에서도 색 없이 구분된다(가로 모드).
    #[test]
    fn horizontal_tracks_get_distinct_line_patterns_without_color() {
        let source = "gitGraph\n commit\n commit\n branch develop\n commit\n commit\n branch feature\n commit\n commit\n";
        let out = rows(source, 80);
        assert_eq!(out.len(), 3);
        assert!(out[0].contains('─'), "main(트랙0)은 실선: {out:?}");
        assert!(out[1].contains('╌'), "develop(트랙1)은 파선: {out:?}");
        assert!(out[2].contains('━'), "feature(트랙2)는 굵은선: {out:?}");
    }

    /// 1.3/2.1/3.1 세로 모드에서도 가로 모드와 같은 색·선패턴 규칙이 적용된다. main이 다른 두
    /// 트랙(develop·feature)의 커밋 여러 개를 사이에 두고 뒤늦게 다시 커밋해, main 자신의 세로선이
    /// 커밋 점에 가리지 않고 중간 줄에 그대로 드러나게 한다.
    #[test]
    fn vertical_tracks_get_distinct_line_patterns_without_color() {
        let source = "gitGraph TB:\n commit\n commit\n branch develop\n commit\n commit\n branch feature\n commit\n commit\n checkout main\n commit\n";
        let joined = rows(source, 80).join("\n");
        assert!(joined.contains('│'), "main(트랙0)은 실선: {joined}");
        assert!(joined.contains('╌'), "develop(트랙1)에서 갈라지는 파선 연결선: {joined}");
        assert!(joined.contains('━'), "feature(트랙2)에서 갈라지는 굵은선 연결선: {joined}");
    }

    /// 4.1/4.2 분기 연결선은 부모가 아니라 자식(새로 생기는) 트랙, 병합 연결선은 대상이 아니라
    /// source(합류해 들어오는) 트랙의 색을 따른다 — 여기선 둘 다 develop(트랙1)이라 main(트랙0)의
    /// `diagram_accent`가 아니라 develop의 `diagram_box`여야 한다.
    #[test]
    fn fork_and_merge_connectors_follow_owning_track_not_the_other_side() {
        let theme = Theme::dark();
        let source = "gitGraph\n commit\n branch develop\n commit\n checkout main\n merge develop\n";
        let lines = render(&parse(source), &theme, 80).unwrap();
        let corner_styles: Vec<Style> =
            lines[1].runs().filter(|(text, _)| text.contains(['╰', '╯'])).map(|(_, s)| s).collect();
        assert!(!corner_styles.is_empty(), "{lines:?}");
        assert!(
            corner_styles.iter().all(|s| *s == theme.diagram_box),
            "분기는 child(develop), 병합은 source(develop) 색을 따라야 한다(main 색 accent 아님): {lines:?}"
        );
    }

    /// 3.1(gitgraph-junction-polish) `fork_connector_start`: id가 비어 있으면 밀지 않고, 있으면
    /// 텍스트 시작 자리(점 뒤 2칸, gitgraph-dot-id-gap) + 텍스트 폭 + 공백 1칸만큼 민다.
    #[test]
    fn fork_connector_start_gap_only_when_id_is_not_empty() {
        assert_eq!(fork_connector_start(10, 0), 10, "빈 id면 밀지 않는다");
        assert_eq!(fork_connector_start(10, 1), 14, "폭 1인 id: 10 + 2(점 뒤 공백+텍스트 시작) + 1(글자) + 1(공백)");
        assert_eq!(fork_connector_start(10, 12), 25, "긴 id도 같은 규칙: 10 + 2 + 12 + 1");
    }

    /// 1.1/1.2 가로 모드 분기·병합 연결선의 끝점 모서리가 둥글게 그려지고, 각진 모서리는 남지
    /// 않는다.
    #[test]
    fn horizontal_fork_and_merge_corners_are_rounded() {
        let out = rows("gitGraph\n commit\n branch develop\n commit\n checkout main\n merge develop\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains('╰'), "분기 모서리는 둥글어야 한다: {joined}");
        assert!(joined.contains('╯'), "병합 모서리는 둥글어야 한다: {joined}");
        assert!(!joined.contains(['└', '┘']), "각진 모서리가 남아있으면 안 된다: {joined}");
    }

    /// 1.3/1.4 세로 모드 분기·병합 연결선의 끝점 모서리가 둥글게 그려지고, 각진 모서리는 남지
    /// 않는다.
    #[test]
    fn vertical_fork_and_merge_corners_are_rounded() {
        let out = rows("gitGraph TB:\n commit\n branch develop\n commit\n checkout main\n merge develop\n", 80);
        let joined = out.join("\n");
        assert!(joined.contains('╮'), "분기 모서리는 둥글어야 한다: {joined}");
        assert!(joined.contains('╯'), "병합 모서리는 둥글어야 한다: {joined}");
        assert!(!joined.contains(['┌', '┘']), "각진 모서리가 남아있으면 안 된다: {joined}");
    }

    /// 1.5 한 지점에서 브랜치 3개 이상이 갈라지는 T자 지점은 둥글게 바뀌지 않고 기존과 같은
    /// 문자(T자/교차)로 남는다 — `canvas.rs`의 `line_char`가 3비트 이상 조합에는 `round`를 반영
    /// 하지 않기 때문에 자동으로 보장된다.
    #[test]
    fn t_junction_at_a_shared_fork_point_is_not_rounded() {
        let source = "gitGraph\n commit id: \"root\"\n branch one\n branch two\n commit id: \"verylongcommitid\"\n checkout main\n merge two\n checkout one\n commit id: \"x\"\n";
        let out = rows(source, 100);
        let joined = out.join("\n");
        assert!(joined.contains('┣'), "T자 지점은 그대로 있어야 한다: {joined}");
        // 같은 줄의 실제 연결선 끝점(two 쪽)은 둥글게 바뀐다.
        assert!(joined.contains('╰'), "{joined}");
        assert!(joined.contains('╯'), "{joined}");
    }

    /// 2.1 세로 모드에서 분기가 부모의 최근 커밋과 같은 행에서 일어나고 그 커밋에 id가 있으면,
    /// id 글자와 분기 연결선 사이에 공백이 생긴다.
    #[test]
    fn vertical_fork_leaves_a_gap_after_a_labeled_parent_commit() {
        let out = rows("gitGraph TB:\n commit id: \"root-long-id\"\n branch develop\n commit id: \"dev1\"\n", 100);
        let row = out.iter().find(|l| l.contains("root-long-id")).expect("부모 커밋 줄이 있어야 한다");
        let after_id = row.split("root-long-id").nth(1).expect("id 뒤 내용이 있어야 한다");
        assert!(after_id.starts_with(' '), "id 글자 뒤에 공백이 있어야 한다: {row}");
    }

    /// 2.2 회귀: id가 없는 커밋(`commit`만)이면 기존처럼 점 글자 바로 뒤에 연결선이 이어진다
    /// (공백을 새로 넣지 않음).
    #[test]
    fn vertical_fork_does_not_shift_when_parent_commit_has_no_id() {
        let out = rows("gitGraph TB:\n commit\n branch develop\n commit\n", 100);
        let row = out.iter().find(|l| l.starts_with(COMMIT_DOT)).expect("main 커밋 줄이 있어야 한다");
        let after_dot: String = row.chars().skip(1).collect();
        assert!(after_dot.starts_with(['╌', '━', '─']), "빈 id면 점 바로 뒤에 선이 이어져야 한다: {row}");
    }
}
