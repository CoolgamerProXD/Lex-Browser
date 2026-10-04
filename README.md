# Lex

Lex is an experimental native Windows browser and browser engine written in Rust. Its long-term goal is to own the HTML → DOM → style → layout → paint pipeline rather than embedding Chromium, WebView2, or another browser engine.

> **Status:** Milestones M0–M7 are complete. The native shell, renderer, networking, HTML parser, mutable Lex-owned DOM, CSS parser/matcher, and typed style-computation subsystem work. Layout and page rendering are not yet supported.

## Current capabilities

- Cargo workspace with strict formatting, testing, and lint CI.
- Native Win32 top-level window with resize, minimize, maximize, close, keyboard, mouse, and per-monitor DPI handling.
- Platform-neutral display lists backed by Direct2D and DirectWrite on Windows. GDI remains only for Win32 paint-cycle validation, not visible rendering.
- Platform-neutral UI and application state with unit tests.
- Independent HTTP/HTTPS navigation layer with validated URLs, TLS certificate validation, redirects, compression, bounded bodies, connection pooling, and basic response caching.
- Lex-owned HTML tokenization, entities, attributes, raw text, malformed-markup recovery, and an arena-backed intermediate document tree.
- Mutable Lex-owned DOM with stable node handles, fragments, attributes, traversal, queries, source spans, validation, and explicit mutation records.
- Lex-owned CSS tokenizer/parser with recoverable diagnostics, selectors, values, matching, specificity, and deterministic cascade foundations.
- Separate Lex-owned style engine with typed computed styles, property validation, author and inline cascade, inheritance, centralized initial values, a small HTML user-agent style layer, and conservative mutation invalidation.

## M7 supported style subset

`lex-style` currently computes:

- `display`, `color`, `background-color`, `width`, and `height`
- `margin`, `padding`, `border-width`, `border-style`, and `border-color`
- `font-size`, `font-family`, `font-weight`, and `line-height`
- `text-align`, `visibility`, and `opacity`

The engine handles `inherit`, `initial`, and `unset`. Color, font, line-height, text alignment, and visibility inheritance is deterministic. Pixel and font-relative lengths are computed where enough context exists; containing-block percentages and viewport-relative lengths remain typed for M8. The HTML defaults cover `body`, `div`, paragraphs, headings, spans, emphasis/strong text, links, and lists.

This is intentionally not complete CSS. Pseudo-classes, custom properties, `calc()`, logical/per-side box properties, advanced color functions, and a comprehensive browser user-agent stylesheet remain unsupported.

## Lex Browser development entry point

The durable demo executable is in the top-level `Lex Browser` folder. On Windows, install stable Rust and the MSVC C++ build tools, open a Visual Studio Developer Command Prompt in the repository root, and run:

```powershell
cargo run -p lex-browser
```

It prints visible proof of the deterministic HTML → DOM → CSS → selector matching → cascade → inheritance → computed-style pipeline, including an inherited parent color. It does not yet lay out or paint a web page; the existing native shell remains available with `cargo run -p lex-app`.

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
