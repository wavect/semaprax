//! Original full-array traversal vs inline+overflow stack: ids, work, backing, allocation.
use super::super::{Admission, Directory, Family, Refusal};
use super::stack::{Stack, INLINE, LIMIT};
use super::{old, visit};
use crate::hir::lexical_exit::ChildCursor;
use crate::hir::{ExpressionId, ResolvedExpr as E, ResolvedExprKind as K};
use std::cell::Cell;
use std::mem::size_of;

#[test]
fn phase_presence_keeps_full_current_directories_and_every_consuming_entry() {
    let source = crate::check(
        r#"module phase_presence;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("plain") fn plain()->i64 {0}
@id("main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});
 let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0 }
"#,
        "phase-presence.spx",
    ).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
    let mut work = |_| true;
    let acquisition_paid = Cell::new(0usize);
    let mut backing = |bytes| {
        acquisition_paid.set(acquisition_paid.get() + bytes);
        true
    };
    let mut acquisition = Admission::new(&mut work, &mut backing);
    // Match the actual caller frontier before evaluating new's arguments.
    acquisition
        .reserve(super::super::RecordPresence::acquisition_receiving_controls())
        .unwrap();
    let presence =
        super::super::RecordPresence::new(&program.declarations, &mut acquisition).unwrap();
    assert_eq!(
        acquisition_paid.get(),
        super::super::RecordPresence::acquisition_receiving_controls()
            + super::super::RecordPresence::constructor_controls()
    );
    let mut prefix = 0;
    for declaration in program.declarations.declarations() {
        prefix += 1;
        if declaration.kind == crate::hir::DeclarationKind::Record
            && declaration.identity_origin == crate::hir::IdentityOrigin::Explicit
        {
            break;
        }
    }
    let mut consuming = 0;
    for member in super::super::current_members(&program) {
        let collect = |cached| {
            let work = Cell::new(0usize);
            let paid = Cell::new(0usize);
            let mut debit = |units| {
                work.set(work.get() + units);
                true
            };
            let mut reserve = |bytes| {
                paid.set(paid.get() + bytes);
                true
            };
            let mut admission = Admission::new(&mut debit, &mut reserve);
            let directory = if cached {
                // Production ModuleIndex pays this Some-route bank before the
                // presence reference and factory arguments are constructed.
                admission
                    .reserve(super::super::RecordPresence::shared_caller_controls())
                    .unwrap();
                Directory::collect_current_with_presence(
                    member,
                    &helpers,
                    &presence,
                    &mut admission,
                )
            } else {
                Directory::collect_current(member, &helpers, &mut admission)
            }
            .unwrap();
            (directory, work.get(), paid.get())
        };
        let (before, old_work, old_paid) = collect(false);
        let (after, new_work, new_paid) = collect(true);
        assert_eq!(new_work + prefix, old_work + 1);
        let none_controls = super::super::RecordPresence::transport_controls();
        let some_controls = super::super::RecordPresence::shared_caller_controls();
        assert_eq!(new_paid, old_paid - none_controls + some_controls);
        // The complete directory backing is identical after separating the
        // genuinely different, explicitly paid None/Some control routes.
        assert_eq!(new_paid - some_controls, old_paid - none_controls);
        assert_eq!(before.entries.len(), after.entries.len());
        for (before, after) in before.entries.iter().zip(&after.entries) {
            assert!(std::ptr::eq(before.at, after.at));
            assert!(std::ptr::eq(before.binding, after.binding));
            assert_eq!(
                (
                    before.families,
                    before.family,
                    before.ordinal,
                    before.rank,
                    before.first
                ),
                (
                    after.families,
                    after.family,
                    after.ordinal,
                    after.rank,
                    after.first
                )
            );
            consuming += usize::from(after.rank == 0 && after.first);
        }
    }
    assert!(consuming > 0);
}

struct Outcome<'r, O> {
    nodes: Vec<&'r E>,
    result: Result<(), Refusal>,
    work_left: usize,
    backing_left: usize,
    observed: O,
}
macro_rules! run {
    ($visit:expr, $root:expr, $work:expr, $backing:expr) => {{
        let root: &E = $root;
        let work: Cell<usize> = Cell::new($work);
        let backing: Cell<usize> = Cell::new($backing);
        let failed = Cell::new(false);
        let mut charge = |units: usize| match work.get().checked_sub(units) {
            Some(left) if !failed.get() => {
                work.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        let mut reserve = |bytes: usize| match backing.get().checked_sub(bytes) {
            Some(left) if !failed.get() => {
                backing.set(left);
                true
            }
            _ => {
                failed.set(true);
                false
            }
        };
        // Reserved outside the observer so only stack storage is measured.
        let mut nodes: Vec<&E> = Vec::with_capacity(LIMIT + 1);
        let (result, observed) = crate::test_allocation_observer::measure(|| {
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let result = ($visit)(root, &mut admission, |expression, _| {
                nodes.push(expression);
                Ok(())
            });
            if result.is_err() {
                assert!(admission.units(0).is_err());
                assert!(admission.reserve(0).is_err());
            }
            result
        });
        Outcome {
            nodes,
            result,
            work_left: work.get(),
            backing_left: backing.get(),
            observed,
        }
    }};
}

fn fixture() -> crate::hir::ResolvedProgram {
    let source = crate::check(
        r#"module consuming_stack; @id("main") fn main()->i64 {let x=0;x}"#,
        "consuming-stack.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn leaf(program: &crate::hir::ResolvedProgram) -> E {
    let K::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("fixture block");
    };
    let mut leaf = (**tail).clone();
    assert!(ChildCursor::new(&leaf).next().is_none());
    leaf.id = ExpressionId::from_owned("leaf".to_owned());
    leaf
}
// `blocks` nested empty blocks over one childless leaf: blocks + 1 frames.
fn chain(leaf: &E, blocks: usize) -> E {
    let mut node = leaf.clone();
    for index in 0..blocks {
        let mut parent = leaf.clone();
        parent.id = ExpressionId::from_owned(format!("block-{index}"));
        parent.kind = K::Block {
            statements: Vec::new(),
            tail: Box::new(node),
        };
        node = parent;
    }
    node
}
fn closure_over(leaf: &E, blocks: usize) -> E {
    let mut closure = leaf.clone();
    closure.id = ExpressionId::from_owned("closure".to_owned());
    closure.kind = K::Closure {
        parameters: Vec::new(),
        captures: Vec::new(),
        body: Box::new(chain(leaf, blocks)),
    };
    let mut root = leaf.clone();
    root.id = ExpressionId::from_owned("root".to_owned());
    root.kind = K::Block {
        statements: Vec::new(),
        tail: Box::new(closure),
    };
    root
}
// (allocated, highwater, allocations, last request bytes) for the doubling spill.
fn heap(frames: usize) -> (usize, usize, usize, usize) {
    let need = frames.min(LIMIT).saturating_sub(INLINE);
    let mut caps = Vec::new();
    let mut cap = 0usize;
    while cap < need {
        cap = (cap * 2).clamp(INLINE, LIMIT - INLINE);
        caps.push(cap);
    }
    let cell = size_of::<ChildCursor<'static>>();
    let allocated = caps.iter().sum::<usize>() * cell;
    let (mut highwater, mut previous) = (0, 0);
    for &cap in &caps {
        highwater = highwater.max((previous + cap) * cell);
        previous = cap;
    }
    let last = caps.last().copied().unwrap_or(0) * cell;
    (allocated, highwater, caps.len(), last)
}
fn charged(frames: usize) -> usize {
    size_of::<Stack<'static>>()
        + heap(frames).0
        + heap(frames).2 * size_of::<Vec<ChildCursor<'static>>>()
}
fn used(left: usize) -> usize {
    usize::MAX - left
}
fn ids<'r, O>(outcome: &Outcome<'r, O>) -> Vec<&'r ExpressionId> {
    outcome.nodes.iter().map(|node| &node.id).collect()
}

#[test]
fn stack_layout_and_original_carrier_size() {
    let cell = size_of::<ChildCursor<'static>>();
    assert_eq!(size_of::<Option<ChildCursor<'static>>>(), cell);
    assert_eq!(size_of::<old::Frames<'static>>(), LIMIT * cell);
    assert_eq!(LIMIT, crate::cache_codec::MAX_DEPTH + 1);
    assert_eq!(
        size_of::<Stack<'static>>(),
        INLINE * cell + size_of::<Vec<ChildCursor<'static>>>() + size_of::<usize>()
    );
    assert!(size_of::<Stack<'static>>() < size_of::<old::Frames<'static>>());
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<old::Frames<'static>>(), 10280);
}

#[test]
fn chains_match_original_ids_work_and_charge_real_formula() {
    let program = fixture();
    let leaf = leaf(&program);
    for blocks in [0usize, 1, 6, 7, 8, 31, 255, 256] {
        let frames = blocks + 1;
        let root = chain(&leaf, blocks);
        let before = run!(old::visit, &root, usize::MAX, usize::MAX);
        let after = run!(visit, &root, usize::MAX, usize::MAX);
        assert_eq!(before.result, Ok(()));
        assert_eq!(after.result, Ok(()));
        assert_eq!(after.nodes.len(), frames);
        assert_eq!(ids(&before), ids(&after));
        assert_eq!(used(before.work_left), used(after.work_left));
        assert_eq!(used(before.backing_left), size_of::<old::Frames<'static>>());
        assert_eq!(used(after.backing_left), charged(frames));
        assert_eq!(before.observed.allocated, 0);
        let (allocated, highwater, count, _) = heap(frames);
        let observed = &after.observed;
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, allocated);
        assert_eq!(observed.highwater, highwater);
        assert_eq!(observed.allocations, count);
        assert_eq!(observed.reallocations, 0);
        assert!(observed.allocated <= used(after.backing_left) - size_of::<Stack<'static>>());
        if frames <= INLINE {
            assert_eq!(observed.allocated, 0);
            assert_eq!(
                used(before.backing_left) - used(after.backing_left),
                size_of::<old::Frames<'static>>() - size_of::<Stack<'static>>()
            );
        }
    }
}

#[test]
fn parsed_block_with_statements_and_deep_tail_match_original() {
    let program = fixture();
    let leaf = leaf(&program);
    let shallow = program.functions[0].body.clone();
    let mut mixed = program.functions[0].body.clone();
    let K::Block { tail, .. } = &mut mixed.kind else {
        panic!("fixture block");
    };
    **tail = chain(&leaf, 30);
    for root in [&shallow, &mixed] {
        let before = run!(old::visit, root, usize::MAX, usize::MAX);
        let after = run!(visit, root, usize::MAX, usize::MAX);
        assert_eq!(before.result, Ok(()));
        assert_eq!(after.result, Ok(()));
        assert!(after.nodes.len() > 1);
        assert_eq!(ids(&before), ids(&after));
        assert_eq!(used(before.work_left), used(after.work_left));
    }
    let after = run!(visit, &shallow, usize::MAX, usize::MAX);
    assert_eq!(used(after.backing_left), size_of::<Stack<'static>>());
}

#[test]
fn closure_bodies_are_elided_even_beyond_the_depth_cap() {
    let program = fixture();
    let leaf = leaf(&program);
    for blocks in [0usize, 20, 300] {
        let root = closure_over(&leaf, blocks);
        let before = run!(old::visit, &root, usize::MAX, usize::MAX);
        let after = run!(visit, &root, usize::MAX, usize::MAX);
        assert_eq!(before.result, Ok(()));
        assert_eq!(after.result, Ok(()));
        assert_eq!(after.nodes.len(), 2);
        assert_eq!(ids(&before), ids(&after));
        assert_eq!(used(before.work_left), used(after.work_left));
        assert_eq!(used(after.backing_left), size_of::<Stack<'static>>());
        assert_eq!(after.observed.allocated, 0);
    }
}

#[test]
fn exact_one_short_zero_and_refusal_contracts() {
    let program = fixture();
    let leaf = leaf(&program);
    for frames in [1usize, 8, 9, 40, 257] {
        let root = chain(&leaf, frames - 1);
        let full = run!(visit, &root, usize::MAX, usize::MAX);
        let work = used(full.work_left);
        let backing = used(full.backing_left);
        assert_eq!(backing, charged(frames));
        assert_eq!(
            work,
            used(run!(old::visit, &root, usize::MAX, usize::MAX).work_left)
        );
        // Exact work and backing succeed for both implementations.
        let exact = run!(visit, &root, work, backing);
        assert_eq!(exact.result, Ok(()));
        assert_eq!((exact.work_left, exact.backing_left), (0, 0));
        let exact = run!(old::visit, &root, work, size_of::<old::Frames<'static>>());
        assert_eq!(exact.result, Ok(()));
        assert_eq!((exact.work_left, exact.backing_left), (0, 0));
        // One unit short of work is a sticky Work refusal.
        assert_eq!(
            run!(visit, &root, work - 1, backing).result,
            Err(Refusal::Work)
        );
        assert_eq!(
            run!(
                old::visit,
                &root,
                work - 1,
                size_of::<old::Frames<'static>>()
            )
            .result,
            Err(Refusal::Work)
        );
        // One byte short of backing is refused before the denied allocation.
        let short = run!(visit, &root, work, backing - 1);
        assert_eq!(short.result, Err(Refusal::Backing));
        let (allocated, _, _, last) = heap(frames);
        assert_eq!(short.observed.allocated, allocated - last);
        assert_eq!(short.observed.live, 0);
        assert!(!short.observed.invalid);
        assert_eq!(
            run!(
                old::visit,
                &root,
                work,
                size_of::<old::Frames<'static>>() - 1
            )
            .result,
            Err(Refusal::Backing)
        );
        // Zero budgets: backing is checked before initialization work.
        for (w, b) in [(0usize, 0usize), (usize::MAX, 0)] {
            let a = run!(old::visit, &root, w, b);
            let n = run!(visit, &root, w, b);
            assert_eq!(a.result, Err(Refusal::Backing));
            assert_eq!(n.result, Err(Refusal::Backing));
            assert!(a.nodes.is_empty() && n.nodes.is_empty());
        }
        let a = run!(old::visit, &root, 0, usize::MAX);
        let n = run!(visit, &root, 0, usize::MAX);
        assert_eq!(a.result, Err(Refusal::Work));
        assert_eq!(n.result, Err(Refusal::Work));
        assert_eq!(used(n.backing_left), size_of::<Stack<'static>>());
        assert_eq!(used(a.backing_left), size_of::<old::Frames<'static>>());
    }
}

#[test]
fn exact_257_frames_succeed_and_258_fail_like_original() {
    let program = fixture();
    let leaf = leaf(&program);
    let ok = chain(&leaf, LIMIT - 1);
    let before = run!(old::visit, &ok, usize::MAX, usize::MAX);
    let after = run!(visit, &ok, usize::MAX, usize::MAX);
    assert_eq!((before.result, after.result), (Ok(()), Ok(())));
    assert_eq!(after.nodes.len(), LIMIT);
    assert_eq!(ids(&before), ids(&after));
    let over = chain(&leaf, LIMIT);
    let before = run!(old::visit, &over, usize::MAX, usize::MAX);
    let after = run!(visit, &over, usize::MAX, usize::MAX);
    assert_eq!(before.result, Err(Refusal::Capacity));
    assert_eq!(after.result, Err(Refusal::Capacity));
    assert_eq!(before.nodes.len(), LIMIT);
    assert_eq!(ids(&before), ids(&after));
    assert_eq!(used(before.work_left), used(after.work_left));
    assert_eq!(used(after.backing_left), charged(LIMIT + 1));
    assert_eq!(after.observed.allocated, heap(LIMIT + 1).0);
    assert_eq!(after.observed.live, 0);
}

#[test]
fn stack_keeps_old_backing_when_replacement_is_refused_and_caps_at_limit() {
    let program = fixture();
    let leaf = leaf(&program);
    let backing = Cell::new(size_of::<Stack<'static>>());
    let mut charge = |_: usize| true;
    let mut reserve = |bytes: usize| match backing.get().checked_sub(bytes) {
        Some(left) => {
            backing.set(left);
            true
        }
        None => false,
    };
    let mut admission = Admission::new(&mut charge, &mut reserve);
    let mut stack = Stack::new(&mut admission).unwrap();
    for _ in 0..INLINE {
        stack.push(ChildCursor::new(&leaf), &mut admission).unwrap();
    }
    assert_eq!(
        stack.push(ChildCursor::new(&leaf), &mut admission),
        Err(Refusal::Backing)
    );
    assert_eq!(stack.depth(), INLINE);
    assert!(std::ptr::eq(stack.top().expression(), &leaf));
    assert!(admission.units(0).is_err());

    let mut charge = |_: usize| true;
    let mut reserve = |_: usize| true;
    let mut admission = Admission::new(&mut charge, &mut reserve);
    let mut stack = Stack::new(&mut admission).unwrap();
    for _ in 0..LIMIT {
        stack.push(ChildCursor::new(&leaf), &mut admission).unwrap();
    }
    assert_eq!(stack.depth(), LIMIT);
    assert_eq!(
        stack.push(ChildCursor::new(&leaf), &mut admission),
        Err(Refusal::Capacity)
    );
    assert!(admission.reserve(0).is_err());
    while !stack.is_empty() {
        stack.pop();
    }
    assert_eq!(stack.depth(), 0);
}

#[test]
fn consuming_directory_still_matches_vec_family_oracle() {
    let source = crate::check(
        r#"module directory_consuming;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});
 let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0 }
"#,
        "consuming-directory.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
    let mut consuming = 0;
    for function in &program.functions {
        let mut charge = |_: usize| true;
        let mut reserve = |_: usize| true;
        let mut admission = Admission::new(&mut charge, &mut reserve);
        let directory = Directory::collect(&program, function, &helpers, &mut admission).unwrap();
        consuming += directory
            .entries
            .iter()
            .filter(|entry| entry.rank == 0 && entry.first)
            .count();
        for (at, expected) in crate::hir::vec_loop_renewal::bindings_in(&program, function) {
            let (binding, families) = directory
                .receive(&program, function, &at, &mut admission)
                .unwrap()
                .unwrap();
            assert!(std::ptr::eq(binding, expected));
            assert!(Directory::contains(families, Family::Vec));
        }
    }
    assert!(consuming > 0, "consuming owned-record emission preserved");
}

#[test]
fn full_consuming_collector_matches_unchanged_original() {
    let source = crate::check(
        r#"module consuming_full_control;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("plain") fn plain()->i64 {0}
@id("main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});
 let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0 }
"#,
        "consuming-full-control.spx",
    ).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let mut emitted = 0;
    for function in &program.functions {
        let collect = |original: bool| {
            let work = Cell::new(0usize);
            let backing = Cell::new(0usize);
            let mut charge = |units| {
                work.set(work.get() + units);
                true
            };
            let mut reserve = |bytes| {
                backing.set(backing.get() + bytes);
                true
            };
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let mut output = super::super::Scratch::new(&mut admission).unwrap();
            if original {
                old::collect(&program, function, &mut output, &mut admission).unwrap();
            } else {
                super::collect(&program, function, &mut output, &mut admission).unwrap();
            }
            (output.into_vec(), work.get(), backing.get())
        };
        let (before, old_work, old_backing) = collect(true);
        let (after, new_work, new_backing) = collect(false);
        assert_eq!(old_work, new_work);
        assert_eq!(before.len(), after.len());
        emitted += after.len();
        for (before, after) in before.iter().zip(&after) {
            assert!(std::ptr::eq(before.at, after.at));
            assert!(std::ptr::eq(before.binding, after.binding));
            assert_eq!(
                (
                    before.families,
                    before.family,
                    before.ordinal,
                    before.rank,
                    before.first
                ),
                (
                    after.families,
                    after.family,
                    after.ordinal,
                    after.rank,
                    after.first
                ),
            );
        }
        if function.id.as_str() == "plain" {
            assert!(new_backing < old_backing);
            assert_eq!(
                old_backing - new_backing,
                size_of::<old::Frames<'static>>() - size_of::<Stack<'static>>()
            );
        }
    }
    assert!(
        emitted > 0,
        "complete consuming control must exercise real emission"
    );
}
