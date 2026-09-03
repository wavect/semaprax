//! Authored, unrun regressions. Source replay is not a claim of runtime proof.
use semaprax::ast::{ExprKind, Function, ParamMode, Statement, Type};
use semaprax::cleanup_plan::{CleanupResultSource, ExitContinuation, StorageId};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::OwnershipMode;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectExecutionOptions, ProjectExecutionOutcome,
    SemanticChange,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(owner: &str, body: Option<&str>, entry_value: i64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-owned-block-extraction-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        std::fs::write(
            fixture.0.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "owned-block-extraction"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "block.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["block.public"]
tests = ["block.tests"]
"#,
        )
        .unwrap();
        let (declaration, ty, constructor, mode) = match owner {
            "Bytes" => ("", "Bytes", "make_bytes()", "own "),
            "Packet" => (
                r#"@id("block.packet") record Packet { @id("block.packet.bytes") bytes:Bytes, }"#,
                "Packet",
                "Packet { bytes: make_bytes() }",
                "own ",
            ),
            "Choice" => (
                r#"@id("block.choice") variant Choice { @id("block.choice.empty") Empty, @id("block.choice.full") Full { @id("block.choice.full.bytes") bytes:Bytes, }, }"#,
                "Choice",
                "Choice::Full { bytes: make_bytes() }",
                "own ",
            ),
            "string" => ("", "string", "\"owned text\"", ""),
            _ => panic!("unknown fixture owner"),
        };
        let body = body.map(str::to_owned).unwrap_or_else(|| {
            format!(
                "let computed = {{ let held = {constructor}; let local = {constructor}; consume(local) + value }}; computed"
            )
        });
        // The String case is checked source evidence only. Do not imply a mixed
        // String/aggregate executable profile merely because HIR was retained.
        let public_body = if owner == "string" {
            "value"
        } else {
            "evaluate(value)"
        };
        fixture.write("src/core.spx", &format!(r#"module block.core;
{declaration}
@id("block.make-bytes") fn make_bytes()->Bytes {{let input=[7u8,8u8];let input_view=array_as_slice(input);bytes_copy(input_view)}}
@id("block.consume") fn consume(input:{mode}{ty})->i64 {{1}}
@id("block.evaluate") fn evaluate(value:i64)->i64 {{ {body} }}
@id("block.identity") fn identity(value:i64)->i64 {{value}}
@id("block.public") fn public_value(value:i64)->i64 {{ {public_body} }}
"#));
        fixture.write(
            "src/app.spx",
            &format!(
                r#"module block.app;
use function @id("block.public") from block.core as public_value;
@id("block.main") fn main()->i64 {{public_value({entry_value})}}
"#
            ),
        );
        fixture.write(
            "src/tests.spx",
            r#"module block.tests;
use function @id("block.public") from block.core as public_value;
@id("block.test") fn main()->i64 {public_value(2)}
"#,
        );
        fixture
    }
    fn write(&self, path: &str, source: &str) {
        let parsed = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
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
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn source(candidate: &ProjectCandidate) -> &str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn function(candidate: &ProjectCandidate, id: &str) -> Function {
    semaprax::parse(source(candidate), "src/core.spx")
        .unwrap()
        .functions
        .iter()
        .find(|f| f.stable_id == id)
        .unwrap()
        .clone()
}
fn selection(candidate: &ProjectCandidate, target: &str, marker: &str, block: bool) -> Value {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            let text = &source(candidate)
                [span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize];
            text.contains(marker) && (!block || text.starts_with('{'))
        })
        .min_by_key(|row| {
            row["source_span"]["end"].as_u64().unwrap()
                - row["source_span"]["start"].as_u64().unwrap()
        })
        .expect("missing authored selection")
        .clone()
}
fn change(candidate: &ProjectCandidate, target: &str, selected: &Value) -> SemanticChange {
    SemanticChange::new(candidate.revision().project_revision(), &json!({"kind":"extract_function","target":target,"expression_id":selected["expression_id"],"new_id":"block.helper","new_name":"extracted_block"})).unwrap()
}
fn extract(
    candidate: &ProjectCandidate,
    target: &str,
    marker: &str,
    block: bool,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = change(
        candidate,
        target,
        &selection(candidate, target, marker, block),
    );
    Ok((
        candidate.apply(candidate.candidate_digest(), &change)?,
        change,
    ))
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("unsupported extraction accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, change: &SemanticChange) {
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        std::slice::from_ref(change),
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    assert_eq!(
        replayed.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
}
fn same_outcome(
    base: &ProjectCandidate,
    candidate: &ProjectCandidate,
    expected: ProjectExecutionOutcome,
) {
    // Future executable regression; this task does not run the interpreter.
    let original = base
        .revision()
        .execute_entry(&ProjectExecutionOptions::default())
        .unwrap();
    let changed = candidate
        .revision()
        .execute_entry(&ProjectExecutionOptions::default())
        .unwrap();
    assert_eq!(original.outcome(), &expected);
    assert_eq!(changed.outcome(), &expected);
}

#[test]
fn resource_free_owned_results_cross_one_authenticated_helper_boundary() {
    for (owner, ty, constructor, expected_type) in [
        ("Bytes", "Bytes", "make_bytes()", Type::Bytes),
        (
            "Packet",
            "Packet",
            "Packet { bytes: make_bytes() }",
            Type::Named {
                name: "Packet".to_owned(),
                arguments: Vec::new(),
            },
        ),
        (
            "Choice",
            "Choice",
            "Choice::Full { bytes: make_bytes() }",
            Type::Named {
                name: "Choice".to_owned(),
                arguments: Vec::new(),
            },
        ),
    ] {
        let fixture = Fixture::new(owner, None, 2);
        let core = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
        fixture.write(
            "src/core.spx",
            &format!(
                "{core}\n@id(\"block.owner-result\") fn owner_result()->{ty} {{let computed={{let local={constructor};local}};computed}}\n@id(\"block.owner-result-use\") fn owner_result_use()->i64 {{consume(owner_result())}}\n"
            ),
        );
        fixture.write(
            "src/app.spx",
            r#"module block.app;
use function @id("block.owner-result-use") from block.core as owner_result_use;
@id("block.main") fn main()->i64 {owner_result_use()}
"#,
        );
        let disk = fixture.bytes();
        let base = fixture.candidate();
        let (candidate, change) = extract(&base, "block.owner-result", "let local", true).unwrap();
        let helper = function(&candidate, "block.helper");
        assert!(helper.params.is_empty());
        assert_eq!(helper.return_type, expected_type);
        assert!(matches!(
            helper.body.kind,
            ExprKind::Block {
                ref statements,
                ref tail
            } if statements.is_empty() && matches!(tail.kind, ExprKind::Block { .. })
        ));
        let checked = candidate
            .revision()
            .entry_program()
            .functions
            .iter()
            .find(|function| function.id.as_str() == "block.helper")
            .unwrap();
        assert_eq!(checked.body.ownership, OwnershipMode::Own);
        assert!(checked.params.is_empty());
        assert!(checked.loan_plan.loans.is_empty());
        assert!(checked
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .is_empty());
        assert!(checked
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty());
        let result_sources = checked
            .cleanup_plan
            .exits
            .iter()
            .filter_map(|exit| match &exit.continuation {
                ExitContinuation::CommitResult { source } => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!result_sources.is_empty());
        assert!(result_sources.iter().all(|source| matches!(
            source,
            CleanupResultSource::Owned { storage }
                if storage.storage == StorageId::ProvisionalResult
                    && storage.projections.is_empty()
        )));
        same_outcome(&base, &candidate, ProjectExecutionOutcome::Returned(1));
        replay(&base, &candidate, &change);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn internal_owners_remain_nested_and_only_copy_values_cross_the_helper_boundary() {
    for owner in ["Bytes", "Packet", "Choice", "string"] {
        let fixture = Fixture::new(owner, None, 2);
        let disk = fixture.bytes();
        let base = fixture.candidate();
        let parent = base.to_json().to_owned();
        let original = function(&base, "block.evaluate");
        let ExprKind::Block { statements, .. } = &original.body.kind else {
            panic!("root block")
        };
        let Statement::Let {
            value: selected, ..
        } = &statements[0]
        else {
            panic!("selected initializer")
        };
        let (candidate, change) = extract(&base, "block.evaluate", "let local", true).unwrap();
        let helper = function(&candidate, "block.helper");
        assert_eq!(helper.params.len(), 1);
        assert_eq!(helper.params[0].name, "value");
        assert_eq!(helper.params[0].mode, ParamMode::Value);
        assert_eq!(helper.params[0].ty, Type::I64);
        assert_eq!(helper.return_type, Type::I64);
        assert!(helper.requires.is_empty() && helper.ensures.is_empty());
        let ExprKind::Block { statements, tail } = &helper.body.kind else {
            panic!("fresh root")
        };
        assert!(
            statements.is_empty(),
            "owners must not become helper root locals"
        );
        assert!(matches!(tail.kind, ExprKind::Block { .. }));
        // Compare canonical AST projections instead of obsolete source spans.
        let mut projection = semaprax::parse(source(&base), "src/core.spx").unwrap();
        let mut probe = original.clone();
        probe.body = selected.clone();
        projection.functions = vec![probe.clone()];
        let before = semaprax::format::canonical(&projection);
        probe.body = (**tail).clone();
        projection.functions = vec![probe];
        assert_eq!(semaprax::format::canonical(&projection), before);
        assert_eq!(
            function(&candidate, "block.consume").stable_id,
            "block.consume"
        );
        assert_eq!(
            candidate.revision().manifest().to_canonical_toml(),
            base.revision().manifest().to_canonical_toml()
        );
        if owner == "string" {
            for item in [&base, &candidate] {
                assert!(!item
                    .revision()
                    .entry_program()
                    .functions
                    .iter()
                    .any(|f| matches!(f.id.as_str(), "block.evaluate" | "block.helper")));
            }
        } else {
            let checked = candidate
                .revision()
                .entry_program()
                .functions
                .iter()
                .find(|f| f.id.as_str() == "block.helper")
                .unwrap();
            assert!(checked
                .params
                .iter()
                .all(|p| p.ownership == semaprax::hir::OwnershipMode::Value));
            assert!(checked.loan_plan.loans.is_empty());
            assert!(checked
                .cleanup_plan
                .entry_state
                .live_owned_parameters
                .is_empty());
            assert!(checked
                .cleanup_plan
                .entry_state
                .conditional_owned_parameters
                .is_empty());
            assert!(checked
                .cleanup_plan
                .regions
                .iter()
                .any(|region| region.parent.is_some()
                    && region
                        .slots
                        .iter()
                        .any(|storage| checked.cleanup_plan.slots.iter().any(|slot| slot
                            .storage
                            == *storage
                            && matches!(
                                slot.ty,
                                semaprax::hir::ResolvedType::Bytes
                                    | semaprax::hir::ResolvedType::Nominal { .. }
                            )))));
            same_outcome(&base, &candidate, ProjectExecutionOutcome::Returned(3));
        }
        replay(&base, &candidate, &change);
        code(
            candidate.apply(base.candidate_digest(), &change),
            "SPX-G224",
        );
        assert_eq!(base.to_json(), parent);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn failing_and_unreached_owned_blocks_keep_their_original_lazy_position() {
    for input in [1, -1] {
        let fixture=Fixture::new("Bytes",Some("if value >= 0 { let held = make_bytes(); let local = make_bytes(); let consumed = consume(local); consumed / (value - value) } else { 42 }"),input);
        let disk = fixture.bytes();
        let base = fixture.candidate();
        let (candidate, change) = extract(&base, "block.evaluate", "let local", true).unwrap();
        let expected = if input < 0 {
            ProjectExecutionOutcome::Returned(42)
        } else {
            ProjectExecutionOutcome::LanguageFailure(
                semaprax::runtime_status::normalize_arithmetic(
                    semaprax::cleanup_plan::StatusCase::DivisionByZero,
                ),
            )
        };
        same_outcome(&base, &candidate, expected);
        let changed = function(&candidate, "block.evaluate");
        let ExprKind::Block { tail, .. } = changed.body.kind else {
            panic!("root")
        };
        let ExprKind::If {
            then_branch,
            else_branch,
            ..
        } = tail.kind
        else {
            panic!("lazy branch retained")
        };
        let ExprKind::Block { tail, .. } = then_branch.kind else {
            panic!("replacement block")
        };
        assert!(matches!(tail.kind,ExprKind::Call{ref name,..} if name=="extracted_block"));
        assert!(!format!("{:?}", else_branch.kind).contains("extracted_block"));
        replay(&base, &candidate, &change);
        assert_eq!(fixture.bytes(), disk);
    }
}

#[test]
fn owner_boundaries_root_postconditions_and_view_subtrees_remain_rejected() {
    let fixture = Fixture::new("Bytes", None, 2);
    let additional = r#"
@id("block.root") fn root(value:i64)->i64 ensures result >= value {let local=make_bytes();consume(local)+value}
@id("block.capture") fn capture(input:own Bytes)->i64 {let computed={consume(input)};computed}
@id("block.non-block") fn non_block()->i64 {consume(make_bytes())}
@id("block.view") fn view()->usize {let computed={let local=make_bytes();let view=bytes_as_slice(local);byte_len(view)};computed}
"#;
    let core = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
    fixture.write("src/core.spx", &(core + additional));
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let parent = base.to_json().to_owned();
    for (target, marker, block) in [
        ("block.root", "let local", true),
        ("block.capture", "consume(input)", true),
        ("block.evaluate", "make_bytes()", false),
        ("block.non-block", "consume(make_bytes())", false),
        ("block.view", "let local", true),
    ] {
        code(extract(&base, target, marker, block), "SPX-G225");
    }
    assert_eq!(base.to_json(), parent);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn rebase_replays_the_owned_block_and_rejects_a_competing_source_body() {
    let fixture = Fixture::new("Bytes", None, 2);
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let (candidate, _) = extract(&base, "block.evaluate", "let local", true).unwrap();
    let rename = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"block.identity","name":"identity_renamed"}),
    )
    .unwrap();
    let shifted = base.apply(base.candidate_digest(), &rename).unwrap();
    let rebased = candidate
        .rebase(
            candidate.candidate_digest(),
            Arc::clone(shifted.revision()),
            shifted.revision().project_revision(),
        )
        .unwrap()
        .into_candidate();
    assert_eq!(
        function(&rebased, "block.identity").name,
        "identity_renamed"
    );
    let helper = function(&rebased, "block.helper");
    assert!(
        matches!(helper.body.kind,ExprKind::Block{ref statements, ref tail} if statements.is_empty() && matches!(tail.kind,ExprKind::Block{..}))
    );
    let restored = ProjectCandidate::restore(
        Arc::clone(rebased.base_revision()),
        rebased.base_revision().project_revision(),
        rebased.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), rebased.to_json());
    let competing=SemanticChange::new(base.revision().project_revision(),&json!({"kind":"replace_function_body","target":"block.evaluate","body":{"kind":"i64","value":99}})).unwrap();
    let competing = base.apply(base.candidate_digest(), &competing).unwrap();
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(competing.revision()),
            competing.revision().project_revision(),
        ),
        "SPX-G235",
    );
    assert_eq!(fixture.bytes(), disk);
}
