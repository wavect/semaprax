//! Loopback-only HTTP serving for the reference service.
//!
//! Transport reuses the existing
//! [`TcpNetworkProvider`](semaprax::network_provider::TcpNetworkProvider):
//! the host binds `127.0.0.1` on an explicit operator-chosen port, accepts
//! one connection at a time, and speaks a closed subset of HTTP/1.1 (exact
//! `Content-Length` framing only; no chunked encoding, no keep-alive).
//! Binding anything but loopback, or port zero (whose assigned port would be
//! undiscoverable through the opaque listener token), is refused.
//!
//! Serving is plaintext by default ([`serve_one`]/[`serve_forever`], over
//! [`NetworkProvider::accept`]). A host that resolved operator-held
//! certificate/key material builds its `TcpNetworkProvider` with
//! [`TcpNetworkProvider::with_server_tls_config`] and calls
//! [`serve_one_tls`]/[`serve_forever_tls`] instead, over
//! [`NetworkProvider::accept_tls`], exclusively: a caller picks one mode at
//! startup and never mixes the two loops over the same listener, so a
//! TLS-configured host never silently falls back to plaintext framing.
//! `accept_tls` itself refuses with `AuthorityDenied` when the provider
//! holds no server policy, so the TLS loop cannot silently downgrade either.

use semaprax::network_provider::{
    NetworkProvider, ProviderConnection, ProviderListener, TcpNetworkProvider,
};

use super::mapping::PendingResponse;

/// One parsed inbound exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpExchange {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Stable refusal categories for the serve boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServeRefusal {
    /// Anything but `127.0.0.1` or port zero was requested.
    NotLoopback,
    /// The loopback bind failed (port in use or unavailable).
    ListenFailed,
}

pub const LOOPBACK_HOST: &str = "127.0.0.1";
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 32;
const MAX_HEADER_NAME_BYTES: usize = 64;
const MAX_HEADER_VALUE_BYTES: usize = 4_096;
const MAX_METHOD_BYTES: usize = 16;
const MAX_TARGET_BYTES: usize = 2_048;
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Bind the loopback listener on an explicit operator-chosen port.
pub fn listen_loopback(
    provider: &mut TcpNetworkProvider,
    port: u16,
) -> Result<ProviderListener, ServeRefusal> {
    if port == 0 {
        return Err(ServeRefusal::NotLoopback);
    }
    provider
        .listen(LOOPBACK_HOST, port)
        .map_err(|_| ServeRefusal::ListenFailed)
}

/// Accept and serve exactly one connection, then close it. Accept failures
/// are idle (the caller loops); a handler panic becomes a 500 rather than a
/// dead server.
pub fn serve_one(
    provider: &mut TcpNetworkProvider,
    listener: ProviderListener,
    handler: &mut impl FnMut(&HttpExchange) -> PendingResponse,
) {
    let connection = match provider.accept(listener) {
        Ok(connection) => connection,
        Err(_) => return,
    };
    serve_accepted(provider, connection, handler);
}

/// Serve until the process is stopped by its operator. There is no graceful
/// shutdown: crash-safety comes from the durable store, not from draining.
pub fn serve_forever(
    provider: &mut TcpNetworkProvider,
    listener: ProviderListener,
    handler: &mut impl FnMut(&HttpExchange) -> PendingResponse,
) -> ! {
    loop {
        serve_one(provider, listener, handler);
    }
}

/// Accept and serve exactly one connection under a TLS handshake, then
/// close it. Requires a provider built with a server TLS policy
/// ([`TcpNetworkProvider::with_server_tls_config`],
/// [`TcpNetworkProvider::with_tls_configs`]); a handshake failure (an
/// untrusted or expired certificate on a mutual-auth policy, or a plaintext
/// client speaking to a TLS-only listener) is idle exactly like a plaintext
/// accept failure -- the caller loops, and no partial exchange ever reaches
/// `handler`.
pub fn serve_one_tls(
    provider: &mut TcpNetworkProvider,
    listener: ProviderListener,
    handler: &mut impl FnMut(&HttpExchange) -> PendingResponse,
) {
    let connection = match provider.accept_tls(listener) {
        Ok(connection) => connection,
        Err(_) => return,
    };
    serve_accepted(provider, connection, handler);
}

/// Serve under TLS until the process is stopped by its operator. See
/// [`serve_forever`] for the plaintext loop this mirrors.
pub fn serve_forever_tls(
    provider: &mut TcpNetworkProvider,
    listener: ProviderListener,
    handler: &mut impl FnMut(&HttpExchange) -> PendingResponse,
) -> ! {
    loop {
        serve_one_tls(provider, listener, handler);
    }
}

/// The shared post-accept path: read one exchange, dispatch it (a handler
/// panic becomes a 500 rather than a dead server), render, send, and close.
/// Identical for a plaintext and a TLS connection, since both are already
/// the same [`ProviderConnection`] abstraction by the time this runs.
fn serve_accepted(
    provider: &mut TcpNetworkProvider,
    connection: ProviderConnection,
    handler: &mut impl FnMut(&HttpExchange) -> PendingResponse,
) {
    let response = match read_exchange(provider, connection) {
        Ok(exchange) => {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&exchange))).unwrap_or(
                PendingResponse {
                    status: 500,
                    body: r#"{"error":"internal"}"#.to_owned(),
                },
            )
        }
        Err(status) => PendingResponse {
            status,
            body: match status {
                400 => r#"{"error":"malformed_request"}"#.to_owned(),
                413 => r#"{"error":"request_too_large"}"#.to_owned(),
                _ => r#"{"error":"internal"}"#.to_owned(),
            },
        },
    };
    let _ = provider.send(connection, &render(&response));
    let _ = provider.close(connection);
}

fn render(response: &PendingResponse) -> Vec<u8> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Content Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    let status = if reason == "Internal Server Error" && response.status != 500 {
        500
    } else {
        response.status
    };
    let mut out = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    )
    .into_bytes();
    out.extend_from_slice(response.body.as_bytes());
    out
}

fn read_exchange(
    provider: &mut TcpNetworkProvider,
    connection: ProviderConnection,
) -> Result<HttpExchange, u16> {
    let mut head = Vec::new();
    loop {
        let chunk = provider.recv(connection, 8_192).map_err(|_| 400_u16)?;
        if chunk.is_empty() {
            return Err(400);
        }
        head.extend_from_slice(&chunk);
        if head.len() > MAX_HEAD_BYTES + MAX_BODY_BYTES {
            return Err(413);
        }
        if let Some(end) = header_end(&head) {
            if end > MAX_HEAD_BYTES {
                return Err(413);
            }
            return split_exchange(&head, end, provider, connection);
        }
    }
}

fn header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn split_exchange(
    received: &[u8],
    end: usize,
    provider: &mut TcpNetworkProvider,
    connection: ProviderConnection,
) -> Result<HttpExchange, u16> {
    let head = std::str::from_utf8(&received[..end]).map_err(|_| 400_u16)?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or(400_u16)?;
    let (method, target) = parse_request_line(request_line).ok_or(400_u16)?;
    let mut headers = Vec::new();
    let mut content_length: Option<usize> = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or(400_u16)?;
        let name = name.trim();
        let value = value.trim();
        if !valid_header_name(name) || value.len() > MAX_HEADER_VALUE_BYTES {
            return Err(400);
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(400);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(400);
            }
            let length = value.parse::<usize>().map_err(|_| 400_u16)?;
            if length > MAX_BODY_BYTES {
                return Err(413);
            }
            content_length = Some(length);
        }
        if headers.len() >= MAX_HEADERS {
            return Err(400);
        }
        headers.push((name.to_owned(), value.to_owned()));
    }
    let mut body = received[end..].to_vec();
    let want = content_length.unwrap_or(0);
    if want > MAX_BODY_BYTES || body.len() > MAX_BODY_BYTES {
        return Err(413);
    }
    while body.len() < want {
        let chunk = provider
            .recv(connection, (want - body.len()).clamp(1, 8_192))
            .map_err(|_| 400_u16)?;
        if chunk.is_empty() {
            return Err(400);
        }
        body.extend_from_slice(&chunk);
        if body.len() > MAX_BODY_BYTES {
            return Err(413);
        }
    }
    body.truncate(want);
    Ok(HttpExchange {
        method,
        target,
        headers,
        body,
    })
}

fn parse_request_line(line: &str) -> Option<(String, String)> {
    let (method, rest) = line.split_once(' ')?;
    let (target, version) = rest.split_once(' ')?;
    if version != "HTTP/1.1" {
        return None;
    }
    if method.is_empty()
        || method.len() > MAX_METHOD_BYTES
        || !method.bytes().all(|byte| byte.is_ascii_uppercase())
    {
        return None;
    }
    if target.is_empty()
        || target.len() > MAX_TARGET_BYTES
        || !target.starts_with('/')
        || !target.is_ascii()
        || target.bytes().any(|byte| byte <= 0x20 || byte == 0x7F)
    {
        return None;
    }
    Some((method.to_owned(), target.to_owned()))
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_HEADER_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    #[test]
    fn request_lines_and_headers_parse_strictly() {
        assert_eq!(
            parse_request_line("GET /v1/health HTTP/1.1"),
            Some(("GET".to_owned(), "/v1/health".to_owned()))
        );
        assert_eq!(parse_request_line("get /v1/health HTTP/1.1"), None);
        assert_eq!(parse_request_line("GET /v1/health HTTP/1.0"), None);
        assert_eq!(parse_request_line("GET /has space HTTP/1.1"), None);
        assert_eq!(parse_request_line("GET nope HTTP/1.1"), None);
        assert_eq!(parse_request_line("GET /v1/health"), None);
        assert!(valid_header_name("Content-Length"));
        assert!(valid_header_name("x-webhook-signature"));
        assert!(!valid_header_name("Bad Name"));
        assert!(!valid_header_name(""));
    }

    #[test]
    fn port_zero_is_refused_before_any_bind() {
        let mut provider = TcpNetworkProvider::new();
        assert_eq!(
            listen_loopback(&mut provider, 0),
            Err(ServeRefusal::NotLoopback)
        );
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind((LOOPBACK_HOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// Some sandboxes deny loopback sockets outright (`EPERM` on bind).
    /// Only that precise denial skips these tests; every other failure is
    /// a real error, and CI runs them fully.
    fn loopback_denied() -> bool {
        matches!(
            std::net::TcpListener::bind((LOOPBACK_HOST, 0)),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied
        )
    }

    fn round_trip(raw_request: &[u8]) -> (String, Vec<u8>) {
        let port = free_port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let worker_seen = seen.clone();
        let worker = std::thread::spawn(move || {
            let mut provider = TcpNetworkProvider::new();
            let listener = listen_loopback(&mut provider, port).unwrap();
            let mut handler = |exchange: &HttpExchange| {
                worker_seen.lock().unwrap().push(exchange.clone());
                PendingResponse {
                    status: 200,
                    body: r#"{"ok":true}"#.to_owned(),
                }
            };
            serve_one(&mut provider, listener, &mut handler);
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match TcpStream::connect((LOOPBACK_HOST, port)) {
                Ok(stream) => break stream,
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("loopback connect failed: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream.write_all(raw_request).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        worker.join().unwrap();
        let seen = seen.lock().unwrap();
        let summary = seen
            .first()
            .map(|exchange| {
                format!(
                    "{} {} headers={} body={}",
                    exchange.method,
                    exchange.target,
                    exchange.headers.len(),
                    String::from_utf8_lossy(&exchange.body)
                )
            })
            .unwrap_or_else(|| "unparsed".to_owned());
        (summary, response)
    }

    #[test]
    fn loopback_round_trip_serves_and_closes() {
        if loopback_denied() {
            eprintln!("skipping: sandbox denies loopback bind");
            return;
        }
        let (seen, response) = round_trip(
            b"POST /v1/tasks HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 7\r\n\r\n{\"a\":1}",
        );
        assert_eq!(seen, "POST /v1/tasks headers=2 body={\"a\":1}");
        let text = String::from_utf8(response).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
        assert!(text.ends_with(r#"{"ok":true}"#), "{text}");
        assert!(text.contains("Connection: close"), "{text}");
    }

    #[test]
    fn hostile_framing_is_refused_without_handler_entry() {
        if loopback_denied() {
            eprintln!("skipping: sandbox denies loopback bind");
            return;
        }
        let (seen, response) = round_trip(
            b"POST /v1/tasks HTTP/1.1\r\nHost: 127.0.0.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        );
        assert_eq!(seen, "unparsed");
        let text = String::from_utf8(response).unwrap();
        assert!(text.starts_with("HTTP/1.1 400 Bad Request\r\n"), "{text}");

        let (seen, response) = round_trip(b"BADLINE\r\n\r\n");
        assert_eq!(seen, "unparsed");
        let text = String::from_utf8(response).unwrap();
        assert!(text.starts_with("HTTP/1.1 400 Bad Request\r\n"), "{text}");
    }

    /// Test-only self-signed certificate/key material (`CN=localhost`,
    /// issued by a private test CA), the same fixture the root crate's own
    /// `network_provider::tcp` TLS tests already carry. It authenticates no
    /// production identity.
    const LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
    const LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";

    fn decode64(input: &str) -> Vec<u8> {
        fn digit(byte: u8) -> Option<u8> {
            match byte {
                b'A'..=b'Z' => Some(byte - b'A'),
                b'a'..=b'z' => Some(byte - b'a' + 26),
                b'0'..=b'9' => Some(byte - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut output = Vec::new();
        let mut bits = 0u32;
        let mut count = 0u8;
        for byte in input.bytes().filter(|byte| *byte != b'=') {
            bits = (bits << 6) | u32::from(digit(byte).expect("test fixture is base64"));
            count += 6;
            if count >= 8 {
                count -= 8;
                output.push((bits >> count) as u8);
                bits &= (1u32 << count) - 1;
            }
        }
        output
    }

    /// A TLS-only listener must never fall back to plaintext framing: a
    /// plaintext client's bytes fail the TLS handshake, so the peer gets at
    /// most a raw TLS alert record (rustls flushes one fatal alert before
    /// the connection closes) and never an HTTP response, and `handler` is
    /// never entered.
    #[test]
    fn tls_listener_refuses_a_plaintext_client_without_any_response() {
        if loopback_denied() {
            eprintln!("skipping: sandbox denies loopback bind");
            return;
        }
        let server_config = semaprax::network_provider::server_tls_config_from_der(
            decode64(LEAF),
            decode64(LEAF_KEY),
        )
        .expect("test fixture is a valid certificate/key pair");
        let port = free_port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(false));
        let worker_seen = seen.clone();
        let worker = std::thread::spawn(move || {
            let mut provider = TcpNetworkProvider::with_server_tls_config(server_config);
            let listener = listen_loopback(&mut provider, port).unwrap();
            let mut handler = |_: &HttpExchange| {
                *worker_seen.lock().unwrap() = true;
                PendingResponse {
                    status: 200,
                    body: r#"{"ok":true}"#.to_owned(),
                }
            };
            serve_one_tls(&mut provider, listener, &mut handler);
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match TcpStream::connect((LOOPBACK_HOST, port)) {
                Ok(stream) => break stream,
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("loopback connect failed: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let _ = stream
            .write_all(b"GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n");
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
        assert!(
            !response.starts_with(b"HTTP/"),
            "a plaintext client must never get an HTTP response from a TLS-only listener \
             (a raw TLS alert record, or nothing, is fine): {response:?}"
        );
        worker.join().unwrap();
        assert!(
            !*seen.lock().unwrap(),
            "the handler must never be entered for a failed handshake"
        );
    }
}
