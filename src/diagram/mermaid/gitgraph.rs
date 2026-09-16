//! mermaid `gitGraph` 파서.

use super::text::{clean_lines, keyword, label};

/// 선언 없이 처음부터 열려 있는 브랜치.
const DEFAULT_BRANCH: &str = "main";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitGraph {
    /// 브랜치(트랙) 이름. 등장 순서대로이며 0번은 언제나 `main`이다.
    pub tracks: Vec<String>,
    pub events: Vec<GitEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitEvent {
    Commit { track: usize, id: String },
    /// `parent`에서 갈라져 나온 새 트랙. 갈라진 뒤 활성 브랜치가 된다.
    Fork { track: usize, parent: usize },
    Checkout { track: usize },
    /// `source`가 `None`이면 선언된 적 없는 브랜치라 연결선 없이 병합 커밋만 남는다.
    Merge { target: usize, source: Option<usize> },
}

impl Default for GitGraph {
    fn default() -> GitGraph {
        GitGraph { tracks: vec![DEFAULT_BRANCH.to_string()], events: Vec::new() }
    }
}

pub fn parse(source: &str) -> GitGraph {
    let mut graph = GitGraph::default();
    let mut active = 0usize;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let first = keyword(trimmed);
        let rest = trimmed[first.len()..].trim();
        let lower = first.to_ascii_lowercase();
        match lower.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-') {
            "commit" => graph.events.push(GitEvent::Commit { track: active, id: attribute(rest, "id") }),
            "branch" | "checkout" | "switch" => {
                let name = first_value(rest);
                if name.is_empty() {
                    continue;
                }
                active = match graph.tracks.iter().position(|t| *t == name) {
                    Some(track) => {
                        graph.events.push(GitEvent::Checkout { track });
                        track
                    }
                    // 선언되지 않은 브랜치로 `checkout` 해도 오류 대신 지금 위치에서 갈라진 새 트랙으로 본다.
                    None => {
                        graph.tracks.push(name);
                        let track = graph.tracks.len() - 1;
                        graph.events.push(GitEvent::Fork { track, parent: active });
                        track
                    }
                };
            }
            "merge" => {
                let name = first_value(rest);
                let source = graph.tracks.iter().position(|t| *t == name).filter(|s| *s != active);
                graph.events.push(GitEvent::Merge { target: active, source });
            }
            _ => {}
        }
    }
    graph
}

/// `commit id: "init"` 같은 `키: 값` 쌍을 찾는다. 없으면 빈 문자열.
fn attribute(rest: &str, key: &str) -> String {
    let lower = rest.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(key) {
        let start = from + found;
        let after = start + key.len();
        let standalone = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        if standalone && let Some(value) = rest[after..].trim_start().strip_prefix(':') {
            return first_value(value);
        }
        from = after;
    }
    String::new()
}

/// 첫 값 하나. 따옴표로 묶였으면 그 안을, 아니면 첫 낱말을 돌려준다.
fn first_value(rest: &str) -> String {
    let rest = rest.trim();
    if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'')
        && let Some(end) = rest[1..].find(quote)
    {
        return label(&rest[..end + 2]);
    }
    label(rest.split(char::is_whitespace).next().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commits_branches_and_merges() {
        let graph = parse("gitGraph\n   commit\n   branch develop\n   checkout develop\n   commit\n   checkout main\n   merge develop\n   commit\n");
        assert_eq!(graph.tracks, vec!["main", "develop"]);
        assert_eq!(
            graph.events,
            vec![
                GitEvent::Commit { track: 0, id: String::new() },
                GitEvent::Fork { track: 1, parent: 0 },
                GitEvent::Checkout { track: 1 },
                GitEvent::Commit { track: 1, id: String::new() },
                GitEvent::Checkout { track: 0 },
                GitEvent::Merge { target: 0, source: Some(1) },
                GitEvent::Commit { track: 0, id: String::new() },
            ]
        );
    }

    #[test]
    fn reads_commit_id_and_ignores_other_attributes() {
        let graph = parse("gitGraph:\n  commit id: \"init\" tag: \"v1\"\n  commit type: HIGHLIGHT\n  commit tag:\"valid\" id:\"second\"\n");
        assert_eq!(
            graph.events,
            vec![
                GitEvent::Commit { track: 0, id: "init".into() },
                GitEvent::Commit { track: 0, id: String::new() },
                GitEvent::Commit { track: 0, id: "second".into() },
            ]
        );
    }

    #[test]
    fn switch_is_an_alias_for_checkout() {
        let graph = parse("gitGraph\n commit\n branch feature\n checkout main\n switch feature\n commit\n");
        assert_eq!(graph.events.last(), Some(&GitEvent::Commit { track: 1, id: String::new() }));
    }

    #[test]
    fn undeclared_names_do_not_abort_the_graph() {
        let graph = parse("gitGraph\n commit\n merge ghost\n checkout phantom\n commit\n cherry-pick id:\"x\"\n");
        assert_eq!(graph.tracks, vec!["main", "phantom"]);
        assert_eq!(graph.events[1], GitEvent::Merge { target: 0, source: None });
        assert_eq!(graph.events[2], GitEvent::Fork { track: 1, parent: 0 });
        assert_eq!(graph.events[3], GitEvent::Commit { track: 1, id: String::new() });
    }

    #[test]
    fn quoted_branch_names_keep_their_spaces() {
        let graph = parse("gitGraph\n branch \"my feature\"\n commit\n");
        assert_eq!(graph.tracks, vec!["main", "my feature"]);
    }
}
