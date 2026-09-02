//! Executable private capacity contract for the resolver and validator.
//!
//! Pins the bounded heap each iterative phase may retain against the
//! prelude identity contract the root module publishes.

use super::*;
use std::path::Path;

#[test]
fn private_capacity_prelude_identity_contract_matches_root_prelude() {
    assert_eq!(
        crate::private_capacity_contract::PRELUDE_CAPACITY_IDENTITIES,
        crate::prelude::all_ids()
    );
}

#[test]
fn declaration_index_drops_exact_depth_generic_record_and_variant_fields_iteratively() {
    fn nested_type(prefix: &str) -> ResolvedType {
        let mut ty = ResolvedType::I64;
        // One scalar leaf plus 511 nominal wrappers exercises the exact
        // 512-slot semantic type-workspace boundary. This HIR carrier is
        // forged because source admission rejects nested user generics.
        for depth in 1..512 {
            ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(format!("{prefix}.{depth}")),
                arguments: vec![ty],
            };
        }
        ty
    }

    std::thread::Builder::new()
        .name("declaration-index-iterative-drop".to_owned())
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut index = DeclarationIndex::default();
            index.record_fields.insert(
                DeclarationId::new("drop.record"),
                vec![ResolvedFieldDeclaration {
                    id: DeclarationId::new("drop.record.field"),
                    name: "field".to_owned(),
                    index: 0,
                    ty: nested_type("drop.record.generic"),
                    span: Span::default(),
                }],
            );
            index.variant_cases.insert(
                DeclarationId::new("drop.variant"),
                vec![ResolvedVariantCaseDeclaration {
                    id: DeclarationId::new("drop.variant.case"),
                    name: "Case".to_owned(),
                    index: 0,
                    fields: vec![ResolvedFieldDeclaration {
                        id: DeclarationId::new("drop.variant.case.field"),
                        name: "field".to_owned(),
                        index: 0,
                        ty: nested_type("drop.variant.generic"),
                        span: Span::default(),
                    }],
                    span: Span::default(),
                }],
            );
            index.case_fields.insert(
                DeclarationId::new("drop.variant.case"),
                vec![ResolvedFieldDeclaration {
                    id: DeclarationId::new("drop.variant.case.field"),
                    name: "field".to_owned(),
                    index: 0,
                    ty: nested_type("drop.case-index.generic"),
                    span: Span::default(),
                }],
            );
            drop(index);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn opaque_declaration_index_is_bounded_by_shared_private_contract() {
    fn maximum_occurrences(program: &crate::ast::Program) -> usize {
        fn type_occurrences(
            ty: &crate::ast::Type,
            program: &crate::ast::Program,
            memo: &mut BTreeMap<String, usize>,
            visiting: &mut BTreeSet<String>,
        ) -> usize {
            let crate::ast::Type::Named { name, arguments } = ty else {
                return 1;
            };
            let argument_total = arguments
                .iter()
                .map(|argument| type_occurrences(argument, program, memo, visiting))
                .sum::<usize>();
            let Some(declaration) = program.types.iter().find(|item| item.name == *name) else {
                return 1 + argument_total;
            };
            if let Some(value) = memo.get(name) {
                return value.saturating_add(argument_total);
            }
            assert!(
                visiting.insert(name.clone()),
                "cycle must fail before capacity proof"
            );
            let fields: Vec<&crate::ast::Type> = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { .. } => Vec::new(),
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                    fields.iter().map(|field| &field.ty).collect()
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .map(|field| &field.ty)
                    .collect(),
            };
            let value = 1usize.saturating_add(
                fields
                    .into_iter()
                    .map(|field| type_occurrences(field, program, memo, visiting))
                    .sum::<usize>(),
            );
            visiting.remove(name);
            memo.insert(name.clone(), value);
            value.saturating_add(argument_total)
        }
        let mut memo = BTreeMap::new();
        let mut visiting = BTreeSet::new();
        let mut maximum = 1;
        for declaration in &program.types {
            let ty = crate::ast::Type::Named {
                name: declaration.name.clone(),
                arguments: Vec::new(),
            };
            maximum = maximum.max(type_occurrences(&ty, program, &mut memo, &mut visiting));
        }
        maximum
    }

    let sources = [
        "module capacity.index;\n@id(\"capacity.main\") fn main() -> i64 { 0 }\n",
        include_str!("../../tests/fixtures/native_rust_hir_capacity.spx"),
        "module capacity.generic;\n@id(\"box\") record Box<T> { @id(\"box.value\") value: T, }\n@id(\"identity\") fn identity<T>(value: T) -> T { value }\n@id(\"capacity.main\") fn main() -> i64 { identity<i64>(1) }\n",
        "module capacity.import;\npermit { host.echo }\n@id(\"host\") interface Host permits { host.echo } { @id(\"host.echo\") import rust fn echo(value: i64) -> i64 effects { host.echo } failure status \"host.echo.v1\"; }\n@id(\"capacity.main\") fn main() -> i64 uses { host.echo } { echo(1) }\n",
    ];
    for source in sources {
        let program = crate::parse(source, Path::new("capacity-index.spx")).unwrap();
        let canonical = crate::format::canonical(&program);
        let resolved = resolve(&program).unwrap();
        let layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
            canonical.len(),
            program.types.len(),
            maximum_occurrences(&program),
        )
        .unwrap();
        assert!(resolved.declarations.type_facts_layout_capacity() <= layout_upper);
        let upper = crate::private_capacity_contract::declaration_index_upper(
            canonical.len(),
            program.types.len(),
            program.interfaces.len(),
            program.functions.len(),
            layout_upper,
        )
        .unwrap();
        assert!(
            resolved.declarations.owned_capacity_for_private_contract() <= upper,
            "opaque DeclarationIndex exceeded shared source-derived upper"
        );
    }

    let mut wide = String::from("module capacity.index.wide;\n");
    for index in 0..514 {
        use std::fmt::Write as _;
        writeln!(
            wide,
            "@id(\"wide.r{index}\") record R{index} {{ @id(\"wide.r{index}.v\") v: i64, }}"
        )
        .unwrap();
    }
    wide.push_str("@id(\"capacity.main\") fn main() -> i64 { 0 }\n");
    let program = crate::parse(&wide, Path::new("capacity-index-wide.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let resolved = resolve(&program).unwrap();
    let layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
        canonical.len(),
        program.types.len(),
        maximum_occurrences(&program),
    )
    .unwrap();
    assert!(resolved.declarations.type_facts_layout_capacity() <= layout_upper);
    let upper = crate::private_capacity_contract::declaration_index_upper(
        canonical.len(),
        program.types.len(),
        program.interfaces.len(),
        program.functions.len(),
        layout_upper,
    )
    .unwrap();
    assert!(resolved.declarations.owned_capacity_for_private_contract() <= upper);

    let mut chain = String::from(
        "module capacity.index.chain;\n@id(\"chain.r0\") record R0 { @id(\"chain.r0.v\") v: i64, }\n",
    );
    for index in 1..514 {
        use std::fmt::Write as _;
        writeln!(
            chain,
            "@id(\"chain.r{index}\") record R{index} {{ @id(\"chain.r{index}.v\") v: R{}, }}",
            index - 1
        )
        .unwrap();
    }
    chain.push_str("@id(\"capacity.main\") fn main() -> i64 { 0 }\n");
    let program = crate::parse(&chain, Path::new("capacity-index-chain.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let resolved = resolve(&program).unwrap();
    let layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
        canonical.len(),
        program.types.len(),
        maximum_occurrences(&program),
    )
    .unwrap();
    assert!(resolved.declarations.type_facts_layout_capacity() <= layout_upper);
    let upper = crate::private_capacity_contract::declaration_index_upper(
        canonical.len(),
        program.types.len(),
        program.interfaces.len(),
        program.functions.len(),
        layout_upper,
    )
    .unwrap();
    assert!(resolved.declarations.owned_capacity_for_private_contract() <= upper);
    drop(resolved);

    let nested = "module capacity.index.nested;\n@id(\"nested.box\") record Box<T> { @id(\"nested.box.v\") v: T, }\n@id(\"nested.deep\") record Deep { @id(\"nested.deep.v\") v: Box<Box<i64>>, }\n@id(\"capacity.main\") fn main() -> i64 { 0 }\n";
    let program = crate::parse(nested, Path::new("capacity-index-nested.spx")).unwrap();
    let error = resolve(&program).unwrap_err();
    assert!(error.iter().any(|diagnostic| diagnostic.code == "SPX-T223"));

    let parameter_argument = "module capacity.index.parameter;\n@id(\"capacity.identity\") fn identity<T>(value: T<i64>) -> i64 { 0 }\n@id(\"capacity.main\") fn main() -> i64 { 0 }\n";
    let program = crate::parse(
        parameter_argument,
        Path::new("capacity-index-parameter.spx"),
    )
    .unwrap();
    let error = resolve(&program).unwrap_err();
    assert!(error.iter().any(|diagnostic| diagnostic.code == "SPX-T220"));
}
