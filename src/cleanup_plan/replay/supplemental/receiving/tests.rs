use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module cursor.supplemental;
@id("cursor.consume") fn consume(value: own Bytes) -> usize {
    byte_len(bytes_as_slice(value))
}
@id("main") fn main() -> i64 requires true ensures result >= 0 {
    let size = consume(bytes_zeroed(1usize));
    if size == 1usize { 0 } else { 1 }
}"#,
            "supplemental-cursor.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

fn prepared(function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    budget.payer = AllocationPayer::ActiveBuilder;
    budget.prepare_builder(function).unwrap();
    budget
}

#[test]
fn complete_original_and_growing_walks_receive_the_same_owned_argument_slots() {
    let program = program();
    let mut received = 0;
    for function in &program.functions {
        let mut current_budget = ReplayBudget::unobserved();
        let mut old_budget = ReplayBudget::unobserved();
        let mut cursor = Scratch::new(function, &mut current_budget).unwrap();
        let mut old = before::Scratch::new(function, &mut old_budget).unwrap();
        let mut flags = u32::try_from(function.cleanup.flags.len()).unwrap();
        let mut old_flags = flags;
        // Reuse the same two owners over repeated complete root sequences.
        for _ in 0..3 {
            for root in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                let mut actual = Vec::new();
                let mut original = Vec::new();
                let result = walk(
                    &program,
                    function,
                    root,
                    &mut flags,
                    &mut cursor,
                    &mut current_budget,
                    |storage, ty, shape| {
                        actual.push((storage, ty.clone(), shape));
                        Ok(())
                    },
                );
                let reference = before::walk(
                    &program,
                    function,
                    root,
                    &mut old_flags,
                    &mut old,
                    &mut old_budget,
                    |storage, ty, shape| {
                        original.push((storage, ty.clone(), shape));
                        Ok(())
                    },
                );
                assert_eq!(result, reference);
                assert_eq!(actual, original);
                assert_eq!(flags, old_flags);
                received += actual.len();
                assert!(cursor.frames.is_empty());
            }
        }
    }
    assert!(
        received > 0,
        "the control must actually receive an owned call argument"
    );
}

const TARGETS: [usize; 11] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 1028];
fn controls() -> usize {
    2 * size_of::<Vec<Frame<'_>>>() + size_of::<Frame<'_>>()
}
fn price(target: usize) -> usize {
    controls() + target * size_of::<Frame<'_>>()
}
fn fill<'a>(cursor: &mut Scratch<'a>, function: &'a ResolvedFunction) {
    while cursor.frames.len() < cursor.frames.capacity() {
        let position = cursor.frames.len();
        cursor.frames.push(Frame::Expr(&function.body, position));
    }
}
fn preserved(cursor: &Scratch<'_>, function: &ResolvedFunction) {
    for (position, frame) in cursor.frames.iter().enumerate() {
        let Frame::Expr(expression, old_position) = frame else {
            panic!("live frame changed kind");
        };
        assert!(std::ptr::eq(*expression, &function.body));
        assert_eq!(*old_position, position);
    }
}

fn check_growth(function: &ResolvedFunction) {
    let mut budget = prepared(function);
    let initial_work = budget.remaining;
    let expected = size_of::<Scratch<'_>>() + TARGETS.into_iter().map(price).sum::<usize>();
    let (((), observed), refused, used) = with_limit_usage(expected, || {
        measure(|| {
            let mut cursor = Scratch::new(function, &mut budget).unwrap();
            for target in TARGETS {
                if target != 2 {
                    fill(&mut cursor, function);
                    reserve_frames(&mut cursor.frames, 1, function, &mut budget).unwrap();
                    preserved(&cursor, function);
                }
                assert_eq!(cursor.frames.capacity(), target);
            }
            drop(cursor);
        })
    });
    assert!(!refused && !observed.invalid);
    assert_eq!(used, expected);
    assert_eq!(budget.remaining, initial_work - 2046);
    assert_eq!(observed.allocated, 3074 * size_of::<Frame<'_>>());
    assert_eq!(observed.highwater, 2052 * size_of::<Frame<'_>>());
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.live, 0);
}

fn check_short(function: &ResolvedFunction) {
    for denied in TARGETS {
        let prior = size_of::<Scratch<'_>>()
            + TARGETS
                .into_iter()
                .take_while(|n| *n < denied)
                .map(price)
                .sum::<usize>();
        let mut budget = prepared(function);
        let (_, refused, used, receipt) =
            with_limit_usage_refusal(prior + price(denied) - 1, || {
                if denied == 2 {
                    assert!(Scratch::new(function, &mut budget).is_err());
                    return;
                }
                let mut cursor = Scratch::new(function, &mut budget).unwrap();
                for target in TARGETS.into_iter().skip(1).take_while(|n| *n < denied) {
                    fill(&mut cursor, function);
                    reserve_frames(&mut cursor.frames, 1, function, &mut budget).unwrap();
                    assert_eq!(cursor.frames.capacity(), target);
                }
                fill(&mut cursor, function);
                let capacity = cursor.frames.capacity();
                assert!(reserve_frames(&mut cursor.frames, 1, function, &mut budget).is_err());
                assert_eq!(cursor.frames.capacity(), capacity);
                preserved(&cursor, function);
                cursor.frames.clear();
                assert!(reserve_frames(&mut cursor.frames, 0, function, &mut budget).is_err());
            });
        assert!(refused);
        assert_eq!(used, prior + controls());
        let receipt = receipt.unwrap();
        assert_eq!(receipt.requested, denied * size_of::<Frame<'_>>());
        assert_eq!(receipt.remaining + 1, receipt.requested);
    }
}

fn check_shallow(function: &ResolvedFunction) {
    let mut budget = prepared(function);
    let (_, refused, used) = with_limit_usage(size_of::<Scratch<'_>>() + price(2), || {
        let cursor = Scratch::new(function, &mut budget).unwrap();
        assert_eq!(cursor.frames.capacity(), 2);
    });
    assert!(!refused);
    assert_eq!(used, size_of::<Scratch<'_>>() + price(2));
    assert!(used < 1028 * size_of::<Frame<'_>>());
}

fn check_bound_and_work(function: &ResolvedFunction) {
    let mut budget = prepared(function);
    let mut cursor = Scratch::new(function, &mut budget).unwrap();
    fill(&mut cursor, function);
    budget.remaining = 0;
    let capacity = cursor.frames.capacity();
    let ((result, observed), refused, used) = with_limit_usage(usize::MAX, || {
        measure(|| reserve_frames(&mut cursor.frames, 1, function, &mut budget))
    });
    assert!(result.is_err() && !refused && !observed.invalid);
    assert_eq!(used, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(cursor.frames.capacity(), capacity);
    preserved(&cursor, function);

    let mut budget = ReplayBudget::unobserved();
    let mut frames = Vec::new();
    reserve_frames(&mut frames, 1028, function, &mut budget).unwrap();
    assert_eq!(frames.capacity(), 1028);
    let pointer = frames.as_ptr();
    assert!(reserve_frames(&mut frames, 1029, function, &mut budget).is_err());
    assert_eq!(frames.as_ptr(), pointer);
    assert_eq!(frames.capacity(), 1028);
}

#[test]
fn every_replacement_is_paid_while_old_live_frames_survive_and_then_release() {
    let program = program();
    let function = &program.functions[0];
    check_growth(function);
    check_short(function);
    check_shallow(function);
    check_bound_and_work(function);
}

#[test]
fn complete_original_receiving_walk_is_preserved_by_only_storage_substitutions() {
    let original = include_str!(
        "../../../../../docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt"
    );
    let current = include_str!("../receiving.rs");
    let helper = current
        .find("// Capacity never supplies traversal authority.")
        .unwrap();
    let mut restored = current[..helper].to_owned();
    let replacements = [
        ("//! One paid depth-bounded cursor, reused after each complete expression walk.", "//! One real paid fixed cursor, reused after each complete source-expression walk."),
        ("const FRAME_CAPACITY: usize = 1028;", "use crate::cleanup_plan::replay::ordinary_inventory::storage::array;"),
        ("        budget.reserve_array::<Self>(function, 1, \"supplemental cursor owner\")?;\n        let mut frames = Vec::new();\n        reserve_frames(&mut frames, 1, function, budget)?;\n        Ok(Self { frames })", "        Ok(Self {\n            frames: array(function, 1028, budget, \"supplemental fixed cursor\")?,\n        })"),
        ("    reserve_frames(frames, 1, function, budget)?;\n", ""),
        ("if frames.len() + 3 > FRAME_CAPACITY", "if frames.len() + 3 > frames.capacity()"),
        ("if frames.len() + 2 > FRAME_CAPACITY", "if frames.len() + 2 > frames.capacity()"),
        ("                        reserve_frames(frames, 3, function, budget)?;\n", ""),
        ("                    reserve_frames(frames, 2, function, budget)?;\n", ""),
    ];
    for (new, old) in replacements {
        assert_eq!(restored.matches(new).count(), 1, "{new}");
        restored = restored.replacen(new, old, 1);
    }
    assert_eq!(restored.strip_suffix('\n').unwrap(), original);
}
