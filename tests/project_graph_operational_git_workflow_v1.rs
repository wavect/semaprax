//! Integrated v5 + real bare Git workflow. Authored, not executed locally.
#![cfg(unix)]

use semaprax::ast::{Expr, ExprKind, Statement};
use semaprax::image_transport::{
    GitCommitHost, VNextPolicy, VNextSession, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA,
};
use semaprax::project::{
    with_authenticated_project, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitProcessAuthority,
    CandidateGitRefUpdate, CandidateGitRepository, CandidateGitTarget, CandidateTestPolicy,
    ProjectCandidate, ProjectRevision,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const BRANCH: &str = "refs/heads/review";
const TASK_PHASES: [&str; 12] = [
    "workspace_snapshot",
    "stable_id_selection",
    "signature_intent",
    "caller_migration",
    "identity_preservation",
    "invariant_preservation",
    "independent_replay",
    "affected_tests",
    "target_admission",
    "semantic_and_source_review",
    "concurrent_change_reconciliation",
    "separate_git_publication",
];
const PATHS: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct UncertainAfterRealCas {
    inner: CandidateGitProcessAuthority,
    state: Arc<Mutex<LostCasState>>,
}
#[derive(Default)]
struct LostCasState {
    calls: usize,
    new_commit: Option<String>,
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

struct Fixture {
    root: PathBuf,
    git: PathBuf,
    repo: PathBuf,
    base: String,
    tree: String,
    revision: Arc<ProjectRevision>,
    original: BTreeMap<String, Vec<u8>>,
}
impl Fixture {
    fn new(format: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-graph-git-workflow-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in PATHS {
            fs::copy(example.join(path), root.join(path)).unwrap();
        }
        // Exercise a retained precondition and postcondition, not just an empty contract list.
        let core = fs::read_to_string(root.join("src/core.spx")).unwrap();
        let signature = "fn add(left: i64, right: i64) -> i64\n";
        assert!(core.contains(signature));
        let core = core.replace(signature, "fn add(left: i64, right: i64) -> i64\n    requires right >= 0\n    ensures result == left + right\n");
        let core = core.replace(
            "@id(\"calculator.subtract\")",
            "@id(\"calculator.local-add\")\nfn local_add() -> i64\n{\n    add(6 / 2, 8 / 2)\n}\n\n@id(\"calculator.subtract\")",
        );
        fs::write(
            root.join("src/core.spx"),
            semaprax::format::canonical(&semaprax::parse(&core, "src/core.spx").unwrap()),
        )
        .unwrap();
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
        let git = PathBuf::from(
            std::env::var_os("SEMAPRAX_TEST_GIT").unwrap_or_else(|| "/usr/bin/git".into()),
        )
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
                &format!("--object-format={format}"),
            ])
            .arg(&repo)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        // Keep the host-selected repository inside the minimal config grammar
        // instead of inheriting git init's platform filesystem probes.
        let config = match format {
            "sha1" => "[core]\nrepositoryformatversion = 0\nbare = true\n",
            "sha256" => "[core]\nrepositoryformatversion = 1\nbare = true\n[extensions]\nobjectformat = sha256\n",
            _ => panic!("unsupported fixture object format"),
        };
        fs::write(repo.join("config"), config).unwrap();
        let mut fixture = Self {
            root,
            git,
            repo,
            base: String::new(),
            tree: String::new(),
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
        let unrelated = fixture.object("blob", b"unrelated executable entry\n");
        fixture.tree = fixture.tree(vec![
            ("40000", "src".into(), sources),
            ("100644", "semaprax.toml".into(), manifest),
            ("100755", "keep.sh".into(), unrelated),
        ]);
        fixture.base = fixture.object("commit", format!("tree {}\nauthor Host <host@example.invalid> 1 +0000\ncommitter Host <host@example.invalid> 1 +0000\n\nOriginal\n", fixture.tree).as_bytes());
        fixture.run(&["update-ref", BRANCH, &fixture.base], &[]);
        fixture
    }
    fn manifest(&self) -> PathBuf {
        self.root.join("semaprax.toml")
    }
    // These real Git commands run only when this regression is explicitly executed.
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
        assert!(output.status.success(), "{output:?}");
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
    fn unchanged_raw_sources(&self) {
        for (path, bytes) in &self.original {
            assert_eq!(&fs::read(self.root.join(path)).unwrap(), bytes, "{path}");
        }
        assert!(!self.root.join(".semaprax-workspace").exists());
    }
    fn commit_session(&self, digest: &str) -> (VNextSession, String) {
        // Open the deadline-bound process provider only after review is finished.
        let authority =
            CandidateGitProcessAuthority::open(&self.git, &self.repo, 4096, 60_000).unwrap();
        self.commit_session_with_authority(digest, Box::new(authority))
    }
    fn commit_session_with_authority(
        &self,
        digest: &str,
        authority: Box<dyn CandidateGitAuthority>,
    ) -> (VNextSession, String) {
        let repository = authority.repository().unwrap();
        let target = CandidateGitTarget::new(&repository.identity, BRANCH, &self.base, "").unwrap();
        let metadata = CandidateGitCommitMetadata::new(
            "Host",
            "host@example.invalid",
            2,
            "Reviewed signature evolution\n",
        )
        .unwrap();
        let mut host = GitCommitHost::new(&self.manifest(), target, metadata, authority).unwrap();
        let approval = host.approve(digest).unwrap();
        let session = VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap()
        .with_git_commit_host(host)
        .unwrap();
        (session, approval)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct TaskTrace {
    active_criteria: Vec<usize>,
    frames: Vec<FrameMetric>,
    review: BTreeMap<String, MaterialMetric>,
    controls: BTreeMap<String, usize>,
    criteria: [bool; 12],
    criterion_evidence: [Vec<&'static str>; 12],
    branch_reconciliations: usize,
    candidate_validations: usize,
    candidate_tests: usize,
    library_recovery_restores: usize,
    protocol_recovery_restores: usize,
    semantic_delta_verifications: usize,
    target_admissions: usize,
    migrated_calls: usize,
    cross_file_migrated_calls: usize,
}

struct FrameMetric {
    session: &'static str,
    method: String,
    criteria: Vec<usize>,
    request: MaterialMetric,
    response: MaterialMetric,
    outcome: &'static str,
}

#[derive(Clone)]
struct MaterialMetric {
    bytes: usize,
    lexical_units: usize,
    sha256: String,
}

impl MaterialMetric {
    fn new(bytes: &[u8]) -> Self {
        let text = std::str::from_utf8(bytes).expect("protocol evidence must be UTF-8 JSON");
        Self {
            bytes: bytes.len(),
            lexical_units: semaprax::agent_economics::lexical_tokens(text),
            sha256: format!(
                "sha256:{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
            ),
        }
    }
    fn json(&self) -> Value {
        json!({"bytes":self.bytes,"lexical_units":self.lexical_units,
            "lexical_schema":"semaprax.lexical-token.v1","model_tokens":false,
            "sha256":self.sha256.as_str()})
    }
}

impl TaskTrace {
    fn phase(step: usize) -> &'static str {
        TASK_PHASES[step.checked_sub(1).expect("task steps are one-based")]
    }

    fn at(&mut self, criteria: &[usize]) {
        assert!(!criteria.is_empty());
        assert!(criteria.iter().all(|step| (1..=12).contains(step)));
        assert_eq!(
            criteria.iter().copied().collect::<BTreeSet<_>>().len(),
            criteria.len()
        );
        self.active_criteria = criteria.to_vec();
    }
    fn pass(&mut self, step: usize, evidence: &'static str) {
        assert!((1..=12).contains(&step));
        self.criteria[step - 1] = true;
        self.criterion_evidence[step - 1].push(evidence);
    }
    fn review(&mut self, name: &str, bytes: &[u8]) {
        assert!(self
            .review
            .insert(name.to_owned(), MaterialMetric::new(bytes))
            .is_none());
    }
    fn control(&mut self, code: &str) {
        *self.controls.entry(code.to_owned()).or_default() += 1;
    }
    fn report(&self, object_format: &str) -> String {
        assert!(self.criteria.iter().all(|passed| *passed));
        let mut methods = BTreeMap::<&str, usize>::new();
        let mut request_bytes = 0usize;
        let mut response_bytes = 0usize;
        let mut request_units = 0usize;
        let mut response_units = 0usize;
        for frame in &self.frames {
            *methods.entry(frame.method.as_str()).or_default() += 1;
            request_bytes += frame.request.bytes;
            response_bytes += frame.response.bytes;
            request_units += frame.request.lexical_units;
            response_units += frame.response.lexical_units;
        }
        let steps = (1..=12)
            .map(|step| {
                let rows = self.frames.iter().filter(|frame| frame.criteria.contains(&step)).collect::<Vec<_>>();
                json!({"criterion":step,"name":Self::phase(step),"associated_protocol_calls":rows.len(),
                "associated_request_bytes":rows.iter().map(|row|row.request.bytes).sum::<usize>(),
                "associated_response_bytes":rows.iter().map(|row|row.response.bytes).sum::<usize>()})
            })
            .collect::<Vec<_>>();
        let frames = self
            .frames
            .iter()
            .map(|frame| {
                // Approval and receipt digests bind the absolute host-selected
                // repository path. Retain their exact invocation metrics while
                // making the non-portable digest boundary explicit.
                let criterion_names = frame
                    .criteria
                    .iter()
                    .map(|step| Self::phase(*step))
                    .collect::<Vec<_>>();
                let host_route_bound = frame.session == "publication"
                    && matches!(
                        frame.method.as_str(),
                        "candidate/commit" | "candidate/commit-report" | "source-commit/status"
                    );
                json!({"session":frame.session,"method":frame.method,"criteria":frame.criteria,
                "criterion_names":criterion_names,"outcome":frame.outcome,
                "request":frame.request.json(),
                "response":frame.response.json(),
                "host_route_bound":host_route_bound})
            })
            .collect::<Vec<_>>();
        let criteria = (1..=12)
            .map(|step| {
                json!({"criterion":step,"name":Self::phase(step),"passed":self.criteria[step-1],
            "evidence":self.criterion_evidence[step-1]})
            })
            .collect::<Vec<_>>();
        let report = json!({
            "schema":"semaprax.agent-task-economics.v1",
            "scenario":"cross_file_signature_review_and_git_publication",
            "git_object_format":object_format,
            "protocol":{"semantic_protocol_calls":self.frames.len(),
                "notifications":0,
                "request_bytes":request_bytes,"response_bytes":response_bytes,
                "request_lexical_units":request_units,"response_lexical_units":response_units,
                "method_histogram":methods,"criterion_associations":steps,"frames":frames,
                "bounds":{"request_max_bytes":65_536,"response_max_bytes":1_048_576,
                    "report_max_bytes":262_144}},
            "review_material":self.review.iter().map(|(name,metric)|(name.clone(),metric.json())).collect::<BTreeMap<_,_>>(),
            "scripted_controls":{"rejections":self.controls,
                "branch_reconciliations":self.branch_reconciliations,"stale_recoveries":0,
                "observed_agent_invalid_attempts":{"count":Value::Null,"status":"not_observed"}},
            "validation":{"candidate_validate_requests":self.candidate_validations,
                "candidate_test_requests":self.candidate_tests,
                "library_recovery_restores":self.library_recovery_restores,
                "protocol_recovery_restores":self.protocol_recovery_restores,
                "semantic_delta_verifications":self.semantic_delta_verifications,
                "counted_explicit_replay_operations":self.candidate_validations+self.library_recovery_restores+self.protocol_recovery_restores,
                "reported_target_admissions":self.target_admissions,
                "native_target_executions":0,"wasm_target_executions":0,
                "status":"operation_counts_not_elapsed_cost"},
            "criteria":criteria,
            "change":{"migrated_calls":self.migrated_calls,
                "cross_file_migrated_calls":self.cross_file_migrated_calls},
            "model":{"id":Value::Null,"tokenizer":Value::Null,"input_tokens":Value::Null,
                "output_tokens":Value::Null,"context_bytes":Value::Null,"context_tokens":Value::Null,
                "tool_calls":Value::Null,"status":"not_observed"},
            "external_agent":{"tool_calls":Value::Null,"status":"not_observed"},
            "timing":{"wall_seconds":Value::Null,"cpu_seconds":Value::Null,"status":"not_observed"},
            "memory":{"peak_bytes":Value::Null,"status":"not_observed"},
            "monetary_cost":{"amount":Value::Null,"currency":Value::Null,"status":"not_observed"},
            "human_review":{"duration_seconds":Value::Null,"status":"not_observed"},
            "publication_digest_policy":{"host_identity_bound":true,
                "portable_snapshot":"counts_sizes_and_lexical_units_only"},
            "source_authority":false,
            "execution_authority":false,
            "publication_authority":false,
            "nonclaims":["scripted_controls_are_not_observed_agent_behavior",
                "semantic_protocol_calls_are_not_model_tool_calls",
                "operation_counts_are_not_latency_or_compute_cost",
                "review_material_size_is_not_human_review_time"]
        });
        serde_json::to_string(&report).unwrap()
    }
}

struct RecordedSession {
    inner: VNextSession,
    label: &'static str,
    trace: TaskTrace,
}

impl RecordedSession {
    fn new(inner: VNextSession, label: &'static str) -> Self {
        Self {
            inner,
            label,
            trace: TaskTrace::default(),
        }
    }
    fn with_trace(inner: VNextSession, label: &'static str, trace: TaskTrace) -> Self {
        Self {
            inner,
            label,
            trace,
        }
    }
    fn finish(mut self) -> TaskTrace {
        self.inner.finish().unwrap();
        self.trace
    }
}
impl Deref for RecordedSession {
    type Target = VNextSession;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for RecordedSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

fn call(session: &mut RecordedSession, method: &str, params: Value) -> Value {
    let request =
        json!({"jsonrpc":"2.0","id":"workflow","method":method,"params":params}).to_string();
    assert!(
        request.len() <= 65_536,
        "fixture request exceeds real transport bound"
    );
    assert!(!session.trace.active_criteria.is_empty());
    let request_metric = MaterialMetric::new(request.as_bytes());
    let response = session.inner.handle_frame(request.as_bytes()).unwrap();
    let response_metric = MaterialMetric::new(&response);
    let parsed: Value = serde_json::from_slice(&response).unwrap();
    session.trace.frames.push(FrameMetric {
        session: session.label,
        method: method.to_owned(),
        criteria: session.trace.active_criteria.clone(),
        request: request_metric,
        response: response_metric,
        outcome: if parsed.get("error").is_some() {
            "error"
        } else {
            "success"
        },
    });
    parsed
}
fn bound(session: &mut RecordedSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["schema"], VNEXT_RESULT_SCHEMA);
    assert_eq!(response["result"]["protocol"], VNEXT_PROTOCOL_SCHEMA);
    response["result"]["payload"].clone()
}
fn error(response: Value, code: &str) {
    assert!(response.get("error").is_some(), "{response}");
    assert!(response["error"].to_string().contains(code), "{response}");
}
fn digest(handle: Value) -> String {
    handle["candidate_revision"].as_str().unwrap().to_owned()
}
fn chunks(session: &mut RecordedSession, method: &str, mut params: Value) -> String {
    let mut result = String::new();
    let mut total = None;
    for _ in 0..8192 {
        params["offset"] = json!(result.len());
        params["chunk_bytes"] = json!(16_384);
        let part = payload(bound(session, method, params.clone()));
        assert_eq!(part["offset"].as_u64().unwrap() as usize, result.len());
        let bytes = part["total_bytes"].as_u64().unwrap() as usize;
        assert!(bytes <= 64 * 1024 * 1024);
        assert_eq!(*total.get_or_insert(bytes), bytes);
        let chunk = part["chunk"].as_str().unwrap();
        assert!(!chunk.is_empty());
        assert!(chunk.len() <= 16_384);
        result.push_str(chunk);
        if part["next_offset"].is_null() {
            assert_eq!(result.len(), bytes);
            return result;
        }
        assert_eq!(part["next_offset"].as_u64().unwrap() as usize, result.len());
    }
    panic!("chunk progress exceeded fixture bound")
}
fn signature(offset_name: &str) -> Value {
    json!({"kind":"change_function_signature","target":"calculator.add","parameters":[
        {"from":"right","name":"rhs"},
        {"from":"left","name":"lhs"},
        {"name":offset_name,"type":"i64","argument":{"kind":"i64","value":0}}
    ]})
}
fn apply(session: &mut RecordedSession, root: &str, intent: Value) -> String {
    digest(payload(bound(
        session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    )))
}
fn declaration_facts(revision: &ProjectRevision) -> BTreeMap<String, Value> {
    let mut facts = BTreeMap::new();
    for source in revision.sources() {
        let program = semaprax::parse(source.source(), source.path()).unwrap();
        for function in &program.functions {
            let value = json!({"path":source.path(),"module":program.module,"permits":program.permits,
                "name":function.name,"return_type":function.return_type.to_string(),"effects":function.effects,
                "parameters":function.params.iter().map(|p|json!({"name":p.name,"type":p.ty.to_string(),"mode":p.mode.text()})).collect::<Vec<_>>(),
                "requires":function.requires.iter().map(|e|semaprax::format::expr(e,0)).collect::<Vec<_>>(),
                "ensures":function.ensures.iter().map(|e|semaprax::format::expr(e,0)).collect::<Vec<_>>()});
            assert!(facts.insert(function.stable_id.clone(), value).is_none());
        }
    }
    facts
}

fn staged_add(expression: &Expr) -> Option<(&[Statement], &Expr)> {
    match &expression.kind {
        ExprKind::Block { statements, tail } => {
            if statements.len() == 2
                && matches!(&tail.kind, ExprKind::Call { name, args, .. } if name == "add" && args.len() == 3)
            {
                return Some((statements, tail));
            }
            statements
                .iter()
                .find_map(|statement| staged_add(statement.value()))
                .or_else(|| staged_add(tail))
        }
        ExprKind::Call { args, .. } => args.iter().find_map(staged_add),
        ExprKind::Unary { value, .. } => staged_add(value),
        ExprKind::Binary { left, right, .. } => staged_add(left).or_else(|| staged_add(right)),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => staged_add(condition)
            .or_else(|| staged_add(then_branch))
            .or_else(|| staged_add(else_branch)),
        _ => None,
    }
}

fn assert_staged_call(
    revision: &ProjectRevision,
    target: &str,
    original_left: &str,
    original_right: &str,
) {
    let function = revision
        .sources()
        .iter()
        .find_map(|source| {
            semaprax::parse(source.source(), source.path())
                .unwrap()
                .functions
                .iter()
                .find(|function| function.stable_id == target)
                .cloned()
        })
        .unwrap();
    let (statements, call) = staged_add(&function.body).expect("migrated add call must be staged");
    let [Statement::Let {
        name: left_stage,
        value: left,
        ..
    }, Statement::Let {
        name: right_stage,
        value: right,
        ..
    }] = statements
    else {
        panic!("signature migration must preserve two original argument stages")
    };
    assert_eq!(semaprax::format::expr(left, 0), original_left);
    assert_eq!(semaprax::format::expr(right, 0), original_right);
    let ExprKind::Call { name, args, .. } = &call.kind else {
        unreachable!("staged add selected above")
    };
    assert_eq!(name, "add");
    assert!(matches!(&args[0].kind, ExprKind::Var(name) if name == right_stage));
    assert!(matches!(&args[1].kind, ExprKind::Var(name) if name == left_stage));
    assert!(matches!(&args[2].kind, ExprKind::Int(0)));
}

struct Reviewed {
    digest: String,
    capsule: String,
    candidate: ProjectCandidate,
    trace: TaskTrace,
}
fn review(fixture: &Fixture) -> Reviewed {
    let mut session = RecordedSession::new(
        VNextSession::open(
            &fixture.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                diagnostics: true,
                test_policy: Some(CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap()),
                ..Default::default()
            },
        )
        .unwrap(),
        "review",
    );
    // 1–2. Open one source-authenticated image and discover the actual stable target.
    session.trace.at(&[1]);
    let workspace = payload(call(&mut session, "workspace/open", json!({})));
    assert_eq!(workspace["image_revision"], session.image_revision());
    session
        .trace
        .pass(1, "source-authenticated workspace image opened");
    session.trace.at(&[2]);
    let summary = payload(bound(
        &mut session,
        "image/function-summary",
        json!({"target":"calculator.add"}),
    ));
    assert_eq!(summary["id"], "calculator.add");
    assert_eq!(summary["parameter_count"], 2);
    assert_eq!(summary["requires_count"], 1);
    assert_eq!(summary["ensures_count"], 1);
    let root = digest(payload(bound(&mut session, "candidate/open", json!({}))));
    session
        .trace
        .pass(2, "stable function identity and contracts discovered");
    // 3–4. Compiler-owned cross-file signature migration and a disjoint sibling.
    // One compiler operation performs the requested signature change and its
    // authenticated caller migration, so the frame is associated with both
    // requirement criteria without counting it as two protocol calls.
    session.trace.at(&[3, 4]);
    let left = apply(&mut session, &root, signature("offset"));
    session.trace.pass(3, "typed signature candidate admitted");
    session.trace.at(&[11]);
    let right = apply(
        &mut session,
        &root,
        json!({"kind":"rename_declaration","target":"calculator.multiply","name":"times"}),
    );
    let reconciled = payload(bound(
        &mut session,
        "candidate/merge",
        json!({"candidate_revision":left,"other_candidate_revision":right}),
    ));
    assert_eq!(reconciled["report"]["operation"], "merge");
    assert_eq!(
        reconciled["report"]["original_base_revision"],
        fixture.revision.project_revision()
    );
    let merged = digest(reconciled["candidate"].clone());
    session.trace.branch_reconciliations += 1;
    // 10. Review exact source differences and selected semantic evidence.
    session.trace.at(&[10]);
    let report_bytes = chunks(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":merged}),
    );
    session
        .trace
        .review("candidate_report", report_bytes.as_bytes());
    let report: Value = serde_json::from_str(&report_bytes).unwrap();
    let signature_operation = report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .unwrap();
    assert_eq!(signature_operation["migrated_calls"], 3);
    session.trace.migrated_calls = 3;
    // The third call is the same-module staging fixture; the application and
    // test callers are the two cross-file migrations in the vertical slice.
    session.trace.cross_file_migrated_calls = 2;
    let source_review = serde_json::to_vec(&report["source_changes"]).unwrap();
    session.trace.review("source_changes", &source_review);
    let paths: BTreeSet<_> = report["source_changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            assert!(!c["source_diff"].as_str().unwrap().is_empty());
            assert!(!c["replacement_source"].as_str().unwrap().is_empty());
            c["path"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(
        paths,
        BTreeSet::from([
            "src/app.spx".into(),
            "src/core.spx".into(),
            "src/tests.spx".into()
        ])
    );
    let impact = payload(bound(
        &mut session,
        "candidate/impact",
        json!({"candidate_revision":merged,"target":"calculator.add"}),
    ));
    assert_eq!(impact["candidate_revision"], merged);
    assert!(impact["impact"].is_object());
    session.trace.review(
        "semantic_impact",
        serde_json::to_string(&impact).unwrap().as_bytes(),
    );
    let delta_bytes = chunks(
        &mut session,
        "candidate/semantic-delta",
        json!({"candidate_revision":merged,"target":"calculator.add"}),
    );
    session
        .trace
        .review("semantic_delta", delta_bytes.as_bytes());
    let delta: Value = serde_json::from_str(&delta_bytes).unwrap();
    assert_eq!(delta["candidate_digest"], merged);
    for name in ["signature", "contracts", "callers", "ownership", "cleanup"] {
        assert!(
            delta["facets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["facet"] == name),
            "missing {name}"
        );
    }
    // 11. A competing signature fails without replacing retained review evidence.
    session.trace.at(&[11]);
    let competing = apply(&mut session, &root, signature("different"));
    error(
        bound(
            &mut session,
            "candidate/merge",
            json!({"candidate_revision":left,"other_candidate_revision":competing}),
        ),
        "SPX-G235",
    );
    session.trace.control("SPX-G235");
    assert_eq!(
        chunks(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":merged})
        ),
        report_bytes
    );
    session.trace.pass(
        11,
        "disjoint branch reconciled and competing signature rejected",
    );
    fixture.unchanged_raw_sources();
    // 7. Explicit replay; target evidence is emission/structural validation only.
    session.trace.at(&[7]);
    let validation = payload(bound(
        &mut session,
        "candidate/validate",
        json!({"candidate_revision":merged}),
    ));
    session.trace.candidate_validations += 1;
    assert_eq!(validation["independently_replayed"], true);
    assert_eq!(validation["tests"], "not_run");
    session.trace.at(&[9]);
    let targets = report["core_targets"]["candidate"].as_array().unwrap();
    assert_eq!(targets.len(), 4);
    session.trace.target_admissions += targets.len();
    let target_pairs: BTreeSet<_> = targets
        .iter()
        .map(|target| {
            (
                target["role"].as_str().unwrap(),
                target["lane"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        target_pairs,
        BTreeSet::from([
            ("entry", "native_c11"),
            ("entry", "wasm_core"),
            ("test", "native_c11"),
            ("test", "wasm_core"),
        ])
    );
    for target in targets {
        assert_eq!(target["admitted"], true);
        assert!(target["bytes"].as_u64().unwrap() > 0);
        assert_eq!(
            target["validation"],
            if target["lane"] == "native_c11" {
                "compiler_emission_not_native_execution"
            } else {
                "wasmparser_structural_not_execution"
            }
        );
    }
    session
        .trace
        .pass(9, "four target artifacts admitted without execution claims");
    // 8. The real v5 test method receives only the host's fixed interpreter policy.
    session.trace.at(&[8]);
    let plan = payload(bound(
        &mut session,
        "candidate/test-plan",
        json!({"candidate_revision":merged}),
    ));
    assert_eq!(plan["schema"], "semaprax.project-candidate-test-plan.v1");
    let tested = payload(bound(
        &mut session,
        "candidate/test",
        json!({"candidate_revision":merged}),
    ));
    session.trace.candidate_tests += 1;
    assert_eq!(tested["passed"], true);
    assert_eq!(tested["candidate_digest"], merged);
    assert_eq!(tested["options"]["max_steps"], 100_000);
    assert_eq!(tested["options"]["max_execution_bytes"], 65_536);
    assert_eq!(tested["options"]["max_report_bytes"], 262_144);
    assert_eq!(
        tested["execution_scope"],
        "complete_manifest_declared_test_closure"
    );
    session
        .trace
        .pass(8, "manifest-declared tests passed under fixed host policy");
    // 7 continued. Export portable intentions; replay authenticates review.
    session.trace.at(&[7]);
    let capsule = chunks(
        &mut session,
        "candidate/recovery-export",
        json!({"candidate_revision":merged}),
    );
    let candidate = ProjectCandidate::restore(
        Arc::clone(&fixture.revision),
        fixture.revision.project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    session.trace.library_recovery_restores += 1;
    assert_eq!(candidate.candidate_digest(), merged);
    assert_eq!(candidate.to_json(), report_bytes);
    assert_staged_call(
        candidate.revision(),
        "calculator.local-add",
        "6 / 2",
        "8 / 2",
    );
    assert_staged_call(
        candidate.revision(),
        "calculator.app.main",
        "multiply(6, 7)",
        "subtract(divide(4, 2), 2)",
    );
    assert_staged_call(candidate.revision(), "calculator.tests.main", "19", "23");
    session
        .trace
        .pass(4, "all three authenticated call sites migrated");
    candidate
        .verify_semantic_delta(&merged, "calculator.add", delta_bytes.as_bytes())
        .unwrap();
    session.trace.semantic_delta_verifications += 1;
    session.trace.pass(
        10,
        "source diff, impact, and semantic delta reviewed and verified",
    );
    assert_eq!(
        candidate.revision().manifest().web_exports(),
        fixture.revision.manifest().web_exports()
    );
    assert_eq!(
        candidate.revision().manifest().capabilities(),
        fixture.revision.manifest().capabilities()
    );
    let mut expected = declaration_facts(&fixture.revision);
    expected.get_mut("calculator.add").unwrap()["parameters"] = json!([
        {"name":"rhs","type":"i64","mode":"value"},
        {"name":"lhs","type":"i64","mode":"value"},
        {"name":"offset","type":"i64","mode":"value"}
    ]);
    expected.get_mut("calculator.add").unwrap()["requires"] = json!(["rhs >= 0"]);
    expected.get_mut("calculator.add").unwrap()["ensures"] = json!(["result == lhs + rhs"]);
    expected.get_mut("calculator.multiply").unwrap()["name"] = json!("times");
    assert_eq!(declaration_facts(candidate.revision()), expected);
    let core = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert!(core.contains("fn add(rhs: i64, lhs: i64, offset: i64) -> i64"));
    assert!(core.contains("requires rhs >= 0"));
    assert!(core.contains("ensures result == lhs + rhs"));
    assert!(core.contains("{\n    lhs + rhs\n}"));
    session.trace.pass(
        5,
        "stable declarations and exports preserved across migration",
    );
    session
        .trace
        .pass(6, "contracts, effects, and manifest capabilities preserved");
    for revision in [&fixture.revision, candidate.revision()] {
        for program in [revision.entry_program(), revision.test_program()] {
            for function in &program.functions {
                // This fixture is Copy/scalar: no owned-resource lifecycle is claimed.
                assert!(function.cleanup.slots.is_empty());
                assert!(function
                    .cleanup
                    .entry_state
                    .live_owned_parameters
                    .is_empty());
            }
        }
    }
    assert_eq!(report["validation"]["tests"], "not_run"); // separate test evidence is not rewritten into the candidate.
    session.trace.pass(
        7,
        "candidate replay and cleanup invariants independently checked",
    );
    let trace = session.finish();
    fixture.unchanged_raw_sources();
    Reviewed {
        digest: merged,
        capsule,
        candidate,
        trace,
    }
}
fn restore(session: &mut RecordedSession, reviewed: &Reviewed) {
    let capsule: Value = serde_json::from_str(&reviewed.capsule).unwrap();
    assert_eq!(
        digest(payload(bound(
            session,
            "candidate/recovery-restore",
            json!({"capsule":capsule})
        ))),
        reviewed.digest
    );
    session.trace.protocol_recovery_restores += 1;
}
fn published_workflow(format: &str) {
    let fixture = Fixture::new(format);
    let mut reviewed = review(&fixture);
    // 12. Separate host approval precedes every request in this new session.
    let (session, approval) = fixture.commit_session(&reviewed.digest);
    let mut session =
        RecordedSession::with_trace(session, "publication", std::mem::take(&mut reviewed.trace));
    session.trace.at(&[12]);
    restore(&mut session, &reviewed);
    assert!(session.approve_git_commit(&reviewed.digest).is_err());
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":format!("sha256:{}","0".repeat(64))}),
        ),
        "SPX-G286",
    );
    session.trace.control("SPX-G286");
    assert_eq!(fixture.head(), fixture.base);
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["pending_approval"]["approval_revision"], approval);
    // 12 continued. Process authority creates Git objects and performs one old-OID CAS.
    let committed = payload(bound(
        &mut session,
        "candidate/commit",
        json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
    ));
    assert_eq!(committed["state"], "published");
    let receipt: Value = serde_json::from_str(&chunks(
        &mut session,
        "candidate/commit-report",
        json!({"report_revision":committed["report_revision"]}),
    ))
    .unwrap();
    assert_eq!(receipt["previous_commit"], fixture.base);
    assert_eq!(receipt["published_commit"], fixture.head());
    assert_eq!(receipt["approved_candidate_digest"], reviewed.digest);
    assert_eq!(
        receipt["candidate_project_revision"],
        reviewed.candidate.revision().project_revision()
    );
    assert_eq!(receipt["git_object_format"], format);
    assert_eq!(receipt["working_tree_rewritten"], false);
    assert_eq!(receipt["tests"], "not_run"); // publication itself never executes tests.
                                             // 12. Read the actual commit, not an exported proposal, and compare every source.
    for source in reviewed.candidate.revision().sources() {
        assert_eq!(
            fixture.run(&["show", &format!("{BRANCH}:{}", source.path())], &[]),
            source.source().as_bytes()
        );
    }
    assert_eq!(
        fixture.run(&["show", &format!("{BRANCH}:semaprax.toml")], &[]),
        fixture.original["semaprax.toml"]
    );
    assert_eq!(
        fixture.run(&["show", &format!("{BRANCH}:keep.sh")], &[]),
        b"unrelated executable entry\n"
    );
    assert!(
        String::from_utf8(fixture.run(&["ls-tree", BRANCH, "keep.sh"], &[]))
            .unwrap()
            .starts_with("100755 blob ")
    );
    let commit = String::from_utf8(fixture.run(&["cat-file", "-p", &fixture.head()], &[])).unwrap();
    assert!(commit
        .lines()
        .any(|line| line == format!("parent {}", fixture.base)));
    let published = fixture.head();
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
        ),
        "SPX-G287",
    );
    session.trace.control("SPX-G287");
    assert_eq!(fixture.head(), published);
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["state"], "published");
    assert!(status["pending_approval"].is_null());
    fixture.unchanged_raw_sources();
    session.trace.pass(
        12,
        "separate approval published one exact Git generation and preserved host files",
    );
    let trace = session.finish();
    let report = assert_task_report(&trace, format);
    export_task_report(format, &report);
}

fn assert_task_report(trace: &TaskTrace, format: &str) -> String {
    let text = trace.report(format);
    assert!(text.len() <= 256 * 1024);
    assert!(!text.ends_with('\n'));
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(serde_json::to_string(&report).unwrap(), text);
    assert_eq!(report["schema"], "semaprax.agent-task-economics.v1");
    assert_eq!(report["git_object_format"], format);
    let protocol = &report["protocol"];
    let call_count = protocol["semantic_protocol_calls"].as_u64().unwrap();
    assert_eq!(call_count as usize, trace.frames.len());
    assert!(call_count > 0);
    assert_eq!(protocol["notifications"], 0);
    assert_eq!(protocol["bounds"]["request_max_bytes"], 65_536);
    assert_eq!(protocol["bounds"]["response_max_bytes"], 1_048_576);
    assert_eq!(protocol["bounds"]["report_max_bytes"], 262_144);
    assert_eq!(
        protocol["method_histogram"]
            .as_object()
            .unwrap()
            .values()
            .map(|count| count.as_u64().unwrap())
            .sum::<u64>(),
        call_count
    );
    let associations = protocol["criterion_associations"].as_array().unwrap();
    assert_eq!(associations.len(), 12);
    for (index, association) in associations.iter().enumerate() {
        assert_eq!(
            association["criterion"].as_u64().unwrap() as usize,
            index + 1
        );
        assert_eq!(association["name"], TaskTrace::phase(index + 1));
    }
    // One signature operation implements criteria 3 and 4. Associations are
    // deliberately not summed as if they were distinct protocol calls.
    assert_eq!(associations[2]["associated_protocol_calls"], 1);
    assert_eq!(associations[3]["associated_protocol_calls"], 1);
    assert_eq!(
        associations[2]["associated_request_bytes"],
        associations[3]["associated_request_bytes"]
    );
    assert_eq!(
        associations[2]["associated_response_bytes"],
        associations[3]["associated_response_bytes"]
    );
    for frame in protocol["frames"].as_array().unwrap() {
        let frame_criteria = frame["criteria"].as_array().unwrap();
        let frame_names = frame["criterion_names"].as_array().unwrap();
        assert_eq!(frame_criteria.len(), frame_names.len());
        for (criterion, name) in frame_criteria.iter().zip(frame_names) {
            assert_eq!(
                name.as_str().unwrap(),
                TaskTrace::phase(criterion.as_u64().unwrap() as usize)
            );
        }
        assert!(matches!(
            frame["session"].as_str().unwrap(),
            "review" | "publication"
        ));
        let host_route_bound = frame["host_route_bound"].as_bool().unwrap();
        let expected_host_route = frame["session"] == "publication"
            && matches!(
                frame["method"].as_str().unwrap(),
                "candidate/commit" | "candidate/commit-report" | "source-commit/status"
            );
        assert_eq!(host_route_bound, expected_host_route);
        // Publication hashes are exact invocation evidence. This assertion
        // freezes only their representation, never their host-bound value.
        for side in ["request", "response"] {
            let digest = frame[side]["sha256"].as_str().unwrap();
            assert_eq!(digest.len(), 71);
            assert!(digest.starts_with("sha256:"));
        }
    }
    assert_eq!(report["scripted_controls"]["rejections"]["SPX-G235"], 1);
    assert_eq!(report["scripted_controls"]["rejections"]["SPX-G286"], 1);
    assert_eq!(report["scripted_controls"]["rejections"]["SPX-G287"], 1);
    assert_eq!(report["scripted_controls"]["branch_reconciliations"], 1);
    assert_eq!(report["scripted_controls"]["stale_recoveries"], 0);
    assert!(report["scripted_controls"]["observed_agent_invalid_attempts"]["count"].is_null());
    assert_eq!(
        report["scripted_controls"]["observed_agent_invalid_attempts"]["status"],
        "not_observed"
    );
    assert_eq!(report["validation"]["candidate_validate_requests"], 1);
    assert_eq!(report["validation"]["candidate_test_requests"], 1);
    assert_eq!(report["validation"]["library_recovery_restores"], 1);
    assert_eq!(report["validation"]["protocol_recovery_restores"], 1);
    assert_eq!(report["validation"]["semantic_delta_verifications"], 1);
    assert_eq!(
        report["validation"]["counted_explicit_replay_operations"],
        3
    );
    assert_eq!(report["validation"]["reported_target_admissions"], 4);
    assert_eq!(report["validation"]["native_target_executions"], 0);
    assert_eq!(report["validation"]["wasm_target_executions"], 0);
    assert_eq!(report["change"]["migrated_calls"], 3);
    assert_eq!(report["change"]["cross_file_migrated_calls"], 2);
    let criteria = report["criteria"].as_array().unwrap();
    assert_eq!(criteria.len(), 12);
    for (index, criterion) in criteria.iter().enumerate() {
        assert_eq!(criterion["criterion"].as_u64().unwrap() as usize, index + 1);
        assert_eq!(criterion["name"], TaskTrace::phase(index + 1));
        assert_eq!(criterion["passed"], true);
        assert!(!criterion["evidence"].as_array().unwrap().is_empty());
    }
    assert_eq!(report["model"]["status"], "not_observed");
    assert!(report["model"]["id"].is_null());
    assert!(report["model"]["tokenizer"].is_null());
    assert!(report["model"]["input_tokens"].is_null());
    assert!(report["model"]["output_tokens"].is_null());
    assert!(report["model"]["context_bytes"].is_null());
    assert!(report["model"]["context_tokens"].is_null());
    assert!(report["model"]["tool_calls"].is_null());
    assert_eq!(report["external_agent"]["status"], "not_observed");
    assert!(report["external_agent"]["tool_calls"].is_null());
    assert_eq!(report["timing"]["status"], "not_observed");
    assert!(report["timing"]["wall_seconds"].is_null());
    assert!(report["timing"]["cpu_seconds"].is_null());
    assert_eq!(report["memory"]["status"], "not_observed");
    assert!(report["memory"]["peak_bytes"].is_null());
    assert_eq!(report["monetary_cost"]["status"], "not_observed");
    assert!(report["monetary_cost"]["amount"].is_null());
    assert!(report["monetary_cost"]["currency"].is_null());
    assert_eq!(report["human_review"]["status"], "not_observed");
    assert!(report["human_review"]["duration_seconds"].is_null());
    assert_eq!(
        report["publication_digest_policy"]["host_identity_bound"],
        true
    );
    assert_eq!(report["review_material"].as_object().unwrap().len(), 4);
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["execution_authority"], false);
    assert_eq!(report["publication_authority"], false);
    text
}

fn export_task_report(format: &str, report: &str) {
    let Some(directory) = std::env::var_os("SEMAPRAX_GRAPH_WORKFLOW_EVIDENCE_DIR") else {
        return;
    };
    let file_name = match format {
        "sha1" => "agent-task-economics-sha1.json",
        "sha256" => "agent-task-economics-sha256.json",
        _ => panic!("unsupported graph-workflow evidence format"),
    };
    let directory = PathBuf::from(directory);
    assert!(
        directory.is_absolute(),
        "SEMAPRAX_GRAPH_WORKFLOW_EVIDENCE_DIR must be absolute"
    );
    fs::create_dir_all(&directory).unwrap();
    let destination = directory.join(file_name);
    let temporary = directory.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(report.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, destination)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        panic!("cannot export graph-workflow evidence: {error}");
    }
}

#[test]
fn twelve_step_v5_review_to_real_sha1_git_commit() {
    published_workflow("sha1");
}
#[test]
fn twelve_step_v5_review_to_real_sha256_git_commit() {
    published_workflow("sha256");
}

#[test]
fn competing_real_git_ref_consumes_approval_without_overwriting_the_other_commit() {
    let fixture = Fixture::new("sha256");
    let mut reviewed = review(&fixture);
    let (session, approval) = fixture.commit_session(&reviewed.digest);
    let mut session =
        RecordedSession::with_trace(session, "publication", std::mem::take(&mut reviewed.trace));
    session.trace.at(&[12]);
    restore(&mut session, &reviewed);
    let competing = fixture.object("commit",format!("tree {}\nparent {}\nauthor Host <host@example.invalid> 3 +0000\ncommitter Host <host@example.invalid> 3 +0000\n\nConcurrent host commit\n",fixture.tree,fixture.base).as_bytes());
    fixture.run(&["update-ref", BRANCH, &competing, &fixture.base], &[]);
    // This is an actual stale expected-base preflight, not a simulated mid-CAS race.
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
        ),
        "SPX-G265",
    );
    assert_eq!(fixture.head(), competing);
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["state"], "available");
    assert!(status["pending_approval"].is_null());
    assert!(session.approve_git_commit(&reviewed.digest).is_err());
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
        ),
        "SPX-G286",
    );
    assert_eq!(fixture.head(), competing);
    fixture.unchanged_raw_sources();
    let _ = session.finish();
}

#[test]
fn real_git_ref_update_with_lost_response_is_terminal_and_requires_inspection() {
    let fixture = Fixture::new("sha256");
    let mut reviewed = review(&fixture);
    let authority =
        CandidateGitProcessAuthority::open(&fixture.git, &fixture.repo, 4096, 60_000).unwrap();
    let lost = Arc::new(Mutex::new(LostCasState::default()));
    let authority = UncertainAfterRealCas {
        inner: authority,
        state: Arc::clone(&lost),
    };
    let (session, approval) =
        fixture.commit_session_with_authority(&reviewed.digest, Box::new(authority));
    let mut session =
        RecordedSession::with_trace(session, "publication", std::mem::take(&mut reviewed.trace));
    session.trace.at(&[12]);
    restore(&mut session, &reviewed);
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
        ),
        "SPX-G267",
    );
    let published = fixture.head();
    assert_ne!(published, fixture.base);
    assert_eq!(lost.lock().unwrap().calls, 1);
    assert_eq!(
        lost.lock().unwrap().new_commit.as_deref(),
        Some(published.as_str())
    );
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["state"], "publication_uncertain");
    assert!(status["pending_approval"].is_null());
    assert!(status["report_revision"].is_null());
    assert!(status["last_error_codes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|code| code == "SPX-G267"));
    error(
        bound(
            &mut session,
            "candidate/commit-report",
            json!({"report_revision":format!("sha256:{}", "0".repeat(64))}),
        ),
        "SPX-G286",
    );
    error(
        bound(
            &mut session,
            "candidate/commit",
            json!({"candidate_revision":reviewed.digest,"approval_revision":approval}),
        ),
        "SPX-G287",
    );
    assert_eq!(fixture.head(), published);
    assert_eq!(lost.lock().unwrap().calls, 1);
    fixture.unchanged_raw_sources();
    session.finish();
}
