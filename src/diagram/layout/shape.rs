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
        // 테두리 없음 — 위치 글자 자체가 도형이다. 위치 글자·공백·본문 폭, 본문이 비어도
        // 높이는 한 줄 확보한다(2.4).
        Shape::Event(_) => (if tw == 0 { 1 } else { tw + 2 }, th.max(1)),
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
    // 닻은 아무것도 그리지 않으므로 자리도 지우지 않는다 — 지우면 그 칸을 지나던 그룹 구분선에 구멍이 난다.
    if shape == Shape::Anchor {
        return;
    }
    canvas.clear_rect(x, y, w, h);
    let border = theme.diagram_box;
    match shape {
        Shape::Anchor => unreachable!(),
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
            // 테두리 없음 — 위치 글자(○/◎/◉) 자체가 도형이고, 이름·둘째 줄은 그 오른쪽에
            // 왼쪽 맞춤으로 붙는다. `draw_sections()`의 가운데 정렬 관례는 양옆 테두리를
            // 전제하므로 쓰지 않는다(`Plain` 분기와 같은 이유). 배치기가 접점 벌림·형제 정렬로
            // `min_width`/`min_height`를 측정값보다 넓게 주더라도, 위치 글자는 항상 노드
            // 원점(`x`, `y`)에 그려진다 — 배치기의 접점·정렬 기준(`Shape::is_point_anchored`)과
            // 짝을 이루는 그리기 쪽 약속이다. 여분은 이름 뒤·아래로만 남는다(2.5).
            let glyph = match position {
                EventPosition::Start => '○',
                EventPosition::Intermediate => '◎',
                EventPosition::End => '◉',
            };
            canvas.put(x, y, glyph, theme.diagram_accent);
            let mut row = y;
            for section in sections {
                for text in section {
                    canvas.text(x + 2, row, text, line_style(0, text, theme));
                    row += 1;
                }
            }
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
            // 마름모 쪽은 `‹›`(U+2039/203A) — 기본 Noto Sans Mono CJK 커버리지 안, EAW N, 1칸
            // (research.md 실측); `⟨⟩`(U+27E8/27E9)는 그 커버리지 밖이라 폰트 폴백 위험이 있었다.
            let (left, right) = if shape == Shape::Hexagon { ('│', '│') } else { ('‹', '›') };
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

/// BPMN 렌더링이 그리는 ASCII·한글 밖 글자 중 기본 CJK 모노 폰트(Noto Sans Mono CJK) 커버리지를
/// 실측한(`fc-list ":family=Noto Sans Mono CJK KR:charset=<코드>"`) 허용 목록. 커버리지 고정 테스트와
/// BPMN 렌더링 글자 속성 테스트가 함께 참조한다.
#[cfg(test)]
pub(crate) const CJK_MONO_COVERED_GLYPHS: &[char] = &[
    '○', '◎', '◉', '╱', '×', '+', '*', // 게이트웨이 기호·이벤트 위치 글자·기본 흐름 빗금
    '╲', '‹', '›', // 게이트웨이 마름모 테두리
    '─', '│', '┌', '┐', '└', '┘', '╭', '╮', '╰', '╯', '├', '┤', '┬', '┴', '┼', // 실선·모서리·이음
    '━', '┃', '┏', '┓', '┗', '┛', '┣', '┫', '┳', '┻', '╋', // 굵은 선
    '╌', '╎', '┈', '┊', // 대시선·잔 점선
    '▶', '◀', '▲', '▼', '▷', '◁', '△', '▽', '∧', '∨', // 채운 화살촉·빈 삼각형·세로 열린 화살촉
    '«', '»', '…', // 태스크 종류 둘째 줄·잘린 이름
];

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
        assert_eq!(diamond, vec![" ╱───╲", "‹  x  ›", " ╲───╱"]);
        assert_eq!(hexagon, vec![" ╱───╲", "│  x  │", " ╲───╱"]);
        assert_ne!(diamond, hexagon, "판단(Diamond)과 예비 단계(Hexagon)가 같은 모양으로 그려지면 안 된다");
    }

    /// (diagram-diamond-side-glyph-coverage) 마름모 크기·모서리·본문 가운데 정렬은 옆면
    /// 글자 선택과 무관하게 고정돼야 한다(design 검증 속성 (c), tasks 2.1) — 옆면 두 칸을
    /// 뺀 나머지 칸으로 확인한다.
    #[test]
    fn diamond_corners_and_size_are_independent_of_side_glyphs() {
        let (w, h) = measure(Shape::Diamond, &[vec!["x".into()]]);
        assert_eq!((w, h), (1 + 6, 1 + 2), "크기 공식은 (tw+6, th+2)여야 한다");
        let rows = draw_rows(Shape::Diamond, vec![vec!["x".into()]]);
        assert_eq!(rows[0], " ╱───╲", "위 모서리는 옆면 글자와 무관해야 한다");
        assert_eq!(rows[2], " ╲───╱", "아래 모서리는 옆면 글자와 무관해야 한다");
        let body_chars: Vec<char> = rows[1].chars().collect();
        let body_without_sides: String = body_chars[1..body_chars.len() - 1].iter().collect();
        assert_eq!(body_without_sides, "  x  ", "옆면 두 칸을 뺀 본문은 가운데 정렬로 그대로여야 한다");
    }

    /// (diagram-diamond-side-glyph-coverage) bugfix 1.1, 2.1 — 마름모 옆면 두 칸은 기본
    /// Noto Sans Mono CJK 커버리지 안 글자(`‹`/`›`)라야 하고, 렌더 결과 어디에도 커버리지
    /// 밖 글자(`⟨`/`⟩`, U+27E8/27E9)가 남아 있으면 안 된다. 수정 전 코드에서는 옆면이
    /// `⟨`/`⟩`라 이 테스트가 실패한다.
    #[test]
    fn diamond_side_glyphs_are_covered_by_default_cjk_mono_font() {
        let rows = draw_rows(Shape::Diamond, vec![vec!["ok?".into()]]);
        let body = rows.iter().find(|row| row.contains("ok?")).expect("본문 줄이 있어야 한다");
        let body_chars: Vec<char> = body.chars().collect();
        assert_eq!(body_chars.first(), Some(&'‹'), "왼쪽 옆면 글자가 커버리지 안 글자여야 한다: {body:?}");
        assert_eq!(body_chars.last(), Some(&'›'), "오른쪽 옆면 글자가 커버리지 안 글자여야 한다: {body:?}");
        for row in &rows {
            assert!(!row.contains('⟨') && !row.contains('⟩'), "커버리지 밖 글자(U+27E8/27E9)가 남아 있으면 안 된다: {row:?}");
        }
    }

    /// (diagram-diamond-side-glyph-coverage) 요구사항 2.1, 2.2 — 마름모 옆면 글자 허용
    /// 목록(`‹›`)·제외 목록(`⟨⟩`)을 게이트웨이 커버리지 허용 목록 테스트와 같은 형식으로
    /// 고정해 재발을 막는다(design 검증 속성 (b)).
    #[test]
    fn diamond_side_glyph_codepoints_are_within_cjk_mono_coverage() {
        let rows = draw_rows(Shape::Diamond, vec![vec!["ok?".into()]]);
        let body = rows.iter().find(|row| row.contains("ok?")).expect("본문 줄이 있어야 한다");
        let body_chars: Vec<char> = body.chars().collect();
        for ch in [*body_chars.first().unwrap(), *body_chars.last().unwrap()] {
            assert!(CJK_MONO_COVERED_GLYPHS.contains(&ch), "{ch:?}(U+{:04X})가 커버리지 허용 목록 안에 있어야 한다", ch as u32);
        }
        for excluded in ['⟨', '⟩'] {
            assert!(!CJK_MONO_COVERED_GLYPHS.contains(&excluded), "{excluded:?}는 커버리지 밖이라 교체됐다");
        }
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
    /// 흐름 종류별 선(대시선·잔 점선)과 메시지 도착 표식(빈 삼각형 네 방향)도 같은 실측 안이다.
    #[test]
    fn bpmn_glyph_codepoints_are_within_cjk_mono_coverage() {
        let gateway_symbols = ['×', '+', '○', '*', '◎'];
        let event_position_glyphs = ['○', '◎', '◉'];
        let slash = ['╱'];
        let flow_kind_glyphs = ['╌', '╎', '┈', '┊', '▷', '◁', '△', '▽'];
        for ch in gateway_symbols.into_iter().chain(event_position_glyphs).chain(slash).chain(flow_kind_glyphs) {
            assert!(CJK_MONO_COVERED_GLYPHS.contains(&ch), "{ch:?}(U+{:04X})가 커버리지 허용 목록 안에 있어야 한다", ch as u32);
        }
        for excluded in ['⬠', '∗'] {
            assert!(!CJK_MONO_COVERED_GLYPHS.contains(&excluded), "{excluded:?}는 커버리지 밖이라 교체됐다");
        }
    }

    /// (bpmn-event-shape-notation) 시작·중간·종료 이벤트는 테두리 없이 위치 글자 +
    /// 오른쪽 이름 한 줄로 그려진다(요구사항 2.1~2.3) — `Round`의 둥근 상자 실루엣과는
    /// 다르다(1.1~1.3의 재현: 이전엔 이 셋이 `Round`·`Stadium`과 같은 세 줄 상자였다).
    #[test]
    fn event_start_intermediate_end_render_with_position_glyphs() {
        let start = draw_rows(Shape::Event(EventPosition::Start), vec![vec!["Go".into()]]);
        assert_eq!(start, vec!["○ Go"]);

        let intermediate = draw_rows(Shape::Event(EventPosition::Intermediate), vec![vec!["Go".into()]]);
        assert_eq!(intermediate, vec!["◎ Go"]);

        let end = draw_rows(Shape::Event(EventPosition::End), vec![vec!["Go".into()]]);
        assert_eq!(end, vec!["◉ Go"]);

        let round = draw_rows(Shape::Round, vec![vec!["Go".into()]]);
        assert_eq!(round, vec!["╭────╮", "│ Go │", "╰────╯"]);
        assert_ne!(start, round, "이벤트는 Round와 실루엣이 달라야 한다");
        const BORDER_CHARS: [char; 12] = ['╭', '╮', '╰', '╯', '─', '│', '┏', '┓', '┗', '┛', '━', '┃'];
        for row in start.iter().chain(intermediate.iter()).chain(end.iter()) {
            assert!(!row.chars().any(|c| BORDER_CHARS.contains(&c)), "이벤트 출력에 테두리 글자가 없어야 한다: {row:?}");
        }
    }

    /// (bpmn-event-shape-notation) 이름이 없는 이벤트는 위치 글자 하나만 1×1로 그려지고,
    /// 패닉이 없다(요구사항 2.4).
    #[test]
    fn event_with_empty_body_has_one_line_height_and_does_not_panic() {
        let rows = draw_rows(Shape::Event(EventPosition::Start), vec![vec![]]);
        assert_eq!(rows, vec!["○"]);
    }

    /// (bpmn-event-shape-notation) 이벤트 둘째 줄 `«timer»`는 이름 시작 열에 맞춰 다음 줄에
    /// 오고, 태스크와 같은 흐림·기울임 규칙을 따른다. 위치 글자는 강조색이다(요구사항 2.5).
    #[test]
    fn event_stereotype_second_line_is_dim_and_italic() {
        let runs = draw_runs(Shape::Event(EventPosition::Start), vec![vec!["Go".into(), "«timer»".into()]]);
        assert_eq!(runs.len(), 2, "테두리 없이 이름 줄 + 둘째 줄");
        let name_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("Go"))).expect("이름 줄이 있어야 한다");
        let stereotype_row = runs.iter().find(|row| row.iter().any(|(text, _)| text.contains("«timer»"))).expect("둘째 줄이 있어야 한다");
        assert!(name_row.iter().any(|(text, style)| text.contains("Go") && style.bold && !style.dim));
        assert!(name_row.iter().any(|(text, style)| text.contains('○') && *style == Theme::none().diagram_accent));
        assert!(stereotype_row.iter().any(|(text, style)| text.contains("«timer»") && style.dim && style.italic));

        let (_, single_line_height) = measure(Shape::Event(EventPosition::Start), &[vec!["Go".into()]]);
        assert_eq!(single_line_height, 1);
    }

    /// bugfix bpmn-event-notation-anchor 검증 속성 (a)/2.5: 배치기가 측정값보다 넓게(+4)·
    /// 높게(+2) 늘려도 원 글자는 항상 노드 원점 (0, 0)에 그려지고, 이름은 (2, 0)에서
    /// 시작한다 — 여분은 이름 뒤·아래로만 남는다(수정 전엔 원 글자가 여분 폭 절반만큼
    /// 밀려 (0, 0)이 아니었다, `block_x = x + (w - measured_width) / 2`).
    #[test]
    fn event_glyph_stays_pinned_to_the_node_origin_when_stretched_by_min_width_and_min_height() {
        let sections = vec![vec!["Go".into()]];
        let (measured_width, measured_height) = measure(Shape::Event(EventPosition::End), &sections);
        let min_width = measured_width + 4;
        let min_height = measured_height + 2;
        let mut canvas = Canvas::new(min_width, min_height);
        draw(&mut canvas, 0, 0, Shape::Event(EventPosition::End), &sections, &Theme::none(), min_width, min_height);
        let rows: Vec<String> = canvas.into_lines().iter().map(Line::plain).collect();
        let first_row: Vec<char> = rows[0].chars().collect();
        assert_eq!(first_row.first(), Some(&'◉'), "원 글자가 노드 원점 (0, 0)에 있어야 한다: {rows:?}");
        assert_eq!(first_row.get(2), Some(&'G'), "이름이 (2, 0)에서 시작해야 한다: {rows:?}");
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
