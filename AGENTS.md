# Repository Guidelines

## Project Structure & Module Organization

This repository is a Cargo workspace rooted at `Cargo.toml`. The active crate is `crates/rlapi`, which contains the Rust port of the Rocket League PsyNet client. Library code lives in `crates/rlapi/src/`, and integration tests live in `crates/rlapi/tests/`. Build output goes to `target/` and must stay untracked.

Keep new code close to the crate that owns it. As `rocketstats-rlapi` grows, prefer splitting large protocol areas into focused modules instead of extending a single large file.

## Build, Test, and Development Commands

Run commands from the repository root:

- `cargo build --workspace`: compile all crates.
- `cargo test --workspace`: run unit and integration tests.
- `cargo clippy --workspace --all-targets -- -D warnings`: enforce lint-clean code.
- `cargo fmt --check`: verify formatting.
- `cargo fmt`: apply standard Rust formatting.

Use the full verification suite before opening a PR or creating a release commit.

## Coding Style & Naming Conventions

Use standard Rust formatting with `rustfmt` and 4-space indentation. Follow idiomatic Rust naming:

- `snake_case` for functions, modules, and test names
- `PascalCase` for structs, enums, and traits
- `SCREAMING_SNAKE_CASE` for constants

Prefer `thiserror` for error types, `serde` for wire structs, and `tokio` async APIs for networked code. Keep public APIs typed; avoid unstructured `serde_json::Value` unless modeling raw PsyNet traffic.

## Testing Guidelines

Use `#[test]` for pure protocol logic and `#[tokio::test]` for async network flows. Put crate-level integration tests in `crates/rlapi/tests/` and name them by behavior, for example `protocol_tests.rs` or `local_rpc_tests.rs`.

Every protocol or endpoint change should add or update tests. If you change PsyNet message formatting, signing, build ID decoding, or auth flow, include a regression test.

## Commit & Pull Request Guidelines

Use short imperative commits with a prefix, matching current history, for example `chore: initialize rocketstats workspace`. Common prefixes here are `feat:`, `fix:`, `test:`, `docs:`, and `chore:`.

PRs should include a concise summary, the commands you ran, and any protocol assumptions or upstream references used. Add screenshots only when UI work is introduced later.

## Security & Configuration Tips

Do not commit personal Epic tokens, refresh tokens, captured traffic, or local debug dumps. When updating Rocket League protocol constants such as `game_version`, `feature_set`, or PsyNet headers, document the source of truth in the PR.
