use super::*;
const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "module-helper-index.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn module_index_reuses_typed_target_proofs_and_preserves_each_exact_site() {
    let program = program(SOURCE);
    let index = ModuleIndex::new(&program).unwrap();
    assert_eq!(
        index.proof_count, 2,
        "leaf and relay each receive one complete proof"
    );
    assert_eq!(index.sites.len(), program.functions.len());
    for function in &program.functions {
        let old = checked_bindings(&program, function).unwrap();
        assert_eq!(index.has_sites(function), !old.is_empty());
        for (at, binding) in old {
            assert_eq!(
                index.function_sites(function).unwrap().get(at),
                Some(binding)
            );
        }
    }
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("source main block")
    };
    let (binding, value) = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Assign { binding, value, .. } => Some((binding, value)),
            _ => None,
        })
        .unwrap();
    assert!(index.admits(binding, value));
    assert!(
        !index.admits(binding, &value.clone()),
        "copied identities are not this immutable phase's site authority"
    );
    let leaf = program
        .functions
        .iter()
        .find(|function| function.name == "leaf")
        .unwrap();
    let (proved, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || (0..1000).all(|_| index.target(leaf)));
    assert!(proved);
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert!(
        !index.target(&leaf.clone()),
        "copied callee cannot reuse held proof authority"
    );
    assert!(index.owned_capacity_bytes() > 0);
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
}
#[test]
fn module_index_exact_reservation_and_one_short_fail_before_publication() {
    let program = program(SOURCE);
    let (first, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || ModuleIndex::new(&program));
    assert!(first.is_ok());
    assert!(!overflowed);
    assert!(required > 0);
    let (exact, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || ModuleIndex::new(&program));
    assert!(exact.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (short, overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || ModuleIndex::new(&program));
    assert_eq!(short.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
}
#[test]
fn irrelevant_modules_have_no_new_backing_and_allocating_legacy_helpers_keep_old_profiles() {
    let plain = program("module index.plain; @id(\"main\") fn main()->i64 {0}");
    let (index, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || ModuleIndex::new_helpers(&plain));
    let index = index.unwrap();
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(index.owned_capacity_bytes(), 0);
    let mut expected = std::mem::size_of::<ModuleIndex<'_>>() + 86 * std::mem::size_of::<usize>();
    // Probes9 + directory(3+5+3+8) + numeric5 + iterator16
    // + vector11 + two leaf collectors26 = 86 fixed requested words.
    // Payload: directory5 + numeric2 + iterator4 + vector2 + leaf2.
    let mut payload = 15 * std::mem::size_of::<usize>();
    for count in crate::hir::renewal_probes::phase::current_map_counts(&plain.declarations) {
        expected += 3 * std::mem::size_of::<usize>();
        let mut capacity = 1;
        if count != 0 {
            loop {
                let bytes = capacity * 2 * std::mem::size_of::<usize>();
                payload += bytes;
                expected += bytes + 3 * std::mem::size_of::<usize>();
                if capacity >= count {
                    break;
                }
                capacity *= 2;
            }
        }
    }
    let ((ordinary, observed), overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            crate::test_allocation_observer::measure(|| {
                let index = ModuleIndex::new(&plain)?;
                let capacity = index.owned_capacity_bytes();
                drop(index);
                Ok::<_, Diagnostic>(capacity)
            })
        });
    assert_eq!(ordinary.unwrap(), 7 * std::mem::size_of::<usize>());
    assert!(!observed.invalid);
    assert_eq!(observed.allocated, payload);
    assert_eq!(observed.live, 0);
    assert_eq!(required, expected);
    let ordinary = ModuleIndex::new(&plain).unwrap();
    assert!(!overflowed);
    assert_eq!(
        ordinary.owned_capacity_bytes(),
        7 * std::mem::size_of::<usize>()
    );
    assert!(ordinary.sites.is_empty());
    assert!(ordinary.expressions.is_empty());
    assert!(ordinary.targets.is_empty());
    let (exact, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || ModuleIndex::new(&plain));
    assert!(exact.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (short, overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || ModuleIndex::new(&plain));
    assert!(short.is_err());
    assert!(overflowed);
    let (zero, overflowed, _) =
        crate::bounded_output::with_limit_usage(0, || ModuleIndex::new(&plain));
    assert!(zero.is_err());
    assert!(overflowed);
    let legacy = program(
        r#"module index.legacy;
@id("old") fn old(input:own Bytes)->Bytes { let length=byte_len(bytes_as_slice(input)); bytes_zeroed(1usize) }
@id("main") fn main()->i64 { let mut output=bytes_zeroed(1usize); let next=old(output); if byte_len(bytes_as_slice(next))==1usize {1}else{0} }
"#,
    );
    let rejected = crate::check(
        r#"module index.legacy;
@id("old") fn old(input:own Bytes)->Bytes { let length=byte_len(bytes_as_slice(input)); bytes_zeroed(1usize) }
@id("main") fn main()->i64 { let mut output=bytes_zeroed(1usize); output=old(output); 0 }
"#,
        "allocating-renewal.spx",
    ).unwrap_err();
    assert!(rejected.iter().any(|error| error.code == "SPX-U105"));
    let index = ModuleIndex::new(&legacy).unwrap();
    // Immutable owned transfer is ordinary old-profile code, so no renewal
    // candidate requires a closure proof in its assignment census.
    assert_eq!(index.proof_count, 0);
    assert!(legacy
        .functions
        .iter()
        .all(|function| !index.has_sites(function)));
    assert!(legacy.functions.iter().all(
        |function| function.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    ));
    crate::hir::validate(&legacy).unwrap();
    // Keep the original one-proof negative oracle on an actual hostile typed
    // assignment candidate. Its real allocating callee must grant no sites.
    let mut hostile = legacy.clone();
    let main = hostile
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("legacy main block")
    };
    let crate::hir::ResolvedStatement::Let { binding: owner, .. } = &statements[0] else {
        panic!("named output owner")
    };
    let owner = owner.clone();
    let crate::hir::ResolvedStatement::Let { value, span, .. } = statements[1].clone() else {
        panic!("ordinary allocating transfer")
    };
    statements[1] = crate::hir::ResolvedStatement::Assign {
        binding: owner,
        field: None,
        value,
        span,
    };
    let hostile_index = ModuleIndex::new(&hostile).unwrap();
    assert_eq!(hostile_index.proof_count, 1);
    assert!(hostile
        .functions
        .iter()
        .all(|function| !hostile_index.has_sites(function)));
    assert!(crate::hir::validate(&hostile).is_err());
}
#[test]
fn changed_contracts_cycles_and_missing_targets_cannot_reuse_prior_index_authority() {
    let mut program = program(SOURCE);
    {
        let original = ModuleIndex::new(&program).unwrap();
        assert!(original.sites.iter().any(|(_, sites)| !sites.is_empty()));
    }
    program
        .functions
        .iter_mut()
        .find(|function| function.name == "leaf")
        .unwrap()
        .requires[0]
        .kind = ResolvedExprKind::String("allocation".into());
    let changed = ModuleIndex::new(&program).unwrap();
    assert!(!changed.has_sites(
        program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
    ));
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
    // Parse the same authenticated source independently; prior attached plans
    // and identities cannot make a new transitive self-call nonallocating.
    let mut cyclic = self::program(SOURCE);
    let leaf = cyclic
        .functions
        .iter_mut()
        .find(|f| f.name == "leaf")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut leaf.body.kind else {
        panic!("source block")
    };
    let crate::hir::ResolvedStatement::Assign { value, .. } = &mut statements[1] else {
        panic!("source assignment")
    };
    let ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
        panic!("store call")
    };
    *callee = leaf.id.clone();
    let changed = ModuleIndex::new(&cyclic).unwrap();
    assert!(cyclic.functions.iter().all(|f| !changed.has_sites(f)));
    assert_eq!(crate::hir::validate(&cyclic).unwrap_err().code, "SPX-H006");
    let mut missing = self::program(SOURCE);
    missing.functions.retain(|function| function.name != "leaf");
    let changed = ModuleIndex::new(&missing).unwrap();
    assert!(missing
        .functions
        .iter()
        .all(|function| !changed.has_sites(function)));
    assert_eq!(crate::hir::validate(&missing).unwrap_err().code, "SPX-H006");
}

#[test]
fn sparse_many_function_queries_keep_exact_identity_and_logarithmic_zero_backing() {
    let mut source = SOURCE.to_owned();
    for i in 0..256 {
        source.push_str(&format!(
            "\n@id(\"irrelevant.{i}\") fn irrelevant_{i}()->i64 {{0}}\n"
        ));
    }
    let typed = program(&source);
    let index = ModuleIndex::new(&typed).unwrap();
    let leaf = typed
        .functions
        .iter()
        .find(|function| function.name == "leaf")
        .unwrap();
    for function in &typed.functions {
        let old = checked_bindings(&typed, function).unwrap();
        assert_eq!(index.has_sites(function), !old.is_empty());
        assert_eq!(index.function_sites(function).unwrap().len(), old.len());
        assert!(index.function_sites(&function.clone()).is_none());
    }
    let before = index.query_probes.get();
    let (accepted, overflowed, bytes) = crate::bounded_output::with_limit_usage(0, || {
        (0..1000).all(|_| index.target(leaf) && index.function_sites(leaf).is_some())
    });
    assert!(accepted && !overflowed);
    assert_eq!(bytes, 0);
    // Two membership queries and one positive-target query, each a numeric
    // lower_bound over the independently counted retained entries.
    let height = |n: usize| usize::BITS as usize - n.leading_zeros() as usize;
    assert!(
        index.query_probes.get() - before
            <= 1000 * (2 * height(index.functions.len()) + height(index.targets.len()))
    );
    let main = typed
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("main block");
    };
    let (binding, value) = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Assign { binding, value, .. } => Some((binding, value)),
            _ => None,
        })
        .unwrap();
    assert!(index.admits(binding, value));
    assert!(
        index.admits(&binding.clone(), value),
        "old structural binding equality remains admitted"
    );
    assert!(
        !index.admits(binding, &value.clone()),
        "equal text cannot grant current expression identity"
    );
    let foreign = program(&source);
    assert!(!index.matches_program(&foreign));
    assert!(foreign
        .functions
        .iter()
        .all(|function| !index.target(function) && index.function_sites(function).is_none()));
    assert_eq!(
        index.expressions.len(),
        index
            .sites
            .iter()
            .map(|(_, sites)| sites.len())
            .sum::<usize>()
    );
}
#[test]
fn numeric_directory_sort_work_and_duplicate_site_keys_preserve_saved_receivers() {
    // Independent count: initial histogram+copy, then each word byte visits
    // 256 reset cells, 256 prefix cells and three complete item/copy passes.
    let exact_work = 256 + 3 + std::mem::size_of::<usize>() * (512 + 3 * 3);
    for permutation in [[3usize, 1, 2], [1, 2, 3], [2, 3, 1], [usize::MAX, 0, 1]] {
        let mut exact = permutation;
        let mut work = exact_work;
        directory::sort(&mut exact, |key| *key, &mut work).unwrap();
        let mut expected = permutation;
        expected.sort();
        assert_eq!(exact, expected);
        assert_eq!(work, 0);
        let mut short = permutation;
        let mut work = exact_work - 1;
        assert_eq!(
            directory::sort(&mut short, |key| *key, &mut work)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    let duplicate_source = SOURCE.replace(
        "output=relay(output,array_as_slice(input));",
        "output=relay(output,array_as_slice(input)); output=relay(output,array_as_slice(input));",
    );
    let mut typed = program(&duplicate_source);
    // Distinct expression fields with equal text remain distinct exact keys.
    // The old per-function textual census replaces an equal textual entry and
    // retains its original ID field; the directory must reproduce that result.
    let main = typed
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("main block");
    };
    let first = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Assign { value, .. } => Some(value.id.clone()),
            _ => None,
        })
        .unwrap();
    for statement in statements.iter_mut() {
        if let crate::hir::ResolvedStatement::Assign { value, .. } = statement {
            value.id = first.clone();
        }
    }
    let index = ModuleIndex::new(&typed).unwrap();
    let main = typed
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("main block");
    };
    let old = checked_bindings(&typed, main).unwrap();
    let saved: Vec<_> = old.into_iter().collect();
    let assignments: Vec<_> = statements
        .iter()
        .filter_map(|statement| match statement {
            crate::hir::ResolvedStatement::Assign { binding, value, .. } => Some((binding, value)),
            _ => None,
        })
        .collect();
    assert_eq!(assignments.len(), 2);
    for (binding, value) in assignments {
        let expected = saved
            .iter()
            .any(|(at, actual)| std::ptr::eq(*at, &value.id) && *actual == binding);
        assert_eq!(index.admits(binding, value), expected);
    }
    assert_eq!(
        crate::hir::validate(&typed).unwrap_err().code,
        "SPX-H006",
        "directory discovery cannot authorize duplicate identities against the ordinary full validator"
    );
    for function in &typed.functions {
        let old = checked_bindings(&typed, function).unwrap();
        // Exact original keys are compared directly without accepting clones.
        for (at, binding) in old {
            assert!(index
                .expressions
                .iter()
                .any(|site| std::ptr::eq(site.at, at) && site.binding == binding));
            assert!(index
                .function_sites(function)
                .unwrap()
                .get_exact(at)
                .is_some());
        }
    }
}

#[test]
fn textual_assignment_directory_matches_first_linear_winner_and_exact_byte_debits() {
    let mut typed = program(SOURCE);
    for (function, id) in
        typed
            .functions
            .iter_mut()
            .zip(["common.prefix.a", "common.prefix.aa", "common.prefix.a"])
    {
        function.id = crate::hir::DeclarationId::new(id);
    }
    let mut work = WORK;
    let names = textual::Functions::new(&typed.functions, &mut work).unwrap();
    for wanted in [
        "",
        "common.prefix",
        "common.prefix.a",
        "common.prefix.aa",
        "common.prefix.b",
        "missing",
    ] {
        let expected = typed
            .functions
            .iter()
            .position(|function| function.id.as_str() == wanted);
        let actual = names.get(wanted, &mut work).unwrap();
        assert_eq!(actual, expected);
    }
    assert_eq!(names.get("common.prefix.a", &mut work).unwrap(), Some(0));
    for (left, right, expected, units) in [
        ("prefix.a", "prefix.a", std::cmp::Ordering::Equal, 9usize),
        ("prefix.a", "prefix.b", std::cmp::Ordering::Less, 8),
        ("prefix.", "prefix.a", std::cmp::Ordering::Less, 8),
        ("é", "ê", std::cmp::Ordering::Less, 2),
    ] {
        let mut exact = units;
        assert_eq!(textual::compare(left, right, &mut exact).unwrap(), expected);
        assert_eq!(exact, 0);
        let mut short = units - 1;
        assert_eq!(
            textual::compare(left, right, &mut short).unwrap_err().code,
            "SPX-H006"
        );
    }
    // Temporary sorted IDs and merge scratch borrow all spelling backing.
    // Two Vec payloads plus their exact owner/header are the whole reservation.
    let expected_bytes = std::mem::size_of::<textual::Functions<'_>>()
        + std::mem::size_of::<Vec<usize>>()
        + 2 * typed.functions.len() * std::mem::size_of::<usize>();
    let (exact, overflowed, used) = crate::bounded_output::with_limit_usage(expected_bytes, || {
        let mut work = WORK;
        let names = textual::Functions::new(&typed.functions, &mut work)?;
        let first = names.get("common.prefix.a", &mut work)?;
        drop(names);
        Ok::<_, Diagnostic>(first)
    });
    assert_eq!(exact.unwrap(), Some(0));
    assert!(!overflowed);
    assert_eq!(used, expected_bytes);
    let (short, overflowed, _) =
        crate::bounded_output::with_limit_usage(expected_bytes - 1, || {
            let mut work = WORK;
            textual::Functions::new(&typed.functions, &mut work)
        });
    assert_eq!(short.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
}
