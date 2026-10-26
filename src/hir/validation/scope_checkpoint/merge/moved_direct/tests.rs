//! Authored whole receivers; no executed gain or application claim.
use super::*;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};
fn binding(count: usize, state: Availability) -> ValidationBinding {
    ValidationBinding {
        contract_local: false,
        mutable: true,
        ty: ResolvedType::Bytes,
        ownership: OwnershipMode::Own,
        availability: Availability::Available,
        active_loans: BTreeSet::new(),
        definitely_partial: BTreeSet::new(),
        moved_places: (0..count)
            .map(|i| {
                (
                    vec![PlaceProjection::Field(DeclarationId::new(format!(
                        "moved.field.{i:04}"
                    )))],
                    state,
                )
            })
            .collect(),
    }
}
fn seeded(input: &BTreeMap<ValueId, ValidationBinding>) -> Result<Scope, Diagnostic> {
    let mut scope = Scope::default();
    for (id, value) in input {
        let prepared = scope.prepare_insert()?;
        charge::reserve(charge::binding(value))?;
        prepared.insert(id.clone(), value.clone())?;
    }
    Ok(scope)
}
fn receive(input: &BTreeMap<ValueId, ValidationBinding>, branches: bool) -> Result<(), Diagnostic> {
    let mut target = seeded(input)?;
    let left = target.clone();
    let right = target.clone();
    let ids = target.ids();
    let expected = input;
    if branches {
        super::super::branches(&mut target, &left, &right, &ids)?;
    } else {
        super::super::conditional(&mut target, &left, &ids)?;
    }
    assert!(target.iter().eq(expected.iter()));
    assert!(left.iter().eq(input.iter()) && right.iter().eq(input.iter()));
    drop(ids);
    drop(right);
    drop(left);
    drop(target);
    Ok(())
}
fn complete(metrics: Metrics, paid: usize) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}
#[test]
fn borrowed_ordered_union_keeps_every_state_prefix_missing_branch_and_empty_case() {
    for count in [0, 1, 2, 17] {
        for a in [
            Availability::Available,
            Availability::Moved,
            Availability::MaybeMoved,
        ] {
            for b in [
                Availability::Available,
                Availability::Moved,
                Availability::MaybeMoved,
            ] {
                let left = binding(count, a);
                let mut right = binding(count, b);
                let keys = right.moved_places.keys().cloned().collect::<Vec<_>>();
                for key in keys.iter().step_by(2) {
                    right.moved_places.remove(key);
                }
                right.moved_places.insert(vec![], b);
                right.moved_places.insert(
                    vec![
                        PlaceProjection::Field(DeclarationId::new("moved.field.0000")),
                        PlaceProjection::Field(DeclarationId::new("nested")),
                    ],
                    b,
                );
                let expected = HirValidator::join_moved_places(&left, &right);
                let (actual, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
                    super::super::join_scratch(&left, &right).unwrap();
                    join(&left, &right)
                });
                assert!(!refused && first.is_none());
                assert_eq!(actual, expected);
                assert!(actual.values().all(|s| *s != Availability::Available));
            }
        }
    }
}
#[test]
fn complete_nonempty_scope_receiver_has_honest_many_field_net_and_visible_cold_loss() {
    for count in [1, 256] {
        let input = BTreeMap::from([(
            ValueId::new("moved.owner".to_owned()),
            binding(count, Availability::MaybeMoved),
        )]);
        for branches in [false, true] {
            let mut results = Vec::new();
            for old in [true, false] {
                let (((), metrics), refused, paid, first) =
                    with_limit_usage_refusal(usize::MAX, || {
                        measure(|| {
                            if old {
                                with_before(|| receive(&input, branches))
                            } else {
                                receive(&input, branches)
                            }
                            .unwrap()
                        })
                    });
                assert!(!refused && first.is_none());
                complete(metrics, paid);
                results.push((metrics, paid));
            }
            eprintln!(
                "moved-map fields={count} branches={branches} old={:?} current={:?}",
                results[0], results[1]
            );
            assert!(results[1].0.allocated < results[0].0.allocated);
            if count == 256 {
                assert!(
                    results[1].1 < results[0].1,
                    "full new transport banks must earn their own invoice"
                );
            } else {
                assert!(
                    results[1].1 > results[0].1,
                    "cold cursor gateway is not free"
                );
            }
        }
    }
}
#[test]
fn current_complete_receiver_keeps_exact_short_zero_floor_sticky_and_full_drop() {
    let input = BTreeMap::from([(
        ValueId::new("moved.owner".to_owned()),
        binding(64, Availability::MaybeMoved),
    )]);
    for branches in [false, true] {
        let run = |limit, floor| {
            with_limit_usage_refusal(limit, || {
                assert!(bounded_output::set_active_floor(floor));
                measure(|| match receive(&input, branches) {
                    Ok(()) => true,
                    Err(error) => {
                        assert_eq!(error.code, "SPX-H006");
                        drop(error);
                        assert!(!bounded_output::reserve_active_required(0));
                        false
                    }
                })
            })
        };
        let ((ok, metrics), refused, exact, first) = run(usize::MAX, 0);
        assert!(ok && !refused && first.is_none());
        complete(metrics, exact);
        for (limit, floor, success) in [
            (exact, 0, true),
            (exact - 1, 0, false),
            (0, 0, false),
            (exact + 97, 97, true),
            (exact + 96, 97, false),
        ] {
            let ((ok, observed), refused, paid, first) = run(limit, floor);
            assert_eq!(ok, success);
            assert_eq!(refused, !success);
            assert_eq!(first.is_none(), success);
            complete(observed, paid);
            let again = run(limit, floor);
            assert_eq!(again.0, (ok, observed));
            assert_eq!(again.1, refused);
            assert_eq!(again.2, paid);
            assert_eq!(again.3, first);
        }
    }
}

#[test]
fn real_many_field_hir_caller_keeps_semantics_diagnostics_and_complete_drop() {
    let mut source = String::from("module moved.real;\n@id(\"Packet\") record Packet {");
    for n in 0..32 {
        source.push_str(&format!("@id(\"field{n}\") field{n}:Bytes,"));
    }
    source.push_str("}\n@id(\"take\") fn take(value:own Bytes)->usize {byte_len(bytes_as_slice(value))}\n@id(\"inspect\") fn inspect(packet:own Packet,flag:bool)->i64 {");
    for n in 0..32 {
        source.push_str(&format!(
            "let branch{n}=if flag {{let used=take(packet.field{n});0}}else{{0}};"
        ));
    }
    source.push_str("0}\n@id(\"main\") fn main()->i64 {0}\n");
    let ast = crate::check(&source, "moved-real.spx").unwrap();
    let program = with_before(|| crate::hir::resolve(&ast)).unwrap();
    let mut measured = Vec::new();
    for old in [true, false] {
        let ((result, metrics), refused, paid, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    if old {
                        with_before(|| crate::hir::validate(&program))
                    } else {
                        crate::hir::validate(&program)
                    }
                })
            });
        assert!(result.is_ok() && !refused && first.is_none());
        drop(result);
        complete(metrics, paid);
        measured.push((metrics, paid));
    }
    eprintln!(
        "whole moved-field HIR old={:?} current={:?}",
        measured[0], measured[1]
    );
    assert!(measured[1].0.allocated < measured[0].0.allocated);
    assert!(
        measured[1].1 < measured[0].1,
        "complete caller must independently earn new controls"
    );
    let mut hostile = program.clone();
    hostile.entrypoint = DeclarationId::new("missing.entry");
    assert_eq!(
        crate::hir::validate(&hostile),
        with_before(|| crate::hir::validate(&hostile))
    );
}

#[test]
fn full_original_merge_payer_and_generic_producers_remain_source_bound() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled) in [
        (
            "src/hir/validation/scope_checkpoint/merge.rs",
            include_str!("../../merge.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/moved_direct.rs",
            include_str!("../moved_direct.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs",
            include_str!("before.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs",
            include_str!("tests.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/moved_direct/before.rs.txt",
            include_str!("before.rs.txt"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/moved_direct/source_inverse.json",
            include_str!("source_inverse.json"),
        ),
        (
            "src/hir/validation/branch_merge.rs",
            include_str!("../../../branch_merge.rs"),
        ),
    ] {
        assert!(compiled.len() <= 2_097_152);
        let mut raw = Vec::new();
        std::fs::File::open(root.join(path))
            .unwrap()
            .take((compiled.len() + 1) as u64)
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(raw, compiled.as_bytes());
    }
    let hash = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(b)));
    let data: serde_json::Value =
        serde_json::from_str(include_str!("source_inverse.json")).unwrap();
    assert_eq!(
        hash(include_str!("../../../branch_merge.rs").as_bytes()),
        data["unchanged_generic"]["sha256"].as_str().unwrap()
    );
    let restore = |current: &[u8]| {
        assert_eq!(
            current.len() as u64,
            data["current"]["length"].as_u64().unwrap()
        );
        assert_eq!(hash(current), data["current"]["sha256"].as_str().unwrap());
        let mut raw = current.to_vec();
        for e in data["edits"].as_array().unwrap().iter().rev() {
            let a = e["start"].as_u64().unwrap() as usize;
            let b = e["end"].as_u64().unwrap() as usize;
            assert_eq!(&raw[a..b], e["current"].as_str().unwrap().as_bytes());
            raw.splice(a..b, e["before"].as_str().unwrap().bytes());
        }
        raw
    };
    let current = include_str!("../../merge.rs").as_bytes();
    let old = restore(current);
    assert_eq!(old, include_str!("before.rs.txt").as_bytes());
    let old = std::str::from_utf8(&old)
        .unwrap()
        .split("use super::*;\n")
        .nth(1)
        .unwrap()
        .split("\n#[cfg(test)]\nmod saved;")
        .next()
        .unwrap();
    let executable = include_str!("before.rs")
        .split("use super::super::*;\n")
        .nth(1)
        .unwrap();
    assert_eq!(executable, old);
    let mut drift = current.to_vec();
    drift.push(b' ');
    assert!(std::panic::catch_unwind(|| restore(&drift)).is_err());
}
