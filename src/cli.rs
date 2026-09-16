use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "dg", version, about = "터미널 마크다운 뷰어 — mermaid·PlantUML 다이어그램을 문자로 그린다")]
pub struct Cli {
    /// 마크다운 파일 (없거나 `-`이면 표준입력)
    pub file: Option<String>,

    /// 폭. 기본은 문단 줄바꿈 120까지·다이어그램/표/코드는 터미널 폭이며, 주면 둘 다 이 값
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

    /// ERD 카디널리티 표기: 까치발(crow, 기본)·글자(text)·둘 다(both). 소스 안 지시자가 우선
    #[arg(long, value_enum)]
    pub er_notation: Option<ErNotationArg>,

    /// 그래프 배치 방향 강제(tb: 위→아래, lr: 왼쪽→오른쪽). 폭에 안 들어가면 반대 방향으로 재시도
    #[arg(long, value_enum)]
    pub direction: Option<DirectionArg>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum DirectionArg {
    Tb,
    Lr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ErNotationArg {
    Crow,
    Text,
    Both,
}

impl From<ErNotationArg> for crate::diagram::ErNotation {
    fn from(value: ErNotationArg) -> Self {
        match value {
            ErNotationArg::Crow => Self::Crow,
            ErNotationArg::Text => Self::Text,
            ErNotationArg::Both => Self::Both,
        }
    }
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
