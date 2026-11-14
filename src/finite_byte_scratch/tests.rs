//! Authored owning source/HIR/graph proofs; execution remains a separate gate.
use crate::{byte_data_capacity, finite_byte_scratch, hir};
const SOURCE: &str = r#"module finite.scratch;
@id("app.main") fn main()->i64 ensures result==4 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let mut count=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(3usize);
  count=count+2;
  if byte_len(bytes_as_slice(item))==1usize {continue;};
  if byte_len(bytes_as_slice(scratch))!=3usize {return 0;};
  0
 }
 return count;
}
"#;
fn parsed(source: &str) -> crate::ast::Program {
    crate::parse(source, std::path::Path::new("finite-scratch.spx")).unwrap()
}
#[test]
fn independent_source_and_hir_bounds_count_each_real_iteration() {
    let source = crate::check(SOURCE, "finite-scratch.spx").unwrap();
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "finite-scratch.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&roundtrip));
    let resolved = hir::resolve(&source).unwrap();
    hir::validate(&resolved).unwrap();
    let summary = hir::analyze_byte_data_capacity(&resolved).unwrap();
    let main = summary.function("app.main").unwrap();
    assert_eq!(
        (main.bytes_copy_sites, main.owned_byte_payload_bytes),
        (4, 9)
    );
    let function = &resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (condition, body) = finite_byte_scratch::loops(function).next().unwrap();
    let proof = finite_byte_scratch::resolved(&resolved, condition, body).unwrap();
    assert_eq!(proof.iterations, 2);
    assert_eq!(proof.sites().count(), 1);
}
#[test]
fn graph_resource_descriptor_replays_and_rejects_changed_authority() {
    let source = crate::check(SOURCE, "finite-graph.spx").unwrap();
    let graph = crate::graph::to_json(&source).unwrap();
    crate::graph::verify_json(&source, &graph).unwrap();
    let descriptor: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(descriptor["schema"], "semaprax.graph.v78");
    let loop_fact = &descriptor["finite_owned_iterator_scratch"]["loops"][0];
    assert_eq!(loop_fact["iterations"], 2);
    let (base, suffix) = graph
        .rsplit_once(",\"finite_owned_iterator_scratch\":")
        .unwrap();
    for (field, forged) in [
        ("iterations", serde_json::json!(1)),
        ("execution", serde_json::json!("app.foreign")),
        ("condition", loop_fact["body"].clone()),
        ("body", loop_fact["condition"].clone()),
        ("allocation_sites", serde_json::json!([])),
        (
            "allocation_sites",
            serde_json::json!([loop_fact["allocation_sites"][0], "foreign.site"]),
        ),
    ] {
        let original = format!("\"{field}\":{}", loop_fact[field]);
        let replacement = format!("\"{field}\":{forged}");
        assert!(suffix.contains(&original));
        let mutated = suffix.replacen(&original, &replacement, 1);
        let wire = format!("{base},\"finite_owned_iterator_scratch\":{mutated}");
        let errors = crate::graph::verify_json(&source, &wire).unwrap_err();
        assert_eq!(errors[0].code, "SPX-G411", "{field}");
    }
    assert_eq!(
        crate::graph::verify_json(&source, &format!("{base}}}")).unwrap_err()[0].code,
        "SPX-G411"
    );
}
#[test]
fn unknown_mutated_and_overlarge_constructor_provenance_refuse() {
    for hostile in [
        SOURCE.replace("let values=", "let mut values="),
        SOURCE.replace(
            "2usize),bytes_zeroed(1usize)",
            "33usize),bytes_zeroed(1usize)",
        ),
        SOURCE.replace(
            "vec_into_iter<Bytes>(values)",
            "vec_into_iter<Bytes>(make())",
        ),
    ] {
        let ast = parsed(&hostile);
        let function = ast.functions.iter().find(|f| f.name == "main").unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("block");
        };
        let body = statements
            .iter()
            .find_map(|s| {
                if let crate::ast::Statement::ForOwn { body, .. } = s {
                    Some(body.as_ref())
                } else {
                    None
                }
            })
            .unwrap();
        assert!(finite_byte_scratch::source(function, body).is_none());
    }
}
#[test]
fn scratch_owner_and_view_escape_never_obtain_iteration_authority() {
    for hostile in [
        SOURCE.replace(
            "byte_len(bytes_as_slice(scratch))!=3usize",
            "byte_len(bytes_as_slice(bytes_set(scratch,0usize,1u8)))!=3usize",
        ),
        SOURCE.replace(
            "let scratch=bytes_zeroed(3usize);",
            "let scratch=bytes_zeroed(3usize); let view=bytes_as_slice(scratch);",
        ),
        SOURCE.replace(
            "let scratch=bytes_zeroed(3usize);",
            "let scratch=bytes_zeroed(3usize); while false { let another=bytes_zeroed(1usize); 0 }",
        ),
    ] {
        let ast = parsed(&hostile);
        let function = ast.functions.iter().find(|f| f.name == "main").unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("block");
        };
        let body = statements
            .iter()
            .find_map(|s| {
                if let crate::ast::Statement::ForOwn { body, .. } = s {
                    Some(body.as_ref())
                } else {
                    None
                }
            })
            .unwrap();
        assert!(finite_byte_scratch::source(function, body).is_none());
    }
}
#[test]
fn finite_count_and_payload_limits_are_cumulative_and_inclusive() {
    use byte_data_capacity::{CapacityFlow as F, FunctionCapacityInput as Input};
    let input = |iterations, bytes| Input {
        function: "f".into(),
        array_slots: vec![],
        execution: F::FiniteOwnedLoop {
            iterations,
            condition: Box::new(F::Empty),
            body: Box::new(F::BytesCopy {
                site: "scratch".into(),
                conservative_payload_bytes: bytes,
            }),
        },
    };
    let exact = byte_data_capacity::analyze(&[input(32, 65_536)]).unwrap();
    let summary = exact.function("f").unwrap();
    assert_eq!(
        (summary.bytes_copy_sites, summary.owned_byte_payload_bytes),
        (32, 2_097_152)
    );
    assert!(byte_data_capacity::analyze(&[input(32, 65_537)]).is_err());
    assert!(byte_data_capacity::analyze(&[input(33, 1)]).is_err());
    let zero = byte_data_capacity::analyze(&[input(0, 131_072)]).unwrap();
    assert_eq!(zero.function("f").unwrap().bytes_copy_sites, 0);
}
#[test]
fn nested_capacity_products_refuse_instead_of_wrapping() {
    use byte_data_capacity::{CapacityDiagnostic, CapacityFlow, FunctionCapacityInput};
    let nested = |levels, payload| {
        (0..levels).fold(
            CapacityFlow::BytesCopy {
                site: "scratch".into(),
                conservative_payload_bytes: payload,
            },
            |body, _| CapacityFlow::FiniteOwnedLoop {
                iterations: 32,
                condition: Box::new(CapacityFlow::Empty),
                body: Box::new(body),
            },
        )
    };
    for (execution, detail) in [
        (nested(7, 1), "finite iterator allocation count overflowed"),
        (
            nested(2, crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES),
            "bytes_copy path reaches 1024 sites; limit is 32",
        ),
    ] {
        let input = FunctionCapacityInput {
            function: "nested".into(),
            array_slots: vec![],
            execution,
        };
        let error = byte_data_capacity::analyze(&[input]).unwrap_err();
        assert_eq!(error.diagnostic, CapacityDiagnostic::Allocation);
        assert!(error.detail.contains(detail), "{}", error.detail);
    }
}

#[test]
fn arbitrary_while_condition_allocation_and_stdin_stay_refused() {
    use byte_data_capacity::{CapacityFlow as F, FunctionCapacityInput as Input};
    let copy = || F::BytesCopy {
        site: "copy".into(),
        conservative_payload_bytes: 1,
    };
    for execution in [
        F::Loop {
            condition: Box::new(F::Empty),
            body: Box::new(copy()),
        },
        F::FiniteOwnedLoop {
            iterations: 1,
            condition: Box::new(copy()),
            body: Box::new(F::Empty),
        },
        F::FiniteOwnedLoop {
            iterations: 1,
            condition: Box::new(F::Empty),
            body: Box::new(F::StdinRead {
                site: "stdin".into(),
                conservative_payload_bytes: 1,
            }),
        },
    ] {
        let input = Input {
            function: "f".into(),
            array_slots: vec![],
            execution,
        };
        let error = byte_data_capacity::analyze(&[input]).unwrap_err();
        assert_eq!(
            error.diagnostic,
            byte_data_capacity::CapacityDiagnostic::Allocation
        );
    }
}
fn boundary_source(rows: usize, input_bytes: usize) -> String {
    let mut source = String::from("module finite.boundary; @id(\"app.main\") fn main()->i64 {\n");
    for index in 0..rows {
        let previous = if index == 0 {
            format!("vec_with_capacity<Bytes>({rows}usize)")
        } else {
            format!("row{}", index - 1)
        };
        source.push_str(&format!(
            "let row{index}=vec_push<Bytes>({previous},bytes_zeroed({input_bytes}usize));\n"
        ));
    }
    let values = if rows == 0 {
        "vec_with_capacity<Bytes>(0usize)".into()
    } else {
        format!("row{}", rows - 1)
    };
    source.push_str(&format!("let values={values}; let mut seen=0; for own item in vec_into_iter<Bytes>(values) {{ let scratch=bytes_zeroed(131072usize); seen=seen+1; if byte_len(bytes_as_slice(scratch))!=131072usize {{return 0;}}; 0 }} return seen; }}"));
    source
}
#[test]
fn source_exact_cumulative_boundary_and_one_extra_do_not_refund_drops() {
    let exact = crate::check(&boundary_source(16, 0), "finite-exact.spx").unwrap();
    let resolved = hir::resolve(&exact).unwrap();
    let summary = hir::analyze_byte_data_capacity(&resolved).unwrap();
    let main = summary.function("app.main").unwrap();
    assert_eq!(
        (main.bytes_copy_sites, main.owned_byte_payload_bytes),
        (32, 2_097_152)
    );
    for hostile in [boundary_source(17, 0), boundary_source(16, 1)] {
        let diagnostics = crate::check(&hostile, "finite-extra.spx").unwrap_err();
        assert!(diagnostics.iter().any(|d| d.code == "SPX-T267"));
    }
    let zero = crate::check(&boundary_source(0, 0), "finite-empty.spx").unwrap();
    let resolved = hir::resolve(&zero).unwrap();
    assert_eq!(
        hir::analyze_byte_data_capacity(&resolved)
            .unwrap()
            .function("app.main")
            .unwrap()
            .bytes_copy_sites,
        0
    );
}
#[test]
fn decoded_hir_replays_the_finite_resource_bound() {
    use crate::cache_codec::{decode, encode};
    let source = crate::check(SOURCE, "finite-cache.spx").unwrap();
    let program = hir::resolve(&source).unwrap();
    let wire = encode(&program).unwrap();
    let decoded: hir::ResolvedProgram = decode(&wire).unwrap();
    assert_eq!(encode(&decoded).unwrap(), wire);
    hir::validate(&decoded).unwrap();
    let capacity = hir::analyze_byte_data_capacity(&decoded).unwrap();
    let main = capacity.function("app.main").unwrap();
    assert_eq!(
        (main.bytes_copy_sites, main.owned_byte_payload_bytes),
        (4, 9)
    );
}

#[test]
fn typed_unknown_constructor_and_forged_latch_lose_finite_authority() {
    let source = crate::check(SOURCE, "finite-forged.spx").unwrap();
    let mut resolved = hir::resolve(&source).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block");
    };
    let first = statements
        .iter_mut()
        .find_map(|s| {
            if let hir::ResolvedStatement::Let { value, .. } = s {
                Some(value)
            } else {
                None
            }
        })
        .unwrap();
    let hir::ResolvedExprKind::Call { args, .. } = &mut first.kind else {
        panic!("push");
    };
    let hir::ResolvedExprKind::Call { callee, .. } = &mut args[0].kind else {
        panic!("capacity");
    };
    *callee = hir::DeclarationId::new("host.untrusted.capacity");
    let function = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (condition, body) = finite_byte_scratch::loops(function).next().unwrap();
    assert!(finite_byte_scratch::resolved(&resolved, condition, body).is_none());
    assert!(hir::validate(&resolved).is_err());
    let mut resolved = hir::resolve(&source).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block");
    };
    let latch = statements
        .iter_mut()
        .find_map(|s| {
            let hir::ResolvedStatement::Let { value, .. } = s else {
                return None;
            };
            let hir::ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
                return None;
            };
            statements.iter_mut().find_map(|s| {
                if let hir::ResolvedStatement::LexicalWhile { latch, .. } = s {
                    Some(latch)
                } else {
                    None
                }
            })
        })
        .unwrap();
    *latch = hir::LoopLatchKind::Condition;
    assert!(hir::validate(&resolved).is_err());
    let wire = crate::cache_codec::encode(&resolved).unwrap();
    let decoded = crate::cache_codec::decode(&wire).unwrap();
    assert!(hir::validate(&decoded).is_err());
}

#[test]
fn resolved_scratch_saved_cursor_precharges_its_complete_actual_frame_layout() {
    let source = parsed(SOURCE);
    let typed = crate::hir::resolve(&source).unwrap();
    let function = typed
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (condition, body) = finite_byte_scratch::loops(function).next().unwrap();
    let ((proof, observed), refused, charged) =
        crate::bounded_output::with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            crate::test_allocation_observer::measure(|| {
                finite_byte_scratch::resolved(&typed, condition, body)
            })
        });
    assert_eq!(proof.unwrap().iterations, 2);
    assert!(!refused && !observed.invalid);
    let frame_bytes = std::mem::size_of::<
        [Option<(crate::hir::lexical_exit::ChildCursor<'_>, bool)>; super::DEPTH],
    >();
    assert!(charged >= frame_bytes);
    assert!(observed.allocated <= charged && observed.highwater <= charged);
    let (exact, refused, used) = crate::bounded_output::with_limit_usage(charged, || {
        finite_byte_scratch::resolved(&typed, condition, body)
    });
    assert_eq!(exact.unwrap().iterations, 2);
    assert!(!refused);
    assert_eq!(used, charged);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(charged - 1, || {
        finite_byte_scratch::resolved(&typed, condition, body)
    });
    assert!(short.is_none() && refused);
}
