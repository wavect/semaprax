//! Independent generated-client execution of the supported v5 product workflow.
#![cfg(unix)]

use semaprax::image_transport::{GitCommitHost, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitProcessAuthority,
    CandidateGitRefUpdate, CandidateGitRepository, CandidateGitTarget, CandidateTestPolicy,
    ProjectCandidate, ProjectRevision,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const BRANCH: &str = "refs/heads/review";
const PATHS: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
const REVIEW_METHODS: [&str; 13] = [
    "workspace/open",
    "image/function-reference-export",
    "image/function-reference-resolve",
    "image/analysis-coverage",
    "candidate/open",
    "candidate/apply-intent",
    "candidate/validate",
    "candidate/semantic-delta",
    "candidate/test-plan",
    "candidate/test",
    "candidate/source-review",
    "candidate/analysis-coverage",
    "candidate/recovery-export",
];
const PUBLISH_METHODS: [&str; 9] = [
    "workspace/open",
    "image/function-reference-resolve",
    "candidate/recovery-restore",
    "candidate/validate",
    "candidate/source-review",
    "source-commit/status",
    "candidate/commit",
    "source-commit/status",
    "candidate/commit-report",
];

fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}
fn selected_command(variable: &str, ordinary: &str) -> PathBuf {
    let Some(value) = std::env::var_os(variable) else {
        return PathBuf::from(ordinary);
    };
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{variable} must be an absolute path");
    path
}

struct Fixture {
    root: PathBuf,
    git: PathBuf,
    repo: PathBuf,
    base: String,
    revision: Arc<ProjectRevision>,
    original: BTreeMap<String, Vec<u8>>,
}
impl Fixture {
    fn new(language: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-generated-product-{language}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in PATHS {
            fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let admitted =
            with_authenticated_project(&root.join("semaprax.toml"), |s| Ok(s.retain_revision()))
                .unwrap();
        fs::write(
            root.join("semaprax.toml"),
            admitted.manifest().to_canonical_toml(),
        )
        .unwrap();
        for source in admitted.sources() {
            fs::write(root.join(source.path()), source.source()).unwrap();
        }
        let revision =
            with_authenticated_project(&root.join("semaprax.toml"), |s| Ok(s.retain_revision()))
                .unwrap();
        let original = PATHS
            .into_iter()
            .map(|p| (p.to_owned(), fs::read(root.join(p)).unwrap()))
            .collect();
        let git = selected_command("SEMAPRAX_TEST_GIT", "/usr/bin/git")
            .canonicalize()
            .unwrap();
        let repo = root.join("published.git");
        let output = Command::new(&git)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--bare",
                "--object-format=sha256",
            ])
            .arg(&repo)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        fs::write(repo.join("config"), "[core]\nrepositoryformatversion = 1\nbare = true\n[extensions]\nobjectformat = sha256\n").unwrap();
        let mut fixture = Self {
            root,
            git,
            repo,
            base: String::new(),
            revision,
            original,
        };
        let sources = fixture
            .revision
            .sources()
            .iter()
            .map(|source| {
                (
                    "100644",
                    source.path().strip_prefix("src/").unwrap().to_owned(),
                    fixture.object("blob", source.source().as_bytes()),
                )
            })
            .collect();
        let sources = fixture.tree(sources);
        let manifest = fixture.object(
            "blob",
            fixture.revision.manifest().to_canonical_toml().as_bytes(),
        );
        let keep = fixture.object("blob", b"unrelated executable entry\n");
        let tree = fixture.tree(vec![
            ("40000", "src".into(), sources),
            ("100644", "semaprax.toml".into(), manifest),
            ("100755", "keep.sh".into(), keep),
        ]);
        fixture.base = fixture.object("commit", format!("tree {tree}\nauthor Host <host@example.invalid> 1 +0000\ncommitter Host <host@example.invalid> 1 +0000\n\nOriginal\n").as_bytes());
        fixture.run(&["update-ref", BRANCH, &fixture.base], &[]);
        fixture
    }
    fn manifest(&self) -> PathBuf {
        self.root.join("semaprax.toml")
    }
    fn run(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
        let mut child = Command::new(&self.git)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.logAllRefUpdates=false",
            ])
            .arg(format!("--git-dir={}", self.repo.display()))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn object(&self, kind: &str, bytes: &[u8]) -> String {
        String::from_utf8(self.run(&["hash-object", "-w", "--stdin", "-t", kind], bytes))
            .unwrap()
            .trim_end()
            .to_owned()
    }
    fn tree(&self, mut entries: Vec<(&str, String, String)>) -> String {
        entries.sort_by_key(|(mode, name, _)| {
            format!("{name}{}", if *mode == "40000" { "/" } else { "\0" })
        });
        let mut bytes = Vec::new();
        for (mode, name, oid) in entries {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            for i in (0..oid.len()).step_by(2) {
                bytes.push(u8::from_str_radix(&oid[i..i + 2], 16).unwrap());
            }
        }
        self.object("tree", &bytes)
    }
    fn head(&self) -> String {
        String::from_utf8(self.run(&["rev-parse", BRANCH], &[]))
            .unwrap()
            .trim_end()
            .to_owned()
    }
    fn unchanged(&self) {
        for (path, bytes) in &self.original {
            assert_eq!(fs::read(self.root.join(path)).unwrap(), *bytes, "{path}");
        }
        assert!(!self.root.join(".semaprax-workspace").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ProfileAuthority;
impl CandidateGitAuthority for ProfileAuthority {
    fn repository(&self) -> io::Result<CandidateGitRepository> {
        Ok(CandidateGitRepository {
            identity: "generated-client-profile".into(),
            bare: true,
            sha256: true,
        })
    }
    fn read_ref(&mut self, _: &str) -> io::Result<Option<String>> {
        Err(io::Error::other("profile authority cannot read"))
    }
    fn read_object(&mut self, _: &str, _: usize) -> io::Result<CandidateGitObject> {
        Err(io::Error::other("profile authority cannot read"))
    }
    fn write_object(&mut self, _: CandidateGitObjectKind, _: &[u8], _: &str) -> io::Result<()> {
        Err(io::Error::other("profile authority cannot write"))
    }
    fn compare_and_swap_ref(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
    ) -> io::Result<CandidateGitRefUpdate> {
        Err(io::Error::other("profile authority cannot publish"))
    }
}

fn policy() -> VNextPolicy {
    VNextPolicy {
        candidate_prepare: true,
        test_policy: Some(CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap()),
        ..Default::default()
    }
}
fn fake_publish_session(fixture: &Fixture) -> VNextSession {
    let repository = ProfileAuthority.repository().unwrap();
    let target =
        CandidateGitTarget::new(&repository.identity, BRANCH, &"0".repeat(64), "").unwrap();
    let metadata = CandidateGitCommitMetadata::new(
        "Host",
        "host@example.invalid",
        2,
        "Reviewed signature evolution\n",
    )
    .unwrap();
    let mut host = GitCommitHost::new(
        &fixture.manifest(),
        target,
        metadata,
        Box::new(ProfileAuthority),
    )
    .unwrap();
    host.approve(&format!("sha256:{}", "0".repeat(64))).unwrap();
    VNextSession::open(&fixture.manifest(), policy())
        .unwrap()
        .with_git_commit_host(host)
        .unwrap()
}

fn raw_call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let frame = json!({"jsonrpc":"2.0","id":"generated-workflow","method":method,"params":params})
        .to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn raw_payload(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let response = raw_call(session, method, params);
    assert!(response.get("error").is_none(), "{method}: {response}");
    response["result"]["payload"].clone()
}
fn client_source(session: &mut VNextSession, language: &str) -> String {
    let value = raw_payload(session, "protocol/client", json!({"language":language}));
    assert_eq!(value["io"], false);
    value["source"].as_str().unwrap().to_owned()
}

#[derive(Clone, Copy)]
enum Language {
    Python,
    Rust,
    TypeScript,
}
impl Language {
    fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
        }
    }
}

struct GeneratedHarness {
    language: Language,
    root: PathBuf,
    program: PathBuf,
    source_bytes: usize,
    source_sha256: String,
    source_artifact: Vec<u8>,
}
impl GeneratedHarness {
    fn build(fixture: &Fixture, language: Language, review: &str, publish: &str) -> Self {
        let root = fixture
            .root
            .join(format!("{}-generated-client", language.name()));
        fs::create_dir_all(&root).unwrap();
        let combined = format!(
            "review-bytes:{}\n{review}publish-bytes:{}\n{publish}",
            review.len(),
            publish.len()
        )
        .into_bytes();
        let source_bytes = combined.len();
        let source_sha256 = sha256(&combined);
        match language {
            Language::Python => {
                Self::build_python(root, review, publish, source_bytes, source_sha256, combined)
            }
            Language::Rust => {
                Self::build_rust(root, review, publish, source_bytes, source_sha256, combined)
            }
            Language::TypeScript => {
                Self::build_typescript(root, review, publish, source_bytes, source_sha256, combined)
            }
        }
    }
    fn build_python(
        root: PathBuf,
        review: &str,
        publish: &str,
        source_bytes: usize,
        source_sha256: String,
        source_artifact: Vec<u8>,
    ) -> Self {
        fs::write(root.join("review_client.py"), review).unwrap();
        fs::write(root.join("publish_client.py"), publish).unwrap();
        fs::write(root.join("harness.py"), PYTHON_HARNESS).unwrap();
        let python = selected_command("SEMAPRAX_TEST_PYTHON", "python3");
        let check = Command::new(&python)
            .args(["-m", "py_compile"])
            .arg(root.join("review_client.py"))
            .arg(root.join("publish_client.py"))
            .arg(root.join("harness.py"))
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        Self {
            language: Language::Python,
            root,
            program: python,
            source_bytes,
            source_sha256,
            source_artifact,
        }
    }
    fn build_rust(
        root: PathBuf,
        review: &str,
        publish: &str,
        source_bytes: usize,
        source_sha256: String,
        source_artifact: Vec<u8>,
    ) -> Self {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/review_client.rs"), review).unwrap();
        fs::write(root.join("src/publish_client.rs"), publish).unwrap();
        fs::write(root.join("src/main.rs"), RUST_HARNESS).unwrap();
        let lock =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).unwrap();
        let version = |name: &str| {
            let marker = format!("name = \"{name}\"\nversion = \"");
            let rest = lock.split(&marker).nth(1).unwrap();
            rest.split('"').next().unwrap().to_owned()
        };
        fs::write(root.join("Cargo.toml"), format!("[package]\nname=\"generated-product-client\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[dependencies]\nserde={{version=\"={}\",features=[\"derive\"]}}\nserde_json=\"={}\"\n", version("serde"), version("serde_json"))).unwrap();
        let cargo = selected_command("SEMAPRAX_TEST_CARGO", "cargo");
        let generated = Command::new(&cargo)
            .args(["generate-lockfile", "--offline", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
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
            .arg(root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        Self {
            language: Language::Rust,
            program: root.join("target/debug/generated-product-client"),
            root,
            source_bytes,
            source_sha256,
            source_artifact,
        }
    }
    fn build_typescript(
        root: PathBuf,
        review: &str,
        publish: &str,
        source_bytes: usize,
        source_sha256: String,
        source_artifact: Vec<u8>,
    ) -> Self {
        let tsc = selected_command("SEMAPRAX_TEST_TSC", "");
        let node = selected_command("SEMAPRAX_TEST_NODE", "");
        assert!(
            tsc.is_absolute() && node.is_absolute(),
            "TypeScript tools must be absolute"
        );
        let version = Command::new(&tsc).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert_eq!(
            String::from_utf8(version.stdout).unwrap().trim(),
            "Version 5.8.3"
        );
        let version = Command::new(&node).arg("--version").output().unwrap();
        assert!(version.status.success());
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
        fs::write(root.join("review_client.ts"), review).unwrap();
        fs::write(root.join("publish_client.ts"), publish).unwrap();
        fs::write(root.join("harness.ts"), TYPESCRIPT_HARNESS).unwrap();
        fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        let out = root.join("out");
        let built = Command::new(&tsc)
            .args([
                "--strict",
                "--noEmitOnError",
                "--target",
                "ES2022",
                "--module",
                "NodeNext",
                "--moduleResolution",
                "NodeNext",
                "--outDir",
            ])
            .arg(&out)
            .arg(root.join("harness.ts"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&built.stdout),
            String::from_utf8_lossy(&built.stderr)
        );
        Self {
            language: Language::TypeScript,
            program: node,
            root,
            source_bytes,
            source_sha256,
            source_artifact,
        }
    }
    fn invoke(&self, profile: &str, action: &str, method: &str, input: &Value) -> Value {
        let input_path = self.root.join("input.json");
        fs::write(&input_path, serde_json::to_vec(input).unwrap()).unwrap();
        let mut command = Command::new(&self.program);
        if matches!(self.language, Language::Python) {
            command.arg(self.root.join("harness.py"));
        }
        if matches!(self.language, Language::TypeScript) {
            command.arg(self.root.join("out/harness.js"));
        }
        let output = command
            .args([profile, action, method])
            .arg(&input_path)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {profile} {action} {method}: {}",
            self.language.name(),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn request(&self, profile: &str, method: &str, params: Value) -> Vec<u8> {
        self.invoke(profile, "request", method, &params)["line"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec()
    }
    fn decode(&self, profile: &str, method: &str, response: &[u8]) -> Value {
        self.invoke(
            profile,
            "decode",
            method,
            &json!({"line":std::str::from_utf8(response).unwrap()}),
        )["payload"]
            .clone()
    }
    fn workflows(&self, profile: &str) -> Value {
        self.invoke(profile, "workflows", "none", &json!({}))["workflows"].clone()
    }
    fn rejects_decode(&self, profile: &str, method: &str, response: &str) {
        let input = self.root.join("input.json");
        fs::write(
            &input,
            serde_json::to_vec(&json!({"line":response})).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(&self.program);
        if matches!(self.language, Language::Python) {
            command.arg(self.root.join("harness.py"));
        }
        if matches!(self.language, Language::TypeScript) {
            command.arg(self.root.join("out/harness.js"));
        }
        let output = command
            .args([profile, "decode", method])
            .arg(&input)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "malformed response unexpectedly decoded"
        );
    }
}

const PYTHON_HARNESS: &str = r#"
import importlib, json, pathlib, sys
profile, action, method, input_path = sys.argv[1:]
client = importlib.import_module(profile + "_client")
value = json.loads(pathlib.Path(input_path).read_text(encoding="utf-8"))
suffix = method.replace("/", "_").replace("-", "_")
if action == "workflows": out = {"workflows": client.WORKFLOWS}
elif action == "request": out = {"line": getattr(client, "request_" + suffix + "_typed")("generated-workflow", value)}
elif action == "decode": out = {"payload": getattr(client, "decode_request_" + suffix + "_typed")(value["line"], "generated-workflow")["payload"]}
else: raise RuntimeError("unknown action")
sys.stdout.buffer.write(json.dumps(out, separators=(",", ":"), sort_keys=True).encode("utf-8"))
"#;

const RUST_HARNESS: &str = r#"
mod review_client; mod publish_client;
use serde::de::DeserializeOwned; use serde_json::{json, Value}; use std::{env,fs};
fn params<T:DeserializeOwned>(v:Value)->T { serde_json::from_value(v).unwrap() }
macro_rules! invoke_publish { ($c:ident,$a:expr,$m:expr,$v:expr) => {{
 let id=$c::RpcId::Text("generated-workflow".into());
 match ($a,$m) {
  ("request","workspace/open")=>json!({"line":$c::request_workspace_open_typed(id,params::<$c::WorkspaceOpenTypedParams>($v)).unwrap()}),
  ("request","image/function-reference-export")=>json!({"line":$c::request_image_function_reference_export_typed(id,params::<$c::ImageFunctionReferenceExportTypedParams>($v)).unwrap()}),
  ("request","image/function-reference-resolve")=>json!({"line":$c::request_image_function_reference_resolve_typed(id,params::<$c::ImageFunctionReferenceResolveTypedParams>($v)).unwrap()}),
  ("request","image/analysis-coverage")=>json!({"line":$c::request_image_analysis_coverage_typed(id,params::<$c::ImageAnalysisCoverageTypedParams>($v)).unwrap()}),
  ("request","candidate/open")=>json!({"line":$c::request_candidate_open_typed(id,params::<$c::CandidateOpenTypedParams>($v)).unwrap()}),
  ("request","candidate/apply-intent")=>json!({"line":$c::request_candidate_apply_intent_typed(id,params::<$c::CandidateApplyIntentTypedParams>($v)).unwrap()}),
  ("request","candidate/validate")=>json!({"line":$c::request_candidate_validate_typed(id,params::<$c::CandidateValidateTypedParams>($v)).unwrap()}),
  ("request","candidate/semantic-delta")=>json!({"line":$c::request_candidate_semantic_delta_typed(id,params::<$c::CandidateSemanticDeltaTypedParams>($v)).unwrap()}),
  ("request","candidate/test-plan")=>json!({"line":$c::request_candidate_test_plan_typed(id,params::<$c::CandidateTestPlanTypedParams>($v)).unwrap()}),
  ("request","candidate/test")=>json!({"line":$c::request_candidate_test_typed(id,params::<$c::CandidateTestTypedParams>($v)).unwrap()}),
  ("request","candidate/source-review")=>json!({"line":$c::request_candidate_source_review_typed(id,params::<$c::CandidateSourceReviewTypedParams>($v)).unwrap()}),
  ("request","candidate/analysis-coverage")=>json!({"line":$c::request_candidate_analysis_coverage_typed(id,params::<$c::CandidateAnalysisCoverageTypedParams>($v)).unwrap()}),
  ("request","candidate/recovery-export")=>json!({"line":$c::request_candidate_recovery_export_typed(id,params::<$c::CandidateRecoveryExportTypedParams>($v)).unwrap()}),
  ("request","candidate/recovery-restore")=>json!({"line":$c::request_candidate_recovery_restore_typed(id,params::<$c::CandidateRecoveryRestoreTypedParams>($v)).unwrap()}),
  ("request","source-commit/status")=>json!({"line":$c::request_source_commit_status_typed(id,params::<$c::SourceCommitStatusTypedParams>($v)).unwrap()}),
  ("request","candidate/commit")=>json!({"line":$c::request_candidate_commit_typed(id,params::<$c::CandidateCommitTypedParams>($v)).unwrap()}),
  ("request","candidate/commit-report")=>json!({"line":$c::request_candidate_commit_report_typed(id,params::<$c::CandidateCommitReportTypedParams>($v)).unwrap()}),
  ("decode","workspace/open")=>json!({"payload":$c::decode_request_workspace_open_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/function-reference-export")=>json!({"payload":$c::decode_request_image_function_reference_export_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/function-reference-resolve")=>json!({"payload":$c::decode_request_image_function_reference_resolve_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/analysis-coverage")=>json!({"payload":$c::decode_request_image_analysis_coverage_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/open")=>json!({"payload":$c::decode_request_candidate_open_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/apply-intent")=>json!({"payload":$c::decode_request_candidate_apply_intent_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/validate")=>json!({"payload":$c::decode_request_candidate_validate_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/semantic-delta")=>json!({"payload":$c::decode_request_candidate_semantic_delta_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/test-plan")=>json!({"payload":$c::decode_request_candidate_test_plan_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/test")=>json!({"payload":$c::decode_request_candidate_test_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/source-review")=>json!({"payload":$c::decode_request_candidate_source_review_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/analysis-coverage")=>json!({"payload":$c::decode_request_candidate_analysis_coverage_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/recovery-export")=>json!({"payload":$c::decode_request_candidate_recovery_export_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/recovery-restore")=>json!({"payload":$c::decode_request_candidate_recovery_restore_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","source-commit/status")=>json!({"payload":$c::decode_request_source_commit_status_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/commit")=>json!({"payload":$c::decode_request_candidate_commit_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/commit-report")=>json!({"payload":$c::decode_request_candidate_commit_report_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  _=>panic!("unknown operation")
 }
}}}
macro_rules! invoke_review { ($c:ident,$a:expr,$m:expr,$v:expr) => {{
 let id=$c::RpcId::Text("generated-workflow".into());
 match ($a,$m) {
  ("request","workspace/open")=>json!({"line":$c::request_workspace_open_typed(id,params::<$c::WorkspaceOpenTypedParams>($v)).unwrap()}),
  ("request","image/function-reference-export")=>json!({"line":$c::request_image_function_reference_export_typed(id,params::<$c::ImageFunctionReferenceExportTypedParams>($v)).unwrap()}),
  ("request","image/function-reference-resolve")=>json!({"line":$c::request_image_function_reference_resolve_typed(id,params::<$c::ImageFunctionReferenceResolveTypedParams>($v)).unwrap()}),
  ("request","image/analysis-coverage")=>json!({"line":$c::request_image_analysis_coverage_typed(id,params::<$c::ImageAnalysisCoverageTypedParams>($v)).unwrap()}),
  ("request","candidate/open")=>json!({"line":$c::request_candidate_open_typed(id,params::<$c::CandidateOpenTypedParams>($v)).unwrap()}),
  ("request","candidate/apply-intent")=>json!({"line":$c::request_candidate_apply_intent_typed(id,params::<$c::CandidateApplyIntentTypedParams>($v)).unwrap()}),
  ("request","candidate/validate")=>json!({"line":$c::request_candidate_validate_typed(id,params::<$c::CandidateValidateTypedParams>($v)).unwrap()}),
  ("request","candidate/semantic-delta")=>json!({"line":$c::request_candidate_semantic_delta_typed(id,params::<$c::CandidateSemanticDeltaTypedParams>($v)).unwrap()}),
  ("request","candidate/test-plan")=>json!({"line":$c::request_candidate_test_plan_typed(id,params::<$c::CandidateTestPlanTypedParams>($v)).unwrap()}),
  ("request","candidate/test")=>json!({"line":$c::request_candidate_test_typed(id,params::<$c::CandidateTestTypedParams>($v)).unwrap()}),
  ("request","candidate/source-review")=>json!({"line":$c::request_candidate_source_review_typed(id,params::<$c::CandidateSourceReviewTypedParams>($v)).unwrap()}),
  ("request","candidate/analysis-coverage")=>json!({"line":$c::request_candidate_analysis_coverage_typed(id,params::<$c::CandidateAnalysisCoverageTypedParams>($v)).unwrap()}),
  ("request","candidate/recovery-export")=>json!({"line":$c::request_candidate_recovery_export_typed(id,params::<$c::CandidateRecoveryExportTypedParams>($v)).unwrap()}),
  ("decode","workspace/open")=>json!({"payload":$c::decode_request_workspace_open_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/function-reference-export")=>json!({"payload":$c::decode_request_image_function_reference_export_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/function-reference-resolve")=>json!({"payload":$c::decode_request_image_function_reference_resolve_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","image/analysis-coverage")=>json!({"payload":$c::decode_request_image_analysis_coverage_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/open")=>json!({"payload":$c::decode_request_candidate_open_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/apply-intent")=>json!({"payload":$c::decode_request_candidate_apply_intent_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/validate")=>json!({"payload":$c::decode_request_candidate_validate_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/semantic-delta")=>json!({"payload":$c::decode_request_candidate_semantic_delta_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/test-plan")=>json!({"payload":$c::decode_request_candidate_test_plan_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/test")=>json!({"payload":$c::decode_request_candidate_test_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/source-review")=>json!({"payload":$c::decode_request_candidate_source_review_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/analysis-coverage")=>json!({"payload":$c::decode_request_candidate_analysis_coverage_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  ("decode","candidate/recovery-export")=>json!({"payload":$c::decode_request_candidate_recovery_export_typed(($v)["line"].as_str().unwrap(),&id).unwrap().payload}),
  _=>panic!("unknown review operation")
 }
}}}
fn main(){ let a:Vec<String>=env::args().collect(); let v:Value=serde_json::from_slice(&fs::read(&a[4]).unwrap()).unwrap(); let out=match (a[1].as_str(),a[2].as_str()) { ("review","workflows")=>json!({"workflows":review_client::workflows().unwrap()}),("publish","workflows")=>json!({"workflows":publish_client::workflows().unwrap()}),("review",_)=>invoke_review!(review_client,a[2].as_str(),a[3].as_str(),v),("publish",_)=>invoke_publish!(publish_client,a[2].as_str(),a[3].as_str(),v),_=>panic!()}; print!("{}",serde_json::to_string(&out).unwrap()); }
"#;

const TYPESCRIPT_HARNESS: &str = r#"
import * as review from "./review_client.js"; import * as publish from "./publish_client.js";
declare const process: {argv:string[],stdout:{write(value:string):void},getBuiltinModule(name:"fs"):{readFileSync(path:string,encoding:"utf8"):string}};
const [profile,action,method,inputPath]=process.argv.slice(2); const c:any=profile==="review"?review:publish; const value:any=JSON.parse(process.getBuiltinModule("fs").readFileSync(inputPath,"utf8")); const suffix=method.replaceAll("/","_").replaceAll("-","_");
let out:any; if(action==="workflows") out={workflows:c.WORKFLOWS}; else if(action==="request") out={line:c["request_"+suffix+"_typed"]("generated-workflow",value)}; else if(action==="decode") out={payload:c["decode_request_"+suffix+"_typed"](value.line,"generated-workflow").payload}; else throw new Error("unknown action"); process.stdout.write(JSON.stringify(out));
"#;

#[derive(Default)]
struct Transcript {
    rows: Vec<Value>,
}
impl Transcript {
    fn call(
        &mut self,
        harness: &GeneratedHarness,
        profile: &str,
        session: &mut VNextSession,
        method: &str,
        params: Value,
    ) -> Value {
        let request = harness.request(profile, method, params);
        assert!(request.ends_with(b"\n"));
        let response = session.handle_frame(&request).unwrap();
        let request_line = std::str::from_utf8(&request).unwrap();
        let response_line = std::str::from_utf8(&response).unwrap();
        self.rows.push(json!({"phase":profile,"sequence":self.rows.len()+1,"method":method,
            "request_line":request_line,"response_line":response_line,
            "request_sha256":sha256(request_line.as_bytes()),"response_sha256":sha256(response_line.as_bytes())}));
        harness.decode(profile, method, &response)
    }
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for row in &self.rows {
            bytes.extend_from_slice(serde_json::to_string(row).unwrap().as_bytes());
            bytes.push(b'\n');
        }
        bytes
    }
}
fn bound(image: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(image);
    params
}
fn chunks(
    transcript: &mut Transcript,
    harness: &GeneratedHarness,
    profile: &str,
    session: &mut VNextSession,
    method: &str,
    image: &str,
    mut params: Value,
) -> String {
    let mut text = String::new();
    loop {
        params["offset"] = json!(text.len());
        params["chunk_bytes"] = json!(16_384);
        let chunk = transcript.call(
            harness,
            profile,
            session,
            method,
            bound(image, params.clone()),
        );
        assert_eq!(chunk["offset"], text.len());
        text.push_str(chunk["chunk"].as_str().unwrap());
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"], text.len());
            return text;
        }
        assert_eq!(chunk["next_offset"], text.len());
    }
}
fn exact_workflow(workflows: &Value, publish: bool) -> &Value {
    let rows = workflows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let workflow = &rows[0];
    assert_eq!(workflow["id"], "function_signature_review_publish_v1");
    assert_eq!(
        workflow["phases"].as_array().unwrap().len(),
        if publish { 2 } else { 1 }
    );
    let blind = workflow["blind_spots"].as_array().unwrap();
    for (area, status) in [
        ("analysis_completeness", "partial"),
        ("deployment_configuration", "not_inspected"),
        ("generated_file_provenance", "not_inspected"),
        ("generated_artifacts", "not_inspected"),
        ("external_api_behavior", "not_inspected"),
        ("external_consumers", "not_inspected"),
    ] {
        assert_eq!(
            blind.iter().find(|row| row["area"] == area).unwrap()["status"],
            status
        );
    }
    workflow
}
fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
fn sources_digest(fixture: &Fixture) -> String {
    let rows = fixture
        .original
        .keys()
        .map(|path| (path, sha256(&fs::read(fixture.root.join(path)).unwrap())))
        .collect::<BTreeMap<_, _>>();
    sha256(&serde_json::to_vec(&rows).unwrap())
}
fn output_directory() -> Option<PathBuf> {
    let value = std::env::var_os("SEMAPRAX_PRODUCT_WORKFLOW_EVIDENCE_DIR")?;
    let path = PathBuf::from(value);
    assert!(path.is_absolute());
    assert!(path.is_dir());
    Some(path)
}
fn write_success_artifacts(
    language: &str,
    mut observation: Value,
    transcript: &Transcript,
    handoff: &Value,
    generated: &[u8],
) {
    let Some(root) = output_directory() else {
        return;
    };
    let transcript_name = format!("workflow-{language}.transcript.ndjson");
    let handoff_name = format!("workflow-{language}.handoff.json");
    let generated_name = format!("workflow-{language}.generated-client.txt");
    let transcript_bytes = transcript.bytes();
    let handoff_bytes = canonical(handoff);
    fs::write(root.join(&transcript_name), &transcript_bytes).unwrap();
    fs::write(root.join(&handoff_name), &handoff_bytes).unwrap();
    fs::write(root.join(&generated_name), generated).unwrap();
    observation["generated_client"] =
        json!({"path":generated_name,"bytes":generated.len(),"sha256":sha256(generated)});
    observation["artifacts"] = json!([
        {"path":generated_name,"bytes":generated.len(),"sha256":sha256(generated)},
        {"path":handoff_name,"bytes":handoff_bytes.len(),"sha256":sha256(&handoff_bytes)},
        {"path":transcript_name,"bytes":transcript_bytes.len(),"sha256":sha256(&transcript_bytes)}
    ]);
    fs::write(
        root.join(format!("workflow-{language}.observation.json")),
        canonical(&observation),
    )
    .unwrap();
}

fn run_supported_workflow(language: Language) {
    let fixture = Fixture::new(language.name());
    let source_before = sources_digest(&fixture);
    let mut review_profile = VNextSession::open(&fixture.manifest(), policy()).unwrap();
    let review_source = client_source(&mut review_profile, language.name());
    let mut fake_publish = fake_publish_session(&fixture);
    let publish_source = client_source(&mut fake_publish, language.name());
    let harness = GeneratedHarness::build(&fixture, language, &review_source, &publish_source);
    exact_workflow(&harness.workflows("review"), false);
    exact_workflow(&harness.workflows("publish"), true);
    let mut transcript = Transcript::default();
    let image = review_profile.image_revision().to_owned();
    let opened = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "workspace/open",
        json!({}),
    );
    assert_eq!(opened["image_revision"], image);
    let project = opened["project_revision"].as_str().unwrap().to_owned();
    let reference = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "image/function-reference-export",
        bound(&image, json!({"target":"calculator.add"})),
    );
    assert_eq!(reference["facet"], Value::Null);
    let reference_text = serde_json::to_string(&reference).unwrap();
    let resolved = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "image/function-reference-resolve",
        bound(&image, json!({"reference":reference_text})),
    );
    assert_eq!(resolved["function_summary"]["id"], "calculator.add");
    let base_coverage = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "image/analysis-coverage",
        bound(&image, json!({})),
    );
    assert_eq!(base_coverage["areas"].as_array().unwrap().len(), 8);
    assert_eq!(
        base_coverage["areas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["area"] == "declared_source_inputs")
            .unwrap()["status"],
        "known"
    );
    for area in [
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        assert_eq!(
            base_coverage["areas"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["area"] == area)
                .unwrap()["status"],
            "not_inspected"
        );
    }
    let root = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/open",
        bound(&image, json!({})),
    )["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let intention = json!({"kind":"change_function_signature","target":"calculator.add","parameters":[{"from":"right","name":"rhs"},{"from":"left","name":"lhs"},{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]});
    let changed = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/apply-intent",
        bound(
            &image,
            json!({"candidate_revision":root,"intent":intention}),
        ),
    );
    let candidate = changed["candidate_revision"].as_str().unwrap().to_owned();
    let validation = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/validate",
        bound(&image, json!({"candidate_revision":candidate})),
    );
    assert_eq!(validation["independently_replayed"], true);
    let delta_text = chunks(
        &mut transcript,
        &harness,
        "review",
        &mut review_profile,
        "candidate/semantic-delta",
        &image,
        json!({"candidate_revision":candidate,"target":"calculator.add"}),
    );
    let delta: Value = serde_json::from_str(&delta_text).unwrap();
    assert_eq!(delta["candidate_digest"], candidate);
    let test_plan = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/test-plan",
        bound(&image, json!({"candidate_revision":candidate})),
    );
    let test_report = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/test",
        bound(&image, json!({"candidate_revision":candidate})),
    );
    assert_eq!(test_report["passed"], true);
    let source_review_text = chunks(
        &mut transcript,
        &harness,
        "review",
        &mut review_profile,
        "candidate/source-review",
        &image,
        json!({"candidate_revision":candidate}),
    );
    let source_review: Value = serde_json::from_str(&source_review_text).unwrap();
    assert_eq!(
        source_review["schema"],
        "semaprax.project-candidate-source-review.v1"
    );
    assert_eq!(source_review["candidate_revision"], candidate);
    assert_eq!(source_review["base_project_revision"], project);
    let mut source_review_core = source_review.clone();
    let report_revision = source_review_core
        .as_object_mut()
        .unwrap()
        .remove("report_revision")
        .unwrap();
    assert_eq!(
        report_revision,
        domain_digest(
            b"semaprax.project-candidate-source-review.v1\0",
            format!("{source_review_core}\n").as_bytes(),
        )
    );
    let reviewed_files = source_review["files"].as_array().unwrap();
    assert!(!reviewed_files.is_empty());
    for file in reviewed_files {
        assert_eq!(
            file["base_digest"],
            domain_digest(
                b"semaprax.semantic-review.source-digest.v1\0",
                file["base_source"].as_str().unwrap().as_bytes(),
            )
        );
        assert_eq!(
            file["candidate_digest"],
            domain_digest(
                b"semaprax.semantic-review.source-digest.v1\0",
                file["candidate_source"].as_str().unwrap().as_bytes(),
            )
        );
        assert!(!file["source_diff"].as_str().unwrap().is_empty());
        assert_eq!(
            file["source_diff_digest"],
            domain_digest(
                b"semaprax.candidate.source-diff.v1\0",
                file["source_diff"].as_str().unwrap().as_bytes(),
            )
        );
    }
    let candidate_coverage = transcript.call(
        &harness,
        "review",
        &mut review_profile,
        "candidate/analysis-coverage",
        bound(&image, json!({"candidate_revision":candidate})),
    );
    assert_eq!(candidate_coverage["candidate_revision"], candidate);
    assert_eq!(candidate_coverage["areas"].as_array().unwrap().len(), 8);
    assert_eq!(
        candidate_coverage["areas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["area"] == "declared_source_inputs")
            .unwrap()["status"],
        "known"
    );
    for area in [
        "declared_external_contracts",
        "deployment_configuration",
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        assert_eq!(
            candidate_coverage["areas"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["area"] == area)
                .unwrap()["status"],
            "not_inspected"
        );
    }
    let recovery_text = chunks(
        &mut transcript,
        &harness,
        "review",
        &mut review_profile,
        "candidate/recovery-export",
        &image,
        json!({"candidate_revision":candidate}),
    );
    let recovery: Value = serde_json::from_str(&recovery_text).unwrap();
    let expected_candidate = ProjectCandidate::restore(
        Arc::clone(&fixture.revision),
        fixture.revision.project_revision(),
        recovery_text.as_bytes(),
    )
    .unwrap();
    assert_eq!(expected_candidate.candidate_digest(), candidate);
    assert_eq!(
        source_review_text,
        expected_candidate.source_review(&candidate).unwrap()
    );
    assert_eq!(
        candidate_coverage,
        serde_json::from_str::<Value>(&expected_candidate.analysis_coverage(&candidate).unwrap())
            .unwrap()
    );
    let delta_verification: Value = serde_json::from_str(
        &expected_candidate
            .verify_semantic_delta(&candidate, "calculator.add", delta_text.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(delta_verification["candidate_digest"], candidate);
    assert_eq!(delta_verification["target"], "calculator.add");
    assert_eq!(
        test_plan,
        serde_json::from_str::<Value>(&expected_candidate.test_plan(&candidate).unwrap()).unwrap()
    );
    let repeated_test = expected_candidate
        .execute_tests(
            &candidate,
            &CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap(),
        )
        .unwrap();
    assert!(repeated_test.passed());
    assert_eq!(
        test_report,
        serde_json::from_str::<Value>(repeated_test.to_json()).unwrap()
    );
    let source = |path: &str| {
        expected_candidate
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap()
            .source()
    };
    let core = source("src/core.spx");
    assert!(core.contains("@id(\"calculator.add\")"));
    assert!(core.contains("fn add(rhs: i64, lhs: i64, offset: i64) -> i64"));
    assert!(core.contains("lhs + rhs"));
    let tests = source("src/tests.spx");
    assert!(
        tests.contains("let spx_sig_stage_2 = 19; let spx_sig_stage_3 = 23;"),
        "{tests}"
    );
    assert!(
        tests.contains("add(spx_sig_stage_3, spx_sig_stage_2, 0)"),
        "{tests}"
    );
    let app = source("src/app.spx");
    assert!(
        app.contains("let spx_sig_stage_0 = multiply(6, 7); let spx_sig_stage_1 = subtract(divide(4, 2), 2);"),
        "{app}"
    );
    assert!(
        app.contains("add(spx_sig_stage_1, spx_sig_stage_0, 0)"),
        "{app}"
    );
    let intention_text = serde_json::to_string(&intention).unwrap();
    let handoff = json!({
        "schema":"semaprax.graph-operational-phase1-product-workflow-handoff.v1",
        "workflow":"function_signature_review_publish_v1",
        "language":language.name(),
        "candidate_revision":candidate,
        "compact_reference":reference_text,
        "typed_intention":intention_text,
        "validation":validation,
        "semantic_delta":delta_text,
        "test_plan":test_plan,
        "test_report":test_report,
        "source_review_sha256":sha256(source_review_text.as_bytes()),
        "base_analysis_coverage_sha256":sha256(&canonical(&base_coverage)),
        "candidate_analysis_coverage_sha256":sha256(&canonical(&candidate_coverage)),
        "recovery_capsule":recovery_text,
    });
    let reviewed_handoff: Value = serde_json::from_slice(&canonical(&handoff)).unwrap();
    assert_eq!(reviewed_handoff, handoff);
    assert_eq!(reviewed_handoff["candidate_revision"], candidate);
    assert_eq!(
        reviewed_handoff["recovery_capsule"].as_str().unwrap(),
        recovery_text
    );
    review_profile.finish().unwrap();
    fixture.unchanged();

    // All generation and compilation happened before opening the deadline-bound process provider.
    let authority =
        CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 4096, 60_000).unwrap();
    let repository = authority.repository().unwrap();
    let target = CandidateGitTarget::new(&repository.identity, BRANCH, &fixture.base, "").unwrap();
    let metadata = CandidateGitCommitMetadata::new(
        "Host",
        "host@example.invalid",
        2,
        "Reviewed signature evolution\n",
    )
    .unwrap();
    let mut host =
        GitCommitHost::new(&fixture.manifest(), target, metadata, Box::new(authority)).unwrap();
    let approval = host
        .approve(reviewed_handoff["candidate_revision"].as_str().unwrap())
        .unwrap();
    let mut publish = VNextSession::open(&fixture.manifest(), policy())
        .unwrap()
        .with_git_commit_host(host)
        .unwrap();
    assert_eq!(client_source(&mut publish, language.name()), publish_source);
    let publish_image = publish.image_revision().to_owned();
    assert_eq!(publish_image, image);
    let publish_open = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "workspace/open",
        json!({}),
    );
    assert_eq!(publish_open["image_revision"], image);
    assert_eq!(publish_open["project_revision"], project);
    let repeat_resolved = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "image/function-reference-resolve",
        bound(
            &image,
            json!({"reference":serde_json::to_string(&reference).unwrap()}),
        ),
    );
    assert_eq!(repeat_resolved, resolved);
    let restored = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "candidate/recovery-restore",
        bound(&image, json!({"capsule":recovery})),
    );
    assert_eq!(restored["candidate_revision"], candidate);
    assert_eq!(
        restored["project_revision"],
        expected_candidate.revision().project_revision()
    );
    assert_eq!(restored["base_revision"], project);
    let repeat_validation = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "candidate/validate",
        bound(&image, json!({"candidate_revision":candidate})),
    );
    assert_eq!(repeat_validation, validation);
    let repeat_review = chunks(
        &mut transcript,
        &harness,
        "publish",
        &mut publish,
        "candidate/source-review",
        &image,
        json!({"candidate_revision":candidate}),
    );
    assert_eq!(repeat_review, source_review_text);
    let pre = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "source-commit/status",
        bound(&image, json!({})),
    );
    assert_eq!(pre["state"], "available");
    assert_eq!(pre["pending_approval"]["candidate_revision"], candidate);
    assert_eq!(pre["pending_approval"]["approval_revision"], approval);
    assert_eq!(pre["report_revision"], Value::Null);
    assert_eq!(pre["last_error_codes"], json!([]));
    let committed = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "candidate/commit",
        bound(
            &image,
            json!({"candidate_revision":candidate,"approval_revision":approval}),
        ),
    );
    assert_eq!(committed["state"], "published");
    let post = transcript.call(
        &harness,
        "publish",
        &mut publish,
        "source-commit/status",
        bound(&image, json!({})),
    );
    assert_eq!(post["state"], "published");
    assert_eq!(post["pending_approval"], Value::Null);
    assert_eq!(post["report_revision"], committed["report_revision"]);
    assert_eq!(post["last_error_codes"], json!([]));
    let receipt_text = chunks(
        &mut transcript,
        &harness,
        "publish",
        &mut publish,
        "candidate/commit-report",
        &image,
        json!({"report_revision":committed["report_revision"]}),
    );
    let receipt: Value = serde_json::from_str(&receipt_text).unwrap();
    publish.finish().unwrap();
    assert_eq!(receipt.as_object().unwrap().len(), 18);
    assert_eq!(
        receipt["schema"],
        "semaprax.project-candidate-git-publication.v1"
    );
    assert_eq!(receipt["publication"], "git_branch_ref_compare_and_swap");
    assert_eq!(receipt["working_tree_rewritten"], false);
    assert_eq!(receipt["project_manifest_changed"], false);
    assert_eq!(receipt["managed_active_changed"], false);
    assert_eq!(
        receipt["source_authority"],
        "explicit_host_git_ref_authority"
    );
    assert_eq!(receipt["tests"], "not_run");
    assert_eq!(
        receipt["nonclaims"],
        json!([
            "no_atomic_raw_working_tree_rewrite",
            "no_network_push_or_remote_publication",
            "no_signature_or_approval_service",
            "unreachable_objects_may_remain_after_failure"
        ])
    );
    let head = fixture.head();
    assert_ne!(head, fixture.base);
    assert_eq!(receipt["published_commit"], head);
    assert_eq!(receipt["previous_commit"], fixture.base);
    assert_eq!(receipt["git_object_format"], "sha256");
    let commit_text = String::from_utf8(fixture.run(&["cat-file", "-p", &head], &[])).unwrap();
    let tree = commit_text
        .lines()
        .find_map(|line| line.strip_prefix("tree "))
        .unwrap()
        .to_owned();
    let parent = commit_text
        .lines()
        .find_map(|line| line.strip_prefix("parent "))
        .unwrap()
        .to_owned();
    assert_eq!(parent, fixture.base);
    assert_eq!(receipt["repository"], repository.identity);
    assert_eq!(receipt["reference"], BRANCH);
    assert_eq!(receipt["tree"], tree);
    assert_eq!(receipt["approved_candidate_digest"], candidate);
    assert_eq!(receipt["base_project_revision"], project);
    assert_eq!(
        receipt["candidate_project_revision"],
        expected_candidate.revision().project_revision()
    );
    assert_eq!(
        receipt["updated_source_paths"],
        Value::Array(
            source_review["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|file| file["path"].clone())
                .collect()
        )
    );
    let mut source_objects = Vec::new();
    for source in expected_candidate.revision().sources() {
        let spec = format!("{BRANCH}:{}", source.path());
        assert_eq!(
            fixture.run(&["show", &spec], &[]),
            source.source().as_bytes()
        );
        let object = String::from_utf8(fixture.run(&["rev-parse", &spec], &[]))
            .unwrap()
            .trim_end()
            .to_owned();
        source_objects.push(json!({"path":source.path(),"object":object}));
    }
    assert_eq!(
        fixture.run(&["show", &format!("{BRANCH}:semaprax.toml")], &[]),
        fixture.original["semaprax.toml"]
    );
    assert_eq!(
        fixture.run(&["show", &format!("{BRANCH}:keep.sh")], &[]),
        b"unrelated executable entry\n"
    );
    let keep_entry = String::from_utf8(fixture.run(&["ls-tree", BRANCH, "keep.sh"], &[])).unwrap();
    assert!(keep_entry.starts_with("100755 blob "), "{keep_entry}");
    assert!(keep_entry.ends_with("\tkeep.sh\n"), "{keep_entry}");
    let manifest_spec = format!("{BRANCH}:semaprax.toml");
    source_objects.push(json!({"path":"semaprax.toml","object":String::from_utf8(fixture.run(&["rev-parse",&manifest_spec],&[])).unwrap().trim_end()}));
    source_objects.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    fixture.unchanged();
    let source_after = sources_digest(&fixture);
    assert_eq!(source_after, source_before);
    let review_policy = json!({"candidate_prepare":true,"source_commit":false,"test_policy":{"engine":"project_interpreter","max_steps":100000,"max_execution_bytes":65536,"max_report_bytes":262144,"request_overrides":false}});
    let publish_policy = json!({"candidate_prepare":true,"source_commit":true,"test_policy":{"engine":"project_interpreter","max_steps":100000,"max_execution_bytes":65536,"max_report_bytes":262144,"request_overrides":false},"repository":{"object_format":"sha256","identity":repository.identity,"ref":BRANCH,"expected_old":fixture.base},"approval":{"candidate_revision":candidate,"approval_revision":approval}});
    let observation = json!({
      "schema":"semaprax.graph-operational-phase1-product-workflow-observation.v1","workflow":"function_signature_review_publish_v1","language":language.name(),"terminal_outcome":"published",
      "generated_client":{"path":"","bytes":harness.source_bytes,"sha256":harness.source_sha256},"methods":{"review":REVIEW_METHODS,"publish":PUBLISH_METHODS},
      "policies":{"review":review_policy,"publish":publish_policy},
      "bindings":{"image_revision":image,"project_revision":project,"candidate_revision":candidate,"compact_reference_sha256":sha256(reference_text.as_bytes()),"intention_sha256":sha256(intention_text.as_bytes()),"validation_sha256":sha256(&canonical(&validation)),"semantic_delta_sha256":sha256(delta_text.as_bytes()),"test_plan_sha256":sha256(&canonical(&test_plan)),"test_report_sha256":sha256(&canonical(&test_report)),"source_review_sha256":sha256(source_review_text.as_bytes()),"base_analysis_coverage_sha256":sha256(&canonical(&base_coverage)),"candidate_analysis_coverage_sha256":sha256(&canonical(&candidate_coverage)),"recovery_capsule_sha256":sha256(recovery_text.as_bytes()),"review_policy_sha256":sha256(&canonical(&review_policy)),"publish_policy_sha256":sha256(&canonical(&publish_policy)),"approval_revision":approval,"commit_report_revision":committed["report_revision"]},
      "blind_spots":{"analysis_completeness":"partial","deployment_configuration":"not_inspected","generated_file_provenance":"not_inspected","generated_artifacts":"not_inspected","external_api_behavior":"not_inspected","runtime_environment":"partial_bounded_reference_interpreter","external_consumers":"not_inspected"},
      "source":{"before_sha256":source_before,"after_sha256":source_after,"unchanged":true},
      "git":{"object_format":"sha256","ref":BRANCH,"old":fixture.base,"new":head,"parent":parent,"tree":tree,"source_objects":source_objects,"independently_inspected":true},
      "receipt":{"bytes":receipt_text.len(),"sha256":sha256(receipt_text.as_bytes()),"commit":receipt["published_commit"],"complete":true},"artifacts":[]
    });
    write_success_artifacts(
        language.name(),
        observation,
        &transcript,
        &handoff,
        &harness.source_artifact,
    );
    if matches!(language, Language::TypeScript) {
        let request = harness.request("review", "workspace/open", json!({}));
        let mut malformed: Value =
            serde_json::from_str(transcript.rows[0]["response_line"].as_str().unwrap()).unwrap();
        malformed["id"] = json!("wrong-generated-workflow");
        let response = malformed.to_string();
        harness.rejects_decode("review", "workspace/open", &response);
        complete_hostile_typescript_artifact(&request, response.as_bytes());
    }
}

fn complete_hostile_typescript_artifact(request: &[u8], response: &[u8]) {
    let Some(root) = output_directory() else {
        return;
    };
    let transcript_path = root.join("hostile-workflow.transcript.ndjson");
    let observation_path = root.join("hostile-workflow.observation.json");
    let mut observation: Value =
        serde_json::from_slice(&fs::read(&observation_path).unwrap()).unwrap();
    let cases = observation["cases"].as_array_mut().unwrap();
    assert_eq!(cases.len(), 9);
    assert_eq!(cases[8]["case"], "malformed_response_rust");
    cases.push(json!({"case":"malformed_response_typescript","terminal_outcome":"transport_uncertain_no_publish_claim","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"}));
    let request_line = std::str::from_utf8(request).unwrap();
    let response_line = std::str::from_utf8(response).unwrap();
    let row = json!({"case":"malformed_response_typescript","sequence":1,"method":"workspace/open","request_line":request_line,"response_line":response_line,"request_sha256":sha256(request_line.as_bytes()),"response_sha256":sha256(response_line.as_bytes())});
    let mut transcript = fs::read(&transcript_path).unwrap();
    transcript.extend_from_slice(serde_json::to_string(&row).unwrap().as_bytes());
    transcript.push(b'\n');
    observation["artifacts"] = json!([{"path":"hostile-workflow.transcript.ndjson","bytes":transcript.len(),"sha256":sha256(&transcript)}]);
    let tag = format!("{}.tmp", std::process::id());
    let transcript_tmp = root.join(format!("hostile-workflow.transcript.ndjson.{tag}"));
    let observation_tmp = root.join(format!("hostile-workflow.observation.json.{tag}"));
    fs::write(&transcript_tmp, transcript).unwrap();
    fs::write(&observation_tmp, canonical(&observation)).unwrap();
    fs::rename(transcript_tmp, transcript_path).unwrap();
    fs::rename(observation_tmp, observation_path).unwrap();
}

#[test]
fn generated_python_reference_review_export_and_real_git_commit() {
    run_supported_workflow(Language::Python);
}
#[test]
fn generated_rust_reference_review_export_and_real_git_commit() {
    run_supported_workflow(Language::Rust);
}
#[test]
#[ignore = "requires provisioned absolute SEMAPRAX_TEST_TSC 5.8.3 and SEMAPRAX_TEST_NODE >=22"]
fn provisioned_typescript_reference_review_export_and_real_git_commit() {
    run_supported_workflow(Language::TypeScript);
}
