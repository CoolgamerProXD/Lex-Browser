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
- `lex-net`: validated URLs, document navigation policy, HTTP/HTTPS transport, redirect handling, decoded response limits, and an in-memory freshness cache. It is independent of application, UI, and rendering crates.
- `lex-html`: Lex-owned tokenizer, character-reference decoder, recovery tree constructor, and arena-backed intermediate document. It has no browser API or platform dependencies.
- `lex-dom`: authoritative mutable document model with stable arena handles, explicit parent/child relationships, fragments, attributes, traversal, queries, source provenance, and mutation records.

Windows API access uses the `windows` crate solely for generated Win32 bindings. `DisplayList` supports frame clearing, filled rectangles, text, and balanced rectangular clip scopes. The Windows backend translates those operations to an HWND Direct2D render target and DirectWrite text formats. Future paint and layout crates will depend on display commands rather than native graphics APIs.

## HTML boundary

M4 accepts decoded response text (or bytes decoded with UTF-8 replacement) and emits an intermediate `Document`. Token spans retain half-open byte offsets for diagnostics and future DevTools. The tokenizer owns tag, attribute, comment, doctype, character-reference, RCDATA, and raw-text recognition. The parser owns parent/child relationships and deterministic recovery for unmatched tags, crossed nesting, paragraphs, list items, and table rows/cells.

The intermediate arena is deliberately not the live DOM: it has stable node IDs and read-only relationships, but no events, mutation, scripting wrappers, style state, or browser globals. This is an intentionally useful subset rather than a claim of full WHATWG conformance.

## DOM ownership and mutation

`lex-dom::Document` owns every node in one arena. `NodeId` values are stable for the document lifetime; removing a node detaches it rather than freeing its slot. Nodes contain IDs rather than owning references, so parent/child relationships cannot create Rust reference cycles. Deterministic preorder traversal and bidirectional invariant validation are first-class APIs.

The M4 intermediate document is converted into the mutable DOM while preserving node and attribute source spans. After conversion, `lex-dom::Document` is authoritative. Mutating methods return ordered `MutationRecord` batches for child, attribute, and character-data changes. M7 can translate these records into style invalidation without embedding style concerns in DOM nodes. Document fragments transfer their children on insertion, matching the useful DOM insertion behavior while leaving the fragment reusable.

## Navigation and networking

The public boundary is `NavigationRequest → NetworkClient → NavigationResponse`. `LexUrl` accepts only absolute HTTP and HTTPS URLs and removes fragments before transport/cache lookup. Lex manually follows redirects to enforce limits, loop detection, relative-location resolution, and refusal of HTTPS-to-HTTP downgrade. Bodies are bounded after decompression before they can enter the future parser.

`reqwest` supplies HTTP framing and pooled connections. Its Rustls backend performs WebPKI certificate validation; there is no API in Lex to bypass validation. Reqwest's maintained codecs decode gzip, Brotli, deflate, and zstd. Lex remains responsible for browser navigation policy, safe limits, headers, redirect history, response representation, and cache decisions. The M3 cache stores only successful responses with explicit positive `Cache-Control: max-age` and rejects `no-store` and `private` responses.

## Safety and lifecycle

The Win32 window owns one boxed `ApplicationState`. Its pointer is attached during `WM_NCCREATE`, read only by the window thread, and released during `WM_NCDESTROY`. All unsafe operations remain confined to the Windows platform module. Per-monitor-v2 DPI awareness is requested before window creation.

## Decisions

1. The repository remains cross-checkable on non-Windows hosts: Windows dependencies and implementation are target-gated.
2. Engine subsystems become separate crates only when their first working behavior is implemented; empty placeholder crates are avoided.
3. M1's GDI text drawing was removed at M2. GDI remains only for Win32 `BeginPaint`/`EndPaint` validation; all visible drawing is performed through the renderer contract by Direct2D/DirectWrite.
4. Display-list clip scopes are validated before rendering. Malformed command streams return a structured error instead of invoking a backend with invalid clip state.
5. Networking uses synchronous navigation in M3, but its API is isolated so a network thread/process can own `NetworkClient` later. The Windows event thread must never perform blocking navigation.
6. Cache freshness requires explicit `max-age` in M3. Conditional requests, persistent caching, cookies, and full RFC cache semantics are deliberately deferred rather than represented as complete.
7. M4 preserves exact whitespace text and merges adjacent text/reference tokens during tree construction. Script and style contents use raw-text handling; title and textarea use RCDATA handling. Parse errors are accumulated rather than fatal.
8. M5 retains detached nodes for handle stability. Explicit garbage collection and generational handles may be added only when lifecycle requirements are understood.
9. Mutation records describe facts, not policy. CSS, layout, events, and JavaScript will subscribe or adapt them in later milestones rather than becoming `lex-dom` dependencies.
