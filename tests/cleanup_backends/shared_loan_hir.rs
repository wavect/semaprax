use std::path::Path;

use semaprax::hir;
use semaprax::hir::{ResolvedExprKind, ResolvedStatement};
use semaprax::loan_plan::{build_plan, LoanId, LoanPointPhase, LoanProgramPoint};
use semaprax::{parse, verify};

const CFG_SOURCE: &str = r#"
module test.shared_loan_hir_v1;

@id("bytes.take")
fn take(value: own Bytes) -> i64 { 1 }

@id("loan.packet")
record Packet {
    @id("loan.packet.left") left: Bytes,
    @id("loan.packet.right") right: Bytes,
}

@id("loan.projected-field")
fn projected_field(packet: own Packet) -> usize {
    let view = bytes_as_slice(packet.left);
    let alias = view;
    let range = byte_range(alias, 0usize, byte_len(alias));
    let moved = take(packet.right);
    byte_len(range)
}

@id("loan.paths")
fn paths(input: borrow Slice<u8>, outer: bool, inner: bool, selector: i64) -> i64 {
    let owned = bytes_copy(input);
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 0usize, byte_len(parent));
    let sibling = bytes_as_slice(owned);
    let observed = if outer {
        inner && match selector {
            n if n > 0 && byte_len(child) > 0usize => true,
            _ => byte_len(parent) > 0usize,
        }
    } else {
        byte_len(sibling) > 0usize
    };
    take(owned)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn fixture() -> hir::ResolvedProgram {
    let parsed = parse(CFG_SOURCE, Path::new("shared-loan-hir-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "unexpected source diagnostics: {diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}

fn paths_index(program: &hir::ResolvedProgram) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.paths")
        .unwrap()
}

fn projected_index(program: &hir::ResolvedProgram) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.projected-field")
        .unwrap()
}

fn reject_mutation(name: &str, mutate: impl FnOnce(&mut hir::ResolvedFunction)) {
    let mut program = fixture();
    let index = paths_index(&program);
    mutate(&mut program.functions[index]);
    let diagnostic = hir::validate(&program).expect_err(name);
    assert_eq!(
        diagnostic.code, "SPX-H006",
        "mutation {name}: {diagnostic:?}"
    );
}

fn source_line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rfind('\n')
        .map_or(offset + 1, |newline| offset - newline);
    (line, column)
}

fn swap_let_statements(function: &mut hir::ResolvedFunction, first: &str, second: &str) {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function body must be a block");
    };
    let position = |name: &str| {
        statements
            .iter()
            .position(|statement| matches!(statement,
                ResolvedStatement::Let { binding, .. } if binding.name == name))
            .unwrap_or_else(|| panic!("missing let binding {name}"))
    };
    let first = position(first);
    let second = position(second);
    statements.swap(first, second);
}

fn move_assignment_before_let(function: &mut hir::ResolvedFunction, let_name: &str) {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function body must be a block");
    };
    let let_index = statements
        .iter()
        .position(|statement| matches!(statement,
            ResolvedStatement::Let { binding, .. } if binding.name == let_name))
        .unwrap_or_else(|| panic!("missing let binding {let_name}"));
    let assign_index = statements
        .iter()
        .position(|statement| matches!(statement, ResolvedStatement::Assign { .. }))
        .expect("fixture must contain the owner assignment");
    statements.swap(let_index, assign_index);
}

fn move_assignment_before_let_without_span(
    function: &mut hir::ResolvedFunction,
    let_name: &str,
) {
    move_assignment_before_let(function, let_name);
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        unreachable!("function body remains a block");
    };
    let Some(ResolvedStatement::Assign { span, .. }) = statements
        .iter_mut()
        .find(|statement| matches!(statement, ResolvedStatement::Assign { .. }))
    else {
        unreachable!("fixture assignment remains after swapping");
    };
    *span = Default::default();
}

fn assert_overlap_span(
    source: &str,
    function_id: &str,
    operation_span: Option<(usize, usize)>,
    reorder: impl FnOnce(&mut hir::ResolvedFunction),
) {
    let parsed = parse(source, Path::new("shared-loan-overlap-span.spx")).unwrap();
    let source_diagnostics = verify::verify(&parsed);
    assert!(
        source_diagnostics.iter().all(|diagnostic| !diagnostic.severity.is_error()),
        "fixture source must be valid before hostile HIR reorder: {source_diagnostics:?}"
    );
    let mut program = hir::resolve(&parsed).unwrap();
    let function_index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == function_id)
        .unwrap();
    reorder(&mut program.functions[function_index]);

    let function = &program.functions[function_index];
    let diagnostic = build_plan(&program, function).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "move, mutation, or transfer overlaps an active shared loan"
    );
    if let Some((operation_start, operation_end)) = operation_span {
        let span = diagnostic.span.expect("overlap points to the source operation");
        let (line, column) = source_line_column(source, operation_start);
        assert_eq!(span.start, operation_start);
        assert_eq!(span.end, operation_end);
        assert_eq!((span.line, span.column), (line, column));
    } else {
        assert_eq!(diagnostic.span, None, "missing HIR span stays locationless");
    }

    let hostile = hir::validate(&program).expect_err("overlapping hostile HIR must remain rejected");
    assert_eq!(hostile.code, "SPX-H006");
}

#[test]
fn overlapping_shared_loan_replay_uses_existing_move_and_assignment_spans() {
    let move_source = r#"
module test.shared_loan_move_span;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.invalid") fn invalid(input: borrow Slice<u8>) -> usize {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = byte_len(view);
    let conflict = take(owned);
    observed
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let move_start = move_source.find("take(owned)").unwrap() + "take(".len();
    assert_overlap_span(
        move_source,
        "loan.invalid",
        Some((move_start, move_start + "owned".len())),
        |function| swap_let_statements(function, "observed", "conflict"),
    );

    let assignment_source = r#"
module test.shared_loan_assignment_span;
@id("loan.invalid") fn invalid(input: borrow Slice<u8>) -> usize {
    let mut owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let observed = byte_len(view);
    owned = bytes_copy(input);
    observed
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let assignment_start = assignment_source
        .find("owned = bytes_copy(input);")
        .unwrap();
    let assignment_end = assignment_start + "owned = bytes_copy(input);".len();
    assert_overlap_span(
        assignment_source,
        "loan.invalid",
        Some((assignment_start, assignment_end)),
        |function| move_assignment_before_let(function, "observed"),
    );

    assert_overlap_span(
        assignment_source,
        "loan.invalid",
        None,
        |function| move_assignment_before_let_without_span(function, "observed"),
    );
}

#[test]
fn nested_if_lazy_and_guard_paths_have_path_exact_last_use_edges() {
    let program = fixture();
    let function = &program.functions[paths_index(&program)];
    let plan = &function.loan_plan;

    assert!(plan.loans.len() >= 3, "parent, child, and sibling loans");
    assert!(plan.loans.iter().any(|loan| loan.parent.is_some()));
    assert!(
        plan.loans
            .iter()
            .filter(|loan| loan.parent.is_none())
            .count()
            >= 2
    );

    let branched = plan
        .loans
        .iter()
        .filter(|loan| loan.end_edges.len() > 1)
        .collect::<Vec<_>>();
    assert!(
        branched.len() >= 3,
        "each parent/child/sibling path must retain edge-qualified exits: {branched:?}"
    );
    assert!(branched.iter().any(|loan| loan.end_edges.len() >= 4));

    for loan in &plan.loans {
        assert!(!loan.ends.is_empty());
        assert!(!loan.end_edges.is_empty());
        for edge in &loan.end_edges {
            let edge = &plan.edges[*edge as usize];
            assert!(!edge.live.contains(&loan.id));
            assert!(plan.endpoints[edge.to as usize].kills.contains(&loan.id));
        }
    }

    let simultaneous = plan
        .edges
        .iter()
        .map(|edge| edge.live.len())
        .max()
        .unwrap_or(0);
    assert!(
        simultaneous >= 3,
        "parent, child, and sibling must coexist on at least one path"
    );
}

#[test]
fn attached_shared_loan_plan_replays_every_authenticated_surface() {
    reject_mutation("schema", |function| function.loan_plan.schema = "forged");
    reject_mutation("loan id", |function| {
        function.loan_plan.loans[0].id = LoanId(255)
    });
    reject_mutation("site", |function| {
        function.loan_plan.loans[0].site = function.body.id.clone()
    });
    reject_mutation("origin", |function| {
        function.loan_plan.loans[0].origin.root = function.result_id.clone()
    });
    reject_mutation("parent", |function| {
        let loan = function
            .loan_plan
            .loans
            .iter_mut()
            .find(|loan| loan.parent.is_some())
            .unwrap();
        loan.parent = None;
    });
    reject_mutation("start", |function| {
        let start = &mut function.loan_plan.loans[0].start;
        start.phase = match start.phase {
            LoanPointPhase::Before => LoanPointPhase::After,
            LoanPointPhase::After => LoanPointPhase::Before,
        };
    });
    reject_mutation("ends", |function| function.loan_plan.loans[0].ends.clear());
    reject_mutation("end edges", |function| {
        function.loan_plan.loans[0].end_edges.clear()
    });
    reject_mutation("endpoint point", |function| {
        function.loan_plan.endpoints[0].point = LoanProgramPoint {
            expression: function.body.id.clone(),
            phase: LoanPointPhase::After,
        }
    });
    reject_mutation("endpoint live before", |function| {
        function.loan_plan.endpoints[0]
            .live_before
            .push(LoanId(255))
    });
    reject_mutation("endpoint starts", |function| {
        function.loan_plan.endpoints[0].starts.push(LoanId(255))
    });
    reject_mutation("endpoint kills", |function| {
        function.loan_plan.endpoints[0].kills.push(LoanId(255))
    });
    reject_mutation("endpoint live after", |function| {
        function.loan_plan.endpoints[0].live_after.push(LoanId(255))
    });
    reject_mutation("edge from", |function| {
        function.loan_plan.edges[0].from = function.loan_plan.edges[0].to
    });
    reject_mutation("edge to", |function| {
        function.loan_plan.edges[0].to = function.loan_plan.edges[0].from
    });
    reject_mutation("edge live", |function| {
        function.loan_plan.edges[0].live.push(LoanId(255))
    });
    reject_mutation("loan order", |function| function.loan_plan.loans.swap(0, 1));
    reject_mutation("endpoint order", |function| {
        function.loan_plan.endpoints.swap(0, 1)
    });
    reject_mutation("edge order", |function| function.loan_plan.edges.swap(0, 1));
    reject_mutation("loan omission", |function| {
        function.loan_plan.loans.pop();
    });
    reject_mutation("endpoint omission", |function| {
        function.loan_plan.endpoints.pop();
    });
    reject_mutation("edge omission", |function| {
        function.loan_plan.edges.pop();
    });
}

#[test]
fn projected_field_loan_retains_the_stable_field_identity_and_rejects_forgery() {
    let program = fixture();
    let function = &program.functions[projected_index(&program)];
    let projected = function
        .loan_plan
        .loans
        .iter()
        .find(|loan| !loan.origin.projections.is_empty())
        .unwrap();
    assert_eq!(
        projected.origin.projections,
        [hir::PlaceProjection::Field(hir::DeclarationId::new(
            "loan.packet.left",
        ))]
    );
    let projected_facts = program
        .declarations
        .byte_slice_provenances()
        .filter(|(_, provenance)| !provenance.projections.is_empty())
        .collect::<Vec<_>>();
    assert!(projected_facts.len() >= 3, "direct, alias, and range facts");
    assert!(projected_facts.iter().all(|(_, provenance)| {
        provenance.projections
            == [hir::PlaceProjection::Field(hir::DeclarationId::new(
                "loan.packet.left",
            ))]
            && provenance.projected_type == hir::ResolvedType::Bytes
    }));
    assert!(projected_facts
        .iter()
        .any(|(_, provenance)| !provenance.ranges.is_empty()));

    for (name, projections) in [
        ("projection omission", Vec::new()),
        (
            "sibling substitution",
            vec![hir::PlaceProjection::Field(hir::DeclarationId::new(
                "loan.packet.right",
            ))],
        ),
        (
            "deeper projection",
            vec![
                hir::PlaceProjection::Field(hir::DeclarationId::new("loan.packet.left")),
                hir::PlaceProjection::Field(hir::DeclarationId::new("loan.packet.right")),
            ],
        ),
    ] {
        let mut forged = fixture();
        let index = projected_index(&forged);
        let function = &mut forged.functions[index];
        function
            .loan_plan
            .loans
            .iter_mut()
            .find(|loan| !loan.origin.projections.is_empty())
            .unwrap()
            .origin
            .projections = projections;
        let diagnostic = hir::validate(&forged).expect_err(name);
        assert_eq!(diagnostic.code, "SPX-H006", "{name}: {diagnostic:?}");
    }

    let mut forged = fixture();
    let index = projected_index(&forged);
    let function = &mut forged.functions[index];
    let result_id = function.result_id.clone();
    function
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| !loan.origin.projections.is_empty())
        .unwrap()
        .origin
        .root = result_id;
    let diagnostic = hir::validate(&forged).expect_err("root substitution");
    assert_eq!(diagnostic.code, "SPX-H006");
}
