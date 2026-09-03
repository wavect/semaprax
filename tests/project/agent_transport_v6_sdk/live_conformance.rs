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

struct Daemon {
    child: Child,
    input: ChildStdin,
    responses: mpsc::Receiver<Vec<u8>>,
    output: Option<thread::JoinHandle<()>>,
    error: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>,
}

impl Daemon {
    fn start(fixture: &Fixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
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
            input: child.stdin.take().unwrap(),
            responses,
            output: Some(output),
            error: Some(thread::spawn(move || read_capped(error, MAX_STDERR_BYTES))),
            child,
        }
    }

    fn request(&mut self, request: &[u8]) -> Vec<u8> {
        assert!(request.ends_with(b"\n") && !request[..request.len() - 1].contains(&b'\n'));
        assert!(request.len() <= MAX_REQUEST_BYTES);
        self.input.write_all(request).unwrap();
        self.input.flush().unwrap();
        match self.responses.recv_timeout(PROCESS_DEADLINE) {
            Ok(response) => response,
            Err(error) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
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
        drop(self.input);
        let deadline = Instant::now() + PROCESS_DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if Instant::now() >= deadline {
                self.child.kill().unwrap();
                let _ = self.child.wait();
                panic!("daemon did not settle before deadline");
            }
            thread::sleep(Duration::from_millis(5));
        }
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
    command: PathBuf,
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
            command,
            arguments: vec![
                "-B".to_owned(),
                root.0.join("client.py").to_string_lossy().into_owned(),
            ],
            root,
        }
    }

    fn rust() -> Self {
        let root = Fixture::empty("rust");
        std::fs::create_dir_all(root.0.join("src")).unwrap();
        std::fs::write(
            root.0.join("src/client.rs"),
            generated(ProjectTransportClientLanguage::Rust),
        )
        .unwrap();
        std::fs::write(root.0.join("src/main.rs"), RUST_ADAPTER).unwrap();
        let lock =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock"))
                .unwrap();
        let marker = "name = \"serde_json\"\nversion = \"";
        let serde_json_version = lock
            .split(marker)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("workspace lock contains serde_json");
        std::fs::write(
            root.0.join("Cargo.toml"),
            format!(
                "[package]\nname=\"semaprax-v6-sdk-consumer\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[dependencies]\nserde_json=\"={serde_json_version}\"\n"
            ),
        )
        .unwrap();
        let cargo = configured_tool("SEMAPRAX_TEST_CARGO", &[]);
        for arguments in [
            vec![
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                "Cargo.toml",
            ],
            vec![
                "build",
                "--locked",
                "--offline",
                "--quiet",
                "--target-dir",
                "target",
                "--manifest-path",
                "Cargo.toml",
            ],
        ] {
            let output = run_bounded(&cargo, &arguments, &root.0, b"", false);
            assert_success(&output, "compile generated Rust codec offline");
        }
        let executable = root.0.join("target").join("debug").join(format!(
            "semaprax-v6-sdk-consumer{}",
            std::env::consts::EXE_SUFFIX
        ));
        Self {
            language: Language::Rust,
            command: executable,
            arguments: Vec::new(),
            root,
        }
    }

    fn typescript() -> Self {
        let root = Fixture::empty("typescript");
        let tsc = configured_tool("SEMAPRAX_TEST_TSC", &[]);
        let node = configured_tool("SEMAPRAX_TEST_NODE", &[]);
        let version = run_bounded(&tsc, &["--version"], &root.0, b"", false);
        assert_success(&version, "query held TypeScript compiler");
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            "Version 5.8.3"
        );
        let version = run_bounded(&node, &["--version"], &root.0, b"", false);
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
            &tsc,
            &[
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
            false,
        );
        assert_success(&output, "compile generated TypeScript codec");
        Self {
            language: Language::TypeScript,
            command: node,
            arguments: vec![root.0.join("client.js").to_string_lossy().into_owned()],
            root,
        }
    }

    fn invoke(&self, arguments: &[&str], input: &[u8], clear_environment: bool) -> Output {
        let mut all = self.arguments.clone();
        all.extend(arguments.iter().map(|value| (*value).to_owned()));
        run_bounded(&self.command, &all, &self.root.0, input, clear_environment)
    }

    fn request(&self, mode: &str, project: &str, workspace: &str) -> Vec<u8> {
        let output = self.invoke(&[mode, project, workspace], b"", true);
        assert_success(&output, "construct generated-client request");
        assert!(output.stdout.ends_with(b"\n"));
        assert!(!output.stdout[..output.stdout.len() - 1].contains(&b'\n'));
        assert!(output.stdout.len() <= MAX_REQUEST_BYTES);
        output.stdout
    }

    fn decode(&self, response: &[u8], build: bool) -> Output {
        self.invoke(
            &["decode", if build { "build" } else { "describe" }],
            response,
            true,
        )
    }

    fn assert_hostile_requests(&self) {
        let output = self.invoke(&["hostile"], b"", true);
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
    if variable == "SEMAPRAX_TEST_CARGO" {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let path = PathBuf::from(&cargo);
        let resolved = if path.is_absolute() {
            path.canonicalize().unwrap()
        } else {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|directory| directory.join(&path))
                .find(|candidate| candidate.is_file())
                .and_then(|candidate| candidate.canonicalize().ok())
                .expect("local smoke test requires Cargo on PATH")
        };
        let output = Command::new(&resolved).arg("--version").output().unwrap();
        assert!(output.status.success());
        return resolved;
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

fn run_bounded(
    command: &Path,
    arguments: &[impl AsRef<OsStr>],
    current_dir: &Path,
    input: &[u8],
    clear_environment: bool,
) -> Output {
    assert!(input.len() <= MAX_RESPONSE_BYTES.saturating_add(1));
    let mut command = Command::new(command);
    command
        .args(arguments)
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if clear_environment {
        command.env_clear();
    }
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let input = input.to_vec();
    let input_thread = thread::spawn(move || stdin.write_all(&input));
    let stdout_thread = thread::spawn(move || read_capped(stdout, MAX_RESPONSE_BYTES));
    let stderr_thread = thread::spawn(move || read_capped(stderr, MAX_STDERR_BYTES));
    let deadline = Instant::now() + PROCESS_DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
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
            let decoded = codec.decode(&response, build);
            assert_success(&decoded, "decode live retained daemon response");
            assert_eq!(decoded.stdout, b"ok\n", "{:?} response", codec.language);
        }
        daemon.finish();
        assert_eq!(fixture.inventory(), before);
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
            !codec.decode(&bytes, false).status.success(),
            "hostile response admitted"
        );
    }
    let build_response = daemon.request(&codec.request("build", &project, &workspace));
    let mut wrong_build: Value = serde_json::from_slice(&build_response).unwrap();
    wrong_build["result"]["build"]["schema"] = json!("semaprax.project-npm-build.v9");
    assert!(!codec
        .decode(&serde_json::to_vec(&wrong_build).unwrap(), true)
        .status
        .success());
    let error = json!({"jsonrpc":"2.0","id":7,"error":{"code":-32602,"message":"rejected"}});
    assert!(!codec
        .decode(&serde_json::to_vec(&error).unwrap(), false)
        .status
        .success());
    assert!(!codec.decode(b"null", false).status.success());
    assert!(!codec.decode(b"{} {}", false).status.success());
    let oversized = vec![b' '; MAX_RESPONSE_BYTES + 1];
    assert!(!codec.decode(&oversized, false).status.success());
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
    assert_success(
        &codec.decode(&recovered, false),
        "decode after stale-subject rejection",
    );
    daemon.finish();
}

#[test]
fn generated_python_and_offline_rust_clients_drive_all_retained_v6_profiles() {
    for codec in [Codec::python(), Codec::rust()] {
        exercise_live(&codec);
        exercise_hostile_decoders(&codec);
    }
}

#[test]
#[ignore = "requires absolute provisioned SEMAPRAX_TEST_TSC 5.8.3 and SEMAPRAX_TEST_NODE >=22"]
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

const RUST_ADAPTER: &str = r#"
mod client { include!("client.rs"); }
use serde_json::{json,Value};
use std::io::{Read,Write};
fn main(){
 let args=std::env::args().collect::<Vec<_>>();
 match args[1].as_str(){
  "describe"|"build"=>{let id=json!(7);let line=if args[1]=="describe"{client::describe(&id,&args[2],&args[3])}else{client::build_inline(&id,&args[2],&args[3],None)}.unwrap();print!("{line}");std::io::stdout().flush().unwrap();}
  "decode"=>{let mut line=String::new();std::io::stdin().read_to_string(&mut line).unwrap();client::decode(&line,&json!(7),args[2]=="build").unwrap();println!("ok");}
  "hostile"=>{let zero=format!("sha256:{}","0".repeat(64));let one=format!("sha256:{}","1".repeat(64));let bad=[client::describe(&json!(-1),&zero,&one),client::describe(&Value::String(String::new()),&zero,&one),client::describe(&json!(7),"x","x"),client::build_inline(&json!(7),&zero,&one,Some(0)),client::build_inline(&json!(7),&zero,&one,Some(41943041))];assert!(bad.iter().all(Result::is_err));println!("ok");}
  _=>panic!("unknown mode")
 }
}
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
