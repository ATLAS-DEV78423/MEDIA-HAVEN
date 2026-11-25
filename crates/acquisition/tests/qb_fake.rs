//! Fake WebUI over a loopback TcpListener: no live qB container needed.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use media_acquisition::{QbClient, QbError, TorrentApi};

#[derive(Debug)]
struct Recorded {
    path: String,
    body: String,
    cookie: Option<String>,
}

struct Scripted {
    status: &'static str,
    body: &'static str,
    set_cookie: Option<&'static str>,
}

fn loopback() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let base = format!("http://{}", listener.local_addr().unwrap());
    (listener, base)
}

/// Record each request and answer with the next scripted response.
fn serve(listener: TcpListener, scripts: Vec<Scripted>) -> JoinHandle<Vec<Recorded>> {
    serve_with_close_delay(listener, scripts, Duration::ZERO)
}

/// Same scripted server, but deliberately keeps the accepted sockets open
/// after writing a Content-Length response. qB behaves this way with
/// `Connection: keep-alive`; the client must not wait for the close.
fn serve_with_close_delay(
    listener: TcpListener,
    scripts: Vec<Scripted>,
    close_delay: Duration,
) -> JoinHandle<Vec<Recorded>> {
    thread::spawn(move || {
        let total = scripts.len();
        let mut recorded = Vec::with_capacity(total);
        for (index, script) in scripts.into_iter().enumerate() {
            let (mut socket, _) = listener.accept().expect("client must connect");
            let mut raw = Vec::new();
            let mut buffer = [0u8; 2048];
            let (head_end, content_length) = loop {
                let read = socket.read(&mut buffer).unwrap();
                assert!(read > 0, "the client must send a request");
                raw.extend_from_slice(&buffer[..read]);
                if let Some(end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .map(|value| value.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    break (end, length);
                }
            };
            while raw.len() < head_end + 4 + content_length {
                let read = socket.read(&mut buffer).unwrap();
                assert!(read > 0, "the client must send the whole body");
                raw.extend_from_slice(&buffer[..read]);
            }

            let head = String::from_utf8_lossy(&raw[..head_end]);
            let path = head
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_string();
            let cookie = head
                .lines()
                .find_map(|line| line.strip_prefix("Cookie: "))
                .map(str::to_string);
            let body = String::from_utf8_lossy(&raw[head_end + 4..head_end + 4 + content_length])
                .into_owned();
            recorded.push(Recorded { path, body, cookie });

            let set_cookie = script
                .set_cookie
                .map(|value| format!("Set-Cookie: {value}\r\n"))
                .unwrap_or_default();
            let connection = if close_delay.is_zero() {
                "close"
            } else {
                "keep-alive"
            };
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\n{set_cookie}Connection: {}\r\n\r\n{}",
                script.status,
                script.body.len(),
                connection,
                script.body
            );
            socket.write_all(response.as_bytes()).unwrap();
            if !close_delay.is_zero() && index + 1 == total {
                // Only the final response holds the socket open; earlier
                // connections must be accepted promptly for the next call.
                thread::sleep(close_delay);
            }
        }
        recorded
    })
}

const LOGIN: Scripted = Scripted {
    status: "204 No Content",
    body: "",
    set_cookie: Some("QBT_SID_8080=testsid; Path=/; HttpOnly"),
};
const OK: Scripted = Scripted {
    status: "200 OK",
    body: "",
    set_cookie: None,
};

#[test]
fn add_sends_magnet_and_save_path_and_returns_hash() {
    let (listener, base) = loopback();
    let info = r#"[{"hash":"abc123","magnet_uri":"magnet:?xt=urn:btih:ABC&dn=ep","state":"downloading","progress":0.1}]"#;
    let handle = serve(
        listener,
        vec![
            LOGIN,
            Scripted {
                status: "200 OK",
                body: "[]",
                set_cookie: None,
            },
            OK,
            Scripted {
                status: "200 OK",
                body: info,
                set_cookie: None,
            },
        ],
    );

    let client = QbClient::new(&base, "admin", "secret").unwrap();
    let hash = client
        .add("magnet:?xt=urn:btih:ABC&dn=ep", "/downloads")
        .unwrap();

    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].path, "/api/v2/auth/login");
    assert_eq!(requests[0].body, "username=admin&password=secret");
    assert_eq!(requests[1].path, "/api/v2/torrents/info");
    assert_eq!(requests[2].path, "/api/v2/torrents/add");
    // `&` and `=` in the magnet are encoded so they cannot inject form fields.
    assert!(requests[2]
        .body
        .contains("urls=magnet%3A%3Fxt%3Durn%3Abtih%3AABC%26dn%3Dep"));
    assert!(requests[2].body.contains("savepath=%2Fdownloads"));
    assert_eq!(requests[2].cookie.as_deref(), Some("QBT_SID_8080=testsid"));
    assert_eq!(requests[3].path, "/api/v2/torrents/info");
    assert_eq!(hash, "abc123");
}

#[test]
fn add_treats_a_duplicate_torrent_as_an_idempotent_add() {
    let (listener, base) = loopback();
    let source = "magnet:?xt=urn:btih:ABC&dn=ep";
    let info = r#"[{"hash":"abc123","magnet_uri":"magnet:?xt=urn:btih:ABC&dn=ep","state":"downloading","progress":0.1}]"#;
    let handle = serve(
        listener,
        vec![
            LOGIN,
            Scripted {
                status: "200 OK",
                body: "[]",
                set_cookie: None,
            },
            Scripted {
                status: "409 Conflict",
                body: "Conflict",
                set_cookie: None,
            },
            Scripted {
                status: "200 OK",
                body: info,
                set_cookie: None,
            },
        ],
    );

    let client = QbClient::new(&base, "admin", "secret").unwrap();
    assert_eq!(client.add(source, "/downloads").unwrap(), "abc123");
    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[2].path, "/api/v2/torrents/add");
    assert_eq!(requests[3].path, "/api/v2/torrents/info");
}

#[test]
fn add_resolves_a_bare_dot_torrent_by_the_info_diff() {
    let (listener, base) = loopback();
    let before = r#"[{"hash":"existing","magnet_uri":"magnet:?xt=urn:btih:OLD","state":"downloading","progress":1.0}]"#;
    let after = r#"[{"hash":"existing","magnet_uri":"magnet:?xt=urn:btih:OLD","state":"downloading","progress":1.0},{"hash":"fresh-hash","state":"downloading","progress":0.0}]"#;
    let handle = serve(
        listener,
        vec![
            LOGIN,
            Scripted {
                status: "200 OK",
                body: before,
                set_cookie: None,
            },
            OK,
            Scripted {
                status: "200 OK",
                body: after,
                set_cookie: None,
            },
        ],
    );

    let client = QbClient::new(&base, "admin", "secret").unwrap();
    // A bare .torrent file name is not a magnet URI: the diff is what resolves it.
    let hash = client.add("episode.torrent", "/downloads").unwrap();

    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[2].body.contains("urls=episode.torrent"));
    assert_eq!(hash, "fresh-hash");
}

#[test]
fn remove_passes_delete_files_only_when_asked() {
    let (listener, base) = loopback();
    let handle = serve(listener, vec![LOGIN, OK, OK]);
    let client = QbClient::new(&base, "admin", "secret").unwrap();

    client.remove("hash-one", false).unwrap();
    client.remove("hash-two", true).unwrap();

    let requests = handle.join().unwrap();
    assert_eq!(requests[0].path, "/api/v2/auth/login");
    assert_eq!(requests[1].path, "/api/v2/torrents/delete");
    assert_eq!(requests[2].path, "/api/v2/torrents/delete");
    assert!(requests[1].body.contains("hashes=hash-one"));
    assert!(requests[1].body.contains("deleteFiles=false"));
    assert!(requests[2].body.contains("hashes=hash-two"));
    assert!(requests[2].body.contains("deleteFiles=true"));
}

#[test]
fn authentication_failure_is_a_legible_api_error() {
    // Login itself rejected: no client is produced.
    let (listener, base) = loopback();
    let handle = serve(
        listener,
        vec![Scripted {
            status: "401 Unauthorized",
            body: "Unauthorized",
            set_cookie: None,
        }],
    );
    let error = QbClient::new(&base, "admin", "wrong")
        .err()
        .expect("login must be rejected");
    assert_eq!(error, QbError::Api(401));
    assert!(error.to_string().contains("401"));
    handle.join().unwrap();

    // Session rejected on a later call: also a closed, legible error.
    let (listener2, base2) = loopback();
    let handle = serve(
        listener2,
        vec![
            LOGIN,
            Scripted {
                status: "401 Unauthorized",
                body: "Unauthorized",
                set_cookie: None,
            },
        ],
    );
    let client = QbClient::new(&base2, "admin", "secret").unwrap();
    assert_eq!(client.status("hash").unwrap_err(), QbError::Api(401));
    handle.join().unwrap();
}

#[test]
fn content_length_response_does_not_wait_for_keepalive_close() {
    // Real qB replies with `Connection: keep-alive` and a Content-Length.
    // Reading to EOF costs ~9 s per call; reading the declared length returns
    // as soon as the JSON body is complete.
    let (listener, base) = loopback();
    let handle = serve_with_close_delay(
        listener,
        vec![
            LOGIN,
            Scripted {
                status: "200 OK",
                body: "[]",
                set_cookie: None,
            },
        ],
        Duration::from_millis(1200),
    );

    let started = Instant::now();
    let client = QbClient::new(&base, "admin", "secret").expect("login");
    assert_eq!(client.status("hash").unwrap_err(), QbError::NotFound);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(600),
        "the client waited {elapsed:?} for a keep-alive socket to close"
    );
    handle.join().unwrap();
}
