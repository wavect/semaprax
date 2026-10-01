//! Loopback TLS evidence for the real native outbound adapter.
//!
//! The private CA is test-only and installed explicitly in the client config.
//! Neither a system trust store nor a public endpoint participates in this
//! corpus.

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::*;

const ROOT: &str = "MIIDJzCCAg+gAwIBAgIUC3kI/KYpwSCFZIOpQLwZZv3fpIUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBsxGTAXBgNVBAMMEFNFTUFQUkFYIFRlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtxpzwCk3e4aRY3ozKBTi94gfLHe6yKDfDggOHGiwUGotJ9dVH8e4Hh82JamO+jH694HBmjlbGXF+BY7Gxv/Vz8Z7R9VqS1uND7J4V4pJABLL4H//k/c0WPMopTkQRmVyit34hTob14aL+hPq4DFOtH+FxXiUyPaJp6xP0UH7KTJpSBJfBlTAmJoBuMP7Ara05oozrVuLNzSDaUulGGkA5kUuv2GnPvQjTx8PG14GUfJt6okOD64JJSaoQCrraxyHIG8UmZgnHyoIq3UgFY9gj4haVW6ykKe+bkWVbwCOZcMAffzx+NKDodSahn3Qy2z0eDI0ARMtVFDE+ijtxlG/1AgMBAAGjYzBhMB0GA1UdDgQWBBT4Dg/tRse2xlFPUoKfa/7M5c40VjAfBgNVHSMEGDAWgBT4Dg/tRse2xlFPUoKfa/7M5c40VjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAQEAmEWc71S2305pR9Ps29VDVdwOcVoetWsqEnCsAIHg0qfioQz3mznfxE3gOZ4gm03AOslf2sqq8ev02MnEuZWt7Y7xwstrTyo0EA4mWXzBTz0EX7Qp1PgV4MV7Lifp+Dv5ACDx75bgOziKx+u6VVvR0RoE1tUB3m3ihO7aT0HMXOBvElkuY7Ev+fR7lgSFOPGYV2IIBcfaro0dGJlixyBjP/TLGAr8S6buf0ZFCBKtMriXyfiqcQ8IPeLEOtFGxhrWKoNoRpkYwM5kut27vDkoc5UekFmU4EaGPl0cWEpoky5RMXgrA0hAzKEmgPnbIVplKwdoELQjon+MR1HA9txCeg==";
const LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
const LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";

fn decode64(input: &str) -> Vec<u8> {
    fn digit(byte: u8) -> u8 {
        match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("test fixture is base64"),
        }
    }
    let mut output = Vec::new();
    let mut bits = 0u32;
    let mut count = 0u8;
    for byte in input.bytes().filter(|byte| *byte != b'=') {
        bits = (bits << 6) | u32::from(digit(byte));
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push((bits >> count) as u8);
            bits &= (1u32 << count) - 1;
        }
    }
    output
}

fn crypto() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn trusted_configs() -> (rustls::ClientConfig, rustls::ServerConfig) {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(decode64(ROOT)))
        .expect("test root is a certificate");
    let client = rustls::ClientConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server = rustls::ServerConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(decode64(LEAF))],
            rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(LEAF_KEY)).into(),
        )
        .expect("test key matches test certificate");
    (client, server)
}

fn empty_root_client() -> rustls::ClientConfig {
    rustls::ClientConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth()
}

/// Retain the certificate's `localhost` identity while pinning this test's
/// one resolution to its held loopback listener.
fn loopback_adapter(tls: rustls::ClientConfig, port: u16) -> NativeHttpsAdapter {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .tls_backend_preconfigured(tls)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(8)
        .resolve("localhost", SocketAddr::from(([127, 0, 0, 1], port)))
        .build()
        .expect("explicit loopback transport config");
    NativeHttpsAdapter { client }
}

fn read_request(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>,
) -> (Vec<u8>, Vec<u8>) {
    read_request_with_required_header(stream, Some("x-request-kind: loopback"))
}

fn read_request_with_required_header(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>,
    required_header: Option<&str>,
) -> (Vec<u8>, Vec<u8>) {
    let mut headers = Vec::new();
    let mut byte = [0u8; 1];
    while !headers.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .expect("bounded request headers");
        headers.push(byte[0]);
        assert!(headers.len() <= 16_384, "request headers are bounded");
    }
    let text = std::str::from_utf8(&headers).expect("request headers are ASCII");
    if let Some(required_header) = required_header {
        assert!(
            text.lines().any(|line| line == required_header),
            "required typed header reaches the TLS peer"
        );
    }
    let length = text
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .expect("request has content length")
        .parse::<usize>()
        .expect("content length is decimal");
    let mut body = vec![0; length];
    stream.read_exact(&mut body).expect("request body");
    (headers, body)
}

fn serve_rejected_handshake(config: rustls::ServerConfig) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let worker = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("one TLS client");
        socket
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("read timeout");
        let connection = rustls::ServerConnection::new(Arc::new(config)).expect("TLS server");
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut byte = [0u8; 1];
        // An untrusted client normally aborts the handshake with a TLS alert.
        // That is the expected server-side observation, not a request to parse.
        let _ = stream.read(&mut byte);
    });
    (port, worker)
}

fn serve_once(
    config: rustls::ServerConfig,
    expected_request_line: &'static [u8],
    expected_body: &'static [u8],
    response: &'static [u8],
) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let worker = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("one TLS client");
        socket
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("read timeout");
        let connection = rustls::ServerConnection::new(Arc::new(config)).expect("TLS server");
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let (headers, body) = read_request(&mut stream);
        assert!(
            headers.starts_with(expected_request_line),
            "exact request target"
        );
        assert_eq!(body, expected_body, "body crosses TLS intact");
        stream.write_all(response).expect("bounded response write");
        stream.flush().expect("response flush");
    });
    (port, worker)
}

fn serve_metric_once(config: rustls::ServerConfig) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let worker = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("one TLS client");
        socket
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("read timeout");
        let connection = rustls::ServerConnection::new(Arc::new(config)).expect("TLS server");
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let (headers, body) = read_request_with_required_header(
            &mut stream,
            Some("content-type: application/vnd.semaprax.metric.v1+json"),
        );
        assert!(
            headers.starts_with(b"POST /v1/metrics HTTP/1.1\r\n"),
            "the decoded collector target owns the fixed metric route"
        );
        assert!(
            std::str::from_utf8(&body)
                .expect("metric JSON")
                .contains("service.requests"),
            "typed metric body reaches the private TLS peer"
        );
        stream
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("bounded response write");
        stream.flush().expect("response flush");
    });
    (port, worker)
}

fn prepared(
    endpoint: String,
    method: HttpMethod,
    body: &[u8],
    max_response_bytes: usize,
) -> PreparedRequest {
    PreparedRequest {
        method,
        endpoint,
        headers: vec![("x-request-kind".into(), "loopback".into())],
        body: body.to_vec(),
        deadline_ms: 2_000,
        max_redirects: 0,
        max_response_bytes,
    }
}

fn loopback_http_request(endpoint: String, body: &[u8]) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Post,
        endpoint,
        request_id: "native-loopback-request-1".into(),
        idempotency_key: "native-loopback-idempotency-1".into(),
        content_type: Some("application/json".into()),
        headers: vec![HttpHeader::new("x-request-kind", "loopback").unwrap()],
        body: body.to_vec(),
        deadline_ms: 2_000,
    }
}

fn loopback_policy(port: u16, max_response_bytes: usize) -> OutboundPolicy {
    OutboundPolicy::new(
        "native.loopback-session.v1",
        [format!("https://localhost:{port}")],
        128,
        max_response_bytes,
        2_000,
        1,
        1,
    )
    .expect("explicit loopback policy")
}

fn loopback_metric_policy(port: u16) -> OutboundPolicy {
    OutboundPolicy::new(
        "native.loopback-metric.v1",
        [format!("https://localhost:{port}")],
        512,
        4_096,
        2_000,
        1,
        1,
    )
    .expect("explicit loopback metric policy")
}

fn loopback_capability(policy: OutboundPolicy) -> OutboundCapability {
    OutboundCapability::grant_for_trusted_host(
        "sha256:native-loopback-session",
        "native-loopback-invocation-1",
        policy,
    )
    .expect("trusted host grant")
}

fn host_service_configuration(telemetry_origin: &str) -> Vec<u8> {
    let mut value = serde_json::json!({
        "schema": "semaprax.service-config.v1",
        "mode": "host",
        "database": {"adapter":"snapshot","dsn_secret_ref":null,"migration_table":"semaprax_migrations"},
        "http": {"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},
        "secrets": {"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},
        "telemetry": {"adapter":"semaprax-json-events","endpoint_origin":telemetry_origin},
    });
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value).expect("canonical host configuration");
    bytes.push(b'\n');
    bytes
}

fn collector_metric() -> MetricExport {
    MetricExport {
        stable_metric_id: "service.requests".into(),
        observation_id: "service-loopback-1".into(),
        labels: vec![("region".into(), "local".into())],
        kind: MetricKind::CounterIncrement(1),
    }
}

#[derive(Default)]
struct CommittedSessionStore {
    checkpoints: Vec<HttpDeliverySessionCheckpoint>,
}

impl HttpDeliverySessionCheckpointStore for CommittedSessionStore {
    fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit {
        self.checkpoints.push(checkpoint.clone());
        CheckpointCommit::Committed
    }
}

#[test]
fn native_adapter_executes_exact_tls_request_and_returns_redirect_response() {
    let (client, server) = trusted_configs();
    let (port, worker) = serve_once(
        server,
        b"PUT /records?tenant=one HTTP/1.1\r\n",
        b"request-body",
        b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
    );
    let endpoint = format!("https://localhost:{port}/records?tenant=one");
    let policy = OutboundPolicy::new(
        "native.loopback.v1",
        [format!("https://localhost:{port}")],
        128,
        16,
        2_000,
        1,
        1,
    )
    .expect("explicit loopback policy");
    let capability = OutboundCapability::grant_for_trusted_host(
        "sha256:native-loopback",
        "native-invocation-1",
        policy,
    )
    .expect("trusted host grant");
    let mut adapter = loopback_adapter(client, port);
    assert_eq!(
        deliver_http(
            capability,
            HttpRequest {
                method: HttpMethod::Put,
                endpoint,
                request_id: "native-request-1".into(),
                idempotency_key: "native-idempotency-1".into(),
                content_type: Some("application/octet-stream".into()),
                headers: vec![HttpHeader::new("x-request-kind", "loopback").unwrap()],
                body: b"request-body".to_vec(),
                deadline_ms: 2_000,
            },
            &mut adapter,
        )
        .expect("admitted native delivery")
        .response_body,
        Some(b"ok".to_vec())
    );
    worker.join().expect("TLS server");

    let (client, server) = trusted_configs();
    let (port, worker) = serve_once(
        server,
        b"POST /redirect HTTP/1.1\r\n",
        b"body",
        b"HTTP/1.1 302 Found\r\nLocation: /moved\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let mut adapter = loopback_adapter(client, port);
    assert_eq!(
        adapter.send(&prepared(
            format!("https://localhost:{port}/redirect"),
            HttpMethod::Post,
            b"body",
            16,
        )),
        AdapterObservation::Response {
            status: 302,
            body: Vec::new()
        }
    );
    worker.join().expect("redirect TLS server");
}

#[test]
fn native_adapter_keeps_tls_and_response_limit_failures_uncertain() {
    let (_, server) = trusted_configs();
    let (port, worker) = serve_rejected_handshake(server);
    let mut adapter = loopback_adapter(empty_root_client(), port);
    assert!(matches!(
        adapter.send(&prepared(
            format!("https://localhost:{port}/untrusted"),
            HttpMethod::Get,
            b"",
            16
        )),
        AdapterObservation::FailedAfterStart {
            reason: AdapterFailure::Transport
        }
    ));
    worker.join().expect("untrusted TLS server");

    let (client, server) = trusted_configs();
    let (port, worker) = serve_once(
        server,
        b"GET /large HTTP/1.1\r\n",
        b"",
        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
    );
    let mut adapter = loopback_adapter(client, port);
    assert_eq!(
        adapter.send(&prepared(
            format!("https://localhost:{port}/large"),
            HttpMethod::Get,
            b"",
            8
        )),
        AdapterObservation::ResponseTooLargeAfterStart
    );
    worker.join().expect("response-limit TLS server");
}

#[test]
fn native_adapter_refuses_invalid_prepared_request_before_connection() {
    let (client, _) = trusted_configs();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let mut adapter = loopback_adapter(client, port);
    let mut request = prepared(
        format!("https://localhost:{port}/not-dispatched"),
        HttpMethod::Get,
        b"",
        16,
    );
    request.max_redirects = 1;
    assert_eq!(
        adapter.send(&request),
        AdapterObservation::NotDispatched {
            reason: AdapterFailure::PolicyRejected
        }
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn checked_service_host_request_binds_only_a_matching_host_grant_to_private_tls_telemetry() {
    let (client, server) = trusted_configs();
    let (port, worker) = serve_metric_once(server);
    let origin = format!("https://localhost:{port}");
    let configuration = host_service_configuration(&origin);
    let request = crate::project::derive_service_host_adapter_request_v1(&configuration)
        .expect("the checked host configuration renders an independently valid request");
    assert_eq!(request.canonical_bytes().last(), Some(&b'\n'));
    assert_eq!(request.requirements().len(), 3);
    let target = TelemetryCollectorTarget::for_trusted_host(
        request
            .telemetry()
            .expect("host mode declares telemetry intent")
            .endpoint_origin(),
    )
    .expect("checked telemetry origin is a collector target");
    assert_eq!(target.origin(), origin);

    let wrong_port = if port == u16::MAX { port - 1 } else { port + 1 };
    let wrong_target =
        TelemetryCollectorTarget::for_trusted_host(format!("https://localhost:{wrong_port}"))
            .expect("individually canonical but nonmatching target");
    assert!(
        matches!(
            TelemetryCollectorCapability::bind_for_trusted_host(
                loopback_capability(loopback_metric_policy(port)),
                wrong_target,
            ),
            Err(CollectorRefusal::AuthorityDenied)
        ),
        "a host policy never expands to a drifted request target"
    );

    let mut drifted: serde_json::Value = serde_json::from_slice(&configuration).unwrap();
    drifted["telemetry"]["endpoint_origin"] = serde_json::json!("http://localhost:1");
    drifted.sort_all_objects();
    let mut drifted = serde_json::to_vec(&drifted).unwrap();
    drifted.push(b'\n');
    assert!(
        crate::project::derive_service_host_adapter_request_v1(&drifted).is_err(),
        "configuration drift refuses before a target, policy, or adapter exists"
    );

    let prepared = TelemetryCollectorCapability::bind_for_trusted_host(
        loopback_capability(loopback_metric_policy(port)),
        target,
    )
    .expect("separately trusted host policy admits the decoded target")
    .prepare_metric(500, collector_metric())
    .expect("typed telemetry request stays within the host policy");
    let mut session = MetricExportSession::new(1).expect("bounded typed telemetry session");
    let mut adapter = loopback_adapter(client, port);
    session
        .reconcile(prepared, &mut adapter)
        .expect("only the matching separately granted capability dispatches");
    worker.join().expect("one telemetry TLS server request");
}

#[test]
fn native_adapter_durably_replays_committed_typed_session_without_a_second_tls_connection() {
    let (client, server) = trusted_configs();
    let body = b"{\"event\":\"job.completed\"}";
    let (port, worker) = serve_once(
        server,
        b"POST /observability HTTP/1.1\r\n",
        body,
        b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let endpoint = format!("https://localhost:{port}/observability");
    let mut session = HttpDeliverySession::new(2).expect("bounded typed session");
    let mut store = CommittedSessionStore::default();
    let mut adapter = loopback_adapter(client, port);
    let first = session
        .reconcile_durable(
            prepare_http_delivery(
                loopback_capability(loopback_policy(port, 16)),
                loopback_http_request(endpoint.clone(), body),
            )
            .expect("explicit host policy admits the loopback request"),
            &mut store,
            &mut adapter,
        )
        .expect("native TLS dispatch settles into the typed session");
    assert!(matches!(first, DurableHttpDeliveryOutcome::Dispatched(_)));
    assert_eq!(
        store.checkpoints.len(),
        2,
        "intent and terminal session states commit"
    );
    worker.join().expect("one TLS server request");

    let checkpoint = store
        .checkpoints
        .last()
        .expect("terminal typed session checkpoint was committed");
    let wire = checkpoint.render();
    let digest = checkpoint.digest();
    let capacity = checkpoint.capacity();
    let mut tampered = wire.clone();
    tampered.push(' ');
    assert!(matches!(
        HttpDeliverySession::restore_authenticated(
            tampered.as_bytes(),
            HttpDeliverySessionRestoreCapability::grant_for_trusted_host(&digest, capacity)
                .expect("trusted store binds its expected checkpoint"),
        ),
        Err(HttpDeliverySessionRestoreRefusal::Checkpoint(
            DeliverySessionCheckpointRefusal::BindingMismatch
        ))
    ));
    let mut restored = HttpDeliverySession::restore_authenticated(
        wire.as_bytes(),
        HttpDeliverySessionRestoreCapability::grant_for_trusted_host(digest, capacity)
            .expect("storage host binds the exact committed checkpoint"),
    )
    .expect("exact typed checkpoint restores");

    // Rebind the same port after the first listener exits. A regression that
    // enters the real adapter on either replay/refusal would be observable as
    // an incoming TCP connection, not merely as a recording-adapter call.
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("rebind loopback port");
    listener
        .set_nonblocking(true)
        .expect("nonblocking replay listener");
    let (client, _) = trusted_configs();
    let mut replay_adapter = loopback_adapter(client, port);
    let replay = restored
        .reconcile(
            prepare_http_delivery(
                loopback_capability(loopback_policy(port, 16)),
                loopback_http_request(endpoint.clone(), body),
            )
            .expect("exact replay request remains admitted"),
            &mut replay_adapter,
        )
        .expect("known exact request replays");
    assert!(replay.was_replayed());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "restored exact replay must never open another TLS connection"
    );

    let changed_request = restored.reconcile(
        prepare_http_delivery(
            loopback_capability(loopback_policy(port, 16)),
            loopback_http_request(endpoint.clone(), b"{\"event\":\"job.failed\"}"),
        )
        .expect("individually valid but changed request"),
        &mut replay_adapter,
    );
    assert_eq!(
        changed_request,
        Err(HttpLedgerRefusal::Ledger(LedgerRefusal::ConflictingRequest))
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "request drift must refuse before native TLS dispatch"
    );

    let drift = restored.reconcile(
        prepare_http_delivery(
            loopback_capability(loopback_policy(port, 15)),
            loopback_http_request(endpoint, body),
        )
        .expect("individually valid but changed policy request"),
        &mut replay_adapter,
    );
    assert_eq!(drift, Err(HttpLedgerRefusal::PolicyChanged));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "policy drift must refuse before native TLS dispatch"
    );
}
