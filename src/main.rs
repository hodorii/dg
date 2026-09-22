mod cli;

use clap::Parser;
use cli::{Cli, LangArg, StyleArg};
use dg::diagram::{self, DiagramOptions, ErNotation, Language};
use dg::markdown::{self, DiagramBlock, Document};
use dg::pager;
use dg::style::Theme;
use dg::watch::{self, PollResult, Watcher};
use std::io::{self, IsTerminal, Read, Write};

const MAX_WIDTH: usize = 120;

fn main() {
    if let Err(error) = run() {
        eprintln!("dg: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let cli = Cli::parse();
    if cli.watch && matches!(cli.file.as_deref(), None | Some("-")) {
        return Err(io::Error::other("--watch는 파일 경로가 필요합니다(표준입력은 감시할 수 없습니다)"));
    }
    let (source, title) = read_input(cli.file.as_deref())?;
    let stdout_is_tty = io::stdout().is_terminal();
    let theme = pick_theme(cli.style, stdout_is_tty);
    let diagram_language = if cli.diagram || cli.lang.is_some() || looks_like_diagram_file(cli.file.as_deref()) {
        Some(match cli.lang {
            Some(LangArg::Mermaid) => Language::Mermaid,
            Some(LangArg::Plantuml) => Language::PlantUml,
            None => diagram::language_of_source(cli.file.as_deref(), &source).unwrap_or(Language::Mermaid),
        })
    } else {
        None
    };
    // `-w`를 주면 문단·블록 모두 그 폭. 아니면 문단은 120까지, 블록은 터미널 폭.
    let explicit_width = cli.width;
    let diagram_options = DiagramOptions {
        er_notation: cli.er_notation.map(Into::into).or_else(|| std::env::var("DG_ER_NOTATION").ok().and_then(|v| ErNotation::parse(&v))).unwrap_or_default(),
        direction: cli
            .direction
            .map(|d| match d {
                cli::DirectionArg::Tb => (diagram::ir::Direction::TopDown, false),
                cli::DirectionArg::Bt => (diagram::ir::Direction::TopDown, true),
                cli::DirectionArg::Lr => (diagram::ir::Direction::LeftRight, false),
                cli::DirectionArg::Rl => (diagram::ir::Direction::LeftRight, true),
            })
            .or_else(|| std::env::var("DG_DIRECTION").ok().and_then(|v| diagram::options::parse_direction(&v))),
    };
    // `source`를 인자로 받는다(캡처하지 않음) — 감시 모드에서 최신 내용을 그리려면 호출마다 다른
    // 소스를 넘길 수 있어야 한다.
    let render = move |source: &str, width: usize, block_width: usize| -> Document {
        let block_width = explicit_width.unwrap_or(block_width);
        match diagram_language {
            Some(language) => {
                let lang = if language == Language::Mermaid { "mermaid" } else { "plantuml" };
                match diagram::render(language, source, &theme, block_width, diagram_options) {
                    Some(lines) => {
                        let block = DiagramBlock { start: 0, end: lines.len(), lang: lang.to_string(), source: source.to_string() };
                        Document { lines, diagrams: vec![block], ..Document::default() }
                    }
                    None => markdown::render_document(&format!("```{lang}\n{}\n```\n", source.trim_end()), &theme, width, block_width, diagram_options),
                }
            }
            None => markdown::render_document(source, &theme, width, block_width, diagram_options),
        }
    };
    // 표준입력(경로 없음)이면 None — 상대 링크 해석·다른 파일 이동·히스토리 전부 자연히 꺼진다.
    let current_path = cli.file.as_deref().filter(|f| *f != "-").map(std::path::PathBuf::from);
    let use_pager = stdout_is_tty && !cli.print;
    if use_pager {
        let max_width = cli.width.unwrap_or(MAX_WIDTH);
        let watcher = cli.watch.then(|| Watcher::new(cli.file.as_deref().expect("--watch는 위에서 파일 경로 유무를 이미 검증함")));
        return pager::Pager::new(&title, &theme, max_width, source, render, watcher, current_path).run();
    }
    let columns = crossterm::terminal::size().map(|(c, _)| c as usize).unwrap_or(80);
    let width = cli.width.unwrap_or(columns.min(MAX_WIDTH));
    let mut out = io::BufWriter::new(io::stdout().lock());
    let print_once = |source: &str, out: &mut io::BufWriter<io::StdoutLock>| -> io::Result<()> {
        for line in render(source, width, columns).lines {
            writeln!(out, "{}", line.to_ansi(&theme))?;
        }
        out.flush()
    };
    if !cli.watch {
        return print_once(&source, &mut out);
    }
    let mut watcher = Watcher::new(cli.file.as_deref().expect("--watch는 위에서 파일 경로 유무를 이미 검증함"));
    let mut current = source;
    print_once(&current, &mut out)?;
    loop {
        std::thread::sleep(watch::POLL_INTERVAL);
        match watcher.poll() {
            PollResult::Unchanged => {}
            PollResult::Changed(new_source) => {
                current = new_source;
                print_once(&current, &mut out)?;
            }
            PollResult::ReadError(message) => eprintln!("dg: 감시 불가: {message}"),
        }
    }
}

fn read_input(path: Option<&str>) -> io::Result<(String, String)> {
    match path {
        Some("-") | None => {
            if io::stdin().is_terminal() {
                return Err(io::Error::other("파일을 지정하거나 표준입력으로 넘겨 주세요 (dg --help)"));
            }
            let mut source = String::new();
            io::stdin().read_to_string(&mut source)?;
            Ok((source, "stdin".to_string()))
        }
        Some(file) => {
            let source = std::fs::read_to_string(file).map_err(|e| io::Error::new(e.kind(), format!("{file}: {e}")))?;
            Ok((source, file.to_string()))
        }
    }
}

fn looks_like_diagram_file(path: Option<&str>) -> bool {
    path.is_some_and(|p| {
        let lower = p.to_ascii_lowercase();
        [".puml", ".plantuml", ".pu", ".iuml", ".mmd", ".mermaid"].iter().any(|ext| lower.ends_with(ext))
    })
}

fn pick_theme(style: StyleArg, stdout_is_tty: bool) -> Theme {
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return Theme::none();
    }
    let style = match style {
        StyleArg::Auto => std::env::var("DG_STYLE").ok().and_then(|v| parse_style(&v)).unwrap_or(StyleArg::Auto),
        explicit => explicit,
    };
    match style {
        StyleArg::Dark => Theme::dark(),
        StyleArg::Light => Theme::light(),
        StyleArg::None => Theme::none(),
        StyleArg::Auto => {
            if !stdout_is_tty {
                Theme::none()
            } else if looks_light_background() {
                Theme::light()
            } else {
                Theme::dark()
            }
        }
    }
}

fn parse_style(value: &str) -> Option<StyleArg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "dark" => Some(StyleArg::Dark),
        "light" => Some(StyleArg::Light),
        "none" | "notty" | "no-color" => Some(StyleArg::None),
        "auto" => Some(StyleArg::Auto),
        _ => None,
    }
}

/// `COLORFGBG`("fg;bg")로 밝은 배경을 추정한다.
fn looks_light_background() -> bool {
    std::env::var("COLORFGBG")
        .ok()
        .and_then(|v| v.rsplit(';').next().and_then(|bg| bg.parse::<u8>().ok()))
        .is_some_and(|bg| bg == 7 || bg == 15)
}
