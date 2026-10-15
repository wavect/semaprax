//! Maximum-family traversal retains independently replayed source and cache authority.
use super::*;
use crate::cache_codec;
use crate::hir::selected_fused_candidate::validate_selected;

const SOURCE: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

#[test]
fn maximum_owned_traversal_roundtrips_source_and_both_cache_projections_with_fresh_selection() {
    let ordinary = crate::check(SOURCE, "maximum-record-traversal.spx").unwrap_err();
    assert!(ordinary.iter().any(|error| error.code == "SPX-T267"));
    let ast = crate::parse(SOURCE, "maximum-record-traversal.spx").unwrap();
    let original = validate_selected(&ast).unwrap();
    assert_eq!(original.revalidate_current().unwrap().rows, 4096);
    let canonical = crate::format::canonical(&ast);
    let checked = crate::parse(&canonical, "maximum-record-traversal-roundtrip.spx").unwrap();
    assert_eq!(validate_selected(&checked).unwrap().capacity().rows, 4096);
    assert_eq!(crate::format::canonical(&checked), canonical);
    let ast_wire = cache_codec::encode(&checked).unwrap();
    let restored_ast: crate::ast::Program = cache_codec::decode(&ast_wire).unwrap();
    assert_eq!(cache_codec::encode(&restored_ast).unwrap(), ast_wire);
    assert_eq!(crate::format::canonical(&restored_ast), canonical);
    // Transported source must pass the complete source/HIR factory again.
    // The ordinary graph route cannot acquire this explicit runtime profile.
    assert!(crate::graph::to_json(&restored_ast).is_err());
    let owner = validate_selected(&restored_ast).unwrap();
    assert_eq!(owner.revalidate_current().unwrap().rows, 4096);
    let resolved = owner.program();
    assert!(crate::hir::validate(resolved).is_err());
    let row = ResolvedType::Nominal {
        declaration: DeclarationId::new("app.leaf.row"),
        arguments: Vec::new(),
    };
    let layout = crate::hir::owned_leaf_collection::layout(&resolved.declarations, &row).unwrap();
    assert_eq!((layout.scalar_count, layout.owned_count), (1, 2));
    assert_eq!(layout.capacity(), 4096);
    let wire = cache_codec::encode(resolved).unwrap();
    let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
    assert!(crate::hir::validate(&restored).is_err());
    owner
        .with_selected_for_test(&restored, |joined, selected| {
            assert!(joined.still_current(&restored_ast, &restored));
            assert_eq!(joined.flow().rows, 4096);
            crate::hir::validate_selected_with_static_text(&restored, Some(selected)).unwrap();
        })
        .unwrap();
    assert_eq!(cache_codec::encode(&restored).unwrap(), wire);
}

#[test]
fn ordinary_record_traversal_graph_preserves_complete_loan_authority_checks() {
    // This separate legacy control retains every original graph assertion.
    // The maximum 4096-row application above is never rescaled for execution.
    let source = SOURCE.replace("4096", "32");
    let ast = crate::check(&source, "ordinary-record-traversal-graph.spx").unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    let compact =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&graph).unwrap()).unwrap();
    assert!(compact.contains("\"kind\":\"traversal_freeze\""));
    assert!(!graph.contains("core.vec.clone_at"));
    let forged_graph = graph.replacen("\"traversal_freeze\"", "\"call_argument\"", 1);
    assert_ne!(forged_graph, graph);
    assert!(crate::graph::verify_json(&ast, &forged_graph).is_err());
}

#[test]
fn maximum_owned_traversal_cached_mutable_carrier_and_missing_freeze_refuse() {
    let ast = crate::parse(SOURCE, "maximum-record-traversal-hostile.spx").unwrap();
    let owner = validate_selected(&ast).unwrap();
    let original = owner.program();
    assert_eq!(owner.revalidate_current().unwrap().rows, 4096);
    for mutable_carrier in [false, true] {
        let mut forged = original.clone();
        let main = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        if mutable_carrier {
            let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
                panic!("main block")
            };
            let mutable = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::Let {
                        binding, mutable, ..
                    } if binding.name == "rows" => Some(mutable),
                    _ => None,
                })
                .expect("immutable traversal carrier");
            assert!(!*mutable);
            *mutable = true;
        } else {
            assert!(main
                .loan_plan
                .loans
                .iter()
                .any(|loan| loan.cause == crate::loan_plan::LoanCause::TraversalFreeze));
            main.loan_plan
                .loans
                .retain(|loan| loan.cause != crate::loan_plan::LoanCause::TraversalFreeze);
        }
        // Cache decoding is transport only; validation must rederive authority.
        let wire = cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
        assert!(crate::hir::validate(&restored).is_err());
        if mutable_carrier {
            // The current selected suffix census refuses a changed binding
            // before lending selected validation authority.
            assert!(matches!(
                owner.with_selected_for_test(&restored, |_, _| ()),
                Err(crate::hir::fused_construction::Refusal::Suffix(
                    "owner mutation or unsupported statement"
                ))
            ));
        } else {
            // Missing freeze metadata must reach the actual loan validator;
            // a join refusal cannot stand in for that independent check.
            owner
                .with_selected_for_test(&restored, |joined, selected| {
                    assert!(joined.still_current(&ast, &restored));
                    assert_eq!(
                        crate::hir::validate_selected_with_static_text(&restored, Some(selected))
                            .unwrap_err()
                            .code,
                        "SPX-H006"
                    );
                })
                .unwrap();
        }
    }
    let hostile = SOURCE.replace(
        "let text = row.title;",
        "let moved = vec_clear<Row>(rows); let text = row.title;",
    );
    assert_ne!(
        hostile, SOURCE,
        "the transfer hostile must change the fixture"
    );
    let parsed = crate::parse(&hostile, "maximum-record-traversal-transfer.spx").unwrap();
    assert!(crate::verify::verify(&parsed)
        .iter()
        .any(|d| d.code == "SPX-T265"));
}
