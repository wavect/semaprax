//! Minimal bounded loopback HTTP/1.1 client for endpoint probing (std only).
//! Loopback-only by construction: the target host must be `127.0.0.1`,
//! `localhost` or `[::1]` and every resolved address must be loopback. The
//! credential, when present, is held only for the `Authorization` header and
//! never appears in a reply, error or record.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

fn err(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Parsed loopback origin plus an optional path prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub base: String,
}

impl Target {
    /// `http://127.0.0.1:PORT[/prefix]` only; anything else is `SPX-HPL002`.
    pub fn parse(url: &str) -> HarnessResult<Self> {
        let rest = url.strip_prefix("http://").ok_or_else(|| {
            err(
                "SPX-HPL002",
                "endpoint url must be http:// on a loopback host",
            )
        })?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], rest[i..].trim_end_matches('/')),
            None => (rest, ""),
        };
        if authority.contains('@') || path.contains(['?', '#']) || path.contains("..") {
            return Err(err(
                "SPX-HPL002",
                "endpoint url must not carry userinfo, query or dot segments",
            ));
        }
        let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
            let (h, p) = v6
                .split_once("]:")
                .ok_or_else(|| err("SPX-HPL002", "bad ipv6 authority"))?;
            (format!("[{h}]"), p)
        } else {
            let (h, p) = authority
                .rsplit_once(':')
                .ok_or_else(|| err("SPX-HPL002", "endpoint url needs an explicit port"))?;
            (h.to_string(), p)
        };
        if !matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]") {
            return Err(err(
                "SPX-HPL002",
                format!("endpoint host `{host}` is not loopback"),
            ));
        }
        let port: u16 = port
            .parse()
            .map_err(|_| err("SPX-HPL002", "bad endpoint port"))?;
        Ok(Self {
            host,
            port,
            base: path.to_string(),
        })
    }

    pub fn origin(&self) -> String {
        format!("http://{}:{}{}", self.host, self.port, self.base)
    }

    fn addr(&self) -> HarnessResult<SocketAddr> {
        let host = self.host.trim_matches(['[', ']']);
        let addrs = (host, self.port)
            .to_socket_addrs()
            .map_err(|e| err("SPX-HPL003", format!("cannot resolve loopback host: {e}")))?;
        addrs
            .filter(|a| a.ip().is_loopback())
            .next()
            .ok_or_else(|| err("SPX-HPL002", "host does not resolve to a loopback address"))
    }
}

/// One complete (non-streaming) reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpReply {
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
    /// Short single-line body excerpt for evidence (never headers).
    pub fn excerpt(&self) -> String {
        let s = String::from_utf8_lossy(&self.body);
        let one: String = s.chars().filter(|c| !c.is_control()).take(160).collect();
        one
    }
}

/// One server-sent event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Result of a bounded streaming read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamReply {
    pub status: u16,
    pub content_type: String,
    pub events: Vec<SseEvent>,
    /// Non-SSE body when the status is not 2xx or the type is not event-stream.
    pub body: Vec<u8>,
    /// The client closed the connection after `cancel_after_events` events.
    pub cancelled: bool,
    /// The server ended the stream before any cancel/limit.
    pub completed: bool,
}

#[derive(Clone, Debug)]
pub struct ProbeClient {
    pub target: Target,
    credential: Option<String>,
    pub timeout: Duration,
    pub max_body: usize,
}

impl ProbeClient {
    pub fn new(target: Target, credential: Option<String>) -> Self {
        Self {
            target,
            credential,
            timeout: Duration::from_secs(60),
            max_body: 4 * 1024 * 1024,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn get(&self, path: &str) -> HarnessResult<HttpReply> {
        self.send("GET", path, &[], None)
    }

    pub fn post_json(&self, path: &str, body: &serde_json::Value) -> HarnessResult<HttpReply> {
        self.send("POST", path, &[], Some(body.to_string().as_bytes()))
    }

    /// Buffered request; `headers` are extra non-secret headers.
    pub fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> HarnessResult<HttpReply> {
        let mut conn = self.open(method, path, headers, body)?;
        let deadline = Instant::now() + self.timeout;
        let (status, ctype, chunked, len, mut raw) = read_head(&mut conn.stream, deadline)?;
        let mut dechunk = Dechunk::default();
        let mut out = Vec::new();
        if chunked {
            out.extend(dechunk.push(&raw));
        } else {
            out.extend_from_slice(&raw);
        }
        raw.clear();
        let mut buf = [0u8; 8192];
        loop {
            if out.len() > self.max_body {
                return Err(err("SPX-HPL003", "reply exceeds the probe body bound"));
            }
            if (chunked && dechunk.done) || len.is_some_and(|l| out.len() >= l) {
                break;
            }
            if Instant::now() > deadline {
                return Err(err("SPX-HPL003", "probe timed out reading the reply"));
            }
            match conn.stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) if chunked => out.extend(dechunk.push(&buf[..n])),
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if is_timeout(&e) => continue,
                Err(e) => return Err(err("SPX-HPL003", format!("read failed: {e}"))),
            }
        }
        Ok(HttpReply {
            status,
            content_type: ctype,
            body: out,
        })
    }

    /// POST a streaming request and read SSE events; closes the connection
    /// (cancellation) once `cancel_after_events` events have arrived, and stops
    /// reading at `max_events` regardless.
    pub fn post_stream(
        &self,
        path: &str,
        body: &serde_json::Value,
        cancel_after_events: Option<usize>,
        max_events: usize,
    ) -> HarnessResult<StreamReply> {
        let mut conn = self.open("POST", path, &[], Some(body.to_string().as_bytes()))?;
        let deadline = Instant::now() + self.timeout;
        let (status, ctype, chunked, _len, raw) = read_head(&mut conn.stream, deadline)?;
        let sse = ctype.starts_with("text/event-stream");
        let mut dechunk = Dechunk::default();
        let mut text = Vec::new();
        let mut reply = StreamReply {
            status,
            content_type: ctype,
            events: Vec::new(),
            body: Vec::new(),
            cancelled: false,
            completed: false,
        };
        let mut pending = raw;
        let mut buf = [0u8; 4096];
        loop {
            let bytes = if chunked {
                dechunk.push(&pending)
            } else {
                pending.clone()
            };
            text.extend(bytes);
            pending.clear();
            if sse && (200..300).contains(&status) {
                drain_events(&mut text, &mut reply.events);
            } else if text.len() > self.max_body {
                break;
            }
            if let Some(n) = cancel_after_events {
                if reply.events.len() >= n {
                    reply.cancelled = true;
                    break;
                }
            }
            if reply.events.len() >= max_events {
                break;
            }
            if chunked && dechunk.done {
                reply.completed = true;
                break;
            }
            if Instant::now() > deadline {
                return Err(err("SPX-HPL003", "probe timed out reading the stream"));
            }
            match conn.stream.read(&mut buf) {
                Ok(0) => {
                    reply.completed = true;
                    break;
                }
                Ok(n) => pending.extend_from_slice(&buf[..n]),
                Err(e) if is_timeout(&e) => continue,
                Err(e) => return Err(err("SPX-HPL003", format!("read failed: {e}"))),
            }
        }
        if !(sse && (200..300).contains(&status)) {
            reply.body = text;
        } else if reply.completed {
            // A final unterminated event is still an event.
            text.extend_from_slice(b"\n\n");
            drain_events(&mut text, &mut reply.events);
        }
        // Dropping `conn` closes the socket; for a cancel that is the signal.
        let _ = conn.stream.shutdown(std::net::Shutdown::Both);
        Ok(reply)
    }

    fn open(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> HarnessResult<Conn> {
        if !path.starts_with('/') || path.contains(['\r', '\n', ' ']) {
            return Err(err(
                "SPX-HPL001",
                "probe path must be an absolute path without whitespace",
            ));
        }
        let addr = self.target.addr()?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| {
            err(
                "SPX-HPL003",
                format!("cannot connect to {}: {e}", self.target.origin()),
            )
        })?;
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok();
        stream.set_write_timeout(Some(self.timeout)).ok();
        let mut head = format!(
            "{method} {}{path} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nAccept: */*\r\n",
            self.target.base, self.target.host, self.target.port
        );
        if let Some(c) = &self.credential {
            if c.contains(['\r', '\n']) {
                return Err(err("SPX-HPL007", "credential value contains a line break"));
            }
            head.push_str(&format!("Authorization: Bearer {c}\r\n"));
        }
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = body {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                b.len()
            ));
        }
        head.push_str("\r\n");
        let mut conn = Conn { stream };
        conn.stream
            .write_all(head.as_bytes())
            .and_then(|_| conn.stream.write_all(body.unwrap_or(&[])))
            .map_err(|e| err("SPX-HPL003", format!("write failed: {e}")))?;
        Ok(conn)
    }
}

struct Conn {
    stream: TcpStream,
}

fn is_timeout(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
    )
}

type Head = (u16, String, bool, Option<usize>, Vec<u8>);

fn read_head(stream: &mut TcpStream, deadline: Instant) -> HarnessResult<Head> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if data.len() > 16 * 1024 {
            return Err(err("SPX-HPL003", "reply head exceeds 16 KiB"));
        }
        if Instant::now() > deadline {
            return Err(err(
                "SPX-HPL003",
                "probe timed out waiting for the reply head",
            ));
        }
        match stream.read(&mut buf) {
            Ok(0) => return Err(err("SPX-HPL003", "connection closed before a reply head")),
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(e) if is_timeout(&e) => continue,
            Err(e) => return Err(err("SPX-HPL003", format!("read failed: {e}"))),
        }
    };
    let head = String::from_utf8_lossy(&data[..end]).to_string();
    let rest = data[end + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| err("SPX-HPL003", "malformed status line"))?;
    let (mut ctype, mut chunked, mut len) = (String::new(), false, None);
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            match k.as_str() {
                "content-type" => ctype = v.to_ascii_lowercase(),
                "transfer-encoding" => chunked = v.to_ascii_lowercase().contains("chunked"),
                "content-length" => len = v.parse().ok(),
                _ => {}
            }
        }
    }
    Ok((status, ctype, chunked, len, rest))
}

/// Incremental HTTP/1.1 chunked-transfer decoder.
#[derive(Default)]
struct Dechunk {
    buf: Vec<u8>,
    remaining: usize,
    in_data: bool,
    done: bool,
}

impl Dechunk {
    fn push(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.done {
                break;
            }
            if self.in_data {
                let take = self.remaining.min(self.buf.len());
                out.extend(self.buf.drain(..take));
                self.remaining -= take;
                if self.remaining > 0 {
                    break;
                }
                // Trailing CRLF after chunk data.
                if self.buf.len() < 2 {
                    self.in_data = false;
                    self.remaining = usize::MAX; // marker: still owe CRLF
                    break;
                }
                self.buf.drain(..2);
                self.in_data = false;
                continue;
            }
            if self.remaining == usize::MAX {
                if self.buf.len() < 2 {
                    break;
                }
                self.buf.drain(..2);
                self.remaining = 0;
            }
            let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") else {
                break;
            };
            let line = String::from_utf8_lossy(&self.buf[..i]).to_string();
            self.buf.drain(..i + 2);
            let size =
                usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
            if size == 0 {
                self.done = true;
            } else {
                self.remaining = size;
                self.in_data = true;
            }
        }
        out
    }
}

/// Move complete events out of `text` (events end at a blank line).
fn drain_events(text: &mut Vec<u8>, events: &mut Vec<SseEvent>) {
    loop {
        let norm = String::from_utf8_lossy(text).replace("\r\n", "\n");
        let Some(i) = norm.find("\n\n") else { return };
        let block = norm[..i].to_string();
        let consumed = norm[..i + 2].len();
        text.drain(..consumed.min(text.len()));
        let (mut event, mut data) = (None, Vec::new());
        for l in block.lines() {
            if let Some(v) = l.strip_prefix("event:") {
                event = Some(v.trim().to_string());
            } else if let Some(v) = l.strip_prefix("data:") {
                data.push(v.strip_prefix(' ').unwrap_or(v).to_string());
            }
        }
        if event.is_some() || !data.is_empty() {
            events.push(SseEvent {
                event,
                data: data.join("\n"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_only() {
        assert!(Target::parse("http://127.0.0.1:11434").is_ok());
        assert!(Target::parse("http://localhost:4000/v1").is_ok());
        assert!(Target::parse("http://[::1]:1").is_ok());
        for bad in [
            "https://127.0.0.1:1",
            "http://example.com:80",
            "http://10.0.0.1:80",
            "http://127.0.0.1",
            "http://u@127.0.0.1:1",
            "http://127.0.0.1:1/?x",
        ] {
            assert_eq!(Target::parse(bad).unwrap_err().code, "SPX-HPL002", "{bad}");
        }
    }

    #[test]
    fn dechunk_split_anywhere() {
        let wire = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        for cut in 0..wire.len() {
            let mut d = Dechunk::default();
            let mut out = d.push(&wire[..cut]);
            out.extend(d.push(&wire[cut..]));
            assert_eq!(out, b"hello world", "cut {cut}");
            assert!(d.done);
        }
    }

    #[test]
    fn sse_events_parse() {
        let mut t = b"event: a\r\ndata: 1\r\n\r\ndata: 2\n\ndata: part".to_vec();
        let mut ev = Vec::new();
        drain_events(&mut t, &mut ev);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].event.as_deref(), Some("a"));
        assert_eq!(ev[1].data, "2");
        assert_eq!(t, b"data: part");
    }
}
