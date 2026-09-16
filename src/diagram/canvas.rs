//! 문자 격자. 선을 그으면 이미 있는 선과 이음 문자로 합쳐진다.

use crate::line::Line;
use crate::style::Style;
use crate::text::{char_width, width_of};

pub const NORTH: u8 = 1;
pub const SOUTH: u8 = 2;
pub const WEST: u8 = 4;
pub const EAST: u8 = 8;
/// 간선이 다른 간선을 건너뛰는 칸의 글자(반원 돌출).
const HOP: char = '◠';

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineKind {
    #[default]
    Solid,
    Dashed,
    Heavy,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    ch: char,
    style: Style,
    /// 이 칸을 지나는 선의 방향 비트. 0이면 글자 칸이다.
    lines: u8,
    dashed: bool,
    heavy: bool,
    round: bool,
    /// 넓은 글자(한글 등)의 오른쪽 절반.
    continuation: bool,
    /// 간선 모드에서 그린 선(테두리·구분선이 아님).
    is_edge: bool,
    /// 간선끼리 직교하는 칸. `┼` 대신 건너뛰기 표시로 그린다.
    is_hop: bool,
}

impl Cell {
    const BLANK: Cell = Cell {
        ch: ' ',
        style: Style::PLAIN,
        lines: 0,
        dashed: false,
        heavy: false,
        round: false,
        continuation: false,
        is_edge: false,
        is_hop: false,
    };
    fn is_text(&self) -> bool {
        self.lines == 0 && self.ch != ' '
    }
}

pub struct Canvas {
    width: usize,
    cells: Vec<Vec<Cell>>,
    /// 켜져 있으면 지금 긋는 선은 간선이며, 다른 간선과 직교할 때 건너뛰기로 그린다.
    edge_mode: bool,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        Canvas { width, cells: vec![vec![Cell::BLANK; width]; height], edge_mode: false }
    }

    /// 간선 모드 전환. 테두리·상자·구분선은 끄고, 간선·생명선은 켜고 긋는다.
    pub fn set_edge_mode(&mut self, on: bool) {
        self.edge_mode = on;
    }

    fn ensure(&mut self, x: usize, y: usize) -> bool {
        if x >= self.width {
            return false;
        }
        while self.cells.len() <= y {
            self.cells.push(vec![Cell::BLANK; self.width]);
        }
        true
    }

    /// 글자 하나를 놓는다. 선 정보는 지운다.
    pub fn put(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if !self.ensure(x, y) {
            return;
        }
        let w = char_width(ch).max(1);
        if x + w > self.width {
            return;
        }
        self.cells[y][x] = Cell { ch, style, ..Cell::BLANK };
        if w == 2 {
            self.cells[y][x + 1] = Cell { ch: ' ', style, continuation: true, ..Cell::BLANK };
        }
    }

    /// 글자열을 놓는다. 돌려주는 값은 쓴 폭.
    pub fn text(&mut self, x: usize, y: usize, s: &str, style: Style) -> usize {
        let mut cursor = x;
        for c in s.chars() {
            let w = char_width(c);
            if w == 0 {
                continue;
            }
            if cursor + w > self.width {
                break;
            }
            self.put(cursor, y, c, style);
            cursor += w;
        }
        cursor - x
    }

    /// 넓이 `w` 안에 가운데 맞춰 글자열을 놓는다.
    pub fn text_centered(&mut self, x: usize, y: usize, w: usize, s: &str, style: Style) {
        let tw = width_of(s);
        let offset = w.saturating_sub(tw) / 2;
        self.text(x + offset, y, s, style);
    }

    fn add_line(&mut self, x: usize, y: usize, bits: u8, kind: LineKind, style: Style, round: bool) {
        if !self.ensure(x, y) {
            return;
        }
        let cell = &mut self.cells[y][x];
        if cell.is_text() || cell.continuation {
            return;
        }
        if self.edge_mode && cell.is_edge && bits != 0 {
            let vertical = NORTH | SOUTH;
            let horizontal = WEST | EAST;
            if (cell.lines == vertical && bits == horizontal) || (cell.lines == horizontal && bits == vertical) {
                cell.is_hop = true;
            }
        }
        if self.edge_mode && bits != 0 {
            cell.is_edge = true;
        }
        cell.lines |= bits;
        cell.style = style;
        cell.round = cell.round || round;
        match kind {
            LineKind::Dashed => cell.dashed = true,
            LineKind::Heavy => cell.heavy = true,
            LineKind::Solid => {}
        }
        cell.ch = if cell.is_hop { HOP } else { line_char(cell.lines, cell.dashed, cell.heavy, cell.round) };
    }

    /// 가로선. 양 끝은 안쪽 방향 비트만 갖는다.
    pub fn hline(&mut self, x0: usize, x1: usize, y: usize, kind: LineKind, style: Style) {
        let (a, b) = (x0.min(x1), x0.max(x1));
        for x in a..=b {
            let mut bits = 0;
            if x > a {
                bits |= WEST;
            }
            if x < b {
                bits |= EAST;
            }
            if bits == 0 {
                bits = WEST | EAST;
            }
            self.add_line(x, y, bits, kind, style, false);
        }
    }

    pub fn vline(&mut self, x: usize, y0: usize, y1: usize, kind: LineKind, style: Style) {
        let (a, b) = (y0.min(y1), y0.max(y1));
        for y in a..=b {
            let mut bits = 0;
            if y > a {
                bits |= NORTH;
            }
            if y < b {
                bits |= SOUTH;
            }
            if bits == 0 {
                bits = NORTH | SOUTH;
            }
            self.add_line(x, y, bits, kind, style, false);
        }
    }

    /// 한 칸에 방향 비트를 더한다(모서리·분기 만들기).
    pub fn join(&mut self, x: usize, y: usize, bits: u8, kind: LineKind, style: Style, round: bool) {
        self.add_line(x, y, bits, kind, style, round);
    }

    /// 테두리 사각형. `round`면 모서리를 둥글게.
    pub fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, kind: LineKind, style: Style, round: bool) {
        if w < 2 || h < 2 {
            return;
        }
        let (x1, y1) = (x + w - 1, y + h - 1);
        self.hline(x, x1, y, kind, style);
        self.hline(x, x1, y1, kind, style);
        self.vline(x, y, y1, kind, style);
        self.vline(x1, y, y1, kind, style);
        if round {
            for (cx, cy) in [(x, y), (x1, y), (x, y1), (x1, y1)] {
                if let Some(cell) = self.cells.get_mut(cy).and_then(|r| r.get_mut(cx)) {
                    cell.round = true;
                    cell.ch = line_char(cell.lines, cell.dashed, cell.heavy, true);
                }
            }
        }
    }

    /// 영역을 지운다.
    pub fn clear_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for yy in y..y + h {
            for xx in x..x + w {
                if self.ensure(xx, yy) {
                    self.cells[yy][xx] = Cell::BLANK;
                }
            }
        }
    }

    pub fn into_lines(self) -> Vec<Line> {
        self.cells
            .into_iter()
            .map(|row| {
                let mut line = Line::empty();
                let mut end = row.len();
                while end > 0 && row[end - 1].ch == ' ' && !row[end - 1].continuation {
                    end -= 1;
                }
                let mut buffer = [0u8; 4];
                for cell in &row[..end] {
                    if cell.continuation {
                        continue;
                    }
                    line.push_str(cell.ch.encode_utf8(&mut buffer), cell.style);
                }
                line
            })
            .collect()
    }
}

fn line_char(bits: u8, dashed: bool, heavy: bool, round: bool) -> char {
    let n = bits & NORTH != 0;
    let s = bits & SOUTH != 0;
    let w = bits & WEST != 0;
    let e = bits & EAST != 0;
    match (n, s, w, e) {
        (false, false, false, false) => ' ',
        (true, false, false, false) | (false, true, false, false) | (true, true, false, false) => {
            if dashed {
                '╎'
            } else if heavy {
                '┃'
            } else {
                '│'
            }
        }
        (false, false, true, false) | (false, false, false, true) | (false, false, true, true) => {
            if dashed {
                '╌'
            } else if heavy {
                '━'
            } else {
                '─'
            }
        }
        (true, false, true, false) => if round { '╯' } else if heavy { '┛' } else { '┘' },
        (true, false, false, true) => if round { '╰' } else if heavy { '┗' } else { '└' },
        (false, true, true, false) => if round { '╮' } else if heavy { '┓' } else { '┐' },
        (false, true, false, true) => if round { '╭' } else if heavy { '┏' } else { '┌' },
        (true, true, true, false) => if heavy { '┫' } else { '┤' },
        (true, true, false, true) => if heavy { '┣' } else { '├' },
        (true, false, true, true) => if heavy { '┻' } else { '┴' },
        (false, true, true, true) => if heavy { '┳' } else { '┬' },
        (true, true, true, true) => if heavy { '╋' } else { '┼' },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(canvas: Canvas) -> Vec<String> {
        canvas.into_lines().iter().map(Line::plain).collect()
    }

    #[test]
    fn crossing_lines_join() {
        let mut c = Canvas::new(5, 3);
        c.hline(0, 4, 1, LineKind::Solid, Style::PLAIN);
        c.vline(2, 0, 2, LineKind::Solid, Style::PLAIN);
        assert_eq!(rows(c), vec!["  │", "──┼──", "  │"]);
    }

    #[test]
    fn edges_hop_over_edges_but_cross_borders() {
        let mut c = Canvas::new(5, 4);
        c.set_edge_mode(true);
        c.vline(2, 0, 3, LineKind::Solid, Style::PLAIN);
        c.hline(0, 4, 1, LineKind::Solid, Style::PLAIN);
        c.set_edge_mode(false);
        c.hline(0, 4, 2, LineKind::Solid, Style::PLAIN);
        assert_eq!(rows(c), vec!["  │", "──◠──", "──┼──", "  │"]);
    }

    #[test]
    fn rect_has_corners_and_round_variant() {
        let mut c = Canvas::new(4, 2);
        c.rect(0, 0, 4, 2, LineKind::Solid, Style::PLAIN, true);
        assert_eq!(rows(c), vec!["╭──╮", "╰──╯"]);
    }

    #[test]
    fn text_blocks_lines_and_wide_chars_take_two_cells() {
        let mut c = Canvas::new(6, 1);
        c.text(0, 0, "한a", Style::PLAIN);
        c.hline(0, 5, 0, LineKind::Solid, Style::PLAIN);
        assert_eq!(rows(c), vec!["한a───"]);
    }
}
