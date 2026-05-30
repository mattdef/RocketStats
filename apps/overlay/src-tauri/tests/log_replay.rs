use rocketstats_overlay::domain::{LogEvent, MatchPhase};
use rocketstats_overlay::logs::parser::parse_log_line;
use rocketstats_overlay::match_tracker::MatchTracker;

#[test]
fn replay_fixture_builds_match_session() {
    let fixture = include_str!("fixtures/launch_excerpt.log");
    let mut tracker = MatchTracker::default();
    let mut parsed = 0;

    for line in fixture.lines() {
        if let Some(event) = parse_log_line(line) {
            parsed += 1;
            tracker.apply(event);
        }
    }

    let session = tracker.session();

    assert_eq!(parsed, 6);
    assert_eq!(session.phase, MatchPhase::Ended);
    assert_eq!(session.playlist, Some(11));
    assert_eq!(session.map.as_deref(), Some("FF_Dusk_P"));
    assert_eq!(
        session.guid.as_deref(),
        Some("706DA47C11F15BB7CB1952B6DEE4DFF5")
    );
    assert_eq!(session.detected_players.len(), 2);
    assert_eq!(session.local_score, Some(323));
    assert_eq!(session.xp, Some(5160));

    assert!(parse_log_line("[0001.00] unrelated").is_none());
    assert!(matches!(
        parse_log_line("[0240.99] ScriptLog: MatchGUID: ABCD"),
        Some(LogEvent::MatchGuidSeen { .. })
    ));
}
