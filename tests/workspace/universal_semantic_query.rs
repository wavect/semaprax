use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectFrontendSource, ProjectManifest, ProjectRevision,
    SemanticQuery, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticWorkspaceService, MAX_SEMANTIC_QUERY_BYTES, MAX_SEMANTIC_QUERY_RESULT_BYTES,
    SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA, SEMANTIC_QUERY_DECLARATIONS_SCHEMA,
    SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA, SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA,
    SEMANTIC_QUERY_RESULT_SCHEMA, SEMANTIC_QUERY_SCHEMA,
};
use semaprax::query::QueryFilters;
use semaprax::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PATHS: [&str; 3] = ["src/app.spx", "src/core.spx", "src/tests.spx"];

struct Fixture(PathBuf);

impl Fixture {
    fn new(comment: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-universal-semantic-query-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        let core = std::fs::read_to_string(root.join("src/core.spx")).unwrap();
        let extra = r#"
@id("calculator.identity")
fn identity<T>(value: T) -> T
{
    value
}

fn automatic_value() -> i64
{
    9
}

@id("calculator.pair")
record Pair
{
    @id("calculator.pair.value") value: i64,
}
"#;
        let combined = format!("{core}{extra}");
        let program = semaprax::parse(&combined, "src/core.spx").unwrap();
        let mut canonical = semaprax::format::canonical(&program);
        if comment {
            canonical.insert_str(0, "// retained human note\n");
        }
        std::fs::write(root.join("src/core.spx"), canonical).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> ProjectManifest {
        ProjectManifest::parse(&std::fs::read_to_string(self.0.join("semaprax.toml")).unwrap())
            .unwrap()
    }

    fn sources(&self) -> Vec<ProjectFrontendSource> {
        PATHS
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path,
                    &std::fs::read_to_string(self.0.join(path)).unwrap(),
                )
                .unwrap()
            })
            .collect()
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                result.insert(relative, Vec::new());
                visit(root, &path, result);
            } else {
                result.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn value(source: &str) -> Value {
    serde_json::from_str(source).unwrap()
}

fn result_payload(result: &semaprax::project::SemanticQueryResult) -> Value {
    assert_eq!(
        value(result.to_json())["schema"],
        SEMANTIC_QUERY_RESULT_SCHEMA
    );
    value(result.payload())
}

fn automatic_id(snapshot: &semaprax::project::SemanticWorkspaceSnapshot) -> String {
    let query = SemanticQuery::declarations(
        snapshot.workspace_revision(),
        &QueryFilters {
            name: Some("automatic_value".to_owned()),
            ..QueryFilters::default()
        },
        0,
        8,
    )
    .unwrap();
    let payload = result_payload(&snapshot.query(&query).unwrap());
    payload["matches"][0]["id"].as_str().unwrap().to_owned()
}

fn operation(snapshot: &semaprax::project::SemanticWorkspaceSnapshot, id: &str) -> Value {
    let query = SemanticQuery::available_operations(snapshot.workspace_revision(), id).unwrap();
    result_payload(&snapshot.query(&query).unwrap())["operations"][0].clone()
}

#[test]
fn checked_ownership_and_direct_consumer_facts_are_canonical_and_replayable() {
    let fixture = Fixture::new(false);
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let generation = service.active_generation();
    let revision = generation.workspace_revision();
    let function = generation
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "calculator.add")
        .unwrap();
    let expression_id = function.body.id.as_str();

    let ownership =
        SemanticQuery::ownership_at_expression(revision, "calculator.add", expression_id).unwrap();
    assert_eq!(
        SemanticQuery::from_json(ownership.to_json().as_bytes()).unwrap(),
        ownership
    );
    let ownership_result = service.query(ownership.to_json().as_bytes()).unwrap();
    let ownership_payload = result_payload(&ownership_result);
    assert_eq!(
        ownership_payload["schema"],
        SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA
    );
    assert_eq!(ownership_payload["stable_id"], "calculator.add");
    assert_eq!(
        ownership_payload["expression"]["expression_id"],
        expression_id
    );
    assert_eq!(ownership_payload["expression"]["ownership_mode"], "value");
    assert_eq!(ownership_payload["expression"]["loans"], json!([]));
    let snapshot = service.snapshot(revision).unwrap();
    assert_eq!(
        SemanticQuery::replay(
            &snapshot,
            ownership.to_json().as_bytes(),
            ownership_result.result_digest(),
            ownership_result.to_json().as_bytes(),
        )
        .unwrap()
        .to_json(),
        ownership_result.to_json()
    );

    let consumers = SemanticQuery::declaration_consumers(revision, "calculator.add", 0, 1).unwrap();
    let consumer_result = service.query(consumers.to_json().as_bytes()).unwrap();
    let payload = result_payload(&consumer_result);
    assert_eq!(
        payload["schema"],
        SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA
    );
    assert_eq!(payload["total_consumers"], 2);
    assert_eq!(
        payload["consumers"][0]["consumer_id"],
        "calculator.app.main"
    );
    assert_eq!(payload["consumers"][0]["visibility"], "local");
    assert_eq!(payload["consumers"][0]["use_kinds"], json!(["direct_call"]));
    assert_eq!(payload["next_offset"], 1);
    let page_two = SemanticQuery::declaration_consumers(revision, "calculator.add", 1, 1).unwrap();
    let payload_two = result_payload(&service.query(page_two.to_json().as_bytes()).unwrap());
    assert_eq!(
        payload_two["consumers"][0]["consumer_id"],
        "calculator.tests.main"
    );
    assert_eq!(payload_two["consumers"][0]["visibility"], "test");
    assert_eq!(payload_two["next_offset"], Value::Null);

    let unknown_expression =
        SemanticQuery::ownership_at_expression(revision, "calculator.add", "foreign.expr").unwrap();
    assert_code(
        service.query(unknown_expression.to_json().as_bytes()),
        "SPX-G531",
    );
    assert_code(
        SemanticQuery::declaration_consumers(revision, "calculator.add", 0, 0),
        "SPX-G531",
    );
}

#[test]
fn five_query_constructors_round_trip_and_delegate_exact_payloads() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    let expected = snapshot.workspace_revision();
    let kind = WorkspaceAnalysisTargetKind::Declaration;
    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        ..QueryFilters::default()
    };
    let queries = [
        SemanticQuery::declarations(expected, &filters, 0, 2).unwrap(),
        SemanticQuery::symbol(expected, "calculator.add").unwrap(),
        SemanticQuery::context(
            expected,
            kind,
            "calculator.add",
            WorkspaceContextOptions::default(),
        )
        .unwrap(),
        SemanticQuery::impact(
            expected,
            kind,
            "calculator.add",
            WorkspaceImpactOptions::default(),
        )
        .unwrap(),
        SemanticQuery::available_operations(expected, "calculator.add").unwrap(),
    ];
    for query in &queries {
        assert_eq!(value(query.to_json())["schema"], SEMANTIC_QUERY_SCHEMA);
        let parsed = SemanticQuery::from_json(query.to_json().as_bytes()).unwrap();
        assert_eq!(&parsed, query);
        assert_eq!(parsed.query_digest(), query.query_digest());
        let first = snapshot.query(query).unwrap();
        let second = query.execute(&snapshot).unwrap();
        assert_eq!(first.to_json(), second.to_json());
        assert_eq!(first.result_digest(), second.result_digest());
        assert_eq!(first.query_digest(), query.query_digest());
        assert_eq!(first.workspace_revision(), expected);
        assert_eq!(value(first.to_json())["authority"], false);
    }

    assert_eq!(
        queries[1].execute(&snapshot).unwrap().payload(),
        snapshot.symbol("calculator.add").unwrap()
    );
    assert_eq!(
        queries[2].execute(&snapshot).unwrap().payload(),
        snapshot
            .context(kind, "calculator.add", WorkspaceContextOptions::default())
            .unwrap()
    );
    assert_eq!(
        queries[3].execute(&snapshot).unwrap().payload(),
        snapshot
            .impact(kind, "calculator.add", WorkspaceImpactOptions::default())
            .unwrap()
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn declaration_pages_are_complete_nonoverlapping_and_match_the_direct_project_query() {
    let fixture = Fixture::new(false);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        ..QueryFilters::default()
    };
    let direct = semaprax::query::run_project(&revision, &filters).unwrap();
    let direct: Value = value(&semaprax::query::project_json(&direct));
    let first = result_payload(
        &snapshot
            .query(
                &SemanticQuery::declarations(snapshot.workspace_revision(), &filters, 0, 2)
                    .unwrap(),
            )
            .unwrap(),
    );
    assert_eq!(first["schema"], SEMANTIC_QUERY_DECLARATIONS_SCHEMA);
    assert_eq!(
        first["matches"],
        Value::Array(direct["matches"].as_array().unwrap()[0..2].to_vec())
    );
    assert_eq!(
        first["total_matches"],
        direct["matches"].as_array().unwrap().len()
    );
    assert_eq!(first["next_offset"], 2);
    let second = result_payload(
        &snapshot
            .query(
                &SemanticQuery::declarations(snapshot.workspace_revision(), &filters, 2, 128)
                    .unwrap(),
            )
            .unwrap(),
    );
    assert_eq!(
        second["matches"],
        Value::Array(direct["matches"].as_array().unwrap()[2..].to_vec())
    );
    assert_eq!(second["next_offset"], Value::Null);
}

#[test]
fn available_rename_is_truthful_but_still_requires_full_transaction_validation() {
    let fixture = Fixture::new(false);
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    let query =
        SemanticQuery::available_operations(snapshot.workspace_revision(), "calculator.add")
            .unwrap();
    let result = service.query(query.to_json().as_bytes()).unwrap();
    let payload = result_payload(&result);
    assert_eq!(
        payload["schema"],
        SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA
    );
    assert_eq!(payload["stable_id"], "calculator.add");
    assert_eq!(payload["operations"][0]["available"], true);
    assert_eq!(payload["operations"][0]["expected_old_value"], "add");
    assert_eq!(
        payload["operations"][0]["transaction_schema"],
        "semaprax.semantic-transaction.v1"
    );
    assert_eq!(payload["operations"].as_array().unwrap().len(), 4);
    assert_eq!(payload["operations"][1]["kind"], "replace_block");
    assert_eq!(payload["operations"][1]["available"], true);
    assert_eq!(
        payload["operations"][1]["expected_old_block"],
        "{\n    left + right\n}"
    );
    assert_eq!(payload["operations"][2]["kind"], "add_contract");
    assert_eq!(payload["operations"][2]["available"], true);
    assert_eq!(
        payload["operations"][2]["expected_old_contract"],
        json!({"ensures":[],"requires":[]})
    );
    assert_eq!(
        payload["operations"][2]["phases"],
        json!(["requires", "ensures"])
    );
    assert_eq!(payload["operations"][3]["kind"], "add_declaration");
    assert_eq!(payload["operations"][3]["available"], true);
    assert_eq!(
        payload["operations"][3]["constructor"],
        "closed_project_candidate_add_declaration"
    );
    let old_module = &payload["operations"][3]["expected_old_module"];
    assert_eq!(old_module["source_path"], "src/core.spx");
    assert!(old_module["declaration_ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "calculator.add"));
    let source_digest = old_module["source_digest"].as_str().unwrap();
    assert!(source_digest.starts_with("sha256:"));
    assert_eq!(source_digest, source_digest.to_ascii_lowercase());

    let transaction = SemanticTransaction::rename_display_name(
        snapshot.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let direct = transaction.validate(Arc::clone(&revision)).unwrap();
    let through_service = service
        .validate_transaction(transaction.to_json().as_bytes())
        .unwrap();
    assert_eq!(through_service.evidence(), direct.evidence());
    assert_eq!(
        service.active_generation().workspace_revision(),
        snapshot.workspace_revision()
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn unavailable_targets_and_comment_bearing_projects_report_exact_constraints() {
    let fixture = Fixture::new(false);
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    assert_eq!(
        operation(&snapshot, "calculator.app.main")["available"],
        false
    );
    assert_eq!(
        operation(&snapshot, "calculator.app.main")["constraints"]["non_main"],
        false
    );
    assert_eq!(
        operation(&snapshot, "calculator.identity")["available"],
        false
    );
    assert_eq!(
        operation(&snapshot, "calculator.identity")["constraints"]["monomorphic"],
        false
    );
    let automatic = automatic_id(&snapshot);
    assert_eq!(operation(&snapshot, &automatic)["available"], false);
    assert_eq!(
        operation(&snapshot, &automatic)["constraints"]["explicit_identity"],
        false
    );
    assert_eq!(operation(&snapshot, "calculator.pair")["available"], false);

    let commented = Fixture::new(true);
    let commented_service = SemanticWorkspaceService::open(commented.revision()).unwrap();
    let commented_snapshot = commented_service
        .snapshot(commented_service.active_generation().workspace_revision())
        .unwrap();
    let rename = operation(&commented_snapshot, "calculator.add");
    assert_eq!(rename["available"], false);
    assert_eq!(
        rename["constraints"]["comment_free_canonical_workspace"],
        false
    );
}

#[test]
fn stale_service_rejects_old_query_while_retained_snapshot_replays_exactly() {
    let fixture = Fixture::new(false);
    let manifest = fixture.manifest();
    let sources = fixture.sources();
    let mut service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let old_revision = service.active_generation().workspace_revision().to_owned();
    let old_snapshot = service.snapshot(&old_revision).unwrap();
    let query = SemanticQuery::symbol(&old_revision, "calculator.add").unwrap();
    let old_result = old_snapshot.query(&query).unwrap();
    let changed = sources
        .iter()
        .map(|source| {
            let text = if source.path() == "src/app.spx" {
                let changed = source.source().replace("multiply(6, 7)", "multiply(6, 8)");
                semaprax::format::canonical(&semaprax::parse(&changed, source.path()).unwrap())
            } else {
                source.source().to_owned()
            };
            ProjectFrontendSource::new(source.path(), &text).unwrap()
        })
        .collect::<Vec<_>>();
    service
        .refresh_owned_sources(&manifest, &changed, &old_revision)
        .unwrap();
    assert_code(service.query(query.to_json().as_bytes()), "SPX-G533");
    let retained = old_snapshot.query(&query).unwrap();
    assert_eq!(retained.to_json(), old_result.to_json());
    assert_eq!(
        SemanticQuery::replay(
            &old_snapshot,
            query.to_json().as_bytes(),
            old_result.result_digest(),
            old_result.to_json().as_bytes()
        )
        .unwrap()
        .to_json(),
        old_result.to_json()
    );
}

#[test]
fn malformed_noncanonical_reminted_and_oversize_documents_fail_closed() {
    let fixture = Fixture::new(false);
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    let query = SemanticQuery::symbol(snapshot.workspace_revision(), "calculator.add").unwrap();
    let result = snapshot.query(&query).unwrap();
    assert_code(SemanticQuery::from_json(b"not-json"), "SPX-G531");
    assert_code(
        SemanticQuery::from_json(query.to_json().trim_end().as_bytes()),
        "SPX-G531",
    );
    assert_code(
        SemanticQuery::from_json(&vec![b' '; MAX_SEMANTIC_QUERY_BYTES + 1]),
        "SPX-G532",
    );
    assert_code(
        SemanticQuery::replay(
            &snapshot,
            query.to_json().as_bytes(),
            result.result_digest(),
            &vec![b' '; MAX_SEMANTIC_QUERY_RESULT_BYTES + 1],
        ),
        "SPX-G532",
    );

    let mut tampered = value(result.to_json());
    tampered["payload"]["symbol"]["name"] = json!("forged");
    let mut bytes = serde_json::to_string(&tampered).unwrap();
    bytes.push('\n');
    let mut hash = Sha256::new();
    hash.update(b"semaprax.semantic-query.result.digest.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes.as_bytes());
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    );
    assert_code(
        SemanticQuery::replay(
            &snapshot,
            query.to_json().as_bytes(),
            &digest,
            bytes.as_bytes(),
        ),
        "SPX-G533",
    );
}
