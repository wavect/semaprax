//! Direct singleton carriers against the complete immediate-8caff execution.
use super::*;
use crate::bounded_output::{active_remaining, set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    mem::{align_of, size_of},
};
thread_local! { static BANKS: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn record_bank() {
    BANKS.with(|n| n.set(n.get().checked_add(1).expect("finite paid banks")));
}
pub(super) fn reset_banks() {
    BANKS.with(|n| n.set(0));
}
pub(super) fn paid_banks() -> usize {
    BANKS.with(Cell::get)
}
const SOURCE: &str = r#"
module boxed;
@id("f") fn f(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let view=bytes_as_slice(scratch);
    if flag && byte_len(view)==3usize {7}else{9}
}
@id("main") fn main()->i64 {f(false)}
"#;

const TINY: &str = "module tiny; @id(\"main\") fn main()->i64 {0}";
const LEXICAL: &str = r#"module ordinary.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
const RENEWAL: &str = r#"module ordinary.byte_renewal;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let untouched=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 return if byte_len(bytes_as_slice(buffer))==2usize && byte_len(bytes_as_slice(untouched))==1usize {7}else{0};
}
"#;
fn ordinary_replay(
    p: &ResolvedProgram,
    f: &ResolvedFunction,
    old: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(f, budget)?;
    let leaves = collect_leaves_paid(f, budget)?;
    let auth = if lexical_cert::requires_lexical(f) {
        let owner = selected_owner::owner(p, f, budget)?;
        let facts = lexical_hir::collect(f, budget)?;
        Some(lexical_cert::authenticate(p, f, &owner, &facts, budget)?)
    } else {
        None
    };
    if old {
        direct_one_before::validate_path_states(
            p,
            f,
            &storage,
            &leaves,
            budget,
            auth.as_ref(),
            None,
        )
    } else {
        validate_path_states(p, f, &storage, &leaves, budget, auth.as_ref(), None)
    }
}
#[test]
fn only_deleted_singleton_iterator_carriers_leave_each_actual_paid_bank() {
    let removed = direct_one_before::removed_iterator_controls();
    assert!(removed > 0);
    assert_eq!(
        boxed_flow::BLOCK_CONTROLS + removed,
        direct_one_before::BLOCK_CONTROLS
    );
    assert_eq!(
        direct_one_before::receiver_layouts(),
        [
            size_of::<frontier::Incoming<PathState>>(),
            align_of::<frontier::Incoming<PathState>>(),
            size_of::<compact_failure_before::Insertions<PathState>>(),
            align_of::<compact_failure_before::Insertions<PathState>>()
        ]
    );
    // Constructor, directory backing, held authority, entry/group banks and
    // final Drop keep their full former prices/layouts. No setup credit exists.
    let old = include_str!("direct_one_before/boxed_flow.rs.txt");
    let now = include_str!("boxed_flow.rs");
    assert_eq!(
        old.split("// Direct lexical producers").next(),
        now.split("// Direct lexical producers").next()
    );
    assert_eq!(
        old.split("// One terminal local Diagnostic").nth(1),
        now.split("// One terminal local Diagnostic").nth(1)
    );
}
// Incremental against immediate8caff only. The unchanged frontier_tests::whole
// owner retains the original pre-frontier callers and their full net invoices;
// this five-control namespace alone cannot establish the original net criterion.
#[test]
fn complete_cold_driver_invoices_follow_actual_bank_count_with_same_physical_drop() {
    for source in [TINY, SOURCE, LEXICAL, RENEWAL] {
        let p =
            crate::hir::resolve(&crate::check(source, "ordinary-frontier.spx").unwrap()).unwrap();
        for f in &p.functions {
            let original = f.cleanup_plan.clone();
            let observe = |old, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut b = ReplayBudget::new();
                        let result = ordinary_replay(&p, f, old, &mut b);
                        let success = result.is_ok();
                        if let Err(e) = &result {
                            assert_eq!(e.code, "SPX-H006");
                        }
                        drop(result);
                        drop(b);
                        success
                    })
                })
            };
            let before = observe(true, usize::MAX);
            reset_banks();
            contains_tests::reset_banks();
            let current = observe(false, usize::MAX);
            let banks = paid_banks();
            assert!(before.0 .0 && current.0 .0 && !before.1 && !current.1 && banks > 0);
            let removed = banks
                .checked_mul(direct_one_before::removed_iterator_controls())
                .unwrap();
            let setup_removed = compact_failure_tests::setup_delta::<PathState>();
            let entry_removed = contains_tests::paid_banks()
                .checked_mul(contains_tests::entry_delta::<PathState>()).unwrap();
            assert_eq!(
                current.2.checked_add(removed).and_then(|n| n.checked_add(setup_removed)).and_then(|n| n.checked_add(entry_removed)),
                Some(before.2)
            );
            assert!(current.2 < before.2);
            eprintln!("direct-one {} immediate8caff={} current={} banks={} iterator_removed={} setup_removed={} entry_removed={} physical_before={:?} physical_current={:?}", f.id,before.2,current.2,banks,removed,setup_removed,entry_removed,before.0.1,current.0.1);
            for o in [&before, &current] {
                let a = &o.0 .1;
                assert!(!a.invalid && a.live == 0 && a.allocated <= o.2);
                assert_eq!(a.allocated, a.freed);
            }
            assert_eq!(current.0 .1.allocated, before.0 .1.allocated);
            assert_eq!(current.0 .1.freed, before.0 .1.freed);
            assert_eq!(current.0 .1.allocations, before.0 .1.allocations);
            for old in [true, false] {
                let exact = if old { before.2 } else { current.2 };
                for limit in [0, exact - 1, exact] {
                    let o = observe(old, limit);
                    assert_eq!(o.0 .0, limit == exact);
                    assert_eq!(o.1, limit != exact);
                    assert_eq!(o.3.is_some(), limit != exact);
                    let a = &o.0 .1;
                    assert!(!a.invalid && a.live == 0 && a.allocated <= o.2 && o.2 <= limit);
                    assert_eq!(a.allocated, a.freed);
                }
            }
            assert_eq!(f.cleanup_plan, original);
        }
    }
}
#[test]
fn both_original_work_remainders_and_complete_diagnostics_keep_every_cut() {
    for source in [TINY, SOURCE, LEXICAL, RENEWAL] {
        let p =
            crate::hir::resolve(&crate::check(source, "ordinary-frontier.spx").unwrap()).unwrap();
        for f in &p.functions {
            let mut complete = ReplayBudget::unobserved();
            ordinary_replay(&p, f, true, &mut complete).unwrap();
            let work = MAX_REPLAY_WORK_UNITS - complete.remaining;
            for skeleton in [false, true] {
                for limit in [
                    0,
                    1,
                    2,
                    7,
                    work.saturating_sub(1),
                    work,
                    MAX_REPLAY_WORK_UNITS,
                ] {
                    let mut old = ReplayBudget::unobserved();
                    let mut now = ReplayBudget::unobserved();
                    if skeleton {
                        old.skeleton_remaining = limit;
                        now.skeleton_remaining = limit;
                    } else {
                        old.remaining = limit;
                        now.remaining = limit;
                    }
                    assert_eq!(
                        ordinary_replay(&p, f, false, &mut now),
                        ordinary_replay(&p, f, true, &mut old)
                    );
                    assert_eq!(
                        (now.remaining, now.skeleton_remaining),
                        (old.remaining, old.skeleton_remaining)
                    );
                }
            }
        }
    }
    let mut p =
        crate::hir::resolve(&crate::check(SOURCE, "ordinary-frontier.spx").unwrap()).unwrap();
    for arity in [0, 1, 3] {
        let mut changed = false;
        for f in &mut p.functions {
            if let Some(edges) = f.cleanup_plan.blocks.iter_mut().find_map(|block| {
                if let CleanupTerminator::Branch(edges) = &mut block.terminator {
                    Some(edges)
                } else {
                    None
                }
            }) {
                let original = edges.clone();
                *edges = (0..arity).map(|i| original[i % original.len()]).collect();
                changed = true;
                break;
            }
        }
        assert!(changed);
        for f in &p.functions {
            for limit in [0, 1, 2, 7, MAX_REPLAY_WORK_UNITS] {
                let mut old = ReplayBudget::unobserved();
                let mut now = ReplayBudget::unobserved();
                old.remaining = limit;
                now.remaining = limit;
                assert_eq!(
                    ordinary_replay(&p, f, false, &mut now),
                    ordinary_replay(&p, f, true, &mut old)
                );
                assert_eq!(
                    (now.remaining, now.skeleton_remaining),
                    (old.remaining, old.skeleton_remaining)
                );
            }
        }
        p = crate::hir::resolve(&crate::check(SOURCE, "ordinary-frontier.spx").unwrap()).unwrap();
    }
}
#[test]
fn failed_floor_keeps_selected_receipt_and_no_retry_can_publish() {
    let p = crate::hir::resolve(&crate::check(TINY, "ordinary-frontier.spx").unwrap()).unwrap();
    let f = &p.functions[0];
    let mut expected = None;
    for old in [true, false] {
        let ((first, second), refused, paid, receipt) = with_limit_usage_refusal(1 << 20, || {
            let mut b = ReplayBudget::new();
            set_active_floor(active_remaining().unwrap());
            let first = ordinary_replay(&p, f, old, &mut b);
            set_active_floor(0);
            let second = ordinary_replay(&p, f, old, &mut b);
            drop(b);
            (first, second)
        });
        assert!(refused && receipt.is_some() && paid <= 1 << 20);
        let first = first.unwrap_err();
        let second = second.unwrap_err();
        assert_eq!(first.code, "SPX-H006");
        assert_eq!(second.code, "SPX-H006");
        let complete = (first, second);
        if let Some(expected) = &expected {
            assert_eq!(&complete, expected);
        } else {
            expected = Some(complete);
        }
    }
}
fn digest(raw: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)))
}
#[test]
fn complete_immediate_driver_receivers_and_original_controls_keep_whole_bytes() {
    let manifest = include_bytes!("direct_one_before/source_manifest.json");
    assert_eq!(manifest.len(), 1336);
    assert_eq!(
        digest(manifest),
        "633004beb107e44ec172a8feb04a22123e0440b862e37082d289f8b7b0b04cb9"
    );
    let rows: serde_json::Value = serde_json::from_slice(manifest).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["original"].as_str().unwrap();
        let raw =
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_eq!(raw.len(), row["length"].as_u64().unwrap() as usize);
        assert_eq!(digest(&raw), row["sha256"].as_str().unwrap());
    }
    let inverse = include_bytes!("direct_one_before/inverse.json");
    assert_eq!(inverse.len(), 19485);
    assert_eq!(
        digest(inverse),
        "027cc73210897771d444e422df2c31c2356d9db4f5e6bc2590d1214263fede51"
    );
    let rows: serde_json::Value = serde_json::from_slice(inverse).unwrap();
    for row in rows.as_array().unwrap() {
        let read = |path: &str| {
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
        };
        let path = row["current"].as_str().unwrap();
        let mut actual =
            compact_failure_tests::before_source(path, String::from_utf8(read(path)).unwrap());
        assert_eq!(
            digest(actual.as_bytes()),
            row["current_sha256"].as_str().unwrap()
        );
        for edit in row["edits"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            let to = edit["to"].as_str().unwrap();
            assert_eq!(actual.matches(from).count(), 1);
            actual = actual.replacen(from, to, 1);
        }
        assert_eq!(actual.as_bytes(), read(row["before"].as_str().unwrap()));
    }
}
