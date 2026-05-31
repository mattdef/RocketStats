use crate::domain::{AuthState, MAX_AUTH_DIAGNOSTICS, StoredTokens};
use crate::error::Result;
use crate::storage::Storage;
use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetRpc};
use tokio::sync::watch;

pub struct AuthService {
    epic: EpicAuthClient,
    psynet: PsyNetClient,
    state_tx: watch::Sender<AuthState>,
    diagnostic_tx: watch::Sender<Vec<String>>,
    rpc: Option<PsyNetRpc>,
}

impl AuthService {
    pub fn new(epic: EpicAuthClient, psynet: PsyNetClient) -> Self {
        let (state_tx, _) = watch::channel(AuthState::Unauthenticated);
        let (diagnostic_tx, _) = watch::channel(Vec::new());
        Self {
            epic,
            psynet,
            state_tx,
            diagnostic_tx,
            rpc: None,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<AuthState> {
        self.state_tx.subscribe()
    }

    pub fn subscribe_diagnostics(&self) -> watch::Receiver<Vec<String>> {
        self.diagnostic_tx.subscribe()
    }

    pub fn state(&self) -> AuthState {
        self.state_tx.borrow().clone()
    }

    pub fn diagnostics(&self) -> Vec<String> {
        self.diagnostic_tx.borrow().clone()
    }

    pub fn rpc(&self) -> Option<PsyNetRpc> {
        self.rpc.clone()
    }

    fn set_state(&self, state: AuthState) {
        self.state_tx.send_replace(state);
    }

    fn set_diagnostics(&self, mut diagnostics: Vec<String>) {
        if diagnostics.len() > MAX_AUTH_DIAGNOSTICS {
            let drop_count = diagnostics.len() - MAX_AUTH_DIAGNOSTICS;
            diagnostics.drain(..drop_count);
        }
        self.diagnostic_tx.send_replace(diagnostics);
    }

    pub fn push_diagnostic(&self, message: impl Into<String>) {
        let mut diagnostics = self.diagnostic_tx.borrow().clone();
        diagnostics.push(message.into());
        self.set_diagnostics(diagnostics);
    }

    pub fn mark_device_login_started(&mut self) {
        tracing::info!("starting Epic device login flow");
        self.set_state(AuthState::StartingDeviceLogin);
        self.set_diagnostics(vec!["Starting Epic device login".to_owned()]);
    }

    fn set_error_state(&self, message: impl Into<String>) {
        let message = message.into();
        tracing::error!(auth_error = %message, "auth flow failed");
        self.set_state(AuthState::Error {
            message: message.clone(),
        });
        self.push_diagnostic(format!("Auth error: {message}"));
    }

    /// Attempts silent re-authentication using a stored refresh token.
    ///
    /// Returns `Ok(true)` if the refresh succeeded and the client is now connected.
    /// Returns `Ok(false)` if no token was stored or the token is expired/revoked.
    /// In the `false` case, stored tokens are cleared and the caller should
    /// fall back to device authentication.
    pub async fn try_refresh(&mut self, storage: &Storage) -> Result<bool> {
        let stored = storage.load_auth_tokens().await?;
        let Some(tokens) = stored else {
            tracing::info!("no stored refresh token available");
            return Ok(false);
        };

        tracing::info!("attempting stored Epic session refresh");
        self.push_diagnostic("Attempting stored session refresh");
        match self.epic.refresh_eos_token(&tokens.refresh_token).await {
            Ok(token_response) => {
                tracing::info!("stored Epic session refreshed; connecting to PsyNet");
                self.push_diagnostic("Stored session refreshed");
                self.push_diagnostic("Connecting to PsyNet");
                let rpc = self
                    .psynet
                    .auth_player_eos(
                        token_response.access_token.as_str(),
                        token_response.account_id.as_str(),
                        token_response.selected_account_id.as_deref(),
                    )
                    .await?;

                let connected = AuthState::Connected {
                    account_id: token_response.account_id.clone(),
                    player_name: token_response.selected_account_id.clone(),
                    refresh_token: token_response.refresh_token.clone(),
                };
                self.rpc = Some(rpc);
                self.set_state(connected);
                self.push_diagnostic("Connected to PsyNet");

                // Persist new refresh token if Epic rotated it
                if let Some(new_refresh) = token_response.refresh_token {
                    let _ = storage
                        .store_auth_tokens(&StoredTokens {
                            refresh_token: new_refresh,
                            account_id: token_response.account_id,
                            player_name: token_response.selected_account_id,
                        })
                        .await;
                }

                Ok(true)
            }
            Err(_) => {
                // Token expired or revoked — clear stored credentials
                tracing::warn!("stored Epic session refresh failed");
                let _ = storage.clear_auth_tokens().await;
                self.push_diagnostic("Stored session expired; manual sign-in required");
                Ok(false)
            }
        }
    }

    /// Starts the Epic device-code authentication flow.
    ///
    /// Emits `WaitingForDeviceCode` so the frontend can display the code,
    /// then waits for the user to approve. On success, establishes a
    /// PsyNet WebSocket connection and persists the refresh token.
    pub async fn start_device_login(&mut self, storage: &Storage) -> Result<AuthState> {
        match self.start_device_login_inner(storage).await {
            Ok(state) => Ok(state),
            Err(error) => {
                self.set_error_state(error.to_string());
                Err(error)
            }
        }
    }

    async fn start_device_login_inner(&mut self, storage: &Storage) -> Result<AuthState> {
        tracing::info!("requesting Epic device code");
        self.push_diagnostic("Requesting Epic device code");
        let device = self.epic.start_device_authorization().await?;
        tracing::info!("received Epic device code");
        self.push_diagnostic("Epic device code ready");
        let waiting = AuthState::WaitingForDeviceCode {
            user_code: device.user_code.clone(),
            verification_uri: device.verification_uri.clone(),
            expires_in: device.expires_in,
        };
        self.set_state(waiting);

        tracing::info!("waiting for Epic approval");
        self.push_diagnostic("Waiting for Epic approval");
        let token = self.epic.wait_for_device_authorization(&device).await?;
        tracing::info!("Epic approval received");
        self.push_diagnostic("Epic approval received");
        tracing::info!("connecting to PsyNet");
        self.push_diagnostic("Connecting to PsyNet");
        let rpc = self
            .psynet
            .auth_player_eos(
                token.access_token.as_str(),
                token.account_id.as_str(),
                token.selected_account_id.as_deref(),
            )
            .await?;

        let connected = AuthState::Connected {
            account_id: token.account_id.clone(),
            player_name: token.selected_account_id.clone(),
            refresh_token: token.refresh_token.clone(),
        };
        self.rpc = Some(rpc);
        self.set_state(connected.clone());
        tracing::info!("connected to PsyNet");
        self.push_diagnostic("Connected to PsyNet");

        // Persist refresh token for future silent re-auth
        if let Some(refresh) = token.refresh_token {
            let _ = storage
                .store_auth_tokens(&StoredTokens {
                    refresh_token: refresh,
                    account_id: token.account_id,
                    player_name: token.selected_account_id,
                })
                .await;
        }

        Ok(connected)
    }

    /// Logs out: closes the PsyNet WebSocket, clears stored tokens,
    /// and resets state to `Unauthenticated`.
    pub async fn logout(&mut self, storage: &Storage) {
        tracing::info!("logging out of Epic/PsyNet session");
        if let Some(rpc) = self.rpc.take() {
            let _ = rpc.close().await;
        }
        let _ = storage.clear_auth_tokens().await;
        self.set_state(AuthState::Unauthenticated);
        self.set_diagnostics(Vec::new());
    }
}

#[cfg(test)]
mod tests {
    use super::AuthService;
    use crate::domain::{AuthState, MAX_AUTH_DIAGNOSTICS};
    use rocketstats_rlapi::{EpicAuthClient, PsyNetClient, PsyNetConfig};

    #[test]
    fn marks_login_as_starting_before_device_code_is_available() {
        let mut service = AuthService::new(
            EpicAuthClient::new(),
            PsyNetClient::new(PsyNetConfig::default()),
        );

        service.mark_device_login_started();

        assert_eq!(service.state(), AuthState::StartingDeviceLogin);
    }

    #[test]
    fn bounds_auth_diagnostics_to_recent_entries() {
        let service = AuthService::new(
            EpicAuthClient::new(),
            PsyNetClient::new(PsyNetConfig::default()),
        );

        for index in 0..(MAX_AUTH_DIAGNOSTICS + 2) {
            service.push_diagnostic(format!("step {index}"));
        }

        let diagnostics = service.diagnostics();
        assert_eq!(diagnostics.len(), MAX_AUTH_DIAGNOSTICS);
        assert_eq!(diagnostics.first().map(String::as_str), Some("step 2"));
        assert_eq!(diagnostics.last().map(String::as_str), Some("step 11"));
    }
}
