//! A5 acceptance: the command list is exactly the renderer route list, and the
//! wire primitives (signature, scan shape) match the API.

use mediahaven_contract::{sign, Command, ScanOutcome, COMMANDS};
use std::str::FromStr;

/// The 24 renderer routes in `crates/core/src/routes.rs` (probes excluded).
const EXPECTED_RENDERER_ROUTES: &[&str] = &[
    "library_home",
    "library_movies",
    "library_shows",
    "library_detail",
    "library_episodes",
    "library_search",
    "library_roots",
    "library_root_add",
    "library_root_remove",
    "library_scan",
    "library_watchlist",
    "watchlist_add",
    "watchlist_remove",
    "watchlist_reorder",
    "queue_list",
    "queue_add",
    "queue_remove",
    "queue_reorder",
    "downloads_list",
    "settings_get",
    "settings_set",
    "playback_start",
    "playback_stop",
    "playback_checkpoint",
];

#[test]
fn commands_equal_renderer_routes_with_no_passthrough() {
    assert_eq!(
        COMMANDS, EXPECTED_RENDERER_ROUTES,
        "the named command list must equal the renderer route list exactly"
    );
    assert_eq!(COMMANDS.len(), 24);

    for name in COMMANDS {
        assert!(!matches!(*name, "http_request" | "api_request"));
        assert!(!name.starts_with("docker_"), "forbidden command: {name}");
        assert_ne!(*name, "healthz");
        assert_ne!(*name, "readyz");

        // The typed enum and the published list must not drift apart.
        let command = Command::from_str(name).unwrap_or_else(|_| panic!("unknown command {name}"));
        assert_eq!(command.as_str(), *name);
    }

    let mut seen = std::collections::BTreeSet::new();
    for name in COMMANDS {
        assert!(seen.insert(*name), "duplicate command: {name}");
    }
}

#[test]
fn forbidden_command_names_are_rejected_by_the_typed_enum() {
    for name in ["http_request", "api_request", "docker_compose", "raw"] {
        assert!(
            Command::from_str(name).is_err(),
            "{name} must not be a command"
        );
    }
}

#[test]
fn sign_matches_fixed_vector() {
    // Python: hmac.new(b"test-token",
    //   b"1700000000\nPOST\n/library_home\n" +
    //   hashlib.sha256(b"{}").hexdigest().encode(), sha256).hexdigest()
    let signature = sign(b"test-token", "1700000000", "POST", "/library_home", b"{}");
    assert_eq!(
        signature,
        "19cdb8ef1aeabb5409495d8e031ef9602a2b68f86509137c38d16ffb9ec8ef47"
    );
}

#[test]
fn scan_outcome_matches_the_backend_inline_json_shape() {
    let value: ScanOutcome = serde_json::from_str(
        r#"{
            "scanned": 2,
            "cancelled": false,
            "errors": ["bad: nope"],
            "duplicates": [{ "hash": "abc", "paths": ["a", "b"] }]
        }"#,
    )
    .expect("library_scan response must deserialize into ScanOutcome");
    assert_eq!(value.scanned, 2);
    assert!(!value.cancelled);
    assert_eq!(value.errors, ["bad: nope"]);
    assert_eq!(value.duplicates[0].paths, ["a", "b"]);
}
