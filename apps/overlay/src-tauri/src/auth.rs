use crate::domain::{AuthState, StoredTokens};
use crate::error::Result;
use crate::storage::Storage;
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

    /// Attempts silent re-authentication using a stored refresh token.
    ///
    /// Returns `Ok(true)` if the refresh succeeded and the client is now connected.
    /// Returns `Ok(false)` if no token was stored or the token is expired/revoked.
    /// In the `false` case, stored tokens are cleared and the caller should
    /// fall back to device authentication.
    pub async fn try_refresh(&mut self, storage: &Storage) -> Result<bool> {
        let stored = storage.load_auth_tokens().await?;
        let Some(tokens) = stored else {
            return Ok(false);
        };

        match self.epic.refresh_eos_token(&tokens.refresh_token).await {
            Ok(token_response) => {
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
                let _ = self.state_tx.send(connected);

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
                let _ = storage.clear_auth_tokens().await;
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
        let device = self.epic.start_device_authorization().await?;
        let waiting = AuthState::WaitingForDeviceCode {
            user_code: device.user_code.clone(),
            verification_uri: device.verification_uri.clone(),
            expires_in: device.expires_in,
        };
        let _ = self.state_tx.send(waiting);

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
            account_id: token.account_id.clone(),
            player_name: token.selected_account_id.clone(),
            refresh_token: token.refresh_token.clone(),
        };
        self.rpc = Some(rpc);
        let _ = self.state_tx.send(connected.clone());

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
        if let Some(rpc) = self.rpc.take() {
            let _ = rpc.close().await;
        }
        let _ = storage.clear_auth_tokens().await;
        let _ = self.state_tx.send(AuthState::Unauthenticated);
    }
}
