//! Independent paired-tail invoices and real compiler-value regressions.
use super::*;
#[allow(
    dead_code,
    reason = "exact original owner layout for the independent storage invoice"
)]
struct BeforeTailOwner<'history> {
    arena: &'history [Segment],
    left_items: &'history [Rc<SkeletonObservation>],
    right_items: &'history [Rc<SkeletonObservation>],
    left_loaded: bool,
    right_loaded: bool,
    left_cursor: Cursor,
    right_cursor: Cursor,
    left_link: usize,
    right_link: usize,
    left_segments: usize,
    right_segments: usize,
    step: Step,
    return_to: Step,
    after_pair: Step,
    len: usize,
    build: usize,
    drain: usize,
    root: usize,
    end: usize,
    child: usize,
    left: usize,
    right: usize,
    ordering: Ordering,
}
pub(super) fn prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {
    let header = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    let pair = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()
        + 6 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonTerminal>()
        + 2 * size_of::<Ordering>();
    let step = 2 * size_of::<usize>()
        + 3 * size_of::<bool>()
        + 3 * size_of::<&Rc<SkeletonObservation>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<&SkeletonObservation>()
        + 2 * size_of::<Option<&SkeletonObservation>>()
        + 3 * size_of::<usize>()
        + size_of::<bool>();
    let tail = tail_receiver_prices().1;
    let payload = 2 * size_of::<&Rc<SkeletonObservation>>()
        + 4 * size_of::<&[Rc<SkeletonObservation>]>()
        + 4 * size_of::<&Rc<SkeletonObservation>>()
        + 6 * size_of::<&SkeletonObservation>()
        + 4 * size_of::<&SkeletonTerminal>()
        + 2 * size_of::<usize>()
        + 4 * size_of::<Ordering>()
        + 3 * size_of::<bool>();
    (
        size_of::<BeforeTailOwner>() + (pair + 2 * step + 2 * tail) * row.fused_turns,
        size_of::<ComparisonOwner>()
            + size_of::<u8>()
            + if paths >= 2 { 5 * header } else { 0 }
            + size_of::<u8>() * row.pairs
            + 3 * size_of::<u8>() * row.tail_marks
            + (2 * size_of::<u8>() + size_of::<bool>()) * row.mask_checks
            + payload * row.fused_turns,
    )
}
#[test]
fn paired_tail_storage_invoice_contains_every_original_owner_field() {
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paired-tails-001-20261018/before-owner.rs.txt"
    ));
    let fields = old
        .split_once("struct ComparisonOwner<'history> {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let ours = include_str!("paired_tails.rs")
        .split_once("struct BeforeTailOwner<'history> {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert_eq!(
        fields, ours,
        "complete original storage, no opaque layout assumption"
    );
}
#[test]
fn paired_tail_complete_values_keep_every_883_work_cut_and_diagnostic() {
    let program = crate::cleanup_plan::replay::ordinary_plan_history::tests::program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "f")
        .unwrap();
    let item = Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    });
    let other = Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: false,
    });
    let expected = vec![
        SkeletonPath {
            observations: vec![item.clone(); 128].into(),
            terminal: SkeletonTerminal::Success
        };
        2
    ];
    for mutation in 0..4 {
        let actual = || HistorySet {
            arena: Vec::new(),
            max_depth: 0,
            paths: (0..2)
                .map(|side| {
                    let mut tail = vec![item.clone(); 128];
                    if side == 0 {
                        match mutation {
                            1 => {
                                tail.pop();
                            }
                            2 => tail[127] = other.clone(),
                            _ => {}
                        }
                    }
                    PlanPath {
                        history: History {
                            previous: None,
                            tail,
                        },
                        terminal: if mutation == 3 && side == 0 {
                            SkeletonTerminal::Failure
                        } else {
                            SkeletonTerminal::Success
                        },
                    }
                })
                .collect(),
        };
        pair_scope_census::reset();
        let (old, old_heap, _) =
            observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);
        pair_scope_census::reset();
        let (new, heap, _) =
            observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);
        let (rows, lost) = pair_scope_census::finish();
        assert_eq!(lost, 0);
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        assert!(!row.old && row.ended && !row.unwound && !row.truncated && !row.overflow);
        assert_eq!(new.result.is_ok(), mutation == 0);
        assert_eq!(
            row.complete,
            mutation == 0,
            "rejected inputs cannot certify completion"
        );
        assert!(row.fused_turns > 0, "exercise real borrowed tail loop");
        assert_eq!(new, old);
        assert_eq!(heap, old_heap);
        for cut in 0..=MAX_REPLAY_WORK_UNITS - old.remaining {
            pair_scope_census::reset();
            let (old, _, _) = observe_shaped_pair(function, &expected, actual(), true, cut);
            pair_scope_census::reset();
            let (new, _, _) = observe_shaped_pair(function, &expected, actual(), false, cut);
            assert_eq!(
                new, old,
                "all diagnostic fields/work, mutation{mutation} cut{cut}"
            );
        }
    }
}
