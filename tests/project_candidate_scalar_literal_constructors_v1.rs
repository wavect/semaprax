//! Authored, unrun evidence for exact Copy-scalar candidate literals.
use semaprax::ast::{Expr, ExprKind, Type, UnaryOp};
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::{ResolvedExpr, ResolvedExprKind, ResolvedType};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateAttempt,
    ProjectCandidateAttemptOutcome, ProjectCandidateDraft, ProjectExecutionOptions,
    ProjectExecutionOutcome, SemanticChange,
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
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-scalar-literal-constructors-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "scalar-literal-constructors"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "scalar.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["scalar.public"]
tests = ["scalar.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module scalar.core;
@id("scalar.pair") record Pair { @id("scalar.pair.value") value:i64, }
@id("scalar.char") fn character()->char {'a'}
@id("scalar.f32") fn narrow()->f32 {1.0f32}
@id("scalar.f64") fn wide()->f64 {1.0}
@id("scalar.checked") fn checked(value:f64)->f64
    requires value>=0.0
    ensures result>=0.0
{value}
@id("scalar.observe") fn observe()->i64 {
    let left=1.0f32/narrow();
    let right=1.0/wide();
    if character()=='\u{1f600}' && left<0.0f32 && right<0.0 {42}else{0}
}
@id("scalar.defaults") fn defaults()->i64 {7}
@id("scalar.defaults-call") fn defaults_call()->i64 {defaults()}
@id("scalar.legacy") fn legacy()->i64 {9}
@id("scalar.legacy-call") fn legacy_call()->i64 {legacy()}
@id("scalar.public") fn public_value()->i64 {
    if 1.0>=0.0 {observe()+defaults_call()+legacy_call()}else{0}
}
"#,
            ),
            (
                "src/app.spx",
                r#"module scalar.app;
use function @id("scalar.public") from scalar.core as public_value;
@id("scalar.main") fn main()->i64 {public_value()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module scalar.tests;
use function @id("scalar.public") from scalar.core as public_value;
@id("scalar.test") fn main()->i64 {if public_value()>=0 {0}else{1}}
"#,
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
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
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
}

fn tail(mut expression: &Expr) -> &Expr {
    while let ExprKind::Block { statements, tail } = &expression.kind {
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    expression
}

fn resolved_tail(mut expression: &ResolvedExpr) -> &ResolvedExpr {
    while let ResolvedExprKind::Block { statements, tail } = &expression.kind {
        if !statements.is_empty() {
            break;
        }
        expression = tail;
    }
    expression
}

fn body(candidate: &ProjectCandidate, target: &str) -> Expr {
    semaprax::parse(source(candidate), "src/core.spx")
        .unwrap()
        .functions
        .iter()
        .find(|function| function.stable_id == target)
        .unwrap()
        .body
        .clone()
}

fn resolved_body<'a>(candidate: &'a ProjectCandidate, target: &str) -> &'a ResolvedExpr {
    &candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == target)
        .unwrap()
        .body
}

fn selected_contract(candidate: &ProjectCandidate, snippet: &str) -> String {
    let catalog: Value = serde_json::from_str(
        &candidate
            .contract_expression_catalog("scalar.checked")
            .unwrap(),
    )
    .unwrap();
    let rows = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["phase"] == "requires"
                && row["replaceable"] == true
                && source(candidate).get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}

fn character(scalar: u32) -> Value {
    json!({"kind":"char","scalar":format!("{scalar:08x}")})
}

fn f32_bits(bits: u32) -> Value {
    json!({"kind":"f32","bits":format!("{bits:08x}")})
}

fn f64_bits(bits: u64) -> Value {
    json!({"kind":"f64","bits":format!("{bits:016x}")})
}

fn apply(
    base: &ProjectCandidate,
    intent: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    let change = SemanticChange::new(base.revision().project_revision(), &intent)?;
    let candidate = base.apply(base.candidate_digest(), &change)?;
    Ok((candidate, change))
}

fn replace(
    base: &ProjectCandidate,
    target: &str,
    expression: Value,
) -> Result<(ProjectCandidate, SemanticChange), Vec<Diagnostic>> {
    apply(
        base,
        json!({"kind":"replace_function_body","target":target,"body":expression}),
    )
}

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid scalar literal accepted");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}

fn replay(base: &ProjectCandidate, candidate: &ProjectCandidate, changes: &[SemanticChange]) {
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        changes,
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    assert_eq!(
        replayed.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    let reparsed = semaprax::parse(source(candidate), "src/core.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&reparsed), source(candidate));
}

fn assert_float32(candidate: &ProjectCandidate, requested: u32) {
    let ast = body(candidate, "scalar.f32");
    let hir = resolved_body(candidate, "scalar.f32");
    const SIGN: u32 = 0x8000_0000;
    let magnitude = requested & !SIGN;
    if requested & SIGN == 0 {
        assert!(matches!(&tail(&ast).kind, ExprKind::Float32(bits) if *bits == requested));
        assert!(
            matches!(&resolved_tail(hir).kind, ResolvedExprKind::Float32(bits) if *bits == requested)
        );
    } else {
        let ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } = &tail(&ast).kind
        else {
            panic!("signed f32 must lower through canonical unary negation")
        };
        assert!(matches!(&value.kind, ExprKind::Float32(bits) if *bits == magnitude));
        let ResolvedExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } = &resolved_tail(hir).kind
        else {
            panic!("checked signed f32 must retain unary negation")
        };
        assert!(matches!(&value.kind, ResolvedExprKind::Float32(bits) if *bits == magnitude));
    }
    assert_eq!(resolved_tail(hir).ty, ResolvedType::F32);
}

fn assert_float64(candidate: &ProjectCandidate, requested: u64) {
    let ast = body(candidate, "scalar.f64");
    let hir = resolved_body(candidate, "scalar.f64");
    const SIGN: u64 = 0x8000_0000_0000_0000;
    let magnitude = requested & !SIGN;
    if requested & SIGN == 0 {
        assert!(matches!(&tail(&ast).kind, ExprKind::Float64(bits) if *bits == requested));
        assert!(
            matches!(&resolved_tail(hir).kind, ResolvedExprKind::Float64(bits) if *bits == requested)
        );
    } else {
        let ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } = &tail(&ast).kind
        else {
            panic!("signed f64 must lower through canonical unary negation")
        };
        assert!(matches!(&value.kind, ExprKind::Float64(bits) if *bits == magnitude));
        let ResolvedExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } = &resolved_tail(hir).kind
        else {
            panic!("checked signed f64 must retain unary negation")
        };
        assert!(matches!(&value.kind, ResolvedExprKind::Float64(bits) if *bits == magnitude));
    }
    assert_eq!(resolved_tail(hir).ty, ResolvedType::F64);
}

#[test]
fn char_scalars_keep_exact_unicode_identity_graph_and_recovery() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for scalar in [0, 9, 10, 0x27, 0x5c, 0x61, 0x301, 0x1f600, 0x10ffff] {
        let (candidate, change) = replace(&base, "scalar.char", character(scalar)).unwrap();
        assert!(
            matches!(&tail(&body(&candidate,"scalar.char")).kind,ExprKind::Char(value) if *value==scalar)
        );
        assert!(
            matches!(&resolved_tail(resolved_body(&candidate,"scalar.char")).kind,ResolvedExprKind::Char(value) if *value==scalar)
        );
        assert_eq!(
            resolved_tail(resolved_body(&candidate, "scalar.char")).ty,
            ResolvedType::Char
        );
        let graph = candidate.revision().semantic_graph();
        assert!(graph.contains("scalar.char"), "{graph}");
        replay(&base, &candidate, std::slice::from_ref(&change));
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn finite_float_bit_lattice_round_trips_and_signed_values_lower_canonically() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    for bits in [
        0x0000_0000,
        0x8000_0000,
        0x0000_0001,
        0x8000_0001,
        f32::MIN_POSITIVE.to_bits(),
        0x3dcc_cccd,
        f32::MAX.to_bits(),
        f32::MIN.to_bits(),
    ] {
        let (candidate, change) = replace(&base, "scalar.f32", f32_bits(bits)).unwrap();
        assert_float32(&candidate, bits);
        assert!(candidate.revision().semantic_graph().contains("scalar.f32"));
        replay(&base, &candidate, std::slice::from_ref(&change));
    }
    for bits in [
        0x0000_0000_0000_0000,
        0x8000_0000_0000_0000,
        0x0000_0000_0000_0001,
        0x8000_0000_0000_0001,
        f64::MIN_POSITIVE.to_bits(),
        0x3fb9_9999_9999_999a,
        f64::MAX.to_bits(),
        f64::MIN.to_bits(),
    ] {
        let (candidate, change) = replace(&base, "scalar.f64", f64_bits(bits)).unwrap();
        assert_float64(&candidate, bits);
        assert!(candidate.revision().semantic_graph().contains("scalar.f64"));
        replay(&base, &candidate, std::slice::from_ref(&change));
    }

    // Future executable evidence: the ordinary scalar target must observe both
    // signed zero values, while the source projection remains unary and exact.
    let (narrow, narrow_change) =
        replace(&base, "scalar.f32", f32_bits((-0.0f32).to_bits())).unwrap();
    let (wide, wide_change) =
        replace(&narrow, "scalar.f64", f64_bits((-0.0f64).to_bits())).unwrap();
    let (complete, char_change) = replace(&wide, "scalar.char", character(0x1f600)).unwrap();
    assert_eq!(
        complete
            .revision()
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap()
            .outcome(),
        &ProjectExecutionOutcome::Returned(58)
    );
    replay(&base, &complete, &[narrow_change, wide_change, char_change]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn body_and_expression_holes_use_the_same_exact_scalar_grammar() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let body_hole = empty
        .with_body_hole(empty.draft_digest(), "scalar.char", "character")
        .unwrap();
    let catalog: Value =
        serde_json::from_str(&base.expression_catalog("scalar.f64").unwrap()).unwrap();
    let expression_id = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "f64")
        .unwrap()["expression_id"]
        .as_str()
        .unwrap();
    let pending = body_hole
        .with_expression_hole(
            body_hole.draft_digest(),
            "scalar.f64",
            expression_id,
            "wide",
        )
        .unwrap();
    let pending = pending
        .with_contract_expression_hole(
            pending.draft_digest(),
            "scalar.checked",
            &selected_contract(&base, "value >= 0.0"),
            "contract",
        )
        .unwrap();
    let unchanged = pending.to_json().to_owned();
    let one = pending
        .fill_hole(pending.draft_digest(), "character", &character(0x10ffff))
        .unwrap();
    code(one.complete(one.draft_digest()), "SPX-G232");
    let ready = one
        .fill_hole(one.draft_digest(), "wide", &f64_bits((-0.0f64).to_bits()))
        .unwrap();
    code(ready.complete(ready.draft_digest()), "SPX-G232");
    let ready = ready
        .fill_hole(
            ready.draft_digest(),
            "contract",
            &json!({"kind":"binary","op":"==","left":f64_bits((-0.0f64).to_bits()),"right":f64_bits(0)}),
        )
        .unwrap();
    let complete = ready.complete(ready.draft_digest()).unwrap();
    assert!(matches!(
        tail(&body(&complete, "scalar.char")).kind,
        ExprKind::Char(0x10ffff)
    ));
    assert_float64(&complete, (-0.0f64).to_bits());
    assert!(source(&complete).contains("requires -0.0 == 0.0"));
    assert_eq!(pending.to_json(), unchanged);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn legacy_and_ordered_signature_migrations_accept_all_eight_copy_scalars() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let legacy_intent = json!({"kind":"change_function_signature","target":"scalar.legacy","append_parameters":[
        {"name":"glyph","type":"char","argument":character(0x1f600)},
        {"name":"narrow","type":"f32","argument":f32_bits((-0.0f32).to_bits())},
        {"name":"wide","type":"f64","argument":f64_bits(f64::from_bits(1).to_bits())}
    ]});
    let (legacy, legacy_change) = apply(&base, legacy_intent).unwrap();
    let parsed = semaprax::parse(source(&legacy), "src/core.spx").unwrap();
    let provider = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "scalar.legacy")
        .unwrap();
    assert_eq!(
        provider
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .collect::<Vec<_>>(),
        [&Type::Char, &Type::F32, &Type::F64]
    );
    assert!(source(&legacy).contains("legacy('\\u{1f600}', -0.0f32,"));
    replay(&base, &legacy, std::slice::from_ref(&legacy_change));

    let parameters = json!([
        {"name":"wide_i64","type":"i64","argument":{"kind":"i64","value":-7}},
        {"name":"small_i32","type":"i32","argument":{"kind":"i32","value":-3}},
        {"name":"glyph","type":"char","argument":character(0x301)},
        {"name":"byte","type":"u8","argument":{"kind":"u8","value":255}},
        {"name":"size","type":"usize","argument":{"kind":"usize","value":42}},
        {"name":"narrow","type":"f32","argument":f32_bits(0x0000_0001)},
        {"name":"wide","type":"f64","argument":f64_bits(0x8000_0000_0000_0001)},
        {"name":"flag","type":"bool","argument":{"kind":"bool","value":true}}
    ]);
    let (ordered, ordered_change) = apply(
        &base,
        json!({"kind":"change_function_signature","target":"scalar.defaults","parameters":parameters}),
    )
    .unwrap();
    let parsed = semaprax::parse(source(&ordered), "src/core.spx").unwrap();
    let provider = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "scalar.defaults")
        .unwrap();
    assert_eq!(
        provider
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .collect::<Vec<_>>(),
        [
            &Type::I64,
            &Type::I32,
            &Type::Char,
            &Type::U8,
            &Type::Usize,
            &Type::F32,
            &Type::F64,
            &Type::Bool,
        ]
    );
    assert!(source(&ordered).contains("defaults(-7, -3i32, '\\u{301}', 255u8"));
    replay(&base, &ordered, std::slice::from_ref(&ordered_change));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn malformed_nonfinite_and_unrelated_default_grammars_fail_without_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = Arc::new(fixture.candidate());
    let before = base.to_json().to_owned();
    for request in [
        json!({"kind":"char","scalar":""}),
        json!({"kind":"char","scalar":"0000000"}),
        json!({"kind":"char","scalar":"0000006100000062"}),
        json!({"kind":"char","scalar":"ab"}),
        json!({"kind":"char","scalar":"0000000A"}),
        json!({"kind":"char","scalar":"0000d800"}),
        json!({"kind":"char","scalar":"00110000"}),
        json!({"kind":"char","scalar":"00000061","value":"a"}),
        json!({"kind":"f32","bits":"0000000"}),
        json!({"kind":"f32","bits":"3F800000"}),
        json!({"kind":"f32","bits":"7f800000"}),
        json!({"kind":"f32","bits":"ff800000"}),
        json!({"kind":"f32","bits":"7fc00001"}),
        json!({"kind":"f64","bits":"7ff0000000000000"}),
        json!({"kind":"f64","bits":"fff0000000000000"}),
        json!({"kind":"f64","bits":"7ff8000000000042"}),
        json!({"kind":"f64","bits":0}),
    ] {
        code(replace(&base, "scalar.char", request), "SPX-G225");
    }
    for (ty, argument) in [
        ("char", json!({"kind":"char","scalar":"0000d800"})),
        ("f32", json!({"kind":"f32","bits":"7f800000"})),
        ("f64", json!({"kind":"f64","bits":"7ff8000000000001"})),
    ] {
        code(
            apply(
                &base,
                json!({"kind":"change_function_signature","target":"scalar.legacy","append_parameters":[{"name":"bad","type":ty,"argument":argument}]}),
            ),
            "SPX-G225",
        );
    }
    code(
        apply(
            &base,
            json!({"kind":"add_record_field","target":"scalar.pair","field":{"id":"scalar.pair.glyph","name":"glyph","type":"char","default":character(0x61)}}),
        ),
        "SPX-G225",
    );

    // Integer-retag repair remains restricted to its existing scalar class;
    // a rejected char body is retained as diagnostics but offers no repair.
    let rejected_intent = json!({"kind":"replace_function_body","target":"scalar.char","body":{"kind":"i64","value":1}});
    let rejected = match ProjectCandidateAttempt::apply(
        Arc::clone(&base),
        base.candidate_digest(),
        &rejected_intent,
    )
    .unwrap()
    {
        ProjectCandidateAttemptOutcome::Rejected(attempt) => attempt,
        ProjectCandidateAttemptOutcome::Accepted(_) => panic!("wrong-typed char body accepted"),
    };
    let repairs: Value =
        serde_json::from_str(&rejected.repair_catalog(rejected.attempt_digest()).unwrap()).unwrap();
    assert!(repairs["repairs"].as_array().unwrap().is_empty());
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
