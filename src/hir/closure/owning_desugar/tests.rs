use super::*;

const TARGET: &str = r#"
@id("owning.checksum") fn checksum(payload: own Bytes) -> i64 {
    42
}
"#;

fn source(body: &str) -> String {
    format!(
        "module test.owning_desugar;\n{TARGET}\n@id(\"owning.main\") fn main() -> i64 {{\n{body}\n}}\n"
    )
}

fn checked(body: &str) -> Program {
    crate::check(&source(body), "owning-desugar.spx").expect("bounded shape admits cleanly")
}

fn main_body(program: &Program) -> &Expr {
    &program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main")
        .body
}

#[test]
fn no_owning_closure_returns_none_unchanged() {
    let program = checked("checksum(bytes_zeroed(4usize))");
    assert!(desugar_owning_closures(&program).is_none());
}

#[test]
fn owning_closure_desugars_to_a_direct_target_call() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
"#;
    let program = checked(body);
    assert!(expr_has_owning_closure(main_body(&program)));

    let rewritten = desugar_owning_closures(&program).expect("owning closure present");
    let rewritten_body = main_body(&rewritten);
    assert!(
        !expr_has_owning_closure(rewritten_body),
        "the rewritten program must contain no `own fn` construct at all"
    );

    // Exactly one statement remains (`let payload = ...`); the `let clo = ...`
    // binding is gone and the tail is now the direct target call.
    let ExprKind::Block { statements, tail } = &rewritten_body.kind else {
        panic!("function body is a block")
    };
    assert_eq!(
        statements.len(),
        1,
        "the dead `let clo = ...` binding must be dropped"
    );
    let ExprKind::Call { name, args, .. } = &tail.kind else {
        panic!("tail must be the direct target call, got {tail:?}")
    };
    assert_eq!(name, "checksum");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0].kind, ExprKind::Var(captured) if captured == "payload"));

    // The rewritten program is ordinary syntax: HIR resolution (unchanged,
    // still refusing the *original* program) must succeed on it.
    crate::hir::resolve(&rewritten).expect("desugared program is ordinary source");
}

/// Locates the desugared local `payload` binding's `ValueId` and confirms
/// every occurrence of it as a cleanup-plan storage location shares that
/// exact identity, so the two assertions below ("transferred, never
/// finalized" vs. "finalized, never transferred") are provably about the
/// *same* captured owner, not two different slots that merely look alike.
fn payload_value_id(plan: &crate::cleanup_plan::CleanupPlan) -> crate::hir::ValueId {
    use crate::cleanup_plan::StorageId;
    plan.slots
        .iter()
        .find_map(|slot| match &slot.storage {
            StorageId::Value(id) if id.as_str().contains(":local:") => Some(id.clone()),
            _ => None,
        })
        .expect("desugared `main` has exactly one local Bytes slot: `payload`")
}

#[test]
fn called_owning_closure_transfers_the_captured_owner_into_the_call_exactly_once_and_never_finalizes_it(
) {
    use crate::cleanup_plan::{CleanupTransition, StorageId};

    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    clo()
"#;
    let program = checked(body);
    let rewritten = desugar_owning_closures(&program).expect("owning closure present");
    let resolved = crate::hir::resolve(&rewritten).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "owning.main")
        .unwrap();
    let payload_id = payload_value_id(&main.cleanup_plan);

    // The captured owner is transferred out of its local slot into the
    // call's one argument slot exactly once.
    let transfers_out_of_payload = main
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| {
            matches!(
                transition,
                CleanupTransition::Transfer { source, destination, .. }
                    if source.storage == StorageId::Value(payload_id.clone())
                        && matches!(destination.storage, StorageId::CallArgument { parameter_index: 0, .. })
            )
        })
        .count();
    assert_eq!(
        transfers_out_of_payload, 1,
        "the captured owner must move into the call's argument slot exactly once"
    );

    // That one argument slot is committed (consumed by the call) exactly
    // once, and no other call commits it.
    let commits_of_argument = main
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| {
            matches!(transition, CleanupTransition::CallCommit { arguments, .. }
                if arguments.iter().any(|argument| argument.parameter_index == 0))
        })
        .count();
    assert_eq!(
        commits_of_argument, 1,
        "the transferred argument must be committed by exactly one call"
    );

    // No exit finalizes (drops) the captured owner: it was fully consumed
    // by the call above, so a second, independent cleanup for the same
    // resource -- the double-free this test exists to rule out -- never
    // appears anywhere in the plan.
    let stray_finalizes = main
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .filter(|action| action.source.storage == StorageId::Value(payload_id.clone()))
        .count();
    assert_eq!(
        stray_finalizes, 0,
        "a transferred owner must never also be independently finalized"
    );
}

#[test]
fn uncalled_owning_closure_finalizes_the_captured_owner_exactly_once() {
    use crate::cleanup_plan::StorageId;

    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    1
"#;
    let program = checked(body);
    let rewritten = desugar_owning_closures(&program).expect("owning closure present");
    let resolved = crate::hir::resolve(&rewritten).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "owning.main")
        .unwrap();
    let payload_id = payload_value_id(&main.cleanup_plan);

    // Dropping the owning closure uncalled settles through the ordinary
    // scope-exit drop of its never-moved capture: exactly one finalize
    // action, on the one exit, freeing `payload` -- not zero (a leak) and
    // not two (a double free).
    let finalizes = main
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .filter(|action| {
            action.source.storage == StorageId::Value(payload_id.clone())
                && action.lifecycle_id.as_str() == "core.bytes.drop"
        })
        .count();
    assert_eq!(
        finalizes, 1,
        "the never-called closure's captured owner must be dropped exactly once"
    );
}

#[test]
fn uncalled_owning_closure_desugars_to_no_call_at_all() {
    let body = r#"
    let payload = bytes_zeroed(4usize);
    let clo = own fn() -> i64 { checksum(payload) };
    1
"#;
    let program = checked(body);
    let rewritten = desugar_owning_closures(&program).expect("owning closure present");
    let rewritten_body = main_body(&rewritten);
    let ExprKind::Block { statements, tail } = &rewritten_body.kind else {
        panic!("function body is a block")
    };
    // `payload` remains an ordinary, still-unmoved owned local; nothing
    // calls `checksum` at all, matching the uncalled-drop semantics.
    assert_eq!(statements.len(), 1);
    assert!(matches!(&tail.kind, ExprKind::Int(1)));
    crate::hir::resolve(&rewritten).expect("desugared program is ordinary source");
}

/// Collects every zero-argument call's name, in source order, through the
/// block/branch/let shapes the #578 regressions below use.
fn zero_argument_calls(expr: &Expr, names: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Call { name, args, .. } => {
            if args.is_empty() {
                names.push(name.clone());
            }
            for arg in args {
                zero_argument_calls(arg, names);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            zero_argument_calls(left, names);
            zero_argument_calls(right, names);
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                if let Statement::Let { value, .. } = statement {
                    zero_argument_calls(value, names);
                }
            }
            zero_argument_calls(tail, names);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            zero_argument_calls(condition, names);
            zero_argument_calls(then_branch, names);
            zero_argument_calls(else_branch, names);
        }
        _ => {}
    }
}

fn calls_after_desugaring(body: &str) -> Vec<String> {
    let program = checked(body);
    let rewritten = desugar_owning_closures(&program).expect("owning closure present");
    crate::hir::resolve(&rewritten).expect("desugared program is ordinary source");
    let mut names = Vec::new();
    zero_argument_calls(main_body(&rewritten), &mut names);
    names
}

/// Issue #578: a substitution recorded in one block must not rewrite a
/// same-spelled ordinary callback declared in a later disjoint block, and
/// must not transplant the earlier capture out of its scope.
#[test]
fn owning_substitution_does_not_leak_into_a_disjoint_sibling_block() {
    let body = r#"
    let first = { let payload = bytes_zeroed(1usize); let callback = own fn() -> i64 { checksum(payload) }; callback() };
    let second = { let callback = fn() -> i64 { 33 }; callback() };
    first + second
"#;
    assert_eq!(calls_after_desugaring(body), ["callback"]);
}

/// Issue #578: sibling conditional branches are separate lexical scopes.
#[test]
fn owning_substitution_does_not_leak_into_a_sibling_branch() {
    let body = r#"
    if true {
        let payload = bytes_zeroed(1usize);
        let callback = own fn() -> i64 { checksum(payload) };
        callback()
    } else {
        let callback = fn() -> i64 { 33 };
        callback()
    }
"#;
    assert_eq!(calls_after_desugaring(body), ["callback"]);
}

/// Issue #578: an outer owning callback stays callable from a nested block
/// inside its own scope; its mapping ends with that scope.
#[test]
fn outer_owning_callback_is_still_substituted_inside_a_nested_block() {
    let body = r#"
    let kept = {
        let payload = bytes_zeroed(4usize);
        let callback = own fn() -> i64 { checksum(payload) };
        let nested = { callback() };
        nested
    };
    let later = { let callback = fn() -> i64 { 1 }; callback() };
    kept + later
"#;
    assert_eq!(calls_after_desugaring(body), ["callback"]);
}
