# Tauri Overlay Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first RocketStats Tauri desktop overlay with a Rust backend that requires Epic/PsyNet auth, watches Rocket League logs, detects match/player events, enriches detected players through `rocketstats-rlapi`, and displays safe overlay state.

**Architecture:** Add a Tauri app under `apps/overlay` with Rust backend modules inside `apps/overlay/src-tauri/src`. Keep protocol code in `crates/rlapi`; the Tauri backend owns auth orchestration, log parsing, match tracking, storage, enrichment, and UI bridge events. Use Tauri commands/events instead of an HTTP server for the MVP.

**Tech Stack:** Rust 2024, Tokio, Tauri v2, TypeScript, SQLite via `sqlx`, `serde`, `thiserror`, `tracing`, `notify`, existing `rocketstats-rlapi`.

---

## Scope

This plan implements the MVP backend and a minimal overlay UI. It does not implement an injected overlay, BakkesMod integration, memory scanning, packet capture, cloud sync, or complete live scoreboard telemetry.

The implementation is split into testable slices. Each task should be committed separately after its verification commands pass.

## File Structure

- Modify `Cargo.toml`: add `apps/overlay/src-tauri` as a workspace member.
- Create `apps/overlay/package.json`: frontend scripts and Tauri dev/build commands.
- Create `apps/overlay/index.html`: Vite entry point.
- Create `apps/overlay/src/main.ts`: frontend bootstrap.
- Create `apps/overlay/src/styles.css`: overlay visual styling.
- Create `apps/overlay/src/state.ts`: frontend event state types.
- Create `apps/overlay/src-tauri/Cargo.toml`: Tauri Rust package and dependencies.
- Create `apps/overlay/src-tauri/tauri.conf.json`: Tauri window configuration.
- Create `apps/overlay/src-tauri/src/main.rs`: Tauri app entry point.
- Create `apps/overlay/src-tauri/src/lib.rs`: module wiring and app builder.
- Create `apps/overlay/src-tauri/src/domain.rs`: shared backend domain types.
- Create `apps/overlay/src-tauri/src/error.rs`: backend error type.
- Create `apps/overlay/src-tauri/src/logs/mod.rs`: log module exports.
- Create `apps/overlay/src-tauri/src/logs/redaction.rs`: sensitive log redaction.
- Create `apps/overlay/src-tauri/src/logs/parser.rs`: `Launch.log` line parser.
- Create `apps/overlay/src-tauri/src/logs/watcher.rs`: tailing watcher with rotation support.
- Create `apps/overlay/src-tauri/src/match_tracker.rs`: match session state machine.
- Create `apps/overlay/src-tauri/src/storage.rs`: SQLite schema and repository.
- Create `apps/overlay/src-tauri/src/enrichment.rs`: player enrichment orchestration.
- Create `apps/overlay/src-tauri/src/auth.rs`: Epic/PsyNet auth orchestration.
- Create `apps/overlay/src-tauri/src/bridge.rs`: Tauri commands and emitted UI state.
- Create `apps/overlay/src-tauri/tests/fixtures/launch_excerpt.log`: anonymized log replay fixture.
- Create `apps/overlay/src-tauri/tests/log_replay.rs`: integration test for log replay.

## Task 1: Scaffold Tauri Workspace App

**Files:**
- Modify: `Cargo.toml`
- Create: `apps/overlay/package.json`
- Create: `apps/overlay/index.html`
- Create: `apps/overlay/src/main.ts`
- Create: `apps/overlay/src/styles.css`
- Create: `apps/overlay/src-tauri/Cargo.toml`
- Create: `apps/overlay/src-tauri/tauri.conf.json`
- Create: `apps/overlay/src-tauri/src/main.rs`
- Create: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Add the Tauri Rust package to the Cargo workspace**

Edit root `Cargo.toml` so the workspace members are:

```toml
[workspace]
members = ["crates/rlapi", "apps/overlay/src-tauri"]
resolver = "3"
```

- [ ] **Step 2: Create the frontend package manifest**

Create `apps/overlay/package.json`:

```json
{
  "name": "rocketstats-overlay",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1",
    "build": "tsc --noEmit && vite build",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0"
  }
}
```

- [ ] **Step 3: Create the minimal frontend files**

Create `apps/overlay/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>RocketStats Overlay</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Create `apps/overlay/src/main.ts`:

```ts
import "./styles.css";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("missing #app root");
}

app.innerHTML = `
  <main class="overlay-shell">
    <section class="panel">
      <p class="eyebrow">RocketStats</p>
      <h1>Overlay starting</h1>
      <p class="muted">Waiting for backend state.</p>
    </section>
  </main>
`;
```

Create `apps/overlay/src/styles.css`:

```css
:root {
  color: #f5f1e8;
  background: transparent;
  font-family: "Space Grotesk", "Segoe UI", sans-serif;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
  background: transparent;
}

.overlay-shell {
  min-height: 100%;
  display: flex;
  align-items: flex-start;
  justify-content: flex-end;
  padding: 28px;
  box-sizing: border-box;
}

.panel {
  width: 360px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 22px;
  padding: 18px 20px;
  background:
    linear-gradient(135deg, rgba(18, 22, 28, 0.88), rgba(35, 43, 48, 0.68)),
    radial-gradient(circle at 20% 10%, rgba(255, 180, 80, 0.22), transparent 36%);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.38);
  backdrop-filter: blur(16px);
}

.eyebrow {
  margin: 0 0 8px;
  color: #ffcf75;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  font-size: 12px;
}

h1 {
  margin: 0 0 8px;
  font-size: 28px;
}

.muted {
  margin: 0;
  color: rgba(245, 241, 232, 0.74);
}
```

- [ ] **Step 4: Create the Tauri Rust manifest**

Create `apps/overlay/src-tauri/Cargo.toml`:

```toml
[package]
name = "rocketstats-overlay"
version = "0.1.0"
edition = "2024"
license = "MIT"
description = "RocketStats local Tauri overlay"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
notify = "7"
rocketstats-rlapi = { path = "../../../crates/rlapi" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "macros", "migrate", "chrono"] }
tauri = { version = "2", features = [] }
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
url = "2"

[dev-dependencies]
tempfile = "3"
```

Create `apps/overlay/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 5: Create Tauri config and Rust entry points**

Create `apps/overlay/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "RocketStats",
  "version": "0.1.0",
  "identifier": "com.rocketstats.overlay",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://127.0.0.1:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "RocketStats Overlay",
        "width": 1280,
        "height": 720,
        "transparent": true,
        "decorations": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

Create `apps/overlay/src-tauri/src/main.rs`:

```rust
fn main() {
    rocketstats_overlay::run();
}
```

Create `apps/overlay/src-tauri/src/lib.rs`:

```rust
use tracing_subscriber::{EnvFilter, fmt};

pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_ignore_cursor_events(true)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run RocketStats overlay");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rocketstats_overlay=info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
```

- [ ] **Step 6: Verify workspace compilation gets to dependency resolution**

Run: `cargo check --workspace`

Expected: Either PASS, or a dependency download error if dependencies are not installed in the sandbox. If the command fails because dependency fetching is blocked, rerun it with escalated permissions during execution.

- [ ] **Step 7: Commit scaffold**

```bash
git add Cargo.toml apps/overlay
git commit -m "feat: scaffold tauri overlay app"
```

## Task 2: Add Backend Domain Types And Errors

**Files:**
- Create: `apps/overlay/src-tauri/src/domain.rs`
- Create: `apps/overlay/src-tauri/src/error.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Add domain types**

Create `apps/overlay/src-tauri/src/domain.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthState {
    Unauthenticated,
    WaitingForDeviceCode {
        user_code: String,
        verification_uri: String,
        expires_in: u64,
    },
    Connected {
        account_id: String,
        player_name: Option<String>,
    },
    Expired,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchPhase {
    Idle,
    Matchmaking,
    Joining,
    InMatch,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedPlayerId {
    pub value: String,
    pub first_seen_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogEvent {
    MatchmakingStarted {
        playlist: i32,
        regions: Vec<String>,
        timestamp_ms: u64,
    },
    ServerReserved {
        server_name: String,
        region: String,
        playlist: i32,
        timestamp_ms: u64,
    },
    ServerJoined {
        map: String,
        server: String,
        timestamp_ms: u64,
    },
    MatchGuidSeen {
        guid: String,
        timestamp_ms: u64,
    },
    PlayerIdSeen {
        player_id: String,
        timestamp_ms: u64,
    },
    MatchEnded {
        guid: Option<String>,
        local_score: i32,
        duration_seconds: f64,
        xp: Option<i32>,
        timestamp_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchSession {
    pub phase: MatchPhase,
    pub playlist: Option<i32>,
    pub regions: Vec<String>,
    pub server_name: Option<String>,
    pub map: Option<String>,
    pub guid: Option<String>,
    pub detected_players: Vec<DetectedPlayerId>,
    pub local_score: Option<i32>,
    pub duration_seconds: Option<f64>,
    pub xp: Option<i32>,
}

impl Default for MatchSession {
    fn default() -> Self {
        Self {
            phase: MatchPhase::Idle,
            playlist: None,
            regions: Vec::new(),
            server_name: None,
            map: None,
            guid: None,
            detected_players: Vec::new(),
            local_score: None,
            duration_seconds: None,
            xp: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerCard {
    pub player_id: String,
    pub name: Option<String>,
    pub playlist: Option<i32>,
    pub mmr: Option<f64>,
    pub tier: Option<i32>,
    pub division: Option<i32>,
    pub data_age_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayState {
    pub auth: AuthState,
    pub match_session: MatchSession,
    pub players: Vec<PlayerCard>,
    pub partial_roster: bool,
    pub status_message: String,
}
```

- [ ] **Step 2: Add the backend error type**

Create `apps/overlay/src-tauri/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("rocket league api error: {0}")]
    RlApi(#[from] rocketstats_rlapi::RlApiError),
    #[error("invalid log line: {0}")]
    InvalidLogLine(String),
    #[error("auth is required")]
    AuthRequired,
}

pub type Result<T> = std::result::Result<T, OverlayError>;
```

- [ ] **Step 3: Wire the modules**

Edit `apps/overlay/src-tauri/src/lib.rs` to include:

```rust
pub mod domain;
pub mod error;

use tracing_subscriber::{EnvFilter, fmt};

pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_ignore_cursor_events(true)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run RocketStats overlay");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rocketstats_overlay=info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
```

- [ ] **Step 4: Verify**

Run: `cargo test --workspace`

Expected: Existing `rocketstats-rlapi` tests pass and the new Tauri crate compiles.

- [ ] **Step 5: Commit**

```bash
git add apps/overlay/src-tauri/src/domain.rs apps/overlay/src-tauri/src/error.rs apps/overlay/src-tauri/src/lib.rs
git commit -m "feat: add overlay backend domain types"
```

## Task 3: Implement Redaction And Launch Log Parser

**Files:**
- Create: `apps/overlay/src-tauri/src/logs/mod.rs`
- Create: `apps/overlay/src-tauri/src/logs/redaction.rs`
- Create: `apps/overlay/src-tauri/src/logs/parser.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Write redaction tests first**

Create `apps/overlay/src-tauri/src/logs/redaction.rs` with tests and a stub:

```rust
pub fn redact_sensitive(input: &str) -> String {
    input.to_owned()
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive;

    #[test]
    fn redacts_epic_launch_and_join_secrets() {
        let line = "Command line: -AUTH_PASSWORD=5139003f31b04a6ba73e914a8860125a -epicuserid=7efc351e447043c4be4447da51b790e4 JoinPassword=\"UM1H9ZNCNIYWKZSC\" JoinCredentials=\"10PJB3O2SM6ZR8JH:UM1H9ZNCNIYWKZSC\"";

        let redacted = redact_sensitive(line);

        assert!(!redacted.contains("5139003f31b04a6ba73e914a8860125a"));
        assert!(!redacted.contains("7efc351e447043c4be4447da51b790e4"));
        assert!(!redacted.contains("UM1H9ZNCNIYWKZSC"));
        assert!(redacted.contains("AUTH_PASSWORD=<redacted>"));
        assert!(redacted.contains("epicuserid=<redacted>"));
        assert!(redacted.contains("JoinPassword=\"<redacted>\""));
        assert!(redacted.contains("JoinCredentials=\"<redacted>\""));
    }

    #[test]
    fn redacts_jwt_like_tokens() {
        let line = "DSRToken=\"aaa.bbb.ccc\" ConnectionID=(aaa.bbb.ccc)";

        let redacted = redact_sensitive(line);

        assert_eq!(
            redacted,
            "DSRToken=\"<redacted>\" ConnectionID=(<redacted>)"
        );
    }
}
```

- [ ] **Step 2: Run redaction tests and verify failure**

Run: `cargo test -p rocketstats-overlay logs::redaction -- --nocapture`

Expected: FAIL because `redact_sensitive` returns the original input.

- [ ] **Step 3: Implement redaction**

Replace `apps/overlay/src-tauri/src/logs/redaction.rs` with:

```rust
fn replace_token_after_prefix(input: &str, prefix: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(prefix) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(prefix);

        let value_start = prefix.len();
        let after_prefix = &after_before[value_start..];
        let value_len = after_prefix
            .find(char::is_whitespace)
            .unwrap_or(after_prefix.len());
        output.push_str("<redacted>");
        remaining = &after_prefix[value_len..];
    }

    output.push_str(remaining);
    output
}

fn replace_quoted_value(input: &str, key: &str) -> String {
    let pattern = format!("{key}=\"");
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(&pattern) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(&pattern);

        let after_pattern = &after_before[pattern.len()..];
        if let Some(end_quote) = after_pattern.find('"') {
            output.push_str("<redacted>\"");
            remaining = &after_pattern[end_quote + 1..];
        } else {
            output.push_str("<redacted>");
            remaining = "";
        }
    }

    output.push_str(remaining);
    output
}

fn replace_parenthesized_value(input: &str, key: &str) -> String {
    let pattern = format!("{key}=(");
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(&pattern) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(&pattern);

        let after_pattern = &after_before[pattern.len()..];
        if let Some(end_paren) = after_pattern.find(')') {
            output.push_str("<redacted>)");
            remaining = &after_pattern[end_paren + 1..];
        } else {
            output.push_str("<redacted>");
            remaining = "";
        }
    }

    output.push_str(remaining);
    output
}

pub fn redact_sensitive(input: &str) -> String {
    let mut redacted = input.to_owned();
    for prefix in [
        "AUTH_PASSWORD=",
        "-AUTH_PASSWORD=",
        "epicuserid=",
        "-epicuserid=",
        "epicusername=",
        "-epicusername=",
    ] {
        redacted = replace_token_after_prefix(&redacted, prefix);
    }

    for key in ["DSRToken", "JoinPassword", "JoinCredentials"] {
        redacted = replace_quoted_value(&redacted, key);
    }

    replace_parenthesized_value(&redacted, "ConnectionID")
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive;

    #[test]
    fn redacts_epic_launch_and_join_secrets() {
        let line = "Command line: -AUTH_PASSWORD=5139003f31b04a6ba73e914a8860125a -epicuserid=7efc351e447043c4be4447da51b790e4 JoinPassword=\"UM1H9ZNCNIYWKZSC\" JoinCredentials=\"10PJB3O2SM6ZR8JH:UM1H9ZNCNIYWKZSC\"";

        let redacted = redact_sensitive(line);

        assert!(!redacted.contains("5139003f31b04a6ba73e914a8860125a"));
        assert!(!redacted.contains("7efc351e447043c4be4447da51b790e4"));
        assert!(!redacted.contains("UM1H9ZNCNIYWKZSC"));
        assert!(redacted.contains("AUTH_PASSWORD=<redacted>"));
        assert!(redacted.contains("epicuserid=<redacted>"));
        assert!(redacted.contains("JoinPassword=\"<redacted>\""));
        assert!(redacted.contains("JoinCredentials=\"<redacted>\""));
    }

    #[test]
    fn redacts_jwt_like_tokens() {
        let line = "DSRToken=\"aaa.bbb.ccc\" ConnectionID=(aaa.bbb.ccc)";

        let redacted = redact_sensitive(line);

        assert_eq!(
            redacted,
            "DSRToken=\"<redacted>\" ConnectionID=(<redacted>)"
        );
    }
}
```

- [ ] **Step 4: Add parser tests and stub**

Create `apps/overlay/src-tauri/src/logs/parser.rs`:

```rust
use crate::domain::LogEvent;

pub fn parse_log_line(_line: &str) -> Option<LogEvent> {
    None
}

#[cfg(test)]
mod tests {
    use super::parse_log_line;
    use crate::domain::LogEvent;

    #[test]
    fn parses_matchmaking_started() {
        let line = "[0223.91] Matchmaking: StartMatchmaking at 2026-05-29 23:37:55 in EU9,EU7,EU5,EU3,EU1 for playlists 11 on game server";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchmakingStarted {
                playlist: 11,
                regions: vec![
                    "EU9".to_owned(),
                    "EU7".to_owned(),
                    "EU5".to_owned(),
                    "EU3".to_owned(),
                    "EU1".to_owned()
                ],
                timestamp_ms: 223_910,
            })
        );
    }

    #[test]
    fn parses_server_joined() {
        let line = "[0238.79] DevNet: Welcomed by server (Level: FF_Dusk_P, Game: TAGame.GameInfo_Soccar_TA, GameTags: )";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::ServerJoined {
                map: "FF_Dusk_P".to_owned(),
                server: "unknown".to_owned(),
                timestamp_ms: 238_790,
            })
        );
    }

    #[test]
    fn parses_match_guid() {
        let line = "[0240.99] ScriptLog: MatchGUID: 706DA47C11F15BB7CB1952B6DEE4DFF5";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchGuidSeen {
                guid: "706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned(),
                timestamp_ms: 240_990,
            })
        );
    }

    #[test]
    fn parses_player_id() {
        let line = "[0643.74] ScriptLog: Uncached PlatformId for Epic|0123456789abcdef0123456789abcdef|0";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::PlayerIdSeen {
                player_id: "Epic|0123456789abcdef0123456789abcdef|0".to_owned(),
                timestamp_ms: 643_740,
            })
        );
    }

    #[test]
    fn parses_match_end_without_xp() {
        let line = "[0618.72] XPProgression: GFxHUD_TA.HandleGameStateChanged Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.2381 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: None,
                local_score: 323,
                duration_seconds: 302.2381,
                xp: None,
                timestamp_ms: 618_720,
            })
        );
    }

    #[test]
    fn parses_match_end_with_xp_and_guid() {
        let line = "[0619.02] XPProgression: SaveData_TA.HandleRewardDropNotification PsyNetService_RewardDropReceived_TA returned Total XP Earned = 5160.0000, in match with ID = 706DA47C11F15BB7CB1952B6DEE4DFF5 , Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.9711 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
                local_score: 323,
                duration_seconds: 302.9711,
                xp: Some(5160),
                timestamp_ms: 619_020,
            })
        );
    }
}
```

- [ ] **Step 5: Run parser tests and verify failure**

Run: `cargo test -p rocketstats-overlay logs::parser -- --nocapture`

Expected: FAIL because `parse_log_line` returns `None`.

- [ ] **Step 6: Implement parser with focused helper functions**

Replace `apps/overlay/src-tauri/src/logs/parser.rs` with:

```rust
use crate::domain::LogEvent;

pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    let timestamp_ms = parse_timestamp_ms(line)?;
    let body = line.split_once("] ")?.1;

    parse_matchmaking_started(body, timestamp_ms)
        .or_else(|| parse_server_joined(body, timestamp_ms))
        .or_else(|| parse_match_guid(body, timestamp_ms))
        .or_else(|| parse_player_id(body, timestamp_ms))
        .or_else(|| parse_match_end_with_xp(body, timestamp_ms))
        .or_else(|| parse_match_end_without_xp(body, timestamp_ms))
}

fn parse_timestamp_ms(line: &str) -> Option<u64> {
    let timestamp = line.strip_prefix('[')?.split_once(']')?.0;
    let (seconds, fractional) = timestamp.split_once('.')?;
    let seconds = seconds.parse::<u64>().ok()?;
    let millis = match fractional.len() {
        0 => 0,
        1 => fractional.parse::<u64>().ok()? * 100,
        2 => fractional.parse::<u64>().ok()? * 10,
        _ => fractional.get(0..3)?.parse::<u64>().ok()?,
    };
    Some(seconds * 1000 + millis)
}

fn parse_matchmaking_started(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "Matchmaking: StartMatchmaking at ";
    if !body.starts_with(marker) {
        return None;
    }
    let regions_part = body.split_once(" in ")?.1.split_once(" for playlists ")?.0;
    let playlist_part = body.split_once(" for playlists ")?.1.split_once(" on ")?.0;
    let regions = regions_part
        .split(',')
        .filter(|region| !region.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let playlist = playlist_part.parse::<i32>().ok()?;
    Some(LogEvent::MatchmakingStarted {
        playlist,
        regions,
        timestamp_ms,
    })
}

fn parse_server_joined(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "DevNet: Welcomed by server (Level: ";
    if !body.starts_with(marker) {
        return None;
    }
    let map = body
        .strip_prefix(marker)?
        .split_once(", Game:")?
        .0
        .to_owned();
    Some(LogEvent::ServerJoined {
        map,
        server: "unknown".to_owned(),
        timestamp_ms,
    })
}

fn parse_match_guid(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let guid = body.strip_prefix("ScriptLog: MatchGUID: ")?;
    Some(LogEvent::MatchGuidSeen {
        guid: guid.trim().to_owned(),
        timestamp_ms,
    })
}

fn parse_player_id(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "ScriptLog: Uncached PlatformId for ";
    let player_id = body.strip_prefix(marker)?;
    if !player_id.contains('|') {
        return None;
    }
    Some(LogEvent::PlayerIdSeen {
        player_id: player_id.trim().to_owned(),
        timestamp_ms,
    })
}

fn parse_match_end_without_xp(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    if !body.starts_with("XPProgression: GFxHUD_TA.HandleGameStateChanged ") {
        return None;
    }
    let local_score = parse_i32_between(body, "Current player match score = ", ",")?;
    let duration_seconds = parse_f64_between(body, "with total match time = ", " seconds")?;
    Some(LogEvent::MatchEnded {
        guid: None,
        local_score,
        duration_seconds,
        xp: None,
        timestamp_ms,
    })
}

fn parse_match_end_with_xp(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    if !body.starts_with("XPProgression: SaveData_TA.HandleRewardDropNotification ") {
        return None;
    }
    let xp = parse_f64_between(body, "Total XP Earned = ", ",")? as i32;
    let guid = parse_str_between(body, "in match with ID = ", " ,")?.to_owned();
    let local_score = parse_i32_between(body, "Current player match score = ", ",")?;
    let duration_seconds = parse_f64_between(body, "with total match time = ", " seconds")?;
    Some(LogEvent::MatchEnded {
        guid: Some(guid),
        local_score,
        duration_seconds,
        xp: Some(xp),
        timestamp_ms,
    })
}

fn parse_i32_between(input: &str, start: &str, end: &str) -> Option<i32> {
    parse_str_between(input, start, end)?.trim().parse().ok()
}

fn parse_f64_between(input: &str, start: &str, end: &str) -> Option<f64> {
    parse_str_between(input, start, end)?.trim().parse().ok()
}

fn parse_str_between<'a>(input: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let after_start = input.split_once(start)?.1;
    Some(after_start.split_once(end)?.0)
}

#[cfg(test)]
mod tests {
    use super::parse_log_line;
    use crate::domain::LogEvent;

    #[test]
    fn parses_matchmaking_started() {
        let line = "[0223.91] Matchmaking: StartMatchmaking at 2026-05-29 23:37:55 in EU9,EU7,EU5,EU3,EU1 for playlists 11 on game server";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchmakingStarted {
                playlist: 11,
                regions: vec![
                    "EU9".to_owned(),
                    "EU7".to_owned(),
                    "EU5".to_owned(),
                    "EU3".to_owned(),
                    "EU1".to_owned()
                ],
                timestamp_ms: 223_910,
            })
        );
    }

    #[test]
    fn parses_server_joined() {
        let line = "[0238.79] DevNet: Welcomed by server (Level: FF_Dusk_P, Game: TAGame.GameInfo_Soccar_TA, GameTags: )";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::ServerJoined {
                map: "FF_Dusk_P".to_owned(),
                server: "unknown".to_owned(),
                timestamp_ms: 238_790,
            })
        );
    }

    #[test]
    fn parses_match_guid() {
        let line = "[0240.99] ScriptLog: MatchGUID: 706DA47C11F15BB7CB1952B6DEE4DFF5";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchGuidSeen {
                guid: "706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned(),
                timestamp_ms: 240_990,
            })
        );
    }

    #[test]
    fn parses_player_id() {
        let line = "[0643.74] ScriptLog: Uncached PlatformId for Epic|0123456789abcdef0123456789abcdef|0";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::PlayerIdSeen {
                player_id: "Epic|0123456789abcdef0123456789abcdef|0".to_owned(),
                timestamp_ms: 643_740,
            })
        );
    }

    #[test]
    fn parses_match_end_without_xp() {
        let line = "[0618.72] XPProgression: GFxHUD_TA.HandleGameStateChanged Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.2381 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: None,
                local_score: 323,
                duration_seconds: 302.2381,
                xp: None,
                timestamp_ms: 618_720,
            })
        );
    }

    #[test]
    fn parses_match_end_with_xp_and_guid() {
        let line = "[0619.02] XPProgression: SaveData_TA.HandleRewardDropNotification PsyNetService_RewardDropReceived_TA returned Total XP Earned = 5160.0000, in match with ID = 706DA47C11F15BB7CB1952B6DEE4DFF5 , Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.9711 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
                local_score: 323,
                duration_seconds: 302.9711,
                xp: Some(5160),
                timestamp_ms: 619_020,
            })
        );
    }
}
```

- [ ] **Step 7: Export logs modules and wire lib**

Create `apps/overlay/src-tauri/src/logs/mod.rs`:

```rust
pub mod parser;
pub mod redaction;
pub mod watcher;
```

Create `apps/overlay/src-tauri/src/logs/watcher.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LogWatcherConfig {
    pub path: PathBuf,
}
```

Edit `apps/overlay/src-tauri/src/lib.rs` and add:

```rust
pub mod logs;
```

- [ ] **Step 8: Verify parser and redaction tests**

Run: `cargo test -p rocketstats-overlay logs -- --nocapture`

Expected: PASS for parser and redaction tests.

- [ ] **Step 9: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/logs
git commit -m "feat: parse rocket league launch log events"
```

## Task 4: Implement Match Tracker State Machine

**Files:**
- Create: `apps/overlay/src-tauri/src/match_tracker.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Write match tracker tests first**

Create `apps/overlay/src-tauri/src/match_tracker.rs`:

```rust
use crate::domain::{LogEvent, MatchSession};

#[derive(Debug, Default)]
pub struct MatchTracker {
    session: MatchSession,
}

impl MatchTracker {
    pub fn apply(&mut self, _event: LogEvent) -> MatchSession {
        self.session.clone()
    }

    pub fn session(&self) -> &MatchSession {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::MatchTracker;
    use crate::domain::{DetectedPlayerId, LogEvent, MatchPhase, MatchSession};

    #[test]
    fn tracks_match_lifecycle_and_deduplicates_players() {
        let mut tracker = MatchTracker::default();

        tracker.apply(LogEvent::MatchmakingStarted {
            playlist: 11,
            regions: vec!["EU9".to_owned(), "EU7".to_owned()],
            timestamp_ms: 223_910,
        });
        tracker.apply(LogEvent::ServerJoined {
            map: "FF_Dusk_P".to_owned(),
            server: "unknown".to_owned(),
            timestamp_ms: 238_790,
        });
        tracker.apply(LogEvent::MatchGuidSeen {
            guid: "706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned(),
            timestamp_ms: 240_990,
        });
        tracker.apply(LogEvent::PlayerIdSeen {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            timestamp_ms: 241_000,
        });
        tracker.apply(LogEvent::PlayerIdSeen {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            timestamp_ms: 241_500,
        });
        let session = tracker.apply(LogEvent::MatchEnded {
            guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
            local_score: 323,
            duration_seconds: 302.9711,
            xp: Some(5160),
            timestamp_ms: 619_020,
        });

        assert_eq!(
            session,
            MatchSession {
                phase: MatchPhase::Ended,
                playlist: Some(11),
                regions: vec!["EU9".to_owned(), "EU7".to_owned()],
                server_name: None,
                map: Some("FF_Dusk_P".to_owned()),
                guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
                detected_players: vec![DetectedPlayerId {
                    value: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                    first_seen_ms: 241_000,
                }],
                local_score: Some(323),
                duration_seconds: Some(302.9711),
                xp: Some(5160),
            }
        );
    }
}
```

- [ ] **Step 2: Run match tracker tests and verify failure**

Run: `cargo test -p rocketstats-overlay match_tracker -- --nocapture`

Expected: FAIL because `apply` does not mutate state.

- [ ] **Step 3: Implement match tracker**

Replace the implementation section in `apps/overlay/src-tauri/src/match_tracker.rs` with:

```rust
use crate::domain::{DetectedPlayerId, LogEvent, MatchPhase, MatchSession};

#[derive(Debug, Default)]
pub struct MatchTracker {
    session: MatchSession,
}

impl MatchTracker {
    pub fn apply(&mut self, event: LogEvent) -> MatchSession {
        match event {
            LogEvent::MatchmakingStarted {
                playlist,
                regions,
                timestamp_ms: _,
            } => {
                self.session = MatchSession::default();
                self.session.phase = MatchPhase::Matchmaking;
                self.session.playlist = Some(playlist);
                self.session.regions = regions;
            }
            LogEvent::ServerReserved {
                server_name,
                region,
                playlist,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Joining;
                self.session.server_name = Some(server_name);
                self.session.playlist = Some(playlist);
                if self.session.regions.is_empty() {
                    self.session.regions.push(region);
                }
            }
            LogEvent::ServerJoined {
                map,
                server,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Joining;
                self.session.map = Some(map);
                if server != "unknown" {
                    self.session.server_name = Some(server);
                }
            }
            LogEvent::MatchGuidSeen { guid, timestamp_ms: _ } => {
                self.session.phase = MatchPhase::InMatch;
                self.session.guid = Some(guid);
            }
            LogEvent::PlayerIdSeen {
                player_id,
                timestamp_ms,
            } => {
                if !self
                    .session
                    .detected_players
                    .iter()
                    .any(|existing| existing.value == player_id)
                {
                    self.session.detected_players.push(DetectedPlayerId {
                        value: player_id,
                        first_seen_ms: timestamp_ms,
                    });
                }
            }
            LogEvent::MatchEnded {
                guid,
                local_score,
                duration_seconds,
                xp,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Ended;
                if guid.is_some() {
                    self.session.guid = guid;
                }
                self.session.local_score = Some(local_score);
                self.session.duration_seconds = Some(duration_seconds);
                if xp.is_some() {
                    self.session.xp = xp;
                }
            }
        }

        self.session.clone()
    }

    pub fn session(&self) -> &MatchSession {
        &self.session
    }
}
```

Keep the test module from Step 1 in the same file.

- [ ] **Step 4: Wire the module**

Edit `apps/overlay/src-tauri/src/lib.rs` and add:

```rust
pub mod match_tracker;
```

- [ ] **Step 5: Verify**

Run: `cargo test -p rocketstats-overlay match_tracker -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/match_tracker.rs
git commit -m "feat: track match state from log events"
```

## Task 5: Add SQLite Storage

**Files:**
- Create: `apps/overlay/src-tauri/src/storage.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Write storage tests first**

Create `apps/overlay/src-tauri/src/storage.rs`:

```rust
use crate::domain::PlayerCard;
use crate::error::Result;

pub struct Storage {
    pool: sqlx::SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        Ok(())
    }

    pub async fn upsert_player_card(&self, _card: &PlayerCard) -> Result<()> {
        Ok(())
    }

    pub async fn get_player_card(&self, _player_id: &str) -> Result<Option<PlayerCard>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::Storage;
    use crate::domain::PlayerCard;

    #[tokio::test]
    async fn stores_and_reads_player_card() {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let card = PlayerCard {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            name: Some("Opponent".to_owned()),
            playlist: Some(11),
            mmr: Some(912.4),
            tier: Some(15),
            division: Some(2),
            data_age_seconds: 0,
        };

        storage.upsert_player_card(&card).await.unwrap();

        let stored = storage
            .get_player_card("Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0")
            .await
            .unwrap();
        assert_eq!(stored, Some(card));
    }
}
```

- [ ] **Step 2: Run storage tests and verify failure**

Run: `cargo test -p rocketstats-overlay storage -- --nocapture`

Expected: FAIL because `get_player_card` returns `None`.

- [ ] **Step 3: Implement schema and repository methods**

Replace `apps/overlay/src-tauri/src/storage.rs` with:

```rust
use crate::domain::PlayerCard;
use crate::error::Result;

pub struct Storage {
    pool: sqlx::SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_cards (
                player_id TEXT PRIMARY KEY NOT NULL,
                name TEXT,
                playlist INTEGER,
                mmr REAL,
                tier INTEGER,
                division INTEGER,
                data_age_seconds INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_player_card(&self, card: &PlayerCard) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO player_cards (
                player_id,
                name,
                playlist,
                mmr,
                tier,
                division,
                data_age_seconds
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(player_id) DO UPDATE SET
                name = excluded.name,
                playlist = excluded.playlist,
                mmr = excluded.mmr,
                tier = excluded.tier,
                division = excluded.division,
                data_age_seconds = excluded.data_age_seconds
            "#,
        )
        .bind(&card.player_id)
        .bind(&card.name)
        .bind(card.playlist)
        .bind(card.mmr)
        .bind(card.tier)
        .bind(card.division)
        .bind(card.data_age_seconds as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_player_card(&self, player_id: &str) -> Result<Option<PlayerCard>> {
        let row = sqlx::query_as::<_, PlayerCardRow>(
            r#"
            SELECT player_id, name, playlist, mmr, tier, division, data_age_seconds
            FROM player_cards
            WHERE player_id = ?1
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(PlayerCardRow::into_card))
    }
}

#[derive(sqlx::FromRow)]
struct PlayerCardRow {
    player_id: String,
    name: Option<String>,
    playlist: Option<i32>,
    mmr: Option<f64>,
    tier: Option<i32>,
    division: Option<i32>,
    data_age_seconds: i64,
}

impl PlayerCardRow {
    fn into_card(self) -> PlayerCard {
        PlayerCard {
            player_id: self.player_id,
            name: self.name,
            playlist: self.playlist,
            mmr: self.mmr,
            tier: self.tier,
            division: self.division,
            data_age_seconds: self.data_age_seconds.max(0) as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Storage;
    use crate::domain::PlayerCard;

    #[tokio::test]
    async fn stores_and_reads_player_card() {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let card = PlayerCard {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            name: Some("Opponent".to_owned()),
            playlist: Some(11),
            mmr: Some(912.4),
            tier: Some(15),
            division: Some(2),
            data_age_seconds: 0,
        };

        storage.upsert_player_card(&card).await.unwrap();

        let stored = storage
            .get_player_card("Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0")
            .await
            .unwrap();
        assert_eq!(stored, Some(card));
    }
}
```

- [ ] **Step 4: Wire module**

Edit `apps/overlay/src-tauri/src/lib.rs` and add:

```rust
pub mod storage;
```

- [ ] **Step 5: Verify**

Run: `cargo test -p rocketstats-overlay storage -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/storage.rs
git commit -m "feat: add overlay sqlite storage"
```

## Task 6: Add Player Enrichment With Mockable PsyNet Client

**Files:**
- Create: `apps/overlay/src-tauri/src/enrichment.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Write enrichment tests first**

Create `apps/overlay/src-tauri/src/enrichment.rs`:

```rust
use crate::domain::PlayerCard;
use crate::error::Result;
use rocketstats_rlapi::PlayerId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

pub trait SkillClient {
    fn enrich_players<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
        playlist: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>>;
}

pub struct PlayerEnrichment<C> {
    client: C,
    local_player_id: Option<String>,
    cache: HashMap<String, PlayerCard>,
}

impl<C> PlayerEnrichment<C>
where
    C: SkillClient,
{
    pub fn new(client: C, local_player_id: Option<String>) -> Self {
        Self {
            client,
            local_player_id,
            cache: HashMap::new(),
        }
    }

    pub async fn enrich_detected(
        &mut self,
        detected: Vec<String>,
        playlist: i32,
    ) -> Result<Vec<PlayerCard>> {
        let _ = (detected, playlist);
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerEnrichment, SkillClient};
    use crate::domain::PlayerCard;
    use crate::error::Result;
    use rocketstats_rlapi::PlayerId;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockSkillClient {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl SkillClient for MockSkillClient {
        fn enrich_players<'a>(
            &'a self,
            player_ids: Vec<PlayerId>,
            playlist: i32,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>> {
            Box::pin(async move {
                let ids = player_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>();
                self.calls.lock().unwrap().push(ids.clone());
                Ok(ids
                    .into_iter()
                    .map(|player_id| PlayerCard {
                        player_id,
                        name: Some("Detected".to_owned()),
                        playlist: Some(playlist),
                        mmr: Some(900.0),
                        tier: Some(14),
                        division: Some(1),
                        data_age_seconds: 0,
                    })
                    .collect())
            })
        }
    }

    #[tokio::test]
    async fn filters_local_player_deduplicates_and_caches() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let client = MockSkillClient {
            calls: Arc::clone(&calls),
        };
        let mut enrichment = PlayerEnrichment::new(
            client,
            Some("Epic|local000000000000000000000000000|0".to_owned()),
        );

        let first = enrichment
            .enrich_detected(
                vec![
                    "Epic|local000000000000000000000000000|0".to_owned(),
                    "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                    "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                ],
                11,
            )
            .await
            .unwrap();
        let second = enrichment
            .enrich_detected(
                vec!["Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned()],
                11,
            )
            .await
            .unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Run enrichment tests and verify failure**

Run: `cargo test -p rocketstats-overlay enrichment -- --nocapture`

Expected: FAIL because `enrich_detected` returns an empty vector.

- [ ] **Step 3: Implement enrichment filtering and cache**

Replace the `impl<C> PlayerEnrichment<C>` block in `apps/overlay/src-tauri/src/enrichment.rs` with:

```rust
impl<C> PlayerEnrichment<C>
where
    C: SkillClient,
{
    pub fn new(client: C, local_player_id: Option<String>) -> Self {
        Self {
            client,
            local_player_id,
            cache: HashMap::new(),
        }
    }

    pub async fn enrich_detected(
        &mut self,
        detected: Vec<String>,
        playlist: i32,
    ) -> Result<Vec<PlayerCard>> {
        let mut ordered = Vec::new();
        for player_id in detected {
            if self.local_player_id.as_deref() == Some(player_id.as_str()) {
                continue;
            }
            if !ordered.contains(&player_id) {
                ordered.push(player_id);
            }
        }

        let mut missing = Vec::new();
        for player_id in &ordered {
            if !self.cache.contains_key(player_id) {
                if let Ok(parsed) = PlayerId::from_str(player_id) {
                    missing.push(parsed);
                }
            }
        }

        if !missing.is_empty() {
            for card in self.client.enrich_players(missing, playlist).await? {
                self.cache.insert(card.player_id.clone(), card);
            }
        }

        Ok(ordered
            .into_iter()
            .filter_map(|player_id| self.cache.get(&player_id).cloned())
            .collect())
    }
}
```

- [ ] **Step 4: Add real PsyNet adapter**

Append this implementation to `apps/overlay/src-tauri/src/enrichment.rs`:

```rust
#[derive(Clone)]
pub struct PsyNetSkillClient {
    rpc: rocketstats_rlapi::PsyNetRpc,
}

impl PsyNetSkillClient {
    pub fn new(rpc: rocketstats_rlapi::PsyNetRpc) -> Self {
        Self { rpc }
    }
}

impl SkillClient for PsyNetSkillClient {
    fn enrich_players<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
        playlist: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>> {
        Box::pin(async move {
            let profiles = self.rpc.get_profiles(player_ids.clone()).await?;
            let skills = self.rpc.get_players_skills(player_ids).await?;

            let mut names = profiles
                .into_iter()
                .map(|profile| (profile.player_id, profile.player_name))
                .collect::<HashMap<_, _>>();

            let cards = skills
                .into_iter()
                .map(|player| {
                    let skill = player.skills.iter().find(|skill| skill.playlist == playlist);
                    PlayerCard {
                        player_id: player.player_id.to_string(),
                        name: names.remove(player.player_id.as_str()),
                        playlist: skill.map(|skill| skill.playlist),
                        mmr: skill.map(|skill| skill.mmr),
                        tier: skill.map(|skill| skill.tier),
                        division: skill.map(|skill| skill.division),
                        data_age_seconds: 0,
                    }
                })
                .collect();

            Ok(cards)
        })
    }
}
```

- [ ] **Step 5: Wire module**

Edit `apps/overlay/src-tauri/src/lib.rs` and add:

```rust
pub mod enrichment;
```

- [ ] **Step 6: Verify**

Run: `cargo test -p rocketstats-overlay enrichment -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/enrichment.rs
git commit -m "feat: enrich detected players through psynet"
```

## Task 7: Add Auth Service Skeleton

**Files:**
- Create: `apps/overlay/src-tauri/src/auth.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Create auth service with explicit states**

Create `apps/overlay/src-tauri/src/auth.rs`:

```rust
use crate::domain::AuthState;
use crate::error::Result;
use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetRpc};
use tokio::sync::watch;

pub struct AuthService {
    epic: EpicAuthClient,
    psynet: PsyNetClient,
    state_tx: watch::Sender<AuthState>,
    rpc: Option<PsyNetRpc>,
}

impl AuthService {
    pub fn new(epic: EpicAuthClient, psynet: PsyNetClient) -> Self {
        let (state_tx, _) = watch::channel(AuthState::Unauthenticated);
        Self {
            epic,
            psynet,
            state_tx,
            rpc: None,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<AuthState> {
        self.state_tx.subscribe()
    }

    pub fn state(&self) -> AuthState {
        self.state_tx.borrow().clone()
    }

    pub fn rpc(&self) -> Option<PsyNetRpc> {
        self.rpc.clone()
    }

    pub async fn start_device_login(&mut self) -> Result<AuthState> {
        let device = self.epic.start_device_authorization().await?;
        let waiting = AuthState::WaitingForDeviceCode {
            user_code: device.user_code.clone(),
            verification_uri: device.verification_uri.clone(),
            expires_in: device.expires_in,
        };
        let _ = self.state_tx.send(waiting.clone());

        let token = self.epic.wait_for_device_authorization(&device).await?;
        let rpc = self
            .psynet
            .auth_player_eos(
                token.access_token.as_str(),
                token.account_id.as_str(),
                token.selected_account_id.as_deref(),
            )
            .await?;

        let connected = AuthState::Connected {
            account_id: token.account_id,
            player_name: token.selected_account_id,
        };
        self.rpc = Some(rpc);
        let _ = self.state_tx.send(connected.clone());
        Ok(connected)
    }

    pub async fn logout(&mut self) {
        if let Some(rpc) = self.rpc.take() {
            let _ = rpc.close().await;
        }
        let _ = self.state_tx.send(AuthState::Unauthenticated);
    }
}
```

- [ ] **Step 2: Wire module**

Edit `apps/overlay/src-tauri/src/lib.rs` and add:

```rust
pub mod auth;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p rocketstats-overlay`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/auth.rs
git commit -m "feat: add epic psynet auth service"
```

## Task 8: Add Tauri Bridge State And Commands

**Files:**
- Create: `apps/overlay/src-tauri/src/bridge.rs`
- Modify: `apps/overlay/src-tauri/src/lib.rs`

- [ ] **Step 1: Create bridge state and commands**

Create `apps/overlay/src-tauri/src/bridge.rs`:

```rust
use crate::domain::{AuthState, MatchSession, OverlayState, PlayerCard};
use crate::error::Result;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct OverlayBackendState {
    pub auth: RwLock<AuthState>,
    pub match_session: RwLock<MatchSession>,
    pub players: RwLock<Vec<PlayerCard>>,
}

impl Default for OverlayBackendState {
    fn default() -> Self {
        Self {
            auth: RwLock::new(AuthState::Unauthenticated),
            match_session: RwLock::new(MatchSession::default()),
            players: RwLock::new(Vec::new()),
        }
    }
}

pub type SharedOverlayBackendState = Arc<OverlayBackendState>;

#[tauri::command]
pub async fn get_overlay_state(
    state: State<'_, SharedOverlayBackendState>,
) -> std::result::Result<OverlayState, String> {
    Ok(build_overlay_state(&state).await)
}

#[tauri::command]
pub async fn set_click_through(
    app: AppHandle,
    enabled: bool,
) -> std::result::Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_owned())?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|error| error.to_string())
}

pub async fn emit_overlay_state(
    app: &AppHandle,
    state: &SharedOverlayBackendState,
) -> Result<()> {
    let overlay = build_overlay_state(state).await;
    app.emit("overlay-state", &overlay)?;
    Ok(())
}

async fn build_overlay_state(state: &SharedOverlayBackendState) -> OverlayState {
    let auth = state.auth.read().await.clone();
    let match_session = state.match_session.read().await.clone();
    let players = state.players.read().await.clone();
    OverlayState {
        partial_roster: true,
        status_message: status_message(&auth, players.len()),
        auth,
        match_session,
        players,
    }
}

fn status_message(auth: &AuthState, player_count: usize) -> String {
    match auth {
        AuthState::Unauthenticated => "Epic/PsyNet auth required".to_owned(),
        AuthState::WaitingForDeviceCode { .. } => "Waiting for Epic device login".to_owned(),
        AuthState::Connected { .. } if player_count == 0 => "Waiting for detected players".to_owned(),
        AuthState::Connected { .. } => format!("Detected players: {player_count}"),
        AuthState::Expired => "Epic/PsyNet auth expired".to_owned(),
        AuthState::Error { message } => format!("Auth error: {message}"),
    }
}

#[derive(Debug, Serialize)]
pub struct BridgeReady {
    pub ready: bool,
}
```

- [ ] **Step 2: Wire Tauri state and commands**

Replace `apps/overlay/src-tauri/src/lib.rs` with:

```rust
pub mod auth;
pub mod bridge;
pub mod domain;
pub mod enrichment;
pub mod error;
pub mod logs;
pub mod match_tracker;
pub mod storage;

use bridge::{SharedOverlayBackendState, get_overlay_state, set_click_through};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt};

pub fn run() {
    init_tracing();
    let state: SharedOverlayBackendState = Arc::new(bridge::OverlayBackendState::default());
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_overlay_state,
            set_click_through
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_ignore_cursor_events(true)?;
            }
            app.emit("bridge-ready", bridge::BridgeReady { ready: true })?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run RocketStats overlay");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rocketstats_overlay=info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p rocketstats-overlay`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/overlay/src-tauri/src/lib.rs apps/overlay/src-tauri/src/bridge.rs
git commit -m "feat: expose overlay state through tauri bridge"
```

## Task 9: Implement Minimal Frontend State Rendering

**Files:**
- Create: `apps/overlay/src/state.ts`
- Modify: `apps/overlay/src/main.ts`
- Modify: `apps/overlay/src/styles.css`

- [ ] **Step 1: Add frontend state types**

Create `apps/overlay/src/state.ts`:

```ts
export type AuthState =
  | "Unauthenticated"
  | { WaitingForDeviceCode: { user_code: string; verification_uri: string; expires_in: number } }
  | { Connected: { account_id: string; player_name: string | null } }
  | "Expired"
  | { Error: { message: string } };

export interface PlayerCard {
  player_id: string;
  name: string | null;
  playlist: number | null;
  mmr: number | null;
  tier: number | null;
  division: number | null;
  data_age_seconds: number;
}

export interface OverlayState {
  auth: AuthState;
  players: PlayerCard[];
  partial_roster: boolean;
  status_message: string;
}

export function authLabel(auth: AuthState): string {
  if (auth === "Unauthenticated") return "Auth required";
  if (auth === "Expired") return "Auth expired";
  if (typeof auth === "object" && "WaitingForDeviceCode" in auth) return "Waiting for login";
  if (typeof auth === "object" && "Connected" in auth) return "Connected";
  if (typeof auth === "object" && "Error" in auth) return `Error: ${auth.Error.message}`;
  return "Unknown";
}
```

- [ ] **Step 2: Render backend state**

Replace `apps/overlay/src/main.ts` with:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { OverlayState, authLabel } from "./state";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("missing #app root");
}

function rankLine(player: OverlayState["players"][number]): string {
  if (player.mmr === null) return "Rank unavailable";
  const tier = player.tier === null ? "?" : String(player.tier);
  const division = player.division === null ? "?" : String(player.division);
  return `MMR ${player.mmr.toFixed(1)} | Tier ${tier} Div ${division}`;
}

function render(state: OverlayState): void {
  const players = state.players
    .map(
      (player) => `
        <article class="player-card">
          <div>
            <strong>${player.name ?? "Detected player"}</strong>
            <span>${player.player_id}</span>
          </div>
          <p>${rankLine(player)}</p>
        </article>
      `,
    )
    .join("");

  app.innerHTML = `
    <main class="overlay-shell">
      <section class="panel">
        <p class="eyebrow">RocketStats</p>
        <h1>${state.status_message}</h1>
        <p class="muted">${authLabel(state.auth)}</p>
        <p class="warning">${state.partial_roster ? "Detected players only. Full lobby is not guaranteed." : ""}</p>
        <div class="players">${players}</div>
      </section>
    </main>
  `;
}

async function boot(): Promise<void> {
  render(await invoke<OverlayState>("get_overlay_state"));
  await listen<OverlayState>("overlay-state", (event) => render(event.payload));
}

void boot();
```

- [ ] **Step 3: Extend frontend styling**

Append to `apps/overlay/src/styles.css`:

```css
.warning {
  min-height: 20px;
  margin: 10px 0 0;
  color: #ffcf75;
  font-size: 13px;
}

.players {
  display: grid;
  gap: 10px;
  margin-top: 14px;
}

.player-card {
  padding: 12px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.player-card div {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.player-card strong {
  font-size: 16px;
}

.player-card span {
  font-size: 11px;
  color: rgba(245, 241, 232, 0.58);
  word-break: break-all;
}

.player-card p {
  margin: 8px 0 0;
  color: rgba(245, 241, 232, 0.82);
}
```

- [ ] **Step 4: Verify frontend build**

Run: `npm install`

Expected: `node_modules` and lockfile are created in `apps/overlay`.

Run: `npm run build`

Expected: TypeScript and Vite build pass.

- [ ] **Step 5: Commit**

```bash
git add apps/overlay/package.json apps/overlay/package-lock.json apps/overlay/src
git commit -m "feat: render overlay state in tauri ui"
```

## Task 10: Add Log Replay Integration Test

**Files:**
- Create: `apps/overlay/src-tauri/tests/fixtures/launch_excerpt.log`
- Create: `apps/overlay/src-tauri/tests/log_replay.rs`

- [ ] **Step 1: Create anonymized fixture**

Create `apps/overlay/src-tauri/tests/fixtures/launch_excerpt.log`:

```text
[0223.91] Matchmaking: StartMatchmaking at 2026-05-29 23:37:55 in EU9,EU7,EU5,EU3,EU1 for playlists 11 on game server
[0238.79] DevNet: Welcomed by server (Level: FF_Dusk_P, Game: TAGame.GameInfo_Soccar_TA, GameTags: )
[0240.99] ScriptLog: MatchGUID: 706DA47C11F15BB7CB1952B6DEE4DFF5
[0241.08] ScriptLog: Uncached PlatformId for Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0
[0241.18] ScriptLog: Uncached PlatformId for Epic|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|0
[0619.02] XPProgression: SaveData_TA.HandleRewardDropNotification PsyNetService_RewardDropReceived_TA returned Total XP Earned = 5160.0000, in match with ID = 706DA47C11F15BB7CB1952B6DEE4DFF5 , Current player match score = 323, UniqueId=(Epic|local000000000000000000000000000|0), with total match time = 302.9711 seconds
```

- [ ] **Step 2: Write replay test**

Create `apps/overlay/src-tauri/tests/log_replay.rs`:

```rust
use rocketstats_overlay::domain::{LogEvent, MatchPhase};
use rocketstats_overlay::logs::parser::parse_log_line;
use rocketstats_overlay::match_tracker::MatchTracker;

#[test]
fn replay_fixture_builds_match_session() {
    let fixture = include_str!("fixtures/launch_excerpt.log");
    let mut tracker = MatchTracker::default();
    let mut parsed = 0;

    for line in fixture.lines() {
        if let Some(event) = parse_log_line(line) {
            parsed += 1;
            tracker.apply(event);
        }
    }

    let session = tracker.session();

    assert_eq!(parsed, 6);
    assert_eq!(session.phase, MatchPhase::Ended);
    assert_eq!(session.playlist, Some(11));
    assert_eq!(session.map.as_deref(), Some("FF_Dusk_P"));
    assert_eq!(
        session.guid.as_deref(),
        Some("706DA47C11F15BB7CB1952B6DEE4DFF5")
    );
    assert_eq!(session.detected_players.len(), 2);
    assert_eq!(session.local_score, Some(323));
    assert_eq!(session.xp, Some(5160));

    assert!(matches!(
        parse_log_line("[0001.00] unrelated"),
        None
    ));
    assert!(matches!(
        parse_log_line("[0240.99] ScriptLog: MatchGUID: ABCD"),
        Some(LogEvent::MatchGuidSeen { .. })
    ));
}
```

- [ ] **Step 3: Verify replay test**

Run: `cargo test -p rocketstats-overlay --test log_replay -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/overlay/src-tauri/tests
git commit -m "test: replay anonymized rocket league log fixture"
```

## Task 11: Add Watcher Tail Skeleton With Parser Hook

**Files:**
- Modify: `apps/overlay/src-tauri/src/logs/watcher.rs`

- [ ] **Step 1: Replace watcher with an async polling tail**

Replace `apps/overlay/src-tauri/src/logs/watcher.rs` with:

```rust
use crate::domain::LogEvent;
use crate::error::Result;
use crate::logs::parser::parse_log_line;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone)]
pub struct LogWatcherConfig {
    pub path: PathBuf,
    pub poll_interval: Duration,
}

impl LogWatcherConfig {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            poll_interval: Duration::from_millis(250),
        }
    }
}

pub async fn watch_log(config: LogWatcherConfig, events: mpsc::Sender<LogEvent>) -> Result<()> {
    let mut offset = 0;

    loop {
        match File::open(&config.path).await {
            Ok(mut file) => {
                let metadata = file.metadata().await?;
                if metadata.len() < offset {
                    offset = 0;
                }

                file.seek(SeekFrom::Start(offset)).await?;
                let mut reader = BufReader::new(file);
                let mut line = String::new();

                loop {
                    line.clear();
                    let bytes = reader.read_line(&mut line).await?;
                    if bytes == 0 {
                        break;
                    }
                    offset += bytes as u64;
                    if let Some(event) = parse_log_line(line.trim_end()) {
                        if events.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        sleep(config.poll_interval).await;
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p rocketstats-overlay`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add apps/overlay/src-tauri/src/logs/watcher.rs
git commit -m "feat: tail rocket league launch log"
```

## Task 12: Full Verification And Manual Overlay Spike

**Files:**
- Modify only files required by failed verification.

- [ ] **Step 1: Run Rust formatting**

Run: `cargo fmt`

Expected: Rust files are formatted.

- [ ] **Step 2: Run Rust test suite**

Run: `cargo test --workspace`

Expected: all Rust unit and integration tests pass.

- [ ] **Step 3: Run Rust clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: no warnings.

- [ ] **Step 4: Run frontend build**

Run from `apps/overlay`: `npm run build`

Expected: TypeScript and Vite build pass.

- [ ] **Step 5: Run Tauri dev overlay spike**

Run from `apps/overlay`: `npm run tauri:dev`

Expected: a transparent borderless RocketStats window opens, appears above normal desktop windows, and starts click-through.

- [ ] **Step 6: Manually test Rocket League display mode**

Set Rocket League to borderless/windowed fullscreen. Start `npm run tauri:dev` from `apps/overlay`.

Expected:

- Overlay is visible above Rocket League.
- Overlay does not appear in exclusive fullscreen if Rocket League uses that mode.
- Click-through allows game input.
- Calling the `set_click_through(false)` command from the dev console or a temporary debug button makes the overlay interactive again.

- [ ] **Step 7: Commit final verification fixes**

If verification changed files, commit them:

```bash
git add Cargo.toml Cargo.lock apps/overlay
git commit -m "chore: verify tauri overlay mvp"
```

If verification changed no files, do not create an empty commit.

## Self-Review

- Spec coverage: The plan covers Tauri shell, Rust backend modules, Epic/PsyNet auth prerequisite, log parsing, match tracking, player enrichment, SQLite cache, Tauri bridge, overlay rendering, security redaction, anonymized fixture replay, and manual overlay validation.
- Scope: The plan excludes BakkesMod, injected overlay behavior, memory scanning, packet capture, cloud sync, and guaranteed full lobby detection, matching the design.
- Completeness scan: The plan contains concrete file paths, commands, tests, data shapes, and commit messages. It avoids unspecified implementation steps.
- Type consistency: `AuthState`, `LogEvent`, `MatchSession`, `PlayerCard`, and `OverlayState` are defined before use and referenced consistently across backend and frontend tasks.
