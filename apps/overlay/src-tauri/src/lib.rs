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
use tauri::{Emitter, Manager};
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
