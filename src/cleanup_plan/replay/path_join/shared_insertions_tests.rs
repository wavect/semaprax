//! Shared actual receivers, immediate-92d complete execution and fixed byte custody.
use super::frontier::{Incoming, Insertions};
use super::*;
use crate::bounded_output::{active_remaining, set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use sha2::{Digest, Sha256};
use std::cell::Cell;
thread_local! { static DROPS: Cell<usize> = const { Cell::new(0) }; }
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct State([u64; 50]);
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
    }
}
fn function() -> ResolvedFunction {
    crate::hir::resolve(&crate::check(TINY, "shared-frontier.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn prices<T: Ord>(f: &ResolvedFunction) -> ([usize; 7], [usize; 4]) {
    let mut budget = ReplayBudget::unobserved();
    shared_insertions_before::observed_setup::<T>(f, &mut budget).unwrap()
}
#[test]
fn const_prices_equal_every_original_tariff_and_checked_layout_including_sentinels() {
    let f = function();
    macro_rules! same {
        ($t:ty) => {{
            let original = prices::<$t>(&f).0;
            let current = Insertions::<$t>::observed_prices();
            for i in 0..6 {
                assert_eq!(
                    current[i].checked_add(contains_tests::entry_delta::<$t>()),
                    original[i].checked_add(if i == 2 || i == 3 {
                        fused_tests::promotion::<$t>()
                    } else {
                        0
                    })
                );
            }
            assert_eq!(current[6], original[6]);
            assert_eq!(
                frontier::const_node_upper::<Box<$t>, ()>(),
                crate::cleanup_plan::replay::physical_storage::btree_node_upper::<Box<$t>, ()>()
            );
        }};
    }
    same!(());
    same!(u8);
    same!([u64; 50]);
    #[repr(align(64))]
    #[derive(Eq, PartialEq, Ord, PartialOrd)]
    struct Aligned([u8; 65]);
    same!(Aligned);
    // Stay below rustc/LLVM's object-size bound while 11 keys overflow usize.
    type Huge = [u8; isize::MAX as usize / 4];
    same!(Huge); // Actual insertion keys are Box<Huge>, hence pointer-sized.
                 // Exercise the generic node algorithm's real checked overflow separately.
    assert_eq!(frontier::const_node_upper::<Huge, ()>(), None);
    assert_eq!(
        crate::cleanup_plan::replay::physical_storage::btree_node_upper::<Huge, ()>(),
        None
    );
    assert_eq!(
        frontier::const_node_upper::<Aligned, [u64; 3]>(),
        crate::cleanup_plan::replay::physical_storage::btree_node_upper::<Aligned, [u64; 3]>()
    );
    let before = prices::<State>(&f).1;
    assert_eq!(
        Insertions::<State>::entry_controls() + contains_tests::entry_delta::<State>(),
        before[1]
    );
    assert_eq!(Insertions::<State>::GROUP_ENTRY_CONTROLS, before[3]);
    let removed = before[0] + before[2] + before[3] - Insertions::<State>::controls();
    assert!(removed > 0);
    let invoice = |old| {
        let ((), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            let mut b = ReplayBudget::new();
            if old {
                let _ = shared_insertions_before::observed_setup::<State>(&f, &mut b).unwrap();
            } else {
                drop(Insertions::<State>::acquire(&f, &mut b).unwrap());
            }
            drop(b);
        });
        assert!(!refused && first.is_none());
        paid
    };
    assert_eq!(
        invoice(false) + removed + contains_tests::entry_delta::<State>(),
        invoice(true)
    );
}
#[test]
fn shared_group_entry_keeps_original_box_duplicate_drop_and_each_paid_frontier() {
    let f = function();
    let tariff = Insertions::<State>::observed_prices();
    let acquired = f.id.as_str().len()
        + 512
        + Insertions::<State>::controls()
        + Insertions::<State>::entry_controls();
    let complete = acquired + tariff[0] + tariff[6];
    for limit in [
        0,
        acquired - 1,
        acquired,
        acquired + tariff[0] - 1,
        acquired + tariff[0],
        complete - 1,
        complete,
    ] {
        DROPS.with(|n| n.set(0));
        let ((success, allocation), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                let mut incoming = Incoming::default();
                let mut group = BTreeSet::new();
                let mut receiver = match Insertions::acquire(&f, &mut b) {
                    Ok(v) => v,
                    Err(e) => {
                        drop(e);
                        return false;
                    }
                };
                receiver.insert_owned(&f, &mut incoming, State([1; 50]), &mut b);
                if !receiver.failed {
                    let value = incoming.into_one().next().unwrap();
                    let pointer = std::ptr::from_ref(value.as_ref());
                    receiver.insert_group_held(&f, &mut group, value, &mut b);
                    if !receiver.failed {
                        assert_eq!(std::ptr::from_ref(group.first().unwrap().as_ref()), pointer);
                    }
                } else {
                    drop(incoming);
                }
                let success = !receiver.failed;
                if !success {
                    drop(receiver.failure());
                }
                drop(group);
                drop(receiver);
                drop(b);
                success
            })
        });
        assert_eq!(success, limit == complete);
        assert_eq!(refused, !success);
        assert_eq!(first.is_some(), !success);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid && paid <= limit);
        assert_eq!(DROPS.with(Cell::get), usize::from(limit >= acquired));
    }
    DROPS.with(|n| n.set(0));
    let (((), a), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut b = ReplayBudget::new();
            let mut r = Insertions::acquire(&f, &mut b).unwrap();
            let mut group = BTreeSet::new();
            let mut incoming = Incoming::default();
            r.insert_owned(&f, &mut incoming, State([7; 50]), &mut b);
            let value = incoming.into_one().next().unwrap();
            let pointer = std::ptr::from_ref(value.as_ref());
            r.insert_group_held(&f, &mut group, value, &mut b);
            let mut incoming = Incoming::default();
            r.insert_owned(&f, &mut incoming, State([7; 50]), &mut b);
            let allocations = current_metrics().allocations;
            r.insert_group_held(&f, &mut group, incoming.into_one().next().unwrap(), &mut b);
            assert!(!r.failed);
            assert_eq!(current_metrics().allocations, allocations);
            assert_eq!(std::ptr::from_ref(group.first().unwrap().as_ref()), pointer);
            assert_eq!(DROPS.with(Cell::get), 1);
            drop(group);
            drop(r);
            drop(b);
            assert_eq!(DROPS.with(Cell::get), 2);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!a.invalid && a.live == 0 && a.allocated <= paid);
    assert_eq!(a.allocated, a.freed);
}
#[test]
fn shared_group_authenticates_before_payment_and_stays_failed_after_floor_or_foreign_scope() {
    let f = function();
    let foreign = f.clone();
    for case in 0..4 {
        let (((), a), _, _, _) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                let mut other = ReplayBudget::new();
                let mut r = Insertions::acquire(&f, &mut b).unwrap();
                let mut incoming = Incoming::default();
                let mut group = BTreeSet::new();
                r.insert_owned(&f, &mut incoming, State([1; 50]), &mut b);
                let value = incoming.into_one().next().unwrap();
                match case {
                    0 => r.insert_group_held(&foreign, &mut group, value, &mut b),
                    1 => r.insert_group_held(&f, &mut group, value, &mut other),
                    2 => {
                        with_limit_usage_refusal(1 << 16, || {
                            r.insert_group_held(&f, &mut group, value, &mut b)
                        });
                    }
                    _ => {
                        assert!(set_active_floor(active_remaining().unwrap()));
                        r.insert_group_held(&f, &mut group, value, &mut b);
                    }
                }
                assert!(r.failed && group.is_empty());
                let allocations = current_metrics().allocations;
                let first = r.failure();
                if case < 3 {
                    assert_eq!(first, Diagnostic::io("SPX-H006", String::new()));
                }
                // Both entry kinds now share the same sticky terminal owner.
                let mut incoming = Incoming::default();
                r.insert_owned(&f, &mut incoming, State([2; 50]), &mut b);
                assert!(r.failed && incoming.is_empty());
                assert_eq!(current_metrics().allocations, allocations);
                drop(first);
                drop(incoming);
                drop(group);
                drop(r);
                drop(other);
                drop(b);
            })
        });
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
    }
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
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let auth = if lexical_cert::requires_lexical(function) {
        let owner = selected_owner::owner(program, function, budget)?;
        let facts = lexical_hir::collect(function, budget)?;
        Some(lexical_cert::authenticate(
            program, function, &owner, &facts, budget,
        )?)
    } else {
        None
    };
    if old {
        shared_insertions_before::validate_path_states(
            program,
            function,
            &storage,
            &leaves,
            budget,
            auth.as_ref(),
            None,
        )
    } else {
        validate_path_states(
            program,
            function,
            &storage,
            &leaves,
            budget,
            auth.as_ref(),
            None,
        )
    }
}
#[test]
fn complete_immediate_receiver_composition_lowers_cold_invoice_with_full_drop_and_same_work_errors()
{
    let mut every_invoice_lower = true;
    for source in [TINY, SOURCE, LEXICAL, RENEWAL] {
        let program =
            crate::hir::resolve(&crate::check(source, "ordinary-frontier.spx").unwrap()).unwrap();
        for f in &program.functions {
            let original = f.cleanup_plan.clone();
            let observe = |old, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut b = ReplayBudget::new();
                        let result = ordinary_replay(&program, f, old, &mut b);
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
            direct_one_tests::reset_banks();
            contains_tests::reset_banks();
            let current = observe(false, usize::MAX);
            let direct_banks = direct_one_tests::paid_banks();
            let entry_removed = fused_tests::prior_entry_saving::<PathState>();
            assert!(before.0 .0 && current.0 .0 && !before.1 && !current.1);
            let old_controls = prices::<PathState>(f).1;
            let removed = old_controls[0] + old_controls[2] + old_controls[3]
                - Insertions::<PathState>::controls();
            eprintln!(
                "shared-frontier {} immediate92d_invoice={} current_invoice={} removed_setup={} before={:?} current={:?}",
                f.id, before.2, current.2, removed, before.0.1, current.0.1
            );
            assert_eq!(
                current.2
                    + removed
                    + direct_banks * direct_one_before::removed_iterator_controls()
                    + entry_removed,
                before.2
            );
            every_invoice_lower &= current.2 < before.2;
            assert!(current.0 .1.allocated <= before.0 .1.allocated);
            for o in [&before, &current] {
                let a = &o.0 .1;
                assert!(!a.invalid && a.live == 0 && a.allocated <= o.2);
                assert_eq!(a.allocated, a.freed);
            }
            for limit in [0, current.2 - 1, current.2] {
                let o = observe(false, limit);
                assert_eq!(o.0 .0, limit == current.2);
                assert_eq!(o.1, limit != current.2);
                assert_eq!(o.3.is_some(), limit != current.2);
                let a = &o.0 .1;
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
                assert!(a.allocated <= o.2 && o.2 <= limit);
            }
            let mut b = ReplayBudget::unobserved();
            ordinary_replay(&program, f, true, &mut b).unwrap();
            let work = MAX_REPLAY_WORK_UNITS - b.remaining;
            for limit in [
                0,
                1,
                2,
                7,
                work.saturating_sub(1),
                work,
                MAX_REPLAY_WORK_UNITS,
            ] {
                let mut before = ReplayBudget::unobserved();
                let mut now = ReplayBudget::unobserved();
                before.remaining = limit;
                now.remaining = limit;
                assert_eq!(
                    ordinary_replay(&program, f, false, &mut now),
                    ordinary_replay(&program, f, true, &mut before)
                );
                assert_eq!(now.remaining, before.remaining);
                assert_eq!(now.skeleton_remaining, before.skeleton_remaining);
            }
            assert_eq!(f.cleanup_plan, original);
        }
    }
    assert!(every_invoice_lower);
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
// Authenticate the current whole source before applying only the finite reverse
// edits to the fixed immediate92d predecessor. Never repin the old target.
pub(super) fn prior_source(path: &str, bytes: Vec<u8>) -> Vec<u8> {
    assert_eq!(
        digest(include_bytes!("shared_insertions_inverse.json")),
        "ce2d057bcc5842f4a2435d59f3f7fc23c8112e32ba37569ccf5ead23ad8e54ae"
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("shared_insertions_inverse.json")).unwrap();
    let Some(row) = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"].as_str() == Some(path))
    else {
        return bytes;
    };
    let compiled: &[u8] = match path {
        "src/cleanup_plan/replay/path_join.rs" => include_bytes!("../path_join.rs"),
        "src/cleanup_plan/replay/path_join/frontier.rs" => include_bytes!("frontier.rs"),
        "src/cleanup_plan/replay/path_join/shared_insertions_before.rs" => {
            include_bytes!("shared_insertions_before.rs")
        }
        "src/cleanup_plan/replay/path_join/shared_insertions_before/frontier.rs" => {
            include_bytes!("shared_insertions_before/frontier.rs")
        }
        "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_states.rs" => {
            include_bytes!("shared_insertions_before/boxed_states.rs")
        }
        "src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_flow.rs" => {
            include_bytes!("shared_insertions_before/boxed_flow.rs")
        }
        "src/cleanup_plan/replay/path_join/held_edge_tests.rs" => {
            include_bytes!("held_edge_tests.rs")
        }
        _ => panic!("unadmitted shared insertion inverse"),
    };
    let compiled = crate::bounded_output::census_before(path, compiled);
    assert_eq!(bytes.as_slice(), compiled.as_slice());
    assert_eq!(bytes.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(digest(&bytes), row["sha256"].as_str().unwrap());
    let mut actual = bytes;
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let at = edit[0].as_u64().unwrap() as usize;
        let value = edit[1].as_str().unwrap().as_bytes();
        assert_eq!(&actual[at..at + value.len()], value);
        actual.splice(at..at + value.len(), edit[2].as_str().unwrap().bytes());
    }
    assert_eq!(actual.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(digest(&actual), row["before_sha256"].as_str().unwrap());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        actual,
        std::fs::read(root.join(row["full_before"].as_str().unwrap())).unwrap()
    );
    actual
}
#[test]
fn complete_immediate_driver_and_receivers_keep_exact_whole_preimages_and_shared_helpers() {
    assert_eq!(
        digest(include_bytes!("shared_insertions_inverse.json")),
        "ce2d057bcc5842f4a2435d59f3f7fc23c8112e32ba37569ccf5ead23ad8e54ae"
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("shared_insertions_inverse.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let raw = std::fs::read(root.join(path)).unwrap();
        prior_source(path, crate::bounded_output::census_before(path, &raw));
    }
    let marker = "fn validate_control_group_with_budget";
    assert_eq!(
        fused_tests::before_source(
            "src/cleanup_plan/replay/path_join.rs",
            include_str!("../path_join.rs").to_owned()
        )
        .split_once(marker)
        .unwrap()
        .1,
        include_str!("shared_insertions_before/driver.rs.txt")
            .split_once(marker)
            .unwrap()
            .1
    );
    for (path, expected) in [
        (
            "src/cleanup_plan/replay/path_join/boxed_states.rs",
            "1cb644157ccf8246f6a81011d0dd099a64f1809cffae15bb3e993c1d64eb93c5",
        ),
        (
            "src/cleanup_plan/replay/path_join/boxed_flow.rs",
            "6632ceeabe694a459ac118deeb68d858ac2dfa75e6d3ab6dccd3fc7c24e91c3d",
        ),
        (
            "src/cleanup_plan/replay/path_join/branch_join.rs",
            "d3633b8f9fd25ed9036c7387541961e94147743613423e2f945051be3c434a54",
        ),
        (
            "src/cleanup_plan/replay/path_join/storage.rs",
            "671900b2a5b62cba38ad34777f69d8f7fa2169bacac174a4237e1a24fbaf07c5",
        ),
        (
            "src/cleanup_plan/replay/physical_storage.rs",
            "5735899d648a1a00c84a00b1d32ebb2f75f3f623a4bda19196afee6f1994dead",
        ),
        (
            "src/cleanup_plan/replay/path_join/before_frontier/custody/shared_readset.json",
            "a6004b3b67c5af61e98e225ae1716e3aa5dd9c08d01f14afa02d1ba73f62c4f7",
        ),
        (
            "src/cleanup_plan/replay/path_join/before_frontier/custody/inverse.json",
            "86a572cc73c1b4981a710e5d7fba725ef9723f62586a30c38fb2bc4b9f0fe510",
        ),
    ] {
        assert_eq!(digest(&std::fs::read(root.join(path)).unwrap()), expected);
    }
}
