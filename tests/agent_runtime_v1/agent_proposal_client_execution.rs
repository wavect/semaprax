//! Physical compilation and execution of the generated Proposal clients.
#![cfg(unix)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::agent_proposal::{compile_agent_proposal_schema, ProposalValue};

use super::agent_definition_v1::definition;
use super::profile;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MODULE_PATH: &str = "generated-proposal-client.spx";
const RECORD_MODULE: &str = r#"module fixture.agent.proposal_client_record;

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.text") text: string,
    @id("fixture.agent.type.proposal.signed_min") signed_min: i64,
    @id("fixture.agent.type.proposal.signed_max") signed_max: i64,
    @id("fixture.agent.type.proposal.int_min") int_min: i32,
    @id("fixture.agent.type.proposal.int_max") int_max: i32,
    @id("fixture.agent.type.proposal.byte_max") byte_max: u8,
    @id("fixture.agent.type.proposal.sequence_max") sequence_max: usize,
    @id("fixture.agent.type.proposal.urgent") urgent: bool,
}

@id("app.main") fn main() -> i64 { 0 }
"#;

const VARIANT_MODULE: &str = r#"module fixture.agent.proposal_client_variant;

@id("fixture.agent.type.proposal")
variant Proposal {
    @id("fixture.agent.type.proposal.finish")
    Finish {
        @id("fixture.agent.type.proposal.finish.code") code: i64,
    },
    @id("fixture.agent.type.proposal.call")
    Call {
        @id("fixture.agent.type.proposal.call.attempts") attempts: usize,
        @id("fixture.agent.type.proposal.call.urgent") urgent: bool,
    },
}

@id("app.main") fn main() -> i64 { 0 }
"#;

const TYPESCRIPT_RUNNER: &str = r#"import * as record from "./record.js";
import * as variant from "./variant.js";

declare const process: {argv: string[]; stdout: {write(value: string): void}};

const text = "é".repeat(2048);
const recordValue: record.ProposalFields = {
  "fixture.agent.type.proposal.text": text,
  "fixture.agent.type.proposal.signed_min": -9223372036854775808n,
  "fixture.agent.type.proposal.signed_max": 9223372036854775807n,
  "fixture.agent.type.proposal.int_min": -2147483648n,
  "fixture.agent.type.proposal.int_max": 2147483647n,
  "fixture.agent.type.proposal.byte_max": 255n,
  "fixture.agent.type.proposal.sequence_max": 18446744073709551615n,
  "fixture.agent.type.proposal.urgent": true,
};

function rejected(action: () => unknown): void {
  try { action(); } catch { process.stdout.write("rejected\n"); return; }
  throw new Error("generated TypeScript client accepted hostile input");
}

switch (process.argv[2]) {
  case "record": process.stdout.write(record.encodeProposal(recordValue)); break;
  case "finish": process.stdout.write(variant.encodeProposal({case: "fixture.agent.type.proposal.finish", fields: {"fixture.agent.type.proposal.finish.code": -9223372036854775808n}})); break;
  case "call": process.stdout.write(variant.encodeProposal({case: "fixture.agent.type.proposal.call", fields: {"fixture.agent.type.proposal.call.attempts": 18446744073709551615n, "fixture.agent.type.proposal.call.urgent": false}})); break;
  case "reject-text": rejected(() => record.encodeProposal({...recordValue, "fixture.agent.type.proposal.text": text + "x"})); break;
  case "reject-integer": rejected(() => record.encodeProposal({...recordValue, "fixture.agent.type.proposal.signed_max": 9223372036854775808n})); break;
  case "reject-case": rejected(() => variant.encodeProposal({case: "fixture.agent.type.proposal.other", fields: {}} as unknown as variant.ProposalValue)); break;
  default: throw new Error("unknown mode");
}
"#;

const PYTHON_RUNNER: &str = r#"import sys
import record
import variant

text = "é" * 2048
record_value = {
    "fixture.agent.type.proposal.text": text,
    "fixture.agent.type.proposal.signed_min": -9223372036854775808,
    "fixture.agent.type.proposal.signed_max": 9223372036854775807,
    "fixture.agent.type.proposal.int_min": -2147483648,
    "fixture.agent.type.proposal.int_max": 2147483647,
    "fixture.agent.type.proposal.byte_max": 255,
    "fixture.agent.type.proposal.sequence_max": 18446744073709551615,
    "fixture.agent.type.proposal.urgent": True,
}

def rejected(action):
    try:
        action()
    except (ValueError, TypeError):
        print("rejected")
        return
    raise RuntimeError("generated Python client accepted hostile input")

mode = sys.argv[1]
if mode == "record":
    sys.stdout.write(record.encode_proposal(record_value))
elif mode == "finish":
    sys.stdout.write(variant.encode_proposal({"case":"fixture.agent.type.proposal.finish","fields":{"fixture.agent.type.proposal.finish.code":-9223372036854775808}}))
elif mode == "call":
    sys.stdout.write(variant.encode_proposal({"case":"fixture.agent.type.proposal.call","fields":{"fixture.agent.type.proposal.call.attempts":18446744073709551615,"fixture.agent.type.proposal.call.urgent":False}}))
elif mode == "reject-text":
    rejected(lambda: record.encode_proposal({**record_value, "fixture.agent.type.proposal.text": text + "x"}))
elif mode == "reject-integer":
    rejected(lambda: record.encode_proposal({**record_value, "fixture.agent.type.proposal.signed_max": 9223372036854775808}))
elif mode == "reject-case":
    rejected(lambda: variant.encode_proposal({"case":"fixture.agent.type.proposal.other","fields":{}}))
else:
    raise RuntimeError("unknown mode")
"#;

const RUST_RUNNER: &str = r#"mod record;
mod variant;

fn main() {
    let mode = std::env::args().nth(1).expect("mode");
    let text = "é".repeat(2048);
    let record_value = record::ProposalFields {
        field_0: text.clone(),
        field_1: i64::MIN,
        field_2: i64::MAX,
        field_3: i32::MIN,
        field_4: i32::MAX,
        field_5: u8::MAX,
        field_6: u64::MAX,
        field_7: true,
    };
    match mode.as_str() {
        "record" => print!("{}", record::encode_proposal(&record_value).unwrap()),
        "finish" => print!("{}", variant::encode_proposal(&variant::ProposalValue::Case0(variant::Case0Fields { field_0: i64::MIN })).unwrap()),
        "call" => print!("{}", variant::encode_proposal(&variant::ProposalValue::Case1(variant::Case1Fields { field_0: u64::MAX, field_1: false })).unwrap()),
        "reject-text" => {
            let hostile = record::ProposalFields { field_0: text + "x", ..record_value };
            assert_eq!(record::encode_proposal(&hostile), Err("proposal string"));
            println!("rejected");
        }
        _ => panic!("unknown mode"),
    }
}
"#;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-agent-proposal-client-{}-{serial}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn command(variable: &str, fallback: &str) -> PathBuf {
    let Some(configured) = std::env::var_os(variable) else {
        return PathBuf::from(fallback);
    };
    let path = PathBuf::from(configured);
    assert!(path.is_absolute(), "{variable} must be absolute");
    path
}

fn run<I, S>(program: &Path, args: I, cwd: &Path) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|error| panic!("could not run {}: {error}", program.display()));
    assert!(
        output.status.success(),
        "{} failed\nstdout: {}\nstderr: {}",
        program.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn dependency_version(name: &str) -> String {
    let lock =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).unwrap();
    let marker = format!("name = \"{name}\"\nversion = \"");
    lock.split(&marker)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("{name} is absent from Cargo.lock"))
        .to_owned()
}

struct ExecutableClients {
    root: TempRoot,
    node: PathBuf,
    python: PathBuf,
    rust: PathBuf,
}

impl ExecutableClients {
    fn build(
        record_typescript: &str,
        variant_typescript: &str,
        record_python: &str,
        variant_python: &str,
        record_rust: &str,
        variant_rust: &str,
    ) -> Self {
        let root = TempRoot::new();
        let node = command("SEMAPRAX_TEST_NODE", "node");
        let python = command("SEMAPRAX_TEST_PYTHON", "python3");
        let cargo = command("SEMAPRAX_TEST_CARGO", "cargo");
        let tsc = command("SEMAPRAX_TEST_TSC", "tsc");

        let version = run(&tsc, ["--version"], &root.0);
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            "Version 5.8.3"
        );
        let version = run(&node, ["--version"], &root.0);
        let major = String::from_utf8(version.stdout)
            .unwrap()
            .trim()
            .trim_start_matches('v')
            .split('.')
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(
            major >= 22,
            "generated clients require provisioned Node >=22"
        );

        let typescript = root.0.join("typescript");
        fs::create_dir(&typescript).unwrap();
        fs::write(typescript.join("record.ts"), record_typescript).unwrap();
        fs::write(typescript.join("variant.ts"), variant_typescript).unwrap();
        fs::write(typescript.join("runner.ts"), TYPESCRIPT_RUNNER).unwrap();
        fs::write(typescript.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        let out = typescript.join("out");
        run(
            &tsc,
            [
                "--strict".as_ref(),
                "--noEmitOnError".as_ref(),
                "--target".as_ref(),
                "ES2022".as_ref(),
                "--module".as_ref(),
                "NodeNext".as_ref(),
                "--moduleResolution".as_ref(),
                "NodeNext".as_ref(),
                "--outDir".as_ref(),
                out.as_os_str(),
                typescript.join("runner.ts").as_os_str(),
            ],
            &typescript,
        );

        let python_root = root.0.join("python");
        fs::create_dir(&python_root).unwrap();
        fs::write(python_root.join("record.py"), record_python).unwrap();
        fs::write(python_root.join("variant.py"), variant_python).unwrap();
        fs::write(python_root.join("runner.py"), PYTHON_RUNNER).unwrap();
        run(
            &python,
            [
                "-m".as_ref(),
                "py_compile".as_ref(),
                python_root.join("record.py").as_os_str(),
                python_root.join("variant.py").as_os_str(),
                python_root.join("runner.py").as_os_str(),
            ],
            &python_root,
        );

        let rust_root = root.0.join("rust");
        fs::create_dir_all(rust_root.join("src")).unwrap();
        fs::write(rust_root.join("src/record.rs"), record_rust).unwrap();
        fs::write(rust_root.join("src/variant.rs"), variant_rust).unwrap();
        fs::write(rust_root.join("src/main.rs"), RUST_RUNNER).unwrap();
        fs::write(
            rust_root.join("Cargo.toml"),
            format!(
                "[package]\nname=\"generated-proposal-client\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[dependencies]\nserde_json=\"={}\"\n",
                dependency_version("serde_json")
            ),
        )
        .unwrap();
        let cargo_target = rust_root.join("target");
        let generated = Command::new(&cargo)
            .args(["generate-lockfile", "--offline", "--manifest-path"])
            .arg(rust_root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &cargo_target)
            .current_dir(&rust_root)
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "offline Cargo lock failed: {}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let built = Command::new(&cargo)
            .args([
                "build",
                "--locked",
                "--offline",
                "--quiet",
                "--manifest-path",
            ])
            .arg(rust_root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &cargo_target)
            .current_dir(&rust_root)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "offline generated Rust client build failed: {}",
            String::from_utf8_lossy(&built.stderr)
        );

        Self {
            rust: cargo_target.join("debug/generated-proposal-client"),
            root,
            node,
            python,
        }
    }

    fn outputs(&self, mode: &str) -> Vec<Vec<u8>> {
        let typescript = self.root.0.join("typescript");
        let python = self.root.0.join("python");
        let rust = self.root.0.join("rust");
        let mut outputs = vec![
            run(
                &self.node,
                [typescript.join("out/runner.js").as_os_str(), mode.as_ref()],
                &typescript,
            )
            .stdout,
            run(
                &self.python,
                [python.join("runner.py").as_os_str(), mode.as_ref()],
                &python,
            )
            .stdout,
        ];
        if matches!(mode, "record" | "finish" | "call" | "reject-text") {
            outputs.push(run(&self.rust, [mode], &rust).stdout);
        }
        outputs
    }
}

#[test]
fn generated_proposal_clients_compile_execute_and_round_trip() {
    if std::env::var_os("SEMAPRAX_REQUIRE_AGENT_PROPOSAL_CLIENTS").is_none() {
        eprintln!("generated Proposal client execution requires an explicit provisioned lane");
        return;
    }

    let definition = definition(&profile());
    let record = compile_agent_proposal_schema(RECORD_MODULE, MODULE_PATH, &definition).unwrap();
    let variant = compile_agent_proposal_schema(VARIANT_MODULE, MODULE_PATH, &definition).unwrap();
    let record_bundle = record.generate_clients().unwrap();
    let variant_bundle = variant.generate_clients().unwrap();
    let clients = ExecutableClients::build(
        record_bundle.typescript_source(),
        variant_bundle.typescript_source(),
        record_bundle.python_source(),
        variant_bundle.python_source(),
        record_bundle.rust_source(),
        variant_bundle.rust_source(),
    );

    for document in clients.outputs("record") {
        let document = String::from_utf8(document).unwrap();
        let decoded = record.decode(&document).unwrap();
        assert_eq!(decoded.canonical_json(), document);
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.text"),
            Some(&ProposalValue::Text("é".repeat(2048)))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.signed_min"),
            Some(&ProposalValue::Signed(i64::MIN))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.signed_max"),
            Some(&ProposalValue::Signed(i64::MAX))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.int_min"),
            Some(&ProposalValue::Signed(i32::MIN.into()))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.int_max"),
            Some(&ProposalValue::Signed(i32::MAX.into()))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.byte_max"),
            Some(&ProposalValue::Unsigned(u8::MAX.into()))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.sequence_max"),
            Some(&ProposalValue::Unsigned(u64::MAX))
        );
        assert_eq!(
            decoded.field("fixture.agent.type.proposal.urgent"),
            Some(&ProposalValue::Bool(true))
        );
    }

    for (mode, expected_case) in [
        ("finish", "fixture.agent.type.proposal.finish"),
        ("call", "fixture.agent.type.proposal.call"),
    ] {
        for document in clients.outputs(mode) {
            let document = String::from_utf8(document).unwrap();
            let decoded = variant.decode(&document).unwrap();
            assert_eq!(decoded.canonical_json(), document);
            assert_eq!(decoded.case(), Some(expected_case));
        }
    }

    for mode in ["reject-text", "reject-integer", "reject-case"] {
        let outputs = clients.outputs(mode);
        for output in outputs {
            assert_eq!(output, b"rejected\n", "{mode}");
        }
    }
}
