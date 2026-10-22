use super::super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(
        crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE,
    )
    .1
}

fn corpus(function: &ResolvedFunction, shape: usize) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    if shape == 2 {
        return (graph, sink, vec![sink]);
    }
    let mut roots = Vec::new();
    for row in 0..32 {
        let root = graph.node(function, &mut budget).unwrap();
        for column in 0..if shape == 0 { 128 } else { 24 } {
            // Duplicate corpus repeats the same full labels; unique-wide uses
            // distinct labels on each row. Both retain distinct state keys.
            let identity_row = if shape == 0 { 0 } else { row };
            let label = Label::Observation(Rc::new(
                crate::cleanup_plan::replay::SkeletonObservation::Boolean {
                    expression: crate::hir::ExpressionId::from_owned(format!(
                        "arena.row.{identity_row}.column.{column}"
                    )),
                    value: column % 2 == 0,
                },
            ));
            graph
                .movement(function, &mut budget, root, label, sink)
                .unwrap();
        }
        roots.push(root);
    }
    (graph, sink, roots)
}

#[derive(Debug, Eq, PartialEq)]
struct Text {
    bytes: [u8; 1024],
    len: usize,
}
impl Text {
    fn capture(value: &str) -> Self {
        let mut bytes = [0; 1024];
        assert!(
            value.len() <= bytes.len(),
            "complete fixture Diagnostic reporting, no truncation"
        );
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Self {
            bytes,
            len: value.len(),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Fields {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: Text,
    path: Option<Text>,
    span: Option<crate::ast::Span>,
    help: Option<Text>,
}
impl Fields {
    fn capture(error: Diagnostic) -> Self {
        Self {
            code: error.code,
            severity: error.severity,
            message: Text::capture(&error.message),
            path: error.path.as_deref().map(Text::capture),
            span: error.span,
            help: error.help.as_deref().map(Text::capture),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Stats {
    paid: usize,
    work: (usize, usize, usize),
    checksum: u64,
    counts: (usize, usize),
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    result: Result<Stats, Fields>,
    sticky: Option<Fields>,
}
fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    old: bool,
    limit: usize,
) -> (Outcome, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                return Outcome {
                    result: Err(Fields::capture(error)),
                    sticky: None,
                }
            }
        };
        let result = (|| {
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "canonical ordered dictionary owner receiving",
            )?;
            let mut owner = Canonicalizer::new(graph, sink);
            let mut checksum = 0u64;
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                let id = if old {
                    owner.before_id(function, vec![(*root, 1)], &mut budget)
                } else {
                    owner.matched_876_id(function, vec![(*root, 1)], &mut budget)
                }?
                .unwrap();
                checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
            }
            Ok::<_, Diagnostic>(Stats {
                paid: budget.physical.as_ref().unwrap().charged(),
                work: (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                ),
                checksum,
                counts: (owner.ids.len(), owner.states.len()),
            })
        })();
        let sticky = if result.is_err() {
            let bytes = budget.physical.as_ref().unwrap().charged();
            let work = (
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent,
            );
            let allocated = current_metrics().allocated;
            let retry = budget
                .reserve_bytes(function, 0, "lexical child scratch sticky retry")
                .unwrap_err();
            assert_eq!(budget.physical.as_ref().unwrap().charged(), bytes);
            assert_eq!(
                (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent
                ),
                work
            );
            assert_eq!(current_metrics().allocated, allocated);
            Some(Fields::capture(retry))
        } else {
            None
        };
        drop(budget);
        Outcome {
            result: result.map_err(Fields::capture),
            sticky,
        }
    })
}
#[test]
fn lexical_child_scratch_preserves_complete_deec_owner_work_backing_and_only_removes_outer_receipts(
) {
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = corpus(&function, shape);
        let (old, old_heap) = observe(&function, &graph, sink, &roots, true, 1 << 26);
        let (current, heap) = observe(&function, &graph, sink, &roots, false, 1 << 26);
        let old = old.result.unwrap();
        let current = current.result.unwrap();
        assert_eq!(
            (current.work, current.checksum, current.counts),
            (old.work, old.checksum, old.counts)
        );
        assert_eq!(
            heap, old_heap,
            "same actual backing, allocation and Drop; only a forwarding stack carrier disappears"
        );
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        let nonempty = roots
            .iter()
            .filter(|root| !graph.nodes[**root].moves().is_empty())
            .count();
        assert_eq!(
            old.paid - current.paid,
            nonempty * std::mem::size_of::<Result<(), Diagnostic>>()
        );
        assert_eq!(
            intern_arena::SignatureQuery::EXIT_CONTROLS,
            intern_arena::SignatureQuery::LEXICAL_EXIT_CONTROLS
                + std::mem::size_of::<Result<(), Diagnostic>>()
        );
        for limit in [0, current.paid / 2, current.paid - 1, current.paid] {
            let first = observe(&function, &graph, sink, &roots, false, limit);
            let again = observe(&function, &graph, sink, &roots, false, limit);
            assert_eq!(
                first, again,
                "all six Diagnostic fields, sticky work, allocation, peak and whole Drop"
            );
            assert_eq!(first.0.result.is_ok(), limit == current.paid);
            assert!(!first.1.invalid && first.1.live == 0);
            assert_eq!(first.1.allocated, first.1.freed);
            if let Err(error) = first.0.result {
                assert_eq!(error.code, "SPX-H006");
            }
        }
        eprintln!(
            "OPT876 lexical child scratch shape={shape} old/current reserved={}/{} actual={heap:?}",
            old.paid, current.paid
        );
    }
}

#[test]
fn lexical_child_scratch_preserves_complete_state_signature_and_every_original_work_failure() {
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = corpus(&function, shape);
        for remaining in [
            0,
            1,
            2,
            16,
            128,
            1024,
            crate::cleanup_plan::replay::MAX_REPLAY_WORK_UNITS,
        ] {
            let mut old = Canonicalizer::new(&graph, sink);
            let mut current = Canonicalizer::new(&graph, sink);
            let mut a = ReplayBudget::unobserved();
            let mut b = ReplayBudget::unobserved();
            a.remaining = remaining;
            b.remaining = remaining;
            for root in &roots {
                assert_eq!(
                    current.id(&function, vec![(*root, 1)], &mut b),
                    old.before_id(&function, vec![(*root, 1)], &mut a),
                    "complete original six fields and first work failure"
                );
                assert_eq!(
                    (b.remaining, b.skeleton_remaining, b.skeleton_spent),
                    (a.remaining, a.skeleton_remaining, a.skeleton_spent)
                );
                assert!(current.ids.iter().eq(old.ids.iter()));
                assert!(current.states.iter().eq(old.states.iter()));
            }
        }
    }
}

#[path = "current_tests.rs"]
mod current_tests;
