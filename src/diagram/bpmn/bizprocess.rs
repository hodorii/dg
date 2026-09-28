//! `biz-process.md` 개요 리더 — 헤딩 계층·괄호 태그·`Logic(AST)` 항목을 줄 기반으로 읽는다.
//!
//! BPMN 이름(Task·Gateway·Subprocess 등)은 전혀 모른다 — 그 매핑은 `parse_bizprocess`가 한다.
//! 레벨은 `L<n>` 토큰만으로 정해진다(`#` 개수·들여쓰기·`L<n>`과 `:` 사이 종류 낱말은 전부 무시).

/// 헤딩 끝(또는 Logic 항목 끝) 괄호에서 읽은 `key: value` 태그. `participant` 이외의 키는
/// `parse_bizprocess`가 무시한다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Action(String),
    Throw(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogicItem {
    /// `condition` = `None`이면 `ELSE`. `note` = `ELSE (메모)`의 메모.
    Branch { condition: Option<String>, note: Option<String>, outcome: Outcome },
    Note(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    /// 1 이상(`L<n>`의 n). L1~L5만 이 스펙의 지원 범위 — L6 이상은 범위 밖(별도 안전장치 없음).
    pub level: u8,
    /// 태그·ID 괄호 제거, 이어지는 줄 합침 뒤의 이름.
    pub name: String,
    /// `key:value` 태그만(ID 목록은 버림).
    pub tags: Vec<Tag>,
    pub logic: Vec<LogicItem>,
    pub children: Vec<Heading>,
    /// 1부터 시작하는 원본 줄 번호(헤딩이 시작된 줄).
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    /// L1 헤딩들 — 문서 순서.
    pub processes: Vec<Heading>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutlineError {
    /// L1 헤딩이 하나도 없다.
    NoProcess,
    /// L2가 하나도 없는 L1 — `line`은 그 L1 헤딩의 줄.
    ProcessWithoutActivity { line: usize },
    /// `key: value` 목록의 한 조각에 `:`가 없다(값 안에 쉼표를 썼다는 뜻) — `line`은 그 괄호를 가진
    /// 헤딩·항목의 줄, `key`는 바로 앞에서 성공적으로 읽힌 키.
    CommaInTagValue { line: usize, key: String },
}

impl std::fmt::Display for OutlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutlineError::NoProcess => write!(f, "NoProcess"),
            OutlineError::ProcessWithoutActivity { line } => write!(f, "ProcessWithoutActivity(line {line})"),
            OutlineError::CommaInTagValue { line, key } => write!(f, "CommaInTagValue(line {line}, key {key:?})"),
        }
    }
}

fn with_line(error: OutlineError, line: usize) -> OutlineError {
    match error {
        OutlineError::CommaInTagValue { key, .. } => OutlineError::CommaInTagValue { line, key },
        other => other,
    }
}

/// 어떤 줄이든 `trim` 후 `#`+공백을 벗기면 `L1` + 공백으로 시작하고 `:`를 포함하면 참. 문서를
/// 파싱하지 않는다(스니핑 전용) — PlantUML의 포괄 판별보다 먼저 오므로 빨라야 한다.
pub fn looks_like_bizprocess(source: &str) -> bool {
    source.lines().any(|line| {
        let stripped = strip_hashes(line);
        stripped.starts_with("L1 ") && stripped.contains(':')
    })
}

/// `#`(있으면 여러 개)와 그 뒤 공백을 벗긴다. `#`가 없어도 그대로 돌려준다(널널한 스니핑).
fn strip_hashes(line: &str) -> &str {
    line.trim().trim_start_matches('#').trim_start()
}

struct HeadingLine {
    level: u8,
    rest: String,
}

/// `trim` 후 `#`+공백+`L<n>`… `:`인 줄을 읽는다. `#`가 전혀 없으면 헤딩이 아니다(게이트·헤딩 모두
/// 마크다운 헤딩이라 `#`가 필수).
fn parse_heading_line(line: &str) -> Option<HeadingLine> {
    let trimmed = line.trim();
    let without_hash = trimmed.trim_start_matches('#');
    if without_hash.len() == trimmed.len() {
        return None;
    }
    let after_hashes = without_hash.trim_start();
    let bytes = after_hashes.as_bytes();
    if bytes.first() != Some(&b'L') {
        return None;
    }
    let mut idx = 1;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == 1 {
        return None;
    }
    let level: u8 = after_hashes[1..idx].parse().ok()?;
    let after_level = &after_hashes[idx..];
    let colon = after_level.find(':')?;
    let rest = after_level[colon + 1..].trim().to_string();
    Some(HeadingLine { level, rest })
}

/// `#` 뒤 `✅` 또는 `검토 요청`으로 시작하는 게이트 줄.
fn is_gate_line(line: &str) -> bool {
    let after_hashes = strip_hashes(line);
    after_hashes.starts_with('✅') || after_hashes.starts_with("검토 요청")
}

/// 헤딩 끝(또는 텍스트 끝)의 괄호를 뒤에서부터 벗긴다: `(이름, 태그)`. ID 목록 괄호(`\d+(\.\d+)*
/// (~\d+(\.\d+)*)?`를 쉼표로 이은 것)는 버리고, `key: value` 목록은 태그로 거둔다. 조각 하나가
/// ID도 `key:`도 아니면 그 괄호부터는 이름(3.3). `line`은 알 수 없으므로 `CommaInTagValue`의
/// `line`은 항상 0 — 호출자가 실제 줄 번호로 다시 씌운다(`with_line`).
pub fn split_trailing_tags(text: &str) -> Result<(String, Vec<Tag>), OutlineError> {
    let mut name_end = text.len();
    let mut tags: Vec<Tag> = Vec::new();
    loop {
        let candidate = text[..name_end].trim_end();
        if !candidate.ends_with(')') {
            break;
        }
        let Some(open) = find_matching_open_paren(candidate) else { break };
        let inner = &candidate[open + 1..candidate.len() - 1];
        match classify_paren(inner) {
            ParenKind::Ids => {
                name_end = open;
            }
            ParenKind::Tags(mut found) => {
                found.append(&mut tags);
                tags = found;
                name_end = open;
            }
            ParenKind::CommaError(key) => return Err(OutlineError::CommaInTagValue { line: 0, key }),
            ParenKind::Name => break,
        }
    }
    Ok((text[..name_end].trim().to_string(), tags))
}

enum ParenKind {
    Ids,
    Tags(Vec<Tag>),
    CommaError(String),
    Name,
}

fn classify_paren(inner: &str) -> ParenKind {
    let pieces: Vec<&str> = inner.split(',').collect();
    if pieces.iter().all(|p| is_id_token(p)) {
        return ParenKind::Ids;
    }
    let mut tags = Vec::new();
    let mut last_key: Option<String> = None;
    for piece in &pieces {
        match piece.split_once(':') {
            Some((key, value)) => {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                last_key = Some(key.clone());
                tags.push(Tag { key, value });
            }
            None => {
                return match last_key {
                    Some(key) => ParenKind::CommaError(key),
                    None => ParenKind::Name,
                };
            }
        }
    }
    if tags.is_empty() { ParenKind::Name } else { ParenKind::Tags(tags) }
}

fn is_id_token(token: &str) -> bool {
    let token = token.trim();
    if token.is_empty() {
        return false;
    }
    match token.split_once('~') {
        Some((a, b)) => is_dotted_number(a) && is_dotted_number(b),
        None => is_dotted_number(token),
    }
}

fn is_dotted_number(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// `text`는 `)`로 끝난다고 전제. 짝이 맞는 `(`의 바이트 인덱스, 없으면 `None`(깨진 괄호는 그냥
/// 이름의 일부로 남는다).
fn find_matching_open_paren(text: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in text.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ── 헤딩 트리 빌더 ─────────────────────────────────────────────────────

struct Building {
    level: u8,
    line: usize,
    name_raw: String,
    /// (줄 번호, 원문) — 이어지는 줄까지 합친 뒤 끝에.
    logic_items_raw: Vec<(usize, String)>,
    children: Vec<Heading>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Continuation {
    None,
    Name,
    Item,
}

/// `source`를 헤딩 트리(`L1`마다 하나)로 읽는다. 줄 하나씩만 보는 명시적 스택 — 재귀 없음.
pub fn parse_outline(source: &str) -> Result<Outline, OutlineError> {
    // 인덱스 0 = 가상 루트(레벨 0, 파이널라이즈되지 않는다). 그 children이 `processes`가 된다.
    let mut stack: Vec<Building> = vec![Building { level: 0, line: 0, name_raw: String::new(), logic_items_raw: Vec::new(), children: Vec::new() }];
    let mut diagnostics = Vec::new();
    let mut continuation = Continuation::None;
    let mut skipping = false;

    for (line_no, raw_line) in source.lines().enumerate() {
        let line_no = line_no + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            continuation = Continuation::None;
            continue;
        }
        if is_gate_line(raw_line) {
            close_to_level(&mut stack, 1, &mut diagnostics)?;
            skipping = true;
            continuation = Continuation::None;
            continue;
        }
        if skipping {
            match parse_heading_line(raw_line) {
                Some(h) if h.level == 1 => skipping = false,
                _ => continue,
            }
        }
        if let Some(h) = parse_heading_line(raw_line) {
            close_to_level(&mut stack, h.level, &mut diagnostics)?;
            stack.push(Building { level: h.level, line: line_no, name_raw: h.rest, logic_items_raw: Vec::new(), children: Vec::new() });
            continuation = Continuation::Name;
            continue;
        }
        if trimmed.starts_with("Logic(AST):") {
            continuation = Continuation::None;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(top) = stack.last_mut() {
                top.logic_items_raw.push((line_no, rest.to_string()));
                continuation = Continuation::Item;
            } else {
                continuation = Continuation::None;
            }
            continue;
        }
        if trimmed.starts_with('#') {
            // 인식하지 못한 마크다운 헤딩(`## 정의`·`## 가치사슬 매핑` 등) — 구조 밖.
            continuation = Continuation::None;
            continue;
        }
        match continuation {
            Continuation::Name => {
                if let Some(top) = stack.last_mut() {
                    top.name_raw.push(' ');
                    top.name_raw.push_str(trimmed);
                }
            }
            Continuation::Item => {
                if let Some(top) = stack.last_mut()
                    && let Some((_, text)) = top.logic_items_raw.last_mut()
                {
                    text.push(' ');
                    text.push_str(trimmed);
                }
            }
            Continuation::None => {}
        }
    }

    close_to_level(&mut stack, 0, &mut diagnostics)?;
    let processes = stack.into_iter().next().expect("루트는 항상 남는다").children;

    if processes.is_empty() {
        return Err(OutlineError::NoProcess);
    }
    if let Some(orphan) = processes.iter().find(|p| !p.children.iter().any(|c| c.level == 2)) {
        return Err(OutlineError::ProcessWithoutActivity { line: orphan.line });
    }

    Ok(Outline { processes, diagnostics })
}

/// 스택 맨 위부터 레벨이 `level` 이상인 것을 전부 닫아(파이널라이즈해) 부모의 `children`에 붙인다.
/// `level == 0`은 EOF에서 전부(가상 루트 제외) 닫는다는 뜻.
fn close_to_level(stack: &mut Vec<Building>, level: u8, diagnostics: &mut Vec<Diagnostic>) -> Result<(), OutlineError> {
    loop {
        let should_close = match stack.last() {
            Some(top) if level == 0 => top.level > 0,
            Some(top) => top.level >= level,
            None => false,
        };
        if !should_close {
            break;
        }
        let building = stack.pop().expect("should_close가 참이면 top이 있다");
        let heading = finalize(building, diagnostics)?;
        stack.last_mut().expect("가상 루트는 pop되지 않는다").children.push(heading);
    }
    Ok(())
}

fn finalize(building: Building, diagnostics: &mut Vec<Diagnostic>) -> Result<Heading, OutlineError> {
    let (name, tags) = split_trailing_tags(&building.name_raw).map_err(|e| with_line(e, building.line))?;
    let mut logic = Vec::with_capacity(building.logic_items_raw.len());
    for (line, raw) in &building.logic_items_raw {
        let (cleaned, _tags) = split_trailing_tags(raw).map_err(|e| with_line(e, *line))?;
        logic.push(parse_logic_item(&cleaned, *line, diagnostics));
    }
    Ok(Heading { level: building.level, name, tags, logic, children: building.children, line: building.line })
}

fn parse_logic_item(text: &str, line: usize, diagnostics: &mut Vec<Diagnostic>) -> LogicItem {
    if let Some(rest) = text.strip_prefix("ELSE IF ") {
        return parse_if(&format!("IF {rest}"), text, line, diagnostics);
    }
    if text.starts_with("IF ") {
        return parse_if(text, text, line, diagnostics);
    }
    if let Some(rest) = text.strip_prefix("ELSE ") {
        let (note, result) = if let Some(paren_rest) = rest.strip_prefix('(') {
            match paren_rest.find(')') {
                Some(close) => {
                    let memo = paren_rest[..close].trim().to_string();
                    let after = paren_rest[close + 1..].trim();
                    match after.strip_prefix("THEN ") {
                        Some(result) => (Some(memo), result.trim()),
                        None => (None, rest),
                    }
                }
                None => (None, rest),
            }
        } else {
            (None, rest)
        };
        return LogicItem::Branch { condition: None, note, outcome: outcome_of(result) };
    }
    LogicItem::Note(text.to_string())
}

/// `original`은 `text.starts_with("IF ")` 형태(`ELSE IF`는 이미 `IF`로 바꿔 들어온다).
/// `display`는 `THEN`이 없을 때 `Note`로 되돌릴 원문(정규화 전 텍스트, `ELSE IF` 그대로).
fn parse_if(normalized: &str, display: &str, line: usize, diagnostics: &mut Vec<Diagnostic>) -> LogicItem {
    let after_if = &normalized["IF ".len()..];
    match after_if.find(" THEN ") {
        Some(pos) => {
            let condition = after_if[..pos].trim().to_string();
            let result = after_if[pos + " THEN ".len()..].trim();
            LogicItem::Branch { condition: Some(condition), note: None, outcome: outcome_of(result) }
        }
        None => {
            diagnostics.push(Diagnostic { line, message: format!("`IF`에 `THEN`이 없어 주석으로 취급합니다: {display}") });
            LogicItem::Note(display.to_string())
        }
    }
}

fn outcome_of(result: &str) -> Outcome {
    match result.strip_prefix("THROW ") {
        Some(error) => Outcome::Throw(error.trim().to_string()),
        None => Outcome::Action(result.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── looks_like_bizprocess(1.2, 1.4) ──────────────────────────────

    #[test]
    fn recognizes_every_real_biz_process_document() {
        for (name, source) in super::super::fixtures::BIZPROCESS_REAL {
            assert!(looks_like_bizprocess(source), "{name}은 bizprocess로 스니핑돼야 한다");
        }
    }

    #[test]
    fn does_not_recognize_other_diagram_sources_or_plain_text() {
        let negatives = [
            "flowchart TB\n A --> B",
            "@startuml\nA -> B\n@enduml",
            "<definitions><process id=\"p1\"/></definitions>",
            "participants:\n  - p1:\n      name: 참여자\n",
            "임의의 한글 문장입니다",
            "## 정의\n이 문서는 ...",
        ];
        for source in negatives {
            assert!(!looks_like_bizprocess(source), "{source:?}는 bizprocess가 아니어야 한다");
        }
    }

    // ── split_trailing_tags(3.1~3.6) ─────────────────────────────────

    #[test]
    fn strips_id_list_parens_including_ranges() {
        assert_eq!(split_trailing_tags("이름 (1.1, 2.3)").unwrap(), ("이름".to_string(), Vec::new()));
        assert_eq!(split_trailing_tags("이름 (1.1~1.5, 2.1)").unwrap(), ("이름".to_string(), Vec::new()));
    }

    #[test]
    fn reads_key_value_parens_as_tags() {
        let (name, tags) = split_trailing_tags("A (valueChainRef: VC-X)").unwrap();
        assert_eq!(name, "A");
        assert_eq!(tags, vec![Tag { key: "valueChainRef".into(), value: "VC-X".into() }]);

        let (name, tags) = split_trailing_tags("B (participant: 학습자)").unwrap();
        assert_eq!(name, "B");
        assert_eq!(tags, vec![Tag { key: "participant".into(), value: "학습자".into() }]);
    }

    #[test]
    fn keeps_non_tag_parens_as_part_of_the_name() {
        assert_eq!(split_trailing_tags("감시 루프(백그라운드)").unwrap(), ("감시 루프(백그라운드)".to_string(), Vec::new()));
        assert_eq!(split_trailing_tags("(표준입력이거나 파일 미지정)").unwrap(), ("(표준입력이거나 파일 미지정)".to_string(), Vec::new()));
    }

    #[test]
    fn multiple_trailing_parens_are_parsed_from_the_right_until_one_is_not_a_tag() {
        let (name, tags) = split_trailing_tags("멘토 지정 (participant: 멘토) (2.1, 2.3)").unwrap();
        assert_eq!(name, "멘토 지정");
        assert_eq!(tags, vec![Tag { key: "participant".into(), value: "멘토".into() }]);

        // 오른쪽 ID 괄호는 태그로 소비되지만, 그다음(왼쪽) 괄호는 태그도 ID도 아니므로 거기서
        // 멈추고 이름에 그대로 남는다.
        let (name, tags) = split_trailing_tags("감시 루프 (백그라운드) (2.1)").unwrap();
        assert_eq!(name, "감시 루프 (백그라운드)");
        assert!(tags.is_empty());
    }

    #[test]
    fn comma_inside_a_tag_value_is_an_error() {
        let err = split_trailing_tags("A (participant: 학습자, 멘토)").unwrap_err();
        assert_eq!(err, OutlineError::CommaInTagValue { line: 0, key: "participant".into() });
    }

    #[test]
    fn tag_values_are_trimmed_but_case_and_inner_spacing_are_preserved() {
        let (_, tags) = split_trailing_tags("A (participant:  학습자 )").unwrap();
        assert_eq!(tags, vec![Tag { key: "participant".into(), value: "학습자".into() }]);

        let (_, tags1) = split_trailing_tags("A (participant: 시스템운영자)").unwrap();
        let (_, tags2) = split_trailing_tags("B (participant: 시스템 운영자)").unwrap();
        assert_ne!(tags1[0].value, tags2[0].value);
    }

    // ── parse_outline: 헤딩 트리(2.1~2.7) ────────────────────────────

    #[test]
    fn level_comes_only_from_the_l_token_regardless_of_hash_count_or_indentation() {
        let source = "## L1 Process: A\n### L2 Activity: B\n  ### L3 FunctionGroup/UI: C\n    ### L4 Step: D\n      ### L5 DetailStep: E\n";
        let outline = parse_outline(source).unwrap();
        assert_eq!(outline.processes.len(), 1);
        let p = &outline.processes[0];
        assert_eq!(p.level, 1);
        assert_eq!(p.name, "A");
        let l2 = &p.children[0];
        assert_eq!(l2.level, 2);
        let l3 = &l2.children[0];
        assert_eq!(l3.level, 3);
        let l4 = &l3.children[0];
        assert_eq!(l4.level, 4);
        let l5 = &l4.children[0];
        assert_eq!(l5.level, 5);
        assert_eq!(l5.name, "E");
    }

    #[test]
    fn a_two_line_heading_is_joined_into_one_name() {
        let source = "## L1 Process: A\n### L2 Activity: 아주 긴 이름이\n어어서 다음 줄로 이어짐\n";
        let outline = parse_outline(source).unwrap();
        assert_eq!(outline.processes[0].children[0].name, "아주 긴 이름이 어어서 다음 줄로 이어짐");
    }

    #[test]
    fn gate_lines_definitions_and_value_chain_tables_are_not_structure() {
        let source = "## L1 Process: A\n### L2 Activity: B\n\n### ✅ 검토 요청 (L1: A)\n승인(✓) 또는 수정 사항을 입력하세요.\n\n## 정의\n이 문서는 ...\n\n## 가치사슬 매핑\n| a | b |\n|---|---|\n";
        let outline = parse_outline(source).unwrap();
        assert_eq!(outline.processes.len(), 1);
        assert_eq!(outline.processes[0].children.len(), 1);
    }

    #[test]
    fn logic_items_join_continuations_and_attach_to_the_preceding_heading() {
        let source = "## L1 Process: A\n### L2 Activity: B\nLogic(AST):\n- IF 조건이 아주 길어서\n  다음 줄로 이어지면 THEN 결과\n";
        let outline = parse_outline(source).unwrap();
        let logic = &outline.processes[0].children[0].logic;
        assert_eq!(logic.len(), 1);
        assert_eq!(logic[0], LogicItem::Branch { condition: Some("조건이 아주 길어서 다음 줄로 이어지면".into()), note: None, outcome: Outcome::Action("결과".into()) });
    }

    #[test]
    fn two_l1_headings_become_two_processes_in_document_order() {
        let source = "## L1 Process: A\n### L2 Activity: B\n## L1 Process: C\n### L2 Activity: D\n";
        let outline = parse_outline(source).unwrap();
        assert_eq!(outline.processes.len(), 2);
        assert_eq!(outline.processes[0].name, "A");
        assert_eq!(outline.processes[1].name, "C");
    }

    #[test]
    fn no_l1_heading_is_an_error() {
        assert_eq!(parse_outline("그냥 평문입니다\n"), Err(OutlineError::NoProcess));
        assert_eq!(parse_outline(""), Err(OutlineError::NoProcess));
    }

    #[test]
    fn an_l1_without_any_l2_is_an_error() {
        let err = parse_outline("## L1 Process: A\n본문만 있음\n").unwrap_err();
        assert_eq!(err, OutlineError::ProcessWithoutActivity { line: 1 });
    }

    #[test]
    fn l4_directly_under_l2_and_an_empty_l2_do_not_panic() {
        let source = "## L1 Process: A\n### L2 Activity: B\n    ### L4 Step: D\n### L2 Activity: 빈 L2\n";
        let outline = parse_outline(source).unwrap();
        assert_eq!(outline.processes[0].children.len(), 2);
        let b = &outline.processes[0].children[0];
        assert_eq!(b.children.len(), 1);
        assert_eq!(b.children[0].level, 4);
        assert!(outline.processes[0].children[1].children.is_empty());
    }

    // ── Logic(AST) 항목 다섯 꼴(2.4, 5.7) ─────────────────────────────

    fn single_item(text: &str) -> LogicItem {
        let source = format!("## L1 Process: A\n### L2 Activity: B\nLogic(AST):\n- {text}\n");
        parse_outline(&source).unwrap().processes[0].children[0].logic[0].clone()
    }

    #[test]
    fn if_then_becomes_a_branch_with_a_condition() {
        assert_eq!(single_item("IF 조건 THEN 결과"), LogicItem::Branch { condition: Some("조건".into()), note: None, outcome: Outcome::Action("결과".into()) });
    }

    #[test]
    fn else_if_then_is_treated_like_if_then() {
        assert_eq!(single_item("ELSE IF 조건2 THEN 결과2"), LogicItem::Branch { condition: Some("조건2".into()), note: None, outcome: Outcome::Action("결과2".into()) });
    }

    #[test]
    fn plain_else_is_a_default_branch_without_a_note() {
        assert_eq!(single_item("ELSE 결과"), LogicItem::Branch { condition: None, note: None, outcome: Outcome::Action("결과".into()) });
    }

    #[test]
    fn else_with_a_memo_carries_the_memo_as_the_note() {
        assert_eq!(single_item("ELSE (표준입력이거나 파일 미지정) THEN 결과"), LogicItem::Branch { condition: None, note: Some("표준입력이거나 파일 미지정".into()), outcome: Outcome::Action("결과".into()) });
    }

    #[test]
    fn throw_outcomes_are_recognized_in_if_and_else() {
        assert_eq!(single_item("IF 조건 THEN THROW 오류"), LogicItem::Branch { condition: Some("조건".into()), note: None, outcome: Outcome::Throw("오류".into()) });
        assert_eq!(single_item("ELSE THROW 오류"), LogicItem::Branch { condition: None, note: None, outcome: Outcome::Throw("오류".into()) });
    }

    #[test]
    fn plain_prose_and_always_prefixed_items_become_notes() {
        assert_eq!(single_item("항상: 실행됨"), LogicItem::Note("항상: 실행됨".into()));
        assert_eq!(single_item("기존 테스트가 전량 통과함"), LogicItem::Note("기존 테스트가 전량 통과함".into()));
    }

    #[test]
    fn if_without_then_becomes_a_note_with_a_diagnostic() {
        let source = "## L1 Process: A\n### L2 Activity: B\nLogic(AST):\n- IF 조건만 있음\n";
        let outline = parse_outline(source).unwrap();
        let logic = &outline.processes[0].children[0].logic;
        assert_eq!(logic[0], LogicItem::Note("IF 조건만 있음".into()));
        assert_eq!(outline.diagnostics.len(), 1);
        assert_eq!(outline.diagnostics[0].line, 4);
    }

    #[test]
    fn a_trailing_id_paren_is_removed_from_logic_item_text() {
        assert_eq!(single_item("IF 조건 THEN 결과 (6.1)"), LogicItem::Branch { condition: Some("조건".into()), note: None, outcome: Outcome::Action("결과".into()) });
    }

    // ── 손상 입력(7.3) ─────────────────────────────────────────────────

    #[test]
    fn malformed_inputs_do_not_panic() {
        let inputs = [
            "",
            "   \n  ",
            "그냥 평문입니다",
            "## L1 Process: A만 있고 L2 없음",
            "## L1 Process: A\n### L2",
            "## L1 Process: A\n### L2 Activity: B\nLogic(AST):\n",
            "- IF 항목만 있고 헤딩 없음\n",
        ];
        for source in inputs {
            let _ = parse_outline(source);
        }
    }
}

#[cfg(test)]
mod sanity_real_docs {
    use super::*;
    #[test]
    fn all_real_docs_parse_without_error_and_report_l2_counts() {
        for (name, source) in super::super::fixtures::BIZPROCESS_REAL {
            match parse_outline(source) {
                Ok(outline) => {
                    let l2 = outline.processes[0].children.len();
                    eprintln!("{name}: L1={} L2={} diagnostics={}", outline.processes.len(), l2, outline.diagnostics.len());
                }
                Err(e) => panic!("{name} failed to parse: {e}"),
            }
        }
    }
}
