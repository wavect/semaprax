//! Complete caller comparisons; all allocation measurements include final Drop.
use super::super::*;
use super::*;
use crate::bounded_output::{
    active_refused, active_remaining, classifier_string::ClassifierLedger, set_active_floor,
    with_limit_usage_refusal, ReservationRefusal,
};
use crate::cleanup_plan::replay::primitive_fixture;
use crate::test_allocation_observer::{measure, Metrics};

const TINY: &str = r#"module lexical.compact_tiny;
@id("probe") fn probe(n:i64)->i64 {while n==1 {break;} return 7;}
@id("main") fn main()->i64 {0}
"#;
const BRANCH: &str = r#"module lexical.compact_branch;
@id("probe") fn probe(n:i64)->i64 {
 let mut i=0;while i<2 {i=i+1;if n==0 {continue;};if n==1 {return 7;};0} return 9;
}
@id("main") fn main()->i64 {0}
"#;
const FULL: &str = r#"module lexical.compact_full;
@id("probe") fn probe(flag:bool)->i64 {
 let owner="live root";let key=str"x";let view=str_as_bytes(key);let mut outer=0;let mut total=0;
 while outer<3 {
  outer=outer+1;let mut inner=0;
  while inner<2 {
   inner=inner+1;if flag {continue;};if inner==2 {break;};
   total=total+inner;0
  }
  if outer==2 {continue;};
  if total>4 {return if byte_len(view)==1usize {7}else{0};};0
 }
 return total;
}
@id("main") fn main()->i64 {0}
"#;
fn fixture(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "lexical-compact.spx").unwrap()).unwrap()
}
fn probe(p: &ResolvedProgram) -> &ResolvedFunction {
    p.functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap()
}
#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 2048], usize),
    path: Option<([u8; 2048], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 2048], usize)>,
}
fn text(s: &str) -> ([u8; 2048], usize) {
    assert!(s.len() <= 2048);
    let mut b = [0; 2048];
    b[..s.len()].copy_from_slice(s.as_bytes());
    (b, s.len())
}
fn failure(d: Diagnostic) -> Failure {
    Failure {
        code: d.code,
        severity: d.severity,
        message: text(&d.message),
        path: d.path.as_deref().map(text),
        span: d.span,
        help: d.help.as_deref().map(text),
    }
}
struct Run {
    result: Result<(), Failure>,
    work: usize,
    held: usize,
    spent: usize,
    paid: usize,
    refused: bool,
    first: Option<ReservationRefusal>,
    heap: Metrics,
}
fn run(
    p: &ResolvedProgram,
    f: &ResolvedFunction,
    old: bool,
    selected: bool,
    work: usize,
    bytes: usize,
    floor: usize,
) -> Run {
    // This test-only adapter verifies its fixture with a temporary walk; fixture
    // preparation is excluded identically. Actual paid selected Context creation
    // and every production caller allocation below remain inside measurement.
    let source = primitive_fixture::Context::new(p);
    let ((result, heap), refused, paid, first) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            assert!(set_active_floor(floor));
            // Test-only observation clones the existing Rc handle: it allocates
            // no backing and makes no production reservation or invoice entry.
            let ledger = ClassifierLedger::capture();
            let mut b = if selected {
                match ReplayBudget::try_from_reservation(
                    f,
                    physical_storage::PhysicalReservation::new(usize::MAX),
                ) {
                    Ok(b) => b,
                    Err(setup) => {
                        // Keep the actual failed setup ledger alive through its
                        // retry and final Drop. Reconstruct the same compatibility
                        // Diagnostic as with_physical, after checking sticky state.
                        let (mut physical, reason) = setup.into_parts();
                        let first = ledger.first_refusal();
                        let remaining = active_remaining();
                        let charged = physical.charged();
                        assert!(physical.reserve(0).is_err());
                        assert!(physical.reserve(1).is_err());
                        assert_eq!(physical.charged(), charged);
                        assert_eq!(ledger.first_refusal(), first);
                        assert_eq!(active_remaining(), remaining);
                        drop(physical);
                        let diagnostic = replay_error(f, reason.message());
                        return (Err(failure(diagnostic)), 0, 0, 0);
                    }
                }
            } else {
                ReplayBudget::new()
            };
            b.remaining = work;
            let result = with_before(old, || {
                if selected {
                    let context = crate::cleanup_plan::replay::selected_context::Context::new_paid(
                        &source, f, &mut b, true,
                    )?;
                    crate::cleanup_plan::replay::lexical_replay::validate_paid(&context, f, &mut b)
                } else {
                    reserve_program_skeleton_work(p, std::iter::once(f), &mut b)?;
                    validate_structure_with_budget(p, f, &mut b, None)
                        .map_err(expression_facts::errors::EngineError::from)
                }
            })
            .map_err(|e| match e {
                expression_facts::errors::EngineError::Owned(d) => d,
                expression_facts::errors::EngineError::Structural(r) => r.legacy(f),
                expression_facts::errors::EngineError::Fact(r) => r.legacy(f),
                expression_facts::errors::EngineError::Failed => {
                    panic!("raw caller has no Session report to replace its first error")
                }
            })
            .map_err(failure);
            let spent = work - b.remaining;
            let held = b.skeleton_remaining;
            let skeleton = b.skeleton_spent;
            if b.refusal_spent || active_refused() {
                let first = ledger.first_refusal();
                let remaining = active_remaining();
                let charged = b.physical.as_ref().map(|physical| physical.charged());
                for bytes in [0, 1] {
                    let retry = b.reserve_bytes(f, bytes, "lexical compact sticky retry");
                    assert!(retry.is_err());
                    drop(retry);
                    assert_eq!(ledger.first_refusal(), first);
                    assert_eq!(active_remaining(), remaining);
                    assert_eq!(
                        b.physical.as_ref().map(|physical| physical.charged()),
                        charged
                    );
                }
            }
            drop(b);
            (result, spent, held, skeleton)
        })
    });
    Run {
        result: result.0,
        work: result.1,
        held: result.2,
        spent: result.3,
        paid,
        refused,
        first,
        heap,
    }
}
fn balanced(r: &Run) {
    assert!(!r.heap.invalid && r.heap.live == 0);
    // Work/semantic refusal need not select an enclosing byte receipt. Keep
    // that distinction: only an actual enclosing refusal requires one here.
    assert_eq!(r.first.is_some(), r.refused);
    if r.result.is_ok() {
        assert!(r.first.is_none());
    }
    assert_eq!(r.heap.allocated, r.heap.freed);
    if r.result.is_ok() {
        assert!(
            r.heap.allocated <= r.paid,
            "actual complete-caller backing must be paid"
        );
    }
}
#[test]
fn tiny_branch_and_full_lexical_callers_preserve_results_work_and_report_actual_costs() {
    for source in [TINY, BRANCH, FULL] {
        let p = fixture(source);
        let f = probe(&p);
        let plan = f.cleanup_plan.clone();
        for selected in [false, true] {
            let old = run(&p, f, true, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
            let current = run(&p, f, false, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
            assert_eq!(old.result, current.result);
            assert!(current.result.is_ok());
            assert!(!old.refused && !current.refused);
            assert_eq!(
                (old.work, old.held, old.spent),
                (current.work, current.held, current.spent)
            );
            balanced(&old);
            balanced(&current);
            eprintln!("lexical-compact {} selected={selected} old paid={} allocs={} allocated={} peak={}; current paid={} allocs={} allocated={} peak={} work={}",f.id,old.paid,old.heap.allocations,old.heap.allocated,old.heap.highwater,current.paid,current.heap.allocations,current.heap.allocated,current.heap.highwater,current.work);
            for _ in 0..3 {
                let repeat = run(&p, f, false, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
                assert_eq!(repeat.result, current.result);
                assert_eq!(repeat.paid, current.paid);
                assert_eq!(repeat.first, current.first);
                balanced(&repeat);
            }
        }
        assert_eq!(f.cleanup_plan, plan);
    }
}
#[test]
fn exact_short_zero_work_bytes_floor_and_sticky_refusal_keep_full_drop() {
    for source in [TINY, BRANCH, FULL] {
        let p = fixture(source);
        let f = probe(&p);
        for selected in [false, true] {
            for old in [false, true] {
                let full = run(&p, f, old, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
                assert!(full.result.is_ok());
                balanced(&full);
                assert!(full.work > 0 && full.paid > 0);
                for (cut, (work, bytes, floor, success)) in [
                    (full.work, full.paid, 0, true),
                    (full.work - 1, full.paid, 0, false),
                    (0, full.paid, 0, false),
                    (full.work, full.paid - 1, 0, false),
                    (full.work, 0, 0, false),
                    (full.work, full.paid, full.paid, false),
                ]
                .into_iter()
                .enumerate()
                {
                    // TINY retains every original cut in both payer modes.
                    // Branch/full add exact, byte-short and floor cuts around
                    // their complete successful invoice, without repeating
                    // the tiny caller's work/zero admission matrix.
                    if source != TINY && matches!(cut, 1 | 2 | 4) {
                        continue;
                    }
                    let r = run(&p, f, old, selected, work, bytes, floor);
                    balanced(&r);
                    assert_eq!(
                        r.result.is_ok() && !r.refused,
                        success,
                        "old={old} selected={selected} work={work} bytes={bytes} floor={floor}"
                    );
                    if let Err(d) = &r.result {
                        assert_eq!(d.code, "SPX-H006");
                    }
                    if let Some(first) = r.first {
                        assert_eq!(first.floor, floor);
                        assert!(first.remaining <= bytes);
                        assert!(first
                            .requested
                            .checked_add(first.floor)
                            .is_none_or(|required| required > first.remaining));
                    }
                }
            }
            if source != TINY {
                continue;
            }
            // Equal work frontiers compare the actual six-field Diagnostic,
            // including selected cold setup, rather than only the error code.
            let full = run(&p, f, false, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
            for work in [0, 1, full.work / 2, full.work - 1, full.work] {
                let old = run(&p, f, true, selected, work, usize::MAX, 0);
                let current = run(&p, f, false, selected, work, usize::MAX, 0);
                assert_eq!(old.result, current.result);
                assert_eq!(
                    (old.work, old.held, old.spent),
                    (current.work, current.held, current.spent)
                );
                balanced(&old);
                balanced(&current);
            }
        }
    }
}
#[test]
fn forged_lexical_targets_and_order_keep_the_first_six_field_diagnostic() {
    for selected in [false, true] {
        for attack in 0..6 {
            let p = fixture(FULL);
            let mut f = probe(&p).clone();
            let plan = &mut f.cleanup_plan;
            match attack {
                0 => plan.entry = BlockId(u32::MAX),
                1 => {
                    plan.lexical_control.as_mut().unwrap().loops[0].latch_entry =
                        Some(BlockId(u32::MAX))
                }
                2 => {
                    plan.lexical_control.as_mut().unwrap().exits[0].resume = Some(BlockId(u32::MAX))
                }
                3 => {
                    plan.lexical_control.as_mut().unwrap().exits[0].exit =
                        crate::cleanup_plan::ExitTargetId(u32::MAX)
                }
                4 => plan.blocks[0].id = BlockId(u32::MAX),
                _ => {
                    let edges = plan
                        .blocks
                        .iter()
                        .find_map(|b| match &b.terminator {
                            CleanupTerminator::Branch(e) => Some(e.clone()),
                            _ => None,
                        })
                        .unwrap();
                    plan.edges[edges[0].0 as usize].condition = EdgeCondition::Always;
                    plan.edges[edges[1].0 as usize].condition = EdgeCondition::Always;
                }
            }
            let old = run(&p, &f, true, selected, MAX_REPLAY_WORK_UNITS, usize::MAX, 0);
            let current = run(
                &p,
                &f,
                false,
                selected,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
            );
            assert!(old.result.is_err());
            assert_eq!(
                old.result, current.result,
                "selected={selected} attack={attack}"
            );
            balanced(&old);
            balanced(&current);
        }
    }
}
#[test]
fn immediate_old_entire_caller_and_unchanged_dependencies_are_source_bound() {
    use sha2::{Digest, Sha256};
    let whole = include_str!("strategy.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(whole.as_bytes()))
        ),
        "3e4e4144890bd2de291803364ea143898f709787fe1888a9fa6dc75b097f9e08"
    );
    let start = whole
        .find("pub(in crate::cleanup_plan::replay) fn validate_authenticated(")
        .unwrap();
    let end = whole[start..].find("\n/// Add this function").unwrap() + start;
    let independently_compiled=include_str!("before.rs").strip_prefix("//! Complete immediate-old authenticated lexical comparison caller.\nuse super::super::*;\n\n").unwrap();
    assert_eq!(independently_compiled, &whole[start..end]);
    for (source,hash) in [
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs")),"39412583a543bf0b2d2f3f38c11d882183d89abfef4b8ae650f827142a3d2470"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs")),"006a2efb76b4d30c6062d0af713a7d2b4f650c01b43a6e0a3b80c11c3b986a66"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared.rs")),"371265c7d8cc05edfe08f5a7f160ab4d1d4a68b50d323aa63bf9c7989b820729"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/lexical_replay.rs")),"c3bc36798a813d536e25570fed41c6210245be8db74d01ca5e4a631cb6747d98"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/lexical_replay/ordinary.rs")),"d947a67e20af65598c445eecf30c19a797793e2ea45d9ca1d096976fa1111927"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs")),"b6471558eaf00c76a99472843a9d96a58a11fe9e5779f50fdccbfc6f0011aa51"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs")),"2a64ea81fe25ac38ad7d3060cb1dae31de835e46c4572c5bf425ea979a477917"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs")),"3e627c55e743f541f5787c64c47af8e31690a002907db35a435f0ce57fc9c72a"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs")),"9d7181c923df3c53a31ebdd1571d3c1f9eac990f605345b06a708dc19d251e8c"),
  (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs")),"75270c55c77ae6ea0697f52348e7c6ad78c5dbbca706bdd4b8faabd3849d1e10"),
 ] {
        // Restore the two finite ordinary-frame source changes before the
        // original active-module inverse and every unchanged whole-source target.
        let transformed = if matches!(hash,
            "2a64ea81fe25ac38ad7d3060cb1dae31de835e46c4572c5bf425ea979a477917" |
            "3e627c55e743f541f5787c64c47af8e31690a002907db35a435f0ce57fc9c72a") {
            let encoded = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/before_frontier/custody/main_d227_transport.json"));
            assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cleanup_plan/replay/path_join/before_frontier/custody/main_d227_transport.json")).unwrap(), encoded.as_bytes());
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(encoded.as_bytes()))), "046a313e58da6a509699409d479b83147b511c5c6693d28d7c0b93c537350c5c");
            let packet: serde_json::Value = serde_json::from_str(encoded).unwrap();
            assert_eq!(packet.as_object().unwrap().len(), 41);
            let path = if hash == "2a64ea81fe25ac38ad7d3060cb1dae31de835e46c4572c5bf425ea979a477917" { "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop.rs" } else { "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact.rs" };
            assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap(), source.as_bytes());
            let row = &packet[path];
            assert_eq!(source.len(), row["length"].as_u64().unwrap() as usize);
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))), row["sha256"].as_str().unwrap());
            let mut restored = source.to_owned();
            for edit in row["edits"].as_array().unwrap().iter().rev() {
                let start = edit["start"].as_u64().unwrap() as usize;
                let end = edit["end"].as_u64().unwrap() as usize;
                assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
                restored.replace_range(start..end, edit["before"].as_str().unwrap());
            }
            assert_eq!(restored.len(), row["before_length"].as_u64().unwrap() as usize);
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), row["before_sha256"].as_str().unwrap());
            std::borrow::Cow::Owned(restored)
        } else { std::borrow::Cow::Borrowed(source) };
        let transformed = if hash == "b6471558eaf00c76a99472843a9d96a58a11fe9e5779f50fdccbfc6f0011aa51" {
            let current = transformed.as_ref();
            assert_eq!(std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs")).unwrap(), current.as_bytes());
            assert_eq!(current.len(), 55878);
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), "9ae1b6828a1fab4818043a53e3f73062dc1484ac851b2c06fec50b300c8ff732");
            let addition = "\npub(in crate::cleanup_plan::replay::factored) mod hir_last_use;\n";
            assert_eq!(current.matches(addition).count(), 1);
            let restored = current.strip_suffix(addition).unwrap().to_owned();
            assert_eq!(restored.len(), 55813);
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514");
            std::borrow::Cow::Owned(restored)
        } else { transformed };
        let source = transformed.as_ref();
        let original = if hash == "3e627c55e743f541f5787c64c47af8e31690a002907db35a435f0ce57fc9c72a" {
            // The complete existing compact entry is unchanged. Its later
            // active-builder sibling is independently source-bound below.
            assert_eq!(source.matches("mod active;\n").count(), 1);
            std::borrow::Cow::Owned(source.replacen("mod active;\n", "", 1))
        } else if hash == "b6471558eaf00c76a99472843a9d96a58a11fe9e5779f50fdccbfc6f0011aa51" {
            // Exact two-arm dispatch/source registration transport; fixed whole predecessor is unchanged.
            assert_eq!(source.len(), 55813);
            assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))), "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514");
    let mut restored = source
        .strip_suffix("\n#[cfg(test)]\nmod ordinary_labels;\n")
        .unwrap()
        .to_owned();
    assert_eq!(
        restored
            .matches("AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved => {")
            .count(),
        2
    );
    restored = restored.replace(
        "AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved => {",
        "AllocationPayer::ActiveBuilder => {",
    );
    for indent in ["                ", "            "] {
        let marker =
            format!("{indent}AllocationPayer::SelectedReservation => {{}}\n{indent}_ => {{");
        assert_eq!(restored.matches(&marker).count(), 1);
        restored = restored.replacen(&marker, &format!("{indent}AllocationPayer::SelectedReservation => {{}}\n{indent}AllocationPayer::LegacyUnobserved => {{}}\n{indent}_ => {{"), 1);
    }
    assert_eq!(restored.as_bytes(), include_bytes!("../../canonical_shared/inline_singleton/ordinary_labels.before.txt"));
    std::borrow::Cow::Owned(restored)
        } else {
            std::borrow::Cow::Borrowed(source)
        };
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))),hash);
    }
    assert_eq!(
        format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(include_bytes!(
            "../../canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs"
        )))),
        "991492eeee4ed2c8783ed2bf0bffddd661f76633738074ad78b4301b032ab75e"
    );
}
