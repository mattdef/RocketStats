use rocketstats_overlay::domain::{LogEvent, MatchPhase, MatchSession};
use rocketstats_overlay::logs::parser::parse_log_line;
use rocketstats_overlay::logs::watcher::{LogWatcherConfig, watch_log};
use rocketstats_overlay::match_tracker::MatchTracker;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const FIXTURE: &str = include_str!("fixtures/launch_excerpt.log");
const WATCHER_SHUTDOWN_LINE: &str = "[9999.99] ScriptLog: MatchGUID: WATCHER_SHUTDOWN";
const ROTATED_GUID_LINE: &str = "[0777.77] ScriptLog: MatchGUID: ROTATED_GUID";

#[test]
fn replay_fixture_builds_match_session() {
    let mut tracker = MatchTracker::default();
    let mut parsed = 0;

    for line in FIXTURE.lines() {
        if let Some(event) = parse_log_line(line) {
            parsed += 1;
            tracker.apply(event);
        }
    }

    assert_fixture_session(tracker.session(), parsed);

    assert!(parse_log_line("[0001.00] unrelated").is_none());
    assert!(matches!(
        parse_log_line("[0240.99] ScriptLog: MatchGUID: ABCD"),
        Some(LogEvent::MatchGuidSeen { .. })
    ));
}

#[tokio::test]
async fn watcher_tails_appended_fixture_lines_into_match_session() {
    let temp_dir = tempdir().expect("should create temp directory");
    let log_path = temp_dir.path().join("Launch.log");
    let fixture_lines = fixture_lines();

    write_lines(&log_path, &fixture_lines[..3]);

    let (watcher, mut receiver) = spawn_watcher(log_path.clone());
    let mut events = collect_events(&mut receiver, 3).await;

    append_lines(&log_path, &fixture_lines[3..]);
    events.extend(collect_events(&mut receiver, 3).await);

    let mut tracker = MatchTracker::default();
    let mut parsed = 0;
    for event in events {
        parsed += 1;
        tracker.apply(event);
    }

    assert_fixture_session(tracker.session(), parsed);
    stop_watcher(&log_path, receiver, watcher).await;
}

#[tokio::test]
async fn watcher_resets_offset_after_log_rotation() {
    let temp_dir = tempdir().expect("should create temp directory");
    let log_path = temp_dir.path().join("Launch.log");

    write_lines(&log_path, &fixture_lines());

    let (watcher, mut receiver) = spawn_watcher(log_path.clone());
    let events = collect_events(&mut receiver, 6).await;

    let mut tracker = MatchTracker::default();
    let mut parsed = 0;
    for event in events {
        parsed += 1;
        tracker.apply(event);
    }

    assert_fixture_session(tracker.session(), parsed);

    write_lines(&log_path, &[ROTATED_GUID_LINE]);

    let rotated_event = timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("timed out waiting for rotated log event")
        .expect("watcher channel closed before rotated log event arrived");

    match rotated_event {
        LogEvent::MatchGuidSeen { guid, timestamp_ms } => {
            assert_eq!(guid, "ROTATED_GUID");
            assert_eq!(timestamp_ms, 777_770);
        }
        other => panic!("expected rotated match guid event, got {other:?}"),
    }

    stop_watcher(&log_path, receiver, watcher).await;
}

fn assert_fixture_session(session: &MatchSession, parsed: usize) {
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
}

fn fixture_lines() -> Vec<&'static str> {
    FIXTURE.lines().collect()
}

fn write_lines(path: &Path, lines: &[&str]) {
    fs::write(path, render_lines(lines)).expect("should write log lines");
}

fn append_lines(path: &Path, lines: &[&str]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("should open log for append");
    file.write_all(render_lines(lines).as_bytes())
        .expect("should append log lines");
    file.flush().expect("should flush appended log lines");
}

fn render_lines(lines: &[&str]) -> String {
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    content
}

fn spawn_watcher(
    path: PathBuf,
) -> (
    JoinHandle<rocketstats_overlay::error::Result<()>>,
    mpsc::Receiver<LogEvent>,
) {
    let mut config = LogWatcherConfig::new(path);
    config.poll_interval = Duration::from_millis(10);

    let (sender, receiver) = mpsc::channel(64);
    let watcher = tokio::spawn(async move { watch_log(config, sender).await });

    (watcher, receiver)
}

async fn collect_events(receiver: &mut mpsc::Receiver<LogEvent>, expected: usize) -> Vec<LogEvent> {
    let mut events = Vec::with_capacity(expected);

    while events.len() < expected {
        let event = timeout(Duration::from_secs(5), receiver.recv())
            .await
            .expect("timed out waiting for log event")
            .expect("watcher channel closed before expected log events arrived");
        events.push(event);
    }

    events
}

async fn stop_watcher(
    path: &Path,
    receiver: mpsc::Receiver<LogEvent>,
    watcher: JoinHandle<rocketstats_overlay::error::Result<()>>,
) {
    drop(receiver);
    append_lines(path, &[WATCHER_SHUTDOWN_LINE]);

    let join_result = timeout(Duration::from_secs(1), watcher)
        .await
        .expect("watcher did not stop after receiver closed");
    let watcher_result = join_result.expect("watcher task panicked");
    watcher_result.expect("watcher returned an error while stopping");
}
