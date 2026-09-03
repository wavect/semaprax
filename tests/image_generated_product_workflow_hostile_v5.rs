//! Independent generated-client execution of the supported v5 product workflow.
#![cfg(unix)]

use semaprax::image_transport::{GitCommitHost, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitProcessAuthority,
    CandidateGitRefUpdate, CandidateGitRepository, CandidateGitTarget, CandidateTestPolicy,
    ProjectRevision,
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

fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
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

fn policy(steps: usize) -> VNextPolicy {
    VNextPolicy {
        candidate_prepare: true,
        test_policy: Some(CandidateTestPolicy::new(steps, 65_536, 262_144).unwrap()),
        ..Default::default()
    }
}

fn bound(image: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(image);
    params
}

fn request(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"generated-workflow","method":method,"params":params})
        .to_string()
        .into_bytes()
}

fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&request(method, params)).unwrap()).unwrap()
}

fn payload(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let response = call(session, method, params);
    assert!(response.get("error").is_none(), "{method}: {response}");
    response["result"]["payload"].clone()
}

#[derive(Default)]
struct HostileTranscript {
    rows: Vec<Value>,
}
impl HostileTranscript {
    fn call(
        &mut self,
        case: &str,
        session: &mut VNextSession,
        method: &str,
        params: Value,
    ) -> Value {
        let request = request(method, params);
        let response = session.handle_frame(&request).unwrap();
        self.push(case, method, &request, &response);
        serde_json::from_slice(&response).unwrap()
    }
    fn push(&mut self, case: &str, method: &str, request: &[u8], response: &[u8]) {
        let sequence = self.rows.iter().filter(|row| row["case"] == case).count() + 1;
        let request_line = std::str::from_utf8(request).unwrap();
        let response_line = std::str::from_utf8(response).unwrap();
        self.rows.push(json!({
            "case":case,
            "sequence":sequence,
            "method":method,
            "request_line":request_line,
            "response_line":response_line,
            "request_sha256":sha256(request_line.as_bytes()),
            "response_sha256":sha256(response_line.as_bytes())
        }));
    }
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for row in &self.rows {
            bytes.extend_from_slice(&serde_json::to_vec(row).unwrap());
            bytes.push(b'\n');
        }
        bytes
    }
}

struct Reviewed {
    image: String,
    candidate: String,
    capsule: Value,
}

fn exact_reviewed(session: &mut VNextSession) -> Reviewed {
    let image = session.image_revision().to_owned();
    payload(session, "workspace/open", json!({}));
    let root = payload(session, "candidate/open", bound(&image, json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let intention = json!({"kind":"change_function_signature","target":"calculator.add","parameters":[{"from":"right","name":"rhs"},{"from":"left","name":"lhs"},{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]});
    let candidate = payload(
        session,
        "candidate/apply-intent",
        bound(
            &image,
            json!({"candidate_revision":root,"intent":intention}),
        ),
    )["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let capsule_text = chunks(
        session,
        "candidate/recovery-export",
        &image,
        json!({"candidate_revision":candidate}),
    );
    Reviewed {
        image,
        candidate,
        capsule: serde_json::from_str(&capsule_text).unwrap(),
    }
}

fn chunks(session: &mut VNextSession, method: &str, image: &str, mut params: Value) -> String {
    let mut text = String::new();
    loop {
        params["offset"] = json!(text.len());
        params["chunk_bytes"] = json!(16_384);
        let part = payload(session, method, bound(image, params.clone()));
        assert_eq!(part["offset"], text.len());
        text.push_str(part["chunk"].as_str().unwrap());
        if part["next_offset"].is_null() {
            assert_eq!(part["total_bytes"], text.len());
            return text;
        }
    }
}

fn change_add_body(fixture: &Fixture) {
    let path = fixture.root.join("src/core.spx");
    let source = fs::read_to_string(&path).unwrap();
    let changed = source.replacen("left + right", "left + right + 0", 1);
    assert_ne!(changed, source);
    let parsed = semaprax::parse(&changed, "src/core.spx").unwrap();
    fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

fn process_authority(fixture: &Fixture) -> CandidateGitProcessAuthority {
    CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 4096, 60_000).unwrap()
}

fn publish_session(
    fixture: &Fixture,
    candidate: &str,
    expected_old: &str,
    authority: Box<dyn CandidateGitAuthority>,
) -> (VNextSession, String) {
    let repository = authority.repository().unwrap();
    let target = CandidateGitTarget::new(&repository.identity, BRANCH, expected_old, "").unwrap();
    let metadata = CandidateGitCommitMetadata::new(
        "Host",
        "host@example.invalid",
        2,
        "Reviewed signature evolution\n",
    )
    .unwrap();
    let mut host = GitCommitHost::new(&fixture.manifest(), target, metadata, authority).unwrap();
    let approval = host.approve(candidate).unwrap();
    let session = VNextSession::open(&fixture.manifest(), policy(100_000))
        .unwrap()
        .with_git_commit_host(host)
        .unwrap();
    (session, approval)
}

#[derive(Default)]
struct LostCasState {
    calls: usize,
    new_commit: Option<String>,
}
struct UncertainAfterRealCas {
    inner: CandidateGitProcessAuthority,
    state: Arc<std::sync::Mutex<LostCasState>>,
}
impl CandidateGitAuthority for UncertainAfterRealCas {
    fn repository(&self) -> io::Result<CandidateGitRepository> {
        self.inner.repository()
    }
    fn read_ref(&mut self, reference: &str) -> io::Result<Option<String>> {
        self.inner.read_ref(reference)
    }
    fn read_object(&mut self, oid: &str, max_bytes: usize) -> io::Result<CandidateGitObject> {
        self.inner.read_object(oid, max_bytes)
    }
    fn write_object(
        &mut self,
        kind: CandidateGitObjectKind,
        bytes: &[u8],
        expected_oid: &str,
    ) -> io::Result<()> {
        self.inner.write_object(kind, bytes, expected_oid)
    }
    fn compare_and_swap_ref(
        &mut self,
        reference: &str,
        expected_old: &str,
        new_commit: &str,
    ) -> io::Result<CandidateGitRefUpdate> {
        let update = self
            .inner
            .compare_and_swap_ref(reference, expected_old, new_commit)?;
        if update == CandidateGitRefUpdate::Updated {
            let mut state = self.state.lock().unwrap();
            state.calls += 1;
            state.new_commit = Some(new_commit.to_owned());
            Err(io::Error::other(
                "injected response loss after real Git ref update",
            ))
        } else {
            Ok(update)
        }
    }
}

fn generated_client_source(session: &mut VNextSession, language: &str) -> String {
    payload(session, "protocol/client", json!({"language":language}))["source"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn generated_malformed_python(fixture: &Fixture, source: &str, malformed: &str) -> String {
    let root = fixture.root.join("hostile-python");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("client.py"), source).unwrap();
    fs::write(
        root.join("check.py"),
        r#"import importlib.util,json,pathlib,sys
root=pathlib.Path(__file__).parent
spec=importlib.util.spec_from_file_location('client',root/'client.py')
client=importlib.util.module_from_spec(spec);sys.modules['client']=client;spec.loader.exec_module(client)
line=(root/'response.json').read_text(encoding='utf-8')
request=client.request_workspace_open_typed('generated-workflow',{})
try: client.decode_request_workspace_open_typed(line,'generated-workflow')
except ValueError: rejected=True
else: rejected=False
sys.stdout.buffer.write(json.dumps({'rejected':rejected,'request_line':request},sort_keys=True,separators=(',',':')).encode())
"#,
    )
    .unwrap();
    fs::write(root.join("response.json"), malformed).unwrap();
    let python = selected_command("SEMAPRAX_TEST_PYTHON", "python3");
    let output = Command::new(python)
        .arg("-I")
        .arg(root.join("check.py"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["rejected"], true);
    value["request_line"].as_str().unwrap().to_owned()
}

fn lock_version(name: &str) -> String {
    let lock =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock")).unwrap();
    let marker = format!("name = \"{name}\"\nversion = \"");
    lock.split(&marker)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}

fn generated_malformed_rust(fixture: &Fixture, source: &str, malformed: &str) -> String {
    let root = fixture.root.join("hostile-rust");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/client.rs"), source).unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"mod client;
use serde_json::{json,Value};use std::fs;
fn main(){let line=fs::read_to_string("response.json").unwrap();let id=client::RpcId::Text("generated-workflow".into());let params:client::WorkspaceOpenTypedParams=serde_json::from_value(json!({})).unwrap();let request=client::request_workspace_open_typed(id,params).unwrap();let id=client::RpcId::Text("generated-workflow".into());let rejected=client::decode_request_workspace_open_typed(&line,&id).is_err();let out:Value=json!({"rejected":rejected,"request_line":request});print!("{}",serde_json::to_string(&out).unwrap());}
"#,
    )
    .unwrap();
    fs::write(root.join("response.json"), malformed).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname=\"hostile-generated-client\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[dependencies]\nserde={{version=\"={}\",features=[\"derive\"]}}\nserde_json=\"={}\"\n",
            lock_version("serde"),
            lock_version("serde_json")
        ),
    )
    .unwrap();
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
    let output = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["rejected"], true);
    value["request_line"].as_str().unwrap().to_owned()
}

fn malformed_workspace_response(fixture: &Fixture) -> String {
    let mut session = VNextSession::open(&fixture.manifest(), policy(100_000)).unwrap();
    let mut response = call(&mut session, "workspace/open", json!({}));
    response["id"] = json!("wrong-generated-workflow");
    serde_json::to_string(&response).unwrap()
}

fn atomic_write(path: &Path, bytes: &[u8]) {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temporary).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    fs::rename(temporary, path).unwrap();
}

fn write_intermediate(transcript: &HostileTranscript) {
    let Some(directory) = std::env::var_os("SEMAPRAX_PRODUCT_WORKFLOW_EVIDENCE_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    assert!(directory.is_absolute() && directory.is_dir());
    let transcript_name = "hostile-workflow.transcript.ndjson";
    let transcript_bytes = transcript.bytes();
    let cases = json!([
        {"case":"stale_reference","terminal_outcome":"stale_subject","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"source_drift","terminal_outcome":"stale_subject","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"failed_test","terminal_outcome":"review_rejected","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged","basis":"distinct_insufficient_fuel_host_test_policy_nonpassing"},
        {"case":"tampered_recovery","terminal_outcome":"publish_precondition_rejected","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"wrong_approval","terminal_outcome":"publish_precondition_rejected","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"definite_pre_pivot_failure","terminal_outcome":"publish_failed_pre_pivot","commit_invoked":true,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"post_ref_result_loss","terminal_outcome":"publication_uncertain","commit_invoked":true,"blind_retry_allowed":false,"git_ref_outcome":"updated_to_prepared_commit"},
        {"case":"malformed_response_python","terminal_outcome":"transport_uncertain_no_publish_claim","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"},
        {"case":"malformed_response_rust","terminal_outcome":"transport_uncertain_no_publish_claim","commit_invoked":false,"blind_retry_allowed":false,"git_ref_outcome":"unchanged"}
    ]);
    let observation = json!({
        "schema":"semaprax.graph-operational-phase1-product-workflow-hostile-observation.v1",
        "workflow":"function_signature_review_publish_v1",
        "cases":cases,
        "artifacts":[{"path":transcript_name,"bytes":transcript_bytes.len(),"sha256":sha256(&transcript_bytes)}]
    });
    atomic_write(&directory.join(transcript_name), &transcript_bytes);
    atomic_write(
        &directory.join("hostile-workflow.observation.json"),
        &serde_json::to_vec(&observation).unwrap(),
    );
}

#[test]
fn hostile_workflow_transitions_fail_closed() {
    let mut transcript = HostileTranscript::default();

    let original = Fixture::new("hostile-stale-reference-original");
    let mut old = VNextSession::open(&original.manifest(), policy(100_000)).unwrap();
    let old_image = old.image_revision().to_owned();
    let reference = payload(
        &mut old,
        "image/function-reference-export",
        bound(
            &old_image,
            json!({"target":"calculator.add","facet":"signature"}),
        ),
    );
    let changed = Fixture::new("hostile-stale-reference-changed");
    change_add_body(&changed);
    let mut fresh = VNextSession::open(&changed.manifest(), policy(100_000)).unwrap();
    let fresh_image = fresh.image_revision().to_owned();
    let response = transcript.call(
        "stale_reference",
        &mut fresh,
        "image/function-reference-resolve",
        bound(
            &fresh_image,
            json!({"reference":serde_json::to_string(&reference).unwrap()}),
        ),
    );
    assert!(response.to_string().contains("SPX-G363"), "{response}");
    assert_eq!(changed.head(), changed.base);

    let drift = Fixture::new("hostile-source-drift");
    let mut session = VNextSession::open(&drift.manifest(), policy(100_000)).unwrap();
    let reviewed = exact_reviewed(&mut session);
    change_add_body(&drift);
    let response = transcript.call(
        "source_drift",
        &mut session,
        "candidate/test",
        bound(
            &reviewed.image,
            json!({"candidate_revision":reviewed.candidate}),
        ),
    );
    assert!(response.get("error").is_some(), "{response}");
    assert_eq!(drift.head(), drift.base);

    let failed = Fixture::new("hostile-failed-test");
    let mut session = VNextSession::open(&failed.manifest(), policy(1)).unwrap();
    let reviewed = exact_reviewed(&mut session);
    let response = transcript.call(
        "failed_test",
        &mut session,
        "candidate/test",
        bound(
            &reviewed.image,
            json!({"candidate_revision":reviewed.candidate}),
        ),
    );
    assert_eq!(response["result"]["payload"]["passed"], false);
    assert_eq!(response["result"]["payload"]["options"]["max_steps"], 1);
    assert_eq!(failed.head(), failed.base);
    failed.unchanged();

    let tampered = Fixture::new("hostile-tampered-recovery");
    let mut review = VNextSession::open(&tampered.manifest(), policy(100_000)).unwrap();
    let mut reviewed = exact_reviewed(&mut review);
    let authority = process_authority(&tampered);
    let base = tampered.base.clone();
    let (mut publish, _) =
        publish_session(&tampered, &reviewed.candidate, &base, Box::new(authority));
    reviewed.capsule["candidate_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    let image = publish.image_revision().to_owned();
    let response = transcript.call(
        "tampered_recovery",
        &mut publish,
        "candidate/recovery-restore",
        bound(&image, json!({"capsule":reviewed.capsule})),
    );
    assert!(response.to_string().contains("SPX-G238"), "{response}");
    assert_eq!(tampered.head(), tampered.base);

    let wrong = Fixture::new("hostile-wrong-approval");
    let mut review = VNextSession::open(&wrong.manifest(), policy(100_000)).unwrap();
    let reviewed = exact_reviewed(&mut review);
    let authority = process_authority(&wrong);
    let base = wrong.base.clone();
    let (mut publish, approval) =
        publish_session(&wrong, &reviewed.candidate, &base, Box::new(authority));
    let image = publish.image_revision().to_owned();
    let restored = payload(
        &mut publish,
        "candidate/recovery-restore",
        bound(&image, json!({"capsule":reviewed.capsule})),
    );
    assert_eq!(restored["candidate_revision"], reviewed.candidate);
    let response = transcript.call(
        "wrong_approval",
        &mut publish,
        "candidate/commit",
        bound(
            &image,
            json!({"candidate_revision":reviewed.candidate,"approval_revision":format!("sha256:{}", "0".repeat(64))}),
        ),
    );
    assert!(response.to_string().contains("SPX-G286"), "{response}");
    let status = payload(
        &mut publish,
        "source-commit/status",
        bound(&image, json!({})),
    );
    assert_eq!(status["pending_approval"]["approval_revision"], approval);
    assert_eq!(wrong.head(), wrong.base);

    let definite = Fixture::new("hostile-definite-pre-pivot");
    let mut review = VNextSession::open(&definite.manifest(), policy(100_000)).unwrap();
    let reviewed = exact_reviewed(&mut review);
    let authority = process_authority(&definite);
    let (mut publish, approval) = publish_session(
        &definite,
        &reviewed.candidate,
        &"0".repeat(64),
        Box::new(authority),
    );
    let image = publish.image_revision().to_owned();
    payload(
        &mut publish,
        "candidate/recovery-restore",
        bound(&image, json!({"capsule":reviewed.capsule})),
    );
    let response = transcript.call(
        "definite_pre_pivot_failure",
        &mut publish,
        "candidate/commit",
        bound(
            &image,
            json!({"candidate_revision":reviewed.candidate,"approval_revision":approval}),
        ),
    );
    assert!(response.to_string().contains("SPX-G265"), "{response}");
    let status = payload(
        &mut publish,
        "source-commit/status",
        bound(&image, json!({})),
    );
    assert!(status["pending_approval"].is_null());
    assert_eq!(definite.head(), definite.base);

    let uncertain = Fixture::new("hostile-post-ref-loss");
    let mut review = VNextSession::open(&uncertain.manifest(), policy(100_000)).unwrap();
    let reviewed = exact_reviewed(&mut review);
    let state = Arc::new(std::sync::Mutex::new(LostCasState::default()));
    let authority = UncertainAfterRealCas {
        inner: process_authority(&uncertain),
        state: Arc::clone(&state),
    };
    let base = uncertain.base.clone();
    let (mut publish, approval) =
        publish_session(&uncertain, &reviewed.candidate, &base, Box::new(authority));
    let image = publish.image_revision().to_owned();
    payload(
        &mut publish,
        "candidate/recovery-restore",
        bound(&image, json!({"capsule":reviewed.capsule})),
    );
    let params = bound(
        &image,
        json!({"candidate_revision":reviewed.candidate,"approval_revision":approval}),
    );
    let response = transcript.call(
        "post_ref_result_loss",
        &mut publish,
        "candidate/commit",
        params.clone(),
    );
    assert!(response.to_string().contains("SPX-G267"), "{response}");
    let prepared = state.lock().unwrap().new_commit.clone().unwrap();
    assert_eq!(state.lock().unwrap().calls, 1);
    assert_eq!(uncertain.head(), prepared);
    let status = payload(
        &mut publish,
        "source-commit/status",
        bound(&image, json!({})),
    );
    assert_eq!(status["state"], "publication_uncertain");
    let retry = transcript.call(
        "post_ref_result_loss",
        &mut publish,
        "candidate/commit",
        params,
    );
    assert!(retry.to_string().contains("SPX-G287"), "{retry}");
    assert_eq!(uncertain.head(), prepared);

    for language in ["python", "rust"] {
        let fixture = Fixture::new(&format!("hostile-malformed-{language}"));
        let malformed = malformed_workspace_response(&fixture);
        let mut profile = VNextSession::open(&fixture.manifest(), policy(100_000)).unwrap();
        let source = generated_client_source(&mut profile, language);
        let request = if language == "python" {
            generated_malformed_python(&fixture, &source, &malformed)
        } else {
            generated_malformed_rust(&fixture, &source, &malformed)
        };
        transcript.push(
            &format!("malformed_response_{language}"),
            "workspace/open",
            request.as_bytes(),
            malformed.as_bytes(),
        );
        assert_eq!(fixture.head(), fixture.base);
        fixture.unchanged();
    }

    write_intermediate(&transcript);
}
