# Lex

Lex is an experimental native Windows browser and browser engine written in Rust. Its long-term goal is to own the HTML → DOM → style → layout → paint pipeline rather than embedding Chromium, WebView2, or another browser engine.

> **Status:** Milestones M0–M5. The native shell, renderer, networking, HTML parser, and mutable Lex-owned DOM work; CSS and page rendering are not yet supported.

## Current capabilities

- Cargo workspace with strict formatting, testing, and lint CI.
- Native Win32 top-level window with resize, minimize, maximize, close, keyboard, mouse, and per-monitor DPI handling.
- A basic GDI bootstrap surface displaying `LEX` and `Browser engine initializing...`.
- Platform-neutral UI and application state with unit tests.
- Independent HTTP/HTTPS navigation layer with validated URLs, TLS certificate validation, redirects, compression, bounded bodies, connection pooling, and basic response caching.
- Lex-owned HTML tokenization, entities, attributes, raw text, malformed-markup recovery, and an arena-backed intermediate document tree.
- Mutable Lex-owned DOM with stable node handles, fragments, attributes, traversal, queries, source spans, validation, and explicit mutation records.

## Build

Install stable Rust and the MSVC C++ build tools, then on Windows:

```powershell
cargo run -p lex-app
```

Development checks (also run in CI):

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

See [ARCHITECTURE.md](ARCHITECTURE.md), [ROADMAP.md](ROADMAP.md), and [COMPATIBILITY.md](COMPATIBILITY.md).
