# Roadmap

| Milestone | Status | Exit criterion |
|---|---|---|
| M0 Repository | **Complete** | Workspace, policy docs, formatting/lint/test CI |
| M1 Windows window | **Complete** | Native DPI-aware resizable window and input event loop |
| M2 Renderer | **Complete** | Renderer abstraction, tested display commands, Direct2D/DirectWrite backend |
| M3 URL/network | **Complete** | Validated URLs, HTTP/HTTPS, redirects, decoding, bounded responses, basic cache |
| M4 HTML tokenizer/parser | **Complete** | Lex tokenizer, character references, recovery parser, intermediate tree |
| M5 DOM | **Complete** | Mutable Lex-owned arena, stable handles, mutations, fragments, queries |
| M6 CSS | Next | CSS tokenizer/parser, selectors, declarations, and values |
| M7–M10 Engine pipeline | Planned | Style, layout, paint, and basic HTML rendering |
| M11–M12 Interaction/UI | Planned | Scrolling, navigation and honest browser chrome |
| M13–M15 JS/APIs/storage | Planned | Embedded JS with Lex bindings and persistent profile data |
| M16–M18 Security/privacy/tabs | Planned | Isolation foundations, enforced policy, tab model |
| M19–M22 DevTools/AI/performance/compatibility | Planned | Incremental tested expansion |

Each milestone must compile, carry tests, and document unsupported behavior before the next starts.
