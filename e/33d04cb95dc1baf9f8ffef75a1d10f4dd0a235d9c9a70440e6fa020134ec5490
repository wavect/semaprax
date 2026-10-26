//! Actual field-local shipping roles versus full original and immediate callers.
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use std::{cell::Cell, mem::size_of};
#[derive(Clone, Copy, Default, Debug)]
struct Counts {
    directory: usize,
    entries: usize,
    promotions: usize,
    one: usize,
    many: usize,
    groups: usize,
    carries: usize,
    history_saving: i128,
    trivial_histories: usize,
    history_calls: usize,
}
thread_local! { static COUNTS:Cell<Counts>=const {Cell::new(Counts{directory:0,entries:0,promotions:0,one:0,many:0,groups:0,carries:0,history_saving:0,trivial_histories:0,history_calls:0})}; }
pub(super) fn begin(directory: usize) {
    COUNTS.with(|c| {
        c.set(Counts {
            directory,
            ..Counts::default()
        })
    });
}
fn bump(select: fn(&mut Counts) -> &mut usize) {
    COUNTS.with(|c| {
        let mut n = c.get();
        let p = select(&mut n);
        *p = p.checked_add(1).unwrap();
        c.set(n);
    });
}
pub(super) fn entry_bank() {
    bump(|n| &mut n.entries);
}
pub(super) fn promotion() {
    bump(|n| &mut n.promotions);
}
pub(super) fn one_block() {
    bump(|n| &mut n.one);
}
pub(super) fn many_block() {
    bump(|n| &mut n.many);
}
pub(super) fn carry_block() {
    bump(|n| &mut n.carries);
}
pub(super) fn history_call() { bump(|n| &mut n.history_calls); }
pub(super) fn trivial_history(count: usize) {
    assert!(count <= 1);
    let node = crate::cleanup_plan::replay::physical_storage::btree_node_upper::<LivenessFlagId, ()>().unwrap();
    let successors = crate::cleanup_plan::replay::physical_storage::btree_node_upper::<LivenessFlagId, BTreeSet<LivenessFlagId>>().unwrap();
    let degrees = crate::cleanup_plan::replay::physical_storage::btree_node_upper::<LivenessFlagId, usize>().unwrap();
    // Full old history-reference backing and flags/successor/degree/ready nodes
    // physically do not form. Selector/zero payment remains in the block bank.
    let saving = size_of::<&Vec<LivenessFlagId>>() as i128
        + count as i128 * (node + successors + degrees + node) as i128;
    COUNTS.with(|c| { let mut n=c.get(); n.trivial_histories+=1; n.history_saving+=saving; c.set(n); });
}
pub(super) fn unique_history(count: usize) {
    // The original algorithm always allocates one history-reference slot.
    // With unique flags it inserts one successor+indegree entry per flag,
    // one edge per adjacent pair, and each flag once into the ready set.
    // Every original flags-set node/insert remains in shipping and cancels.
    let node = |map: usize| map as i128;
    let set =
        crate::cleanup_plan::replay::physical_storage::btree_node_upper::<LivenessFlagId, ()>()
            .unwrap();
    let successors = crate::cleanup_plan::replay::physical_storage::btree_node_upper::<
        LivenessFlagId,
        BTreeSet<LivenessFlagId>,
    >()
    .unwrap();
    let degrees =
        crate::cleanup_plan::replay::physical_storage::btree_node_upper::<LivenessFlagId, usize>()
            .unwrap();
    let saving = size_of::<&Vec<LivenessFlagId>>() as i128
        + count as i128 * (node(successors) + node(degrees) + node(set))
        + count.saturating_sub(1) as i128 * node(set)
        - (count + 1) as i128 * boxed_flow::SINGLE_HISTORY_STEP as i128;
    COUNTS.with(|c| {
        let mut n = c.get();
        n.history_saving += saving;
        c.set(n);
    });
}
pub(super) fn group_block() {
    bump(|n| &mut n.groups);
}
// Signed whole-invoice difference: an added role cannot disappear by a
// saturating subtraction or a promise that one branch is usually absent.
pub(in crate::cleanup_plan::replay::path_join) fn immediate_saving() -> i128 {
    let n = COUNTS.with(Cell::get);
    type Old = super::super::frontier::Insertions<PathState>;
    type New = frontier::ShippingInsertions<PathState>;
    type OI = super::super::frontier::Incoming<PathState>;
    type NI = frontier::Incoming<PathState>;
    let old_directory = size_of::<(OI, OI, OI, &mut fn() -> OI)>();
    let new_directory = size_of::<(
        NI,
        NI,
        NI,
        &mut fn() -> NI,
        Option<Box<PathState>>,
        Option<Box<BTreeSet<Box<PathState>>>>,
        &Option<Box<PathState>>,
        &Option<Box<PathState>>,
        usize,
        usize,
        usize,
        usize,
        bool,
        bool,
        &VecDeque<BlockId>,
        &VecDeque<BlockId>,
        &Option<Box<PathState>>,
        &Option<Box<BTreeSet<Box<PathState>>>>,
        &usize,
        &usize,
        usize,
        usize,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
    )>();
    let d = |a: usize, b: usize| a as i128 - b as i128;
    n.history_saving - n.history_calls as i128 * boxed_flow::SINGLE_HISTORY_CALL as i128
        + d(Old::controls(), New::controls())
        + n.entries as i128 * d(Old::entry_controls(), New::entry_controls())
        + n.promotions as i128 * d(Old::promotion_controls(), New::promotion_controls())
        + n.directory as i128
            * (d(old_directory, new_directory) + d(size_of::<OI>(), size_of::<NI>()))
        + n.one as i128
            * d(
                super::super::boxed_flow::BLOCK_CONTROLS,
                boxed_flow::BLOCK_ONE,
            )
        + n.many as i128
            * d(
                super::super::boxed_flow::BLOCK_CONTROLS,
                boxed_flow::BLOCK_MANY,
            )
        // Each carried edge existed in the independent immediate driver as
        // one full held ENTRY renewal and one One block. Shipping retains
        // the actual auth/common block bank and the full cold selectors.
        + n.carries as i128
            * (Old::entry_controls() as i128
                + super::super::boxed_flow::BLOCK_CONTROLS as i128
                - New::CARRY_AUTH as i128
                - boxed_flow::CARRY_BLOCK as i128)
        + n.groups as i128
            * d(
                super::super::boxed_flow::BLOCK_CONTROLS,
                boxed_flow::BLOCK_CONTROLS,
            )
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

fn replay(
    p: &ResolvedProgram,
    f: &ResolvedFunction,
    kind: u8,
    b: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(f, b)?;
    let leaves = collect_leaves_paid(f, b)?;
    let auth = if lexical_cert::requires_lexical(f) {
        let owner = selected_owner::owner(p, f, b)?;
        let facts = lexical_hir::collect(f, b)?;
        Some(lexical_cert::authenticate(p, f, &owner, &facts, b)?)
    } else {
        None
    };
    match kind {
        0 => super::super::before_frontier::validate_path_states(
            p,
            f,
            &storage,
            &leaves,
            b,
            auth.as_ref(),
            None,
        ),
        1 => super::super::validate_path_states_before_field_local(
            p,
            f,
            &storage,
            &leaves,
            b,
            auth.as_ref(),
            None,
        ),
        2 => super::validate_path_states(p, f, &storage, &leaves, b, auth.as_ref(), None),
        _ => unreachable!(),
    }
}
#[test]
fn complete_original_and_immediate_callers_require_net_invoice_improvement_including_cold_directory(
) {
    // Real owned bytes and a retained view cross repeated ordinary branches;
    // every source/HIR/PLAN is checked before observing its complete driver.
    let mut dense = String::from("module dense; @id(\"dense.f\") fn f(flag:bool)->i64 {let owned=bytes_zeroed(3usize);let view=bytes_as_slice(owned);let mut next=0;");
    for _ in 0..64 {
        dense.push_str("if flag {next=next+1;0}else{0};");
    }
    dense.push_str(
        "if byte_len(view)==3usize {next+7}else{0}} @id(\"main\") fn main()->i64 {f(false)}",
    );
    let mut actual_carry = false;
    let mut positive_dense = false;
    for source in [TINY, SOURCE, LEXICAL, RENEWAL, dense.as_str()] {
        let p = crate::hir::resolve(&crate::check(source, "field-local.spx").unwrap()).unwrap();
        for f in &p.functions {
            let original = f.cleanup_plan.clone();
            let run = |kind, limit, floor| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        assert!(set_active_floor(floor));
                        let mut b = ReplayBudget::new();
                        let result = replay(&p, f, kind, &mut b);
                        let work = (b.remaining, b.skeleton_remaining, b.skeleton_spent);
                        if result.is_err() {
                            assert!(b.reserve_bytes(f, 0, "field-local sticky zero").is_err());
                        }
                        drop(b);
                        result.map(|()| work).map_err(|e| {
                            let fields = (e.code, e.severity, e.message, e.path, e.span, e.help);
                            drop(fields);
                            ()
                        })
                    })
                })
            };
            let old = run(0, 64 * 1024 * 1024, 0);
            let immediate = run(1, 64 * 1024 * 1024, 0);
            let current = run(2, 64 * 1024 * 1024, 0);
            assert!(old.0 .0.is_ok() && immediate.0 .0.is_ok() && current.0 .0.is_ok());
            assert_eq!(old.0 .0, current.0 .0);
            assert_eq!(immediate.0 .0, current.0 .0);
            assert_eq!(
                immediate.2 as i128 - current.2 as i128,
                immediate_saving(),
                "every actual typed role, including final unused entries"
            );
            for row in [&old, &immediate, &current] {
                assert!(!row.1 && row.3.is_none());
                let a = row.0 .1;
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
                assert!(a.allocated <= row.2);
            }
            eprintln!("919 field-local function={} original_reservations={} immediate_reservations={} current_reservations={} original_allocator={:?} immediate_allocator={:?} current_allocator={:?} counts={:?}",f.id.as_str(),old.2,immediate.2,current.2,old.0.1,immediate.0.1,current.0.1,COUNTS.with(Cell::get));
            // Cold losses remain visible in the full receipts. Positive net
            // must be earned on the whole ORIGINAL ordinary composition, not
            // inferred from an isolated receiver or immediate-fusion bank.
            if f.id.as_str() == "probe" {
                assert!(COUNTS.with(Cell::get).carries > 0, "probe caller executes retained Box carry");
                actual_carry = true;
            }
            if f.id.as_str() == "dense.f" {
                assert!(
                    COUNTS.with(Cell::get).trivial_histories > 0,
                    "dense caller executes the actual trivial-history owner elimination"
                );
                assert!(
                    current.2 < old.2 && current.2 < immediate.2,
                    "dense complete caller must earn original net: original={} immediate={} current={}",
                    old.2, immediate.2, current.2
                );
                positive_dense = true;
            }
            for (kind, paid) in [(0, old.2), (1, immediate.2), (2, current.2)] {
                for (limit, floor, success) in [
                    (paid, 0, true),
                    (paid - 1, 0, false),
                    (0, 0, false),
                    (paid, paid, false),
                ] {
                    let row = run(kind, limit, floor);
                    assert_eq!(row.0 .0.is_ok(), success);
                    assert_eq!(row.1, !success);
                    let a = row.0 .1;
                    assert!(!a.invalid && a.live == 0);
                    assert_eq!(a.allocated, a.freed);
                    assert!(a.allocated <= row.2 && row.2 <= limit);
                }
            }
            assert_eq!(f.cleanup_plan, original);
        }
    }
    assert!(actual_carry, "the actual eligible probe carry was executed");
    assert!(
        positive_dense,
        "the actual dense ordinary caller was executed"
    );
}
#[test]
fn complete_old_work_and_six_field_refusals_remain_exact() {
    for source in [TINY, SOURCE, LEXICAL, RENEWAL] {
        let p =
            crate::hir::resolve(&crate::check(source, "field-local-work.spx").unwrap()).unwrap();
        for f in &p.functions {
            let run = |kind, cut| {
                let mut b = ReplayBudget::unobserved();
                b.remaining = cut;
                let r = replay(&p, f, kind, &mut b);
                (r, (b.remaining, b.skeleton_remaining, b.skeleton_spent))
            };
            // The independent full original defines the complete ordinary
            // work frontier. Skeleton/physical domains keep their separate
            // controls; this loop varies only remaining ordinary work.
            let original_full = run(0, MAX_REPLAY_WORK_UNITS);
            let current_full = run(2, MAX_REPLAY_WORK_UNITS);
            assert!(original_full.0.is_ok());
            assert_eq!(original_full, current_full);
            let spent = MAX_REPLAY_WORK_UNITS
                .checked_sub(original_full.1 .0)
                .unwrap();
            for cut in 0..=spent.checked_add(1).unwrap() {
                let (old, ow) = run(0, cut);
                let (new, nw) = run(2, cut);
                assert_eq!(ow, nw);
                match (old, new) {
                    (Ok(()), Ok(())) => {}
                    (Err(a), Err(b)) => assert_eq!(
                        (a.code, a.severity, a.message, a.path, a.span, a.help),
                        (b.code, b.severity, b.message, b.path, b.span, b.help)
                    ),
                    _ => panic!("original deterministic work frontier changed"),
                }
            }
        }
    }
}
#[test]
fn all_architecture_layouts_price_actual_field_and_surviving_tree_roles() {
    assert_eq!(
        boxed_flow::BLOCK_CONTROLS,
        super::super::boxed_flow::BLOCK_CONTROLS,
        "unchanged group replay keeps every original tariff"
    );
    macro_rules! check {
        ($t:ty) => {{
            type O = super::super::frontier::Incoming<$t>;
            type N = frontier::Incoming<$t>;
            assert_eq!(
                size_of::<N>(),
                size_of::<(Option<Box<$t>>, Option<Box<BTreeSet<Box<$t>>>>)>()
            );
            assert!(
                size_of::<N>() >= size_of::<Option<Box<$t>>>(),
                "no assumed niche/cold directory credit"
            );
            assert_eq!(
                frontier::ShippingInsertions::<$t>::GROUP_ENTRY_CONTROLS,
                super::super::frontier::Insertions::<$t>::GROUP_ENTRY_CONTROLS
            );
            assert_eq!(
                frontier::ShippingInsertions::<$t>::promotion_controls(),
                4 * size_of::<Option<Box<$t>>>()
                    + 2 * size_of::<&mut Option<Box<$t>>>()
                    + 4 * size_of::<BTreeSet<Box<$t>>>()
                    + 4 * size_of::<Box<BTreeSet<Box<$t>>>>()
                    + 2 * size_of::<Option<Box<BTreeSet<Box<$t>>>>>()
            );
            let _ = (size_of::<O>(), size_of::<N>());
        }};
    }
    check!(());
    check!(u64);
    check!([u64; 50]);
}

#[derive(Debug)]
struct Key {
    value: u64,
    serial: usize,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Key {}
thread_local! {
 static TRACE:Cell<[(u64,u64);2048]>=const{Cell::new([(0,0);2048])};
 static TRACE_LEN:Cell<usize>=const{Cell::new(0)};
 static DROPS:Cell<[usize;128]>=const{Cell::new([0;128])};
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        TRACE_LEN.with(|n| {
            let at = n.get();
            assert!(at < 2048);
            TRACE.with(|t| {
                let mut a = t.get();
                a[at] = (self.value, other.value);
                t.set(a);
            });
            n.set(at + 1);
        });
        self.value.cmp(&other.value)
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        DROPS.with(|d| {
            let mut a = d.get();
            a[self.serial] = a[self.serial].checked_add(1).unwrap();
            d.set(a);
        });
    }
}
fn function() -> ResolvedFunction {
    crate::hir::resolve(&crate::check(TINY, "field-local-key.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
#[test]
fn direct_field_publication_keeps_compare_order_boxes_and_partial_drop_at_every_actual_cut() {
    let function_owner = function();
    let function = &function_owner;
    let run = |old, limit, floor, reverse| {
        TRACE.with(|t| t.set([(0, 0); 2048]));
        TRACE_LEN.with(|n| n.set(0));
        DROPS.with(|d| d.set([0; 128]));
        begin(0);
        let row = with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let mut budget_owner = ReplayBudget::new();
                let budget = &mut budget_owner;
                macro_rules! execute {
                    ($module:ident,$receiver:ident,$old:tt) => {{
                        let mut target = $module::Incoming::<Key>::default();
                        let mut receiver =
                            match $module::$receiver::<Key>::acquire(function, budget) {
                                Ok(r) => r,
                                Err(e) => {
                                    drop(e);
                                    return false;
                                }
                            };
                        macro_rules! insert {
                            (true,$r:ident,$target:expr,$value:expr) => {
                                $r.insert_owned(function, $target, $value, budget)
                            };
                            (false,$r:ident,$target:expr,$value:expr) => {
                                frontier::insert_owned!(Key, $r, function, $target, $value, budget)
                            };
                        }
                        macro_rules! held {
                            (true,$r:ident,$target:expr,$value:expr) => {
                                $r.insert_held(function, $target, $value, budget)
                            };
                            (false,$r:ident,$target:expr,$value:expr) => {
                                frontier::insert_held!(Key, $r, function, $target, $value, budget)
                            };
                        }
                        // Both orders promote One and then cross a real B-tree split; duplicates
                        // still execute the same comparator before positive publication renewal.
                        let mut first_identity = None;
                        for serial in 0..64 {
                            let key = (serial % 32) as u64;
                            let value = if reverse { 31 - key } else { key };
                            if serial % 2 == 0 {
                                insert!($old, receiver, &mut target, Key { value, serial });
                            } else {
                                // This caller really constructs the held Box; prepay its
                                // payload and constructor/receiving carriers on both arms.
                                if budget
                                    .reserve_bytes(
                                        function,
                                        size_of::<Key>() + 2 * size_of::<Box<Key>>(),
                                        "field-local held caller",
                                    )
                                    .is_err()
                                {
                                    drop(target);
                                    drop(receiver);
                                    return false;
                                }
                                held!($old, receiver, &mut target, Box::new(Key { value, serial }));
                            }
                            if serial == 0 && !receiver.failed {
                                first_identity = Some(std::ptr::from_ref(target.one().as_ref()));
                            }
                            if receiver.failed {
                                let error = receiver.failure();
                                assert_eq!(error.code, "SPX-H006");
                                drop(error);
                                drop(target);
                                drop(receiver);
                                return false;
                            }
                        }
                        assert!(target
                            .iter()
                            .any(|v| Some(std::ptr::from_ref(v.as_ref())) == first_identity));
                        assert_eq!(target.len(), 32);
                        let mut values = [0u64; 32];
                        for (i, value) in target.iter().enumerate() {
                            values[i] = value.value;
                        }
                        assert_eq!(values, std::array::from_fn(|i| i as u64));
                        drop(target);
                        drop(receiver);
                        true
                    }};
                }
                let success = if old {
                    use super::super::frontier as immediate;
                    execute!(immediate, Insertions, true)
                } else {
                    execute!(frontier, ShippingInsertions, false)
                };
                drop(budget_owner);
                success
            })
        });
        (
            row,
            TRACE.with(Cell::get),
            TRACE_LEN.with(Cell::get),
            DROPS.with(Cell::get),
        )
    };
    for reverse in [false, true] {
        let old = run(true, usize::MAX, 0, reverse);
        let new = run(false, usize::MAX, 0, reverse);
        assert!(old.0 .0 .0 && new.0 .0 .0 && !old.0 .1 && !new.0 .1);
        assert_eq!((old.1, old.2, old.3), (new.1, new.2, new.3));
        for (immediate, paid) in [(true, old.0 .2), (false, new.0 .2)] {
            for (limit, floor, success) in [
                (paid, 0, true),
                (paid - 1, 0, false),
                (0, 0, false),
                (paid, paid, false),
            ] {
                let r = run(immediate, limit, floor, reverse);
                assert_eq!(r.0 .0 .0, success);
                assert_eq!(r.0 .1, !success);
                let a = r.0 .0 .1;
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
                assert!(a.allocated <= r.0 .2 && r.0 .2 <= limit);
            }
        }
    }
}
#[test]
fn foreign_function_first_diagnostic_and_sticky_zero_remain_the_complete_old_contract() {
    let function_owner = function();
    let foreign_owner = function_owner.clone();
    let function = &function_owner;
    let foreign = &foreign_owner;
    let run = |old| {
        with_limit_usage_refusal(1 << 20, || {
            let mut budget_owner = ReplayBudget::new();
            let budget = &mut budget_owner;
            if old {
                let mut receiver =
                    super::super::frontier::Insertions::<u64>::acquire(function, budget).unwrap();
                let mut target = super::super::frontier::Incoming::default();
                receiver.insert_owned(foreign, &mut target, 1, budget);
                assert!(receiver.failed && target.is_empty());
                let error = receiver.failure();
                assert!(budget
                    .reserve_bytes(function, 0, "field-local foreign sticky")
                    .is_err());
                drop(target);
                drop(receiver);
                drop(budget_owner);
                error
            } else {
                let mut receiver =
                    frontier::ShippingInsertions::<u64>::acquire(function, budget).unwrap();
                let mut target = frontier::Incoming::<u64>::default();
                frontier::insert_owned!(u64, receiver, foreign, &mut target, 1, budget);
                assert!(receiver.failed && target.is_empty());
                let error = receiver.failure();
                assert!(budget
                    .reserve_bytes(function, 0, "field-local foreign sticky")
                    .is_err());
                drop(target);
                drop(receiver);
                drop(budget_owner);
                error
            }
        })
    };
    let old = run(true);
    let new = run(false);
    assert_eq!(
        (
            old.0.code,
            old.0.severity,
            old.0.message,
            old.0.path,
            old.0.span,
            old.0.help
        ),
        (
            new.0.code,
            new.0.severity,
            new.0.message,
            new.0.path,
            new.0.span,
            new.0.help
        )
    );
    assert!(old.1 && new.1);
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct AuthorityState([u64; 50]);
thread_local! { static AUTH_DROPS:Cell<usize>=const {Cell::new(0)}; }
impl Drop for AuthorityState {
    fn drop(&mut self) {
        AUTH_DROPS.with(|n| n.set(n.get() + 1));
    }
}
#[test]
fn shipping_ordinary_and_group_entries_keep_all_six_authority_and_restored_ledger_scenarios() {
    use super::super::frontier as immediate;
    macro_rules! owned {
        (true,$receiver:ident,$function:expr,$target:expr,$value:expr,$budget:expr $(,)?) => {
            $receiver.insert_owned($function, $target, $value, $budget)
        };
        (false,$receiver:ident,$function:expr,$target:expr,$value:expr,$budget:expr $(,)?) => {{
            let function = $function;
            let budget = $budget;
            frontier::insert_owned!(AuthorityState, $receiver, function, $target, $value, budget);
        }};
    }
    let f = function();
    let other = function();
    for grouped in [false, true] {
        for scenario in 0..6 {
            let observe = |old| {
                AUTH_DROPS.with(|n| n.set(0));
                let (answer, refused, paid, first) = with_limit_usage_refusal(1 << 20, || {
                    let mut b = ReplayBudget::new();
                    let mut foreign = ReplayBudget::new();
                    macro_rules! run {
                        ($module:ident,$receiving:ident,$lexical:tt) => {{
                            let mut receiver = $module::$receiving::acquire(&f, &mut b).unwrap();
                            let mut incoming = $module::Incoming::<AuthorityState>::default();
                            let mut group = BTreeSet::new();
                            let mut held = if grouped {
                                owned!(
                                    $lexical,
                                    receiver,
                                    &f,
                                    &mut incoming,
                                    AuthorityState([7; 50]),
                                    &mut b
                                );
                                assert!(!receiver.failed);
                                Some(incoming.into_one().next().unwrap())
                            } else {
                                None
                            };
                            let mut incoming = $module::Incoming::<AuthorityState>::default();
                            match scenario {
                                1 => {
                                    assert!(set_active_floor(
                                        crate::bounded_output::active_remaining().unwrap()
                                    ));
                                }
                                2 => {
                                    let remaining =
                                        crate::bounded_output::active_remaining().unwrap();
                                    assert!(set_active_floor(remaining - 1));
                                }
                                5 => {
                                    let ((), child_refused, child_paid, child_first) =
                                        with_limit_usage_refusal(1, || {
                                            if grouped {
                                                receiver.insert_group_held(
                                                    &f,
                                                    &mut group,
                                                    held.take().unwrap(),
                                                    &mut b,
                                                );
                                            } else {
                                                owned!(
                                                    $lexical,
                                                    receiver,
                                                    &f,
                                                    &mut incoming,
                                                    AuthorityState([1; 50]),
                                                    &mut b,
                                                );
                                            }
                                        });
                                    assert!(
                                        !child_refused && child_paid == 0 && child_first.is_none()
                                    );
                                }
                                _ => {}
                            }
                            if scenario != 5 {
                                let function = if scenario == 3 { &other } else { &f };
                                let budget = if scenario == 4 { &mut foreign } else { &mut b };
                                if grouped {
                                    receiver.insert_group_held(
                                        function,
                                        &mut group,
                                        held.take().unwrap(),
                                        budget,
                                    );
                                } else {
                                    owned!(
                                        $lexical,
                                        receiver,
                                        function,
                                        &mut incoming,
                                        AuthorityState([1; 50]),
                                        budget,
                                    );
                                }
                            }
                            let error = if receiver.failed {
                                Some(receiver.failure())
                            } else {
                                None
                            };
                            if scenario == 0 {
                                assert!(error.is_none());
                                if grouped {
                                    owned!(
                                        $lexical,
                                        receiver,
                                        &f,
                                        &mut incoming,
                                        AuthorityState([7; 50]),
                                        &mut b,
                                    );
                                    receiver.insert_group_held(
                                        &f,
                                        &mut group,
                                        incoming.into_one().next().unwrap(),
                                        &mut b,
                                    );
                                    assert_eq!(group.len(), 1);
                                } else {
                                    for v in [2, 2, 3] {
                                        owned!(
                                            $lexical,
                                            receiver,
                                            &f,
                                            &mut incoming,
                                            AuthorityState([v; 50]),
                                            &mut b,
                                        );
                                    }
                                    assert_eq!(incoming.len(), 3);
                                    assert_eq!(incoming.iter().map(|v| v.0[0]).sum::<u64>(), 6);
                                    drop(incoming);
                                }
                                assert!(!receiver.failed);
                            } else {
                                assert!(error.is_some());
                                let at = crate::bounded_output::active_remaining();
                                // An already-failed owner must neither query nor publish on retry.
                                let mut retry = $module::Incoming::<AuthorityState>::default();
                                owned!(
                                    $lexical,
                                    receiver,
                                    &other,
                                    &mut retry,
                                    AuthorityState([99; 50]),
                                    &mut foreign,
                                );
                                assert!(retry.is_empty() && receiver.failed);
                                assert_eq!(crate::bounded_output::active_remaining(), at);
                                drop(incoming);
                            }
                            assert_eq!(foreign.remaining, MAX_REPLAY_WORK_UNITS);
                            crate::bounded_output::clear_active_floor();
                            drop(group);
                            drop(receiver);
                            (
                                error,
                                b.remaining,
                                b.skeleton_remaining,
                                AUTH_DROPS.with(std::cell::Cell::get),
                            )
                        }};
                    }
                    if old {
                        run!(immediate, Insertions, true)
                    } else {
                        run!(frontier, ShippingInsertions, false)
                    }
                });
                assert!(paid <= 1 << 20);
                (answer, refused, first)
            };
            // The physical first receipt is intentionally compared only where
            // requests are identical; owner-acquisition byte prices differ.
            let old = observe(true);
            let new = observe(false);
            assert_eq!(old.0, new.0, "group={grouped} scenario={scenario}");
            assert_eq!(old.1, new.1);
            assert_eq!(old.2.is_some(), new.2.is_some());
        }
    }
}

#[test]
fn real_shipping_partial_promotion_unwind_drops_both_original_owners() {
    // Rust 1.97's default assertion reporter initializes a pthread-once global
    // outside the compiler allocation observer, equally before both measured
    // arms. Keep the default hook and every measured injected panic unchanged.
    let report = std::panic::catch_unwind(|| assert!(false, "field-local panic reporter setup"));
    assert!(report.is_err());
    drop(report);
    thread_local! {
        static COMPARES:Cell<usize>=const {Cell::new(0)};
        static FREED:Cell<usize>=const {Cell::new(0)};
    }
    #[derive(Eq, PartialEq)]
    struct Unwind(u64);
    impl Ord for Unwind {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            COMPARES.with(|n| {
                n.set(n.get() + 1);
                assert_ne!(n.get(), 2, "injected publication comparison unwind");
            });
            self.0.cmp(&other.0)
        }
    }
    impl PartialOrd for Unwind {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Drop for Unwind {
        fn drop(&mut self) {
            FREED.with(|n| n.set(n.get() + 1));
        }
    }
    let f = function();
    let function = &f;
    for old in [true, false] {
        COMPARES.with(|n| n.set(0));
        FREED.with(|n| n.set(0));
        let (((), a), refused, paid, first) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut budget_owner = ReplayBudget::new();
                let budget = &mut budget_owner;
                macro_rules! run {
                    ($module:ident,$receiver:ident,$old:tt) => {{
                        let mut owner =
                            $module::$receiver::<Unwind>::acquire(function, budget).unwrap();
                        let mut target = $module::Incoming::<Unwind>::default();
                        macro_rules! owned {
                            (true,$value:expr) => {
                                owner.insert_owned(function, &mut target, $value, budget)
                            };
                            (false,$value:expr) => {
                                frontier::insert_owned!(
                                    Unwind,
                                    owner,
                                    function,
                                    &mut target,
                                    $value,
                                    budget
                                )
                            };
                        }
                        owned!($old, Unwind(1));
                        assert!(!owner.failed);
                        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            owned!($old, Unwind(2));
                        }));
                        assert!(error.is_err());
                        drop(error);
                        assert!(target.is_empty());
                        assert_eq!(FREED.with(Cell::get), 2);
                        drop(target);
                        drop(owner);
                    }};
                }
                if old {
                    use super::super::frontier as immediate;
                    run!(immediate, Insertions, true);
                } else {
                    run!(frontier, ShippingInsertions, false);
                }
                drop(budget_owner);
            })
        });
        assert!(!refused && first.is_none() && !a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
        assert!(a.allocated <= paid);
    }
}

const SOURCE_PACKET_SHA: &str = "0bdc59ae516de9e1678b998e6b8d4fdb54c4bc5db08607acbd5fd788a3d9e98a";
fn source_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn restore_source(row: &serde_json::Value, current: &[u8]) -> Vec<u8> {
    assert_eq!(current.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(source_digest(current), row["sha256"].as_str().unwrap());
    let mut old = current.to_vec();
    for e in row["edits"].as_array().unwrap() {
        let at = e[0].as_u64().unwrap() as usize;
        let from = e[1].as_str().unwrap().as_bytes();
        assert_eq!(old.get(at..at + from.len()), Some(from));
        old.splice(at..at + from.len(), e[2].as_str().unwrap().bytes());
    }
    assert_eq!(old.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(source_digest(&old), row["before_sha256"].as_str().unwrap());
    old
}
// Closed source acquisition before the untouched earlier generation packets.
// The supplied whole source must equal compiled and freshly physical current;
// every edit has an exact index and the complete b23 endpoint is pinned.
pub(in crate::cleanup_plan::replay::path_join) fn source_before(
    path: &str,
    supplied: &[u8],
) -> Vec<u8> {
    let compiled: &[u8] = match path {
        "src/cleanup_plan/replay/path_join.rs" => include_bytes!("../../path_join.rs"),
        "src/cleanup_plan/replay/path_join/fused_tests.rs" => include_bytes!("../fused_tests.rs"),
        "src/cleanup_plan/replay/path_join/direct_one_tests.rs" => {
            include_bytes!("../direct_one_tests.rs")
        }
        "src/cleanup_plan/replay/path_join/shared_insertions_tests.rs" => {
            include_bytes!("../shared_insertions_tests.rs")
        }
        _ => return supplied.to_vec(),
    };
    let packet = include_bytes!("inverse.json");
    assert_eq!(source_digest(packet), SOURCE_PACKET_SHA);
    let data: serde_json::Value = serde_json::from_slice(packet).unwrap();
    assert_eq!(data["base"], "b23f09dd902d2fd79078acd9f07ff35d70e06a21");
    let rows = data["acquisitions"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let row = rows
        .iter()
        .find(|r| r["path"].as_str() == Some(path))
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = std::fs::read(root.join(path)).unwrap();
    assert_eq!(physical, compiled);
    assert_eq!(supplied, compiled);
    let old = restore_source(row, &physical);
    assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled);
    assert_eq!(
        std::fs::read(root.join("src/cleanup_plan/replay/path_join/field_local/inverse.json"))
            .unwrap(),
        packet
    );
    old
}
#[test]
fn all_shipping_bodies_and_complete_generation_sources_authenticate_before_every_inverse() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = include_bytes!("inverse.json");
    assert_eq!(source_digest(packet), SOURCE_PACKET_SHA);
    let data: serde_json::Value = serde_json::from_slice(packet).unwrap();
    let acquisitions = data["acquisitions"].as_array().unwrap();
    assert_eq!(acquisitions.len(), 4);
    for row in acquisitions {
        let path = row["path"].as_str().unwrap();
        let raw = std::fs::read(root.join(path)).unwrap();
        let old = source_before(path, &raw);
        if let Some(epoch) = row.get("warm_epoch") {
            // Preserve the whole current warm-harness predecessor before b23;
            // neither complete source image is relabelled or retargeted.
            assert_eq!(path, "src/cleanup_plan/replay/path_join/fused_tests.rs");
            let mut first = row.clone();
            first["edits"] = epoch["from_current"].clone();
            first["before_length"] = epoch["length"].clone();
            first["before_sha256"] = epoch["sha256"].clone();
            let warm = restore_source(&first, &raw);
            let mut second = row.clone();
            second["length"] = epoch["length"].clone();
            second["sha256"] = epoch["sha256"].clone();
            second["edits"] = epoch["to_b23"].clone();
            assert_eq!(restore_source(&second, &warm), old);
            let prior = &row["prior_b23_acquisition"];
            assert_eq!(prior["before_length"], row["before_length"]);
            assert_eq!(prior["before_sha256"], row["before_sha256"]);
        }
        assert_eq!(source_digest(&old), row["before_sha256"].as_str().unwrap());
        let mut changed = raw.clone();
        changed.push(0);
        assert!(std::panic::catch_unwind(|| source_before(path, &changed)).is_err());
        let mut missing = row.clone();
        missing["edits"].as_array_mut().unwrap().pop();
        assert!(std::panic::catch_unwind(|| restore_source(&missing, &raw)).is_err());
        let mut extra = row.clone();
        extra["edits"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!([0, "FOREIGN", "x"]));
        assert!(std::panic::catch_unwind(|| restore_source(&extra, &raw)).is_err());
    }
    let oldroot = source_before(
        "src/cleanup_plan/replay/path_join.rs",
        include_bytes!("../../path_join.rs"),
    );
    let oldroot = String::from_utf8(oldroot).unwrap();
    let start = oldroot
        .find("#[allow(clippy::too_many_arguments)]")
        .unwrap();
    let end = oldroot
        .find("#[cfg(test)]\nfn validate_control_group(")
        .unwrap();
    let shipping = data["shipping"].as_array().unwrap();
    assert_eq!(shipping.len(), 3);
    for row in shipping {
        let path = row["path"].as_str().unwrap();
        let (compiled, expected): (&[u8], &[u8]) = match path {
            "src/cleanup_plan/replay/path_join/field_local/mod.rs" => {
                (include_bytes!("mod.rs"), oldroot[start..end].as_bytes())
            }
            "src/cleanup_plan/replay/path_join/field_local/frontier.rs" => (
                include_bytes!("frontier.rs"),
                include_bytes!("../frontier.rs"),
            ),
            "src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs" => (
                include_bytes!("boxed_flow.rs"),
                include_bytes!("../boxed_flow.rs"),
            ),
            _ => panic!("foreign shipping source owner"),
        };
        assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled);
        assert_eq!(restore_source(row, compiled), expected);
        assert_eq!(std::fs::read(root.join(path)).unwrap(), compiled);
    }
    // Every current shipping child is joined above, and every independently
    // compiled b23 helper remains the same complete source, not a live alias.
    assert_eq!(
        std::fs::read(root.join("src/cleanup_plan/replay/path_join/field_local/tests.rs")).unwrap(),
        include_bytes!("tests.rs")
    );
    assert_eq!(
        std::fs::read(root.join("src/cleanup_plan/replay/path_join/field_local/inverse.json"))
            .unwrap(),
        packet
    );
}

#[test]
fn shipping_malformed_branch_arity_keeps_full_original_semantics_and_work_frontiers() {
    for arity in [0, 1, 3] {
        let mut p =
            crate::hir::resolve(&crate::check(SOURCE, "field-local-arity.spx").unwrap()).unwrap();
        let mut changed = false;
        for f in &mut p.functions {
            if let Some(edges) = f.cleanup_plan.blocks.iter_mut().find_map(|block| {
                if let CleanupTerminator::Branch(edges) = &mut block.terminator {
                    Some(edges)
                } else {
                    None
                }
            }) {
                let old = edges.clone();
                *edges = (0..arity).map(|i| old[i % old.len()]).collect();
                changed = true;
                break;
            }
        }
        assert!(changed);
        for f in &p.functions {
            for cut in [0, 1, 2, 7, MAX_REPLAY_WORK_UNITS] {
                let run = |kind| {
                    let mut b = ReplayBudget::unobserved();
                    b.remaining = cut;
                    let result = replay(&p, f, kind, &mut b);
                    (result, b.remaining, b.skeleton_remaining, b.skeleton_spent)
                };
                assert_eq!(
                    run(0),
                    run(2),
                    "full original malformed arity={arity} cut={cut}"
                );
                assert_eq!(
                    run(1),
                    run(2),
                    "whole immediate malformed arity={arity} cut={cut}"
                );
            }
        }
    }
}

#[test]
fn unique_single_history_retires_only_absent_graphs_and_duplicates_keep_the_whole_old_algorithm() {
    let owner = function();
    let function = &owner;
    for ids in [
        &[][..],
        &[9][..],
        &[9, 2, 7][..],
        &[9, 9][..],
        &[9, 2, 9][..],
    ] {
        let state = PathState {
            renewals: BTreeMap::new(),
            live_order: ids.iter().copied().map(LivenessFlagId).collect(),
            conditional_variants: Vec::new(),
            pending_failure: None,
            selected_failure: None,
            staged_copy_result: None,
            published: false,
            returned: None,
        };
        let old_state = state.clone();
        let original = {
            let mut b = ReplayBudget::unobserved();
            let result = validate_join_compatibility(
                function,
                std::iter::empty(),
                &state,
                BlockId(0),
                &mut b,
            );
            (result, b.remaining, b.skeleton_remaining, b.skeleton_spent)
        };
        let current = {
            let mut b = ReplayBudget::unobserved();
            let result =
                boxed_flow::validate_singleton_history(function, &state, BlockId(0), &mut b);
            (result, b.remaining, b.skeleton_remaining, b.skeleton_spent)
        };
        assert_eq!(
            original, current,
            "entire old result/diagnostic and work: {ids:?}"
        );
        assert_eq!(state, old_state);
        let run = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut b = ReplayBudget::new();
                    let result = match b.reserve_bytes(
                        function,
                        boxed_flow::SINGLE_HISTORY_CALL,
                        "ordinary singleton history call controls",
                    ) {
                        Ok(()) => boxed_flow::validate_singleton_history(
                            function,
                            &state,
                            BlockId(0),
                            &mut b,
                        ),
                        Err(error) => Err(error),
                    };
                    let success = result.is_ok();
                    if b.refusal_spent || crate::bounded_output::active_refused() {
                        assert!(b
                            .reserve_bytes(function, 0, "single history sticky zero")
                            .is_err());
                    }
                    drop(result);
                    drop(b);
                    success
                })
            })
        };
        let full = run(64 * 1024 * 1024);
        assert_eq!(full.0 .0, original.0.is_ok());
        assert!(!full.0 .1.invalid && full.0 .1.live == 0);
        assert_eq!(full.0 .1.allocated, full.0 .1.freed);
        assert!(full.0 .1.allocated <= full.2);
        for (limit, succeeds) in [
            (full.2, original.0.is_ok()),
            (full.2 - 1, false),
            (0, false),
        ] {
            let row = run(limit);
            assert_eq!(row.0 .0, succeeds);
            assert!(!row.0 .1.invalid && row.0 .1.live == 0);
            assert_eq!(row.0 .1.allocated, row.0 .1.freed);
            assert!(row.0 .1.allocated <= row.2 && row.2 <= limit);
        }
        assert_eq!(state, old_state);
    }
}
