//! Full predecessor constructors, exact new work, real backing and final Drop.
use super::*;
#[path = "../oracle_closure_before/custody.rs"]
mod oracle_closure_custody;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(
            "module reached; @id(\"reached.main\") fn main()->i64 { 1 + 2 + 3 }",
            "reached.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn lent_budget() -> ReplayBudget {
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    budget.lent_skeleton = true;
    budget
}
fn fill<'a>(frames: &mut Vec<Frame<'a>>, function: &'a ResolvedFunction) {
    while frames.len() < frames.capacity() {
        if frames.len().is_multiple_of(2) {
            frames.push(Frame::Eval(&function.body));
        } else {
            frames.push(Frame::UpcastPassthrough);
        }
    }
}
fn order(frames: &[Frame<'_>], function: &ResolvedFunction) {
    for (index, frame) in frames.iter().enumerate() {
        match frame {
            Frame::Eval(expression) if index.is_multiple_of(2) => {
                assert!(std::ptr::eq(*expression, &function.body))
            }
            Frame::UpcastPassthrough if !index.is_multiple_of(2) => {}
            _ => panic!("relocation changed a real continuation"),
        }
    }
}

#[test]
fn legacy_constructor_retains_original_reservation_and_bounds_real_requested_backing() {
    let program = program();
    let function = &program.functions[0];
    let observe = |old| {
        measure(|| {
            let mut budget = lent_budget();
            let before = budget.remaining;
            let frames = if old {
                reached_before::fresh(function, &mut budget)
            } else {
                fresh(function, &mut budget)
            }
            .unwrap();
            let capacity = frames.capacity();
            assert_eq!(budget.remaining, before);
            drop(frames);
            drop(budget);
            capacity
        })
    };
    let (old_capacity, old) = observe(true);
    let (current_capacity, current) = observe(false);
    assert_eq!((old_capacity, current_capacity), (516, 2));
    assert_eq!(old.allocated, 516 * std::mem::size_of::<Frame<'_>>());
    assert_eq!(current.allocated, 2 * std::mem::size_of::<Frame<'_>>());
    for metrics in [old, current] {
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
    // This is genuine Legacy storage evidence, never a claimed paying invoice.
    assert!(matches!(
        ReplayBudget::unobserved().payer,
        AllocationPayer::LegacyUnobserved
    ));
}

#[test]
fn legacy_growth_preserves_order_original_depth_and_each_extra_work_debit() {
    let program = program();
    let function = &program.functions[0];
    let mut original_budget = lent_budget();
    let mut original = reached_before::fresh(function, &mut original_budget).unwrap();
    fill(&mut original, function);
    let original_depth =
        reached_before::reserve_push(&mut original, function, &mut original_budget).unwrap_err();
    drop(original);
    reached::reset();
    let ((growth_work, slots), physical) = measure(|| {
        let mut budget = lent_budget();
        let before = budget.remaining;
        let mut frames = fresh(function, &mut budget).unwrap();
        let mut work = 0;
        let mut slots = 2;
        for target in [4, 8, 16, 32, 64, 128, 256, 512, 516] {
            fill(&mut frames, function);
            let old_capacity = frames.capacity();
            work += 8 + 2 * old_capacity;
            reserve_push(&mut frames, function, &mut budget).unwrap();
            slots += target;
            assert_eq!(frames.capacity(), target);
            assert_eq!(frames.len(), old_capacity);
            order(&frames, function);
        }
        assert_eq!(budget.remaining, before);
        assert_eq!(budget.skeleton_spent, work);
        assert_eq!(reached::receipt().work, work);
        assert_eq!(reached::receipt().growths, 9);
        assert_eq!(reached::receipt().allocated_slots, slots - 2);
        fill(&mut frames, function);
        assert_eq!(frames.len(), 516);
        let pointer = frames.as_ptr();
        let error = reserve_push(&mut frames, function, &mut budget).unwrap_err();
        assert_eq!(error, original_depth);
        drop(error);
        assert_eq!(frames.as_ptr(), pointer);
        assert_eq!(frames.capacity(), 516);
        order(&frames, function);
        drop(frames);
        drop(budget);
        (work, slots)
    });
    assert_eq!(
        growth_work,
        8 * 9 + 2 * (2 + 4 + 8 + 16 + 32 + 64 + 128 + 256 + 512)
    );
    // The error Diagnostic also owns its original text; no fake allocation or
    // capacity credit is subtracted from this complete observed lifetime.
    assert!(physical.allocated >= slots * std::mem::size_of::<Frame<'_>>());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    // A lent budget spends exactly the same extra work from its existing pool.
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    budget.lent_skeleton = true;
    let remaining = budget.remaining;
    let mut frames = fresh(function, &mut budget).unwrap();
    fill(&mut frames, function);
    reserve_push(&mut frames, function, &mut budget).unwrap();
    assert_eq!(budget.remaining, remaining);
    assert_eq!(budget.skeleton_spent, 12);
    assert_eq!(budget.skeleton_remaining, MAX_REPLAY_WORK_UNITS - 12);
}

#[test]
fn legacy_partial_failure_drops_owned_frames_and_preserves_complete_sticky_diagnostic() {
    let program = program();
    let function = &program.functions[0];
    for available in [0, 11, 12] {
        let mut expected_budget = lent_budget();
        expected_budget.skeleton_remaining = available;
        let expected = expected_budget.charge(function, 12, "held continuation relocation");
        reached::reset();
        let ((), physical) = measure(|| {
            let mut budget = lent_budget();
            let mut frames = fresh(function, &mut budget).unwrap();
            // Actual owned buffers survive a refused grow or a successful move,
            // and are destroyed once by the original continuation Drop path.
            for _ in 0..2 {
                frames.push(Frame::BinaryRight {
                    expression: &function.body,
                    op: BinaryOp::Add,
                    left_paths: Vec::with_capacity(17),
                });
            }
            let mut payloads = [std::ptr::null(); 2];
            for (index, frame) in frames.iter().enumerate() {
                let Frame::BinaryRight { left_paths, .. } = frame else {
                    unreachable!()
                };
                payloads[index] = left_paths.as_ptr();
            }
            budget.skeleton_remaining = available;
            let pointer = frames.as_ptr();
            let result = reserve_push(&mut frames, function, &mut budget);
            assert_eq!(
                result, expected,
                "all six Diagnostic fields at each real work cut"
            );
            assert_eq!(reached::receipt().growths, usize::from(available == 12));
            assert_eq!(frames.capacity(), if available == 12 { 4 } else { 2 });
            if available != 12 {
                assert_eq!(frames.as_ptr(), pointer);
            }
            assert_eq!(frames.len(), 2);
            for (frame, payload) in frames.iter().zip(&payloads) {
                let Frame::BinaryRight { left_paths, .. } = frame else {
                    unreachable!()
                };
                assert_eq!(left_paths.capacity(), 17);
                assert_eq!(left_paths.as_ptr(), *payload);
            }
            while frames.len() < frames.capacity() {
                frames.push(Frame::UpcastPassthrough);
            }
            let remaining = budget.skeleton_remaining;
            let retry = reserve_push(&mut frames, function, &mut budget);
            assert!(retry.is_err());
            drop(retry);
            assert_eq!(budget.skeleton_remaining, remaining);
            drop(result);
            drop(frames);
            drop(budget);
        });
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        assert!(physical.allocated >= 34 * std::mem::size_of::<ExprSkeletonPath>());
    }
}

#[test]
fn legacy_held_roots_nested_slots_and_nonlegacy_scope_keep_independent_old_behavior() {
    let program = program();
    let function = &program.functions[0];
    let mut old = reached_before::Storage::new(&program);
    let mut current = Storage::new(&program);
    let mut old_budget = lent_budget();
    let mut current_budget = lent_budget();
    let (oa, outer_old) = old.acquire(function, &mut old_budget).unwrap();
    let (ca, outer) = current.acquire(function, &mut current_budget).unwrap();
    let (ob, inner_old) = old.acquire(function, &mut old_budget).unwrap();
    let (cb, inner) = current.acquire(function, &mut current_budget).unwrap();
    assert_eq!((outer_old.capacity(), inner_old.capacity()), (516, 516));
    assert_eq!((outer.capacity(), inner.capacity()), (2, 2));
    let outer_ptr = outer.as_ptr();
    let inner_ptr = inner.as_ptr();
    assert_ne!(outer_ptr, inner_ptr);
    old.release(ob, inner_old);
    current.release(cb, inner);
    let (cb, inner) = current.acquire(function, &mut current_budget).unwrap();
    assert_eq!(inner.as_ptr(), inner_ptr);
    current.release(cb, inner);
    old.release(oa, outer_old);
    current.release(ca, outer);
    current_budget.lent_skeleton = true;
    let (ca, outer) = current.acquire(function, &mut current_budget).unwrap();
    assert_eq!(
        outer.as_ptr(),
        outer_ptr,
        "lent mode cannot replace drained reached backing with516"
    );
    assert_eq!(outer.capacity(), 2);
    current.release(ca, outer);
    let foreign = program.clone();
    assert!(!old.matches(&foreign) && !current.matches(&foreign));
    // Nonlent Legacy remains the complete predecessor constructor, including
    // a captured receiving owner whose payer is replaced inside the same scope.
    let mut nonlent = ReplayBudget::unobserved();
    assert_eq!(fresh(function, &mut nonlent).unwrap().capacity(), 516);
    with_limit_usage_refusal(usize::MAX, || {
        let mut old_budget = ReplayBudget::new();
        let mut current_budget = ReplayBudget::new();
        let mut old = reached_before::Storage::for_program(&program, &mut old_budget).unwrap();
        let mut current = Storage::for_program(&program, &mut current_budget).unwrap();
        old_budget.payer = AllocationPayer::LegacyUnobserved;
        current_budget.payer = AllocationPayer::LegacyUnobserved;
        let before = old.acquire(function, &mut old_budget);
        let after = current.acquire(function, &mut current_budget);
        assert!(before.is_err() && after.is_err());
        assert_eq!(
            before.err().unwrap(),
            after.err().unwrap(),
            "all six cold nonlent receiving Diagnostic fields"
        );
        old.frames = Some(Vec::with_capacity(2));
        current.frames = Some(Vec::with_capacity(2));
        let (oa, mut before) = old.acquire(function, &mut old_budget).unwrap();
        let (ca, mut after) = current.acquire(function, &mut current_budget).unwrap();
        assert_eq!(before.capacity(), after.capacity());
        fill(&mut before, function);
        fill(&mut after, function);
        assert_eq!(
            reached_before::reserve_push(&mut before, function, &mut old_budget),
            reserve_push(&mut after, function, &mut current_budget),
            "complete original warm nonlent receiving failure"
        );
        before.clear();
        after.clear();
        old.release(oa, before);
        current.release(ca, after);
    });
    // Real original/current allocation payers retain the exact old fixed path.
    let observe = |before| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = if before {
                    reached_before::fresh(function, &mut budget)
                } else {
                    fresh(function, &mut budget)
                };
                let frames = result.unwrap();
                assert_eq!(frames.capacity(), 516);
                drop(frames);
                drop(budget);
            })
        })
    };
    let (((), a), ar, ap, af) = observe(true);
    let (((), b), br, bp, bf) = observe(false);
    assert_eq!((ar, ap, af), (br, bp, bf));
    assert!(!ar);
    assert_eq!(
        (a.allocated, a.freed, a.highwater),
        (b.allocated, b.freed, b.highwater)
    );
    assert!(!a.invalid && !b.invalid && a.live == 0 && b.live == 0);
}

fn source_before(path: &str, supplied: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = include_bytes!("reached_before/inverse.json");
    assert_eq!(
        hash(packet),
        "502ce5028fe9c180ff71c8e7f19ad6724f5a729244d828dadcc8ccdf1e59d129"
    );
    let packet_path =
        root.join("src/cleanup_plan/replay/continuation_storage/reached_before/inverse.json");
    assert_eq!(
        oracle_closure_custody::exact_compiled(&packet_path, std::str::from_utf8(packet).unwrap())
            .as_bytes(),
        packet
    );
    let rows: serde_json::Value = serde_json::from_slice(packet).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let compiled = match path {
        "src/cleanup_plan/replay/continuation_storage.rs" => {
            include_str!("../continuation_storage.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/reached_before.rs" => {
            include_str!("reached_before.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/tests.rs" => include_str!("tests.rs"),
        _ => panic!("foreign reached continuation source"),
    };
    assert_eq!(supplied, compiled);
    assert_eq!(
        oracle_closure_custody::exact_compiled(&root.join(path), compiled).as_bytes(),
        compiled.as_bytes()
    );
    let live_compiled = compiled;
    // Reconstruct the ENTIRE immediate R image before the unchanged older
    // packet applies. The adapter authenticates every new module, not just
    // the root that dispatches to them; foreign/drift input still fails first.
    let immediate = if path == "src/cleanup_plan/replay/continuation_storage.rs" {
        oracle_closure_custody::restore(path, compiled)
    } else {
        compiled.to_owned()
    };
    let compiled = immediate.as_str();
    let row = rows.iter().find(|row| row["path"] == path).unwrap();
    assert_eq!(
        compiled.len(),
        row["current_length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hash(compiled.as_bytes()),
        row["current_sha256"].as_str().unwrap()
    );
    let (fixture, target) = match path {
        "src/cleanup_plan/replay/continuation_storage.rs"
        | "src/cleanup_plan/replay/continuation_storage/reached_before.rs" => (
            include_str!("reached_before/continuation_storage.rs.txt"),
            (
                "50e0fcee065b5222376f153597daa16ffc1044875e286c0bfc92ecf50e88a25f",
                14296,
            ),
        ),
        _ => (
            include_str!("reached_before/tests.rs.txt"),
            (
                "ef071c7def7542ab0365cdd9566779a4b37270365bf6c147df7e87122debbdc2",
                27342,
            ),
        ),
    };
    assert_eq!(row["before_length"].as_u64().unwrap() as usize, target.1);
    assert_eq!(row["before_sha256"].as_str().unwrap(), target.0);
    assert_eq!(
        (fixture.len(), hash(fixture.as_bytes())),
        (target.1, target.0.to_owned())
    );
    let fixture_path = if path.ends_with("/tests.rs") {
        "tests.rs.txt"
    } else {
        "continuation_storage.rs.txt"
    };
    let fixture_path = root
        .join("src/cleanup_plan/replay/continuation_storage/reached_before")
        .join(fixture_path);
    assert_eq!(
        oracle_closure_custody::exact_compiled(&fixture_path, fixture).as_bytes(),
        fixture.as_bytes()
    );
    let mut restored = compiled.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let a = edit["start"].as_u64().unwrap() as usize;
        let b = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&restored[a..b], edit["current"].as_str().unwrap());
        restored.replace_range(a..b, edit["before"].as_str().unwrap());
    }
    assert_eq!(restored, fixture);
    assert_eq!(
        oracle_closure_custody::exact_compiled(&root.join(path), live_compiled).as_bytes(),
        live_compiled.as_bytes()
    );
    assert_eq!(
        oracle_closure_custody::exact_compiled(&fixture_path, fixture).as_bytes(),
        fixture.as_bytes()
    );
    assert_eq!(
        oracle_closure_custody::exact_compiled(&packet_path, std::str::from_utf8(packet).unwrap())
            .as_bytes(),
        packet
    );
    restored
}

#[test]
fn complete_immediate_old_sources_and_unchanged_shared_frame_declarations_remain_exact() {
    use sha2::{Digest, Sha256};
    let root = source_before(
        "src/cleanup_plan/replay/continuation_storage.rs",
        include_str!("../continuation_storage.rs"),
    );
    let independent = source_before(
        "src/cleanup_plan/replay/continuation_storage/reached_before.rs",
        include_str!("reached_before.rs"),
    );
    assert_eq!(root, independent);
    source_before(
        "src/cleanup_plan/replay/continuation_storage/tests.rs",
        include_str!("tests.rs"),
    );
    let frame_start = "/// An owning traversal's Step assignment:";
    let frame_end = "// A current invocation-held receiver";
    let old_frame = root
        .split_once(frame_start)
        .unwrap()
        .1
        .split_once(frame_end)
        .unwrap()
        .0;
    let current_frame = include_str!("../continuation_storage.rs")
        .split_once(frame_start)
        .unwrap()
        .1
        .split_once(frame_end)
        .unwrap()
        .0;
    assert_eq!(
        current_frame, old_frame,
        "whole immutable shared Frame/OwnedStep declarations"
    );
    let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled, expected) in [
        (
            "src/cleanup_plan/replay/continuation_storage/paid_receiving.rs",
            include_str!("paid_receiving.rs"),
            "6557a6254c5a47b24cb4a7a6c300e7e4df21aec5cfcbafc0ac788f38bf8d5be3",
        ),
        (
            "src/cleanup_plan/replay/continuation_storage/reached.rs",
            include_str!("reached.rs"),
            "bf60981f88a31c20b7151d010bc1360461048573b9164e00dd8c9124305d9278",
        ),
    ] {
        assert_eq!(
            oracle_closure_custody::exact_compiled(&physical.join(path), compiled).as_bytes(),
            compiled.as_bytes()
        );
        let prior = if path == "src/cleanup_plan/replay/continuation_storage/reached.rs" {
            oracle_closure_custody::restore(path, compiled)
        } else {
            compiled.to_owned()
        };
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(prior.as_bytes()))
            ),
            expected
        );
        assert_eq!(
            oracle_closure_custody::exact_compiled(&physical.join(path), compiled).as_bytes(),
            compiled.as_bytes()
        );
    }
    let current = include_str!("../continuation_storage.rs");
    assert_eq!(current.matches("FRAME_CAPACITY: usize = 516").count(), 1);
    for foreign in [
        current.to_owned() + "\n",
        current[..current.len() - 1].to_owned(),
        current.replacen(
            "FRAME_CAPACITY: usize = 516",
            "FRAME_CAPACITY: usize = 517",
            1,
        ),
    ] {
        assert!(std::panic::catch_unwind(|| source_before(
            "src/cleanup_plan/replay/continuation_storage.rs",
            &foreign
        ))
        .is_err());
    }
    assert!(
        std::panic::catch_unwind(|| source_before("src/cleanup_plan/replay.rs", current)).is_err()
    );
}
