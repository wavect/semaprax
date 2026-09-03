use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use semaprax::project_transport::{
    generate_project_public_api_transport_client, ProjectTransportClientLanguage,
};
use serde_json::{json, Value};

#[rustfmt::skip]
mod generated_rust {
    include!("generated_rust.txt");
}

const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const PROCESS_DEADLINE: Duration = Duration::from_secs(30);
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct ProfileCase {
    label: &'static str,
    manifest: &'static str,
    source: &'static str,
    project_schema: &'static str,
    descriptor_schema: &'static str,
    carrier_schema: &'static str,
}

const TEST_SOURCE: &str =
    "module transport.tests;\n\n@id(\"transport.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";

const CASES: [ProfileCase; 4] = [
    ProfileCase {
        label: "v8",
        manifest: "schema = \"semaprax.project.v8\"\nname = \"sdk-v8\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"sdk.v8\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"sdk.v8.copy\"]\ntests = [\"transport.tests\"]\n",
        source: "module sdk.v8;\n\n@id(\"sdk.v8.copy\")\nfn copy(input: borrow Slice<u8>) -> Bytes\n{\n    bytes_copy(input)\n}\n\n@id(\"sdk.v8.main\")\nfn main() -> i64\n{\n    0\n}\n",
        project_schema: "semaprax.project.v8",
        descriptor_schema: "semaprax.public-owned-data-api.v1",
        carrier_schema: "semaprax.project-npm-build.v7",
    },
    ProfileCase {
        label: "v9",
        manifest: "schema = \"semaprax.project.v9\"\nname = \"sdk-v9\"\nversion = \"1.0.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"sdk.v9\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"sdk.v9.make\"]\ntests = [\"transport.tests\"]\n",
        source: "module sdk.v9;\n\n@id(\"sdk.v9.packet\")\nrecord Packet {\n    @id(\"sdk.v9.packet.payload\") payload: Bytes,\n    @id(\"sdk.v9.packet.size\") size: usize,\n}\n\n@id(\"sdk.v9.make\")\nfn make(input: borrow Slice<u8>) -> Packet\n{\n    Packet { payload: bytes_copy(input), size: byte_len(input) }\n}\n\n@id(\"sdk.v9.main\")\nfn main() -> i64\n{\n    0\n}\n",
        project_schema: "semaprax.project.v9",
        descriptor_schema: "semaprax.public-flat-owned-record-api.v1",
        carrier_schema: "semaprax.project-npm-build.v8",
    },
    ProfileCase {
        label: "v10",
        manifest: "schema = \"semaprax.project.v10\"\nname = \"sdk-v10\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"sdk.v10\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"sdk.v10.greeting\"]\ntests = [\"transport.tests\"]\n",
        source: "module sdk.v10;\n\n@id(\"sdk.v10.greeting\")\nfn greeting() -> string\n{\n    \"hello\"\n}\n\n@id(\"sdk.v10.main\")\nfn main() -> i64\n{\n    0\n}\n",
        project_schema: "semaprax.project.v10",
        descriptor_schema: "semaprax.public-owned-utf8-api.v1",
        carrier_schema: "semaprax.project-npm-build.v9",
    },
    ProfileCase {
        label: "v11",
        manifest: "schema = \"semaprax.project.v11\"\nname = \"sdk-v11\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"sdk.v11\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"sdk.v11.make\"]\ntests = [\"transport.tests\"]\n",
        source: "module sdk.v11;\n\n@id(\"sdk.v11.payload\")\nrecord Payload {\n    @id(\"sdk.v11.payload.bytes\") bytes: Bytes,\n    @id(\"sdk.v11.payload.size\") size: usize,\n}\n\n@id(\"sdk.v11.envelope\")\nrecord Envelope {\n    @id(\"sdk.v11.envelope.left\") left: Payload,\n    @id(\"sdk.v11.envelope.right\") right: Payload,\n}\n\n@id(\"sdk.v11.make\")\nfn make(input: borrow Slice<u8>) -> Envelope\n{\n    Envelope {\n        left: Payload { bytes: bytes_copy(input), size: byte_len(input) },\n        right: Payload { bytes: bytes_copy(input), size: byte_len(input) },\n    }\n}\n\n@id(\"sdk.v11.main\")\nfn main() -> i64\n{\n    0\n}\n",
        project_schema: "semaprax.project.v11",
        descriptor_schema: "semaprax.public-nested-owned-record-api.v1",
        carrier_schema: "semaprax.project-npm-build.v10",
    },
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(case: ProfileCase) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-v6-sdk-live-{}-{}-{}",
            case.label,
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), case.manifest).unwrap();
        for (name, source) in [("app.spx", case.source), ("tests.spx", TEST_SOURCE)] {
            let canonical = semaprax::format::canonical(
                &semaprax::parse(source, Path::new(name)).expect("valid SDK fixture"),
            );
            std::fs::write(root.join("src").join(name), canonical).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn inventory(&self) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, path: &Path, rows: &mut Vec<(String, Vec<u8>)>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, rows);
                } else {
                    rows.push((
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        std::fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let mut rows = Vec::new();
        visit(&self.0, &self.0, &mut rows);
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        rows
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct OwnedChild(Option<Child>);

impl OwnedChild {
    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("owned live child")
    }

    fn settle(&mut self, require_success: bool) {
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            let Some(child) = self.0.as_mut() else {
                return;
            };
            if let Some(status) = child.try_wait().unwrap() {
                self.0.take();
                if require_success {
                    assert!(status.success(), "owned child failed: {status}");
                }
                return;
            }
            if Instant::now() >= deadline {
                self.abort();
                panic!("owned child did not settle before deadline");
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn abort(&mut self) {
        let Some(child) = self.0.as_mut() else {
            return;
        };
        let _ = child.kill();
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.0.take();
                    return;
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                _ => std::process::abort(),
            }
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.abort();
    }
}

struct Daemon {
    child: OwnedChild,
    input: Option<ChildStdin>,
    responses: mpsc::Receiver<Vec<u8>>,
    output: Option<thread::JoinHandle<()>>,
    error: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
}

impl Daemon {
    fn start(fixture: &Fixture) -> Self {
        let executable = Path::new(env!("CARGO_BIN_EXE_semapraxd"));
        assert!(
            executable.is_absolute() && executable.is_file(),
            "the harness must supply an absolute semapraxd image"
        );
        let executable = executable.canonicalize().unwrap();
        let mut child = Command::new(executable)
            .args(["--stdio", "--manifest-path"])
            .arg(fixture.manifest())
            .args([
                "--max-response-bytes",
                "16777216",
                "--allow-project-public-api",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .spawn()
            .unwrap();
        let error = child.stderr.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, responses) = mpsc::sync_channel(1);
        let output = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(line) = read_bounded_line(&mut reader, MAX_RESPONSE_BYTES) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            input: Some(child.stdin.take().unwrap()),
            responses,
            output: Some(output),
            error: Some(thread::spawn(move || read_capped(error, MAX_STDERR_BYTES))),
            child: OwnedChild(Some(child)),
        }
    }

    fn request(&mut self, request: &[u8]) -> Vec<u8> {
        assert!(request.ends_with(b"\n") && !request[..request.len() - 1].contains(&b'\n'));
        assert!(request.len() <= MAX_REQUEST_BYTES);
        let input = self.input.as_mut().unwrap();
        input.write_all(request).unwrap();
        input.flush().unwrap();
        match self.responses.recv_timeout(PROCESS_DEADLINE) {
            Ok(response) => response,
            Err(error) => {
                self.input.take();
                self.child.abort();
                panic!("daemon response did not arrive before deadline: {error}");
            }
        }
    }

    fn json(&mut self, value: Value) -> Value {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        serde_json::from_slice(&self.request(&bytes)).unwrap()
    }

    fn open(&mut self) -> (String, String) {
        let response = self.json(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
        (
            response["result"]["project_revision"]
                .as_str()
                .unwrap()
                .to_owned(),
            response["result"]["workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned(),
        )
    }

    fn finish(mut self) {
        assert_eq!(
            self.json(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}))["result"]["ok"],
            true
        );
        self.input.take();
        self.child.settle(true);
        if let Some(error) = self.error.take() {
            let bytes = error.join().unwrap().unwrap();
            assert!(
                bytes.is_empty(),
                "daemon stderr: {}",
                String::from_utf8_lossy(&bytes)
            );
        }
        if let Some(output) = self.output.take() {
            output.join().unwrap();
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.input.take();
        self.child.abort();
        if let Some(error) = self.error.take() {
            let _ = error.join();
        }
        if let Some(output) = self.output.take() {
            let _ = output.join();
        }
    }
}

fn read_bounded_line(reader: &mut impl BufRead, maximum: usize) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf().unwrap();
        if buffer.is_empty() {
            assert!(line.is_empty(), "process closed stdout within a response");
            return None;
        }
        let take = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        assert!(
            line.len().saturating_add(take) <= maximum,
            "response exceeds bound"
        );
        line.extend_from_slice(&buffer[..take]);
        reader.consume(take);
        if line.ends_with(b"\n") {
            assert!(!line[..line.len() - 1].contains(&b'\r'));
            line.pop();
            return Some(line);
        }
    }
}

fn read_capped(mut reader: impl Read, maximum: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(maximum.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    assert!(bytes.len() <= maximum, "process output exceeds test bound");
    Ok(bytes)
}

#[derive(Clone, Copy, Debug)]
enum Language {
    Python,
    Rust,
    TypeScript,
}

struct Codec {
    language: Language,
    command: Option<PathBuf>,
    arguments: Vec<String>,
    root: Fixture,
}

impl Codec {
    fn python() -> Self {
        let root = Fixture::empty("python");
        let generated = generated(ProjectTransportClientLanguage::Python);
        std::fs::write(
            root.0.join("client.py"),
            format!("{generated}\n{PYTHON_ADAPTER}"),
        )
        .unwrap();
        if cfg!(windows) {
            assert!(
                std::env::var_os("SEMAPRAX_TEST_PYTHON").is_some(),
                "Windows must explicitly provision absolute SEMAPRAX_TEST_PYTHON"
            );
        }
        let command = configured_tool(
            "SEMAPRAX_TEST_PYTHON",
            &[
                "/usr/bin/python3",
                "/opt/homebrew/bin/python3",
                "/usr/local/bin/python3",
            ],
        );
        Self {
            language: Language::Python,
            command: Some(command),
            arguments: vec![
                "-B".to_owned(),
                root.0.join("client.py").to_string_lossy().into_owned(),
            ],
            root,
        }
    }

    fn rust() -> Self {
        let root = Fixture::empty("rust");
        assert_eq!(
            generated(ProjectTransportClientLanguage::Rust),
            include_str!("generated_rust.txt")
        );
        assert_eq!(
            generated_rust::DISCOVERY_JSON,
            semaprax::project_transport::project_public_api_transport_discovery()
        );
        Self {
            language: Language::Rust,
            command: None,
            arguments: Vec::new(),
            root,
        }
    }

    fn typescript() -> Self {
        let root = Fixture::empty("typescript");
        let tsc = configured_tool("SEMAPRAX_TEST_TSC", &[]);
        let node = configured_tool("SEMAPRAX_TEST_NODE", &[]);
        let tsc_source = std::fs::read_to_string(&tsc).expect("read held TypeScript entry");
        let implementation = tsc.with_file_name("_tsc.js");
        let implementation_source =
            std::fs::read_to_string(&implementation).expect("read held TypeScript implementation");
        assert!(
            tsc_source.contains("require(\"./_tsc.js\")")
                && !tsc_source.contains("require(\"child_process\")")
                && !tsc_source.contains(".spawn(")
                && !implementation_source.contains("require(\"child_process\")")
                && !implementation_source.contains(".spawn(")
                && !implementation_source.contains(".execFile("),
            "SEMAPRAX_TEST_TSC must name the held TypeScript lib/tsc.js entry"
        );
        let version = run_bounded(
            &node,
            &[tsc.as_os_str(), OsStr::new("--version")],
            &root.0,
            b"",
        );
        assert_success(&version, "query held TypeScript compiler");
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            "Version 5.8.3"
        );
        let version = run_bounded(&node, &["--version"], &root.0, b"");
        assert_success(&version, "query held Node runtime");
        let major: u64 = String::from_utf8(version.stdout)
            .unwrap()
            .trim()
            .trim_start_matches('v')
            .split('.')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(major >= 22);
        std::fs::write(
            root.0.join("client.ts"),
            format!(
                "{}\n{}",
                generated(ProjectTransportClientLanguage::TypeScript),
                TYPESCRIPT_ADAPTER
            ),
        )
        .unwrap();
        std::fs::write(root.0.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        let output = run_bounded(
            &node,
            &[
                tsc.to_str().unwrap(),
                "--strict",
                "--noEmitOnError",
                "--target",
                "ES2022",
                "--module",
                "NodeNext",
                "--moduleResolution",
                "NodeNext",
                "client.ts",
            ],
            &root.0,
            b"",
        );
        assert_success(&output, "compile generated TypeScript codec");
        Self {
            language: Language::TypeScript,
            command: Some(node),
            arguments: vec![root.0.join("client.js").to_string_lossy().into_owned()],
            root,
        }
    }

    fn invoke(&self, arguments: &[&str], input: &[u8]) -> Output {
        let mut all = self.arguments.clone();
        all.extend(arguments.iter().map(|value| (*value).to_owned()));
        run_bounded(
            self.command.as_deref().expect("process-backed codec"),
            &all,
            &self.root.0,
            input,
        )
    }

    fn request(&self, mode: &str, project: &str, workspace: &str) -> Vec<u8> {
        if matches!(self.language, Language::Rust) {
            let id = json!(7);
            let line = if mode == "describe" {
                generated_rust::describe(&id, project, workspace)
            } else {
                generated_rust::build_inline(&id, project, workspace, None)
            }
            .expect("construct generated Rust request");
            return line.into_bytes();
        }
        let output = self.invoke(&[mode, project, workspace], b"");
        assert_success(&output, "construct generated-client request");
        assert!(output.stdout.ends_with(b"\n"));
        assert!(!output.stdout[..output.stdout.len() - 1].contains(&b'\n'));
        assert!(output.stdout.len() <= MAX_REQUEST_BYTES);
        output.stdout
    }

    fn decode_succeeds(&self, response: &[u8], build: bool) -> bool {
        if matches!(self.language, Language::Rust) {
            return std::str::from_utf8(response)
                .map_err(|_| "response is not UTF-8".to_owned())
                .and_then(|line| generated_rust::decode(line, &json!(7), build))
                .is_ok();
        }
        let output = self.invoke(
            &["decode", if build { "build" } else { "describe" }],
            response,
        );
        output.status.success() && output.stdout == b"ok\n"
    }

    fn assert_hostile_requests(&self) {
        if matches!(self.language, Language::Rust) {
            let zero = format!("sha256:{}", "0".repeat(64));
            let one = format!("sha256:{}", "1".repeat(64));
            assert!(generated_rust::describe(&json!(-1), &zero, &one).is_err());
            assert!(generated_rust::describe(&Value::String(String::new()), &zero, &one).is_err());
            assert!(generated_rust::describe(&json!(7), "x", "x").is_err());
            assert!(generated_rust::build_inline(&json!(7), &zero, &one, Some(0)).is_err());
            assert!(
                generated_rust::build_inline(&json!(7), &zero, &one, Some(41_943_041)).is_err()
            );
            return;
        }
        let output = self.invoke(&["hostile"], b"");
        assert_success(&output, "generated-client hostile request checks");
        assert_eq!(output.stdout, b"ok\n");
    }
}

impl Fixture {
    fn empty(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-v6-sdk-codec-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
}

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(value) = std::env::var_os(variable) {
        let path = PathBuf::from(value);
        assert!(
            path.is_absolute() && path.is_file(),
            "{variable} must be an absolute file"
        );
        return path.canonicalize().unwrap();
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must select a provisioned tool"))
}

fn generated(language: ProjectTransportClientLanguage) -> String {
    let source = generate_project_public_api_transport_client(language).unwrap();
    for forbidden in [
        "Command::new",
        "subprocess",
        "child_process",
        "process.env",
        "std::env",
        "std::fs",
        "TcpStream",
        "fetch(",
    ] {
        assert!(
            !source.contains(forbidden),
            "authority tripwire found {forbidden}"
        );
    }
    source
}

fn assert_direct_child_contract() {
    for source in [PYTHON_ADAPTER, TYPESCRIPT_ADAPTER] {
        for forbidden in [
            "subprocess",
            "child_process",
            "Command::new",
            ".spawn(",
            ".execFile(",
        ] {
            assert!(
                !source.contains(forbidden),
                "adapter violates direct-child contract with {forbidden}"
            );
        }
    }
    for source in [
        include_str!("../../../src/project_transport/session/public_api.rs"),
        include_str!("../../../src/project/npm.rs"),
        include_str!("../../../src/project/npm/carrier.rs"),
        include_str!("../../../src/project/npm/owned_data.rs"),
        include_str!("../../../src/project/npm/flat_owned_record.rs"),
        include_str!("../../../src/project/npm/owned_utf8.rs"),
        include_str!("../../../src/project/npm/nested_owned_record.rs"),
    ] {
        for forbidden in ["Command::new", ".spawn(", "std::process"] {
            assert!(
                !source.contains(forbidden),
                "admitted daemon route violates direct-child contract with {forbidden}"
            );
        }
    }
}

fn run_bounded(
    command: &Path,
    arguments: &[impl AsRef<OsStr>],
    current_dir: &Path,
    input: &[u8],
) -> Output {
    assert!(input.len() <= MAX_RESPONSE_BYTES.saturating_add(1));
    let mut command = Command::new(command);
    command
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    let mut child = OwnedChild(Some(command.spawn().unwrap()));
    let mut stdin = child.child_mut().stdin.take().unwrap();
    let stdout = child.child_mut().stdout.take().unwrap();
    let stderr = child.child_mut().stderr.take().unwrap();
    let input = input.to_vec();
    let input_thread = thread::spawn(move || stdin.write_all(&input));
    let stdout_thread = thread::spawn(move || read_capped(stdout, MAX_RESPONSE_BYTES));
    let stderr_thread = thread::spawn(move || read_capped(stderr, MAX_STDERR_BYTES));
    let deadline = Instant::now() + PROCESS_DEADLINE;
    let status = loop {
        if let Some(status) = child.child_mut().try_wait().unwrap() {
            child.0.take();
            break status;
        }
        if Instant::now() >= deadline {
            child.abort();
            panic!("{:?} exceeded bounded test deadline", command.get_program());
        }
        thread::sleep(Duration::from_millis(5));
    };
    input_thread.join().unwrap().unwrap();
    Output {
        status,
        stdout: stdout_thread.join().unwrap().unwrap(),
        stderr: stderr_thread.join().unwrap().unwrap(),
    }
}

fn assert_success(output: &Output, action: &str) {
    assert!(
        output.status.success(),
        "{action} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn exercise_live(codec: &Codec) {
    assert_direct_child_contract();
    codec.assert_hostile_requests();
    for case in CASES {
        let fixture = Fixture::new(case);
        let before = fixture.inventory();
        let mut daemon = Daemon::start(&fixture);
        let (project, workspace) = daemon.open();
        for (mode, build) in [("describe", false), ("build", true)] {
            let request = codec.request(mode, &project, &workspace);
            let response = daemon.request(&request);
            let value: Value = serde_json::from_slice(&response).unwrap();
            assert_eq!(value["result"]["project_schema"], case.project_schema);
            assert_eq!(value["result"]["descriptor_schema"], case.descriptor_schema);
            assert_eq!(value["result"]["carrier_schema"], case.carrier_schema);
            assert!(
                codec.decode_succeeds(&response, build),
                "{:?} rejected a live response",
                codec.language
            );
            assert_profile_binding_rejections(codec, &response, case, build);
        }
        for mode in ["describe", "build"] {
            for (bad_project, bad_workspace) in [
                (format!("sha256:{}", "f".repeat(64)), workspace.clone()),
                (project.clone(), format!("sha256:{}", "e".repeat(64))),
            ] {
                let rejected = daemon.request(&codec.request(mode, &bad_project, &bad_workspace));
                assert_eq!(
                    serde_json::from_slice::<Value>(&rejected).unwrap()["error"]["code"],
                    -32602,
                    "{} {mode} admitted a foreign retained subject",
                    case.label
                );
                assert_eq!(
                    daemon.json(json!({"jsonrpc":"2.0","id":10,"method":"ping"}))["result"]
                        ["state"],
                    "open"
                );
            }
        }
        daemon.finish();
        assert_eq!(fixture.inventory(), before);
    }
}

fn assert_profile_binding_rejections(
    codec: &Codec,
    response: &[u8],
    expected: ProfileCase,
    build: bool,
) {
    let original: Value = serde_json::from_slice(response).unwrap();
    for foreign in CASES {
        if foreign.project_schema == expected.project_schema {
            continue;
        }
        for (path, value) in [
            ("project", foreign.project_schema),
            ("descriptor", foreign.descriptor_schema),
            ("carrier", foreign.carrier_schema),
        ] {
            let mut hostile = original.clone();
            match path {
                "project" => hostile["result"]["project_schema"] = json!(value),
                "descriptor" => hostile["result"]["descriptor_schema"] = json!(value),
                "carrier" => hostile["result"]["carrier_schema"] = json!(value),
                _ => unreachable!(),
            }
            assert!(
                !codec.decode_succeeds(&serde_json::to_vec(&hostile).unwrap(), build),
                "{:?} admitted {path} binding from {} into {}",
                codec.language,
                foreign.label,
                expected.label
            );
        }
        if build {
            let mut hostile = original.clone();
            hostile["result"]["build"]["schema"] = json!(foreign.carrier_schema);
            assert!(
                !codec.decode_succeeds(&serde_json::to_vec(&hostile).unwrap(), true),
                "{:?} admitted build binding from {} into {}",
                codec.language,
                foreign.label,
                expected.label
            );
        }
    }
    for path in ["outer", "descriptor"] {
        let mut hostile = original.clone();
        if path == "outer" {
            hostile["result"]["descriptor_digest"] = json!("sha256:00");
        } else {
            hostile["result"]["descriptor"]["schema"] = json!("foreign");
        }
        assert!(
            !codec.decode_succeeds(&serde_json::to_vec(&hostile).unwrap(), build),
            "{:?} admitted hostile {path} binding for {}",
            codec.language,
            expected.label
        );
    }
}

fn exercise_hostile_decoders(codec: &Codec) {
    let fixture = Fixture::new(CASES[3]);
    let mut daemon = Daemon::start(&fixture);
    let (project, workspace) = daemon.open();
    let response = daemon.request(&codec.request("describe", &project, &workspace));
    let original: Value = serde_json::from_slice(&response).unwrap();
    let mut hostiles = Vec::new();
    let mut value = original.clone();
    value
        .as_object_mut()
        .unwrap()
        .insert("surplus".into(), json!(true));
    hostiles.push(value);
    let mut value = original.clone();
    value["id"] = json!(8);
    hostiles.push(value);
    let mut value = original.clone();
    value["result"]
        .as_object_mut()
        .unwrap()
        .insert("surplus".into(), json!(true));
    hostiles.push(value);
    let mut value = original.clone();
    value["result"]["project_schema"] = json!("semaprax.project.v12");
    hostiles.push(value);
    let mut value = original.clone();
    value["result"]["descriptor_schema"] = json!("semaprax.public-owned-data-api.v1");
    hostiles.push(value);
    let mut value = original.clone();
    value["result"]["descriptor_digest"] = json!("sha256:00");
    hostiles.push(value);
    let mut value = original;
    value["result"]["descriptor"]["project_schema"] = json!("semaprax.project.v8");
    hostiles.push(value);
    for value in hostiles {
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            !codec.decode_succeeds(&bytes, false),
            "hostile response admitted"
        );
    }
    let build_response = daemon.request(&codec.request("build", &project, &workspace));
    let mut wrong_build: Value = serde_json::from_slice(&build_response).unwrap();
    wrong_build["result"]["build"]["schema"] = json!("semaprax.project-npm-build.v9");
    assert!(!codec.decode_succeeds(&serde_json::to_vec(&wrong_build).unwrap(), true));
    let error = json!({"jsonrpc":"2.0","id":7,"error":{"code":-32602,"message":"rejected"}});
    assert!(!codec.decode_succeeds(&serde_json::to_vec(&error).unwrap(), false));
    assert!(!codec.decode_succeeds(b"null", false));
    assert!(!codec.decode_succeeds(b"{} {}", false));
    let oversized = vec![b' '; MAX_RESPONSE_BYTES + 1];
    assert!(!codec.decode_succeeds(&oversized, false));
    let stale = format!("sha256:{}", "f".repeat(64));
    let rejected = daemon.request(&codec.request("describe", &stale, &workspace));
    assert_eq!(
        serde_json::from_slice::<Value>(&rejected).unwrap()["error"]["code"],
        -32602
    );
    assert_eq!(
        daemon.json(json!({"jsonrpc":"2.0","id":10,"method":"ping"}))["result"]["state"],
        "open"
    );
    let recovered = daemon.request(&codec.request("describe", &project, &workspace));
    assert!(codec.decode_succeeds(&recovered, false));
    daemon.finish();
}

#[test]
fn generated_python_and_embedded_rust_clients_drive_all_retained_v6_profiles() {
    for codec in [Codec::python(), Codec::rust()] {
        exercise_live(&codec);
        exercise_hostile_decoders(&codec);
    }
}

#[test]
#[ignore = "requires absolute provisioned TypeScript lib/tsc.js 5.8.3 and Node >=22"]
fn provisioned_typescript_client_drives_all_retained_v6_profiles() {
    let codec = Codec::typescript();
    exercise_live(&codec);
    exercise_hostile_decoders(&codec);
}

const PYTHON_ADAPTER: &str = r#"
import sys
mode=sys.argv[1]
if mode in ('describe','build'):
    subject={'project_revision':sys.argv[2],'workspace_revision':sys.argv[3]}
    value=describe(7,subject) if mode=='describe' else build_inline(7,subject)
    sys.stdout.write(value)
elif mode=='decode':
    decode(sys.stdin.read(),7,sys.argv[2]=='build'); print('ok')
elif mode=='hostile':
    bad=[lambda:describe(-1,{'project_revision':'x','workspace_revision':'x'}),lambda:describe('',{'project_revision':'sha256:'+'0'*64,'workspace_revision':'sha256:'+'1'*64}),lambda:build_inline(7,{'project_revision':'sha256:'+'0'*64,'workspace_revision':'sha256:'+'1'*64},0),lambda:build_inline(7,{'project_revision':'sha256:'+'0'*64,'workspace_revision':'sha256:'+'1'*64},41943041)]
    for call in bad:
        try: call(); raise AssertionError('hostile request admitted')
        except ValueError: pass
    print('ok')
else: raise AssertionError('unknown mode')
"#;

const TYPESCRIPT_ADAPTER: &str = r#"
declare const process:{argv:string[];stdin:{on:(event:string,callback:(value:any)=>void)=>void};stdout:{write:(value:string)=>void}};
const mode=process.argv[2];
if(mode==='describe'||mode==='build'){
 const subject={project_revision:process.argv[3],workspace_revision:process.argv[4]};
 process.stdout.write(mode==='describe'?describe(7,subject):buildInline(7,subject));
}else if(mode==='decode'){
 let line='';process.stdin.on('data',(value:any)=>line+=String(value));process.stdin.on('end',()=>{decode(line,7,process.argv[3]==='build');process.stdout.write('ok\n');});
}else if(mode==='hostile'){
 const zero='sha256:'+'0'.repeat(64),one='sha256:'+'1'.repeat(64);const bad=[()=>describe(-1,{project_revision:zero,workspace_revision:one}),()=>describe('',{project_revision:zero,workspace_revision:one}),()=>describe(7,{project_revision:'x',workspace_revision:'x'}),()=>buildInline(7,{project_revision:zero,workspace_revision:one},0),()=>buildInline(7,{project_revision:zero,workspace_revision:one},41943041)];for(const call of bad){let rejected=false;try{call()}catch{rejected=true}if(!rejected)throw Error('hostile request admitted')}process.stdout.write('ok\n');
}else{throw Error('unknown mode')}
"#;
