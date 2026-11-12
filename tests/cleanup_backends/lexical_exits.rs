//! Actual native exits: shared postconditions, output poisoning, and owned cleanup.
#[path = "lexical_exits/contract_owners.rs"]
mod contract_owners;

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"
module test.lexical_exits;
@id("lexical.run")
fn run(fail: bool) -> i64
    requires string_len("req") == 3
    ensures string_len("post") == 4
    ensures result == 9
{
    let outer = "outer";
    let mut index = 0;
    while index < 5 {
        let inner = "inner";
        index = index + 1;
        if index == 2 { continue; };
        if index == 4 { break; };
        0
    }
    if fail { return 8; };
    return if index == 4 { 9 } else { 0 };
}
@id("lexical.take")
fn take(payload: string) -> i64 ensures result == 9 {
    while false { break; }
    return 9;
}
@id("app.main")
fn main() -> i64 { run(false) + take("kept") }
"#;

#[test]
fn native_lexical_loops_return_cleanup_and_failed_ensures_preserve_reentry_o0_o2() {
    native_contract_probe(SOURCE);
}

/// A valued guard terminated by `;` is the unterminated control program:
/// the same C, contract failure selection, cleanup counts and reentry.
#[test]
fn terminated_valued_guards_keep_native_ensures_cleanup_and_reentry_o0_o2() {
    let (mut control, mut terminated) = (SOURCE.to_owned(), SOURCE.to_owned());
    for guard in [
        "if index == 2 { continue; }",
        "if index == 4 { break; }",
        "if fail { return 8; }",
    ] {
        let authored = format!("{guard};");
        assert_eq!(SOURCE.matches(&authored).count(), 1, "{guard}");
        control = control.replace(&authored, &format!("{guard} else {{ 0 }}"));
        terminated = terminated.replace(&authored, &format!("{guard} else {{ 0 }};"));
    }
    let parsed =
        |source: &str| semaprax::parse(source, Path::new("native-lexical-guards.spx")).unwrap();
    let (new, old) = (parsed(&terminated), parsed(&control));
    assert_eq!(
        semaprax::format::canonical(&new),
        semaprax::format::canonical(&old)
    );
    assert_eq!(
        semaprax::codegen::emit_c(&new).unwrap(),
        semaprax::codegen::emit_c(&old).unwrap()
    );
    native_contract_probe(&control);
    native_contract_probe(&terminated);
}

fn native_contract_probe(source: &str) {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = semaprax::parse(source, Path::new("native-lexical-exits.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&parsed).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&parsed).unwrap());
    assert!(generated.contains("spx_lexical_latch_"));
    assert!(generated.contains("spx_lexical_break_"));
    assert!(generated.contains("spx_canonical_cleanup_done = true;"));
    let symbol = format!(
        "spx_decl_{}",
        "lexical.run"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let take_symbol = format!(
        "spx_decl_{}",
        "lexical.take"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let observer = r#"
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
static size_t spx_test_allocated = 0;
static size_t spx_test_freed = 0;
static void *spx_test_malloc(size_t size) {
    void *value = malloc(size);
    if (value != NULL) spx_test_allocated += 1;
    return value;
}
static void spx_test_free(void *value) {
    if (value != NULL) spx_test_freed += 1;
    free(value);
}
#define malloc spx_test_malloc
#define free spx_test_free
"#;
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[32];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, 734, entries, 32, NULL, NULL, NULL)) return 10;
    for (int pass = 0; pass < 3; pass += 1) {{
        int64_t result = INT64_C(1234567);
        size_t allocated_before = spx_test_allocated;
        size_t freed_before = spx_test_freed;
        spx_status_token status = {symbol}(&context, pass == 1, &result);
        if (pass == 1) {{
            const struct spx_normalized_status *selected = spx_status_resolve(&context, status);
            if (status == SPX_STATUS_SUCCESS || selected == NULL || strcmp(selected->domain_id, "semaprax.contract.v1") != 0 || selected->code != UINT32_C(2) || result != INT64_C(1234567)) return 11;
        }} else if (status != SPX_STATUS_SUCCESS || result != INT64_C(9)) return 12;
        if (spx_test_allocated - allocated_before != 7 || spx_test_freed - freed_before != 7) return 13;
    }}
    // The ABI already transferred this owner to the callee. Entry refusal
    // must settle it once before publication, without entering or decrementing
    // a frame that was never admitted.
    context.call_depth = UINT32_C(256);
    int64_t result = INT64_C(1234567);
    size_t allocated_before = spx_test_allocated;
    size_t freed_before = spx_test_freed;
    char *payload = spx_string_from_literal("held", UINT64_C(4));
    spx_status_token status = {take_symbol}(&context, payload, &result);
    const struct spx_normalized_status *selected = spx_status_resolve(&context, status);
    if (status == SPX_STATUS_SUCCESS || selected == NULL || strcmp(selected->domain_id, "semaprax.runtime.v1") != 0 || selected->code != UINT32_C(1) || result != INT64_C(1234567) || context.call_depth != UINT32_C(256)) return 14;
    if (spx_test_allocated - allocated_before != 1 || spx_test_freed - freed_before != 1) return 15;
    context.call_depth = UINT32_C(0);
    allocated_before = spx_test_allocated;
    freed_before = spx_test_freed;
    payload = spx_string_from_literal("again", UINT64_C(5));
    status = {take_symbol}(&context, payload, &result);
    if (status != SPX_STATUS_SUCCESS || result != INT64_C(9) || context.call_depth != UINT32_C(0)) return 16;
    if (spx_test_allocated - allocated_before != 1 || spx_test_freed - freed_before != 1) return 17;
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-lexical-native-{}-{unique}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let binary = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{observer}\n{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary).output().unwrap();
        std::fs::remove_file(&source).unwrap();
        std::fs::remove_file(&binary).unwrap();
        assert!(run.status.success(), "{optimization}: {run:?}");
    }
}

const OWNED_BREAK: &str = r#"
module test.lexical_owned_break;
@id("app.main") fn main() -> i64 {
    let outer = bytes_zeroed(7usize);
    let first = vec_push<Bytes>(vec_with_capacity<Bytes>(3usize), bytes_zeroed(1usize));
    let second = vec_push<Bytes>(first, bytes_zeroed(2usize));
    let values = vec_push<Bytes>(second, bytes_zeroed(3usize));
    let mut seen = 0;
    for own item in vec_into_iter<Bytes>(values) {
        let nested = bytes_zeroed(3usize);
        seen = seen + 1;
        if seen == 2 { continue; };
        if seen == 3 { break; };
        0
    }
    if seen == 3 && byte_len(bytes_as_slice(outer)) == 7usize { 9 } else { 0 }
}
"#;

#[test]
fn owned_iterator_break_finishes_only_its_wrapper_and_preserves_parent_order() {
    use semaprax::{ast::LexicalExitKind, cleanup_plan::ExitContinuation, hir};
    let ast = semaprax::check(OWNED_BREAK, "owning-break.spx").expect("source admission");
    let resolved = hir::resolve(&ast).expect("owning wrapper cleanup construction");
    hir::validate(&resolved).expect("independent owning Break replay");
    let function = resolved
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let plan = &function.cleanup_plan;
    let control = plan.lexical_control.as_ref().expect("lexical certificate");
    assert_eq!(
        control
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Continue && exit.resume.is_some())
            .count(),
        1,
        "owning Continue has an independent advance domain"
    );
    let exits: Vec<_> = control
        .exits
        .iter()
        .filter(|exit| exit.kind == LexicalExitKind::Break)
        .collect();
    assert_eq!(exits.len(), 1);
    let site = exits[0];
    let resume = site
        .resume
        .expect("owning Break completes its wrapper independently");
    let domain = plan.lexical_resume(resume).expect("unique resume domain");
    let exit = &plan.exits[site.exit.0 as usize];
    let ExitContinuation::LoopBreak { edge, .. } = &exit.continuation else {
        panic!("typed Break route");
    };
    assert_eq!(plan.edges[edge.0 as usize].to, resume);
    assert!(
        !exit.finalize_in_order.is_empty(),
        "item/remainder/nested owner settlement"
    );
    assert!(plan
        .exits
        .iter()
        .any(|exit| domain.blocks.contains(&exit.from)
            && !exit.leaves_regions.is_empty()
            && matches!(exit.continuation, ExitContinuation::Continue(_))));
    let generated = semaprax::codegen::emit_c(&ast).expect("native owning Break route");
    assert_eq!(generated, semaprax::codegen::emit_c(&ast).unwrap());
}

#[test]
fn owning_break_replay_refuses_a_forged_nonzero_wrapper_tail() {
    use semaprax::hir::{self, LoopLatchKind, ResolvedExpr, ResolvedExprKind, ResolvedStatement};
    fn forge(expression: &mut ResolvedExpr) -> bool {
        let ResolvedExprKind::Block { statements, tail } = &mut expression.kind else {
            return false;
        };
        if matches!(
            statements.last(),
            Some(ResolvedStatement::LexicalWhile {
                latch: LoopLatchKind::OwnedIteratorStep { .. },
                ..
            })
        ) {
            assert!(matches!(tail.kind, ResolvedExprKind::Int(0)));
            tail.kind = ResolvedExprKind::Int(1);
            return true;
        }
        for statement in statements {
            if let ResolvedStatement::Let { value, .. } = statement {
                if forge(value) {
                    return true;
                }
            }
        }
        forge(tail)
    }
    let ast = semaprax::check(OWNED_BREAK, "owning-break-forged.spx").unwrap();
    let mut resolved = hir::resolve(&ast).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(
        forge(&mut function.body),
        "authenticated generated wrapper retained"
    );
    let failure =
        hir::validate(&resolved).expect_err("tail drift must not inherit wrapper authority");
    assert_eq!(failure.code, "SPX-H006");
}

#[test]
fn native_owning_loop_normal_then_continue_then_break_settles_every_owner_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let entry = r#"@id("app.main")"#;
    assert_eq!(OWNED_BREAK.matches(entry).count(), 1);
    let source = OWNED_BREAK.replace(entry, r#"@id("owned.leaf.main")"#);
    // Outer Bytes, three iterator items and one scratch owner per iteration.
    // The native observer measures the actual constructor and drop events.
    super::owned_leaf_collections::native::run_observing_zeroed_payloads(&source, 9, 7, 22, 5);
    let normal_tail = "if seen == 2 { continue; };\n        if seen == 3 { break; };\n        0";
    assert_eq!(source.matches(normal_tail).count(), 1);
    let all_abrupt = source.replace(
        normal_tail,
        "if seen == 1 { continue; }\n        if seen == 2 { continue; }\n        break;",
    );
    // Both finite advance domains must run their own canonical scope cleanup;
    // the ordinary body and inactive Done arm have no normal completion.
    super::owned_leaf_collections::native::run_observing_zeroed_payloads(&all_abrupt, 9, 7, 22, 5);
}

#[test]
fn terminated_valued_owning_guards_settle_every_owner_like_controls_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let source = OWNED_BREAK.replace(r#"@id("app.main")"#, r#"@id("owned.leaf.main")"#);
    let (mut control, mut terminated) = (source.clone(), source.clone());
    for guard in ["if seen == 2 { continue; }", "if seen == 3 { break; }"] {
        let authored = format!("{guard};");
        assert_eq!(source.matches(&authored).count(), 1, "{guard}");
        control = control.replace(&authored, &format!("{guard} else {{ 0 }}"));
        terminated = terminated.replace(&authored, &format!("{guard} else {{ 0 }};"));
    }
    let checked = |source: &str| semaprax::check(source, "owning-guards.spx").unwrap();
    let (new, old) = (checked(&terminated), checked(&control));
    assert_eq!(
        semaprax::format::canonical(&new),
        semaprax::format::canonical(&old)
    );
    assert_eq!(
        semaprax::codegen::emit_c(&new).unwrap(),
        semaprax::codegen::emit_c(&old).unwrap()
    );
    // Item, remainder and nested owners settle once on every route at O0/O2.
    for source in [&control, &terminated] {
        super::owned_leaf_collections::native::run(
            source,
            0,
            9,
            super::owned_leaf_collections::native::Failure::None,
        );
    }
}

#[test]
fn owning_break_replay_refuses_a_forged_step_renewal_reservation() {
    use semaprax::{
        cleanup_plan::{CleanupPlace, CleanupTransition, StorageId},
        hir::{self, LoopLatchKind},
    };
    let ast = semaprax::check(OWNED_BREAK, "owning-break-reservation-forged.spx").unwrap();
    let mut resolved = hir::resolve(&ast).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let loops = &function
        .cleanup_plan
        .lexical_control
        .as_ref()
        .unwrap()
        .loops;
    let owned: Vec<_> = loops
        .iter()
        .filter_map(|item| match &item.latch {
            LoopLatchKind::OwnedIteratorStep {
                assignment, step, ..
            } => Some((item.condition_entry, assignment.clone(), step.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(owned.len(), 1, "exact generated owning wrapper");
    let (entry, assignment, step) = &owned[0];
    let block = &mut function.cleanup_plan.blocks[entry.0 as usize];
    assert_eq!(block.id, *entry);
    assert!(!block
        .transitions
        .iter()
        .any(|transition| matches!(transition,
        CleanupTransition::ReserveRenewal { at, .. } if at == assignment)));
    // Retain source, identities, loop/exit certificates and every original
    // transition. Only the purported one-leaf authority is forged.
    block.transitions.insert(
        0,
        CleanupTransition::ReserveRenewal {
            at: assignment.clone(),
            binding: CleanupPlace {
                storage: StorageId::Value(step.clone()),
                projections: Vec::new(),
            },
        },
    );
    let failure = hir::validate(&resolved)
        .expect_err("conditional Step cannot borrow one-leaf renewal authority");
    assert_eq!(failure.code, "SPX-H006");
}

#[path = "lexical_exits/provisional_result.rs"]
mod provisional_result;
