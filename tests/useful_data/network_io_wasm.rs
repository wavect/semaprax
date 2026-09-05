//! WebAssembly lane of Bounded Language Network I/O v1.
//!
//! The lane appends seven closed `env` imports after the frozen command
//! imports, exports `__spx_network_status_v1`, and lowers the six network
//! operations with the same fail-closed provider discipline as command input.
//! A Node facade (`network_io_wasm_facade.mjs`) serves a
//! `semaprax.network-fixture.v1` document as the provider: a synchronous Wasm
//! import cannot block on Node's asynchronous sockets, so the facade opens
//! none, and real sockets on Wasm stay out of scope for v1.

use std::path::Path;
use std::process::Command;

use semaprax::{parse, wasm};

const FACADE: &str = include_str!("network_io_wasm_facade.mjs");

const STREAM_SOURCE: &str = r#"module test.network_wasm_stream;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("test.network_wasm_stream.run")
fn run() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
    let host = [104u8, 111u8, 115u8, 116u8];
    let handle = net_connect(array_as_slice(host), 80usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let first = net_stream_stdout(handle, 4096usize);
    let second = net_stream_stdout(handle, 4096usize);
    let tail = net_stream_stdout(handle, 4096usize);
    net_close(handle) == 0usize && sent == 5usize && first == 7usize && second == 6usize && tail == 0usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;

const RECV_SOURCE: &str = r#"module test.network_wasm_recv;

permit { network.connect, network.read, process.stdout.write }

@id("test.network_wasm_recv.run")
fn run() -> bool
    uses { network.connect, network.read, process.stdout.write }
{
    let host = [104u8, 111u8, 115u8, 116u8];
    let handle = net_connect(array_as_slice(host), 443usize);
    let chunk = net_recv(handle, 5usize);
    let appended = stdout_append(bytes_as_slice(chunk));
    let rest = net_recv(handle, 64usize);
    let more = stdout_append(bytes_as_slice(rest));
    let end = net_recv(handle, 64usize);
    let closed = net_close(handle);
    appended == 5usize && more == 6usize && byte_len(bytes_as_slice(end)) == 0usize && closed == 0usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;

const WAIT_SOURCE: &str = r#"module test.network_wasm_wait;

permit { network.connect, network.read, process.stdout.write }

@id("test.network_wasm_wait.run")
fn run() -> bool
    uses { network.connect, network.read, process.stdout.write }
{
    let host = [104u8, 111u8, 115u8, 116u8];
    let handle = net_connect(array_as_slice(host), 9usize);
    let first = net_wait(handle, 10usize);
    let second = net_wait(handle, 10usize);
    let chunk = net_recv(handle, 16usize);
    let appended = stdout_append(bytes_as_slice(chunk));
    let third = net_wait(handle, 30000usize);
    let end = net_recv(handle, 16usize);
    first == 0usize && second == 1usize && appended == 1usize && third == 2usize && byte_len(bytes_as_slice(end)) == 0usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;

const STREAM_FIXTURE: &str = r#"{"schema":"semaprax.network-fixture.v1","connections":[{"host":"host","port":80,"recv":["hello, ","world\n"],"expect_send":"PING\n","ready":true}]}"#;
const RECV_FIXTURE: &str = r#"{"schema":"semaprax.network-fixture.v1","connections":[{"host":"host","port":443,"recv":["hello world"]}]}"#;
const WAIT_FIXTURE: &str = r#"{"schema":"semaprax.network-fixture.v1","connections":[{"host":"host","port":9,"recv":["x"],"ready":false}]}"#;

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn emit(source: &str, name: &str, command_id: &str) -> Vec<u8> {
    let program = parse(source, Path::new(name)).unwrap();
    let bytes = wasm::emit_language_network_io_v1(&program, command_id).unwrap();
    assert_eq!(
        bytes,
        wasm::emit_language_network_io_v1(&program, command_id).unwrap(),
        "network-lane Wasm bytes must be deterministic"
    );
    bytes
}

fn export_symbol(command_id: &str) -> String {
    let mut symbol = String::from("spx_data_");
    for byte in command_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

/// Run the facade and return its `validate` flag and result record.
fn run_facade(
    bytes: &[u8],
    command_id: &str,
    fixture: Option<&str>,
    hostile: Option<&str>,
) -> (bool, Vec<String>) {
    let root = std::env::temp_dir().join(format!(
        "spx-network-io-wasm-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("app.wasm"), bytes).unwrap();
    std::fs::write(root.join("facade.mjs"), FACADE).unwrap();
    let fixture_argument = match fixture {
        Some(fixture) => {
            std::fs::write(root.join("fixture.json"), fixture).unwrap();
            "fixture.json".to_owned()
        }
        None => "-".to_owned(),
    };
    let output = Command::new("node")
        .arg("facade.mjs")
        .arg("app.wasm")
        .arg(export_symbol(command_id))
        .arg(fixture_argument)
        .arg(hostile.unwrap_or("-"))
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    let validate = lines.next().unwrap();
    assert!(
        validate == "validate 1" || validate == "validate 0",
        "{text}"
    );
    let record = lines
        .next()
        .unwrap_or_else(|| panic!("facade printed no result record: {text}"))
        .split(' ')
        .map(str::to_owned)
        .collect();
    (validate == "validate 1", record)
}

fn assert_success(record: &[String], stdout: &[u8]) {
    let expected = stdout
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let expected = if expected.is_empty() {
        "-".to_owned()
    } else {
        expected
    };
    assert_eq!(
        record,
        ["ok", "1", expected.as_str(), "-", "0", "0", "0"],
        "result true, exact stdout, empty stderr, settled arena and handles, clean staging"
    );
}

fn assert_normalized_failure(record: &[String], code: u32) {
    let code = code.to_string();
    assert_eq!(
        record,
        ["failure", code.as_str(), code.as_str(), "0", "0", "0", "0", "0"],
        "sticky status, network marker, no input marker, empty transcripts, settled arena, clean memory"
    );
}

fn assert_invariant_failure(record: &[String]) {
    assert_eq!(record[0], "invariant", "{record:?}");
    assert_eq!(
        &record[1..],
        ["0", "0", "0", "0", "0", "0"],
        "no domain marker, empty transcripts, settled arena, clean memory: {record:?}"
    );
}

#[test]
fn stream_stdout_delivers_fixture_chunks_into_the_published_transcript() {
    let bytes = emit(
        STREAM_SOURCE,
        "network-wasm-stream.spx",
        "test.network_wasm_stream.run",
    );
    assert!(bytes
        .windows(22)
        .any(|window| window == b"spx_network_connect_v1"));
    if !node_available() {
        return;
    }
    let (valid, record) = run_facade(
        &bytes,
        "test.network_wasm_stream.run",
        Some(STREAM_FIXTURE),
        None,
    );
    assert!(valid, "WebAssembly.validate must accept the module");
    assert_success(&record, b"hello, world\n");
}

#[test]
fn recv_owns_each_chunk_and_the_program_appends_it_exactly() {
    let bytes = emit(
        RECV_SOURCE,
        "network-wasm-recv.spx",
        "test.network_wasm_recv.run",
    );
    if !node_available() {
        return;
    }
    let (valid, record) = run_facade(
        &bytes,
        "test.network_wasm_recv.run",
        Some(RECV_FIXTURE),
        None,
    );
    assert!(valid);
    assert_success(&record, b"hello world");
}

#[test]
fn wait_reports_one_timeout_then_readable_then_peer_closed() {
    let bytes = emit(
        WAIT_SOURCE,
        "network-wasm-wait.spx",
        "test.network_wasm_wait.run",
    );
    if !node_available() {
        return;
    }
    let (valid, record) = run_facade(
        &bytes,
        "test.network_wasm_wait.run",
        Some(WAIT_FIXTURE),
        None,
    );
    assert!(valid);
    assert_success(&record, b"x");
}

#[test]
fn every_normalized_failure_code_is_sticky_marked_and_discards_the_transcript() {
    if !node_available() {
        return;
    }
    let command = "test.network_wasm_stream.run";
    let stream = emit(STREAM_SOURCE, "network-wasm-failures.spx", command);

    // 1 CONNECT_FAILED: the fixture's next connection is another endpoint.
    let other = STREAM_FIXTURE.replace("\"host\":\"host\"", "\"host\":\"other\"");
    assert_normalized_failure(&run_facade(&stream, command, Some(&other), None).1, 1);

    // 2 INVALID_ENDPOINT: port zero is rejected before any provider call.
    let port_zero = emit(
        &STREAM_SOURCE.replace("80usize", "0usize"),
        "network-wasm-port-zero.spx",
        command,
    );
    assert_normalized_failure(
        &run_facade(&port_zero, command, Some(STREAM_FIXTURE), None).1,
        2,
    );

    // 3 UNKNOWN_HANDLE: closing twice after both chunks were staged; the
    // staged transcript must not survive the failure.
    let close_twice = emit(
        &STREAM_SOURCE.replace(
            "net_close(handle) == 0usize && sent",
            "net_close(handle) == 0usize && net_close(handle) == 0usize && sent",
        ),
        "network-wasm-close-twice.spx",
        command,
    );
    assert_normalized_failure(
        &run_facade(&close_twice, command, Some(STREAM_FIXTURE), None).1,
        3,
    );
    // Also a forged handle outside 1..=8, rejected before the provider.
    let forged = emit(
        &STREAM_SOURCE.replace("net_close(handle) == 0usize", "net_close(9usize) == 0usize"),
        "network-wasm-forged-handle.spx",
        command,
    );
    assert_normalized_failure(
        &run_facade(&forged, command, Some(STREAM_FIXTURE), None).1,
        3,
    );

    // 4 CAPACITY_EXCEEDED: a chunk bound over 65,536 on the last stream call.
    let over = emit(
        &STREAM_SOURCE.replace(
            "let tail = net_stream_stdout(handle, 4096usize);",
            "let tail = net_stream_stdout(handle, 65537usize);",
        ),
        "network-wasm-chunk-over.spx",
        command,
    );
    assert_normalized_failure(&run_facade(&over, command, Some(STREAM_FIXTURE), None).1, 4);

    // 5 TRANSFER_FAILED: the program's bytes disagree with `expect_send`.
    let pong = STREAM_FIXTURE.replace("PING", "PONG");
    assert_normalized_failure(&run_facade(&stream, command, Some(&pong), None).1, 5);

    // 6 AUTHORITY_DENIED: the invocation was given no provider at all.
    assert_normalized_failure(&run_facade(&stream, command, None, None).1, 6);
}

#[test]
fn hostile_provider_answers_take_the_fail_stop_edge_without_leaking_bytes() {
    if !node_available() {
        return;
    }
    let command = "test.network_wasm_stream.run";
    let stream = emit(STREAM_SOURCE, "network-wasm-hostile.spx", command);
    for mode in ["status-7", "handle-9", "count-over-max"] {
        let (_, record) = run_facade(&stream, command, Some(STREAM_FIXTURE), Some(mode));
        assert_invariant_failure(&record);
    }
    let recv = emit(
        RECV_SOURCE,
        "network-wasm-hostile-recv.spx",
        "test.network_wasm_recv.run",
    );
    let (_, record) = run_facade(
        &recv,
        "test.network_wasm_recv.run",
        Some(RECV_FIXTURE),
        Some("bad-token"),
    );
    assert_invariant_failure(&record);
}

#[test]
fn network_lane_admission_requires_a_network_permit_and_a_reachable_network_op() {
    let no_permit = r#"module test.network_wasm_no_permit;

permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }

@id("test.network_wasm_no_permit.run")
fn run() -> bool
    uses { process.stdin.read, process.stdout.write }
{
    let input = stdin_read();
    stdout_append(bytes_as_slice(input)) == 0usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;
    let program = parse(no_permit, Path::new("network-wasm-no-permit.spx")).unwrap();
    let error = wasm::emit_language_network_io_v1(&program, "test.network_wasm_no_permit.run")
        .expect_err("a module without network permits is not a network command");
    assert_eq!(error.code, "SPX-W114", "{error:?}");

    let unreached = STREAM_SOURCE.replace(
        "@id(\"main\")\nfn main() -> i64\n{\n    0\n}",
        "@id(\"test.network_wasm_stream.plain\")\nfn plain() -> bool\n{\n    true\n}\n\n@id(\"main\")\nfn main() -> i64\n{\n    0\n}",
    );
    let program = parse(&unreached, Path::new("network-wasm-unreached.spx")).unwrap();
    let error = wasm::emit_language_network_io_v1(&program, "test.network_wasm_stream.plain")
        .expect_err("the selected command must reach a network operation");
    assert_eq!(error.code, "SPX-W114", "{error:?}");
}
