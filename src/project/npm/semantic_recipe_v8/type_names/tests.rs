use super::*;
use crate::hir::{DeclarationId, ResolvedTypeDeclarationKind};

const SOURCE: &str = r#"
module recipe.collision;
@id("z.type") record Left { @id("z.field") value: i64, }
@id("a.type") record Right { @id("a.field") flag: bool, }
@id("m.type") variant SpxRecipeType0 { @id("m.case") Ready, }
@id("z.main") fn main() -> i64 { read(Left { value: 7 }) }
@id("a.read") fn read(value: Left) -> i64 { value.value }
@id("m.read") fn other(value: Right) -> bool { value.flag }
"#;

fn resolve(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(source, std::path::Path::new("type-names.spx")).unwrap())
        .unwrap()
}

fn assert_fields(
    left: &[crate::hir::ResolvedFieldDeclaration],
    right: &[crate::hir::ResolvedFieldDeclaration],
) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.id, right.id);
        assert_eq!(left.name, right.name);
        assert_eq!(left.index, right.index);
        assert_eq!(left.ty, right.ty);
    }
}

fn assert_shape(left: &ResolvedTypeDeclarationKind, right: &ResolvedTypeDeclarationKind) {
    match (left, right) {
        (
            ResolvedTypeDeclarationKind::Record { fields: left },
            ResolvedTypeDeclarationKind::Record { fields: right },
        ) => assert_fields(left, right),
        (
            ResolvedTypeDeclarationKind::Variant { cases: left },
            ResolvedTypeDeclarationKind::Variant { cases: right },
        ) => {
            assert_eq!(left.len(), right.len());
            for (left, right) in left.iter().zip(right) {
                assert_eq!(left.id, right.id);
                assert_eq!(left.name, right.name);
                assert_fields(&left.fields, &right.fields);
            }
        }
        _ => panic!("type kind changed"),
    }
}

/// Independent setup uses the existing workspace linker, not the restoration
/// under test. Two verified module-like record declarations retain distinct
/// IDs but the same authored presentation name. The integration fixture also
/// exercises actual multi-file Project loading and npm/native handoff.
fn linked(source: &str) -> ResolvedProgram {
    let mut program = resolve(source);
    let declarations = program.declarations.workspace_declarations();
    let functions = program
        .functions
        .into_iter()
        .map(|function| {
            let origin = declarations
                .iter()
                .find(|fact| fact.id == function.id)
                .unwrap()
                .identity_origin;
            LinkedScalarFunction { function, origin }
        })
        .collect();
    program
        .types
        .retain(|ty| !crate::prelude::is_compiler_owned_id(ty.id.as_str()));
    for ty in &mut program.types {
        if ty.name == "Left" || ty.name == "Right" {
            ty.name = "Payload".to_owned();
        }
    }
    let mut result = crate::hir::link_owned_data_api_workspace(
        program.module,
        program.entrypoint,
        functions,
        LinkedOwnedDataParts {
            permits: Vec::new(),
            types: program.types,
            interfaces: Vec::new(),
            declaration_facts: declarations
                .into_iter()
                .filter(|fact| !crate::prelude::is_compiler_owned_id(fact.id.as_str()))
                .map(|fact| {
                    (
                        fact.id,
                        LinkedDeclarationFact {
                            kind: fact.kind,
                            origin: fact.identity_origin,
                            owner: fact.owner,
                        },
                    )
                })
                .collect(),
            function_templates: Vec::new(),
            function_instances: Vec::new(),
        },
    )
    .unwrap();
    // Valid supplied ordering is not required to equal the linker's ID order.
    result.functions.reverse();
    crate::hir::validate(&result).unwrap();
    result
}

#[test]
fn colliding_records_restore_exact_names_ids_members_and_function_order() {
    let program = linked(SOURCE);
    let recipe = super::super::render(&program).unwrap();
    let expected = concat!(
        "// semaprax-owned-data-type-names.v1 ",
        "[[\"a.type\",\"Payload\"],[\"m.type\",\"SpxRecipeType0\"],[\"z.type\",\"Payload\"]]\n"
    );
    assert!(recipe.starts_with(expected));
    let replayed = super::super::replay_against(&program, &recipe).unwrap();
    assert_eq!(super::super::render(&replayed).unwrap(), recipe);
    assert_eq!(
        program.functions.iter().map(|f| &f.id).collect::<Vec<_>>(),
        replayed.functions.iter().map(|f| &f.id).collect::<Vec<_>>()
    );
    for ty in &program.types {
        let restored = replayed
            .types
            .iter()
            .find(|other| other.id == ty.id)
            .unwrap();
        assert_eq!(restored.name, ty.name);
        assert_shape(&restored.kind, &ty.kind);
        assert_eq!(
            replayed.declarations.declaration(&ty.id).unwrap().name,
            ty.name
        );
    }
    let right = replayed
        .types
        .iter()
        .find(|ty| ty.id.as_str() == "a.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &right.kind else {
        panic!("record")
    };
    assert_eq!(fields[0].id.as_str(), "a.field");
    assert_eq!(fields[0].ty, crate::hir::ResolvedType::Bool);
    let left = replayed
        .types
        .iter()
        .find(|ty| ty.id.as_str() == "z.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &left.kind else {
        panic!("record")
    };
    assert_eq!(fields[0].id.as_str(), "z.field");
    assert_eq!(fields[0].ty, crate::hir::ResolvedType::I64);
}

#[test]
fn collision_free_legacy_recipe_has_no_header_and_keeps_exact_bytes() {
    let program = resolve("module original; @id(\"entry\") fn main() -> i64 { 0 }");
    let expected = "module semaprax_npm_recipe;\n\n@id(\"entry\")\nfn main() -> i64\n{ 0 }\n\n";
    assert_eq!(super::super::render(&program).unwrap(), expected);
    assert_eq!(
        super::super::render(&super::super::replay(expected).unwrap()).unwrap(),
        expected
    );
    let named = super::super::render(&resolve(SOURCE)).unwrap();
    assert!(!named.starts_with(PREFIX));
    assert!(named.contains("record Left"));
    assert!(named.contains("record Right"));
    assert!(named.contains("variant SpxRecipeType0"));
    assert_eq!(
        super::super::render(&super::super::replay(&named).unwrap()).unwrap(),
        named
    );
}

#[test]
fn collision_header_preserves_control_bearing_type_identity_bytes() {
    let source = SOURCE.replace("a.type", r"a.type\u{8}\u{c}\u{7f}\u{85}");
    let program = linked(&source);
    let recipe = super::super::render(&program).unwrap();
    assert!(recipe.starts_with(&format!(
        "{PREFIX}[[\"a.type\\u0008\\u000c\\u007f\\u0085\",\"Payload\"]"
    )));
    assert!(recipe.contains(r#"@id("a.type\u{8}\u{c}\u{7f}"#));
    let replayed = super::super::replay_against(&program, &recipe).unwrap();
    let id = DeclarationId::new("a.type\u{8}\u{c}\u{7f}\u{85}");
    assert_eq!(
        replayed.declarations.declaration(&id).unwrap().name,
        "Payload"
    );
}

#[test]
fn hostile_headers_cannot_change_inventory_aliases_or_source_admission() {
    let program = linked(SOURCE);
    let recipe = super::super::render(&program).unwrap();
    let (Some(rows), body) = read_header(&recipe).unwrap() else {
        panic!("header")
    };
    let mut mutations = Vec::new();
    let mut changed = rows.clone();
    changed.remove(1);
    mutations.push(changed);
    let mut changed = rows.clone();
    changed[0][0] = "unknown.type".to_owned();
    mutations.push(changed);
    let mut changed = rows.clone();
    changed[0][0] = "core.option".to_owned();
    mutations.push(changed);
    let mut changed = rows.clone();
    changed.insert(0, changed[0].clone());
    mutations.push(changed);
    let mut changed = rows.clone();
    changed.swap(0, 1);
    mutations.push(changed);
    let mut changed = rows.clone();
    changed[0][1] = "Distinct".to_owned();
    mutations.push(changed);
    for name in [
        "",
        "Payload ",
        " Payload",
        "Payload//comment",
        "a.b",
        "a\nrecord Evil",
        "Option",
        "Result",
        "λ",
        "123",
        "A\0B",
    ] {
        let mut changed = rows.clone();
        changed[0][1] = name.to_owned();
        changed[2][1] = name.to_owned();
        mutations.push(changed);
    }
    for changed in mutations {
        let mutated = format!("{}{body}", render_header(&changed).unwrap());
        let error = super::super::replay(&mutated).expect_err("hostile header must reject");
        assert_eq!(error.code, "SPX-W120", "wrong error for {changed:?}");
    }
    for mutated in [
        recipe.replacen(PREFIX, "// semaprax-owned-data-type-names.v2 ", 1),
        recipe.replacen(PREFIX, "// unknown-type-names.v1 ", 1),
        recipe.replacen("[[", "[ [", 1),
        format!(" {recipe}"),
        format!("{recipe}\n"),
        recipe.replace("SpxRecipeType2", "AlternateAlias"),
    ] {
        assert_ne!(mutated, recipe);
        assert!(super::super::replay(&mutated).is_err());
    }
    // A genuine display rename remains representable, but is not the original
    // linked subject. The enclosing descriptor/payload digest owns that bind.
    let renamed = recipe.replace("\"Payload\"", "\"Renamed\"");
    assert!(super::super::replay(&renamed).is_ok());
    assert!(super::super::replay_against(&program, &renamed).is_err());
}

#[test]
fn header_shape_and_whole_recipe_bound_are_checked_before_source_parse() {
    for json in [
        "{}",
        "[]",
        "null",
        "[[\"a\",\"Payload\",\"extra\"]]",
        "[[[\"a\"],\"Payload\"]]",
        "[[\"a\",1]]",
        "[[\"a\",\"Payload\"]]",
    ] {
        assert!(read_header(&format!("{PREFIX}{json}\nmodule x;")).is_err());
    }
    assert!(read_header(PREFIX).is_err());
    let exact = "x".repeat(super::super::MAX_RECIPE_BYTES);
    let (header, body) = read_header(&exact).unwrap();
    assert!(header.is_none());
    assert_eq!(body.len(), super::super::MAX_RECIPE_BYTES);
    let over = format!("{exact}x");
    assert!(read_header(&over).is_err());
    assert!(super::super::replay(&over).is_err());
    // Bound success above is only byte admission, not valid source admission.
    assert!(super::super::replay(&exact).is_err());
}
