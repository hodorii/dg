# Tech Stack

## Language & Toolchain

- **Rust**, edition 2024, MSRV 1.88
- Build tool: `cargo` (wrapped by `make`)

## Dependencies

Only 4 crates — intentional minimalism. New dependencies require strong justification.

| Crate | Version | Purpose | Feature-gated? |
|---|---|---|---|
| `pulldown-cmark` | 0.13 | Markdown parsing (CommonMark + GFM extensions) | No |
| `unicode-width` | 0.2 | Terminal column-width for CJK/wide chars | No |
| `clap` | 4.5 (derive) | CLI argument parsing | `cli` feature only |
| `crossterm` | 0.29 | Raw terminal, key/mouse events, cursor control | `cli` feature only |

No `notify` (uses mtime polling), no `syntect` (no syntax highlighting), no `base64` (hand-rolled), no `open` crate (uses `std::process::Command`).

## Features

- `default = ["cli"]` — includes the binary and TUI pager
- `cli` — gates `clap`, `crossterm`, `pager`, `watch`, `links` modules; disable when using as a library

## Release Profile

```toml
[profile.release]
opt-level = "s"   # size-optimized
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

## Common Commands

```sh
make build        # cargo build --release → target/release/dg
make test         # cargo test
make lint         # cargo clippy -- -D warnings
make install      # install to ~/.local/bin/dg (PREFIX=... to override)
make uninstall    # remove installed binary
make clean        # cargo clean
make examples     # re-render examples/architecture.md → examples/architecture.txt (width 140)
make deploy-mac   # rsync to MAC_HOST + cargo install there
```

Direct cargo equivalents:
```sh
cargo build --release
cargo test
cargo clippy
cargo install --path .
cargo run --example lib_usage
```

## Runtime Environment Variables

| Variable | Values | Effect |
|---|---|---|
| `DG_STYLE` | `dark`, `light`, `none`, `auto` | Color theme (overridden by `-s` flag) |
| `DG_ER_NOTATION` | `crow`, `uml` | ER diagram notation |
| `DG_DIRECTION` | `tb`, `lr`, `bt`, `rl` | Default diagram direction |
| `DG_DEPTH` | integer | Diagram expand depth |
| `NO_COLOR` | any non-empty | Disable all color output |
| `DG_DEBUG=1` | `1` | Print layout debug info to stderr |
