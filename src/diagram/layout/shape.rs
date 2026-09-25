//! 노드 도형의 크기 계산과 그리기.

use crate::diagram::canvas::{Canvas, LineKind, NORTH, SOUTH};
use crate::diagram::ir::{EventPosition, Node, Shape};
use crate::style::{Style, Theme};
use crate::text::{width_of, wrap_plain};

/// 라벨 폭 상한에 맞춰 줄바꿈한 본문.
pub fn wrapped_sections(node: &Node, cap: usize) -> Vec<Vec<String>> {
    node.sections
        .iter()
        .enumerate()
        .filter(|(index, section)| *index == 0 || !section.is_empty())
        .map(|(_, section)| section.iter().flat_map(|line| wrap_plain(line, cap)).collect())
        .collect()
}

fn text_size(sections: &[Vec<String>]) -> (usize, usize) {
    let width = sections.iter().flatten().map(|s| width_of(s)).max().unwrap_or(0);
    let lines: usize = sections.iter().map(Vec::len).sum();
    let separators = sections.len().saturating_sub(1);
    (width, lines + separators)
}

/// (폭, 높이)
pub fn measure(shape: Shape, sections: &[Vec<String>]) -> (usize, usize) {
    let (tw, th) = text_size(sections);
    match shape {
        Shape::Start | Shape::End | Shape::Anchor => (1, 1),
        Shape::Rect | Shape::Round | Shape::Note | Shape::Circle | Shape::Subprocess => (tw + 4, th + 2),
        // 테두리·공백·위치 글자·공백·본문·공백·테두리. 본문이 비어도 한 줄은 확보한다(1.6).
        Shape::Event(_) => (tw + 6, th.max(1) + 2),
        Shape::Stadium | Shape::Diamond | Shape::Hexagon | Shape::Subroutine => (tw + 6, th + 2),
        Shape::Cylinder => (tw + 4, th + 3),
        Shape::Actor => (tw.max(3), th + 3),
        Shape::Interface => (tw.max(1), th + 1),
        Shape::Plain => (tw, th),
    }
}

/// `min_height`보다 낮으면 위아래를 늘려 본문을 세로 가운데에 둔다.
/// `min_width`·`min_height`보다 작으면 늘려 그린다(접점·라벨 자리 확보용). 본문은 가운데에 둔다.
pub fn draw(canvas: &mut Canvas, x: usize, y: usize, shape: Shape, sections: &[Vec<String>], theme: &Theme, min_width: usize, min_height: usize) {
    let (measured_width, measured) = measure(shape, sections);
    let w = if matches!(shape, Shape::Start | Shape::End | Shape::Anchor) { measured_width } else { measured_width.max(min_width) };
    let h = measured.max(min_height);
    let extra_top = (h - measured) / 2;
    canvas.clear_rect(x, y, w, h);
    let border = theme.diagram_box;
    match shape {
        Shape::Anchor => {}
        Shape::Start => canvas.put(x, y, '●', theme.diagram_accent),
        Shape::End => canvas.put(x, y, '◉', theme.diagram_accent),
        Shape::Rect | Shape::Round | Shape::Circle | Shape::Note => {
            let kind = if shape == Shape::Note { LineKind::Dashed } else { LineKind::Solid };
            canvas.rect(x, y, w, h, kind, border, shape != Shape::Rect && shape != Shape::Note);
            draw_sections(canvas, x, y + 1 + extra_top, w, sections, theme, true);
        }
        Shape::Subprocess => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, true);
            draw_sections(canvas, x, y + 1 + extra_top, w, sections, theme, true);
            // 본문을 그린 뒤 아래 테두리 가운데를 글자 칸으로 덮어쓴다 — 이후 add_line()은
            // 글자 칸(is_text())을 건너뛰므로 배치기가 여기로 선을 그어도 `[+]`가 남는다(2.2).
            canvas.text_centered(x, y + h - 1, w, "[+]", border);
        }
        Shape::Event(position) => {
            let round = position != EventPosition::End;
            let kind = if round { LineKind::Solid } else { LineKind::Heavy };
            canvas.rect(x, y, w, h, kind, border, round);
            let row0 = y + 1 + extra_top;
            let glyph = match position {
                EventPosition::Start => '○',
                EventPosition::Intermediate => '◎',
                EventPosition::End => '●',
            };
            canvas.put(x + 2, row0, glyph, theme.diagram_accent);
            draw_sections(canvas, x + 2, row0, w - 2, sections, theme, false);
        }
        Shape::Stadium => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, true);
            draw_sections(canvas, x, y + 1 + extra_top, w, sections, theme, true);
        }
        Shape::Diamond | Shape::Hexagon => {
            canvas.hline(x + 2, x + w - 3, y, LineKind::Solid, border);
            canvas.hline(x + 2, x + w - 3, y + h - 1, LineKind::Solid, border);
            canvas.put(x + 1, y, '╱', border);
            canvas.put(x + w - 2, y, '╲', border);
            canvas.put(x + 1, y + h - 1, '╲', border);
            canvas.put(x + w - 2, y + h - 1, '╱', border);
            // 옆면 글자로 판단(마름모)과 예비 단계(육각형)를 구분한다 — 파서는 `{`(Diamond)와
            // `{{`(Hexagon)를 이미 구분해 넘기는데, 이 두 글자만 같으면 그려질 땐 똑같아 보였다.
            // 육각형은 모서리를 깎은 직사각형처럼 곧은 세로선을, 마름모는 좀 더 각진 꺾쇠를 쓴다.
            let (left, right) = if shape == Shape::Hexagon { ('│', '│') } else { ('⟨', '⟩') };
            for row in y + 1..y + h - 1 {
                canvas.put(x, row, left, border);
                canvas.put(x + w - 1, row, right, border);
            }
            draw_sections(canvas, x + 1, y + 1 + extra_top, w - 2, sections, theme, false);
        }
        Shape::Subroutine => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, false);
            for row in y + 1..y + h - 1 {
                canvas.put(x + 1, row, '│', border);
                canvas.put(x + w - 2, row, '│', border);
            }
            draw_sections(canvas, x + 1, y + 1 + extra_top, w - 2, sections, theme, false);
        }
        Shape::Cylinder => {
            canvas.rect(x, y, w, h, LineKind::Solid, border, true);
            canvas.hline(x, x + w - 1, y + 1, LineKind::Solid, border);
            draw_sections(canvas, x, y + 2 + extra_top, w, sections, theme, true);
        }
        Shape::Actor => {
            let mid = x + w / 2;
            canvas.put(mid, y, '○', border);
            canvas.put(mid.saturating_sub(1), y + 1, '╱', border);
            canvas.put(mid, y + 1, '│', border);
            canvas.put(mid + 1, y + 1, '╲', border);
            canvas.put(mid.saturating_sub(1), y + 2, '╱', border);
            canvas.put(mid + 1, y + 2, '╲', border);
            draw_sections(canvas, x, y + 3, w, sections, theme, false);
        }
        Shape::Interface => {
            canvas.put(x + w / 2, y, '○', border);
            draw_sections(canvas, x, y + 1, w, sections, theme, false);
        }
        Shape::Plain => {
            // draw_sections()의 x+1/w-2 관례는 테두리 한 칸을 전제하는데, Plain은
            // 테두리가 아예 없어 그대로 쓰면 앞뒤 여백이 비대칭으로 남는다(뒤쪽
            // 빈 칸은 출력 시 잘리지만 앞쪽은 남음) — 직접 채운다.
            let mut row = y + extra_top;
            for section in sections {
                for text in section {
                    canvas.text_centered(x, row, w, text, line_style(0, text, theme));
                    row += 1;
                }
            }
        }
    }
}

/// 칸 사이에 구분선을 넣어 가며 본문을 쓴다. `x`는 테두리 칸, 내용은 `x+2`부터.
fn draw_sections(canvas: &mut Canvas, x: usize, y: usize, w: usize, sections: &[Vec<String>], theme: &Theme, separators: bool) {
    let mut row = y;
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            if separators {
                canvas.hline(x, x + w - 1, row, LineKind::Solid, theme.diagram_box);
                canvas.join(x, row, NORTH | SOUTH, LineKind::Solid, theme.diagram_box, false);
                canvas.join(x + w - 1, row, NORTH | SOUTH, LineKind::Solid, theme.diagram_box, false);
            }
            row += 1;
        }
        for text in section {
            let style = line_style(index, text, theme);
            if index == 0 {
                canvas.text_centered(x + 1, row, w - 2, text, style);
            } else {
                canvas.text(x + 2, row, text, style);
            }
            row += 1;
        }
    }
}

fn line_style(section: usize, text: &str, theme: &Theme) -> Style {
    if text.starts_with('«') {
        theme.diagram_text.dim().italic()
    } else if section == 0 {
        theme.diagram_text.bold()
    } else {
        theme.diagram_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;

    fn draw_rows(shape: Shape, sections: Vec<Vec<String>>) -> Vec<String> {
        let (w, h) = measure(shape, &sections);
        let mut canvas = Canvas::new(w, h);
        draw(&mut canvas, 0, 0, shape, &sections, &Theme::none(), 0, 0);
        canvas.into_lines().iter().map(Line::plain).collect()
    }

    /// `draw_rows`와 같되 `Line::plain()`으로 버리는 스타일(굵음·흐림·기울임)을 함께 돌려준다.
    fn draw_runs(shape: Shape, sections: Vec<Vec<String>>) -> Vec<Vec<(String, Style)>> {
        let (w, h) = measure(shape, &sections);
        let mut canvas = Canvas::new(w, h);
        draw(&mut canvas, 0, 0, shape, &sections, &Theme::none(), 0, 0);
        canvas.into_lines().iter().map(|line| line.runs().map(|(text, style)| (text.to_string(), style)).collect()).collect()
    }

    #[test]
    fn rect_with_two_sections() {
        let rows = draw_rows(Shape::Rect, vec![vec!["User".into()], vec!["+id".into(), "+name".into()]]);
        assert_eq!(rows, vec!["┌───────┐", "│ User  │", "├───────┤", "│ +id   │", "│ +name │", "└───────┘"]);
    }

    #[test]
    fn cylinder_has_lid() {
        let rows = draw_rows(Shape::Cylinder, vec![vec!["db".into()]]);
        assert_eq!(rows, vec!["╭────╮", "├────┤", "│ db │", "╰────╯"]);
    }

    /// mermaid는 `{ }`(판단, Diamond)와 `{{ }}`(예비 단계, Hexagon)를 서로 다른 모양으로
    /// 구분해 파싱하는데, 그리는 쪽이 같은 글자를 재사용하면 그 구분이 사라져 버렸다.
    /// 옆면 글자만이라도 달라야 두 모양이 눈으로 구분된다.
    #[test]
    fn diamond_and_hexagon_render_differently() {
        let diamond = draw_rows(Shape::Diamond, vec![vec!["x".into()]]);
        let hexagon = draw_rows(Shape::Hexagon, vec![vec!["x".into()]]);
        assert_eq!(diamond, vec![" ╱───╲", "⟨  x  ⟩", " ╲───╱"]);
        assert_eq!(hexagon, vec![" ╱───╲", "│  x  │", " ╲───╱"]);
        assert_ne!(diamond, hexagon, "판단(Diamond)과 예비 단계(Hexagon)가 같은 모양으로 그려지면 안 된다");
    }

    /// (plantuml-wbs) `Shape::Plain`은 테두리 없이 글자만 그려야 한다(PlantUML WBS의
    /// `_` 접미사 노드).
    #[test]
    fn plain_has_no_border() {
        let rows = draw_rows(Shape::Plain, vec![vec!["leaf".into()]]);
        assert_eq!(rows, vec!["leaf"]);
        for row in &rows {
            assert!(!row.contains(['┌', '┐', '└', '┘', '│', '─']), "테두리 문자가 없어야 한다: {row:?}");
        }
    }

    #[test]
    fn plain_supports_multiple_lines() {
        let rows = draw_rows(Shape::Plain, vec![vec!["one".into(), "two".into()]]);
        assert_eq!(rows, vec!["one", "two"]);
    }

    /// (bpmn-shapes) 태스크(`Round`)의 이름 아래 둘째 줄이 `«user»`처럼 `«`로 시작하면
    /// 흐리고 기울여 그려지고, 이름 줄은 굵게 유지된다(요구사항 3.2, 3.4 — 태스크는
    /// 종류별 새 모양 없이 이 둘째 줄로만 구분된다).
    #[test]
    fn task_stereotype_second_line_is_dim_and_italic_while_name_stays_bold() {
        let runs = draw_runs(Shape::Round, vec![vec!["주문 검토".into(), "«user»".into()]]);
        let name_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("주문 검토"))).expect("이름 줄이 있어야 한다");
        let stereotype_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("«user»"))).expect("둘째 줄이 있어야 한다");
        assert!(name_row.iter().any(|(text, style)| text.contains("주문 검토") && style.bold && !style.dim && !style.italic));
        assert!(stereotype_row.iter().any(|(text, style)| text.contains("«user»") && style.dim && style.italic));
    }

    /// (bpmn-shapes) 둘째 줄이 없으면 자리도 비지 않고 이름 한 줄 높이로 그려진다(요구사항 3.3).
    #[test]
    fn task_single_line_body_has_name_line_height() {
        let (_, h) = measure(Shape::Round, &[vec!["주문 검토".into()]]);
        assert_eq!(h, 3, "테두리 두 줄 + 이름 한 줄");
    }

    /// (bpmn-shapes) 게이트웨이 다섯 라벨이 마름모 가운데 줄에 원문 그대로 그려지고, 기호가
    /// `«`로 시작하지 않으므로 흐려지지 않는다(요구사항 5.1 — 코드 변경 없이 기존 `Diamond` +
    /// 임의 라벨로 성립함을 고정한다).
    #[test]
    fn gateway_symbol_labels_render_verbatim_and_symbols_are_not_dimmed() {
        for label in ["× 승인?", "+ 병렬", "○ 선택", "* 복합", "◎ 이벤트"] {
            let rows = draw_rows(Shape::Diamond, vec![vec![label.to_string()]]);
            assert!(rows.iter().any(|row| row.contains(label)), "라벨 원문이 그대로 있어야 한다: {label:?} in {rows:?}");
            let runs = draw_runs(Shape::Diamond, vec![vec![label.to_string()]]);
            let symbol_run = runs.iter().flatten().find(|(text, _)| text.contains(label)).expect("라벨 run이 있어야 한다");
            assert!(!symbol_run.1.dim && !symbol_run.1.italic, "게이트웨이 기호는 흐리거나 기울지 않아야 한다: {label:?}");
        }
    }

    /// (bpmn-shapes) 게이트웨이 기호 다섯 개·이벤트 위치 글자 세 개·빗금이 기본 CJK 모노
    /// 폰트(Noto Sans Mono CJK) 커버리지 실측 안에 있는지 고정한다(요구사항 5.2,
    /// research.md "글자 커버리지 실측"). `⬠`(U+2B20)·`∗`(U+2217)는 커버리지 밖이라
    /// design §Key Decisions에서 `◎`·`*`로 교체됐다 — 그 교체 결과만 허용 목록에 남는다.
    #[test]
    fn bpmn_glyph_codepoints_are_within_cjk_mono_coverage() {
        const COVERED: [char; 7] = ['○', '●', '◎', '╱', '×', '+', '*'];
        let gateway_symbols = ['×', '+', '○', '*', '◎'];
        let event_position_glyphs = ['○', '◎', '●'];
        let slash = ['╱'];
        for ch in gateway_symbols.into_iter().chain(event_position_glyphs).chain(slash) {
            assert!(COVERED.contains(&ch), "{ch:?}(U+{:04X})가 커버리지 허용 목록 안에 있어야 한다", ch as u32);
        }
        for excluded in ['⬠', '∗'] {
            assert!(!COVERED.contains(&excluded), "{excluded:?}는 커버리지 밖이라 교체됐다");
        }
    }

    /// (bpmn-shapes) 시작·중간·종료 이벤트의 위치 글자와 테두리(요구사항 1.1~1.4).
    #[test]
    fn event_start_intermediate_end_render_with_position_glyphs() {
        let start = draw_rows(Shape::Event(EventPosition::Start), vec![vec!["Go".into()]]);
        assert_eq!(start, vec!["╭──────╮", "│ ○ Go │", "╰──────╯"]);

        let intermediate = draw_rows(Shape::Event(EventPosition::Intermediate), vec![vec!["Go".into()]]);
        assert_eq!(intermediate, vec!["╭──────╮", "│ ◎ Go │", "╰──────╯"]);

        let end = draw_rows(Shape::Event(EventPosition::End), vec![vec!["Go".into()]]);
        assert_eq!(end, vec!["┏━━━━━━┓", "┃ ● Go ┃", "┗━━━━━━┛"]);

        let (_, event_height) = measure(Shape::Event(EventPosition::Start), &[vec!["Go".into()]]);
        let (_, round_height) = measure(Shape::Round, &[vec!["Go".into()]]);
        assert_eq!(event_height, round_height, "위치 글자가 줄을 추가하지 않는다(1.4) — 이름 줄 수가 같은 기존 둥근 상자와 높이가 같아야 한다");
    }

    /// (bpmn-shapes) 이름이 없는 이벤트는 위치 글자만 든 한 줄 높이 상자로, 패닉 없이 그려진다(요구사항 1.6).
    #[test]
    fn event_with_empty_body_has_one_line_height_and_does_not_panic() {
        let rows = draw_rows(Shape::Event(EventPosition::Start), vec![vec![]]);
        assert_eq!(rows, vec!["╭────╮", "│ ○  │", "╰────╯"]);
    }

    /// (bpmn-shapes) 이벤트 둘째 줄 `«timer»`도 태스크와 같은 흐림·기울임 규칙을 따르고,
    /// 한 줄 본문보다 높이가 하나 더 크다(요구사항 3.1, 3.3).
    #[test]
    fn event_stereotype_second_line_is_dim_and_italic() {
        let runs = draw_runs(Shape::Event(EventPosition::Start), vec![vec!["Go".into(), "«timer»".into()]]);
        assert_eq!(runs.len(), 4, "테두리 두 줄 + 이름·둘째 줄");
        let name_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("Go"))).expect("이름 줄이 있어야 한다");
        let stereotype_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("«timer»"))).expect("둘째 줄이 있어야 한다");
        assert!(name_row.iter().any(|(text, style)| text.contains("Go") && style.bold && !style.dim));
        assert!(stereotype_row.iter().any(|(text, style)| text.contains("«timer»") && style.dim && style.italic));

        let (_, single_line_height) = measure(Shape::Event(EventPosition::Start), &[vec!["Go".into()]]);
        assert_eq!(single_line_height, 3);
    }

    /// (bpmn-shapes) 접힌 서브프로세스: 테두리·본문은 `Round`와 같고, 아래 테두리 가운데에
    /// `[+]`가 글자로 덮여 그려진다(요구사항 2.1).
    #[test]
    fn subprocess_marks_plus_on_bottom_border() {
        let rows = draw_rows(Shape::Subprocess, vec![vec!["Sub".into()]]);
        assert_eq!(rows, vec!["╭─────╮", "│ Sub │", "╰─[+]─╯"]);
    }

    /// (bpmn-shapes) 배치기가 노드를 측정값보다 넓게 늘려도 `[+]`는 늘어난 아래 테두리의
    /// 가운데에 온다(요구사항 2.3).
    #[test]
    fn subprocess_plus_stays_centered_when_stretched_by_min_width() {
        let sections = vec![vec!["Sub".into()]];
        let (measured_width, measured_height) = measure(Shape::Subprocess, &sections);
        let min_width = measured_width + 4;
        let mut canvas = Canvas::new(min_width, measured_height);
        draw(&mut canvas, 0, 0, Shape::Subprocess, &sections, &Theme::none(), min_width, 0);
        let rows: Vec<String> = canvas.into_lines().iter().map(Line::plain).collect();
        let bottom = rows.last().expect("아래 테두리 줄이 있어야 한다");
        let chars: Vec<char> = bottom.chars().collect();
        let plus_start = chars.iter().position(|&c| c == '[').expect("[+]가 있어야 한다");
        let plus_end = chars.iter().position(|&c| c == ']').expect("[+]가 있어야 한다");
        let left = plus_start - 1; // 왼쪽 모서리 글자 한 칸 제외
        let right = (chars.len() - 1) - (plus_end + 1); // 오른쪽 모서리 글자 한 칸 제외
        assert!(left.abs_diff(right) <= 1, "왼쪽·오른쪽 `─` 수가 같거나 1 차이여야 한다: {bottom:?} (left={left}, right={right})");
    }

    /// (bpmn-shapes) 서브프로세스 둘째 줄 `«call»`도 태스크·이벤트와 같은 흐림·기울임 규칙을 따른다(요구사항 3.2).
    #[test]
    fn subprocess_stereotype_second_line_is_dim_and_italic() {
        let runs = draw_runs(Shape::Subprocess, vec![vec!["Sub".into(), "«call»".into()]]);
        let stereotype_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("«call»"))).expect("둘째 줄이 있어야 한다");
        assert!(stereotype_row.iter().any(|(text, style)| text.contains("«call»") && style.dim && style.italic));
    }
}
