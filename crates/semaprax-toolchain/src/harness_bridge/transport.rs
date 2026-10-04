//! Plain-HTTP, loopback-only `HostHttpStreamTransport` (std only).
//!
//! The origin is fixed at construction, the bearer secret never appears in
//! `Debug` output or any failure, response bytes are bounded, and `cancel`
//! closes the socket (the remote may still finish; that is reported as
//! `CancelledAfterDispatch`, never as "stopped").

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use semaprax::provider_adapter_sdk::vendor::{
    HostHttpStream, HostHttpStreamTransport, ProviderHttpRequest, TransportFailure,
    TransportFailureKind, TransportPoll,
};
use semaprax_harness::diag::{HarnessDiagnostic, HarnessResult};
use semaprax_harness::endpoint::probe::Target;

const MAX_HEAD_BYTES: usize = 16 * 1024;
const POLL_SLICE: Duration = Duration::from_millis(25);

/// A host-provided bearer secret. Not `Display`; `Debug` is redacted.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> HarnessResult<Self> {
        let value = value.into();
        if value.is_empty() || value.contains(['\r', '\n', '\0']) {
            return Err(HarnessDiagnostic::new(
                "SPX-HPL007",
                "credential value is empty or contains a control character",
            ));
        }
        Ok(Self(value))
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Clone, Debug)]
pub struct LoopbackTransport {
    target: Target,
    secret: Option<Secret>,
    idle_timeout: Duration,
}

impl LoopbackTransport {
    /// `http://127.0.0.1|localhost|[::1]:PORT[/prefix]` only (`SPX-HPL002`).
    pub fn new(url: &str, secret: Option<Secret>) -> HarnessResult<Self> {
        Ok(Self {
            target: Target::parse(url)?,
            secret,
            idle_timeout: Duration::from_secs(120),
        })
    }

    #[must_use]
    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    fn connect(&self) -> Result<TcpStream, TransportFailure> {
        let not_dispatched = || TransportFailure {
            kind: TransportFailureKind::NotDispatched,
            attempted_bytes: 0,
        };
        let host = self.target.host.trim_matches(['[', ']']);
        let addr = (host, self.target.port)
            .to_socket_addrs()
            .map_err(|_| not_dispatched())?
            .find(|a| a.ip().is_loopback())
            .ok_or_else(not_dispatched)?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .map_err(|_| not_dispatched())?;
        if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
            return Err(not_dispatched());
        }
        stream.set_read_timeout(Some(POLL_SLICE)).ok();
        stream.set_write_timeout(Some(Duration::from_secs(10))).ok();
        Ok(stream)
    }
}

impl HostHttpStreamTransport for LoopbackTransport {
    fn start(
        &mut self,
        request: ProviderHttpRequest,
    ) -> Result<Box<dyn HostHttpStream>, TransportFailure> {
        let refuse = || TransportFailure {
            kind: TransportFailureKind::NotDispatched,
            attempted_bytes: 0,
        };
        let bad = |s: &str| s.contains(['\r', '\n', '\0']);
        if !request.path.starts_with('/')
            || request.path.contains([' ', '?', '#'])
            || bad(request.path)
            || bad(request.method)
            || request.method.contains(' ')
            || request
                .headers
                .iter()
                .any(|(k, v)| bad(k) || bad(v) || k.eq_ignore_ascii_case("authorization"))
        {
            return Err(refuse());
        }
        let mut stream = self.connect()?;
        let mut head = format!(
            "{} {}{} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n",
            request.method, self.target.base, request.path, self.target.host, self.target.port
        );
        if let Some(Secret(token)) = &self.secret {
            head.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        for (k, v) in &request.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str(&format!("Content-Length: {}\r\n\r\n", request.body.len()));
        stream
            .write_all(head.as_bytes())
            .and_then(|_| stream.write_all(&request.body))
            .map_err(|_| TransportFailure {
                kind: TransportFailureKind::UncertainAfterDispatch,
                attempted_bytes: 0,
            })?;
        Ok(Box::new(LoopbackStream {
            stream,
            phase: Phase::Head(Vec::new()),
            observed: 0,
            max: request.max_response_bytes,
            last_progress: Instant::now(),
            idle: self.idle_timeout,
        }))
    }
}

enum Framing {
    Chunked(Dechunk),
    Length(usize),
    Close,
}

enum Phase {
    Head(Vec<u8>),
    Body(Framing),
    Done,
    Failed(TransportFailure),
}

struct LoopbackStream {
    stream: TcpStream,
    phase: Phase,
    observed: usize,
    max: usize,
    last_progress: Instant,
    idle: Duration,
}

impl LoopbackStream {
    fn fail(&mut self, kind: TransportFailureKind) -> TransportPoll {
        let failure = TransportFailure {
            kind,
            attempted_bytes: self.observed,
        };
        self.phase = Phase::Failed(failure.clone());
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        TransportPoll::Failed(failure)
    }

    /// Decode raw body bytes; `Err` carries the failure kind.
    fn body(&mut self, raw: &[u8]) -> Result<Vec<u8>, TransportFailureKind> {
        let Phase::Body(framing) = &mut self.phase else {
            return Ok(Vec::new());
        };
        let (data, finished) = match framing {
            Framing::Chunked(d) => {
                let data = d
                    .push(raw)
                    .map_err(|()| TransportFailureKind::MalformedResponse)?;
                (data, d.done)
            }
            Framing::Length(left) => {
                let take = raw.len().min(*left);
                *left -= take;
                (raw[..take].to_vec(), *left == 0)
            }
            Framing::Close => (raw.to_vec(), false),
        };
        self.observed = self.observed.saturating_add(data.len());
        if self.observed > self.max {
            return Err(TransportFailureKind::CapacityExceeded);
        }
        if finished {
            self.phase = Phase::Done;
        }
        Ok(data)
    }

    /// Parse the response head; returns leftover body bytes.
    fn head(&mut self, buf: &[u8]) -> Result<Vec<u8>, TransportFailureKind> {
        let end = buf.windows(4).position(|w| w == b"\r\n\r\n");
        let Some(end) = end else {
            if buf.len() > MAX_HEAD_BYTES {
                return Err(TransportFailureKind::MalformedResponse);
            }
            self.phase = Phase::Head(buf.to_vec());
            return Ok(Vec::new());
        };
        let text = String::from_utf8_lossy(&buf[..end]).into_owned();
        let mut lines = text.lines();
        let status: u16 = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .ok_or(TransportFailureKind::MalformedResponse)?;
        if !(200..300).contains(&status) {
            return Err(match status {
                400 | 401 | 403 | 404 | 405 | 409 | 413 | 422 | 429 => {
                    TransportFailureKind::RejectedBeforeProcessing
                }
                _ => TransportFailureKind::UncertainAfterDispatch,
            });
        }
        let (mut chunked, mut length, mut sse) = (false, None, false);
        for line in lines {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
            match k.as_str() {
                "transfer-encoding" => chunked = v.contains("chunked"),
                "content-length" => length = v.parse::<usize>().ok(),
                "content-type" => sse = v.starts_with("text/event-stream"),
                _ => {}
            }
        }
        if !sse {
            return Err(TransportFailureKind::MalformedResponse);
        }
        self.phase = Phase::Body(if chunked {
            Framing::Chunked(Dechunk::default())
        } else if let Some(n) = length {
            Framing::Length(n)
        } else {
            Framing::Close
        });
        Ok(buf[end + 4..].to_vec())
    }
}

impl HostHttpStream for LoopbackStream {
    fn poll(&mut self) -> TransportPoll {
        match &self.phase {
            Phase::Done => return TransportPoll::End,
            Phase::Failed(f) => return TransportPoll::Failed(f.clone()),
            _ => {}
        }
        let mut buf = [0u8; 8192];
        match self.stream.read(&mut buf) {
            Ok(0) => {
                let clean = matches!(self.phase, Phase::Body(Framing::Close));
                if clean {
                    self.phase = Phase::Done;
                    TransportPoll::End
                } else {
                    self.fail(TransportFailureKind::UncertainAfterDispatch)
                }
            }
            Ok(n) => {
                self.last_progress = Instant::now();
                let raw = match std::mem::replace(&mut self.phase, Phase::Done) {
                    Phase::Head(mut pending) => {
                        pending.extend_from_slice(&buf[..n]);
                        match self.head(&pending) {
                            Ok(rest) => rest,
                            Err(kind) => return self.fail(kind),
                        }
                    }
                    other => {
                        self.phase = other;
                        buf[..n].to_vec()
                    }
                };
                match self.body(&raw) {
                    Ok(data) if data.is_empty() => TransportPoll::Pending,
                    Ok(data) => TransportPoll::Chunk(data),
                    Err(kind) => self.fail(kind),
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                if self.last_progress.elapsed() > self.idle {
                    self.fail(TransportFailureKind::TimeoutAfterDispatch)
                } else {
                    TransportPoll::Pending
                }
            }
            Err(_) => self.fail(TransportFailureKind::UncertainAfterDispatch),
        }
    }

    fn cancel(&mut self, _reason: &str) {
        if !matches!(self.phase, Phase::Failed(_)) {
            let _ = self.fail(TransportFailureKind::CancelledAfterDispatch);
        }
    }
}

enum DState {
    Size,
    Data(usize),
    Crlf,
    Trailers,
}

struct Dechunk {
    buf: Vec<u8>,
    state: DState,
    done: bool,
}

impl Default for Dechunk {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            state: DState::Size,
            done: false,
        }
    }
}

impl Dechunk {
    /// Incremental chunked-transfer decoder; arbitrary split points are fine.
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ()> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.done {
                break;
            }
            match self.state {
                DState::Size => {
                    let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") else {
                        if self.buf.len() > 64 {
                            return Err(());
                        }
                        break;
                    };
                    let line = std::str::from_utf8(&self.buf[..i]).map_err(|_| ())?;
                    let size = line.split(';').next().unwrap_or("").trim();
                    let n = usize::from_str_radix(size, 16).map_err(|_| ())?;
                    self.buf.drain(..i + 2);
                    self.state = if n == 0 {
                        DState::Trailers
                    } else {
                        DState::Data(n)
                    };
                }
                DState::Data(left) => {
                    if self.buf.is_empty() {
                        break;
                    }
                    let take = left.min(self.buf.len());
                    out.extend(self.buf.drain(..take));
                    self.state = if take == left {
                        DState::Crlf
                    } else {
                        DState::Data(left - take)
                    };
                }
                DState::Crlf => {
                    if self.buf.len() < 2 {
                        break;
                    }
                    if &self.buf[..2] != b"\r\n" {
                        return Err(());
                    }
                    self.buf.drain(..2);
                    self.state = DState::Size;
                }
                DState::Trailers => {
                    let Some(i) = self.buf.windows(2).position(|w| w == b"\r\n") else {
                        if self.buf.len() > 4096 {
                            return Err(());
                        }
                        break;
                    };
                    let empty = i == 0;
                    self.buf.drain(..i + 2);
                    if empty {
                        self.done = true;
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dechunk_split_at_every_byte() {
        let wire = b"5\r\nhello\r\n6;x=1\r\n world\r\n0\r\n\r\n";
        let mut d = Dechunk::default();
        let mut out = Vec::new();
        for b in wire {
            out.extend(d.push(&[*b]).unwrap());
        }
        assert!(d.done);
        assert_eq!(out, b"hello world");
    }

    #[test]
    fn secret_is_redacted_and_validated() {
        assert_eq!(
            format!("{:?}", Secret::new("tok-123").unwrap()),
            "Secret(<redacted>)"
        );
        assert!(Secret::new("a\nb").is_err());
    }
}
