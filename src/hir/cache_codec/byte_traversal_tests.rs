//! Borrowed byte traversal survives both cache projections; cache bytes carry
//! no authority, so decoded HIR is replayed again before any use.
use super::*;
use crate::cache_codec;

const SOURCE: &str = include_str!("../../../tests/fixtures/byte-slice-traversal.spx");

fn latch(program: &mut ResolvedProgram) -> &mut LoopLatchKind {
    let exits = program
        .functions
        .iter_mut()
        .find(|function| function.name == "exits")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut exits.body.kind else {
        panic!("function block")
    };
    let wrapper = statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .expect("byte traversal wrapper");
    let ResolvedExprKind::Block { statements, .. } = &mut wrapper.kind else {
        panic!("wrapper block")
    };
    let ResolvedStatement::LexicalWhile { latch, .. } = &mut statements[2] else {
        panic!("lexical traversal loop")
    };
    latch
}

#[test]
fn byte_traversal_roundtrips_source_graph_and_both_cache_projections() {
    let ast = crate::check(SOURCE, "byte-slice-traversal.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let checked = crate::check(&canonical, "byte-slice-traversal-roundtrip.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let ast_wire = cache_codec::encode(&checked).unwrap();
    let restored_ast: crate::ast::Program = cache_codec::decode(&ast_wire).unwrap();
    assert_eq!(cache_codec::encode(&restored_ast).unwrap(), ast_wire);
    let graph = crate::graph::to_json(&restored_ast).unwrap();
    crate::graph::verify_json(&restored_ast, &graph).unwrap();
    assert_eq!(graph, crate::graph::to_json(&checked).unwrap());

    let mut resolved = crate::hir::resolve(&restored_ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    assert!(matches!(
        latch(&mut resolved),
        LoopLatchKind::SourceForStep {
            kind: SourceForKind::BorrowBytes,
            ..
        }
    ));
    let wire = cache_codec::encode(&resolved).unwrap();
    let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&restored).unwrap();
    crate::loan_plan::validate_program(&restored).unwrap();
    assert_eq!(cache_codec::encode(&restored).unwrap(), wire);
    assert_eq!(
        crate::graph::graph_schema(&restored).unwrap(),
        "semaprax.graph.v83"
    );
    // The checked-HIR wire that may carry tag 3 is owned by cache v8.
    assert_eq!(
        crate::project::PROJECT_SEMANTIC_CACHE_COMPATIBILITY,
        // v12 since e13c35e26/23e8f1d0a; was v11.
        "semaprax.project-checked-module-hir.v12"
    );
}

#[test]
fn source_for_kind_tags_roundtrip_and_refuse_the_tag_after_borrow_bytes() {
    let prior = [
        (SourceForKind::Copy, 0u8),
        (SourceForKind::BorrowRecord, 1),
        (SourceForKind::OwnedIterator, 2),
        (SourceForKind::BorrowBytes, 3),
    ];
    for (kind, tag) in prior {
        let wire = cache_codec::encode(&kind).unwrap();
        let mut forged = wire.clone();
        let position = wire.iter().rposition(|byte| *byte == tag).unwrap();
        assert_eq!(cache_codec::decode::<SourceForKind>(&wire).unwrap(), kind);
        forged[position] = 4;
        assert!(cache_codec::decode::<SourceForKind>(&forged).is_err());
    }
}

#[test]
fn cached_byte_traversal_latch_drift_is_refused_after_decode() {
    let ast = crate::check(SOURCE, "byte-slice-traversal-drift.spx").unwrap();
    let original = crate::hir::resolve(&ast).unwrap();
    for kind in [
        SourceForKind::Copy,
        SourceForKind::BorrowRecord,
        SourceForKind::OwnedIterator,
    ] {
        let mut forged = original.clone();
        let LoopLatchKind::SourceForStep { kind: slot, .. } = latch(&mut forged) else {
            panic!("byte latch")
        };
        *slot = kind;
        let wire = cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
        assert_eq!(
            crate::hir::validate(&restored).unwrap_err().code,
            "SPX-H006"
        );
    }
}
