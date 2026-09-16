//! mermaid `gantt` 파서.
//!
//! 작업 줄은 `이름 : [상태,] 메타` 꼴이고 메타는 쉼표로 나뉜다. 메타 칸은 차례가 아니라 생김새로 뜻이
//! 갈린다(날짜로 읽히면 시작일, `after …`면 앞선 작업, `5d`처럼 단위가 붙으면 기간, 나머지는 작업
//! 아이디). 그래서 `<id>, <날짜>, <기간>`과 `<날짜>, <날짜>`처럼 칸 수가 달라도 같은 코드가 읽는다.
//!
//! 읽어 낸 값은 [`GanttPlan`]에 담아 [`GanttPlan::schedule`]이 풀게 맡긴다. 여기서는 날짜를 기준일
//! 기준 일수로만 바꾼다.

use std::collections::HashMap;

use crate::diagram::layout::gantt::{
    DEFAULT_DATE_FORMAT, DEFAULT_DURATION_DAYS, GanttChart, GanttPlan, PlannedTask, StartRule, day_number_of, day_offset,
    duration_token_in_days, parse_date,
};
use crate::diagram::mermaid::text;

/// 작업 이름 뒤 메타 앞에 붙는 상태 낱말. `done`만 쓰이고 나머지는 흘려보낸다.
const STATUS_TAGS: &[&str] = &["done", "active", "crit", "critical", "milestone"];
/// 그리기에 쓰지 않는 지시어. 있어도 무너지지 않게 이름만 알아 두고 건너뛴다.
const IGNORED_DIRECTIVES: &[&str] = &[
    "accdescr", "acctitle", "axisformat", "click", "displaymode", "excludes", "includes", "inclusiveenddates", "tickinterval",
    "todaymarker", "topaxis", "vert", "weekday", "weekend",
];

/// mermaid `gantt` 소스를 간트 차트로 읽는다. 어떤 입력에도 패닉하지 않는다.
pub fn parse(source: &str) -> GanttChart {
    let mut reader = Reader::new();
    for line in text::clean_lines(source) {
        reader.read_line(line.trim());
    }
    reader.plan.baseline_day_number = reader.baseline;
    reader.plan.schedule()
}

struct Reader {
    plan: GanttPlan,
    /// 일수 0에 해당하는 절대 일련일. 처음 만난 날짜가 기준일이 된다.
    baseline: Option<i64>,
    date_format: String,
    section: usize,
    index_by_id: HashMap<String, usize>,
}

impl Reader {
    fn new() -> Reader {
        Reader {
            // 첫 `section`보다 앞에 오는 작업을 담을 이름 없는 섹션을 미리 둔다.
            plan: GanttPlan { section_names: vec![String::new()], ..GanttPlan::default() },
            baseline: None,
            date_format: DEFAULT_DATE_FORMAT.to_string(),
            section: 0,
            index_by_id: HashMap::new(),
        }
    }

    fn read_line(&mut self, line: &str) {
        let (keyword, rest) = split_keyword(line);
        match keyword.to_ascii_lowercase().as_str() {
            "gantt" => {}
            "title" => self.plan.title = text::label(rest),
            "dateformat" => self.date_format = rest.trim().to_string(),
            "section" => {
                self.plan.section_names.push(text::label(rest));
                self.section = self.plan.section_names.len() - 1;
            }
            ignored if IGNORED_DIRECTIVES.contains(&ignored) => {}
            _ => self.read_task(line),
        }
    }

    fn read_task(&mut self, line: &str) {
        let Some((name, meta)) = line.split_once(':') else {
            return;
        };
        let name = text::label(name);
        if name.is_empty() {
            return;
        }
        let fields: Vec<&str> = meta.split(',').map(str::trim).filter(|field| !field.is_empty()).collect();
        let tags = fields.iter().take_while(|field| is_status_tag(field)).count();
        let done = fields[..tags].iter().any(|tag| tag.eq_ignore_ascii_case("done"));
        let fields = &fields[tags..];

        let (start, duration, id) = match fields.iter().position(|field| self.is_start_field(field)) {
            Some(position) => {
                let start = self.start_rule(fields[position]);
                let duration = self.duration_after_start(&start, fields.get(position + 1).copied());
                (start, duration, position.checked_sub(1).and_then(|index| fields.get(index)))
            }
            // 시작을 안 밝힌 작업은 바로 앞 작업이 끝나는 자리에서 이어진다(mermaid 기본값).
            None => {
                let position = fields.iter().position(|field| duration_token_in_days(field).is_some());
                let duration = position.and_then(|index| duration_token_in_days(fields[index]));
                let id = match position {
                    Some(position) => position.checked_sub(1).and_then(|index| fields.get(index)),
                    None => fields.first(),
                };
                (StartRule::AfterPreviousTask, duration, id)
            }
        };

        if let Some(id) = id {
            self.index_by_id.insert(id.to_string(), self.plan.tasks.len());
        }
        self.plan.tasks.push(PlannedTask {
            name,
            section: self.section,
            duration_days: duration.unwrap_or(DEFAULT_DURATION_DAYS),
            done,
            start,
        });
    }

    fn is_start_field(&self, field: &str) -> bool {
        starts_with_word(field, "after") || self.day_of(field).is_some()
    }

    fn start_rule(&mut self, field: &str) -> StartRule {
        if starts_with_word(field, "after") {
            // 없는 아이디를 가리키면 빈 목록이 되어 기준일부터 시작한다.
            let targets = field.split_whitespace().skip(1).filter_map(|id| self.index_by_id.get(id).copied()).collect();
            return StartRule::AfterTasks(targets);
        }
        match self.day_of(field) {
            Some(day) => StartRule::Day(day_offset(&mut self.baseline, day)),
            None => StartRule::AfterPreviousTask,
        }
    }

    /// 시작 칸 다음 칸은 기간이거나 끝나는 날짜다. 끝나는 날짜는 시작일을 알 때만 기간으로 바뀐다.
    fn duration_after_start(&mut self, start: &StartRule, field: Option<&str>) -> Option<f64> {
        let field = field?;
        if let Some(days) = duration_token_in_days(field) {
            return Some(days);
        }
        let end = self.day_of(field)?;
        let end = day_offset(&mut self.baseline, end);
        match start {
            StartRule::Day(begin) => Some((end - begin).max(0.0)),
            _ => None,
        }
    }

    fn day_of(&self, field: &str) -> Option<i64> {
        day_number_of(parse_date(field, &self.date_format)?)
    }
}

fn is_status_tag(field: &str) -> bool {
    STATUS_TAGS.iter().any(|tag| field.eq_ignore_ascii_case(tag))
}

fn starts_with_word(text: &str, word: &str) -> bool {
    let mut parts = text.split_whitespace();
    parts.next().is_some_and(|first| first.eq_ignore_ascii_case(word)) && parts.next().is_some()
}

/// 첫 낱말과 나머지로 가른다.
fn split_keyword(line: &str) -> (&str, &str) {
    match line.split_once(char::is_whitespace) {
        Some((keyword, rest)) => (keyword, rest),
        None => (line, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::layout::gantt::{CivilDate, GanttTask};

    fn tasks(chart: &GanttChart) -> Vec<&GanttTask> {
        chart.sections.iter().flat_map(|section| &section.tasks).collect()
    }

    const SAMPLE: &str = "\
gantt
    dateFormat YYYY-MM-DD
    title Project Timeline
    excludes weekends
    section Development
    Task1 :t1, 2024-01-01, 5d
    Task2 :crit, after t1, 3d
    milestone :m1, after t2, 1d";

    // 4.1 제목, 2.1 섹션, 2.2 기간, 3.1 after, 3.3 없는 아이디.
    #[test]
    fn reads_the_documented_grammar() {
        let chart = parse(SAMPLE);
        assert_eq!(chart.title, "Project Timeline");
        assert_eq!(chart.sections.len(), 1);
        assert_eq!(chart.sections[0].name, "Development");
        let tasks = tasks(&chart);
        assert_eq!(tasks.iter().map(|task| task.name.as_str()).collect::<Vec<_>>(), vec!["Task1", "Task2", "milestone"]);
        assert_eq!((tasks[0].start_day, tasks[0].duration_days), (0.0, 5.0));
        assert_eq!((tasks[1].start_day, tasks[1].duration_days), (5.0, 3.0));
        // `t2`는 없는 아이디라 기준일부터 시작한다.
        assert_eq!((tasks[2].start_day, tasks[2].duration_days), (0.0, 1.0));
        assert_eq!(chart.baseline_day_number, day_number_of(CivilDate { year: 2024, month: 1, day: 1 }));
    }

    // 2.1 섹션마다 따로 묶인다.
    #[test]
    fn sections_split_tasks() {
        let chart = parse("gantt\nsection A\nx :a1, 2024-01-01, 2d\nsection B\ny :b1, after a1, 2d");
        assert_eq!(chart.sections.iter().map(|section| section.name.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
        assert_eq!(chart.sections[1].tasks[0].start_day, 2.0);
        // 섹션보다 앞에 오는 작업은 이름 없는 섹션에 담긴다.
        let chart = parse("gantt\nfirst :f1, 2024-01-01, 1d\nsection A\nx :a1, after f1, 1d");
        assert_eq!(chart.sections[0].name, "");
        assert_eq!(chart.sections[1].name, "A");
    }

    // 3.2 시작일을 직접 적으면 그 자리에서 시작한다. 메타 차례가 달라도 읽는다.
    #[test]
    fn metadata_variants_all_parse() {
        let chart = parse(
            "gantt\n\
             a :2024-01-01, 2024-01-11\n\
             b :id2, 2024-01-05, 3d\n\
             c :2024-01-20, 2d\n\
             d :id4, after id2, 2024-01-15\n\
             e :4d",
        );
        let tasks = tasks(&chart);
        assert_eq!((tasks[0].start_day, tasks[0].duration_days), (0.0, 10.0));
        assert_eq!((tasks[1].start_day, tasks[1].duration_days), (4.0, 3.0));
        assert_eq!((tasks[2].start_day, tasks[2].duration_days), (19.0, 2.0));
        // 시작이 앞선 작업이라 끝나는 날짜로는 기간을 알 수 없어 기본값이 된다.
        assert_eq!((tasks[3].start_day, tasks[3].duration_days), (7.0, 1.0));
        // 시작을 안 밝히면 바로 앞 작업이 끝난 자리다.
        assert_eq!((tasks[4].start_day, tasks[4].duration_days), (8.0, 4.0));
    }

    // 2.3 dateFormat이 정한 형식으로 날짜를 읽는다.
    #[test]
    fn date_format_directive_changes_parsing() {
        let chart = parse("gantt\ndateFormat DD-MM-YYYY\na :a1, 05-01-2024, 3d\nb :b1, 10-01-2024, 1d");
        let tasks = tasks(&chart);
        assert_eq!(tasks[0].start_day, 0.0);
        assert_eq!(tasks[1].start_day, 5.0);
        assert_eq!(chart.baseline_day_number, day_number_of(CivilDate { year: 2024, month: 1, day: 5 }));
    }

    #[test]
    fn done_tag_is_kept_and_other_tags_are_ignored() {
        let chart = parse("gantt\na :done, a1, 2024-01-01, 2d\nb :crit, active, b1, 2024-01-03, 2d\nc :milestone, done, c1, 2d");
        let tasks = tasks(&chart);
        assert_eq!(tasks.iter().map(|task| task.done).collect::<Vec<_>>(), vec![true, false, true]);
    }

    // 5.3 이상한 입력에도 패닉하지 않는다.
    #[test]
    fn odd_sources_do_not_panic() {
        let cases = [
            "gantt",
            "gantt\nsection",
            "gantt\n:",
            "gantt\na :",
            "gantt\n: 2024-01-01, 5d",
            "gantt\ntitle\ndateFormat\nsection\n",
            "gantt\na :after\nb :after a\n",
            "gantt\na :a1, 9999-12-31, 3650000d",
            "gantt\na :a1, after a1, 2d",
            "gantt\na :a1, 2024-99-99, x\n",
            "gantt\n아주 긴 작업 이름입니다 정말로 길게 써 봅니다 :x1, 2024-01-01, 1000w",
            "gantt\n%% 주석만",
        ];
        for source in cases {
            let _ = parse(source);
        }
    }
}
