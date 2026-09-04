//! Builder ledger proofs: cumulative limits, the post-HIR reservation
//! envelopes, and the named scratch bounds they may not exceed.

use super::*;

#[test]
fn cumulative_builder_limit_is_exact_and_cannot_be_widened() {
    let (program, spec) = fixture();
    let (mut low, mut high) = (0_usize, MAX_BUILDER_BYTES);
    while low < high {
        let middle = low + (high - low) / 2;
        if prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let minimum = low;
    assert!(minimum > 0 && minimum <= MAX_BUILDER_BYTES);
    let prepared =
        prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), minimum).unwrap();
    assert_eq!(prepared.canonical_spec, spec);
    let error =
        match prepare_native_rust_interop_with_test_limit(&program, spec.as_bytes(), minimum - 1) {
            Ok(_) => panic!("one-under builder limit was accepted"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );

    let widened = std::panic::catch_unwind(|| {
        let _ = prepare_native_rust_interop_with_test_limit(
            &program,
            spec.as_bytes(),
            MAX_BUILDER_BYTES + 1,
        );
    });
    assert!(widened.is_err());
}

#[test]
fn full_bundle_builder_limit_is_cumulative_exact_and_cannot_be_widened() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-builder-exact-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    let output = root.join("bundle");
    let prepare_probe =
        |limit: usize| prepare_phase_b_with_test_limit(&program, spec.as_bytes(), &output, limit);
    let build_probe = |limit: usize| {
        std::fs::create_dir(&root).unwrap();
        let result = build_native_rust_interop_bundle_with_test_limit(
            &program,
            spec.as_bytes(),
            &output,
            limit,
        );
        std::fs::remove_dir_all(&root).unwrap();
        result.map(|_| ())
    };

    let (mut low, mut high) = (0_usize, MAX_BUILDER_BYTES);
    while low < high {
        let middle = low + (high - low) / 2;
        match prepare_probe(middle) {
            Ok(()) => high = middle,
            Err(error) => {
                assert_eq!(error.len(), 1);
                assert_eq!(error[0].code, "SPX-B109");
                assert_eq!(
                    error[0].message,
                    "Native Rust Interop max_builder_bytes exceeds 33554432"
                );
                low = middle + 1;
            }
        }
    }
    let minimum = low;
    assert!(minimum > 0 && minimum <= MAX_BUILDER_BYTES);
    prepare_probe(minimum).unwrap();
    let error = prepare_probe(minimum - 1).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(
        error[0].message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
    build_probe(minimum).unwrap();

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-builder-widen-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let widened = std::panic::catch_unwind(|| {
        let _ = build_native_rust_interop_bundle_with_test_limit(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            MAX_BUILDER_BYTES + 1,
        );
    });
    std::fs::remove_dir_all(&root).unwrap();
    assert!(widened.is_err());
}

#[test]
fn hir_capacity_layout_constants_are_bound_to_root_const_assertions() {
    // The resolver is split across submodules; its layout pins live beside the
    // frame machinery they describe, so bind the root and the resolver files.
    let hir_resolver = concat!(
        include_str!("../../../../../src/hir.rs"),
        include_str!("../../../../../src/hir/resolve_class.rs"),
        include_str!("../../../../../src/hir/resolve_expr.rs"),
        include_str!("../../../../../src/hir/resolve_expr_frame.rs"),
        include_str!("../../../../../src/hir/resolve_expr_reference.rs"),
        include_str!("../../../../../src/hir/resolve_pattern.rs"),
        include_str!("../../../../../src/hir/resolve_program.rs"),
        include_str!("../../../../../src/hir/resolve_statement.rs"),
    );
    let hir_validator = concat!(
        include_str!("../../../../../src/hir/validation.rs"),
        include_str!("../../../../../src/hir/validation/borrowed_str.rs"),
        include_str!("../../../../../src/hir/validation/type_profiles.rs"),
        include_str!("../../../../../src/hir/validation/unsafe_scan.rs"),
    );
    let verifier = include_str!("../../../../../src/source_verify.rs");
    let cleanup = include_str!("../../../../../src/cleanup.rs");
    let lower = concat!(
        include_str!("../../../../../src/cleanup_plan/build.rs"),
        include_str!("../../../../../src/cleanup_plan/build/schema.rs"),
        include_str!("../../../../../src/cleanup_plan/build/record_destructure.rs"),
        include_str!("../../../../../src/cleanup_plan/build/record_destructure/update.rs"),
    );
    let calls = include_str!("../../../../../src/call_index.rs");
    for (source, expected) in [
        (hir_resolver, "size_of::<Frame<'static>>() == 592"),
        (hir_validator, "size_of::<Frame<'static>>() == 288"),
        (verifier, "size_of::<VerifierFrame<'static>>() == 320"),
        (verifier, "size_of::<VariantMatchState<'static>>() == 312"),
        (cleanup, "size_of::<Frame<'static>>() == 40"),
        (cleanup, "size_of::<Frame<'static>>() == 24"),
        (lower, "size_of::<Frame<'static>>() == 368"),
        (calls, "size_of::<Frame<'static>>() == 16"),
    ] {
        assert!(
            source.contains(expected),
            "missing root layout pin `{expected}`"
        );
    }
}

#[test]
fn hir_complete_reservation_is_exact_and_one_less_prevents_resolution() {
    let (program, _) = fixture();
    let canonical = crate::format::canonical(&program);
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
    assert_eq!(capacity.retained_upper, 50_035);
    assert_eq!(capacity.scratch_upper, 16_170);
    assert_eq!(
        capacity.phase_peaks(),
        [5_028, 15_620, 4_900, 3_488, 5_792, 3_456, 16_170, 1_032]
    );
    assert_eq!(capacity.complete().unwrap(), 66_205);
    assert_eq!(
        capacity.scratch_upper,
        capacity.phase_peaks().into_iter().max().unwrap(),
        "scratch must equal the largest sequential phase"
    );
    let complete = capacity.complete().unwrap();
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| counts.set([0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(complete - 1, || {
            let budget = reserve_temporary_exact(complete)?;
            note_hir_resolve_pass();
            let _ = hir::resolve(&program).map_err(|_| b107("selected identity missing"))?;
            drop(budget);
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| assert_eq!(counts.get(), [0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| assert_eq!(water.get(), [0; 3]));

    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| counts.set([0; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
    let (result, overflowed, _) = crate::bounded_output::with_limit_usage(complete, || {
        let budget = reserve_temporary_exact(complete)?;
        note_hir_resolve_pass();
        let resolved = hir::resolve(&program).map_err(|_| b107("selected identity missing"))?;
        reset_closure_capacity_high_water();
        let (closure, _) = selected_closure(&resolved, &["interop.add".to_owned()])?;
        validate_native_rust_expression_budget_for_closure(&closure, true)?;
        validate_selected_scalar_closure(&closure)?;
        validate_native_unit_discard_bindings(&closure)?;
        assert!(closure_capacity_high_water() <= capacity.phase_peaks()[6]);
        drop(budget);
        Ok::<_, Diagnostic>(())
    });
    result.unwrap();
    assert!(!overflowed);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| assert_eq!(counts.get(), [1; 4]));
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| {
        assert!(water.get().into_iter().all(|bytes| bytes > 0));
    });
}

#[test]
fn post_hir_nontransfer_reservation_precedes_all_fact_and_render_work() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let complete = capacity.complete().unwrap();
    let transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let reservation = complete.checked_sub(transfer).unwrap();

    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(reservation - 1, || {
            let _budget = reserve_temporary_exact(reservation)?;
            note_post_hir_facts_entry();
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 0));
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| assert_eq!(water.get(), 0));

    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(reservation, || {
            let budget = reserve_temporary_exact(reservation)?;
            note_post_hir_facts_entry();
            drop(budget);
            Ok::<_, Diagnostic>(())
        });
    result.unwrap();
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn post_hir_spec_transfer_is_single_charged_across_target_triple_lengths() {
    fn terms(triple: &str) -> [usize; 5] {
        with_test_target(
            Target {
                triple: triple.to_owned(),
                pointer_width: 64,
                endian: "little".to_owned(),
                panic_strategy: "unwind".to_owned(),
                thread_policy: "same_thread".to_owned(),
            },
            || {
                let (program, canonical_spec) = fixture();
                POST_HIR_AUTHORITY_TRANSFER_TERMS.with(|terms| terms.set([0; 5]));
                prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
                POST_HIR_AUTHORITY_TRANSFER_TERMS.with(std::cell::Cell::get)
            },
        )
    }

    // [complete formula, moved Spec ownership, net facts reservation,
    //  new persistent facts, total persistent Prepared ownership]
    let apple = terms("aarch64-apple-darwin");
    let linux = terms("x86_64-unknown-linux-gnu");
    for observed in [apple, linux] {
        assert!(observed.into_iter().all(|value| value > 0));
        assert_eq!(observed[0] - observed[1], observed[2]);
        assert_eq!(observed[4] - observed[1], observed[3]);
    }
    assert_eq!(linux[0] - apple[0], 4);
    assert_eq!(linux[1] - apple[1], 4);
    assert_eq!(linux[2], apple[2]);
    assert_eq!(linux[3], apple[3]);
    assert_eq!(linux[4] - apple[4], 4);
}

#[test]
fn windows_target_phase_a_preparation_stays_inside_the_builder_ledger() {
    with_test_target(
        Target {
            triple: "x86_64-pc-windows-msvc".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, canonical_spec) = fixture();
            let (result, overflowed, consumed) =
                crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
                    prepare_native_rust_interop_bounded(&program, canonical_spec.as_bytes())
                });
            assert!(
                !overflowed,
                "Windows-target phase A overflowed; consumed={consumed}",
            );
            result.unwrap();
        },
    );
}

#[test]
fn post_hir_spec_transfer_capacity_slack_does_not_consume_scratch_authority() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let mut spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    let base = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let base_transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let digest = spec.source_revision.clone().unwrap();
    let requested_capacity = digest.len() + 37;
    let mut over_capacity_digest = String::with_capacity(requested_capacity);
    over_capacity_digest.push_str(&digest);
    assert!(over_capacity_digest.capacity() > over_capacity_digest.len());
    spec.source_revision = Some(over_capacity_digest);

    let hostile = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let hostile_transfer = prepared_spec_transfer_capacity(&spec).unwrap();
    let transfer_delta = hostile_transfer.checked_sub(base_transfer).unwrap();
    assert!(transfer_delta > 0);
    assert_eq!(
        hostile.complete().unwrap() - base.complete().unwrap(),
        transfer_delta,
    );
    assert_eq!(
        hostile.complete().unwrap() - hostile_transfer,
        base.complete().unwrap() - base_transfer,
    );
}

#[test]
fn final_artifact_sinks_reject_one_less_before_output_allocation() {
    let (program, canonical_spec) = fixture();
    let canonical_source = crate::format::canonical(&program);
    let spec =
        parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source).unwrap();
    let prepared = prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let status_domains = prepared
        .imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| count.set(0));
    let descriptor = render_descriptor_with_limit(
        &spec,
        &prepared.hir_digest,
        &status_domains,
        &prepared.exports,
        &prepared.imports,
        prepared.descriptor.len() - 1,
    )
    .unwrap_err();
    let header = generate_header_with_limit(
        &prepared.exports,
        &prepared.imports,
        prepared.generated_header.len() - 1,
    )
    .unwrap_err();
    let generated_c = render_exact_artifact(
        "max_generated_c_bytes",
        prepared.generated_c.len() - 1,
        |sink| generate_c_into(sink, &spec, &closure, &prepared.exports, &prepared.imports),
    )
    .unwrap_err();
    let rust_combined = prepared
        .generated_rust
        .len()
        .checked_add(prepared.private_ffi_source.len())
        .unwrap();
    let rust_aggregate_one_less = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        rust_combined - 1,
    )
    .unwrap_err();
    let rust_first_sink_one_less = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        prepared.generated_rust.len() - 1,
    )
    .unwrap_err();
    for diagnostic in [
        descriptor,
        header,
        generated_c,
        rust_aggregate_one_less,
        rust_first_sink_one_less,
    ] {
        assert_eq!(diagnostic.code, "SPX-B109");
    }
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| assert_eq!(count.get(), 0));

    assert_eq!(
        render_descriptor_with_limit(
            &spec,
            &prepared.hir_digest,
            &status_domains,
            &prepared.exports,
            &prepared.imports,
            prepared.descriptor.len(),
        )
        .unwrap(),
        prepared.descriptor
    );
    assert_eq!(
        generate_header_with_limit(
            &prepared.exports,
            &prepared.imports,
            prepared.generated_header.len(),
        )
        .unwrap(),
        prepared.generated_header
    );
    assert_eq!(
        render_exact_artifact(
            "max_generated_c_bytes",
            prepared.generated_c.len(),
            |sink| generate_c_into(sink, &spec, &closure, &prepared.exports, &prepared.imports,),
        )
        .unwrap(),
        prepared.generated_c
    );
    let exact_rust = generate_rust_artifacts_with_limit(
        &spec,
        &prepared.exports,
        &prepared.imports,
        rust_combined,
    )
    .unwrap();
    assert_eq!(exact_rust.0, prepared.generated_rust);
    assert_eq!(exact_rust.1, prepared.private_ffi_source);
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| assert_eq!(count.get(), 5));
}

#[test]
fn post_hir_named_phase_envelopes_cover_representative_and_depth_512_c() {
    fn measure(program: &Program, spec: &Spec) -> ([usize; 4], [usize; 3]) {
        let canonical_source = crate::format::canonical(program);
        let canonical_spec = render_spec(spec);
        let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let disposal_frames = hir_pre_resolve_capacity(program, canonical_source.len(), &mut scan)
            .unwrap()
            .disposal_frames;
        let resolved = hir::resolve(program).unwrap();
        let resolved = ResolvedProgramOwner::new(
            resolved,
            Vec::with_capacity(disposal_frames),
            disposal_frames,
        );
        let (closure, _) = selected_closure(resolved.program(), &spec.exports).unwrap();
        let capacity = post_hir_facts_capacity(
            canonical_source.len(),
            canonical_spec.len(),
            resolved.program(),
            &closure,
            spec,
        )
        .unwrap();
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
        prepare_native_rust_interop(program, canonical_spec.as_bytes()).unwrap();
        let actual = [
            POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
            POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
            POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        ];
        assert!(
            actual[0]
                <= capacity
                    .retained_upper
                    .checked_add(capacity.facts_scratch_upper)
                    .unwrap()
        );
        assert!(actual[1] <= capacity.render_scratch_upper);
        assert!(actual[2] <= capacity.replay_scratch_upper);
        let measured = (
            [
                capacity.retained_upper,
                capacity.facts_scratch_upper,
                capacity.render_scratch_upper,
                capacity.replay_scratch_upper,
            ],
            actual,
        );
        drop(resolved);
        measured
    }

    // This historical evidence tuple was authorized for the Apple-arm
    // target. Freeze that target explicitly so host triple length cannot
    // silently repin a target-specific retained-allocation census.
    with_test_target(
        Target {
            triple: "aarch64-apple-darwin".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, canonical_spec) = fixture();
            let canonical_source = crate::format::canonical(&program);
            let spec =
                parse_spec_with_source(&program, canonical_spec.as_bytes(), &canonical_source)
                    .unwrap();
            let representative = measure(&program, &spec);

            let mut deep = program;
            let function = deep
                .functions
                .iter_mut()
                .find(|function| function.stable_id == "interop.add")
                .unwrap();
            for _ in 0..MAX_SEMANTIC_EXPRESSION_DEPTH - 4 {
                let expression = std::mem::replace(
                    &mut function.body,
                    crate::ast::Expr {
                        kind: crate::ast::ExprKind::Int(0),
                        span: crate::ast::Span::default(),
                    },
                );
                function.body = crate::ast::Expr {
                    span: expression.span,
                    kind: crate::ast::ExprKind::Unary {
                        op: crate::ast::UnaryOp::Neg,
                        value: Box::new(expression),
                    },
                };
            }
            validate_native_rust_source_expression_budget(&deep).unwrap();
            let deep_source = crate::format::canonical(&deep);
            let deep_spec = Spec {
                module: deep.module.clone(),
                source_revision: Some(domain_digest(SOURCE_DOMAIN, deep_source.as_bytes())),
                target: current_target().unwrap(),
                exports: vec!["interop.add".to_owned()],
                imports: vec!["host.add".to_owned()],
                capabilities: vec!["host.math".to_owned()],
            };
            let deep = measure(&deep, &deep_spec);

            assert_eq!(
                [representative, deep],
                [
                    (
                        [1_630, 115_266, 8_390_881, 8_390_881],
                        [116_499, 4_195_020, 4_195_020]
                    ),
                    (
                        [1_630, 115_266, 8_447_777, 8_447_777],
                        [116_499, 4_251_916, 4_251_916]
                    ),
                ],
                "named phase formula or observed high-water pins drifted"
            );
        },
    );
}

#[test]
fn post_hir_facts_cross_product_maxima_stay_inside_named_scratch() {
    let capabilities = (0..MAX_IMPORTS)
        .map(|index| format!("cap.c{index:02}"))
        .collect::<Vec<_>>();
    let capability_list = capabilities.join(", ");
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = (0..MAX_PARAMETERS)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!(
        "module post.cross_product; permit {{ {capability_list} }} @id(\"host.cross\") interface HostCross permits {{ {capability_list} }} {{ "
    );
    for index in 0..MAX_IMPORTS {
        write!(
                source,
                "@id(\"import.{index:02}\") import rust fn import_{index:02}({parameters}) -> i64 effects {{ cap.c{index:02} }} failure status \"status.{index:02}\"; "
            )
            .unwrap();
    }
    source.push_str("} ");
    let call_sum = (0..MAX_IMPORTS)
        .map(|index| format!("import_{index:02}({arguments})"))
        .collect::<Vec<_>>()
        .join(" + ");
    write!(
            source,
            "@id(\"bridge.all\") fn bridge_all({parameters}) -> i64 uses {{ {capability_list} }} {{ {call_sum} }} "
        )
        .unwrap();
    for index in 0..MAX_EXPORTS {
        write!(
                source,
                "@id(\"export.{index:02}\") fn export_{index:02}({parameters}) -> i64 uses {{ {capability_list} }} {{ bridge_all({arguments}) }} "
            )
            .unwrap();
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }");
    let program = crate::parse(&source, Path::new("post-hir-cross-product.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes())),
        target: current_target().unwrap(),
        exports: (0..MAX_EXPORTS)
            .map(|index| format!("export.{index:02}"))
            .collect(),
        imports: (0..MAX_IMPORTS)
            .map(|index| format!("import.{index:02}"))
            .collect(),
        capabilities,
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let mut hir_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let hir_capacity =
        hir_pre_resolve_capacity(&program, canonical_source.len(), &mut hir_scan).unwrap();
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "cross-product prepare failed: {diagnostics:?}; source={}, spec={}, hir={}, retained={}, facts={}, render={}, replay={}, complete={}",
                    canonical_source.len(),
                    canonical_spec.len(),
                    hir_capacity.complete().unwrap(),
                    capacity.retained_upper,
                    capacity.facts_scratch_upper,
                    capacity.render_scratch_upper,
                    capacity.replay_scratch_upper,
                    capacity.complete().unwrap()
                )
            },
        );
    let actual = [
        POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
        POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get),
    ];
    let facts_scratch_actual = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(
        actual[0]
            <= capacity
                .retained_upper
                .checked_add(capacity.facts_scratch_upper)
                .unwrap(),
        "facts total-live / retained+scratch: {}/{}, all={actual:?}",
        actual[0],
        capacity.retained_upper + capacity.facts_scratch_upper
    );
    assert!(facts_scratch_actual > 0);
    assert!(facts_scratch_actual <= capacity.facts_scratch_upper);
    assert!(
        actual[1] <= capacity.render_scratch_upper,
        "render actual/formula: {}/{}, all={actual:?}",
        actual[1],
        capacity.render_scratch_upper
    );
    assert!(
        actual[2] <= capacity.replay_scratch_upper,
        "replay actual/formula: {}/{}, all={actual:?}",
        actual[2],
        capacity.replay_scratch_upper
    );
}

#[test]
fn post_hir_facts_zero_entry_collections_have_zero_backing_and_stay_bounded() {
    let empty_strings = Vec::<String>::new();
    let empty_pairs = Vec::<(String, String)>::new();
    let empty_ordinals = Vec::<u16>::new();
    let empty_set = BTreeSet::<String>::new();
    assert_eq!(
        checked_owned_string_vec(&empty_strings, empty_strings.capacity()),
        Some(0)
    );
    assert_eq!(checked_owned_string_pairs(&empty_pairs), Some(0));
    assert_eq!(checked_u16_vec(&empty_ordinals), Some(0));
    assert_eq!(checked_owned_string_set(&empty_set), Some(0));

    let source = "module post.zero; @id(\"zero.export\") fn export(value: i64) -> i64 { value } @id(\"app.main\") fn main() -> i64 { 0 }";
    let program = crate::parse(source, Path::new("post-hir-zero.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["zero.export".to_owned()],
        imports: Vec::new(),
        capabilities: Vec::new(),
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let total = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
    let scratch = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(total <= capacity.retained_upper + capacity.facts_scratch_upper);
    assert!(scratch <= capacity.facts_scratch_upper);

    // Unselected source text does not multiply any post-HIR owned
    // collection. A near-limit source with this same one-function closure
    // therefore has the same fieldwise facts authority and stays admitted.
    let near_max_source = post_hir_facts_capacity(
        MAX_SOURCE_BYTES,
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    assert_eq!(near_max_source.retained_upper, capacity.retained_upper);
    assert_eq!(
        near_max_source.facts_scratch_upper,
        capacity.facts_scratch_upper
    );
    assert!(near_max_source.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn post_hir_dense_fan_in_duplicates_and_all_interface_imports_stay_bounded() {
    let mut source = String::from(
        "module post.fanin; permit { cap.fan } @id(\"host.fan\") interface HostFan permits { cap.fan } { @id(\"import.fan\") import rust fn host_fan() -> i64 effects { cap.fan } failure status \"status.fan\"; } @id(\"host.unused\") interface HostUnused permits { cap.fan } { ",
    );
    for index in 0..24 {
        write!(source, "@id(\"unused.{index:02}\") import rust fn unused_{index:02}() -> i64 effects {{ cap.fan }} failure status \"status.unused.{index:02}\"; ").unwrap();
    }
    source
        .push_str("} @id(\"fanin.leaf\") fn fanin_leaf() -> i64 uses { cap.fan } { host_fan() } ");
    for index in 0..16 {
        write!(source, "@id(\"fanin.mid.{index:02}\") fn fanin_mid_{index:02}() -> i64 uses {{ cap.fan }} {{ fanin_leaf() + fanin_leaf() + fanin_leaf() }} ").unwrap();
    }
    let fan_in = (0..16)
        .map(|index| format!("fanin_mid_{index:02}()"))
        .collect::<Vec<_>>()
        .join(" + ");
    write!(source, "@id(\"fanin.export\") fn fanin_export() -> i64 uses {{ cap.fan }} {{ {fan_in} }} @id(\"app.main\") fn main() -> i64 {{ 0 }}").unwrap();
    let program = crate::parse(&source, Path::new("post-hir-fanin.spx")).unwrap();
    let canonical_source = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["fanin.export".to_owned()],
        imports: vec!["import.fan".to_owned()],
        capabilities: vec!["cap.fan".to_owned()],
    };
    let canonical_spec = render_spec(&spec);
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();
    let capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        &resolved,
        &closure,
        &spec,
    )
    .unwrap();
    let census = traversal_call_site_census(&closure).unwrap();
    assert!(census.function_sites > closure.len());
    assert_eq!(
        capacity.traversal_pending_capacity,
        census.function_sites + 1
    );
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(0));
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(0));
    prepare_native_rust_interop(&program, canonical_spec.as_bytes()).unwrap();
    let total = POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(std::cell::Cell::get);
    let scratch = POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(std::cell::Cell::get);
    assert!(total <= capacity.retained_upper + capacity.facts_scratch_upper);
    assert!(scratch <= capacity.facts_scratch_upper);
}

#[test]
fn serde_json_lock_and_near_max_escaped_payload_match_parser_contract() {
    assert!(include_str!("../../../Cargo.toml").contains("serde_json = \"=1.0.151\""));
    let serde_package = include_str!("../../../../../Cargo.lock")
        .split("[[package]]")
        .find(|package| package.lines().any(|line| line == "name = \"serde_json\""))
        .expect("serde_json package is locked");
    for expected in [
        "version = \"1.0.151\"",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"",
        "checksum = \"c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14\"",
    ] {
        assert!(serde_package.lines().any(|line| line == expected));
    }
    let mut encoded = String::with_capacity(MAX_DESCRIPTOR_BYTES);
    encoded.push_str("{\"escaped\":\"");
    while encoded.len() + "\\u0061\"}".len() <= MAX_DESCRIPTOR_BYTES {
        encoded.push_str("\\u0061");
    }
    encoded.push_str("\"}");
    assert!(encoded.len() >= MAX_DESCRIPTOR_BYTES - 6);
    let value: Value = serde_json::from_str(&encoded).unwrap();
    let string_payload = checked_json_string_payload(&value).unwrap();
    assert!(string_payload <= encoded.len());
    assert!(encoded.len().checked_mul(2).unwrap() <= MAX_DESCRIPTOR_BYTES * 2);
}
