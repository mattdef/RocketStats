//! RocketStats Rocket League PsyNet SDK.

use std::{
    collections::HashMap,
    fmt,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use reqwest::{Client, Url, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::Sha256;
use thiserror::Error;
use tokio::{
    sync::{Mutex, broadcast, mpsc, oneshot},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

const PSY_SIG_KEY: &[u8] = b"c338bd36fb8c42b1a431d30add939fc7";
const EGS_USER_AGENT: &str =
    "UELauncher/11.0.1-14907503+++Portal+Release-Live Windows/10.0.19041.1.256.64bit";
const EOS_DEPLOYMENT_ID: &str = "da32ae9c12ae40e8a112c52e1f17f3ba";
const EOS_CLIENT_ID: &str = "xyza7891p5D7s9R6Gm6moTHWGloerp7B";
const EOS_SECRET: &str = "Knh18du4NVlFs+3uQ+ZPpDCVto0WYf4yXP8+OcwVt1o";
const PING_INTERVAL: Duration = Duration::from_secs(20);
const PONG_TIMEOUT: Duration = Duration::from_secs(10);

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type WsWrite =
    futures_util::stream::SplitSink<WsStream, tokio_tungstenite::tungstenite::protocol::Message>;

#[derive(Debug, Error)]
pub enum RlApiError {
    #[error("invalid player id: {0}")]
    InvalidPlayerId(String),
    #[error("invalid psynet message: {0}")]
    InvalidMessage(String),
    #[error("psynet error {error_type}: {message}")]
    PsyNet { error_type: String, message: String },
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("request timed out")]
    Timeout,
    #[error("connection is closed")]
    Closed,
    #[error("response channel closed")]
    ResponseClosed,
}

pub type Result<T> = std::result::Result<T, RlApiError>;

#[derive(Debug, Clone)]
pub struct PsyNetConfig {
    pub base_url: Url,
    pub game_version: String,
    pub feature_set: String,
    pub environment: String,
}

impl Default for PsyNetConfig {
    fn default() -> Self {
        Self {
            base_url: Url::parse("https://api.rlpp.psynet.gg/rpc").expect("valid default URL"),
            game_version: "260506.26700.517210".to_owned(),
            feature_set: "PrimeUpdate58_1".to_owned(),
            environment: "Prod".to_owned(),
        }
    }
}

impl PsyNetConfig {
    pub fn with_base_url(mut self, base_url: Url) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn with_game_version(mut self, game_version: impl Into<String>) -> Self {
        self.game_version = game_version.into();
        self
    }

    pub fn with_feature_set(mut self, feature_set: impl Into<String>) -> Self {
        self.feature_set = feature_set.into();
        self
    }

    pub fn psy_build_id(&self) -> String {
        decode_build_id(&self.game_version).to_string()
    }
}

pub fn decode_build_id(s: &str) -> i32 {
    let mut data = Vec::with_capacity(s.len() * 2);
    for unit in s.encode_utf16() {
        data.push((unit & 0xff) as u8);
        data.push((unit >> 8) as u8);
    }
    crc32_be(&data, 0)
}

fn crc32_be(data: &[u8], seed: u32) -> i32 {
    const POLY: u32 = 0x04C11DB7;
    let mut crc = seed ^ 0xffff_ffff;
    for byte in data {
        crc ^= (*byte as u32) << 24;
        for _ in 0..8 {
            if crc & 0x8000_0000 != 0 {
                crc = (crc << 1) ^ POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    (crc ^ 0xffff_ffff) as i32
}

pub fn generate_psy_sig(body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(PSY_SIG_KEY).expect("HMAC accepts any key length");
    mac.update(b"-");
    mac.update(body);
    BASE64.encode(mac.finalize().into_bytes())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "Epic")]
    Epic,
    #[serde(rename = "Steam")]
    Steam,
    #[serde(rename = "PS4")]
    Ps4,
    #[serde(rename = "XboxOne")]
    XboxOne,
    #[serde(rename = "Switch")]
    Switch,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Epic => "Epic",
            Self::Steam => "Steam",
            Self::Ps4 => "PS4",
            Self::XboxOne => "XboxOne",
            Self::Switch => "Switch",
        })
    }
}

impl FromStr for Platform {
    type Err = RlApiError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Epic" => Ok(Self::Epic),
            "Steam" => Ok(Self::Steam),
            "PS4" => Ok(Self::Ps4),
            "XboxOne" => Ok(Self::XboxOne),
            "Switch" => Ok(Self::Switch),
            _ => Err(RlApiError::InvalidPlayerId(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(String);

impl PlayerId {
    pub fn new(platform: Platform, account_id: impl AsRef<str>) -> Self {
        Self(format!("{}|{}|0", platform, account_id.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn platform(&self) -> Platform {
        self.parts().0
    }

    pub fn account_id(&self) -> &str {
        self.parts().1
    }

    fn parts(&self) -> (Platform, &str) {
        let mut parts = self.0.split('|');
        let platform = parts
            .next()
            .unwrap_or_default()
            .parse()
            .unwrap_or(Platform::Epic);
        let account_id = parts.next().unwrap_or_default();
        (platform, account_id)
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PlayerId {
    type Err = RlApiError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<_> = s.split('|').collect();
        if parts.len() != 3 || parts[2] != "0" {
            return Err(RlApiError::InvalidPlayerId(s.to_owned()));
        }
        parts[0].parse::<Platform>()?;
        if parts[1].is_empty() {
            return Err(RlApiError::InvalidPlayerId(s.to_owned()));
        }
        Ok(Self(s.to_owned()))
    }
}

#[derive(Default)]
pub struct RequestIdCounter {
    value: AtomicI64,
}

impl RequestIdCounter {
    pub fn next_id(&self) -> String {
        let id = self.value.fetch_add(1, Ordering::SeqCst);
        format!("PsyNetMessage_X_{id}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PsyNetErrorPayload {
    #[serde(rename = "Type")]
    pub error_type: String,
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ParsedPsyNetMessage {
    pub response_id: Option<String>,
    #[serde(rename = "Result")]
    pub result: Option<Value>,
    #[serde(rename = "Error")]
    pub error: Option<PsyNetErrorPayload>,
}

pub fn build_psynet_message<'a, I, B>(headers: I, body: Option<&B>) -> Result<String>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
    B: Serialize + ?Sized,
{
    let mut header_pairs: Vec<(String, String)> = headers
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    let body_bytes = match body {
        Some(body) => {
            let bytes = serde_json::to_vec(body)?;
            header_pairs.push(("PsySig".to_owned(), generate_psy_sig(&bytes)));
            bytes
        }
        None => Vec::new(),
    };

    let mut message = String::new();
    for (key, value) in header_pairs {
        message.push_str(&key);
        message.push_str(": ");
        message.push_str(&value);
        message.push_str("\r\n");
    }
    message.push_str("\r\n");
    message.push_str(std::str::from_utf8(&body_bytes).unwrap_or_default());
    Ok(message)
}

pub fn parse_psynet_message(message: &str) -> Result<ParsedPsyNetMessage> {
    let (headers, body) = message
        .split_once("\r\n\r\n")
        .ok_or_else(|| RlApiError::InvalidMessage("missing header/body delimiter".to_owned()))?;

    let response_id = headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "PsyResponseID").then(|| value.trim().to_owned())
    });

    let mut parsed: ParsedPsyNetMessage = serde_json::from_str(body)?;
    parsed.response_id = response_id;
    Ok(parsed)
}

#[derive(Debug, Deserialize)]
pub struct DeviceAuthResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EosTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: u64,
    pub expires_at: Option<String>,
    pub refresh_expires_in: Option<u64>,
    pub refresh_expires_at: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub client_id: String,
    pub application_id: Option<String>,
    pub account_id: String,
    pub selected_account_id: Option<String>,
}

#[derive(Clone)]
pub struct EpicAuthClient {
    client: Client,
}

impl Default for EpicAuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EpicAuthClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()
                .expect("valid Epic auth HTTP client"),
        }
    }

    pub async fn start_device_authorization(&self) -> Result<DeviceAuthResponse> {
        tracing::info!("requesting Epic device authorization");
        let response = self
            .client
            .post("https://api.epicgames.dev/epic/oauth/v2/deviceAuthorization")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::USER_AGENT, EGS_USER_AGENT)
            .body(format!("client_id={EOS_CLIENT_ID}"))
            .send()
            .await?
            .error_for_status()?;
        tracing::info!("received Epic device authorization response");
        Ok(response.json().await?)
    }

    pub async fn wait_for_device_authorization(
        &self,
        device: &DeviceAuthResponse,
    ) -> Result<EosTokenResponse> {
        let attempts = device.expires_in / device.interval.max(1);
        tracing::info!(
            attempts,
            interval_seconds = device.interval.max(1),
            "polling for Epic device approval"
        );
        for attempt in 0..attempts {
            match self
                .request_eos_token([
                    ("grant_type", "device_code"),
                    ("device_code", device.device_code.as_str()),
                ])
                .await
            {
                Ok(token) => {
                    tracing::info!(attempt = attempt + 1, "Epic device approval completed");
                    return Ok(token);
                }
                Err(RlApiError::Http(_)) => {
                    tracing::debug!(attempt = attempt + 1, "Epic device approval still pending");
                    time::sleep(Duration::from_secs(device.interval.max(1))).await;
                }
                Err(err) => return Err(err),
            }
        }
        tracing::warn!("Epic device approval timed out");
        Err(RlApiError::Timeout)
    }

    pub async fn refresh_eos_token(&self, refresh_token: &str) -> Result<EosTokenResponse> {
        self.request_eos_token([
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .await
    }

    pub async fn revoke_eos_token(&self, access_token: &str) -> Result<()> {
        self.client
            .post("https://api.epicgames.dev/epic/oauth/v2/revoke")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::USER_AGENT, EGS_USER_AGENT)
            .body(format!("token={access_token}"))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn request_eos_token<'a, I>(&self, params: I) -> Result<EosTokenResponse>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut form: Vec<(&str, &str)> = params.into_iter().collect();
        form.push(("deployment_id", EOS_DEPLOYMENT_ID));
        form.push(("scope", "basic_profile"));

        let basic = BASE64.encode(format!("{EOS_CLIENT_ID}:{EOS_SECRET}"));
        let response = self
            .client
            .post("https://api.epicgames.dev/epic/oauth/v2/token")
            .header(header::AUTHORIZATION, format!("Basic {basic}"))
            .header(header::USER_AGENT, EGS_USER_AGENT)
            .form(&form)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }
}

#[derive(Clone)]
pub struct PsyNetClient {
    client: Client,
    config: PsyNetConfig,
    request_ids: Arc<RequestIdCounter>,
}

impl Default for PsyNetClient {
    fn default() -> Self {
        Self::new(PsyNetConfig::default())
    }
}

impl PsyNetClient {
    pub fn new(config: PsyNetConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            request_ids: Arc::new(RequestIdCounter::default()),
        }
    }

    pub async fn auth_player_eos(
        &self,
        access_token: &str,
        account_id: &str,
        account_name: Option<&str>,
    ) -> Result<PsyNetRpc> {
        tracing::info!("authenticating with PsyNet");
        let local_player_id = PlayerId::new(Platform::Epic, account_id);
        let request = AuthPlayerRequest {
            platform: Platform::Epic.to_string(),
            player_name: account_name.unwrap_or_default().to_owned(),
            player_id: account_id.to_owned(),
            language: "INT".to_owned(),
            auth_ticket: access_token.to_owned(),
            build_region: String::new(),
            feature_set: self.config.feature_set.clone(),
            device: "PC".to_owned(),
            local_first_player_id: local_player_id.to_string(),
            skip_auth: false,
            set_as_primary_account: true,
            epic_auth_ticket: access_token.to_owned(),
            epic_account_id: account_id.to_owned(),
        };

        let auth: AuthPlayerResponse = self
            .post_json(&["Auth", "AuthPlayer", "v2"], &request)
            .await?;
        let rpc = self
            .establish_socket(
                auth.per_con_url_v2.as_str(),
                local_player_id,
                auth.psy_token.as_str(),
                auth.session_id.as_str(),
            )
            .await?;
        tracing::info!("connected to PsyNet websocket");
        Ok(rpc)
    }

    async fn post_json<TReq, TRes>(&self, path: &[&str], params: &TReq) -> Result<TRes>
    where
        TReq: Serialize + ?Sized,
        TRes: DeserializeOwned,
    {
        let body = serde_json::to_vec(params)?;
        let mut url = self.config.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| RlApiError::InvalidMessage("base URL cannot be a base".to_owned()))?
            .extend(path);
        let response = self
            .client
            .post(url)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::USER_AGENT,
                format!(
                    "RL Win/{} gzip (x86_64-pc-win32) curl-7.67.0 Schannel",
                    self.config.game_version
                ),
            )
            .header("PsyBuildID", self.config.psy_build_id())
            .header("PsyEnvironment", self.config.environment.as_str())
            .header("PsyRequestID", self.request_ids.next_id())
            .header("PsySig", generate_psy_sig(&body))
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        let wrapper: PsyWrapperResponse<TRes> = response.json().await?;
        match (wrapper.result, wrapper.error) {
            (Some(result), None) => Ok(result),
            (_, Some(error)) => Err(RlApiError::PsyNet {
                error_type: error.error_type,
                message: error.message,
            }),
            (None, None) => Err(RlApiError::InvalidMessage(
                "response contains neither Result nor Error".to_owned(),
            )),
        }
    }

    async fn establish_socket(
        &self,
        url: &str,
        local_player_id: PlayerId,
        psy_token: &str,
        session_id: &str,
    ) -> Result<PsyNetRpc> {
        let request = http_request_with_headers(
            url,
            &self.config.psy_build_id(),
            &self.config.game_version,
            &self.config.environment,
            psy_token,
            session_id,
        )?;
        let (stream, _) = connect_async(request).await?;
        Ok(PsyNetRpc::new(
            stream,
            local_player_id,
            Arc::clone(&self.request_ids),
        ))
    }
}

fn http_request_with_headers(
    url: &str,
    psy_build_id: &str,
    game_version: &str,
    environment: &str,
    psy_token: &str,
    session_id: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let mut request = url
        .into_client_request()
        .map_err(|err| RlApiError::InvalidMessage(err.to_string()))?;
    let headers = request.headers_mut();
    headers.insert(
        "PsyBuildID",
        psy_build_id
            .parse()
            .map_err(|err| RlApiError::InvalidMessage(format!("invalid PsyBuildID: {err}")))?,
    );
    headers.insert(
        "User-Agent",
        format!("RL Win/{game_version} gzip")
            .parse()
            .map_err(|err| RlApiError::InvalidMessage(format!("invalid User-Agent: {err}")))?,
    );
    headers.insert(
        "PsyEnvironment",
        environment
            .parse()
            .map_err(|err| RlApiError::InvalidMessage(format!("invalid PsyEnvironment: {err}")))?,
    );
    headers.insert(
        "PsyToken",
        psy_token
            .parse()
            .map_err(|err| RlApiError::InvalidMessage(format!("invalid PsyToken: {err}")))?,
    );
    headers.insert(
        "PsySessionID",
        session_id
            .parse()
            .map_err(|err| RlApiError::InvalidMessage(format!("invalid PsySessionID: {err}")))?,
    );
    Ok(request)
}

#[derive(Serialize)]
struct AuthPlayerRequest {
    #[serde(rename = "Platform")]
    platform: String,
    #[serde(rename = "PlayerName")]
    player_name: String,
    #[serde(rename = "PlayerID")]
    player_id: String,
    #[serde(rename = "Language")]
    language: String,
    #[serde(rename = "AuthTicket")]
    auth_ticket: String,
    #[serde(rename = "BuildRegion")]
    build_region: String,
    #[serde(rename = "FeatureSet")]
    feature_set: String,
    #[serde(rename = "Device")]
    device: String,
    #[serde(rename = "LocalFirstPlayerID")]
    local_first_player_id: String,
    #[serde(rename = "bSkipAuth")]
    skip_auth: bool,
    #[serde(rename = "bSetAsPrimaryAccount")]
    set_as_primary_account: bool,
    #[serde(rename = "EpicAuthTicket")]
    epic_auth_ticket: String,
    #[serde(rename = "EpicAccountID")]
    epic_account_id: String,
}

#[derive(Deserialize)]
struct AuthPlayerResponse {
    #[serde(rename = "SessionID")]
    session_id: String,
    #[serde(rename = "PerConURLv2")]
    per_con_url_v2: String,
    #[serde(rename = "PsyToken")]
    psy_token: String,
}

#[derive(Deserialize)]
struct PsyWrapperResponse<T> {
    #[serde(rename = "Result")]
    result: Option<T>,
    #[serde(rename = "Error")]
    error: Option<PsyNetErrorPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsyNetEvent {
    Disconnected,
    Message(String),
}

#[derive(Clone)]
pub struct PsyNetRpc {
    write: Arc<Mutex<WsWrite>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ParsedPsyNetMessage>>>>,
    request_ids: Arc<RequestIdCounter>,
    local_player_id: PlayerId,
    events: broadcast::Sender<PsyNetEvent>,
    closed: Arc<tokio::sync::watch::Sender<bool>>,
    read_task: Arc<JoinHandle<()>>,
    ping_task: Arc<JoinHandle<()>>,
}

impl PsyNetRpc {
    fn new(
        stream: WsStream,
        local_player_id: PlayerId,
        request_ids: Arc<RequestIdCounter>,
    ) -> Self {
        let (write, mut read) = stream.split();
        let write = Arc::new(Mutex::new(write));
        let pending: Arc<Mutex<HashMap<String, oneshot::Sender<ParsedPsyNetMessage>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(32);
        let (closed_tx, closed_rx) = tokio::sync::watch::channel(false);
        let (pong_tx, mut pong_rx) = mpsc::channel::<()>(1);

        let read_pending = Arc::clone(&pending);
        let read_events = events.clone();
        let read_closed = closed_tx.clone();
        let read_task = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        let text = text.to_string();
                        if text.starts_with("PsyPong:") {
                            let _ = pong_tx.try_send(());
                            continue;
                        }
                        match parse_psynet_message(&text) {
                            Ok(parsed) => {
                                if let Some(response_id) = parsed.response_id.as_deref()
                                    && let Some(sender) =
                                        read_pending.lock().await.remove(response_id)
                                {
                                    let _ = sender.send(parsed);
                                    continue;
                                }
                                let _ = read_events.send(PsyNetEvent::Message(text));
                            }
                            Err(_) => {
                                let _ = read_events.send(PsyNetEvent::Message(text));
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
            let _ = read_closed.send(true);
            read_pending.lock().await.clear();
            let _ = read_events.send(PsyNetEvent::Disconnected);
        });

        let ping_write = Arc::clone(&write);
        let ping_closed = closed_tx.clone();
        let mut ping_closed_rx = closed_rx.clone();
        let ping_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = ping_closed_rx.changed() => break,
                    _ = time::sleep(PING_INTERVAL) => {}
                }

                let Ok(ping) = build_psynet_message([("PsyPing", "")], Option::<&()>::None) else {
                    break;
                };
                if ping_write
                    .lock()
                    .await
                    .send(Message::Text(ping.into()))
                    .await
                    .is_err()
                {
                    break;
                }

                match time::timeout(PONG_TIMEOUT, pong_rx.recv()).await {
                    Ok(Some(())) => {}
                    _ => break,
                }
            }
            let _ = ping_closed.send(true);
        });

        Self {
            write,
            pending,
            request_ids,
            local_player_id,
            events,
            closed: Arc::new(closed_tx),
            read_task: Arc::new(read_task),
            ping_task: Arc::new(ping_task),
        }
    }

    pub fn local_player_id(&self) -> &PlayerId {
        &self.local_player_id
    }

    pub fn events(&self) -> broadcast::Receiver<PsyNetEvent> {
        self.events.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        !*self.closed.borrow()
    }

    pub async fn close(&self) -> Result<()> {
        let _ = self.closed.send(true);
        let _ = self
            .write
            .lock()
            .await
            .send(Message::Close(None))
            .await
            .map_err(RlApiError::from);
        Ok(())
    }

    pub async fn send_request<TReq, TRes>(&self, service: &str, data: &TReq) -> Result<TRes>
    where
        TReq: Serialize + ?Sized,
        TRes: DeserializeOwned,
    {
        if !self.is_connected() {
            return Err(RlApiError::Closed);
        }

        let request_id = self.request_ids.next_id();
        let message = build_psynet_message(
            [
                ("PsyService", service),
                ("PsyRequestID", request_id.as_str()),
            ],
            Some(data),
        )?;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);

        if let Err(err) = self
            .write
            .lock()
            .await
            .send(Message::Text(message.into()))
            .await
        {
            self.pending.lock().await.remove(&request_id);
            return Err(err.into());
        }

        let parsed = receiver.await.map_err(|_| RlApiError::ResponseClosed)?;
        if let Some(error) = parsed.error {
            return Err(RlApiError::PsyNet {
                error_type: error.error_type,
                message: error.message,
            });
        }
        let result = parsed
            .result
            .ok_or_else(|| RlApiError::InvalidMessage("missing Result".to_owned()))?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_profiles(&self, player_ids: Vec<PlayerId>) -> Result<Vec<PlayerData>> {
        let response: GetProfileResponse = self
            .send_request("Players/GetProfile v1", &json!({ "PlayerIDs": player_ids }))
            .await?;
        Ok(response.player_data)
    }

    pub async fn get_xp(&self) -> Result<PlayerXpInfo> {
        let response: GetXpResponse = self
            .send_request(
                "Players/GetXP v1",
                &json!({ "PlayerID": self.local_player_id }),
            )
            .await?;
        Ok(response.xp_info_response)
    }

    pub async fn get_player_skill(&self, player_id: PlayerId) -> Result<GetPlayerSkillResponse> {
        self.send_request(
            "Skills/GetPlayerSkill v1",
            &json!({ "PlayerID": player_id }),
        )
        .await
    }

    pub async fn get_players_skills(
        &self,
        player_ids: Vec<PlayerId>,
    ) -> Result<Vec<PlayerWithSkills>> {
        let response: GetPlayersSkillsResponse = self
            .send_request(
                "Skills/GetPlayersSkills v1",
                &json!({ "PlayerIDs": player_ids }),
            )
            .await?;
        Ok(response.players)
    }

    pub async fn get_skill_leaderboard_value_for_user(
        &self,
        playlist: PlaylistId,
        player_id: PlayerId,
    ) -> Result<GetSkillLeaderboardValueForUserResponse> {
        self.send_request(
            "Skills/GetSkillLeaderboardValueForUser v1",
            &json!({ "Playlist": playlist, "PlayerID": player_id }),
        )
        .await
    }

    pub async fn get_active_playlists(&self) -> Result<GetActivePlaylistsResponse> {
        self.send_request("Playlists/GetActivePlaylists v1", &json!({}))
            .await
    }

    pub async fn get_population(&self) -> Result<Vec<PlaylistPopulation>> {
        let response: GetPopulationResponse = self
            .send_request("Population/GetPopulation v1", &json!({}))
            .await?;
        Ok(response.playlists)
    }

    pub async fn get_match_history(&self) -> Result<Vec<MatchEntry>> {
        let response: GetMatchHistoryResponse = self
            .send_request(
                "Matches/GetMatchHistory v1",
                &json!({ "PlayerID": self.local_player_id }),
            )
            .await?;
        Ok(response.matches)
    }
}

impl Drop for PsyNetRpc {
    fn drop(&mut self) {
        let _ = self.closed.send(true);
        self.read_task.abort();
        self.ping_task.abort();
    }
}

pub type PlaylistId = i32;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Skill {
    #[serde(rename = "Playlist")]
    pub playlist: i32,
    #[serde(rename = "Mu")]
    pub mu: f64,
    #[serde(rename = "Sigma")]
    pub sigma: f64,
    #[serde(rename = "Tier")]
    pub tier: i32,
    #[serde(rename = "Division")]
    pub division: i32,
    #[serde(rename = "MMR")]
    pub mmr: f64,
    #[serde(rename = "WinStreak")]
    pub win_streak: i32,
    #[serde(rename = "MatchesPlayed")]
    pub matches_played: i32,
    #[serde(rename = "PlacementMatchesPlayed")]
    pub placement_matches_played: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RewardLevels {
    #[serde(rename = "SeasonLevel")]
    pub season_level: i32,
    #[serde(rename = "SeasonLevelWins")]
    pub season_level_wins: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PlayerWithSkills {
    #[serde(rename = "PlayerID")]
    pub player_id: PlayerId,
    #[serde(rename = "Skills")]
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GetPlayerSkillResponse {
    #[serde(rename = "Skills")]
    pub skills: Vec<Skill>,
    #[serde(rename = "RewardLevels")]
    pub reward_levels: RewardLevels,
}

#[derive(Debug, Deserialize)]
struct GetPlayersSkillsResponse {
    #[serde(rename = "Players")]
    players: Vec<PlayerWithSkills>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GetSkillLeaderboardValueForUserResponse {
    #[serde(rename = "LeaderboardID")]
    pub leaderboard_id: String,
    #[serde(rename = "bHasSkill")]
    pub has_skill: bool,
    #[serde(rename = "MMR")]
    pub mmr: f64,
    #[serde(rename = "Value")]
    pub value: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlayerData {
    #[serde(rename = "PlayerID")]
    pub player_id: String,
    #[serde(rename = "PlayerName")]
    pub player_name: String,
    #[serde(rename = "PresenceState")]
    pub presence_state: String,
    #[serde(rename = "PresenceInfo")]
    pub presence_info: String,
}

#[derive(Debug, Deserialize)]
struct GetProfileResponse {
    #[serde(rename = "PlayerData")]
    player_data: Vec<PlayerData>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlayerXpInfo {
    #[serde(rename = "TotalXP")]
    pub total_xp: i32,
    #[serde(rename = "XPLevel")]
    pub xp_level: i32,
    #[serde(rename = "XPTitle")]
    pub xp_title: String,
    #[serde(rename = "XPProgressInCurrentLevel")]
    pub xp_progress_in_current_level: i32,
    #[serde(rename = "XPRequiredForNextLevel")]
    pub xp_required_for_next_level: i32,
}

#[derive(Debug, Deserialize)]
struct GetXpResponse {
    #[serde(rename = "XPInfoResponse")]
    xp_info_response: PlayerXpInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Playlist {
    #[serde(rename = "NodeID")]
    pub node_id: String,
    #[serde(rename = "Playlist")]
    pub playlist: i32,
    #[serde(rename = "Type")]
    pub playlist_type: i32,
    #[serde(rename = "StartTime")]
    pub start_time: Option<i32>,
    #[serde(rename = "EndTime")]
    pub end_time: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GetActivePlaylistsResponse {
    #[serde(rename = "CasualPlaylists")]
    pub casual_playlists: Vec<Playlist>,
    #[serde(rename = "RankedPlaylists")]
    pub ranked_playlists: Vec<Playlist>,
    #[serde(rename = "XPLevelUnlocked")]
    pub xp_level_unlocked: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlaylistPopulation {
    #[serde(rename = "PlaylistID")]
    pub playlist_id: PlaylistId,
    #[serde(rename = "Population")]
    pub population: i32,
}

#[derive(Debug, Deserialize)]
struct GetPopulationResponse {
    #[serde(rename = "Playlists")]
    playlists: Vec<PlaylistPopulation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MatchEntry {
    #[serde(rename = "ReplayUrl")]
    pub replay_url: String,
    #[serde(rename = "Match")]
    pub game_match: Match,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Match {
    #[serde(rename = "MatchGUID")]
    pub match_guid: String,
    #[serde(rename = "RecordStartTimestamp")]
    pub record_start_timestamp: i64,
    #[serde(rename = "MapName")]
    pub map_name: String,
    #[serde(rename = "Playlist")]
    pub playlist: i32,
    #[serde(rename = "SecondsPlayed")]
    pub seconds_played: f64,
    #[serde(rename = "OvertimeSecondsPlayed")]
    pub overtime_seconds_played: f64,
    #[serde(rename = "WinningTeam")]
    pub winning_team: i32,
    #[serde(rename = "Team0Score")]
    pub team0_score: i32,
    #[serde(rename = "Team1Score")]
    pub team1_score: i32,
    #[serde(rename = "bOverTime")]
    pub over_time: bool,
    #[serde(rename = "bNoContest")]
    pub no_contest: bool,
    #[serde(rename = "bForfeit")]
    pub forfeit: bool,
    #[serde(rename = "bClubVsClub")]
    pub club_vs_club: bool,
    #[serde(rename = "Mutators")]
    pub mutators: Vec<String>,
    #[serde(rename = "Players")]
    pub players: Vec<MatchPlayer>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MatchPlayer {
    #[serde(rename = "PlayerID")]
    pub player_id: String,
    #[serde(rename = "PlayerName")]
    pub player_name: String,
    #[serde(rename = "Score")]
    pub score: i32,
    #[serde(rename = "Goals")]
    pub goals: i32,
    #[serde(rename = "Assists")]
    pub assists: i32,
    #[serde(rename = "Saves")]
    pub saves: i32,
    #[serde(rename = "Shots")]
    pub shots: i32,
}

#[derive(Debug, Deserialize)]
struct GetMatchHistoryResponse {
    #[serde(rename = "Matches")]
    matches: Vec<MatchEntry>,
}
