//! Immutable registry read batches: evidence authored, deliberately unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession, MAX_REQUEST_BYTES};
use semaprax::project::{
    with_authenticated_project, CandidateTestPolicy, ProjectCandidate, ProjectCandidateArchive,
    ProjectCandidateDraft, ProjectCandidateDraftArchive, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-parallel-registry-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
    fn edit_app(&self) {
        let path = self.0.join("src/app.spx");
        let old = std::fs::read_to_string(&path).unwrap();
        let changed = old.replace("multiply(6, 7)", "multiply(6, 8)");
        assert_ne!(old, changed);
        let parsed = semaprax::parse(&changed, "src/app.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn changed(base: &ProjectCandidate, target: &str, value: i64) -> Arc<ProjectCandidate> {
    let change=SemanticChange::new(base.revision().project_revision(),&json!({"kind":"replace_function_body","target":target,"body":{"kind":"i64","value":value}})).unwrap();
    Arc::new(base.apply(base.candidate_digest(), &change).unwrap())
}
fn selected(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let text = if contract {
        base.contract_expression_catalog(target)
    } else {
        base.expression_catalog(target)
    }
    .unwrap();
    let catalog: Value = serde_json::from_str(&text).unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows: Vec<_> = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
struct State {
    candidates: Vec<ProjectCandidateArchive>,
    drafts: Vec<ProjectCandidateDraftArchive>,
    private_candidate: String,
}
impl State {
    fn new(fixture: &Fixture) -> Self {
        let base = fixture.candidate();
        let left = changed(&base, "calculator.add", 17);
        let right = changed(&base, "calculator.subtract", 23);
        let merged = Arc::new(
            left.merge(left.candidate_digest(), &right, right.candidate_digest())
                .unwrap()
                .into_candidate(),
        );
        let left_draft = ProjectCandidateDraft::open(Arc::clone(&left)).unwrap();
        let left_draft = left_draft
            .with_body_hole(left_draft.draft_digest(), "calculator.multiply", "multiply")
            .unwrap();
        let left_draft = left_draft
            .with_expression_hole(
                left_draft.draft_digest(),
                "calculator.is-negative",
                &selected(&left, "calculator.is-negative", false, "value < 0"),
                "negative",
            )
            .unwrap();
        let right_draft = ProjectCandidateDraft::open(Arc::clone(&right)).unwrap();
        let right_draft = right_draft
            .with_contract_expression_hole(
                right_draft.draft_digest(),
                "calculator.divide",
                &selected(&right, "calculator.divide", true, "right != 0"),
                "divide",
            )
            .unwrap();
        let merged_draft = left_draft
            .merge(
                left_draft.draft_digest(),
                &right_draft,
                right_draft.draft_digest(),
            )
            .unwrap()
            .into_draft();
        let merged_draft = merged_draft
            .with_body_hole(merged_draft.draft_digest(), "calculator.not", "private")
            .unwrap();
        let merged_draft = merged_draft
            .fill_hole(
                merged_draft.draft_digest(),
                "private",
                &json!({"kind":"bool","value":true}),
            )
            .unwrap();
        let summary: Value =
            serde_json::from_str(merged_draft.summary(merged_draft.draft_digest()).unwrap())
                .unwrap();
        Self {
            candidates: [base, left, right, merged]
                .iter()
                .map(|candidate| {
                    ProjectCandidateArchive::prepare(candidate, candidate.candidate_digest())
                        .unwrap()
                })
                .collect(),
            drafts: [left_draft, right_draft, merged_draft]
                .iter()
                .map(|draft| {
                    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
                })
                .collect(),
            private_candidate: summary["last_valid_candidate_digest"]
                .as_str()
                .unwrap()
                .to_owned(),
        }
    }
    fn session(&self, fixture: &Fixture, policy: VNextPolicy) -> VNextSession {
        let mut session = VNextSession::open(&fixture.manifest(), policy).unwrap();
        for archive in &self.candidates {
            session
                .restore_candidate_archive(
                    archive.to_json().as_bytes(),
                    archive.archive_digest(),
                    archive.candidate_digest(),
                )
                .unwrap();
        }
        for archive in &self.drafts {
            session
                .restore_draft_archive(
                    archive.to_json().as_bytes(),
                    archive.archive_digest(),
                    archive.draft_digest(),
                )
                .unwrap();
        }
        session
    }
}
fn policy() -> VNextPolicy {
    VNextPolicy {
        candidate_prepare: true,
        diagnostics: true,
        ..Default::default()
    }
}
fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn bound_frame(session: &VNextSession, id: usize, method: &str, mut params: Value) -> Vec<u8> {
    params["image_revision"] = json!(session.image_revision());
    frame(id, method, params)
}
fn decode(row: &Option<Vec<u8>>) -> Value {
    serde_json::from_slice(row.as_ref().unwrap()).unwrap()
}
fn payload(row: Option<Vec<u8>>) -> Value {
    let value = decode(&row);
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}
fn ordinary(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request = bound_frame(session, 1, method, params);
    payload(session.handle_frame(&request))
}
fn parity(
    sequential: &mut VNextSession,
    parallel: &mut VNextSession,
    requests: &[Vec<u8>],
    success: bool,
) -> Vec<Option<Vec<u8>>> {
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    if success {
        for row in &expected {
            let value = decode(row);
            assert!(value.get("error").is_none(), "{value}");
        }
    }
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        assert_eq!(
            parallel.handle_read_batch(&refs, workers).unwrap(),
            expected
        );
    }
    expected
}

#[test]
fn historical_parent_merged_and_recovered_reads_match_sequential_bytes_and_preserve_registration() {
    let fixture = Fixture::new();
    let state = State::new(&fixture);
    fixture.edit_app();
    let disk = fixture.bytes();
    let mut sequential = state.session(&fixture, policy());
    let mut parallel = state.session(&fixture, policy());
    let image = parallel.image_revision().to_owned();
    let merged = state.candidates[3].candidate_digest();
    let draft = state.drafts[2].draft_digest();
    let mut requests = state
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            bound_frame(
                &parallel,
                20 - index,
                "candidate/query",
                json!({"candidate_revision":candidate.candidate_digest(),"chunk_bytes":1024}),
            )
        })
        .collect::<Vec<_>>();
    for (method, params) in [
        (
            "candidate/compare",
            json!({"candidate_revision":state.candidates[1].candidate_digest(),"other_candidate_revision":state.candidates[2].candidate_digest()}),
        ),
        (
            "candidate/impact",
            json!({"candidate_revision":merged,"target":"calculator.add"}),
        ),
        (
            "expression/catalog",
            json!({"candidate_revision":merged,"target":"calculator.is-negative"}),
        ),
        (
            "change/catalog",
            json!({"candidate_revision":merged,"target":"calculator.multiply"}),
        ),
        ("candidate/validate", json!({"candidate_revision":merged})),
        (
            "candidate/recovery-export",
            json!({"candidate_revision":merged,"chunk_bytes":1024}),
        ),
        (
            "candidate/interface-delta",
            json!({"candidate_revision":merged,"chunk_bytes":1024}),
        ),
        (
            "candidate/contract-delta",
            json!({"candidate_revision":merged,"chunk_bytes":1024}),
        ),
        (
            "candidate/ownership-delta",
            json!({"candidate_revision":merged,"chunk_bytes":1024}),
        ),
        ("protocol/conformance", json!({"chunk_bytes":1024})),
        (
            "candidate/contract-expression-catalog",
            json!({"candidate_revision":merged,"target":"calculator.divide"}),
        ),
        (
            "image/target-admission",
            json!({"target":"calculator.add","chunk_bytes":1024}),
        ),
    ] {
        requests.push(bound_frame(&parallel, 100 - requests.len(), method, params));
    }
    let first = parity(&mut sequential, &mut parallel, &requests, true);
    let validation = decode(&first[8]);
    assert_eq!(
        validation["result"]["payload"]["independently_replayed"],
        true
    );
    assert_eq!(validation["result"]["payload"]["commit_authority"], false);
    assert_eq!(validation["result"]["payload"]["tests"], "not_run");
    let mut draft_requests = Vec::new();
    for hole in ["multiply", "negative", "divide"] {
        draft_requests.push(bound_frame(
            &parallel,
            draft_requests.len(),
            "hole/query",
            json!({"draft_revision":draft,"hole_id":hole}),
        ));
    }
    for archive in &state.drafts {
        draft_requests.push(bound_frame(
            &parallel,
            draft_requests.len(),
            "hole/recovery-export",
            json!({"draft_revision":archive.draft_digest(),"chunk_bytes":1024}),
        ));
    }
    draft_requests.push(bound_frame(
        &parallel,
        12,
        "hole/archive-export",
        json!({"draft_revision":draft,"chunk_bytes":1024}),
    ));
    let before = parity(&mut sequential, &mut parallel, &draft_requests, true);
    for row in &before[..3] {
        let value = decode(row);
        assert_eq!(value["result"]["payload"]["source_authority"], false);
        assert_eq!(value["result"]["payload"]["materializable"], false);
    }
    let private = bound_frame(
        &parallel,
        99,
        "candidate/query",
        json!({"candidate_revision":state.private_candidate}),
    );
    let denied = parity(&mut sequential, &mut parallel, &[private], false);
    assert!(decode(&denied[0]).get("error").is_some());
    // Functions are not typed declaration selections for cleanup dependencies.
    // Keep this boundary explicit while the owned-record fixture separately
    // exercises successful candidate cleanup reads in parallel.
    let invalid_cleanup = bound_frame(
        &parallel,
        98,
        "candidate/cleanup-dependencies",
        json!({"candidate_revision":merged,"target":"calculator.multiply","chunk_bytes":1024}),
    );
    let rejected = parity(&mut sequential, &mut parallel, &[invalid_cleanup], false);
    let error = decode(&rejected[0]);
    assert!(error.get("error").is_some(), "{error}");
    assert!(error.to_string().contains("SPX-G334"), "{error}");
    for session in [&mut sequential, &mut parallel] {
        ordinary(
            session,
            "candidate/apply-intent",
            json!({"candidate_revision":merged,"intent":{"kind":"rename_declaration","target":"calculator.not","name":"logical_not"}}),
        );
        ordinary(
            session,
            "hole/fill",
            json!({"draft_revision":draft,"hole_id":"multiply","expression":{"kind":"i64","value":42}}),
        );
    }
    assert_eq!(
        parity(&mut sequential, &mut parallel, &requests, true),
        first
    );
    assert_eq!(
        parity(&mut sequential, &mut parallel, &draft_requests, true),
        before
    );
    for saved in &state.drafts {
        assert_eq!(
            parallel
                .export_draft_archive(&image, saved.draft_digest())
                .unwrap()
                .to_json(),
            saved.to_json()
        );
    }
    assert_eq!(parallel.image_revision(), image);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn archive_chunks_and_attempt_diagnostics_have_exact_sequential_parity_without_apply_authority() {
    let fixture = Fixture::new();
    let state = State::new(&fixture);
    let mut sequential = state.session(&fixture, policy());
    let mut parallel = state.session(&fixture, policy());
    let candidate = state.candidates[3].candidate_digest();
    let mut attempts = Vec::new();
    for session in [&mut sequential, &mut parallel] {
        let outcome = ordinary(
            session,
            "candidate/attempt",
            json!({"candidate_revision":candidate,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":9}}}),
        );
        assert_eq!(outcome["status"], "rejected");
        attempts.push(
            outcome["attempt"]["attempt_revision"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(attempts[0], attempts[1]);
    let requests = [
        ("attempt/summary", json!({"attempt_revision":attempts[0]})),
        (
            "attempt/query",
            json!({"attempt_revision":attempts[0],"chunk_bytes":1024}),
        ),
        (
            "attempt/repair-catalog",
            json!({"attempt_revision":attempts[0]}),
        ),
        (
            "candidate/symbol-diagnostics",
            json!({"candidate_revision":candidate,"target":"calculator.add","chunk_bytes":1024}),
        ),
        (
            "candidate/semantic-delta-catalog",
            json!({"candidate_revision":candidate,"chunk_bytes":1024}),
        ),
        (
            "candidate/semantic-delta",
            json!({"candidate_revision":candidate,"target":"calculator.add","chunk_bytes":1024}),
        ),
        (
            "protocol/conformance",
            json!({"candidate_revision":candidate,"chunk_bytes":1024}),
        ),
        ("protocol/constructor-schemas", json!({})),
        ("validation/catalog", json!({})),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (method, params))| bound_frame(&parallel, id, method, params))
    .collect::<Vec<_>>();
    parity(&mut sequential, &mut parallel, &requests, true);
    let saved = &state.drafts[2];
    let mut actual = String::new();
    loop {
        let request = bound_frame(
            &parallel,
            1,
            "hole/archive-export",
            json!({"draft_revision":saved.draft_digest(),"offset":actual.len(),"chunk_bytes":1024}),
        );
        let rows = parity(&mut sequential, &mut parallel, &[request], true);
        let chunk = &decode(&rows[0])["result"]["payload"];
        assert_eq!(chunk["offset"], actual.len());
        assert_eq!(chunk["source_authority"], false);
        actual.push_str(chunk["chunk"].as_str().unwrap());
        if chunk["next_offset"].is_null() {
            assert_eq!(chunk["total_bytes"], actual.len());
            break;
        }
        assert_eq!(chunk["next_offset"], actual.len());
    }
    assert_eq!(actual, saved.to_json());
}

#[test]
fn execution_and_all_registry_mutations_remain_denied_even_with_build_and_test_grants() {
    let fixture = Fixture::new();
    let state = State::new(&fixture);
    let mut granted = policy();
    granted.build_enabled = true;
    granted.test_policy = Some(CandidateTestPolicy::new(100, 4096, 16384).unwrap());
    let mut sequential = state.session(&fixture, granted);
    let mut parallel = state.session(&fixture, granted);
    let plan = bound_frame(
        &parallel,
        1,
        "candidate/test-plan",
        json!({"candidate_revision":state.candidates[3].candidate_digest()}),
    );
    parity(&mut sequential, &mut parallel, &[plan], true);
    assert!(parallel
        .parallel_read_methods()
        .contains(&"candidate/test-plan"));
    let mut no_test = state.session(&fixture, policy());
    assert!(!no_test
        .parallel_read_methods()
        .contains(&"candidate/test-plan"));
    let request = bound_frame(
        &no_test,
        1,
        "candidate/test-plan",
        json!({"candidate_revision":state.candidates[3].candidate_digest()}),
    );
    let row = no_test.handle_read_batch(&[request.as_slice()], 1).unwrap();
    assert_eq!(decode(&row[0])["error"]["code"], -32601);
    for methods in [
        vec![
            "candidate/open",
            "candidate/apply-intent",
            "candidate/discard",
            "candidate/recovery-restore",
            "candidate/rebase",
            "candidate/merge",
            "candidate/attempt",
            "attempt/repair-apply",
            "attempt/discard",
            "hole/open",
            "hole/open-expression",
            "hole/open-contract-expression",
            "hole/fill",
            "hole/complete",
            "hole/discard",
            "hole/merge",
        ],
        vec![
            "hole/rebase",
            "hole/recovery-restore",
            "hole/archive-restore",
            "candidate/build",
            "candidate/artifact-delta",
            "candidate/test",
            "candidate/commit",
            "source-commit/status",
            "workspace/refresh",
            "workspace/refresh-preview",
        ],
    ] {
        let requests = methods
            .iter()
            .enumerate()
            .map(|(id, name)| bound_frame(&parallel, id, name, json!({})))
            .collect::<Vec<_>>();
        let rows = parallel
            .handle_read_batch(&requests.iter().map(Vec::as_slice).collect::<Vec<_>>(), 4)
            .unwrap();
        for (name, row) in methods.iter().zip(rows) {
            assert!(!parallel.parallel_read_methods().contains(name));
            assert_eq!(decode(&row)["error"]["code"], -32601, "{name}");
        }
    }
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    for method in [
        "candidate/query",
        "candidate/validate",
        "hole/query",
        "hole/archive-export",
    ] {
        let request = bound_frame(&readonly, 1, method, json!({}));
        let row = readonly
            .handle_read_batch(&[request.as_slice()], 1)
            .unwrap();
        assert_eq!(decode(&row[0])["error"]["code"], -32601);
    }
}

#[test]
fn malformed_stale_missing_references_and_bounds_preserve_order_without_mutation() {
    let fixture = Fixture::new();
    let state = State::new(&fixture);
    let mut sequential = state.session(&fixture, policy());
    let mut parallel = state.session(&fixture, policy());
    let wrong = format!("sha256:{}", "0".repeat(64));
    let requests = vec![
        bound_frame(
            &parallel,
            7,
            "candidate/query",
            json!({"candidate_revision":wrong}),
        ),
        bound_frame(
            &parallel,
            1,
            "hole/query",
            json!({"draft_revision":wrong,"hole_id":"multiply"}),
        ),
        bound_frame(
            &parallel,
            9,
            "attempt/query",
            json!({"attempt_revision":wrong}),
        ),
        bound_frame(
            &parallel,
            3,
            "candidate/query",
            json!({"candidate_revision":state.candidates[0].candidate_digest(),"chunk_bytes":1023}),
        ),
        frame(
            4,
            "candidate/query",
            json!({"image_revision":wrong,"candidate_revision":state.candidates[0].candidate_digest()}),
        ),
        b"{".to_vec(),
        br#"{"jsonrpc":"2.0","method":"hole/fill","params":{}}"#.to_vec(),
        Vec::new(),
    ];
    let rows = parity(&mut sequential, &mut parallel, &requests, false);
    for row in &rows[..5] {
        assert!(decode(row).get("error").is_some());
    }
    assert_eq!(decode(&rows[5])["error"]["code"], -32700);
    assert_eq!(rows[6], None);
    assert_eq!(rows[7], None);
    let valid = bound_frame(
        &parallel,
        1,
        "candidate/query",
        json!({"candidate_revision":state.candidates[0].candidate_digest()}),
    );
    for workers in [0, 5] {
        assert_eq!(
            parallel
                .handle_read_batch(&[valid.as_slice()], workers)
                .unwrap_err()[0]
                .code,
            "SPX-G294"
        );
    }
    assert!(parallel.handle_read_batch(&[], 1).is_err());
    assert!(parallel
        .handle_read_batch(&vec![valid.as_slice(); 17], 1)
        .is_err());
    let oversized = vec![b' '; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        parallel
            .handle_read_batch(&[oversized.as_slice()], 1)
            .unwrap_err()[0]
            .code,
        "SPX-G294"
    );
    parity(&mut sequential, &mut parallel, &[valid], true);
}

#[test]
fn source_drift_authenticates_even_all_unknown_registry_reads_and_remains_absorbing() {
    let fixture = Fixture::new();
    let state = State::new(&fixture);
    let mut known = state.session(&fixture, policy());
    let mut unknown = state.session(&fixture, policy());
    let wrong = format!("sha256:{}", "0".repeat(64));
    let known_requests = [
        bound_frame(
            &known,
            1,
            "candidate/query",
            json!({"candidate_revision":state.candidates[0].candidate_digest()}),
        ),
        bound_frame(
            &known,
            2,
            "hole/query",
            json!({"draft_revision":state.drafts[2].draft_digest(),"hole_id":"multiply"}),
        ),
    ];
    let unknown_requests = [
        bound_frame(
            &unknown,
            1,
            "candidate/query",
            json!({"candidate_revision":wrong}),
        ),
        bound_frame(
            &unknown,
            2,
            "hole/query",
            json!({"draft_revision":wrong,"hole_id":"multiply"}),
        ),
        bound_frame(
            &unknown,
            3,
            "attempt/query",
            json!({"attempt_revision":wrong}),
        ),
    ];
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid source drift\n").unwrap();
    assert!(known
        .handle_read_batch(
            &known_requests.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            2
        )
        .is_err());
    assert!(unknown
        .handle_read_batch(
            &unknown_requests
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>(),
            3
        )
        .is_err());
    std::fs::write(&path, original).unwrap();
    assert!(known
        .handle_read_batch(
            &known_requests.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            1
        )
        .is_err());
    assert!(unknown
        .handle_read_batch(
            &unknown_requests
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>(),
            1
        )
        .is_err());
    assert!(known.finish().is_err());
    assert!(unknown.finish().is_err());
}
