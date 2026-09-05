//! Bounded Language Network I/O v1 through the reference interpreter seam:
//! the fixture provider, the real-socket provider against a loopback
//! listener, and every normalized failure code.
//!
//! Loopback only; every socket test binds `127.0.0.1:0` and talks to its own
//! listener thread, so nothing here depends on the host's network.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::thread::{self, JoinHandle};

use semaprax::hosted_interpreter::{
    execute_network_command, HostedCommandInput, HostedCommandResult,
};
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::network_provider::{
    DeniedNetworkProvider, FixtureNetworkProvider, HttpFailure, NetworkFailure, NetworkProvider,
    ProviderConnection, TcpNetworkProvider, WaitState,
};
use semaprax::{hir, parse, verify};

const ENTRY: &str = "net.run";
const STATUS_DOMAIN: &str = "semaprax.network.v1";

const REQUEST: &str = "PING\n";
const MORE: &str = "MORE\n";
const CHUNK_ONE: &str = "chunk-one|";
const CHUNK_TWO: &str = "chunk-two\n";

/// Module shell shared by every program: full network and stdout authority,
/// a `BODY` placeholder for the command body, and an unrelated `main`.
const SHELL: &str = r#"
module net.app;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("net.run")
fn run() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
BODY
}

@id("main")
fn main() -> i64
{
    0
}
"#;

/// Connect, send `PING`, stream one chunk, send `MORE`, stream the second
/// chunk, observe end of stream twice (read and wait), close.
const STREAM_BODY: &str = r#"
    let host = HOST;
    let handle = net_connect(array_as_slice(host), PORT);
    let request = REQUEST;
    let sent = net_send(handle, array_as_slice(request));
    let first = net_stream_stdout(handle, 4096usize);
    let more = MORE;
    let sent_more = net_send(handle, array_as_slice(more));
    let second = net_stream_stdout(handle, 4096usize);
    let third = net_stream_stdout(handle, 4096usize);
    let state = net_wait(handle, 5000usize);
    net_close(handle) == 0usize && sent == 5usize && sent_more == 5usize && first > 0usize && second > 0usize && third == 0usize && state == 2usize
"#;

/// Receive one owned chunk and append it, then observe end of stream through
/// an empty receive and a closed wait.
const RECV_BODY: &str = r#"
    let host = HOST;
    let handle = net_connect(array_as_slice(host), PORT);
    let request = REQUEST;
    let sent = net_send(handle, array_as_slice(request));
    let received = net_recv(handle, 4096usize);
    let appended = stdout_append(bytes_as_slice(received));
    let second = net_recv(handle, 4096usize);
    let tail = byte_len(bytes_as_slice(second));
    let state = net_wait(handle, 10usize);
    net_close(handle) == 0usize && sent == 5usize && appended > 0usize && tail == 0usize && state == 2usize
"#;

/// Connect and return without closing; settlement must release the socket.
const LEAK_BODY: &str = r#"
    let host = HOST;
    let handle = net_connect(array_as_slice(host), PORT);
    let request = REQUEST;
    net_send(handle, array_as_slice(request)) == 5usize
"#;

fn byte_array(text: &str) -> String {
    let items = text
        .bytes()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>();
    format!("[{}]", items.join(", "))
}

fn program(body: &str, host: &str, port: u16) -> String {
    let body = body
        .replace("HOST", &byte_array(host))
        .replace("PORT", &format!("{port}usize"))
        .replace("REQUEST", &byte_array(REQUEST))
        .replace("MORE", &byte_array(MORE));
    SHELL.replace("BODY", body.trim_end())
}

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("network-io-interpreter.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}\n{source}");
    hir::resolve(&ast).unwrap()
}

fn run(source: &str, provider: &mut dyn NetworkProvider) -> HostedCommandResult {
    execute_network_command(
        &resolved(source),
        ENTRY,
        &HostedCommandInput::default(),
        provider,
        100_000,
    )
    .unwrap()
}

#[derive(Default)]
struct HttpsFixture;

impl NetworkProvider for HttpsFixture {
    fn https_get(&mut self, url: &str, max: usize) -> Result<Vec<u8>, HttpFailure> {
        assert_eq!(url, "https://example.test/data");
        let response = b"HTTP/1.1 200 semaprax\r\ncontent-length: 2\r\n\r\nok".to_vec();
        assert!(response.len() <= max);
        Ok(response)
    }

    fn connect(&mut self, _: &str, _: u16) -> Result<ProviderConnection, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn send(&mut self, _: ProviderConnection, _: &[u8]) -> Result<usize, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn recv(&mut self, _: ProviderConnection, _: usize) -> Result<Vec<u8>, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn wait(&mut self, _: ProviderConnection, _: u32) -> Result<WaitState, NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn close(&mut self, _: ProviderConnection) -> Result<(), NetworkFailure> {
        Err(NetworkFailure::AuthorityDenied)
    }

    fn settle(&mut self) {}
}

fn fixture(port: u16, chunks: &[&str], expect_send: Option<&str>, ready: bool) -> String {
    let recv = chunks
        .iter()
        .map(|chunk| format!("{chunk:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let expect = expect_send.map_or(String::new(), |sent| format!(", \"expect_send\": {sent:?}"));
    format!(
        "{{\"schema\": \"semaprax.network-fixture.v1\", \"connections\": [{{\"host\": \"127.0.0.1\", \"port\": {port}, \"recv\": [{recv}]{expect}, \"ready\": {ready}}}]}}"
    )
}

fn fixture_provider(
    port: u16,
    chunks: &[&str],
    expect_send: Option<&str>,
) -> FixtureNetworkProvider {
    FixtureNetworkProvider::from_json(&fixture(port, chunks, expect_send, true)).unwrap()
}

fn expect_true(result: &HostedCommandResult) {
    assert_eq!(
        result.evaluation.outcome,
        CommandEvaluationOutcome::ReturnedBool(true),
        "{result:?}"
    );
}

/// The normalized network status code of a failed invocation; both
/// transcripts must have been discarded.
fn failure_code(result: &HostedCommandResult) -> u32 {
    let CommandEvaluationOutcome::LanguageFailure(status) = &result.evaluation.outcome else {
        panic!("expected a normalized network failure: {result:?}");
    };
    assert_eq!(status.domain_id(), STATUS_DOMAIN);
    assert!(result.stdout.is_empty(), "failure must discard stdout");
    assert!(result.stderr.is_empty(), "failure must discard stderr");
    status.code()
}

/// A body that appends to stdout before failing, so the discard is observable.
fn failing_body(tail: &str) -> String {
    format!(
        "    let request = REQUEST;\n    let marker = stdout_append(array_as_slice(request));\n{tail}"
    )
}

fn failure(body_tail: &str, provider: &mut dyn NetworkProvider) -> u32 {
    let result = run(
        &program(&failing_body(body_tail), "127.0.0.1", 8080),
        provider,
    );
    failure_code(&result)
}

#[test]
fn fixture_streams_two_chunks_into_the_stdout_transcript() {
    let mut provider = fixture_provider(8080, &[CHUNK_ONE, CHUNK_TWO], Some(REQUEST));
    let result = run(&program(STREAM_BODY, "127.0.0.1", 8080), &mut provider);
    expect_true(&result);
    assert_eq!(result.stdout, format!("{CHUNK_ONE}{CHUNK_TWO}").as_bytes());
    assert!(result.stderr.is_empty());
}

#[test]
fn fixture_recv_returns_owned_bytes_and_then_end_of_stream() {
    let mut provider = fixture_provider(8080, &["payload"], Some(REQUEST));
    let result = run(&program(RECV_BODY, "127.0.0.1", 8080), &mut provider);
    expect_true(&result);
    assert_eq!(result.stdout, b"payload");
}

#[test]
fn fixture_splits_oversized_chunks_by_max_and_keeps_the_remainder_pending() {
    let body = r#"
    let host = HOST;
    let handle = net_connect(array_as_slice(host), PORT);
    let first = net_stream_stdout(handle, 3usize);
    let second = net_stream_stdout(handle, 3usize);
    let third = net_stream_stdout(handle, 3usize);
    let fourth = net_stream_stdout(handle, 3usize);
    net_close(handle) == 0usize && first == 3usize && second == 3usize && third == 1usize && fourth == 0usize
"#;
    let mut provider = fixture_provider(8080, &["abcdefg"], None);
    let result = run(&program(body, "127.0.0.1", 8080), &mut provider);
    expect_true(&result);
    assert_eq!(result.stdout, b"abcdefg");
}

#[test]
fn fixture_ready_false_times_out_once_then_reports_readable() {
    let body = r#"
    let host = HOST;
    let handle = net_connect(array_as_slice(host), PORT);
    let first = net_wait(handle, 100usize);
    let second = net_wait(handle, 100usize);
    let streamed = net_stream_stdout(handle, 16usize);
    let closed = net_wait(handle, 0usize);
    net_close(handle) == 0usize && first == 0usize && second == 1usize && streamed == 1usize && closed == 2usize
"#;
    let mut provider =
        FixtureNetworkProvider::from_json(&fixture(8080, &["x"], None, false)).unwrap();
    let result = run(&program(body, "127.0.0.1", 8080), &mut provider);
    expect_true(&result);
    assert_eq!(result.stdout, b"x");
}

#[test]
fn connect_failed_for_a_host_outside_the_fixture() {
    let mut provider = fixture_provider(8080, &[], None);
    let tail = "    let endpoint = HOST;\n    let handle = net_connect(array_as_slice(endpoint), PORT);\n    marker == 5usize"
        .replace("HOST", &byte_array("10.0.0.1"))
        .replace("PORT", "8080usize");
    assert_eq!(failure(&tail, &mut provider), 1);
    let mut provider = fixture_provider(8080, &[], None);
    let tail =
        "    let endpoint = HOST;\n    let handle = net_connect(array_as_slice(endpoint), 8081usize);\n    marker == 5usize"
            .replace("HOST", &byte_array("127.0.0.1"));
    assert_eq!(failure(&tail, &mut provider), 1);
}

#[test]
fn invalid_endpoints_fail_before_the_provider_sees_them() {
    let host = byte_array("127.0.0.1");
    for tail in [
        format!("    let endpoint = {host};\n    let handle = net_connect(array_as_slice(endpoint), 0usize);\n    marker == 5usize"),
        format!("    let endpoint = {host};\n    let handle = net_connect(array_as_slice(endpoint), 65536usize);\n    marker == 5usize"),
        "    let endpoint = [0u8, 49u8];\n    let handle = net_connect(array_as_slice(endpoint), 80usize);\n    marker == 5usize".to_owned(),
        "    let endpoint = [255u8, 254u8];\n    let handle = net_connect(array_as_slice(endpoint), 80usize);\n    marker == 5usize".to_owned(),
        format!(
            "    let endpoint = {};\n    let handle = net_connect(array_as_slice(endpoint), 80usize);\n    marker == 5usize",
            byte_array(&"a".repeat(254))
        ),
    ] {
        let mut provider = fixture_provider(8080, &[], None);
        assert_eq!(failure(&tail, &mut provider), 2, "{tail}");
    }
}

#[test]
fn forged_and_closed_handles_are_unknown() {
    let mut provider = fixture_provider(8080, &[], None);
    assert_eq!(
        failure(
            "    let sent = net_send(7usize, array_as_slice(request));\n    marker == 5usize",
            &mut provider
        ),
        3
    );
    let mut provider = fixture_provider(8080, &[], None);
    let tail = format!(
        "    let endpoint = {};\n    let handle = net_connect(array_as_slice(endpoint), 8080usize);\n    let closed = net_close(handle);\n    let state = net_wait(handle, 1usize);\n    marker == 5usize",
        byte_array("127.0.0.1")
    );
    assert_eq!(failure(&tail, &mut provider), 3);
    let mut provider = fixture_provider(8080, &[], None);
    assert_eq!(
        failure(
            "    let state = net_wait(0usize, 1usize);\n    marker == 5usize",
            &mut provider
        ),
        3
    );
}

#[test]
fn capacity_failures_cover_handles_chunks_and_timeouts() {
    let host = byte_array("127.0.0.1");
    let mut eight = String::new();
    for index in 0..9 {
        eight.push_str(&format!(
            "    let e{index} = {host};\n    let h{index} = net_connect(array_as_slice(e{index}), 8080usize);\n"
        ));
    }
    eight.push_str("    marker == 5usize");
    // The ninth connect is rejected by the evaluator before provider entry,
    // so the bounded fixture needs exactly the eight reachable connections.
    let connections = (0..8)
        .map(|_| "{\"host\": \"127.0.0.1\", \"port\": 8080}")
        .collect::<Vec<_>>()
        .join(", ");
    let mut provider = FixtureNetworkProvider::from_json(&format!(
        "{{\"schema\": \"semaprax.network-fixture.v1\", \"connections\": [{connections}]}}"
    ))
    .unwrap();
    assert_eq!(failure(&eight, &mut provider), 4);

    for op in ["net_recv", "net_stream_stdout"] {
        let tail = format!(
            "    let endpoint = {host};\n    let handle = net_connect(array_as_slice(endpoint), 8080usize);\n    let chunk = {op}(handle, 65537usize);\n    marker == 5usize"
        );
        let mut provider = fixture_provider(8080, &["x"], None);
        assert_eq!(failure(&tail, &mut provider), 4, "{op}");
    }
    let tail = format!(
        "    let endpoint = {host};\n    let handle = net_connect(array_as_slice(endpoint), 8080usize);\n    let state = net_wait(handle, 30001usize);\n    marker == 5usize"
    );
    let mut provider = fixture_provider(8080, &["x"], None);
    assert_eq!(failure(&tail, &mut provider), 4);
}

#[test]
fn capacity_bounds_at_the_edge_are_accepted() {
    let host = byte_array("127.0.0.1");
    let body = format!(
        "    let endpoint = {host};\n    let handle = net_connect(array_as_slice(endpoint), 8080usize);\n    let received = net_recv(handle, 65536usize);\n    let state = net_wait(handle, 30000usize);\n    net_close(handle) == 0usize && byte_len(bytes_as_slice(received)) == 1usize && state == 2usize"
    );
    let mut provider = fixture_provider(8080, &["x"], None);
    let result = run(&program(&body, "127.0.0.1", 8080), &mut provider);
    expect_true(&result);
}

#[test]
fn expected_send_mismatch_is_a_transfer_failure() {
    let mut provider = fixture_provider(8080, &[CHUNK_ONE], Some("PONG\n"));
    let result = run(&program(STREAM_BODY, "127.0.0.1", 8080), &mut provider);
    assert_eq!(failure_code(&result), 5);
}

#[test]
fn denied_provider_reports_authority_denied_and_discards_output() {
    let mut provider = DeniedNetworkProvider;
    let result = run(&program(STREAM_BODY, "127.0.0.1", 8080), &mut provider);
    assert_eq!(failure_code(&result), 6);
}

#[test]
fn network_programs_are_refused_by_the_plain_command_seam() {
    let program = resolved(&program(STREAM_BODY, "127.0.0.1", 8080));
    let refused = semaprax::hosted_interpreter::execute_language_command(
        &program,
        ENTRY,
        &HostedCommandInput::default(),
        1_000,
    );
    assert!(
        refused.is_err(),
        "the effect-free command seam must not run network operations"
    );
}

#[test]
fn hosted_service_profile_executes_tls_and_listen_fixtures() {
    let source = r#"
module net.service;

permit { network.accept, network.connect, network.listen, network.read, network.tls, network.write }

@id("net.run")
fn run() -> bool
    uses { network.accept, network.connect, network.listen, network.read, network.tls, network.write }
{
    let secure_host = [115u8, 101u8, 99u8, 117u8, 114u8, 101u8, 46u8, 101u8, 120u8, 97u8, 109u8, 112u8, 108u8, 101u8];
    let tls = net_tls_connect(array_as_slice(secure_host), 443usize);
    let request = [71u8, 69u8, 84u8];
    let sent = net_send(tls, array_as_slice(request));
    let reply = net_recv(tls, 8usize);
    let tls_closed = net_close(tls);
    let bind_host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let listener = net_listen(array_as_slice(bind_host), 8080usize);
    let peer = net_tls_accept(listener);
    let inbound = net_recv(peer, 8usize);
    let peer_closed = net_close(peer);
    let listener_closed = net_close_listener(listener);
    sent == 3usize && byte_len(bytes_as_slice(reply)) == 2usize && byte_len(bytes_as_slice(inbound)) == 5usize && tls_closed == 0usize && peer_closed == 0usize && listener_closed == 0usize
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let fixture = r#"{
        "schema":"semaprax.network-fixture.v2",
        "connections":[{"host":"secure.example","port":443,"tls":true,"expect_send":"GET","recv":["ok"]}],
        "listeners":[{"host":"127.0.0.1","port":8080,"accept":[{"host":"peer","port":1,"tls":true,"recv":["hello"]}]}]
    }"#;
    let mut provider = FixtureNetworkProvider::from_json(fixture).unwrap();
    let result = run(source, &mut provider);
    expect_true(&result);
}

#[test]
fn hosted_http_profile_executes_a_turnkey_https_get() {
    let source = r#"
module net.https;

permit { network.http }

@id("net.run")
fn run() -> bool
    uses { network.http }
{
    let url = [104u8, 116u8, 116u8, 112u8, 115u8, 58u8, 47u8, 47u8, 101u8, 120u8, 97u8, 109u8, 112u8, 108u8, 101u8, 46u8, 116u8, 101u8, 115u8, 116u8, 47u8, 100u8, 97u8, 116u8, 97u8];
    let response = https_get(array_as_slice(url), 1024usize);
    byte_len(bytes_as_slice(response)) == 46usize
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let mut provider = HttpsFixture;
    expect_true(&run(source, &mut provider));
}

#[test]
fn seam_admission_rejects_foreign_permits_and_non_network_profiles() {
    let source = r#"
module net.plain;

permit { process.stdout.write }

@id("net.run")
fn run() -> bool
    uses { process.stdout.write }
{
    let text = [104u8, 105u8];
    stdout_append(array_as_slice(text)) == 2usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;
    let mut provider = fixture_provider(8080, &[], None);
    let error = execute_network_command(
        &resolved(source),
        ENTRY,
        &HostedCommandInput::default(),
        &mut provider,
        1_000,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-F102");
}

/// Serve one connection: expect `PING`, write the first chunk, expect `MORE`,
/// write the second chunk, close.
fn spawn_two_chunk_server() -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        expect_exact(&mut stream, REQUEST);
        stream.write_all(CHUNK_ONE.as_bytes()).unwrap();
        expect_exact(&mut stream, MORE);
        stream.write_all(CHUNK_TWO.as_bytes()).unwrap();
    });
    (port, server)
}

fn expect_exact(stream: &mut TcpStream, expected: &str) {
    let mut buffer = vec![0u8; expected.len()];
    stream.read_exact(&mut buffer).unwrap();
    assert_eq!(buffer, expected.as_bytes());
}

#[test]
fn tcp_provider_agrees_with_the_fixture_on_the_same_program() {
    let (port, server) = spawn_two_chunk_server();
    let source = program(STREAM_BODY, "127.0.0.1", port);
    let mut tcp = TcpNetworkProvider::new();
    let over_tcp = run(&source, &mut tcp);
    server.join().unwrap();
    expect_true(&over_tcp);

    let mut fixture = fixture_provider(port, &[CHUNK_ONE, CHUNK_TWO], Some(REQUEST));
    let over_fixture = run(&source, &mut fixture);
    expect_true(&over_fixture);

    assert_eq!(over_tcp.stdout, over_fixture.stdout);
    assert_eq!(
        over_tcp.stdout,
        format!("{CHUNK_ONE}{CHUNK_TWO}").as_bytes()
    );
    assert_eq!(over_tcp.stderr, over_fixture.stderr);
}

#[test]
fn tcp_recv_returns_owned_bytes_and_observes_the_peer_closing() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        expect_exact(&mut stream, REQUEST);
        stream.write_all(b"payload").unwrap();
    });
    let mut tcp = TcpNetworkProvider::new();
    let result = run(&program(RECV_BODY, "127.0.0.1", port), &mut tcp);
    server.join().unwrap();
    expect_true(&result);
    assert_eq!(result.stdout, b"payload");
}

#[test]
fn tcp_connect_to_a_closed_port_is_connect_failed() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut tcp = TcpNetworkProvider::new();
    let result = run(&program(LEAK_BODY, "127.0.0.1", port), &mut tcp);
    assert_eq!(failure_code(&result), 1);
}

#[test]
fn settlement_closes_handles_the_program_left_open() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut everything = Vec::new();
        // Returns only once the peer closes; settlement is what closes it.
        stream.read_to_end(&mut everything).unwrap();
        everything
    });
    let mut tcp = TcpNetworkProvider::new();
    let result = run(&program(LEAK_BODY, "127.0.0.1", port), &mut tcp);
    expect_true(&result);
    let observed = server.join().unwrap();
    assert_eq!(observed, REQUEST.as_bytes());
}
