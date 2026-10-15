//! Owning controls for the closed scalar lexical receiver. Every positive case
//! is an actual retained Workspace build of the original persisted
//! terminated-guard owner (or a mixed scalar module), compared with the
//! independently resolved full-program Graph schema, HIR, cleanup and runtime
//! oracle. Every hostile case first requires the same unmodified admission.
//! These definitions are authored; executable qualification is pending.
use super::*;
use crate::cleanup_plan::{ReplayPhysicalRefusal, SelectedReplayCause, SelectedReplayUnpaidReason};
use crate::interpreter::ResolvedEvaluationOutcome;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
use crate::workspace_graph::source_schema::REPORT;

const PATH: &str = "src/app.spx";
const TESTS_PATH: &str = "src/tests.spx";
const TESTS: &str =
    "module calculator.tests;\n@id(\"calculator.tests.main\") fn main() -> i64 { 0 }\n";
/// The original OPT766 body, verbatim from the persisted CLI owner.
const BODY: &str = "let mut i=0; let mut total=0; while i<3 { i=i+1; if i==2 {continue;} else {0}; total=total+i; 0 } total";

fn source(body: &str) -> String {
    format!("module calculator.app; @id(\"calculator.app.main\") fn main()->i64 {{ {body} }}")
}
fn with_spare(body: &str, spare: &str) -> String {
    format!(
        "module calculator.app; {spare} @id(\"calculator.app.main\") fn main()->i64 {{ {body} }}"
    )
}
fn canonical(text: &str) -> String {
    format::canonical(&crate::parse(text, std::path::Path::new(PATH)).unwrap())
}
fn sources_with(app: &str, extra: &[(&str, &str)]) -> Vec<WorkspaceSource> {
    [(PATH, app), (TESTS_PATH, TESTS)]
        .into_iter()
        .chain(extra.iter().copied())
        .map(|(path, text)| WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&crate::parse(text, std::path::Path::new(path)).unwrap()),
        })
        .collect()
}
fn try_build_with(
    app: &str,
    extra: &[(&str, &str)],
) -> Result<
    (
        WorkspaceGraphBuild,
        crate::workspace_graph::current_source::SourceFacts,
    ),
    Vec<Diagnostic>,
> {
    build_owned_retaining_sources_for_change(sources_with(app, extra), MAX_CHANGE_BUILDER_BYTES)
}
fn build(
    app: &str,
) -> (
    WorkspaceGraphBuild,
    crate::workspace_graph::current_source::SourceFacts,
) {
    try_build_with(app, &[]).unwrap()
}
fn module<'a>(build: &'a WorkspaceGraphBuild, path: &str) -> &'a WorkspaceResolvedModule {
    build.hir.modules.iter().find(|m| m.path == path).unwrap()
}
fn module_mut<'a>(
    build: &'a mut WorkspaceGraphBuild,
    path: &str,
) -> &'a mut WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter_mut()
        .find(|m| m.path == path)
        .unwrap()
}
fn main_mut(build: &mut WorkspaceGraphBuild) -> &mut ResolvedFunction {
    module_mut(build, PATH)
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "calculator.app.main")
        .unwrap()
}

/// Independent full-program oracle on independently resolved source: complete
/// HIR validation, canonical cleanup replay and the original Graph schema.
fn oracle(text: &str) -> (hir::ResolvedProgram, &'static str) {
    let program = crate::check(text, PATH).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    crate::cleanup_plan::validate_program(&resolved).unwrap();
    let schema = crate::graph::graph_schema(&resolved).unwrap();
    (resolved, schema)
}
fn evaluate(program: &hir::ResolvedProgram) -> ResolvedEvaluationOutcome {
    crate::interpreter::evaluate_resolved_zero_arg_i64(program, "calculator.app.main", 100_000)
        .unwrap()
        .outcome
}
fn received(
    build: &WorkspaceGraphBuild,
    path: &str,
    sources: &dyn ReceivingSources,
) -> Result<Option<&'static str>, SelectedReplayRefusal> {
    received_member(build, module(build, path), sources)
}
fn received_member(
    build: &WorkspaceGraphBuild,
    held: &WorkspaceResolvedModule,
    sources: &dyn ReceivingSources,
) -> Result<Option<&'static str>, SelectedReplayRefusal> {
    receive_paid(
        held,
        &build.hir.modules,
        &build.hir.declarations,
        sources,
        REPORT,
    )
    .map(|receipt| receipt.map(|receipt| receipt.select(held).expect("own module")))
}
fn refused(build: &WorkspaceGraphBuild, sources: &dyn ReceivingSources) -> SelectedReplayRefusal {
    match received(build, PATH, sources) {
        Err(refusal) => refusal,
        Ok(schema) => panic!("hostile reception must refuse, selected {schema:?}"),
    }
}
fn assert_public(report: &[Diagnostic]) {
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].code, "SPX-G410");
    assert_eq!(
        report[0].message,
        "retained static UTF-8 source-site authority disagrees"
    );
    assert_eq!(report[0].message_owned_capacity(), report[0].message.len());
}
fn assert_mismatch(refusal: &SelectedReplayRefusal, case: &str) {
    assert!(refusal.is_paid(), "{case}");
    assert_public(refusal.report().unwrap());
    assert_eq!(
        refusal.refused().map(|r| r.cause()),
        Some(SelectedReplayCause::Mismatch),
        "{case}"
    );
}
fn first_receipt(refusal: &mut SelectedReplayRefusal) -> ReplayPhysicalRefusal {
    let ledger = refusal.ledger_mut().expect("retained ledger");
    let first = ledger.reserve(0).unwrap_err();
    assert_eq!(ledger.reserve(1).unwrap_err(), first);
    first
}

#[test]
fn original_terminated_guard_is_received_from_current_source_with_full_program_parity() {
    let text = canonical(&source(BODY));
    let (build, sources) = build(&text);
    let app = module(&build, PATH);
    assert!(
        app.scalar_lexical.is_some(),
        "lexical scalar owner captured"
    );
    assert!(
        module(&build, TESTS_PATH).scalar_lexical.is_none(),
        "an exit-free module keeps its existing route"
    );
    // Frozen from-parts and source-less routes keep their original refusal.
    let frozen = graph::graph_schema_from_parts_and_instances(
        &app.interfaces,
        &app.types,
        &app.functions,
        &app.function_templates,
        &app.function_instances,
    )
    .unwrap_err();
    assert_eq!(frozen.code, "SPX-G410");
    assert_eq!(
        build.source_graph_schemas().unwrap_err()[0].code,
        "SPX-G410"
    );
    let (direct, schema) = oracle(&text);
    let tests_schema = crate::graph::graph_schema(
        &hir::resolve(&crate::check(TESTS, TESTS_PATH).unwrap()).unwrap(),
    )
    .unwrap();
    let schemas = build.source_graph_schemas_receiving(&sources).unwrap();
    assert_eq!(schemas[PATH], schema, "original full-program Graph family");
    assert_eq!(schemas[TESTS_PATH], tests_schema);
    assert_eq!(
        received(&build, PATH, &sources).unwrap_or_else(|_| panic!("paid source receipt required")),
        Some(schema)
    );
    // Retained HIR and canonical plans equal the independent program's.
    for retained in &app.functions {
        let control = direct
            .functions
            .iter()
            .find(|f| f.id == retained.id)
            .unwrap();
        assert_eq!(retained.params, control.params, "{}", retained.id);
        assert_eq!(retained.return_type, control.return_type, "{}", retained.id);
        assert_eq!(retained.body, control.body, "{}", retained.id);
        assert_eq!(
            retained.cleanup_plan, control.cleanup_plan,
            "{}",
            retained.id
        );
    }
    let linked = build
        .linked_owned_data_api_program_with_roots("calculator.app", &[])
        .unwrap();
    for _ in 0..3 {
        assert_eq!(evaluate(&linked), evaluate(&direct));
        assert_eq!(evaluate(&linked), ResolvedEvaluationOutcome::ReturnedI64(4));
    }
}

#[test]
fn canonical_semicolon_control_is_the_same_generation_and_body_drift_is_a_new_receipt() {
    let text = canonical(&source(BODY));
    let control = canonical(&source(&BODY.replace("else {0};", "else {0}")));
    assert_eq!(
        control, text,
        "optional guard terminator canonicalizes away"
    );
    let drift = canonical(&source(&BODY.replace("total+i", "total+i+1")));
    for (text, expected) in [(&text, 4), (&drift, 6)] {
        let (build, sources) = build(text);
        let (direct, schema) = oracle(text);
        assert_eq!(
            build.source_graph_schemas_receiving(&sources).unwrap()[PATH],
            schema
        );
        let linked = build
            .linked_owned_data_api_program_with_roots("calculator.app", &[])
            .unwrap();
        assert_eq!(evaluate(&linked), evaluate(&direct));
        assert_eq!(
            evaluate(&linked),
            ResolvedEvaluationOutcome::ReturnedI64(expected)
        );
    }
    // A former generation's source cannot authorize the drifted build, and
    // the drifted source cannot authorize the former build.
    let (original, original_sources) = build(&text);
    let (drifted, drifted_sources) = build(&drift);
    assert_mismatch(
        &refused(&original, &drifted_sources),
        "old build, new source",
    );
    assert_mismatch(
        &refused(&drifted, &original_sources),
        "new build, old source",
    );
}

#[test]
fn mixed_unrelated_scalar_member_is_replayed_and_selects_the_oracle_usize_family() {
    let text = canonical(&with_spare(
        BODY,
        "@id(\"calculator.app.spare\") fn spare(n:usize)->usize { n }",
    ));
    let (build, sources) = build(&text);
    assert_eq!(module(&build, PATH).functions.len(), 2);
    let (direct, schema) = oracle(&text);
    assert_eq!(
        build.source_graph_schemas_receiving(&sources).unwrap()[PATH],
        schema
    );
    let linked = build
        .linked_owned_data_api_program_with_roots("calculator.app", &[])
        .unwrap();
    assert_eq!(evaluate(&linked), evaluate(&direct));
}

#[test]
fn mixed_exit_free_looping_member_is_replayed_by_the_paid_scalar_validator() {
    let text = canonical(&with_spare(
        BODY,
        "@id(\"calculator.app.count\") fn count(n:i64)->i64 { let mut k=0; while k<n { k=k+1; 0 } k }",
    ));
    let (build, sources) = build(&text);
    let (_, schema) = oracle(&text);
    assert_eq!(
        build.source_graph_schemas_receiving(&sources).unwrap()[PATH],
        schema
    );
}

#[test]
fn current_source_namespace_member_and_layout_attacks_refuse_with_one_paid_report() {
    let text = canonical(&source(BODY));
    let drift = canonical(&source(&BODY.replace("total+i", "total+i+1")));
    for attack in 0..10 {
        let (mut build, sources) = build(&text);
        assert!(
            received(&build, PATH, &sources)
                .unwrap_or_else(|_| panic!("paid source receipt required"))
                .is_some(),
            "{attack}"
        );
        // Preserve the original owned hostile source mutations after genuine
        // metadata-backed positive admission, without mutating shared custody.
        let mut sources: Vec<WorkspaceSource> = sources
            .iter()
            .map(|source| source.input().clone())
            .collect();
        let at = sources.iter().position(|s| s.path == PATH).unwrap();
        // Record exact current membership before corrupting the path. A lookup
        // by the stale authored path would panic before the paid receiver.
        let member = build
            .hir
            .modules
            .iter()
            .position(|m| m.path == PATH)
            .unwrap();
        match attack {
            // Authentic HIR paired with another generation's source bytes.
            0 => sources[at].source = drift.clone(),
            // Missing or duplicated current source path.
            1 => {
                sources.remove(at);
            }
            2 => sources.push(sources[at].clone()),
            // Current HIR signature, body span and contract drift.
            3 => main_mut(&mut build).return_type = hir::ResolvedType::I32,
            4 => main_mut(&mut build).body.span.start += 1,
            5 => {
                let tail = main_mut(&mut build).body.clone();
                main_mut(&mut build).ensures.push(tail);
            }
            // Explicit identity origin and current namespace/path drift.
            6 => {
                build
                    .hir
                    .declarations
                    .get_mut("calculator.app.main")
                    .unwrap()
                    .origin = hir::IdentityOrigin::Automatic
            }
            7 => module_mut(&mut build, PATH).module.push('x'),
            8 => module_mut(&mut build, PATH).path.push('x'),
            // Forged loop-latch kind is outside the closed family.
            _ => {
                let hir::ResolvedExprKind::Block { statements, .. } =
                    &mut main_mut(&mut build).body.kind
                else {
                    panic!("block")
                };
                let mut forged = false;
                for statement in statements {
                    if let hir::ResolvedStatement::LexicalWhile { latch, body, .. } = statement {
                        *latch = hir::lexical_exit::LoopLatchKind::SourceForStep {
                            kind: hir::lexical_exit::SourceForKind::Copy,
                            step: body.id.clone(),
                        };
                        forged = true;
                    }
                }
                assert!(forged, "the authored loop is a typed lexical while");
            }
        }
        let refusal = match received_member(&build, &build.hir.modules[member], &sources) {
            Err(refusal) => refusal,
            Ok(schema) => panic!("attack {attack} selected {schema:?}"),
        };
        assert_mismatch(&refusal, &format!("attack {attack}"));
        assert!(
            refusal.receipt().is_none(),
            "attack {attack}: no Session receipt"
        );
        assert_public(&build.source_graph_schemas_receiving(&sources).unwrap_err());
    }
}

#[test]
fn reordered_missing_transplanted_detached_and_inserted_inventories_refuse() {
    let spare = "@id(\"calculator.app.spare\") fn spare(n:i64)->i64 { n+1 }";
    let text = canonical(&with_spare(BODY, spare));
    let other = canonical(&with_spare(&BODY.replace("total+i", "total+i+1"), spare));
    for attack in 0..5 {
        let (mut build, sources) = build(&text);
        assert!(received(&build, PATH, &sources)
            .unwrap_or_else(|_| panic!("paid source receipt required"))
            .is_some());
        match attack {
            0 => module_mut(&mut build, PATH)
                .scalar_lexical
                .as_mut()
                .unwrap()
                .members_mut()
                .swap(0, 1),
            1 => {
                module_mut(&mut build, PATH)
                    .scalar_lexical
                    .as_mut()
                    .unwrap()
                    .members_mut()
                    .pop();
            }
            // Same-cardinality inventory copied from another generation.
            2 => {
                let (mut donor, _) = self::build(&other);
                std::mem::swap(
                    &mut module_mut(&mut build, PATH).scalar_lexical,
                    &mut module_mut(&mut donor, PATH).scalar_lexical,
                );
            }
            // Inventory inserted into an otherwise unqualified module.
            3 => {
                let inventory = module_mut(&mut build, PATH).scalar_lexical.take();
                module_mut(&mut build, TESTS_PATH).scalar_lexical = inventory;
                let refusal = match received(&build, TESTS_PATH, &sources) {
                    Err(refusal) => refusal,
                    Ok(schema) => panic!("inserted inventory selected {schema:?}"),
                };
                assert_mismatch(&refusal, "inserted");
                continue;
            }
            // A detached equal module object outside the live module slice.
            _ => {
                let (detached, _) = self::build(&text);
                let held = module(&detached, PATH);
                let answer = receive_paid(
                    held,
                    &build.hir.modules,
                    &build.hir.declarations,
                    &sources,
                    REPORT,
                );
                let Err(refusal) = answer else {
                    panic!("detached equal HIR must not authorize publication")
                };
                assert_mismatch(&refusal, "detached");
                continue;
            }
        }
        assert_mismatch(&refused(&build, &sources), &format!("inventory {attack}"));
    }
}

#[test]
fn certificate_and_plan_mutations_refuse_without_repair() {
    let text = canonical(&source(BODY));
    for attack in 0..3 {
        let (mut build, sources) = build(&text);
        assert!(received(&build, PATH, &sources)
            .unwrap_or_else(|_| panic!("paid source receipt required"))
            .is_some());
        let function = main_mut(&mut build);
        let site = function.body.id.clone();
        let control = function.cleanup_plan.lexical_control.as_mut().unwrap();
        match attack {
            0 => control.exits[0].site = site,
            1 => {
                control.loops[0].target = hir::ControlTarget::Function(
                    hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                )
            }
            // A forged higher cleanup family is excluded before the Session.
            _ => function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15,
        }
        let mut refusal = refused(&build, &sources);
        assert!(refusal.is_paid());
        assert_public(refusal.report().unwrap());
        let cause = refusal.refused().unwrap().cause();
        if attack < 2 {
            // The independently replayed cause is the Session's own refusal,
            // retained as the first receipt (an owned Diagnostic or a fact).
            assert_eq!(cause, SelectedReplayCause::Session, "attack {attack}");
            assert!(
                refusal.receipt().is_some() || refusal.fact_receipt().is_some(),
                "attack {attack}: Session receipt"
            );
        } else {
            assert_eq!(cause, SelectedReplayCause::Mismatch);
        }
        let _ = first_receipt(&mut refusal);
        // The paid report restores no admission: the owner remains failed.
        let mut preparation = refusal.into_preparation().expect("retained owner");
        let anchor = module(&build, PATH).functions.first().unwrap();
        assert!(preparation.try_charge(anchor, 0, "restore").is_err());
        drop(preparation);
    }
}

#[test]
fn closed_family_exclusions_keep_their_existing_route() {
    const PROVIDER: &str =
        "module calculator.lib;\n@id(\"calculator.lib.one\") fn one()->i64 { 1 }\n";
    let probe = "@id(\"calculator.app.probe\") fn probe(x:i64)->i64 { x }";
    let cases: [(String, &[(&str, &str)]); 6] = [
        // An ordinary Call in a dead branch.
        (
            with_spare(
                &BODY.replace("else {0};", "else {if false {let unused=probe(1);0} else {0}};"),
                probe,
            ),
            &[],
        ),
        // An owned String operation hidden in a dead branch.
        (
            source(&BODY.replace("else {0};", "else {if false {let unused=string_len(\"dead\");0} else {0}};")),
            &[],
        ),
        // An unused nominal record declaration.
        (
            with_spare(
                BODY,
                "@id(\"calculator.app.row\") record Row { @id(\"calculator.app.row.n\") n:i64, }",
            ),
            &[],
        ),
        // A Vec carrier.
        (
            source(&format!("let v=vec_with_capacity<i64>(1usize); {BODY}")),
            &[],
        ),
        // A Call hidden in a contract.
        (
            format!(
                "module calculator.app; {probe} @id(\"calculator.app.main\") fn main()->i64 requires probe(0)==0 {{ {BODY} }}"
            ),
            &[],
        ),
        // An import, even unused, leaves the no-import profile.
        (
            format!(
                "module calculator.app; use function @id(\"calculator.lib.one\") from calculator.lib as one; @id(\"calculator.app.main\") fn main()->i64 {{ {BODY} }}"
            ),
            &[("src/lib.spx", PROVIDER)],
        ),
    ];
    for (index, (text, extra)) in cases.iter().enumerate() {
        if index < 4 {
            // Independently establish ordinary language/HIR admission before
            // asking the retained builder to prove its narrower route. The
            // dormant Call/String children are preserved, not constant-folded
            // away to make the selected family accept them.
            let ordinary_source = crate::check(text, PATH).unwrap();
            let ordinary = hir::resolve(&ordinary_source).unwrap();
            hir::validate(&ordinary).unwrap();
            let main = ordinary
                .functions
                .iter()
                .find(|f| f.id.as_str() == "calculator.app.main")
                .unwrap();
            assert!(main.cleanup_plan.lexical_control.is_some(), "case {index}");
        }
        // Dormant operations remain actual children, while every normal
        // lexical guard completion retains its mandatory literal-zero tail.
        // The contract and import shapes may be refused by the ordinary
        // checker itself, which also keeps them outside every route; the
        // dead-branch Call, String, record and Vec controls must build.
        let built = try_build_with(&canonical(text), extra);
        let (build, sources) = match built {
            Ok(built) => built,
            Err(errors) => {
                assert!(
                    index >= 4,
                    "case {index} must be an admitted ordinary module: {errors:?}"
                );
                continue;
            }
        };
        assert!(
            module(&build, PATH).scalar_lexical.is_none(),
            "case {index}: excluded shapes capture no inventory"
        );
        let receiving = build.source_graph_schemas_receiving(&sources);
        let legacy = build.source_graph_schemas();
        match (receiving, legacy) {
            (Ok(receiving), Ok(legacy)) => assert_eq!(receiving, legacy, "case {index}"),
            (Err(receiving), Err(legacy)) => {
                assert_eq!(receiving[0].code, legacy[0].code, "case {index}")
            }
            _ => panic!("case {index}: sources changed an excluded route"),
        }
    }
}

#[test]
fn receipt_selects_only_its_own_module_without_post_release_work() {
    let (build, sources) = build(&canonical(&source(BODY)));
    let held = module(&build, PATH);
    for own in [true, false] {
        let receipt = receive_paid(
            held,
            &build.hir.modules,
            &build.hir.declarations,
            &sources,
            REPORT,
        )
        .unwrap_or_else(|_| panic!("paid source receipt required"))
        .expect("captured owner");
        let target = if own {
            held
        } else {
            module(&build, TESTS_PATH)
        };
        let (selected, metrics) = measure(|| receipt.select(target));
        assert_eq!(selected.is_some(), own);
        assert!(!metrics.invalid);
        assert_eq!((metrics.allocations, metrics.allocated), (0, 0));
    }
}

/// Independent reference for one in-family census: the signature charge, the
/// `DEPTH` cursor initialization, one unit per walk step (each node once plus
/// once per returned child) and each block's statement count.
fn census_reference_units(function: &ResolvedFunction) -> usize {
    let mut units = function.params.len() + 1 + census::DEPTH;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        let mut pending = vec![(root, 0usize)];
        while let Some((expression, next)) = pending.pop() {
            units += 1;
            if next == 0 {
                if let hir::ResolvedExprKind::Block { statements, .. } = &expression.kind {
                    units += statements.len();
                }
            }
            if let Some(child) = hir::lexical_exit::child(expression, next) {
                pending.push((expression, next + 1));
                pending.push((child, 0));
            }
        }
    }
    units
}

#[test]
fn census_charges_cursor_initialization_before_the_walk_with_exact_one_short_and_zero_work() {
    let program = crate::check(&canonical(&source(BODY)), PATH).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "calculator.app.main")
        .unwrap();
    let signature = function.params.len() + 1;
    let expected = census_reference_units(function);
    let run = |budget: usize| {
        let mut charges = Vec::new();
        let mut left = budget;
        let answer = census::function(function, &mut |units| {
            charges.push(units);
            left = left.checked_sub(units).ok_or(())?;
            Ok::<(), ()>(())
        });
        (answer, charges, left)
    };
    let (answer, charges, left) = run(expected);
    assert_eq!(
        answer,
        Ok(Some(census::Flags {
            lexical: true,
            usize: false,
            body_loop: true,
        }))
    );
    assert_eq!(left, 0, "the reference is the exact census work");
    // Signature, then the slot initialization, then the first walk step.
    assert_eq!(&charges[..3], &[signature, census::DEPTH, 1]);
    let (answer, charges, _) = run(expected - 1);
    assert_eq!(answer, Err(()));
    // The same debit sequence refuses at its final charge, never earlier.
    assert_eq!(charges.iter().sum::<usize>(), expected);
    // One short of the initialization refuses before the cursor exists.
    let (answer, charges, _) = run(signature + census::DEPTH - 1);
    assert_eq!((answer, charges), (Err(()), vec![signature, census::DEPTH]));
    let (answer, charges, _) = run(0);
    assert_eq!((answer, charges), (Err(()), vec![signature]));
}

#[test]
fn capture_selection_is_metered_and_unaffected_modules_keep_exact_builder_usage() {
    let text = canonical(&source(BODY));
    let program = crate::check(&text, PATH).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let authored = crate::workspace_graph::index_authored(std::slice::from_ref(&program)).unwrap();
    let sources = sources_with(&text, &[]);
    let start = crate::cache_codec::MAX_NODES;
    let mut left = start;
    assert!(
        capture::capture_metered(&program, &resolved, &authored, &sources, &mut left)
            .unwrap()
            .is_some()
    );
    let spent = start - left;
    let mut exact = spent;
    assert!(
        capture::capture_metered(&program, &resolved, &authored, &sources, &mut exact)
            .unwrap()
            .is_some()
    );
    assert_eq!(exact, 0, "capture work is exactly the successful debit sum");
    // One short fails the final member digest debit: not captured, no error.
    let mut short = spent - 1;
    assert!(matches!(
        capture::capture_metered(&program, &resolved, &authored, &sources, &mut short),
        Ok(None)
    ));
    // Zero work declines at the first selection debit, before any builder byte.
    let (answer, overflowed, used) =
        crate::bounded_output::with_limit_usage(MAX_CHANGE_BUILDER_BYTES, || {
            capture::capture_metered(&program, &resolved, &authored, &sources, &mut 0)
        });
    assert!(matches!(answer, Ok(None)) && !overflowed);
    assert_eq!(used, 0);
    // An exit-free closed module pays exactly the one reserved census cursor.
    let tests = crate::check(TESTS, TESTS_PATH).unwrap();
    let tests_resolved = hir::resolve(&tests).unwrap();
    let tests_authored =
        crate::workspace_graph::index_authored(std::slice::from_ref(&tests)).unwrap();
    let mut work = start;
    let (answer, overflowed, used) =
        crate::bounded_output::with_limit_usage(MAX_CHANGE_BUILDER_BYTES, || {
            capture::capture_metered(
                &tests,
                &tests_resolved,
                &tests_authored,
                &sources,
                &mut work,
            )
        });
    assert!(matches!(answer, Ok(None)) && !overflowed);
    assert_eq!(used, census::cursor_bytes());
}

#[test]
fn capture_capacity_excess_is_debited_after_the_fact_and_stays_refused() {
    let member = std::mem::size_of::<capture::Member>();
    let excess = |limit: usize, actual: usize| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            let report = capture::refuse_excess::<capture::Member>(actual, 3);
            // No later zero or fitting request can restore admission.
            let restored = crate::bounded_output::reserve_active(0);
            (report[0].code, restored)
        })
    };
    // Admitted actual excess is debited exactly, then the mismatch poisons.
    let ((code, restored), overflowed, used, first) = excess(2 * member + 7, 5);
    assert_eq!((code, restored, overflowed), ("SPX-G171", false, true));
    assert_eq!(used, 2 * member);
    assert_eq!(first.unwrap().requested, usize::MAX);
    // Unpayable actual excess is the sticky first refusal and debits nothing.
    let ((code, restored), overflowed, used, first) = excess(2 * member - 1, 5);
    assert_eq!((code, restored, overflowed), ("SPX-G171", false, true));
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 2 * member);
    // A short allocation has no excess to debit and still poisons.
    let ((code, restored), overflowed, used, first) = excess(MAX_CHANGE_BUILDER_BYTES, 2);
    assert_eq!(
        (code, restored, overflowed, used),
        ("SPX-G171", false, true, 0)
    );
    assert_eq!(first.unwrap().requested, usize::MAX);
}

#[test]
fn source_less_change_view_keeps_the_original_refusal() {
    let (build, _) = build(&canonical(&source(BODY)));
    match build.into_change_view() {
        Err(errors) => assert_eq!(errors[0].code, "SPX-G410"),
        Ok(_) => panic!("source-less change view must refuse"),
    }
}

/// One observation for reception and the whole lifetime of what it returns.
/// `inspect` receives the owned answer and the live metrics at return; it
/// inspects and drops every receipt, report and ledger owner inside the
/// observed scope and hands back Copy scalar facts only.
fn receive_observed<F: Copy>(
    build: &WorkspaceGraphBuild,
    sources: &dyn ReceivingSources,
    limit: usize,
    inspect: impl FnOnce(Result<Option<&'static str>, SelectedReplayRefusal>, Metrics) -> F,
) -> (F, Metrics, bool, usize) {
    let ((facts, metrics), overflowed, used) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let answer = received(build, PATH, sources);
                inspect(answer, current_metrics())
            })
        });
    (facts, metrics, overflowed, used)
}

#[test]
fn zero_and_report_one_short_return_the_unpaid_ledger_without_allocating() {
    let (build, sources) = build(&canonical(&source(BODY)));
    let slot = std::mem::size_of::<Diagnostic>();
    for (limit, charged) in [(0, 0), (slot - 1, 0), (REPORT.paid_bytes() - 1, slot)] {
        let ((), metrics, overflowed, used) =
            receive_observed(&build, &sources, limit, |answer, returned| {
                let Err(mut refusal) = answer else {
                    panic!("a one-short enclosing budget cannot fund the report carrier")
                };
                assert!(!returned.invalid);
                assert_eq!(
                    (returned.allocations, returned.allocated, returned.live),
                    (0, 0, 0)
                );
                assert!(!refusal.is_paid());
                assert_eq!(
                    first_receipt(&mut refusal),
                    ReplayPhysicalRefusal::EnclosingBudget
                );
                // The typed boundary consumes the refusal and its ledger here.
                let unpaid = refusal.into_published().expect_err("unpaid carrier");
                assert_eq!(current_metrics().allocations, returned.allocations);
                assert!(matches!(
                    unpaid.reason(),
                    SelectedReplayUnpaidReason::Carrier(_)
                ));
                // Only the named unfunded boundary allocates the same literal.
                let literal = unpaid.into_unfunded();
                assert_public(&literal);
                drop(literal);
            });
        assert!(!metrics.invalid);
        assert_eq!(
            metrics.live, 0,
            "ledger and unfunded literal dropped in scope"
        );
        assert!(overflowed);
        assert_eq!(used, charged);
    }
}

#[test]
fn exact_success_one_short_refusal_and_drop_share_one_observed_owner() {
    let (build, sources) = build(&canonical(&source(BODY)));
    // Exact report payment: paid setup refusal on the original carrier.
    let paid = REPORT.paid_bytes();
    let (cause, metrics, overflowed, used) =
        receive_observed(&build, &sources, paid, |answer, _| {
            let Err(refusal) = answer else {
                panic!("an exact report payment leaves no preparation backing")
            };
            assert!(refusal.is_paid());
            let cause = refusal.refused().map(|r| r.cause());
            drop(refusal);
            cause
        });
    assert!(overflowed && used == paid);
    assert_eq!(cause, Some(SelectedReplayCause::Setup));
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0, "the paid setup refusal dropped in scope");
    // Successful receipt: acquisition, setup, authentication, Session, every
    // member, finish, the returned receipt and Drop in one observed scope.
    let (selected, metrics, overflowed, used) =
        receive_observed(&build, &sources, MAX_CHANGE_BUILDER_BYTES, |answer, _| {
            matches!(answer, Ok(Some(_)))
        });
    assert!(selected && !overflowed);
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0, "every owner and the receipt dropped");
    assert!(
        metrics.allocated <= used,
        "every allocation was charged first"
    );
    // One byte short of the successful route refuses with the prepaid receipt;
    // the report, receipt and ledger are inspected and dropped in scope.
    let (cause, metrics, overflowed, _) =
        receive_observed(&build, &sources, used - 1, |answer, _| {
            let Err(refusal) = answer else {
                panic!("one byte short of the successful route must refuse")
            };
            assert!(refusal.is_paid());
            assert_public(refusal.report().unwrap());
            assert!(
                refusal.receipt().is_some() || refusal.fact_receipt().is_some(),
                "the first paid receipt stays with the failed owner"
            );
            let cause = refusal.refused().unwrap().cause();
            drop(refusal);
            cause
        });
    assert!(overflowed);
    assert!(
        matches!(
            cause,
            SelectedReplayCause::Budget | SelectedReplayCause::Session
        ),
        "{cause:?}"
    );
    assert!(!metrics.invalid);
    assert_eq!(
        metrics.live, 0,
        "the paid refusal and its ledger dropped in scope"
    );
}
