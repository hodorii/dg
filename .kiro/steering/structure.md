# Project Structure

## Top-Level Layout

```
dg/
├── Cargo.toml          # package manifest (lib + [[bin]], 4 deps, release profile)
├── Makefile            # build/install/examples/deploy targets
├── README.md           # full feature documentation (Korean)
├── brief.md            # design brief for the current in-flight feature
├── roadmap.md          # feature roadmap with spec tracking
├── examples/           # demo files: architecture.md (source) + architecture.txt (rendered)
├── src/                # all source code
└── .kiro/
    ├── specs/          # spec-driven feature specs (40+ completed specs)
    └── steering/       # Kiro steering files (this directory)
```

## Source Tree (`src/`)

```
src/
├── main.rs             # CLI entry point: arg parsing, theme selection, render closure, pager/print dispatch
├── lib.rs              # Public library API: render_markdown(), render_diagram(), RenderOptions
├── cli.rs              # clap Cli struct and enum conversions
├── line.rs             # Line + Span — universal output of every renderer; to_ansi(), text_between_cols()
├── style.rs            # Style (fg/bg/bold/italic/underline), Color, Theme (dark/light/none)
├── text.rs             # width_of(), char_width(), wrap_plain(), truncate()
├── pager.rs            # TUI pager: event loop, keyboard/mouse, search, link focus/follow, history, clipboard
├── watch.rs            # mtime-polling file watcher; poll() → PollResult::{Unchanged, Changed, ReadError}
├── links.rs            # classify(), locate_links(), History, open_external(), clipboard_sequence() (OSC 52)
├── markdown/
│   ├── mod.rs          # render_document() → Document{lines, diagrams, text_blocks, links, headings}
│   ├── table.rs        # table column widths, header/body separator, cell wrapping
│   └── wrap.rs         # styled text wrapping that preserves span boundaries
└── diagram/
    ├── mod.rs          # Language enum, language_of_fence/path/source() sniffing, render() dispatch
    ├── ir.rs           # Shared IR: Graph, Sequence, Node, Edge, Group, Shape, Marker
    ├── canvas.rs       # 2D char canvas: hline/vline/rect/text, box-drawing char merging, into_lines()
    ├── options.rs      # DiagramOptions, ErNotation, ExpandPolicy, in-source directive parsing
    ├── layout/         # IR → Canvas renderers
    │   ├── graph.rs    # Main graph layout: layer assignment, barycenter ordering, edge routing
    │   ├── sequence.rs # Sequence layout: constraint-based participant spacing
    │   ├── gitgraph.rs # Git graph: horizontal lane or vertical column
    │   ├── block.rs, chart.rs, gantt.rs, pie.rs, quadrant.rs, shape.rs
    ├── mermaid/        # Mermaid parsers → IR (one file per diagram type)
    │   └── mod.rs, flow.rs, class.rs, er.rs, sequence.rs, state.rs, block.rs,
    │       gitgraph.rs, pie.rs, quadrant.rs, xychart.rs, gantt.rs, text.rs
    ├── plantuml/       # PlantUML parsers → IR
    │   └── mod.rs, sequence.rs, class.rs, component.rs, gantt.rs, wbs.rs, relation.rs, text.rs
    └── bpmn/           # bizprocess/BPMN pipeline
        └── mod.rs, model.rs, bizprocess.rs, lower.rs,
            parse_bizprocess.rs, parse_xml.rs, parse_yaml.rs,
            validate.rs, vocabulary.rs, flow_properties.rs, xml.rs, yaml.rs
```

## Architectural Layers

1. **Parsers** (`diagram/mermaid/`, `diagram/plantuml/`, `diagram/bpmn/`) — convert source text to the shared IR
2. **IR** (`diagram/ir.rs`) — `Graph` and `Sequence`: the single interface between all parsers and all layout engines
3. **Layout engines** (`diagram/layout/`) — convert IR to a `Canvas`
4. **Canvas** (`diagram/canvas.rs`) — 2D char grid → `Vec<Line>`
5. **Line/Span** (`line.rs`, `style.rs`) — universal output; supports ANSI, plain text, search highlight, width truncation
6. **Pager** (`pager.rs`, `watch.rs`, `links.rs`) — TUI layer on top of rendered lines (CLI feature only)

## Key Conventions

- **All parsers produce the same IR.** A new diagram type only needs a new parser; layout engines are shared.
- **`cli` feature gate**: `pager`, `watch`, `links` are excluded when building the library without the `cli` feature.
- **No panic on bad input**: public rendering functions return `Option` or `io::Result`. Width-fit failures return `None`, causing graceful fallback to raw code block.
- **Code and comments are in Korean** (the author's language). This is intentional — match the surrounding style.
- **Inline tests**: unit tests live in `#[cfg(test)] mod tests` blocks at the bottom of each file.
- **`examples/architecture.txt`** must be regenerated with `make examples` whenever `examples/architecture.md` changes.
- **Spec-driven development**: new features follow the spec process in `.kiro/specs/`. See `AGENTS.md` and `methodology/AGENTS.md` for the workflow.
