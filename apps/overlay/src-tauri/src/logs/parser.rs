use crate::domain::LogEvent;

pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    let timestamp_ms = parse_timestamp_ms(line)?;
    let body = line.split_once("] ")?.1;

    parse_matchmaking_started(body, timestamp_ms)
        .or_else(|| parse_server_joined(body, timestamp_ms))
        .or_else(|| parse_match_guid(body, timestamp_ms))
        .or_else(|| parse_player_id(body, timestamp_ms))
        .or_else(|| parse_match_end_with_xp(body, timestamp_ms))
        .or_else(|| parse_match_end_without_xp(body, timestamp_ms))
}

fn parse_timestamp_ms(line: &str) -> Option<u64> {
    let timestamp = line.strip_prefix('[')?.split_once(']')?.0;
    let (seconds, fractional) = timestamp.split_once('.')?;
    let seconds = seconds.parse::<u64>().ok()?;
    let millis = match fractional.len() {
        0 => 0,
        1 => fractional.parse::<u64>().ok()? * 100,
        2 => fractional.parse::<u64>().ok()? * 10,
        _ => fractional.get(0..3)?.parse::<u64>().ok()?,
    };
    Some(seconds * 1000 + millis)
}

fn parse_matchmaking_started(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "Matchmaking: StartMatchmaking at ";
    if !body.starts_with(marker) {
        return None;
    }
    let regions_part = body.split_once(" in ")?.1.split_once(" for playlists ")?.0;
    let playlist_part = body.split_once(" for playlists ")?.1.split_once(" on ")?.0;
    let regions = regions_part
        .split(',')
        .filter(|region| !region.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let playlist = playlist_part.parse::<i32>().ok()?;
    Some(LogEvent::MatchmakingStarted {
        playlist,
        regions,
        timestamp_ms,
    })
}

fn parse_server_joined(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "DevNet: Welcomed by server (Level: ";
    if !body.starts_with(marker) {
        return None;
    }
    let map = body
        .strip_prefix(marker)?
        .split_once(", Game:")?
        .0
        .to_owned();
    Some(LogEvent::ServerJoined {
        map,
        server: "unknown".to_owned(),
        timestamp_ms,
    })
}

fn parse_match_guid(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let guid = body.strip_prefix("ScriptLog: MatchGUID: ")?;
    Some(LogEvent::MatchGuidSeen {
        guid: guid.trim().to_owned(),
        timestamp_ms,
    })
}

fn parse_player_id(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    let marker = "ScriptLog: Uncached PlatformId for ";
    let player_id = body.strip_prefix(marker)?;
    if !player_id.contains('|') {
        return None;
    }
    Some(LogEvent::PlayerIdSeen {
        player_id: player_id.trim().to_owned(),
        timestamp_ms,
    })
}

fn parse_match_end_without_xp(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    if !body.starts_with("XPProgression: GFxHUD_TA.HandleGameStateChanged ") {
        return None;
    }
    let local_score = parse_i32_between(body, "Current player match score = ", ",")?;
    let duration_seconds = parse_f64_between(body, "with total match time = ", " seconds")?;
    Some(LogEvent::MatchEnded {
        guid: None,
        local_score,
        duration_seconds,
        xp: None,
        timestamp_ms,
    })
}

fn parse_match_end_with_xp(body: &str, timestamp_ms: u64) -> Option<LogEvent> {
    if !body.starts_with("XPProgression: SaveData_TA.HandleRewardDropNotification ") {
        return None;
    }
    let xp = parse_f64_between(body, "Total XP Earned = ", ",")? as i32;
    let guid = parse_str_between(body, "in match with ID = ", " ,")?.to_owned();
    let local_score = parse_i32_between(body, "Current player match score = ", ",")?;
    let duration_seconds = parse_f64_between(body, "with total match time = ", " seconds")?;
    Some(LogEvent::MatchEnded {
        guid: Some(guid),
        local_score,
        duration_seconds,
        xp: Some(xp),
        timestamp_ms,
    })
}

fn parse_i32_between(input: &str, start: &str, end: &str) -> Option<i32> {
    parse_str_between(input, start, end)?.trim().parse().ok()
}

fn parse_f64_between(input: &str, start: &str, end: &str) -> Option<f64> {
    parse_str_between(input, start, end)?.trim().parse().ok()
}

fn parse_str_between<'a>(input: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let after_start = input.split_once(start)?.1;
    Some(after_start.split_once(end)?.0)
}

#[cfg(test)]
mod tests {
    use super::parse_log_line;
    use crate::domain::LogEvent;

    #[test]
    fn parses_matchmaking_started() {
        let line = "[0223.91] Matchmaking: StartMatchmaking at 2026-05-29 23:37:55 in EU9,EU7,EU5,EU3,EU1 for playlists 11 on game server";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchmakingStarted {
                playlist: 11,
                regions: vec![
                    "EU9".to_owned(),
                    "EU7".to_owned(),
                    "EU5".to_owned(),
                    "EU3".to_owned(),
                    "EU1".to_owned()
                ],
                timestamp_ms: 223_910,
            })
        );
    }

    #[test]
    fn parses_server_joined() {
        let line = "[0238.79] DevNet: Welcomed by server (Level: FF_Dusk_P, Game: TAGame.GameInfo_Soccar_TA, GameTags: )";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::ServerJoined {
                map: "FF_Dusk_P".to_owned(),
                server: "unknown".to_owned(),
                timestamp_ms: 238_790,
            })
        );
    }

    #[test]
    fn parses_match_guid() {
        let line = "[0240.99] ScriptLog: MatchGUID: 706DA47C11F15BB7CB1952B6DEE4DFF5";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchGuidSeen {
                guid: "706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned(),
                timestamp_ms: 240_990,
            })
        );
    }

    #[test]
    fn parses_player_id() {
        let line =
            "[0643.74] ScriptLog: Uncached PlatformId for Epic|0123456789abcdef0123456789abcdef|0";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::PlayerIdSeen {
                player_id: "Epic|0123456789abcdef0123456789abcdef|0".to_owned(),
                timestamp_ms: 643_740,
            })
        );
    }

    #[test]
    fn parses_match_end_without_xp() {
        let line = "[0618.72] XPProgression: GFxHUD_TA.HandleGameStateChanged Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.2381 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: None,
                local_score: 323,
                duration_seconds: 302.2381,
                xp: None,
                timestamp_ms: 618_720,
            })
        );
    }

    #[test]
    fn parses_match_end_with_xp_and_guid() {
        let line = "[0619.02] XPProgression: SaveData_TA.HandleRewardDropNotification PsyNetService_RewardDropReceived_TA returned Total XP Earned = 5160.0000, in match with ID = 706DA47C11F15BB7CB1952B6DEE4DFF5 , Current player match score = 323, UniqueId=(Epic|0123456789abcdef0123456789abcdef|0), with total match time = 302.9711 seconds";

        let event = parse_log_line(line);

        assert_eq!(
            event,
            Some(LogEvent::MatchEnded {
                guid: Some("706DA47C11F15BB7CB1952B6DEE4DFF5".to_owned()),
                local_score: 323,
                duration_seconds: 302.9711,
                xp: Some(5160),
                timestamp_ms: 619_020,
            })
        );
    }
}
