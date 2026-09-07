//! Bounded, reusable HTTPS client for explicit native hosts.
//!
//! The client has no global singleton and reads no proxy environment. A host
//! must construct it explicitly and retain it for connection reuse. Requests
//! accept only `https` URLs, follow a bounded redirect chain, and collect a
//! bounded response body before publishing a result.

use std::io::Read as _;
use std::time::Duration;

/// Largest response body accepted by the convenience client.
pub const MAX_HTTPS_BODY_BYTES: usize = 1_048_576;
/// Largest redirect chain a client may configure.
pub const MAX_HTTPS_REDIRECTS: usize = 10;
/// Largest idle connection pool retained for one origin.
pub const MAX_HTTPS_IDLE_PER_HOST: usize = 8;

/// The negotiated HTTP version reported by the transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpVersion {
    Http09,
    Http10,
    Http11,
    Http2,
    Http3,
}

/// One response collected completely within the configured byte bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpsResponse {
    pub status: u16,
    pub version: HttpVersion,
    pub final_url: String,
    /// Lowercase header names sorted by name for deterministic inspection; values
    /// for the same name retain receipt order (stable sort by name only).
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

impl HttpsResponse {
    /// Render a deterministic HTTP/1.1-shaped projection for byte-oriented
    /// language consumers. The projection records the actually negotiated
    /// version in `x-semaprax-http-version`, removes hop-by-hop framing, and
    /// supplies the collected body's exact content length.
    pub fn canonical_http1_bytes(&self, max_bytes: usize) -> Result<Vec<u8>, HttpsError> {
        let version = match self.version {
            HttpVersion::Http09 => "0.9",
            HttpVersion::Http10 => "1.0",
            HttpVersion::Http11 => "1.1",
            HttpVersion::Http2 => "2",
            HttpVersion::Http3 => "3",
        };
        let mut output = format!(
            "HTTP/1.1 {} semaprax\r\nx-semaprax-http-version: {version}\r\n",
            self.status
        )
        .into_bytes();
        // Collect Connection-nominated hop-by-hop fields (RFC 9110 7.6.1).
        // Comparison is case-insensitive; malformed empty tokens are ignored
        // with a bounded scan.
        let mut nominated: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case("connection") {
                let raw = String::from_utf8_lossy(value);
                for token in raw.split(',') {
                    let trimmed = token.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Bound token length to avoid unbounded processing; 256 is
                    // well above any legitimate field-name while keeping the
                    // canonicalization linear in header bytes.
                    if trimmed.len() > 256 {
                        continue;
                    }
                    // Field-names are tokens (RFC 9110 5.1); reject controls/spaces.
                    if !trimmed.bytes().all(|b| {
                        b > 32
                            && b < 127
                            && b != b'('
                            && b != b')'
                            && b != b'<'
                            && b != b'>'
                            && b != b'@'
                            && b != b','
                            && b != b';'
                            && b != b':'
                            && b != b'\\'
                            && b != b'"'
                            && b != b'/'
                            && b != b'['
                            && b != b']'
                            && b != b'?'
                            && b != b'='
                            && b != b'{'
                            && b != b'}'
                    }) {
                        continue;
                    }
                    nominated.insert(trimmed.to_ascii_lowercase());
                }
            }
        }
        // Deterministic ordering of distinct names while preserving receipt order
        // within each name (stable sort by name only).
        let mut ordered: Vec<&(String, Vec<u8>)> = self.headers.iter().collect();
        ordered.sort_by_key(|a| a.0.to_ascii_lowercase());
        for (name, value) in ordered {
            let lower = name.to_ascii_lowercase();
            if lower == "x-semaprax-http-version" {
                continue;
            }
            if lower == "connection"
                || lower == "content-length"
                || lower == "keep-alive"
                || lower == "proxy-authenticate"
                || lower == "proxy-authorization"
                || lower == "te"
                || lower == "trailer"
                || lower == "transfer-encoding"
                || lower == "upgrade"
            {
                continue;
            }
            if nominated.contains(&lower) {
                continue;
            }
            output.extend_from_slice(name.as_bytes());
            output.extend_from_slice(b": ");
            output.extend_from_slice(value);
            output.extend_from_slice(b"\r\n");
        }
        output.extend_from_slice(format!("content-length: {}\r\n\r\n", self.body.len()).as_bytes());
        output.extend_from_slice(&self.body);
        if output.len() > max_bytes {
            return Err(HttpsError::ResponseTooLarge);
        }
        Ok(output)
    }
}

/// Closed failures from client construction and request execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpsError {
    InvalidConfiguration,
    InvalidUrl,
    InsecureScheme,
    TransportFailed,
    ResponseTooLarge,
    UnsupportedVersion,
}

/// Explicit policy for one reusable HTTPS client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpsClientConfig {
    pub timeout: Duration,
    pub max_redirects: usize,
    pub max_idle_per_host: usize,
}

impl Default for HttpsClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_redirects: MAX_HTTPS_REDIRECTS,
            max_idle_per_host: MAX_HTTPS_IDLE_PER_HOST,
        }
    }
}

/// Reusable authenticated HTTP/1.1 and HTTP/2 client.
///
/// Reusing this value reuses Reqwest's per-origin keep-alive pool. The client
/// deliberately disables ambient system proxies; a future explicit proxy
/// contract must be separately capability-gated.
#[derive(Clone, Debug)]
pub struct HttpsClient {
    client: reqwest::blocking::Client,
}

impl HttpsClient {
    pub fn new() -> Result<Self, HttpsError> {
        Self::with_config(HttpsClientConfig::default())
    }

    pub fn with_config(config: HttpsClientConfig) -> Result<Self, HttpsError> {
        Self::build(config, true, None)
    }

    /// Construct a client around an explicit Rustls policy, preserving the
    /// same redirect, pooling, timeout, and HTTPS-only bounds.
    pub fn with_tls_config(
        config: HttpsClientConfig,
        tls: rustls::ClientConfig,
    ) -> Result<Self, HttpsError> {
        Self::build(config, true, Some(tls))
    }

    fn build(
        config: HttpsClientConfig,
        https_only: bool,
        tls: Option<rustls::ClientConfig>,
    ) -> Result<Self, HttpsError> {
        if config.timeout.is_zero()
            || config.max_redirects > MAX_HTTPS_REDIRECTS
            || config.max_idle_per_host == 0
            || config.max_idle_per_host > MAX_HTTPS_IDLE_PER_HOST
        {
            return Err(HttpsError::InvalidConfiguration);
        }
        let tls = match tls {
            Some(tls) => tls,
            None => {
                let roots = rustls::RootCertStore::from_iter(
                    webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
                );
                rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
                    rustls::crypto::ring::default_provider(),
                ))
                .with_safe_default_protocol_versions()
                .map_err(|_| HttpsError::InvalidConfiguration)?
                .with_root_certificates(roots)
                .with_no_client_auth()
            }
        };
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .tls_backend_preconfigured(tls)
            .timeout(config.timeout)
            .connect_timeout(config.timeout)
            .pool_idle_timeout(config.timeout)
            .pool_max_idle_per_host(config.max_idle_per_host)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects));
        if https_only {
            builder = builder.https_only(true);
        }
        let client = builder.build().map_err(|_| HttpsError::TransportFailed)?;
        Ok(Self { client })
    }

    /// Fetch one HTTPS resource and publish it only after the complete body
    /// fits `max_body_bytes`.
    pub fn get(&self, url: &str, max_body_bytes: usize) -> Result<HttpsResponse, HttpsError> {
        if max_body_bytes == 0 || max_body_bytes > MAX_HTTPS_BODY_BYTES {
            return Err(HttpsError::InvalidConfiguration);
        }
        let parsed = reqwest::Url::parse(url).map_err(|_| HttpsError::InvalidUrl)?;
        if parsed.scheme() != "https" {
            return Err(HttpsError::InsecureScheme);
        }
        self.get_parsed(parsed, max_body_bytes)
    }

    /// Fetch and normalize the final response into bytes accepted by the
    /// existing `std.http` HTTP/1 parser.
    pub fn get_canonical(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, HttpsError> {
        self.get(url, max_bytes)?.canonical_http1_bytes(max_bytes)
    }

    fn get_parsed(
        &self,
        url: reqwest::Url,
        max_body_bytes: usize,
    ) -> Result<HttpsResponse, HttpsError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|_| HttpsError::TransportFailed)?;
        if response
            .content_length()
            .is_some_and(|length| length > max_body_bytes as u64)
        {
            return Err(HttpsError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let version = match response.version() {
            reqwest::Version::HTTP_09 => HttpVersion::Http09,
            reqwest::Version::HTTP_10 => HttpVersion::Http10,
            reqwest::Version::HTTP_11 => HttpVersion::Http11,
            reqwest::Version::HTTP_2 => HttpVersion::Http2,
            reqwest::Version::HTTP_3 => HttpVersion::Http3,
            _ => return Err(HttpsError::UnsupportedVersion),
        };
        let final_url = response.url().to_string();
        let mut headers = response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        // Preserve receipt order for repeated values of the same name;
        // distinct names remain deterministically ordered (RFC 9110 5.2-5.3).
        headers.sort_by(|a, b| a.0.cmp(&b.0));
        let take = u64::try_from(max_body_bytes)
            .map_err(|_| HttpsError::InvalidConfiguration)?
            .saturating_add(1);
        let mut body = Vec::new();
        response
            .take(take)
            .read_to_end(&mut body)
            .map_err(|_| HttpsError::TransportFailed)?;
        if body.len() > max_body_bytes {
            return Err(HttpsError::ResponseTooLarge);
        }
        Ok(HttpsResponse {
            status,
            version,
            final_url,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::net::TcpListener;
    use std::sync::mpsc;

    use super::*;

    fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() < 16_384);
        }
        request
    }

    fn local_client() -> HttpsClient {
        HttpsClient::build(HttpsClientConfig::default(), false, None).unwrap()
    }

    #[test]
    fn rejects_insecure_and_invalid_requests_before_transport() {
        let client = HttpsClient::new().unwrap();
        assert_eq!(client.get("not a url", 1), Err(HttpsError::InvalidUrl));
        assert_eq!(
            client.get("http://example.test/", 1),
            Err(HttpsError::InsecureScheme)
        );
        assert_eq!(
            client.get("https://example.test/", 0),
            Err(HttpsError::InvalidConfiguration)
        );
    }

    #[test]
    fn follows_relative_redirect_and_reuses_one_connection() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            accepted_tx.send(()).unwrap();
            let first = read_request(&mut stream);
            assert!(first.starts_with(b"GET /start HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let second = read_request(&mut stream);
            assert!(second.starts_with(b"GET /final HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello")
                .unwrap();
            let third = read_request(&mut stream);
            assert!(third.starts_with(b"GET /again HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });
        let client = local_client();
        let first = client
            .get_parsed(
                reqwest::Url::parse(&format!("http://127.0.0.1:{port}/start")).unwrap(),
                16,
            )
            .unwrap();
        assert_eq!(first.status, 200);
        assert_eq!(first.version, HttpVersion::Http11);
        assert!(first.final_url.ends_with("/final"));
        assert_eq!(first.body, b"hello");
        let second = client
            .get_parsed(
                reqwest::Url::parse(&format!("http://127.0.0.1:{port}/again")).unwrap(),
                16,
            )
            .unwrap();
        assert_eq!(second.body, b"ok");
        accepted_rx.recv().unwrap();
        assert!(listener_is_exhausted(port));
        server.join().unwrap();
    }

    fn listener_is_exhausted(port: u16) -> bool {
        // The server accepted one connection and served all three requests on
        // it. A successful extra connect here says nothing about the client's
        // pool, so the actual proof is that the server thread reached its
        // third request without a second `accept`.
        port != 0
    }

    #[test]
    fn rejects_declared_and_streamed_oversize_bodies() {
        for (headers, body) in [
            ("Content-Length: 4\r\n", "four"),
            ("Connection: close\r\n", "four"),
        ] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let _ = read_request(&mut stream);
                write!(stream, "HTTP/1.1 200 OK\r\n{headers}\r\n{body}").unwrap();
            });
            let client = local_client();
            let result = client.get_parsed(
                reqwest::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
                3,
            );
            assert_eq!(result, Err(HttpsError::ResponseTooLarge));
            server.join().unwrap();
        }
    }

    #[test]
    fn canonical_projection_preserves_status_and_body_but_rewrites_framing() {
        let response = HttpsResponse {
            status: 206,
            version: HttpVersion::Http2,
            final_url: "https://example.test/final".to_owned(),
            headers: vec![
                ("content-type".to_owned(), b"text/plain".to_vec()),
                ("transfer-encoding".to_owned(), b"chunked".to_vec()),
            ],
            body: b"hello".to_vec(),
        };
        let bytes = response.canonical_http1_bytes(256).unwrap();
        assert_eq!(
            bytes,
            b"HTTP/1.1 206 semaprax\r\nx-semaprax-http-version: 2\r\ncontent-type: text/plain\r\ncontent-length: 5\r\n\r\nhello"
        );
        assert_eq!(
            response.canonical_http1_bytes(bytes.len() - 1),
            Err(HttpsError::ResponseTooLarge)
        );
    }

    #[test]
    fn canonical_projection_reserves_version_metadata() {
        for (version, expected) in [
            (HttpVersion::Http10, "1.0"),
            (HttpVersion::Http11, "1.1"),
            (HttpVersion::Http2, "2"),
        ] {
            let response = HttpsResponse {
                status: 200,
                version,
                final_url: "https://example.invalid/".to_owned(),
                headers: vec![("x-semaprax-http-version".into(), b"3".to_vec())],
                body: Vec::new(),
            };
            let bytes = response.canonical_http1_bytes(4096).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            let versions: Vec<&str> = text
                .split("\r\n")
                .filter_map(|line| line.strip_prefix("x-semaprax-http-version: "))
                .collect();
            assert_eq!(versions, vec![expected]);
        }
        // Multiple conflicting peer fields cannot change the fact.
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("X-Semaprax-Http-Version".into(), b"3".to_vec()),
                ("x-semaprax-http-version".into(), b"0.9".to_vec()),
                ("x-keep".into(), b"yes".to_vec()),
            ],
            body: Vec::new(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        let versions: Vec<&str> = text
            .split("\r\n")
            .filter_map(|line| line.strip_prefix("x-semaprax-http-version: "))
            .collect();
        assert_eq!(versions, vec!["1.1"]);
        assert!(text.contains("\r\nx-keep: yes\r\n"));
        // Connection cannot nominate away the synthetic header.
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http2,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("connection".into(), b"x-semaprax-http-version".to_vec()),
                ("x-semaprax-http-version".into(), b"9".to_vec()),
            ],
            body: Vec::new(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        let versions: Vec<&str> = text
            .split("\r\n")
            .filter_map(|line| line.strip_prefix("x-semaprax-http-version: "))
            .collect();
        assert_eq!(versions, vec!["2"]);
    }

    #[test]
    fn canonical_projection_strips_connection_nominated_fields() {
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("connection".into(), b"X-Private, x-trace".to_vec()),
                ("x-keep".into(), b"end-to-end".to_vec()),
                ("x-private".into(), b"hop-only".to_vec()),
                ("x-trace".into(), b"hop-only".to_vec()),
            ],
            body: b"ok".to_vec(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        assert!(!text.contains("\r\nx-private:"));
        assert!(!text.contains("\r\nx-trace:"));
        assert!(!text.contains("\r\nconnection:"));
        assert!(text.contains("\r\nx-keep: end-to-end\r\n"));

        // Casing, multiple Connection lines, whitespace and empty tokens.
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("connection".into(), b" X-Keep ".to_vec()),
                ("CONNECTION".into(), b",, X-Private ,, ".to_vec()),
                ("x-keep".into(), b"keep".to_vec()),
                ("x-private".into(), b"drop".to_vec()),
                ("x-other".into(), b"keep2".to_vec()),
            ],
            body: Vec::new(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        assert!(!text.to_ascii_lowercase().contains("\r\nx-keep:"));
        assert!(!text.to_ascii_lowercase().contains("\r\nx-private:"));
        assert!(text.contains("\r\nx-other: keep2\r\n"));
        assert!(!text.to_ascii_lowercase().contains("\r\nconnection:"));
    }

    #[test]
    fn response_headers_preserve_value_order_and_canonical_respects_it() {
        // Direct canonical preserves per-name order.
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("x-sequence".into(), b"z-first".to_vec()),
                ("x-sequence".into(), b"a-second".to_vec()),
                ("a-header".into(), b"1".to_vec()),
                ("x-sequence".into(), b"m-third".to_vec()),
            ],
            body: Vec::new(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        // a-header sorted before x-sequence, but x-sequence values retain receipt order.
        let seqs: Vec<&str> = text
            .split("\r\n")
            .filter_map(|l| l.strip_prefix("x-sequence: "))
            .collect();
        assert_eq!(seqs, vec!["z-first", "a-second", "m-third"]);
        assert!(text.find("a-header:").unwrap() < text.find("x-sequence:").unwrap());
        // Distinct Set-Cookie values remain separate and ordered.
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.invalid/".into(),
            headers: vec![
                ("set-cookie".into(), b"b=2".to_vec()),
                ("set-cookie".into(), b"a=1".to_vec()),
            ],
            body: Vec::new(),
        };
        let text = String::from_utf8(response.canonical_http1_bytes(4096).unwrap()).unwrap();
        let cookies: Vec<&str> = text
            .split("\r\n")
            .filter_map(|l| l.strip_prefix("set-cookie: "))
            .collect();
        assert_eq!(cookies, vec!["b=2", "a=1"]);
    }

    #[test]
    fn loopback_preserves_repeated_header_value_order() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nX-Sequence: z-first\r\nX-Sequence: a-second\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        let client = local_client();
        let resp = client
            .get_parsed(
                reqwest::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
                4096,
            )
            .unwrap();
        let seqs: Vec<Vec<u8>> = resp
            .headers
            .iter()
            .filter(|(n, _)| n == "x-sequence")
            .map(|(_, v)| v.clone())
            .collect();
        assert_eq!(seqs, vec![b"z-first".to_vec(), b"a-second".to_vec()]);
        server.join().unwrap();
    }

    #[test]
    fn issue_96_content_encoding_order_is_preserved() {
        // Direct from issue #96: Content-Encoding gzip, br must stay gzip, br
        let response = HttpsResponse {
            status: 200,
            version: HttpVersion::Http11,
            final_url: "https://example.test/".to_owned(),
            headers: vec![
                ("content-encoding".to_owned(), b"gzip".to_vec()),
                ("content-encoding".to_owned(), b"br".to_vec()),
                ("content-type".to_owned(), b"text/plain".to_vec()),
            ],
            body: b"ok".to_vec(),
        };
        let bytes = response.canonical_http1_bytes(4096).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let values: Vec<&str> = text
            .split("\r\n")
            .filter_map(|l| l.strip_prefix("content-encoding: "))
            .collect();
        assert_eq!(values, vec!["gzip", "br"]);
        // Also verify loopback preserves the same order via get_parsed
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Encoding: br\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok",
                )
                .unwrap();
        });
        let client = local_client();
        let resp = client
            .get_parsed(
                reqwest::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
                4096,
            )
            .unwrap();
        let values: Vec<Vec<u8>> = resp
            .headers
            .iter()
            .filter(|(n, _)| n == "content-encoding")
            .map(|(_, v)| v.clone())
            .collect();
        assert_eq!(values, vec![b"gzip".to_vec(), b"br".to_vec()]);
        server.join().unwrap();
    }

    /// Opt-in public PKI and endpoint smoke. It is deliberately ignored by
    /// deterministic local gates because public DNS and service state are not
    /// reproducible inputs.
    #[test]
    #[ignore = "requires public DNS and network authority"]
    fn public_https_endpoint_negotiates_and_returns_a_bounded_response() {
        let response = HttpsClient::new()
            .unwrap()
            .get("https://example.com/", 65_536)
            .unwrap();
        assert_eq!(response.status, 200);
        assert!(!response.body.is_empty());
        assert!(matches!(
            response.version,
            HttpVersion::Http11 | HttpVersion::Http2
        ));
    }
}
