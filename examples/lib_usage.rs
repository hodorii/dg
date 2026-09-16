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
}
