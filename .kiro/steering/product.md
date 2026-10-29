# Product: dg

`dg` is a lightweight terminal Markdown viewer that renders Mermaid and PlantUML diagrams as pure Unicode text art — no Java, no browser, no image protocols required.

## Core Purpose

View Markdown files in the terminal with readable typography and real ASCII/Unicode diagrams rendered inline. Works inside SSH sessions and tmux.

## Key Features

- **Markdown rendering**: headings, CJK-aware text wrapping, tables, lists, footnotes, blockquotes, GFM alerts (`[!NOTE]`), inline code, YAML/TOML frontmatter stripping
- **Diagram rendering**: Mermaid (flowchart, sequence, class, ER, state, block-beta, gitGraph, pie, xychart, quadrant, gantt) and PlantUML (sequence, class, ER, gantt, component, WBS)
- **Custom `bizprocess` language**: BPMN-style business process drill-downs from structured Markdown headings
- **TUI pager**: keyboard/mouse navigation, search (`/`), source toggle (`o`), link navigation (anchor jump, external URL, cross-file navigation with back/forward history), watch mode (`-W`), mouse drag + OSC 52 clipboard copy
- **Dual-mode**: standalone binary or embeddable Rust library (without `cli` feature, no clap/crossterm deps)

## Design Philosophy

- Graceful degradation: if a diagram doesn't fit the terminal width, `dg` retries with the opposite direction (TB↔LR), progressively reduces label widths, then falls back to a raw code block — never crashes
- Minimal dependencies: only 4 crates total; no new dependencies without strong justification
- No panic contract: all public-facing rendering functions return `Option` or `io::Result`, never panic on bad input
- Size-optimized binary: ~1.2 MB static binary (`opt-level = "s"`, LTO, stripped)
