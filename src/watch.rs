//! 파일 경로를 폴링해 변경을 감지한다. 신규 의존성 없이 `std::fs::metadata`의 수정 시각만 쓴다
//! (실측: OS 이벤트 기반 `notify` 크레이트 대비 바이너리 +114KB, 의존성 +11개, 상시 스레드 1개
//! 추가, RSS 약 2배 — dg의 "의존성 4개, 단일 바이너리" 원칙과 맞지 않아 폴링을 택했다).

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// 폴링 주기. 요구사항 2.1(1초 이내 반영)보다 넉넉히 짧게 잡는다.
pub const POLL_INTERVAL: Duration = Duration::from_millis(300);

/// `poll`/`poll_forced` 한 번의 결과.
#[derive(Debug)]
pub enum PollResult {
    /// 변화 없음, 또는 이미 알린 오류가 계속되는 중(재보고 억제).
    Unchanged,
    /// 파일이 바뀌어(또는 강제로) 다시 읽었다.
    Changed(String),
    /// 읽기 실패 상태로 막 들어갔다(전이 1회만 보고).
    ReadError(String),
}

/// 파일 하나를 감시한다.
pub struct Watcher {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    in_error: bool,
}

impl Watcher {
    /// `path`의 현재 mtime을 기준선으로 감시를 시작한다. 시작 시점에 메타데이터를 못 읽어도
    /// 패닉하지 않는다(다음 `poll()`에서 오류로 보고된다).
    pub fn new(path: impl Into<PathBuf>) -> Watcher {
        let path = path.into();
        let last_modified = fs::metadata(&path).ok().and_then(|meta| meta.modified().ok());
        Watcher { path, last_modified, in_error: false }
    }

    /// mtime이 바뀌었을 때만 다시 읽는다.
    pub fn poll(&mut self) -> PollResult {
        self.check(false)
    }

    /// mtime과 무관하게 즉시 다시 읽는다(수동 갱신). 오류 보고 규칙은 `poll`과 동일(전이 1회만).
    pub fn poll_forced(&mut self) -> PollResult {
        self.check(true)
    }

    fn check(&mut self, forced: bool) -> PollResult {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) => return self.report_error(error.to_string()),
        };
        let modified = metadata.modified().ok();
        if !forced && !self.in_error && modified == self.last_modified {
            return PollResult::Unchanged;
        }
        match fs::read_to_string(&self.path) {
            Ok(content) => {
                self.last_modified = modified;
                self.in_error = false;
                PollResult::Changed(content)
            }
            Err(error) => self.report_error(error.to_string()),
        }
    }

    /// 오류 전이일 때만 `ReadError`를 돌려주고, 이미 오류 상태면 `Unchanged`로 조용히 넘어간다.
    fn report_error(&mut self, message: String) -> PollResult {
        if self.in_error {
            return PollResult::Unchanged;
        }
        self.in_error = true;
        PollResult::ReadError(format!("{}: {message}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn with_content(content: &str) -> TempFile {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("dg-watch-test-{}-{n}", std::process::id()));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(content.as_bytes()).unwrap();
            TempFile { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn touch_with(path: &std::path::Path, content: &str) {
        // mtime이 확실히 앞서게 살짝 늦춘다(이 환경은 나노초 해상도라 여유만 둔다).
        std::thread::sleep(Duration::from_millis(20));
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    /// 2.2 막 시작했을 때는 아직 아무 변화도 없다.
    #[test]
    fn fresh_watcher_reports_unchanged() {
        let temp = TempFile::with_content("처음");
        let mut watcher = Watcher::new(&temp.path);
        assert!(matches!(watcher.poll(), PollResult::Unchanged));
    }

    /// 2.1 내용이 바뀌면 새 내용과 함께 `Changed`.
    #[test]
    fn changed_file_is_reread() {
        let temp = TempFile::with_content("처음");
        let mut watcher = Watcher::new(&temp.path);
        touch_with(&temp.path, "바뀜");
        match watcher.poll() {
            PollResult::Changed(content) => assert_eq!(content, "바뀜"),
            _ => panic!("Changed를 기대했다"),
        }
        // 바로 다음 폴은 다시 안 바뀌었으니 Unchanged.
        assert!(matches!(watcher.poll(), PollResult::Unchanged));
    }

    /// 5.1/5.3 파일이 사라지면 패닉 없이 오류를 한 번만 보고하고, 그 뒤로는 조용하다.
    #[test]
    fn missing_file_reports_error_once_then_stays_quiet() {
        let temp = TempFile::with_content("처음");
        let mut watcher = Watcher::new(&temp.path);
        fs::remove_file(&temp.path).unwrap();
        assert!(matches!(watcher.poll(), PollResult::ReadError(_)));
        assert!(matches!(watcher.poll(), PollResult::Unchanged), "오류가 이어지는 동안은 조용해야 한다");
        assert!(matches!(watcher.poll(), PollResult::Unchanged));
    }

    /// 5.2 파일이 다시 읽을 수 있게 되면 자동으로 갱신이 재개된다.
    #[test]
    fn recovers_after_file_comes_back() {
        let temp = TempFile::with_content("처음");
        let mut watcher = Watcher::new(&temp.path);
        fs::remove_file(&temp.path).unwrap();
        assert!(matches!(watcher.poll(), PollResult::ReadError(_)));
        touch_with(&temp.path, "복구됨");
        match watcher.poll() {
            PollResult::Changed(content) => assert_eq!(content, "복구됨"),
            other => panic!("복구 후 Changed를 기대했다: {other:?}"),
        }
    }

    /// 3.5 `poll_forced`는 mtime이 그대로여도 항상 다시 읽는다.
    #[test]
    fn poll_forced_rereads_even_without_mtime_change() {
        let temp = TempFile::with_content("처음");
        let mut watcher = Watcher::new(&temp.path);
        assert!(matches!(watcher.poll(), PollResult::Unchanged));
        match watcher.poll_forced() {
            PollResult::Changed(content) => assert_eq!(content, "처음"),
            _ => panic!("poll_forced는 mtime과 무관하게 Changed여야 한다"),
        }
    }
}
