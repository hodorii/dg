use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "dg", version, about = "터미널 마크다운 뷰어 — mermaid·PlantUML 다이어그램을 문자로 그린다")]
pub struct Cli {
    /// 마크다운 파일 (없거나 `-`이면 표준입력)
    pub file: Option<String>,

    /// 줄바꿈 폭 (기본: 터미널 폭, 최대 120)
    #[arg(short, long)]
    pub width: Option<usize>,

    /// 색 테마
    #[arg(short, long, value_enum, default_value_t = StyleArg::Auto)]
    pub style: StyleArg,

    /// 페이저 없이 그대로 출력
    #[arg(short = 'P', long, alias = "no-pager")]
    pub print: bool,

    /// 입력을 다이어그램 소스로 취급 (.puml/.mmd는 자동)
    #[arg(short, long)]
    pub diagram: bool,

    /// 다이어그램 언어를 지정 (자동 판별 대신)
    #[arg(short, long, value_enum)]
    pub lang: Option<LangArg>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum StyleArg {
    Auto,
    Dark,
    Light,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LangArg {
    Mermaid,
    Plantuml,
}
