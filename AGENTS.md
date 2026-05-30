# Repository Guidelines

## Project Structure & Module Organization

This repository is a Cargo workspace rooted at `Cargo.toml`. It contains two crates:

- **`crates/rlapi`** — Rust port of the Rocket League PsyNet client. Library code in `crates/rlapi/src/`, integration tests in `crates/rlapi/tests/`.
- **`apps/overlay/src-tauri`** — Tauri v2 desktop overlay backend. Library code in `apps/overlay/src-tauri/src/`, integration tests in `apps/overlay/src-tauri/tests/`.

The frontend lives in `apps/overlay/src/` (TypeScript + Vite). Build output goes to `target/` and `apps/overlay/dist/` and must stay untracked.

Keep new code close to the crate that owns it. Prefer splitting large areas into focused modules instead of extending a single large file.

## Build, Test, and Development Commands

Run commands from the repository root:

- `cargo build --workspace`: compile all crates.
- `cargo test --workspace`: run unit and integration tests.
- `cargo clippy --workspace --all-targets -- -D warnings`: enforce lint-clean code.
- `cargo fmt --check`: verify formatting.
- `cargo fmt`: apply standard Rust formatting.

For the overlay frontend:

```bash
cd apps/overlay
npm install
npm run build
```

Use the full verification suite before opening a PR or creating a release commit.

## Coding Style & Naming Conventions

Use standard Rust formatting with `rustfmt` and 4-space indentation. Follow idiomatic Rust naming:

- `snake_case` for functions, modules, and test names
- `PascalCase` for structs, enums, and traits
- `SCREAMING_SNAKE_CASE` for constants

Prefer `thiserror` for error types, `serde` for wire structs, and `tokio` async APIs for networked code. Keep public APIs typed; avoid unstructured `serde_json::Value` unless modeling raw PsyNet traffic.

For the frontend, use TypeScript with strict mode. Follow standard web naming conventions.

## Testing Guidelines

Use `#[test]` for pure protocol logic and `#[tokio::test]` for async network flows.

- `crates/rlapi/tests/` — PsyNet protocol and RPC integration tests
- `apps/overlay/src-tauri/tests/` — log replay and overlay backend integration tests

Every protocol or endpoint change should add or update tests. If you change PsyNet message formatting, signing, build ID decoding, or auth flow, include a regression test. If you change log parsing, match tracking, or enrichment, include a test in the overlay crate.

## Commit & Pull Request Guidelines

Use short imperative commits with a prefix, matching current history, for example `chore: initialize rocketstats workspace`. Common prefixes here are `feat:`, `fix:`, `test:`, `docs:`, and `chore:`.

PRs should include a concise summary, the commands you ran, and any protocol assumptions or upstream references used. Add screenshots when UI work is involved.

## Security & Configuration Tips

Do not commit personal Epic tokens, refresh tokens, captured traffic, or local debug dumps. The log redaction module (`apps/overlay/src-tauri/src/logs/redaction.rs`) strips sensitive tokens from Rocket League logs — verify redaction tests pass before changing it.

When updating Rocket League protocol constants such as `game_version`, `feature_set`, or PsyNet headers, document the source of truth in the PR.
