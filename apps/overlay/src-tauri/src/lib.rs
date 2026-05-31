pub mod auth;
pub mod bridge;
pub mod domain;
pub mod enrichment;
pub mod error;
pub mod logs;
pub mod match_tracker;
pub mod storage;

use auth::AuthService;
use bridge::{
    SharedAuthService, SharedOverlayBackendState, get_overlay_state, logout, set_click_through,
    start_login,
};
use domain::InitEvent;
use logs::parser::parse_init_line;
use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetConfig};
use std::path::PathBuf;
use std::sync::Arc;
use storage::Storage;
use tauri::{Emitter, Manager};
use tracing_subscriber::{EnvFilter, fmt};

pub fn run() {
    init_tracing();

    let overlay_state: SharedOverlayBackendState = Arc::new(bridge::OverlayBackendState::default());

    // Detect game version from Launch.log (non-blocking best-effort)
    let psynet_config = detect_psynet_config();

    let epic = EpicAuthClient::new();
    let psynet = PsyNetClient::new(psynet_config);
    let auth_service: SharedAuthService =
        Arc::new(tokio::sync::Mutex::new(AuthService::new(epic, psynet)));

    tauri::Builder::default()
        .manage(overlay_state.clone())
        .manage(auth_service.clone())
        .invoke_handler(tauri::generate_handler![
            get_overlay_state,
            set_click_through,
            start_login,
            logout
        ])
        .setup(move |app| {
            // Disable cursor events on the overlay window
            if let Some(window) = app.get_webview_window("main") {
                window.set_ignore_cursor_events(true)?;
            }
            app.emit("bridge-ready", bridge::BridgeReady { ready: true })?;

            // Initialize storage asynchronously
            let app_handle = app.handle().clone();
            let overlay = overlay_state.clone();
            let auth = auth_service.clone();

            tokio::spawn(async move {
                // Connect to SQLite (in-memory for now; swap to file path for persistence)
                let storage = match Storage::connect("sqlite::memory:").await {
                    Ok(s) => {
                        if let Err(e) = s.migrate().await {
                            tracing::error!("storage migration failed: {e}");
                            return;
                        }
                        Arc::new(s)
                    }
                    Err(e) => {
                        tracing::error!("storage connect failed: {e}");
                        return;
                    }
                };

                // Manage storage so bridge commands can access it
                app_handle.manage(storage.clone());

                // Spawn state sync task: AuthService changes → OverlayBackendState → emit
                let sync_overlay = overlay.clone();
                let sync_auth = auth.clone();
                let sync_handle = app_handle.clone();
                tokio::spawn(async move {
                    let mut rx = {
                        let svc = sync_auth.lock().await;
                        svc.subscribe()
                    };
                    loop {
                        if rx.changed().await.is_err() {
                            break;
                        }
                        let auth_state = rx.borrow().clone();
                        {
                            let mut stored = sync_overlay.auth.write().await;
                            *stored = auth_state;
                        }
                        let _ = bridge::emit_overlay_state(&sync_handle, &sync_overlay).await;
                    }
                });

                // Auto-auth: try silent refresh with stored tokens
                {
                    let mut svc = auth.lock().await;
                    match svc.try_refresh(&storage).await {
                        Ok(true) => tracing::info!("auto-auth succeeded via refresh token"),
                        Ok(false) => {
                            tracing::info!("no stored refresh token, manual login required")
                        }
                        Err(e) => tracing::warn!("refresh attempt failed: {e}"),
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run RocketStats overlay");
}

/// Attempts to detect the game version from the Rocket League Launch.log.
///
/// Checks, in order:
/// 1. `ROCKETSTATS_LOG_PATH` environment variable
/// 2. Common Proton/Wine path on Linux
/// 3. Falls back to `PsyNetConfig::default()` if nothing is found
fn detect_psynet_config() -> PsyNetConfig {
    let log_path = find_launch_log();
    let Some(path) = log_path else {
        tracing::info!("no Launch.log found, using default PsyNetConfig");
        return PsyNetConfig::default();
    };

    tracing::info!("reading Launch.log from {}", path.display());

    // Read the first ~32KB — init section is at the very top.
    // Launch.log may use ISO-8859 (Latin-1) or UTF-8; lossy decode handles both.
    let Ok(bytes) = std::fs::read(&path) else {
        tracing::warn!("failed to read Launch.log at {}", path.display());
        return PsyNetConfig::default();
    };
    let content = String::from_utf8_lossy(&bytes);

    let mut game_version: Option<String> = None;
    let mut feature_set: Option<String> = None;

    for line in content.lines().take(500) {
        match parse_init_line(line) {
            Some(InitEvent::BuildVersion(v)) => {
                tracing::info!("detected game version: {v}");
                game_version = Some(v);
            }
            Some(InitEvent::FeatureSet(f)) => {
                tracing::info!("detected feature set: {f}");
                feature_set = Some(f);
            }
            Some(InitEvent::EpicIdentity {
                epic_user_id,
                epic_user_name,
            }) => {
                tracing::info!(
                    "detected epic identity: id={epic_user_id} name={:?}",
                    epic_user_name
                );
            }
            _ => {}
        }
    }

    let mut config = PsyNetConfig::default();
    if let Some(v) = game_version {
        config = config.with_game_version(v);
    }
    if let Some(f) = feature_set {
        config = config.with_feature_set(f);
    }
    config
}

fn find_launch_log() -> Option<PathBuf> {
    // 1. Environment variable override
    if let Ok(path) = std::env::var("ROCKETSTATS_LOG_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Dev convenience: .tmp/Launch.log at project root
    //    Works when running `cargo run` from the workspace root.
    let dev_path = PathBuf::from(".tmp/Launch.log");
    if dev_path.exists() {
        return Some(dev_path);
    }

    // 3. Launch.log at CWD (where the app is started from)
    let cwd_log = PathBuf::from("Launch.log");
    if cwd_log.exists() {
        return Some(cwd_log);
    }

    // 4. Relative to crate manifest dir (compile-time)
    //    From apps/overlay/src-tauri/ → project root/.tmp/Launch.log
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent() // src-tauri/
        .and_then(|p| p.parent()) // overlay/
        .and_then(|p| p.parent()); // apps/ → project root
    if let Some(root) = project_root {
        let manifest_relative = root.join(".tmp/Launch.log");
        if manifest_relative.exists() {
            return Some(manifest_relative);
        }
    }

    // 5. Common Proton/Wine path on Linux
    if let Some(home) = dirs_next() {
        let proton_path = home
            .join(".steam/steam/steamapps/compatdata/252950/pfx/drive_c")
            .join("users/steamuser/Documents/My Games/Rocket League/TAGame/Logs/Launch.log");
        if proton_path.exists() {
            return Some(proton_path);
        }
    }

    None
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rocketstats_overlay=info"));
    let _ = fmt().with_env_filter(filter).try_init();
}
