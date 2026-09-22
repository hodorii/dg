//! 마크다운 링크의 목적지 분류, 화면 위치 인식, 헤딩 슬러그, 파일 탐색 히스토리.

use crate::line::Line;
use crate::text::width_of;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 링크 목적지 종류.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// `#slug` — `#`를 뗀 슬러그.
    Anchor(String),
    /// 스킴이 있는 절대 URL(`http://`, `https://`, `mailto:` 등) — OS 핸들러로 연다.
    External(String),
    /// 그 외(상대/절대 파일 경로).
    File(String),
}

/// `dest_url`을 종류로 분류한다. 스킴 판정은 `scheme ":"` 형태(영문자로 시작, 이후
/// 영숫자/`+`/`-`/`.`)를 최소한으로 흉내 낸다 — 길이 1짜리 "스킴"(Windows 드라이브 문자
/// `C:\`)은 스킴으로 보지 않고 파일 경로로 취급한다.
pub fn classify(dest_url: &str) -> LinkKind {
    if let Some(slug) = dest_url.strip_prefix('#') {
        return LinkKind::Anchor(slug.to_string());
    }
    if let Some(colon) = dest_url.find(':') {
        let scheme = &dest_url[..colon];
        let looks_like_scheme = scheme.len() > 1
            && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
        if looks_like_scheme {
            return LinkKind::External(dest_url.to_string());
        }
    }
    LinkKind::File(dest_url.to_string())
}

/// 화면에 그려진 링크 하나의 위치. `col_start`/`col_end`는 문자 폭 기준(터미널 칸 수)이다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkPosition {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
    /// `Document.links`의 인덱스.
    pub link_index: usize,
}

/// 최종 표시 줄에서 밑줄(`Style::underline`) 구간을 찾아 등장 순서대로 위치 목록을 만든다.
/// `style.rs` 전체에서 밑줄은 링크 하나뿐이라(`Style::merge`가 밑줄을 OR로만 합쳐 굵게·기울임과
/// 섞여도 남는다) 신뢰할 수 있는 마커다.
pub fn locate_links(lines: &[Line]) -> Vec<LinkPosition> {
    let mut result = Vec::new();
    let mut link_index = 0usize;
    for (row, line) in lines.iter().enumerate() {
        let mut col = 0usize;
        for (text, style) in line.runs() {
            let w = width_of(text);
            if style.underline {
                result.push(LinkPosition { line: row, col_start: col, col_end: col + w, link_index });
                link_index += 1;
            }
            col += w;
        }
    }
    result
}

/// 뒤로/앞으로 히스토리 한 칸 — 렌더된 내용이 아니라 경로·스크롤 위치만 담는다(되돌아갈 때
/// 파일을 다시 읽어, 그 사이의 수정 사항도 자연히 반영된다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub path: PathBuf,
    pub top: usize,
}

/// 방문한 파일의 뒤로/앞으로 히스토리.
#[derive(Default)]
pub struct History {
    back: Vec<HistoryEntry>,
    forward: Vec<HistoryEntry>,
}

impl History {
    /// 새 파일로 이동하기 직전에 현재 위치를 쌓는다. 앞으로 기록은 버려진다(웹 브라우저와
    /// 같은 관례 — 되돌아간 지점에서 새 링크를 따라가면 그 이후의 "앞으로"는 의미가 없다).
    pub fn record(&mut self, current: HistoryEntry) {
        self.back.push(current);
        self.forward.clear();
    }

    /// 뒤로 갈 수 있으면 `current`를 앞으로 기록에 쌓고 이전 항목을 돌려준다.
    pub fn go_back(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        let previous = self.back.pop()?;
        self.forward.push(current);
        Some(previous)
    }

    /// 앞으로 갈 수 있으면 `current`를 뒤로 기록에 쌓고 다음 항목을 돌려준다.
    pub fn go_forward(&mut self, current: HistoryEntry) -> Option<HistoryEntry> {
        let next = self.forward.pop()?;
        self.back.push(current);
        Some(next)
    }
}

/// OS 기본 핸들러로 `url`을 연다. 프로세스 실행 자체의 성공/실패만 본다(브라우저 안에서
/// 실제로 열렸는지는 확인할 수 없다).
pub fn open_external(url: &str) -> io::Result<()> {
    let mut command = opener_command();
    command.arg(url).stdout(Stdio::null()).stderr(Stdio::null());
    command.spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn opener_command() -> Command {
    Command::new("open")
}

#[cfg(target_os = "windows")]
fn opener_command() -> Command {
    let mut command = Command::new("cmd");
    command.args(["/c", "start", ""]);
    command
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn opener_command() -> Command {
    Command::new("xdg-open")
}

/// `base_dir` 기준으로 상대 경로를 편다. 이미 절대 경로면 그대로 돌려준다(`Path::join`의
/// 기본 동작).
pub fn resolve_file_path(base_dir: &Path, relative: &str) -> PathBuf {
    base_dir.join(relative)
}

const BASE64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// RFC 4648 표준 base64(패딩 포함). 알고리즘이 짧고 고정적이라 새 크레이트를 들일 값어치가
/// 없다 — `open_external`이 `opener` 대신 `std::process::Command`를 쓴 것과 같은 판단
/// (markdown-source-view).
fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (b0 as u32) << 16 | (b1 as u32) << 8 | b2 as u32;
        out.push(BASE64_ALPHABET[(n >> 18 & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[(n >> 12 & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 { BASE64_ALPHABET[(n >> 6 & 0x3f) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64_ALPHABET[(n & 0x3f) as usize] as char } else { '=' });
    }
    out
}

/// `text`를 OSC 52(클립보드) 이스케이프 시퀀스로 감싼다. 순수 함수 — 어떤 I/O도 하지 않고
/// 호출자(`pager::draw`)가 기존 출력 스트림에 그대로 쓴다. 지원 안 하는 터미널은 시퀀스를
/// 그냥 무시하므로 실패 상태 자체가 없다(markdown-source-view).
pub fn clipboard_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64_encode(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    #[test]
    fn classify_recognizes_anchor_external_and_file() {
        assert_eq!(classify("#section"), LinkKind::Anchor("section".into()));
        assert_eq!(classify("https://example.com/x"), LinkKind::External("https://example.com/x".into()));
        assert_eq!(classify("http://example.com"), LinkKind::External("http://example.com".into()));
        assert_eq!(classify("mailto:a@b.com"), LinkKind::External("mailto:a@b.com".into()));
        assert_eq!(classify("../docs/other.md"), LinkKind::File("../docs/other.md".into()));
        assert_eq!(classify(""), LinkKind::File("".into()));
        // Windows 드라이브 문자는 스킴이 아니라 파일 경로로 본다.
        assert_eq!(classify("C:\\docs\\x.md"), LinkKind::File("C:\\docs\\x.md".into()));
    }

    #[test]
    fn locate_links_finds_underlined_runs_in_order() {
        let mut a = Line::empty();
        a.push_str("보통 텍스트 ", Style::PLAIN);
        a.push_str("링크1", Style::PLAIN.underline());
        a.push_str(" 사이 ", Style::PLAIN);
        a.push_str("굵은링크", Style::PLAIN.bold().underline());
        let mut b = Line::empty();
        b.push_str("둘째 줄, 링크 없음", Style::PLAIN);
        let positions = locate_links(&[a, b]);
        assert_eq!(positions.len(), 2, "{positions:?}");
        assert_eq!(positions[0].link_index, 0);
        assert_eq!(positions[0].line, 0);
        assert_eq!(positions[1].link_index, 1);
        assert!(positions[1].col_start > positions[0].col_end, "{positions:?}");
    }

    #[test]
    fn locate_links_returns_empty_for_no_links() {
        let mut a = Line::empty();
        a.push_str("링크 없는 줄", Style::PLAIN);
        assert!(locate_links(&[a]).is_empty());
    }

    #[test]
    fn history_round_trips_and_drops_forward_on_new_record() {
        // A -> B -> C 순서로 이동했다고 가정한다: 각 파일을 "떠날 때" 그 파일을 record한다.
        let mut history = History::default();
        let a = HistoryEntry { path: PathBuf::from("a.md"), top: 0 };
        let b = HistoryEntry { path: PathBuf::from("b.md"), top: 5 };
        let c = HistoryEntry { path: PathBuf::from("c.md"), top: 10 };

        assert_eq!(history.go_back(a.clone()), None, "빈 히스토리는 뒤로 갈 수 없다");

        history.record(a.clone()); // A를 떠나 B로
        history.record(b.clone()); // B를 떠나 C로
        // 지금 C에 있다. 뒤로 가면 가장 최근에 떠난 B로 돌아가야 한다.
        let back_to_b = history.go_back(c.clone()).expect("C -> B로 돌아가야 한다");
        assert_eq!(back_to_b, b);
        // 다시 앞으로 가면 C로.
        let forward_to_c = history.go_forward(b.clone()).expect("다시 C로 가야 한다");
        assert_eq!(forward_to_c, c);

        // 되돌아간 지점(B)에서 새 링크를 따라가면(D로) 앞으로 기록(C)은 버려진다.
        let back_to_b_again = history.go_back(c.clone()).expect("다시 B로 돌아가야 한다");
        assert_eq!(back_to_b_again, b);
        history.record(HistoryEntry { path: PathBuf::from("d.md"), top: 0 });
        assert_eq!(history.go_forward(HistoryEntry { path: PathBuf::from("d.md"), top: 0 }), None, "새 기록 뒤엔 앞으로 갈 곳이 없어야 한다");
    }

    #[test]
    fn resolve_file_path_joins_relative_to_base_dir() {
        let base = Path::new("/docs/guide");
        assert_eq!(resolve_file_path(base, "../other.md"), PathBuf::from("/docs/guide/../other.md"));
        assert_eq!(resolve_file_path(base, "sub/x.md"), PathBuf::from("/docs/guide/sub/x.md"));
    }

    /// RFC 4648 표준 테스트 벡터로 손으로 짠 base64 인코더를 검증한다.
    #[test]
    fn base64_encode_matches_rfc_4648_test_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// 4.1/4.5 클립보드 시퀀스는 OSC 52로 감싼 base64 텍스트이고, 빈 문자열·유니코드도
    /// 패닉 없이 처리한다.
    #[test]
    fn clipboard_sequence_wraps_base64_in_osc52() {
        assert_eq!(clipboard_sequence("foo"), "\x1b]52;c;Zm9v\x07");
        assert_eq!(clipboard_sequence(""), "\x1b]52;c;\x07");
        let unicode = clipboard_sequence("한글");
        assert!(unicode.starts_with("\x1b]52;c;") && unicode.ends_with('\x07'));
    }
}
