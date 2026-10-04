# Lex Architecture

## Principles

Lex owns parsing, DOM, styling, layout, painting, browser integration, and security policy. Mature libraries may provide infrastructure such as TLS, URL parsing, image codecs, SQLite, Unicode, and OS bindings. A complete third-party browser engine is never an acceptable dependency.

The target pipeline is:

```text
navigation → network → HTML → DOM → style → layout → display list → renderer
```

The browser chrome communicates with an engine controller; it does not mutate rendering internals.

## Current workspace

- `lex-app`: executable and native platform boundary. Win32 messages are translated to testable Rust application state.
- `lex-ui`: platform-neutral browser chrome state and presentation copy.

Windows API access uses the `windows` crate solely for generated Win32 bindings. M1 uses GDI only as a bootstrap drawing surface. M2 will introduce a renderer trait and Direct2D/DirectWrite backend; engine crates will consume display commands and remain independent of Windows graphics APIs.

## Safety and lifecycle

The Win32 window owns one boxed `ApplicationState`. Its pointer is attached during `WM_NCCREATE`, read only by the window thread, and released during `WM_NCDESTROY`. All unsafe operations remain confined to the Windows platform module. Per-monitor-v2 DPI awareness is requested before window creation.

## Decisions

1. The repository remains cross-checkable on non-Windows hosts: Windows dependencies and implementation are target-gated.
2. Engine subsystems become separate crates only when their first working behavior is implemented; empty placeholder crates are avoided.
3. The initial renderer is deliberately not presented as the browser renderer. It proves native lifecycle and a paint surface only.
