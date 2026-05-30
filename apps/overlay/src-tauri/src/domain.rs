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
