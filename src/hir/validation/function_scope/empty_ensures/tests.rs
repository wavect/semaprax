use super::*;
use crate::test_allocation_observer::measure;

const SCALAR: &str = "module empty.post; @id(\"main\") fn main()->i64 {0}";
const OWNED: &str = "module empty.post; @id(\"make\") fn make()->Bytes {bytes_zeroed(1usize)} @id(\"main\") fn main()->i64 {0}";
const NONEMPTY: &str = "module empty.post; @id(\"make\") fn make()->Bytes ensures byte_len(bytes_as_slice(result))==1usize {bytes_zeroed(1usize)} @id(\"main\") fn main()->i64 {0}";

fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "empty-postcondition.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn compare(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut current = HirValidator::new(program).unwrap();
    let mut old = current.clone();
    let expected = old.validate_function_before_empty_ensures(function, &execution);
    let actual = current.validate_function(function, &execution);
    HirValidator::assert_validation_oracle(
        &actual,
        &expected,
        &current,
        &old,
        &BTreeMap::new(),
        &BTreeMap::new(),
        function.id.as_str(),
    );
    HirValidator::assert_scope_auxiliary_oracle(&current, &old);
    actual
}

#[test]
fn empty_and_nonempty_postconditions_keep_complete_prior_function_receiver() {
    for source in [
        SCALAR,
        OWNED,
        NONEMPTY,
        crate::hir::iterative_validator_tests::SOURCE,
    ] {
        let program = program(source);
        for function in &program.functions {
            compare(&program, function).unwrap();
        }
    }
}

#[test]
fn empty_postconditions_keep_late_identity_body_type_and_ownership_errors() {
    let owned_program = program(OWNED);
    let original = owned_program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "make")
        .unwrap();
    assert!(original.ensures.is_empty());
    for mutation in 0..4 {
        let mut function = original.clone();
        match mutation {
            0 => function.result_id = ValueId::new("forged.result".to_owned()),
            1 => function.body.ty = ResolvedType::I64,
            2 => function.body.ownership = OwnershipMode::Borrow,
            _ => function.return_type = ResolvedType::Unit,
        }
        compare(&owned_program, &function).unwrap_err();
    }
    let program = program(NONEMPTY);
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "make")
        .unwrap()
        .clone();
    function.ensures[0].ty = ResolvedType::I64;
    compare(&program, &function).unwrap_err();
}

fn lifetime(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
    old: bool,
) -> (
    bool,
    crate::test_allocation_observer::Metrics,
    bool,
    usize,
    bool,
) {
    // Hold identical constructor custody first; the entire function receiver,
    // result-scope construction and their receiving Drop stay in the observer.
    let mut validator = HirValidator::new(program).unwrap();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let ((success, observed), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if old {
                    validator.validate_function_before_empty_ensures(function, &execution)
                } else {
                    validator.validate_function(function, &execution)
                };
                let success = result.is_ok();
                if !success {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                drop(validator);
                success
            })
        });
    (success, observed, refused, paid, first.is_some())
}

#[test]
fn unused_result_carrier_is_removed_with_exact_short_zero_and_complete_drop() {
    for source in [SCALAR, OWNED, NONEMPTY] {
        let program = program(source);
        for function in &program.functions {
            let (success, actual, refused, paid, receipt) =
                lifetime(&program, function, usize::MAX, false);
            let (old_success, old, old_refused, old_paid, old_receipt) =
                lifetime(&program, function, usize::MAX, true);
            assert!(success && old_success && !refused && !old_refused && !receipt && !old_receipt);
            assert!(!actual.invalid && !old.invalid);
            assert_eq!(actual.live, 0);
            assert_eq!(old.live, 0);
            assert_eq!(actual.allocated, actual.freed);
            assert_eq!(old.allocated, old.freed);
            assert!(actual.allocated <= paid && actual.highwater <= paid);
            if function.ensures.is_empty() {
                assert!(actual.allocated < old.allocated);
                assert!(actual.allocations < old.allocations);
                assert!(paid < old_paid);
            } else {
                assert_eq!(actual, old);
                assert_eq!(paid, old_paid);
            }
            assert!(paid > 0);
            let (success, exact, refused, exact_paid, receipt) =
                lifetime(&program, function, paid, false);
            assert!(success && !refused && !receipt && !exact.invalid);
            assert_eq!(exact, actual);
            assert_eq!(exact_paid, paid);
            for limit in [0, paid - 1] {
                let (success, short, refused, short_paid, receipt) =
                    lifetime(&program, function, limit, false);
                assert!(!success && refused && receipt && !short.invalid);
                assert_eq!(short.live, 0);
                assert_eq!(short.allocated, short.freed);
                assert!(short.allocated <= short_paid && short.highwater <= short_paid);
            }
        }
    }
}

#[test]
fn no_result_carrier_success_still_authenticates_a_sticky_payer() {
    let (((), observed), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(scope_checkpoint::charge::reserve(Some(1)).is_err());
                assert!(scope_checkpoint::charge::reserve(Some(0)).is_err());
                assert!(!crate::bounded_output::reserve_active_required(0));
            })
        });
    assert!(refused && first.is_some());
    assert_eq!(paid, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
}

#[test]
fn whole_current_function_is_exact_prior_source_plus_named_empty_guard() {
    use sha2::{Digest, Sha256};
    let binding: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../../../docs/evidence/empty-postcondition-scope/source-binding.json"
    ))
    .unwrap();
    let original = ci_str!(
        "../../../../../docs/evidence/empty-postcondition-scope/function_scope.rs.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original))
        ),
        binding["original"]["sha256"].as_str().unwrap()
    );
    let mut current = ci_str!("../../function_scope.rs").to_owned();
    for field in ["additive_guard", "registration"] {
        let addition = binding[field].as_str().unwrap();
        assert_eq!(current.matches(addition).count(), 1, "{field}");
        current = current.replacen(addition, "", 1);
    }
    assert_eq!(current, original);
    let saved = ci_str!("before.rs");
    let start = original.find("impl HirValidator").unwrap();
    let end = original.find("    fn validate_expr_persistent(").unwrap();
    let expected = format!("use super::*;\n{}}}\n", &original[start..end]);
    assert_eq!(
        saved.replacen(
            "fn validate_function_before_empty_ensures(",
            "fn validate_function(",
            1
        ),
        expected
    );
}
