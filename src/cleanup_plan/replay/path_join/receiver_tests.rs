//! Complete pre-change receiver and current receiver over the same real HIR.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"
module boxed;
@id("f") fn f(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let view=bytes_as_slice(scratch);
    if flag && byte_len(view)==3usize {7}else{9}
}
@id("main") fn main()->i64 {f(false)}
"#;

fn program() -> ResolvedProgram {
    let checked = crate::check(SOURCE, "boxed-path-states.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let reparsed = crate::check(&canonical, "boxed-path-states.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    crate::hir::resolve(&reparsed).unwrap()
}

fn replay(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    original: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    if original {
        before::validate_path_states(program, function, &storage, &leaves, budget, None, None)
    } else {
        validate_path_states(program, function, &storage, &leaves, budget, None, None)
    }
}

#[test]
fn whole_original_and_boxed_replay_keep_success_work_and_hostile_diagnostic() {
    let program = program();
    for function in &program.functions {
        let plan = function.cleanup_plan.clone();
        let mut original = ReplayBudget::new();
        let mut current = ReplayBudget::new();
        replay(&program, function, true, &mut original).unwrap();
        replay(&program, function, false, &mut current).unwrap();
        assert_eq!(current.remaining, original.remaining);
        assert_eq!(current.skeleton_remaining, original.skeleton_remaining);
        assert_eq!(function.cleanup_plan, plan);
    }
    let function = program.functions.iter().find(|f| f.name == "f").unwrap();
    let mut forged = function.clone();
    let block = forged
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
        })
        .expect("the owned allocation must initialize its storage");
    let index = block
        .transitions
        .iter()
        .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
        .unwrap();
    block
        .transitions
        .insert(index + 1, block.transitions[index].clone());
    let plan = forged.cleanup_plan.clone();
    let mut original = ReplayBudget::new();
    let mut current = ReplayBudget::new();
    let expected = replay(&program, &forged, true, &mut original).unwrap_err();
    let actual = replay(&program, &forged, false, &mut current).unwrap_err();
    assert_eq!(expected.code, "SPX-H006");
    assert_eq!(actual, expected);
    assert_eq!(current.remaining, original.remaining);
    assert_eq!(forged.cleanup_plan, plan);
}

#[test]
fn complete_current_replay_has_exact_and_one_short_physical_limits_with_last_drop() {
    let program = program();
    let function = program.functions.iter().find(|f| f.name == "f").unwrap();
    let observe = |original, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = replay(&program, function, original, &mut budget);
                let success = result.is_ok();
                if let Err(error) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert!(crate::bounded_output::active_refused());
                }
                drop(result);
                drop(budget);
                success
            })
        })
    };
    let ((success, allocation), refused, old_paid, first) = observe(true, usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= old_paid);
    let ((success, allocation), refused, exact, first) = observe(false, usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    assert!(exact < old_paid);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = observe(false, limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(!success && refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn complete_original_receiver_and_exact_current_source_inverse_stay_bound() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-path-states-v1/path_join.before.rs.txt"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "bcabe39f9158261ce682392252b437ea2e8d23cec3c68cd379a2d18ce50aad69"
    );
    // Only module registrations change: the old body uses the same unchanged
    // branch-normalization and storage dependencies as the current receiver.
    let expected = original
        .replacen("mod branch_join;\n", "", 1)
        .replacen("pub(super) mod storage;\n", "", 1)
        .replacen("#[cfg(test)]\nmod tests;\n\n", "", 1);
    assert_eq!(include_str!("before.rs"), expected);
    let changes: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-path-states-v1/inverse.json"
    )))
    .unwrap();
    let mut restored = include_str!("../path_join.rs").to_owned();
    for change in changes.as_array().unwrap().iter().rev() {
        let start = change[0].as_u64().unwrap() as usize;
        let current = change[1].as_str().unwrap();
        let old = change[2].as_str().unwrap();
        assert_eq!(restored.get(start..start + current.len()), Some(current));
        restored.replace_range(start..start + current.len(), old);
    }
    assert_eq!(restored, original);

    let old_bundle = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-path-states-v1/bundle.before.json"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old_bundle.as_bytes()))
        ),
        "6a95041037b52f3b75abbffb2f5b3c3877e871ef8d7d66a01cba9e01e0eb3580"
    );
    let receipt: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-path-states-v1/catalog-receipt.json"
    )))
    .unwrap();
    let mut bundle = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/AGENT-SKILL-BUNDLE-V1.json"
    ))
    .to_owned();
    for (new, old) in [
        ("new_catalog", "old_catalog"),
        ("new_envelope", "old_envelope"),
    ] {
        let new = receipt[new].as_str().unwrap();
        let old = receipt[old].as_str().unwrap();
        assert_eq!(bundle.matches(new).count(), 1);
        bundle = bundle.replacen(new, old, 1);
    }
    assert_eq!(bundle, old_bundle);
}
