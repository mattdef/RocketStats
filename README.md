# RocketStats

RocketStats is a Rust workspace for building a Rocket League overlay stack.

## Workspace

```text
.
├── Cargo.toml
├── apps/
│   └── overlay/          # Tauri v2 desktop overlay
│       ├── src/           # TypeScript frontend (Vite)
│       └── src-tauri/     # Rust backend
└── crates/
    └── rlapi/             # PsyNet protocol client
```

### `rocketstats-rlapi`

Ports the reverse-engineered `rlapi` Go project into Rust:

- Epic EOS device-code authentication
- PsyNet HTTP bootstrap and WebSocket RPC client
- Request signing and build ID decoding compatible with the upstream protocol
- Typed read-only endpoints for profiles, XP, playlists, population, skills, and match history
- Local protocol and RPC integration tests

### `rocketstats-overlay`

Tauri v2 desktop overlay with a Rust backend and TypeScript frontend:

- **Log parsing** — extracts match events from Rocket League `Launch.log` (matchmaking, server join, GUID, player IDs, match end)
- **Log redaction** — strips sensitive tokens (AUTH_PASSWORD, epicuserid, JoinPassword, DSRToken, ConnectionID) before processing
- **Match tracker** — state machine that builds a `MatchSession` from log events (Idle → Matchmaking → Joining → InMatch → Ended)
- **Player enrichment** — looks up detected players through PsyNet skill/profile endpoints with local caching
- **SQLite storage** — persists player cards (MMR, rank, division) across sessions
- **Auth service** — orchestrates Epic device login and PsyNet WebSocket connection
- **Tauri bridge** — exposes `get_overlay_state` and `set_click_through` commands with event-based state updates
- **Overlay UI** — transparent borderless window rendering player cards and match status

## Development

Run the full verification suite from the repository root:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For the frontend:

```bash
cd apps/overlay
npm install
npm run build
```

## Current scope

The workspace contains the PsyNet protocol client and a Tauri overlay MVP. The overlay backend provides log parsing, match tracking, player enrichment, and SQLite caching. Background task orchestration (spawning the log watcher, auth flow, and enrichment pipeline) and BakkesMod/injected overlay integration are still to be implemented.
