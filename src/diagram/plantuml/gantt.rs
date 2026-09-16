//! PlantUML `@startgantt` 파서.
//!
//! 문장은 대개 `[작업] <서술>` 꼴이고, `then`으로 한 줄에 두 문장을 이을 수 있다. 그래서 줄을 먼저
//! `then`으로 가른 뒤 마디마다 같은 방식으로 읽는다. `[A] -> [B]`와 `[B] starts at [A]'s end`는 뜻이
//! 같아 둘 다 같은 의존으로 담는다.
//!
//! 작업은 처음 이름이 나온 자리에서 만들어진다. `[A] -> [B]`가 `[B] requires …`보다 먼저 와도 되기
//! 때문이다. 시작 시점은 [`GanttPlan::schedule`]이 나중에 한꺼번에 푼다. PlantUML에는 섹션도 완료
//! 표시도 없으므로 작업은 모두 이름 없는 섹션 하나에 담긴다.

use std::collections::HashMap;

use crate::diagram::layout::gantt::{
    DEFAULT_DATE_FORMAT, DEFAULT_DURATION_DAYS, GanttChart, GanttPlan, PlannedTask, StartRule, day_number_of, day_offset,
    duration_phrase_in_days, parse_date,
};
use crate::diagram::plantuml::text;

/// PlantUML `@startgantt` 소스를 간트 차트로 읽는다. 어떤 입력에도 패닉하지 않는다.
pub fn parse(source: &str) -> GanttChart {
    let lines = text::clean_lines(source);
    let mut reader = Reader::new(baseline_of(&lines));
    let mut skip_until: Option<&str> = None;
    for line in &lines {
        let trimmed = line.trim();
        let lowered = trimmed.to_ascii_lowercase();
        if let Some(terminator) = skip_until {
            if lowered.starts_with(terminator) {
                skip_until = None;
            }
            continue;
        }
        if lowered.starts_with("<style") {
            skip_until = Some("</style>");
            continue;
        }
        if lowered == "note" || lowered.starts_with("note ") {
            if !lowered.contains("end note") {
                skip_until = Some("end note");
            }
            continue;
        }
        if trimmed.starts_with("--") || lowered.starts_with("end note") || is_project_start(trimmed).is_some() {
            continue;
        }
        if let Some(rest) = strip_prefix_ci(trimmed, "title ") {
            reader.plan.title = text::label(rest);
            continue;
        }
        reader.read_statement(trimmed);
    }
    reader.plan.baseline_day_number = reader.baseline;
    reader.plan.schedule()
}

/// `Project starts <날짜>`를 먼저 훑어 기준일을 잡는다.
///
/// 본문을 읽기 전에 정해 둬야 뒤에 오든 앞에 오든 같은 날이 일수 0이 된다.
fn baseline_of(lines: &[String]) -> Option<i64> {
    let date = lines.iter().find_map(|line| is_project_start(line.trim()))?;
    let date = strip_prefix_ci(date.trim(), "the ").unwrap_or(date).trim();
    day_number_of(parse_date(date, DEFAULT_DATE_FORMAT)?)
}

fn is_project_start(line: &str) -> Option<&str> {
    let rest = strip_prefix_ci(line, "project ")?;
    strip_prefix_ci(rest.trim_start(), "starts").or_else(|| strip_prefix_ci(rest.trim_start(), "start"))
}

struct Reader {
    plan: GanttPlan,
    /// 일수 0에 해당하는 절대 일련일.
    baseline: Option<i64>,
    index_by_name: HashMap<String, usize>,
    /// 줄머리 `then`이 이어붙일 바로 앞 작업.
    last_task: Option<usize>,
}

impl Reader {
    fn new(baseline: Option<i64>) -> Reader {
        Reader {
            // PlantUML에는 섹션이 없으므로 이름 없는 섹션 하나에 모두 담는다.
            plan: GanttPlan { section_names: vec![String::new()], ..GanttPlan::default() },
            baseline,
            index_by_name: HashMap::new(),
            last_task: None,
        }
    }

    fn read_statement(&mut self, statement: &str) {
        let mut previous = if find_keyword(statement, "then") == Some(0) { self.last_task } else { None };
        for clause in split_on_keyword(statement, "then") {
            let Some(index) = self.read_clause(&clause, previous) else {
                continue;
            };
            previous = Some(index);
            self.last_task = Some(index);
        }
    }

    /// 마디 하나를 읽고 그 마디가 다룬 작업 번호를 돌려준다. `[이름]`으로 시작하지 않으면 `None`이다.
    fn read_clause(&mut self, clause: &str, previous: Option<usize>) -> Option<usize> {
        let (name, rest) = split_bracketed(clause)?;
        let index = self.task_index(&name);
        if let Some(previous) = previous.filter(|previous| *previous != index) {
            self.plan.tasks[index].depend_on(previous);
        }
        let rest = rest.trim();
        if let Some(target) = arrow_target(rest) {
            let target = self.task_index(&target);
            if target != index {
                self.plan.tasks[target].depend_on(index);
            }
            return Some(target);
        }
        if let Some(phrase) = strip_prefix_ci(rest, "requires ").or_else(|| strip_prefix_ci(rest, "lasts ")) {
            if let Some(days) = duration_phrase_in_days(phrase) {
                self.plan.tasks[index].duration_days = days;
            }
        } else if let Some(spec) = strip_prefix_ci(rest, "starts ") {
            self.read_start(index, spec);
        } else if let Some(alias) = strip_prefix_ci(rest, "as ").and_then(|rest| split_bracketed(rest)) {
            self.index_by_name.insert(alias.0, index);
        }
        Some(index)
    }

    /// `starts` 뒤의 시작 시점. 날짜·`D+N`·다른 작업 참조 셋 중 하나다.
    fn read_start(&mut self, index: usize, spec: &str) {
        let spec = spec.trim();
        let spec = strip_prefix_ci(spec, "at ").unwrap_or(spec).trim();
        // `3 days after [X]'s end`처럼 앞에 덧말이 붙으면 참조만 본다(며칠 뒤인지는 안 따진다).
        let spec = match find_keyword(spec, "after") {
            Some(position) => spec[position + "after".len()..].trim(),
            None => spec,
        };
        if let Some((target, rest)) = split_bracketed(spec) {
            let target = self.task_index(&target);
            if target == index {
                return;
            }
            if rest.trim_start().to_ascii_lowercase().starts_with("'s start") {
                self.plan.tasks[index].start = StartRule::WithTask(target);
            } else {
                self.plan.tasks[index].depend_on(target);
            }
            return;
        }
        if let Some(offset) = strip_prefix_ci(spec, "D+") {
            if let Ok(days) = offset.trim().parse::<f64>() {
                self.plan.tasks[index].start = StartRule::Day(days);
            }
            return;
        }
        let spec = strip_prefix_ci(spec, "the ").unwrap_or(spec).trim();
        if let Some(day) = parse_date(spec, DEFAULT_DATE_FORMAT).and_then(day_number_of) {
            self.plan.tasks[index].start = StartRule::Day(day_offset(&mut self.baseline, day));
        }
    }

    /// 이름으로 작업을 찾고, 처음 보는 이름이면 만든다.
    fn task_index(&mut self, name: &str) -> usize {
        let name = text::label(name);
        if let Some(index) = self.index_by_name.get(&name) {
            return *index;
        }
        let index = self.plan.tasks.len();
        self.index_by_name.insert(name.clone(), index);
        self.plan.tasks.push(PlannedTask {
            name,
            section: 0,
            duration_days: DEFAULT_DURATION_DAYS,
            done: false,
            start: StartRule::Baseline,
        });
        index
    }
}

/// `[이름] 나머지`를 (이름, 나머지)로 가른다.
fn split_bracketed(text: &str) -> Option<(String, &str)> {
    let rest = text.trim_start().strip_prefix('[')?;
    let end = rest.find(']')?;
    Some((rest[..end].trim().to_string(), &rest[end + 1..]))
}

/// `-> [B]` / `--> [B]` 꼴에서 화살표가 가리키는 작업 이름.
fn arrow_target(rest: &str) -> Option<String> {
    let arrow = rest.find("->")?;
    // 화살표 앞에는 선 글자만 온다. 다른 글자가 끼어 있으면 화살표 문장이 아니다.
    if !rest[..arrow].chars().all(|c| matches!(c, '-' | '.' | ' ')) {
        return None;
    }
    split_bracketed(&rest[arrow + 2..]).map(|(name, _)| name)
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len()).filter(|head| head.eq_ignore_ascii_case(prefix)).map(|_| &text[prefix.len()..])
}

fn split_on_keyword(text: &str, keyword: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(position) = find_keyword(rest, keyword) {
        parts.push(rest[..position].trim().to_string());
        rest = &rest[position + keyword.len()..];
    }
    parts.push(rest.trim().to_string());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

/// 대괄호 밖에서 낱말 하나로 떨어져 있는 `keyword`의 자리.
fn find_keyword(text: &str, keyword: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for index in 0..text.len() {
        match bytes[index] {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => {
                let matched = text.get(index..index + keyword.len()).is_some_and(|word| word.eq_ignore_ascii_case(keyword));
                let before_free = index == 0 || bytes[index - 1].is_ascii_whitespace();
                let after_free = bytes.get(index + keyword.len()).is_none_or(u8::is_ascii_whitespace);
                if matched && before_free && after_free {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::layout::gantt::{CivilDate, GanttTask};

    fn tasks(chart: &GanttChart) -> Vec<&GanttTask> {
        chart.sections.iter().flat_map(|section| &section.tasks).collect()
    }

    fn spans(chart: &GanttChart) -> Vec<(String, f64, f64)> {
        tasks(chart).iter().map(|task| (task.name.clone(), task.start_day, task.duration_days)).collect()
    }

    // 6.2 requires(주·일 결합 포함), 6.5 Project starts, 4.1 title.
    #[test]
    fn durations_and_baseline_are_read() {
        let chart = parse(
            "@startgantt\n\
             title My Title\n\
             Project starts 2020-07-01\n\
             [Task name] requires 10 days\n\
             [Other] requires 1 week and 4 days\n\
             @endgantt",
        );
        assert_eq!(chart.title, "My Title");
        assert_eq!(chart.baseline_day_number, day_number_of(CivilDate { year: 2020, month: 7, day: 1 }));
        assert_eq!(spans(&chart), vec![("Task name".into(), 0.0, 10.0), ("Other".into(), 0.0, 11.0)]);
        assert_eq!(chart.sections.len(), 1);
        assert_eq!(chart.sections[0].name, "");
    }

    // 6.4 then 연결, 6.3 starts 날짜·D+N, `starts at [X]'s end`, `->`.
    #[test]
    fn dependencies_and_explicit_starts() {
        let chart = parse(
            "@startgantt\n\
             Project starts 2020-07-01\n\
             [T1] requires 10 days then [T2] requires 5 days\n\
             [T3] starts at [T2]'s end\n\
             [T3] requires 2 days\n\
             [T4] starts 2020-07-06\n\
             [T4] requires 1 day\n\
             [T5] starts D+14\n\
             [T1] -> [T6]\n\
             [T6] requires 3 days\n\
             [T7] starts at [T4]'s start\n\
             @endgantt",
        );
        assert_eq!(
            spans(&chart),
            vec![
                ("T1".into(), 0.0, 10.0),
                ("T2".into(), 10.0, 5.0),
                ("T3".into(), 15.0, 2.0),
                ("T4".into(), 5.0, 1.0),
                ("T5".into(), 14.0, 1.0),
                ("T6".into(), 10.0, 3.0),
                ("T7".into(), 5.0, 1.0),
            ]
        );
    }

    // 6.3 Project starts가 없으면 처음 만난 날짜가 기준일이 된다.
    #[test]
    fn first_date_becomes_the_baseline_without_project_start() {
        let chart = parse("@startgantt\n[a] starts 2024-03-01\n[a] requires 2 days\n[b] starts 2024-03-05\n@endgantt");
        assert_eq!(chart.baseline_day_number, day_number_of(CivilDate { year: 2024, month: 3, day: 1 }));
        assert_eq!(spans(&chart), vec![("a".into(), 0.0, 2.0), ("b".into(), 4.0, 1.0)]);
    }

    // 6.6 같은 일정을 mermaid로 적은 것과 결과가 같다.
    #[test]
    fn matches_the_equivalent_mermaid_schedule() {
        let plantuml = parse(
            "@startgantt\n\
             Project starts 2024-01-01\n\
             [설계] requires 5 days\n\
             [설계] -> [구현]\n\
             [구현] requires 1 week\n\
             @endgantt",
        );
        let mermaid = crate::diagram::mermaid::gantt::parse(
            "gantt\n\
             dateFormat YYYY-MM-DD\n\
             설계 :a1, 2024-01-01, 5d\n\
             구현 :a2, after a1, 1w",
        );
        assert_eq!(spans(&plantuml), spans(&mermaid));
        assert_eq!(plantuml.baseline_day_number, mermaid.baseline_day_number);
    }

    #[test]
    fn decorations_and_blocks_are_ignored() {
        let chart = parse(
            "@startgantt\n\
             ' 주석\n\
             /' 여러 줄\n\
             주석 '/\n\
             <style>\n\
             ganttDiagram { }\n\
             </style>\n\
             Project starts 2024-01-01\n\
             saturday are closed\n\
             2024-01-10 is closed\n\
             -- 구분선 --\n\
             [a] requires 3 days\n\
             [a] is colored in GreenYellow\n\
             [a] is 50% completed\n\
             [a] on {Alice}\n\
             note bottom\n\
             [b] requires 99 days\n\
             end note\n\
             [b] requires 4 days\n\
             @endgantt",
        );
        assert_eq!(spans(&chart), vec![("a".into(), 0.0, 3.0), ("b".into(), 0.0, 4.0)]);
    }

    // 5.3 이상한 입력에도 패닉하지 않는다.
    #[test]
    fn odd_sources_do_not_panic() {
        let cases = [
            "@startgantt\n@endgantt",
            "@startgantt\n[\n@endgantt",
            "@startgantt\n[] requires\n@endgantt",
            "@startgantt\nthen [a] requires 2 days\n@endgantt",
            "@startgantt\n[a] -> [a]\n[a] starts at [a]'s end\n@endgantt",
            "@startgantt\n[a] -> [b]\n[b] -> [a]\n@endgantt",
            "@startgantt\n[한글 작업] requires 1 week and 3 days then [다음] requires 2 days\n@endgantt",
            "@startgantt\n[a] starts D+\n[a] starts D+abc\n@endgantt",
            "@startgantt\nProject starts 없는날짜\n[a] requires 999999999 days\n@endgantt",
            "@startgantt\n<style>\n@endgantt",
            "@startgantt\nnote\n@endgantt",
        ];
        for source in cases {
            let _ = parse(source);
        }
    }
}
