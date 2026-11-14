use super::*;
const SOURCE: &str = r#"module partition_checked;
@id("partition_checked.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(6usize);
 bytes_partition(buffer,3usize,2usize)
}
@id("partition_checked.main") fn main()->i64{
 let values=split();
 if vec_len<Bytes>(values)==3usize {7}else{0}
}
"#;
#[test]
fn original_source_resolves_only_through_the_private_current_owner() {
    let ordinary = crate::check(SOURCE, "partition_checked.spx").unwrap_err();
    assert!(ordinary.iter().any(|d| d.code == "SPX-T263"));
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&source)
        .iter()
        .all(|d| !d.severity.is_error()));
    // The result remains affine, and borrowing still needs a named carrier.
    let moved = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "let transferred=vec_into_iter<Bytes>(values);if vec_len<Bytes>(values)==3usize {7}else{0}",
    );
    let moved = crate::parse(&moved, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&moved)
        .iter()
        .any(|d| d.severity.is_error()));
    let unnamed = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "if vec_len<Bytes>(split())==3usize {7}else{0}",
    );
    let unnamed = crate::parse(&unnamed, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&unnamed)
        .iter()
        .any(|d| d.code == "SPX-O118"));
    assert!(ValidatedPartitionProgram::compile_source(&source, &mut 0).is_err());
    let mut work = usize::MAX;
    let owner = ValidatedPartitionProgram::compile_source(&source, &mut work).unwrap();
    assert!(std::ptr::eq(owner.source(), &source));
    owner.revalidate_compiler().unwrap();
    assert!(super::super::validate(owner.program()).is_err());
    let function = &owner.program().functions[0];
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("block")
    };
    let operands = crate::construction_fusion::partition::operation::typed(tail, &mut work)
        .unwrap()
        .unwrap();
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("call")
    };
    assert!(std::ptr::eq(operands.owner, &args[0]));
    assert!(function
        .cleanup_plan
        .status_sources
        .iter()
        .any(|status| status.id.lane == crate::cleanup_plan::StatusLane::OperationFailure));
    let mut probe = usize::MAX;
    owner.receive_current_graph(&mut probe).unwrap();
    let used = usize::MAX - probe;
    let mut exact = used;
    owner.receive_current_graph(&mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(owner.receive_current_graph(&mut short).is_err());
    assert_eq!(short, 0);

    let canonical = crate::format::canonical(&source);
    let canonical_ast = crate::parse(&canonical, "partition_checked.spx").unwrap();
    let ast_wire = crate::cache_codec::encode(&canonical_ast).unwrap();
    let restored_ast: crate::ast::Program = crate::cache_codec::decode(&ast_wire).unwrap();
    assert_eq!(crate::format::canonical(&restored_ast), canonical);
    assert_eq!(crate::cache_codec::encode(&restored_ast).unwrap(), ast_wire);

    // Source transport must go through the complete private compiler factory.
    // The frozen graph and public target facades cannot grant that authority.
    let canonical_owner =
        ValidatedPartitionProgram::compile_source(&canonical_ast, &mut { usize::MAX }).unwrap();
    let restored_owner =
        ValidatedPartitionProgram::compile_source(&restored_ast, &mut { usize::MAX }).unwrap();
    let selected_cache = canonical_owner.cache_projection().unwrap();
    assert_eq!(restored_owner.cache_projection().unwrap(), selected_cache);
    super::super::super::graph::verify_partition_json(
        &restored_ast,
        &crate::graph::to_partition_json(&canonical_ast, &mut { usize::MAX }, 8_388_608).unwrap(),
        &mut { usize::MAX },
        8_388_608,
    )
    .unwrap();
    let mut remaining = usize::MAX;
    let replayed =
        ValidatedPartitionProgram::replay_cache(&restored_ast, &selected_cache, &mut remaining)
            .unwrap();
    assert!(std::ptr::eq(replayed.source(), &restored_ast));
    assert_eq!(replayed.program(), restored_owner.program());
    let spent = usize::MAX - remaining;
    let mut exact = spent;
    ValidatedPartitionProgram::replay_cache(&restored_ast, &selected_cache, &mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = spent - 1;
    assert!(
        ValidatedPartitionProgram::replay_cache(&restored_ast, &selected_cache, &mut short)
            .is_err()
    );
    assert_eq!(short, 0);

    let hir_wire = crate::cache_codec::encode(restored_owner.program()).unwrap();
    assert_eq!(
        crate::cache_codec::encode(canonical_owner.program()).unwrap(),
        hir_wire
    );
    let edited = SOURCE.replace("buffer,3usize,2usize", "buffer,2usize,3usize");
    assert_ne!(SOURCE, edited);
    let edited_ast = crate::parse(&edited, "partition_checked.spx").unwrap();
    let edited_owner =
        ValidatedPartitionProgram::compile_source(&edited_ast, &mut { usize::MAX }).unwrap();
    let edited_cache = edited_owner.cache_projection().unwrap();
    let mut stale_hir = edited_cache[..super::cache::HEADER_BYTES].to_vec();
    stale_hir.extend_from_slice(&hir_wire);
    let mut forged_hir = restored_owner.program().clone();
    forged_hir.functions[0].cleanup_plan.status_sources.clear();
    let mut forged_cache = selected_cache[..super::cache::HEADER_BYTES].to_vec();
    forged_cache.extend_from_slice(&crate::cache_codec::encode(&forged_hir).unwrap());
    let mut trailing = selected_cache.clone();
    trailing.push(0);
    let mut wrong_version = selected_cache.clone();
    wrong_version[0] ^= 1;
    for (source, cache) in [
        (&edited_ast, selected_cache.as_slice()),
        (&edited_ast, stale_hir.as_slice()),
        (&restored_ast, forged_cache.as_slice()),
        (&restored_ast, hir_wire.as_slice()),
        (&restored_ast, wrong_version.as_slice()),
        (&restored_ast, trailing.as_slice()),
        (&restored_ast, &selected_cache[..selected_cache.len() - 1]),
    ] {
        let mut work = usize::MAX;
        assert!(ValidatedPartitionProgram::replay_cache(source, cache, &mut work).is_err());
        assert_eq!(work, 0);
    }
    let legacy_text = r#"module partition_checked;
@id("partition_checked.main") fn main()->i64{7}
"#;
    let legacy_source = crate::check(legacy_text, "partition_checked.spx").unwrap();
    let legacy_graph = crate::graph::to_json(&legacy_source).unwrap();
    crate::graph::verify_json(&legacy_source, &legacy_graph).unwrap();
    assert!(crate::graph::to_json(&restored_ast).is_err());
    assert!(crate::graph::verify_json(&restored_ast, &legacy_graph).is_err());

    let restored_hir: ResolvedProgram = crate::cache_codec::decode(&hir_wire).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored_hir).unwrap(), hir_wire);
    let expected = "byte partition requires the private checked source profile";
    let revision = crate::graph::revision(&restored_ast);
    let exports = [restored_hir.entrypoint.as_str().to_owned()];
    for diagnostic in [
        super::super::validate(&restored_hir).unwrap_err(),
        crate::graph::to_hir_json(&restored_hir, &revision).unwrap_err(),
        crate::codegen::emit_hir_c(&restored_hir).unwrap_err(),
        crate::codegen::emit_native_callable_admission(&restored_hir, &restored_hir.entrypoint)
            .err()
            .expect("frozen callable admission must refuse"),
        crate::wasm::emit_resolved_module_with_scalar_exports(&restored_hir, &exports).unwrap_err(),
    ] {
        assert!(diagnostic.message.contains(expected), "{diagnostic:?}");
    }

    let inherited = SOURCE.replace(
        "@id(\"partition_checked.main\")",
        r#"
@id("partition_checked.inherited") fn inherited(input:borrow Bytes)->usize {
 let alias=bytes_as_slice(input);byte_len(alias)
}
@id("partition_checked.main")"#,
    );
    let inherited = crate::parse(&inherited, "partition_checked.spx").unwrap();
    let inherited =
        ValidatedPartitionProgram::compile_source(&inherited, &mut { usize::MAX }).unwrap();
    let helper = inherited
        .program()
        .functions
        .iter()
        .find(|f| f.name == "inherited")
        .unwrap();
    assert!(
        helper.loan_plan.loans.is_empty(),
        "Borrow parameter descriptor cannot mint an Own-root loan"
    );
    assert_eq!(
        inherited
            .simultaneous_view_bound(helper, &mut { usize::MAX })
            .unwrap(),
        0
    );

    // Four live caller frames over four whole backings. Each caller retains
    // its 32 independent SliceView sites until after the nested call returns.
    // Each later named byte_len read has one canonical BorrowedCall site.
    // VecLen<Bytes> borrows its container; it adds no Byte-root physical loan.
    // The leaf has 31 views and 33/34 explicit byte_len calls, not a guessed
    // extra VecLen loan. The original 256/257 max-path boundary stays exact.
    for (extra_read, expected) in [(false, Some(256)), (true, None)] {
        let text = view_chain(extra_read);
        let ast = crate::parse(&text, "partition_view_bound.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let entry = &owner.program().functions[0];
        let counts: Vec<_> = owner
            .program()
            .functions
            .iter()
            .map(|f| f.loan_plan.loans.len())
            .collect();
        let slice_sites: Vec<_> = owner
            .program()
            .functions
            .iter()
            .map(|f| {
                f.loan_plan
                    .loans
                    .iter()
                    .filter(|loan| loan.cause == crate::loan_plan::LoanCause::SliceView)
                    .count()
            })
            .collect();
        let call_sites: Vec<_> = owner
            .program()
            .functions
            .iter()
            .map(|f| {
                f.loan_plan
                    .loans
                    .iter()
                    .filter(|loan| {
                        matches!(
                            loan.cause,
                            crate::loan_plan::LoanCause::BorrowedCall { argument: 0 }
                        )
                    })
                    .count()
            })
            .collect();
        assert_eq!(slice_sites, [32, 32, 32, 31]);
        assert_eq!(call_sites, [32, 32, 32, 33 + usize::from(extra_read)]);
        assert_eq!(counts, vec![64, 64, 64, 64 + usize::from(extra_read)]);
        let mut work = usize::MAX;
        match expected {
            Some(bound) => {
                assert_eq!(
                    owner.simultaneous_view_bound(entry, &mut work).unwrap(),
                    bound
                );
                let spent = usize::MAX - work;
                let mut exact = spent;
                assert_eq!(
                    owner.simultaneous_view_bound(entry, &mut exact).unwrap(),
                    bound
                );
                assert_eq!(exact, 0);
                let mut short = spent - 1;
                assert!(owner.simultaneous_view_bound(entry, &mut short).is_err());
                assert_eq!(short, 0);
                let clone = entry.clone();
                let mut clone_work = usize::MAX;
                assert!(owner
                    .simultaneous_view_bound(&clone, &mut clone_work)
                    .is_err());
                assert_eq!(clone_work, 0);
            }
            None => {
                let diagnostic = owner.simultaneous_view_bound(entry, &mut work).unwrap_err();
                assert!(diagnostic.message.contains("exceeds 256"), "{diagnostic:?}");
                assert_eq!(work, 0);
            }
        }
    }
}
#[test]
fn private_owner_refuses_foreign_carriers_and_actual_call_or_plan_mutations() {
    for source in [
        SOURCE.replace("fn split()", "fn split<T>()"),
        SOURCE.replace(
            "bytes_partition(buffer,3usize,2usize)",
            "bytes_partition(bytes_zeroed(6usize),3usize,2usize)",
        ),
    ] {
        let source = crate::parse(&source, "partition_checked.spx").unwrap();
        assert!(ValidatedPartitionProgram::compile_source(&source, &mut { usize::MAX }).is_err());
    }
    let alias = SOURCE
        .replace("let buffer=bytes_zeroed", "let original=bytes_zeroed")
        .replace(
            "bytes_partition(buffer,3usize,2usize)",
            "let buffer=original; bytes_partition(buffer,3usize,2usize)",
        );
    let alias = crate::parse(&alias, "partition_checked.spx").unwrap();
    ValidatedPartitionProgram::compile_source(&alias, &mut { usize::MAX }).unwrap();
    let child_helper = r#"module partition_checked;
@id("partition_checked.again") fn again(buffer:own Bytes)->Vec<Bytes>{bytes_partition(buffer,1usize,2usize)}
@id("partition_checked.main") fn main()->i64{
 let buffer=bytes_zeroed(4usize);
 let children=bytes_partition(buffer,2usize,2usize);
 match own iter_next<Bytes>(vec_into_iter<Bytes>(children)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>{let nested=again(item);if vec_len<Bytes>(nested)==1usize {7}else{0}},
 }
}
"#;
    let child_helper = crate::parse(child_helper, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&child_helper)
        .iter()
        .all(|d| !d.severity.is_error()));
    let error = ValidatedPartitionProgram::compile_source(&child_helper, &mut { usize::MAX })
        .err()
        .expect("child helper must refuse");
    assert!(error.message.contains("whole producer"), "{error:?}");
    let mut forbidden = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    forbidden.permits.push("foreign".to_owned());
    assert!(ValidatedPartitionProgram::compile_source(&forbidden, &mut { usize::MAX }).is_err());
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    for mutation in 0..4 {
        let mut owner =
            ValidatedPartitionProgram::compile_source(&source, &mut { usize::MAX }).unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut owner.program.functions[0].body.kind else {
            panic!("block")
        };
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            args,
            ..
        } = &mut tail.kind
        else {
            panic!("call")
        };
        match mutation {
            0 => *callee = DeclarationId::new("foreign.partition"),
            1 => type_arguments.push(ResolvedType::Bytes),
            2 => args[0].ownership = OwnershipMode::Borrow,
            _ => owner.program.functions[0]
                .cleanup_plan
                .status_sources
                .clear(),
        }
        let mut work = usize::MAX;
        assert!(
            owner.revalidate_compiler().is_err() || owner.receive_current_graph(&mut work).is_err(),
            "mutation {mutation}"
        );
        if mutation != 3 {
            assert!(owner.receive_current_graph(&mut work).is_err());
            assert_eq!(work, 0);
        }
        let mut bound_work = usize::MAX;
        assert!(owner
            .simultaneous_view_bound(&owner.program.functions[1], &mut bound_work)
            .is_err());
        assert_eq!(
            bound_work, 0,
            "failed compiler renewal must poison view receipt"
        );
    }
    let converted = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "i64_from_usize(vec_len<Bytes>(values))",
    );
    assert!(crate::check(&converted, "partition-conversion.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T263"));
    let ast = crate::parse(&converted, "partition-conversion.spx").unwrap();
    for mutation in 0..3 {
        let mut owner =
            ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut owner.program.functions[1].body.kind else {
            panic!("main block")
        };
        let ResolvedExprKind::Call { callee, args, .. } = &mut tail.kind else {
            panic!("conversion")
        };
        assert_eq!(callee.as_str(), crate::string_ops::I64_FROM_USIZE_ID);
        match mutation {
            0 => *callee = DeclarationId::new("foreign.i64_from_usize"),
            1 => args[0].ty = ResolvedType::I64,
            _ => args[0].ownership = OwnershipMode::Borrow,
        }
        // Actual receivers renew the full compiler proof before acquiring the
        // additional graph receipt. Type/mode mutations belong to that proof,
        // not to the graph's narrower admitted-type/call-universe predicate.
        assert!(
            owner.revalidate_compiler().is_err(),
            "compiler conversion mutation{mutation}"
        );
        if mutation == 0 {
            let mut work = usize::MAX;
            assert!(owner.receive_current_graph(&mut work).is_err());
            assert_eq!(work, 0, "foreign-call graph refusal is sticky");
        }
    }
    let unsupported = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "i64_from_u8(1u8)",
    );
    let ast = crate::parse(&unsupported, "partition-conversion.spx").unwrap();
    let error = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX })
        .err()
        .unwrap();
    assert!(error.message.contains("foreign calls"), "{error:?}");
}

fn view_chain(extra_read: bool) -> String {
    use std::fmt::Write;
    let mut source = String::from("module partition_view_bound;\n");
    for frame in 0..4 {
        let name = if frame == 0 {
            "main".to_owned()
        } else {
            format!("frame{frame}")
        };
        writeln!(
            source,
            "@id(\"partition_view_bound.{name}\") fn {name}()->i64 {{"
        )
        .unwrap();
        source.push_str("let buffer=bytes_zeroed(1usize);\n");
        let views = if frame == 3 { 31 } else { 32 };
        for site in 0..views {
            writeln!(source, "let view{site}=bytes_as_slice(buffer);").unwrap();
        }
        if frame < 3 {
            writeln!(source, "let value=frame{}();", frame + 1).unwrap();
        }
        for site in 0..views {
            writeln!(source, "let observed{site}=byte_len(view{site});").unwrap();
        }
        if frame == 3 {
            source.push_str(
                "let penultimate_observed=byte_len(view0);\nlet final_observed=byte_len(view0);\n",
            );
            if extra_read {
                source.push_str("let extra_observed=byte_len(view0);\n");
            }
            source.push_str("let children=bytes_partition(buffer,1usize,1usize);if vec_len<Bytes>(children)==1usize {7}else{0}\n");
        } else {
            source.push_str("value\n");
        }
        source.push_str("}\n");
    }
    source
}
