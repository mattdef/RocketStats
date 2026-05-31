use crate::auth::AuthService;
use crate::domain::{AuthState, MatchSession, OverlayState, PlayerCard};
use crate::error::Result;
use crate::storage::Storage;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, RwLock};

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
pub async fn set_click_through(app: AppHandle, enabled: bool) -> std::result::Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_owned())?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|error| error.to_string())
}

pub async fn emit_overlay_state(app: &AppHandle, state: &SharedOverlayBackendState) -> Result<()> {
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
        AuthState::Connected { .. } if player_count == 0 => {
            "Waiting for detected players".to_owned()
        }
        AuthState::Connected { .. } => format!("Detected players: {player_count}"),
        AuthState::Expired => "Epic/PsyNet auth expired".to_owned(),
        AuthState::Error { message } => format!("Auth error: {message}"),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BridgeReady {
    pub ready: bool,
}

/// Shared AuthService managed by Tauri.
pub type SharedAuthService = Arc<Mutex<AuthService>>;

/// Starts the Epic device-code login flow.
///
/// Spawns a background task so the command returns immediately.
/// State updates are emitted via the `overlay-state` event.
#[tauri::command]
pub async fn start_login(
    auth: State<'_, SharedAuthService>,
    storage: State<'_, Arc<Storage>>,
) -> std::result::Result<(), String> {
    let svc = auth.inner().clone();
    let store = storage.inner().clone();
    tokio::spawn(async move {
        let mut svc = svc.lock().await;
        if let Err(e) = svc.start_device_login(&store).await {
            tracing::error!("device login failed: {e}");
        }
    });
    Ok(())
}

/// Logs out: closes PsyNet connection, clears stored tokens.
#[tauri::command]
pub async fn logout(
    auth: State<'_, SharedAuthService>,
    storage: State<'_, Arc<Storage>>,
) -> std::result::Result<(), String> {
    let mut svc = auth.lock().await;
    svc.logout(&storage).await;
    Ok(())
}
