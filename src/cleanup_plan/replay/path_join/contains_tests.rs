//! Lexical duplicate probes retain every bank except the removed formal loan.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    mem::{align_of, size_of},
};
thread_local! {
    static BANKS: Cell<usize> = const { Cell::new(0) };
    static DROPS: Cell<usize> = const { Cell::new(0) };
    static COMPARES: Cell<[(u64, u64); 128]> = const { Cell::new([(0, 0); 128]) };
    static COMPARE_COUNT: Cell<usize> = const { Cell::new(0) };
}
pub(super) fn record_entry_bank() {
    BANKS.with(|n| {
        n.set(
            n.get()
                .checked_add(1)
                .expect("finite actual paid entry banks"),
        )
    });
}
pub(super) fn reset_banks() {
    BANKS.with(|n| n.set(0));
    fused_tests::reset_promotions();
}
pub(super) fn paid_banks() -> usize {
    BANKS.with(Cell::get)
}
pub(super) fn entry_delta<T: Ord>() -> usize {
    let removed = size_of::<&frontier::Incoming<T>>() + fused_tests::common_saving::<T>();
    assert_eq!(
        contains_before::Insertions::<T>::controls(),
        frontier::Insertions::<T>::controls()
    );
    assert_eq!(
        contains_before::Insertions::<T>::entry_controls(),
        frontier::Insertions::<T>::entry_controls() + removed
    );
    assert_eq!(
        contains_before::Insertions::<T>::GROUP_ENTRY_CONTROLS,
        frontier::Insertions::<T>::GROUP_ENTRY_CONTROLS
    );
    let old = contains_before::Insertions::<T>::observed_prices();
    let new = frontier::Insertions::<T>::observed_prices();
    for i in 0..6 {
        assert_eq!(
            old[i].checked_add(if i == 2 || i == 3 {
                fused_tests::promotion::<T>()
            } else {
                0
            }),
            new[i].checked_add(removed)
        );
    }
    assert_eq!(old[6], new[6]);
    assert!(removed > 0);
    removed
}
#[test]
fn only_the_absent_callee_formal_leaves_each_actual_entry_and_complete_layouts_stay_equal() {
    assert_eq!(
        size_of::<contains_before::Incoming<PathState>>(),
        size_of::<frontier::Incoming<PathState>>()
    );
    assert_eq!(
        align_of::<contains_before::Incoming<PathState>>(),
        align_of::<frontier::Incoming<PathState>>()
    );
    assert_eq!(
        size_of::<contains_before::Insertions<PathState>>(),
        size_of::<frontier::Insertions<PathState>>()
    );
    assert_eq!(
        align_of::<contains_before::Insertions<PathState>>(),
        align_of::<frontier::Insertions<PathState>>()
    );
    assert_eq!(
        size_of::<Result<contains_before::Insertions<PathState>, Diagnostic>>(),
        size_of::<Result<frontier::Insertions<PathState>, Diagnostic>>()
    );
    entry_delta::<PathState>();
}
#[derive(Eq, PartialEq)]
struct State([u64; 50]);
impl Ord for State {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        COMPARE_COUNT.with(|n| {
            let at = n.get();
            assert!(at < 128);
            COMPARES.with(|trace| {
                let mut data = trace.get();
                data[at] = (self.0[0], other.0[0]);
                trace.set(data);
            });
            n.set(at + 1);
        });
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
    }
}
#[test]
fn whole_paid_ordinary_and_unchanged_group_producers_keep_every_compare_box_and_drop_at_all_cuts() {
    let f = crate::hir::resolve(
        &crate::check(
            "module contains; @id(\"main\") fn main()->i64 {0}",
            "contains.spx",
        )
        .unwrap(),
    )
    .unwrap()
    .functions
    .remove(0);
    let observe = |old, limit| {
        reset_banks();
        DROPS.with(|n| n.set(0));
        COMPARES.with(|n| n.set([(0, 0); 128]));
        COMPARE_COUNT.with(|n| n.set(0));
        let ((success, allocation), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                macro_rules! run {
                    ($module:ident) => {{
                        let mut r = match $module::Insertions::acquire(&f, &mut b) {
                            Ok(r) => r,
                            Err(e) => {
                                assert_eq!(e.code, "SPX-H006");
                                drop(e);
                                return false;
                            }
                        };
                        let mut target = $module::Incoming::default();
                        let mut small = $module::Incoming::default();
                        let mut group = BTreeSet::new();
                        let operation = (|| -> Result<(), Diagnostic> {
                            for value in [1, 1, 2, 3] {
                                r.insert_owned(&f, &mut target, State([value; 50]), &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                            }
                            assert_eq!(target.len(), 3);
                            for value in [4, 4] {
                                let mut one = $module::Incoming::default();
                                r.insert_owned(&f, &mut one, State([value; 50]), &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                                let held = one.into_one().next().unwrap();
                                let at = std::ptr::from_ref(held.as_ref());
                                let prior = target
                                    .iter()
                                    .find(|v| v.0[0] == value)
                                    .map(|v| std::ptr::from_ref(v.as_ref()));
                                r.insert_held(&f, &mut target, held, &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                                let actual = target
                                    .iter()
                                    .find(|v| v.0[0] == value)
                                    .map(|v| std::ptr::from_ref(v.as_ref()))
                                    .unwrap();
                                assert_eq!(actual, prior.unwrap_or(at));
                            }
                            for value in [7, 8] {
                                let mut one = $module::Incoming::default();
                                r.insert_owned(&f, &mut one, State([value; 50]), &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                                let held = one.into_one().next().unwrap();
                                let at = std::ptr::from_ref(held.as_ref());
                                r.insert_held(&f, &mut small, held, &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                                assert!(small.iter().any(|v| std::ptr::from_ref(v.as_ref()) == at));
                            }
                            for value in [9, 9] {
                                let mut one = $module::Incoming::default();
                                r.insert_owned(&f, &mut one, State([value; 50]), &mut b);
                                if r.failed {
                                    return Err(r.failure());
                                }
                                r.insert_group_held(
                                    &f,
                                    &mut group,
                                    one.into_one().next().unwrap(),
                                    &mut b,
                                );
                                if r.failed {
                                    return Err(r.failure());
                                }
                            }
                            assert_eq!((target.len(), small.len(), group.len()), (4, 2, 1));
                            Ok(())
                        })();
                        let success = operation.is_ok();
                        if let Err(e) = &operation {
                            assert_eq!(e.code, "SPX-H006");
                            assert_eq!(e.severity, crate::diagnostic::Severity::Error);
                            assert!(e.path.is_none() && e.span.is_none() && e.help.is_none());
                        }
                        drop(operation);
                        drop(group);
                        drop(small);
                        drop(target);
                        drop(r);
                        success
                    }};
                }
                if old {
                    run!(contains_before)
                } else {
                    run!(frontier)
                }
            })
        });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= paid && paid <= limit);
        assert_eq!(refused, !success);
        assert_eq!(first.is_some(), !success);
        (
            success,
            allocation,
            paid,
            paid_banks(),
            DROPS.with(Cell::get),
            COMPARES.with(Cell::get),
            COMPARE_COUNT.with(Cell::get),
        )
    };
    let old = observe(true, usize::MAX);
    let new = observe(false, usize::MAX);
    assert!(old.0 && new.0);
    assert_eq!(new.3, 15); // one setup bank plus fourteen actual ordinary renewals
    assert_eq!(
        new.2
            .checked_add(new.3 * entry_delta::<State>())
            .and_then(|n| n.checked_sub(2 * fused_tests::promotion::<State>())),
        Some(old.2)
    );
    assert_eq!(new.1, old.1);
    assert_eq!(new.4, 10);
    assert_eq!((new.4, new.5, new.6), (old.4, old.5, old.6));
    for prior in [false, true] {
        let exact = if prior { old.2 } else { new.2 };
        for limit in [0, 1, exact - 1, exact] {
            let result = observe(prior, limit);
            assert_eq!(result.0, limit == exact);
        }
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)))
}
pub(super) fn before_source(path: &str, actual: String) -> String {
    let actual = fused_tests::before_source(path, actual);
    let mut actual = String::from_utf8(crate::bounded_output::census_source_before(
        path,
        actual.as_bytes(),
    ))
    .unwrap();
    let inverse = include_bytes!("contains_before/inverse.json");
    assert_eq!(inverse.len(), INVERSE_LENGTH);
    assert_eq!(digest(inverse), INVERSE_SHA);
    let rows: serde_json::Value = serde_json::from_slice(inverse).unwrap();
    let mut matched = false;
    for row in rows.as_array().unwrap() {
        if row["current"].as_str().unwrap() != path {
            continue;
        }
        assert!(!matched);
        matched = true;
        assert_eq!(
            actual.len(),
            row["current_length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            digest(actual.as_bytes()),
            row["current_sha256"].as_str().unwrap()
        );
        for edit in row["edits"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            assert_eq!(actual.matches(from).count(), 1);
            actual = actual.replacen(from, edit["to"].as_str().unwrap(), 1);
        }
        let before = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["before"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(digest(&before), row["before_sha256"].as_str().unwrap());
        assert_eq!(actual.as_bytes(), before);
    }
    actual
}
#[test]
fn complete_immediate_owner_and_every_preceding_inverse_keep_all_original_bytes() {
    let manifest = include_bytes!("contains_before/source_manifest.json");
    assert_eq!(manifest.len(), MANIFEST_LENGTH);
    assert_eq!(digest(manifest), MANIFEST_SHA);
    let rows: serde_json::Value = serde_json::from_slice(manifest).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 5);
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let read = |path: &str| {
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
        };
        let raw = read(path);
        let restored = before_source(path, String::from_utf8(raw).unwrap());
        let before = read(row["before"].as_str().unwrap());
        assert_eq!(before.len(), row["length"].as_u64().unwrap() as usize);
        assert_eq!(digest(&before), row["sha256"].as_str().unwrap());
        assert_eq!(restored.as_bytes(), before);
    }
    assert_eq!(
        include_bytes!("contains_before.rs"),
        include_bytes!("contains_before/frontier.rs.txt")
    );
    // Old and new owners bind the actual same full compact-zero/authentication
    // producer: the old body keeps its independent contains helper, not a call
    // redirected through the current lexical implementation.
    let auth = include_bytes!("../physical_budget/authentication.rs");
    assert_eq!(auth.len(), 4022);
    assert_eq!(
        digest(auth),
        "94aa393a6f0cfc4937f0c59a013cd705aa76eea260b35d4a7c572c2d8ffcc112"
    );
}
const INVERSE_LENGTH: usize = 19839;
const INVERSE_SHA: &str = "6f1e6c3b8f3e8fb4e8ba13a215243b3b2050efdd987dd619a359131069b3381a";
const MANIFEST_LENGTH: usize = 1342;
const MANIFEST_SHA: &str = "f06c0bf9959af63ea1e332e81ad6b18170af84019797590ee60ca8f27237db60";
