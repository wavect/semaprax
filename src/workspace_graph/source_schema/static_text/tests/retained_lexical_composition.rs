//! OPT-745 retained static UTF-8 compositions with String, builtin Vec and
//! borrowed-record lexical renewal. Every positive case is an actual immutable
//! retained Workspace/Project build replayed against the direct full-Program
//! oracle; every hostile case first requires that same unmodified admission.
//! These definitions are authored; grouped executable qualification is pending.
use super::*;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::{CleanupTransition, SelectedReplayPreparation, SelectedReplaySession};
use crate::interpreter::ResolvedEvaluationOutcome;

/// Same-owner String append plus whole String replacement, both with authored
/// loop exits. `compose` returns from inside its loop; `replace` continues.
const STRING_SOURCE: &str = r#"module static.writer;
@id("static.compose") fn compose(limit:i64)->i64 {
 let key=str"é"; let mut text="a"; let mut at=0;
 while at<limit {
  at=at+1;
  if at==1 {continue;};
  text=string_concat(text,"b");
  if at==3 {return string_len(text)+str_len_bytes(key);};
  0
 }
 string_len(text)
}
@id("static.replace") fn replace(flag:bool)->i64 {
 let key=str"x"; let mut text="initial"; let mut at=0;
 while at<3 {
  text=if flag {"new"} else {"other!"};
  at=at+1;
  if at==2 {continue;};
  0
 }
 string_len(text)+str_len_bytes(key)
}
@id("static.run") fn run()->i64 {compose(4)+replace(true)}
@id("app.main") fn main()->i64 {run()}
"#;

/// Exact builtin `Vec<string>` and flat owned-leaf record Vec renewal.
const VEC_SOURCE: &str = r#"module static.writer;
@id("static.row") record Row { @id("static.row.n") n:i64, @id("static.row.text") text:string, }
@id("static.collect") fn collect(limit:i64)->i64 {
 let key=str"é";
 let mut words=vec_with_capacity<string>(4usize);
 let mut rows=vec_with_capacity<Row>(4usize);
 let mut at=0;
 while at<limit {
  at=at+1;
  if at==2 {continue;};
  words=vec_push<string>(words,"w");
  rows=vec_push<Row>(rows,Row{n:at,text:"r"});
  if at==3 {return i64_from_usize(vec_len<Row>(rows))+str_len_bytes(key);};
  0
 }
 i64_from_usize(vec_len<string>(words))
}
@id("app.main") fn main()->i64 {collect(4)}
"#;

/// Source `for borrow` over a borrowed flat record Vec with Continue/Return.
/// A String field selected from the row is an iteration-local `borrow str`.
const SOURCE_FOR: &str = r#"module static.writer;
@id("static.row") record Row { @id("static.row.n") n:i64, @id("static.row.text") text:string, }
@id("static.weight") fn weight(n:i64)->i64 {n+1}
@id("static.scan") fn scan(rows:borrow Vec<Row>)->i64 {
 let key=str"é"; let mut seen=0;
 for borrow row at index in rows {
  let text=row.text;
  seen=seen+1;
  if index==0usize {continue;};
  if str_len_bytes(text)==str_len_bytes(key) {return weight(row.n);};
  0
 }
 seen
}
@id("app.main") fn main()->i64 {
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:3,text:"é"});
 let rows=vec_push<Row>(first,Row{n:7,text:"é"});
 scan(rows)
}
"#;

/// The same borrowed-record traversal with its record and scalar helper
/// supplied by the actual retained provider module through typed imports.
const PROVIDER: &str = r#"module static.rows;
@id("rows.row") record Row { @id("rows.row.n") n:i64, @id("rows.row.text") text:string, }
@id("rows.weight") fn weight(n:i64)->i64 {n+1}
"#;
const IMPORTED: &str = r#"module static.writer;
use type @id("rows.row") from static.rows as Row;
use function @id("rows.weight") from static.rows as weight;
@id("static.scan") fn scan(rows:borrow Vec<Row>)->i64 {
 let key=str"é"; let mut seen=0;
 for borrow row at index in rows {
  let text=row.text;
  seen=seen+1;
  if index==0usize {continue;};
  if str_len_bytes(text)==str_len_bytes(key) {return weight(row.n);};
  0
 }
 seen
}
@id("app.main") fn main()->i64 {
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:3,text:"é"});
 let rows=vec_push<Row>(first,Row{n:7,text:"é"});
 scan(rows)
}
"#;

const WIDE_WORK: usize = 32_000_000;
const WIDE_BYTES: usize = 1 << 28;

fn try_build_imported(
    provider: &str,
    consumer: &str,
) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    let sources = [("rows.spx", provider), ("writer.spx", consumer)]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&crate::parse(source, path).unwrap()),
        })
        .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES).map(|built| built.0)
}
fn build_imported(provider: &str, consumer: &str) -> WorkspaceGraphBuild {
    try_build_imported(provider, consumer).unwrap()
}

/// Direct full-Program oracle: canonical round trip, graph replay and the
/// complete independent HIR validation of every lexical function.
fn direct(source: &str) -> hir::ResolvedProgram {
    let ast = crate::check(source, "writer.spx").unwrap();
    let canonical = format::canonical(&ast);
    let current = crate::check(&canonical, "writer.spx").unwrap();
    assert_eq!(format::canonical(&current), canonical);
    let graph = graph::to_json(&current).unwrap();
    graph::verify_json(&current, &graph).unwrap();
    let resolved = hir::resolve(&current).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}

/// An actual certificate, not an acyclic guard-return normalization.
fn assert_lexical(function: &hir::ResolvedFunction, kinds: &[LexicalExitKind]) {
    assert!(
        hir::lexical_exit::requires(function).unwrap(),
        "{}",
        function.id
    );
    assert_eq!(
        function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
    );
    let control = function
        .cleanup_plan
        .lexical_control
        .as_ref()
        .expect("authored exits retain an actual lexical certificate");
    assert!(!control.loops.is_empty());
    for kind in kinds {
        assert!(
            control.exits.iter().any(|exit| exit.kind == *kind),
            "{}: {kind:?}",
            function.id
        );
    }
    // Every authored exit names this function or one of its own certified loops.
    let owner = hir::FunctionExecutionId::Monomorphic(function.id.clone());
    for exit in &control.exits {
        if exit.kind == LexicalExitKind::Return {
            assert_eq!(exit.target, hir::ControlTarget::Function(owner.clone()));
        } else {
            assert!(
                control.loops.iter().any(|certificate| certificate.target == exit.target
                    && matches!(&certificate.target, hir::ControlTarget::Loop { owner: loop_owner, .. }
                        if *loop_owner == owner)),
                "{}: {:?}",
                function.id,
                exit.kind
            );
        }
    }
}

fn function<'a>(module: &'a WorkspaceResolvedModule, id: &str) -> &'a hir::ResolvedFunction {
    module
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap()
}
fn function_mut<'a>(
    module: &'a mut WorkspaceResolvedModule,
    id: &str,
) -> &'a mut hir::ResolvedFunction {
    module
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == id)
        .unwrap()
}

/// Retained HIR and plans equal the direct program's for every held function.
fn plan_parity(module: &WorkspaceResolvedModule, direct: &hir::ResolvedProgram) {
    for retained in &module.functions {
        let control = direct
            .functions
            .iter()
            .find(|f| f.id == retained.id)
            .unwrap();
        assert_eq!(retained.body, control.body, "{}", retained.id);
        assert_eq!(
            retained.cleanup_plan, control.cleanup_plan,
            "{}",
            retained.id
        );
    }
}

fn admitted(build: &WorkspaceGraphBuild) {
    assert_eq!(
        schema(writer(build), &build.hir.declarations).unwrap(),
        Some(crate::graph::STATIC_TEXT_GRAPH_SCHEMA),
        "exact literal-site authentication precedes composed lexical replay"
    );
    let module = writer(build);
    let expected = if module
        .functions
        .iter()
        .chain(
            module
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            std::iter::once(&function.body)
                .chain(&function.requires)
                .chain(&function.ensures)
                .any(crate::vec_field::expression_uses)
        }) {
        crate::graph::VEC_FIELD_GRAPH_SCHEMA
    } else {
        crate::graph::STATIC_TEXT_GRAPH_SCHEMA
    };
    assert_eq!(
        build.source_graph_schemas().unwrap()["writer.spx"],
        expected
    );
}

fn evaluate(program: &hir::ResolvedProgram) -> ResolvedEvaluationOutcome {
    crate::interpreter::evaluate_resolved_zero_arg_i64(program, "app.main", 100_000)
        .unwrap()
        .outcome
}

/// The linked retained program and the direct program execute identically
/// on repeated entry; `None` expects the sticky arithmetic failure.
fn runtime_parity(
    build: &WorkspaceGraphBuild,
    direct: &hir::ResolvedProgram,
    expected: Option<i64>,
) {
    let linked = build
        .linked_owned_data_api_program_with_roots("static.writer", &[])
        .unwrap();
    for _ in 0..3 {
        let actual = evaluate(&linked);
        assert_eq!(actual, evaluate(direct));
        match expected {
            Some(value) => assert_eq!(actual, ResolvedEvaluationOutcome::ReturnedI64(value)),
            None => assert!(
                matches!(&actual, ResolvedEvaluationOutcome::LanguageFailure(status)
                    if status.domain_id() == "semaprax.arithmetic.v1"
                        && status.code() == crate::cleanup_plan::StatusCase::DivisionByZero.code()),
                "{actual:?}"
            ),
        }
    }
}

type Replayed = (Vec<Result<(), Diagnostic>>, (usize, usize), bool);

/// One held selected session validates every member against one cumulative
/// work and backing ledger, then finishes (or refuses) and drops it.
fn replay(
    build: &WorkspaceGraphBuild,
    members: &[&hir::ResolvedFunction],
    work: usize,
    bytes: usize,
) -> Result<Replayed, Vec<Diagnostic>> {
    let anchor = members[0];
    let preparation =
        SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|e| vec![e])?;
    super::super::primitive::with_prepared_test_context(
        writer(build),
        &build.hir.modules,
        &build.hir.declarations,
        preparation,
        |context, preparation| -> Result<Replayed, Diagnostic> {
            let mut session =
                SelectedReplaySession::from_preparation(context, anchor, preparation)?;
            let mut results = Vec::new();
            for member in members {
                results.push(session.validate(member));
            }
            let metrics = session.metrics();
            if results.iter().any(Result::is_err) {
                // The refusal is sticky: no later member restarts the spent ledger.
                let again = session.validate(anchor).unwrap_err();
                assert_eq!(again.code, "SPX-H006");
                assert!(again.message.contains("already failed"), "{again:?}");
                assert_eq!(
                    session.metrics(),
                    metrics,
                    "a refused retry charges nothing"
                );
            }
            let finished = session.finish().is_ok();
            Ok((results, metrics, finished))
        },
    )?
    .map_err(|e| vec![e])
}

fn lexical_members(module: &WorkspaceResolvedModule) -> Vec<&hir::ResolvedFunction> {
    module
        .functions
        .iter()
        .filter(|f| hir::lexical_exit::requires(f).unwrap())
        .collect()
}

fn loop_statements_mut(function: &mut hir::ResolvedFunction) -> &mut Vec<hir::ResolvedStatement> {
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("source block")
    };
    let body = statements
        .iter_mut()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::LexicalWhile { body, .. } => Some(body),
            _ => None,
        })
        .expect("authored while");
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop block")
    };
    statements
}

fn drop_reservations(function: &mut hir::ResolvedFunction) {
    let reserved = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|t| matches!(t, CleanupTransition::ReserveRenewal { .. }))
        .count();
    assert!(reserved > 0, "{} renews an actual owner", function.id);
    for block in &mut function.cleanup_plan.blocks {
        block
            .transitions
            .retain(|t| !matches!(t, CleanupTransition::ReserveRenewal { .. }));
    }
}

fn forge_instance(function: &mut hir::ResolvedFunction) {
    let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("source main block")
    };
    let hir::ResolvedExprKind::Call {
        callee, instance, ..
    } = &mut tail.kind
    else {
        panic!("source call")
    };
    assert!(instance.is_none());
    *instance = Some(hir::FunctionInstanceId::derive(
        callee,
        &[hir::ResolvedType::I64],
    ));
}

fn record_fields_mut<'a>(
    module: &'a mut WorkspaceResolvedModule,
    id: &str,
) -> &'a mut Vec<hir::ResolvedFieldDeclaration> {
    let record = module
        .types
        .iter_mut()
        .find(|record| record.id.as_str() == id)
        .unwrap();
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
        panic!("record")
    };
    crate::workspace_graph::source_schema::materialize_host_fields(fields)
}

#[test]
fn retained_static_string_append_and_replacement_compose_with_direct_parity() {
    let control = direct(STRING_SOURCE);
    for id in ["static.compose", "static.replace"] {
        let function = control
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        let kinds: &[LexicalExitKind] = if id == "static.compose" {
            &[LexicalExitKind::Continue, LexicalExitKind::Return]
        } else {
            &[LexicalExitKind::Continue]
        };
        assert_lexical(function, kinds);
    }
    let retained = build(STRING_SOURCE);
    admitted(&retained);
    let module = writer(&retained);
    assert!(
        module.static_text.as_ref().unwrap().lexical.is_some(),
        "authenticated primitive source inventory is captured"
    );
    plan_parity(module, &control);
    for id in ["static.compose", "static.replace"] {
        assert_lexical(function(module, id), &[LexicalExitKind::Continue]);
    }
    // The frozen generic reader still cannot select this composition itself.
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    // compose(4)=3+2 after one Continue and the in-loop Return; replace=3+1.
    runtime_parity(&retained, &control, Some(9));
    // A division by zero inside the Return operand follows the String renewal
    // and still reads the static literal; `replace` (right operand) never runs.
    let failing = STRING_SOURCE.replace(
        "{return string_len(text)+str_len_bytes(key);}",
        "{return (string_len(text)+str_len_bytes(key))/(at-3);}",
    );
    assert_ne!(failing, STRING_SOURCE);
    let failing_control = direct(&failing);
    let failing_retained = build(&failing);
    admitted(&failing_retained);
    runtime_parity(&failing_retained, &failing_control, None);
}

#[test]
fn retained_string_composition_rejects_certificate_schema_renewal_and_signature_drift() {
    for attack in 0..8 {
        let mut retained = build(STRING_SOURCE);
        admitted(&retained);
        let module = writer_mut(&mut retained);
        match attack {
            0 => {
                let function = function_mut(module, "static.compose");
                let control = function.cleanup_plan.lexical_control.as_mut().unwrap();
                let exit = control
                    .exits
                    .iter_mut()
                    .find(|exit| exit.kind == LexicalExitKind::Return)
                    .expect("in-loop Return certificate");
                exit.kind = LexicalExitKind::Continue;
            }
            1 => {
                let function = function_mut(module, "static.compose");
                let control = function.cleanup_plan.lexical_control.as_mut().unwrap();
                let certificate = &mut control.loops[0];
                assert_eq!(certificate.latch, hir::LoopLatchKind::Condition);
                certificate.latch = hir::LoopLatchKind::SourceForStep {
                    kind: hir::lexical_exit::SourceForKind::Copy,
                    step: certificate.condition.clone(),
                };
            }
            2 => {
                let plan = &mut function_mut(module, "static.replace").cleanup_plan;
                assert_eq!(plan.schema, crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18);
                plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16
            }
            3 => drop_reservations(function_mut(module, "static.replace")),
            4 => {
                function_mut(module, "static.compose")
                    .cleanup_plan
                    .status_sources
                    .pop()
                    .expect("authored status producers");
            }
            5 => forge_instance(function_mut(module, "app.main")),
            6 => function_mut(module, "static.compose").params[0]
                .name
                .push('x'),
            _ => module
                .static_text
                .as_mut()
                .unwrap()
                .lexical
                .as_mut()
                .unwrap()
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "static.replace")
                .unwrap()
                .name
                .push('x'),
        }
        refused(&retained);
    }
}

#[test]
fn two_function_selected_session_is_cumulative_exact_and_one_short_through_drop() {
    let retained = build(STRING_SOURCE);
    admitted(&retained);
    let module = writer(&retained);
    let members = lexical_members(module);
    assert_eq!(
        members.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
        ["static.compose", "static.replace"]
    );
    with_test_context(module, &retained.hir.declarations, |context| {
        for member in &members {
            // Whole selected entry-to-finish on the real held-source context.
            crate::cleanup_plan::validate_retained_lexical_with_reservation(
                context,
                member,
                crate::cleanup_plan::ReplayPhysicalReservation::new(usize::MAX),
            )
            .unwrap();
        }
        let (results, (remaining, charged), finished) =
            replay(&retained, &members, WIDE_WORK, WIDE_BYTES).unwrap();
        assert!(results.iter().all(Result::is_ok) && finished);
        let spent = WIDE_WORK - remaining;
        let (first, (first_remaining, first_charged), _) =
            replay(&retained, &members[..1], WIDE_WORK, WIDE_BYTES).unwrap();
        assert!(first.iter().all(Result::is_ok));
        // The second member continues the first member's ledger.
        assert!(WIDE_WORK - first_remaining < spent);
        assert!(first_charged < charged);
        let (exact, metrics, finished) = replay(&retained, &members, spent, charged).unwrap();
        assert!(exact.iter().all(Result::is_ok) && finished);
        assert_eq!(metrics, (0, charged));
        // One unit short: the first member still fits, the final debit belongs
        // to the second member, and the refusal survives into finish/Drop.
        for (work, bytes) in [(spent - 1, charged), (spent, charged - 1)] {
            let (results, _, finished) = replay(&retained, &members, work, bytes).unwrap();
            assert!(results[0].is_ok(), "{work}/{bytes}");
            assert_eq!(results[1].as_ref().unwrap_err().code, "SPX-H006");
            assert!(!finished);
        }
    })
    .unwrap();
}

#[test]
fn retained_builtin_string_vec_and_flat_record_renewal_compose_with_direct_parity() {
    let control = direct(VEC_SOURCE);
    let collect = control
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.collect")
        .unwrap();
    assert_lexical(
        collect,
        &[LexicalExitKind::Continue, LexicalExitKind::Return],
    );
    let retained = build(VEC_SOURCE);
    admitted(&retained);
    let module = writer(&retained);
    let records = module
        .static_text
        .as_ref()
        .unwrap()
        .lexical
        .as_ref()
        .expect("authenticated primitive source inventory is captured")
        .records
        .as_ref()
        .expect("flat owned-leaf record layout is captured");
    assert_eq!(records.records.len(), 1);
    plan_parity(module, &control);
    // collect(4): one push at 1, Continue at 2, two Row values at 3 + "é".
    runtime_parity(&retained, &control, Some(4));
}

#[test]
fn retained_vec_composition_rejects_element_layout_finalizer_and_renewal_drift() {
    for attack in 0..8 {
        let mut retained = build(VEC_SOURCE);
        admitted(&retained);
        let module = writer_mut(&mut retained);
        match attack {
            0 | 1 => {
                let statements = loop_statements_mut(function_mut(module, "static.collect"));
                let (callee, type_arguments) = statements
                    .iter_mut()
                    .find_map(|statement| match statement {
                        hir::ResolvedStatement::Assign { value, .. } => match &mut value.kind {
                            hir::ResolvedExprKind::Call {
                                callee,
                                type_arguments,
                                instance: None,
                                ..
                            } if callee.as_str() == crate::vec_ops::PUSH_ID
                                && type_arguments.as_slice() == [hir::ResolvedType::String] =>
                            {
                                Some((callee, type_arguments))
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .expect("builtin Vec<string> same-owner push");
                if attack == 0 {
                    *type_arguments = vec![hir::ResolvedType::Bytes];
                } else {
                    // An actual local function identity is still not the builtin.
                    *callee = hir::DeclarationId::new("static.collect");
                }
            }
            2 => record_fields_mut(module, "static.row").swap(0, 1),
            3 => {
                let field = &mut record_fields_mut(module, "static.row")[1];
                assert_eq!(field.ty, hir::ResolvedType::String);
                field.ty = hir::ResolvedType::Bytes
            }
            4 => {
                let plan = &mut function_mut(module, "static.collect").cleanup_plan;
                let control = plan.lexical_control.as_ref().unwrap();
                let certificate = control
                    .exits
                    .iter()
                    .find(|exit| exit.kind == LexicalExitKind::Return)
                    .expect("in-loop Return certificate");
                let return_exit = plan
                    .exits
                    .iter()
                    .find(|exit| exit.id == certificate.exit)
                    .expect("certified Return exit target");
                assert!(
                    matches!(
                        return_exit.continuation,
                        crate::cleanup_plan::ExitContinuation::FunctionReturn { .. }
                    ),
                    "original Return continuation remains authenticated"
                );
                let resume = control
                    .resumes
                    .iter()
                    .find(|resume| Some(resume.entry) == certificate.resume)
                    .expect("exact certified Return resume");
                // The nested Return contour leaves inner regions. Its held
                // root owners settle only after resumed postconditions.
                let terminal = plan
                    .exits
                    .iter()
                    .filter(|exit| {
                        resume.blocks.contains(&exit.from)
                            && matches!(
                                exit.continuation,
                                crate::cleanup_plan::ExitContinuation::CommitResult { .. }
                            )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    terminal.len(),
                    1,
                    "one current resumed successful publication"
                );
                let terminal_id = terminal[0].id;
                let exit = plan
                    .exits
                    .iter_mut()
                    .find(|exit| exit.id == terminal_id)
                    .unwrap();
                assert!(
                    exit.finalize_in_order.len() >= 2
                        && exit.finalize_in_order[0] != exit.finalize_in_order[1],
                    "the certified Return resume settles both outer owners"
                );
                exit.finalize_in_order.swap(0, 1);
            }
            5 => drop_reservations(function_mut(module, "static.collect")),
            6 => {
                let plan = &mut function_mut(module, "static.collect").cleanup_plan;
                assert_eq!(plan.schema, crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18);
                plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15
            }
            _ => {
                let field = retained
                    .hir
                    .declarations
                    .get_mut("static.row.text")
                    .unwrap();
                assert_eq!(field.owner.as_deref(), Some("static.row"));
                field.owner = None
            }
        }
        refused(&retained);
    }
}

#[test]
fn retained_borrowed_record_source_for_continue_and_return_has_direct_parity() {
    let control = direct(SOURCE_FOR);
    let scan = control
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.scan")
        .unwrap();
    assert_lexical(scan, &[LexicalExitKind::Continue, LexicalExitKind::Return]);
    assert!(scan
        .cleanup_plan
        .lexical_control
        .as_ref()
        .unwrap()
        .loops
        .iter()
        .any(|certificate| matches!(
            certificate.latch,
            hir::LoopLatchKind::SourceForStep {
                kind: hir::lexical_exit::SourceForKind::BorrowRecord,
                ..
            }
        )));
    let retained = build(SOURCE_FOR);
    admitted(&retained);
    plan_parity(writer(&retained), &control);
    // Index 0 continues; index 1 matches "é" and returns weight(7).
    runtime_parity(&retained, &control, Some(8));
}

#[test]
fn retained_source_for_rejects_latch_target_and_field_namespace_drift() {
    for attack in 0..5 {
        let mut retained = build(SOURCE_FOR);
        admitted(&retained);
        let module = writer_mut(&mut retained);
        match attack {
            0 | 1 => {
                let function = function_mut(module, "static.scan");
                let control = function.cleanup_plan.lexical_control.as_mut().unwrap();
                let certificate = control
                    .loops
                    .iter_mut()
                    .find(|certificate| {
                        matches!(
                            certificate.latch,
                            hir::LoopLatchKind::SourceForStep {
                                kind: hir::lexical_exit::SourceForKind::BorrowRecord,
                                ..
                            }
                        )
                    })
                    .expect("borrowed record source-for latch");
                if attack == 0 {
                    certificate.latch = hir::LoopLatchKind::Condition;
                } else if let hir::LoopLatchKind::SourceForStep { kind, .. } =
                    &mut certificate.latch
                {
                    *kind = hir::lexical_exit::SourceForKind::Copy;
                }
            }
            2 => {
                let function = function_mut(module, "static.scan");
                let target = hir::ControlTarget::Function(hir::FunctionExecutionId::Monomorphic(
                    function.id.clone(),
                ));
                let control = function.cleanup_plan.lexical_control.as_mut().unwrap();
                let exit = control
                    .exits
                    .iter_mut()
                    .find(|exit| exit.kind == LexicalExitKind::Continue)
                    .expect("source-for Continue certificate");
                assert_ne!(exit.target, target);
                exit.target = target;
            }
            3 => forge_instance(function_mut(module, "app.main")),
            _ => {
                let field = retained
                    .hir
                    .declarations
                    .get_mut("static.row.text")
                    .unwrap();
                assert_eq!(field.owner.as_deref(), Some("static.row"));
                field.owner = None
            }
        }
        refused(&retained);
    }
}

#[test]
fn retained_imported_record_and_helper_use_the_actual_typed_namespace() {
    let retained = build_imported(PROVIDER, IMPORTED);
    admitted(&retained);
    let provider = retained
        .hir
        .modules
        .iter()
        .find(|module| module.module == "static.rows")
        .unwrap();
    assert!(provider.static_text.is_none());
    for (id, kind) in [
        ("rows.row", hir::DeclarationKind::Record),
        ("rows.weight", hir::DeclarationKind::Function),
    ] {
        let fact = &retained.hir.declarations[id];
        assert!(
            fact.kind == kind
                && fact.owner.is_none()
                && fact.path.as_deref() == Some("rows.spx")
                && fact.module.as_deref() == Some("static.rows"),
            "{id} is defined by the held provider source"
        );
    }
    let scan = function(writer(&retained), "static.scan");
    assert_lexical(scan, &[LexicalExitKind::Continue, LexicalExitKind::Return]);
    // The saved all-local control declares the provider's exact stable
    // identities; the same-spelling SOURCE_FOR control is a distinct type.
    let local = SOURCE_FOR
        .replace("@id(\"static.row", "@id(\"rows.row")
        .replace("@id(\"static.weight\")", "@id(\"rows.weight\")");
    assert!(!local.contains("static.row") && !local.contains("static.weight"));
    let control = direct(&local);
    let imported_rows = crate::vec_ops::resolved_vec(hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("rows.row"),
        arguments: Vec::new(),
    });
    assert_eq!(scan.params[0].ty, imported_rows);
    let local_scan = control
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.scan")
        .unwrap();
    assert_eq!(local_scan.params[0].ty, imported_rows);
    let spelled = direct(SOURCE_FOR);
    let spelled_scan = spelled
        .functions
        .iter()
        .find(|f| f.id.as_str() == "static.scan")
        .unwrap();
    assert_ne!(spelled_scan.params[0].ty, imported_rows);
    runtime_parity(&retained, &control, Some(8));

    // This independently exercises the new defining-record helper signature,
    // rather than only importing a record used by a primitive-parameter helper.
    let borrowed_provider = PROVIDER.replace(
        "fn weight(n:i64)->i64 {n+1}",
        "fn weight(row:borrow Row)->i64 {row.n+1}",
    );
    // The traversal row is source-only and cannot be passed (Scoped Record Vec
    // Traversal v2). Inside the unchanged matched branch the authored Return
    // lends a named owned probe whose `n` is read from the matched row itself.
    let matched = "if str_len_bytes(text)==str_len_bytes(key) {return weight(row.n);};";
    let probe = |source: &str| {
        assert_eq!(source.matches(matched).count(), 1);
        let lent = source.replace(
            matched,
            "if str_len_bytes(text)==str_len_bytes(key) \
             {let probe=Row{n:row.n,text:\"é\"}; return weight(probe);};",
        );
        assert!(!lent.contains("weight(row.n)") && lent.contains("return weight(probe);"));
        lent
    };
    let borrowed_consumer = probe(IMPORTED);
    let borrowed = build_imported(&borrowed_provider, &borrowed_consumer);
    admitted(&borrowed);
    let borrowed_local = probe(&local.replace(
        "fn weight(n:i64)->i64 {n+1}",
        "fn weight(row:borrow Row)->i64 {row.n+1}",
    ));
    let borrowed_control = direct(&borrowed_local);
    runtime_parity(&borrowed, &borrowed_control, Some(8));
    // Changing only the matched source row's `n` changes the helper result in
    // both the retained build and the direct oracle, so the probe is not a
    // constant.
    let reweighted = |source: &str| {
        let matched_row = "Row{n:7,text:\"é\"}";
        assert_eq!(source.matches(matched_row).count(), 1);
        source.replace(matched_row, "Row{n:40,text:\"é\"}")
    };
    let moved = build_imported(&borrowed_provider, &reweighted(&borrowed_consumer));
    admitted(&moved);
    runtime_parity(&moved, &direct(&reweighted(&borrowed_local)), Some(41));
    // Authentication and every helper/consumer replay transfer the same
    // preparation; this is distinct from the independent direct runtime oracle.
    let scan = function(writer(&borrowed), "static.scan");
    let preparation = SelectedReplayPreparation::with_limits(scan, WIDE_WORK, WIDE_BYTES).unwrap();
    super::super::primitive::with_prepared_test_context(
        writer(&borrowed),
        &borrowed.hir.modules,
        &borrowed.hir.declarations,
        preparation,
        |context, preparation| -> Result<(), Diagnostic> {
            let helper = (0..context.function_count())
                .filter_map(|i| context.function_at(i))
                .find(|f| f.id.as_str() == "rows.weight")
                .unwrap();
            let mut session = SelectedReplaySession::from_preparation(context, scan, preparation)?;
            session.validate_scalar(helper)?;
            session.validate(scan)?;
            // finish returns the held accounting owner, not unit. Drop it
            // inside this same callback after successful complete replay.
            session.finish().map(drop)
        },
    )
    .unwrap()
    .unwrap();
}

#[test]
fn retained_traversal_row_lends_to_an_imported_borrowed_scalar_reader() {
    // Row Loan v1: this exact original source was the former blanket refusal.
    // It is now the admitted call-only loan; the imported helper is proven from
    // the current provider body and executes identically to the direct oracle.
    let helper = "fn weight(row:borrow Row)->i64 {row.n+1}";
    let provider = PROVIDER.replace("fn weight(n:i64)->i64 {n+1}", helper);
    let consumer = IMPORTED.replace("weight(row.n)", "weight(row)");
    assert_ne!(consumer, IMPORTED);
    let retained = build_imported(&provider, &consumer);
    assert_eq!(
        retained.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v82"
    );
    let scan = function(writer(&retained), "static.scan");
    assert_lexical(scan, &[LexicalExitKind::Continue, LexicalExitKind::Return]);
    let local = SOURCE_FOR
        .replace("@id(\"static.row", "@id(\"rows.row")
        .replace("@id(\"static.weight\")", "@id(\"rows.weight\")")
        .replace("fn weight(n:i64)->i64 {n+1}", helper)
        .replace("weight(row.n)", "weight(row)");
    runtime_parity(&retained, &direct(&local), Some(8));
    // Imported helper bodies that escape, view or forward the row refuse, and
    // a provider edit after a successful build is re-proven, never retained.
    for escaping in [
        "fn keep(row:borrow Row)->i64 {row.n} @id(\"rows.weight\") fn weight(row:borrow Row)->i64 {keep(row)}",
        "fn weight(row:borrow Row)->i64 {str_len_bytes(string_as_str(row.text))}",
        "fn weight(row:borrow Row)->i64 {weight(row)}",
    ] {
        let escaping = if escaping.starts_with("fn keep") {
            PROVIDER.replace(
                "@id(\"rows.weight\") fn weight(n:i64)->i64 {n+1}",
                &format!("@id(\"rows.keep\") {escaping}"),
            )
        } else {
            PROVIDER.replace("fn weight(n:i64)->i64 {n+1}", escaping)
        };
        assert_ne!(escaping, PROVIDER);
        let errors = try_build_imported(&escaping, &consumer)
            .err()
            .expect("an escaping imported helper is not a row reader");
        assert!(
            errors.iter().any(|error| error.code == "SPX-T284"),
            "{escaping}: {errors:#?}"
        );
    }
}

#[test]
fn retained_traversal_row_cannot_be_owned_or_stored_through_imported_helpers() {
    // Transferring or storing the source-only row stays refused even when an
    // imported helper names the exact provider record type.
    for (helper, authored, consumer_use) in [
        (
            "fn weight(row:own Row)->i64 {row.n+1}",
            "weight(row.n)",
            "weight(row)",
        ),
        (
            "fn weight(n:i64)->i64 {n+1}",
            "let text=row.text;",
            "let text=row.text; let kept=row;",
        ),
    ] {
        let provider = PROVIDER.replace("fn weight(n:i64)->i64 {n+1}", helper);
        let consumer = IMPORTED.replace(authored, consumer_use);
        assert_ne!(consumer, IMPORTED);
        let errors = try_build_imported(&provider, &consumer)
            .err()
            .expect("retained build refuses the escaping row");
        assert!(
            errors.iter().any(|error| error.code == "SPX-T284"
                && error.message
                    == "record traversal row cannot escape or materialize aggregate storage; select a field"),
            "{consumer_use}: {errors:#?}"
        );
        let local = SOURCE_FOR
            .replace("fn weight(n:i64)->i64 {n+1}", helper)
            .replace(authored, consumer_use);
        let errors = crate::check(&local, "writer.spx")
            .err()
            .expect("direct source verification refuses the escaping row");
        assert!(
            errors.iter().any(|error| error.code == "SPX-T284"
                && error.message == "record traversal row cannot escape or materialize aggregate storage; select a field"),
            "{consumer_use}: {errors:#?}"
        );
    }
}

#[test]
fn retained_imported_provider_identity_layout_path_and_body_drift_fail_closed() {
    for attack in 0..8 {
        let mut retained = build_imported(PROVIDER, IMPORTED);
        admitted(&retained);
        let provider = retained
            .hir
            .modules
            .iter_mut()
            .find(|module| module.module == "static.rows")
            .unwrap();
        match attack {
            // Same spelling, different stable identity.
            0 => {
                let record = provider
                    .types
                    .iter_mut()
                    .find(|record| record.id.as_str() == "rows.row")
                    .unwrap();
                assert_eq!(record.name, "Row");
                record.id = hir::DeclarationId::new("rows.row.other");
            }
            1 => record_fields_mut(provider, "rows.row").swap(0, 1),
            2 => {
                let field = &mut record_fields_mut(provider, "rows.row")[1];
                assert_eq!(field.ty, hir::ResolvedType::String);
                field.ty = hir::ResolvedType::Bytes
            }
            3 => {
                let body = &mut function_mut(provider, "rows.weight").body;
                assert!(!matches!(body.kind, hir::ResolvedExprKind::Int(9)));
                body.kind = hir::ResolvedExprKind::Int(9)
            }
            4 => {
                retained.hir.declarations.get_mut("rows.row").unwrap().path =
                    Some("forged.spx".into())
            }
            5 => {
                let field = retained.hir.declarations.get_mut("rows.row.n").unwrap();
                assert_eq!(field.owner.as_deref(), Some("rows.row"));
                field.owner = Some("rows.weight".into())
            }
            // The consumer cannot claim the provider's defining source.
            6 => {
                retained
                    .hir
                    .declarations
                    .get_mut("rows.row")
                    .unwrap()
                    .module = Some("static.writer".into())
            }
            _ => forge_instance(function_mut(writer_mut(&mut retained), "app.main")),
        }
        refused(&retained);
    }
}

mod statement_marker_resources;
