//! Integrated v5 + real bare Git workflow. Authored, not executed locally.
#![cfg(unix)]

use semaprax::image_transport::{
    GitCommitHost, VNextPolicy, VNextSession, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA,
};
use semaprax::project::{
    with_authenticated_project, CandidateGitCommitMetadata, CandidateGitProcessAuthority,
    CandidateGitTarget, CandidateTestPolicy, ProjectCandidate, ProjectRevision,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
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
        let target =
            CandidateGitTarget::new(authority.repository_identity(), BRANCH, &self.base, "")
                .unwrap();
        let metadata = CandidateGitCommitMetadata::new(
            "Host",
            "host@example.invalid",
            2,
            "Reviewed signature evolution\n",
        )
        .unwrap();
        let mut host =
            GitCommitHost::new(&self.manifest(), target, metadata, Box::new(authority)).unwrap();
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

fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request =
        json!({"jsonrpc":"2.0","id":"workflow","method":method,"params":params}).to_string();
    assert!(
        request.len() <= 65_536,
        "fixture request exceeds real transport bound"
    );
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
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
fn chunks(session: &mut VNextSession, method: &str, mut params: Value) -> String {
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
fn signature(name: &str) -> Value {
    json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":name,"type":"i64","argument":{"kind":"i64","value":0}}]})
}
fn apply(session: &mut VNextSession, root: &str, intent: Value) -> String {
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
struct Reviewed {
    digest: String,
    capsule: String,
    candidate: ProjectCandidate,
}
fn review(fixture: &Fixture) -> Reviewed {
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            test_policy: Some(CandidateTestPolicy::new(100_000, 65_536, 262_144).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    // 1–2. Open one source-authenticated image and discover the actual stable target.
    let workspace = payload(call(&mut session, "workspace/open", json!({})));
    assert_eq!(workspace["image_revision"], session.image_revision());
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
    // 3–4. Compiler-owned cross-file signature migration and a disjoint sibling.
    let left = apply(&mut session, &root, signature("unused"));
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
    // 5. Review exact source differences and selected semantic evidence.
    let report_bytes = chunks(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":merged}),
    );
    let report: Value = serde_json::from_str(&report_bytes).unwrap();
    let signature_operation = report["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "change_function_signature")
        .unwrap();
    assert_eq!(signature_operation["migrated_calls"], 2);
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
    let delta_bytes = chunks(
        &mut session,
        "candidate/semantic-delta",
        json!({"candidate_revision":merged,"target":"calculator.add"}),
    );
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
    // 6. A competing signature fails without replacing retained review evidence.
    let competing = apply(&mut session, &root, signature("different"));
    error(
        bound(
            &mut session,
            "candidate/merge",
            json!({"candidate_revision":left,"other_candidate_revision":competing}),
        ),
        "SPX-G235",
    );
    assert_eq!(
        chunks(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":merged})
        ),
        report_bytes
    );
    fixture.unchanged_raw_sources();
    // 7. Explicit replay; target evidence is emission/structural validation only.
    let validation = payload(bound(
        &mut session,
        "candidate/validate",
        json!({"candidate_revision":merged}),
    ));
    assert_eq!(validation["independently_replayed"], true);
    assert_eq!(validation["tests"], "not_run");
    let targets = report["core_targets"]["candidate"].as_array().unwrap();
    assert_eq!(targets.len(), 4);
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
    // 8. The real v5 test method receives only the host's fixed interpreter policy.
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
    assert_eq!(tested["passed"], true);
    assert_eq!(tested["candidate_digest"], merged);
    assert_eq!(tested["options"]["max_steps"], 100_000);
    assert_eq!(tested["options"]["max_execution_bytes"], 65_536);
    assert_eq!(tested["options"]["max_report_bytes"], 262_144);
    assert_eq!(
        tested["execution_scope"],
        "complete_manifest_declared_test_closure"
    );
    // 9. Export portable complete intentions; independent replay authenticates review.
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
    assert_eq!(candidate.candidate_digest(), merged);
    assert_eq!(candidate.to_json(), report_bytes);
    candidate
        .verify_semantic_delta(&merged, "calculator.add", delta_bytes.as_bytes())
        .unwrap();
    assert_eq!(
        candidate.revision().manifest().web_exports(),
        fixture.revision.manifest().web_exports()
    );
    let mut expected = declaration_facts(&fixture.revision);
    expected.get_mut("calculator.add").unwrap()["parameters"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"unused","type":"i64","mode":"value"}));
    expected.get_mut("calculator.multiply").unwrap()["name"] = json!("times");
    assert_eq!(declaration_facts(candidate.revision()), expected);
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
    session.finish().unwrap();
    drop(session);
    fixture.unchanged_raw_sources();
    Reviewed {
        digest: merged,
        capsule,
        candidate,
    }
}
fn restore(session: &mut VNextSession, reviewed: &Reviewed) {
    let capsule: Value = serde_json::from_str(&reviewed.capsule).unwrap();
    assert_eq!(
        digest(payload(bound(
            session,
            "candidate/recovery-restore",
            json!({"capsule":capsule})
        ))),
        reviewed.digest
    );
}
fn published_workflow(format: &str) {
    let fixture = Fixture::new(format);
    let reviewed = review(&fixture);
    // 10. Separate host approval precedes every request in this NEW session.
    let (mut session, approval) = fixture.commit_session(&reviewed.digest);
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
    assert_eq!(fixture.head(), fixture.base);
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["pending_approval"]["approval_revision"], approval);
    // 11. Actual process authority creates Git objects and performs one old-OID CAS.
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
    assert_eq!(fixture.head(), published);
    let status = payload(bound(&mut session, "source-commit/status", json!({})));
    assert_eq!(status["state"], "published");
    assert!(status["pending_approval"].is_null());
    fixture.unchanged_raw_sources();
    session.finish().unwrap();
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
    let reviewed = review(&fixture);
    let (mut session, approval) = fixture.commit_session(&reviewed.digest);
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
    session.finish().unwrap();
}
