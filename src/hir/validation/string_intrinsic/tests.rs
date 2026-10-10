use super::*;
use crate::hir::{self, IdentityOrigin};
use crate::string_ops::StringOp;

const SOURCE: &str = r#"module test.intrinsic_identity;
@id("app.row") record Row { @id("app.row.value") value:i64, }
@id("app.helper") fn helper(value:i64)->i64 { value }
@id("app.identity") fn identity<T>(value:T)->T { value }
@id("app.main") fn main()->i64 { identity<i64>(helper(42)) }
"#;

fn operations() -> impl Iterator<Item = StringOp> {
    StringOp::ALL
        .into_iter()
        .chain(StringOp::TEXT_TOOLKIT)
        .chain(StringOp::COLLECTIONS)
        .chain(StringOp::CONVERSIONS)
        .chain([
            StringOp::MapRemove,
            StringOp::I64FromU8,
            StringOp::I64FromI32,
            StringOp::UsizeFromU8,
            StringOp::U8FromI64,
            StringOp::CharFromU8,
            StringOp::CharFromI64,
            StringOp::FromUtf8,
        ])
}

fn program() -> ResolvedProgram {
    let source = crate::check(SOURCE, "intrinsic-identity.spx").unwrap();
    let program = hir::resolve(&source).unwrap();
    assert!(!program.function_templates.is_empty());
    assert!(!program.function_instances.is_empty());
    program
}

fn assert_reserved(program: &ResolvedProgram) {
    let error = hir::validate(program).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(
        error
            .message
            .contains("aliases a compiler-owned string operation"),
        "{error:?}"
    );
}

#[test]
fn every_string_operation_reserves_its_exact_authored_identity() {
    let mut count = 0;
    for operation in operations() {
        let source = format!(
            "module test.alias; @id(\"{}\") fn ordinary_name(value:i64)->i64 {{ value }} @id(\"app.main\") fn main()->i64{{ordinary_name(1)}}",
            operation.id()
        );
        let source = crate::parse(&source, "reserved-id.spx").unwrap();
        assert!(
            crate::verify::verify(&source)
                .iter()
                .any(|d| d.code == "SPX-S113"),
            "{}",
            operation.id()
        );
        assert!(hir::resolve(&source)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-S113"));
        count += 1;
    }
    assert_eq!(count, 37);
    for declaration in [
        "@id(\"core.num.char_from_i64\") record Alias { @id(\"alias.field\") value:i64, }",
        "@id(\"alias.record\") record Alias { @id(\"core.num.char_from_i64\") value:i64, }",
        "@id(\"alias.variant\") variant Alias { @id(\"core.num.char_from_i64\") Empty {}, }",
        "@id(\"alias.variant\") variant Alias { @id(\"alias.case\") Value { @id(\"core.num.char_from_i64\") value:i64, }, }",
        "@id(\"core.num.char_from_i64\") fn ordinary_name<T>(value:T)->T { value }",
    ] {
        let source =
            format!("module test.alias; {declaration} @id(\"app.main\") fn main()->i64{{0}}");
        let ast = crate::parse(&source, "reserved-declaration.spx").unwrap();
        assert!(
            crate::verify::verify(&ast)
                .iter()
                .any(|d| d.code == "SPX-S113"),
            "{declaration}"
        );
        assert!(
            hir::resolve(&ast)
                .unwrap_err()
                .iter()
                .any(|d| d.code == "SPX-S113")
        );
    }
}

#[test]
fn reserved_declaration_metadata_is_not_authority_after_cache_decode() {
    let source = program();
    for operation in operations() {
        for kind in [
            DeclarationKind::Function,
            DeclarationKind::Record,
            DeclarationKind::Field,
        ] {
            for origin in [
                IdentityOrigin::Explicit,
                IdentityOrigin::Automatic,
                IdentityOrigin::CompilerOwned,
            ] {
                let mut forged = source.clone();
                let declaration = forged
                    .declarations
                    .declarations
                    .values_mut()
                    .find(|declaration| declaration.kind == kind)
                    .unwrap();
                declaration.id = DeclarationId::new(operation.id());
                declaration.identity_origin = origin;
                assert_reserved(&forged);
                let wire = crate::cache_codec::encode(&forged).unwrap();
                let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
                assert_reserved(&restored);
            }
        }
    }
}

#[test]
fn reserved_identity_with_a_matching_index_key_is_rejected_for_every_declaration_kind() {
    let source = program();
    for kind in [
        DeclarationKind::Resource,
        DeclarationKind::ResourceDrop,
        DeclarationKind::Record,
        DeclarationKind::Field,
        DeclarationKind::Class,
        DeclarationKind::Variant,
        DeclarationKind::VariantCase,
        DeclarationKind::CaseField,
        DeclarationKind::Interface,
        DeclarationKind::Import,
        DeclarationKind::Function,
    ] {
        for origin in [
            IdentityOrigin::Explicit,
            IdentityOrigin::Automatic,
            IdentityOrigin::CompilerOwned,
        ] {
            let mut forged = source.clone();
            let id = DeclarationId::new(StringOp::CharFromI64.id());
            forged.declarations.declarations.insert(
                id.clone(),
                hir::Declaration {
                    id,
                    name: "ordinary_name".to_owned(),
                    kind,
                    identity_origin: origin,
                    owner: None,
                },
            );
            // Matching the map key and attached identity must not establish
            // intrinsic authority, regardless of the claimed origin or kind.
            assert_reserved(&forged);
            let wire = crate::cache_codec::encode(&forged).unwrap();
            let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
            assert_reserved(&restored);
        }
    }
}

#[test]
fn resource_and_interface_declarations_cannot_alias_string_operation_identities() {
    let source = r#"module test.resource_identity;
@id("app.file") resource File {
 @id("app.file.drop") drop trivial;
}
@id("app.host") interface Host permits {} {
 @id("app.host.consume") import fn consume(file: own File) -> unit
  effects {} failure infallible consumes file always;
}
@id("app.main") fn main()->i64 { 0 }
"#;
    crate::check(source, "resource-identities.spx").unwrap();
    for identity in ["app.file", "app.file.drop", "app.host", "app.host.consume"] {
        let forged = source.replace(
            &format!("@id(\"{identity}\")"),
            "@id(\"core.num.char_from_i64\")",
        );
        assert_ne!(forged, source);
        let ast = crate::parse(&forged, "resource-identities.spx").unwrap();
        assert!(crate::verify::verify(&ast)
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-S113"));
        assert!(hir::resolve(&ast)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-S113"));
    }
}

#[test]
fn graph_replay_rejects_reserved_declaration_reminting_in_graph_or_source() {
    let ast = crate::check(SOURCE, "intrinsic-identity.spx").unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    for operation in operations() {
        let forged_graph = graph.replace("\"app.helper\"", &format!("\"{}\"", operation.id()));
        assert_ne!(forged_graph, graph);
        let errors = crate::graph::verify_json(&ast, &forged_graph).unwrap_err();
        assert!(errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G411"));

        // Changing retained source as well cannot launder a reserved identity
        // through graph reconstruction; ordinary source admission runs first.
        let forged_source = SOURCE.replace(
            "@id(\"app.helper\")",
            &format!("@id(\"{}\")", operation.id()),
        );
        assert_ne!(forged_source, SOURCE);
        let forged_ast = crate::parse(&forged_source, "intrinsic-identity.spx").unwrap();
        let errors = crate::graph::verify_json(&forged_ast, &forged_graph).unwrap_err();
        assert!(errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-S113"));
    }
}

#[test]
fn reserved_function_headers_templates_and_instances_cannot_impersonate_intrinsics() {
    let source = program();
    for operation in operations() {
        for target in 0..6 {
            let mut forged = source.clone();
            match target {
                0 => forged.functions[0].id = DeclarationId::new(operation.id()),
                1 => forged.functions[0].name = operation.name().to_owned(),
                2 => forged.function_templates[0].id = DeclarationId::new(operation.id()),
                3 => forged.function_templates[0].name = operation.name().to_owned(),
                4 => forged.function_instances[0].function.id = DeclarationId::new(operation.id()),
                _ => forged.function_instances[0].function.name = operation.name().to_owned(),
            }
            assert_reserved(&forged);
            let wire = crate::cache_codec::encode(&forged).unwrap();
            let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
            assert_reserved(&restored);
        }
    }
}

#[test]
fn ordinary_namespaced_identity_and_nonfunction_field_names_remain_admitted() {
    let source = r#"module test.ordinary_names;
@id("app.core.num.char_from_i64") record Row {
 @id("app.row.char_from_i64") char_from_i64:i64,
}
@id("app.string_from_i64") fn string_from_i64_copy(value:i64)->i64 { value }
@id("app.main") fn main()->i64 {
 let row=Row{char_from_i64:42};
 string_from_i64_copy(row.char_from_i64)
}
"#;
    let ast = crate::check(source, "ordinary-identities.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let round = crate::check(&canonical, "ordinary-identities.spx").unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&round, &graph).unwrap();
    let program = hir::resolve(&round).unwrap();
    hir::validate(&program).unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    let result =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&restored, "app.main", 1000).unwrap();
    assert!(matches!(
        result.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    ));
}
