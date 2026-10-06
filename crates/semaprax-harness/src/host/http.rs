//! Authenticated loopback HTTP/1.1 client facility for HP-11/HP-12.
//!
//! Separately enabled host facility, never selectable by model output: the
//! only way to name a destination is [`ApprovedEndpoint::from_host_config`],
//! fed from host configuration. v1 speaks plain HTTP to loopback only. Any
//! other host or `https` is refused (`SPX-HPC030`): remote use requires TLS
//! verification, which v1 does not implement, and an honest refusal beats a
//! cleartext remote call. Redirects are never followed. The credential header
//! is injected here from a host-supplied secret and never printed.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

fn err(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}

/// An exact loopback `host:port` approved by host configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedEndpoint {
    addr: SocketAddr,
}

impl ApprovedEndpoint {
    /// Parse `http://127.0.0.1:PORT[/]`, `http://[::1]:PORT` or
    /// `http://localhost:PORT` (fixed to 127.0.0.1; no DNS is consulted).
    pub fn from_host_config(url: &str) -> HarnessResult<Self> {
        const TLS: &str = "remote transport requires TLS, unsupported in v1";
        let rest = if let Some(r) = url.strip_prefix("http://") {
            r
        } else if url.starts_with("https://") {
            return Err(err("SPX-HPC030", TLS));
        } else {
            return Err(err(
                "SPX-HPC037",
                "endpoint must be an http:// URL from host configuration",
            ));
        };
        let authority = rest.trim_end_matches('/');
        if authority.is_empty() || authority.contains(['/', '@', '?', '#', ' ']) {
            return Err(err(
                "SPX-HPC037",
                "endpoint must be exactly `http://host:port` (no path, userinfo or query)",
            ));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => (
                h,
                p.parse::<u16>()
                    .map_err(|_| err("SPX-HPC037", "invalid port"))?,
            ),
            _ => (authority, 80),
        };
        let ip: IpAddr = match host.trim_matches(['[', ']']) {
            "localhost" => Ipv4Addr::LOCALHOST.into(),
            h => h.parse().map_err(|_| err("SPX-HPC030", TLS))?,
        };
        if !ip.is_loopback() {
            return Err(err("SPX-HPC030", TLS));
        }
        Ok(Self {
            addr: SocketAddr::new(ip, port),
        })
    }

    pub fn authority(&self) -> String {
        self.addr.to_string()
    }
}

/// Host-supplied credential header. `Debug` never shows the value.
#[derive(Clone)]
pub struct Credential {
    header: String,
    value: String,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Credential({}: <redacted>)", self.header)
    }
}

impl Credential {
    pub fn new(header: &str, value: &str) -> HarnessResult<Self> {
        let token = !header.is_empty()
            && header
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        let reserved = [
            "host",
            "content-length",
            "transfer-encoding",
            "connection",
            "content-type",
        ];
        if !token || reserved.contains(&header.to_ascii_lowercase().as_str()) {
            return Err(err(
                "SPX-HPC037",
                "credential header name is malformed or reserved",
            ));
        }
        if value.is_empty() || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(err(
                "SPX-HPC037",
                "credential value is empty or contains a line break",
            ));
        }
        Ok(Self {
            header: header.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpLimits {
    pub max_response_bytes: usize,
    pub connect_timeout_ms: u64,
    pub total_timeout_ms: u64,
}

impl Default for HttpLimits {
    fn default() -> Self {
        Self {
            max_response_bytes: 1024 * 1024,
            connect_timeout_ms: 2_000,
            total_timeout_ms: 30_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpClient {
    endpoint: ApprovedEndpoint,
    credential: Option<Credential>,
    limits: HttpLimits,
}

const HEADER_CAP: usize = 16 * 1024;

impl HttpClient {
    pub fn new(
        endpoint: ApprovedEndpoint,
        credential: Option<Credential>,
        limits: HttpLimits,
    ) -> Self {
        Self {
            endpoint,
            credential,
            limits,
        }
    }

    /// One request/response. `method` is `GET` or `POST`; `path` is
    /// origin-form (`/x?y`). Redirects (3xx) are refused, not followed.
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> HarnessResult<HttpResponse> {
        if !matches!(method, "GET" | "POST") {
            return Err(err("SPX-HPC037", "only GET and POST are supported"));
        }
        if !path.starts_with('/') || !path.bytes().all(|b| (0x21..0x7f).contains(&b)) {
            return Err(err(
                "SPX-HPC037",
                "request path must be origin-form ASCII without spaces",
            ));
        }
        let started = Instant::now();
        let total = Duration::from_millis(self.limits.total_timeout_ms);
        let io = |e: std::io::Error| err("SPX-HPC035", format!("loopback request failed: {e}"));
        let mut s = TcpStream::connect_timeout(
            &self.endpoint.addr,
            Duration::from_millis(self.limits.connect_timeout_ms),
        )
        .map_err(io)?;
        let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n", self.endpoint.authority());
        if let Some(c) = &self.credential {
            head.push_str(&format!("{}: {}\r\n", c.header, c.value));
        }
        if let Some(b) = body {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                b.len()
            ));
        }
        head.push_str("\r\n");
        let mut wire = head.into_bytes();
        wire.extend_from_slice(body.unwrap_or_default());
        s.set_write_timeout(Some(total)).map_err(io)?;
        s.write_all(&wire).map_err(io)?;
        let hard = HEADER_CAP + self.limits.max_response_bytes * 2 + 64;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let left = total
                .checked_sub(started.elapsed())
                .filter(|d| !d.is_zero());
            let Some(left) = left else {
                return Err(err(
                    "SPX-HPC034",
                    "loopback request exceeded its total timeout",
                ));
            };
            // A peer that already closed makes this fail on some platforms; the read
            // below then reports EOF or the real error.
            let _ = s.set_read_timeout(Some(left));
            match s.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(err("SPX-HPC034", "loopback request timed out"))
                }
                Err(e) => return Err(io(e)),
            }
            if buf.len() > hard {
                return Err(err("SPX-HPC033", "response exceeds the host size bound"));
            }
            if let Some(end) = find(&buf, b"\r\n\r\n") {
                if let Some(status) = status_of(&buf[..end]) {
                    if (300..400).contains(&status) {
                        return Err(err(
                            "SPX-HPC032",
                            format!("endpoint answered {status}; redirects are never followed"),
                        ));
                    }
                }
            }
        }
        self.parse(&buf)
    }

    fn parse(&self, buf: &[u8]) -> HarnessResult<HttpResponse> {
        let bad = |m: &str| err("SPX-HPC036", format!("malformed HTTP response: {m}"));
        let end = find(buf, b"\r\n\r\n").ok_or_else(|| bad("no header terminator"))?;
        if end > HEADER_CAP {
            return Err(err(
                "SPX-HPC033",
                "response headers exceed the host size bound",
            ));
        }
        let head = std::str::from_utf8(&buf[..end]).map_err(|_| bad("non-UTF-8 headers"))?;
        let status = status_of(&buf[..end]).ok_or_else(|| bad("bad status line"))?;
        if (300..400).contains(&status) {
            return Err(err(
                "SPX-HPC032",
                format!("endpoint answered {status}; redirects are never followed"),
            ));
        }
        let (mut len, mut chunked) = (None, false);
        for line in head.split("\r\n").skip(1) {
            let Some((k, v)) = line.split_once(':') else {
                return Err(bad("header without colon"));
            };
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => {
                    len = Some(
                        v.trim()
                            .parse::<usize>()
                            .map_err(|_| bad("bad content-length"))?,
                    )
                }
                "transfer-encoding" if v.trim().eq_ignore_ascii_case("chunked") => chunked = true,
                _ => {}
            }
        }
        let raw = &buf[end + 4..];
        let max = self.limits.max_response_bytes;
        let body = if chunked {
            dechunk(raw, max)?
        } else if let Some(n) = len {
            if n > max {
                return Err(err(
                    "SPX-HPC033",
                    format!("response of {n} bytes exceeds the {max}-byte bound"),
                ));
            }
            raw.get(..n)
                .ok_or_else(|| bad("body shorter than content-length"))?
                .to_vec()
        } else {
            raw.to_vec()
        };
        if body.len() > max {
            return Err(err(
                "SPX-HPC033",
                format!("response exceeds the {max}-byte bound"),
            ));
        }
        Ok(HttpResponse { status, body })
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn status_of(head: &[u8]) -> Option<u16> {
    let line = head.split(|b| *b == b'\r').next()?;
    let s = std::str::from_utf8(line).ok()?;
    let mut it = s.split(' ');
    it.next().filter(|v| v.starts_with("HTTP/1."))?;
    it.next()?.parse().ok()
}

/// Bound on the trailer section that may follow the last chunk.
const TRAILER_CAP: usize = 8 * 1024;

fn dechunk(mut raw: &[u8], max: usize) -> HarnessResult<Vec<u8>> {
    let bad = |m: &str| err("SPX-HPC036", format!("malformed chunked body: {m}"));
    let over = || {
        err(
            "SPX-HPC033",
            format!("response exceeds the {max}-byte bound"),
        )
    };
    let mut out = Vec::new();
    loop {
        let eol = find(raw, b"\r\n").ok_or_else(|| bad("missing chunk size line"))?;
        let digits = std::str::from_utf8(&raw[..eol])
            .ok()
            .and_then(|s| s.split(';').next())
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| bad("bad chunk size"))?;
        // A size that cannot fit a usize can never fit the response bound.
        let size = usize::from_str_radix(digits, 16).map_err(|_| over())?;
        raw = &raw[eol + 2..];
        if size == 0 {
            return trailers(raw).map(|()| out).map_err(bad);
        }
        if size > max.saturating_sub(out.len()) {
            return Err(over());
        }
        let data = raw.get(..size).ok_or_else(|| bad("truncated chunk"))?;
        out.extend_from_slice(data);
        let rest = &raw[size..];
        match rest.get(..2) {
            Some(b"\r\n") => raw = &rest[2..],
            Some(_) => return Err(bad("chunk data not followed by CRLF")),
            None => return Err(bad("truncated chunk")),
        }
    }
}

/// Validate the section after the last chunk: bounded `name: value` lines and
/// the terminating empty line.
fn trailers(mut raw: &[u8]) -> Result<(), &'static str> {
    let mut used = 0usize;
    loop {
        let eol = find(raw, b"\r\n").ok_or("unterminated trailer section")?;
        if eol == 0 {
            return Ok(());
        }
        used += eol + 2;
        if used > TRAILER_CAP {
            return Err("trailer section too large");
        }
        let line = std::str::from_utf8(&raw[..eol]).map_err(|_| "non-UTF-8 trailer")?;
        match line.split_once(':') {
            Some((name, _)) if !name.is_empty() && !name.contains(char::is_whitespace) => {}
            _ => return Err("malformed trailer"),
        }
        raw = &raw[eol + 2..];
    }
}
