# Tiiny Gauge

- Read [CONTEXT.md](CONTEXT.md) before changing hardware, telemetry, display behavior, or the crate layout.
- Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) with an imperative subject, for example `feat(display): render resource meters`.
- Keep single-file modules in `name.rs`. Use `name/mod.rs` and sibling files when a module spans multiple files.
- Keep firmware `no_std` and use Embassy for async I/O and timers. Prefer borrowed data, bounded buffers, and narrow module visibility. Use `Result` and handle recoverable errors at the caller, including hardware and network failures.
- Add direct dependencies and crate features for current requirements. Use the existing driver or standard `core` APIs where they fit. Track `Cargo.lock` when dependency resolution changes.
- Before handing off firmware changes, run `cargo fmt --check`, `cargo clippy --release --locked -- -D warnings`, and `cargo build --release --locked`. Report hardware checks that still need a board.
