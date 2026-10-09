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
        crate::prelude::all_type_ids_v2()
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

fn expression_backing_cleanup_plan(ids: &[ExpressionId]) -> crate::cleanup_plan::CleanupPlan {
    use crate::cleanup_plan::*;
    let mut plan = CleanupPlan::unresolved();
    plan.entry_state.live_owned_parameters = vec![CleanupPlace {
        storage: StorageId::CallArgument {
            call: ids[0].clone(),
            parameter_index: 0,
            value_expression: ids[1].clone(),
        },
        projections: Vec::new(),
    }];
    plan.status_sources = vec![StatusSource {
        id: StatusSourceId {
            expression: ids[2].clone(),
            lane: StatusLane::ContractFalse,
        },
        producer: StatusProducer::ContractFalse {
            phase: ContractPhase::Requires,
            ordinal: 0,
        },
    }];
    plan.blocks = vec![CleanupBlock {
        id: BlockId(0),
        region: CleanupRegionId(0),
        transitions: vec![
            CleanupTransition::Initialize {
                at: ids[3].clone(),
                destination: CleanupPlace {
                    storage: StorageId::Temporary(ids[4].clone()),
                    projections: Vec::new(),
                },
            },
            CleanupTransition::StageCopyResult {
                source: StagedCopyResultSource::Body {
                    expression: ids[5].clone(),
                    instance: ResolvedType::I64,
                },
            },
        ],
        terminator: CleanupTerminator::Exit(ExitTargetId(0)),
    }];
    plan.edges = vec![CleanupEdge {
        id: EdgeId(0),
        from: BlockId(0),
        to: BlockId(0),
        condition: EdgeCondition::BooleanResult(ids[6].clone(), true),
    }];
    plan.exits = vec![ExitTarget {
        id: ExitTargetId(0),
        from: BlockId(0),
        leaves_regions: Vec::new(),
        finalize_in_order: Vec::new(),
        continuation: ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar {
                expression: ids[7].clone(),
            },
        },
    }];
    plan
}

#[test]
fn cleanup_capacity_counts_full_expression_backing_per_retained_occurrence() {
    use crate::cleanup::*;
    let id = ExpressionId::from_owned(String::from("capacity.expression"));
    let backing = id.owned_allocation_bytes().unwrap();
    assert!(backing > id.as_str().len());
    let shared_ids: [ExpressionId; 8] = std::array::from_fn(|_| id.clone());
    let independent_ids: [ExpressionId; 8] =
        std::array::from_fn(|_| ExpressionId::from_owned(id.as_str().to_owned()));
    let shared = expression_backing_cleanup_plan(&shared_ids);
    let independent = expression_backing_cleanup_plan(&independent_ids);
    let headers = shared.entry_state.live_owned_parameters.capacity()
        * std::mem::size_of::<crate::cleanup_plan::CleanupPlace>()
        + shared.status_sources.capacity()
            * std::mem::size_of::<crate::cleanup_plan::StatusSource>()
        + shared.blocks.capacity() * std::mem::size_of::<crate::cleanup_plan::CleanupBlock>()
        + shared.blocks[0].transitions.capacity()
            * std::mem::size_of::<crate::cleanup_plan::CleanupTransition>()
        + shared.edges.capacity() * std::mem::size_of::<crate::cleanup_plan::CleanupEdge>()
        + shared.exits.capacity() * std::mem::size_of::<crate::cleanup_plan::ExitTarget>();
    let census = crate::private_capacity_contract::cleanup_plan_owned_capacity;
    assert_eq!(census(&shared), Some(headers + 8 * backing));
    assert_eq!(census(&independent), census(&shared));
    let mut inventory = CleanupInventory::unresolved();
    inventory.slots = (0..2)
        .map(|index| CleanupStorageSlot {
            id: CleanupStorageId(index),
            discovery_index: index,
            origin: CleanupStorageOrigin::Temporary {
                expression: id.clone(),
            },
            ty: ResolvedType::I64,
            shape: FieldLivenessShape::NoDrop,
        })
        .collect();
    assert_eq!(
        crate::private_capacity_contract::cleanup_inventory_owned_capacity(&inventory),
        Some(inventory.slots.capacity() * std::mem::size_of::<CleanupStorageSlot>() + 2 * backing)
    );
}

#[test]
fn cleanup_capacity_refuses_missing_backing_in_each_expression_metadata_family() {
    let (refused, overflowed, _) = crate::bounded_output::with_limit_usage(0, || {
        ExpressionId::from_owned(String::from("refused.expression"))
    });
    assert!(overflowed);
    assert_eq!(refused.owned_allocation_bytes(), None);
    let id = ExpressionId::from_owned(String::from("capacity.expression"));
    for position in 0..8 {
        let mut ids: [ExpressionId; 8] = std::array::from_fn(|_| id.clone());
        ids[position] = refused.clone();
        let plan = expression_backing_cleanup_plan(&ids);
        assert_eq!(
            crate::private_capacity_contract::cleanup_plan_owned_capacity(&plan),
            None,
            "refused backing at metadata position {position}"
        );
    }
    let mut inventory = crate::cleanup::CleanupInventory::unresolved();
    inventory.slots = vec![crate::cleanup::CleanupStorageSlot {
        id: crate::cleanup::CleanupStorageId(0),
        discovery_index: 0,
        origin: crate::cleanup::CleanupStorageOrigin::Temporary {
            expression: refused,
        },
        ty: ResolvedType::I64,
        shape: crate::cleanup::FieldLivenessShape::NoDrop,
    }];
    assert_eq!(
        crate::private_capacity_contract::cleanup_inventory_owned_capacity(&inventory),
        None
    );
}
