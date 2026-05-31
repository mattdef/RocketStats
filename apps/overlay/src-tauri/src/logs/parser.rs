use crate::domain::{InitEvent, LogEvent};

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

/// Parses initialization lines from the Launch.log header.
///
/// These lines appear at the very start of the log and contain game metadata
/// (version, feature set) and the Epic launcher's command-line arguments.
pub fn parse_init_line(line: &str) -> Option<InitEvent> {
    parse_build_version(line)
        .or_else(|| parse_feature_set(line))
        .or_else(|| parse_command_line_identity(line))
}

fn parse_build_version(line: &str) -> Option<InitEvent> {
    let body = line.strip_prefix('[')?.split_once(']')?.1.trim_start();
    let version = body.strip_prefix("LogInit: Build: ")?;
    let version = version.trim();
    if version.is_empty() {
        return None;
    }
    Some(InitEvent::BuildVersion(version.to_owned()))
}

fn parse_feature_set(line: &str) -> Option<InitEvent> {
    let body = line.strip_prefix('[')?.split_once(']')?.1.trim_start();
    let feature_set = body.strip_prefix("LogInit: FeatureSet: ")?;
    let feature_set = feature_set.trim();
    if feature_set.is_empty() {
        return None;
    }
    Some(InitEvent::FeatureSet(feature_set.to_owned()))
}

fn parse_command_line_identity(line: &str) -> Option<InitEvent> {
    let body = line.strip_prefix('[')?.split_once(']')?.1.trim_start();
    let cmdline = body.strip_prefix("Command line: ")?;
    let epic_user_id = extract_flag_value(cmdline, "-epicuserid=")?;
    if epic_user_id.is_empty() {
        return None;
    }
    let epic_user_name = extract_flag_value(cmdline, "-epicusername=").map(str::to_owned);
    Some(InitEvent::EpicIdentity {
        epic_user_id: epic_user_id.to_owned(),
        epic_user_name,
    })
}

fn extract_flag_value<'a>(cmdline: &'a str, flag: &str) -> Option<&'a str> {
    let start = cmdline.find(flag)?;
    let after_flag = &cmdline[start + flag.len()..];
    // Value is either the next token (until whitespace) or until end of string
    let end = after_flag
        .find(char::is_whitespace)
        .unwrap_or(after_flag.len());
    let value = &after_flag[..end];
    if value.is_empty() {
        return None;
    }
    Some(value)
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
    use super::{parse_init_line, parse_log_line};
    use crate::domain::{InitEvent, LogEvent};

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

    #[test]
    fn parses_build_version() {
        let line = "[0000.00] LogInit: Build: 260506.26700.517210";
        let event = parse_init_line(line);
        assert_eq!(
            event,
            Some(InitEvent::BuildVersion("260506.26700.517210".to_owned()))
        );
    }

    #[test]
    fn parses_feature_set() {
        let line = "[0000.00] LogInit: FeatureSet: PrimeUpdate58_1";
        let event = parse_init_line(line);
        assert_eq!(
            event,
            Some(InitEvent::FeatureSet("PrimeUpdate58_1".to_owned()))
        );
    }

    #[test]
    fn parses_command_line_identity() {
        let line = "[0000.00] Command line: -AUTH_PASSWORD=5139003f31b04a6ba73e914a8860125a -epicuserid=7efc351e447043c4be4447da51b790e4 -epicusername=TestPlayer";
        let event = parse_init_line(line);
        assert_eq!(
            event,
            Some(InitEvent::EpicIdentity {
                epic_user_id: "7efc351e447043c4be4447da51b790e4".to_owned(),
                epic_user_name: Some("TestPlayer".to_owned()),
            })
        );
    }

    #[test]
    fn parses_command_line_without_username() {
        let line = "[0000.00] Command line: -epicuserid=7efc351e447043c4be4447da51b790e4";
        let event = parse_init_line(line);
        assert_eq!(
            event,
            Some(InitEvent::EpicIdentity {
                epic_user_id: "7efc351e447043c4be4447da51b790e4".to_owned(),
                epic_user_name: None,
            })
        );
    }

    #[test]
    fn init_line_returns_none_for_match_events() {
        assert_eq!(
            parse_init_line("[0223.91] Matchmaking: StartMatchmaking"),
            None
        );
    }

    #[test]
    fn init_line_returns_none_for_empty_body() {
        assert_eq!(parse_init_line("[0000.00] LogInit: Build: "), None);
        assert_eq!(parse_init_line("[0000.00] Command line: "), None);
    }
}
