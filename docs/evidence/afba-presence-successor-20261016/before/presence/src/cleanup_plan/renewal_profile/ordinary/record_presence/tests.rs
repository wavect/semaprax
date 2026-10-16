use super::*;
use crate::test_allocation_observer::measure;
use std::cell::Cell;
fn constructor_layout() -> usize {
    use std::mem::size_of;
    2 * size_of::<RecordPresence<'_>>()
        + size_of::<bool>()
        + size_of::<Result<RecordPresence<'_>, Refusal>>()
        + size_of::<Result<(), Refusal>>()
        + size_of::<&mut Admission<'_>>()
}
fn read_layout() -> usize {
    use std::mem::size_of;
    2 * size_of::<Result<bool, Refusal>>()
        + 2 * size_of::<Result<(), Refusal>>()
        + size_of::<&RecordPresence<'_>>()
        + 4 * size_of::<&DeclarationIndex>()
        + 4 * size_of::<&mut Admission<'_>>()
}

fn program(record: bool) -> crate::hir::ResolvedProgram {
    let record = if record {
        "@id(\"row\") record Row {@id(\"row.n\") n:i64,}"
    } else {
        ""
    };
    let source = format!("module presence; {record} @id(\"main\") fn main()->i64 {{0}}");
    crate::hir::resolve(&crate::check(&source, "record-presence.spx").unwrap()).unwrap()
}

// The full old prefix, independent of the cached result and its constructor.
fn old_prefix(index: &DeclarationIndex) -> (bool, usize) {
    let mut visited = 0;
    for declaration in index.declarations() {
        visited += 1;
        if declaration.kind == DeclarationKind::Record
            && declaration.identity_origin == IdentityOrigin::Explicit
        {
            return (true, visited);
        }
    }
    (false, visited)
}

#[test]
fn presence_matches_complete_old_prefix_and_authenticates_the_exact_immutable_index() {
    assert_eq!(
        RecordPresence::constructor_controls() + RecordPresence::acquisition_receiving_controls(),
        constructor_layout()
    );
    assert_eq!(RecordPresence::read_controls(), read_layout());
    for record in [false, true] {
        let program = program(record);
        let foreign = program.clone();
        let index = &program.declarations;
        let (expected, steps) = old_prefix(index);
        assert_eq!(expected, record);
        let work = Cell::new(0usize);
        let paid = Cell::new(0usize);
        let mut charge = |units| {
            work.set(work.get() + units);
            true
        };
        let mut reserve = |bytes| {
            paid.set(paid.get() + bytes);
            true
        };
        let mut admission = Admission::new(&mut charge, &mut reserve);
        let ((), allocation) = measure(|| {
            admission
                .reserve(RecordPresence::acquisition_receiving_controls())
                .unwrap();
            let presence = RecordPresence::new(index, &mut admission).unwrap();
            assert_eq!(work.get(), steps);
            admission.reserve(18 * read_layout()).unwrap();
            for _ in 0..16 {
                assert_eq!(presence.get(index, &mut admission), Ok(expected));
            }
            assert_eq!(work.get(), steps + 16);
            assert_eq!(
                presence.get(&foreign.declarations, &mut admission),
                Err(Refusal::Subject)
            );
            assert!(presence.get(index, &mut admission).is_err());
        });
        assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
        assert_eq!(paid.get(), constructor_layout() + 18 * read_layout());
    }
}

#[test]
fn exact_metadata_and_census_work_and_one_short_fail_closed_without_allocation() {
    for record in [false, true] {
        let program = program(record);
        let index = &program.declarations;
        let (expected, steps) = old_prefix(index);
        let constructor = constructor_layout();
        let metadata = constructor + 2 * read_layout();
        for (bytes, units, succeeds) in [
            (metadata, steps + 1, true),
            (constructor - 1, steps + 1, false),
            (metadata, steps - 1, false),
        ] {
            let backing = Cell::new(bytes);
            let work = Cell::new(units);
            let mut reserve = |bytes| match backing.get().checked_sub(bytes) {
                Some(left) => {
                    backing.set(left);
                    true
                }
                None => false,
            };
            let mut charge = |units| match work.get().checked_sub(units) {
                Some(left) => {
                    work.set(left);
                    true
                }
                None => false,
            };
            let mut admission = Admission::new(&mut charge, &mut reserve);
            let (success, allocation) = measure(|| {
                admission
                    .reserve(RecordPresence::acquisition_receiving_controls())
                    .unwrap();
                match RecordPresence::new(index, &mut admission) {
                    Ok(presence) => {
                        admission.reserve(2 * read_layout()).unwrap();
                        assert_eq!(presence.get(index, &mut admission), Ok(expected));
                        // No remaining work is recreated by the held proof.
                        assert_eq!(presence.get(index, &mut admission), Err(Refusal::Work));
                        true
                    }
                    Err(_) => {
                        assert!(admission.reserve(0).is_err());
                        assert!(admission.units(0).is_err());
                        false
                    }
                }
            });
            assert_eq!(success, succeeds);
            assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
            if bytes < constructor {
                assert_eq!(work.get(), units);
            }
        }
        // The read has its own exact positive receiving frontier; acquisition
        // does not donate one paid slot to arbitrarily many future reads.
        for (read_bytes, succeeds) in [
            (read_layout(), true),
            (read_layout() - 1, false),
            (0, false),
        ] {
            let remaining = Cell::new(constructor + read_bytes);
            let mut work = |_| true;
            let mut backing = |bytes| match remaining.get().checked_sub(bytes) {
                Some(left) => {
                    remaining.set(left);
                    true
                }
                None => false,
            };
            let mut admission = Admission::new(&mut work, &mut backing);
            admission
                .reserve(RecordPresence::acquisition_receiving_controls())
                .unwrap();
            let presence = RecordPresence::new(index, &mut admission).unwrap();
            let (success, allocation) = measure(|| {
                if admission.reserve(read_layout()).is_err() {
                    // No read Result/receiver is constructed when its bank
                    // was not acquired; this is Directory setup refusal.
                    return false;
                }
                assert_eq!(presence.get(index, &mut admission), Ok(expected));
                true
            });
            assert_eq!(success, succeeds);
            assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
        }
    }
}

#[test]
fn scalar_callable_boundary_reuses_one_census_under_the_unchanged_work_limit() {
    let mut source = String::from("module boundary;\n");
    for index in 0..1024 {
        let name = if index == 0 {
            "main".to_owned()
        } else {
            format!("f{index}")
        };
        source.push_str(&format!("@id(\"c.f{index}\") fn {name}()->i64 {{0}}\n"));
    }
    let program =
        crate::hir::resolve(&crate::check(&source, "presence-callables.spx").unwrap()).unwrap();
    let index = &program.declarations;
    let (found, steps) = old_prefix(index);
    assert!(!found && steps >= 1024);
    let unchanged_limit = crate::cache_codec::MAX_NODES;
    assert!(steps.checked_mul(1024).unwrap() > unchanged_limit);
    let left = Cell::new(unchanged_limit);
    let mut work = |units| match left.get().checked_sub(units) {
        Some(n) => {
            left.set(n);
            true
        }
        None => false,
    };
    let mut backing = |_| true;
    let mut admission = Admission::new(&mut work, &mut backing);
    admission
        .reserve(RecordPresence::acquisition_receiving_controls())
        .unwrap();
    let presence = RecordPresence::new(index, &mut admission).unwrap();
    admission.reserve(1024 * read_layout()).unwrap();
    for _ in 0..1024 {
        assert_eq!(presence.get(index, &mut admission), Ok(false));
    }
    assert_eq!(unchanged_limit - left.get(), steps + 1024);
}

#[test]
fn prior_enclosing_refusal_cannot_be_hidden_by_a_prepared_presence_proof() {
    let program = program(false);
    let size = constructor_layout() + read_layout();
    let (((), observed), refused, paid) = crate::bounded_output::with_limit_usage(size, || {
        measure(|| {
            let mut work = |_| true;
            let mut backing = crate::bounded_output::reserve_active_required;
            let mut admission = Admission::new(&mut work, &mut backing);
            admission
                .reserve(RecordPresence::acquisition_receiving_controls())
                .unwrap();
            let presence = RecordPresence::new(&program.declarations, &mut admission).unwrap();
            admission.reserve(read_layout()).unwrap();
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert_eq!(
                presence.get(&program.declarations, &mut admission),
                Err(Refusal::Backing)
            );
            assert!(admission.units(0).is_err());
        })
    });
    assert!(refused);
    assert_eq!(paid, size);
    assert!(!observed.invalid && observed.allocated == 0 && observed.live == 0);
}

#[test]
fn full_original_collector_and_phase_source_inverses_remain_exact() {
    use sha2::{Digest, Sha256};
    fn immediate_main_input(path: &str, current: &str) -> String {
        let direct_previous = super::super::direct_presence_tests::previous(path, current);
        let current = direct_previous.as_str();
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/record-presence-controls-850-20261016/runtime-inputs.json"
        )))
        .unwrap();
        let caller_rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/record-presence-caller-frontier-850-20261016/runtime-inputs.json"
        )))
        .unwrap();
        let previous;
        let current = if let Some(row) = caller_rows.get(path) {
            assert_eq!(row["current"].as_str().unwrap(), current);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
                ),
                row["current_sha256"].as_str().unwrap()
            );
            previous = row["before"].as_str().unwrap().to_owned();
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
                ),
                row["before_sha256"].as_str().unwrap()
            );
            previous.as_str()
        } else {
            current
        };
        let row = &rows[path];
        assert_eq!(row["current"].as_str().unwrap(), current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"].as_str().unwrap()
        );
        let previous = row["before"].as_str().unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
            ),
            row["before_sha256"].as_str().unwrap()
        );
        previous.to_owned()
    }
    let _complete_main_presence = immediate_main_input(
        "src/cleanup_plan/renewal_profile/ordinary/record_presence.rs",
        include_str!("../record_presence.rs"),
    );
    let packets = [
        (
            "src/cleanup_plan/renewal_profile/ordinary/consuming.rs",
            include_str!("../consuming.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/record-presence-phase-v1/consuming.before.rs.txt"
            )),
            "2b38148b70b905f4e77a15fb32a9da6981d1823aaf8db5b2586a514f62f94a43",
        ),
        (
            "src/cleanup_plan/renewal_profile/ordinary/mod.rs",
            include_str!("../mod.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/record-presence-phase-v1/ordinary.before.rs.txt"
            )),
            "3d144f7b6e1cab723dc866b2dc72fe367a5f99a6683534ffb703f605ee35b161",
        ),
        (
            "src/loop_calls/byte_writer/module_index/ordinary.rs",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/loop_calls/byte_writer/module_index/ordinary.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/record-presence-phase-v1/module_index.before.rs.txt"
            )),
            "43d7320266650d043041012a2283dc3acb07951fa65f4860d582d7af50d27388",
        ),
    ];
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/record-presence-phase-v1/inverse.json"
    )))
    .unwrap();
    for (path, current, old, hash) in packets {
        let current = immediate_main_input(path, current);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
            ),
            hash
        );
        let mut restored = current.to_owned();
        for change in inverse[path].as_array().unwrap().iter().rev() {
            let start = change[0].as_u64().unwrap() as usize;
            let new = change[1].as_str().unwrap();
            let previous = change[2].as_str().unwrap();
            assert_eq!(restored.get(start..start + new.len()), Some(new));
            restored.replace_range(start..start + new.len(), previous);
        }
        assert_eq!(restored, old);
    }
    let original = include_str!("../consuming/old.rs");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "407d4f111b7b432fdbeea0ecffcf227c718cb84a7687576e8c3f7d624c5f7a7a"
    );
    let marker = "pub(super) fn collect<'a>(";
    assert_eq!(
        original.split_once(marker).unwrap().1,
        packets[0].2.split_once(marker).unwrap().1
    );
}
