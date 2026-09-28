//! 줄 기반 YAML 블록 스타일 하위집합 리더 · 트리 빌더.
//!
//! BPMN 문서를 손으로 쓰기에 충분한 YAML 부분집합(블록 매핑·블록 시퀀스·평문/홑따옴표/겹따옴표
//! 스칼라·`#` 주석·임의 폭 들여쓰기)만 값 트리로 읽는다. BPMN 이름은 하나도 모른다 — 스키마는
//! `super::parse_yaml`이 안다. 완전한 YAML 1.2가 아니다: 앵커·별칭·태그·흐름 컬렉션·블록 스칼라·
//! 복합 키·다중 문서·암묵 타입 변환은 거부하거나(`Unsupported`) 문자열로만 다룬다.
//!
//! 줄 단위로 따옴표 밖 `#` 주석을 지우고 빈 줄을 건너뛴 뒤, 들여쓰기 폭을 명시적 스택으로 추적해
//! 트리를 쌓는다(재귀 없음). `source.lines()`·`char_indices`·`str` 슬라이스만 쓴다.

use std::collections::HashSet;
use std::fmt;

/// YAML 값 트리 하나의 노드.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YamlValue {
    Scalar(String),
    Seq(Vec<YamlValue>),
    /// 선언 순서 보존. 같은 매핑의 키 중복은 오류.
    Map(Vec<(String, YamlValue)>),
}

impl YamlValue {
    /// `Map`이 아니거나 키가 없으면 `None`.
    pub fn get(&self, key: &str) -> Option<&YamlValue> {
        match self {
            YamlValue::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_scalar(&self) -> Option<&str> {
        match self {
            YamlValue::Scalar(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_seq(&self) -> Option<&[YamlValue]> {
        match self {
            YamlValue::Seq(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(String, YamlValue)]> {
        match self {
            YamlValue::Map(entries) => Some(entries),
            _ => None,
        }
    }
}

/// 범위 밖 YAML 1.2 기능 — 거부만 한다(§Out-of-Scope).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFeature {
    Anchor,
    Alias,
    Tag,
    FlowCollection,
    BlockScalar,
    ComplexKey,
    MultipleDocuments,
}

impl fmt::Display for UnsupportedFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            UnsupportedFeature::Anchor => "앵커",
            UnsupportedFeature::Alias => "별칭",
            UnsupportedFeature::Tag => "태그",
            UnsupportedFeature::FlowCollection => "흐름 컬렉션",
            UnsupportedFeature::BlockScalar => "블록 스칼라",
            UnsupportedFeature::ComplexKey => "복합 키",
            UnsupportedFeature::MultipleDocuments => "다중 문서",
        };
        write!(f, "{name}")
    }
}

/// `line`은 1부터 세는 원문 줄 번호.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YamlError {
    /// 내용 줄이 없음(빈 문자열·공백·주석만·`---`만).
    Empty,
    TabIndentation { line: usize },
    /// 열린 어떤 수준과도 맞지 않는 들여쓰기, 여러 줄 평문 스칼라.
    BadIndentation { line: usize },
    /// `키:`도 `- `도 아닌 줄, 닫는 따옴표 뒤 남은 글.
    MalformedLine { line: usize },
    /// 같은 수준에 `- 항목`과 `키: 값` 혼재.
    MixedCollection { line: usize },
    UnterminatedQuote { line: usize },
    DuplicateKey { line: usize, key: String },
    Unsupported { line: usize, feature: UnsupportedFeature },
    TooDeep { line: usize },
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YamlError::Empty => write!(f, "문서에 내용이 없다"),
            YamlError::TabIndentation { line } => write!(f, "{line}번째 줄의 들여쓰기에 탭이 있다"),
            YamlError::BadIndentation { line } => write!(f, "{line}번째 줄의 들여쓰기가 열린 어떤 수준과도 맞지 않는다"),
            YamlError::MalformedLine { line } => write!(f, "{line}번째 줄의 형식이 올바르지 않다"),
            YamlError::MixedCollection { line } => write!(f, "{line}번째 줄에서 시퀀스 항목과 매핑 키가 섞였다"),
            YamlError::UnterminatedQuote { line } => write!(f, "{line}번째 줄의 따옴표가 닫히지 않았다"),
            YamlError::DuplicateKey { line, key } => write!(f, "{line}번째 줄의 키 {key:?}가 같은 매핑에서 중복됐다"),
            YamlError::Unsupported { line, feature } => write!(f, "{line}번째 줄에 지원하지 않는 YAML 기능이 있다: {feature}"),
            YamlError::TooDeep { line } => write!(f, "{line}번째 줄에서 중첩 깊이 상한({MAX_DEPTH})을 넘었다"),
        }
    }
}

pub const MAX_DEPTH: usize = 64;

/// 문서 → 값 트리. 첫 내용 줄의 `---`는 무시, 그 밖의 `---`/`...`는 `MultipleDocuments`.
pub fn parse_document(source: &str) -> Result<YamlValue, YamlError> {
    let lines = preprocess(source)?;

    // 문서가 내용 줄 하나뿐이고 그 줄이 매핑도 시퀀스도 아니면 문서 자체가 스칼라 하나다.
    if lines.len() == 1 {
        let (line_no, _indent, content) = &lines[0];
        if let LineShape::Other(text) = classify_line(content) {
            return Ok(YamlValue::Scalar(decode_scalar(text, *line_no)?));
        }
    }

    let mut stack: Vec<Frame> = Vec::new();
    for (line_no, indent, content) in &lines {
        let (line_no, indent) = (*line_no, *indent);

        // 이 줄이 이어받지 않는 프레임을 닫는다(더 얕거나 같은 들여쓰기).
        while let Some(top) = stack.last() {
            let top_indent = top.indent();
            if indent < top_indent {
                if stack.len() == 1 {
                    return Err(YamlError::BadIndentation { line: line_no });
                }
                let finished = stack.pop().expect("스택이 비어있지 않음을 위에서 확인했다").finish();
                stack.last_mut().expect("길이 1 초과를 위에서 확인했다").attach(finished);
                continue;
            }
            if indent == top_indent {
                stack.last_mut().expect("top이 Some이었다").resolve_pending_as_empty();
            }
            break;
        }

        let opens_new_frame = match stack.last() {
            None => true,
            Some(top) if top.indent() == indent => false,
            Some(top) => {
                if !top.has_pending() {
                    return Err(YamlError::BadIndentation { line: line_no });
                }
                true
            }
        };

        let shape = classify_line(content);
        if opens_new_frame {
            if matches!(shape, LineShape::Other(_)) {
                return Err(YamlError::MalformedLine { line: line_no });
            }
            if stack.len() >= MAX_DEPTH {
                return Err(YamlError::TooDeep { line: line_no });
            }
            let mut frame = match &shape {
                LineShape::Dash { .. } => Frame::new_seq(indent),
                LineShape::MapEntry { .. } => Frame::new_map(indent),
                LineShape::Other(_) => unreachable!("위에서 걸러졌다"),
            };
            let extra = insert_entry_into(&mut frame, shape, line_no)?;
            stack.push(frame);
            if let Some(child) = extra {
                if stack.len() >= MAX_DEPTH {
                    return Err(YamlError::TooDeep { line: line_no });
                }
                stack.push(child);
            }
        } else {
            let top = stack.last_mut().expect("opens_new_frame이 거짓이면 top이 있다");
            let extra = insert_entry_into(top, shape, line_no)?;
            if let Some(child) = extra {
                if stack.len() >= MAX_DEPTH {
                    return Err(YamlError::TooDeep { line: line_no });
                }
                stack.push(child);
            }
        }
    }

    while stack.len() > 1 {
        let finished = stack.pop().expect("길이 1 초과").finish();
        stack.last_mut().expect("길이 1 초과").attach(finished);
    }
    Ok(stack.pop().expect("내용 줄이 있으면 프레임이 하나는 만들어진다").finish())
}

// --- 줄 전처리: 주석 제거·빈 줄 건너뜀·탭 검사·문서 구분자 ---

fn preprocess(source: &str) -> Result<Vec<(usize, usize, String)>, YamlError> {
    let mut lines: Vec<(usize, usize, String)> = Vec::new();
    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let stripped = strip_comment(raw);
        let trimmed_end = stripped.trim_end();
        if trimmed_end.trim_start().is_empty() {
            continue;
        }
        let indent = trimmed_end.chars().take_while(|c| *c == ' ').count();
        if trimmed_end[indent..].starts_with('\t') {
            return Err(YamlError::TabIndentation { line: line_no });
        }
        lines.push((line_no, indent, trimmed_end[indent..].to_string()));
    }

    if let Some((_, indent, content)) = lines.first()
        && *indent == 0
        && content == "---"
    {
        lines.remove(0);
    }
    for (line_no, indent, content) in &lines {
        if *indent == 0 && (content == "---" || content == "...") {
            return Err(YamlError::Unsupported { line: *line_no, feature: UnsupportedFeature::MultipleDocuments });
        }
    }

    if lines.is_empty() {
        return Err(YamlError::Empty);
    }
    Ok(lines)
}

/// 따옴표 밖의, 줄 시작 또는 공백 뒤에 오는 첫 `#`부터 줄 끝까지 지운다(YAML §6.6).
fn strip_comment(raw: &str) -> &str {
    #[derive(Clone, Copy, PartialEq)]
    enum Quote {
        None,
        Single,
        Double,
    }
    let mut quote = Quote::None;
    let mut prev_is_space = true;
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match quote {
            Quote::Double => {
                if c == '\\' {
                    chars.next();
                } else if c == '"' {
                    quote = Quote::None;
                }
                prev_is_space = false;
            }
            Quote::Single => {
                if c == '\'' {
                    if chars.peek().map(|&(_, nc)| nc) == Some('\'') {
                        chars.next();
                    } else {
                        quote = Quote::None;
                    }
                }
                prev_is_space = false;
            }
            Quote::None => {
                if c == '"' {
                    quote = Quote::Double;
                    prev_is_space = false;
                } else if c == '\'' {
                    quote = Quote::Single;
                    prev_is_space = false;
                } else if c == '#' && prev_is_space {
                    return &raw[..i];
                } else {
                    prev_is_space = c.is_whitespace();
                }
            }
        }
    }
    raw
}

// --- 줄 모양: 시퀀스 항목 / 매핑 항목 / 그 밖 ---

enum LineShape<'a> {
    /// `-` 단독(값은 다음 줄의 더 깊은 블록) 또는 `- 나머지`(나머지의 시작 열).
    Dash { inline: Option<(&'a str, usize)> },
    MapEntry { key: String, value: Option<String> },
    Other(&'a str),
}

enum Inner<'a> {
    MapEntry { key: String, value: Option<String> },
    Scalar(&'a str),
}

fn classify_line(content: &str) -> LineShape<'_> {
    if content == "-" {
        return LineShape::Dash { inline: None };
    }
    if let Some(rest) = content.strip_prefix("- ") {
        let trimmed = rest.trim_start_matches(' ');
        let col = 2 + (rest.len() - trimmed.len());
        return LineShape::Dash { inline: Some((trimmed, col)) };
    }
    match classify_scalar_or_entry(content) {
        Inner::MapEntry { key, value } => LineShape::MapEntry { key, value },
        Inner::Scalar(_) => LineShape::Other(content),
    }
}

fn classify_scalar_or_entry(text: &str) -> Inner<'_> {
    if let Some(pos) = find_unquoted_colon(text) {
        let key = text[..pos].trim();
        let after = text[pos + 1..].trim_start();
        if !key.is_empty() {
            return Inner::MapEntry { key: key.to_string(), value: if after.is_empty() { None } else { Some(after.to_string()) } };
        }
    }
    Inner::Scalar(text)
}

/// 따옴표 밖에서, 줄 끝이거나 공백이 뒤따르는 첫 `:`의 바이트 위치.
fn find_unquoted_colon(content: &str) -> Option<usize> {
    #[derive(Clone, Copy, PartialEq)]
    enum Quote {
        None,
        Single,
        Double,
    }
    let mut quote = Quote::None;
    let mut chars = content.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match quote {
            Quote::Double => {
                if c == '\\' {
                    chars.next();
                } else if c == '"' {
                    quote = Quote::None;
                }
            }
            Quote::Single => {
                if c == '\'' {
                    if chars.peek().map(|&(_, nc)| nc) == Some('\'') {
                        chars.next();
                    } else {
                        quote = Quote::None;
                    }
                }
            }
            Quote::None => {
                if c == '"' {
                    quote = Quote::Double;
                } else if c == '\'' {
                    quote = Quote::Single;
                } else if c == ':' && content[i + 1..].chars().next().is_none_or(|nc| nc.is_whitespace()) {
                    return Some(i);
                }
            }
        }
    }
    None
}

// --- 스칼라 디코딩: 평문 trim · 홑따옴표 · 겹따옴표. 타입 변환 없음 ---

fn decode_scalar(raw: &str, line_no: usize) -> Result<String, YamlError> {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('\'') {
        return decode_single_quoted(rest, line_no);
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        return decode_double_quoted(rest, line_no);
    }
    reject_unsupported_plain(trimmed, line_no)?;
    Ok(trimmed.to_string())
}

fn decode_single_quoted(rest: &str, line_no: usize) -> Result<String, YamlError> {
    let mut out = String::new();
    let mut chars = rest.char_indices().peekable();
    let mut closed_at: Option<usize> = None;
    while let Some((i, c)) = chars.next() {
        if c == '\'' {
            if chars.peek().map(|&(_, nc)| nc) == Some('\'') {
                out.push('\'');
                chars.next();
                continue;
            }
            closed_at = Some(i + 1);
            break;
        }
        out.push(c);
    }
    let Some(end) = closed_at else {
        return Err(YamlError::UnterminatedQuote { line: line_no });
    };
    if !rest[end..].trim().is_empty() {
        return Err(YamlError::MalformedLine { line: line_no });
    }
    Ok(out)
}

fn decode_double_quoted(rest: &str, line_no: usize) -> Result<String, YamlError> {
    let mut out = String::new();
    let mut chars = rest.char_indices().peekable();
    let mut closed_at: Option<usize> = None;
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, other)) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
            continue;
        }
        if c == '"' {
            closed_at = Some(i + 1);
            break;
        }
        out.push(c);
    }
    let Some(end) = closed_at else {
        return Err(YamlError::UnterminatedQuote { line: line_no });
    };
    if !rest[end..].trim().is_empty() {
        return Err(YamlError::MalformedLine { line: line_no });
    }
    Ok(out)
}

fn reject_unsupported_plain(text: &str, line_no: usize) -> Result<(), YamlError> {
    let feature = if text.starts_with('&') {
        Some(UnsupportedFeature::Anchor)
    } else if text.starts_with('*') {
        Some(UnsupportedFeature::Alias)
    } else if text.starts_with('!') {
        Some(UnsupportedFeature::Tag)
    } else if text.starts_with('[') || text.starts_with('{') {
        Some(UnsupportedFeature::FlowCollection)
    } else if text.starts_with('|') || text.starts_with('>') {
        Some(UnsupportedFeature::BlockScalar)
    } else if text == "?" || text.starts_with("? ") {
        Some(UnsupportedFeature::ComplexKey)
    } else {
        None
    };
    match feature {
        Some(feature) => Err(YamlError::Unsupported { line: line_no, feature }),
        None => Ok(()),
    }
}

// --- 트리 빌더: 명시적 스택 ---

struct MapFrame {
    indent: usize,
    entries: Vec<(String, YamlValue)>,
    keys: HashSet<String>,
    /// 값이 없어 다음 줄을 기다리는 키.
    pending: Option<String>,
}

struct SeqFrame {
    indent: usize,
    items: Vec<YamlValue>,
    /// `-` 단독 또는 `- 키: 값`이 만든 중첩 프레임이 아직 안 닫힘.
    pending: bool,
}

enum Frame {
    Map(MapFrame),
    Seq(SeqFrame),
}

impl Frame {
    fn new_map(indent: usize) -> Frame {
        Frame::Map(MapFrame { indent, entries: Vec::new(), keys: HashSet::new(), pending: None })
    }

    fn new_seq(indent: usize) -> Frame {
        Frame::Seq(SeqFrame { indent, items: Vec::new(), pending: false })
    }

    fn indent(&self) -> usize {
        match self {
            Frame::Map(m) => m.indent,
            Frame::Seq(s) => s.indent,
        }
    }

    fn has_pending(&self) -> bool {
        match self {
            Frame::Map(m) => m.pending.is_some(),
            Frame::Seq(s) => s.pending,
        }
    }

    fn resolve_pending_as_empty(&mut self) {
        match self {
            Frame::Map(m) => {
                if let Some(key) = m.pending.take() {
                    m.entries.push((key, YamlValue::Scalar(String::new())));
                }
            }
            Frame::Seq(s) => {
                if s.pending {
                    s.items.push(YamlValue::Scalar(String::new()));
                    s.pending = false;
                }
            }
        }
    }

    fn finish(mut self) -> YamlValue {
        self.resolve_pending_as_empty();
        match self {
            Frame::Map(m) => YamlValue::Map(m.entries),
            Frame::Seq(s) => YamlValue::Seq(s.items),
        }
    }

    /// 자식 프레임이 다 만들어져 값을 부모의 대기 슬롯에 채운다. 대기 슬롯이 없으면 내부 불변식
    /// 위반(자식은 대기 슬롯이 있을 때만 만들어진다).
    fn attach(&mut self, value: YamlValue) {
        match self {
            Frame::Map(m) => {
                let key = m.pending.take().expect("부모 매핑은 자식을 만들 때 대기 중인 키가 있다");
                m.entries.push((key, value));
            }
            Frame::Seq(s) => {
                debug_assert!(s.pending, "부모 시퀀스는 자식을 만들 때 대기 상태다");
                s.pending = false;
                s.items.push(value);
            }
        }
    }
}

/// `frame`에 이 줄의 항목을 넣는다. `frame`이 갓 만들어졌든 기존 것이든 같다. 시퀀스의
/// `- 키: 값`처럼 새 중첩 프레임을 열어야 하면 그 프레임을 돌려준다(호출자가 깊이를 확인하고 쌓는다).
fn insert_entry_into(frame: &mut Frame, shape: LineShape, line_no: usize) -> Result<Option<Frame>, YamlError> {
    match (frame, shape) {
        (Frame::Seq(seq), LineShape::Dash { inline: None }) => {
            seq.pending = true;
            Ok(None)
        }
        (Frame::Seq(seq), LineShape::Dash { inline: Some((text, col)) }) => match classify_scalar_or_entry(text) {
            Inner::Scalar(raw) => {
                seq.items.push(YamlValue::Scalar(decode_scalar(raw, line_no)?));
                Ok(None)
            }
            Inner::MapEntry { key, value } => {
                let mut child = MapFrame { indent: seq.indent + col, entries: Vec::new(), keys: HashSet::new(), pending: None };
                insert_map_entry(&mut child, key, value, line_no)?;
                seq.pending = true;
                Ok(Some(Frame::Map(child)))
            }
        },
        (Frame::Map(map), LineShape::MapEntry { key, value }) => {
            insert_map_entry(map, key, value, line_no)?;
            Ok(None)
        }
        (Frame::Seq(_), LineShape::MapEntry { .. }) | (Frame::Map(_), LineShape::Dash { .. }) => Err(YamlError::MixedCollection { line: line_no }),
        (_, LineShape::Other(_)) => Err(YamlError::MalformedLine { line: line_no }),
    }
}

fn insert_map_entry(map: &mut MapFrame, key: String, value: Option<String>, line_no: usize) -> Result<(), YamlError> {
    if !map.keys.insert(key.clone()) {
        return Err(YamlError::DuplicateKey { line: line_no, key });
    }
    match value {
        None => map.pending = Some(key),
        Some(raw) => map.entries.push((key, YamlValue::Scalar(decode_scalar(&raw, line_no)?))),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(s: &str) -> YamlValue {
        YamlValue::Scalar(s.to_string())
    }

    fn map(entries: Vec<(&str, YamlValue)>) -> YamlValue {
        YamlValue::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    // --- 2.1 임의 폭 들여쓰기 ---

    #[test]
    fn width_two_four_and_mixed_indentation_produce_the_same_tree() {
        let width2 = "nodes:\n  a: startEvent\n  b:\n    kind: task\n    name: 작업\n";
        let width4 = "nodes:\n    a: startEvent\n    b:\n        kind: task\n        name: 작업\n";
        let mixed = "nodes:\n   a: startEvent\n   b:\n       kind: task\n       name: 작업\n";
        let expected = map(vec![(
            "nodes",
            map(vec![("a", scalar("startEvent")), ("b", map(vec![("kind", scalar("task")), ("name", scalar("작업"))]))]),
        )]);
        assert_eq!(parse_document(width2), Ok(expected.clone()));
        assert_eq!(parse_document(width4), Ok(expected.clone()));
        assert_eq!(parse_document(mixed), Ok(expected));
    }

    // --- 2.2 스칼라 세 종류 ---

    #[test]
    fn plain_single_and_double_quoted_scalars_decode_and_unknown_escapes_stay_literal() {
        assert_eq!(parse_document("a: 그대로\n"), Ok(map(vec![("a", scalar("그대로"))])));
        assert_eq!(parse_document("a: 'it''s'\n"), Ok(map(vec![("a", scalar("it's"))])));
        assert_eq!(parse_document(r#"a: "줄1\n줄2\t끝\\slash""#), Ok(map(vec![("a", scalar("줄1\n줄2\t끝\\slash"))])));
        // `\x`처럼 알려지지 않은 이스케이프는 원문 그대로(백슬래시 포함) 유지한다.
        assert_eq!(parse_document(r#"a: "\x""#), Ok(map(vec![("a", scalar("\\x"))])));
    }

    // --- 2.3 주석·빈 줄·줄 끝 공백 ---

    #[test]
    fn comments_blank_lines_and_trailing_whitespace_do_not_change_the_tree() {
        let bare = "nodes:\n  a: startEvent\n";
        let decorated = "# 머리말\nnodes:   \n\n  # 주석 줄\n  a: startEvent   \n\n";
        assert_eq!(parse_document(bare), parse_document(decorated));
        // 따옴표 밖 `#`만 주석이고, `a#b`처럼 공백 없이 붙은 `#`는 스칼라 본문이다.
        assert_eq!(parse_document("k: a#b\n"), Ok(map(vec![("k", scalar("a#b"))])));
    }

    // --- 2.4 따옴표 안 특수문자 ---

    #[test]
    fn quoted_hash_colon_and_dash_survive_as_scalar_body() {
        assert_eq!(parse_document("k: '# not a comment: - not a dash'\n"), Ok(map(vec![("k", scalar("# not a comment: - not a dash"))])));
        assert_eq!(parse_document(r##"k: "# c : d - e""##), Ok(map(vec![("k", scalar("# c : d - e"))])));
    }

    // --- 2.5 암묵 타입 변환 없음 ---

    #[test]
    fn type_like_plain_scalars_stay_strings() {
        let doc = "a: true\nb: 007\nc: 1.0\nd: null\ne: ~\n";
        let model = parse_document(doc).unwrap();
        for (key, expected) in [("a", "true"), ("b", "007"), ("c", "1.0"), ("d", "null"), ("e", "~")] {
            assert_eq!(model.get(key).and_then(YamlValue::as_scalar), Some(expected));
        }
    }

    // --- 2.6 빈 값 → 블록 또는 빈 문자열 ---

    #[test]
    fn empty_value_becomes_the_following_deeper_block_or_an_empty_string() {
        assert_eq!(parse_document("a:\n  b: 1\nc:\n"), Ok(map(vec![("a", map(vec![("b", scalar("1"))])), ("c", scalar(""))])));
    }

    // --- 2.7 선두 --- 무시 ---

    #[test]
    fn a_leading_document_marker_is_ignored() {
        assert_eq!(parse_document("---\na: 1\n"), parse_document("a: 1\n"));
    }

    // --- 2.8 거부 목록 ---

    #[test]
    fn anchor_alias_tag_flow_collection_block_scalar_complex_key_and_second_marker_are_rejected() {
        let cases: &[(&str, UnsupportedFeature)] = &[
            ("k: &a\n", UnsupportedFeature::Anchor),
            ("k: *a\n", UnsupportedFeature::Alias),
            ("k: !!str\n", UnsupportedFeature::Tag),
            ("k: [a, b]\n", UnsupportedFeature::FlowCollection),
            ("k: {k: v}\n", UnsupportedFeature::FlowCollection),
            ("k: |\n", UnsupportedFeature::BlockScalar),
            ("k: >\n", UnsupportedFeature::BlockScalar),
            ("k: ? k\n", UnsupportedFeature::ComplexKey),
        ];
        for (source, feature) in cases {
            assert_eq!(parse_document(source), Err(YamlError::Unsupported { line: 1, feature: *feature }), "{source:?}");
        }
        assert_eq!(parse_document("a: 1\n---\nb: 2\n"), Err(YamlError::Unsupported { line: 2, feature: UnsupportedFeature::MultipleDocuments }));
        assert_eq!(parse_document("a: 1\n...\n"), Err(YamlError::Unsupported { line: 2, feature: UnsupportedFeature::MultipleDocuments }));
    }

    // --- 2.9 구조 오류 ---

    #[test]
    fn structural_violations_are_rejected_with_the_offending_line() {
        assert_eq!(parse_document("a:\n\tb: 1\n"), Err(YamlError::TabIndentation { line: 2 }));
        assert_eq!(parse_document("a:\n  b: 1\n c: 2\n"), Err(YamlError::BadIndentation { line: 3 }));
        assert_eq!(parse_document("a: 1\n  b: 2\n"), Err(YamlError::BadIndentation { line: 2 }));
        assert_eq!(parse_document("a: 1\na: 2\n"), Err(YamlError::DuplicateKey { line: 2, key: "a".into() }));
        assert_eq!(parse_document("- a\nb: 1\n"), Err(YamlError::MixedCollection { line: 2 }));
        assert_eq!(parse_document("a: '안닫힘\n"), Err(YamlError::UnterminatedQuote { line: 1 }));
        assert_eq!(parse_document("그냥 평문\n또 다른 줄\n"), Err(YamlError::MalformedLine { line: 1 }));
    }

    // --- 2.10 깊이 상한 ---

    fn nested_document(depth: usize) -> String {
        let mut source = String::new();
        for level in 0..depth {
            source.push_str(&"  ".repeat(level));
            source.push_str(&format!("k{level}:\n"));
        }
        source
    }

    #[test]
    fn depth_beyond_the_limit_is_rejected_and_the_limit_itself_succeeds() {
        assert!(parse_document(&nested_document(MAX_DEPTH)).is_ok());
        assert_eq!(parse_document(&nested_document(MAX_DEPTH + 1)), Err(YamlError::TooDeep { line: MAX_DEPTH + 1 }));
    }

    // --- 6.2 압축 매핑 시퀀스 항목 · 빈 문서 ---

    #[test]
    fn dash_key_value_extends_via_matching_column_and_bare_dash_takes_the_deeper_block() {
        let doc = "participants:\n  - id: p1\n    name: 고객\n  -\n    id: p2\n";
        let expected = map(vec![(
            "participants",
            YamlValue::Seq(vec![map(vec![("id", scalar("p1")), ("name", scalar("고객"))]), map(vec![("id", scalar("p2"))])]),
        )]);
        assert_eq!(parse_document(doc), Ok(expected));
    }

    #[test]
    fn blank_whitespace_comment_only_and_marker_only_documents_are_empty() {
        for source in ["", "   \n  ", "# 그냥 주석\n", "---\n"] {
            assert_eq!(parse_document(source), Err(YamlError::Empty), "{source:?}");
        }
    }

    // --- 조회 도우미 ---

    #[test]
    fn lookup_helpers_return_the_matching_variant_only() {
        let value = map(vec![("k", scalar("v"))]);
        assert_eq!(value.get("k"), Some(&scalar("v")));
        assert_eq!(value.get("missing"), None);
        assert_eq!(value.as_map(), Some(&[("k".to_string(), scalar("v"))][..]));
        assert_eq!(value.as_scalar(), None);
        assert_eq!(value.as_seq(), None);

        let seq = YamlValue::Seq(vec![scalar("a")]);
        assert_eq!(seq.as_seq(), Some(&[scalar("a")][..]));
        assert_eq!(seq.get("k"), None);

        let s = scalar("x");
        assert_eq!(s.as_scalar(), Some("x"));
        assert_eq!(s.as_map(), None);
    }
}
