//! Complete receiving controls. All are authored, not qualification evidence.
use super::*;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};

fn fixture() -> ResolvedProgram {
    crate::hir::canonical_loan_storage::tests::fixture()
}
fn selected<T>(old: bool, operation: impl FnOnce() -> T) -> T {
    if old {
        with_before(operation)
    } else {
        operation()
    }
}
fn selected_stage<T>(stage: u8, operation: impl FnOnce() -> T) -> T {
    match stage {
        0 => with_before(operation),
        1 => with_owned_before(operation),
        2 => operation(),
        _ => unreachable!("three explicit compiler-owned test stages"),
    }
}
fn assert_storage(actual: &Storage<'_>, expected: &Storage<'_>) {
    assert_eq!(actual.ids, expected.ids);
    assert_eq!(actual.liveness, expected.liveness);
    assert_eq!(actual.origins, expected.origins);
    assert!(actual.is_current(expected.program));
}
fn clean(metrics: Metrics, paid: usize) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.highwater <= paid && metrics.allocated <= paid);
}

#[test]
fn complete_direct_rows_keep_every_attached_field_and_the_full_late_error_frontier() {
    let program = fixture();
    let first = program
        .functions
        .iter()
        .position(|f| !f.loan_plan.loans.is_empty())
        .unwrap();
    for mutation in 0..26 {
        let mut forged = program.clone();
        let plan = &mut forged.functions[first].loan_plan;
        match mutation {
            0 => (),
            1 => plan.schema = "forged",
            2 => {
                plan.loans.pop();
            }
            3 => plan.loans[0].id = LoanId(255),
            4 => plan.loans[0].site = ExpressionId::from_owned("forged.site".into()),
            5 => plan.loans[0].origin.root = crate::hir::ValueId::new("forged.root".into()),
            6 => plan.loans[0].parent = Some(LoanId(255)),
            7 => plan.loans[0].start.phase = LoanPointPhase::After,
            8 => plan.loans[0].ends.clear(),
            9 => plan.loans[0].end_edges.clear(),
            10 => plan.loans[0].cause = crate::loan_plan::LoanCause::StrView,
            11 => {
                plan.endpoints.pop();
            }
            12 => plan.endpoints[0].point.phase = LoanPointPhase::After,
            13 => plan.endpoints[0].live_before = Box::new([LoanId(255)]),
            14 => plan.endpoints[0].starts = Box::new([LoanId(255)]),
            15 => plan.endpoints[0].kills = Box::new([LoanId(255)]),
            16 => plan.endpoints[0].live_after = Box::new([LoanId(255)]),
            17 => {
                plan.edges.pop();
            }
            18 => plan.edges[0].from = u16::MAX,
            19 => plan.edges[0].to = u16::MAX,
            20 => plan.edges[0].live = Box::new([LoanId(255)]),
            21 => plan.loans.push(plan.loans[0].clone()),
            22 => plan.endpoints.push(plan.endpoints[0].clone()),
            23 => plan.edges.push(plan.edges[0].clone()),
            24 => plan.endpoints.reverse(),
            25 => plan.edges.reverse(),
            _ => unreachable!(),
        }
        let seed = ConstructorSeed::authenticate(&forged, None).unwrap();
        let expected = selected(true, || prepare_with_comparison(&seed, &forged)).unwrap();
        let expected_result = expected.1.finish(&forged);
        let (((), metrics), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let (storage, deferred) = prepare_with_comparison(&seed, &forged).unwrap();
                assert_storage(&storage, &expected.0);
                let result = deferred.finish(&forged);
                drop(storage);
                assert_eq!(result, expected_result);
                drop(result);
            })
        });
        assert!(!refused);
        clean(metrics, paid);
        // Full validator, not just the new receipt, preserves priority of any
        // earlier semantic/cleanup/entrypoint error over forged evidence.
        let current = with_limit_usage_refusal(usize::MAX, || crate::hir::validate(&forged));
        let before = with_limit_usage_refusal(usize::MAX, || {
            selected(true, || crate::hir::validate(&forged))
        });
        assert_eq!(current.0, before.0);
    }
    for early in [true, false] {
        let mut forged = program.clone();
        forged.functions[first].loan_plan.schema = "forged";
        if early {
            forged.entrypoint = crate::hir::DeclarationId::new("missing.entry");
        } else {
            forged.functions[first].cleanup_plan.schema = "forged.cleanup";
        }
        let current = with_limit_usage_refusal(usize::MAX, || crate::hir::validate(&forged)).0;
        let before = with_limit_usage_refusal(usize::MAX, || {
            selected(true, || crate::hir::validate(&forged))
        })
        .0;
        assert_eq!(current, before);
        assert!(!current.unwrap_err().message.contains("forged shared-loan"));
    }
}

#[test]
fn current_raw_and_shared_receiving_preserve_exact_short_zero_floor_sticky_and_full_drop() {
    let program = fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    let final_function = program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .last()
        .unwrap();
    let final_plan = crate::loan_plan::build_plan(&program, final_function).unwrap();
    assert_eq!(final_plan.schema, crate::loan_plan::LOAN_PLAN_SCHEMA_V1);
    assert!(
        final_plan.loans.is_empty()
            && final_plan.endpoints.is_empty()
            && final_plan.edges.is_empty()
    );
    drop(final_plan);
    let seal_request = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Storage<'_>>())
        .unwrap()
        .0
        .pad_to_align()
        .size();
    let (expected, deferred) = selected(true, || prepare_with_comparison(&seed, &program)).unwrap();
    deferred.finish(&program).unwrap();
    for stage in [0, 1, 2] {
        for shared in [true, false] {
            let run = |limit, floor| {
                with_limit_usage_refusal(limit, || {
                    assert!(bounded_output::set_active_floor(floor));
                    measure(|| {
                        selected_stage(stage, || {
                            let result = if shared {
                                prepare_shared(&seed).map(|s| {
                                    assert_storage(&s, &expected);
                                    drop(s);
                                })
                            } else {
                                prepare_with_comparison(&seed, &program).map(|(s, d)| {
                                    assert_storage(&s, &expected);
                                    d.finish(&program).unwrap();
                                    drop(s);
                                })
                            };
                            match result {
                                Ok(()) => true,
                                Err(error) => {
                                    assert_eq!(error.code, "SPX-H006");
                                    assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                                    assert_eq!(
                                        error.message,
                                        if limit == 0 {
                                            ""
                                        } else {
                                            "canonical loan CFG storage reservation refused"
                                        }
                                    );
                                    assert!(
                                        error.path.is_none()
                                            && error.span.is_none()
                                            && error.help.is_none()
                                    );
                                    drop(error);
                                    let before = crate::test_allocation_observer::current_metrics();
                                    let retry =
                                        prepare_with_comparison(&seed, &program).err().unwrap();
                                    assert_eq!(retry.code, "SPX-H006");
                                    assert_eq!(retry.severity, crate::diagnostic::Severity::Error);
                                    assert!(
                                        retry.message.is_empty()
                                            && retry.path.is_none()
                                            && retry.span.is_none()
                                            && retry.help.is_none()
                                    );
                                    drop(retry);
                                    assert!(!bounded_output::reserve_active_required(0));
                                    assert_eq!(
                                        crate::test_allocation_observer::current_metrics(),
                                        before
                                    );
                                    false
                                }
                            }
                        })
                    })
                })
            };
            let ((ok, metrics), refused, exact, first) = run(usize::MAX, 0);
            assert!(ok && !refused && first.is_none());
            clean(metrics, exact);
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
                if let Some(receipt) = first {
                    assert_eq!(receipt.floor, floor);
                    if limit == 0 || shared {
                        assert_eq!(
                            receipt.stage,
                            Some(bounded_output::ReservationStage::CanonicalLoanStorage)
                        );
                        if limit != 0 {
                            assert_eq!(receipt.requested, seal_request);
                            assert_eq!(receipt.remaining, floor + seal_request - 1);
                        }
                    } else {
                        assert_eq!(
                            receipt.stage,
                            Some(bounded_output::ReservationStage::LoanCandidateInventory)
                        );
                    }
                }
                if success {
                    assert_eq!(paid, exact);
                    assert_eq!(observed, metrics);
                }
                clean(observed, paid);
                let repeated = run(limit, floor);
                assert_eq!(repeated.0, (ok, observed));
                assert_eq!(repeated.1, refused);
                assert_eq!(repeated.2, paid);
                assert_eq!(repeated.3, first);
            }
        }
    }
}

#[test]
fn complete_direct_caller_has_honest_large_net_and_keeps_its_cold_loss_visible() {
    let mut source = String::from("module direct.net;\n@id(\"packet\") record Packet { @id(\"data\") data:Bytes, }\n@id(\"large\") fn large(packet:own Packet)->usize {let view=bytes_as_slice(packet.data); let mut count=0usize; ");
    for _ in 0..192 {
        source.push_str("count=count+1usize; ");
    }
    source.push_str("count+byte_len(view)}\n@id(\"main\") fn main()->i64 {0}\n");
    let large = crate::hir::resolve(&crate::check(&source, "direct-net.spx").unwrap()).unwrap();
    let tiny = crate::hir::resolve(
        &crate::check(
            "module direct.tiny; @id(\"main\") fn main()->i64 {0}",
            "direct-tiny.spx",
        )
        .unwrap(),
    )
    .unwrap();
    // Preserve the original many-borrowcall shape and both strict conditions;
    // the one-view shape above is an additional case, not a replacement.
    let mut many_source = String::from("module direct.net;\n@id(\"packet\") record Packet { @id(\"data\") data:Bytes, }\n@id(\"large\") fn large(packet:own Packet)->usize {let view=bytes_as_slice(packet.data); ");
    // Preserve all 192 calls and 191 left-to-right additions without a
    // left-associated expression deeper than the ordinary parser limit.
    many_source.push_str("let mut count=byte_len(view); ");
    for _ in 1..192 {
        many_source.push_str("count=count+byte_len(view); ");
    }
    many_source.push_str("count}\n@id(\"main\") fn main()->i64 {0}\n");
    let many = crate::hir::resolve(&crate::check(&many_source, "direct-net.spx").unwrap()).unwrap();
    assert_eq!(
        many.functions.iter().map(|f| f.loan_plan.loans.len()).max(),
        Some(193)
    );
    for (shape, program, require_gain) in [
        ("193-loan", &many, true),
        ("one-view-assignments", &large, true),
        ("cold-empty", &tiny, false),
    ] {
        let seed = ConstructorSeed::authenticate(program, None).unwrap();
        let mut results = Vec::new();
        for stage in [0, 2, 1] {
            let (((), metrics), refused, paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        selected_stage(stage, || {
                            let (storage, deferred) =
                                prepare_with_comparison(&seed, program).unwrap();
                            deferred.finish(program).unwrap();
                            drop(storage);
                        })
                    })
                });
            assert!(!refused && first.is_none());
            clean(metrics, paid);
            results.push((metrics, paid));
        }
        eprintln!("direct canonical complete caller: shape={shape} large={require_gain} old allocated={} paid={} highwater={}; new allocated={} paid={} highwater={}", results[0].0.allocated,results[0].1,results[0].0.highwater,results[1].0.allocated,results[1].1,results[1].0.highwater);
        eprintln!(
            "immediate owned V2 complete caller: shape={shape} allocated={} paid={} highwater={}",
            results[2].0.allocated, results[2].1, results[2].0.highwater
        );
        if require_gain {
            assert!(results[1].0.allocated < results[0].0.allocated);
            assert!(
                results[1].1 < results[0].1,
                "complete new banks and growth must fit, not just output allocation"
            );
            assert!(
                results[1].0.allocated < results[2].0.allocated,
                "stream must independently improve owned V2 backing"
            );
            assert!(
                results[1].1 < results[2].1,
                "stream must independently earn its complete owned V2 invoice"
            );
        } else {
            assert!(
                results[1].1 > results[0].1,
                "new paid cold gateway is not free"
            );
        }
    }
}

#[test]
fn direct_source_requires_actual_member_and_exact_live_affine_owner() {
    let program = fixture();
    let foreign = program.functions[0].clone();
    let (_, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let before = bounded_output::active_remaining();
        let error = crate::loan_plan::build_canonical(&program, &foreign, &owner, |_, _| {
            panic!("foreign callback")
        })
        .err()
        .unwrap();
        assert!(matches!(error, crate::loan_plan::FundedError::Semantic(_)));
        assert_eq!(bounded_output::active_remaining(), before);
    });
    assert!(!refused && first.is_none());
    let function = program
        .functions
        .iter()
        .find(|f| !f.loan_plan.loans.is_empty())
        .unwrap();
    let (_, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let result = crate::loan_plan::build_canonical(&program, function, &owner, |source, _| {
            let source = source.unwrap();
            assert!(source.is_current(&program, function, &owner));
            let (_, foreign_refused, foreign_used, _) =
                with_limit_usage_refusal(usize::MAX, || {
                    assert!(!source.is_current(&program, function, &owner));
                });
            assert!(!foreign_refused);
            assert_eq!(foreign_used, 0);
            assert!(!source.is_current(&program, function, &owner));
            Ok(())
        });
        assert!(matches!(
            result,
            Err(crate::loan_plan::FundedError::Physical(
                crate::bounded_output::cfg_reservation::Refused::Detached
            ))
        ));
    });
    assert!(refused && first.is_some());
}

#[test]
fn current_complete_driver_keeps_old_checked_work_and_later_duplicate_identity_priority() {
    let program = fixture();
    let seed = ConstructorSeed::authenticate(&program, None).unwrap();
    for limit in 0..=64 {
        let run = |old| {
            with_limit_usage_refusal(usize::MAX, || {
                selected(old, || {
                    crate::loan_plan::canonical_sink::with_work_limit(limit, || {
                        prepare_with_comparison(&seed, &program).map(|(storage, deferred)| {
                            deferred.finish(&program).unwrap();
                            drop(storage);
                        })
                    })
                })
            })
            .0
        };
        assert_eq!(run(false), run(true));
    }
    let mut duplicate = program.clone();
    let mut owning = duplicate
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.loan_plan.loans.is_empty())
        .map(|(i, _)| i);
    let first = owning.next().unwrap();
    let second = owning.next().unwrap();
    duplicate.functions[first].loan_plan.schema = "forged";
    let crate::hir::ResolvedExprKind::Block { statements, .. } =
        &duplicate.functions[second].body.kind
    else {
        panic!("block")
    };
    let crate::hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("view binding")
    };
    duplicate.functions[second].body.id = value.id.clone();
    let current = with_limit_usage_refusal(usize::MAX, || crate::hir::validate(&duplicate)).0;
    let before = with_limit_usage_refusal(usize::MAX, || {
        selected(true, || crate::hir::validate(&duplicate))
    })
    .0;
    assert_eq!(current, before);
    assert_eq!(
        current.unwrap_err().message,
        "CFG contains a duplicate expression identity"
    );
}

#[test]
fn independent_selected_capacity_covers_duplicate_branches_starts_kills_and_zero_edges() {
    use crate::loan_plan::{canonical_sink::Parts, LoanProgramPoint};
    let points = [
        LoanProgramPoint {
            expression: ExpressionId::from_owned("branch".into()),
            phase: LoanPointPhase::After,
        },
        LoanProgramPoint {
            expression: ExpressionId::from_owned("left".into()),
            phase: LoanPointPhase::Before,
        },
        LoanProgramPoint {
            expression: ExpressionId::from_owned("right".into()),
            phase: LoanPointPhase::Before,
        },
    ];
    let edges = [(0, 1), (0, 2)];
    let live = [vec![LoanId(0)], vec![LoanId(0)]];
    let starts = [vec![LoanId(7)], vec![], vec![]];
    let kills = [vec![], vec![LoanId(9)], vec![]];
    let parts = Parts {
        points: &points,
        edges: &edges,
        live: &live,
        starts: &starts,
        kills: &kills,
        successors: &[0, 2, 2, 2],
        predecessor_offsets: &[0, 0, 1, 2],
        predecessors: &[0, 0],
    };
    let (_, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let upper = selected_capacity(&parts, Payer::Captured(&owner)).unwrap();
        let selected_after = canonical_sink::merge(live.iter().map(Vec::as_slice), &owner).unwrap();
        let actual = selected_after.len() + live[0].len() + live[1].len();
        assert_eq!(actual, 3);
        assert_eq!(upper, 4);
        assert!(
            upper > actual,
            "duplicate branches are charged, not claimed exact"
        );
        let empty = Parts {
            points: &points,
            edges: &[],
            live: &[],
            starts: &starts,
            kills: &kills,
            successors: &[0, 0, 0, 0],
            predecessor_offsets: &[0, 0, 0, 0],
            predecessors: &[],
        };
        assert_eq!(
            selected_capacity(&empty, Payer::Captured(&owner)).unwrap(),
            0
        );
    });
    assert!(!refused && first.is_none());
}

// One existing bounded reader; its complete source is authenticated below too.
#[path = "../../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod source_reader;

#[test]
fn entire_immediate_sources_and_independent_old_receivers_are_authenticated() {
    use sha2::{Digest, Sha256};
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let raw = include_str!("../../../../loan_plan/canonical_sink/immediate-before.json");
    assert!(raw.len() <= 2 * 1024 * 1024);
    assert_eq!(
        digest(raw.as_bytes()),
        "0f9b47d44e062f48ea99385888151db83cae1b6a4ac00a4ed959b76fd3105b45"
    );
    let snapshot: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(snapshot["base"], "8086b7e5047e7199c8fccb2f07c49c3513982d2a");
    // Authenticate the entire actual document before selecting its fixed
    // old source-stage image; the unchanged snapshot/old hashes remain below.
    let architecture_current = include_str!("../../../../../docs/ARCHITECTURE.md");
    assert_eq!(architecture_current.len(), 395407);
    assert_eq!(digest(architecture_current.as_bytes()), "7404359862f60c6adda119c1e32fbff955d9568105b6046ba5c712354a246317");
    let architecture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/ARCHITECTURE.md");
    assert_eq!(source_reader::read(&architecture_path, 395407, "7404359862f60c6adda119c1e32fbff955d9568105b6046ba5c712354a246317", 2 * 1024 * 1024).unwrap(), architecture_current);
    let architecture_before = include_str!("../../../../../e/20fdbcf1ec5fefc07b95342fea78b1e30edcd3ebf22a052b196cb0a9a0db0a41");
    assert_eq!(architecture_before.len(), 394768);
    assert_eq!(digest(architecture_before.as_bytes()), "20fdbcf1ec5fefc07b95342fea78b1e30edcd3ebf22a052b196cb0a9a0db0a41");
    let mut architecture_inverse = architecture_current.to_owned();
    for &(at, after, before) in [
        (9746, "\n`expression_facts/receiving/directory/held_invocation.rs` owns the invocation's\nempty status backing and first receiver-error slot; `retained/held_status_invocation.rs`\nowns its ordinary caller. The controller retains complete fresh validation;\nthe owner reauthenticates each full_program callback. Complete previous status owners and the whole program caller\nremain test-only in `directory/invocation.rs`, `retained/ordinary_status_invocation.rs`\nand `fact_preflight/before_status_receivers.rs`. The private transport contract and\npending executable qualification are in [retained fact lookup v1](RETAINED-EXPRESSION-FACT-LOOKUP-V1.md).\n", ""),
    ].iter().rev() {
        assert_eq!(architecture_inverse.get(at..at + after.len()), Some(after));
        architecture_inverse.replace_range(at..at + after.len(), before);
    }
    assert_eq!(architecture_inverse, architecture_before, "ENTIRE unchanged source-stage architecture");
    let caller_before_empty = before_empty_terminal_source("src/hir/canonical_loan_storage/caller.rs", include_str!("../../caller.rs"));
    let stream_before_empty = before_empty_terminal_source("src/hir/canonical_loan_storage/caller/stream.rs", include_str!("../stream.rs"));
    let cases: &[(&str, &str)] = &[
        ("docs/ARCHITECTURE.md", architecture_before),
        ("docs/SHARED-LOAN-PLAN-V1.md", include_str!("../../../../../docs/SHARED-LOAN-PLAN-V1.md")),
        ("src/hir/canonical_loan_storage/caller.rs", caller_before_empty.as_ref()),
        ("src/hir/canonical_loan_storage/caller/stage_tests.rs", include_str!("../stage_tests.rs")),
        ("src/hir/canonical_loan_storage/deferred.rs", include_str!("../../deferred.rs")),
        ("src/loan_plan.rs", include_str!("../../../../loan_plan.rs")),
        ("src/loan_plan/adjacency.rs", include_str!("../../../../loan_plan/adjacency.rs")),
        ("src/loan_plan/funded_flat.rs", include_str!("../../../../loan_plan/funded_flat.rs")),
        ("src/loan_plan/paid_materialize.rs", include_str!("../../../../loan_plan/paid_materialize.rs")),
        ("src/loan_plan/paid_materialize/ordered_union.rs", include_str!("../../../../loan_plan/paid_materialize/ordered_union.rs")),
        ("src/loan_plan/paid_plan.rs", include_str!("../../../../loan_plan/paid_plan.rs")),
        ("src/loan_plan/paid_plan/retained.rs", include_str!("../../../../loan_plan/paid_plan/retained.rs")),
        ("src/loan_plan/canonical_sink.rs", include_str!("../../../../loan_plan/canonical_sink.rs")),
        ("src/hir/canonical_loan_storage/caller/stream.rs", stream_before_empty.as_ref()),
        ("src/workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs", include_str!("../../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs")),
        ("src/loan_plan/paid_materialize/owned_inputs_tests.rs", include_str!("../../../../loan_plan/paid_materialize/owned_inputs_tests.rs")),
    ];
    source_reader::controls();
    let historical = include_str!(
        "../../../../loan_plan/canonical_sink/source-stage-before-owned-integration.json"
    );
    let retained = &snapshot["retained_source_stage"];
    assert_eq!(
        retained["commit"],
        "d29613dda4d0c993956a35b6e6ea3ed8f735b527"
    );
    assert_eq!(
        retained["length"].as_u64().unwrap() as usize,
        historical.len()
    );
    assert_eq!(
        digest(historical.as_bytes()),
        retained["sha256"].as_str().unwrap()
    );
    assert_eq!(
        source_reader::read(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(retained["path"].as_str().unwrap()),
            historical.len(),
            retained["sha256"].as_str().unwrap(),
            2 * 1024 * 1024
        )
        .unwrap(),
        historical
    );

    // Authenticate the entire preceding source contract and both old test
    // bodies, rather than relabeling their snapshots as current coverage.
    let history = &snapshot["retained_pre_admitted_fixtures"];
    let prior = &history["source_metadata"];
    let read_archive = |record: &serde_json::Value| {
        source_reader::read(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(record["path"].as_str().unwrap()),
            record["length"].as_u64().unwrap() as usize,
            record["sha256"].as_str().unwrap(),
            2 * 1024 * 1024,
        )
        .unwrap()
    };
    let repair = &snapshot["retained_before_control_repair"];
    let repair_prior_raw = read_archive(&repair["metadata"]);
    let repair_prior: serde_json::Value = serde_json::from_str(&repair_prior_raw).unwrap();
    assert_eq!(repair_prior_raw.len(), 670127);
    assert_eq!(
        digest(repair_prior_raw.as_bytes()),
        "8aee7eb9e4279066053937a40a42ff32c2bdff678136e3741157b1f93d9af499"
    );
    let repair_sources = repair["test_sources"].as_array().unwrap();
    assert_eq!(repair_sources.len(), 2);
    for record in repair_sources {
        let archived = read_archive(record);
        assert_eq!(
            digest(archived.as_bytes()),
            record["sha256"].as_str().unwrap()
        );
    }
    let mut repair_metadata = snapshot.clone();
    repair_metadata
        .as_object_mut()
        .unwrap()
        .remove("retained_before_control_repair");
    for stage in ["rows", "immediate_owned_stage"] {
        let (current_rows, previous_rows) = if stage == "rows" {
            (&mut repair_metadata["rows"], &repair_prior["rows"])
        } else {
            (
                &mut repair_metadata[stage]["rows"],
                &repair_prior[stage]["rows"],
            )
        };
        let current_rows = current_rows.as_array_mut().unwrap();
        let previous_rows = previous_rows.as_array().unwrap();
        assert_eq!(current_rows.len(), previous_rows.len());
        for (current, previous) in current_rows.iter_mut().zip(previous_rows) {
            for key in ["after_length", "after_sha256", "edits"] {
                current[key] = previous[key].clone();
            }
            assert_eq!(
                &*current, previous,
                "whole target and unknown fields remain exact"
            );
        }
    }
    assert_eq!(repair_metadata, repair_prior);
    let prior_raw = read_archive(prior);
    assert_eq!(prior_raw.len(), 669479);
    assert_eq!(
        digest(prior_raw.as_bytes()),
        "b0431b45c4448896a962c8bfb7be0cb93f2b648c976eeae81865c44741cfc238"
    );
    let prior_snapshot: serde_json::Value = serde_json::from_str(&prior_raw).unwrap();
    let test_sources = history["test_sources"].as_array().unwrap();
    assert_eq!(test_sources.len(), 2);
    for record in test_sources {
        let archived = read_archive(record);
        assert_eq!(
            digest(archived.as_bytes()),
            record["sha256"].as_str().unwrap()
        );
    }
    for (current_rows, prior_rows) in [
        (&snapshot["rows"], &prior_snapshot["rows"]),
        (
            &snapshot["immediate_owned_stage"]["rows"],
            &prior_snapshot["immediate_owned_stage"]["rows"],
        ),
    ] {
        let current_rows = current_rows.as_array().unwrap();
        let prior_rows = prior_rows.as_array().unwrap();
        assert_eq!(current_rows.len(), prior_rows.len());
        for (current, prior) in current_rows.iter().zip(prior_rows) {
            assert_eq!(current["path"], prior["path"]);
            assert_eq!(current["before"], prior["before"]);
            assert_eq!(current["before_sha256"], prior["before_sha256"]);
            let mut original_row = current.clone();
            for key in ["after_length", "after_sha256", "edits"] {
                original_row[key] = prior[key].clone();
            }
            assert_eq!(&original_row, prior, "all other row metadata is retained");
        }
    }
    let mut retained = snapshot.clone();
    retained
        .as_object_mut()
        .unwrap()
        .remove("retained_pre_admitted_fixtures");
    retained
        .as_object_mut()
        .unwrap()
        .remove("retained_before_control_repair");
    retained["rows"] = prior_snapshot["rows"].clone();
    retained["immediate_owned_stage"]["rows"] =
        prior_snapshot["immediate_owned_stage"]["rows"].clone();
    assert_eq!(
        retained, prior_snapshot,
        "all preceding unknown metadata remains whole"
    );

    let rows = snapshot["rows"].as_array().unwrap();
    assert_eq!(rows.len(), cases.len());
    for (row, (path, compiled)) in rows.iter().zip(cases) {
        assert_eq!(row["path"], *path);
        assert_eq!(
            row["after_length"].as_u64().unwrap() as usize,
            compiled.len()
        );
        let pin = row["after_sha256"].as_str().unwrap();
        assert_eq!(digest(compiled.as_bytes()), pin);
        let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(match *path { "docs/ARCHITECTURE.md" => "e/20fdbcf1ec5fefc07b95342fea78b1e30edcd3ebf22a052b196cb0a9a0db0a41", "src/hir/canonical_loan_storage/caller.rs" => "e/e6182e13a2435197176743921951ba2a88ebc15ef9af0ce4007f2e778c1f3e7b", "src/hir/canonical_loan_storage/caller/stream.rs" => "e/cf07788c9675cdaad420205cc55ae6920acb24ad554a6f9ffe4c51b6c8e4a9ea", _ => path });
        assert_eq!(
            source_reader::read(&physical, compiled.len(), pin, 2 * 1024 * 1024).unwrap(),
            *compiled
        );
        if let Some(before) = row["before"].as_str() {
            assert_eq!(
                digest(before.as_bytes()),
                row["before_sha256"].as_str().unwrap()
            );
            let mut restored = Vec::new();
            let mut cursor = 0usize;
            for edit in row["edits"].as_array().unwrap() {
                let start = edit["start"].as_u64().unwrap() as usize;
                let end = edit["end"].as_u64().unwrap() as usize;
                assert!(cursor <= start && start <= end && end <= compiled.len());
                assert_eq!(
                    &compiled.as_bytes()[start..end],
                    edit["after"].as_str().unwrap().as_bytes()
                );
                restored.extend_from_slice(&compiled.as_bytes()[cursor..start]);
                restored.extend_from_slice(edit["before"].as_str().unwrap().as_bytes());
                cursor = end;
            }
            restored.extend_from_slice(&compiled.as_bytes()[cursor..]);
            assert_eq!(restored, before.as_bytes());
        } else {
            assert!(row["edits"].as_array().unwrap().is_empty());
        }
        // Physical acquisition is repeated after the entire inverse replay.
        assert_eq!(
            source_reader::read(&physical, compiled.len(), pin, 2 * 1024 * 1024).unwrap(),
            *compiled
        );
    }

    assert_eq!(source_reader::read(&architecture_path, 395407, "7404359862f60c6adda119c1e32fbff955d9568105b6046ba5c712354a246317", 2 * 1024 * 1024).unwrap(), architecture_current);

    assert_eq!(source_reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/hir/canonical_loan_storage/caller.rs"), 28421, "f2ef60ecb5dfe623dc8db8d0b3468841b5c74e8b77cd6f5c219cd221ecfce646", 2 * 1024 * 1024).unwrap(), include_str!("../../caller.rs"));
    assert_eq!(source_reader::read(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/hir/canonical_loan_storage/caller/stream.rs"), 16291, "d5d7e473a47fd41d44b8dd587b21cb49bec47d799b2a3a2c275b5f5a6b5f1cff", 2 * 1024 * 1024).unwrap(), include_str!("../stream.rs"));

    assert_eq!(
        snapshot["immediate_owned_stage"]["commit"],
        "548dee26c5643d49307b0dd1fde60a36a6ac1434"
    );
    for row in snapshot["immediate_owned_stage"]["rows"]
        .as_array()
        .unwrap()
    {
        let current = cases
            .iter()
            .find(|r| r.0 == row["path"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(
            digest(current.as_bytes()),
            row["after_sha256"].as_str().unwrap()
        );
        let mut restored = current.as_bytes().to_vec();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let start = edit["start"].as_u64().unwrap() as usize;
            let end = edit["end"].as_u64().unwrap() as usize;
            assert_eq!(
                &restored[start..end],
                edit["after"].as_str().unwrap().as_bytes()
            );
            restored.splice(start..end, edit["before"].as_str().unwrap().bytes());
        }
        assert_eq!(restored, row["before"].as_str().unwrap().as_bytes());
        assert_eq!(digest(&restored), row["before_sha256"].as_str().unwrap());
    }
    let old_caller = rows
        .iter()
        .find(|r| r["path"] == "src/hir/canonical_loan_storage/caller.rs")
        .unwrap()["before"]
        .as_str()
        .unwrap();
    let original = old_caller
        .split("fn prepare_sealed<'a, T>(")
        .nth(1)
        .unwrap()
        .split("\nfn append(")
        .next()
        .unwrap();
    let current = cases
        .iter()
        .find(|r| r.0 == "src/hir/canonical_loan_storage/caller.rs")
        .unwrap()
        .1;
    assert!(current.contains(&format!("fn prepare_sealed_before<'a, T>({original}")));
}

fn empty_scalar_program(functions: usize) -> ResolvedProgram {
    let mut source = String::from("module empty.canonical; ");
    for index in 0..functions {
        source.push_str(&format!("@id(\"empty.f{index}\") fn f{index}(x:i64)->i64 {{x+1}} "));
    }
    source.push_str("@id(\"empty.main\") fn main()->i64{0}");
    crate::hir::resolve(&crate::check(&source, "empty-canonical.spx").unwrap()).unwrap()
}
fn empty_stage<T>(old: bool, operation: impl FnOnce() -> T) -> T {
    if old { with_before_empty_terminal(operation) } else { operation() }
}
fn empty_run(program: &ResolvedProgram, old: bool, cap: usize, floor: usize, success: bool) -> usize {
    let seed = ConstructorSeed::authenticate(program, None).unwrap();
    take_empty_counts();
    let ((ok, metrics), exhausted, paid, receipt) = with_limit_usage_refusal(cap, || {
        assert!(bounded_output::set_active_floor(floor));
        let measured = measure(|| {
            let result = empty_stage(old, || prepare_with_comparison(&seed, program));
            let ok = result.is_ok();
            assert_eq!(ok, success, "old={old} cap={cap}: {:?}", result.as_ref().err());
            match result {
                Ok((storage, deferred)) => {
                    assert!(storage.is_current(program));
                    assert!(storage.ids.is_empty() && storage.liveness.is_empty() && storage.origins.is_empty());
                    deferred.finish(program).unwrap();
                    drop(storage);
                }
                Err(error) => {
                    assert_eq!(error.code, "SPX-H006");
                    drop(error);
                    let first = crate::bounded_output::classifier_string::ClassifierLedger::capture().first_refusal();
                    let remaining = bounded_output::active_remaining();
                    let retry = empty_stage(old, || prepare_with_comparison(&seed, program));
                    assert!(retry.is_err()); drop(retry);
                    assert_eq!(bounded_output::active_remaining(), remaining);
                    assert_eq!(crate::bounded_output::classifier_string::ClassifierLedger::capture().first_refusal(), first);
                }
            }
            ok
        });
        bounded_output::clear_active_floor();
        measured
    });
    assert_eq!(ok, success);
    assert_eq!(exhausted, !success);
    assert_eq!(receipt.is_some(), !success);
    clean(metrics, paid);
    if success && !old { assert_eq!(take_empty_counts(), (program.functions.len(), 0)); }
    paid
}
#[test]
fn authenticated_empty_terminal_has_whole_scalar_caller_net_after_all_new_branch_and_clear_roles() {
    assert_eq!(std::mem::size_of::<Receiver<'_, '_, '_>>(), std::mem::size_of::<BeforeEmptyReceiver<'_, '_, '_>>());
    for functions in [0, 16, 64] {
        let program = empty_scalar_program(functions);
        let old = empty_run(&program, true, usize::MAX, 0, true);
        let current = empty_run(&program, false, usize::MAX, 0, true);
        let count = program.functions.len();
        assert_eq!(old + count * (BRANCH_SELECTION_BYTES + EMPTY_CLEAR_BYTES), current + count * APPEND_PRESENT_BYTES);
        assert!(current < old, "complete scalar caller must repay every new branch/clear role");
        for (old, exact) in [(true, old), (false, current)] {
            assert_eq!(empty_run(&program, old, exact, 0, true), exact);
            empty_run(&program, old, exact - 1, 0, false);
            empty_run(&program, old, exact, 1, false);
            empty_run(&program, old, 0, 0, false);
        }
        eprintln!("EMPTY_CANONICAL functions={count} old={old} current={current} delta={}", old - current);
    }
}
#[test]
fn empty_terminal_keeps_complete_deferred_empty_schema_and_earlier_semantic_priority() {
    let original = empty_scalar_program(1);
    for schema in ["forged.empty", crate::loan_plan::LOAN_PLAN_SCHEMA_V3] {
        let mut program = original.clone();program.functions[0].loan_plan.schema = schema;
        let seed = ConstructorSeed::authenticate(&program, None).unwrap();
        let run = |old| with_limit_usage_refusal(usize::MAX, || {
            let (storage, deferred) = empty_stage(old, || prepare_with_comparison(&seed, &program)).unwrap();
            assert!(storage.ids.is_empty() && storage.liveness.is_empty() && storage.origins.is_empty());
            drop(storage);deferred.finish(&program).unwrap_err()
        });
        let before = run(true);let current = run(false);
        assert!(!before.1 && !current.1 && before.3.is_none() && current.3.is_none());
        assert_eq!(before.0, current.0, "all six deferred fields remain exact");
        assert!(current.0.message.contains("shared-loan"));
        program.entrypoint = crate::hir::DeclarationId::new("missing.entry");
        let before = with_limit_usage_refusal(usize::MAX, || empty_stage(true, || crate::hir::validate(&program))).0;
        let current = with_limit_usage_refusal(usize::MAX, || crate::hir::validate(&program)).0;
        assert_eq!(before, current);
        assert!(!current.unwrap_err().message.contains("forged shared-loan"));
    }
}
#[test]
fn complete_immediate_empty_driver_receiver_and_invoice_are_preserved_verbatim() {
    let old_driver: &str = "fn prepare_sealed<'a, T>(\n    seed: &ConstructorSeed<'_>,\n    program: &'a ResolvedProgram,\n    seal: impl FnOnce(Storage<'a>, Payer<'_>) -> Result<T, Diagnostic>,\n) -> Result<(T, Deferred<'a>), Diagnostic> {\n    #[cfg(test)]\n    if stream::before_selected() {\n        return prepare_sealed_before(seed, program, seal);\n    }\n    #[cfg(test)]\n    let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"hir::canonical_loan_storage::prepare_sealed\",\n    );\n    let _stage = crate::bounded_output::reservation_stage_guard(\n        crate::bounded_output::ReservationStage::CanonicalLoanStorage,\n    );\n    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::CanonicalLoanStorage,\n    );\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n        crate::bounded_output::exclusive_debits::Operation::CanonicalLoanStorage,\n    );\n    if !std::ptr::eq(seed.program(), program) {\n        return Err(hir_error(\n            \"canonical shared-loan constructor belongs to another program\",\n        ));\n    }\n    // Acquisition follows the unchanged ModuleIndex and ConstructorSeed\n    // phases. One report/ledger is held across ALL plans and their append; a\n    // storage-only test injection keeps its separate legacy payer below.\n    let captured = if crate::bounded_output::active_remaining().is_some() {\n        Some(CfgReservation::acquire().map_err(|_| refused())?)\n    } else {\n        None\n    };\n    let payer = captured\n        .as_ref()\n        .map_or_else(Payer::current, Payer::Captured);\n    let result = (|| {\n        payer.reserve(0)?;\n        let mut deferred = Deferred::new(program);\n        let mut storage = Storage {\n            program,\n            ids: Vec::new(),\n            liveness: Vec::new(),\n            origins: Vec::new(),\n            _scratch: Scratch::default(),\n        };\n        for function in program\n            .functions\n            .iter()\n            .chain(program.function_instances.iter().map(|i| &i.function))\n        {\n            #[cfg(test)]\n            let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n                \"hir::canonical_loan_storage::next_function\",\n            );\n            if let Some(owner) = captured.as_ref() {\n                // Pay the complete new captured receiver/closure, actual and\n                // formal move, Result/Option receiving and scratch detach/drop\n                // owners before forming either the owner or closure.\n                payer.reserve(\n                    2 * std::mem::size_of::<(\n                        &crate::loan_plan::canonical_sink::Parts<'_>,\n                        Payer<'_>,\n                    )>() + 2 * std::mem::size_of::<Result<usize, Diagnostic>>()\n                        + 12 * std::mem::size_of::<stream::Receiver<'_, '_, '_>>()\n                        + 8 * std::mem::size_of::<Result<(), Diagnostic>>()\n                        + 8 * std::mem::size_of::<\n                            Option<&mut crate::loan_plan::canonical_sink::Source<'_, '_>>,\n                        >()\n                        + 4 * std::mem::size_of::<Scratch>()\n                        + 4 * std::mem::size_of::<Option<(usize, bool)>>()\n                        + 8 * std::mem::size_of::<usize>()\n                        + 16 * std::mem::size_of::<bool>(),\n                )?;\n                let receiver = stream::Receiver {\n                    storage: &mut storage,\n                    deferred: &mut deferred,\n                    function,\n                    payer,\n                };\n                match crate::loan_plan::build_canonical(\n                    program,\n                    function,\n                    owner,\n                    move |source, matched| receiver.receive(source, matched),\n                ) {\n                    Ok(()) => (),\n                    Err(crate::loan_plan::FundedError::Physical(reason)) => {\n                        debug_assert_eq!(owner.first(), Some(reason));\n                        return Err(refused());\n                    }\n                    Err(crate::loan_plan::FundedError::Semantic(error)) => return Err(error),\n                }\n            } else {\n                // Unobserved planning retains its entire original producer.\n                let plan = crate::loan_plan::build_plan(program, function)?;\n                deferred.compare(function, &plan);\n                let _stage = crate::bounded_output::reservation_stage_guard(\n                    crate::bounded_output::ReservationStage::LoanCanonicalAppend,\n                );\n                append(&mut storage, plan, payer)?;\n            }\n        }\n        Ok((seal(storage, payer)?, deferred))\n    })();\n    // Only this exact affine owner can mark physical failure. Never infer\n    // payment from H006 text, and never reserve/render after semantic error.\n    if let Some(owner) = captured {\n        if owner.first().is_some() {\n            return Err(owner.diagnostic());\n        }\n    }\n    result\n}\n";
    let old_receiver: &str = "pub(super) struct Receiver<'a, 's, 'p> {\n    pub(super) storage: &'s mut Storage<'a>,\n    pub(super) deferred: &'s mut Deferred<'a>,\n    pub(super) function: &'a ResolvedFunction,\n    pub(super) payer: Payer<'p>,\n}\nimpl<'a, 'p> Receiver<'a, '_, 'p> {\n    pub(super) fn receive(\n        self,\n        source: Option<&mut Source<'a, 'p>>,\n        matched: bool,\n    ) -> Result<(), Diagnostic> {\n        self.payer.reserve(0)?;\n        let _stage = crate::bounded_output::reservation_stage_guard(\n            crate::bounded_output::ReservationStage::LoanCanonicalAppend,\n        );\n        let Some(source) = source else {\n            self.deferred.compare_match(self.function, matched);\n            // Empty append still pays the old zero-request affinity check.\n            return append(self.storage, LoanPlan::empty_v1(), self.payer);\n        };\n        if !source.is_current(\n            self.storage.program,\n            self.function,\n            match self.payer {\n                Payer::Captured(owner) => owner,\n                _ => unreachable!(\"stream requires captured owner\"),\n            },\n        ) {\n            return Err(hir_error(\n                \"canonical shared-loan source belongs to another program\",\n            ));\n        }\n        let mut scratch = std::mem::take(&mut self.storage._scratch);\n        let result = append_source(\n            self.storage,\n            self.deferred,\n            self.function,\n            source,\n            matched,\n            self.payer,\n            &mut scratch,\n        );\n        scratch.local.clear();\n        scratch.liveness.clear();\n        scratch.ids.clear();\n        self.storage._scratch = scratch;\n        result\n    }\n}\n";
    let driver_copy = old_driver.replacen("fn prepare_sealed<'a, T>(", "fn prepare_sealed_before_empty_terminal<'a, T>(", 1)
        .replace("stream::Receiver", "stream::BeforeEmptyReceiver");
    let receiver_copy = old_receiver.replacen("struct Receiver<", "struct BeforeEmptyReceiver<", 1)
        .replacen("impl<'a, 'p> Receiver<", "impl<'a, 'p> BeforeEmptyReceiver<", 1)
        .replacen("\nimpl<'a, 'p> BeforeEmptyReceiver<", "\n#[cfg(test)]\nimpl<'a, 'p> BeforeEmptyReceiver<", 1);
    assert_eq!(include_str!("../../caller.rs").matches(&driver_copy).count(), 1);
    assert_eq!(include_str!("../stream.rs").matches(&receiver_copy).count(), 1);
}

fn mixed_empty_program(interleave: bool) -> ResolvedProgram {
    let first = r#"@id("mixed.first") fn first(packet:own Packet)->usize { let view=bytes_as_slice(packet.payload); byte_len(view)+byte_len(view) }"#;
    let second = r#"@id("mixed.second") fn second(packet:own Packet)->usize { let view=bytes_as_slice(packet.payload); byte_len(view)+byte_len(view) }"#;
    let empty = r#"@id("mixed.empty") fn empty(x:i64)->i64 {x+1}"#;
    let mut source = String::from(r#"module mixed.canonical; @id("mixed.packet") record Packet { @id("mixed.payload") payload:Bytes, } "#);
    source.push_str(first);
    source.push_str(if interleave { empty } else { second });
    source.push_str(if interleave { second } else { empty });
    source.push_str(r#"@id("mixed.main") fn main()->i64 {0}"#);
    crate::hir::resolve(&crate::check(&source, "mixed-canonical.spx").unwrap()).unwrap()
}
fn mixed_empty_run(program: &ResolvedProgram, expected: &Storage<'_>, old: bool,
    cap: usize, floor: usize, success: bool) -> usize
{
    let seed = ConstructorSeed::authenticate(program, None).unwrap();
    take_empty_counts();
    let ((ok, metrics), refused, paid, receipt) = with_limit_usage_refusal(cap, || {
        assert!(bounded_output::set_active_floor(floor));
        let measured = measure(|| {
            let result = empty_stage(old, || prepare_with_comparison(&seed, program));
            let ok = result.is_ok();
            assert_eq!(ok, success, "old={old} cap={cap}: {:?}", result.as_ref().err());
            match result {
                Ok((storage, deferred)) => {
                    assert_storage(&storage, expected);
                    deferred.finish(program).unwrap();
                    drop(storage);
                }
                Err(error) => {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                    assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
                    assert_eq!(error.message, if cap == 0 { "" } else { "canonical loan CFG storage reservation refused" });
                    drop(error);
                    let remaining = bounded_output::active_remaining();
                    let first = crate::bounded_output::classifier_string::ClassifierLedger::capture().first_refusal();
                    let retry = empty_stage(old, || prepare_with_comparison(&seed, program));
                    assert!(retry.is_err());drop(retry);
                    assert_eq!(bounded_output::active_remaining(), remaining);
                    assert_eq!(crate::bounded_output::classifier_string::ClassifierLedger::capture().first_refusal(), first);
                }
            }
            ok
        });
        bounded_output::clear_active_floor();measured
    });
    assert_eq!(ok, success);assert_eq!(refused, !success);assert_eq!(receipt.is_some(), !success);
    clean(metrics, paid);
    if success && !old { assert_eq!(take_empty_counts(), (2, 2), "actual independent producer has two Some and two None outcomes"); }
    paid
}
#[test]
fn actual_some_and_some_none_some_keep_whole_immediate_rows_scratch_and_cold_surcharge_limits() {
    for interleave in [false, true] {
        let program = mixed_empty_program(interleave);
        let seed = ConstructorSeed::authenticate(&program, None).unwrap();
        // The complete immediate producer runs outside the allocation observer
        // solely to supply semantic expected rows; old/current measured runs
        // independently execute their actual captured producer and full Drop.
        let (expected, deferred) = with_limit_usage_refusal(usize::MAX, ||
            empty_stage(true, || prepare_with_comparison(&seed, &program))).0.unwrap();
        deferred.finish(&program).unwrap();
        assert!(!expected.origins.is_empty() && !expected.ids.is_empty() && !expected.liveness.is_empty());
        let old = mixed_empty_run(&program, &expected, true, usize::MAX, 0, true);
        let current = mixed_empty_run(&program, &expected, false, usize::MAX, 0, true);
        // BOTH Some branches retain their complete old append bank. Only the
        // two real None results remove it; every function pays branch controls.
        assert_eq!(old + 4 * BRANCH_SELECTION_BYTES + 2 * EMPTY_CLEAR_BYTES,
            current + 2 * APPEND_PRESENT_BYTES);
        for (old, exact) in [(true, old), (false, current)] {
            assert_eq!(mixed_empty_run(&program, &expected, old, exact, 0, true), exact);
            mixed_empty_run(&program, &expected, old, exact - 1, 0, false);
            mixed_empty_run(&program, &expected, old, exact, 1, false);
            mixed_empty_run(&program, &expected, old, 0, 0, false);
        }
        drop(expected);
    }
}

// Actual complete current images authenticate before any fixed historical
// source-stage probe. No snapshot/old target is repriced or selected by wire.
fn before_empty_terminal_source<'a>(path: &str, supplied: &'a str) -> std::borrow::Cow<'a, str> {
    use sha2::{Digest, Sha256};
    let hash = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let (compiled, current_length, current_sha, before, before_length, before_sha, edits):
        (&str, usize, &str, &str, usize, &str, &[(usize, &str, &str)]) = match path {
        "src/hir/canonical_loan_storage/caller.rs"=> (include_str!("../../caller.rs"), 28421, "f2ef60ecb5dfe623dc8db8d0b3468841b5c74e8b77cd6f5c219cd221ecfce646",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/e6182e13a2435197176743921951ba2a88ebc15ef9af0ce4007f2e778c1f3e7b")), 23404, "e6182e13a2435197176743921951ba2a88ebc15ef9af0ce4007f2e778c1f3e7b", &[
                (7703, "    if stream::empty_before_selected() {\n        return prepare_sealed_before_empty_terminal(seed, program, seal);\n    }\n    #[cfg(test)]\n", ""),
                (9917, "                // Common receiver/closure and Result/Option transports remain\n                // paid before formation. The separately priced branch selection is\n                // additional; actual append-only roles are paid after Some.\n", "                // Pay the complete new captured receiver/closure, actual and\n                // formal move, Result/Option receiving and scratch detach/drop\n                // owners before forming either the owner or closure.\n"),
                (10188, "                    12 * std::mem::size_of::<stream::Receiver<'_, '_, '_>>()\n", "                    2 * std::mem::size_of::<(\n                        &crate::loan_plan::canonical_sink::Parts<'_>,\n                        Payer<'_>,\n                    )>() + 2 * std::mem::size_of::<Result<usize, Diagnostic>>()\n                        + 12 * std::mem::size_of::<stream::Receiver<'_, '_, '_>>()\n"),
                (10511, "                        + stream::BRANCH_SELECTION_BYTES,\n", "                        + 4 * std::mem::size_of::<Scratch>()\n                        + 4 * std::mem::size_of::<Option<(usize, bool)>>()\n                        + 8 * std::mem::size_of::<usize>()\n                        + 16 * std::mem::size_of::<bool>(),\n"),
                (23120, "\n// Complete immediate driver and original invoice; namespace changes only.\n#[cfg(test)]\nfn prepare_sealed_before_empty_terminal<'a, T>(\n    seed: &ConstructorSeed<'_>,\n    program: &'a ResolvedProgram,\n    seal: impl FnOnce(Storage<'a>, Payer<'_>) -> Result<T, Diagnostic>,\n) -> Result<(T, Deferred<'a>), Diagnostic> {\n    #[cfg(test)]\n    if stream::before_selected() {\n        return prepare_sealed_before(seed, program, seal);\n    }\n    #[cfg(test)]\n    let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n        \"hir::canonical_loan_storage::prepare_sealed\",\n    );\n    let _stage = crate::bounded_output::reservation_stage_guard(\n        crate::bounded_output::ReservationStage::CanonicalLoanStorage,\n    );\n    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::CanonicalLoanStorage,\n    );\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n        crate::bounded_output::exclusive_debits::Operation::CanonicalLoanStorage,\n    );\n    if !std::ptr::eq(seed.program(), program) {\n        return Err(hir_error(\n            \"canonical shared-loan constructor belongs to another program\",\n        ));\n    }\n    // Acquisition follows the unchanged ModuleIndex and ConstructorSeed\n    // phases. One report/ledger is held across ALL plans and their append; a\n    // storage-only test injection keeps its separate legacy payer below.\n    let captured = if crate::bounded_output::active_remaining().is_some() {\n        Some(CfgReservation::acquire().map_err(|_| refused())?)\n    } else {\n        None\n    };\n    let payer = captured\n        .as_ref()\n        .map_or_else(Payer::current, Payer::Captured);\n    let result = (|| {\n        payer.reserve(0)?;\n        let mut deferred = Deferred::new(program);\n        let mut storage = Storage {\n            program,\n            ids: Vec::new(),\n            liveness: Vec::new(),\n            origins: Vec::new(),\n            _scratch: Scratch::default(),\n        };\n        for function in program\n            .functions\n            .iter()\n            .chain(program.function_instances.iter().map(|i| &i.function))\n        {\n            #[cfg(test)]\n            let _allocation_phase = crate::test_allocation_observer::function_phase_guard(\n                \"hir::canonical_loan_storage::next_function\",\n            );\n            if let Some(owner) = captured.as_ref() {\n                // Pay the complete new captured receiver/closure, actual and\n                // formal move, Result/Option receiving and scratch detach/drop\n                // owners before forming either the owner or closure.\n                payer.reserve(\n                    2 * std::mem::size_of::<(\n                        &crate::loan_plan::canonical_sink::Parts<'_>,\n                        Payer<'_>,\n                    )>() + 2 * std::mem::size_of::<Result<usize, Diagnostic>>()\n                        + 12 * std::mem::size_of::<stream::BeforeEmptyReceiver<'_, '_, '_>>()\n                        + 8 * std::mem::size_of::<Result<(), Diagnostic>>()\n                        + 8 * std::mem::size_of::<\n                            Option<&mut crate::loan_plan::canonical_sink::Source<'_, '_>>,\n                        >()\n                        + 4 * std::mem::size_of::<Scratch>()\n                        + 4 * std::mem::size_of::<Option<(usize, bool)>>()\n                        + 8 * std::mem::size_of::<usize>()\n                        + 16 * std::mem::size_of::<bool>(),\n                )?;\n                let receiver = stream::BeforeEmptyReceiver {\n                    storage: &mut storage,\n                    deferred: &mut deferred,\n                    function,\n                    payer,\n                };\n                match crate::loan_plan::build_canonical(\n                    program,\n                    function,\n                    owner,\n                    move |source, matched| receiver.receive(source, matched),\n                ) {\n                    Ok(()) => (),\n                    Err(crate::loan_plan::FundedError::Physical(reason)) => {\n                        debug_assert_eq!(owner.first(), Some(reason));\n                        return Err(refused());\n                    }\n                    Err(crate::loan_plan::FundedError::Semantic(error)) => return Err(error),\n                }\n            } else {\n                // Unobserved planning retains its entire original producer.\n                let plan = crate::loan_plan::build_plan(program, function)?;\n                deferred.compare(function, &plan);\n                let _stage = crate::bounded_output::reservation_stage_guard(\n                    crate::bounded_output::ReservationStage::LoanCanonicalAppend,\n                );\n                append(&mut storage, plan, payer)?;\n            }\n        }\n        Ok((seal(storage, payer)?, deferred))\n    })();\n    // Only this exact affine owner can mark physical failure. Never infer\n    // payment from H006 text, and never reserve/render after semantic error.\n    if let Some(owner) = captured {\n        if owner.first().is_some() {\n            return Err(owner.diagnostic());\n        }\n    }\n    result\n}\n\n", ""),
            ]),
        "src/hir/canonical_loan_storage/caller/stream.rs"=> (include_str!("../stream.rs"), 16291, "d5d7e473a47fd41d44b8dd587b21cb49bec47d799b2a3a2c275b5f5a6b5f1cff",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/cf07788c9675cdaad420205cc55ae6920acb24ad554a6f9ffe4c51b6c8e4a9ea")), 11718, "cf07788c9675cdaad420205cc55ae6920acb24ad554a6f9ffe4c51b6c8e4a9ea", &[
                (258, "\n// These exact original roles are formed only by the nonempty append path.\n// Their counts/tariffs are retained; None never constructs their carriers.\npub(super) const APPEND_PRESENT_BYTES: usize = 2 * std::mem::size_of::<(\n    &crate::loan_plan::canonical_sink::Parts<'_>, Payer<'_>,\n)>() + 2 * std::mem::size_of::<Result<usize, Diagnostic>>()\n    + 4 * std::mem::size_of::<Scratch>()\n    + 4 * std::mem::size_of::<Option<(usize, bool)>>()\n    + 8 * std::mem::size_of::<usize>()\n    + 16 * std::mem::size_of::<bool>();\n// Only Option::is_some's actual/formal references, returned/received bool,\n// and the branch/receiving usize are new. The original first reserve call\n// retains its complete original payer/active/required/Result chain; there is\n// no extra reserve invocation and no credit borrowed from another bank.\npub(super) const BRANCH_SELECTION_BYTES: usize = 2 * std::mem::size_of::<\n    &Option<&mut crate::loan_plan::canonical_sink::Source<'_, '_>>,\n>() + 2 * std::mem::size_of::<bool>() + 2 * std::mem::size_of::<usize>();\nconst EMPTY_CLEAR_BYTES: usize = 2 * std::mem::size_of::<&mut Vec<(LoanId, usize)>>()\n    + 2 * std::mem::size_of::<&mut Vec<(LiveKey, Option<usize>, usize)>>()\n    + 2 * std::mem::size_of::<&mut Vec<(IdKey, LoanId, usize)>>();\n", ""),
                (1920, "        let charge = if source.is_some() { APPEND_PRESENT_BYTES } else { EMPTY_CLEAR_BYTES };\n        self.payer.reserve(charge)?;\n", "        self.payer.reserve(0)?;\n"),
                (2311, "            // This is the producer's authenticated complete empty result,\n            // after its schema/row comparison. Keep the second old zero check\n            // and clear retained scratch without forming an empty plan or any\n            // append/detach/probe/growth/sort carrier.\n            self.payer.reserve(0)?;\n            self.storage._scratch.local.clear();\n            self.storage._scratch.liveness.clear();\n            self.storage._scratch.ids.clear();\n            #[cfg(test)]\n            EMPTY_COUNTS.with(|counts| { let (empty, present) = counts.get(); counts.set((empty + 1, present)); });\n            return Ok(());\n", "            // Empty append still pays the old zero-request affinity check.\n            return append(self.storage, LoanPlan::empty_v1(), self.payer);\n"),
                (2963, "        #[cfg(test)]\n        EMPTY_COUNTS.with(|counts| { let (empty, present) = counts.get(); counts.set((empty, present + 1)); });\n", ""),
                (13707, "\n// Complete immediate receiver, including its original None append pipeline.\n#[cfg(test)]\npub(super) struct BeforeEmptyReceiver<'a, 's, 'p> {\n    pub(super) storage: &'s mut Storage<'a>,\n    pub(super) deferred: &'s mut Deferred<'a>,\n    pub(super) function: &'a ResolvedFunction,\n    pub(super) payer: Payer<'p>,\n}\n#[cfg(test)]\nimpl<'a, 'p> BeforeEmptyReceiver<'a, '_, 'p> {\n    pub(super) fn receive(\n        self,\n        source: Option<&mut Source<'a, 'p>>,\n        matched: bool,\n    ) -> Result<(), Diagnostic> {\n        self.payer.reserve(0)?;\n        let _stage = crate::bounded_output::reservation_stage_guard(\n            crate::bounded_output::ReservationStage::LoanCanonicalAppend,\n        );\n        let Some(source) = source else {\n            self.deferred.compare_match(self.function, matched);\n            // Empty append still pays the old zero-request affinity check.\n            return append(self.storage, LoanPlan::empty_v1(), self.payer);\n        };\n        if !source.is_current(\n            self.storage.program,\n            self.function,\n            match self.payer {\n                Payer::Captured(owner) => owner,\n                _ => unreachable!(\"stream requires captured owner\"),\n            },\n        ) {\n            return Err(hir_error(\n                \"canonical shared-loan source belongs to another program\",\n            ));\n        }\n        let mut scratch = std::mem::take(&mut self.storage._scratch);\n        let result = append_source(\n            self.storage,\n            self.deferred,\n            self.function,\n            source,\n            matched,\n            self.payer,\n            &mut scratch,\n        );\n        scratch.local.clear();\n        scratch.liveness.clear();\n        scratch.ids.clear();\n        self.storage._scratch = scratch;\n        result\n    }\n}\n\n\n#[cfg(test)]\nthread_local! {\n    static EMPTY_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };\n    static EMPTY_COUNTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };\n}\n#[cfg(test)]\npub(super) fn empty_before_selected() -> bool { EMPTY_BEFORE.with(|flag| flag.get()) }\n#[cfg(test)]\nfn with_before_empty_terminal<T>(operation: impl FnOnce() -> T) -> T {\n    struct Reset(bool);\n    impl Drop for Reset { fn drop(&mut self) { EMPTY_BEFORE.with(|flag| flag.set(self.0)); } }\n    let reset = Reset(EMPTY_BEFORE.with(|flag| flag.replace(true)));\n    let output = operation();\n    drop(reset);\n    output\n}\n#[cfg(test)]\nfn take_empty_counts() -> (usize, usize) { EMPTY_COUNTS.with(|counts| counts.replace((0, 0))) }\n", ""),
            ]),
        _ => return std::borrow::Cow::Borrowed(supplied),
    };
    assert_eq!(supplied, compiled);
    assert_eq!(compiled.len(), current_length);
    assert_eq!(hash(compiled.as_bytes()), current_sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(source_reader::read(&root.join(path), current_length, current_sha, 2 * 1024 * 1024).unwrap(), compiled);
    assert_eq!(before.len(), before_length);
    assert_eq!(hash(before.as_bytes()), before_sha);
    let mut restored = compiled.to_owned();
    for &(at, after, prior) in edits.iter().rev() {
        assert_eq!(restored.get(at..at + after.len()), Some(after));
        restored.replace_range(at..at + after.len(), prior);
    }
    assert_eq!(restored, before, "ENTIRE immediate source stage unchanged");
    assert_eq!(source_reader::read(&root.join("e").join(before_sha), before_length, before_sha, 2 * 1024 * 1024).unwrap(), before);
    assert_eq!(source_reader::read(&root.join(path), current_length, current_sha, 2 * 1024 * 1024).unwrap(), compiled);
    std::borrow::Cow::Owned(restored)
}

#[test]
fn both_new_first_positive_receiver_requests_refuse_before_any_terminal_or_append_effect() {
    use crate::bounded_output::cfg_reservation::CfgReservation;
    for present in [false, true] {
        let mut program = mixed_empty_program(true);
        let index = if present { 0 } else { 1 };
        // The independent producer must finish its full comparison and return
        // false; reaching Deferred comparison would visibly select a mismatch.
        program.functions[index].loan_plan.schema = "forged.first-payment";
        let function = &program.functions[index];
        let id = ExpressionId::from_owned("first-payment.sentinel".into());
        let local = vec![(LoanId(9), 10)];
        let liveness = vec![((id.clone(), LoanPointPhase::Before, LoanId(9)), Some(10), 11)];
        let ids = vec![((id, crate::loan_plan::LoanCause::StrView), LoanId(9), 11)];
        let expected_local = local.clone();let expected_liveness = liveness.clone();let expected_ids = ids.clone();
        let mut storage = Storage { program: &program, ids: Vec::new(), liveness: Vec::new(), origins: Vec::new(),
            _scratch: Scratch { local, liveness, ids } };
        let backing = (storage._scratch.local.as_ptr(), storage._scratch.local.capacity(),
            storage._scratch.liveness.as_ptr(), storage._scratch.liveness.capacity(),
            storage._scratch.ids.as_ptr(), storage._scratch.ids.capacity());
        let mut deferred = Deferred::new(&program);
        take_empty_counts();
        let mut completed = false;
        let charge = if present { APPEND_PRESENT_BYTES } else { EMPTY_CLEAR_BYTES };
        let ((), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let result = crate::loan_plan::build_canonical(&program, function, &owner, |source, matched| {
                assert_eq!(source.is_some(), present);
                assert!(!matched, "complete independent comparison must finish before the cut");
                completed = true;
                let remaining = bounded_output::active_remaining().unwrap();
                assert!(remaining >= charge);
                // Spend the live account normally; never reset or replace it.
                assert!(bounded_output::reserve_active_required(remaining - (charge - 1)));
                let error = Receiver { storage: &mut storage, deferred: &mut deferred,
                    function, payer: Payer::Captured(&owner) }.receive(source, matched).unwrap_err();
                assert_eq!(error.code, "SPX-H006");
                assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                assert!(error.message.is_empty() && error.path.is_none() && error.span.is_none() && error.help.is_none());
                assert_eq!(take_empty_counts(), (0, 0), "neither post-payment branch marker was reached");
                let first = owner.first();let remaining = bounded_output::active_remaining();
                let retry = Payer::Captured(&owner).reserve(0).unwrap_err();
                assert_eq!(retry, error, "all six raw refusal fields remain sticky");
                assert_eq!(owner.first(), first);assert_eq!(bounded_output::active_remaining(), remaining);
                Err(error)
            });
            assert!(result.is_err());drop(result);
            let first = crate::bounded_output::classifier_string::ClassifierLedger::capture().first_refusal().unwrap();
            assert_eq!(first.requested, charge);
            assert_eq!(first.remaining, charge - 1);
            assert_eq!(first.floor, 0);
            let error = owner.diagnostic();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(error.severity, crate::diagnostic::Severity::Error);
            assert_eq!(error.message, "canonical loan CFG storage reservation refused");
            assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
        });
        assert!(completed && refused);assert_eq!(receipt.unwrap().requested, charge);
        assert!(storage.ids.is_empty() && storage.liveness.is_empty() && storage.origins.is_empty());
        assert!(storage.is_current(&program));
        assert_eq!(storage._scratch.local, expected_local);
        assert_eq!(storage._scratch.liveness, expected_liveness);
        assert_eq!(storage._scratch.ids, expected_ids);
        assert_eq!(backing, (storage._scratch.local.as_ptr(), storage._scratch.local.capacity(),
            storage._scratch.liveness.as_ptr(), storage._scratch.liveness.capacity(),
            storage._scratch.ids.as_ptr(), storage._scratch.ids.capacity()));
        deferred.finish(&program).unwrap();
    }
}
