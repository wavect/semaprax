//! Independent local TLS-provider acceptance cases for the reference-service host.
//!
//! This stays inside the existing `runtime_host` test binary so test discovery and
//! the production service fixture remain unchanged while the parent module remains
//! within the repository source-size limit.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use semaprax::network_provider::{server_tls_config_from_der, NetworkProvider, TcpNetworkProvider};
use semaprax_native_host::reference_service::mapping::PendingResponse;
use semaprax_native_host::reference_service::serve;

use super::{
    decode64, field, free_port, http, loopback_denied, Server, Workdir, TELEMETRY_ROOT_ARGS,
    TLS_LEAF, TLS_LEAF_KEY,
};

const PROVIDER_WAIT: Duration = Duration::from_secs(30);
const PROVIDER_POLL: Duration = Duration::from_millis(20);
const PROVIDER_CHILD_MODE: &str = "SEMAPRAX_REFERENCE_SERVICE_PROVIDER_CHILD_MODE";
const PROVIDER_CHILD_PORT: &str = "SEMAPRAX_REFERENCE_SERVICE_PROVIDER_CHILD_PORT";
const PROVIDER_CHILD_DONE: &str = "SEMAPRAX_REFERENCE_SERVICE_PROVIDER_CHILD_DONE";
const PROVIDER_CHILD_RECEIPT: &str = "SEMAPRAX_REFERENCE_SERVICE_PROVIDER_CHILD_RECEIPT";
const PROVIDER_CHILD_OUTCOME: &str = "SEMAPRAX_REFERENCE_SERVICE_PROVIDER_CHILD_OUTCOME";
const PROVIDER_SUCCESS_TEST: &str = "reference_service_acceptance::local_provider::completion_delivers_to_local_tls_provider_and_restart_does_not_duplicate";
const PROVIDER_REFUSAL_TEST: &str = "reference_service_acceptance::local_provider::completion_provider_refusal_persists_across_restart_without_duplicate";
const PROVIDER_UNCERTAIN_TEST: &str = "reference_service_acceptance::local_provider::completion_provider_close_after_request_is_uncertain_across_restart_without_duplicate";
const PROVIDER_OTLP_SUCCESS_TEST: &str = "reference_service_acceptance::local_provider::otlp_logs_completion_delivers_to_local_tls_provider_and_restart_does_not_duplicate";
const PROVIDER_OTLP_PARTIAL_TEST: &str = "reference_service_acceptance::local_provider::otlp_logs_partial_response_persists_failure_across_restart_without_duplicate";
const PROVIDER_OTLP_MALFORMED_TEST: &str = "reference_service_acceptance::local_provider::otlp_logs_malformed_response_persists_failure_across_restart_without_duplicate";

/// A separate real TLS provider process used by the outbound acceptance
/// journey. It records each received HTTP request in a file outside the
/// service process, then stays available through the service restart so the
/// parent can prove no duplicate identity arrives before it signals shutdown.
struct ProviderChild {
    child: Child,
    lines: mpsc::Receiver<String>,
    done: PathBuf,
    receipt: PathBuf,
    port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderOutcome {
    Accepted,
    Rejected,
    ClosedAfterRequest,
    Partial,
    Malformed,
}

impl ProviderOutcome {
    fn as_child_value(self) -> &'static str {
        match self {
            Self::Accepted => "accepted-v1",
            Self::Rejected => "rejected-v1",
            Self::ClosedAfterRequest => "closed-after-request-v1",
            Self::Partial => "partial-v1",
            Self::Malformed => "malformed-v1",
        }
    }

    fn from_child_value(value: &str) -> Self {
        match value {
            "accepted-v1" => Self::Accepted,
            "rejected-v1" => Self::Rejected,
            "closed-after-request-v1" => Self::ClosedAfterRequest,
            "partial-v1" => Self::Partial,
            "malformed-v1" => Self::Malformed,
            _ => panic!("provider child outcome is unknown"),
        }
    }

    fn receipt_outcome(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::ClosedAfterRequest => "closed-after-request",
            Self::Partial => "partial",
            Self::Malformed => "malformed",
        }
    }

    fn assert_webhook(self, actual: &str) {
        match self {
            Self::Accepted => {
                let digest = actual
                    .strip_prefix("delivered:sha256:")
                    .expect("successful delivery carries its evidence digest");
                assert_eq!(digest.len(), 64, "SHA-256 digest has 64 hex digits");
                assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
            }
            Self::Rejected | Self::Partial | Self::Malformed => assert_eq!(actual, "failed"),
            Self::ClosedAfterRequest => assert_eq!(actual, "uncertain"),
        }
    }

    fn response(self) -> PendingResponse {
        match self {
            Self::Accepted => PendingResponse {
                status: 200,
                body: "{}".to_owned(),
            },
            Self::Rejected => PendingResponse {
                status: 409,
                body: "{}".to_owned(),
            },
            Self::Partial => PendingResponse {
                status: 200,
                body: r#"{"partialSuccess":{"rejectedLogRecords":"1"}}"#.to_owned(),
            },
            Self::Malformed => PendingResponse {
                status: 200,
                body: "{".to_owned(),
            },
            Self::ClosedAfterRequest => unreachable!("closed provider sends no response"),
        }
    }
}

impl ProviderChild {
    fn spawn(workdir: &Workdir, test_name: &str, outcome: ProviderOutcome) -> Self {
        let port = free_port();
        let done = workdir.path("provider.done");
        let receipt = workdir.path("provider-receipt.txt");
        let executable = std::env::current_exe().expect("current runtime_host test executable");
        let mut command = Command::new(executable);
        command
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env_clear();
        inherit_platform_loader_environment(&mut command);
        let mut child = command
            .env(PROVIDER_CHILD_MODE, "serve-v1")
            .env(PROVIDER_CHILD_PORT, port.to_string())
            .env(PROVIDER_CHILD_DONE, &done)
            .env(PROVIDER_CHILD_RECEIPT, &receipt)
            .env(PROVIDER_CHILD_OUTCOME, outcome.as_child_value())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn independent local TLS provider");
        let stdout = child.stdout.take().expect("provider stdout is piped");
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let mut provider = Self {
            child,
            lines,
            done,
            receipt,
            port,
        };
        let deadline = Instant::now() + PROVIDER_WAIT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "provider never announced readiness");
            match provider.lines.recv_timeout(remaining) {
                Ok(line) if line == format!("provider-ready port={}", provider.port) => {
                    return provider;
                }
                Ok(_) => continue,
                Err(_) => {
                    let mut stderr = String::new();
                    if let Some(mut pipe) = provider.child.stderr.take() {
                        let _ = pipe.read_to_string(&mut stderr);
                    }
                    panic!("provider exited before readiness: {stderr}");
                }
            }
        }
    }

    fn finish(mut self) -> String {
        std::fs::write(&self.done, b"done\n").expect("signal provider shutdown");
        let deadline = Instant::now() + PROVIDER_WAIT;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll provider child") {
                if !status.success() {
                    let mut stderr = String::new();
                    if let Some(mut pipe) = self.child.stderr.take() {
                        let _ = pipe.read_to_string(&mut stderr);
                    }
                    panic!("provider child must pass: {status}; stderr: {stderr}");
                }
                return std::fs::read_to_string(&self.receipt)
                    .expect("provider persisted its independently observed receipt");
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("provider child exceeded its bounded shutdown wait");
            }
            std::thread::sleep(PROVIDER_POLL);
        }
    }
}

impl Drop for ProviderChild {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.done, b"done\n");
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn inherit_platform_loader_environment(command: &mut Command) {
    for name in [
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
        "LIBPATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn provider_child_value(name: &str, maximum: usize) -> String {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("provider child missing {name}"));
    assert!(
        !value.is_empty() && value.len() <= maximum,
        "provider child {name} exceeds its bounded fixture input"
    );
    value
}

fn run_provider_child() {
    assert_eq!(
        std::env::var(PROVIDER_CHILD_MODE).as_deref(),
        Ok("serve-v1"),
        "provider child mode is one-shot and cannot recurse"
    );
    let port = provider_child_value(PROVIDER_CHILD_PORT, 5)
        .parse::<u16>()
        .expect("provider port is decimal");
    assert_ne!(port, 0, "provider port is explicit");
    let done = PathBuf::from(provider_child_value(PROVIDER_CHILD_DONE, 4_096));
    let receipt = PathBuf::from(provider_child_value(PROVIDER_CHILD_RECEIPT, 4_096));
    let outcome =
        ProviderOutcome::from_child_value(&provider_child_value(PROVIDER_CHILD_OUTCOME, 32));
    let server_config = server_tls_config_from_der(decode64(TLS_LEAF), decode64(TLS_LEAF_KEY))
        .expect("provider fixture certificate is valid");
    let mut provider = TcpNetworkProvider::with_server_tls_config(server_config)
        .with_deadline_policy(semaprax::network_provider::deadline::DeadlinePolicy::new(
            // The same deadline covers accept, TLS, and the response. Keep the
            // fixture's budget above the client's 10-second delivery deadline
            // so runner scheduling cannot turn a partial response into an
            // unrelated uncertain transport outcome.
            Duration::from_secs(15),
        ));
    let listener = serve::listen_loopback(&mut provider, port).expect("bind provider loopback TLS");
    println!("provider-ready port={port}");
    std::io::stdout().flush().expect("flush provider readiness");

    let mut count = 0usize;
    match outcome {
        ProviderOutcome::Accepted
        | ProviderOutcome::Rejected
        | ProviderOutcome::Partial
        | ProviderOutcome::Malformed => {
            let mut handler = |exchange: &serve::HttpExchange| {
                count += 1;
                persist_provider_receipt(
                    &receipt,
                    count,
                    outcome,
                    &exchange.method,
                    &exchange.target,
                    &exchange.headers,
                    &exchange.body,
                );
                outcome.response()
            };
            while !done.exists() {
                serve::serve_one_tls(&mut provider, listener, &mut handler);
            }
        }
        ProviderOutcome::ClosedAfterRequest => {
            while !done.exists() {
                serve_one_provider_connection_then_close(
                    &mut provider,
                    listener,
                    &receipt,
                    &mut count,
                    outcome,
                );
            }
        }
    }
    provider.settle();
    assert!(
        receipt.exists(),
        "provider must receive at least one request"
    );
}

fn persist_provider_receipt(
    receipt: &std::path::Path,
    count: usize,
    outcome: ProviderOutcome,
    method: &str,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) {
    let idempotency_key = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("idempotency-key"))
        .map(|(_, value)| value.as_str())
        .unwrap_or("");
    let body = std::str::from_utf8(body).expect("provider received UTF-8 JSON");
    std::fs::write(
        receipt,
        format!(
            "count={count}\noutcome={}\nmethod={method}\ntarget={target}\nidempotency-key={idempotency_key}\nbody={body}\n",
            outcome.receipt_outcome(),
        ),
    )
    .expect("provider persists received request evidence");
}

fn serve_one_provider_connection_then_close(
    provider: &mut TcpNetworkProvider,
    listener: semaprax::network_provider::ProviderListener,
    receipt: &std::path::Path,
    count: &mut usize,
    outcome: ProviderOutcome,
) {
    let Ok(connection) = provider.accept_tls(listener) else {
        return;
    };
    let mut received = Vec::new();
    let request = loop {
        match provider.recv(connection, 8_192) {
            Ok(chunk) if chunk.is_empty() => break None,
            Ok(chunk) => {
                received.extend_from_slice(&chunk);
                if received.len() > 24 * 1024 {
                    break None;
                }
                if let Some(request) = complete_provider_request(&received) {
                    break Some(request);
                }
            }
            Err(_) => break None,
        }
    };
    if let Some((method, target, headers, body)) = request {
        *count += 1;
        persist_provider_receipt(receipt, *count, outcome, &method, &target, &headers, &body);
    }
    // This is the physical post-start uncertainty boundary: the peer has
    // persisted receipt of the request but deliberately sends no HTTP bytes.
    let _ = provider.close(connection);
}

type ProviderRequest = (String, String, Vec<(String, String)>, Vec<u8>);

fn complete_provider_request(bytes: &[u8]) -> Option<ProviderRequest> {
    let head_end = bytes.windows(4).position(|window| window == b"\r\n\r\n")? + 4;
    let head = std::str::from_utf8(&bytes[..head_end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_owned();
    let target = request_line.next()?.to_owned();
    if request_line.next()? != "HTTP/1.1" || request_line.next().is_some() {
        return None;
    }
    let mut headers = Vec::new();
    let mut content_length = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        let value = value.trim_start().to_owned();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().ok();
        }
        headers.push((name.to_owned(), value));
    }
    let content_length = content_length?;
    let end = head_end.checked_add(content_length)?;
    if bytes.len() < end {
        return None;
    }
    Some((method, target, headers, bytes[head_end..end].to_vec()))
}

/// Exercise the production delivery route against an independently running
/// local TLS provider. The provider receives and persists the real HTTP
/// request before it returns its response, then remains alive across the
/// service restart to make a duplicate identity observable from outside the
/// service process.
#[test]
fn completion_delivers_to_local_tls_provider_and_restart_does_not_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_SUCCESS_TEST,
        "real-provider-restart",
        ProviderOutcome::Accepted,
        "semaprax-json-events",
        "/v1/events",
    );
}

#[test]
fn completion_provider_refusal_persists_across_restart_without_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_REFUSAL_TEST,
        "real-provider-refusal",
        ProviderOutcome::Rejected,
        "semaprax-json-events",
        "/v1/events",
    );
}

#[test]
fn completion_provider_close_after_request_is_uncertain_across_restart_without_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_UNCERTAIN_TEST,
        "real-provider-uncertain",
        ProviderOutcome::ClosedAfterRequest,
        "semaprax-json-events",
        "/v1/events",
    );
}

#[test]
fn otlp_logs_completion_delivers_to_local_tls_provider_and_restart_does_not_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_OTLP_SUCCESS_TEST,
        "otlp-provider-restart",
        ProviderOutcome::Accepted,
        "otlp-http-json",
        "/v1/logs",
    );
}

#[test]
fn otlp_logs_partial_response_persists_failure_across_restart_without_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_OTLP_PARTIAL_TEST,
        "otlp-provider-partial",
        ProviderOutcome::Partial,
        "otlp-http-json",
        "/v1/logs",
    );
}

#[test]
fn otlp_logs_malformed_response_persists_failure_across_restart_without_duplicate() {
    if std::env::var_os(PROVIDER_CHILD_MODE).is_some() {
        run_provider_child();
        return;
    }
    completion_provider_case(
        PROVIDER_OTLP_MALFORMED_TEST,
        "otlp-provider-malformed",
        ProviderOutcome::Malformed,
        "otlp-http-json",
        "/v1/logs",
    );
}

fn completion_provider_case(
    test_name: &str,
    label: &str,
    outcome: ProviderOutcome,
    telemetry_adapter: &str,
    expected_target: &str,
) {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create(label);
    let provider = ProviderChild::spawn(&workdir, test_name, outcome);
    let origin = format!("https://localhost:{}", provider.port);
    workdir.write_inputs_with_telemetry(&origin, telemetry_adapter);
    workdir.write_telemetry_root_material();

    let server = Server::spawn(&workdir, TELEMETRY_ROOT_ARGS);
    let port = server.port;
    let (status, body) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 201, "{body}");
    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();
    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "created");

    let (status, body) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    let initial_webhook = field(&body, "webhook").to_owned();
    outcome.assert_webhook(&initial_webhook);
    let digest = field(&body, "state").to_owned();
    drop(server);

    let restart_args = [
        "--state",
        digest.as_str(),
        "--telemetry-root-certificate-secret",
        "telemetry.root",
    ];
    let server = Server::spawn(&workdir, &restart_args);
    let (status, body) = http(server.port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");
    assert_eq!(field(&body, "webhook"), initial_webhook);
    let (status, _) = http(server.port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 409, "a completed identity cannot re-enter delivery");
    drop(server);

    let receipt = provider.finish();
    assert!(
        receipt.starts_with("count=1\n"),
        "the independently observed provider count must remain one after restart: {receipt}"
    );
    assert!(
        receipt.contains(&format!("outcome={}\n", outcome.receipt_outcome())),
        "{receipt}"
    );
    assert!(receipt.contains("method=POST\n"), "{receipt}");
    assert!(
        receipt.contains(&format!("target={expected_target}\n")),
        "{receipt}"
    );
    assert!(
        receipt.contains("idempotency-key=completion\n"),
        "{receipt}"
    );
    if expected_target == "/v1/events" {
        assert!(receipt.contains(r#""event":"job.completed""#), "{receipt}");
        assert!(receipt.contains(r#""job_id":1"#), "{receipt}");
    } else {
        assert!(receipt.contains(r#""resourceLogs""#), "{receipt}");
        assert!(receipt.contains(r#""job.completed""#), "{receipt}");
        assert!(receipt.contains(r#""semaprax.job.id""#), "{receipt}");
        assert!(!receipt.contains("signature"), "{receipt}");
    }
}
