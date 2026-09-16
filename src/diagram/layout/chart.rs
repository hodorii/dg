//! 수치 차트 공통 렌더링.
//!
//! `pie`·`xychart-beta`·`gantt`·`quadrantChart`처럼 값을 그림으로 옮기는 차트들이 축·막대·범례·수치
//! 표기를 **같은 규칙**으로 그리도록 모아 둔 도구 상자다. 이 모듈은 mermaid 문법을 모른다. 각 차트
//! 스펙이 소스를 파싱해 [`ChartItem`] 목록을 만든 뒤 여기 있는 함수를 불러 쓴다.
//!
//! 구성 요소는 세 층이다.
//!
//! 1. 수치 표기: [`format_value`]·[`format_percent`]·[`format_display`]. 모든 차트가 같은 반올림
//!    규칙을 쓰게 하는 단일 출처다.
//! 2. 값 → 칸 변환: [`Axis`]. 눈금값·눈금 위치([`Axis::ticks`])와 막대가 차지할 칸
//!    ([`Axis::bar_span`])을 모두 이 하나에서 뽑는다. 가로·세로 어느 쪽 축이든 계산은 같고,
//!    그리기 도우미 [`render_horizontal_axis`]만 가로 전용이다.
//! 3. 줄 만들기: [`render_bar`]·[`render_legend`]·[`render_horizontal_axis`]. 모두
//!    [`Line`] 목록을 돌려주므로 호출한 쪽에서 이어 붙이기만 하면 된다.
//!
//! 가로 막대 차트 한 벌이 필요하면 [`BarChart`]가 위 셋을 조립해 준다. 그릴 수 없으면(폭 부족 등)
//! `None`을 돌려주므로 `render_body`의 관례대로 그대로 흘려보내면 코드블록으로 대체된다.
//!
//! ```
//! use dg::diagram::layout::chart::{BarChart, BarScale, ChartItem, ValueDisplay};
//! use dg::style::Theme;
//!
//! let items = [ChartItem::new("Dogs", 50.0), ChartItem::new("Cats", 30.0), ChartItem::new("Birds", 20.0)];
//! let chart = BarChart { scale: BarScale::Total, value_display: ValueDisplay::ValueAndPercent, ..BarChart::new(&items, 40) };
//! let lines = chart.render(&Theme::none()).unwrap();
//! assert!(lines[0].text().starts_with("Dogs"));
//! assert!(lines[0].text().ends_with("50 (50.0%)"));
//! ```

use crate::line::{Line, Span};
use crate::style::{Style, Theme};
use crate::text::{truncate, width_of};

/// 절댓값이 이 값 이상이면 소수 자리를 따지지 않고 정수로만 적는다(f64가 그 아래를 못 담는다).
const INTEGER_ONLY_MAGNITUDE: f64 = 1e15;
/// 막대밭에 최소한 남겨 두는 칸 수. 이보다 좁으면 막대가 뜻을 잃는다.
const MIN_BAR_WIDTH: usize = 4;
/// 눈금 사이에 최소한 두는 칸 수. 눈금값이 서로 붙지 않을 만큼이다.
const COLUMNS_PER_TICK: usize = 7;
/// 눈금 생성이 끝나지 않는 일이 없도록 두는 상한.
const MAX_TICKS: usize = 64;

/// 양수 막대 칸.
const POSITIVE_FILL: char = '█';
/// 음수 막대 칸. 방향뿐 아니라 글자로도 양수와 구분된다(색 없는 테마 대비).
const NEGATIVE_FILL: char = '▒';
/// 0 기준선 표시. 축이 음수와 양수에 걸칠 때만 나온다.
const BASELINE_MARK: char = '│';
/// 범례 항목 머리표.
const LEGEND_MARKER: char = '■';

/// [`BarChart`]가 그릴 수 있는 가장 좁은 폭. 이보다 좁으면 [`BarChart::render`]가 `None`이다.
pub const MIN_CHART_WIDTH: usize = 8;
/// [`render_legend`]가 그릴 수 있는 가장 좁은 폭(머리표 `■ ` + 값 두 칸).
pub const MIN_LEGEND_WIDTH: usize = 4;
/// [`BarChart::new`]가 쓰는 기본 최대 줄 수.
pub const DEFAULT_MAX_ROWS: usize = 24;

/// 차트 항목 하나. 이름과 값의 쌍이다.
///
/// 값은 `f64`이며 NaN·무한대가 들어와도 된다. 그런 값은 수치 표기에서 `-`가 되고, 막대 길이·축 범위
/// 계산에서는 없는 값으로 친다.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartItem {
    /// 항목 이름. 한글·이모지처럼 폭이 2인 글자가 들어와도 된다.
    pub name: String,
    /// 항목 값. 음수도 그대로 둔다([`Axis::bar_span`]이 반대 방향으로 그린다).
    pub value: f64,
}

impl ChartItem {
    /// 항목을 만든다.
    pub fn new(name: impl Into<String>, value: f64) -> ChartItem {
        ChartItem { name: name.into(), value }
    }
}

/// 항목 값 절댓값의 합. 백분율의 분모이자 [`BarScale::Total`]의 기준값이다.
///
/// 유한하지 않은 값은 건너뛴다. 항목이 없으면 `0.0`이다. 절댓값을 쓰는 이유는 음수가 섞여도 분모가
/// 0에 가까워지거나 음수가 되지 않게 하기 위해서다.
pub fn total_of(items: &[ChartItem]) -> f64 {
    items.iter().map(|item| item.value).filter(|v| v.is_finite()).map(f64::abs).sum()
}

/// 수치 하나를 사람이 읽을 문자열로 바꾼다.
///
/// - 정수 값은 소수점 없이 적는다(`50.0` → `"50"`).
/// - 소수는 둘째 자리에서 반올림해 두 자리로 적는다(`3.14159` → `"3.14"`, `0.5` → `"0.50"`).
///   반올림 결과가 정수면 소수점을 떼므로 `2.999` 는 `"3"`이 된다.
/// - 절댓값이 1e15 이상이면 지수 표기 없이 정수로만 적는다(자릿수가 길어질 수 있다).
/// - NaN·무한대는 `"-"`.
pub fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }
    if value.abs() >= INTEGER_ONLY_MAGNITUDE {
        return format!("{value:.0}");
    }
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == 0.0 {
        return "0".to_string();
    }
    if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded:.2}") }
}

/// `value`가 `total`에서 차지하는 몫을 백분율로 적는다. 소수 한 자리 고정이다(`"33.3%"`).
///
/// 범례에서 세로로 자릿수가 맞도록 [`format_value`]와 달리 자리를 떼지 않는다. `total`이 0이거나
/// 유한하지 않거나 값이 NaN이면 `"0.0%"`다(나눗셈을 하지 않는다). 반올림해서 0에 붙는 값은 부호
/// 없이 `"0.0%"`로 적는다.
pub fn format_percent(value: f64, total: f64) -> String {
    if !value.is_finite() || !total.is_finite() || total == 0.0 {
        return "0.0%".to_string();
    }
    let percent = value / total * 100.0;
    if !percent.is_finite() || percent.abs() < 0.05 {
        return "0.0%".to_string();
    }
    format!("{percent:.1}%")
}

/// 자리에 다 담지 못한 항목이 몇 개인지 알리는 글. 차트마다 말이 달라지지 않게 여기 하나만 둔다.
pub fn overflow_notice(hidden: usize) -> String {
    format!("… +{hidden} more")
}

/// 값 옆에 무엇을 적을지.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueDisplay {
    /// 값만(`12.5`).
    #[default]
    Value,
    /// 몫만(`25.0%`).
    Percent,
    /// 값과 몫(`12.5 (25.0%)`).
    ValueAndPercent,
}

/// [`ValueDisplay`]에 맞춰 값을 적는다. `total`은 보통 [`total_of`]로 구한다.
pub fn format_display(value: f64, total: f64, display: ValueDisplay) -> String {
    match display {
        ValueDisplay::Value => format_value(value),
        ValueDisplay::Percent => format_percent(value, total),
        ValueDisplay::ValueAndPercent => format!("{} ({})", format_value(value), format_percent(value, total)),
    }
}

/// 축 눈금 하나.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisTick {
    /// 눈금이 가리키는 값.
    pub value: f64,
    /// 축 위 칸 번호(`0..length`). 가로축이면 왼쪽부터, 세로축이면 축을 그리는 쪽이 정한다.
    pub position: usize,
}

/// 값 범위를 축의 칸으로 옮기는 자.
///
/// 만들 때 눈금값과 눈금 위치를 한 번 계산해 들고 있는다. 가로·세로 구분이 없다. 잘못된 입력에는
/// 생성자가 `None`을 돌려주므로, 만들어진 `Axis`는 항상 `length >= 2`, `min < max`, 유한한 범위다.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    min: f64,
    max: f64,
    length: usize,
    ticks: Vec<AxisTick>,
}

impl Axis {
    /// 범위와 칸 수로 축을 만든다.
    ///
    /// `min > max`면 뒤집어 받는다. 범위 폭이 0이면(예: 값이 모두 같을 때) `min..min+1`로 넓힌다.
    /// `length`가 2 미만이거나, 값이 유한하지 않거나, 폭이 무한대가 되도록 범위가 넓거나, 값이 너무
    /// 커서 1을 더해도 폭이 0에 머무르면 `None`이다.
    ///
    /// 눈금은 1·2·5 × 10ⁿ 중 하나를 간격으로 고르고 그 배수 위에 놓는다. 눈금 개수는 칸 수에서
    /// 정한다(눈금 사이를 7칸 이상 벌린다). 그래서 범위에 0이 들어 있으면 0은 항상 눈금이다.
    pub fn new(min: f64, max: f64, length: usize) -> Option<Axis> {
        if length < 2 || !min.is_finite() || !max.is_finite() {
            return None;
        }
        let (lo, mut hi) = if min <= max { (min, max) } else { (max, min) };
        if hi - lo <= 0.0 {
            hi = lo + 1.0;
        }
        let span = hi - lo;
        if !span.is_finite() || span <= 0.0 {
            return None;
        }
        let mut axis = Axis { min: lo, max: hi, length, ticks: Vec::new() };
        axis.ticks = axis.compute_ticks();
        Some(axis)
    }

    /// 값들을 보고 축 범위를 자동으로 정한다(범위를 안 준 차트용).
    ///
    /// 0을 반드시 포함하도록 늘린 뒤, 눈금 간격의 배수로 바깥쪽으로 올림·내림해 여유를 준다. 그래서
    /// 양 끝은 항상 눈금이고, 가장 큰 값은 축 끝에 살짝 못 미친다. 막대 길이가 값에 정확히 비례하길
    /// 원하면 [`bar_axis`]를 대신 쓴다.
    ///
    /// 유한한 값이 하나도 없으면 `0..1`로 잡는다. `length`가 2 미만이면 `None`이다.
    pub fn from_values(values: &[f64], length: usize) -> Option<Axis> {
        let mut lo = 0.0f64;
        let mut hi = 0.0f64;
        let mut seen = false;
        for value in values.iter().copied().filter(|v| v.is_finite()) {
            lo = if seen { lo.min(value) } else { value.min(0.0) };
            hi = if seen { hi.max(value) } else { value.max(0.0) };
            seen = true;
        }
        if !seen || hi - lo <= 0.0 {
            hi = lo + 1.0;
        }
        let step = nice_step(hi - lo, length);
        if step.is_finite() && step > 0.0 {
            let padded_lo = (lo / step).floor() * step;
            let padded_hi = (hi / step).ceil() * step;
            if padded_lo.is_finite() && padded_hi.is_finite() && padded_hi > padded_lo {
                lo = padded_lo;
                hi = padded_hi;
            }
        }
        Axis::new(lo, hi, length)
    }

    /// 축 아래끝 값.
    pub fn min(&self) -> f64 {
        self.min
    }

    /// 축 위끝 값.
    pub fn max(&self) -> f64 {
        self.max
    }

    /// 축이 차지하는 칸 수. 항상 2 이상이다.
    pub fn length(&self) -> usize {
        self.length
    }

    /// 눈금 목록. 값이 커지는 순서이고, 두 눈금이 같은 칸에 놓이는 일은 없다.
    pub fn ticks(&self) -> &[AxisTick] {
        &self.ticks
    }

    /// 값이 놓이는 칸 번호(`0..length`). 눈금·점 표시용이다.
    ///
    /// 범위를 벗어나면 가까운 끝으로 붙이고, NaN·무한대는 0으로 본다. 칸의 *가운데*를 값으로 보므로
    /// `min`은 0번, `max`는 `length-1`번 칸이다.
    pub fn position_of(&self, value: f64) -> usize {
        let last = self.length.saturating_sub(1);
        if !value.is_finite() {
            return 0;
        }
        let ratio = (value - self.min) / (self.max - self.min);
        if !ratio.is_finite() || ratio <= 0.0 {
            return 0;
        }
        let position = (ratio * last as f64).round();
        if position >= last as f64 { last } else { position as usize }
    }

    /// 값 0이 놓이는 칸 *경계*(`0..=length`). 막대가 뻗어 나가는 자리다.
    ///
    /// 축이 음수 쪽에 걸치지 않으면 0(왼쪽 끝), 양수 쪽에 걸치지 않으면 `length`(오른쪽 끝)다.
    pub fn baseline(&self) -> usize {
        if self.min >= 0.0 {
            return 0;
        }
        if self.max <= 0.0 {
            return self.length;
        }
        let ratio = -self.min / (self.max - self.min);
        let boundary = (ratio * self.length as f64).round();
        if !boundary.is_finite() || boundary <= 0.0 {
            0
        } else if boundary >= self.length as f64 {
            self.length
        } else {
            boundary as usize
        }
    }

    /// 0 기준선에서 `value`까지 막대가 차지하는 `(시작 칸, 칸 수)`.
    ///
    /// - 값이 0이거나 유한하지 않으면 칸 수는 0이다(막대를 그리지 않는다).
    /// - 양수는 기준선에서 오른쪽으로, 음수는 기준선에서 왼쪽으로 뻗는다.
    /// - 0이 아닌 값은 아무리 작아도 한 칸은 차지해 0과 구분된다.
    /// - 칸 수는 `|value| / (max - min) × length`를 반올림한 값이며, 축 밖으로 넘치면 잘린다.
    pub fn bar_span(&self, value: f64) -> (usize, usize) {
        let baseline = self.baseline();
        if !value.is_finite() || value == 0.0 {
            return (baseline, 0);
        }
        let raw = value.abs() / (self.max - self.min) * self.length as f64;
        let cells = if raw.is_finite() { (raw.round() as usize).max(1) } else { 1 };
        if value > 0.0 {
            (baseline, cells.min(self.length - baseline))
        } else {
            let cells = cells.min(baseline);
            (baseline - cells, cells)
        }
    }

    fn compute_ticks(&self) -> Vec<AxisTick> {
        let step = nice_step(self.max - self.min, self.length);
        let mut ticks = Vec::new();
        let start = (self.min / step).ceil();
        if step.is_finite() && step > 0.0 && start.is_finite() {
            // 부동소수 오차로 마지막 눈금이 떨어져 나가지 않도록 아주 작은 여유를 둔다.
            let limit = self.max + step * 1e-9;
            let mut index = start;
            // 값이 커서 `index + 1`이 제자리걸음을 하면 같은 눈금이 되풀이되므로 횟수로도 끊는다.
            for _ in 0..MAX_TICKS {
                let value = index * step;
                if !value.is_finite() || value > limit {
                    break;
                }
                let position = self.position_of(value);
                if ticks.last().is_none_or(|last: &AxisTick| last.position < position) {
                    ticks.push(AxisTick { value, position });
                }
                index += 1.0;
            }
        }
        if ticks.len() < 2 {
            ticks = vec![
                AxisTick { value: self.min, position: 0 },
                AxisTick { value: self.max, position: self.length - 1 },
            ];
        }
        ticks
    }
}

/// 막대 길이를 무엇에 견줄지.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BarScale {
    /// 가장 큰 값이 막대밭을 가득 채운다. 막대·xy 차트의 보통 관례다.
    #[default]
    Max,
    /// 값 절댓값의 합이 막대밭을 가득 채운다. 곧 막대 길이가 곧 몫이다(파이 차트용).
    Total,
}

/// 항목 목록과 기준으로 막대용 축을 만든다.
///
/// [`Axis::from_values`]와 달리 여유를 두지 않는다. 막대 길이가 값에 정확히 비례해야 하기 때문이다.
///
/// - [`BarScale::Max`]: 범위는 `min(0, 최솟값) .. max(0, 최댓값)`. 음수가 없으면 최댓값 막대가
///   막대밭을 가득 채운다. 음수가 있으면 기준선이 안쪽으로 들어오므로 각 막대는 제 몫만큼만 찬다.
/// - [`BarScale::Total`]: 범위 폭이 절댓값의 합과 같아지도록 `-(음수 합) .. (양수 합)`으로 잡는다.
///   그래서 막대 길이는 언제나 `|값| / 합 × 칸 수`다.
///
/// 항목이 없거나 값이 모두 0이면 범위가 `0..1`이 되어 모든 막대가 0칸이 된다. `length`가 2 미만이면
/// `None`이다.
pub fn bar_axis(items: &[ChartItem], scale: BarScale, length: usize) -> Option<Axis> {
    let values = items.iter().map(|item| item.value).filter(|v| v.is_finite());
    let (lo, hi) = match scale {
        BarScale::Max => values.fold((0.0f64, 0.0f64), |(lo, hi), value| (lo.min(value), hi.max(value))),
        BarScale::Total => values.fold((0.0f64, 0.0f64), |(lo, hi), value| if value < 0.0 { (lo + value, hi) } else { (lo, hi + value) }),
    };
    Axis::new(lo, hi, length)
}

/// 막대 한 줄을 그린다. 돌려주는 줄은 언제나 `axis.length()`칸이다(뒤를 공백으로 채운다).
///
/// 칸 수가 고정이라 여러 줄을 세로로 쌓으면 축과 자리가 맞는다. 양수는 기준선 오른쪽으로 `█`,
/// 음수는 왼쪽으로 `▒`로 뻗고, 축이 음수와 양수에 걸칠 때는 남은 기준선 자리에 `│`를 둔다.
/// 값이 0이면 막대 없이 공백만 있는 줄이다.
pub fn render_bar(axis: &Axis, value: f64, theme: &Theme) -> Line {
    let (start, cells) = axis.bar_span(value);
    let baseline = axis.baseline();
    let has_baseline_mark = axis.min() < 0.0 && axis.max() > 0.0 && baseline < axis.length();
    let (fill, fill_style) =
        if value < 0.0 { (NEGATIVE_FILL, theme.diagram_box) } else { (POSITIVE_FILL, theme.diagram_accent) };
    let mut line = Line::empty();
    let mut buffer = [0u8; 4];
    for column in 0..axis.length() {
        let (ch, style) = if column >= start && column < start + cells {
            (fill, fill_style)
        } else if has_baseline_mark && column == baseline {
            (BASELINE_MARK, theme.rule)
        } else {
            (' ', Style::PLAIN)
        };
        line.push_str(ch.encode_utf8(&mut buffer), style);
    }
    line
}

/// 가로축을 눈금선·눈금값·축 제목 세 줄(제목이 없으면 두 줄)로 그린다.
///
/// 줄마다 폭은 `axis.length()`를 넘지 않으므로 막대밭 왼쪽 여백만큼 밀어 놓으면 막대와 자리가 맞는다.
/// 눈금값은 눈금 칸 가운데에 맞추되 앞 눈금값과 붙게 되면 그 눈금값은 건너뛴다(글자가 겹쳐 뭉개지는
/// 쪽보다 낫다). 축 제목은 따로 한 줄을 차지하므로 눈금값과 겹치지 않으며, 축보다 길 때만 잘린다.
pub fn render_horizontal_axis(axis: &Axis, title: Option<&str>, theme: &Theme) -> Vec<Line> {
    let length = axis.length();
    let mut glyphs = vec!['─'; length];
    for tick in axis.ticks() {
        if let Some(cell) = glyphs.get_mut(tick.position) {
            *cell = '┼';
        }
    }
    glyphs[0] = '├';
    glyphs[length - 1] = '┤';
    let mut rows = vec![Line::single(glyphs.into_iter().collect::<String>(), theme.rule)];

    let mut labels = String::new();
    let mut used = 0;
    for tick in axis.ticks() {
        let label = format_value(tick.value);
        let label_width = width_of(&label);
        if label_width > length {
            continue;
        }
        let start = tick.position.saturating_sub(label_width / 2).min(length - label_width);
        if used > 0 && start <= used {
            continue;
        }
        labels.push_str(&" ".repeat(start - used));
        labels.push_str(&label);
        used = start + label_width;
    }
    rows.push(Line::single(labels, theme.diagram_caption));

    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        let title = truncate(title, length);
        let indent = (length - width_of(&title)) / 2;
        let mut line = Line::empty();
        line.push_str(&" ".repeat(indent), Style::PLAIN);
        line.push(Span::new(title, theme.diagram_label));
        rows.push(line);
    }
    rows
}

/// 항목을 `■ 이름   값` 꼴의 범례 줄로 만든다. 줄 하나가 항목 하나다.
///
/// 값은 오른쪽에 맞춰 세로로 자릿수가 맞는다. 이름이든 값이든 남는 자리보다 길면 `…`로 줄여 넣으므로
/// 줄 폭은 언제나 `width` 이하다. 몫(`%`)은 항목 값 절댓값의 합([`total_of`])을 분모로 삼는다.
///
/// 항목이 없거나 `width`가 [`MIN_LEGEND_WIDTH`]보다 좁으면 빈 목록이다. 자리가 모자라면 이름부터
/// 밀려나 값만 남는다.
pub fn render_legend(items: &[ChartItem], display: ValueDisplay, width: usize, theme: &Theme) -> Vec<Line> {
    if items.is_empty() || width < MIN_LEGEND_WIDTH {
        return Vec::new();
    }
    let total = total_of(items);
    // 머리표 "■ "를 뺀 자리가 값이 쓸 수 있는 최대 폭이다.
    let values: Vec<String> = items.iter().map(|item| truncate(&format_display(item.value, total, display), width - 2)).collect();
    let value_width = values.iter().map(|value| width_of(value)).max().unwrap_or(0);
    // "■ " + 이름 + 공백 + 값
    let name_width = width.saturating_sub(value_width + 3);
    items
        .iter()
        .zip(&values)
        .map(|(item, value)| {
            let mut line = Line::empty();
            line.push(Span::new(LEGEND_MARKER.to_string(), theme.diagram_accent));
            line.push_str(" ", Style::PLAIN);
            if name_width > 0 {
                let name = truncate(&item.name, name_width);
                let pad = name_width - width_of(&name);
                line.push(Span::new(name, theme.diagram_text));
                line.push_str(&" ".repeat(pad + 1), Style::PLAIN);
            }
            line.push_str(&" ".repeat(value_width - width_of(value)), Style::PLAIN);
            line.push(Span::new(value.clone(), theme.diagram_caption));
            line
        })
        .collect()
}

/// 가로 막대 차트 한 벌.
///
/// `이름  막대  값` 줄을 항목마다 하나씩 쌓고, 원하면 아래에 눈금 축을 붙인다. 이름 칸 폭은 가장 긴
/// 이름에 맞추되, 막대밭이 너무 좁아지면 이름을 `…`로 줄여서라도 막대 자리를 남긴다.
///
/// 구조체를 그대로 채워 쓰거나 [`BarChart::new`]로 기본값을 받은 뒤 필요한 칸만 바꾼다.
#[derive(Clone, Copy, Debug)]
pub struct BarChart<'a> {
    /// 그릴 항목. 순서는 건드리지 않는다(정렬은 부르는 쪽 몫이다).
    pub items: &'a [ChartItem],
    /// 차트 전체 폭(칸).
    pub width: usize,
    /// 막대 길이 기준.
    pub scale: BarScale,
    /// 막대 오른쪽에 적을 수치 표기.
    pub value_display: ValueDisplay,
    /// 막대 줄의 최대 개수. 항목이 더 많으면 마지막 한 줄을 `… +N more`로 쓴다.
    pub max_rows: usize,
    /// 막대밭 아래에 눈금 축을 그릴지.
    pub show_axis: bool,
    /// 축 아래에 적을 제목. `show_axis`가 꺼져 있으면 무시한다.
    pub axis_title: Option<&'a str>,
}

impl<'a> BarChart<'a> {
    /// 기본값(최댓값 기준, 값만 표기, 축 없음, [`DEFAULT_MAX_ROWS`]줄)으로 만든다.
    pub fn new(items: &'a [ChartItem], width: usize) -> BarChart<'a> {
        BarChart {
            items,
            width,
            scale: BarScale::Max,
            value_display: ValueDisplay::Value,
            max_rows: DEFAULT_MAX_ROWS,
            show_axis: false,
            axis_title: None,
        }
    }

    /// 차트를 줄 목록으로 그린다.
    ///
    /// - 항목이 없으면 빈 목록을 돌려준다. 빈 틀을 그리든 코드블록으로 물러나든 부르는 쪽이 정한다.
    /// - `width`가 [`MIN_CHART_WIDTH`]보다 좁거나, 수치 표기가 길어 막대 넣을 자리가 안 남거나,
    ///   `max_rows`가 0이면 `None`이다. 다이어그램 렌더러 관례대로 그대로 흘려보내면 코드블록으로
    ///   대체된다.
    /// - 항목이 `max_rows`보다 많으면 앞에서부터 `max_rows - 1`줄만 그리고 마지막 줄에 `… +N more`로
    ///   몇 개가 빠졌는지 알린다. 막대 기준값은 *모든* 항목으로 계산하므로 줄을 접어도 길이가 변하지
    ///   않는다.
    /// - 어떤 입력에도 패닉하지 않는다.
    pub fn render(&self, theme: &Theme) -> Option<Vec<Line>> {
        if self.items.is_empty() {
            return Some(Vec::new());
        }
        if self.width < MIN_CHART_WIDTH || self.max_rows == 0 {
            return None;
        }
        let hidden = self.items.len().saturating_sub(self.max_rows);
        let shown = if hidden == 0 { self.items } else { &self.items[..self.max_rows - 1] };
        let hidden = self.items.len() - shown.len();

        let total = total_of(self.items);
        let values: Vec<String> = shown.iter().map(|item| format_display(item.value, total, self.value_display)).collect();
        let value_width = values.iter().map(|value| width_of(value)).max().unwrap_or(0);
        let available = self.width.saturating_sub(value_width + 2);
        if available < MIN_BAR_WIDTH + 1 {
            return None;
        }
        let longest_name = shown.iter().map(|item| width_of(&item.name)).max().unwrap_or(0);
        let name_width = longest_name.clamp(1, available - MIN_BAR_WIDTH);
        let bar_width = available - name_width;
        let axis = bar_axis(self.items, self.scale, bar_width)?;

        let mut lines: Vec<Line> = shown
            .iter()
            .zip(&values)
            .map(|(item, value)| {
                let name = truncate(&item.name, name_width);
                let mut line = Line::empty();
                let pad = name_width - width_of(&name);
                line.push(Span::new(name, theme.diagram_text));
                line.push_str(&" ".repeat(pad + 1), Style::PLAIN);
                line.append(&render_bar(&axis, item.value, theme));
                line.push_str(&" ".repeat(value_width - width_of(value) + 1), Style::PLAIN);
                line.push(Span::new(value.clone(), theme.diagram_caption));
                line
            })
            .collect();
        if hidden > 0 {
            lines.push(Line::single(truncate(&overflow_notice(hidden), self.width), theme.diagram_caption));
        }
        if self.show_axis {
            let indent = " ".repeat(name_width + 1);
            for row in render_horizontal_axis(&axis, self.axis_title, theme) {
                let mut line = Line::empty();
                line.push_str(&indent, Style::PLAIN);
                line.append(&row);
                lines.push(line);
            }
        }
        Some(lines)
    }
}

/// `length`칸에 `span`을 담을 때 쓸 눈금 간격.
///
/// 눈금 사이가 [`COLUMNS_PER_TICK`]칸 이상 벌어지는 선에서 가장 촘촘한 1·2·5 × 10ⁿ을 고른다.
/// 그래서 축이 넓으면 눈금이 촘촘해지고 좁으면 저절로 성기어진다.
fn nice_step(span: f64, length: usize) -> f64 {
    // 나누기를 먼저 해야 아주 큰 범위에서 곱이 무한대로 넘치지 않는다.
    let raw = span / length.max(1) as f64 * COLUMNS_PER_TICK as f64;
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f64.powf(raw.log10().floor());
    if !magnitude.is_finite() || magnitude <= 0.0 {
        return raw;
    }
    let fraction = raw / magnitude;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    let step = nice * magnitude;
    if step.is_finite() && step > 0.0 { step } else { raw }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<ChartItem> {
        vec![ChartItem::new("Dogs", 50.0), ChartItem::new("Cats", 30.0), ChartItem::new("Birds", 20.0)]
    }

    fn items(pairs: &[(&str, f64)]) -> Vec<ChartItem> {
        pairs.iter().map(|&(name, value)| ChartItem::new(name, value)).collect()
    }

    fn rows(lines: &[Line]) -> Vec<String> {
        lines.iter().map(Line::plain).collect()
    }

    // 1.1 범위와 폭에서 고른 간격의 눈금값·눈금 위치가 나온다.
    #[test]
    fn axis_ticks_are_evenly_spaced_with_positions() {
        let axis = Axis::new(0.0, 100.0, 41).unwrap();
        let values: Vec<f64> = axis.ticks().iter().map(|tick| tick.value).collect();
        assert_eq!(values, vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        let positions: Vec<usize> = axis.ticks().iter().map(|tick| tick.position).collect();
        assert_eq!(positions, vec![0, 8, 16, 24, 32, 40]);
        assert_eq!(axis.position_of(50.0), 20);
        assert_eq!(axis.position_of(-999.0), 0);
        assert_eq!(axis.position_of(999.0), 40);
        assert_eq!(axis.position_of(f64::NAN), 0);
    }

    // 1.1 눈금은 1·2·5 × 10ⁿ 배수에 놓인다.
    #[test]
    fn axis_ticks_use_nice_numbers() {
        let axis = Axis::new(0.0, 7.0, 40).unwrap();
        let values: Vec<f64> = axis.ticks().iter().map(|tick| tick.value).collect();
        assert_eq!(values, vec![0.0, 2.0, 4.0, 6.0]);
        let wide = Axis::new(3.0, 2700.0, 80).unwrap();
        let values: Vec<f64> = wide.ticks().iter().map(|tick| tick.value).collect();
        assert_eq!(values, vec![500.0, 1000.0, 1500.0, 2000.0, 2500.0]);
    }

    // 1.1 축은 눈금이 찍힌 선과 눈금값 줄로 그려진다.
    #[test]
    fn horizontal_axis_renders_line_and_labels() {
        let axis = Axis::new(0.0, 100.0, 21).unwrap();
        let rendered = rows(&render_horizontal_axis(&axis, None, &Theme::none()));
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[0], "├─────────┼─────────┤");
        assert_eq!(rendered[1], "0        50       100");
        assert_eq!(width_of(&rendered[0]), axis.length());
        assert!(width_of(&rendered[1]) <= axis.length());
    }

    // 1.2 축 제목은 따로 한 줄을 써서 눈금값과 겹치지 않고 잘리지도 않는다.
    #[test]
    fn axis_title_sits_on_its_own_row_untruncated() {
        let axis = Axis::new(0.0, 100.0, 40).unwrap();
        let rendered = rows(&render_horizontal_axis(&axis, Some("sales (KRW)"), &Theme::none()));
        assert_eq!(rendered.len(), 3);
        assert_eq!(rendered[2].trim(), "sales (KRW)");
        assert!(!rendered[2].contains('…'));
        assert!(rendered.iter().all(|row| width_of(row) <= axis.length()));
        // 제목이 비어 있으면 줄을 만들지 않는다.
        assert_eq!(render_horizontal_axis(&axis, Some("  "), &Theme::none()).len(), 2);
    }

    // 1.3 범위를 안 주면 값에서 0을 포함한 여유 있는 범위를 뽑는다.
    #[test]
    fn axis_from_values_adds_headroom_and_includes_zero() {
        let axis = Axis::from_values(&[3.0, 47.0, 12.0], 40).unwrap();
        assert_eq!(axis.min(), 0.0);
        assert_eq!(axis.max(), 50.0);
        assert_eq!(axis.ticks().first().map(|tick| tick.value), Some(0.0));
        assert_eq!(axis.ticks().last().map(|tick| tick.value), Some(50.0));
        let negative = Axis::from_values(&[-13.0, 8.0], 40).unwrap();
        assert!(negative.min() <= -13.0 && negative.max() >= 8.0);
        assert!(negative.min() < 0.0 && negative.max() > 0.0);
        // 값이 없거나 모두 같아도 무너지지 않는다.
        assert_eq!(Axis::from_values(&[], 20).unwrap().max(), 1.0);
        assert!(Axis::from_values(&[f64::NAN, f64::INFINITY], 20).is_some());
        assert!(Axis::from_values(&[5.0, 5.0], 20).is_some());
        assert!(Axis::from_values(&[1.0], 1).is_none());
    }

    #[test]
    fn axis_new_rejects_impossible_ranges() {
        assert!(Axis::new(0.0, 1.0, 1).is_none());
        assert!(Axis::new(f64::NAN, 1.0, 20).is_none());
        assert!(Axis::new(0.0, f64::INFINITY, 20).is_none());
        assert!(Axis::new(-f64::MAX, f64::MAX, 20).is_none());
        // 뒤집힌 범위는 바로잡고, 폭이 0이면 넓힌다.
        let flipped = Axis::new(10.0, 0.0, 20).unwrap();
        assert_eq!((flipped.min(), flipped.max()), (0.0, 10.0));
        let degenerate = Axis::new(4.0, 4.0, 20).unwrap();
        assert_eq!((degenerate.min(), degenerate.max()), (4.0, 5.0));
    }

    /// 범위가 칸 수에 견줘 터무니없이 넓으면 눈금이 한 칸에 겹친다. 그래도 눈금 만들기가 끝나야 한다.
    #[test]
    fn extreme_range_still_terminates_with_usable_ticks() {
        let axis = Axis::new(0.0, f64::MAX, 40).unwrap();
        assert!(axis.ticks().len() >= 2);
        assert!(axis.ticks().windows(2).all(|pair| pair[0].position < pair[1].position));
        assert!(axis.ticks().iter().all(|tick| tick.value <= axis.max() && tick.position < axis.length()));
        // 값이 너무 커서 1을 더해도 제자리면 폭 0을 넓힐 수 없으므로 그릴 수 없다고 알린다.
        assert!(Axis::new(1e300, 1e300, 40).is_none());
    }

    // 2.1 막대 길이는 최댓값 기준으로 값에 비례한다.
    #[test]
    fn bars_are_proportional_to_max() {
        let axis = bar_axis(&sample(), BarScale::Max, 20).unwrap();
        let bars = rows(&sample().iter().map(|item| render_bar(&axis, item.value, &Theme::none())).collect::<Vec<_>>());
        assert_eq!(bars[0], "████████████████████");
        assert_eq!(bars[1], "████████████".to_string() + "        ");
        assert_eq!(bars[2], "████████".to_string() + "            ");
        assert!(bars.iter().all(|bar| width_of(bar) == 20));
    }

    // 2.1 합 기준이면 막대 길이가 곧 몫이다.
    #[test]
    fn bars_are_proportional_to_total() {
        let axis = bar_axis(&sample(), BarScale::Total, 20).unwrap();
        let fills: Vec<usize> = sample().iter().map(|item| axis.bar_span(item.value).1).collect();
        assert_eq!(fills, vec![10, 6, 4]);
        assert_eq!(fills.iter().sum::<usize>(), 20);
    }

    // 2.2 값이 모두 0이면 막대는 0칸이다(0으로 나누지 않는다).
    #[test]
    fn all_zero_values_render_empty_bars() {
        let zeros = items(&[("a", 0.0), ("b", 0.0)]);
        for scale in [BarScale::Max, BarScale::Total] {
            let axis = bar_axis(&zeros, scale, 20).unwrap();
            for item in &zeros {
                assert_eq!(axis.bar_span(item.value).1, 0);
                assert_eq!(render_bar(&axis, item.value, &Theme::none()).plain().trim(), "");
            }
        }
        let chart = BarChart::new(&zeros, 40).render(&Theme::none()).unwrap();
        assert_eq!(rows(&chart), vec![format!("a{}0", " ".repeat(38)), format!("b{}0", " ".repeat(38))]);
    }

    // 2.3 음수는 기준선 반대쪽으로 뻗고 글자도 다르다.
    #[test]
    fn negative_values_extend_from_baseline() {
        let mixed = items(&[("up", 10.0), ("down", -5.0), ("zero", 0.0)]);
        let axis = bar_axis(&mixed, BarScale::Max, 15).unwrap();
        assert_eq!((axis.min(), axis.max()), (-5.0, 10.0));
        let baseline = axis.baseline();
        assert_eq!(baseline, 5);
        let bars = rows(&mixed.iter().map(|item| render_bar(&axis, item.value, &Theme::none())).collect::<Vec<_>>());
        assert_eq!(bars[0], "     ██████████");
        assert_eq!(bars[1], "▒▒▒▒▒│         ");
        assert_eq!(bars[2], "     │         ");
        assert!(bars[1].starts_with(NEGATIVE_FILL));
        // 값이 전부 음수면 기준선이 오른쪽 끝이다.
        let negatives = items(&[("a", -4.0), ("b", -2.0)]);
        let axis = bar_axis(&negatives, BarScale::Max, 8).unwrap();
        assert_eq!(axis.baseline(), 8);
        assert_eq!(axis.bar_span(-4.0), (0, 8));
        assert_eq!(axis.bar_span(-2.0), (4, 4));
    }

    // 2.1 0이 아닌 작은 값도 한 칸은 보인다.
    #[test]
    fn tiny_values_keep_one_visible_cell() {
        let axis = bar_axis(&items(&[("big", 1000.0), ("tiny", 0.4)]), BarScale::Max, 20).unwrap();
        assert_eq!(axis.bar_span(0.4), (0, 1));
        assert_eq!(axis.bar_span(0.0).1, 0);
    }

    // 3.1 범례는 이름과 값(또는 몫)을 함께 보여 준다.
    #[test]
    fn legend_shows_name_and_value() {
        let legend = rows(&render_legend(&sample(), ValueDisplay::Value, 20, &Theme::none()));
        assert_eq!(
            legend,
            vec![format!("■ Dogs{}50", " ".repeat(12)), format!("■ Cats{}30", " ".repeat(12)), format!("■ Birds{}20", " ".repeat(11))]
        );
        assert!(legend.iter().all(|row| width_of(row) == 20));
        let percent = rows(&render_legend(&sample(), ValueDisplay::ValueAndPercent, 30, &Theme::none()));
        assert_eq!(percent[0], format!("■ Dogs{}50 (50.0%)", " ".repeat(14)));
        assert!(percent.iter().all(|row| width_of(row) <= 30));
        assert!(render_legend(&[], ValueDisplay::Value, 20, &Theme::none()).is_empty());
    }

    // 3.2 긴 이름은 줄여 넣고 나머지 배치를 망가뜨리지 않는다.
    #[test]
    fn legend_truncates_long_names() {
        let long = items(&[("아주 길고 긴 한글 항목 이름입니다", 12.5), ("짧음", 1.0)]);
        let legend = rows(&render_legend(&long, ValueDisplay::Value, 24, &Theme::none()));
        assert!(legend[0].contains('…'));
        assert!(legend.iter().all(|row| width_of(row) == 24));
        // 자리가 아주 좁으면 이름이 밀려나고 값만 남지만 폭은 지킨다.
        for width in MIN_LEGEND_WIDTH..12 {
            let narrow = rows(&render_legend(&long, ValueDisplay::ValueAndPercent, width, &Theme::none()));
            assert!(narrow.iter().all(|row| width_of(row) <= width), "폭 {width}: {narrow:?}");
        }
        assert!(render_legend(&long, ValueDisplay::Value, MIN_LEGEND_WIDTH - 1, &Theme::none()).is_empty());
    }

    // 4.1 정수는 소수점 없이.
    #[test]
    fn integers_format_without_decimal_point() {
        assert_eq!(format_value(50.0), "50");
        assert_eq!(format_value(-7.0), "-7");
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(-0.0), "0");
        assert_eq!(format_value(1234567.0), "1234567");
    }

    // 4.2 소수는 두 자리 고정.
    #[test]
    fn fractions_format_with_two_decimals() {
        assert_eq!(format_value(12.3456), "12.35");
        assert_eq!(format_value(0.5), "0.50");
        assert_eq!(format_value(-2.005), "-2.01");
        assert_eq!(format_value(2.999), "3");
        assert_eq!(format_value(0.001), "0");
        assert_eq!(format_value(f64::NAN), "-");
        assert_eq!(format_value(f64::INFINITY), "-");
        assert_eq!(format_value(1e20), "100000000000000000000");
    }

    // 4.3 몫은 백분율로.
    #[test]
    fn shares_format_as_percent() {
        assert_eq!(format_percent(50.0, 100.0), "50.0%");
        assert_eq!(format_percent(1.0, 3.0), "33.3%");
        assert_eq!(format_percent(-25.0, 100.0), "-25.0%");
        assert_eq!(format_percent(1.0, 0.0), "0.0%");
        assert_eq!(format_percent(f64::NAN, 100.0), "0.0%");
        assert_eq!(format_percent(0.0, 100.0), "0.0%");
        assert_eq!(format_display(30.0, 100.0, ValueDisplay::Percent), "30.0%");
        assert_eq!(format_display(30.0, 100.0, ValueDisplay::ValueAndPercent), "30 (30.0%)");
        assert_eq!(total_of(&sample()), 100.0);
        assert_eq!(total_of(&items(&[("a", -3.0), ("b", 1.0), ("c", f64::NAN)])), 4.0);
    }

    // 5.1 항목이 자리보다 많으면 남은 개수를 알리는 줄로 접는다.
    #[test]
    fn overflowing_items_fold_into_more_row() {
        let many: Vec<ChartItem> = (0..10).map(|index| ChartItem::new(format!("i{index}"), index as f64 + 1.0)).collect();
        let chart = BarChart { max_rows: 4, ..BarChart::new(&many, 40) };
        let lines = rows(&chart.render(&Theme::none()).unwrap());
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[3], "… +7 more");
        assert!(lines[0].starts_with("i0"));
        // 기준값은 접기 전 전체 항목으로 잡으므로 보이는 막대가 꽉 차지 않는다.
        assert!(!lines[2].contains(&POSITIVE_FILL.to_string().repeat(30)));
        assert!(BarChart { max_rows: 0, ..BarChart::new(&many, 40) }.render(&Theme::none()).is_none());
    }

    // 5.2 항목이 없으면 빈 결과(패닉 아님).
    #[test]
    fn empty_items_render_empty_result() {
        assert_eq!(BarChart::new(&[], 40).render(&Theme::none()), Some(Vec::new()));
        assert_eq!(BarChart::new(&[], 4).render(&Theme::none()), Some(Vec::new()));
    }

    #[test]
    fn narrow_width_or_long_numbers_signal_fallback() {
        let long_numbers = items(&[("a", 1e18), ("b", 2e18)]);
        assert!(BarChart::new(&sample(), 7).render(&Theme::none()).is_none());
        assert!(BarChart::new(&long_numbers, 20).render(&Theme::none()).is_none());
        assert!(BarChart::new(&long_numbers, 60).render(&Theme::none()).is_some());
    }

    #[test]
    fn bar_chart_with_axis_aligns_columns() {
        let sample = sample();
        let chart = BarChart { show_axis: true, axis_title: Some("count"), ..BarChart::new(&sample, 40) };
        let lines = rows(&chart.render(&Theme::none()).unwrap());
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], format!("Dogs  {} 50", POSITIVE_FILL.to_string().repeat(31)));
        let bar_start = lines[0].find('█').unwrap();
        assert_eq!(lines[3].find('├'), Some(bar_start));
        assert!(lines[5].contains("count"));
        assert!(lines.iter().all(|row| width_of(row) <= 40));
    }

    // 5.3 어떤 입력·폭에도 패닉하지 않는다.
    #[test]
    fn never_panics_across_edge_cases() {
        let long_name = "아주".repeat(60);
        let cases: Vec<Vec<ChartItem>> = vec![
            Vec::new(),
            items(&[("solo", 1.0)]),
            items(&[("solo", 0.0)]),
            sample(),
            items(&[("a", 0.0), ("b", 0.0), ("c", 0.0)]),
            items(&[("neg", -5.0), ("pos", 5.0), ("zero", 0.0)]),
            items(&[("all neg", -5.0), ("more neg", -50.0)]),
            items(&[("huge", f64::MAX), ("small", f64::MIN_POSITIVE)]),
            items(&[("nan", f64::NAN), ("inf", f64::INFINITY), ("ninf", f64::NEG_INFINITY)]),
            items(&[(long_name.as_str(), 1.5), ("🙂🙂🙂", -2.25)]),
            (0..200).map(|index| ChartItem::new(format!("item {index}"), (index as f64 - 100.0) * 1.5)).collect(),
        ];
        for theme in [Theme::none(), Theme::dark()] {
            for case in &cases {
                for width in [0usize, 1, 8, 13, 40, 80, 200] {
                    for scale in [BarScale::Max, BarScale::Total] {
                        for display in [ValueDisplay::Value, ValueDisplay::Percent, ValueDisplay::ValueAndPercent] {
                            let chart = BarChart {
                                scale,
                                value_display: display,
                                show_axis: true,
                                axis_title: Some("축 제목"),
                                max_rows: 5,
                                ..BarChart::new(case, width)
                            };
                            if let Some(lines) = chart.render(&theme) {
                                assert!(lines.iter().all(|line| line.width() <= width));
                            }
                            for line in render_legend(case, display, width, &theme) {
                                assert!(line.width() <= width);
                            }
                        }
                        if let Some(axis) = bar_axis(case, scale, width) {
                            for item in case {
                                let _ = render_bar(&axis, item.value, &theme);
                            }
                            let _ = render_horizontal_axis(&axis, Some("t"), &theme);
                        }
                    }
                    let values: Vec<f64> = case.iter().map(|item| item.value).collect();
                    if let Some(axis) = Axis::from_values(&values, width) {
                        let _ = render_horizontal_axis(&axis, None, &theme);
                    }
                }
            }
        }
    }
}
