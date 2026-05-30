use std::collections::HashSet;
use std::sync::Arc;

use rocketstats_rlapi::{
    Platform, PlayerId, PsyNetConfig, build_psynet_message, decode_build_id, generate_psy_sig,
    parse_psynet_message,
};

#[test]
fn decodes_psynet_build_id_like_upstream_rlapi() {
    assert_eq!(decode_build_id("260316.80791.512269"), 1_210_528_741);
    assert_eq!(decode_build_id("260420.86069.515605"), 1_273_328_361);
    assert_eq!(decode_build_id("260506.26700.517210"), -1_652_286_008);
}

#[test]
fn default_config_uses_current_upstream_constants() {
    let config = PsyNetConfig::default();

    assert_eq!(config.base_url.as_str(), "https://api.rlpp.psynet.gg/rpc");
    assert_eq!(config.game_version, "260506.26700.517210");
    assert_eq!(config.feature_set, "PrimeUpdate58_1");
    assert_eq!(config.environment, "Prod");
    assert_eq!(config.psy_build_id(), "-1652286008");
}

#[test]
fn signs_body_with_psynet_hmac_shape() {
    assert_eq!(
        generate_psy_sig(b"{}"),
        "fMPMoP62q5HjQXDLS6U5vH0oiWh2Y5Ji8nJDVOPJH9U="
    );
    assert_eq!(
        generate_psy_sig(br#"{"test":"data"}"#),
        "EBkRNl96hSCXgKkK6FnpCd1A+0abUyvJ8liCncvxsNs="
    );
}

#[test]
fn formats_and_parses_player_ids() {
    let id = PlayerId::new(Platform::Epic, "account-123");

    assert_eq!(id.as_str(), "Epic|account-123|0");
    assert_eq!(id.platform(), Platform::Epic);
    assert_eq!(id.account_id(), "account-123");
    assert!("bad-player-id".parse::<PlayerId>().is_err());
}

#[test]
fn builds_and_parses_psynet_messages() {
    let message = build_psynet_message(
        [
            ("PsyService", "Skills/GetPlayerSkill v1"),
            ("PsyRequestID", "PsyNetMessage_X_123"),
        ],
        Some(&serde_json::json!({"PlayerID":"Epic|account-123|0"})),
    )
    .expect("message builds");

    assert!(message.contains("PsyService: Skills/GetPlayerSkill v1\r\n"));
    assert!(message.contains("PsyRequestID: PsyNetMessage_X_123\r\n"));
    assert!(message.contains("PsySig: "));
    assert!(message.ends_with(r#"{"PlayerID":"Epic|account-123|0"}"#));

    let parsed = parse_psynet_message(
        "PsyTime: 1\r\nPsySig: test\r\nPsyResponseID: PsyNetMessage_X_123\r\n\r\n{\"Result\":{\"ok\":true}}",
    )
    .expect("message parses");

    assert_eq!(parsed.response_id.as_deref(), Some("PsyNetMessage_X_123"));
    assert_eq!(parsed.result.unwrap(), serde_json::json!({"ok": true}));
    assert!(parsed.error.is_none());
}

#[test]
fn request_ids_are_unique_under_concurrency() {
    let counter = Arc::new(rocketstats_rlapi::RequestIdCounter::default());
    assert_eq!(counter.next_id(), "PsyNetMessage_X_0");
    assert_eq!(counter.next_id(), "PsyNetMessage_X_1");

    let mut handles = Vec::new();
    for _ in 0..50 {
        let counter = Arc::clone(&counter);
        handles.push(std::thread::spawn(move || counter.next_id()));
    }

    let mut ids = HashSet::new();
    for handle in handles {
        assert!(ids.insert(handle.join().expect("thread joins")));
    }

    assert_eq!(ids.len(), 50);
}
