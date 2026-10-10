//! Projected Vec leaves retain the same physical ownership as whole vectors.
use super::*;

#[test]
fn owned_outcome_vectors_have_case_qualified_move_and_drop_slots() {
    let source = r#"module projected.vectors;
@id("row") record Row { @id("row.n") n:i64, @id("row.label") label:string, }
@id("out") variant Output {
 @id("out.ok") Ready { @id("out.words") words:Vec<string>, @id("out.rows") rows:Vec<Row>, },
 @id("out.bad") Invalid { @id("out.code") code:i64, @id("out.offset") offset:usize, @id("out.field") field:i64, },
}
@id("forward") fn forward(value:own Output)->Output {value}
@id("discard") fn discard(value:own Output)->i64 {0}
@id("app.main") fn main()->i64 {0}
"#;
    let ast = crate::check(source, "projected-vectors.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "forward")
        .unwrap();
    let plan = NativeBytesPlan::build(function)
        .unwrap()
        .expect("owning variant plan");
    let storage = StorageId::Value(function.params[0].id.clone());
    let expected = ["out.words", "out.rows"].map(|field| CleanupPlace {
        storage: storage.clone(),
        projections: vec![DeclarationId::new("out.ok"), DeclarationId::new(field)],
    });
    assert_eq!(plan.storage_leaves[&storage], expected);
    for place in &expected {
        assert_eq!(plan.slots[place].kind, OwnedLeafKind::Vec);
        let result_place = CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: place.projections.clone(),
        };
        assert_eq!(plan.slots[&result_place].kind, OwnedLeafKind::Vec);
    }
    let layout = VariantLayout::for_type(
        &program,
        crate::variant_layout::VariantTarget::Native64,
        &function.params[0].ty,
    )
    .unwrap();
    let entry = plan
        .initialize_variant_parameter(&storage, "argument", &layout)
        .unwrap();
    assert_eq!(entry.matches("spx_vec_move(spx_ctx,").count(), 2);
    assert!(!entry.contains("spx_bytes_move("));
    assert!(
        plan.finalizers.is_empty(),
        "forward transfers every active leaf into the published result"
    );
    assert!(!plan.epilogue().contains("spx_vec_drop(spx_ctx,"));
    let discard = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "discard")
        .unwrap();
    let plan = NativeBytesPlan::build(discard).unwrap().unwrap();
    let expected = discard
        .cleanup_plan
        .exits
        .iter()
        .filter(|exit| {
            !matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::Continue(_)
            )
        })
        .flat_map(|exit| &exit.finalize_in_order)
        .map(|action| action.guard_flag.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        expected.len(),
        2,
        "both case-qualified Vec owners must settle"
    );
    assert_eq!(plan.finalizers.len(), 2);
    let epilogue = plan.epilogue();
    assert_eq!(epilogue.matches("spx_vec_drop(spx_ctx,").count(), 2);
    for slot in &plan.finalizers {
        assert_eq!(slot.kind, OwnedLeafKind::Vec);
        assert_eq!(slot.place.projections[0].as_str(), "out.ok");
        assert!(epilogue.contains(&format!("if ({})", slot.flag)));
        assert!(epilogue.contains(&slot.kind.drop_call(&slot.value)));
    }
}
