//! 문자 단위 XML 토크나이저·트리 빌더.
//!
//! BPMN 문서에 충분한 최소 XML 하위집합을 요소 트리로 읽는다. 이름·속성·텍스트·주석·CDATA·
//! `<?…?>`·`<!DOCTYPE`·엔티티·요소 접두사·불투명 구획·깊이 상한을 다루되, BPMN 로컬 이름은
//! 하나도 모른다(불투명 구획 이름은 호출자가 준다) — 재사용 가능한 범용 XML 표면 문법 계층이다.

use std::fmt;

/// 요소 하나. 이름은 접두사를 벗긴 로컬 이름, 속성 이름은 쓴 그대로(`xmlns:bpmn`·`xsi:type` 포함).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    /// (이름, 디코딩된 값). 중복 이름은 첫 것만.
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlElement>,
    /// 직접 텍스트 + CDATA를 문서 순서로 이어붙여 양끝 공백을 제거한 것(자식 요소의 텍스트는 제외).
    pub text: String,
}

impl XmlElement {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElement> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// 이름이 일치하는 첫 자식의 `text`.
    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.children.iter().find(|c| c.name == name).map(|c| c.text.as_str())
    }
}

/// 첫 시작 태그(프롤로그·주석·DOCTYPE 뒤). 속성 규칙은 `XmlElement`와 같다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartTag {
    pub name: String,
    pub attributes: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlError {
    /// 요소가 하나도 없음(빈 문자열·공백·`<` 하나).
    NoRootElement,
    /// 태그·따옴표·주석·CDATA·선언이 열린 채 끝남; 요소가 안 닫힘.
    UnexpectedEnd,
    /// 바이트 오프셋: 이름 없는 태그, 값 없는 속성, `<` 뒤 이상 문자.
    MalformedTag(usize),
    MismatchedClosingTag { expected: String, found: String, offset: usize },
    /// 중첩 깊이(64) 초과 지점.
    TooDeep(usize),
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlError::NoRootElement => write!(f, "요소가 하나도 없다"),
            XmlError::UnexpectedEnd => write!(f, "문서가 열린 채로 끝났다"),
            XmlError::MalformedTag(offset) => write!(f, "{offset}번째 바이트 부근의 태그 형식이 잘못됐다"),
            XmlError::MismatchedClosingTag { expected, found, offset } => write!(f, "{offset}번째 바이트에서 닫는 태그가 맞지 않다(기대 {expected:?}, 실제 {found:?})"),
            XmlError::TooDeep(offset) => write!(f, "{offset}번째 바이트에서 중첩 깊이 상한({MAX_DEPTH})을 넘었다"),
        }
    }
}

pub const MAX_DEPTH: usize = 64;

/// 프롤로그(`<?…?>`·주석·`<!DOCTYPE`)를 건너뛴 첫 시작 태그. 없거나 태그가 망가졌으면 `None`.
/// 문서 전체를 파싱하지 않는다.
pub fn first_start_tag(source: &str) -> Option<StartTag> {
    let mut pos = 0usize;
    loop {
        if pos >= source.len() {
            return None;
        }
        let (token, next_pos) = next_token(source, pos).ok()?;
        pos = next_pos;
        match token {
            Token::Comment | Token::Pi | Token::Doctype | Token::Text(_) | Token::CData(_) => continue,
            Token::StartTag { local_name, attributes, .. } => return Some(StartTag { name: local_name, attributes }),
            Token::EndTag { .. } => return None,
        }
    }
}

/// 루트 요소 트리. `opaque`에 든 로컬 이름의 요소는 시작·끝 태그 균형만 세어 건너뛰고 트리에
/// 넣지 않는다(주석·CDATA는 그 안에서도 정상 인식). 루트가 닫힌 뒤 내용은 무시한다.
pub fn parse_document(source: &str, opaque: &[&str]) -> Result<XmlElement, XmlError> {
    struct Frame {
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<XmlElement>,
        text: String,
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut pos = 0usize;
    loop {
        if pos >= source.len() {
            return Err(if stack.is_empty() { XmlError::NoRootElement } else { XmlError::UnexpectedEnd });
        }
        let (token, next_pos) = next_token(source, pos)?;
        pos = next_pos;
        match token {
            Token::Comment | Token::Pi | Token::Doctype => {}
            Token::Text(text) => {
                if let Some(frame) = stack.last_mut() {
                    frame.text.push_str(&text);
                }
            }
            Token::CData(text) => {
                if let Some(frame) = stack.last_mut() {
                    frame.text.push_str(&text);
                }
            }
            Token::StartTag { local_name, attributes, self_closing, offset } => {
                if !self_closing && opaque.contains(&local_name.as_str()) {
                    pos = skip_opaque_element(source, pos, &local_name)?;
                    continue;
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(XmlError::TooDeep(offset));
                }
                if self_closing {
                    let elem = XmlElement { name: local_name, attributes, children: Vec::new(), text: String::new() };
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(elem),
                        None => return Ok(elem),
                    }
                } else {
                    stack.push(Frame { name: local_name, attributes, children: Vec::new(), text: String::new() });
                }
            }
            Token::EndTag { local_name, offset } => {
                let frame = match stack.pop() {
                    Some(frame) => frame,
                    None => return Err(XmlError::MismatchedClosingTag { expected: String::new(), found: local_name, offset }),
                };
                if frame.name != local_name {
                    return Err(XmlError::MismatchedClosingTag { expected: frame.name, found: local_name, offset });
                }
                let elem = XmlElement { name: frame.name, attributes: frame.attributes, children: frame.children, text: frame.text.trim().to_string() };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(elem),
                    None => return Ok(elem),
                }
            }
        }
    }
}

/// `local_name` 요소 하나(비어 있지 않은 시작 태그가 이미 소비된 뒤)의 내용을 시작·끝 태그
/// 균형만 세어 건너뛴다. 자식 트리를 만들지 않는다.
fn skip_opaque_element(source: &str, mut pos: usize, local_name: &str) -> Result<usize, XmlError> {
    let mut depth = 1i32;
    loop {
        if pos >= source.len() {
            return Err(XmlError::UnexpectedEnd);
        }
        let (token, next_pos) = next_token(source, pos)?;
        pos = next_pos;
        match token {
            Token::StartTag { local_name: name, self_closing, .. } => {
                if !self_closing && name == local_name {
                    depth += 1;
                }
            }
            Token::EndTag { local_name: name, .. } if name == local_name => {
                depth -= 1;
                if depth == 0 {
                    return Ok(pos);
                }
            }
            _ => {}
        }
    }
}

// --- 저수준 토큰 ---

enum Token {
    Comment,
    Pi,
    Doctype,
    Text(String),
    CData(String),
    StartTag { local_name: String, attributes: Vec<(String, String)>, self_closing: bool, offset: usize },
    EndTag { local_name: String, offset: usize },
}

fn char_at(source: &str, pos: usize) -> Option<char> {
    source.get(pos..)?.chars().next()
}

fn find_from(source: &str, pos: usize, needle: &str) -> Option<usize> {
    source.get(pos..)?.find(needle).map(|i| pos + i)
}

fn starts_with_at(source: &str, pos: usize, needle: &str) -> bool {
    source.get(pos..).is_some_and(|s| s.starts_with(needle))
}

fn next_token(source: &str, pos: usize) -> Result<(Token, usize), XmlError> {
    if !starts_with_at(source, pos, "<") {
        let end = find_from(source, pos, "<").unwrap_or(source.len());
        let raw = &source[pos..end];
        return Ok((Token::Text(decode_entities(raw)), end));
    }
    if starts_with_at(source, pos, "<!--") {
        let close = find_from(source, pos + 4, "-->").ok_or(XmlError::UnexpectedEnd)?;
        return Ok((Token::Comment, close + 3));
    }
    if starts_with_at(source, pos, "<![CDATA[") {
        let close = find_from(source, pos + 9, "]]>").ok_or(XmlError::UnexpectedEnd)?;
        let content = source[pos + 9..close].to_string();
        return Ok((Token::CData(content), close + 3));
    }
    if starts_with_at(source, pos, "<?") {
        let close = find_from(source, pos + 2, "?>").ok_or(XmlError::UnexpectedEnd)?;
        return Ok((Token::Pi, close + 2));
    }
    if source.len() >= pos + 9 && source.as_bytes()[pos + 1..pos + 9].eq_ignore_ascii_case(b"!DOCTYPE") {
        let close = scan_doctype(source, pos)?;
        return Ok((Token::Doctype, close));
    }
    if starts_with_at(source, pos, "</") {
        let (local_name, end) = parse_end_tag(source, pos)?;
        return Ok((Token::EndTag { local_name, offset: pos }, end));
    }
    // 알 수 없는 `<!...>` 선언(예: 내부 서브셋 없는 다른 마크업 선언) — 관대하게 다음 `>`까지 건너뜀.
    if starts_with_at(source, pos, "<!") {
        let close = find_from(source, pos, ">").ok_or(XmlError::UnexpectedEnd)?;
        return Ok((Token::Doctype, close + 1));
    }
    let (local_name, attributes, self_closing, end) = parse_start_tag(source, pos)?;
    Ok((Token::StartTag { local_name, attributes, self_closing, offset: pos }, end))
}

fn scan_doctype(source: &str, pos: usize) -> Result<usize, XmlError> {
    let mut depth = 0i32;
    let mut i = pos + 9; // "<!DOCTYPE" 뒤
    loop {
        let ch = char_at(source, i).ok_or(XmlError::UnexpectedEnd)?;
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            '>' if depth <= 0 => return Ok(i + 1),
            _ => {}
        }
        i += ch.len_utf8();
    }
}

fn parse_end_tag(source: &str, pos: usize) -> Result<(String, usize), XmlError> {
    let name_start = pos + 2; // "</" 뒤
    let mut i = name_start;
    loop {
        match char_at(source, i) {
            None => return Err(XmlError::UnexpectedEnd),
            Some(ch) if ch.is_whitespace() || ch == '>' => break,
            Some(ch) => i += ch.len_utf8(),
        }
    }
    if i == name_start {
        return Err(XmlError::MalformedTag(pos));
    }
    let raw_name = &source[name_start..i];
    loop {
        match char_at(source, i) {
            Some(ch) if ch.is_whitespace() => i += ch.len_utf8(),
            _ => break,
        }
    }
    match char_at(source, i) {
        Some('>') => Ok((local_name(raw_name), i + 1)),
        Some(_) => Err(XmlError::MalformedTag(pos)),
        None => Err(XmlError::UnexpectedEnd),
    }
}

/// (로컬 이름, 속성, 자기닫힘 여부, 태그 다음 바이트 위치).
type StartTagParts = (String, Vec<(String, String)>, bool, usize);

fn parse_start_tag(source: &str, pos: usize) -> Result<StartTagParts, XmlError> {
    let name_start = pos + 1; // '<' 뒤
    let mut i = name_start;
    loop {
        match char_at(source, i) {
            None => return Err(XmlError::UnexpectedEnd),
            Some(ch) if ch.is_whitespace() || ch == '>' || ch == '/' => break,
            Some(ch) => i += ch.len_utf8(),
        }
    }
    if i == name_start {
        return Err(XmlError::MalformedTag(pos));
    }
    let raw_name = source[name_start..i].to_string();
    let mut attributes: Vec<(String, String)> = Vec::new();
    loop {
        loop {
            match char_at(source, i) {
                Some(ch) if ch.is_whitespace() => i += ch.len_utf8(),
                _ => break,
            }
        }
        match char_at(source, i) {
            None => return Err(XmlError::UnexpectedEnd),
            Some('/') => {
                if char_at(source, i + 1) == Some('>') {
                    return Ok((local_name(&raw_name), attributes, true, i + 2));
                }
                return Err(XmlError::MalformedTag(pos));
            }
            Some('>') => return Ok((local_name(&raw_name), attributes, false, i + 1)),
            Some(_) => {
                let attr_name_start = i;
                loop {
                    match char_at(source, i) {
                        None => return Err(XmlError::UnexpectedEnd),
                        Some(ch) if ch.is_whitespace() || ch == '=' || ch == '>' || ch == '/' => break,
                        Some(ch) => i += ch.len_utf8(),
                    }
                }
                if i == attr_name_start {
                    return Err(XmlError::MalformedTag(pos));
                }
                let attr_name = source[attr_name_start..i].to_string();
                loop {
                    match char_at(source, i) {
                        Some(ch) if ch.is_whitespace() => i += ch.len_utf8(),
                        _ => break,
                    }
                }
                match char_at(source, i) {
                    Some('=') => i += 1,
                    _ => return Err(XmlError::MalformedTag(pos)),
                }
                loop {
                    match char_at(source, i) {
                        Some(ch) if ch.is_whitespace() => i += ch.len_utf8(),
                        _ => break,
                    }
                }
                let quote = match char_at(source, i) {
                    Some(q @ ('"' | '\'')) => q,
                    Some(_) => return Err(XmlError::MalformedTag(pos)),
                    None => return Err(XmlError::UnexpectedEnd),
                };
                i += 1;
                let value_start = i;
                let close = find_from(source, i, &quote.to_string()).ok_or(XmlError::UnexpectedEnd)?;
                let raw_value = &source[value_start..close];
                let decoded_value = decode_entities(raw_value);
                i = close + 1;
                if !attributes.iter().any(|(n, _)| n == &attr_name) {
                    attributes.push((attr_name, decoded_value));
                }
            }
        }
    }
}

/// 첫 `:`까지 접두사를 벗긴다(`bpmn:process → process`). 접두사 없으면 그대로.
fn local_name(raw: &str) -> String {
    match raw.find(':') {
        Some(i) => raw[i + 1..].to_string(),
        None => raw.to_string(),
    }
}

/// `&lt; &gt; &amp; &quot; &apos;`와 `&#N;`·`&#xH;`만 디코딩한다. 그 외 이름(`&nbsp;`)과 잘못된
/// 참조(`&#;`·`&#xD800;`·범위 초과)는 원문 그대로 남긴다.
fn decode_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < raw.len() {
        let ch = raw[i..].chars().next().expect("i < len이면 문자가 있다");
        if ch != '&' {
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if let Some(semi_rel) = raw[i + 1..].find(';') {
            let entity = &raw[i + 1..i + 1 + semi_rel];
            if let Some(decoded) = decode_one_entity(entity) {
                out.push(decoded);
                i = i + 1 + semi_rel + 1;
                continue;
            }
        }
        out.push('&');
        i += 1;
    }
    out
}

fn decode_one_entity(entity: &str) -> Option<char> {
    match entity {
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "amp" => return Some('&'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        _ => {}
    }
    if let Some(hex) = entity.strip_prefix('#').and_then(|s| s.strip_prefix(['x', 'X'])) {
        let code = u32::from_str_radix(hex, 16).ok()?;
        return char::from_u32(code);
    }
    if let Some(dec) = entity.strip_prefix('#') {
        let code: u32 = dec.parse().ok()?;
        return char::from_u32(code);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_document_with_no_prefix_or_any_of_three_prefixes_yields_the_same_tree() {
        let bare = parse_document(r#"<definitions><process id="p1"><startEvent id="s"/></process></definitions>"#, &[]).unwrap();
        let bpmn = parse_document(r#"<bpmn:definitions xmlns:bpmn="x"><bpmn:process id="p1"><bpmn:startEvent id="s"/></bpmn:process></bpmn:definitions>"#, &[]).unwrap();
        let bpmn2 = parse_document(r#"<bpmn2:definitions xmlns:bpmn2="x"><bpmn2:process id="p1"><bpmn2:startEvent id="s"/></bpmn2:process></bpmn2:definitions>"#, &[]).unwrap();
        let semantic = parse_document(r#"<semantic:definitions xmlns:semantic="x"><semantic:process id="p1"><semantic:startEvent id="s"/></semantic:process></semantic:definitions>"#, &[]).unwrap();
        // 스니핑에 필요한 `xmlns:*` 속성은 원본 접두사가 남아 있어야 하므로 attributes는 비교 대상에서 뺀다.
        let shape = |e: &XmlElement| (e.name.clone(), e.children.iter().map(|c| (c.name.clone(), c.attribute("id").map(str::to_string))).collect::<Vec<_>>());
        assert_eq!(shape(&bare), shape(&bpmn));
        assert_eq!(shape(&bare), shape(&bpmn2));
        assert_eq!(shape(&bare), shape(&semantic));
    }

    #[test]
    fn single_and_double_quoted_attribute_entities_and_char_refs_decode() {
        let doc = parse_document(
            r#"<a x='&amp;&lt;&gt;&quot;&apos;&#10;&#x2014;' y="&amp;&lt;&gt;&quot;&apos;&#10;&#x2014;"/>"#,
            &[],
        )
        .unwrap();
        let expected = "&<>\"'\n—";
        assert_eq!(doc.attribute("x"), Some(expected));
        assert_eq!(doc.attribute("y"), Some(expected));
    }

    #[test]
    fn declaration_doctype_comment_and_inter_element_whitespace_do_not_change_the_tree() {
        let compact = parse_document(r#"<a><b id="1"/><c id="2"/></a>"#, &[]).unwrap();
        let spaced = parse_document(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE a>\n<!-- 주석 <안에> 꺾쇠 -->\n<a>\n  <b id=\"1\"/>\n  <c id=\"2\"/>\n</a>\n",
            &[],
        )
        .unwrap();
        assert_eq!(compact, spaced);
    }

    #[test]
    fn cdata_keeps_angle_brackets_and_ampersands_literal() {
        let doc = parse_document(r#"<text><![CDATA[a < b && &amp; stays]]></text>"#, &[]).unwrap();
        assert_eq!(doc.text, "a < b && &amp; stays");
    }

    #[test]
    fn opaque_element_is_skipped_and_survives_inner_comments_cdata_and_same_name_nesting() {
        let doc = parse_document(
            r#"<definitions>
                <process id="p1"/>
                <bpmndi:BPMNDiagram id="d1">
                    <!-- fake </bpmndi:BPMNDiagram> inside a comment -->
                    <![CDATA[ another </bpmndi:BPMNDiagram> fake ]]>
                    <bpmndi:BPMNDiagram id="nested"></bpmndi:BPMNDiagram>
                </bpmndi:BPMNDiagram>
                <extra id="e1"/>
            </definitions>"#,
            &["BPMNDiagram"],
        )
        .unwrap();
        let names: Vec<&str> = doc.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["process", "extra"]);
    }

    #[test]
    fn undefined_entity_and_invalid_character_references_are_kept_verbatim() {
        let doc = parse_document(r#"<a name="&nbsp; &#; &#xD800; &#99999999999;"/>"#, &[]).unwrap();
        assert_eq!(doc.attribute("name"), Some("&nbsp; &#; &#xD800; &#99999999999;"));
    }

    #[test]
    fn self_closing_and_open_close_tags_are_equivalent() {
        let self_closed = parse_document(r#"<startEvent id="s"/>"#, &[]).unwrap();
        let open_close = parse_document(r#"<startEvent id="s"></startEvent>"#, &[]).unwrap();
        assert_eq!(self_closed, open_close);
    }

    #[test]
    fn lookup_helpers_return_the_first_match() {
        let doc = parse_document(r#"<a x="1" x="2"><b>first</b><b>second</b></a>"#, &[]).unwrap();
        assert_eq!(doc.attribute("x"), Some("1"));
        assert_eq!(doc.child_text("b"), Some("first"));
        assert_eq!(doc.children_named("b").count(), 2);
    }

    #[test]
    fn truncated_mid_tag_yields_unexpected_end() {
        assert_eq!(parse_document("<definitions><process", &[]), Err(XmlError::UnexpectedEnd));
    }

    #[test]
    fn element_left_open_yields_unexpected_end() {
        let err = parse_document("<definitions><process></definitions>", &[]).unwrap_err();
        assert!(matches!(err, XmlError::MismatchedClosingTag { ref expected, ref found, .. } if expected == "process" && found == "definitions"));
        assert_eq!(parse_document("<definitions><process>", &[]), Err(XmlError::UnexpectedEnd));
    }

    #[test]
    fn mismatched_closing_tag_names_the_expected_and_found_names() {
        let err = parse_document("<a></b>", &[]).unwrap_err();
        assert_eq!(err, XmlError::MismatchedClosingTag { expected: "a".into(), found: "b".into(), offset: 3 });
    }

    #[test]
    fn unterminated_quote_comment_cdata_and_declaration_are_unexpected_end() {
        assert_eq!(parse_document(r#"<a x="unterminated>"#, &[]), Err(XmlError::UnexpectedEnd));
        assert_eq!(parse_document("<!-- unterminated <a/>", &[]), Err(XmlError::UnexpectedEnd));
        assert_eq!(parse_document("<a><![CDATA[unterminated</a>", &[]), Err(XmlError::UnexpectedEnd));
        assert_eq!(parse_document("<?xml unterminated", &[]), Err(XmlError::UnexpectedEnd));
    }

    #[test]
    fn nesting_depth_65_fails_and_64_succeeds() {
        let open = |n: usize| "<a>".repeat(n);
        let close = |n: usize| "</a>".repeat(n);
        assert!(matches!(parse_document(&format!("{}{}", open(65), close(65)), &[]), Err(XmlError::TooDeep(_))));
        assert!(parse_document(&format!("{}{}", open(64), close(64)), &[]).is_ok());
    }

    #[test]
    fn empty_blank_and_a_single_angle_bracket_fail_gracefully() {
        assert_eq!(parse_document("", &[]), Err(XmlError::NoRootElement));
        assert_eq!(parse_document("   \n  ", &[]), Err(XmlError::NoRootElement));
        assert!(matches!(parse_document("<", &[]), Err(XmlError::NoRootElement) | Err(XmlError::UnexpectedEnd)));
    }

    #[test]
    fn first_start_tag_skips_prologue_and_reports_none_when_missing_or_broken() {
        let tag = first_start_tag(r#"<?xml version="1.0"?><!-- c --><bpmn:definitions xmlns:bpmn="urn:x">"#).unwrap();
        assert_eq!(tag.name, "definitions");
        assert_eq!(tag.attributes, vec![("xmlns:bpmn".to_string(), "urn:x".to_string())]);

        assert_eq!(first_start_tag(""), None);
        assert_eq!(first_start_tag("not xml at all"), None);
        assert_eq!(first_start_tag("<?xml unterminated"), None);
        assert_eq!(first_start_tag("<"), None);
    }
}
