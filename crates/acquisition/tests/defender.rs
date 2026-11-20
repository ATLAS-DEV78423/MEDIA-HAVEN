//! Task 2 evidence: the Defender gate and the combined probe/Defender gate.
//! All scans are faked — no live Defender call and no real media file is used.

use media_acquisition::{
    both_gates_clean, defender_scan_with, refusal_reason, DefenderState, Scanner,
};

struct FakeScanner {
    result: DefenderState,
}

impl Scanner for FakeScanner {
    fn scan(&self, _host_path: &str) -> DefenderState {
        self.result
    }
}

#[test]
fn fake_reports_threat_clean_and_unavailable() {
    for result in [
        DefenderState::Threat,
        DefenderState::Clean,
        DefenderState::Unavailable,
    ] {
        let scanner = FakeScanner { result };
        assert_eq!(defender_scan_with(&scanner, "C:\\synthetic.mkv"), result);
    }
}

#[test]
fn a_threat_is_never_reported_playable() {
    // Absence assertion: no Threat/Unavailable/NotScanned value passes the
    // combined gate, even when the probe gate reports clean.
    for defender in [
        DefenderState::Threat,
        DefenderState::Unavailable,
        DefenderState::NotScanned,
    ] {
        assert!(
            !both_gates_clean(true, defender),
            "defender {defender:?} must never be playable"
        );
        assert!(
            refusal_reason(true, defender).is_some(),
            "defender {defender:?} must carry a refusal reason"
        );
    }
}

#[test]
fn combined_gate_blocks_probe_clean_but_defender_threat() {
    let scanner = FakeScanner {
        result: DefenderState::Threat,
    };
    let defender = defender_scan_with(&scanner, "C:\\synthetic.mkv");
    assert!(!both_gates_clean(true, defender));
    assert!(refusal_reason(true, defender).is_some());
}

#[test]
fn combined_gate_allows_only_both_clean() {
    let scanner = FakeScanner {
        result: DefenderState::Clean,
    };
    let defender = defender_scan_with(&scanner, "C:\\synthetic.mkv");
    assert!(both_gates_clean(true, defender));
    assert_eq!(refusal_reason(true, defender), None);

    // The Defender half alone is not enough: a suspicious probe still blocks.
    assert!(!both_gates_clean(false, defender));
    assert!(refusal_reason(false, defender).is_some());
}

#[test]
fn refusal_reason_is_display_only_and_path_free() {
    for defender in [
        DefenderState::Threat,
        DefenderState::Unavailable,
        DefenderState::NotScanned,
    ] {
        let reason = refusal_reason(true, defender).expect("blocked");
        assert!(!reason.contains("C:"));
        assert!(!reason.contains("synthetic"));
        assert!(!reason.contains('\\'));
    }
}
