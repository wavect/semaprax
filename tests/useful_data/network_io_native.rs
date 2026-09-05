//! Native C11 execution of Bounded Language Network I/O v1 against a loopback listener.
//!
//! Each executable case binds a `TcpListener` on `127.0.0.1:0`, embeds the
//! chosen port into the program before emission, compiles the generated C with
//! clang, and drives the binary either through the shared process adapter or
//! through a tiny harness that prints the normalized status of the runner.
//! Cases that need clang skip cleanly when it is unavailable, like the sibling
//! native command tests.

use std::collections::HashMap;
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

/// Connect, send, and read once with no readiness wait, so an interrupted
/// `recv` is the only thing that can decide the outcome.
const RECV_ONLY_SOURCE: &str = r#"
module test.network_recv_only;

permit { network.connect, network.read, network.write }

@id("network-recv-only.run")
fn run() -> bool
    uses { network.connect, network.read, network.write }
{
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), __PORT__usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let received = net_recv(handle, 4096usize);
    let view = bytes_as_slice(received);
    let length = byte_len(view);
    let closed = net_close(handle);
    closed == 0usize && sent == 5usize && length == 0usize
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

/// A Windows cross compiler, when the machine has one.
fn windows_cross_compiler() -> Option<&'static str> {
    ["x86_64-w64-mingw32-gcc", "i686-w64-mingw32-gcc"]
        .into_iter()
        .find(|tool| Command::new(tool).arg("--version").output().is_ok())
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
            // The network profile owns a resolver worker where POSIX threads
            // exist, so the profile's translation unit links the platform
            // thread library.
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"])
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
    run_harness_with(generated, label, "", &[])
}

/// `run_harness`, plus C appended ahead of the harness `main` and extra
/// compiler flags. The appended text is how a case injects a fault: it lands
/// in the same translation unit as the emitted adapter and defines the libc
/// symbol the adapter calls, so the *shipped* text is what runs.
fn run_harness_with(
    generated: &str,
    label: &str,
    injected: &str,
    extra: &[&str],
) -> HarnessOutcome {
    let mut source = generated.to_owned();
    source.push_str(injected);
    source.push_str(HARNESS_MAIN_C);
    let mut flags = vec![
        "-O0",
        "-DSPX_NO_LANGUAGE_COMMAND_PROCESS_ADAPTER",
        "-Wno-unused-function",
    ];
    flags.extend_from_slice(extra);
    let compiled = Compiled::build(&source, label, &flags);
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

/// Compile the emitted translation unit with an appended probe `main` and
/// return the `key=value` fields it prints.
///
/// A probe calls the profile's own static helpers directly. That is deliberate:
/// the aggregate deadline is thirty seconds, so driving a budget boundary
/// through a whole program would mean a thirty-second test. A probe selects a
/// short deadline instead, and reaches the boundary in milliseconds.
fn run_probe(generated: &str, label: &str, probe: &str) -> HashMap<String, String> {
    let mut source = generated.to_owned();
    source.push_str(probe);
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
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    text.split_whitespace()
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn probe_number(fields: &HashMap<String, String>, key: &str) -> u64 {
    fields
        .get(key)
        .unwrap_or_else(|| panic!("probe printed no {key}: {fields:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("probe field {key} is not a number: {fields:?}"))
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

/// A numeric endpoint and a spent budget, with the platform name service
/// untouched: the numeric answer needs neither budget nor worker, and a name
/// is refused before any worker starts.
const NUMERIC_RESOLUTION_PROBE_C: &str = r#"
int main(void) {
    struct spx_network_deadline_v1 spent;
    spent.expires_at_millis = spx_network_monotonic_millis_v1();
    struct addrinfo *answer = NULL;
    bool numeric = spx_network_resolve_bounded_v1("127.0.0.1", "8080", spent, &answer);
    if (answer != NULL) freeaddrinfo(answer);
    uint64_t after_numeric = spx_network_reap_resolvers_v1();
    answer = NULL;
    bool named = spx_network_resolve_bounded_v1("localhost", "8080", spent, &answer);
    if (answer != NULL) freeaddrinfo(answer);
    uint64_t after_named = spx_network_reap_resolvers_v1();
    printf(
        "numeric=%d after_numeric=%llu named=%d after_named=%llu\n",
        (int)numeric,
        (unsigned long long)after_numeric,
        (int)named,
        (unsigned long long)after_named
    );
    return 0;
}
"#;

/// A name service that always costs 300 ms. The numeric probe is refused so
/// the owned-worker path is the one under test, and the emitted adapter's own
/// call is what this definition answers.
const SLOW_RESOLUTION_PROBE_C: &str = r#"
int getaddrinfo(
    const char *node,
    const char *service,
    const struct addrinfo *hints,
    struct addrinfo **res
) {
    (void)node;
    (void)service;
    *res = NULL;
    if (hints != NULL && (hints->ai_flags & AI_NUMERICHOST) != 0) return EAI_NONAME;
    struct timespec nap = { 0, 300L * 1000000L };
    (void)nanosleep(&nap, NULL);
    return EAI_FAIL;
}

int main(void) {
    struct spx_network_deadline_v1 deadline;
    deadline.expires_at_millis = spx_network_monotonic_millis_v1() + UINT64_C(30);
    uint64_t started = spx_network_monotonic_millis_v1();
    struct addrinfo *answer = NULL;
    bool resolved = spx_network_resolve_bounded_v1("slow.invalid", "80", deadline, &answer);
    uint64_t elapsed = spx_network_monotonic_millis_v1() - started;
    if (answer != NULL) freeaddrinfo(answer);
    uint64_t owned = spx_network_reap_resolvers_v1();
    for (int attempt = 0; attempt < 400; ++attempt) {
        if (spx_network_reap_resolvers_v1() == UINT64_C(0)) break;
        struct timespec nap = { 0, 10L * 1000000L };
        (void)nanosleep(&nap, NULL);
    }
    printf(
        "resolved=%d elapsed=%llu owned=%llu reaped=%llu\n",
        (int)resolved,
        (unsigned long long)elapsed,
        (unsigned long long)owned,
        (unsigned long long)spx_network_reap_resolvers_v1()
    );
    return 0;
}
"#;

/// An interrupted read. A negative budget interrupts every call; a positive
/// one interrupts that many times and then delegates to the real kernel path.
const EINTR_READ_PROBE_C: &str = r#"
static int spx_probe_eintr_budget = 0;

ssize_t recv(int fd, void *buffer, size_t length, int flags) {
    if (spx_probe_eintr_budget != 0) {
        if (spx_probe_eintr_budget > 0) spx_probe_eintr_budget -= 1;
        struct timespec nap = { 0, 20L * 1000000L };
        (void)nanosleep(&nap, NULL);
        errno = EINTR;
        return (ssize_t)-1;
    }
    return recvfrom(fd, buffer, length, flags, NULL, NULL);
}

int main(void) {
    int pair[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, pair) != 0) return 1;
    uint8_t buffer[8];
    memset(buffer, 0, sizeof(buffer));

    /* Interrupted forever: each retry resumes against the same deadline, so
       the call ends when the budget is spent instead of spinning. */
    spx_probe_eintr_budget = -1;
    struct spx_network_deadline_v1 deadline;
    deadline.expires_at_millis = spx_network_monotonic_millis_v1() + UINT64_C(120);
    uint64_t started = spx_network_monotonic_millis_v1();
    spx_net_ssize_v1 interrupted =
        spx_network_recv_once_v1(pair[0], buffer, sizeof(buffer), 0, deadline);
    uint64_t elapsed = spx_network_monotonic_millis_v1() - started;

    /* Interrupted twice, then the real read: the same deadline covers both
       retries and the payload arrives intact. */
    spx_probe_eintr_budget = 2;
    if (write(pair[1], "pong", (size_t)4) != (ssize_t)4) return 1;
    struct spx_network_deadline_v1 resumed_deadline;
    resumed_deadline.expires_at_millis = spx_network_monotonic_millis_v1() + UINT64_C(2000);
    uint64_t resumed_started = spx_network_monotonic_millis_v1();
    spx_net_ssize_v1 resumed =
        spx_network_recv_once_v1(pair[0], buffer, sizeof(buffer), 0, resumed_deadline);
    uint64_t resumed_elapsed = spx_network_monotonic_millis_v1() - resumed_started;

    printf(
        "interrupted=%d elapsed=%llu resumed=%d resumed_elapsed=%llu payload=%s\n",
        (int)interrupted,
        (unsigned long long)elapsed,
        (int)resumed,
        (unsigned long long)resumed_elapsed,
        (const char *)buffer
    );
    (void)close(pair[0]);
    (void)close(pair[1]);
    return 0;
}
"#;

/// Every read reports `EINTR`, so only the aggregate deadline can end the
/// operation and only one status can be selected.
const ALWAYS_EINTR_RECV_C: &str = r#"
ssize_t recv(int fd, void *buffer, size_t length, int flags) {
    (void)fd;
    (void)buffer;
    (void)length;
    (void)flags;
    struct timespec nap = { 0, 5L * 1000000L };
    (void)nanosleep(&nap, NULL);
    errno = EINTR;
    return (ssize_t)-1;
}
"#;

/// A numeric endpoint consults no name service, so it needs no budget and no
/// worker; a name under a spent budget is refused before a worker starts.
/// This mirrors the two `SystemResolver` cases in the Rust provider.
#[test]
fn native_numeric_resolution_needs_no_budget_and_no_worker() {
    if !clang_available() {
        return;
    }
    let program = resolved(&with_port(STREAM_SOURCE, 8080), "network-stream.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let fields = run_probe(&generated, "numeric-resolution", NUMERIC_RESOLUTION_PROBE_C);
    assert_eq!(probe_number(&fields, "numeric"), 1, "{fields:?}");
    assert_eq!(probe_number(&fields, "after_numeric"), 0, "{fields:?}");
    assert_eq!(
        probe_number(&fields, "named"),
        0,
        "a spent budget must refuse a name: {fields:?}"
    );
    assert_eq!(
        probe_number(&fields, "after_named"),
        0,
        "a refused name must start no worker: {fields:?}"
    );
}

/// Name resolution is inside the aggregate deadline: a resolver that takes
/// 300 ms under a 30 ms budget ends the wait at the budget, and the worker it
/// left behind is owned and reaped rather than detached.
#[test]
fn native_slow_name_resolution_ends_at_the_budget_and_leaves_an_owned_worker() {
    if !clang_available() {
        return;
    }
    let program = resolved(&with_port(STREAM_SOURCE, 8080), "network-stream.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let fields = run_probe(&generated, "slow-resolution", SLOW_RESOLUTION_PROBE_C);
    assert_eq!(
        probe_number(&fields, "resolved"),
        0,
        "a resolver slower than the budget must not answer: {fields:?}"
    );
    assert!(
        probe_number(&fields, "elapsed") < 250,
        "the caller must stop waiting at its budget, not at the resolver's \
         300 ms cost: {fields:?}"
    );
    assert_eq!(
        probe_number(&fields, "owned"),
        1,
        "an abandoned resolver worker must stay owned, never detached: {fields:?}"
    );
    assert_eq!(
        probe_number(&fields, "reaped"),
        0,
        "an owned resolver worker must be joined and freed: {fields:?}"
    );
}

/// `EINTR` re-slices the same deadline rather than restarting it, so an always
/// interrupted read ends at the budget, and an interrupted-then-successful
/// read still delivers its payload.
#[test]
fn native_interrupted_reads_resume_against_the_same_deadline() {
    if !clang_available() {
        return;
    }
    let program = resolved(&with_port(RECV_SOURCE, 8080), "network-recv.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-recv.run").unwrap();
    let fields = run_probe(&generated, "eintr-read", EINTR_READ_PROBE_C);
    assert_eq!(
        fields.get("interrupted").map(String::as_str),
        Some("-1"),
        "{fields:?}"
    );
    let elapsed = probe_number(&fields, "elapsed");
    assert!(
        (100..2_000).contains(&elapsed),
        "an always interrupted read must end at its 120 ms budget, neither \
         instantly nor by spinning: {fields:?}"
    );
    assert_eq!(
        fields.get("resumed").map(String::as_str),
        Some("4"),
        "two interruptions must not lose the payload: {fields:?}"
    );
    assert_eq!(fields.get("payload").map(String::as_str), Some("pong"));
    assert!(
        probe_number(&fields, "resumed_elapsed") < 2_000,
        "the resumed read stayed inside its own budget: {fields:?}"
    );
}

/// The same failure is selected however often the read is interrupted: the
/// operation ends at the selected aggregate deadline with the network
/// domain's transfer failure, not with a different status and not after the
/// default thirty seconds.
#[test]
fn native_interrupted_reads_select_the_same_failure_under_a_short_deadline() {
    if !clang_available() {
        return;
    }
    let (port, server) = spawn_server(5, Vec::new());
    let program = resolved(&with_port(RECV_ONLY_SOURCE, port), "network-recv-only.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-recv-only.run").unwrap();
    let started = std::time::Instant::now();
    let outcome = run_harness_with(
        &generated,
        "eintr-status",
        ALWAYS_EINTR_RECV_C,
        &["-DSPX_NETWORK_OPERATION_DEADLINE_MILLIS_V1=250"],
    );
    let elapsed = started.elapsed();
    assert_eq!(server.join().unwrap(), b"PING\n");
    assert!(outcome.ok);
    assert!(!outcome.success);
    assert_eq!(outcome.domain, "semaprax.network.v1");
    assert_eq!(outcome.code, 5, "TRANSFER_FAILED");
    assert!(outcome.stdout.is_empty());
    assert!(
        elapsed < Duration::from_secs(20),
        "a selected 250 ms deadline must bound the run, not the 30 s default: \
         {elapsed:?}"
    );
}

/// The project route publishes a network-profile executable, so the emitted
/// translation unit's resolver worker must actually link. This is a build and
/// link fact only: nothing here executes the published program, and no name is
/// resolved.
#[test]
fn network_profile_project_publishes_a_linked_native_executable() {
    if !clang_available() {
        return;
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/network-http-project")
        .join("semaprax.toml");
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-network-project-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let executable = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
    let built = semaprax::project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.build_native(&executable)
    });
    let published = executable.is_file();
    let _ = std::fs::remove_dir_all(&directory);
    built.unwrap();
    assert!(published, "the network project published no executable");
}

/// The `_WIN32` branch is a *compile* check and nothing more. No Windows host
/// runs here, so Bounded Language Network I/O v1 scopes Windows execution out
/// explicitly; cross-compiling the branch where a Windows toolchain exists
/// keeps its Winsock deadline path from rotting unobserved. The process
/// adapter is POSIX by construction and is excluded, exactly as the status
/// harness excludes it.
#[test]
fn native_win32_branch_cross_compiles_where_a_windows_toolchain_exists() {
    let Some(compiler) = windows_cross_compiler() else {
        return;
    };
    let program = resolved(&with_port(STREAM_SOURCE, 8080), "network-stream.spx");
    let generated = codegen::emit_hir_c_with_network_io(&program, "network-stream.run").unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-network-win32-{}-{id}", std::process::id());
    let source = std::env::temp_dir().join(format!("{stem}.c"));
    let object = std::env::temp_dir().join(format!("{stem}.o"));
    std::fs::write(&source, &generated).unwrap();
    let compiled = Command::new(compiler)
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-unused-function",
            "-DSPX_NO_LANGUAGE_COMMAND_PROCESS_ADAPTER",
            "-c",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&compiled.stderr).into_owned();
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&object);
    assert!(compiled.status.success(), "{compiler}: {stderr}");
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

/// The native C11 lane has no HTTPS Client I/O v1 adapter. It must reject
/// `https_get` precisely and *before* emission, naming the profile rather than
/// borrowing the TLS/listen message, and never emit socket or HTTPS text.
#[test]
fn native_lane_rejects_https_get_precisely_and_emits_no_https_text() {
    let source = r#"
module test.native_https;
permit { network.http, process.stdout.write }
@id("https.run")
fn run() -> bool uses { network.http, process.stdout.write } {
    let url = [104u8, 116u8, 116u8, 112u8, 115u8, 58u8, 47u8, 47u8, 97u8, 46u8, 116u8, 101u8, 115u8, 116u8, 47u8];
    let response = https_get(array_as_slice(url), 16usize);
    stdout_append(bytes_as_slice(response)) > 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program = resolved(source, "native-https.spx");
    // The permit gate refuses `network.http` before the operation is reached,
    // so the network lane never emits a translation unit for this module.
    let permits = codegen::emit_hir_c_with_network_io(&program, "https.run")
        .expect_err("the native network lane admits no network.http permit");
    assert!(
        permits
            .message
            .contains("network command permits must stay within"),
        "{permits:?}"
    );

    // The language lane refuses the same module at its own permit gate, so
    // both native entry points fail closed before any translation unit is
    // produced. The emitter's HTTPS arm is therefore unreachable defence in
    // depth, not the rejection a user actually observes.
    let language = codegen::emit_hir_c_with_language_command_io(&program, "https.run")
        .expect_err("the language lane admits no network.http permit");
    assert_eq!(language.code, "SPX-B103", "{language:?}");
    assert!(
        language.message.contains("permit inventory"),
        "{language:?}"
    );

    // Nothing HTTPS-shaped leaks into a translation unit the native lane does
    // emit for a neighbouring profile.
    let plain = resolved(
        r#"
module test.native_plain;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("plain.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view) == byte_len(view)
}
@id("main") fn main() -> i64 { 0 }
"#,
        "native-plain.spx",
    );
    let generated = codegen::emit_hir_c_with_language_command_io(&plain, "plain.run").unwrap();
    for forbidden in ["https", "HTTPS", "spx_https_get_v1", "SPX_HTTP_"] {
        assert!(!generated.contains(forbidden), "found {forbidden}");
    }
}
