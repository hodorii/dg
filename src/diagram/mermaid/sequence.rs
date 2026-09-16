//! mermaid `sequenceDiagram` 파서.

use super::text::{clean_lines, label};
use crate::diagram::ir::{LineKind, Marker, NotePlacement, ParticipantKind, Sequence, SequenceItem};

pub fn parse(source: &str) -> Sequence {
    let mut sequence = Sequence::default();
    // `end`가 프레임을 닫는지 `box`를 닫는지 구분한다.
    let mut block_stack: Vec<bool> = Vec::new();
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        let (first, rest) = split_first_word(trimmed);
        match first.to_ascii_lowercase().as_str() {
            "sequencediagram" => continue,
            "title" => {
                sequence.title = rest.trim_start_matches(':').trim().to_string();
                continue;
            }
            "autonumber" => {
                sequence.autonumber = true;
                continue;
            }
            "participant" | "actor" | "create" => {
                let (word, rest) = if first.eq_ignore_ascii_case("create") { split_first_word(rest) } else { (first, rest) };
                let kind = if word.eq_ignore_ascii_case("actor") { ParticipantKind::Actor } else { ParticipantKind::Box };
                declare_participant(&mut sequence, rest, kind);
                continue;
            }
            "destroy" | "links" | "link" | "properties" | "details" => continue,
            "box" => {
                block_stack.push(true);
                continue;
            }
            "activate" => {
                let p = sequence.intern(rest.trim(), "", ParticipantKind::Box);
                sequence.items.push(SequenceItem::Activate(p));
                continue;
            }
            "deactivate" => {
                let p = sequence.intern(rest.trim(), "", ParticipantKind::Box);
                sequence.items.push(SequenceItem::Deactivate(p));
                continue;
            }
            "note" => {
                parse_note(&mut sequence, rest);
                continue;
            }
            "loop" | "alt" | "opt" | "par" | "critical" | "break" | "rect" => {
                let kind = if first.eq_ignore_ascii_case("rect") { String::new() } else { first.to_string() };
                sequence.items.push(SequenceItem::FragmentStart { kind, label: label(rest) });
                block_stack.push(false);
                continue;
            }
            "else" | "and" | "option" => {
                sequence.items.push(SequenceItem::FragmentElse { label: label(rest) });
                continue;
            }
            "end" => {
                if block_stack.pop() != Some(true) {
                    sequence.items.push(SequenceItem::FragmentEnd);
                }
                continue;
            }
            _ => {}
        }
        if lower.starts_with("%%") {
            continue;
        }
        parse_message(&mut sequence, trimmed);
    }
    sequence
}

fn split_first_word(text: &str) -> (&str, &str) {
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    (&text[..end], text[end..].trim())
}

fn declare_participant(sequence: &mut Sequence, rest: &str, kind: ParticipantKind) {
    let (id, alias) = match rest.find(" as ") {
        Some(p) => (&rest[..p], rest[p + 4..].trim()),
        None => (rest, ""),
    };
    let id = id.trim();
    let text = if alias.is_empty() { label(id) } else { label(alias) };
    let index = sequence.intern(id, &text, kind);
    sequence.participants[index].kind = kind;
    sequence.participants[index].label = text;
}

fn parse_note(sequence: &mut Sequence, rest: &str) {
    let Some(colon) = rest.find(':') else { return };
    let (place, text) = (rest[..colon].trim(), label(&rest[colon + 1..]));
    let lower = place.to_ascii_lowercase();
    let placement = if let Some(names) = lower.strip_prefix("over") {
        let names = &place[place.len() - names.len()..];
        let mut parts = names.split(',').map(str::trim).filter(|s| !s.is_empty());
        let a = parts.next().unwrap_or("");
        let b = parts.next().unwrap_or(a);
        let a = sequence.intern(a, "", ParticipantKind::Box);
        let b = sequence.intern(b, "", ParticipantKind::Box);
        NotePlacement::Over(a.min(b), a.max(b))
    } else if let Some(name) = lower.strip_prefix("left of") {
        let name = &place[place.len() - name.len()..];
        NotePlacement::LeftOf(sequence.intern(name.trim(), "", ParticipantKind::Box))
    } else if let Some(name) = lower.strip_prefix("right of") {
        let name = &place[place.len() - name.len()..];
        NotePlacement::RightOf(sequence.intern(name.trim(), "", ParticipantKind::Box))
    } else {
        return;
    };
    sequence.items.push(SequenceItem::Note { placement, lines: text.lines().map(str::to_string).collect() });
}

/// `A->>+B: text`
fn parse_message(sequence: &mut Sequence, line: &str) {
    let Some(arrow_start) = line.find('-') else { return };
    // 화살표 앞의 `<`도 화살표에 속한다.
    let mut start = arrow_start;
    if start > 0 && line[..start].ends_with('<') {
        start -= 1;
        if start > 0 && line[..start].ends_with('<') {
            start -= 1;
        }
    }
    let from_text = line[..start].trim();
    let after = &line[arrow_start..];
    let mut arrow_end = 0;
    for c in after.chars() {
        if matches!(c, '-' | '>' | 'x' | ')' | '<') {
            arrow_end += c.len_utf8();
        } else {
            break;
        }
    }
    let arrow = &after[..arrow_end];
    let remainder = &after[arrow_end..];
    let (target_text, text) = match remainder.find(':') {
        Some(p) => (remainder[..p].trim(), label(&remainder[p + 1..])),
        None => (remainder.trim(), String::new()),
    };
    let mut activate_target = false;
    let mut deactivate_source = false;
    let target_text = if let Some(t) = target_text.strip_prefix('+') {
        activate_target = true;
        t.trim()
    } else if let Some(t) = target_text.strip_prefix('-') {
        deactivate_source = true;
        t.trim()
    } else {
        target_text
    };
    if from_text.is_empty() || target_text.is_empty() {
        return;
    }
    let kind = if arrow.starts_with("--") { LineKind::Dashed } else { LineKind::Solid };
    let head = if arrow.ends_with(">>") {
        Marker::Arrow
    } else if arrow.ends_with('x') {
        Marker::Cross
    } else if arrow.ends_with(')') || arrow.ends_with('>') {
        Marker::OpenArrow
    } else {
        Marker::None
    };
    let from = sequence.intern(from_text, "", ParticipantKind::Box);
    let to = sequence.intern(target_text, "", ParticipantKind::Box);
    sequence.items.push(SequenceItem::Message { from, to, label: text, kind, head, activate_target, deactivate_source });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_participants_messages_and_blocks() {
        let s = parse("sequenceDiagram\n actor U as User\n participant S\n U->>+S: login\n alt ok\n S-->>-U: token\n else fail\n S-xU: error\n end\n Note over U,S: done\n");
        assert_eq!(s.participants[0].label, "User");
        assert_eq!(s.participants[0].kind, ParticipantKind::Actor);
        assert_eq!(s.items.len(), 7);
        match &s.items[0] {
            SequenceItem::Message { label, activate_target, head, .. } => {
                assert_eq!(label, "login");
                assert!(activate_target);
                assert_eq!(*head, Marker::Arrow);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(s.items[6], SequenceItem::Note { placement: NotePlacement::Over(0, 1), .. }));
    }
}
