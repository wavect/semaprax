use super::super::super::prefix_stream::before_guard;
use super::*;

#[test]
fn full_scope_stream_strictly_reduces_allocations_and_invoice_at_depth_one_to_three() {
    for depth in [1, 2, 3] {
        let input = input(64, depth);
        let expected = expected(&input);
        for branches in [false, true] {
            for (repetitions, cow) in [(1, false), (3, true)] {
                let run = |old: bool| {
                    let _old = old.then(before_guard);
                    let (((), metrics), refused, paid, first) =
                        with_limit_usage_refusal(usize::MAX, || {
                            measure(|| {
                                receive(&input, &expected, branches, repetitions, cow).unwrap()
                            })
                        });
                    assert!(!refused && first.is_none());
                    complete(metrics, paid);
                    (metrics, paid)
                };
                let old = run(true);
                let current = run(false);
                eprintln!("PREFIX_STREAM depth={depth} branches={branches} repetitions={repetitions} old={old:?} current={current:?}");
                assert!(current.0.allocated < old.0.allocated);
                assert!(current.1 < old.1);
            }
        }
    }
}

#[test]
fn exact_short_zero_positive_floor_and_sticky_replay_drop_every_owned_value() {
    let input = input(24, 2);
    let expected = expected(&input);
    for branches in [false, true] {
        let mut diagnostics = Vec::new();
        for old in [true, false] {
            let _old = old.then(before_guard);
            let run = |limit, floor| {
                with_limit_usage_refusal(limit, || {
                    assert!(bounded_output::set_active_floor(floor));
                    measure(|| {
                        receive(&input, &expected, branches, 2, true)
                            .err()
                            .map(|error| {
                                assert!(!bounded_output::reserve_active_required(0));
                                (
                                    error.code,
                                    error.severity,
                                    error.message,
                                    error.path,
                                    error.span,
                                    error.help,
                                )
                            })
                    })
                })
            };
            let ((success, metrics), refused, exact, first) = run(usize::MAX, 0);
            assert!(success.is_none() && !refused && first.is_none());
            complete(metrics, exact);
            let mut errors = Vec::new();
            for (limit, floor, expected_success) in [
                (exact, 0, true),
                (exact - 1, 0, false),
                (0, 0, false),
                (exact + 97, 97, true),
                (exact + 96, 97, false),
            ] {
                let result = run(limit, floor);
                assert_eq!(result, run(limit, floor));
                let ((diagnostic, metrics), refused, paid, first) = result;
                assert_eq!(diagnostic.is_none(), expected_success);
                assert_eq!(refused, !expected_success);
                assert_eq!(first.is_none(), expected_success);
                complete(metrics, paid);
                errors.push(diagnostic);
            }
            diagnostics.push(errors);
        }
        assert_eq!(diagnostics[0], diagnostics[1]);
    }
}

#[test]
fn full_scope_empty_and_small_rejected_inputs_report_complete_costs() {
    for count in [0, 1, 2] {
        for state in [
            Availability::Available,
            Availability::MaybeMoved,
            Availability::Moved,
        ] {
            let id = ValueId::new("prefix.stream.cold".to_owned());
            let input = BTreeMap::from([(id, binding(count, 1, state))]);
            let expected = expected(&input);
            for branches in [false, true] {
                for old in [true, false] {
                    let _old = old.then(before_guard);
                    let (((), metrics), refused, paid, first) =
                        with_limit_usage_refusal(usize::MAX, || {
                            measure(|| receive(&input, &expected, branches, 1, true).unwrap())
                        });
                    assert!(!refused && first.is_none());
                    complete(metrics, paid);
                    eprintln!("PREFIX_STREAM_COLD count={count} state={state:?} branches={branches} old={old} paid={paid} {metrics:?}");
                }
            }
        }
    }
}

#[test]
fn real_many_field_hir_keeps_semantics_and_reduces_full_invoice_and_allocation() {
    let mut source = String::from("module prefix.stream.real;\n@id(\"Packet\") record Packet {");
    for n in 0..32 {
        source.push_str(&format!("@id(\"field{n}\") field{n}:Bytes,"));
    }
    source.push_str("}\n@id(\"take\") fn take(value:own Bytes)->usize {byte_len(bytes_as_slice(value))}\n@id(\"inspect\") fn inspect(packet:own Packet,flag:bool)->i64 {");
    for n in 0..32 {
        source.push_str(&format!("let branch{n}=if flag {{let used=take(packet.field{n});0}}else{{let used=take(packet.field{n});0}};"));
    }
    source.push_str("0}\n@id(\"main\") fn main()->i64 {0}\n");
    let ast = crate::check(&source, "prefix-stream-real.spx").unwrap();
    let program = {
        let _old = before_guard();
        crate::hir::resolve(&ast).unwrap()
    };
    assert_eq!(crate::hir::resolve(&ast).unwrap(), program);
    let mut observations = Vec::new();
    for old in [true, false] {
        let _old = old.then(before_guard);
        let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| crate::hir::validate(&program).unwrap())
        });
        assert!(!refused && first.is_none());
        complete(metrics, paid);
        observations.push((metrics, paid));
    }
    eprintln!(
        "PREFIX_STREAM_HIR old={:?} current={:?}",
        observations[0], observations[1]
    );
    assert!(observations[1].0.allocated < observations[0].0.allocated);
    assert!(observations[1].1 < observations[0].1);
    let mut hostile = program.clone();
    hostile.entrypoint = DeclarationId::new("missing.prefix.entry");
    let expected = {
        let _old = before_guard();
        crate::hir::validate(&hostile)
    };
    assert!(expected.is_err());
    assert_eq!(crate::hir::validate(&hostile), expected);
}
