//! Whole prior/current receivers; authored only, no application-fit claim.
use super::*;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};
use sha2::{Digest, Sha256};
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(super) fn before_selected() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
fn before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|v| v.set(self.0));
        }
    }
    let _restore = Restore(BEFORE.with(|v| v.replace(true)));
    body()
}
fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module direct.move; @id(\"main\") fn main()->i64 {0}",
            "direct-move.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn binding() -> ValidationBinding {
    ValidationBinding {
        contract_local: false,
        mutable: true,
        ty: ResolvedType::Bytes,
        ownership: OwnershipMode::Own,
        availability: Availability::Available,
        active_loans: BTreeSet::new(),
        moved_places: BTreeMap::new(),
        definitely_partial: BTreeSet::new(),
    }
}
fn seed(id: &ValueId) -> Result<scope_checkpoint::Scope, Diagnostic> {
    let mut scope = scope_checkpoint::Scope::default();
    let prepared = scope.prepare_insert()?;
    prepared.insert(id.clone(), binding())?;
    Ok(scope)
}
fn place(program: &ResolvedProgram, id: &ValueId) -> ResolvedExpr {
    let mut expression = program.functions[0].body.clone();
    expression.ty = ResolvedType::Bytes;
    expression.ownership = OwnershipMode::Own;
    expression.kind = ResolvedExprKind::Place(Place {
        root: id.clone(),
        projections: Vec::new(),
    });
    expression
}
fn complete(metrics: Metrics, paid: usize) {
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}
#[test]
fn direct_place_matches_complete_old_missing_states_projection_errors_and_failed_publication() {
    let program = program();
    let id = ValueId::new("direct.owner".to_owned());
    let validator = HirValidator::new(&program).unwrap();
    for state in [
        Availability::Available,
        Availability::Moved,
        Availability::MaybeMoved,
    ] {
        for malformed in [false, true] {
            for missing in [false, true] {
                let mut expression = place(&program, &id);
                if let ResolvedExprKind::Place(p) = &mut expression.kind {
                    if malformed {
                        p.projections
                            .push(PlaceProjection::Field(DeclarationId::new("missing")));
                    }
                    if missing {
                        p.root = ValueId::new("absent".to_owned());
                    }
                }
                let mut old = seed(&id).unwrap();
                old.get_mut(&id).unwrap().unwrap().availability = state;
                let mut current = old.clone();
                let expected = before(|| validator.mark_value_sources_moved(&expression, &mut old));
                let actual = validator.mark_value_sources_moved(&expression, &mut current);
                assert_eq!(actual, expected);
                assert_eq!(current, old, "full state and failed publication");
                if actual.is_err() {
                    assert_eq!(current.len(), 0);
                }
            }
        }
    }
    let mut expression = place(&program, &id);
    let ResolvedExprKind::Place(p) = &expression.kind else {
        unreachable!()
    };
    let mut validator = validator;
    validator.canonical_loans = canonical_loans::Lookup::Legacy(canonical_loans::Inventory {
        ids: BTreeMap::new(),
        liveness: BTreeMap::from([(
            (expression.id.clone(), LoanPointPhase::Before, LoanId(0)),
            0,
        )]),
        origins: vec![p.clone()],
    });
    let mut current = seed(&id).unwrap();
    current
        .get_mut(&id)
        .unwrap()
        .unwrap()
        .active_loans
        .insert(LoanId(0));
    let mut old = current.clone();
    let expected = before(|| validator.mark_value_sources_moved(&expression, &mut old));
    let actual = validator.mark_value_sources_moved(&expression, &mut current);
    assert_eq!(actual, expected);
    assert!(actual.unwrap_err().message.contains("SPX-T265"));
    assert_eq!(current, old);
    assert_eq!(current.len(), 0);
    // Unproved, nonterminal shapes retain the entire original driver.
    expression.kind = ResolvedExprKind::Block {
        statements: Vec::new(),
        tail: Box::new(place(&program, &id)),
    };
    let mut old = seed(&id).unwrap();
    let mut current = old.clone();
    assert_eq!(
        validator.mark_value_sources_moved(&expression, &mut current),
        before(|| validator.mark_value_sources_moved(&expression, &mut old))
    );
    assert_eq!(current, old);
}
fn receive(
    validator: &HirValidator<'_>,
    expression: &ResolvedExpr,
    id: &ValueId,
    count: usize,
    old: bool,
) -> Result<(), Diagnostic> {
    for _ in 0..count {
        let mut scope = seed(id)?;
        if old {
            before(|| validator.mark_value_sources_moved(expression, &mut scope))?;
        } else {
            validator.mark_value_sources_moved(expression, &mut scope)?;
        }
        assert_eq!(scope.get(id).unwrap().availability, Availability::Moved);
        drop(scope);
    }
    Ok(())
}
#[test]
fn exact_direct_receivers_keep_single_and_repeated_real_allocation_invoice_and_drop_gains() {
    let program = program();
    let id = ValueId::new("direct.owner".to_owned());
    let expression = place(&program, &id);
    let validator = HirValidator::new(&program).unwrap();
    for count in [1, 32, 256] {
        let mut captures = Vec::new();
        for old in [true, false] {
            let ((result, metrics), refused, paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| receive(&validator, &expression, &id, count, old))
                });
            result.unwrap();
            assert!(!refused && first.is_none());
            complete(metrics, paid);
            captures.push((metrics, paid));
        }
        eprintln!(
            "OPT939 direct count={count} old={:?} current={:?} full_drop=true",
            captures[0], captures[1]
        );
        assert!(captures[1].0.allocated < captures[0].0.allocated);
        assert!(captures[1].1 < captures[0].1);
        assert_eq!(
            captures[0].0.allocations - captures[1].0.allocations,
            count * 2
        );
    }
}
#[test]
fn direct_exact_short_zero_floor_sticky_and_unwind_keep_full_drop_and_immutable_program() {
    let program = program();
    let id = ValueId::new("direct.owner".to_owned());
    let expression = place(&program, &id);
    let validator = HirValidator::new(&program).unwrap();
    let (_, refused, exact, first) = with_limit_usage_refusal(usize::MAX, || {
        receive(&validator, &expression, &id, 1, false).unwrap()
    });
    assert!(!refused && first.is_none());
    for (limit, floor, success) in [
        (exact, 0, true),
        (exact - 1, 0, false),
        (0, 0, false),
        (exact, 1, false),
    ] {
        let (((), metrics), refused, _, first) = with_limit_usage_refusal(limit, || {
            if floor != 0 {
                assert!(bounded_output::set_active_floor(floor));
            }
            measure(|| {
                let result = receive(&validator, &expression, &id, 1, false);
                assert_eq!(result.is_ok(), success);
                if result.is_err() {
                    let again = receive(&validator, &expression, &id, 1, false);
                    assert_eq!(again, result, "complete sticky diagnostic");
                    drop(again);
                }
                drop(result);
            })
        });
        assert_eq!(refused, !success);
        assert_eq!(first.is_none(), success);
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    for old in [true, false] {
        let (_, metrics) = measure(|| {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut scope = seed(&id).unwrap();
                if old {
                    before(|| validator.mark_value_sources_moved(&expression, &mut scope)).unwrap();
                } else {
                    validator
                        .mark_value_sources_moved(&expression, &mut scope)
                        .unwrap();
                }
                panic!("source-owned unwind after actual root return");
            }));
            assert!(caught.is_err());
            drop(caught);
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}
#[test]
fn full_many_owned_argument_hir_caller_has_strict_old_current_net_and_error_parity() {
    let mut source = String::from("module direct.caller; @id(\"consume\") fn consume(");
    for i in 0..64 {
        if i != 0 {
            source.push(',');
        }
        source.push_str(&format!("p{i}:own Bytes"));
    }
    source.push_str(")->usize {byte_len(bytes_as_slice(p0))} @id(\"exercise\") fn exercise(");
    for i in 0..64 {
        if i != 0 {
            source.push(',');
        }
        source.push_str(&format!("p{i}:own Bytes"));
    }
    source.push_str(")->usize {consume(");
    for i in 0..64 {
        if i != 0 {
            source.push(',');
        }
        source.push_str(&format!("p{i}"));
    }
    source.push_str(")} @id(\"main\") fn main()->i64 {0}");
    let ast = crate::check(&source, "direct-caller.spx").unwrap();
    let program = before(|| crate::hir::resolve(&ast)).unwrap();
    let mut captures = Vec::new();
    for old in [true, false] {
        let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let result = if old {
                    before(|| crate::hir::validate(&program))
                } else {
                    crate::hir::validate(&program)
                };
                assert!(result.is_ok());
                drop(result);
            })
        });
        assert!(!refused && first.is_none());
        complete(metrics, paid);
        captures.push((metrics, paid));
    }
    eprintln!(
        "OPT939 full64ownedargs old={:?} current={:?}",
        captures[0], captures[1]
    );
    assert!(captures[1].0.allocated < captures[0].0.allocated);
    assert!(captures[1].1 < captures[0].1);
    let mut bad = program.clone();
    bad.entrypoint = DeclarationId::new("absent");
    assert_eq!(
        crate::hir::validate(&bad),
        before(|| crate::hir::validate(&bad))
    );
}
#[test]
fn inert_tiny_root_has_no_new_owner_bank_or_cold_invoice_loss() {
    let program = program();
    let id = ValueId::new("direct.owner".to_owned());
    let expression = &program.functions[0].body;
    let validator = HirValidator::new(&program).unwrap();
    let mut receipts = Vec::new();
    for old in [true, false] {
        let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut scope = seed(&id).unwrap();
                if old {
                    before(|| validator.mark_value_sources_moved(expression, &mut scope)).unwrap();
                } else {
                    validator
                        .mark_value_sources_moved(expression, &mut scope)
                        .unwrap();
                }
                assert_eq!(
                    scope.get(&id).unwrap().availability,
                    Availability::Available
                );
                drop(scope);
            })
        });
        assert!(!refused && first.is_none());
        complete(metrics, paid);
        receipts.push((metrics, paid));
    }
    assert_eq!(receipts[0], receipts[1]);
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
/// Explicit old-stage acquisition for the original inert-source oracle. This
/// verifies full current root/children and every whole-source inverse first.
pub(super) fn before_source() -> String {
    let metadata_bytes = include_str!("direct_source_inverse.json");
    let physical_metadata = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/hir/validation/scope_checkpoint/marks/direct_source_inverse.json"),
    )
    .unwrap();
    assert_eq!(physical_metadata, metadata_bytes.as_bytes());
    let metadata: serde_json::Value = serde_json::from_str(metadata_bytes).unwrap();
    let mut original = None;
    for (path, compiled) in [
        (
            "src/hir/validation/scope_checkpoint/marks.rs",
            include_str!("../marks.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/marks/direct_before.rs",
            include_str!("direct_before.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/marks/direct_tests.rs",
            include_str!("direct_tests.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/marks/tests.rs",
            include_str!("tests.rs"),
        ),
    ] {
        let physical =
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(physical, compiled.as_bytes());
        if let Some(row) = metadata["records"].get(path) {
            assert_eq!(
                physical.len(),
                row["current_length"].as_u64().unwrap() as usize
            );
            assert_eq!(sha(&physical), row["current_sha256"].as_str().unwrap());
            let mut restored = compiled.to_owned();
            for e in row["edits"].as_array().unwrap().iter().rev() {
                let a = e["start"].as_u64().unwrap() as usize;
                let b = e["end"].as_u64().unwrap() as usize;
                assert_eq!(&restored[a..b], e["after"].as_str().unwrap());
                restored.replace_range(a..b, e["before"].as_str().unwrap());
            }
            assert_eq!(restored, row["before_text"].as_str().unwrap());
            if path.ends_with("/marks.rs") {
                original = Some(restored);
            }
        }
        let late =
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(late, physical, "fresh whole physical source after inverse");
    }
    let original = original.unwrap();
    let executable = include_str!("direct_before.rs")
        .replacen("use super::super::super::*;", "use super::super::*;", 1)
        .replacen(
            "fn mark_value_sources_moved_before_direct(",
            "fn mark_value_sources_moved(",
            1,
        )
        .replacen(
            "if super::tests::before_selected()",
            "if tests::before_selected()",
            1,
        )
        + "\n#[cfg(test)]\nmod before;\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(
        executable, original,
        "complete independently executable immediate source"
    );
    original
}
#[test]
fn whole_immediate_walker_payer_and_all_current_children_remain_source_bound() {
    let original = before_source();
    let before = include_str!("direct_before.rs")
        .replacen("use super::super::super::*;", "use super::super::*;", 1)
        .replacen(
            "fn mark_value_sources_moved_before_direct(",
            "fn mark_value_sources_moved(",
            1,
        )
        .replacen(
            "if super::tests::before_selected()",
            "if tests::before_selected()",
            1,
        )
        + "\n#[cfg(test)]\nmod before;\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(before, original);
    assert!(original.contains("let mut scopes = scope_checkpoint::Storage::new();"));
    assert!(original.contains("let mut frames = scope_checkpoint::Storage::new();"));
    let current = include_str!("../marks.rs");
    let start = "        enum Frame<'a> {";
    let end = "\n// One real primitive site,";
    assert_eq!(
        original
            .split_once(start)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0,
        current
            .split_once(start)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0,
        "entire unproved-shape DFS and every original growth/payer byte"
    );
}
