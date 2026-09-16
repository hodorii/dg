//! mermaid `pie` 파서.
//!
//! 문법은 머리글 한 줄과 `"이름" : 값` 줄들이다.
//!
//! ```text
//! pie showData title 반려동물
//!   "Dogs" : 50
//!   "Cats" : 30
//! ```

use super::text::{clean_lines, keyword, label};
use crate::diagram::layout::chart::ChartItem;

/// 읽어낸 파이 차트 한 벌.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pie {
    /// `title`로 준 제목. 없으면 빈 문자열.
    pub title: String,
    /// `showData`가 붙었는지. 붙으면 백분율 옆에 원본 값도 적는다.
    pub show_data: bool,
    /// 항목. 소스에 적힌 순서 그대로다.
    pub items: Vec<ChartItem>,
}

/// 소스를 [`Pie`]로 읽는다. 알아볼 수 없는 줄은 오류 대신 건너뛴다.
pub fn parse(source: &str) -> Pie {
    let mut pie = Pie::default();
    let mut header_seen = false;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        // 머리글을 찾기 전 줄(예: `---` 머리말)은 흘려보낸다.
        if !header_seen {
            let first = keyword(trimmed);
            if !is_pie_keyword(first) {
                continue;
            }
            read_header(&mut pie, trimmed[first.len()..].trim());
            header_seen = true;
            continue;
        }
        if let Some(item) = data_item(trimmed) {
            pie.items.push(item);
        }
    }
    pie
}

/// 첫 낱말이 `pie`인지. `pie:`처럼 뒤에 기호가 붙어도 받는다.
fn is_pie_keyword(first: &str) -> bool {
    first.trim_end_matches(|c: char| !c.is_alphanumeric()).eq_ignore_ascii_case("pie")
}

/// `pie` 뒤에 남은 부분에서 `showData`와 `title`을 읽는다.
fn read_header(pie: &mut Pie, rest: &str) {
    let mut rest = rest;
    if let Some(after) = strip_leading_token(rest, "showData") {
        pie.show_data = true;
        rest = after;
    }
    let Some(after_title) = strip_leading_token(rest, "title") else { return };
    let mut title = after_title;
    // 진짜 mermaid는 `showData`를 `title` 앞에만 두지만, 뒤에 붙여 쓴 글도 받아 준다.
    if let Some(head) = strip_trailing_token(title, "showData") {
        pie.show_data = true;
        title = head;
    }
    pie.title = label(title);
}

/// 맨 앞의 낱말이 `token`이면 떼어낸 나머지를. 대소문자는 가리지 않는다.
fn strip_leading_token<'a>(text: &'a str, token: &str) -> Option<&'a str> {
    let head = text.get(..token.len())?;
    if !head.eq_ignore_ascii_case(token) {
        return None;
    }
    let rest = &text[token.len()..];
    // 낱말 경계를 확인해야 `titles`가 `title`로 걸리지 않는다.
    if rest.is_empty() || rest.starts_with(char::is_whitespace) { Some(rest.trim_start()) } else { None }
}

/// 맨 뒤의 낱말이 `token`이면 떼어낸 앞부분을.
fn strip_trailing_token<'a>(text: &'a str, token: &str) -> Option<&'a str> {
    let start = text.len().checked_sub(token.len())?;
    if !text.get(start..)?.eq_ignore_ascii_case(token) {
        return None;
    }
    let head = &text[..start];
    if head.is_empty() || head.ends_with(char::is_whitespace) { Some(head.trim_end()) } else { None }
}

/// `"이름" : 값` 한 줄. 값이 수가 아니면 `None`이라 그 줄만 빠진다.
fn data_item(line: &str) -> Option<ChartItem> {
    let separator = separator_position(line)?;
    let value: f64 = line[separator + 1..].trim().parse().ok()?;
    Some(ChartItem::new(label(&line[..separator]), value))
}

/// 따옴표 밖에 있는 첫 `:`의 바이트 위치. 이름 안의 `:`를 구분자로 잘못 잡지 않게 한다.
fn separator_position(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if quote.is_some() => escaped = true,
            '"' | '\'' => match quote {
                Some(open) if open == c => quote = None,
                None => quote = Some(c),
                Some(_) => {}
            },
            ':' if quote.is_none() => return Some(index),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(pie: &Pie) -> Vec<(&str, f64)> {
        pie.items.iter().map(|item| (item.name.as_str(), item.value)).collect()
    }

    // 2.1 `"이름" : 값` 줄이 이름과 값의 쌍으로 읽힌다.
    #[test]
    fn reads_quoted_name_and_value_pairs() {
        let pie = parse("pie\n  \"Dogs\" : 50\n  \"Cats\" : 30\n  \"Birds\" : 20\n");
        assert_eq!(pairs(&pie), vec![("Dogs", 50.0), ("Cats", 30.0), ("Birds", 20.0)]);
        assert_eq!(pie.title, "");
        assert!(!pie.show_data);
    }

    // 2.2 제목은 `pie` 줄에 이어 붙고, 남은 글 전부가 제목이다.
    #[test]
    fn reads_inline_title() {
        assert_eq!(parse("pie title 반려동물 분포\n \"A\" : 1\n").title, "반려동물 분포");
        assert_eq!(parse("pie title \"따옴표 제목\"\n \"A\" : 1\n").title, "따옴표 제목");
        // `title`이 없으면 제목도 없다. 낱말 경계를 보므로 `titles`는 제목 지시어가 아니다.
        assert_eq!(parse("pie\n \"A\" : 1\n").title, "");
        assert_eq!(parse("pie titles 뭔가\n \"A\" : 1\n").title, "");
    }

    // 2.3·2.4 `showData`는 어느 자리에 오든, 대소문자가 달라도 읽힌다.
    #[test]
    fn reads_show_data_in_either_order() {
        assert!(parse("pie showData\n \"A\" : 1\n").show_data);
        let before = parse("pie showData title 값 보기\n \"A\" : 1\n");
        assert!(before.show_data && before.title == "값 보기");
        let after = parse("pie title 값 보기 showData\n \"A\" : 1\n");
        assert!(after.show_data && after.title == "값 보기");
        assert!(parse("PIE SHOWDATA\n \"A\" : 1\n").show_data);
        assert!(!parse("pie title showDataset 설명\n \"A\" : 1\n").show_data);
    }

    // 1.1 `pie:`처럼 기호가 붙거나 대소문자가 달라도 머리글로 본다.
    #[test]
    fn accepts_decorated_and_cased_keyword() {
        assert_eq!(pairs(&parse("Pie:\n \"A\" : 2\n")), vec![("A", 2.0)]);
        assert_eq!(pairs(&parse("%% 주석\n\n   pie\n \"A\" : 2\n")), vec![("A", 2.0)]);
        // 머리글 앞의 머리말은 건너뛴다.
        assert_eq!(pairs(&parse("---\ntitle: x\n---\npie\n \"A\" : 2\n")), vec![("A", 2.0)]);
    }

    // 4.1 값이 수가 아닌 줄은 그 줄만 빠지고 나머지는 살아남는다.
    #[test]
    fn skips_unparsable_rows_without_losing_the_rest() {
        let pie = parse("pie\n \"A\" : 1\n 이건 구분자가 없다\n \"B\" : 값없음\n \"C\" : 3\n");
        assert_eq!(pairs(&pie), vec![("A", 1.0), ("C", 3.0)]);
        assert!(parse("pie").items.is_empty());
        assert!(parse("").items.is_empty());
        // 값이 모두 0이어도 항목 자체는 남는다.
        assert_eq!(pairs(&parse("pie\n \"A\" : 0\n \"B\" : 0\n")), vec![("A", 0.0), ("B", 0.0)]);
    }

    // 4.2 이름 속 이스케이프된 큰따옴표는 풀려서 들어온다.
    #[test]
    fn unescapes_quotes_inside_names() {
        assert_eq!(pairs(&parse("pie\n \"그는 #quot;예#quot;라 했다\" : 4\n")), vec![("그는 \"예\"라 했다", 4.0)]);
        assert_eq!(pairs(&parse("pie\n \"그는 &quot;예&quot;라 했다\" : 4\n")), vec![("그는 \"예\"라 했다", 4.0)]);
        assert_eq!(pairs(&parse("pie\n \"그는 \\\"예\\\"라 했다\" : 4\n")), vec![("그는 \"예\"라 했다", 4.0)]);
    }

    // 이름 안의 `:`는 구분자가 아니다.
    #[test]
    fn colon_inside_a_quoted_name_is_not_the_separator() {
        assert_eq!(pairs(&parse("pie\n \"12:30 출발\" : 7\n")), vec![("12:30 출발", 7.0)]);
        // 따옴표가 없으면 첫 `:`가 구분자다.
        assert_eq!(pairs(&parse("pie\n 그냥 이름 : 7\n")), vec![("그냥 이름", 7.0)]);
    }

    // 4.4 잘린 입력·이상한 값에도 파서가 무너지지 않는다.
    #[test]
    fn never_panics_on_odd_input() {
        for source in [
            "pie title",
            "pie showData title",
            "pie\n : 5\n",
            "pie\n \"열린 따옴표 : 5\n",
            "pie\n \"A\" : \n",
            "pie\n \"A\" : 1e400\n \"B\" : NaN\n \"C\" : -3\n",
            "pie\n \"가나다\\\" : 1\n",
            "pie:::\n\"A\":1",
        ] {
            let _ = parse(source);
        }
    }
}
