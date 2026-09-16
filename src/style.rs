//! 글자 스타일과 테마, ANSI 출력.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub reverse: bool,
}

impl Style {
    pub const PLAIN: Style = Style {
        fg: Color::Default,
        bg: Color::Default,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        strike: false,
        reverse: false,
    };

    pub const fn fg(index: u8) -> Style {
        Style { fg: Color::Indexed(index), ..Style::PLAIN }
    }
    pub const fn bold(mut self) -> Style {
        self.bold = true;
        self
    }
    pub const fn dim(mut self) -> Style {
        self.dim = true;
        self
    }
    pub const fn italic(mut self) -> Style {
        self.italic = true;
        self
    }
    pub const fn underline(mut self) -> Style {
        self.underline = true;
        self
    }
    pub const fn strike(mut self) -> Style {
        self.strike = true;
        self
    }
    pub const fn reverse(mut self) -> Style {
        self.reverse = true;
        self
    }
    pub const fn with_bg(mut self, index: u8) -> Style {
        self.bg = Color::Indexed(index);
        self
    }

    /// 두 스타일을 겹친다. `over`에 설정된 속성이 우선한다.
    pub fn merge(self, over: Style) -> Style {
        Style {
            fg: if over.fg == Color::Default { self.fg } else { over.fg },
            bg: if over.bg == Color::Default { self.bg } else { over.bg },
            bold: self.bold || over.bold,
            dim: self.dim || over.dim,
            italic: self.italic || over.italic,
            underline: self.underline || over.underline,
            strike: self.strike || over.strike,
            reverse: self.reverse || over.reverse,
        }
    }

    /// SGR 시퀀스. 색이 없는 테마에서는 빈 문자열이다.
    pub fn ansi(&self, enabled: bool) -> String {
        if !enabled || *self == Style::PLAIN {
            return String::new();
        }
        let mut codes: Vec<String> = Vec::new();
        if self.bold {
            codes.push("1".into());
        }
        if self.dim {
            codes.push("2".into());
        }
        if self.italic {
            codes.push("3".into());
        }
        if self.underline {
            codes.push("4".into());
        }
        if self.reverse {
            codes.push("7".into());
        }
        if self.strike {
            codes.push("9".into());
        }
        if let Color::Indexed(i) = self.fg {
            codes.push(format!("38;5;{i}"));
        }
        if let Color::Indexed(i) = self.bg {
            codes.push(format!("48;5;{i}"));
        }
        let mut out = String::new();
        let _ = write!(out, "\x1b[{}m", codes.join(";"));
        out
    }
}

/// 문서 요소별 스타일 모음.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub enabled: bool,
    pub text: Style,
    pub heading: [Style; 6],
    pub rule: Style,
    pub emphasis: Style,
    pub strong: Style,
    pub strike: Style,
    pub code: Style,
    pub code_block: Style,
    pub code_border: Style,
    pub code_lang: Style,
    pub link: Style,
    pub link_url: Style,
    pub image: Style,
    pub quote: Style,
    pub quote_bar: Style,
    pub bullet: Style,
    pub table_border: Style,
    pub table_head: Style,
    pub html: Style,
    pub diagram_caption: Style,
    pub diagram_line: Style,
    pub diagram_box: Style,
    pub diagram_text: Style,
    pub diagram_group: Style,
    pub diagram_label: Style,
    pub diagram_accent: Style,
    pub diagram_note: Style,
    pub search_hit: Style,
    pub status: Style,
}

impl Theme {
    /// 어두운 배경. 제목·링크는 파랑 계열 하나로 통일하고 나머지는 회색 명도로만 구분한다.
    pub fn dark() -> Theme {
        Theme {
            enabled: true,
            text: Style::PLAIN,
            heading: [
                Style::fg(117).bold(),
                Style::fg(111).bold(),
                Style::fg(153).bold(),
                Style::fg(252).bold(),
                Style::fg(250).bold(),
                Style::fg(248).bold(),
            ],
            rule: Style::fg(240),
            emphasis: Style::PLAIN.italic(),
            strong: Style::PLAIN.bold(),
            strike: Style::PLAIN.strike().dim(),
            code: Style::fg(180).with_bg(236),
            code_block: Style::fg(250),
            code_border: Style::fg(240),
            code_lang: Style::fg(244).italic(),
            link: Style::fg(111).underline(),
            link_url: Style::fg(244),
            image: Style::fg(244).italic(),
            quote: Style::fg(246).italic(),
            quote_bar: Style::fg(240),
            bullet: Style::fg(111),
            table_border: Style::fg(240),
            table_head: Style::PLAIN.bold(),
            html: Style::fg(243),
            diagram_caption: Style::fg(244).italic(),
            diagram_line: Style::fg(245),
            diagram_box: Style::fg(109),
            diagram_text: Style::fg(252),
            diagram_group: Style::fg(240),
            diagram_label: Style::fg(144),
            diagram_accent: Style::fg(111),
            diagram_note: Style::fg(144),
            search_hit: Style::PLAIN.reverse(),
            status: Style::fg(245).reverse(),
        }
    }

    /// 밝은 배경.
    pub fn light() -> Theme {
        Theme {
            enabled: true,
            text: Style::PLAIN,
            heading: [
                Style::fg(25).bold(),
                Style::fg(26).bold(),
                Style::fg(31).bold(),
                Style::fg(236).bold(),
                Style::fg(238).bold(),
                Style::fg(240).bold(),
            ],
            rule: Style::fg(250),
            emphasis: Style::PLAIN.italic(),
            strong: Style::PLAIN.bold(),
            strike: Style::PLAIN.strike().dim(),
            code: Style::fg(94).with_bg(254),
            code_block: Style::fg(236),
            code_border: Style::fg(250),
            code_lang: Style::fg(244).italic(),
            link: Style::fg(26).underline(),
            link_url: Style::fg(244),
            image: Style::fg(244).italic(),
            quote: Style::fg(242).italic(),
            quote_bar: Style::fg(250),
            bullet: Style::fg(26),
            table_border: Style::fg(250),
            table_head: Style::PLAIN.bold(),
            html: Style::fg(246),
            diagram_caption: Style::fg(244).italic(),
            diagram_line: Style::fg(244),
            diagram_box: Style::fg(24),
            diagram_text: Style::fg(235),
            diagram_group: Style::fg(250),
            diagram_label: Style::fg(94),
            diagram_accent: Style::fg(26),
            diagram_note: Style::fg(94),
            search_hit: Style::PLAIN.reverse(),
            status: Style::fg(244).reverse(),
        }
    }

    /// 색 없는 테마. 굵기·밑줄 같은 속성도 모두 끈다.
    pub fn none() -> Theme {
        let mut theme = Theme::dark();
        theme.enabled = false;
        theme
    }
}
