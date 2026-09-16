//! 간트 차트의 언어 중립 모형·일정 계산·렌더링.
//!
//! mermaid `gantt`와 PlantUML `@startgantt`는 문법만 다를 뿐 담는 뜻이 같다. 그래서 두 파서는 여기
//! 있는 [`GanttPlan`]을 채워 [`GanttPlan::schedule`]로 [`GanttChart`]를 만들고, 그리기는 모두
//! [`render`] 하나를 거친다([`crate::diagram::layout::graph`]가 여러 언어의 그래프를 함께 그리는 것과
//! 같은 관례다).
//!
//! 층은 넷이다.
//!
//! 1. 날짜 셈: [`parse_date`]·[`day_number_of`]·[`date_of_day_number`]·[`format_date`]. 달력 크레이트를
//!    쓰지 않으므로 그레고리력 일련일을 직접 센다.
//! 2. 기간 읽기: [`duration_token_in_days`](mermaid `5d`)·[`duration_phrase_in_days`](PlantUML
//!    `1 week and 4 days`). 두 문법이 같은 "일수" 단위로 모인다.
//! 3. 일정 풀기: [`StartRule`]과 [`GanttPlan::schedule`]. `after`·`->`·`then` 같은 앞뒤 참조를 되풀이
//!    완화로 푼다. 없는 작업을 가리키거나 참조가 고리를 이뤄도 멈춘다.
//! 4. 그리기: [`render`]. 날짜를 기준일로부터의 일수로 바꾼 뒤
//!    [`crate::diagram::layout::chart::Axis`]에 맡겨 칸을 정한다.

use crate::diagram::layout::chart::{Axis, render_horizontal_axis};
use crate::line::{Line, Span};
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

/// mermaid `dateFormat`을 안 줬을 때 쓰는 날짜 형식. PlantUML은 이 형식만 쓴다.
pub const DEFAULT_DATE_FORMAT: &str = "YYYY-MM-DD";
/// 기간을 안 밝힌 작업의 기본 길이.
pub const DEFAULT_DURATION_DAYS: f64 = 1.0;
/// [`render`]가 그릴 수 있는 가장 좁은 폭.
pub const MIN_GANTT_WIDTH: usize = MIN_NAME_WIDTH + 1 + MIN_BAR_FIELD;

/// 작업 이름 칸의 최소 폭.
const MIN_NAME_WIDTH: usize = 3;
/// 막대밭의 최소 폭.
const MIN_BAR_FIELD: usize = 8;
/// 섹션 이름이 있을 때 작업 줄을 들여쓰는 칸 수.
const TASK_INDENT: usize = 2;
/// 막대 줄의 최대 개수. 넘치면 마지막 한 줄로 접는다.
const MAX_TASK_ROWS: usize = 40;
/// 일정 풀기를 되풀이하는 최대 횟수. 참조가 고리를 이뤄도 여기서 멈춘다.
const MAX_SCHEDULE_PASSES: usize = 64;
/// 타임라인이 다룰 수 있는 일수 상한. 이보다 크면 잘라 넣어 축 계산이 무한대로 넘치지 않게 한다.
const MAX_TIMELINE_DAYS: f64 = 1e6;
/// [`date_of_day_number`]가 받아들이는 일련일 범위(대략 서기 1년~9999년).
const MAX_DAY_NUMBER: i64 = 3_000_000;

/// 아직 안 끝난 작업 막대.
const PENDING_FILL: char = '█';
/// 끝난 작업 막대. 색 없는 테마에서도 글자로 구분된다.
const DONE_FILL: char = '▓';

// ── 모형 ────────────────────────────────────────────────────────────────────

/// 그릴 준비가 끝난 간트 차트. 모든 날짜가 기준일로부터의 일수로 풀려 있다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GanttChart {
    /// 차트 위에 얹을 제목. 없으면 빈 문자열이다.
    pub title: String,
    /// 섹션 차례. PlantUML처럼 섹션이 없는 문법은 이름 없는 섹션 하나만 갖는다.
    pub sections: Vec<GanttSection>,
    /// 일수 0에 해당하는 절대 일련일. 소스에 날짜가 없으면 `None`이다.
    pub baseline_day_number: Option<i64>,
}

/// 작업 묶음 하나.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GanttSection {
    /// 섹션 이름. 비어 있으면 머리줄을 그리지 않는다.
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

/// 작업 하나. 시작과 길이를 모두 일수로 갖는다.
#[derive(Clone, Debug, PartialEq)]
pub struct GanttTask {
    pub name: String,
    /// 기준일로부터 며칠 뒤에 시작하는지. 기준일보다 앞서면 음수다.
    pub start_day: f64,
    /// 며칠 걸리는지. 0이어도 막대는 한 칸 보인다.
    pub duration_days: f64,
    /// 끝난 작업인지(mermaid `done` 표시).
    pub done: bool,
}

// ── 일정 풀기 ───────────────────────────────────────────────────────────────

/// 작업이 언제 시작하는지 밝히는 방법.
#[derive(Clone, Debug, PartialEq)]
pub enum StartRule {
    /// 기준일로부터 며칠 뒤.
    Day(f64),
    /// 나열한 작업이 모두 끝난 뒤. 비어 있으면 기준일이다(없는 작업을 가리켰을 때).
    AfterTasks(Vec<usize>),
    /// 소스 차례로 바로 앞 작업이 끝난 뒤(mermaid에서 시작을 안 밝힌 작업).
    AfterPreviousTask,
    /// 다른 작업과 같은 날 시작(PlantUML `starts at [X]'s start`).
    WithTask(usize),
    /// 밝힌 것이 없으면 기준일.
    Baseline,
}

/// 파서가 채우는 중간 모형. 시작 시점이 아직 규칙으로만 적혀 있다.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedTask {
    pub name: String,
    /// [`GanttPlan::section_names`]의 몇 번째 섹션에 드는지.
    pub section: usize,
    pub duration_days: f64,
    pub done: bool,
    pub start: StartRule,
}

/// 두 파서가 함께 채우는 파싱 결과.
#[derive(Clone, Debug, Default)]
pub struct GanttPlan {
    pub title: String,
    /// 일수 0에 해당하는 절대 일련일.
    pub baseline_day_number: Option<i64>,
    pub section_names: Vec<String>,
    /// 소스에 적힌 차례 그대로의 작업 목록.
    pub tasks: Vec<PlannedTask>,
}

impl GanttPlan {
    /// 시작 규칙을 실제 일수로 풀어 그릴 수 있는 차트로 만든다.
    ///
    /// 앞뒤 어느 쪽을 가리켜도 풀리도록 작업 수만큼(최대 [`MAX_SCHEDULE_PASSES`]번) 되풀이해 값을
    /// 다듬고, 더 바뀌지 않으면 멈춘다. 참조가 고리를 이뤄도 횟수 상한에서 끝나므로 멎지 않는 일이 없다.
    pub fn schedule(self) -> GanttChart {
        let count = self.tasks.len();
        let mut starts = vec![0.0f64; count];
        let mut ends = vec![0.0f64; count];
        for _ in 0..count.clamp(1, MAX_SCHEDULE_PASSES) {
            let mut changed = false;
            for index in 0..count {
                let task = &self.tasks[index];
                let start = match &task.start {
                    StartRule::Day(day) => *day,
                    StartRule::Baseline => 0.0,
                    StartRule::AfterPreviousTask => index.checked_sub(1).map_or(0.0, |previous| ends[previous]),
                    StartRule::WithTask(target) => starts.get(*target).copied().unwrap_or(0.0),
                    StartRule::AfterTasks(targets) => {
                        targets.iter().filter_map(|target| ends.get(*target)).copied().fold(0.0f64, f64::max)
                    }
                };
                let start = clamp_days(start);
                let end = clamp_days(start + clamp_days(task.duration_days).max(0.0));
                changed |= starts[index] != start || ends[index] != end;
                starts[index] = start;
                ends[index] = end;
            }
            if !changed {
                break;
            }
        }

        let mut sections: Vec<GanttSection> =
            self.section_names.iter().map(|name| GanttSection { name: name.clone(), tasks: Vec::new() }).collect();
        for (index, task) in self.tasks.into_iter().enumerate() {
            let Some(section) = sections.get_mut(task.section) else {
                continue;
            };
            section.tasks.push(GanttTask {
                name: task.name,
                start_day: starts[index],
                duration_days: (ends[index] - starts[index]).max(0.0),
                done: task.done,
            });
        }
        sections.retain(|section| !section.tasks.is_empty());
        GanttChart { title: self.title, sections, baseline_day_number: self.baseline_day_number }
    }
}

impl PlannedTask {
    /// 아직 시작일을 못 박지 않았다면 `target`이 끝난 뒤로 미룬다.
    ///
    /// 절대 시작일을 이미 적은 작업은 건드리지 않는다. PlantUML에서 `[A] -> [B]`와 `[B] starts 날짜`가
    /// 함께 있으면 적어 둔 날짜가 뜻이 더 뚜렷하기 때문이다.
    pub fn depend_on(&mut self, target: usize) {
        match &mut self.start {
            StartRule::AfterTasks(targets) => {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
            StartRule::Baseline | StartRule::AfterPreviousTask => self.start = StartRule::AfterTasks(vec![target]),
            StartRule::Day(_) | StartRule::WithTask(_) => {}
        }
    }
}

/// 절대 일련일을 기준일 기준 일수로 바꾼다. 기준일이 아직 없으면 이 날짜가 기준일이 된다.
pub fn day_offset(baseline: &mut Option<i64>, day_number: i64) -> f64 {
    let base = *baseline.get_or_insert(day_number);
    day_number.saturating_sub(base) as f64
}

fn clamp_days(days: f64) -> f64 {
    if days.is_finite() { days.clamp(-MAX_TIMELINE_DAYS, MAX_TIMELINE_DAYS) } else { 0.0 }
}

// ── 날짜 셈 ─────────────────────────────────────────────────────────────────

/// 그레고리력 날짜.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CivilDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateField {
    Year,
    Month,
    Day,
}

/// 날짜를 1970-01-01을 0으로 하는 일련일로 바꾼다. 달력에 없는 날짜면 `None`이다.
///
/// 시간대·윤초는 따지지 않는다. 간트 차트는 날짜 사이의 "며칠"만 알면 되기 때문이다.
pub fn day_number_of(date: CivilDate) -> Option<i64> {
    if !(1..=9999).contains(&date.year) || !(1..=12).contains(&date.month) {
        return None;
    }
    if date.day < 1 || date.day > days_in_month(date.year, date.month) {
        return None;
    }
    let shifted = i64::from(date.year) - i64::from(date.month <= 2);
    let era = if shifted >= 0 { shifted } else { shifted - 399 } / 400;
    let year_of_era = shifted - era * 400;
    let month = i64::from(date.month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// [`day_number_of`]의 반대. 범위를 벗어난 값은 가까운 끝으로 붙인다.
pub fn date_of_day_number(days: i64) -> CivilDate {
    let shifted = days.clamp(-MAX_DAY_NUMBER, MAX_DAY_NUMBER) + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = month_position + if month_position < 10 { 3 } else { -9 };
    CivilDate {
        year: (year + i64::from(month <= 2)).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        month: month.clamp(1, 12) as u32,
        day: day.clamp(1, 31) as u32,
    }
}

/// `YYYY-MM-DD`로 적는다.
pub fn format_date(date: CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// `pattern`이 정한 차례대로 `text`에서 연·월·일을 읽는다.
///
/// `pattern`에서 `Y`·`M`·`D`가 처음 나오는 차례만 본다(`YYYY-MM-DD`, `DD/MM/YYYY`처럼). 셋을 못 찾으면
/// [`DEFAULT_DATE_FORMAT`] 차례로 읽는다. `text`에서는 숫자 덩어리 셋만 뽑아 쓰므로 구분자가 무엇이든,
/// 뒤에 시각이 붙어 있든 상관없다. 숫자 덩어리가 셋보다 적거나 달력에 없는 날짜면 `None`이다.
pub fn parse_date(text: &str, pattern: &str) -> Option<CivilDate> {
    let mut groups = text.split(|c: char| !c.is_ascii_digit()).filter(|group| !group.is_empty());
    let values = [groups.next()?, groups.next()?, groups.next()?];
    let (mut year, mut month, mut day) = (0i32, 0u32, 0u32);
    for (field, raw) in field_order(pattern).iter().zip(values) {
        match field {
            DateField::Year => year = raw.parse().ok()?,
            DateField::Month => month = raw.parse().ok()?,
            DateField::Day => day = raw.parse().ok()?,
        }
    }
    let date = CivilDate { year, month, day };
    day_number_of(date).map(|_| date)
}

fn field_order(pattern: &str) -> [DateField; 3] {
    let mut order = Vec::new();
    for c in pattern.chars() {
        let field = match c {
            'Y' => DateField::Year,
            'M' => DateField::Month,
            'D' => DateField::Day,
            _ => continue,
        };
        if !order.contains(&field) {
            order.push(field);
        }
    }
    match order[..] {
        [first, second, third] => [first, second, third],
        _ => [DateField::Year, DateField::Month, DateField::Day],
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

// ── 기간 읽기 ───────────────────────────────────────────────────────────────

/// mermaid 식으로 붙여 쓴 기간(`5d`, `3w`, `24h`, `1.5w`)을 일수로 바꾼다.
///
/// 단위가 없거나 모르는 단위면 `None`이다. `m`은 분, `M`은 달로 mermaid 관례를 따라 대소문자를 가린다.
pub fn duration_token_in_days(token: &str) -> Option<f64> {
    let token = token.trim();
    let split = token.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(token.len());
    let (number, unit) = token.split_at(split);
    let amount: f64 = number.parse().ok()?;
    let factor = match unit {
        "M" => 30.0,
        _ => unit_in_days(unit)?,
    };
    let days = amount * factor;
    days.is_finite().then_some(days)
}

/// PlantUML 식으로 띄어 쓴 기간(`10 days`, `1 week and 4 days`)을 일수로 바꾼다.
pub fn duration_phrase_in_days(phrase: &str) -> Option<f64> {
    let lowered = phrase.trim().to_ascii_lowercase();
    let mut total = 0.0;
    let mut found = false;
    for part in lowered.split(" and ") {
        let mut words = part.split_whitespace();
        let Some(amount) = words.next().and_then(|word| word.parse::<f64>().ok()) else {
            continue;
        };
        let factor = words.next().and_then(unit_in_days).unwrap_or(1.0);
        total += amount * factor;
        found = true;
    }
    (found && total.is_finite()).then_some(total)
}

fn unit_in_days(unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().trim_end_matches('s') {
        "ms" | "millisecond" => Some(1.0 / 86_400_000.0),
        "sec" | "second" => Some(1.0 / 86_400.0),
        "s" => Some(1.0 / 86_400.0),
        "m" | "min" | "minute" => Some(1.0 / 1_440.0),
        "h" | "hr" | "hour" => Some(1.0 / 24.0),
        "d" | "day" => Some(1.0),
        "w" | "week" => Some(7.0),
        "mo" | "month" => Some(30.0),
        "y" | "year" => Some(365.0),
        _ => None,
    }
}

// ── 그리기 ──────────────────────────────────────────────────────────────────

/// 간트 차트를 줄 목록으로 그린다.
///
/// 줄 하나가 작업 하나이고, 섹션 이름은 그 위에 머리줄로 앉는다. 막대 자리는 전체 기간을
/// [`Axis`]에 맡겨 정하므로 기간이 아무리 길어도 폭 안으로 줄어든다. 맨 아래에는 일수 눈금과 날짜
/// 범위를 적는다.
///
/// 그릴 작업이 없거나 `width`가 [`MIN_GANTT_WIDTH`]보다 좁으면 `None`이다(부르는 쪽에서 코드블록으로
/// 물러난다). 작업이 [`MAX_TASK_ROWS`]보다 많으면 마지막 한 줄을 `… +N more`로 접는다. 어떤 입력에도
/// 패닉하지 않는다.
pub fn render(chart: &GanttChart, theme: &Theme, width: usize) -> Option<Vec<Line>> {
    if width < MIN_GANTT_WIDTH {
        return None;
    }
    let tasks: Vec<&GanttTask> = chart.sections.iter().flat_map(|section| &section.tasks).collect();
    if tasks.is_empty() {
        return None;
    }

    let indent = usize::from(chart.sections.iter().any(|section| !section.name.trim().is_empty())) * TASK_INDENT;
    let longest = tasks.iter().map(|task| indent + width_of(&task.name)).max().unwrap_or(MIN_NAME_WIDTH);
    let name_limit = width.saturating_sub(1 + MIN_BAR_FIELD).max(MIN_NAME_WIDTH);
    let name_width = longest.clamp(MIN_NAME_WIDTH, name_limit);
    let bar_field = width.saturating_sub(name_width + 1);
    if bar_field < MIN_BAR_FIELD {
        return None;
    }

    let mut lowest = 0.0f64;
    let mut highest = 0.0f64;
    for task in &tasks {
        lowest = lowest.min(task.start_day);
        highest = highest.max(task.start_day + task.duration_days);
    }
    let axis = Axis::new(lowest, highest, bar_field)?;

    let hidden = tasks.len().saturating_sub(MAX_TASK_ROWS);
    let budget = if hidden == 0 { tasks.len() } else { MAX_TASK_ROWS - 1 };

    let mut lines = Vec::new();
    lines.extend(title_line(&chart.title, width, theme));
    let mut drawn = 0;
    for section in &chart.sections {
        if drawn >= budget {
            break;
        }
        if !section.name.trim().is_empty() {
            lines.push(Line::single(truncate(&section.name, width), theme.diagram_group));
        }
        for task in &section.tasks {
            if drawn >= budget {
                break;
            }
            lines.push(task_line(task, &axis, indent, name_width, theme));
            drawn += 1;
        }
    }
    if drawn < tasks.len() {
        lines.push(Line::single(truncate(&format!("… +{} more", tasks.len() - drawn), width), theme.diagram_caption));
    }

    let caption = axis_caption(chart, &axis);
    let indent_text = " ".repeat(name_width + 1);
    for row in render_horizontal_axis(&axis, Some(&caption), theme) {
        let mut line = Line::empty();
        line.push_str(&indent_text, Style::PLAIN);
        line.append(&row);
        lines.push(line);
    }
    lines.extend(legend_line(&tasks, width, theme));
    Some(lines)
}

fn title_line(title: &str, width: usize, theme: &Theme) -> Option<Line> {
    let title = truncate(title.trim(), width);
    if title.trim().is_empty() {
        return None;
    }
    let mut line = Line::empty();
    line.push_str(&" ".repeat(width.saturating_sub(width_of(&title)) / 2), Style::PLAIN);
    line.push(Span::new(title, theme.diagram_label));
    Some(line)
}

fn task_line(task: &GanttTask, axis: &Axis, indent: usize, name_width: usize, theme: &Theme) -> Line {
    let name = truncate(&task.name, name_width.saturating_sub(indent).max(1));
    let mut line = Line::empty();
    line.push_str(&" ".repeat(indent), Style::PLAIN);
    line.push(Span::new(name.clone(), theme.diagram_text));
    let used = indent + width_of(&name);
    line.push_str(&" ".repeat(name_width.saturating_sub(used) + 1), Style::PLAIN);

    let start = axis.position_of(task.start_day);
    let end = axis.position_of(task.start_day + task.duration_days);
    let cells = end.saturating_sub(start).max(1).min(axis.length() - start);
    let (fill, style) = if task.done { (DONE_FILL, theme.diagram_box) } else { (PENDING_FILL, theme.diagram_accent) };
    line.push_str(&" ".repeat(start), Style::PLAIN);
    line.push(Span::new(fill.to_string().repeat(cells), style));
    line
}

/// 축 아래에 적을 날짜 범위. 소스에 날짜가 없으면 눈금 단위만 알린다.
fn axis_caption(chart: &GanttChart, axis: &Axis) -> String {
    match chart.baseline_day_number {
        Some(base) => {
            let from = format_date(date_of_day_number(base.saturating_add(axis.min().round() as i64)));
            let to = format_date(date_of_day_number(base.saturating_add(axis.max().round() as i64)));
            format!("{from} … {to}")
        }
        None => "일".to_string(),
    }
}

/// 끝난 작업이 하나라도 있을 때만 막대 글자의 뜻을 알린다.
fn legend_line(tasks: &[&GanttTask], width: usize, theme: &Theme) -> Option<Line> {
    if !tasks.iter().any(|task| task.done) {
        return None;
    }
    let text = format!("{DONE_FILL} 완료  {PENDING_FILL} 예정");
    (width_of(&text) <= width).then(|| Line::single(text, theme.diagram_caption))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, start: f64, duration: f64) -> PlannedTask {
        PlannedTask { name: name.into(), section: 0, duration_days: duration, done: false, start: StartRule::Day(start) }
    }

    fn plan(tasks: Vec<PlannedTask>) -> GanttPlan {
        GanttPlan { section_names: vec![String::new()], tasks, ..GanttPlan::default() }
    }

    fn rows(lines: &[Line]) -> Vec<String> {
        lines.iter().map(Line::plain).collect()
    }

    #[test]
    fn day_numbers_count_real_calendar_days() {
        let epoch = CivilDate { year: 1970, month: 1, day: 1 };
        assert_eq!(day_number_of(epoch), Some(0));
        let leap = CivilDate { year: 2024, month: 3, day: 1 };
        let before = CivilDate { year: 2024, month: 2, day: 28 };
        assert_eq!(day_number_of(leap).unwrap() - day_number_of(before).unwrap(), 2);
        let start = CivilDate { year: 2026, month: 9, day: 16 };
        let end = CivilDate { year: 2026, month: 10, day: 16 };
        assert_eq!(day_number_of(end).unwrap() - day_number_of(start).unwrap(), 30);
        for date in [start, end, leap, CivilDate { year: 1, month: 1, day: 1 }, CivilDate { year: 9999, month: 12, day: 31 }] {
            assert_eq!(date_of_day_number(day_number_of(date).unwrap()), date);
        }
        // 달력에 없는 날짜는 거른다.
        assert_eq!(day_number_of(CivilDate { year: 2023, month: 2, day: 29 }), None);
        assert_eq!(day_number_of(CivilDate { year: 2024, month: 13, day: 1 }), None);
        assert_eq!(day_number_of(CivilDate { year: 0, month: 1, day: 1 }), None);
    }

    // 2.3 dateFormat이 정한 차례대로 날짜를 읽는다.
    #[test]
    fn dates_follow_the_declared_format() {
        assert_eq!(parse_date("2024-01-05", "YYYY-MM-DD"), Some(CivilDate { year: 2024, month: 1, day: 5 }));
        assert_eq!(parse_date("05/01/2024", "DD/MM/YYYY"), Some(CivilDate { year: 2024, month: 1, day: 5 }));
        assert_eq!(parse_date("01.05.2024", "MM.DD.YYYY"), Some(CivilDate { year: 2024, month: 1, day: 5 }));
        assert_eq!(parse_date("2024-01-05 10:30", "YYYY-MM-DD HH:mm"), Some(CivilDate { year: 2024, month: 1, day: 5 }));
        // 형식을 못 알아보면 기본 차례로 읽는다.
        assert_eq!(parse_date("2024-01-05", "unknown"), Some(CivilDate { year: 2024, month: 1, day: 5 }));
        assert_eq!(parse_date("5d", DEFAULT_DATE_FORMAT), None);
        assert_eq!(parse_date("after t1", DEFAULT_DATE_FORMAT), None);
        assert_eq!(parse_date("2024-99-99", DEFAULT_DATE_FORMAT), None);
    }

    #[test]
    fn durations_convert_to_days() {
        assert_eq!(duration_token_in_days("5d"), Some(5.0));
        assert_eq!(duration_token_in_days("2w"), Some(14.0));
        assert_eq!(duration_token_in_days("24h"), Some(1.0));
        assert_eq!(duration_token_in_days("1M"), Some(30.0));
        assert!(duration_token_in_days("30m").is_some_and(|days| (days - 30.0 / 1440.0).abs() < 1e-9));
        assert_eq!(duration_token_in_days("5"), None);
        assert_eq!(duration_token_in_days("t1"), None);
        assert_eq!(duration_phrase_in_days("10 days"), Some(10.0));
        assert_eq!(duration_phrase_in_days("1 week"), Some(7.0));
        assert_eq!(duration_phrase_in_days("1 week and 4 days"), Some(11.0));
        assert_eq!(duration_phrase_in_days("소요"), None);
    }

    // 3.1 앞선 작업이 끝난 시점을 이어받는다. 3.3 없는 작업을 가리키면 기준일부터다.
    #[test]
    fn schedule_resolves_references_in_both_directions() {
        let mut first = task("a", 0.0, 5.0);
        first.start = StartRule::Baseline;
        let mut second = task("b", 0.0, 3.0);
        second.start = StartRule::AfterTasks(vec![0]);
        let mut third = task("c", 0.0, 2.0);
        // 아직 안 나온 작업(뒤에 있는 3번)을 가리켜도 풀린다.
        third.start = StartRule::AfterTasks(vec![3]);
        let mut fourth = task("d", 0.0, 4.0);
        fourth.start = StartRule::AfterTasks(vec![1]);
        let mut missing = task("e", 0.0, 1.0);
        missing.start = StartRule::AfterTasks(Vec::new());
        let chart = plan(vec![first, second, third, fourth, missing]).schedule();
        let starts: Vec<f64> = chart.sections[0].tasks.iter().map(|task| task.start_day).collect();
        assert_eq!(starts, vec![0.0, 5.0, 12.0, 8.0, 0.0]);
    }

    // 5.3 참조가 고리를 이뤄도 멎지 않는다.
    #[test]
    fn cyclic_references_still_terminate() {
        let mut first = task("a", 0.0, 2.0);
        first.start = StartRule::AfterTasks(vec![1]);
        let mut second = task("b", 0.0, 2.0);
        second.start = StartRule::AfterTasks(vec![0]);
        let chart = plan(vec![first, second]).schedule();
        assert_eq!(chart.sections[0].tasks.len(), 2);
    }

    // 2.1 섹션 머리줄 아래로 작업이 들여쓰여 묶인다. 2.2 막대 길이가 기간에 비례한다.
    #[test]
    fn sections_group_tasks_and_bars_match_durations() {
        let chart = GanttPlan {
            title: "Plan".into(),
            section_names: vec!["설계".into(), "구현".into()],
            tasks: vec![task("a", 0.0, 10.0), PlannedTask { section: 1, ..task("b", 10.0, 5.0) }],
            baseline_day_number: day_number_of(CivilDate { year: 2024, month: 1, day: 1 }),
        }
        .schedule();
        let lines = rows(&render(&chart, &Theme::none(), 40).unwrap());
        assert!(lines[0].contains("Plan"));
        assert_eq!(lines[1], "설계");
        assert!(lines[2].starts_with("  a "));
        assert_eq!(lines[3], "구현");
        assert!(lines[4].starts_with("  b "));
        let first = lines[2].matches(PENDING_FILL).count();
        let second = lines[4].matches(PENDING_FILL).count();
        assert!(first > second, "10일 막대가 5일 막대보다 길어야 한다: {first} vs {second}");
        // b는 a가 끝난 자리에서 시작한다.
        assert!(lines[4].find(PENDING_FILL) > lines[2].find(PENDING_FILL));
        assert!(lines.iter().any(|row| row.contains("2024-01-01")));
        assert!(lines.iter().all(|row| width_of(row) <= 40));
    }

    #[test]
    fn done_tasks_use_a_distinct_fill_and_legend() {
        let chart = plan(vec![PlannedTask { done: true, ..task("a", 0.0, 3.0) }, task("b", 3.0, 3.0)]).schedule();
        let lines = rows(&render(&chart, &Theme::none(), 40).unwrap());
        assert!(lines[0].contains(DONE_FILL));
        assert!(lines[1].contains(PENDING_FILL));
        assert!(lines.last().unwrap().contains("완료"));
    }

    // 5.1 기간이 폭보다 훨씬 길어도 축소 비례로 들어간다.
    #[test]
    fn long_timelines_shrink_to_fit() {
        let chart = plan(vec![task("a", 0.0, 4000.0), task("b", 4000.0, 4000.0)]).schedule();
        let lines = rows(&render(&chart, &Theme::none(), 40).unwrap());
        assert!(lines.iter().all(|row| width_of(row) <= 40));
        assert!(lines[0].contains(PENDING_FILL) && lines[1].contains(PENDING_FILL));
        assert!(lines[1].find(PENDING_FILL) > lines[0].find(PENDING_FILL));
    }

    // 5.2 작업이 너무 많으면 남은 개수를 알리는 줄로 접는다.
    #[test]
    fn overflowing_tasks_fold_into_more_row() {
        let many: Vec<PlannedTask> = (0..60).map(|index| task(&format!("t{index}"), index as f64, 1.0)).collect();
        let chart = plan(many).schedule();
        let lines = rows(&render(&chart, &Theme::none(), 60).unwrap());
        assert!(lines.iter().any(|row| row.starts_with("… +21 more")));
        assert_eq!(lines.iter().filter(|row| row.contains(PENDING_FILL)).count(), MAX_TASK_ROWS - 1);
    }

    // 5.3 어떤 차트·폭에도 패닉 없이 결과나 `None`을 돌려준다.
    #[test]
    fn never_panics_across_edge_cases() {
        let long_name = "아주".repeat(80);
        let cases = vec![
            plan(Vec::new()),
            plan(vec![task("solo", 0.0, 0.0)]),
            plan(vec![task("neg", -50.0, 10.0), task("pos", 0.0, 10.0)]),
            plan(vec![task(&long_name, 0.0, f64::NAN), task("🙂🙂", f64::INFINITY, f64::MAX)]),
            plan(vec![PlannedTask { section: 99, ..task("없는 섹션", 0.0, 1.0) }]),
            GanttPlan { section_names: Vec::new(), tasks: vec![task("섹션 없음", 0.0, 1.0)], ..GanttPlan::default() },
            GanttPlan {
                title: long_name.clone(),
                baseline_day_number: Some(i64::MAX),
                section_names: vec!["s".into()],
                tasks: vec![task("a", f64::MIN, f64::MAX)],
            },
        ];
        for case in cases {
            let chart = case.schedule();
            for theme in [Theme::none(), Theme::dark()] {
                for width in [0usize, 1, 8, MIN_GANTT_WIDTH, 20, 40, 80, 200] {
                    if let Some(lines) = render(&chart, &theme, width) {
                        assert!(lines.iter().all(|line| line.width() <= width), "폭 {width}: {:?}", rows(&lines));
                    }
                }
            }
        }
    }
}
