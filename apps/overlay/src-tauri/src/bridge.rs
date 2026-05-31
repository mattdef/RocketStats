use crate::auth::AuthService;
use crate::domain::{AuthState, MAX_AUTH_DIAGNOSTICS, MatchSession, OverlayState, PlayerCard};
use crate::error::Result;
use crate::storage::Storage;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, RwLock};

const STORAGE_NOT_READY_MESSAGE: &str =
    "Authentication services are still initializing. Retry in a moment.";

#[derive(Debug)]
pub struct OverlayBackendState {
    pub auth: RwLock<AuthState>,
    pub auth_diagnostics: RwLock<Vec<String>>,
    pub match_session: RwLock<MatchSession>,
    pub players: RwLock<Vec<PlayerCard>>,
}

impl Default for OverlayBackendState {
    fn default() -> Self {
        Self {
            auth: RwLock::new(AuthState::Unauthenticated),
            auth_diagnostics: RwLock::new(Vec::new()),
            match_session: RwLock::new(MatchSession::default()),
            players: RwLock::new(Vec::new()),
        }
    }
}

pub type SharedOverlayBackendState = Arc<OverlayBackendState>;

impl OverlayBackendState {
    pub async fn replace_auth_diagnostics(&self, mut diagnostics: Vec<String>) {
        if diagnostics.len() > MAX_AUTH_DIAGNOSTICS {
            let drop_count = diagnostics.len() - MAX_AUTH_DIAGNOSTICS;
            diagnostics.drain(..drop_count);
        }

        let mut stored = self.auth_diagnostics.write().await;
        *stored = diagnostics;
    }

    pub async fn push_auth_diagnostic(&self, message: impl Into<String>) {
        let mut diagnostics = self.auth_diagnostics.write().await;
        diagnostics.push(message.into());
        if diagnostics.len() > MAX_AUTH_DIAGNOSTICS {
            let drop_count = diagnostics.len() - MAX_AUTH_DIAGNOSTICS;
            diagnostics.drain(..drop_count);
        }
    }
}

#[tauri::command]
pub async fn get_overlay_state(
    state: State<'_, SharedOverlayBackendState>,
    auth: State<'_, SharedAuthService>,
) -> std::result::Result<OverlayState, String> {
    Ok(build_overlay_state_with_live_auth(&state, auth.inner()).await)
}

#[tauri::command]
pub async fn set_click_through<R: tauri::Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> std::result::Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_owned())?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|error| error.to_string())
}

pub async fn emit_overlay_state<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &SharedOverlayBackendState,
) -> Result<()> {
    let overlay = build_overlay_state(state).await;
    app.emit("overlay-state", &overlay)?;
    Ok(())
}

async fn build_overlay_state(state: &SharedOverlayBackendState) -> OverlayState {
    let auth = state.auth.read().await.clone();
    let auth_diagnostics = state.auth_diagnostics.read().await.clone();
    build_overlay_state_from_parts(state, auth, auth_diagnostics).await
}

async fn build_overlay_state_with_live_auth(
    state: &SharedOverlayBackendState,
    auth: &SharedAuthService,
) -> OverlayState {
    let (auth_state, auth_diagnostics) = {
        let svc = auth.lock().await;
        (svc.state(), svc.diagnostics())
    };
    build_overlay_state_from_parts(state, auth_state, auth_diagnostics).await
}

async fn build_overlay_state_from_parts(
    state: &SharedOverlayBackendState,
    auth: AuthState,
    auth_diagnostics: Vec<String>,
) -> OverlayState {
    let match_session = state.match_session.read().await.clone();
    let players = state.players.read().await.clone();
    OverlayState {
        partial_roster: true,
        status_message: status_message(&auth, players.len()),
        auth,
        auth_diagnostics,
        match_session,
        players,
    }
}

fn status_message(auth: &AuthState, player_count: usize) -> String {
    match auth {
        AuthState::Unauthenticated => "Epic/PsyNet auth required".to_owned(),
        AuthState::StartingDeviceLogin => "Starting Epic device login".to_owned(),
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
pub async fn start_login<R: tauri::Runtime>(
    app: AppHandle<R>,
    auth: State<'_, SharedAuthService>,
    overlay: State<'_, SharedOverlayBackendState>,
) -> std::result::Result<(), String> {
    let overlay = overlay.inner().clone();
    let store = match app.try_state::<Arc<Storage>>() {
        Some(storage) => storage.inner().clone(),
        None => {
            tracing::warn!("start_login requested before storage was ready");
            overlay
                .push_auth_diagnostic(STORAGE_NOT_READY_MESSAGE)
                .await;
            if let Err(error) = emit_overlay_state(&app, &overlay).await {
                tracing::warn!("failed to emit overlay state: {error}");
            }
            return Err(STORAGE_NOT_READY_MESSAGE.to_owned());
        }
    };
    tracing::info!("received start_login command");
    let svc = auth.inner().clone();
    {
        let mut svc = svc.lock().await;
        svc.mark_device_login_started();
        {
            let mut stored_auth = overlay.auth.write().await;
            *stored_auth = svc.state();
        }
        overlay.replace_auth_diagnostics(svc.diagnostics()).await;
    }
    if let Err(error) = emit_overlay_state(&app, &overlay).await {
        tracing::warn!("failed to emit overlay state: {error}");
    }
    tauri::async_runtime::spawn(async move {
        let mut svc = svc.lock().await;
        if let Err(e) = svc.start_device_login(&store).await {
            tracing::error!("device login failed: {e}");
        }
    });
    Ok(())
}

/// Logs out: closes PsyNet connection, clears stored tokens.
#[tauri::command]
pub async fn logout<R: tauri::Runtime>(
    app: AppHandle<R>,
    auth: State<'_, SharedAuthService>,
) -> std::result::Result<(), String> {
    tracing::info!("received logout command");
    let storage = app
        .try_state::<Arc<Storage>>()
        .ok_or_else(|| STORAGE_NOT_READY_MESSAGE.to_owned())?
        .inner()
        .clone();
    let mut svc = auth.lock().await;
    svc.logout(&storage).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        OverlayBackendState, STORAGE_NOT_READY_MESSAGE, build_overlay_state, get_overlay_state,
        start_login, status_message,
    };
    use crate::auth::AuthService;
    use crate::domain::{AuthState, MAX_AUTH_DIAGNOSTICS, OverlayState};
    use crate::storage::Storage;
    use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetConfig};
    use std::sync::Arc;
    use tauri::WebviewWindowBuilder;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;

    fn login_request() -> InvokeRequest {
        InvokeRequest {
            cmd: "start_login".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        }
    }

    fn state_request() -> InvokeRequest {
        InvokeRequest {
            cmd: "get_overlay_state".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        }
    }

    fn build_app(
        storage: Option<Arc<Storage>>,
    ) -> (
        tauri::App<MockRuntime>,
        Arc<tokio::sync::Mutex<AuthService>>,
    ) {
        let auth_service = Arc::new(tokio::sync::Mutex::new(AuthService::new(
            EpicAuthClient::new(),
            PsyNetClient::new(PsyNetConfig::default()),
        )));
        let overlay_state = Arc::new(OverlayBackendState::default());

        let mut builder = mock_builder()
            .manage(overlay_state)
            .manage(auth_service.clone())
            .invoke_handler(tauri::generate_handler![start_login, get_overlay_state]);

        if let Some(storage) = storage {
            builder = builder.manage(storage);
        }

        (
            builder.build(mock_context(noop_assets())).unwrap(),
            auth_service,
        )
    }

    #[test]
    fn start_login_returns_friendly_error_when_storage_is_unavailable() {
        let (app, _) = build_app(None);
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let error = get_ipc_response(&webview, login_request()).unwrap_err();
        assert_eq!(error, serde_json::json!(STORAGE_NOT_READY_MESSAGE));
    }

    #[tokio::test]
    async fn start_login_returns_ok_once_storage_is_managed() {
        let storage = Arc::new(Storage::connect("sqlite::memory:").await.unwrap());
        storage.migrate().await.unwrap();

        let (app, _) = build_app(Some(storage));
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let response = get_ipc_response(&webview, login_request())
            .unwrap()
            .deserialize::<()>();
        assert!(response.is_ok(), "expected empty success response");
    }

    #[tokio::test]
    async fn get_overlay_state_reads_live_auth_service_state() {
        let (app, auth_service) = build_app(None);
        {
            let mut service = auth_service.lock().await;
            service.mark_device_login_started();
        }

        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let overlay = get_ipc_response(&webview, state_request())
            .unwrap()
            .deserialize::<OverlayState>()
            .expect("expected overlay state response");

        assert_eq!(overlay.auth, AuthState::StartingDeviceLogin);
        assert_eq!(
            overlay.auth_diagnostics,
            vec!["Starting Epic device login".to_owned()]
        );
    }

    #[test]
    fn status_message_reports_starting_login() {
        assert_eq!(
            status_message(&AuthState::StartingDeviceLogin, 0),
            "Starting Epic device login"
        );
    }

    #[tokio::test]
    async fn build_overlay_state_exposes_bounded_auth_diagnostics() {
        let state = Arc::new(OverlayBackendState::default());

        for index in 0..(MAX_AUTH_DIAGNOSTICS + 2) {
            state
                .push_auth_diagnostic(format!("diagnostic {index}"))
                .await;
        }

        let overlay = build_overlay_state(&state).await;
        assert_eq!(overlay.auth_diagnostics.len(), MAX_AUTH_DIAGNOSTICS);
        assert_eq!(
            overlay.auth_diagnostics.first().map(String::as_str),
            Some("diagnostic 2")
        );
        assert_eq!(
            overlay.auth_diagnostics.last().map(String::as_str),
            Some("diagnostic 11")
        );
    }
}
