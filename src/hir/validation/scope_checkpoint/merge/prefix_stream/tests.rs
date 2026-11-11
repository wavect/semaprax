use super::*;
use std::cell::Cell;
use std::io::Read;

thread_local! { static BEFORE: Cell<bool> = const { Cell::new(false) }; }

pub(in crate::hir::validation::scope_checkpoint::merge) struct BeforeGuard(bool);

impl Drop for BeforeGuard {
    fn drop(&mut self) {
        BEFORE.with(|slot| slot.set(self.0));
    }
}

pub(in crate::hir::validation::scope_checkpoint::merge) fn before_guard() -> BeforeGuard {
    BeforeGuard(BEFORE.with(|slot| slot.replace(true)))
}

pub(in crate::hir::validation::scope_checkpoint::merge) fn before_selected() -> bool {
    BEFORE.with(Cell::get)
}

const PREVIOUS_DIGEST: &str = "72e5ee8da215752bb51be5f448f18707f2c9b89fd80f5d3f1f95cc4ebb9d5b75";
const PREVIOUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/e/72e5ee8da215752bb51be5f448f18707f2c9b89fd80f5d3f1f95cc4ebb9d5b75"
));

pub(in crate::hir::validation::scope_checkpoint::merge) fn historical_merge(
    current: &[u8],
) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled) in [
        (
            "src/hir/validation/scope_checkpoint/merge.rs",
            include_str!("../../merge.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/prefix_stream/scope_tests.rs",
            include_str!("scope_tests.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/prefix_stream/before.rs",
            include_str!("before.rs"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/prefix_stream/inverse.json",
            include_str!("inverse.json"),
        ),
        (
            "src/hir/validation/scope_checkpoint/merge/prefix_stream/tests.rs",
            include_str!("tests.rs"),
        ),
        (
            "e/72e5ee8da215752bb51be5f448f18707f2c9b89fd80f5d3f1f95cc4ebb9d5b75",
            PREVIOUS,
        ),
    ] {
        let mut physical = Vec::new();
        std::fs::File::open(root.join(path))
            .unwrap()
            .take((compiled.len() + 1) as u64)
            .read_to_end(&mut physical)
            .unwrap();
        assert_eq!(physical, compiled.as_bytes(), "{path}");
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(PREVIOUS.as_bytes()))
        ),
        PREVIOUS_DIGEST
    );
    assert_eq!(current, include_str!("../../merge.rs").as_bytes());
    let inverse: Vec<(usize, String, String)> =
        serde_json::from_str(include_str!("inverse.json")).unwrap();
    let mut restored = current.to_vec();
    let mut last = restored.len();
    for (start, after, before) in inverse.into_iter().rev() {
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= last);
        assert_eq!(&restored[start..end], after.as_bytes());
        restored.splice(start..end, before.bytes());
        last = start;
    }
    assert_eq!(restored, PREVIOUS.as_bytes());
    let mut executable = include_str!("before.rs")
        .replacen(
            "use super::super::*;\nuse super::super::{moved_direct, partial_retain};",
            "use super::*;\nmod moved_direct;\nmod partial_retain;",
            1,
        )
        .replacen(
            "pub(in crate::hir::validation::scope_checkpoint::merge) fn join_scratch(\n    left: &ValidationBinding,\n    right: &ValidationBinding,\n) -> Result<(), Diagnostic> {",
            "fn join_scratch(left: &ValidationBinding, right: &ValidationBinding) -> Result<(), Diagnostic> {",
            1,
        );
    executable.push_str("\n#[cfg(test)]\nmod saved;\n#[cfg(test)]\nmod tests;\n");
    assert_eq!(executable, PREVIOUS);
    restored
}

#[test]
fn whole_previous_receiver_and_payer_restore_from_live_and_executable_sources() {
    let current = include_str!("../../merge.rs").as_bytes();
    assert_eq!(historical_merge(current), PREVIOUS.as_bytes());
    let mut drift = current.to_vec();
    drift[0] ^= 1;
    assert!(std::panic::catch_unwind(|| historical_merge(&drift)).is_err());
}

fn field(id: &str) -> PlaceProjection {
    PlaceProjection::Field(DeclarationId::new(id))
}

fn variant(case: &str, field: &str) -> PlaceProjection {
    PlaceProjection::VariantField {
        case: DeclarationId::new(case),
        field: DeclarationId::new(field),
    }
}

#[test]
fn live_four_inventory_stream_matches_sorted_prefixes_and_both_original_predicates() {
    use crate::bounded_output::with_limit_usage_refusal;
    let samples = [
        Vec::new(),
        vec![field("a")],
        vec![field("a"), variant("β", "字段")],
        vec![variant("β", "字段"), field("a"), variant("α", "𐐀")],
    ];
    for count in 0..16 {
        for mask in 0..16 {
            for state in [
                Availability::Available,
                Availability::MaybeMoved,
                Availability::Moved,
            ] {
                let mut left = super::super::partial_retain::tests::binding(0, 0, state);
                let mut right = left.clone();
                for (n, path) in samples.iter().enumerate() {
                    if count & (1 << n) != 0 {
                        left.moved_places.insert(path.clone(), state);
                        right.definitely_partial.insert(path.clone());
                    }
                    if mask & (1 << n) != 0 {
                        right.moved_places.insert(path.clone(), Availability::Moved);
                        left.definitely_partial.insert(path.clone());
                    }
                }
                let expected = HirValidator::join_definitely_partial(&left, &right);
                let immediate = HirValidator::join_definitely_partial_retained(&left, &right);
                let (actual, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
                    charge::reserve(Some(CONTROL)).unwrap();
                    join(&left, &right).unwrap()
                });
                assert!(!refused && first.is_none());
                assert_eq!(actual, expected);
                assert_eq!(actual, immediate);
            }
        }
    }
}
