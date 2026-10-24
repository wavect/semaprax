//! Actual compiler-type controls; no replacement IR or layout definitions.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
fn fixture() -> ResolvedFunction {
    let source="module paired.controls; @id(\"probe\") fn probe()->i64 { 0 } @id(\"main\") fn main()->i64 {0}";
    let parsed = crate::check(source, "paired-controls.spx").unwrap();
    crate::hir::resolve(&parsed)
        .unwrap()
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap()
        .clone()
}
fn labels() -> [Label; 3] {
    [
        Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned("prefix.é\0tail".to_owned()),
            value: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned("prefix.é\0tail".to_owned()),
            value: false,
        })),
        Label::End(SkeletonTerminal::Failure),
    ]
}
fn branching(f: &ResolvedFunction, duplicate: bool, mismatch: bool) -> BuiltComparison {
    let mut budget = ReplayBudget::new();
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let labels = labels();
    let b = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, b, labels[1].clone(), sink)
        .unwrap();
    let c = a.node(f, &mut budget).unwrap();
    a.movement(
        f,
        &mut budget,
        c,
        if duplicate {
            labels[1].clone()
        } else {
            labels[2].clone()
        },
        sink,
    )
    .unwrap();
    let left = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, left, labels[0].clone(), b)
        .unwrap();
    a.movement(f, &mut budget, left, labels[0].clone(), c)
        .unwrap();
    let joined = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, joined, labels[1].clone(), sink)
        .unwrap();
    if !mismatch {
        a.movement(
            f,
            &mut budget,
            joined,
            if duplicate {
                labels[1].clone()
            } else {
                labels[2].clone()
            },
            sink,
        )
        .unwrap();
    }
    let right = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, right, labels[0].clone(), joined)
        .unwrap();
    BuiltComparison {
        automaton: a,
        sink,
        hir_root: left,
        plan_root: right,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Outcome {
    kind: u8,
    remaining: usize,
    error: Option<&'static str>,
}
fn at_limit(
    f: &ResolvedFunction,
    b: &BuiltComparison,
    limit: usize,
    work: Option<usize>,
) -> (
    (Outcome, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    crate::bounded_output::with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            if let Some(work) = work {
                budget.remaining = work;
            }
            let result = budget
                .reserve_bytes(f, entry_controls(), "paired control entry")
                .and_then(|_| positive(f, b, &mut budget));
            let kind = match &result {
                Ok(Some(true)) => 1,
                Ok(None) => 0,
                Ok(Some(false)) => panic!("positive-only proof invented a mismatch"),
                Err(d) => {
                    assert_eq!(d.code, "SPX-H006");
                    assert_eq!(d.severity, crate::diagnostic::Severity::Error);
                    assert!(d.path.is_none() && d.span.is_none() && d.help.is_none());
                    2
                }
            };
            let out = Outcome {
                kind,
                remaining: budget.remaining,
                error: result.as_ref().err().map(|d| d.code),
            };
            drop(result);
            drop(budget);
            out
        })
    })
}
fn old_equal(f: &ResolvedFunction, b: &BuiltComparison) -> Option<bool> {
    let mut canonical =
        canonical_shared::inline_singleton::Canonicalizer::new(&b.automaton, b.sink);
    let mut budget = ReplayBudget::new();
    let left = canonical
        .id_for_comparison(f, vec![(b.hir_root, 1)], &mut budget)
        .unwrap()?;
    let right = canonical
        .id_for_comparison(f, vec![(b.plan_root, 1)], &mut budget)
        .unwrap()?;
    Some(left == right)
}
fn balanced(m: Metrics) {
    assert!(!m.invalid, "actual allocator observation invalid: {m:?}");
    assert_eq!(m.live, 0);
    assert_eq!(m.allocated, m.freed);
}
fn semantic_and_all_cuts(f: &ResolvedFunction, b: &BuiltComparison, expected: u8) {
    let ((out, m), refused, charged, _) = at_limit(f, b, 1 << 24, None);
    assert_eq!(out.kind, expected);
    assert!(!refused);
    balanced(m);
    assert!(
        m.allocated <= charged,
        "every requested allocation is paid: allocated={} charged={charged}",
        m.allocated
    );
    let ((exact, m), refused, paid, _) = at_limit(f, b, charged, None);
    assert_eq!(exact, out);
    assert_eq!(paid, charged);
    assert!(!refused);
    balanced(m);
    for limit in [0, 1, entry_controls().saturating_sub(1), charged - 1] {
        let ((out, m), refused, paid, receipt) = at_limit(f, b, limit, None);
        assert_eq!(out.kind, 2);
        assert!(refused);
        assert!(paid <= limit);
        assert!(receipt.is_some());
        assert!(m.allocated <= paid);
        balanced(m);
    }
    // Exhaust every integral byte cut through the actual composition, including
    // current bank, first backing, geometric growth and final publication.
    for limit in 0..charged {
        let ((out, m), refused, paid, receipt) = at_limit(f, b, limit, None);
        assert_eq!(out.kind, 2, "cut{limit}/{charged}");
        assert!(refused);
        assert!(paid <= limit);
        assert!(receipt.is_some());
        assert!(m.allocated <= paid);
        balanced(m);
    }
    if expected == 1 {
        let used = MAX_REPLAY_WORK_UNITS - out.remaining;
        let ((exact, m), _, _, _) = at_limit(f, b, 1 << 24, Some(used));
        assert_eq!(exact.kind, 1);
        assert_eq!(exact.remaining, 0);
        balanced(m);
        let ((short, m), _, _, _) = at_limit(f, b, 1 << 24, Some(used - 1));
        assert_eq!(short.kind, 2);
        balanced(m);
    }
    eprintln!(
        "PAIRED_CONTROL complete bound={charged} allocated={} work={}",
        m.allocated,
        MAX_REPLAY_WORK_UNITS - out.remaining
    );
}
fn scalar_cases(f: &ResolvedFunction) {
    for duplicate in [false, true] {
        let b = branching(f, duplicate, false);
        assert_eq!(old_equal(f, &b), Some(true));
        semantic_and_all_cuts(f, &b, 1);
    }
    let b = branching(f, false, true);
    assert_eq!(old_equal(f, &b), Some(false));
    semantic_and_all_cuts(f, &b, 0);
}
fn hostile_cases(f: &ResolvedFunction) {
    for case in 0..5 {
        let mut b = branching(f, false, false);
        match case {
            0 => b.hir_root = usize::MAX,
            1 => b.automaton.nodes[b.hir_root] = Node::Epsilon(usize::MAX),
            2 => b.automaton.nodes[b.hir_root] = Node::Epsilon(b.hir_root),
            3 => b.automaton.nodes[b.sink] = Node::Epsilon(b.hir_root),
            _ => {
                b.automaton.nodes.resize_with(MAX + 1, Node::default);
            }
        }
        let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
        assert_eq!(out.kind, 0);
        assert!(!refused);
        balanced(m);
    }
    let b = branching(f, false, false);
    let ((out, m), _, _, _) = at_limit(f, &b, 1 << 24, Some(0));
    assert_eq!(out.kind, 2);
    assert_eq!(out.remaining, 0);
    balanced(m);
    // Sticky physical refusal is checked on every root and warm frame use.
    crate::bounded_output::with_limit(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(f, entry_controls(), "paired sticky entry")
            .unwrap();
        let mut ledger =
            crate::bounded_output::classifier_string::ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        let bank = FrameBank {
            function: std::ptr::from_ref(f),
            budget: std::ptr::from_ref(&budget),
            ledger,
        };
        let mut w = Work {
            function: f,
            budget: &mut budget,
            bank,
        };
        w.frame(Frame::Compare).unwrap();
        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
        assert!(w.frame(Frame::Compare).is_err());
        assert!(w.frame(Frame::Compare).is_err());
    });
    // A held invocation cannot spend the foreign ledger or revive on restore.
    crate::bounded_output::with_limit(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(f, entry_controls(), "paired foreign entry")
            .unwrap();
        let mut ledger =
            crate::bounded_output::classifier_string::ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        let bank = FrameBank {
            function: std::ptr::from_ref(f),
            budget: std::ptr::from_ref(&budget),
            ledger,
        };
        let mut w = Work {
            function: f,
            budget: &mut budget,
            bank,
        };
        let (_, refused, charged) =
            crate::bounded_output::with_limit_usage(1 << 20, || assert!(w.bytes(1).is_err()));
        assert!(!refused);
        assert_eq!(charged, 0);
        assert!(w.bytes(0).is_err());
    });
}
fn overflow_and_width(f: &ResolvedFunction) {
    let mut budget = ReplayBudget::new();
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let mut root = sink;
    for _ in 0..64 {
        let next = a.node(f, &mut budget).unwrap();
        a.epsilon(f, &mut budget, next, root).unwrap();
        a.epsilon(f, &mut budget, next, root).unwrap();
        root = next;
    }
    let b = BuiltComparison {
        automaton: a,
        sink,
        hir_root: root,
        plan_root: root,
    };
    let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
    assert_eq!(out.kind, 0);
    assert!(!refused);
    balanced(m);
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let left = a.node(f, &mut budget).unwrap();
    let right = a.node(f, &mut budget).unwrap();
    for i in 0usize..96 {
        let label = Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned(format!("wide.\0.é.{i}")),
            value: i.is_multiple_of(2),
        }));
        a.movement(f, &mut budget, left, label.clone(), sink)
            .unwrap();
        a.movement(f, &mut budget, right, label, sink).unwrap();
    }
    let b = BuiltComparison {
        automaton: a,
        sink,
        hir_root: left,
        plan_root: right,
    };
    assert_eq!(old_equal(f, &b), Some(true));
    let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
    assert_eq!(out.kind, 1);
    assert!(!refused);
    balanced(m);
}
#[test]
fn paired_complete_legacy_semantics_and_every_byte_work_cut() {
    authenticate_prepaired_compare();
    scalar_cases(&fixture());
}
#[test]
fn paired_malformed_cycles_work_and_stale_foreign_sticky_receivers_fail_closed() {
    hostile_cases(&fixture());
}
#[test]
fn paired_duplicate_overflow_and_wide_utf8_nul_labels_keep_complete_order() {
    overflow_and_width(&fixture());
}

// Complete immediate pre-paired shipping receiver from Git 6b935c0a3.
// The only source adaptation is this function name and visibility.
pub(in crate::cleanup_plan::replay::factored) fn prepaired_compare<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Option<bool>, Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase =
        budget.paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::FactoredComparison);

    #[cfg(test)]
    let _canonical_allocation_phase = crate::test_allocation_observer::function_phase_guard(
        "cleanup_plan::factored::comparison_complete",
    );
    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    budget.reserve_bytes(
        function,
        canonical_shared::inline_singleton::Canonicalizer::CONSTRUCTOR_CONTROLS,
        "canonical ordered dictionary owner receiving",
    )?;
    let mut canonical = canonical_shared::inline_singleton::Canonicalizer::new(&automaton, sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::inline_singleton::Canonicalizer::COLD_NODE_CONTROLS,
            "factored cold terminal node predicate",
        )?;
    }
    if canonical.cold_dead_node(hir_root) {
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
        let Some(hir) = canonical.id_for_comparison(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_bytes(
            function,
            canonical_shared::inline_singleton::hir_last_use::CALL,
            "factored HIR-private release call receiving",
        )?;
        canonical.release_hir_private(function, budget, hir_root, plan_root)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
        let Some(plan) = canonical.id_for_comparison(function, vec![(plan_root, 1)], budget)?
        else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS
                + canonical_shared::inline_singleton::Canonicalizer::ACTIVE_DISPATCH_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving =
        canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    let canonical_id = canonical.canonical_dispatch(&budget.payer);
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) = canonical_id(
        &mut canonical,
        function,
        vec![(hir_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    budget.reserve_bytes(
        function,
        canonical_shared::inline_singleton::hir_last_use::CALL,
        "factored HIR-private release call receiving",
    )?;
    canonical.release_hir_private(function, budget, hir_root, plan_root)?;
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) = canonical_id(
        &mut canonical,
        function,
        vec![(plan_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

// The complete archive remains authoritative; extract full definitions rather
// than matching a crux, prefix or a rewritten projection of the old source.
pub(in crate::cleanup_plan::replay::factored) fn authenticate_prepaired_compare() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"
    ));
    assert_eq!(original.len(), 52_363);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"
    );
    fn definition<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).expect("complete source definition");
        let end = start
            + source[start..]
                .find("\n}")
                .expect("complete definition end")
            + 2;
        &source[start..end]
    }
    let compiled = definition(
        include_str!("tests.rs"),
        "pub(in crate::cleanup_plan::replay::factored) fn prepaired_compare<'hir>(",
    );
    assert_eq!(
        compiled.replacen(
            "pub(in crate::cleanup_plan::replay::factored) fn prepaired_compare<'hir>(",
            "fn compare_with_index_storage<'hir>(",
            1
        ),
        definition(original, "fn compare_with_index_storage<'hir>(")
    );
    assert_eq!(
        definition(
            include_str!("../../factored.rs"),
            "fn build_comparison<'hir>("
        ),
        definition(original, "fn build_comparison<'hir>(")
    );
}

// Complete pre-HIR-release generation; only name and visibility differ.
pub(in crate::cleanup_plan::replay::factored) fn preprivate_compare<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Option<bool>, Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase =
        budget.paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::FactoredComparison);

    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    budget.reserve_bytes(
        function,
        canonical_shared::inline_singleton::Canonicalizer::CONSTRUCTOR_CONTROLS,
        "canonical ordered dictionary owner receiving",
    )?;
    let mut canonical = canonical_shared::inline_singleton::Canonicalizer::new(&automaton, sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::inline_singleton::Canonicalizer::COLD_NODE_CONTROLS,
            "factored cold terminal node predicate",
        )?;
    }
    if canonical.cold_dead_node(hir_root) {
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
        let Some(hir) = canonical.id_for_comparison(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
        let Some(plan) = canonical.id_for_comparison(function, vec![(plan_root, 1)], budget)?
        else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS
                + canonical_shared::inline_singleton::Canonicalizer::ACTIVE_DISPATCH_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving =
        canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    let canonical_id = canonical.canonical_dispatch(&budget.payer);
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) = canonical_id(
        &mut canonical,
        function,
        vec![(hir_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) = canonical_id(
        &mut canonical,
        function,
        vec![(plan_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

pub(in crate::cleanup_plan::replay::factored) fn authenticate_preprivate_compare() {
    authenticate_prepaired_compare();
    use sha2::{Digest, Sha256};
    fn definition<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).expect("complete source definition");
        let end = start
            + source[start..]
                .find("\n}")
                .expect("complete definition end")
            + 2;
        &source[start..end]
    }
    let paid_original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"
    ));
    let mut original = paid_original.to_owned();
    for added in [
        "    #[cfg(test)]\n    let _canonical_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::comparison_complete\",\n    );\n",
        "        budget.reserve_bytes(\n            function,\n            canonical_shared::inline_singleton::hir_last_use::CALL,\n            \"factored HIR-private release call receiving\",\n        )?;\n        canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n",
        "    budget.reserve_bytes(\n        function,\n        canonical_shared::inline_singleton::hir_last_use::CALL,\n        \"factored HIR-private release call receiving\",\n    )?;\n    canonical.release_hir_private(function, budget, hir_root, plan_root)?;\n",
        "    #[cfg(test)]\n    let _construction_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::build_comparison\",\n    );\n",
        "        #[cfg(test)]\n        let hir_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_paths\",\n        );\n",
        "        #[cfg(test)]\n        drop(hir_allocation_phase);\n",
        "        #[cfg(test)]\n        let automaton_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_automaton\",\n        );\n",
        "        #[cfg(test)]\n        drop(automaton_allocation_phase);\n        #[cfg(test)]\n        let _plan_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_plan_automaton\",\n        );\n",
        "\n#[cfg(test)]\nmod hir_memo_release_tests;\n",
    ] {
        assert_eq!(original.matches(added).count(), 1);
        original = original.replacen(added, "", 1);
    }
    assert_eq!(original.len(), 50_781);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "ebfa584f4bc1fc2e33ef27a31902893293dd147bb90fd767b97308a747b60d49"
    );
    let compiled = definition(
        include_str!("tests.rs"),
        "pub(in crate::cleanup_plan::replay::factored) fn preprivate_compare<'hir>(",
    );
    assert_eq!(
        compiled.replacen(
            "pub(in crate::cleanup_plan::replay::factored) fn preprivate_compare<'hir>(",
            "fn compare_with_index_storage<'hir>(",
            1
        ),
        definition(&original, "fn compare_with_index_storage<'hir>(")
    );
    let mut build = definition(
        include_str!("../../factored.rs"),
        "fn build_comparison<'hir>(",
    )
    .to_owned();
    for added in [
        "    #[cfg(test)]\n    let _construction_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"cleanup_plan::factored::build_comparison\",\n    );\n",
        "        #[cfg(test)]\n        let hir_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_paths\",\n        );\n",
        "        #[cfg(test)]\n        drop(hir_allocation_phase);\n",
        "        #[cfg(test)]\n        let automaton_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_hir_automaton\",\n        );\n",
        "        #[cfg(test)]\n        drop(automaton_allocation_phase);\n        #[cfg(test)]\n        let _plan_allocation_phase = crate::test_allocation_observer::function_phase_guard(\n            \"cleanup_plan::factored::construct_plan_automaton\",\n        );\n",
    ] {
        assert_eq!(build.matches(added).count(), 1);
        build = build.replacen(added, "", 1);
    }
    assert_eq!(build, definition(&original, "fn build_comparison<'hir>("));
}

// Full pre-builder-crossover/pre-fitting generation, including its reference hook.
#[rustfmt::skip]
fn prefitting_representation_candidate(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
) -> bool {
    if cfg_paths <= FACTORED_PATH_THRESHOLD && semantic_paths <= FACTORED_PATH_THRESHOLD {
        return false;
    }
    let plan = &function.cleanup_plan;
    cfg_paths > ENUMERATION_CROSS_CHECK_THRESHOLD
        || semantic_paths > ENUMERATION_CROSS_CHECK_THRESHOLD
        || (plan.slots.is_empty()
            && plan.entry_state.live_owned_parameters.is_empty()
            && plan.entry_state.conditional_owned_parameters.is_empty())
}

#[rustfmt::skip]
pub(in crate::cleanup_plan::replay) fn prefitting_selected_paid(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if !prefitting_representation_candidate(function, cfg_paths, semantic_paths) {
        return Ok(false);
    }
    match structural_control::validate_ordinary_cfg(function, budget) {
        Ok(()) => Ok(true),
        Err(error) if budget.refusal_spent || crate::bounded_output::active_refused() => Err(error),
        Err(_) => Ok(false),
    }
}

#[rustfmt::skip]
pub(in crate::cleanup_plan::replay) fn prefitting_validate<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayFactored,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayFactored,
    );
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    let selected = prefitting_selected_paid(
        function,
        cfg_paths,
        semantic_paths,
        &mut ReplayBudget::new(),
    )?;
    #[cfg(test)]
    let continuations = {
        let mut continuations = continuations;
        if !selected
            || (cfg_paths <= ENUMERATION_CROSS_CHECK_THRESHOLD
                && semantic_paths <= ENUMERATION_CROSS_CHECK_THRESHOLD)
        {
            oracle_hook::cross_check_with_storage(
                program,
                function,
                budget,
                continuations.as_deref_mut(),
            )?;
        }
        continuations
    };
    if !selected {
        return Ok(false);
    }
    match prepaired_compare(program, function, budget, helpers, continuations) {
        Ok(Some(true)) => Ok(true),
        Ok(Some(false)) => Err(skeleton_mismatch(function)),
        Err(error)
            if cfg_paths <= MAX_REPLAY_PATHS
                && semantic_paths <= MAX_REPLAY_PATHS
                && (budget.refusal_spent || crate::bounded_output::active_refused()) =>
        {
            Err(error)
        }
        verdict => {
            if cfg_paths <= MAX_REPLAY_PATHS && semantic_paths <= MAX_REPLAY_PATHS {
                // Enumeration still fits; an attempted factored comparison
                // keeps every work and physical debit already spent.
                return Ok(false);
            }
            match verdict {
                Err(error) if !error.message.contains("budget") => Err(error),
                _ => path_summary::path_budget_errors(function, cfg_paths, semantic_paths)
                    .map(|()| false),
            }
        }
    }
}

pub(in crate::cleanup_plan::replay) fn authenticate_prefitting_dispatch() {
    authenticate_prepaired_compare();
    use sha2::{Digest, Sha256};
    fn definition<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).expect("complete source definition");
        let end = start
            + source[start..]
                .find("\n}")
                .expect("complete definition end")
            + 2;
        &source[start..end]
    }
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"
    ));
    assert_eq!(old.len(), 52363);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "82353ce21ba46fb5ae1746e30f837d9f251da7a17bdc53c9ba53355c13d9f4a0"
    );
    let current = include_str!("tests.rs");
    assert_eq!(
        definition(current, "fn prefitting_representation_candidate(").replacen(
            "fn prefitting_representation_candidate(",
            "fn representation_candidate(",
            1
        ),
        definition(old, "fn representation_candidate(")
    );
    assert_eq!(
        definition(
            current,
            "pub(in crate::cleanup_plan::replay) fn prefitting_selected_paid("
        )
        .replacen(
            "pub(in crate::cleanup_plan::replay) fn prefitting_selected_paid(",
            "pub(super) fn selected_paid(",
            1
        )
        .replacen(
            "prefitting_representation_candidate(",
            "representation_candidate(",
            1
        ),
        definition(old, "pub(super) fn selected_paid(")
    );
    assert_eq!(
        definition(
            current,
            "pub(in crate::cleanup_plan::replay) fn prefitting_validate<'hir>("
        )
        .replacen(
            "pub(in crate::cleanup_plan::replay) fn prefitting_validate<'hir>(",
            "pub(super) fn validate_with_index_storage<'hir>(",
            1
        )
        .replacen(
            "let selected = prefitting_selected_paid(",
            "let selected = selected_paid(",
            1
        )
        .replacen(
            "match prepaired_compare(",
            "match compare_with_index_storage(",
            1
        ),
        definition(old, "pub(super) fn validate_with_index_storage<'hir>(")
    );
    for declaration in [
        "const FACTORED_PATH_THRESHOLD: usize = 256;",
        "const ENUMERATION_CROSS_CHECK_THRESHOLD: usize = 4_096;",
    ] {
        assert_eq!(old.matches(declaration).count(), 1);
        assert_eq!(
            include_str!("../../factored.rs")
                .matches(declaration)
                .count(),
            1
        );
    }
}
