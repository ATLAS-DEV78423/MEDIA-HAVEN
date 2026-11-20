//! Integration tests: the public surface must be usable headlessly (no mpv,
//! no display) through the injected fake transport.

use std::path::Path;

use media_playback::ipc::FakeIpc;
use media_playback::mpv::{MpvPlayer, Player, Status};
use media_playback::resolve_source;

fn ok(data: &str) -> String {
    format!(r#"{{"error":"success","data":{data}}}"#)
}

#[test]
fn fake_backed_player_runs_the_full_command_set() {
    // loadfile, seek (from open start), set pause false/true, seek, stop,
    // then three get_property replies for status.
    let replies = vec![
        ok("null"),
        ok("null"),
        ok("null"),
        ok("null"),
        ok("null"),
        ok("null"),
        ok("7.5"),
        ok("false"),
        ok("100.0"),
    ];
    let mut player = MpvPlayer::with_transport(FakeIpc::with_replies(replies));

    player.open(Path::new("/media/root-0/a.mkv"), 7.5).unwrap();
    player.play().unwrap();
    player.pause().unwrap();
    player.seek(123.0).unwrap();
    player.stop().unwrap();
    let status: Status = player.status().unwrap();

    assert_eq!(
        status,
        Status {
            time_pos: 7.5,
            pause: false,
            volume: 100.0
        }
    );
    assert_eq!(player.transport().written().len(), 9);
    let all = player.transport().written().join("");
    assert!(all.contains("\"loadfile\""));
    assert!(all.contains("\"set_property\",\"pause\",true"));
    assert!(all.contains("\"seek\",123.0,\"absolute\""));
    assert!(all.contains("\"get_property\",\"volume\""));
}

#[test]
fn resolved_host_path_is_the_only_thing_handed_to_the_player() {
    let base = std::env::temp_dir().join("mediahaven-b2-integration");
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("movies");
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("clip.mkv");
    std::fs::write(&file, b"x").unwrap();

    let map = std::collections::BTreeMap::from([("/media/root-0".to_string(), root)]);
    let host_path = resolve_source("/media/root-0/clip.mkv", &map).unwrap();
    assert_eq!(host_path, std::fs::canonicalize(&file).unwrap());

    // The host path is passed only to Player::open (host-internal); no DTO here.
    let mut player = MpvPlayer::with_transport(FakeIpc::with_replies([ok("null")]));
    player.open(&host_path, 0.0).unwrap();
    // The path is JSON-escaped on the wire; compare against its serialized form.
    let expected = serde_json::to_string(&host_path.to_string_lossy()).unwrap();
    assert!(player.transport().written()[0].contains(&expected));

    let _ = std::fs::remove_dir_all(&base);
}
