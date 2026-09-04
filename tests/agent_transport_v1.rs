//! Graph Agent Transport v1 executable evidence.
//!
//! Proves the bounded JSON-RPC 2.0 loop end to end: canonical envelopes and
//! known answers, exact payload preservation against the direct library
//! projections, the closed request grammar, hostile framing/params closure,
//! deterministic replay, notification silence, and the shutdown pivot.

use std::io::Write;
use std::process::{Command, Stdio};

use semaprax::agent_transport::{
    Session, TransportLimits, APPLICATION_ERROR, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND,
    PARSE_ERROR, TRANSPORT_SCHEMA,
};

const SOURCE: &str = "module transport.fixture;\n\n@id(\"math.add\")\nfn add(left: i64, right: i64) -> i64\n    requires left >= 0\n    ensures result == left + right\n{\n    left + right\n}\n\n@id(\"app.main\")\nfn main() -> i64\n    ensures result == 42\n{\n    add(19, 23)\n}\n";

struct Fixture {
    directory: std::path::PathBuf,
    source: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-agent-transport-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("fixture.spx");
        std::fs::write(&source, SOURCE).unwrap();
        Self { directory, source }
    }

    fn session(&self) -> Session {
        Session::open(&self.source, TransportLimits::default()).unwrap()
    }

    fn drive(&self, requests: &[&str]) -> Vec<String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve")
            .arg(&self.source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let stdin = child.stdin.as_mut().expect("piped stdin");
            for request in requests {
                stdin.write_all(request.as_bytes()).unwrap();
                stdin.write_all(b"\n").unwrap();
            }
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "serve exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        stdout
            .lines()
            .filter(|line| !line.starts_with("agent transport"))
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

static NEXT_FIXTURE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn protocol_request() -> &'static str {
    r#"{"jsonrpc":"2.0","id":1,"method":"protocol","params":{}}"#
}

#[test]
fn protocol_ping_and_shutdown_have_canonical_envelopes() {
    let fixture = Fixture::new();
    let responses = fixture.drive(&[
        protocol_request(),
        r#"{"jsonrpc":"2.0","id":"alpha","method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":7,"method":"shutdown"}"#,
    ]);
    assert_eq!(responses.len(), 3);
    assert!(responses[0].starts_with(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocol\":\"semaprax.agent-transport.v1\",\"revision\":\"sha256:"
    ));
    assert!(responses[0].contains("\"version\":\"0.3.0\""));
    assert!(responses[0].contains(
        "\"methods\":[\"context\",\"context_v2\",\"graph\",\"ping\",\"protocol\",\"shutdown\"]"
    ));
    assert!(responses[0].contains("\"limits\":{\"max_request_bytes\":65536}"));
    let tail = ["\"bytes\":", &SOURCE.len().to_string(), "}}}"].concat();
    assert!(responses[0].ends_with(&tail));
    assert_eq!(
        responses[1],
        "{\"jsonrpc\":\"2.0\",\"id\":\"alpha\",\"result\":{\"pong\":true}}"
    );
    assert_eq!(
        responses[2],
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}"
    );
}

#[test]
fn protocol_revision_matches_graph_revision_and_source_bytes() {
    let fixture = Fixture::new();
    let program = semaprax::parse(SOURCE, &fixture.source).unwrap();
    let expected_revision = semaprax::graph::revision(&program);
    let mut session = fixture.session();
    let response = session
        .handle_line(protocol_request())
        .expect("protocol response");
    assert!(response.contains(&format!("\"revision\":\"{expected_revision}\"")));
    assert_eq!(session.revision(), expected_revision);
    assert_eq!(session.source_bytes(), SOURCE.len());
}

#[test]
fn graph_result_preserves_exact_library_payload_bytes() {
    let fixture = Fixture::new();
    let program = semaprax::parse(SOURCE, &fixture.source).unwrap();
    let expected_payload = semaprax::graph::to_json(&program).unwrap();
    let mut session = fixture.session();
    let response = session
        .handle_line(r#"{"jsonrpc":"2.0","id":11,"method":"graph"}"#)
        .unwrap();
    let expected_response =
        format!("{{\"jsonrpc\":\"2.0\",\"id\":11,\"result\":{{\"graph\":{expected_payload}}}}}");
    assert_eq!(response, expected_response);
}

#[test]
fn context_results_match_direct_cli_equivalent_calls() {
    let fixture = Fixture::new();
    let program = semaprax::parse(SOURCE, &fixture.source).unwrap();
    let options = semaprax::graph::AgentContextOptions::default();
    let v1_expected = semaprax::graph::agent_context_json(&program, "math.add", &options)
        .unwrap()
        .expect("v1 fact");
    let default_filters = [
        semaprax::graph::AgentContextFilter::Contracts,
        semaprax::graph::AgentContextFilter::Ownership,
        semaprax::graph::AgentContextFilter::Effects,
        semaprax::graph::AgentContextFilter::Types,
    ];
    let v2_options = semaprax::graph::AgentContextV2Options::new(
        2,
        128 * 1024,
        256,
        default_filters,
        semaprax::graph::AgentContextDirection::Reverse,
    )
    .unwrap();
    let v2_expected = semaprax::graph::agent_context_v2_json(&program, "math.add", &v2_options)
        .unwrap()
        .expect("v2 fact");

    let mut session = fixture.session();
    let v1 = session
        .handle_line(
            r#"{"jsonrpc":"2.0","id":21,"method":"context","params":{"symbol":"math.add"}}"#,
        )
        .unwrap();
    assert_eq!(
        v1,
        format!("{{\"jsonrpc\":\"2.0\",\"id\":21,\"result\":{{\"context\":{v1_expected}}}}}")
    );
    let v2 = session
        .handle_line(
            r#"{"jsonrpc":"2.0","id":22,"method":"context_v2","params":{"symbol":"math.add","direction":"reverse","depth":2,"max_bytes":131072}}"#,
        )
        .unwrap();
    assert_eq!(
        v2,
        format!("{{\"jsonrpc\":\"2.0\",\"id\":22,\"result\":{{\"context\":{v2_expected}}}}}")
    );
}

#[test]
fn sessions_replay_identical_bytes_across_repeated_runs() {
    let fixture = Fixture::new();
    let requests = [
        protocol_request(),
        r#"{"jsonrpc":"2.0","id":2,"method":"graph"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"context","params":{"symbol":"app.main"}}"#,
    ];
    let first = fixture.drive(&requests);
    let second = fixture.drive(&requests);
    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
}

#[test]
fn blank_lines_are_silent_and_notifications_never_respond() {
    let fixture = Fixture::new();
    let responses = fixture.drive(&[
        "",
        "   ",
        r#"{"jsonrpc":"2.0","method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"graph"}"#,
        r#"{"jsonrpc":"2.0","method":"bogus"}"#,
        r#"{"jsonrpc":"2.0","method":"shutdown"}"#,
    ]);
    assert_eq!(responses.len(), 1);
    assert!(responses[0].contains("\"id\":4"));
}

#[test]
fn closed_grammar_rejects_every_malformed_frame_deterministically() {
    let fixture = Fixture::new();
    let cases: [(&str, String); 10] = [
        (
            "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]",
            batch_or_invalid_error(),
        ),
        (
            "42",
            invalid_request("invalid request: expected one JSON object per line"),
        ),
        (
            "\"hello\"",
            invalid_request("invalid request: expected one JSON object per line"),
        ),
        (
            "{\"jsonrpc\":\"1.0\",\"id\":1,\"method\":\"ping\"}",
            invalid_request("invalid request: requires jsonrpc \"2.0\" and a string method"),
        ),
        (
            "{\"id\":1,\"method\":\"ping\"}",
            invalid_request("invalid request: requires jsonrpc \"2.0\" and a string method"),
        ),
        (
            "{\"jsonrpc\":\"2.0\",\"id\":true,\"method\":\"ping\"}",
            invalid_request("invalid request: id must be an unsigned integer or bounded string"),
        ),
        (
            "{\"jsonrpc\":\"2.0\",\"id\":1.5,\"method\":\"ping\"}",
            invalid_request("invalid request: id must be an unsigned integer or bounded string"),
        ),
        (
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"extra\":1}",
            invalid_request("invalid request: unknown member"),
        ),
        ("not json", parse_error()),
        (
            "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"nope\"}",
            method_not_found(),
        ),
    ];
    for (request, expected) in cases {
        let mut session = fixture.session();
        assert_eq!(
            session.handle_line(request).as_deref(),
            Some(expected.as_str()),
            "unexpected response for {request}"
        );
        assert!(!session.stop_requested());
    }
}

fn envelope(id: &str, code: i64, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":{}}}}}",
        semaprax::diagnostic::quote_json(message),
    )
}

fn batch_or_invalid_error() -> String {
    envelope(
        "null",
        INVALID_REQUEST,
        "invalid request: expected one JSON object per line",
    )
}

fn invalid_request(message: &str) -> String {
    envelope("null", INVALID_REQUEST, message)
}

fn parse_error() -> String {
    envelope("null", PARSE_ERROR, "parse error")
}

fn method_not_found() -> String {
    envelope("5", METHOD_NOT_FOUND, "method not found: nope")
}

#[test]
fn params_errors_echo_ids_and_stay_closed() {
    let fixture = Fixture::new();
    let cases = [
        (31, "ping", r#"{"x":1}"#, "this method takes no parameters"),
        (32, "graph", "[1]", "params must be absent or an object"),
    ];
    for (id, method, params, message) in cases {
        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}"
        );
        let mut session = fixture.session();
        let response = session.handle_line(&request).unwrap();
        assert_eq!(
            response,
            envelope(
                &id.to_string(),
                INVALID_PARAMS,
                &format!("invalid params: {message}")
            ),
            "unexpected response for {request}"
        );
    }
    let context_cases = [
        (
            33,
            r#"{"symbol":""}"#,
            "symbol must be a nonempty string of at most 256 bytes without control characters",
        ),
        (34, "{\"symbol\":5}", "symbol must be a string"),
        (
            35,
            "{\"symbol\":\"math.add\",\"depth\":-1}",
            "depth must be an unsigned integer",
        ),
        (
            351,
            "{\"symbol\":\"math.add\",\"max_bytes\":\"big\"}",
            "max_bytes must be an unsigned integer",
        ),
        (
            36,
            "{\"symbol\":\"math.add\",\"filters\":[\"nope\"]}",
            "unknown context filter `nope`",
        ),
        (
            37,
            "{\"symbol\":\"math.add\",\"direction\":\"both\"}",
            "direction is only valid for context_v2; resend with method context_v2",
        ),
        (
            40,
            "{\"symbol\":\"math.add\",\"wat\":1}",
            "unknown context parameter `wat`",
        ),
        (
            41,
            "{\"symbol\":\"math.add\",\"max_nodes\":0}",
            "agent context max_nodes 0 is outside 1..=65536",
        ),
        (
            42,
            "{\"symbol\":\"math.add\",\"max_bytes\":1}",
            "agent context max_bytes 1 is outside 1024..=16777216",
        ),
        (
            43,
            "{\"symbol\":\"math.add\",\"filters\":{\"a\":1}}",
            "filters must be an array",
        ),
        (
            44,
            "{\"symbol\":\"math.add\",\"filters\":[\"contracts\",\"contracts\"]}",
            "duplicate context filter `contracts`",
        ),
    ];
    for (id, params, message) in context_cases {
        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"context\",\"params\":{params}}}"
        );
        let mut session = fixture.session();
        let response = session.handle_line(&request).unwrap();
        assert_eq!(
            response,
            envelope(&id.to_string(), INVALID_PARAMS, message),
            "unexpected response for {request}"
        );
    }
    let v2_cases = [
        (
            38,
            r#"{"symbol":"math.add"}"#,
            "context_v2 requires a direction parameter",
        ),
        (
            39,
            r#"{"symbol":"math.add","direction":"sideways"}"#,
            "unknown context direction `sideways`",
        ),
    ];
    for (id, params, message) in v2_cases {
        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"context_v2\",\"params\":{params}}}"
        );
        let mut session = fixture.session();
        let response = session.handle_line(&request).unwrap();
        assert_eq!(
            response,
            envelope(&id.to_string(), INVALID_PARAMS, message),
            "unexpected response for {request}"
        );
    }
}

#[test]
fn oversized_frames_fail_closed_and_stop_the_session() {
    let fixture = Fixture::new();
    let limits = TransportLimits::new(2048).unwrap();
    let oversized = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"context\",\"params\":{{\"symbol\":\"{}\"}}}}",
        "s".repeat(4096),
    );
    assert!(oversized.len() > 2048);
    let mut session = Session::open(&fixture.source, limits).unwrap();
    let response = session.handle_line(&oversized).unwrap();
    assert_eq!(
        response,
        envelope(
            "null",
            PARSE_ERROR,
            "request exceeds agent transport max_request_bytes 2048"
        )
    );
    assert!(session.stop_requested());
    assert_eq!(
        session.handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#),
        None
    );
}

#[test]
fn symbol_not_found_is_a_closed_application_error() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let response = session
        .handle_line(r#"{"jsonrpc":"2.0","id":51,"method":"context","params":{"symbol":"ghost"}}"#)
        .unwrap();
    assert_eq!(
        response,
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":51,\"error\":{{\"code\":{APPLICATION_ERROR},\"message\":\"symbol `ghost` was not found\"}}}}"
        )
    );
}

#[test]
fn transport_limits_bounds_are_exact() {
    assert_eq!(TransportLimits::default().max_request_bytes(), 64 * 1024);
    assert!(TransportLimits::new(1023).is_err());
    assert_eq!(
        TransportLimits::new(1024).unwrap().max_request_bytes(),
        1024
    );
    assert_eq!(
        TransportLimits::new(1024 * 1024)
            .unwrap()
            .max_request_bytes(),
        1024 * 1024
    );
    assert!(TransportLimits::new(1024 * 1024 + 1).is_err());
    let message = TransportLimits::new(99).unwrap_err();
    assert_eq!(
        message,
        "agent transport max_request_bytes 99 is outside 1024..=1048576"
    );
}

#[test]
fn serve_reports_missing_and_unverifiable_sources_before_reading_requests() {
    let missing = Fixture::new();
    let outcome = semaprax::agent_transport::serve(
        &mut std::io::empty(),
        &mut Vec::new(),
        &missing.directory.join("absent.spx"),
        TransportLimits::default(),
    );
    let errors = outcome.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-I001");

    let broken_directory = std::env::temp_dir().join(format!(
        "semaprax-agent-transport-broken-{}",
        std::process::id()
    ));
    std::fs::create_dir(&broken_directory).unwrap();
    let broken = broken_directory.join("broken.spx");
    std::fs::write(&broken, "fn (").unwrap();
    let errors = semaprax::agent_transport::serve(
        &mut std::io::empty(),
        &mut Vec::new(),
        &broken,
        TransportLimits::default(),
    )
    .unwrap_err();
    assert!(!errors.is_empty());
    std::fs::remove_dir_all(&broken_directory).unwrap();
}

#[test]
fn serve_loop_answers_each_line_and_pivots_on_shutdown() {
    let fixture = Fixture::new();
    let requests = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
        "\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"shutdown\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n",
    );
    let mut input = std::io::BufReader::new(requests.as_bytes());
    let mut sink = Vec::new();
    let outcome = semaprax::agent_transport::serve(
        &mut input,
        &mut sink,
        &fixture.source,
        TransportLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome.responses, 2);
    assert!(outcome.stopped_by_shutdown);
    let stdout = String::from_utf8(sink).unwrap();
    assert_eq!(
        stdout,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"pong\":true}}\n\
         {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\n"
    );
}

#[test]
fn serve_bounds_buffering_for_frames_without_a_newline() {
    let fixture = Fixture::new();
    // One hostile frame far above the declared maximum whose only newline is
    // its terminator; the transport must bound its buffering, drain the
    // frame, and report the exact declared-limit failure once.
    let mut requests =
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"context\",\"params\":{\"symbol\":\"".to_owned();
    requests.push_str(&"s".repeat(300_000));
    requests.push_str("\"}}\n");
    let mut input = std::io::BufReader::new(requests.as_bytes());
    let mut sink = Vec::new();
    let outcome = semaprax::agent_transport::serve(
        &mut input,
        &mut sink,
        &fixture.source,
        TransportLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome.responses, 1);
    assert!(outcome.stopped_by_shutdown);
    let stdout = String::from_utf8(sink).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(
        stdout.contains("request exceeds agent transport max_request_bytes 65536"),
        "{stdout}"
    );
}

#[test]
fn serve_at_end_of_file_is_a_clean_stop_without_shutdown() {
    let fixture = Fixture::new();
    let mut input =
        std::io::BufReader::new("{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n".as_bytes());
    let mut sink = Vec::new();
    let outcome = semaprax::agent_transport::serve(
        &mut input,
        &mut sink,
        &fixture.source,
        TransportLimits::default(),
    )
    .unwrap();
    assert_eq!(outcome.responses, 1);
    assert!(!outcome.stopped_by_shutdown);
}

#[test]
fn transport_schema_constant_is_frozen() {
    assert_eq!(TRANSPORT_SCHEMA, "semaprax.agent-transport.v1");
}
