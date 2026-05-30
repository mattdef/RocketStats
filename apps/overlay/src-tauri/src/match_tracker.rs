use crate::domain::{DetectedPlayerId, LogEvent, MatchPhase, MatchSession};

#[derive(Debug, Default)]
pub struct MatchTracker {
    session: MatchSession,
}

impl MatchTracker {
    pub fn apply(&mut self, event: LogEvent) -> MatchSession {
        match event {
            LogEvent::MatchmakingStarted {
                playlist,
                regions,
                timestamp_ms: _,
            } => {
                self.session = MatchSession::default();
                self.session.phase = MatchPhase::Matchmaking;
                self.session.playlist = Some(playlist);
                self.session.regions = regions;
            }
            LogEvent::ServerReserved {
                server_name,
                region,
                playlist,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Joining;
                self.session.server_name = Some(server_name);
                self.session.playlist = Some(playlist);
                if self.session.regions.is_empty() {
                    self.session.regions.push(region);
                }
            }
            LogEvent::ServerJoined {
                map,
                server,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Joining;
                self.session.map = Some(map);
                if server != "unknown" {
                    self.session.server_name = Some(server);
                }
            }
            LogEvent::MatchGuidSeen {
                guid,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::InMatch;
                self.session.guid = Some(guid);
            }
            LogEvent::PlayerIdSeen {
                player_id,
                timestamp_ms,
            } => {
                if !self
                    .session
                    .detected_players
                    .iter()
                    .any(|existing| existing.value == player_id)
                {
                    self.session.detected_players.push(DetectedPlayerId {
                        value: player_id,
                        first_seen_ms: timestamp_ms,
                    });
                }
            }
            LogEvent::MatchEnded {
                guid,
                local_score,
                duration_seconds,
                xp,
                timestamp_ms: _,
            } => {
                self.session.phase = MatchPhase::Ended;
                if guid.is_some() {
                    self.session.guid = guid;
                }
                self.session.local_score = Some(local_score);
                self.session.duration_seconds = Some(duration_seconds);
                if xp.is_some() {
                    self.session.xp = xp;
                }
            }
        }

        self.session.clone()
    }

    pub fn session(&self) -> &MatchSession {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::MatchTracker;
    use crate::domain::{DetectedPlayerId, LogEvent, MatchPhase, MatchSession};

    #[test]
    fn tracks_match_lifecycle_and_deduplicates_players() {
        let mut tracker = MatchTracker::default();

        tracker.apply(LogEvent::MatchmakingStarted {
            playlist: 11,
            regions: vec!["EU9".to_owned(), "EU7".to_owned()],
            timestamp_ms: 223_910,
        });
        tracker.apply(LogEvent::ServerJoined {
            map: "FF_Dusk_P".to_owned(),
            server: "unknown".to_owned(),
            timestamp_ms: 238_790,
        });
        tracker.apply(LogEvent::MatchGuidSeen {
            guid: "706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned(),
            timestamp_ms: 240_990,
        });
        tracker.apply(LogEvent::PlayerIdSeen {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            timestamp_ms: 241_000,
        });
        tracker.apply(LogEvent::PlayerIdSeen {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            timestamp_ms: 241_500,
        });
        let session = tracker.apply(LogEvent::MatchEnded {
            guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
            local_score: 323,
            duration_seconds: 302.9711,
            xp: Some(5160),
            timestamp_ms: 619_020,
        });

        assert_eq!(
            session,
            MatchSession {
                phase: MatchPhase::Ended,
                playlist: Some(11),
                regions: vec!["EU9".to_owned(), "EU7".to_owned()],
                server_name: None,
                map: Some("FF_Dusk_P".to_owned()),
                guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
                detected_players: vec![DetectedPlayerId {
                    value: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                    first_seen_ms: 241_000,
                }],
                local_score: Some(323),
                duration_seconds: Some(302.9711),
                xp: Some(5160),
            }
        );
    }
}
