use super::*;

#[test]
fn additive_field_and_outcome_graphs_do_not_fall_through_to_legacy_rendering() {
    let projected = r#"module projected.only;
@id("row") record Row {@id("row.text") text:string,}
@id("inspect") fn inspect(value:borrow Row)->i64 {
 str_len_bytes(string_as_str(value.text))
}
@id("app.main") fn main()->i64 {0}
"#;
    let outcome = r#"module outcome.only;
@id("inner") record Inner {@id("inner.bytes") bytes:Bytes,}
@id("payload") record Payload {@id("payload.inner") inner:Inner,@id("payload.mark") mark:i64,}
@id("decoded") variant Decoded {
 @id("decoded.ready") Ready {@id("decoded.value") value:Payload,},
 @id("decoded.error") Error {@id("decoded.code") code:i64,@id("decoded.offset") offset:usize,@id("decoded.field") field:i64,},
}
@id("fail") fn fail()->Decoded {Decoded::Error{code:1,offset:0usize,field:0}}
@id("app.main") fn main()->i64 {0}
"#;
    for (source, schema, fact) in [
        (projected, "semaprax.graph.v73", "projected_string_views"),
        (outcome, "semaprax.graph.v74", "owned_nested_outcomes"),
    ] {
        let ast = crate::check(source, "additive-routing.spx").unwrap();
        let program = hir::resolve(&ast).unwrap();
        assert!(program.function_instances.is_empty());
        assert!(!nested_owned::requires_generic_result_schema(&program));
        assert!(!super::super::owned_collection_records::requires(&program));
        assert!(!super::super::owned_text_record_loans::requires(&program));
        assert!(!super::super::scoped_vec_field::requires(&program));
        if schema == "semaprax.graph.v74" {
            // Error-only construction still needs its case-qualified carrier
            // schema, without incidental Vec, String-view or loan selection.
            assert!(!super::super::projected_string_view::requires(&program));
            assert!(program
                .functions
                .iter()
                .all(|f| f.loan_plan.loans.is_empty()));
            assert!(!program
                .declarations
                .byte_slice_provenances()
                .any(|(_, p)| p.root_kind == ByteSliceRootKind::OwnedString));
        }
        assert_eq!(graph_schema(&program).unwrap(), schema);
        assert_eq!(
            graph_schema_from_parts_and_instances(
                &program.interfaces,
                &program.types,
                &program.functions,
                &program.function_templates,
                &program.function_instances,
            )
            .unwrap(),
            schema,
        );
        let graph = to_json(&ast).unwrap();
        let document: Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(document["schema"], schema);
        assert_eq!(document[fact]["authority"], false);
        assert_eq!(to_json(&ast).unwrap(), graph);
        verify_json(&ast, &graph).unwrap();
        assert!(to_legacy_json(&ast).is_err());
        let stale = graph.replacen(schema, "semaprax.graph.v34", 1);
        assert_ne!(stale, graph);
        assert!(verify_json(&ast, &stale).is_err());
    }
}
