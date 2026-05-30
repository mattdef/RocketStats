# RocketStats

RocketStats is a Rust workspace for building a Rocket League overlay stack.

The first crate, `rocketstats-rlapi`, ports the reverse-engineered `rlapi` Go project into Rust and currently provides:

- Epic EOS device-code authentication
- PsyNet HTTP bootstrap and WebSocket RPC client
- Request signing and build ID decoding compatible with the upstream protocol
- Typed read-only endpoints for profiles, XP, playlists, population, skills, and match history
- Local protocol and RPC integration tests

## Workspace

```text
.
├── Cargo.toml
└── crates/
    └── rlapi/
```

## Development

Run the full verification suite from the repository root:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Current scope

This repository currently contains the Rust SDK layer only. The local backend and in-game overlay UI are still to be added on top of this crate.
