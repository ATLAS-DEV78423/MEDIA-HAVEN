//! Hand-written qBittorrent WebUI client over `std::net::TcpStream` (ADR-0018).
//!
//! The WebUI is reached only on the container `acquire` network; it is never
//! published to the LAN or exposed to the browser. Authentication is mandatory:
//! the client logs in with the host-injected secret, keeps the returned session
//! cookie, and sends it on every API call. No credential is logged or returned.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::Value;

/// Container-network address of the qBittorrent WebUI.
pub const DEFAULT_BASE: &str = "http://qbittorrent:8080";
/// WebUI username configured by the container init (ADR-0018).
pub const DEFAULT_USERNAME: &str = "admin";

/// A path-free torrent status. `state` is qBittorrent's own state string.
#[derive(Debug, Clone, PartialEq)]
pub struct TorrentStatus {
    pub hash: String,
    pub state: String,
    pub progress: f64,
}

/// Closed, legible error set. No WebUI response text is ever embedded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QbError {
    #[error("qBittorrent API returned HTTP {0}")]
    Api(u16),
    #[error("qBittorrent WebUI is unreachable")]
    Transport,
    #[error("qBittorrent response was malformed")]
    MalformedResponse,
    #[error("qBittorrent login returned no session cookie")]
    NoSession,
    #[error("invalid qBittorrent base URL")]
    InvalidBase,
    #[error("qBittorrent credentials are not configured")]
    MissingCredentials,
    #[error("added torrent was not found in qBittorrent")]
    NotFound,
}

/// The port Task 4/5/6 drive. Implementations are fakeable for tests.
pub trait TorrentApi {
    fn add(&self, source: &str, save_path: &str) -> Result<String, QbError>;
    fn status(&self, hash: &str) -> Result<TorrentStatus, QbError>;
    fn files(&self, hash: &str) -> Result<Vec<String>, QbError>;
    fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError>;
}

struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Only plaintext loopback/container HTTP is supported: no TLS dependency and
/// no proxy. A credential in the URL is refused so it cannot leak into a log.
fn parse_address(base: &str) -> Result<(String, u16), QbError> {
    let authority = base
        .strip_prefix("http://")
        .ok_or(QbError::InvalidBase)?
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
    if authority.is_empty() || authority.contains('@') {
        return Err(QbError::InvalidBase);
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| QbError::InvalidBase)?;
        Ok((host.to_string(), port))
    } else {
        Ok((authority, 80))
    }
}

/// `application/x-www-form-urlencoded` escaping. Magnet links contain `&` and
/// `=`, which must be encoded or they would inject extra form parameters.
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn exchange(
    address: &(String, u16),
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<&str>,
) -> Result<HttpResponse, QbError> {
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\n",
        address.0, address.1
    );
    if let Some(cookie) = cookie {
        request.push_str(&format!("Cookie: {cookie}\r\n"));
    }
    if let Some(body) = body {
        request.push_str("Content-Type: application/x-www-form-urlencoded\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }

    let mut stream =
        TcpStream::connect((address.0.as_str(), address.1)).map_err(|_| QbError::Transport)?;
    let timeout = Some(Duration::from_secs(10));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    stream
        .write_all(request.as_bytes())
        .map_err(|_| QbError::Transport)?;

    // Read only the response head first. qB answers with `Connection:
    // keep-alive` and a Content-Length; `read_to_end` would wait for qB to
    // close the idle socket (~9 s here). Honour Content-Length and return as
    // soon as the declared body is complete.
    let mut head_bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !head_bytes.ends_with(b"\r\n\r\n") {
        let read = stream.read(&mut byte).map_err(|_| QbError::Transport)?;
        if read == 0 {
            break;
        }
        head_bytes.push(byte[0]);
    }
    if !head_bytes.ends_with(b"\r\n\r\n") {
        return Err(QbError::MalformedResponse);
    }
    let split = head_bytes.len() - 4;
    let head = String::from_utf8_lossy(&head_bytes[..split]);
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .ok_or(QbError::MalformedResponse)?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect();
    let content_length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok());
    let body = if let Some(length) = content_length {
        let mut body = vec![0u8; length];
        stream
            .read_exact(&mut body)
            .map_err(|_| QbError::Transport)?;
        body
    } else {
        let mut body = Vec::new();
        stream
            .read_to_end(&mut body)
            .map_err(|_| QbError::Transport)?;
        body
    };
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

/// An authenticated WebUI session. `cookie` is the full `Name=Value` pair
/// returned by login; qB 5.x names it `QBT_SID_8080`, older builds used `SID`.
pub struct QbClient {
    base: String,
    cookie: String,
    username: String,
    password: String,
}

impl QbClient {
    /// Log in immediately. Never succeeds against an unauthenticated WebUI:
    /// a missing session cookie is an error, not a licence to proceed.
    pub fn new(
        base: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Self, QbError> {
        let mut client = Self {
            base: base.into(),
            cookie: String::new(),
            username: username.into(),
            password: password.into(),
        };
        client.login()?;
        Ok(client)
    }

    /// Production constructor: base URL and host-injected WebUI secret.
    pub fn from_env() -> Result<Self, QbError> {
        let base = std::env::var("MEDIAHAVEN_QB_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
        let password =
            std::env::var("MEDIAHAVEN_QB_TOKEN").map_err(|_| QbError::MissingCredentials)?;
        Self::new(base, DEFAULT_USERNAME, password)
    }

    fn login(&mut self) -> Result<(), QbError> {
        let address = parse_address(&self.base)?;
        let body = form(&[("username", &self.username), ("password", &self.password)]);
        let response = exchange(&address, "POST", "/api/v2/auth/login", None, Some(&body))?;
        if !(200..300).contains(&response.status) {
            return Err(QbError::Api(response.status));
        }
        let set_cookie = response.header("set-cookie").ok_or(QbError::NoSession)?;
        let pair = set_cookie.split(';').next().unwrap_or("").trim();
        if pair.is_empty() || !pair.contains('=') {
            return Err(QbError::NoSession);
        }
        self.cookie = pair.to_string();
        Ok(())
    }

    fn api(&self, method: &str, path: &str, body: Option<&str>) -> Result<HttpResponse, QbError> {
        let address = parse_address(&self.base)?;
        let response = exchange(&address, method, path, Some(&self.cookie), body)?;
        if !(200..300).contains(&response.status) {
            return Err(QbError::Api(response.status));
        }
        Ok(response)
    }

    fn info(&self) -> Result<Vec<Value>, QbError> {
        let response = self.api("GET", "/api/v2/torrents/info", None)?;
        serde_json::from_slice(&response.body).map_err(|_| QbError::MalformedResponse)
    }

    fn hashes(&self) -> Result<HashSet<String>, QbError> {
        Ok(self
            .info()?
            .iter()
            .filter_map(|item| item.get("hash").and_then(Value::as_str).map(str::to_string))
            .collect())
    }
}

impl TorrentApi for QbClient {
    fn add(&self, source: &str, save_path: &str) -> Result<String, QbError> {
        // qB's add endpoint returns no hash. Diff the info list before/after so
        // BOTH magnets and bare .torrent sources resolve by the newly-created
        // hash, instead of relying on fragile exact magnet-URI equality.
        let before = self.hashes()?;
        let body = form(&[("urls", source), ("savepath", save_path)]);
        match self.api("POST", "/api/v2/torrents/add", Some(&body)) {
            Ok(_) => {}
            // qB returns 409 for a torrent it already holds. That is a
            // successful idempotent add for our purposes: resolve the row
            // from the info list below instead of reporting a backend failure.
            Err(QbError::Api(409)) => {}
            Err(error) => return Err(error),
        }
        let after = self.hashes()?;
        let added: Vec<String> = after.difference(&before).cloned().collect();
        if added.len() == 1 {
            return Ok(added[0].clone());
        }

        // Fallback for a source already present (no diff) or several new rows:
        // prefer an exact magnet URI, then a lone result.
        let info = self.info()?;
        info.iter()
            .find(|item| item.get("magnet_uri").and_then(Value::as_str) == Some(source))
            .or_else(|| if info.len() == 1 { info.first() } else { None })
            .and_then(|item| item.get("hash").and_then(Value::as_str))
            .map(str::to_string)
            .ok_or(QbError::NotFound)
    }

    fn status(&self, hash: &str) -> Result<TorrentStatus, QbError> {
        let path = format!("/api/v2/torrents/info?hashes={}", form_encode(hash));
        let response = self.api("GET", &path, None)?;
        let value: Value =
            serde_json::from_slice(&response.body).map_err(|_| QbError::MalformedResponse)?;
        let item = value
            .as_array()
            .and_then(|list| list.first())
            .ok_or(QbError::NotFound)?;
        Ok(TorrentStatus {
            hash: item
                .get("hash")
                .and_then(Value::as_str)
                .unwrap_or(hash)
                .to_string(),
            state: item
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            progress: item.get("progress").and_then(Value::as_f64).unwrap_or(0.0),
        })
    }

    fn files(&self, hash: &str) -> Result<Vec<String>, QbError> {
        let path = format!("/api/v2/torrents/files?hash={}", form_encode(hash));
        let response = self.api("GET", &path, None)?;
        let value: Value =
            serde_json::from_slice(&response.body).map_err(|_| QbError::MalformedResponse)?;
        let list = value.as_array().ok_or(QbError::MalformedResponse)?;
        Ok(list
            .iter()
            .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
            .collect())
    }

    fn remove(&self, hash: &str, delete_files: bool) -> Result<(), QbError> {
        let body = form(&[
            ("hashes", hash),
            ("deleteFiles", if delete_files { "true" } else { "false" }),
        ]);
        self.api("POST", "/api/v2/torrents/delete", Some(&body))?;
        Ok(())
    }
}
