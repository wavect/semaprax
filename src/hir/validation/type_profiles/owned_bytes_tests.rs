//! Query-only parity and storage observations; no new type-family admission.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
use std::mem::size_of;
mod old;

const SOURCE: &str = r#"module app;
@id("query.some") fn some()->Option<Bytes> { Option<Bytes>::Some { value:bytes_zeroed(0usize) } }
@id("app.main") fn main()->i64 {
 let values=vec_with_capacity<Bytes>(1usize);
 if vec_len<Bytes>(values)==0usize {0}else{1}
}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "owned-byte-type-query.spx").unwrap()).unwrap()
}
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
fn parity(program: &ResolvedProgram, ty: &ResolvedType, expected: bool) {
    assert_eq!(old::query(program, ty), expected, "saved {ty:?}");
    assert_eq!(
        resolved_type_contains_owned_bytes(program, ty),
        expected,
        "borrowed {ty:?}"
    );
}
#[test]
fn borrowed_owned_byte_query_matches_saved_generic_arguments_opaque_functions_and_current_declarations(
) {
    let program = program();
    let original = program.clone();
    crate::hir::validate(&program).unwrap();
    let some = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "query.some")
        .unwrap();
    parity(&program, &some.return_type, true);
    for (ty, expected) in [
        (nominal("core.vec", vec![ResolvedType::Bytes]), true),
        (nominal("core.vec", vec![ResolvedType::I64]), false),
        (nominal("missing", vec![ResolvedType::Bytes]), true),
        (nominal("missing", Vec::new()), false),
        (
            ResolvedType::Function {
                parameters: vec![ResolvedType::Bytes],
                result: Box::new(ResolvedType::Bytes),
            },
            false,
        ),
        (
            ResolvedType::TypeParameter {
                owner: DeclarationId::new("query.owner"),
                index: 0,
            },
            false,
        ),
        (ResolvedType::OnceFunction, true),
        (ResolvedType::OnceFunctionI64, true),
        (ResolvedType::OnceFunctionI64Pair, true),
        (ResolvedType::MutFunctionI64, false),
        (ResolvedType::String, false),
        (ResolvedType::SliceU8, false),
        (ResolvedType::Never, false),
    ] {
        parity(&program, &ty, expected);
    }
    assert_eq!(program, original);
}

#[test]
fn borrowed_owned_byte_query_keeps_text_visited_cycles_duplicate_kinds_and_unsubstituted_fields() {
    let mut program = program();
    let mut declaration = program
        .types
        .iter()
        .find(|d| d.id.as_str() == "core.vec")
        .unwrap()
        .clone();
    declaration.id = DeclarationId::new("query.cycle");
    declaration.name = "Cycle".to_owned();
    let field = ResolvedFieldDeclaration {
        id: DeclarationId::new("query.cycle.field"),
        name: "field".to_owned(),
        index: 0,
        ty: nominal("query.cycle", Vec::new()),
        span: declaration.span,
    };
    declaration.kind = ResolvedTypeDeclarationKind::Record {
        fields: vec![field.clone()].into(),
    };
    program.types.push(declaration.clone());
    parity(&program, &nominal("query.cycle", Vec::new()), false);
    // Arguments remain visited even for an already visited declaration key.
    parity(
        &program,
        &nominal("query.cycle", vec![ResolvedType::Bytes]),
        true,
    );
    declaration.kind = ResolvedTypeDeclarationKind::Class {
        fields: vec![ResolvedFieldDeclaration {
            ty: ResolvedType::Bytes,
            ..field.clone()
        }]
        .into(),
        methods: Vec::new(),
    };
    // Original source order wins a duplicate exact identity; no pointer-key shortcut.
    program.types.push(declaration.clone());
    parity(&program, &nominal("query.cycle", Vec::new()), false);
    program.types.remove(program.types.len() - 2);
    parity(&program, &nominal("query.cycle", Vec::new()), true);
    let parameters = [ResolvedType::TypeParameter {
        owner: declaration.id.clone(),
        index: 0,
    }];
    declaration.kind = ResolvedTypeDeclarationKind::Record {
        fields: vec![ResolvedFieldDeclaration {
            ty: parameters[0].clone(),
            ..field.clone()
        }]
        .into(),
    };
    *program.types.last_mut().unwrap() = declaration.clone();
    parity(
        &program,
        &nominal("query.cycle", vec![ResolvedType::I64]),
        false,
    );
    parity(
        &program,
        &nominal("query.cycle", vec![ResolvedType::Bytes]),
        true,
    );
    // Variant field traversal is unchanged, including declaration argument order.
    let case = program
        .types
        .iter()
        .find_map(|d| match &d.kind {
            ResolvedTypeDeclarationKind::Variant { cases } => cases.first(),
            _ => None,
        })
        .unwrap()
        .clone();
    declaration.kind = ResolvedTypeDeclarationKind::Variant {
        cases: vec![ResolvedVariantCaseDeclaration {
            fields: vec![ResolvedFieldDeclaration {
                ty: ResolvedType::Bytes,
                ..field
            }],
            ..case
        }]
        .into(),
    };
    *program.types.last_mut().unwrap() = declaration.clone();
    parity(&program, &nominal("query.cycle", Vec::new()), true);
    declaration.kind = ResolvedTypeDeclarationKind::Resource {
        drop: ResolvedResourceDrop {
            id: DeclarationId::new("query.cycle.drop"),
            kind: ResolvedResourceDropKind::Trivial,
        },
    };
    *program.types.last_mut().unwrap() = declaration;
    parity(&program, &nominal("query.cycle", Vec::new()), false);
    // Resource kind skips its payload, but never skips the nominal arguments.
    parity(
        &program,
        &nominal("query.cycle", vec![ResolvedType::Bytes]),
        true,
    );
}

fn settled(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.reallocations, 0);
}
#[test]
fn borrowed_owned_generic_query_removes_literal_type_payload_clones_through_whole_query_drop() {
    let program = program();
    let ty = nominal("core.vec", vec![ResolvedType::Bytes]);
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &ty
    else {
        unreachable!()
    };
    assert_eq!(arguments.as_slice(), [ResolvedType::Bytes]);
    assert!(program.types.iter().any(|d| d.id == *declaration
        && matches!(&d.kind, ResolvedTypeDeclarationKind::Record { fields } if fields.is_empty())));
    // Observe the two map carrier layouts independently, not an assumed private
    // BTree node size. Exact text identity is shared on this current foundation.
    let (_, old_map) = measure(|| {
        let mut keys = BTreeSet::new();
        keys.insert(declaration.clone());
        drop(keys);
    });
    let (_, borrowed_map) = measure(|| {
        let mut keys = BTreeSet::new();
        keys.insert(declaration);
        drop(keys);
    });
    settled(old_map);
    settled(borrowed_map);
    assert_eq!(old_map.allocations, 1);
    assert_eq!(borrowed_map.allocations, 1);
    let (old_value, old_metrics) = measure(|| old::query(&program, &ty));
    let (new_value, new_metrics) = measure(|| resolved_type_contains_owned_bytes(&program, &ty));
    assert_eq!((old_value, new_value), (true, true));
    settled(old_metrics);
    settled(new_metrics);
    // Saved query clones the real one-element generic argument backing AND
    // owns a one-element type frontier; current query owns only a ref frontier.
    assert_eq!(
        old_metrics.allocated,
        old_map.allocated + 2 * size_of::<ResolvedType>()
    );
    assert_eq!(
        new_metrics.allocated,
        borrowed_map.allocated + size_of::<&ResolvedType>()
    );
    assert_eq!(old_metrics.allocations, 3);
    assert_eq!(new_metrics.allocations, 2);
    assert!(new_metrics.allocated < old_metrics.allocated);
}
