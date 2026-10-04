# Lex Architecture

## Principles

Lex owns parsing, DOM, styling, layout, painting, browser integration, and security policy. Mature libraries may provide infrastructure such as TLS, URL parsing, image codecs, SQLite, Unicode, and OS bindings. A complete third-party browser engine is never an acceptable dependency.

The target pipeline is:

```text
navigation → network → HTML → DOM → style → layout → display list → renderer
```

The browser chrome communicates with an engine controller; it does not mutate rendering internals.

## Current workspace

- `lex-app`: executable and native platform boundary. Win32 messages are translated to testable Rust application state. It builds the temporary startup display list but does not call Direct2D directly.
- `lex-ui`: platform-neutral browser chrome state and presentation copy.
- `lex-render`: platform-independent display commands and `Renderer` contract, plus the target-gated Direct2D/DirectWrite implementation.

Windows API access uses the `windows` crate solely for generated Win32 bindings. `DisplayList` supports frame clearing, filled rectangles, text, and balanced rectangular clip scopes. The Windows backend translates those operations to an HWND Direct2D render target and DirectWrite text formats. Future paint and layout crates will depend on display commands rather than native graphics APIs.

## Safety and lifecycle

The Win32 window owns one boxed `ApplicationState`. Its pointer is attached during `WM_NCCREATE`, read only by the window thread, and released during `WM_NCDESTROY`. All unsafe operations remain confined to the Windows platform module. Per-monitor-v2 DPI awareness is requested before window creation.

## Decisions

1. The repository remains cross-checkable on non-Windows hosts: Windows dependencies and implementation are target-gated.
2. Engine subsystems become separate crates only when their first working behavior is implemented; empty placeholder crates are avoided.
3. M1's GDI text drawing was removed at M2. GDI remains only for Win32 `BeginPaint`/`EndPaint` validation; all visible drawing is performed through the renderer contract by Direct2D/DirectWrite.
4. Display-list clip scopes are validated before rendering. Malformed command streams return a structured error instead of invoking a backend with invalid clip state.
