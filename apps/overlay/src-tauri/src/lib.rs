pub mod auth;
pub mod bridge;
pub mod domain;
pub mod enrichment;
pub mod error;
pub mod local_player;
pub mod logs;
pub mod match_tracker;
pub mod settings;
pub mod storage;

use auth::AuthService;
use bridge::{
    SharedAuthService, SharedOverlayBackendState, emit_overlay_state, get_overlay_state,
    get_settings, logout, open_settings_window, save_settings, set_click_through, start_login,
    toggle_click_through,
};
use domain::{AuthState, InitEvent, LocalPlayerSummary};
use enrichment::{PlayerEnrichment, PsyNetSkillClient};
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use gtk::prelude::WidgetExt;
use local_player::{LocalPlayerSummaryLoader, PsyNetLocalPlayerClient};
use logs::parser::parse_init_line;
use logs::watcher::{LogWatcherConfig, watch_log};
use match_tracker::MatchTracker;
use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetConfig};
use settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use storage::Storage;
use tauri::{Emitter, Manager};
use tokio::sync::{RwLock, mpsc};
use tracing_subscriber::{EnvFilter, fmt};

pub fn run() {
    init_tracing();

    let overlay_state: SharedOverlayBackendState = Arc::new(bridge::OverlayBackendState::default());
    let initial_settings = Settings::load();
    let settings: Arc<RwLock<Settings>> = Arc::new(RwLock::new(initial_settings.clone()));

    // Detect game version from Launch.log (non-blocking best-effort)
    let psynet_config = detect_psynet_config();

    let epic = EpicAuthClient::new();
    let psynet = PsyNetClient::new(psynet_config);
    let auth_service: SharedAuthService =
        Arc::new(tokio::sync::Mutex::new(AuthService::new(epic, psynet)));

    tauri::Builder::default()
        .manage(overlay_state.clone())
        .manage(auth_service.clone())
        .manage(settings.clone())
        .invoke_handler(tauri::generate_handler![
            get_overlay_state,
            get_settings,
            save_settings,
            set_click_through,
            toggle_click_through,
            open_settings_window,
            start_login,
            logout
        ])
        .setup(move |app| {
            // Apply persisted window settings on startup
            if let Some(window) = app.get_webview_window("main") {
                apply_main_window_settings(&window, &initial_settings)?;
            }
            app.emit("bridge-ready", bridge::BridgeReady { ready: true })?;

            // --- Log watcher → mpsc channel ---
            let (log_tx, mut log_rx) = mpsc::channel::<domain::LogEvent>(64);
            let log_path = initial_settings.resolved_log_path();
            tauri::async_runtime::spawn(async move {
                tracing::info!(path = %log_path.display(), "starting log watcher");
                let config = LogWatcherConfig::new(log_path);
                if let Err(e) = watch_log(config, log_tx).await {
                    tracing::warn!(error = %e, "log watcher exited with error");
                }
            });

            let app_handle = app.handle().clone();
            let overlay = overlay_state.clone();
            let auth = auth_service.clone();

            tauri::async_runtime::spawn(async move {
                let storage_path = match app_handle.path().app_data_dir() {
                    Ok(dir) => storage_database_path(&dir),
                    Err(error) => {
                        tracing::error!("failed to resolve app data directory: {error}");
                        return;
                    }
                };
                tracing::info!("using storage database at {}", storage_path.display());

                let storage = match Storage::connect_file(&storage_path).await {
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

                app_handle.manage(storage.clone());

                let mut rx = {
                    let svc = auth.lock().await;
                    svc.subscribe()
                };
                let mut diagnostics_rx = {
                    let svc = auth.lock().await;
                    svc.subscribe_diagnostics()
                };

                {
                    let auth_state = rx.borrow().clone();
                    {
                        let mut stored = overlay.auth.write().await;
                        *stored = auth_state.clone();
                    }
                    sync_local_player_summary(&overlay, &auth, &auth_state).await;
                }
                let diagnostics = diagnostics_rx.borrow().clone();
                overlay.replace_auth_diagnostics(diagnostics).await;
                if let Err(error) = emit_overlay_state(&app_handle, &overlay).await {
                    tracing::warn!("failed to emit initial overlay state: {error}");
                }

                let sync_overlay = overlay.clone();
                let sync_handle = app_handle.clone();
                let sync_auth = auth.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        if rx.changed().await.is_err() {
                            break;
                        }
                        let auth_state = rx.borrow().clone();
                        {
                            let mut stored = sync_overlay.auth.write().await;
                            *stored = auth_state.clone();
                        }
                        sync_local_player_summary(&sync_overlay, &sync_auth, &auth_state).await;
                        if let Err(error) = emit_overlay_state(&sync_handle, &sync_overlay).await {
                            tracing::warn!("failed to emit auth overlay state: {error}");
                        }
                    }
                });

                let diagnostics_overlay = overlay.clone();
                let diagnostics_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        if diagnostics_rx.changed().await.is_err() {
                            break;
                        }
                        let diagnostics = diagnostics_rx.borrow().clone();
                        diagnostics_overlay
                            .replace_auth_diagnostics(diagnostics)
                            .await;
                        if let Err(error) =
                            emit_overlay_state(&diagnostics_handle, &diagnostics_overlay).await
                        {
                            tracing::warn!("failed to emit diagnostics overlay state: {error}");
                        }
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

                // --- Orchestration pipeline ---
                // Receives log events from the watcher, feeds MatchTracker,
                // enriches detected players when auth is available, and
                // pushes updates to the overlay state.
                let mut tracker = MatchTracker::default();
                let mut enrichment_client: Option<PlayerEnrichment<PsyNetSkillClient>> = None;

                while let Some(event) = log_rx.recv().await {
                    let session = tracker.apply(event);

                    let players = if let Some(client) = enrichment_client.as_mut() {
                        let detected: Vec<String> = session
                            .detected_players
                            .iter()
                            .map(|d| d.value.clone())
                            .collect();
                        let playlist = session.playlist.unwrap_or(11);
                        match client.enrich_detected(detected, playlist).await {
                            Ok(cards) => {
                                for card in &cards {
                                    if let Err(e) = storage.upsert_player_card(card).await {
                                        tracing::warn!(error = %e, "failed to upsert player card");
                                    }
                                }
                                cards
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "player enrichment failed");
                                Vec::new()
                            }
                        }
                    } else {
                        let svc = auth.lock().await;
                        if let Some(rpc) = svc.rpc() {
                            enrichment_client =
                                Some(PlayerEnrichment::new(PsyNetSkillClient::new(rpc), None));
                            tracing::info!("auth rpc available, enrichment client created");
                        } else {
                            tracing::debug!("no auth rpc available yet, skipping enrichment");
                        }
                        Vec::new()
                    };

                    {
                        let mut ms = overlay.match_session.write().await;
                        *ms = session;
                    }
                    {
                        let mut p = overlay.players.write().await;
                        *p = players;
                    }

                    if let Err(error) = emit_overlay_state(&app_handle, &overlay).await {
                        tracing::warn!("failed to emit orchestration overlay state: {error}");
                    }
                }

                tracing::info!("log event channel closed, orchestration pipeline stopped");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run RocketStats overlay");
}

// --- Startup helpers ---

fn apply_main_window_settings<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    settings: &Settings,
) -> tauri::Result<()> {
    apply_window_opacity(window, settings.opacity)?;
    window.set_always_on_top(settings.always_on_top)?;
    window.set_ignore_cursor_events(settings.click_through)?;
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn apply_window_opacity<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    opacity: f64,
) -> tauri::Result<()> {
    window.gtk_window()?.set_opacity(opacity);
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn apply_window_opacity<R: tauri::Runtime>(
    _window: &tauri::WebviewWindow<R>,
    _opacity: f64,
) -> tauri::Result<()> {
    Ok(())
}

// --- Storage ---

fn storage_database_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("overlay.sqlite3")
}

// --- Local player summary ---

async fn sync_local_player_summary(
    overlay: &SharedOverlayBackendState,
    auth: &SharedAuthService,
    auth_state: &AuthState,
) {
    let summary = match auth_state {
        AuthState::Connected {
            account_id,
            player_name,
            ..
        } => {
            let rpc = {
                let svc = auth.lock().await;
                svc.rpc()
            };
            Some(load_local_player_summary(rpc, account_id, player_name.as_deref()).await)
        }
        _ => None,
    };

    let mut local_player = overlay.local_player.write().await;
    *local_player = summary;
}

async fn load_local_player_summary(
    rpc: Option<rocketstats_rlapi::PsyNetRpc>,
    account_id: &str,
    fallback_name: Option<&str>,
) -> LocalPlayerSummary {
    let fallback = fallback_local_player_summary(account_id, fallback_name);
    let Some(rpc) = rpc else {
        return fallback;
    };

    let loader = LocalPlayerSummaryLoader::new(PsyNetLocalPlayerClient::new(rpc));
    match loader.load(account_id, fallback_name).await {
        Ok(summary) => summary,
        Err(error) => {
            tracing::warn!("failed to load local player summary: {error}");
            fallback
        }
    }
}

fn fallback_local_player_summary(
    account_id: &str,
    fallback_name: Option<&str>,
) -> LocalPlayerSummary {
    LocalPlayerSummary {
        display_name: fallback_name.unwrap_or(account_id).to_owned(),
        ranked_2v2_mmr: None,
        ranked_2v2_tier: None,
        ranked_2v2_division: None,
    }
}

// --- PsyNet config detection ---

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
    let dev_path = PathBuf::from(".tmp/Launch.log");
    if dev_path.exists() {
        return Some(dev_path);
    }

    // 3. Launch.log at CWD
    let cwd_log = PathBuf::from("Launch.log");
    if cwd_log.exists() {
        return Some(cwd_log);
    }

    // 4. Relative to crate manifest dir (compile-time)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent());
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

// --- Tests ---

#[cfg(test)]
mod capability_tests {
    use super::storage_database_path;
    use std::path::Path;

    #[test]
    fn storage_database_path_stays_under_app_data_dir() {
        let path = storage_database_path(Path::new("/tmp/rocketstats-data"));
        assert_eq!(path, Path::new("/tmp/rocketstats-data/overlay.sqlite3"));
    }

    #[test]
    fn main_window_capability_allows_event_listen() {
        let capability = std::fs::read_to_string("capabilities/main.json")
            .expect("expected Tauri capability file for main window");
        let json: serde_json::Value =
            serde_json::from_str(&capability).expect("expected valid capability JSON");

        let windows = json["windows"]
            .as_array()
            .expect("expected windows array in capability");
        assert!(
            windows.iter().any(|window| window.as_str() == Some("main")),
            "capability must target the main window"
        );

        let permissions = json["permissions"]
            .as_array()
            .expect("expected permissions array in capability");
        assert!(
            permissions.iter().any(|permission| {
                matches!(
                    permission.as_str(),
                    Some("core:event:default") | Some("core:default")
                )
            }),
            "capability must allow core:event:default directly or through core:default so the frontend can listen for overlay-state"
        );
    }
}
