//! Caller-owned source, helper inventory and ledger callbacks are outside the
//! directory window in both arms. Presence construction and every directory
//! owner/backing live through their real Drop inside that same window.
use super::*;
use crate::test_allocation_observer::measure;
use std::cell::Cell;

fn program(record: bool) -> ResolvedProgram {
    let source = if record {
        r#"module phase_presence;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("plain") fn plain()->i64 {0}
@id("main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});
 let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0 }
"#
    } else {
        "module direct_presence; @id(\"plain\") fn plain()->i64 {0} @id(\"main\") fn main()->i64 {0}"
    };
    crate::hir::resolve(&crate::check(source, "direct-presence.spx").unwrap()).unwrap()
}

fn equal(a: &Directory<'_>, b: &Directory<'_>) {
    assert!(std::ptr::eq(a.program, b.program));
    assert!(std::ptr::eq(a.function, b.function));
    assert_eq!(a.entries.len(), b.entries.len());
    for (a, b) in a.entries.iter().zip(&b.entries) {
        assert!(std::ptr::eq(a.at, b.at));
        assert!(std::ptr::eq(a.binding, b.binding));
        assert_eq!(
            (a.families, a.family, a.ordinal, a.rank, a.first),
            (b.families, b.family, b.ordinal, b.rank, b.first)
        );
    }
}
#[test]
fn direct_presence_keeps_complete_old_directory_work_and_real_receiving_drop() {
    for record in [false, true] {
        let p = program(record);
        let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&p).unwrap();
        for member in current_members(&p) {
            let mut work = |_| true;
            let mut backing = |_| true;
            let mut admission = Admission::new(&mut work, &mut backing);
            admission
                .reserve(RecordPresence::acquisition_receiving_controls())
                .unwrap();
            let presence = RecordPresence::new(&p.declarations, &mut admission).unwrap();
            admission
                .reserve(RecordPresence::shared_caller_controls())
                .unwrap();
            let expected = Directory::collect_current_with_presence(
                member,
                &helpers,
                &presence,
                &mut admission,
            )
            .unwrap();
            let observe = |direct: bool, limit: usize, units: usize, floor: usize| {
                let left = Cell::new(limit);
                let remaining = Cell::new(units);
                let mut work = |n| match remaining.get().checked_sub(n) {
                    Some(x) => {
                        remaining.set(x);
                        true
                    }
                    None => false,
                };
                let mut backing = |n| match left.get().checked_sub(n) {
                    Some(x) if x >= floor => {
                        left.set(x);
                        true
                    }
                    _ => false,
                };
                let mut admission = Admission::new(&mut work, &mut backing);
                let (result, seen) = measure(|| -> Result<(), Refusal> {
                    admission.reserve(RecordPresence::acquisition_receiving_controls())?;
                    let presence = RecordPresence::new(&p.declarations, &mut admission)?;
                    let bank = if direct {
                        RecordPresence::direct_caller_controls()
                    } else {
                        RecordPresence::shared_caller_controls()
                    };
                    admission.reserve(bank)?;
                    let actual = if direct {
                        Directory::collect_current_shared(
                            member,
                            &helpers,
                            &presence,
                            &mut admission,
                        )
                    } else {
                        Directory::collect_current_with_presence(
                            member,
                            &helpers,
                            &presence,
                            &mut admission,
                        )
                    }?;
                    equal(&actual, &expected);
                    drop(actual);
                    drop(presence);
                    Ok(())
                });
                if result.is_err() {
                    assert_eq!(admission.reserve(0), Err(Refusal::Backing));
                    assert_eq!(admission.units(0), Err(Refusal::Work));
                }
                assert!(!seen.invalid);
                assert_eq!(seen.live, 0);
                assert_eq!(seen.allocated, seen.freed);
                (result, limit - left.get(), units - remaining.get(), seen)
            };
            let old = observe(false, usize::MAX, usize::MAX, 0);
            let new = observe(true, usize::MAX, usize::MAX, 0);
            assert_eq!(old.0, Ok(()));
            assert_eq!(new.0, Ok(()));
            assert_eq!(old.2, new.2);
            assert_eq!(old.3.allocated, new.3.allocated);
            // Actual new receiver layouts, not an observed deficit donation.
            assert_eq!(
                old.1 - RecordPresence::shared_caller_controls(),
                new.1 - RecordPresence::direct_caller_controls()
            );
            assert!(new.1 < old.1);
            assert_eq!(observe(true, new.1, usize::MAX, 0).0, Ok(()));
            assert!(observe(true, new.1 - 1, usize::MAX, 0).0.is_err());
            assert!(observe(true, 0, usize::MAX, 0).0.is_err());
            assert_eq!(observe(true, new.1 + 17, usize::MAX, 17).0, Ok(()));
            assert!(observe(true, new.1 + 16, usize::MAX, 17).0.is_err());
            assert_eq!(observe(true, usize::MAX, new.2, 0).0, Ok(()));
            assert!(observe(true, usize::MAX, new.2 - 1, 0).0.is_err());
        }
    }
}

#[test]
fn direct_presence_preserves_original_subject_and_sticky_work_order() {
    let p = program(true);
    let other = program(true);
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&p).unwrap();
    for direct in [false, true] {
        let mut work = |_| true;
        let mut reserve = |_| true;
        let mut admission = Admission::new(&mut work, &mut reserve);
        admission
            .reserve(RecordPresence::acquisition_receiving_controls())
            .unwrap();
        let presence = RecordPresence::new(&other.declarations, &mut admission).unwrap();
        let member = current_member_at(&p, 0).unwrap();
        admission
            .reserve(if direct {
                RecordPresence::direct_caller_controls()
            } else {
                RecordPresence::shared_caller_controls()
            })
            .unwrap();
        let result = if direct {
            Directory::collect_current_shared(member, &helpers, &presence, &mut admission)
        } else {
            Directory::collect_current_with_presence(member, &helpers, &presence, &mut admission)
        };
        assert!(matches!(result, Err(Refusal::Subject)));
        assert_eq!(admission.units(1), Err(Refusal::Work));
        assert_eq!(admission.reserve(0), Err(Refusal::Backing));
    }
}

// Whole current->f82 source view before every unchanged Main269/older splice.
// The only raw sources consumed are the three closed production rows.
pub(super) fn previous(path: &str, current: &str) -> String {
    use sha2::{Digest, Sha256};
    let table: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/direct-presence-receiving-850-next-20261016/runtime-inputs.json"
    )))
    .unwrap();
    let Some(row) = table.get(path) else {
        return current.to_owned();
    };
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    assert_eq!(digest(current), row["current_sha256"].as_str().unwrap());
    let mut before = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        let old = edit["before"].as_str().unwrap();
        assert_eq!(before.matches(after).count(), 1);
        before = before.replacen(after, old, 1);
    }
    assert_eq!(digest(&before), row["before_sha256"].as_str().unwrap());
    before
}
