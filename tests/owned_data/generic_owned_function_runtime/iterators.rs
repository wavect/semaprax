//! Consuming iterators preserve order and settle every successor exactly once.
use super::collections;

fn source() -> String {
    source_with_reconstruction(false)
}

fn source_with_reconstruction(reconstruct: bool) -> String {
    let mut source = String::from("module test.owning_iterators;\n");
    let mut calls = Vec::new();
    for (ty, first, second) in [
        ("i64", "-17", "29"),
        ("i32", "-17i32", "29i32"),
        ("u8", "17u8", "29u8"),
        ("usize", "17usize", "29usize"),
        ("char", "'a'", "'z'"),
        ("f32", "-1.25f32", "2.5f32"),
        ("f64", "-1.25", "2.5"),
        ("bool", "false", "true"),
    ] {
        let rebuild_function = if reconstruct {
            format!(
                r#"@id("iter.rebuild.{ty}") fn rebuild_{ty}(value:own IterStep<{ty}>)->IterStep<{ty}> {{
 match own value {{
  IterStep::Done {{}} => IterStep<{ty}>::Done {{}},
  IterStep::Yield {{item,rest}} => IterStep<{ty}>::Yield {{item:item,rest:rest}},
 }}
}}
"#
            )
        } else {
            String::new()
        };
        let step_call = if reconstruct {
            format!("rebuild_{ty}(iter_next<{ty}>(early))")
        } else {
            format!("iter_next<{ty}>(early)")
        };
        let reconstruction_check = if reconstruct {
            format!(
                r#"{{
                let rebuilt_empty=rebuild_{ty}(IterStep<{ty}>::Done {{}});
                let empty_rebuild=match own rebuilt_empty {{
                    IterStep::Done {{}} => true,
                    IterStep::Yield {{item:unexpected_item,rest:unexpected_rest}} => false,
                }};
                let yielded_ok=match own unconsumed_step {{
                    IterStep::Done {{}} => false,
                    IterStep::Yield {{item:rebuilt_item,rest:rebuilt_rest}} => rebuilt_item=={second},
                }};
                empty_rebuild && yielded_ok
            }}"#
            )
        } else {
            "true".to_owned()
        };
        source.push_str(&format!(
            r#"
@id("iter.finish.{ty}") fn finish_{ty}(value:own Iter<{ty}>)->i64 {{
 match own iter_next<{ty}>(value) {{
  IterStep::Done {{}} => 1,
  IterStep::Yield {{item,rest}} => 0,
 }}
}}
{rebuild_function}
@id("iter.run.{ty}") fn run_{ty}()->i64 {{
 let empty=vec_into_iter<{ty}>(vec_with_capacity<{ty}>(0usize));
 let empty_ok=finish_{ty}(empty);
 let first_vec=vec_push<{ty}>(vec_with_capacity<{ty}>(2usize),{first});
 let values=vec_push<{ty}>(first_vec,{second});
 let iter=vec_into_iter<{ty}>(values);
 let ordered=match own iter_next<{ty}>(iter) {{
  IterStep::Done {{}} => 0,
  IterStep::Yield {{item,rest}} => {{
   let first_ok=item=={first};
   match own iter_next<{ty}>(rest) {{
    IterStep::Done {{}} => 0,
    IterStep::Yield {{item:second_item,rest:successor}} => {{
     let second_ok=second_item=={second};
     let exhausted=finish_{ty}(successor);
     if first_ok && second_ok && exhausted==1 {{1}}else{{0}}
    }},
   }}
  }},
 }};
 let early=vec_into_iter<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{second}));
 let unconsumed_step={step_call};
 let reconstruction_ok={reconstruction_check};
 let discarded=vec_into_iter<{ty}>(vec_push<{ty}>(vec_with_capacity<{ty}>(1usize),{first}));
 if empty_ok==1 && ordered==1 && reconstruction_ok {{1}}else{{0}}
}}
"#
        ));
        calls.push(format!("run_{ty}()"));
    }
    source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    ));
    source
}

#[test]
fn owning_iterators_all_scalars_order_empty_exhaustion_and_early_drop() {
    collections::run_source_value(&source(), 8);
}

#[test]
fn owning_iterators_contract_failure_settles_live_successor_and_pending_step() {
    let source = source();
    let source = source.replace(
        "if empty_ok==1 && ordered==1 && reconstruction_ok {1}else{0}",
        "let _ = reject(); if empty_ok==1 && ordered==1 && reconstruction_ok {1}else{0}",
    );
    let source = source + "\n@id(\"iter.reject\") fn reject()->i64 requires false {0}\n";
    collections::run_source(&source, 1);
}

#[test]
fn owning_iterators_cleanup_v10_rejects_downgrade_and_conditional_owner_forgery() {
    let ast = semaprax::check(&source(), "iterator-cleanup.spx").unwrap();
    let program = semaprax::hir::resolve(&ast).unwrap();
    semaprax::hir::validate(&program).unwrap();
    let index = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "iter.run.i64")
        .unwrap();
    let function = &program.functions[index];
    assert_eq!(function.cleanup_plan.schema, "semaprax.cleanup-plan.v10");
    assert!(function
        .cleanup
        .flags
        .iter()
        .any(|flag| flag.lifecycle.as_str() == "core.iter.drop"));
    assert!(function.cleanup.flags.iter().any(|flag| flag
        .place
        .projections
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        == ["core.iter-step.yield", "core.iter-step.yield.rest"]));
    for schema in [
        "semaprax.cleanup-plan.v2",
        "semaprax.cleanup-plan.v6",
        "semaprax.cleanup-plan.v9",
    ] {
        let mut hostile = program.clone();
        hostile.functions[index].cleanup_plan.schema = schema;
        assert!(semaprax::hir::validate(&hostile).is_err());
        assert!(semaprax::codegen::emit_hir_c(&hostile).is_err());
        assert!(semaprax::wasm::emit_resolved_module(&hostile).is_err());
    }
    let mut hostile = program.clone();
    let flag = hostile.functions[index]
        .cleanup
        .flags
        .iter_mut()
        .find(|flag| {
            flag.lifecycle.as_str() == "core.iter.drop" && !flag.place.projections.is_empty()
        })
        .unwrap();
    flag.lifecycle = semaprax::hir::DeclarationId::new("core.bytes.drop");
    assert!(semaprax::hir::validate(&hostile).is_err());
    let mut omitted = program.clone();
    let flags = &mut omitted.functions[index].cleanup.flags;
    let at = flags
        .iter()
        .position(|flag| !flag.place.projections.is_empty())
        .unwrap();
    flags.remove(at);
    assert!(semaprax::hir::validate(&omitted).is_err());
    let mut wrong_path = program.clone();
    let flag = wrong_path.functions[index]
        .cleanup
        .flags
        .iter_mut()
        .find(|flag| !flag.place.projections.is_empty())
        .unwrap();
    flag.place.projections[0] = semaprax::hir::DeclarationId::new("core.iter-step.done");
    assert!(semaprax::hir::validate(&wrong_path).is_err());
}

#[test]
fn owning_iterators_reject_reuse_and_unsupported_elements() {
    let reused = r#"
module iterator.reuse;
@id("app.main") fn main()->i64 {
 let iter=vec_into_iter<i64>(vec_with_capacity<i64>(0usize));
 let first=iter_next<i64>(iter);
 let second=iter_next<i64>(iter);
 0
}
"#;
    let errors = semaprax::check(reused, "iterator-reuse.spx").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O101"),
        "{errors:?}"
    );
    let bytes = "module iterator.bytes; @id(\"bytes\") fn bytes(value:own Iter<Bytes>)->i64{0} @id(\"app.main\") fn main()->i64{0}";
    semaprax::check(bytes, "iterator-bytes.spx").unwrap();
    for ty in ["String", "Vec<i64>", "fn(i64)->i64"] {
        let source=format!("module iterator.invalid; @id(\"invalid\") fn invalid(value:own Iter<{ty}>)->i64{{0}} @id(\"app.main\") fn main()->i64{{0}}");
        let first = semaprax::check(&source, "iterator-invalid.spx").unwrap_err();
        let second = semaprax::check(&source, "iterator-invalid.spx").unwrap_err();
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }
}

#[test]
fn owning_iterators_wasm_has_valid_typed_moves_without_owner_memory_copy() {
    let ast = semaprax::check(&source(), "iterator-wasm.spx").unwrap();
    let bytes = semaprax::wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(!matches!(
                    operators.read().unwrap(),
                    wasmparser::Operator::MemoryCopy { .. }
                        | wasmparser::Operator::MemoryGrow { .. }
                ));
            }
        }
    }
}

#[test]
fn owning_iterators_all_scalars_step_reconstruction_and_return() {
    collections::run_source_value(&source_with_reconstruction(true), 8);
}

#[test]
fn owning_iterators_local_done_carrier_without_intrinsics_or_owner_signature() {
    collections::run_source_value(
        r#"module test.iterator_local_done;
@id("app.main") fn main()->i64 {
 let finished=IterStep<i64>::Done {};
 match own finished {
  IterStep::Done {} => 7,
  IterStep::Yield {item,rest} => 0,
 }
}
"#,
        7,
    );
    collections::run_source_value(
        r#"module test.iterator_direct_done;
@id("app.main") fn main()->i64 {
 match own IterStep<i64>::Done {} {
  IterStep::Done {} => 7,
  IterStep::Yield {item,rest} => 0,
 }
}
"#,
        7,
    );
}
