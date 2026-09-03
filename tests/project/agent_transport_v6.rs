use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{self, ProjectNpmBuild, ProjectProfile};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const TESTS: &str =
    "module transport.tests;\n\n@id(\"transport.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";

#[derive(Clone, Copy)]
struct ProfileCase {
    label: &'static str,
    manifest: &'static str,
    source: &'static str,
    profile: ProjectProfile,
    descriptor_schema: &'static str,
    carrier_schema: &'static str,
}

const CASES: [ProfileCase; 4] = [
    ProfileCase {
        label: "v8",
        manifest: "schema = \"semaprax.project.v8\"\nname = \"transport-v8\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"transport.v8\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"transport.v8.copy\"]\ntests = [\"transport.tests\"]\n",
        source: "module transport.v8;\n\n@id(\"transport.v8.copy\")\nfn copy(input: borrow Slice<u8>) -> Bytes\n{\n    bytes_copy(input)\n}\n\n@id(\"transport.v8.main\")\nfn main() -> i64\n{\n    0\n}\n",
        profile: ProjectProfile::OwnedDataApiV1,
        descriptor_schema: "semaprax.public-owned-data-api.v1",
        carrier_schema: "semaprax.project-npm-build.v7",
    },
    ProfileCase {
        label: "v9",
        manifest: "schema = \"semaprax.project.v9\"\nname = \"transport-v9\"\nversion = \"1.0.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"transport.v9\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"transport.v9.make\"]\ntests = [\"transport.tests\"]\n",
        source: "module transport.v9;\n\n@id(\"transport.v9.packet\")\nrecord Packet {\n    @id(\"transport.v9.packet.payload\") payload: Bytes,\n    @id(\"transport.v9.packet.size\") size: usize,\n}\n\n@id(\"transport.v9.make\")\nfn make(input: borrow Slice<u8>) -> Packet\n{\n    Packet { payload: bytes_copy(input), size: byte_len(input) }\n}\n\n@id(\"transport.v9.main\")\nfn main() -> i64\n{\n    0\n}\n",
        profile: ProjectProfile::FlatOwnedRecordApiV1,
        descriptor_schema: "semaprax.public-flat-owned-record-api.v1",
        carrier_schema: "semaprax.project-npm-build.v8",
    },
    ProfileCase {
        label: "v10",
        manifest: "schema = \"semaprax.project.v10\"\nname = \"transport-v10\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"transport.v10\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"transport.v10.greeting\"]\ntests = [\"transport.tests\"]\n",
        source: "module transport.v10;\n\n@id(\"transport.v10.greeting\")\nfn greeting() -> string\n{\n    \"hello\"\n}\n\n@id(\"transport.v10.main\")\nfn main() -> i64\n{\n    0\n}\n",
        profile: ProjectProfile::OwnedUtf8ApiV1,
        descriptor_schema: "semaprax.public-owned-utf8-api.v1",
        carrier_schema: "semaprax.project-npm-build.v9",
    },
    ProfileCase {
        label: "v11",
        manifest: "schema = \"semaprax.project.v11\"\nname = \"transport-v11\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"transport.v11\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"transport.v11.make\"]\ntests = [\"transport.tests\"]\n",
        source: "module transport.v11;\n\n@id(\"transport.v11.payload\")\nrecord Payload {\n    @id(\"transport.v11.payload.bytes\") bytes: Bytes,\n    @id(\"transport.v11.payload.size\") size: usize,\n}\n\n@id(\"transport.v11.envelope\")\nrecord Envelope {\n    @id(\"transport.v11.envelope.left\") left: Payload,\n    @id(\"transport.v11.envelope.right\") right: Payload,\n}\n\n@id(\"transport.v11.make\")\nfn make(input: borrow Slice<u8>) -> Envelope\n{\n    Envelope {\n        left: Payload { bytes: bytes_copy(input), size: byte_len(input) },\n        right: Payload { bytes: bytes_copy(input), size: byte_len(input) },\n    }\n}\n\n@id(\"transport.v11.main\")\nfn main() -> i64\n{\n    0\n}\n",
        profile: ProjectProfile::NestedOwnedRecordApiV1,
        descriptor_schema: "semaprax.public-nested-owned-record-api.v1",
        carrier_schema: "semaprax.project-npm-build.v10",
    },
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(case: ProfileCase) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-transport-v6-{}-{}-{}",
            case.label,
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("semaprax.toml"), case.manifest).unwrap();
        let source = semaprax::format::canonical(
            &semaprax::parse(case.source, Path::new("src/app.spx")).unwrap(),
        );
        let tests = semaprax::format::canonical(
            &semaprax::parse(TESTS, Path::new("src/tests.spx")).unwrap(),
        );
        std::fs::write(root.join("src/app.spx"), source).unwrap();
        std::fs::write(root.join("src/tests.spx"), tests).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn inventory(&self) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, rows: &mut Vec<(String, Vec<u8>)>) {
            for entry in std::fs::read_dir(directory).unwrap() {
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
    output: BufReader<ChildStdout>,
    error: BufReader<ChildStderr>,
}

impl Daemon {
    fn start(fixture: &Fixture, enabled: bool) -> Self {
        Self::start_with_limit(fixture, enabled, MAX_RESPONSE_BYTES)
    }

    fn start_with_limit(fixture: &Fixture, enabled: bool, max_response_bytes: usize) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semapraxd"));
        command
            .arg("--stdio")
            .arg("--manifest-path")
            .arg(fixture.manifest())
            .arg("--max-response-bytes")
            .arg(max_response_bytes.to_string());
        if enabled {
            command.arg("--allow-project-public-api");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            error: BufReader::new(child.stderr.take().unwrap()),
            child,
        }
    }

    fn raw(&mut self, request: Value) -> String {
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        if self.output.read_line(&mut line).unwrap() == 0 {
            let status = self.child.wait().unwrap();
            let mut error = String::new();
            self.error.read_to_string(&mut error).unwrap();
            panic!("daemon exited before a response: status={status} stderr={error}");
        }
        assert!(line.ends_with('\n'));
        line.pop();
        assert!(!line.bytes().any(|byte| matches!(byte, b'\n' | b'\r')));
        line
    }

    fn call(&mut self, request: Value) -> Value {
        serde_json::from_str(&self.raw(request)).unwrap()
    }

    fn notify(&mut self, request: Value) {
        self.input
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn finish(mut self) {
        assert_eq!(
            self.call(json!({"jsonrpc":"2.0","id":99,"method":"shutdown"}))["result"]["ok"],
            true
        );
        drop(self.input);
        assert!(self.child.wait().unwrap().success());
    }
}

fn subject(project: &str, workspace: &str) -> Value {
    json!({"project_revision":project,"workspace_revision":workspace})
}

fn raw_build(response: &str) -> &str {
    let marker = "\"build\":";
    let start = response.find(marker).unwrap() + marker.len();
    assert!(response.ends_with("}}"));
    &response[start..response.len() - 2]
}

fn direct(fixture: &Fixture, profile: ProjectProfile) -> (Value, String, ProjectNpmBuild) {
    project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        assert_eq!(snapshot.manifest().project_profile(), profile);
        let (bytes, digest) = match profile {
            ProjectProfile::OwnedDataApiV1 => {
                let descriptor = snapshot.public_api_descriptor()?;
                (descriptor.canonical_bytes(), descriptor.digest())
            }
            ProjectProfile::FlatOwnedRecordApiV1 => {
                let descriptor = snapshot.flat_owned_record_api_descriptor()?;
                (descriptor.canonical_bytes(), descriptor.digest())
            }
            ProjectProfile::OwnedUtf8ApiV1 => {
                let descriptor = snapshot.owned_utf8_api_descriptor()?;
                (descriptor.canonical_bytes(), descriptor.digest())
            }
            ProjectProfile::NestedOwnedRecordApiV1 => {
                let descriptor = snapshot.nested_owned_record_api_descriptor()?;
                (descriptor.canonical_bytes(), descriptor.digest())
            }
            _ => unreachable!("closed v6 profile set"),
        };
        let build = snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)?;
        Ok((serde_json::from_slice(&bytes).unwrap(), digest, build))
    })
    .unwrap()
}

#[test]
fn v6_returns_exact_v8_through_v11_descriptors_and_authenticated_carriers() {
    for case in CASES {
        let fixture = Fixture::new(case);
        let before = fixture.inventory();
        let (descriptor, descriptor_digest, direct_build) = direct(&fixture, case.profile);
        let mut daemon = Daemon::start(&fixture, true);
        let protocol = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"protocol"}));
        assert_eq!(
            protocol["result"]["protocol"],
            "semaprax.agent-transport.v6"
        );
        let opened = daemon.call(json!({"jsonrpc":"2.0","id":2,"method":"workspace/open"}));
        let project_revision = opened["result"]["project_revision"].as_str().unwrap();
        let workspace_revision = opened["result"]["workspace_revision"].as_str().unwrap();

        let described = daemon.call(json!({
            "jsonrpc":"2.0","id":3,"method":"project/api-describe",
            "params":subject(project_revision, workspace_revision)
        }));
        assert_eq!(
            described["result"]["project_schema"],
            case.manifest
                .lines()
                .next()
                .unwrap()
                .split('"')
                .nth(1)
                .unwrap()
        );
        assert_eq!(
            described["result"]["descriptor_schema"],
            case.descriptor_schema
        );
        assert_eq!(described["result"]["carrier_schema"], case.carrier_schema);
        assert_eq!(described["result"]["descriptor"], descriptor);
        assert_eq!(described["result"]["descriptor_digest"], descriptor_digest);
        assert_eq!(
            described["result"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "carrier_schema",
                "descriptor",
                "descriptor_digest",
                "descriptor_schema",
                "project_schema",
            ]
        );

        let raw = daemon.raw(json!({
            "jsonrpc":"2.0","id":4,"method":"project/npm-build-inline",
            "params":{
                "project_revision":project_revision,
                "workspace_revision":workspace_revision,
                "max_bytes":direct_build.envelope().len()
            }
        }));
        let built: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(built["result"]["descriptor"], descriptor);
        assert_eq!(built["result"]["descriptor_digest"], descriptor_digest);
        assert_eq!(
            built["result"]["project_schema"],
            descriptor["project_schema"]
        );
        assert_eq!(built["result"]["descriptor_schema"], case.descriptor_schema);
        assert_eq!(built["result"]["carrier_schema"], case.carrier_schema);
        assert_eq!(
            built["result"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "build",
                "carrier_schema",
                "descriptor",
                "descriptor_digest",
                "descriptor_schema",
                "project_schema",
            ]
        );
        assert_eq!(
            built["result"]["build"],
            serde_json::from_str::<Value>(direct_build.envelope()).unwrap()
        );
        let returned = raw_build(&raw);
        assert_eq!(returned, direct_build.envelope());
        ProjectNpmBuild::inspect_envelope(returned, direct_build.envelope().len()).unwrap();
        daemon.finish();
        assert_eq!(fixture.inventory(), before);
    }
}

#[test]
fn v6_build_wrapper_budget_is_exact_and_rejections_are_recoverable() {
    let case = CASES[3];
    let fixture = Fixture::new(case);
    let (_, _, build) = direct(&fixture, case.profile);
    let mut wide = Daemon::start(&fixture, true);
    let opened = wide.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    let expected = wide.raw(json!({
        "jsonrpc":"2.0","id":7,"method":"project/npm-build-inline","params":{
            "project_revision":opened["result"]["project_revision"],
            "workspace_revision":opened["result"]["workspace_revision"],
            "max_bytes":build.envelope().len()
        }
    }));
    assert_eq!(raw_build(&expected), build.envelope());
    wide.finish();
    let exact_response_bytes = expected.len() + 1;

    let mut exact = Daemon::start_with_limit(&fixture, true, exact_response_bytes);
    let opened = exact.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    let raw = exact.raw(json!({
        "jsonrpc":"2.0","id":7,"method":"project/npm-build-inline","params":{
            "project_revision":opened["result"]["project_revision"],
            "workspace_revision":opened["result"]["workspace_revision"],
            "max_bytes":build.envelope().len()
        }
    }));
    assert_eq!(raw.len() + 1, exact_response_bytes);
    assert_eq!(raw, expected);
    exact.finish();

    let mut short = Daemon::start_with_limit(&fixture, true, exact_response_bytes - 1);
    let opened = short.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    for max_bytes in [
        0,
        build.envelope().len(),
        project::MAX_PROJECT_NPM_BUILD_BYTES + 1,
    ] {
        let rejected = short.call(json!({
            "jsonrpc":"2.0","id":7,"method":"project/npm-build-inline","params":{
                "project_revision":opened["result"]["project_revision"],
                "workspace_revision":opened["result"]["workspace_revision"],
                "max_bytes":max_bytes
            }
        }));
        assert_eq!(rejected["error"]["code"], -32602);
        assert_eq!(
            short.call(json!({"jsonrpc":"2.0","id":8,"method":"ping"}))["result"]["state"],
            "open"
        );
    }
    short.finish();
}

#[test]
fn v6_methods_are_explicit_and_subject_bound() {
    let case = CASES[3];
    let fixture = Fixture::new(case);
    let mut daemon = Daemon::start(&fixture, false);
    let opened = daemon.call(json!({"jsonrpc":"2.0","id":1,"method":"workspace/open"}));
    let revisions = subject(
        opened["result"]["project_revision"].as_str().unwrap(),
        opened["result"]["workspace_revision"].as_str().unwrap(),
    );
    for method in ["project/api-describe", "project/npm-build-inline"] {
        assert_eq!(
            daemon.call(json!({"jsonrpc":"2.0","id":2,"method":method,"params":revisions.clone()}))
                ["error"]["code"],
            -32601
        );
    }
    daemon.finish();

    let mut daemon = Daemon::start(&fixture, true);
    let opened = daemon.call(json!({"jsonrpc":"2.0","id":3,"method":"workspace/open"}));
    let mut stale = subject(
        opened["result"]["project_revision"].as_str().unwrap(),
        opened["result"]["workspace_revision"].as_str().unwrap(),
    );
    stale.as_object_mut().unwrap().insert(
        "project_revision".into(),
        json!("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
    );
    assert_eq!(
        daemon.call(json!({"jsonrpc":"2.0","id":4,"method":"project/api-describe","params":stale}))
            ["error"]["code"],
        -32602
    );
    for forbidden in ["path", "target", "output", "tool", "environment"] {
        let mut params = subject(
            opened["result"]["project_revision"].as_str().unwrap(),
            opened["result"]["workspace_revision"].as_str().unwrap(),
        );
        params
            .as_object_mut()
            .unwrap()
            .insert(forbidden.into(), json!("x"));
        assert_eq!(
            daemon.call(
                json!({"jsonrpc":"2.0","id":5,"method":"project/npm-build-inline","params":params})
            )["error"]["code"],
            -32602
        );
    }
    daemon.notify(json!({
        "jsonrpc":"2.0","method":"project/npm-build-inline",
        "params":subject(
            opened["result"]["project_revision"].as_str().unwrap(),
            opened["result"]["workspace_revision"].as_str().unwrap(),
        )
    }));
    assert_eq!(
        daemon.call(json!({"jsonrpc":"2.0","id":6,"method":"ping"}))["result"]["state"],
        "open"
    );
    daemon.finish();
}
