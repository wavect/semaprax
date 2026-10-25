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
    for index in 0..192 {
        if index != 0 {
            many_source.push('+');
        }
        many_source.push_str("byte_len(view)");
    }
    many_source.push_str("}\n@id(\"main\") fn main()->i64 {0}\n");
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
        "b0431b45c4448896a962c8bfb7be0cb93f2b648c976eeae81865c44741cfc238"
    );
    let snapshot: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(snapshot["base"], "8086b7e5047e7199c8fccb2f07c49c3513982d2a");
    let cases: &[(&str, &str)] = &[
        ("docs/ARCHITECTURE.md", include_str!("../../../../../docs/ARCHITECTURE.md")),
        ("docs/SHARED-LOAN-PLAN-V1.md", include_str!("../../../../../docs/SHARED-LOAN-PLAN-V1.md")),
        ("src/hir/canonical_loan_storage/caller.rs", include_str!("../../caller.rs")),
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
        ("src/hir/canonical_loan_storage/caller/stream.rs", include_str!("../stream.rs")),
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
        let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
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
