//! Native C11 execution of Bounded Language Network I/O v1 against a loopback listener.
//!
//! Each executable case binds a `TcpListener` on `127.0.0.1:0`, embeds the
//! chosen port into the program before emission, compiles the generated C with
//! clang, and drives the binary either through the shared process adapter or
//! through a tiny harness that prints the normalized status of the runner.
//! Cases that need clang skip cleanly when it is unavailable, like the sibling
//! native command tests.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use semaprax::{codegen, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const PORT_PLACEHOLDER: &str = "__PORT__";

/// Connect, send `PING\n`, stream the two-part response into stdout, observe
/// end of stream twice (a zero-length stream and a `net_wait` of 2), close.
const STREAM_SOURCE: &str = r#"
module test.network_stream;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("network-stream.run")
fn run() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), __PORT__usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let first = net_stream_stdout(handle, 4096usize);
    let second = net_stream_stdout(handle, 4096usize);
    let third = net_stream_stdout(handle, 4096usize);
    let waited = net_wait(handle, 1000usize);
    let closed = net_close(handle);
    closed == 0usize && sent == 5usize && first > 0usize && second > 0usize && third == 0usize && waited == 2usize
}

@id("main")
fn main() -> i64 { 0 }
"#;

/// Connect, send, wait for readability (1), receive one owned chunk, and
/// append it to stdout through the ordinary append operation.
const RECV_SOURCE: &str = r#"
module test.network_recv;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("network-recv.run")
fn run() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), __PORT__usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let ready = net_wait(handle, 5000usize);
    let received = net_recv(handle, 4096usize);
    let view = bytes_as_slice(received);
    let appended = stdout_append(view);
    let closed = net_close(handle);
    closed == 0usize && sent == 5usize && ready == 1usize && appended == byte_len(view)
}

@id("main")
fn main() -> i64 { 0 }
"#;

/// The peer closes without answering: `net_wait` reports 2 and the program
/// exposes that through its boolean result.
const WAIT_CLOSED_SOURCE: &str = r#"
module test.network_wait_closed;

permit { network.connect, network.read, network.write }

@id("network-wait.run")
fn run() -> bool
    uses { network.connect, network.read, network.write }
{
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), __PORT__usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let waited = net_wait(handle, 5000usize);
    let closed = net_close(handle);
    closed == 0usize && sent == 5usize && waited == 2usize
}

@id("main")
fn main() -> i64 { 0 }
"#;

/// A handle no connect issued.
const FORGED_HANDLE_SOURCE: &str = r#"
module test.network_forged;

permit { network.connect }

@id("network-forged.run")
fn run() -> bool
    uses { network.connect }
{
    net_close(7usize) == 0usize
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str, name: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new(name)).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn with_port(source: &str, port: u16) -> String {
    assert!(source.contains(PORT_PLACEHOLDER));
    source.replace(PORT_PLACEHOLDER, &port.to_string())
}

fn clang_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

struct Compiled {
    source: PathBuf,
    executable: PathBuf,
}

impl Compiled {
    fn build(generated: &str, label: &str, extra: &[&str]) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-network-command-{}-{id}-{label}",
            std::process::id()
        );
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, generated).unwrap();
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .args(extra)
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        Self { source, executable }
    }

    fn run(&self) -> std::process::Output {
        Command::new(&self.executable).output().unwrap()
    }
}

impl Drop for Compiled {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.source);
        let _ = std::fs::remove_file(&self.executable);
    }
}

/// A C harness that bypasses the process adapter and prints the runner's
/// normalized outcome, so tests can observe exact status codes.
const HARNESS_MAIN_C: &str = r#"
int main(void) {
    struct spx_language_command_input_v1 input;
    memset(&input, 0, sizeof(input));
    struct spx_language_command_result_v1 *result =
        (struct spx_language_command_result_v1 *)malloc(sizeof(*result));
    if (result == NULL) return 3;
    int ok = spx_language_command_run_v1(&input, result);
    printf(
        "SPXNET ok=%d success=%d matched=%d domain=%s code=%u stdout_len=%llu\n",
        ok,
        (int)result->semantic_success,
        (int)result->matched,
        result->status_domain[0] != '\0' ? result->status_domain : "-",
        (unsigned)result->status_code,
        (unsigned long long)result->stdout_length
    );
    fflush(stdout);
    if (result->stdout_length != 0) {
        fwrite(result->stdout_bytes, 1, (size_t)result->stdout_length, stdout);
    }
    free(result);
    return 0;
}
"#;

#[derive(Debug, PartialEq, Eq)]
struct HarnessOutcome {
    ok: bool,
    success: bool,
    matched: bool,
    domain: String,
    code: u32,
    stdout: Vec<u8>,
}

fn run_harness(generated: &str, label: &str) -> HarnessOutcome {
    let mut source = generated.to_owned();
    source.push_str(HARNESS_MAIN_C);
    let compiled = Compiled::build(
        &source,
        label,
        &[
            "-O0",
            "-DSPX_NO_LANGUAGE_COMMAND_PROCESS_ADAPTER",
            "-Wno-unused-function",
        ],
    );
    let output = compiled.run();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let newline = output
        .stdout
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("harness header line");
    let header = std::str::from_utf8(&output.stdout[..newline]).unwrap();
    let mut fields = header.split(' ');
    assert_eq!(fields.next(), Some("SPXNET"));
    let mut field = |name: &str| -> String {
        let entry = fields.next().unwrap_or_else(|| panic!("missing {name}"));
        let (key, value) = entry.split_once('=').unwrap();
        assert_eq!(key, name);
        value.to_owned()
    };
    let ok = field("ok") == "1";
    let success = field("success") == "1";
    let matched = field("matched") == "1";
    let domain = field("domain");
    let code = field("code").parse::<u32>().unwrap();
    let stdout_len = field("stdout_len").parse::<usize>().unwrap();
    let stdout = output.stdout[newline + 1..].to_vec();
    assert_eq!(stdout.len(), stdout_len);
    HarnessOutcome {
        ok,
        success,
        matched,
        domain,
        code,
        stdout,
    }
}

/// Accept one connection, read exactly `request_len` bytes, write each
/// response part with a short pause between them, then close. Returns the
/// bytes the client sent.
fn spawn_server(request_len: usize, parts: Vec<Vec<u8>>) -> (u16, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut request = vec![0u8; request_len];
        stream.read_exact(&mut request).unwrap();
        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                std::thread::sleep(Duration::from_millis(80));
            }
            stream.write_all(part).unwrap();
            stream.flush().unwrap();
        }
        drop(stream);
        request
    });
    (port, handle)
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[test]
fn network_profile_emission_is_deterministic_and_self_contained() {
    let program = resolved(&with_port(STREAM_SOURCE, 8080), "network-stream.spx");
    let first = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let second = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    assert_eq!(first, second);
    for required in [
        "spx_language_command_run_v1",
        "spx_network_settle_v1",
        "spx_host_net_connect_v1",
        "spx_host_net_send_v1",
        "spx_host_net_stream_stdout_v1",
        "spx_host_net_wait_v1",
        "spx_host_net_close_v1",
        "#define SPX_NETWORK_STATUS_DOMAIN_V1 \"semaprax.network.v1\"",
        "#define SPX_NETWORK_MAX_HANDLES_V1 UINT64_C(8)",
        "#define SPX_NETWORK_MAX_TOTAL_BYTES_V1 UINT64_C(1048576)",
        "int main(int argc, char **argv)",
        "int wmain(int argc, wchar_t **argv)",
    ] {
        assert!(first.contains(required), "missing {required}");
    }
    assert!(!first.contains("getenv("));
    // Sockets are created only inside the network helpers; the runner
    // withdraws authority and closes every handle after the command returns.
    let settle_calls = first
        .matches("spx_network_settle_v1(&state.network);")
        .count();
    assert_eq!(
        settle_calls, 2,
        "settlement on the init-failure and ordinary paths"
    );
}

#[test]
fn network_profile_rejects_wrong_shapes_and_language_lane_rejects_network_ops() {
    // The language lane exposes the shared SPX-W114 profile diagnostic when a
    // network operation is reachable without the network profile. The permit
    // set is exactly the language-command inventory, so this reaches profile
    // validation rather than the permit check; verification is skipped because
    // the effect rules would already refuse the missing network permit.
    let unpermitted = r#"
module test.network_unpermitted;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("command.run")
fn run() -> bool uses { network.connect } {
    net_close(1usize) == 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;
    let ast = parse(unpermitted, Path::new("network-unpermitted.spx")).unwrap();
    if let Ok(program) = hir::resolve(&ast) {
        let language = codegen::emit_hir_c_with_language_command_io(&program, "command.run")
            .expect_err("language lane must refuse network operations");
        assert_eq!(language.code, "SPX-W114", "{language:?}");
        let network = codegen::emit_hir_c_with_network_io(&program, "command.run")
            .expect_err("network lane requires a network permit");
        assert!(
            network.message.contains("at least one network permit"),
            "{network:?}"
        );
    }

    // A language-command module never receives socket text.
    let language_only = resolved(
        r#"
module test.language_only;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("command.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view) == byte_len(view)
}
@id("main") fn main() -> i64 { 0 }
"#,
        "language-only.spx",
    );
    let generated =
        codegen::emit_hir_c_with_language_command_io(&language_only, "command.run").unwrap();
    for forbidden in [
        "socket",
        "spx_host_net_",
        "SPX_NETWORK_",
        "getaddrinfo",
        "<poll.h>",
    ] {
        assert!(!generated.contains(forbidden), "found {forbidden}");
    }
    let refused = codegen::emit_hir_c_with_network_io(&language_only, "command.run")
        .expect_err("network lane requires a network permit");
    assert!(refused.message.contains("network permit"), "{refused:?}");

    // The network lane still demands the NetworkV1 profile: a network-permitted
    // module whose command reaches no network operation is refused.
    let idle = resolved(
        r#"
module test.network_idle;
permit { network.connect, process.stdout.write }
@id("idle.run")
fn run() -> bool uses { process.stdout.write } {
    let banner = [111u8, 107u8];
    stdout_append(array_as_slice(banner)) == 2usize
}
@id("main") fn main() -> i64 { 0 }
"#,
        "network-idle.spx",
    );
    let idle_error = codegen::emit_hir_c_with_network_io(&idle, "idle.run").unwrap_err();
    assert_eq!(idle_error.code, "SPX-W114", "{idle_error:?}");

    let program = resolved(&with_port(STREAM_SOURCE, 8080), "network-stream.spx");
    let absent = codegen::emit_hir_c_with_network_io(&program, "missing.run").unwrap_err();
    assert!(absent.message.contains("is absent"), "{absent:?}");
    let not_command = codegen::emit_hir_c_with_network_io(&program, "main").unwrap_err();
    assert!(
        not_command
            .message
            .contains("explicit stable-ID `fn () -> bool`"),
        "{not_command:?}"
    );
}

#[test]
fn native_stream_stdout_round_trips_a_two_part_response() {
    if !clang_available() {
        return;
    }
    for optimization in ["-O0", "-O2"] {
        let (port, server) = spawn_server(5, vec![b"HELLO, ".to_vec(), b"WORLD".to_vec()]);
        let program = resolved(&with_port(STREAM_SOURCE, port), "network-stream.spx");
        let generated =
            codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
        let compiled = Compiled::build(
            &generated,
            &format!("stream{optimization}"),
            &[optimization],
        );
        let output = compiled.run();
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"HELLO, WORLD");
        assert!(output.stderr.is_empty());
        assert_eq!(server.join().unwrap(), b"PING\n");
    }
}

#[test]
fn native_recv_appends_owned_bytes_after_wait_reports_readable() {
    if !clang_available() {
        return;
    }
    let (port, server) = spawn_server(5, vec![b"pong".to_vec()]);
    let program = resolved(&with_port(RECV_SOURCE, port), "network-recv.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-recv.run").unwrap();
    assert!(generated.contains("spx_host_net_recv_v1"));
    let compiled = Compiled::build(&generated, "recv", &["-O0"]);
    let output = compiled.run();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"pong");
    assert_eq!(server.join().unwrap(), b"PING\n");
}

#[test]
fn native_wait_reports_peer_close_through_the_program_result() {
    if !clang_available() {
        return;
    }
    let (port, server) = spawn_server(5, Vec::new());
    let program = resolved(&with_port(WAIT_CLOSED_SOURCE, port), "network-wait.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-wait.run").unwrap();
    let outcome = run_harness(&generated, "wait-closed");
    assert_eq!(server.join().unwrap(), b"PING\n");
    assert_eq!(
        outcome,
        HarnessOutcome {
            ok: true,
            success: true,
            matched: true,
            domain: "-".to_owned(),
            code: 0,
            stdout: Vec::new(),
        }
    );
}

#[test]
fn native_failures_normalize_to_the_network_status_domain() {
    if !clang_available() {
        return;
    }
    // Connection refused: the process adapter reports the failure the way the
    // language-command adapter does and publishes nothing.
    let refused_port = closed_port();
    let program = resolved(
        &with_port(STREAM_SOURCE, refused_port),
        "network-stream.spx",
    );
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let adapter = Compiled::build(&generated, "refused-adapter", &["-O0"]);
    let output = adapter.run();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"SEMAPRAX language command failed\n");
    let refused = run_harness(&generated, "refused");
    assert!(refused.ok);
    assert!(!refused.success);
    assert_eq!(refused.domain, "semaprax.network.v1");
    assert_eq!(refused.code, 1, "CONNECT_FAILED");
    assert!(refused.stdout.is_empty());

    // Port 0 is an invalid endpoint before any socket exists.
    let program = resolved(&with_port(STREAM_SOURCE, 0), "network-stream.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let invalid = run_harness(&generated, "port-zero");
    assert_eq!(invalid.domain, "semaprax.network.v1");
    assert_eq!(invalid.code, 2, "INVALID_ENDPOINT");
    assert!(invalid.stdout.is_empty());

    // A forged handle is unknown.
    let program = resolved(FORGED_HANDLE_SOURCE, "network-forged.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-forged.run").unwrap();
    let forged = run_harness(&generated, "forged");
    assert_eq!(forged.domain, "semaprax.network.v1");
    assert_eq!(forged.code, 3, "UNKNOWN_HANDLE");
    assert!(forged.stdout.is_empty());
}
