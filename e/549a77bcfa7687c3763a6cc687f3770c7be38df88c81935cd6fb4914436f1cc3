//! Legacy construction and work are compared with the complete original owner.
use super::*;

#[test]
fn unobserved_cursor_keeps_the_original_fixed_allocation_and_zero_relocation_work() {
    let program = program();
    let function = &program.functions[0];
    let mut old_budget = ReplayBudget::unobserved();
    let mut current_budget = ReplayBudget::unobserved();
    let ((), original) = measure(|| {
        let cursor = before::Scratch::new(function, &mut old_budget).unwrap();
        drop(cursor);
    });
    let ((), current) = measure(|| {
        let mut cursor = Scratch::new(function, &mut current_budget).unwrap();
        assert_eq!(cursor.frames.capacity(), FRAME_CAPACITY);
        assert_eq!(current_budget.remaining, old_budget.remaining);
        // No relocation is needed, even when the original work allowance is
        // exhausted. Capacity remains storage, never traversal authority.
        current_budget.remaining = 0;
        fill(&mut cursor, function);
        preserved(&cursor, function);
        let pointer = cursor.frames.as_ptr();
        reserve_frames(&mut cursor.frames, 0, function, &mut current_budget).unwrap();
        assert_eq!(cursor.frames.as_ptr(), pointer);
        cursor.frames.clear();
        for _ in 0..3 {
            reserve_frames(
                &mut cursor.frames,
                FRAME_CAPACITY,
                function,
                &mut current_budget,
            )
            .unwrap();
            assert_eq!(cursor.frames.as_ptr(), pointer);
        }
        assert_eq!(current_budget.remaining, 0);
        drop(cursor);
    });
    assert_eq!(current, original);
    assert!(!current.invalid);
    assert_eq!(current.allocations, 1);
    assert_eq!(current.allocated, FRAME_CAPACITY * size_of::<Frame<'_>>());
    assert_eq!(current.allocated, current.freed);
    assert_eq!(current.live, 0);
    assert_eq!(old_budget.remaining, MAX_REPLAY_WORK_UNITS);
}

#[test]
fn legacy_cursor_complete_sources_preserve_original_walk_and_paid_growth() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/supplemental-legacy-cursor-v1");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(records.as_array().unwrap().len(), 2);
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let restore = |text: &str| -> Option<String> {
            let mut text = text.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if text.matches(new).count() != 1 {
                    return None;
                }
                text = text.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(text)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
        if record["path"] == "src/cleanup_plan/replay/supplemental/receiving.rs" {
            let start = "pub(in crate::cleanup_plan::replay) fn walk";
            let end = "// Capacity never supplies traversal authority.";
            let body = |text: &str| {
                text.split_once(start)
                    .unwrap()
                    .1
                    .split_once(end)
                    .unwrap()
                    .0
                    .to_owned()
            };
            assert_eq!(body(&current), body(&original), "entire receiving walk");
        }
    }
}
