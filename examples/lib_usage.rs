//! 라이브러리 사용 예: `cargo run --example lib_usage`
use dg::{render_diagram, render_markdown, to_ansi, to_text, Language, RenderOptions, Theme};

fn main() {
    let markdown = "# 배포 흐름\n\n| 단계 | 담당 |\n|---|---|\n| 빌드 | Jenkins |\n\n```plantuml\n@startuml\nleft to right direction\ndatabase DB\n[Web] --> [API] : REST\n[API] --> DB\n@enduml\n```\n";
    let options = RenderOptions { width: 70, theme: Theme::dark(), ..RenderOptions::default() };
    println!("{}", to_ansi(&render_markdown(markdown, &options), &options.theme));

    let plain = RenderOptions { width: 40, ..RenderOptions::default() };
    if let Some(lines) = render_diagram("sequenceDiagram\n A->>B: hello\n B-->>A: hi", Some(Language::Mermaid), &plain) {
        println!("{}", to_text(&lines));
    }

    // dg 저장소 자신의 실제 커밋 로그 스냅샷(`git log --oneline -8 --reverse`)을 그대로 그려본다.
    let gitgraph = "gitGraph\n\
        commit id: \"b168c69\"\n\
        commit id: \"d2dab9f\"\n\
        commit id: \"418dd41\"\n\
        commit id: \"0d33f46\"\n\
        commit id: \"fdc044c\"\n\
        commit id: \"67957e6\"\n\
        commit id: \"7005413\"\n\
        commit id: \"ac6d870\"\n";
    let wide = RenderOptions { width: 100, ..RenderOptions::default() };
    if let Some(lines) = render_diagram(gitgraph, Some(Language::Mermaid), &wide) {
        println!("{}", to_text(&lines));
    }

    // dg 자신의 모듈 구조를 block-beta로 그려본다: 진입점 → 마크다운 렌더러 → 파서 → 배치기 → 출력 단위.
    let block = "block-beta\n\
        columns 3\n\
        main[\"main.rs\"] cli[\"cli.rs\"] pager[\"pager.rs\"]\n\
        markdown[\"markdown/\"] space:2\n\
        block:parsers\n\
        \x20 columns 2\n\
        \x20 mmd[\"mermaid/*\"] puml[\"plantuml/*\"]\n\
        end\n\
        block:layouts\n\
        \x20 columns 3\n\
        \x20 lgraph[\"graph\"] lseq[\"sequence\"] lblk[\"block\"]\n\
        \x20 lgit[\"gitgraph\"] lchart[\"chart\"] lshape[\"shape\"]\n\
        end\n\
        canvas[\"canvas.rs\"] line[\"line.rs\"] style[\"style.rs\"]\n\
        main --> cli\n\
        pager --> markdown\n\
        markdown --> parsers\n\
        parsers --> layouts\n\
        layouts --> canvas\n\
        canvas --> line\n";
    if let Some(lines) = render_diagram(block, Some(Language::Mermaid), &wide) {
        println!("{}", to_text(&lines));
    }
}
