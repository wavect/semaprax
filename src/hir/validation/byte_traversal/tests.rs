//! Owner freeze, provenance and receiver-shape authentication, each exercised
//! against its own independent check rather than through one shared refusal.
use super::*;

type Mutation = (&'static str, fn(&mut ResolvedExpr));
type Replay = fn(&DeclarationIndex, &ResolvedFunction) -> Result<(), Diagnostic>;
use crate::loan_plan::{LoanCause, LoanPointPhase};

/// An owned, renewable root viewed through a subrange alias: the source
/// owner is mutable, so only the traversal freeze keeps it fixed.
const SOURCE: &str = r#"module app.byte_traversal_freeze;

@id("app.freeze.main")
fn main() -> i64 {
    let data = [1u8, 2u8, 3u8];
    let mut owned = bytes_copy(array_as_slice(data));
    let whole = bytes_as_slice(owned);
    let view = byte_range(whole, 0usize, 2usize);
    let mut total = 0;
    for byte in view { let first = byte; total = total + 1; 0 }
    total
}
"#;

fn resolved() -> ResolvedProgram {
    let ast = crate::parse(SOURCE, std::path::Path::new("byte-traversal-freeze.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}

fn statements(function: &mut ResolvedFunction) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    statements
}

fn owner(program: &ResolvedProgram) -> ResolvedBinding {
    let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
        panic!("function block")
    };
    statements
        .iter()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, .. } if binding.name == "owned" => {
                Some(binding.clone())
            }
            _ => None,
        })
        .unwrap()
}

fn region_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    statements(function)
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}

fn loop_body(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region block")
    };
    let (ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. }) =
        &mut statements[2]
    else {
        panic!("traversal loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop body")
    };
    statements
}

fn authored(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedStatement::Let { value, .. } = &mut loop_body(region)[1] else {
        panic!("authored body")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    statements
}

fn receiver(region: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let ResolvedStatement::Let { value, .. } = &mut loop_body(region)[0] else {
        panic!("item")
    };
    value
}

fn execution(program: &ResolvedProgram) -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(program.functions[0].id.clone())
}

#[test]
fn owner_assignment_anywhere_in_the_body_is_refused_by_the_census_freeze() {
    let mut program = resolved();
    let owned = owner(&program);
    let execution = execution(&program);
    let assignment = ResolvedStatement::Assign {
        binding: owned.clone(),
        field: None,
        value: ResolvedExpr {
            id: ExpressionId::new(&execution, "forged.renewal"),
            ty: owned.ty.clone(),
            ownership: OwnershipMode::Own,
            kind: ResolvedExprKind::Place(Place {
                root: owned.id.clone(),
                projections: Vec::new(),
            }),
            span: owned.span,
        },
        span: owned.span,
    };
    authored(region_mut(&mut program.functions[0])).insert(0, assignment);
    // The census reads only the authenticated provenance root, never liveness.
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("owner"), "{}", error.message);
    // Loan planning refuses the same renewal through the traversal freeze.
    let error = crate::loan_plan::build_plan(&program, &program.functions[0]).unwrap_err();
    assert!(
        error.message.contains("active shared loan"),
        "{}",
        error.message
    );
}

#[test]
fn traversal_freeze_loan_spans_the_whole_region_on_the_resolved_owner() {
    let program = resolved();
    let owned = owner(&program);
    let mut copy = program.clone();
    let region = region_mut(&mut copy.functions[0]).clone();
    let body_id = {
        let mut region = region.clone();
        let ResolvedStatement::Let { value, .. } = &loop_body(&mut region)[1] else {
            panic!("authored body")
        };
        value.id.clone()
    };
    let plan = &program.functions[0].loan_plan;
    let freeze = plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::TraversalFreeze)
        .expect("byte traversal freeze loan");
    // Through `view` -> `whole` -> `owned`: the subrange alias reaches the root.
    assert_eq!(freeze.origin.root, owned.id);
    assert_eq!(freeze.site, region.id);
    assert_eq!(freeze.start.expression, region.id);
    assert_eq!(freeze.start.phase, LoanPointPhase::Before);
    let live_at = |expression: &ExpressionId, phase: LoanPointPhase| {
        plan.endpoints.iter().any(|endpoint| {
            endpoint.point.expression == *expression
                && endpoint.point.phase == phase
                && endpoint.live_after.contains(&freeze.id)
        })
    };
    // Live after the authored body on every iteration, and until the region ends.
    assert!(live_at(&body_id, LoanPointPhase::After));
    assert!(plan
        .endpoints
        .iter()
        .any(|endpoint| endpoint.point.expression == region.id
            && endpoint.point.phase == LoanPointPhase::After
            && endpoint.live_before.contains(&freeze.id)));

    // Attached evidence without the freeze is forged.
    let mut forged = program.clone();
    forged.functions[0]
        .loan_plan
        .loans
        .retain(|loan| loan.cause != LoanCause::TraversalFreeze);
    assert!(crate::loan_plan::validate_program(&forged).is_err());
}

#[test]
fn detached_view_without_authenticated_provenance_is_refused() {
    let mut program = resolved();
    let execution = execution(&program);
    let detached = ValueId::local(&execution, "forged.view");
    let region = region_mut(&mut program.functions[0]);
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region block")
    };
    let ResolvedStatement::Let { value: count, .. } = &mut statements[0] else {
        panic!("snapshot")
    };
    let ResolvedExprKind::Call { args, .. } = &mut count.kind else {
        panic!("byte_len")
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("view place")
    };
    place.root = detached.clone();
    let ResolvedExprKind::Match { scrutinee, .. } = &mut receiver(region).kind else {
        panic!("receiver")
    };
    let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
        panic!("byte_get")
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("view place")
    };
    place.root = detached;
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert!(error.message.contains("provenance"), "{}", error.message);
}

#[test]
fn nested_receiver_in_the_authored_body_is_refused() {
    let mut program = resolved();
    let region = region_mut(&mut program.functions[0]);
    let nested = receiver(region).clone();
    let ResolvedStatement::Let { value, .. } = &mut authored(region)[0] else {
        panic!("authored let")
    };
    *value = nested;
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert!(error.message.contains("nested"), "{}", error.message);
}

/// Every partial `Option<u8>` consumer (HIR exhaustiveness, Graph projection)
/// keys on `partial_receiver`. Each single-fact deviation loses the exemption
/// and is refused as an ordinary non-exhaustive or ill-typed match.
#[test]
fn only_the_fully_authenticated_receiver_shape_is_partial() {
    let program = resolved();
    let mut copy = program.clone();
    let original = receiver(region_mut(&mut copy.functions[0])).clone();
    assert!(partial_receiver(&original));
    let mutations: [Mutation; 9] = [
        ("guarded arm", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            arms[0].guard = Some(Box::new(arms[0].value.clone()));
        }),
        ("None case", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedMatchPattern::Variant { case, .. } = &mut arms[0].pattern else {
                unreachable!()
            };
            *case = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
        }),
        ("foreign field", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
                unreachable!()
            };
            fields[0].field = DeclarationId::new("core.option.some.other");
        }),
        ("borrow mode", |e| {
            let ResolvedExprKind::Match { mode, .. } = &mut e.kind else {
                unreachable!()
            };
            *mode = ResolvedMatchMode::Borrow;
        }),
        ("vec_get scrutinee", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { callee, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            *callee = DeclarationId::new(crate::vec_ops::GET_ID);
        }),
        ("typed byte_get", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { type_arguments, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            type_arguments.push(ResolvedType::U8);
        }),
        ("projected index", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            args[1].kind = ResolvedExprKind::Usize(0);
        }),
        ("arm reads a literal", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            arms[0].value.kind = ResolvedExprKind::Uint8(0);
        }),
        ("widened result", |e| {
            e.ty = ResolvedType::I64;
        }),
    ];
    for (case, mutate) in mutations {
        let mut forged = program.clone();
        mutate(receiver(region_mut(&mut forged.functions[0])));
        assert!(
            !partial_receiver(receiver(region_mut(&mut forged.functions[0]))),
            "{case}"
        );
        assert!(crate::hir::validate(&forged).is_err(), "{case}");
    }
}

#[test]
fn complete_receiver_census_region_and_discovery_have_no_heap_scratch() {
    let program = resolved();
    // Both complete proof routes still have no heap scratch. Keep the old
    // zero-debit witness and give the current route only its new root fees.
    let roots = program
        .functions
        .iter()
        .map(|f| f.ensures.len() + 1 + f.requires.len())
        .sum::<usize>();
    for (replay, debit) in [
        (validate_function_before_scalar_roots as Replay, 0),
        (validate_function as Replay, roots * SCALAR_ROOT_CONTROLS),
    ] {
        let (((), observed), refused, used) =
            crate::bounded_output::with_limit_usage(debit, || {
                crate::test_allocation_observer::measure(|| {
                    for function in &program.functions {
                        replay(&program.declarations, function).unwrap();
                    }
                    assert!(program_uses(&program));
                })
            });
        assert!(!refused);
        assert_eq!(used, debit);
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.allocated, 0);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.freed, 0);
    }

    let ast = crate::parse(
        "module app.plain; @id(\"app.plain.main\") fn main() -> i64 { 0 }",
        std::path::Path::new("plain.spx"),
    )
    .unwrap();
    let plain = crate::hir::resolve(&ast).unwrap();
    let ((), absent) = crate::test_allocation_observer::measure(|| {
        validate_function(&plain.declarations, &plain.functions[0]).unwrap();
        assert!(!program_uses(&plain));
    });
    assert!(!absent.invalid);
    assert_eq!(absent.allocations, 0);
    assert_eq!(absent.live, 0);
}

// Immediate predecessor, including the nested handoff and zero-affinity check.
fn validate_function_v1_before_scalar_roots(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut unauthorized = false;
    for root in function
        .ensures
        .iter()
        .rev()
        .chain(std::iter::once(&function.body))
        .chain(function.requires.iter().rev())
    {
        walk(root, |expression, mut authorized| {
            if let Some(region) = region(expression)? {
                let view = bare_place(region.view)
                    .ok_or_else(|| fail("borrowed byte traversal view is not a bare place"))?;
                let provenance = declarations.byte_slice_provenance(&view.root)
                    .ok_or_else(|| fail("borrowed byte traversal view lacks authenticated byte-slice provenance"))?;
                if provenance.root != view.root {
                    frozen_body(region.authored_body, region.payload, &[&provenance.root])?;
                }
                authorized = Some(region.receiver);
            }
            if receiver_shape(expression).is_some()
                && !authorized.is_some_and(|receiver| std::ptr::eq(receiver, expression))
            {
                unauthorized = true;
            }
            Ok(authorized)
        })?;
    }
    // Region/provenance errors retain their original precedence over the
    // final whole-function detached-receiver census refusal.
    if unauthorized {
        return Err(fail(
            "borrowed byte receiver is outside an authenticated Slice<u8> traversal",
        ));
    }
    Ok(())
}

/// Nested replay derives a fresh function-bound census after primary HIR
/// validation has independently replayed stored byte provenance. The complete
/// v1 receiver remains the ordinary non-nested and contextless path.
fn validate_function_before_scalar_roots(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    nested::slot::controls(0)?;
    match validate_function_v1_before_scalar_roots(declarations, function) {
        Err(error) if nested::slot::nested_refusal(&error) => {
            nested::Census::validate(declarations, function)
        }
        original => original,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RootObservation {
    selections: usize,
    setups: usize,
}

thread_local! {
    static ROOT_OBSERVATION: std::cell::Cell<Option<RootObservation>> =
        const { std::cell::Cell::new(None) };
}

pub(super) fn record_walk_setup() {
    if let Some(mut observed) = ROOT_OBSERVATION.get() {
        observed.setups += 1;
        ROOT_OBSERVATION.set(Some(observed));
    }
}

pub(super) fn record_scalar_selection() {
    if let Some(mut observed) = ROOT_OBSERVATION.get() {
        observed.selections += 1;
        ROOT_OBSERVATION.set(Some(observed));
    }
}

fn observe_root_replay<T>(operation: impl FnOnce() -> T) -> (T, RootObservation) {
    assert!(ROOT_OBSERVATION
        .replace(Some(RootObservation::default()))
        .is_none());
    let result = operation();
    (result, ROOT_OBSERVATION.replace(None).unwrap())
}

#[test]
fn scalar_roots_preserve_the_whole_immediate_predecessor_and_inverse() {
    use std::io::Read as _;

    const DIGEST: &str = "0d71a63bfe68e872a57d7ce42c0862b87dfcea6203ac412e9d34b1346cb2bbec";
    let read = |path: &str| {
        let mut contents = String::new();
        std::fs::File::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap()
            .take(32 * 1024)
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.len() < 32 * 1024);
        contents
    };
    let old = read(&format!("e/{DIGEST}"));
    assert_eq!(
        crate::audit_capsule::sha256_digest(old.as_bytes()),
        format!("sha256:{DIGEST}")
    );
    let mut restored = read("src/hir/validation/byte_traversal.rs");
    let selector = restored
        .find("// Pay for selector actual/formal root loans")
        .unwrap();
    let census = restored.find("/// Census: every partial receiver").unwrap();
    restored.replace_range(selector..census, "");
    for insertion in [
        "        nested::slot::controls(SCALAR_ROOT_CONTROLS)?;\n        if scalar_root(root) {\n            continue;\n        }\n",
        "// Keep the fixed frame array on the walk side of the scalar-root branch.\n#[inline(never)]\n",
        "    #[cfg(test)]\n    tests::record_walk_setup();\n",
    ] {
        assert_eq!(restored.matches(insertion).count(), 1);
        restored = restored.replacen(insertion, "", 1);
    }
    assert_eq!(restored, old);

    let tests = read("src/hir/validation/byte_traversal/tests.rs");
    let saved = &tests[tests
        .find("fn validate_function_v1_before_scalar_roots(")
        .unwrap()
        ..tests
            .find("#[derive(Clone, Copy, Debug, Default)]")
            .unwrap()];
    let saved = saved
        .replace(
            "validate_function_v1_before_scalar_roots(",
            "validate_function_v1(",
        )
        .replace(
            "fn validate_function_before_scalar_roots(",
            "pub(in crate::hir) fn validate_function(",
        );
    let old_driver = &old[old.find("fn validate_function_v1(").unwrap()
        ..old.find("/// Receiver presence selects").unwrap()];
    assert_eq!(saved.trim_end(), old_driver.trim_end());
}

#[test]
fn scalar_roots_skip_only_exact_shapes_in_a_mixed_program() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;

    let source = format!(
        "{SOURCE}\n{}",
        r#"
@id("app.freeze.literal")
fn literal() -> i64
    requires true
    ensures true
{ 17 }
@id("app.freeze.count")
fn count() -> usize { 2usize }
@id("app.freeze.flag")
fn flag() -> bool { false }
@id("app.freeze.byte")
fn byte() -> u8 { 1u8 }
@id("app.freeze.identity")
fn identity<T>(value: T) -> T { value }
@id("app.freeze.call")
fn call() -> i64 { identity<i64>(17) }
@id("app.freeze.statements")
fn statements() -> i64 { let value = 17; 0 }
@id("app.freeze.double")
fn double_block() -> i64 { { 17 } }
"#
    );
    let ast = crate::parse(&source, std::path::Path::new("mixed-byte-replay.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    assert_eq!(program.function_instances.len(), 1);
    let run = |validate: Replay| {
        let ((result, metrics), refused, debit, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    observe_root_replay(|| {
                        for function in program
                            .functions
                            .iter()
                            .chain(program.function_instances.iter().map(|i| &i.function))
                        {
                            validate(&program.declarations, function).unwrap();
                        }
                    })
                })
            });
        assert!(!refused);
        assert_eq!(first, None);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        (
            result.1.setups,
            debit,
            metrics.allocated,
            result.1.selections,
        )
    };
    let old = run(validate_function_before_scalar_roots);
    let current = run(validate_function);
    let roots = program.functions.len() + program.function_instances.len() + 2;
    assert_eq!(
        old.0 - current.0,
        5,
        "two contracts and three scalar bodies"
    );
    assert_eq!(current.1 - old.1, roots * SCALAR_ROOT_CONTROLS);
    assert_eq!(current.2, old.2);
    assert_eq!(old.3, 0);
    assert_eq!(current.3, roots);
    eprintln!(
        "mixed roots: {roots}; old (setups, debit, allocated bytes, selections) {old:?}; current {current:?}"
    );
}

#[test]
fn scalar_roots_preserve_hostile_region_receiver_depth_and_late_error_order() {
    let mut program = resolved();
    let binding = owner(&program);
    assert!(program
        .declarations
        .byte_slice_provenance(&binding.id)
        .is_none());
    let function = &mut program.functions[0];
    let mut malformed = region_mut(function).clone();
    let detached = receiver(&mut malformed).clone();
    let ResolvedExprKind::Block { tail, .. } = &mut malformed.kind else {
        unreachable!()
    };
    tail.kind = ResolvedExprKind::Usize(1);
    let mut no_provenance = region_mut(function).clone();
    let ResolvedExprKind::Block { statements, .. } = &mut no_provenance.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let { value: count, .. } = &mut statements[0] else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &mut count.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Place(view) = &mut args[0].kind else {
        unreachable!()
    };
    // Point both uses at an identity with no byte-slice provenance. Changing
    // only one of them would select the earlier structural error.
    view.root = binding.id.clone();
    let new_view = args[0].clone();
    let ResolvedExprKind::Match { scrutinee, .. } = &mut receiver(&mut no_provenance).kind else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
        unreachable!()
    };
    args[0] = new_view;

    let mut scalar = function.body.clone();
    scalar.kind = ResolvedExprKind::Int(0);
    let mut deep = scalar.clone();
    for _ in 0..=crate::cache_codec::MAX_DEPTH {
        deep = ResolvedExpr {
            kind: ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(deep),
            },
            ..scalar.clone()
        };
    }
    let decl = &program.declarations;
    let compare = |function: &ResolvedFunction, expected: &str| {
        let old = validate_function_before_scalar_roots(decl, function);
        let current = validate_function(decl, function);
        assert_eq!(current, old);
        assert!(
            current.unwrap_err().message.contains(expected),
            "{expected}"
        );
    };
    function.body = scalar.clone();
    function.ensures = vec![detached.clone(), scalar.clone()];
    function.requires = vec![malformed.clone(), scalar.clone()];
    compare(function, "differs from its authenticated lowering");
    function.requires = vec![no_provenance, scalar.clone()];
    compare(function, "lacks authenticated byte-slice provenance");
    function.requires = vec![malformed.clone(), deep];
    compare(function, "exceeds admitted expression depth");
    let deep = function.requires.pop().unwrap();
    function.ensures = vec![deep, malformed];
    compare(function, "differs from its authenticated lowering");
    function.ensures.clear();
    function.requires = vec![scalar];
    function.body = detached.clone();
    compare(function, "outside an authenticated Slice<u8> traversal");

    // An exact empty block with a non-scalar tail and closure captures must
    // still enter the full walker and authenticate the detached receiver.
    let mut empty = function.body.clone();
    empty.kind = ResolvedExprKind::Block {
        statements: Vec::new(),
        tail: Box::new(detached.clone()),
    };
    function.body = empty;
    compare(function, "outside an authenticated Slice<u8> traversal");
    function.body.kind = ResolvedExprKind::Closure {
        parameters: Vec::new(),
        captures: Vec::new(),
        body: Box::new(detached.clone()),
    };
    // Closure bodies have their own scope; only evaluated captures are walked here.
    assert_eq!(
        validate_function_before_scalar_roots(decl, function),
        Ok(())
    );
    assert_eq!(validate_function(decl, function), Ok(()));
    let ResolvedExprKind::Closure { captures, .. } = &mut function.body.kind else {
        unreachable!()
    };
    captures.push(crate::hir::ResolvedClosureCapture {
        binding,
        value: detached,
    });
    compare(function, "outside an authenticated Slice<u8> traversal");
}

#[test]
fn scalar_root_controls_respect_exact_short_zero_floor_sticky_and_full_drop() {
    use crate::bounded_output::{
        reserve_active_required, set_active_floor, with_limit_usage_refusal, ReservationRefusal,
    };
    use crate::test_allocation_observer::measure;

    let mut program = resolved();
    let function = &mut program.functions[0];
    function.body.kind = ResolvedExprKind::Bool(true);
    let fee = SCALAR_ROOT_CONTROLS;
    let observe = |cap, floor, poison| {
        with_limit_usage_refusal(cap, || {
            assert!(set_active_floor(floor));
            if poison {
                assert!(!reserve_active_required(cap + 1));
            }
            measure(|| {
                observe_root_replay(|| {
                    let result = validate_function(&program.declarations, function);
                    let failed = result.is_err();
                    drop(result);
                    failed
                })
            })
        })
    };
    for (cap, floor, poison, expected_request) in [
        (fee, 0, false, None),
        (fee - 1, 0, false, Some(fee)),
        (0, 0, false, Some(fee)),
        (fee + 1, 1, false, None),
        (fee, 1, false, Some(fee)),
        (fee, 0, true, Some(fee + 1)),
    ] {
        let (((failed, observed), metrics), refused, consumed, first) = observe(cap, floor, poison);
        assert_eq!(observed.setups, 0);
        assert_eq!(observed.selections, usize::from(!failed));
        assert!(!metrics.invalid);
        assert_eq!((metrics.allocated, metrics.freed, metrics.live), (0, 0, 0));
        assert_eq!(failed, expected_request.is_some());
        assert_eq!(refused, failed);
        assert_eq!(consumed, if failed { 0 } else { fee });
        assert_eq!(
            first,
            expected_request.map(|requested| ReservationRefusal {
                requested,
                remaining: cap,
                floor,
                stage: None,
            })
        );
    }
}
