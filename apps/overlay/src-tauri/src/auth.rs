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
