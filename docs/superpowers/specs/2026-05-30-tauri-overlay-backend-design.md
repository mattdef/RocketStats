# Tauri Overlay Backend Design

Date: 2026-05-30
Status: Approved design

## Context

RocketStats is currently a Rust workspace with one active crate, `rocketstats-rlapi`.
That crate already provides the protocol-sensitive layer: Epic EOS device-code auth,
PsyNet HTTP bootstrap, WebSocket RPC, typed PsyNet endpoints, request signing, and
local protocol tests.

The next layer is a local desktop companion for a Rocket League overlay. The overlay
requires a valid Epic/PsyNet session. The MVP intentionally excludes true live match
telemetry because BakkesMod is not an acceptable dependency under the current
EasyAntiCheat environment, and memory or network capture is not a healthy baseline.

Inspection of a real `Launch.log` showed that Rocket League logs are useful for local
match state detection. They expose matchmaking start, playlist, region, server join,
map load, match GUID, local score, match duration, XP, and match end markers. They can
also expose other players' Epic IDs within roughly a few seconds of server join, but
they do not reliably expose a complete roster, names, teams, or live scoreboard data.

## Goals

- Build a Tauri desktop app that can display a borderless transparent always-on-top
  overlay above Rocket League in borderless/windowed fullscreen mode.
- Keep the backend in Rust and reuse `rocketstats-rlapi` directly.
- Require a valid Epic/PsyNet session before overlay features are active.
- Watch Rocket League `Launch.log` and convert relevant lines into normalized events.
- Enrich detected player IDs through PsyNet profile and skill endpoints.
- Cache public player profile, rank/MMR snapshots, and post-match history locally.
- Treat Rocket League logs as sensitive input and never persist or expose raw lines.

## Non-Goals

- No injected in-game overlay.
- No dependency on BakkesMod.
- No process memory reading, packet capture, or anti-cheat-adjacent technique.
- No guarantee that detected log IDs represent a complete lobby.
- No cloud backend or user account system in the MVP.
- No real live scoreboard, ball, boost, goal, or positional telemetry in the MVP.

## Recommended Stack

- Desktop shell: Tauri v2.
- Backend runtime: Rust, Tokio, Tauri commands/events.
- Existing protocol client: `rocketstats-rlapi`.
- Log watching: `notify` plus a polling/tail fallback for file rotation and partial writes.
- Storage: SQLite through `sqlx`, using checked queries where practical.
- Serialization and errors: `serde`, `serde_json`, `thiserror`.
- Observability: `tracing` with explicit redaction for sensitive values.
- Frontend: TypeScript UI inside Tauri. The frontend framework is a UI concern and
  must not affect backend boundaries.

Tauri is selected because it can package the local Rust backend and provide an overlay
window with `transparent`, `decorations: false`, `alwaysOnTop`, `skipTaskbar`, and
click-through behavior through `setIgnoreCursorEvents`. The supported display target
is Rocket League in borderless/windowed fullscreen, not exclusive fullscreen.

## Architecture

The app should start as a single Tauri binary with an embedded Rust backend. The MVP
does not need an HTTP server between the UI and backend. Tauri commands are used for
request/response actions such as starting auth, listing cached players, changing
overlay settings, and selecting a log path. Tauri events are used for streaming state
updates such as auth state, match state, detected players, and enrichment results.

The backend owns all protocol and file-system work. The frontend is a renderer of
backend state and sends user intent through commands. This prevents UI code from
reading raw logs, holding tokens, or calling PsyNet directly.

## Components

- `auth`: Manages Epic device-code auth, token refresh, PsyNet connection, and exposes
  states such as `Unauthenticated`, `WaitingForDeviceCode`, `Connected`, `Expired`,
  and `Error`.
- `log_watcher`: Opens and tails `Launch.log`, handles rotation and partial writes,
  parses only known patterns, and emits normalized events.
- `match_tracker`: Converts log events into match sessions. It tracks matchmaking,
  server join, map, match GUID, detected player IDs, match end, local score, duration,
  and XP.
- `player_enrichment`: Deduplicates player IDs, skips the local player where needed,
  calls `get_profiles`, `get_players_skills`, and targeted skill endpoints, then
  publishes enriched player cards.
- `storage`: Persists SQLite cache data, including public profiles, skill snapshots,
  match summaries, event timestamps, and non-sensitive error metadata.
- `tauri_bridge`: Provides commands and events between Rust state and the frontend.
- `overlay_ui`: Displays auth status, current match status, detected player cards,
  rank/MMR information, data freshness, and partial-roster warnings.
- `settings_ui`: Lets the user configure log path, overlay placement, always-on-top,
  opacity, click-through, and safe keyboard shortcuts.

Each component should have a narrow interface. The log parser emits facts only. It
does not decide which players are important, when to call PsyNet, or what the overlay
should show.

## Data Flow

On app startup, Tauri initializes storage, settings, auth state, and the log watcher.
If no valid Epic/PsyNet session exists, the UI shows the login/device-code flow and
the overlay remains inactive.

When `Launch.log` changes, `log_watcher` emits events such as:

- `MatchmakingStarted { playlist, regions, timestamp }`
- `ServerReserved { server_name, region, playlist, timestamp }`
- `ServerJoined { map, server, timestamp }`
- `MatchGuidSeen { guid, timestamp }`
- `PlayerIdSeen { player_id, timestamp }`
- `MatchEnded { guid, local_score, duration, xp, timestamp }`

`match_tracker` associates these events with the current match session. When a
non-local `PlayerIdSeen` arrives, `player_enrichment` checks cache freshness, calls
PsyNet when needed, persists the result, and emits an overlay state update.

After `MatchEnded`, the backend schedules a post-match refresh through
`get_match_history()`. If PsyNet returns the match, the stored match summary is marked
confirmed. If not, the log-derived summary remains partial and can be retried later.

## Overlay Behavior

The overlay window is transparent, borderless, always on top, and hidden from the
taskbar by default. Click-through should be enabled during normal play and must be
toggleable through a keyboard shortcut or configuration window. The settings window
must remain recoverable even if the overlay is click-through.

The player display must use precise wording:

- "Detected players" is allowed.
- "Full lobby" is not allowed unless a future data source proves completeness.
- Stale cached data must show an age or freshness indicator.
- PsyNet failures should degrade to cached data where possible.

## Security Rules

Rocket League logs are sensitive. The real log sample contained Epic launch secrets,
Epic account IDs, DSR tokens, join passwords, datarouter URLs, server details, and
other session data.

The backend must enforce these rules:

- Never expose raw log lines to the frontend.
- Never store raw log lines in SQLite.
- Never write raw log lines to application logs.
- Redact Epic launch secrets, DSR tokens, join credentials, JWT-like values, account
  IDs, and server join passwords in all diagnostic output.
- Persist only data the product needs: player IDs, public profile fields, public skill
  data, match GUID, playlist, timestamps, local score, duration, XP, and structured
  non-sensitive error codes.
- Treat the local SQLite database as user-local private data and do not sync it in the
  MVP.

## Error Handling

If Epic/PsyNet auth is missing or expired, the backend stops player enrichment and the
UI moves to an auth-required state. The log watcher can keep parsing match state, but
the overlay must not claim stats are available.

If `Launch.log` is missing, moved, or rotated, the watcher retries and the UI shows
that Rocket League logs are not currently detected. Rotation must not create duplicate
match sessions.

If PsyNet rate-limits or fails, the backend records a structured non-sensitive error,
uses cached data when available, and emits a freshness warning. It should avoid
aggressive retry loops.

If the overlay window becomes hard to manipulate because click-through is enabled, a
global shortcut or separate settings window must allow the user to disable it.

## Testing And Validation

Unit tests should cover:

- `Launch.log` parser patterns for matchmaking, server join, match GUID, detected
  player IDs, match end, local score, duration, and XP.
- Redaction of sensitive tokens and IDs.
- File rotation and partial-line handling.
- `match_tracker` state transitions and duplicate suppression.
- `player_enrichment` behavior using a mocked PsyNet client.

Integration tests should replay anonymized log fixtures and verify normalized events,
overlay state transitions, and SQLite persistence. The real `.tmp/Launch.log` must not
be committed.

A manual Tauri spike is required before considering the overlay display path proven:

- Transparent borderless window.
- Always-on-top behavior.
- Click-through behavior.
- Recoverable settings controls.
- Visibility above Rocket League in borderless/windowed fullscreen.

## Implementation Shape

The workspace should grow by adding a Tauri app crate or app directory that depends on
`rocketstats-rlapi`. The protocol crate should remain focused on PsyNet/Epic protocol
logic. Overlay-specific concerns belong in the Tauri app/backend modules.

Expected high-level structure:

```text
crates/
  rlapi/
apps/
  overlay/
    src-tauri/
    src/
```

If Cargo workspace ergonomics make `apps/overlay/src-tauri` awkward, the Rust backend
can live in `crates/overlayd` and the Tauri shell can depend on it. The key boundary is
that protocol code stays in `rocketstats-rlapi`, while overlay orchestration stays out
of that crate.

## Key Assumptions

- The user is willing to authenticate with Epic/PsyNet before using overlay stats.
- Rocket League is run in borderless/windowed fullscreen for overlay visibility.
- Log-derived player IDs are opportunistic and may be incomplete.
- PsyNet profile and skill endpoints can enrich detected `PlayerId` values sufficiently
  for the first useful overlay.
- Post-match detail should prefer PsyNet match history when available and fall back to
  log-derived summaries when necessary.
