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
- `lex-css`: CSS tokenization, recovering parsing, stylesheet data, values, selector matching, specificity, and the declared-value cascade foundation.
- `lex-style`: property-aware cascade validation, inline declarations, HTML defaults, inheritance, initial values, supported computed-value conversion, typed `ComputedStyle`, and conservative style invalidation.
- `lex-browser` (in `Lex Browser`): deterministic development executable proving the integrated engine pipeline without coupling engine crates to the native shell.

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
9. Mutation records describe facts, not policy. CSS, layout, events, and JavaScript subscribe or adapt them rather than becoming `lex-dom` dependencies.
10. `lex-css` remains the syntax and matching layer. Property semantics and complete styles live in `lex-style`, preventing the parser from becoming coupled to layout policy.
11. M7 style invalidation favors correctness: class, ID, other attribute, insertion, removal, and stylesheet changes dirty the complete style snapshot. The invalidation records retain an affected node so later milestones can safely introduce subtree/dependency-based recomputation.

## M6 CSS foundations

`lex-css` is platform-neutral and depends only on `lex-dom`. It owns CSS tokenization, recoverable parsing, stylesheet/rule/declaration/value data, selector specificity and matching, and a deterministic declared-value cascade. Selectors are represented as compounds joined by descendant or child combinators. Matching reads stable M5 `NodeId` handles and never owns or mutates DOM nodes. Source byte spans remain on tokens, rules, selectors, declarations, and diagnostics for future DevTools.

For compatibility, `lex-css::compute_style` still exposes the M6 map of winning declarations. M7 also exposes matching declarations before property-specific validation. This is important because invalid CSS must be discarded before winner selection: an invalid high-specificity value must not hide a valid lower-specificity declaration.

## M7 style computation boundary

`lex-style` depends on `lex-css` and `lex-dom`, but neither parser nor DOM depends on it. `StyleEngine` owns author stylesheets and snapshots keyed by stable `NodeId`. A full recomputation walks connected elements in DOM preorder, then performs:

```text
selector matching → property validation → cascade → specified values
    → inheritance/initial values → HTML defaults → computed values
```

`SpecifiedStyle` retains supported winning declarations as ordinary values or the CSS-wide keywords `inherit`, `initial`, and `unset`. `ComputedStyle` is a complete typed record: there is no missing-property state. Central initial values cover display, foreground/background color, sizing, box edges, borders, font data, line height, text alignment, visibility, and opacity. Layout can call `computed_style(node_id)` without parsing declarations, matching selectors, or implementing inheritance.

Lex supplies a small explicit user-agent layer for `html`, `body`, `div`, `p`, headings, `span`, `strong`, `em`, `a`, `ul`, `ol`, and `li`. Author and inline declarations override those defaults through the normal cascade. Inherited properties are color, font family/size/weight, line height, text alignment, and visibility. Non-inherited properties use their central initial value unless an HTML default or winning declaration changes them.

Pixel and font-relative lengths become computed pixels. Font-size percentages resolve against the parent; line-height percentages resolve against the element's computed font size. Percentages requiring a containing block and viewport units remain typed and unresolved for M8. M7 box properties support one-to-four-value shorthands; per-side/logical longhands, CSS variables, functions such as `calc()`, pseudo-classes, and a complete standards UA sheet are deferred rather than represented as complete.

The effective boundary is:

```text
lex-dom Document + lex-css Stylesheet(s)
                    ↓
             lex-style StyleEngine
                    ↓
       NodeId → complete ComputedStyle
                    ↓
                M8 layout
```

The top-level [`Lex Browser`](Lex%20Browser) package is the durable development executable. Its M7 page visibly demonstrates cascade and inherited color without coupling the style system to rendering or native UI.
