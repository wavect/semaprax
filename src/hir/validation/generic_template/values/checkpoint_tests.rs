//! Independent full map policies, transactional publication and real lifetime owners.
use super::*;
use crate::test_allocation_observer::measure;

#[derive(Clone, Copy)]
enum Policy {
    Current,
    BorrowedMap,
    OwnedMap,
}
fn inputs(count: usize) -> (Vec<ValueId>, Vec<ResolvedType>) {
    let ids = (0..count)
        .map(|i| ValueId::new(format!("generic.value.{i:04}:é\0")))
        .collect();
    let types = (0..count)
        .map(|i| ResolvedType::Function {
            parameters: vec![ResolvedType::Nominal {
                declaration: DeclarationId::new("checkpoint.nominal"),
                arguments: vec![ResolvedType::String, ResolvedType::Usize],
            }],
            result: Box::new(if i % 2 == 0 {
                ResolvedType::Bool
            } else {
                ResolvedType::I64
            }),
        })
        .collect();
    (ids, types)
}
fn populated<'a>(
    policy: Policy,
    ids: &[ValueId],
    types: &'a [ResolvedType],
    order: &[usize],
) -> Values<'a> {
    let mut scope = match policy {
        Policy::Current => Values::new().unwrap(),
        Policy::BorrowedMap => Values::borrowed_reference(),
        Policy::OwnedMap => Values::owned(),
    };
    for &i in order {
        assert!(!scope.insert(ids[i].clone(), &types[i]).unwrap());
    }
    scope
}
#[test]
fn ordered_checkpoints_preserve_both_complete_old_maps_through_rotations_and_replacements() {
    for count in [0, 1, 2, 17, 128] {
        let (ids, types) = inputs(count);
        let ascending = (0..count).collect::<Vec<_>>();
        let descending = ascending.iter().copied().rev().collect::<Vec<_>>();
        for order in [ascending, descending] {
            let mut current = populated(Policy::Current, &ids, &types, &order);
            let mut borrowed = populated(Policy::BorrowedMap, &ids, &types, &order);
            let mut owned = populated(Policy::OwnedMap, &ids, &types, &order);
            assert_eq!(current.ordered_entries(), borrowed.ordered_entries());
            assert_eq!(current.ordered_entries(), owned.ordered_entries());
            let old_root = current.checkpoint().unwrap();
            let saved = current.ordered_entries();
            for step in 0..count * 3 {
                let i = step % count;
                let next = (i + 1) % count;
                assert!(current.insert(ids[i].clone(), &types[next]).unwrap());
                assert!(borrowed.insert(ids[i].clone(), &types[next]).unwrap());
                assert!(owned.insert(ids[i].clone(), &types[next]).unwrap());
                assert_eq!(current.ordered_entries(), borrowed.ordered_entries());
                assert_eq!(current.ordered_entries(), owned.ordered_entries());
                assert_eq!(old_root.ordered_entries(), saved);
            }
            for scope in [&current, &borrowed, &owned] {
                let empty = scope.empty_like().unwrap();
                assert!(empty.ordered_entries().is_empty());
                assert!(empty.get(&ValueId::new("absent".into())).is_none());
                assert!(empty.root(&ValueId::new("absent".into())).is_none());
            }
        }
    }
    let (ids, types) = inputs(3);
    // Minimal LL/RR/LR/RL insertions exercise each unique and shared rotation.
    for order in [vec![0, 1, 2], vec![2, 1, 0], vec![2, 0, 1], vec![0, 2, 1]] {
        for shared in [false, true] {
            let mut current = Values::new().unwrap();
            let mut old = Values::borrowed_reference();
            let mut snapshots = Vec::new();
            for &i in &order {
                if shared {
                    snapshots.push(current.checkpoint().unwrap());
                }
                assert_eq!(
                    current.insert(ids[i].clone(), &types[i]),
                    old.insert(ids[i].clone(), &types[i])
                );
                assert_eq!(current.ordered_entries(), old.ordered_entries());
            }
            drop(snapshots);
        }
    }
}
#[test]
fn legacy_borrowed_snapshot_requests_are_removed_independently_of_owned_type_payloads() {
    let (ids, types) = inputs(128);
    let order = (0..ids.len()).collect::<Vec<_>>();
    let old = populated(Policy::BorrowedMap, &ids, &types, &order);
    let current = populated(Policy::Current, &ids, &types, &order);
    let (_, one) = measure(|| drop(old.checkpoint().unwrap()));
    let (_, before) = measure(|| {
        for _ in 0..64 {
            let snapshot = old.checkpoint().unwrap();
            assert_eq!(snapshot.get(&ids[0]), Some(&types[0]));
            assert!(matches!(snapshot.root(&ids[0]), Some(Cow::Borrowed(_))));
            drop(snapshot);
        }
    });
    let (((), after), refused, paid) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        measure(|| {
            for _ in 0..64 {
                let snapshot = current.checkpoint().unwrap();
                assert!(std::ptr::eq(snapshot.get(&ids[0]).unwrap(), &types[0]));
                assert!(matches!(snapshot.root(&ids[0]), Some(Cow::Borrowed(_))));
                drop(snapshot);
            }
        })
    });
    assert!(!refused);
    assert!(one.allocated > 0 && one.allocations > 0);
    assert_eq!(before.allocated, one.allocated * 64);
    assert_eq!(before.allocations, one.allocations * 64);
    assert_eq!(
        (after.allocated, after.allocations, after.highwater),
        (0, 0, 0)
    );
    assert_eq!(paid, 64 * std::mem::size_of::<Values<'_>>());
    for observation in [one, before, after] {
        assert!(!observation.invalid && observation.live == 0);
        assert_eq!(observation.allocated, observation.freed);
    }
}
// Whole environment receiver: setup, inserts, shared edits, reads and all Drop.
// The held caller identities/types are identical immutable inputs in all policies.
fn receiver(
    policy: Policy,
    ids: &[ValueId],
    types: &[ResolvedType],
    forks: usize,
) -> Result<(), Diagnostic> {
    let mut scope = match policy {
        Policy::Current => Values::new()?,
        Policy::BorrowedMap => Values::borrowed_reference(),
        Policy::OwnedMap => Values::owned(),
    };
    for (id, ty) in ids.iter().zip(types) {
        assert!(!scope.insert(id.clone(), ty)?);
    }
    for i in 0..forks {
        let mut branch = scope.checkpoint()?;
        if !ids.is_empty() {
            let key = i % ids.len();
            let other = (key + 1) % ids.len();
            assert!(branch.insert(ids[key].clone(), &types[other])?);
            assert_eq!(branch.get(&ids[key]), Some(&types[other]));
            assert_eq!(scope.get(&ids[key]), Some(&types[key]));
            assert_eq!(branch.root(&ids[key]).unwrap().as_ref(), &types[other]);
        }
        drop(branch);
    }
    for (id, ty) in ids.iter().zip(types) {
        assert_eq!(scope.get(id), Some(ty));
    }
    drop(scope);
    Ok(())
}
#[test]
fn whole_insertion_and_snapshot_receivers_report_actual_cost_without_owned_payload_conflation() {
    for count in [0, 1, 2, 17, 128] {
        let (ids, types) = inputs(count);
        for forks in [0, 64] {
            let observe = |policy| {
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    measure(|| {
                        let result = receiver(policy, &ids, &types, forks);
                        assert!(result.is_ok());
                        drop(result);
                    })
                })
            };
            let (((), borrowed), old_refused, old_paid) = observe(Policy::BorrowedMap);
            let (((), owned), owned_refused, owned_paid) = observe(Policy::OwnedMap);
            let (((), current), refused, paid) = observe(Policy::Current);
            assert!(!old_refused && !owned_refused && !refused);
            assert_eq!(
                (old_paid, owned_paid),
                (0, 0),
                "complete old directories were uncharged"
            );
            assert!(current.allocated <= paid && current.highwater <= paid);
            for observation in [borrowed, owned, current] {
                assert!(!observation.invalid && observation.live == 0);
                assert_eq!(observation.allocated, observation.freed);
            }
            // Insertion-heavy cost is retained even if current exceeds the old
            // BTreeMap: no discount, normalization or savings assertion hides it.
            println!(
                "generic_values count={count} forks={forks} borrowed_bytes={} borrowed_requests={} owned_bytes={} current_bytes={} current_requests={} current_highwater={} paid={paid}",
                borrowed.allocated,
                borrowed.allocations,
                owned.allocated,
                current.allocated,
                current.allocations,
                current.highwater
            );
        }
    }
}
#[test]
fn whole_current_receiver_exact_short_and_sticky_refusal_settle_all_storage() {
    let (ids, types) = inputs(17);
    let observe = |limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = receiver(Policy::Current, &ids, &types, 64);
                let accepted = result.is_ok();
                if !accepted {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                accepted
            })
        })
    };
    let ((ok, wide), refused, paid, _) = observe(usize::MAX);
    assert!(ok && !refused && paid > 0 && wide.allocated <= paid && wide.highwater <= paid);
    let ((ok, exact), refused, actual, _) = observe(paid);
    assert!(ok && !refused && actual == paid && exact.allocated <= paid && exact.highwater <= paid);
    let ((ok, short), refused, _, receipt) = observe(paid - 1);
    assert!(!ok && refused && receipt.is_some());
    for observation in [wide, exact, short] {
        assert!(!observation.invalid && observation.live == 0);
        assert_eq!(observation.allocated, observation.freed);
    }
}
#[test]
fn failed_unique_and_shared_publication_retains_full_prior_map_at_each_paid_node_boundary() {
    let (ids, types) = inputs(17);
    let extra = ValueId::new("generic.value.extra".into());
    let observe = |shared, limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            let mut scope = Values::new().unwrap();
            for (id, ty) in ids.iter().zip(&types) {
                scope.insert(id.clone(), ty).unwrap();
            }
            let held = if shared {
                Some(scope.checkpoint().unwrap())
            } else {
                None
            };
            let before = scope.ordered_entries();
            let result = scope.insert(extra.clone(), &types[0]);
            let accepted = result.is_ok();
            if !accepted {
                assert_eq!(scope.ordered_entries(), before);
                assert!(!crate::bounded_output::reserve_active_required(0));
                assert!(scope.insert(extra.clone(), &types[0]).is_err());
                assert_eq!(scope.ordered_entries(), before);
            }
            if let Some(held) = held {
                assert_eq!(held.ordered_entries(), before);
            }
            let remaining = crate::bounded_output::active_remaining().unwrap();
            drop(result);
            assert_eq!(crate::bounded_output::active_remaining(), Some(remaining));
            (accepted, remaining)
        })
    };
    // Semantic projection allocations here are outside the storage lifetime
    // witness and receive no funding claim. Both path policies retain the old
    // complete map when any leaf/path reservation is refused.
    for shared in [false, true] {
        let (setup_result, setup_refused, setup, _) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                let mut scope = Values::new().unwrap();
                for (id, ty) in ids.iter().zip(&types) {
                    scope.insert(id.clone(), ty).unwrap();
                }
                if shared {
                    drop(scope.checkpoint().unwrap());
                }
            });
        let () = setup_result;
        assert!(!setup_refused);
        let ((ok, remaining), refused, total, _) = observe(shared, usize::MAX);
        assert_eq!(total, usize::MAX - remaining);
        assert!(ok && !refused && total > setup);
        let mut paid_intermediate = false;
        for limit in setup..total {
            let ((ok, remaining), refused, used, receipt) = observe(shared, limit);
            assert!(!ok && refused && receipt.is_some());
            assert_eq!(used, limit - remaining);
            assert!(used >= setup && used <= limit);
            paid_intermediate |= used > setup;
        }
        if shared {
            assert!(
                paid_intermediate,
                "a refused parent retains its paid child debit"
            );
        }
        let ((ok, remaining), refused, actual, _) = observe(shared, total);
        assert_eq!(actual, total - remaining);
        assert!(ok && !refused && actual == total);
    }
}
#[test]
fn sibling_contract_domains_and_unwind_leave_complete_checkpoint_roots_isolated() {
    let (ids, types) = inputs(17);
    let parent = populated(Policy::Current, &ids, &types, &(0..17).collect::<Vec<_>>());
    let before = parent.ordered_entries();
    let mut sibling = parent.checkpoint().unwrap();
    sibling.insert(ids[0].clone(), &types[1]).unwrap();
    let mut contract = parent.empty_like().unwrap();
    contract.insert(ids[0].clone(), &types[2]).unwrap();
    assert_eq!(
        contract.ordered_entries(),
        vec![(ids[0].clone(), types[2].clone())]
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut branch = parent.checkpoint().unwrap();
        branch.insert(ids[1].clone(), &types[2]).unwrap();
        panic!("late generic checkpoint unwind");
    }));
    assert!(panic.is_err());
    assert_eq!(parent.ordered_entries(), before);
    assert_eq!(sibling.get(&ids[0]), Some(&types[1]));
    assert_eq!(contract.get(&ids[0]), Some(&types[2]));
}

fn source_constant<'a>(text: &'a str, declaration: &str) -> &'a str {
    text.split_once(declaration)
        .expect("retained source constant")
        .1
        .split_once("\"#;")
        .expect("retained raw literal terminator")
        .0
}
fn checked(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "generic-checkpoint-full.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let again = crate::check(&canonical, "generic-checkpoint-canonical.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&again));
    let program = crate::hir::resolve(&again).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn full_walk(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    policy: Policy,
) -> (
    Result<(), Diagnostic>,
    BTreeSet<ExpressionId>,
    BTreeSet<ValueId>,
) {
    let mut validator = HirValidator::new(program).unwrap();
    if !matches!(policy, Policy::Current) {
        validator.use_legacy_identities();
    }
    let execution = FunctionExecutionId::Monomorphic(template.id.clone());
    let scope = match policy {
        Policy::Current => Values::new().unwrap(),
        Policy::BorrowedMap => Values::borrowed_reference(),
        Policy::OwnedMap => Values::owned(),
    };
    let result = validator.validate_template_expressions_with_values(template, &execution, scope);
    (
        result,
        validator.expression_ids.into_btree_projection(),
        validator.value_ids.into_btree_projection(),
    )
}
#[test]
fn full_original_symbolic_walks_preserve_contract_closure_nested_and_late_error_diagnostics() {
    // Reuse the exact retained original application/template fixtures, including
    // the complete callback map/filter/fold body and nested owned reconstruction.
    let old_tests = include_str!("tests.rs");
    let nested = source_constant(old_tests, "const SOURCE: &str = r#\"");
    let language = include_str!("../../../../../tests/language/function_values_generic.rs");
    let adapters = source_constant(language, "pub const ADAPTERS: &str = r#\"");
    let closure = source_constant(language, "const GENERIC_TEMPLATE_CLOSURE: &str = r#\"");
    let contract = "module checkpoint.contract; @id(\"c.take\") fn take<T>(value:own Box<T>,allowed:bool)->T requires allowed && true ensures true {box_into_inner<T>(value)} @id(\"c.main\") fn main()->i64 {0}";
    for source in [nested, adapters, closure, contract] {
        let program = checked(source);
        for template in &program.function_templates {
            let before = full_walk(&program, template, Policy::BorrowedMap);
            let owned = full_walk(&program, template, Policy::OwnedMap);
            let current = full_walk(&program, template, Policy::Current);
            assert!(before.0.is_ok());
            assert_eq!(before, owned);
            assert_eq!(before, current);
            // A genuine late body error follows all earlier block bindings.
            let mut hostile = template.clone();
            let original_id = hostile.body.id.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut hostile.body.kind else {
                panic!("retained block body")
            };
            tail.id = original_id;
            let before = full_walk(&program, &hostile, Policy::BorrowedMap);
            let owned = full_walk(&program, &hostile, Policy::OwnedMap);
            let current = full_walk(&program, &hostile, Policy::Current);
            assert!(before.0.is_err());
            assert_eq!(before, owned);
            assert_eq!(before, current);
            if template.params.len() > 1 {
                let mut duplicate = template.clone();
                duplicate.params[1].id = duplicate.params[0].id.clone();
                let before = full_walk(&program, &duplicate, Policy::BorrowedMap);
                assert!(before.0.is_err());
                assert_eq!(before, full_walk(&program, &duplicate, Policy::OwnedMap));
                assert_eq!(before, full_walk(&program, &duplicate, Policy::Current));
            }
        }
    }
}
#[test]
fn nested_generic_block_depth_preserves_full_old_identity_state_and_late_error() {
    fn run() {
        for depth in [1, 17, 64] {
            let mut body = "value".to_owned();
            for _ in 0..depth {
                body = format!("{{{body}}}");
            }
            let source = format!(
            "module checkpoint.depth; @id(\"c.local\") fn local<T>(value:T)->T {{{body}}} @id(\"c.main\") fn main()->i64 {{0}}"
        );
            let program = checked(&source);
            let template = &program.function_templates[0];
            let before = full_walk(&program, template, Policy::BorrowedMap);
            assert!(before.0.is_ok());
            assert_eq!(before, full_walk(&program, template, Policy::OwnedMap));
            assert_eq!(before, full_walk(&program, template, Policy::Current));
            let mut hostile = template.clone();
            let wrong_id = hostile.body.id.clone();
            spoil_deep_leaf(&mut hostile.body, wrong_id);
            let before = full_walk(&program, &hostile, Policy::BorrowedMap);
            assert!(before.0.is_err());
            assert_eq!(before, full_walk(&program, &hostile, Policy::OwnedMap));
            assert_eq!(before, full_walk(&program, &hostile, Policy::Current));
        }
    }
    // Complete reference walkers use the existing bounded oracle-test stack
    // convention. This changes no production depth or runtime stack contract.
    std::thread::Builder::new()
        .name("generic-checkpoint-depth-oracle".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn unique_replacement_has_zero_requests_but_sticky_refusal_blocks_unique_and_shared_publication() {
    let (ids, types) = inputs(17);
    let (_, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut scope = populated(Policy::Current, &ids, &types, &(0..17).collect::<Vec<_>>());
        let ((replaced, observed), refused, debit) =
            crate::bounded_output::with_limit_usage(0, || {
                measure(|| scope.insert(ids[0].clone(), &types[1]).unwrap())
            });
        assert!(replaced && !refused && debit == 0);
        assert_eq!(
            (observed.allocated, observed.allocations, observed.live),
            (0, 0, 0)
        );
        let before = scope.ordered_entries();
        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
        let result = scope.insert(ids[0].clone(), &types[2]);
        assert!(result.is_err());
        drop(result);
        assert_eq!(scope.ordered_entries(), before);
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(refused);
    let (_, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut scope = populated(Policy::Current, &ids, &types, &(0..17).collect::<Vec<_>>());
        let held = scope.checkpoint().unwrap();
        let before = scope.ordered_entries();
        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
        let result = scope.insert(ids[0].clone(), &types[1]);
        assert!(result.is_err());
        drop(result);
        assert_eq!(scope.ordered_entries(), before);
        assert_eq!(held.ordered_entries(), before);
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(refused);
}
fn spoil_deep_leaf(expression: &mut ResolvedExpr, identity: ExpressionId) {
    match &mut expression.kind {
        ResolvedExprKind::Block { tail, .. } => spoil_deep_leaf(tail, identity),
        _ => expression.id = identity,
    }
}
