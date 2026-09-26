//! `diagram::mod`가 mermaid·PlantUML과 같은 모양으로 부르는 BPMN 언어 진입점.
//!
//! 파서는 없다(하류 `bpmn-xml`·`bpmn-yaml`·`bizprocess-bpmn` 스펙 몫) — `kind_of`는 항상 `None`이고
//! `render(source)`는 그래서 항상 코드블록 폴백으로 물러난다. 손으로 만든 `Model`을 그리려면
//! [`render_model`]을 직접 부른다.

pub mod lower;
pub mod model;
pub mod validate;
pub mod vocabulary;

use crate::diagram::layout;
use crate::diagram::options::DiagramOptions;
use crate::line::Line;
use crate::style::Theme;
use model::Model;

/// 코드펜스 본문의 문법 갈래. 이 스펙에서는 항상 `None`(파서 없음) — 파서 스펙이 갈래를 더한다.
pub fn kind_of(_source: &str) -> Option<&'static str> {
    None
}

/// `kind_of` → 파서 → `render_model`. 파서가 없어 지금은 항상 `None`.
pub fn render(source: &str, _theme: &Theme, _width: usize, _options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    kind_of(source)?;
    None
}

/// `elements`가 비면 `None` → 검증 → 변환 → 방향 옵션 덮어쓰기 → 그래프 배치기 렌더링.
/// 종류 이름은 참여자 2개 이상이면 `"collaboration"`, 아니면 `"process"`.
pub fn render_model(model: &Model, theme: &Theme, width: usize, options: DiagramOptions) -> Option<(&'static str, Vec<Line>)> {
    if model.elements.is_empty() {
        return None;
    }
    validate::validate(model).ok()?;
    let mut graph = lower::lower(model);
    options.apply_to_graph(&mut graph);
    let lines = layout::graph::render(&graph, theme, width)?;
    let kind = if model.participants.len() >= 2 { "collaboration" } else { "process" };
    Some((kind, lines))
}

#[cfg(test)]
mod tests {
    use super::model::{Element, ElementKind, EventPosition, EventTrigger, Flow, FlowKind, GatewayKind, Lane, Participant, TaskKind};
    use super::*;
    use crate::diagram::ir::Direction;
    use crate::diagram::options::DiagramOptions;

    fn task(id: &str, name: &str, container: Option<&str>) -> Element {
        Element { id: id.into(), name: name.into(), kind: ElementKind::Task(TaskKind::None), container: container.map(str::to_string), attached_to: None }
    }

    fn one_pool_model() -> Model {
        Model {
            participants: vec![Participant { id: "p1".into(), name: "프로세스".into(), lanes: Vec::new() }],
            elements: vec![
                Element { id: "start".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: Some("p1".into()), attached_to: None },
                task("t1", "처리", Some("p1")),
                Element { id: "end".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("p1".into()), attached_to: None },
            ],
            flows: vec![
                Flow { id: "f1".into(), source: "start".into(), target: "t1".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f2".into(), source: "t1".into(), target: "end".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
            ],
            ..Model::default()
        }
    }

    #[test]
    fn kind_name_is_process_for_one_pool_and_collaboration_for_two_or_more() {
        let model = one_pool_model();
        let (kind, _) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "process");

        let mut two_pools = model.clone();
        two_pools.participants.push(Participant { id: "p2".into(), name: "상대".into(), lanes: Vec::new() });
        two_pools.elements.push(task("t2", "상대 작업", Some("p2")));
        let (kind, _) = render_model(&two_pools, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
    }

    #[test]
    fn model_without_elements_or_invalid_model_renders_nothing() {
        assert_eq!(render_model(&Model::default(), &Theme::none(), 100, DiagramOptions::default()), None);

        let invalid = Model {
            elements: vec![task("a", "a", Some("ghost-pool"))],
            ..Model::default()
        };
        assert_eq!(render_model(&invalid, &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn source_entry_point_is_always_none_until_a_parser_spec_adds_a_dialect() {
        assert_eq!(kind_of("<definitions></definitions>"), None);
        assert_eq!(render("<definitions></definitions>", &Theme::none(), 100, DiagramOptions::default()), None);
    }

    #[test]
    fn empty_pool_renders_band_and_title_alongside_the_populated_pool() {
        let model = Model {
            title: "빈 협업".into(),
            participants: vec![Participant { id: "empty".into(), name: "미확정 참여자".into(), lanes: Vec::new() }, Participant { id: "full".into(), name: "우리".into(), lanes: Vec::new() }],
            elements: vec![task("a", "작업", Some("full"))],
            ..Model::default()
        };
        let (kind, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains("빈 협업"));
        assert!(rendered.contains("미확정 참여자"));
        assert!(rendered.contains("우리"));
        assert!(rendered.contains("작업"));
    }

    #[test]
    fn nodes_outside_any_pool_mixed_with_pools_do_not_panic() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![task("in", "안", Some("p1")), task("out", "밖", None)],
            flows: vec![Flow { id: "f1".into(), source: "out".into(), target: "in".into(), label: String::new(), kind: FlowKind::Association }],
            ..Model::default()
        };
        let _ = render_model(&model, &Theme::none(), 100, DiagramOptions::default());
    }

    #[test]
    fn boundary_event_sits_right_of_host_in_the_same_lane_with_dashed_link_then_solid_sequence() {
        let model = Model {
            participants: vec![Participant { id: "p1".into(), name: "P1".into(), lanes: Vec::new() }],
            elements: vec![
                task("host", "작업", Some("p1")),
                // container를 호스트와 같은 값으로 명시(1.7 "소속이 없거나 호스트와 같음") — 검증의
                // 풀 판정은 리터럴 container만 보므로(호스트 상속은 lower 몫), 풀 안쪽으로 나가는
                // 시퀀스 흐름(5.2)이 통과하려면 여기서 명시적으로 맞춰 둔다.
                Element { id: "boundary".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: Some("p1".into()), attached_to: Some("host".into()) },
                Element { id: "end".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("p1".into()), attached_to: None },
            ],
            flows: vec![Flow { id: "f1".into(), source: "boundary".into(), target: "end".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } }],
            ..Model::default()
        };
        let (_, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        assert!(rendered.contains('╌'));
        assert!(rendered.contains("«error»"));

        // LR: 경계 이벤트가 호스트 오른쪽 다음 층·같은 레인 안(비슷한 줄 대역)에 있고, 그 뒤
        // 시퀀스 흐름은 실선(대시가 아닌 가로줄)으로 이어진다(5.1, 5.2).
        let host_row = lines.iter().position(|l| l.text().contains("작업")).expect("호스트 상자가 있어야 한다");
        let host_col = lines[host_row].text().find("작업").unwrap();
        let boundary_row = lines.iter().position(|l| l.text().contains("«error»")).expect("경계 이벤트 상자가 있어야 한다");
        let boundary_col = lines[boundary_row].text().find("«error»").unwrap();
        assert!(boundary_col > host_col, "경계 이벤트는 호스트보다 오른쪽 칸(다음 층)에 있어야 한다");
        assert!(boundary_row.abs_diff(host_row) <= 4, "경계 이벤트는 호스트와 같은 레인 대역 안에 있어야 한다");
    }

    #[test]
    fn vertical_stacks_pool_bands_left_right_and_horizontal_stacks_top_down() {
        let mut model = one_pool_model();
        model.participants.push(Participant { id: "p2".into(), name: "P2".into(), lanes: Vec::new() });
        model.elements.push(task("t2", "다른 작업", Some("p2")));

        let row_of = |lines: &[Line], needle: &str| lines.iter().position(|l| l.text().contains(needle)).expect("풀 제목이 있어야 한다");
        let col_of = |lines: &[Line], row: usize, needle: &str| lines[row].text().find(needle).expect("풀 제목이 있어야 한다");

        model.orientation = model::Orientation::Horizontal;
        let (_, horizontal) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        // 가로(흐름 왼쪽→오른쪽): 풀 띠가 위아래로 쌓여 두 풀 제목이 서로 다른 줄에 있다.
        assert_ne!(row_of(&horizontal, "프로세스"), row_of(&horizontal, "P2"));

        model.orientation = model::Orientation::Vertical;
        let (_, vertical) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("렌더링돼야 한다");
        // 세로(흐름 위→아래): 풀 띠가 좌우로 나란해 두 풀 제목이 같은 줄의 서로 다른 칸에 있다.
        let p1_row = row_of(&vertical, "프로세스");
        assert_eq!(p1_row, row_of(&vertical, "P2"));
        assert_ne!(col_of(&vertical, p1_row, "프로세스"), col_of(&vertical, p1_row, "P2"));
    }

    #[test]
    fn direction_option_overrides_horizontal_model_orientation() {
        let model = one_pool_model();
        let options = DiagramOptions { direction: Some((Direction::TopDown, false)), ..DiagramOptions::default() };
        // 방향 옵션이 lower()가 만든 LeftRight를 덮어써도 패닉 없이 렌더링된다.
        assert!(render_model(&model, &Theme::none(), 100, options).is_some());
    }

    #[test]
    fn width_below_minimum_yields_none() {
        let model = one_pool_model();
        assert_eq!(render_model(&model, &Theme::none(), 8, DiagramOptions::default()), None);
    }

    #[test]
    fn odd_shapes_do_not_panic_self_loop_cycle_unnamed_three_level_lanes_multiple_boundaries() {
        let model = Model {
            participants: vec![Participant {
                id: "p1".into(),
                name: "".into(),
                lanes: vec![Lane { id: "l1".into(), name: "".into(), sub_lanes: vec![Lane { id: "l1-1".into(), name: "".into(), sub_lanes: Vec::new() }] }],
            }],
            elements: vec![
                task("a", "", Some("l1-1")),
                task("b", "", Some("l1-1")),
                Element { id: "boundary1".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Timer) }, container: None, attached_to: Some("a".into()) },
                Element { id: "boundary2".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) }, container: None, attached_to: Some("a".into()) },
                Element { id: "gw".into(), name: "".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: Some("l1-1".into()), attached_to: None },
            ],
            flows: vec![
                Flow { id: "self".into(), source: "a".into(), target: "a".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "ab".into(), source: "a".into(), target: "b".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "ba".into(), source: "b".into(), target: "a".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "a_gw".into(), source: "a".into(), target: "gw".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "gw_b".into(), source: "gw".into(), target: "b".into(), label: String::new(), kind: FlowKind::Sequence { is_default: true } },
            ],
            ..Model::default()
        };
        for orientation in [model::Orientation::Horizontal, model::Orientation::Vertical] {
            let mut model = model.clone();
            model.orientation = orientation;
            let _ = render_model(&model, &Theme::none(), 100, DiagramOptions::default());
        }
    }

    /// design §Testing Strategy 8.3 — 풀 `고객`, 풀 `판매사`(레인 `영업`·`창고`)의 주문 처리
    /// 협업. 파일을 파싱하지 않고 손으로 만든 `Model`로만 검증한다.
    fn order_processing_collaboration_model() -> Model {
        let user_task = |id: &str, name: &str, container: &str| Element { id: id.into(), name: name.into(), kind: ElementKind::Task(TaskKind::User), container: Some(container.into()), attached_to: None };
        Model {
            title: "주문 처리".into(),
            orientation: model::Orientation::Horizontal,
            participants: vec![
                Participant { id: "customer".into(), name: "고객".into(), lanes: Vec::new() },
                Participant {
                    id: "vendor".into(),
                    name: "판매사".into(),
                    lanes: vec![Lane { id: "lane-sales".into(), name: "영업".into(), sub_lanes: Vec::new() }, Lane { id: "lane-warehouse".into(), name: "창고".into(), sub_lanes: Vec::new() }],
                },
            ],
            elements: vec![
                // 고객
                Element { id: "start-cust".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: None }, container: Some("customer".into()), attached_to: None },
                user_task("order-task", "주문서 작성", "customer"),
                Element { id: "end-cust".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("customer".into()), attached_to: None },
                // 판매사 · 영업
                Element { id: "msg-start".into(), name: "".into(), kind: ElementKind::Event { position: EventPosition::Start, trigger: Some(EventTrigger::Message) }, container: Some("lane-sales".into()), attached_to: None },
                user_task("review-task", "주문 검토", "lane-sales"),
                // container를 호스트(review-task)와 같은 값으로 명시 — 검증의 풀 판정은 리터럴
                // container만 보므로(호스트 상속은 lower 몫), 뒤이은 시퀀스 흐름(5.2)이 통과하려면
                // 여기서 맞춰 둔다.
                Element {
                    id: "boundary-error".into(),
                    name: "시한 초과".into(),
                    kind: ElementKind::Event { position: EventPosition::Intermediate, trigger: Some(EventTrigger::Error) },
                    container: Some("lane-sales".into()),
                    attached_to: Some("review-task".into()),
                },
                Element { id: "end-cancel".into(), name: "취소".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-sales".into()), attached_to: None },
                Element { id: "notify-oos".into(), name: "품절 안내".into(), kind: ElementKind::Task(TaskKind::Service), container: Some("lane-sales".into()), attached_to: None },
                Element { id: "end-oos".into(), name: "품절 취소".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-sales".into()), attached_to: None },
                // 판매사 · 창고
                Element { id: "gateway-stock".into(), name: "재고 있음?".into(), kind: ElementKind::Gateway(GatewayKind::Exclusive), container: Some("lane-warehouse".into()), attached_to: None },
                user_task("prepare-task", "출고 준비", "lane-warehouse"),
                Element { id: "end-shipped".into(), name: "완료".into(), kind: ElementKind::Event { position: EventPosition::End, trigger: None }, container: Some("lane-warehouse".into()), attached_to: None },
            ],
            flows: vec![
                // 고객
                Flow { id: "f-cust-1".into(), source: "start-cust".into(), target: "order-task".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-cust-2".into(), source: "order-task".into(), target: "end-cust".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 풀 사이 메시지 흐름
                Flow { id: "f-msg".into(), source: "order-task".into(), target: "msg-start".into(), label: String::new(), kind: FlowKind::Message },
                // 영업
                Flow { id: "f-sales-1".into(), source: "msg-start".into(), target: "review-task".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-sales-2".into(), source: "boundary-error".into(), target: "end-cancel".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-sales-3".into(), source: "notify-oos".into(), target: "end-oos".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 영업 → 창고(같은 풀, 다른 레인)
                Flow { id: "f-cross-1".into(), source: "review-task".into(), target: "gateway-stock".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 창고
                Flow { id: "f-wh-1".into(), source: "gateway-stock".into(), target: "prepare-task".into(), label: "예".into(), kind: FlowKind::Sequence { is_default: false } },
                Flow { id: "f-wh-2".into(), source: "prepare-task".into(), target: "end-shipped".into(), label: String::new(), kind: FlowKind::Sequence { is_default: false } },
                // 창고 → 영업(같은 풀, 다른 레인) — default 흐름
                Flow { id: "f-cross-2".into(), source: "gateway-stock".into(), target: "notify-oos".into(), label: String::new(), kind: FlowKind::Sequence { is_default: true } },
            ],
        }
    }

    #[test]
    fn order_processing_collaboration_renders_every_convention_in_one_picture() {
        let model = order_processing_collaboration_model();
        assert_eq!(validate::validate(&model), Ok(()));
        let (kind, lines) = render_model(&model, &Theme::none(), 100, DiagramOptions::default()).expect("폭 100 안에서 렌더링돼야 한다");
        assert_eq!(kind, "collaboration");
        let rendered = lines.iter().map(Line::text).collect::<Vec<_>>().join("\n");
        for glyph in ["○", "●", "◎", "┃", "«user»", "«service»", "«message»", "«error»", "× 재고 있음?", "╱", "╌"] {
            assert!(rendered.contains(glyph), "렌더링 결과에 {glyph:?}가 있어야 한다:\n{rendered}");
        }
        // `cargo test -- --nocapture`로 육안 확인(design §Key Decisions 표대로 보이는지).
        eprintln!("{rendered}");
    }
}

