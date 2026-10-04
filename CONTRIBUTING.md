# Contributing

Use stable Rust, keep changes small, and add tests for observable behavior. Run `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` before submitting changes.

Do not add a complete browser engine, disable certificate validation, hard-code secrets, or label stubs as supported. Document new dependencies and preserve subsystem boundaries. Unsafe Rust must be narrowly scoped, justified, and reviewed especially carefully.
