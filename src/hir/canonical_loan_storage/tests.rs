//! Storage-only payment controls and actual current constructor parity.
//! Independently rebuilt plan scratch/diagnostic text are not covered by the
//! storage-only observer. Whole compiler gates retain their original inequality.
use super::*;
use crate::hir::validation::{constructor_seed::ConstructorSeed, owner_renewal};
use crate::hir::{hir_error, HirValidator};
use crate::loan_plan::{Loan, LoanEndpoint, LoanPlan, LoanProgramPoint};
use std::collections::BTreeMap;
mod old;
mod reuse;
// The existing literal from_plans controls below retain the complete saved
// allocation receiver. reuse tests exercise the actual production append path.

pub(super) fn fixture() -> ResolvedProgram {
    let source = crate::check(
        r#"module loan.storage;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes, }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling); byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#,
        "canonical-loan-storage.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn same(actual: &Storage<'_>, expected: &old::Inventory) {
    assert_eq!(actual.ids.len(), expected.ids.len());
    for (key, id) in &expected.ids {
        assert_eq!(actual.id(key), Some(id));
    }
    assert_eq!(actual.liveness.len(), expected.liveness.len());
    for (key, index) in &expected.liveness {
        assert_eq!(actual.live(key), Some(index));
        assert_eq!(actual.origin(*index), expected.origins.get(*index));
    }
    assert_eq!(actual.origins, expected.origins);
}
fn message(error: Diagnostic) -> (&'static str, String) {
    (error.code, error.message.into_owned())
}

#[test]
fn funded_current_storage_matches_saved_complete_inventory_and_rejects_transplants() {
    let program = fixture();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    let seed = ConstructorSeed::authenticate(&program, Some(&directory)).unwrap();
    assert!(!program.function_instances.is_empty());
    let expected = old::Inventory::rebuild(&program).unwrap();
    let storage = caller::prepare(&seed).unwrap();
    same(&storage, &expected);
    assert_eq!(storage.origins.len(), 9);
    let mut offset = 0;
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let plan = crate::loan_plan::build_plan(&program, function).unwrap();
        for (local, loan) in plan.loans.iter().enumerate() {
            assert_eq!(storage.origin(offset + local), Some(&loan.origin));
        }
        offset += plan.loans.len();
    }
    let current = caller::validate(&program, None, Some(&directory));
    let legacy = owner_renewal::validate_with_directory(&program, None, Some(&directory));
    assert_eq!(current, legacy);
    current.unwrap();
    legacy.unwrap();

    // These actual anonymous bodies take the shared-storage closure branch.
    // The second parameter shadows the first closure's captured outer name;
    // accepting both requires fresh body identity/value/scope state.
    let ast = crate::check(
        r#"module loan.closure_storage;
@id("snapshot") fn snapshot()->i64 {
 let mut captured=7;
 let callback=fn(value:i64)->i64 {captured+value};
 captured=100;
 callback(5)
}
@id("shadow") fn shadow(captured:i64)->i64 {
 let callback=fn(captured:i64)->i64 {captured+1};
 callback(41)
}
@id("main") fn main()->i64 {snapshot()+shadow(99)}
"#,
        "canonical-loan-closure-storage.spx",
    )
    .unwrap();
    let closures = crate::hir::resolve(&ast).unwrap();
    let inventory = crate::hir::closure::inventory(&closures);
    assert_eq!(inventory.len(), 2);
    let mut captures = inventory
        .iter()
        .map(|expression| {
            let crate::hir::ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
                panic!("actual closure inventory");
            };
            captures.len()
        })
        .collect::<Vec<_>>();
    captures.sort_unstable();
    assert_eq!(captures, [0, 1]);
    let closure_directory = crate::hir::function_directory::caller::prepare(&closures).unwrap();
    let closure_seed = ConstructorSeed::authenticate(&closures, Some(&closure_directory)).unwrap();
    let closure_storage = caller::prepare(&closure_seed).unwrap();
    same(
        &closure_storage,
        &old::Inventory::rebuild(&closures).unwrap(),
    );
    let current = caller::validate(&closures, None, Some(&closure_directory));
    let legacy = owner_renewal::validate_with_directory(&closures, None, Some(&closure_directory));
    assert_eq!(current, legacy);
    current.unwrap();
    legacy.unwrap();
    for reorder in [false, true] {
        let mut clone = program.clone();
        if reorder {
            clone.functions.reverse();
        }
        let foreign_directory = crate::hir::function_directory::caller::prepare(&clone).unwrap();
        let foreign_seed = ConstructorSeed::authenticate(&clone, Some(&foreign_directory)).unwrap();
        let error = HirValidator::with_storage(foreign_seed, &storage)
            .err()
            .unwrap();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            error.message,
            "canonical shared-loan storage does not belong to the current program"
        );
    }
    let mut drift = program.clone();
    drift
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "first")
        .unwrap()
        .loan_plan
        .loans[0]
        .origin
        .projections[0] =
        crate::hir::PlaceProjection::Field(crate::hir::DeclarationId::new("sibling"));
    assert_eq!(crate::hir::validate(&drift).unwrap_err().code, "SPX-H006");
    // A later loan-builder error cannot replace the existing constructor NUL
    // error. Seed still performs the complete original preflight first.
    let mut hostile = program.clone();
    hostile.entrypoint = crate::hir::DeclarationId::new("bad\0entry");
    hostile.functions[0].body.id = ExpressionId::from_owned("bad\0body".into());
    assert_eq!(
        message(caller::validate(&hostile, None, None).unwrap_err()),
        message(owner_renewal::validate_with_directory(&hostile, None, None).unwrap_err())
    );
}

fn plan(prefix: &str) -> LoanPlan {
    let point = |name: &str, phase| LoanProgramPoint {
        expression: ExpressionId::from_owned(format!("{prefix}.{name}")),
        phase,
    };
    let loans = (0..2)
        .map(|i| Loan {
            id: LoanId(i),
            site: point(&format!("site{i}"), LoanPointPhase::Before).expression,
            origin: crate::hir::Place {
                root: crate::hir::ValueId::new(format!("{prefix}.owner{i}")),
                projections: vec![crate::hir::PlaceProjection::Field(
                    crate::hir::DeclarationId::new("field"),
                )],
            },
            parent: None,
            start: point(&format!("site{i}"), LoanPointPhase::Before),
            ends: Vec::new(),
            end_edges: Vec::new(),
            cause: LoanCause::SliceView,
        })
        .collect();
    LoanPlan {
        schema: "test-storage-only",
        loans,
        endpoints: vec![
            LoanEndpoint {
                point: point("before", LoanPointPhase::Before),
                live_before: vec![LoanId(0), LoanId(1)].into_boxed_slice(),
                starts: Box::new([]),
                kills: Box::new([]),
                live_after: vec![LoanId(99)].into_boxed_slice(),
            },
            LoanEndpoint {
                point: point("after", LoanPointPhase::After),
                live_before: vec![LoanId(99)].into_boxed_slice(),
                starts: Box::new([]),
                kills: Box::new([]),
                live_after: vec![LoanId(0)].into_boxed_slice(),
            },
        ],
        edges: Vec::new(),
    }
}

#[test]
fn plan_batch_priority_last_local_id_wins_and_moved_origins_match_saved_receiver() {
    let program = fixture();
    let plans = vec![plan("first"), plan("second")];
    let pointers = plans
        .iter()
        .flat_map(|plan| {
            plan.loans
                .iter()
                .map(|loan| loan.origin.projections.as_ptr())
        })
        .collect::<Vec<_>>();
    let expected = old::from_plans(plans.clone()).unwrap();
    let actual = caller::from_plans(&program, plans).unwrap();
    same(&actual, &expected);
    for (origin, pointer) in actual.origins.iter().zip(pointers) {
        assert_eq!(origin.projections.as_ptr(), pointer);
    }
    let empty = crate::loan_plan::LoanPlan {
        schema: "test-storage-only",
        loans: Vec::new(),
        endpoints: Vec::new(),
        edges: Vec::new(),
    };
    same(
        &caller::from_plans(&program, vec![empty.clone()]).unwrap(),
        &old::from_plans(vec![empty]).unwrap(),
    );
    let mut last = plan("last");
    last.loans[1].id = LoanId(0);
    last.endpoints[0].live_before = vec![LoanId(0)].into_boxed_slice();
    let old = old::from_plans(vec![last.clone()]).unwrap();
    let current = caller::from_plans(&program, vec![last]).unwrap();
    same(&current, &old);
    assert!(current.liveness.iter().all(|(_, ordinal)| *ordinal == 1));
    for attack in 0..4 {
        let mut first = plan("attack");
        let mut second = plan("later");
        match attack {
            0 => {
                first.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
                first.loans[0].start.phase = LoanPointPhase::After;
            }
            1 => {
                first.endpoints[0].live_before =
                    vec![LoanId(0), LoanId(0), LoanId(99)].into_boxed_slice();
            }
            2 => {
                first.loans[0].start.phase = LoanPointPhase::After;
                second.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
            }
            3 => {
                second.loans[0].site = first.loans[0].site.clone();
            }
            _ => unreachable!(),
        }
        let plans = vec![first, second];
        let old = old::from_plans(plans.clone()).err().unwrap();
        let current = caller::from_plans(&program, plans).err().unwrap();
        assert_eq!(message(current), message(old));
    }
}

#[test]
fn actual_storage_payer_literal_capacities_exact_short_and_drop_keep_first_receipt() {
    use crate::test_allocation_observer::measure;
    use std::mem::size_of;
    let program = fixture();
    // Two plans: local2/probeIDs2/probeLive3 each; global buffers grow
    // IDs2→4, endpoints3→6, origins2→4. These are literal requested layouts,
    // not observed calibration or attached-plan sizing authority.
    let bytes = 4 * size_of::<(LoanId, usize)>()
        + 4 * size_of::<(IdKey, LoanId, usize)>()
        + 6 * size_of::<(LiveKey, Option<usize>, usize)>()
        + 6 * size_of::<(IdKey, LoanId)>()
        + 9 * size_of::<(LiveKey, usize)>()
        + 6 * size_of::<Place>();
    for allowance in [bytes, bytes - 1, 0] {
        let plans = vec![plan("first"), plan("second")];
        // Explicitly excluded held subject, input plan/payload and fixed Rc
        // ledger setup. Observe EVERY new probe/global buffer through Drop.
        let (((), observed), overflowed, used, receipt) =
            crate::bounded_output::with_limit_usage_refusal(allowance, || {
                measure(|| {
                    let result = caller::from_plans(&program, plans);
                    assert_eq!(result.is_ok(), allowance == bytes);
                    if let Ok(storage) = result {
                        assert_eq!(storage.origins.len(), 4);
                        drop(storage);
                    }
                    if allowance < bytes {
                        assert!(!crate::bounded_output::reserve_active_required(0));
                    }
                })
            });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= used);
        assert_eq!(overflowed, allowance < bytes);
        if allowance == bytes {
            assert_eq!(used, bytes);
            assert_eq!(observed.allocated, bytes);
            assert!(receipt.is_none());
        } else {
            assert!(receipt.is_some());
        }
    }
    let (_, overflowed, used, receipt) = crate::bounded_output::with_limit_usage_refusal(7, || {
        assert!(crate::bounded_output::reserve_active_required(2));
        let error = caller::capacity_failure(5, 1);
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.is_empty());
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(overflowed);
    assert_eq!(used, 3);
    assert_eq!(receipt.unwrap().requested, 5);
}

#[test]
fn actual_canonical_caller_funds_rebuild_append_and_drop_on_one_invocation() {
    use crate::test_allocation_observer::measure;
    let program = fixture();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    let seed = ConstructorSeed::authenticate(&program, Some(&directory)).unwrap();
    let expected = old::Inventory::rebuild(&program).unwrap();
    assert_eq!(expected.origins.len(), 9);
    assert!(!program.function_instances.is_empty());
    // Source checking, ModuleIndex/constructor authentication, and the fixed
    // existing Budget context are outside this planner+append observation.
    // No claim about the later full validator allocations is made here.
    let run = |limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                match caller::prepare(&seed) {
                    Ok(storage) => {
                        same(&storage, &expected);
                        assert!(storage.is_current(&program));
                        drop(storage);
                        true
                    }
                    Err(report) => {
                        assert_eq!(report.code, "SPX-H006");
                        assert_eq!(
                            report.message,
                            "canonical loan CFG storage reservation refused"
                        );
                        drop(report);
                        // The same invocation cannot acquire a fresh report/allowance
                        // after failure. This inline setup refusal allocates no text.
                        let retry = caller::prepare(&seed).err().unwrap();
                        assert_eq!(retry.code, "SPX-H006");
                        assert!(retry.message.is_empty());
                        drop(retry);
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        false
                    }
                }
            })
        })
    };
    let ((success, observed), refused, exact, first) = run(usize::MAX);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert!(observed.allocations > 8);
    assert!(observed.allocated <= exact);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, repeated), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, exact);
    assert_eq!(first, None);
    assert_eq!(repeated.allocated, observed.allocated);
    assert_eq!(repeated.allocations, observed.allocations);
    assert_eq!(repeated.freed, repeated.allocated);
    assert_eq!(repeated.live, 0);
    assert!(!repeated.invalid);
    let ((success, short), refused, _, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
    // A different invocation gets its own existing ledger; the prior failure
    // neither grants storage nor poisons an unrelated fresh caller.
    let ((success, isolated), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, exact);
    assert_eq!(first, None);
    assert_eq!(isolated.freed, isolated.allocated);
    assert_eq!(isolated.live, 0);
    assert!(!isolated.invalid);
}

#[path = "../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod source_reader;

// Bind the physical successor before passing exact predecessor bytes to the
// existing independent empty-terminal and earlier source-stage controls.
pub(super) fn restore_owned_receiver_sources(path: &str, compiled: &str) -> String {
    let (length, digest, edits): (usize, &str, &[(usize, &str, &str)]) = match path {
        "src/hir/canonical_loan_storage/caller.rs" => (29679, "566fcb1a91a576f5422e0d1278ba84de0eb77c2af57785d3ceeacf49b8613751", &[
            (10511, "                        + stream::BRANCH_SELECTION_BYTES\n                        // Retain all old caller-owned\n                        // roles. Reserve for the local and transported receiver loan\n                        // and the two projected append reborrows separately.\n                        + 4 * std::mem::size_of::<&mut stream::Receiver<'_, '_, '_>>()\n                        + 2 * std::mem::size_of::<(&mut Storage<'_>, &mut Deferred<'_>)>(),\n", "                        + stream::BRANCH_SELECTION_BYTES,\n"),
            (10986, "                let mut receiver = stream::Receiver {\n", "                let receiver = stream::Receiver {\n"),
            (11204, "                let held_receiver = &mut receiver;\n", ""),
            (11398, "                    move |source, matched| held_receiver.receive(source, matched),\n", "                    move |source, matched| receiver.receive(source, matched),\n"),
        ]),
        "src/hir/canonical_loan_storage/caller/stream.rs" => (17186, "6871bba711362f0d36b9a252b972ad3aead38faba80948210f4596a1bf4a069f", &[
            (1814, "        &mut self,\n", "        self,\n"),
        ]),
        _ => unreachable!("only canonical-loan receiver owners have an inverse"),
    };
    let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    assert_eq!(
        source_reader::read(&physical, length, digest, 2 * 1024 * 1024).unwrap(),
        compiled,
    );
    let mut restored = compiled.to_owned();
    for &(offset, after, prior) in edits.iter().rev() {
        assert_eq!(restored.get(offset..offset + after.len()), Some(after));
        restored.replace_range(offset..offset + after.len(), prior);
    }
    restored
}

pub(super) fn borrowed_gateway_saving<Receiver>() -> usize {
    use std::mem::{align_of, size_of};

    // All other EmitArgs members have at most pointer alignment. The two
    // EmitArgs roles and eight callback roles move the same smaller payload.
    assert_eq!(align_of::<Receiver>(), align_of::<&mut Receiver>());
    let loan_width = size_of::<&mut Receiver>();
    10 * (size_of::<Receiver>() - loan_width)
        - 4 * loan_width
        - 2 * size_of::<(&mut Storage<'static>, &mut Deferred<'static>)>()
}
