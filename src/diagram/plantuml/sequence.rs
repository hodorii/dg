//! PlantUML 시퀀스 다이어그램 파서.

use super::text::{clean_lines, keyword, label, split_alias};
use crate::diagram::ir::{LineKind, Marker, NotePlacement, ParticipantKind, Sequence, SequenceItem};

struct Message {
    from: String,
    to: String,
    kind: LineKind,
    head: Marker,
    label: String,
    activate_target: bool,
    deactivate_source: bool,
}

pub fn looks_like_message(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.starts_with('[') && !trimmed.ends_with(']') && split_message(trimmed).is_some()
}

pub fn parse(source: &str) -> Sequence {
    let mut sequence = Sequence::default();
    let mut block_stack: Vec<bool> = Vec::new(); // true = box
    let mut note_lines: Option<(NotePlacement, Vec<String>)> = None;
    let mut skip_until: Option<&'static str> = None;
    let mut last_message: Option<(usize, usize)> = None;
    for line in clean_lines(source) {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if let Some(terminator) = skip_until {
            if lower.starts_with(terminator) {
                skip_until = None;
            }
            continue;
        }
        if let Some((placement, lines)) = note_lines.as_mut() {
            if lower.starts_with("end note") || lower.starts_with("endnote") || lower.starts_with("end hnote") || lower.starts_with("end rnote") || lower.starts_with("end ref") {
                let placement = *placement;
                let lines = std::mem::take(lines);
                note_lines = None;
                sequence.items.push(SequenceItem::Note { placement, lines });
            } else {
                lines.push(label(trimmed));
            }
            continue;
        }
        let first = keyword(&lower);
        let rest = trimmed[first.len()..].trim();
        match first {
            "title" => {
                if rest.is_empty() {
                    skip_until = Some("end title");
                } else {
                    sequence.title = label(rest);
                }
                continue;
            }
            "autonumber" => {
                sequence.autonumber = !rest.starts_with("stop");
                continue;
            }
            "participant" | "actor" | "boundary" | "control" | "entity" | "database" | "collections" | "queue" => {
                let kind = match first {
                    "actor" => ParticipantKind::Actor,
                    "boundary" => ParticipantKind::Boundary,
                    "control" => ParticipantKind::Control,
                    "entity" => ParticipantKind::Entity,
                    "database" => ParticipantKind::Database,
                    "collections" => ParticipantKind::Collections,
                    "queue" => ParticipantKind::Queue,
                    _ => ParticipantKind::Box,
                };
                let (id, text) = split_alias(rest);
                let index = sequence.intern(&id, &text, kind);
                sequence.participants[index].kind = kind;
                sequence.participants[index].label = text;
                continue;
            }
            "create" => {
                let rest = rest.strip_prefix("participant").unwrap_or(rest).trim();
                let (id, text) = split_alias(rest);
                sequence.intern(&id, &text, ParticipantKind::Box);
                continue;
            }
            "activate" => {
                let index = sequence.intern(rest, "", ParticipantKind::Box);
                sequence.items.push(SequenceItem::Activate(index));
                continue;
            }
            "deactivate" | "destroy" => {
                let index = sequence.intern(rest, "", ParticipantKind::Box);
                sequence.items.push(SequenceItem::Deactivate(index));
                continue;
            }
            "note" | "hnote" | "rnote" | "ref" => {
                let default_participant = last_message.map(|(from, _)| from);
                if let Some((placement, text)) = parse_note_header(&mut sequence, rest, default_participant) {
                    match text {
                        Some(text) => sequence.items.push(SequenceItem::Note { placement, lines: text.lines().map(str::to_string).collect() }),
                        None => note_lines = Some((placement, Vec::new())),
                    }
                }
                continue;
            }
            "alt" | "opt" | "loop" | "par" | "break" | "critical" | "group" => {
                let kind = if first == "group" { String::new() } else { first.to_string() };
                sequence.items.push(SequenceItem::FragmentStart { kind, label: label(rest) });
                block_stack.push(false);
                continue;
            }
            "else" => {
                sequence.items.push(SequenceItem::FragmentElse { label: label(rest) });
                continue;
            }
            "end" | "endalt" | "endloop" | "endopt" | "endpar" | "endgroup" => {
                if rest.starts_with("box") {
                    block_stack.pop();
                } else if block_stack.pop() != Some(true) {
                    sequence.items.push(SequenceItem::FragmentEnd);
                }
                continue;
            }
            "box" => {
                block_stack.push(true);
                continue;
            }
            "return" => {
                if let Some((from, to)) = last_message {
                    sequence.items.push(SequenceItem::Message {
                        from: to,
                        to: from,
                        label: label(rest),
                        kind: LineKind::Dashed,
                        head: Marker::Arrow,
                        activate_target: false,
                        deactivate_source: true,
                    });
                    last_message = Some((to, from));
                }
                continue;
            }
            "|||" | "delay" => continue,
            _ => {}
        }
        if trimmed.starts_with("==") {
            sequence.items.push(SequenceItem::Divider { label: label(trimmed.trim_matches('=')) });
            continue;
        }
        if trimmed.starts_with("...") {
            sequence.items.push(SequenceItem::Delay { label: label(trimmed.trim_matches('.')) });
            continue;
        }
        if trimmed.starts_with("||") || trimmed.starts_with('[') || trimmed.ends_with(']') {
            continue;
        }
        if let Some(message) = split_message(trimmed) {
            let from = sequence.intern(&message.from, "", ParticipantKind::Box);
            let to = sequence.intern(&message.to, "", ParticipantKind::Box);
            sequence.items.push(SequenceItem::Message {
                from,
                to,
                label: message.label,
                kind: message.kind,
                head: message.head,
                activate_target: message.activate_target,
                deactivate_source: message.deactivate_source,
            });
            last_message = Some((from, to));
        }
    }
    if let Some((placement, lines)) = note_lines {
        sequence.items.push(SequenceItem::Note { placement, lines });
    }
    sequence
}

/// `left of A : text` / `over A, B` / `right` → (위치, 한 줄 본문이면 Some)
fn parse_note_header(sequence: &mut Sequence, rest: &str, default_participant: Option<usize>) -> Option<(NotePlacement, Option<String>)> {
    let (header, text) = match rest.find(':') {
        Some(p) => (rest[..p].trim(), Some(label(&rest[p + 1..]))),
        None => (rest.trim(), None),
    };
    let header = super::text::strip_decorations(header);
    let lower = header.to_ascii_lowercase();
    let mut intern = |name: &str| sequence.intern(name.trim().trim_matches('"'), "", ParticipantKind::Box);
    let placement = if let Some(names) = lower.strip_prefix("over") {
        let names = header[header.len() - names.len()..].trim();
        let mut parts = names.split(',').map(str::trim).filter(|s| !s.is_empty());
        let a = parts.next().map(&mut intern)?;
        let b = parts.next().map(&mut intern).unwrap_or(a);
        NotePlacement::Over(a.min(b), a.max(b))
    } else if lower.starts_with("across") {
        let last = sequence.participants.len().saturating_sub(1);
        NotePlacement::Over(0, last)
    } else if let Some(name) = lower.strip_prefix("left of") {
        NotePlacement::LeftOf(intern(&header[header.len() - name.len()..]))
    } else if let Some(name) = lower.strip_prefix("right of") {
        NotePlacement::RightOf(intern(&header[header.len() - name.len()..]))
    } else if lower.starts_with("left") {
        NotePlacement::LeftOf(default_participant?)
    } else if lower.starts_with("right") {
        NotePlacement::RightOf(default_participant?)
    } else {
        return None;
    };
    Some((placement, text))
}

/// 따옴표 밖에서 화살표 토큰의 바이트 범위를 찾는다.
fn find_arrow(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let length = bytes.len();
    let mut in_quote = false;
    let mut index = 0;
    while index < length {
        let c = bytes[index];
        if c == b'"' {
            in_quote = !in_quote;
            index += 1;
            continue;
        }
        if in_quote || !(c == b'-' || c == b'.') {
            index += 1;
            continue;
        }
        let mut start = index;
        if start > 0 && bytes[start - 1] == b'<' {
            start -= 1;
            if start > 0 && bytes[start - 1] == b'<' {
                start -= 1;
            }
        } else if start > 0 && matches!(bytes[start - 1], b'\\' | b'/' | b'o' | b'x') && (start < 2 || bytes[start - 2].is_ascii_whitespace()) {
            start -= 1;
        }
        let mut end = index;
        while end < length && (bytes[end] == b'-' || bytes[end] == b'.') {
            end += 1;
        }
        if end < length && bytes[end] == b'[' {
            while end < length && bytes[end] != b']' {
                end += 1;
            }
            end = (end + 1).min(length);
            while end < length && (bytes[end] == b'-' || bytes[end] == b'.') {
                end += 1;
            }
        }
        let body_end = end;
        if end < length && bytes[end] == b'>' {
            end += 1;
            if end < length && bytes[end] == b'>' {
                end += 1;
            }
            if end < length && matches!(bytes[end], b'o' | b'x') && (end + 1 >= length || bytes[end + 1].is_ascii_whitespace()) {
                end += 1;
            }
        } else if end < length && matches!(bytes[end], b'\\' | b'/') {
            end += 1;
            if end < length && bytes[end] == bytes[end - 1] {
                end += 1;
            }
        } else if end < length && matches!(bytes[end], b'o' | b'x') && (end + 1 >= length || bytes[end + 1].is_ascii_whitespace()) {
            end += 1;
        }
        if start < index || end > body_end {
            return Some((start, end));
        }
        index = end.max(index + 1);
    }
    None
}

fn split_message(line: &str) -> Option<Message> {
    let (start, end) = find_arrow(line)?;
    let arrow = &line[start..end];
    let from = line[..start].trim().trim_matches('"').to_string();
    let remainder = &line[end..];
    let (target_part, text) = match remainder.find(':') {
        Some(p) => (remainder[..p].trim(), label(&remainder[p + 1..])),
        None => (remainder.trim(), String::new()),
    };
    let mut activate_target = false;
    let mut deactivate_source = false;
    let mut target = target_part.to_string();
    for token in ["++", "--", "**", "!!"] {
        if let Some(stripped) = target.strip_suffix(token) {
            match token {
                "++" => activate_target = true,
                "--" => deactivate_source = true,
                _ => {}
            }
            target = stripped.trim().to_string();
        }
    }
    let target = target.trim_matches('"').to_string();
    if from.is_empty() || target.is_empty() || from.contains(char::is_whitespace) && !line[..start].trim().starts_with('"') {
        return None;
    }
    // 시퀀스에서는 `-->`가 점선이다(클래스도와 다르다).
    let kind = if arrow.contains("--") || arrow.contains('.') { LineKind::Dashed } else { LineKind::Solid };
    let has_left = arrow.starts_with('<') || arrow.starts_with('\\') || arrow.starts_with('/') || arrow.starts_with('o') || arrow.starts_with('x');
    let has_right = arrow.ends_with('>') || arrow.ends_with('\\') || arrow.ends_with('/') || arrow.ends_with('o') || arrow.ends_with('x');
    let reversed = has_left && !has_right;
    let head = if reversed {
        if arrow.starts_with("<<") {
            Marker::OpenArrow
        } else if arrow.starts_with('<') {
            Marker::Arrow
        } else if arrow.starts_with('x') {
            Marker::Cross
        } else if arrow.starts_with('o') {
            Marker::Circle
        } else {
            Marker::OpenArrow
        }
    } else if arrow.ends_with(">>") || arrow.ends_with('\\') || arrow.ends_with('/') {
        Marker::OpenArrow
    } else if arrow.ends_with('x') {
        Marker::Cross
    } else if arrow.ends_with('o') {
        Marker::Circle
    } else if arrow.ends_with('>') {
        Marker::Arrow
    } else {
        Marker::None
    };
    let (from, to) = if reversed { (target, from) } else { (from, target) };
    Some(Message { from, to, kind, head, label: text, activate_target, deactivate_source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_messages_notes_and_fragments() {
        let source = "@startuml\nactor User as U\nparticipant \"Web App\" as W\nU -> W ++ : login\nW --> U -- : token\nW <- U : refresh\nnote right of W : cached\nalt ok\n  W -> W : verify\nelse fail\n  W ->x U : error\nend\n== Phase 2 ==\n...later...\nreturn done\n@enduml";
        let s = parse(source);
        assert_eq!(s.participants[1].label, "Web App");
        assert!(matches!(s.items[0], SequenceItem::Message { activate_target: true, .. }));
        assert!(matches!(s.items[1], SequenceItem::Message { kind: LineKind::Dashed, deactivate_source: true, .. }));
        match &s.items[2] {
            SequenceItem::Message { from, to, label, .. } => {
                assert_eq!((*from, *to), (0, 1));
                assert_eq!(label, "refresh");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(s.items[3], SequenceItem::Note { placement: NotePlacement::RightOf(1), .. }));
        assert!(matches!(s.items[4], SequenceItem::FragmentStart { .. }));
        assert!(matches!(s.items[7], SequenceItem::Message { head: Marker::Cross, .. }));
        assert!(matches!(s.items[9], SequenceItem::Divider { .. }));
        assert!(matches!(s.items[10], SequenceItem::Delay { .. }));
    }

    #[test]
    fn detects_message_lines() {
        assert!(looks_like_message("A -> B : hi"));
        assert!(looks_like_message("Bob <-- Alice"));
        assert!(!looks_like_message("class A"));
        assert!(!looks_like_message("A <|-- B"));
    }
}
