use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
    SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SEMANTIC_TRANSACTION_SCHEMA,
    SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA, SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA,
    SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA, SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA,
    SEMANTIC_TRANSACTION_V2_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-semantic-transaction-v2-{}-{}",
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
        let core_path = root.join("src/core.spx");
        let source = std::fs::read_to_string(&core_path).unwrap();
        let source = source.replacen(
            "{\n    left + right\n}",
            "{\n    let subtotal = left + right;\n    let bonus = 1;\n    subtotal + bonus - 1\n}",
            1,
        );
        let parsed = semaprax::parse(&source, &core_path).unwrap();
        std::fs::write(&core_path, semaprax::format::canonical(&parsed)).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    /// A fixture whose `src/core.spx` is exactly `core_source`, verbatim, not
    /// reformatted. Used to author comment-bearing or non-canonical-spacing
    /// sources without the constructor silently canonicalizing them away.
    fn with_core_source(core_source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-universal-semantic-transaction-v2-comments-{}-{}",
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
        std::fs::write(root.join("src/core.spx"), core_source).unwrap();
        Self(root.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

fn selection(revision: &Arc<ProjectRevision>, target: &str, snippet: &str) -> (String, String) {
    let root = ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let catalog: Value = serde_json::from_str(&root.expression_catalog(target).unwrap()).unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == catalog["source"]["path"].as_str().unwrap())
        .unwrap()
        .source();
    let row = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let start = row["source_span"]["start"].as_u64().unwrap() as usize;
            let end = row["source_span"]["end"].as_u64().unwrap() as usize;
            source.get(start..end) == Some(snippet)
        })
        .unwrap_or_else(|| panic!("missing expression {snippet:?}"));
    (
        row["expression_id"].as_str().unwrap().to_owned(),
        snippet.to_owned(),
    )
}

/// Like [`selection`], but the exact old-source snippet is not known in
/// advance (for example because a comment sits inside its span): returns the
/// first replaceable body expression whose raw source slice satisfies
/// `predicate`, verbatim, so a caller never has to hand-transcribe bytes the
/// compiler itself computed.
fn selection_where(
    revision: &Arc<ProjectRevision>,
    target: &str,
    predicate: impl Fn(&str) -> bool,
) -> (String, String) {
    let root = ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap();
    let catalog: Value = serde_json::from_str(&root.expression_catalog(target).unwrap()).unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == catalog["source"]["path"].as_str().unwrap())
        .unwrap()
        .source();
    let row = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            if row["replaceable"] != true {
                return false;
            }
            let start = row["source_span"]["start"].as_u64().unwrap() as usize;
            let end = row["source_span"]["end"].as_u64().unwrap() as usize;
            source.get(start..end).is_some_and(&predicate)
        })
        .unwrap_or_else(|| panic!("no replaceable expression satisfies the predicate"));
    let start = row["source_span"]["start"].as_u64().unwrap() as usize;
    let end = row["source_span"]["end"].as_u64().unwrap() as usize;
    (
        row["expression_id"].as_str().unwrap().to_owned(),
        source[start..end].to_owned(),
    )
}

fn replacement() -> Value {
    json!({
        "kind":"binary", "op":"+",
        "left":{"kind":"place","name":"subtotal"},
        "right":{"kind":"i64","value":2}
    })
}

#[test]
fn nested_expression_is_deterministic_candidate_equivalent_read_only_and_replayable() {
    let fixture = Fixture::new();
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old,
            replacement(),
        ),
    )
    .unwrap();
    assert_eq!(
        SemanticTransactionV2::from_json(transaction.to_json().as_bytes())
            .unwrap()
            .to_json(),
        transaction.to_json()
    );
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let repeated = transaction.validate(Arc::clone(&revision)).unwrap();
    assert_eq!(artifacts.evidence(), repeated.evidence());
    assert_eq!(artifacts.result(), repeated.result());
    assert_eq!(
        serde_json::from_str::<Value>(transaction.to_json()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.impact()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.review()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.result()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(artifacts.evidence()).unwrap()["schema"],
        SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA
    );
    assert!(artifacts
        .candidate()
        .revision()
        .sources()
        .iter()
        .any(|source| source.path() == "src/core.spx"
            && source.source().contains("subtotal + 2 - 1")));

    let direct_root =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let direct_change = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "expression_id":expression_id, "kind":"replace_expression",
            "replacement":replacement(), "target":"calculator.add"
        }),
    )
    .unwrap();
    let direct = direct_root
        .apply(direct_root.candidate_digest(), &direct_change)
        .unwrap();
    assert_eq!(artifacts.candidate().to_json(), direct.to_json());
    assert_eq!(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            artifacts.evidence().as_bytes(),
        )
        .unwrap()
        .result(),
        artifacts.result()
    );
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn stale_contract_type_and_wire_mutations_fail_closed() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let make = |id: &str, expected: &str, replacement: Value| {
        SemanticTransactionV2::replace_expression(
            workspace.workspace_revision(),
            SemanticTransactionReplaceExpression::new("calculator.add", id, expected, replacement),
        )
        .unwrap()
    };
    assert_code(
        make("stale-expression-id", &old, replacement()).validate(Arc::clone(&revision)),
        "SPX-G527",
    );
    assert_code(
        make(&expression_id, "subtotal - bonus", replacement()).validate(Arc::clone(&revision)),
        "SPX-G527",
    );
    assert!(
        make(&expression_id, &old, json!({"kind":"bool","value":true}))
            .validate(Arc::clone(&revision))
            .is_err()
    );

    let (contract_id, contract) = selection(&revision, "calculator.divide", "right != 0");
    let contract = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.divide",
            contract_id,
            contract,
            json!({"kind":"bool","value":true}),
        ),
    )
    .unwrap();
    assert_code(contract.validate(Arc::clone(&revision)), "SPX-G525");

    let transaction = make(&expression_id, &old, replacement());
    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();
    let other = make(
        &expression_id,
        &old,
        json!({
            "kind":"binary", "op":"+",
            "left":{"kind":"place","name":"subtotal"},
            "right":{"kind":"i64","value":3}
        }),
    );
    let other_artifacts = other.validate(Arc::clone(&revision)).unwrap();
    assert_code(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            other_artifacts.evidence().as_bytes(),
        ),
        "SPX-G527",
    );
    let mut tampered: Value = serde_json::from_str(artifacts.evidence()).unwrap();
    tampered["artifacts"]["result"]["value"]["authority"]["granted"] = json!(true);
    let mut tampered = serde_json::to_string(&tampered).unwrap();
    tampered.push('\n');
    assert_code(
        SemanticTransactionV2::replay(
            Arc::clone(&revision),
            transaction.to_json().as_bytes(),
            tampered.as_bytes(),
        ),
        "SPX-G527",
    );
    assert_code(
        SemanticTransactionV2::from_json(transaction.to_json().trim_end().as_bytes()),
        "SPX-G525",
    );
}

#[test]
fn main_is_admitted_and_v1_wire_remains_closed_and_byte_stable() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.app.main", "6");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.app.main",
            expression_id,
            old,
            json!({"kind":"i64","value":7}),
        ),
    )
    .unwrap();
    transaction.validate(Arc::clone(&revision)).unwrap();

    let v1 = SemanticTransaction::rename_display_name(
        workspace.workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let expected_v1: Value = json!({
        "expected_workspace_revision": workspace.workspace_revision(),
        "invariants": [
            "preserve_stable_identity", "preserve_public_exports", "update_all_callers",
            "no_new_effects", "no_new_capabilities", "preserve_contracts",
            "revalidate_ownership_and_cleanup", "preserve_project_profile_admission",
            "preserve_admitted_core_targets"
        ],
        "operations": [{
            "expected_old_value":"add", "kind":"rename_display_name",
            "new_value":"sum", "target":"calculator.add"
        }],
        "requested_authority":"none",
        "requested_validation":[
            "canonical_source_round_trip", "complete_project_admission",
            "ownership_and_cleanup", "native_and_wasm_emission",
            "canonical_workspace_revision"
        ],
        "schema":SEMANTIC_TRANSACTION_SCHEMA,
    });
    let mut expected_v1 = serde_json::to_string(&expected_v1).unwrap();
    expected_v1.push('\n');
    assert_eq!(v1.to_json(), expected_v1);
    assert_code(
        SemanticTransaction::from_json(transaction.to_json().as_bytes()),
        "SPX-G525",
    );
    assert_code(
        SemanticTransactionV2::from_json(v1.to_json().as_bytes()),
        "SPX-G525",
    );
}

#[test]
fn replacement_values_are_bounded_iteratively_before_cloning_or_rendering() {
    let digest = format!("sha256:{}", "0".repeat(64));
    let mut deep = json!({"kind":"i64","value":0});
    for _ in 0..65 {
        deep = json!({"kind":"nested","value":deep});
    }
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new("target", "expression", "0", deep),
        ),
        "SPX-G526",
    );
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new(
                "target",
                "expression",
                "0",
                Value::Array(vec![Value::Null; 8_193]),
            ),
        ),
        "SPX-G526",
    );
    assert_code(
        SemanticTransactionV2::replace_expression(
            &digest,
            SemanticTransactionReplaceExpression::new(
                "target",
                "expression",
                "0",
                json!({"kind":"text","value":"x".repeat(1024 * 1024 + 1)}),
            ),
        ),
        "SPX-G526",
    );
}

/// A `calculator.core` source with a top-of-file note, a leading and a
/// trailing declaration comment, a comment inside `add`'s body (outside the
/// edited span), a comment whose own text itself contains `//` (proving a
/// comment's interior text is verbatim, not a fresh comment boundary), and an
/// end-of-file comment after the last declaration — the position families
/// docs/CANONICAL-COMMENTS-V1.md documents, reused unmodified here against
/// the v2 ReplaceExpression route rather than `fmt`/`patch`.
fn commented_core_source() -> String {
    concat!(
        "// Top-of-file license note.\n",
        "module calculator.core;\n",
        "\n",
        "// calculator.add sums two operands, then folds in a bonus.\n",
        "@id(\"calculator.add\")\n",
        "fn add(left: i64, right: i64) -> i64\n",
        "{\n",
        "    // subtotal is the plain sum before the bonus is folded in.\n",
        "    let subtotal = left + right;\n",
        "    let bonus = 1;\n",
        "    subtotal + bonus - 1\n",
        "}\n",
        "// trails add\n",
        "\n",
        "@id(\"calculator.subtract\")\n",
        "fn subtract(left: i64, right: i64) -> i64\n",
        "{\n",
        "    left - right\n",
        "}\n",
        "\n",
        "@id(\"calculator.multiply\")\n",
        "fn multiply(left: i64, right: i64) -> i64\n",
        "{\n",
        "    left * right\n",
        "}\n",
        "\n",
        "@id(\"calculator.divide\")\n",
        "fn divide(left: i64, right: i64) -> i64\n",
        "    requires right != 0\n",
        "{\n",
        "    left / right\n",
        "}\n",
        "\n",
        "@id(\"calculator.is-negative\")\n",
        "fn is_negative(value: i64) -> bool\n",
        "{\n",
        "    value < 0\n",
        "}\n",
        "\n",
        "// see http://example.com for background.\n",
        "@id(\"calculator.not\")\n",
        "fn not(value: bool) -> bool\n",
        "{\n",
        "    !value\n",
        "}\n",
        "// end of file\n",
    )
    .to_owned()
}

/// Negative control matching this fixture's own documentation: before this
/// change, `docs/UNIVERSAL-SEMANTIC-TRANSACTION-V2.md` listed
/// `comment/trivia-preserving editing` as an explicit nonclaim, and
/// `semantic_transaction_v2.rs::validate` rejected *any* commented base with
/// `SPX-G525` ("semantic transaction v2 requires comment-free canonical
/// source") before selecting an expression at all. This asserts the fixture
/// really does carry comments and is not comment-free canonical, so the test
/// below is not vacuous.
#[test]
fn commented_core_source_fixture_actually_has_comments() {
    let source = commented_core_source();
    let (program, comments) = semaprax::parse_with_comments(&source, "src/core.spx").unwrap();
    assert_eq!(comments.items.len(), 6);
    assert_ne!(semaprax::format::canonical(&program), source);
    // The fixture is exactly canonical once comments are restored: it is
    // `fmt --check`-clean with comments admitted, the precondition this
    // route now accepts for its one authenticated edited source.
    assert_eq!(
        semaprax::format::comments::canonical_with_comments(&program, &comments),
        source
    );
}

#[test]
fn commented_target_source_is_preserved_outside_the_edited_span() {
    let core_source = commented_core_source();
    let fixture = Fixture::with_core_source(&core_source);
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old,
            replacement(),
        ),
    )
    .unwrap();

    let artifacts = transaction.validate(Arc::clone(&revision)).unwrap();

    // The only bytes that may differ from the original commented source are
    // exactly the authenticated "subtotal + bonus" span; every comment,
    // every unrelated declaration, and every blank line survive verbatim.
    let expected_preserved = core_source.replacen("subtotal + bonus", "subtotal + 2", 1);
    assert_eq!(
        artifacts.preserved_target_source(),
        Some(expected_preserved.as_str())
    );
    for needle in [
        "// Top-of-file license note.",
        "// calculator.add sums two operands, then folds in a bonus.",
        "// subtotal is the plain sum before the bonus is folded in.",
        "// trails add",
        "// see http://example.com for background.",
        "// end of file",
    ] {
        assert!(
            artifacts
                .preserved_target_source()
                .unwrap()
                .contains(needle),
            "missing {needle:?}"
        );
    }

    // Independent reparse and byte comparison, beyond the transaction's own
    // internal check: stripping comments from the preserved text and
    // canonicalizing it reproduces the already fully validated candidate
    // text exactly, and no comment was lost or duplicated.
    let preserved = artifacts.preserved_target_source().unwrap();
    let (reparsed, reparsed_comments) =
        semaprax::parse_with_comments(preserved, "src/core.spx").unwrap();
    let candidate_target = artifacts
        .candidate()
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    assert_eq!(semaprax::format::canonical(&reparsed), candidate_target);
    assert_eq!(reparsed_comments.items.len(), 6);

    // Every source other than the edited one, and the edited source's own
    // disk bytes, are untouched: validation never writes.
    assert_eq!(inventory(&fixture.0), disk_before);

    // Re-validating the same transaction against the same base is exactly
    // reproducible: evidence, result, and the preserved projection agree.
    let repeated = transaction.validate(Arc::clone(&revision)).unwrap();
    assert_eq!(artifacts.evidence(), repeated.evidence());
    assert_eq!(artifacts.result(), repeated.result());
    assert_eq!(
        artifacts.preserved_target_source(),
        repeated.preserved_target_source()
    );
}

#[test]
fn comment_embedded_in_the_edited_expression_span_is_refused() {
    // The audit note sits between two statements of `add`'s body, a
    // well-formed comment position (CANONICAL-COMMENTS-V1's "between
    // statements"), but this test selects the *whole body block* for
    // replacement, so the comment falls inside the authenticated span.
    let core_source = commented_core_source().replacen(
        "    let bonus = 1;\n",
        "    // audit: keep this note with the bonus binding\n    let bonus = 1;\n",
        1,
    );
    let fixture = Fixture::with_core_source(&core_source);
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection_where(&revision, "calculator.add", |snippet| {
        snippet.contains("audit") && snippet.contains("left + right")
    });
    assert!(old.contains("audit: keep this note"));
    // The whole body block is selected, so only `add`'s parameters are in
    // scope for the replacement (no local bindings yet).
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old,
            json!({
                "kind": "binary", "op": "+",
                "left": {"kind": "place", "name": "left"},
                "right": {"kind": "place", "name": "right"},
            }),
        ),
    )
    .unwrap();

    let errors = match transaction.validate(Arc::clone(&revision)) {
        Ok(_) => panic!("expected the embedded-comment span to be refused"),
        Err(errors) => errors,
    };
    assert!(
        errors.iter().any(|error| error.code == "SPX-G525"),
        "{errors:?}"
    );
    // Assert the *specific* refusal, not merely some SPX-G525: this fixture
    // is canonical-with-comments (the whole-body-block precondition holds),
    // so this must be refused by the embedded-comment guard specifically,
    // never by silently dropping the comment.
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("refuses a comment inside the authenticated expression span")),
        "{errors:?}"
    );
    // A failed transaction leaves authoritative source unchanged.
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn non_canonical_target_spacing_is_refused_not_silently_reformatted() {
    // Legal but non-canonical: two blank lines between statements. This is
    // not a comment, so it is outside this bounded route's claim; the
    // canonical formatter's own guarantees are not weakened to accept it,
    // per AGENTS.md's instruction to say so explicitly rather than choose
    // silently between the two. It turns out this fails even earlier than
    // `validate`: `workspace_graph`'s own workspace admission already
    // requires every source be `canonical_with_comments`-clean (SPX-G170)
    // before a `ProjectRevision` can exist at all, so this route's own
    // narrower precondition in `validate` is unreachable for this case and
    // is defense in depth, not the enforcement point. Either way, no source
    // byte is rewritten.
    let core_source = examples_core_source().replacen(
        "    left + right\n",
        "    let subtotal = left + right;\n\n\n    subtotal\n",
        1,
    );
    let fixture = Fixture::with_core_source(&core_source);
    let disk_before = inventory(&fixture.0);
    let errors = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .map(|_: Arc<ProjectRevision>| ())
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G170"),
        "{errors:?}"
    );
    assert_eq!(inventory(&fixture.0), disk_before);
}

#[test]
fn a_comment_in_an_unrelated_source_is_still_refused() {
    // This bounded route admits comments only in the one authenticated
    // edited source; every other source keeps the pre-existing exact
    // canonical, comment-free requirement. Documented explicitly rather than
    // silently widened, since materialize's shared candidate rebuild has no
    // comment representation for sources it is not asked to preserve.
    let fixture = Fixture::new();
    let app_path = fixture.0.join("src/app.spx");
    let app_source = std::fs::read_to_string(&app_path).unwrap();
    std::fs::write(&app_path, format!("// entry point\n{app_source}")).unwrap();
    let disk_before = inventory(&fixture.0);
    let revision = fixture.revision();
    let workspace = revision.canonical_workspace_revision().unwrap();
    let (expression_id, old) = selection(&revision, "calculator.add", "subtotal + bonus");
    let transaction = SemanticTransactionV2::replace_expression(
        workspace.workspace_revision(),
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            &expression_id,
            &old,
            replacement(),
        ),
    )
    .unwrap();

    assert_code(transaction.validate(Arc::clone(&revision)), "SPX-G525");
    assert_eq!(inventory(&fixture.0), disk_before);
}

fn examples_core_source() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/src/core.spx"),
    )
    .unwrap()
}
