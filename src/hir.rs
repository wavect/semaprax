//! Resolved high-level intermediate representation.
//!
//! The parsed AST keeps the names humans wrote. HIR replaces every nominal,
//! callable, and value reference with a deterministic identity. Backends should
//! consume this layer as the language grows rather than repeating name lookup.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::ast::{
    BinaryOp, Expr, ExprKind, ImportFailure, MatchMode, MatchPattern, ParamMode, Program,
    ResourceLifecycleKind, Span, Statement, Type, TypeDeclarationKind, UnaryOp,
};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;
use crate::loan_plan::{LoanId, LoanPlan};
use crate::source_verify::{self, is_scalar_source_type};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

#[cfg(test)]
mod command_io_provenance_hostile_tests {
    use super::*;

    #[test]
    fn command_argument_root_provenance_cannot_be_reclassified_or_recharged() {
        let source = r#"
module test.command_root_hostile;
permit { process.args.read }
@id("command.view")
fn view() -> usize uses { process.args.read } {
    let argument = arg_utf8(0usize);
    let bytes = str_as_bytes(argument);
    byte_len(bytes)
}

@id("app.main") fn main() -> i64 { 0 }
"#;
        let ast = crate::parse(source, "command-root-hostile.spx").unwrap();
        let mut program = resolve(&ast).unwrap();
        let value = program
            .declarations
            .byte_slice_roots
            .iter()
            .find_map(|(value, provenance)| {
                (provenance.root_kind == ByteSliceRootKind::CommandArguments)
                    .then_some(value.clone())
            })
            .expect("arg_utf8/str_as_bytes records its invocation root");
        let provenance = program
            .declarations
            .byte_slice_roots
            .get_mut(&value)
            .unwrap();
        provenance.root_kind = ByteSliceRootKind::BorrowedStr;
        provenance.root = value;
        let error = validate(&program).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(
            error.message.contains("byte-slice symbolic provenance"),
            "{}",
            error.message
        );
    }
}

mod inspection;
#[cfg(test)]
mod projected_byte_field_provenance_tests;
mod type_reachability;
mod validation;
mod workspace_link;

/// Validate resolved HIR and independently replay its canonical shared-loan
/// proof attachment before any semantic consumer may trust it.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    inspection::validate(program)?;
    crate::loan_plan::validate_program(program)
}
use inspection::{
    path_is_prefix, reject_nul_identity, resolved_lifecycle_effects, validate_nul_free_identities,
    visit_resolved_calls,
};
pub(crate) use inspection::{
    validate_attached_identity_references, workspace_call_sites, workspace_expression_identity,
};

pub(crate) use type_reachability::reachable_authored_types;
pub(crate) use validation::validate_core;
#[cfg(test)]
use validation::HirValidator;
pub(crate) use workspace_link::{
    link_language_command_io_workspace, link_line_command_io_workspace,
    link_owned_data_api_workspace, link_scalar_workspace, link_useful_data_command_workspace,
    link_useful_data_workspace, link_useful_text_workspace,
    owned_data_api_workspace_return_admitted, useful_data_workspace_parameter_admitted,
    useful_data_workspace_return_admitted,
};

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_call_edges(
    program: &ResolvedProgram,
) -> BTreeSet<(DeclarationId, DeclarationId)> {
    inspection::workspace_call_edges(program)
}

#[cfg(test)]
thread_local! {
    static ITERATIVE_PHASE_CAPACITY_HIGH_WATER: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    static TYPE_FACTS_OUTER_BASELINE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod private_capacity_contract_tests {
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
            include_str!("../tests/fixtures/native_rust_hir_capacity.spx"),
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
}

#[cfg(test)]
pub(crate) fn reset_iterative_phase_capacity_high_water() {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
}

#[cfg(test)]
pub(crate) fn iterative_phase_capacity_high_water() -> [usize; 3] {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_iterative_phase_capacity(index: usize, bytes: usize) {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(|water| {
        let mut values = water.get();
        values[index] = values[index].max(bytes);
        water.set(values);
    });
}

#[cfg(test)]
fn type_facts_outer_baseline() -> usize {
    TYPE_FACTS_OUTER_BASELINE.with(std::cell::Cell::get)
}

#[cfg(test)]
fn validation_scope_owned_capacity(scope: &BTreeMap<ValueId, ValidationBinding>) -> usize {
    let node_bytes = scope.len().saturating_mul(
        std::mem::size_of::<(ValueId, ValidationBinding)>()
            + std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>(),
    );
    node_bytes
        + scope.iter().fold(0usize, |bytes, (id, binding)| {
            let moved = binding
                .moved_places
                .iter()
                .fold(0usize, |bytes, (place, _)| {
                    bytes
                        + std::mem::size_of::<(Vec<PlaceProjection>, Availability)>()
                        + place.capacity() * std::mem::size_of::<PlaceProjection>()
                        + place
                            .iter()
                            .map(place_projection_owned_capacity)
                            .sum::<usize>()
                });
            let partial = binding
                .definitely_partial
                .iter()
                .fold(0usize, |bytes, place| {
                    bytes
                        + std::mem::size_of::<Vec<PlaceProjection>>()
                        + place.capacity() * std::mem::size_of::<PlaceProjection>()
                        + place
                            .iter()
                            .map(place_projection_owned_capacity)
                            .sum::<usize>()
                });
            bytes + id.as_str().len() + resolved_type_owned_capacity(&binding.ty) + moved + partial
        })
}

#[cfg(test)]
fn place_projection_owned_capacity(projection: &PlaceProjection) -> usize {
    match projection {
        PlaceProjection::Field(field) => field.as_str().len(),
        PlaceProjection::VariantField { case, field } => {
            case.as_str().len().saturating_add(field.as_str().len())
        }
    }
}

#[cfg(test)]
fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
        ResolvedType::String | ResolvedType::Bytes | ResolvedType::Str | ResolvedType::SliceU8 => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => declaration
            .as_str()
            .len()
            .saturating_add(arguments.capacity() * std::mem::size_of::<ResolvedType>())
            .saturating_add(
                arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>(),
            ),
    }
}

#[cfg(test)]
fn resolved_place_owned_capacity(place: &Place) -> usize {
    place.root.as_str().len()
        + place.projections.capacity() * std::mem::size_of::<PlaceProjection>()
        + place
            .projections
            .iter()
            .map(place_projection_owned_capacity)
            .sum::<usize>()
}

#[cfg(test)]
fn resolved_expr_owned_capacity(expression: &ResolvedExpr) -> usize {
    let mut bytes = expression
        .id
        .as_str()
        .len()
        .saturating_add(resolved_type_owned_capacity(&expression.ty));
    let child = |value: &ResolvedExpr| {
        std::mem::size_of::<ResolvedExpr>().saturating_add(resolved_expr_owned_capacity(value))
    };
    match &expression.kind {
        ResolvedExprKind::Place(place) => bytes += resolved_place_owned_capacity(place),
        ResolvedExprKind::BorrowPlace { operation, place } => {
            bytes += operation.as_str().len() + resolved_place_owned_capacity(place);
        }
        ResolvedExprKind::ByteRange {
            operation,
            source,
            start,
            end,
        } => {
            bytes += operation.as_str().len() + child(source) + child(start) + child(end);
        }
        ResolvedExprKind::ArrayU8(values) => bytes += values.capacity(),
        ResolvedExprKind::Unary { value: operand, .. } => bytes += child(operand),
        ResolvedExprKind::Upcast { source } => bytes += child(source),
        ResolvedExprKind::Project { base, field } => {
            bytes += child(base) + field.as_str().len();
        }
        ResolvedExprKind::Binary { left, right, .. } => bytes += child(left) + child(right),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => bytes += child(condition) + child(then_branch) + child(else_branch),
        ResolvedExprKind::Call {
            callee,
            args,
            type_arguments,
            instance,
        } => {
            bytes += callee.as_str().len();
            bytes += instance.as_ref().map_or(0, |id| id.as_str().len());
            bytes += args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += args.iter().map(resolved_expr_owned_capacity).sum::<usize>();
            bytes += type_arguments.capacity() * std::mem::size_of::<ResolvedType>();
            bytes += type_arguments
                .iter()
                .map(resolved_type_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            bytes += call.expression.as_str().len() + call.import.as_str().len();
            bytes += call.args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += call
                .args
                .iter()
                .map(resolved_expr_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::HostCommandCall(call) => {
            bytes += call.expression.as_str().len();
            bytes += call.args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += call
                .args
                .iter()
                .map(resolved_expr_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => {
            bytes += child(operand)
                + result.as_str().len()
                + ok_case.as_str().len()
                + ok_field.as_str().len()
                + err_case.as_str().len()
                + err_field.as_str().len()
                + resolved_type_owned_capacity(residual_type);
        }
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => {
            bytes += child(operand)
                + option.as_str().len()
                + some_case.as_str().len()
                + some_field.as_str().len()
                + none_case.as_str().len()
                + resolved_type_owned_capacity(residual_type);
        }
        ResolvedExprKind::Block { statements, tail } => {
            bytes += statements.capacity() * std::mem::size_of::<ResolvedStatement>();
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    bytes += binding.id.as_str().len()
                        + binding.name.capacity()
                        + resolved_type_owned_capacity(&binding.ty)
                        + resolved_expr_owned_capacity(value);
                } else {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            bytes += resolved_expr_owned_capacity(child);
                        }
                    }
                }
            }
            bytes += child(tail);
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            bytes += record.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            bytes += variant.as_str().len() + case.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            bytes += child(scrutinee);
            bytes += arms.capacity() * std::mem::size_of::<ResolvedMatchArm>();
            bytes += arms
                .iter()
                .map(|arm| {
                    resolved_match_pattern_owned_capacity(&arm.pattern)
                        + arm.guard.as_ref().map_or(0, |guard| child(guard))
                        + resolved_expr_owned_capacity(&arm.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => {
            bytes += child(base) + record.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_) => {}
    }
    bytes
}

#[cfg(test)]
fn resolved_binding_owned_capacity(binding: &ResolvedBinding) -> usize {
    binding.id.as_str().len() + binding.name.capacity() + resolved_type_owned_capacity(&binding.ty)
}

#[cfg(test)]
fn resolved_record_pattern_field_owned_capacity(field: &ResolvedRecordMatchPatternField) -> usize {
    field.field.as_str().len()
        + match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                resolved_binding_owned_capacity(binding)
            }
            ResolvedRecordMatchFieldPattern::Wildcard => 0,
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => {
                record.as_str().len()
                    + resolved_type_owned_capacity(instance)
                    + fields.capacity() * std::mem::size_of::<ResolvedRecordMatchPatternField>()
                    + fields
                        .iter()
                        .map(resolved_record_pattern_field_owned_capacity)
                        .sum::<usize>()
            }
        }
}

#[cfg(test)]
fn resolved_match_pattern_owned_capacity(pattern: &ResolvedMatchPattern) -> usize {
    match pattern {
        ResolvedMatchPattern::Wildcard => 0,
        ResolvedMatchPattern::Literal(_) => 0,
        ResolvedMatchPattern::Binding(binding) => resolved_binding_owned_capacity(binding),
        ResolvedMatchPattern::Or(alternatives) => {
            alternatives.capacity() * std::mem::size_of::<ResolvedMatchPattern>()
                + alternatives
                    .iter()
                    .map(resolved_match_pattern_owned_capacity)
                    .sum::<usize>()
        }
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            variant.as_str().len()
                + case.as_str().len()
                + fields.capacity() * std::mem::size_of::<ResolvedMatchPatternField>()
                + fields
                    .iter()
                    .map(|field| {
                        field.field.as_str().len() + resolved_binding_owned_capacity(&field.binding)
                    })
                    .sum::<usize>()
        }
        ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => {
            record.as_str().len()
                + resolved_type_owned_capacity(instance)
                + fields.capacity() * std::mem::size_of::<ResolvedRecordMatchPatternField>()
                + fields
                    .iter()
                    .map(resolved_record_pattern_field_owned_capacity)
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn resolved_statement_owned_capacity(statement: &ResolvedStatement) -> usize {
    match statement {
        ResolvedStatement::Let { .. } | ResolvedStatement::Assign { .. } => {
            resolved_binding_owned_capacity(statement.binding())
                + resolved_expr_owned_capacity(statement.value())
        }
        // Unsafe boundaries carry only the verbatim audit summary plus their
        // ordinary block body.
        ResolvedStatement::Unsafe { audit, body, .. } => {
            audit.capacity() + resolved_expr_owned_capacity(body)
        }
        // While loops carry their condition plus their ordinary block body.
        ResolvedStatement::While {
            condition, body, ..
        } => resolved_expr_owned_capacity(condition) + resolved_expr_owned_capacity(body),
    }
}

/// Explicit Mutation v1 admits exactly the checked Copy scalar value types.
pub(crate) fn is_scalar_resolved_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Char
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}

#[cfg(test)]
fn resolved_field_initializer_owned_capacity(field: &ResolvedFieldInitializer) -> usize {
    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
}

#[cfg(test)]
fn resolved_match_arm_owned_capacity(arm: &ResolvedMatchArm) -> usize {
    resolved_match_pattern_owned_capacity(&arm.pattern)
        + arm
            .guard
            .as_ref()
            .map_or(0, |guard| resolved_expr_owned_capacity(guard))
        + resolved_expr_owned_capacity(&arm.value)
}

#[cfg(test)]
fn resolved_field_declaration_owned_capacity(field: &ResolvedFieldDeclaration) -> usize {
    field.id.as_str().len() + field.name.capacity() + resolved_type_owned_capacity(&field.ty)
}

#[cfg(test)]
fn resolved_variant_case_owned_capacity(case: &ResolvedVariantCaseDeclaration) -> usize {
    case.id.as_str().len()
        + case.name.capacity()
        + case.fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
        + case
            .fields
            .iter()
            .map(resolved_field_declaration_owned_capacity)
            .sum::<usize>()
}

#[cfg(test)]
fn resolver_scope_owned_capacity(scope: &BTreeMap<String, Binding>) -> usize {
    scope
        .len()
        .saturating_mul(
            std::mem::size_of::<(String, Binding)>()
                + std::mem::size_of::<BTreeMap<String, Binding>>(),
        )
        .saturating_add(
            scope
                .iter()
                .map(|(name, binding)| {
                    name.capacity()
                        + binding.id.as_str().len()
                        + resolved_type_owned_capacity(&binding.ty)
                })
                .sum::<usize>(),
        )
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationId(String);

impl DeclarationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(exact_string(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for DeclarationId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

impl fmt::Display for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionInstanceId(String);

impl FunctionInstanceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for FunctionInstanceId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionExecutionId {
    Monomorphic(DeclarationId),
    Generic(FunctionInstanceId),
}

impl FunctionExecutionId {
    fn diagnostic_text(&self) -> &str {
        match self {
            Self::Monomorphic(id) => id.as_str(),
            Self::Generic(id) => id.as_str(),
        }
    }

    pub fn identity_key(&self) -> String {
        match self {
            Self::Monomorphic(declaration) => format!(
                "semaprax.function-execution.v1:monomorphic:{}:{}",
                declaration.as_str().len(),
                declaration
            ),
            Self::Generic(instance) => format!(
                "semaprax.function-execution.v1:generic:{}:{}",
                instance.as_str().len(),
                instance
            ),
        }
    }

    pub fn instance(&self) -> Option<&FunctionInstanceId> {
        match self {
            Self::Monomorphic(_) => None,
            Self::Generic(instance) => Some(instance),
        }
    }

    pub fn monomorphic_declaration(&self) -> Option<&DeclarationId> {
        match self {
            Self::Monomorphic(declaration) => Some(declaration),
            Self::Generic(_) => None,
        }
    }
}

impl fmt::Display for FunctionExecutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.diagnostic_text())
    }
}

impl fmt::Display for FunctionInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(String);

impl ValueId {
    /// Synthetic identity for one compiler-owned intrinsic operation
    /// parameter; intrinsic operations have no authored declaration, so the
    /// identity only labels diagnostics and never indexes a binding.
    pub(crate) fn intrinsic_parameter(operation: &str, index: usize) -> Self {
        Self(exact_string(format!("{operation}.param.{index}")))
    }

    fn parameter(function: &FunctionExecutionId, index: usize) -> Self {
        Self(exact_string(scoped_identity(
            function,
            "value:param",
            &index.to_string(),
        )))
    }

    fn local(function: &FunctionExecutionId, path: &str) -> Self {
        Self(exact_string(scoped_identity(function, "value:local", path)))
    }

    fn result(function: &FunctionExecutionId) -> Self {
        Self(exact_string(scoped_identity(function, "value:result", "")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for ValueId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExpressionId(String);

impl ExpressionId {
    pub(crate) fn new(function: &FunctionExecutionId, path: &str) -> Self {
        Self(exact_string(scoped_identity(function, "expression", path)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for ExpressionId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

fn exact_string(value: String) -> String {
    value.into_boxed_str().into_string()
}

fn scoped_identity(owner: &FunctionExecutionId, kind: &str, path: &str) -> String {
    match owner {
        FunctionExecutionId::Monomorphic(owner) => format!(
            "declaration:{}:{}:{kind}:{}:{path}",
            owner.as_str().len(),
            owner,
            path.len()
        ),
        FunctionExecutionId::Generic(_) => {
            let owner = owner.identity_key();
            format!(
                "function-execution:{}:{}:{kind}:{}:{path}",
                owner.len(),
                owner,
                path.len()
            )
        }
    }
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Resource,
    ResourceDrop,
    Record,
    Field,
    Class,
    Variant,
    VariantCase,
    CaseField,
    Interface,
    Import,
    Function,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityOrigin {
    Explicit,
    Automatic,
    CompilerOwned,
}

impl IdentityOrigin {
    pub fn text(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Automatic => "automatic",
            Self::CompilerOwned => "compiler_owned",
        }
    }

    pub fn is_persistent(self) -> bool {
        matches!(self, Self::Explicit | Self::CompilerOwned)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: DeclarationId,
    pub name: String,
    pub kind: DeclarationKind,
    pub identity_origin: IdentityOrigin,
    pub owner: Option<DeclarationId>,
}

/// Authenticated origin class for one non-escaping byte-slice root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByteSliceRootKind {
    /// A symbolic function-parameter root. Callers substitute the argument's
    /// existing root; only a concrete host entry turns it into external input.
    FunctionParameter,
    OwnedBytes,
    FixedArray,
    BorrowedStr,
    /// The one immutable argument arena owned by the enclosing command
    /// invocation. Every `arg_utf8` view authenticates this same root.
    CommandArguments,
}

/// A symbolic extent deliberately independent of the compiler host's pointer
/// width. External lengths are checked as semantic u64 values at invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByteSliceExtent {
    Constant(u64),
    ParameterLength,
    ValueLength,
}

/// One authenticated dynamic half-open subrange step. Steps are stored from
/// the original root toward the current view, so nested named ranges form a
/// bounded acyclic derivation chain without inventing a new root identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteSliceRangeStep {
    pub source: ValueId,
    pub producer: ExpressionId,
    pub start: ExpressionId,
    pub end: ExpressionId,
}

/// Exact provenance for a byte view. Legacy views retain a complete symbolic
/// root (`offset = 0`, `length = root length`). The additive projected-field
/// profile retains one stable field-ID projection and its authenticated type;
/// aliases and ranges preserve those facts rather than minting a new root.
/// Host boundaries alone bind external parameter symbols to input storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteSliceProvenance {
    pub root: ValueId,
    /// Exact stable-ID path from `root` to the borrowed storage. Empty retains
    /// the byte-for-byte legacy root provenance carried through Graph v23.
    pub projections: Vec<PlaceProjection>,
    /// Independently resolved type at the end of `projections`.
    pub projected_type: ResolvedType,
    pub root_kind: ByteSliceRootKind,
    pub root_length: ByteSliceExtent,
    pub offset: ByteSliceExtent,
    pub length: ByteSliceExtent,
    /// The authenticated compiler-owned view expression, absent only for a
    /// symbolic external parameter root.
    pub producer: Option<ExpressionId>,
    /// Dynamic range steps relative to each immediately preceding view.
    /// Empty preserves the exact whole-root v17-v19 meaning.
    pub ranges: Vec<ByteSliceRangeStep>,
}

mod declaration_index;
pub use declaration_index::{dispose_declaration_index_for_private_contract, DeclarationIndex};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedType {
    Unit,
    I64,
    /// A checked signed 32-bit integer.
    I32,
    /// One Unicode scalar value.
    Char,
    /// One unsigned 8-bit integer value.
    U8,
    /// A target-independent checked unsigned 64-bit semantic integer.
    Usize,
    /// Inline Copy byte storage with exact target-independent length.
    ArrayU8(u32),
    /// IEEE-754 single precision.
    F32,
    /// IEEE-754 double precision.
    F64,
    Bool,
    /// An owned heap UTF-8 string value; never `Copy`.
    String,
    /// Uniquely owned immutable bytes. This needs drop but is not a resource.
    Bytes,
    /// A non-owning UTF-8 view rooted in the current invocation.
    Str,
    /// A non-owning byte view rooted in the current invocation.
    SliceU8,
    TypeParameter {
        owner: DeclarationId,
        index: u32,
    },
    Nominal {
        declaration: DeclarationId,
        arguments: Vec<ResolvedType>,
    },
}

impl ResolvedType {
    /// Canonical ownership classification shared by the resolver, cleanup
    /// builder, hostile validator, and backends. Unique ownership is not the
    /// same fact as containing an opaque resource.
    pub fn is_uniquely_owned(&self) -> bool {
        matches!(self, Self::String | Self::Bytes)
    }
    pub fn is_compiler_byte_option(&self) -> bool {
        matches!(
            self,
            Self::Nominal {
                declaration,
                arguments,
            } if declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::U8]
        )
    }

    pub fn nominal_id(&self) -> Option<&DeclarationId> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::Unit
            | Self::I64
            | Self::I32
            | Self::Char
            | Self::U8
            | Self::Usize
            | Self::ArrayU8(_)
            | Self::F32
            | Self::F64
            | Self::Bool
            | Self::String
            | Self::Bytes
            | Self::Str
            | Self::SliceU8
            | Self::TypeParameter { .. } => None,
        }
    }

    /// A name-independent key suitable as an input to future layout hashing.
    pub fn identity_key(&self) -> String {
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a DeclarationId, usize),
        }
        let mut frames = vec![Frame::Enter(self)];
        let mut keys = Vec::<String>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Self::Unit => keys.push("unit".to_owned()),
                    Self::I64 => keys.push("i64".to_owned()),
                    Self::I32 => keys.push("i32".to_owned()),
                    Self::Char => keys.push("char".to_owned()),
                    Self::U8 => keys.push("u8".to_owned()),
                    Self::Usize => keys.push("usize".to_owned()),
                    Self::ArrayU8(length) => keys.push(format!("array:u8:{length}")),
                    Self::F32 => keys.push("f32".to_owned()),
                    Self::F64 => keys.push("f64".to_owned()),
                    Self::Bool => keys.push("bool".to_owned()),
                    Self::String => keys.push("string".to_owned()),
                    Self::Bytes => keys.push("bytes".to_owned()),
                    Self::Str => keys.push("str".to_owned()),
                    Self::SliceU8 => keys.push("slice-u8".to_owned()),
                    Self::TypeParameter { owner, index } => keys.push(format!(
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    )),
                    Self::Nominal {
                        declaration,
                        arguments,
                    } => {
                        frames.push(Frame::Finish(declaration, arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(declaration, count) => {
                    let split = keys
                        .len()
                        .checked_sub(count)
                        .expect("type-key traversal has one result per argument");
                    let mut encoded = crate::bounded_output::CappedString::new();
                    for key in keys.drain(split..) {
                        write!(encoded, "{}:{key}", key.len())
                            .expect("writing to a string cannot fail");
                    }
                    keys.push(format!(
                        "nominal:{}:{}:{}:{}",
                        declaration.as_str().len(),
                        declaration,
                        count,
                        encoded.into_string()
                    ));
                }
            }
        }
        keys.pop().expect("a type always produces an identity key")
    }
}

impl FunctionInstanceId {
    pub fn derive(template: &DeclarationId, arguments: &[ResolvedType]) -> Self {
        let mut encoded_arguments = crate::bounded_output::CappedString::new();
        for argument in arguments {
            let key = argument.identity_key();
            write!(encoded_arguments, "{}:{key}", key.len())
                .expect("writing to a string cannot fail");
        }
        Self(exact_string(format!(
            "semaprax.function-instance.v1:{}:{}:{}:{}",
            template.as_str().len(),
            template,
            arguments.len(),
            encoded_arguments.into_string()
        )))
    }
}

/// Substitute one concrete generic instantiation into a declaration-owned
/// type template. Consumers share this helper so payload validation, type
/// facts, layouts, and backends cannot disagree about parameter identity.
pub(crate) fn substitute_type(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                ResolvedType::Unit => resolved.push(ResolvedType::Unit),
                ResolvedType::I64 => resolved.push(ResolvedType::I64),
                ResolvedType::I32 => resolved.push(ResolvedType::I32),
                ResolvedType::Char => resolved.push(ResolvedType::Char),
                ResolvedType::U8 => resolved.push(ResolvedType::U8),
                ResolvedType::Usize => resolved.push(ResolvedType::Usize),
                ResolvedType::ArrayU8(length) => resolved.push(ResolvedType::ArrayU8(*length)),
                ResolvedType::F32 => resolved.push(ResolvedType::F32),
                ResolvedType::F64 => resolved.push(ResolvedType::F64),
                ResolvedType::Bool => resolved.push(ResolvedType::Bool),
                ResolvedType::String => resolved.push(ResolvedType::String),
                ResolvedType::Bytes => resolved.push(ResolvedType::Bytes),
                ResolvedType::Str => resolved.push(ResolvedType::Str),
                ResolvedType::SliceU8 => resolved.push(ResolvedType::SliceU8),
                ResolvedType::TypeParameter {
                    owner: parameter_owner,
                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(format!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    resolved.push(
                        arguments
                            .get(usize::try_from(*index).map_err(|_| {
                                hir_error(format!("type parameter index {index} does not fit usize"))
                            })?)
                            .cloned()
                            .ok_or_else(|| {
                                hir_error(format!(
                                    "type template for `{owner}` references missing parameter {index}"
                                ))
                            })?,
                    );
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(declaration, count) => {
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("type substitution traversal is incomplete"))?;
                let nested = resolved.drain(split..).collect();
                resolved.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: nested,
                });
            }
        }
    }
    if resolved.len() != 1 {
        return Err(hir_error("type substitution traversal did not settle"));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}

fn substitute_source_function_type(
    function: &crate::ast::Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(&'a str, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                Type::I64 => resolved.push(Type::I64),
                Type::I32 => resolved.push(Type::I32),
                Type::Char => resolved.push(Type::Char),
                Type::U8 => resolved.push(Type::U8),
                Type::Usize => resolved.push(Type::Usize),
                Type::ArrayU8(length) => resolved.push(Type::ArrayU8(*length)),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
                Type::String => resolved.push(Type::String),
                Type::Bytes => resolved.push(Type::Bytes),
                Type::Str => resolved.push(Type::Str),
                Type::SliceU8 => resolved.push(Type::SliceU8),
                Type::Named {
                    name,
                    arguments: nested,
                } => {
                    if nested.is_empty() {
                        if let Some(index) = function
                            .type_parameters
                            .iter()
                            .position(|parameter| parameter.name == *name)
                        {
                            resolved.push(arguments.get(index)?.clone());
                            continue;
                        }
                    }
                    frames.push(Frame::Finish(name, nested.len()));
                    frames.extend(nested.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(name, count) => {
                let split = resolved.len().checked_sub(count)?;
                let arguments = resolved.drain(split..).collect();
                resolved.push(Type::Named {
                    name: name.to_owned(),
                    arguments,
                });
            }
        }
    }
    (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
}

fn specialize_source_function(
    function: &crate::ast::Function,
    arguments: &[Type],
) -> Option<crate::ast::Function> {
    let mut specialized = function.clone();
    specialized.type_parameters.clear();
    for param in &mut specialized.params {
        param.ty = substitute_source_function_type(function, arguments, &param.ty)?;
    }
    specialized.return_type =
        substitute_source_function_type(function, arguments, &function.return_type)?;
    Some(specialized)
}

fn materialize_function_template(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
) -> Result<ResolvedFunction, Diagnostic> {
    let instance = FunctionInstanceId::derive(&template.id, arguments);
    let execution = FunctionExecutionId::Generic(instance);
    let mut values = BTreeMap::new();
    let params = template
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let id = ValueId::parameter(&execution, index);
            values.insert(parameter.id.clone(), id.clone());
            Ok(ResolvedParam {
                id,
                name: parameter.name.clone(),
                ownership: parameter.ownership,
                ty: substitute_type(&parameter.ty, &template.id, arguments)?,
                span: parameter.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let result_id = ValueId::result(&execution);
    let return_type = substitute_type(&template.return_type, &template.id, arguments)?;
    let requires = template
        .requires
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &values,
                &format!("requires.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    let body = materialize_template_expr(
        template,
        arguments,
        &execution,
        &template.body,
        &values,
        "body",
    )?;
    let mut ensures_values = values;
    ensures_values.insert(template.result_id.clone(), result_id.clone());
    let ensures = template
        .ensures
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &ensures_values,
                &format!("ensures.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(ResolvedFunction {
        id: template.id.clone(),
        name: template.name.clone(),
        params,
        result_id,
        return_type,
        effects: template.effects.clone(),
        requires,
        ensures,
        body,
        cleanup: CleanupInventory::unresolved(),
        cleanup_plan: CleanupPlan::unresolved(),
        loan_plan: LoanPlan::unresolved(),
        span: template.span,
    })
}

fn resolved_scalar_substitutions(parameter_count: usize) -> Vec<Vec<ResolvedType>> {
    debug_assert!((1..=2).contains(&parameter_count));
    (0..(1_usize << parameter_count))
        .map(|bits| {
            (0..parameter_count)
                .map(|index| {
                    if bits & (1 << index) == 0 {
                        ResolvedType::I64
                    } else {
                        ResolvedType::Bool
                    }
                })
                .collect()
        })
        .collect()
}

fn same_function_meaning(expected: &ResolvedFunction, actual: &ResolvedFunction) -> bool {
    expected.id == actual.id
        && expected.name == actual.name
        && expected.params == actual.params
        && expected.result_id == actual.result_id
        && expected.return_type == actual.return_type
        && expected.effects == actual.effects
        && expected.requires == actual.requires
        && expected.ensures == actual.ensures
        && expected.body == actual.body
        && expected.span == actual.span
}

fn materialize_template_expr(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    values: &BTreeMap<ValueId, ValueId>,
    path: &str,
) -> Result<ResolvedExpr, Diagnostic> {
    let kind = match &expression.kind {
        ResolvedExprKind::Int(value) => ResolvedExprKind::Int(*value),
        ResolvedExprKind::Int32(value) => ResolvedExprKind::Int32(*value),
        ResolvedExprKind::Char(value) => ResolvedExprKind::Char(*value),
        ResolvedExprKind::Uint8(value) => ResolvedExprKind::Uint8(*value),
        ResolvedExprKind::Usize(value) => ResolvedExprKind::Usize(*value),
        ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::HostCommandCall(_) => {
            return Err(hir_error(
                "generic template uses portable byte data outside the generic slice",
            ));
        }
        ResolvedExprKind::Float32(bits) => ResolvedExprKind::Float32(*bits),
        ResolvedExprKind::Float64(bits) => ResolvedExprKind::Float64(*bits),
        ResolvedExprKind::Bool(value) => ResolvedExprKind::Bool(*value),
        ResolvedExprKind::String(value) => ResolvedExprKind::String(value.clone()),
        ResolvedExprKind::Place(place) => ResolvedExprKind::Place(Place {
            root: values
                .get(&place.root)
                .cloned()
                .ok_or_else(|| hir_error("generic template place is out of scope"))?,
            projections: place.projections.clone(),
        }),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            if instance.is_some() || !type_arguments.is_empty() {
                return Err(hir_error(
                    "generic templates cannot call generic function instances",
                ));
            }
            ResolvedExprKind::Call {
                callee: callee.clone(),
                type_arguments: Vec::new(),
                instance: None,
                args: args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            argument,
                            values,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?,
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            ResolvedExprKind::NativeRustImportCall(ResolvedNativeRustImportCall {
                expression: ExpressionId::new(execution, path),
                import: call.import.clone(),
                args: call
                    .args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            argument,
                            values,
                            &format!("{path}.native-rust-arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?,
                result: call.result.clone(),
            })
        }
        ResolvedExprKind::Unary { op, value } => ResolvedExprKind::Unary {
            op: *op,
            value: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                value,
                values,
                &format!("{path}.value"),
            )?),
        },
        ResolvedExprKind::Binary { op, left, right } => ResolvedExprKind::Binary {
            op: *op,
            left: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                left,
                values,
                &format!("{path}.left"),
            )?),
            right: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                right,
                values,
                &format!("{path}.right"),
            )?),
        },
        ResolvedExprKind::Block { statements, tail } => {
            let mut block_values = values.clone();
            let mut materialized = Vec::with_capacity(statements.len());
            for (index, statement) in statements.iter().enumerate() {
                let statement_path = format!("{path}.s{index}");
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        span,
                    } => {
                        let value = materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            value,
                            &block_values,
                            &format!("{statement_path}.value"),
                        )?;
                        let id = ValueId::local(execution, &statement_path);
                        block_values.insert(binding.id.clone(), id.clone());
                        materialized.push(ResolvedStatement::Let {
                            binding: ResolvedBinding {
                                id,
                                name: binding.name.clone(),
                                ownership: binding.ownership,
                                ty: substitute_type(&binding.ty, &template.id, arguments)?,
                                span: binding.span,
                            },
                            mutable: *mutable,
                            value,
                            span: *span,
                        });
                    }
                    ResolvedStatement::Assign { .. } => {
                        return Err(hir_error(
                            "generic template statements cannot assign to local bindings",
                        ));
                    }
                    ResolvedStatement::While { .. } => {
                        return Err(hir_error("generic templates cannot contain while loops"));
                    }
                    ResolvedStatement::Unsafe { audit, body, span } => {
                        let body = materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            body,
                            &block_values,
                            &format!("{statement_path}.body"),
                        )?;
                        materialized.push(ResolvedStatement::Unsafe {
                            audit: audit.clone(),
                            body: Box::new(body),
                            span: *span,
                        });
                    }
                }
            }
            ResolvedExprKind::Block {
                statements: materialized,
                tail: Box::new(materialize_template_expr(
                    template,
                    arguments,
                    execution,
                    tail,
                    &block_values,
                    &format!("{path}.tail"),
                )?),
            }
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => ResolvedExprKind::If {
            condition: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                condition,
                values,
                &format!("{path}.condition"),
            )?),
            then_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                then_branch,
                values,
                &format!("{path}.then"),
            )?),
            else_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                else_branch,
                values,
                &format!("{path}.else"),
            )?),
        },
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. } => {
            return Err(hir_error(
                "generic template uses an expression outside the direct-scalar slice",
            ));
        }
    };
    Ok(ResolvedExpr {
        id: ExpressionId::new(execution, path),
        ty: substitute_type(&expression.ty, &template.id, arguments)?,
        ownership: expression.ownership,
        kind,
        span: expression.span,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFacts {
    pub copy: bool,
    pub contains_resource: bool,
    pub sized: bool,
    pub needs_drop: bool,
    pub layout_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Value,
    Own,
    Borrow,
    Shared,
}

/// The authenticated ownership spelling attached to a resolved match.
///
/// This tranche preserves source meaning only. Ownership-changing behavior is
/// admitted separately after source and HIR verification can prove it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMatchMode {
    Value,
    Own,
    Borrow,
}

impl From<MatchMode> for ResolvedMatchMode {
    fn from(mode: MatchMode) -> Self {
        match mode {
            MatchMode::Value => Self::Value,
            MatchMode::Own => Self::Own,
            MatchMode::Borrow => Self::Borrow,
        }
    }
}

pub(crate) fn admitted_owned_byte_prelude_instance(
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> bool {
    matches!(
        (declaration.as_str(), arguments),
        (crate::prelude::OPTION_ID, [ResolvedType::Bytes])
            | (
                crate::prelude::RESULT_ID,
                [ResolvedType::Bytes, ResolvedType::I64 | ResolvedType::Bool]
            )
            | (
                crate::prelude::RESULT_ID,
                [ResolvedType::I64 | ResolvedType::Bool, ResolvedType::Bytes]
            )
    )
}

fn resolver_admits_flat_owned_byte_variant(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if admitted_owned_byte_prelude_instance(declaration, arguments) {
        return true;
    }
    if !arguments.is_empty() {
        return false;
    }
    declarations
        .variant_cases(declaration)
        .is_some_and(|cases| {
            cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| field.ty == ResolvedType::Bytes)
                && cases.iter().flat_map(|case| &case.fields).all(|field| {
                    field.ty == ResolvedType::Bytes || is_scalar_resolved_type(&field.ty)
                })
        })
}

impl From<ParamMode> for OwnershipMode {
    fn from(mode: ParamMode) -> Self {
        match mode {
            ParamMode::Value => Self::Value,
            ParamMode::Own => Self::Own,
            ParamMode::Borrow => Self::Borrow,
            ParamMode::Shared => Self::Shared,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub module: String,
    pub permits: Vec<String>,
    pub entrypoint: DeclarationId,
    pub declarations: DeclarationIndex,
    pub types: Vec<ResolvedTypeDeclaration>,
    pub interfaces: Vec<ResolvedInterface>,
    pub function_templates: Vec<ResolvedFunctionTemplate>,
    pub functions: Vec<ResolvedFunction>,
    pub function_instances: Vec<ResolvedFunctionInstance>,
}

/// One real, owned monomorphic function admitted to the private workspace
/// scalar linker.
pub(crate) struct LinkedScalarFunction {
    pub(crate) function: ResolvedFunction,
    pub(crate) origin: IdentityOrigin,
}

/// Phase-A-authenticated declaration identity used only while projecting one
/// exact linked Project closure into an independently validated HIR program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkedDeclarationFact {
    pub(crate) kind: DeclarationKind,
    pub(crate) origin: IdentityOrigin,
    pub(crate) owner: Option<DeclarationId>,
}

/// Exact non-function semantic inventory needed by a Project-v8 closure.
pub(crate) struct LinkedOwnedDataParts {
    pub(crate) permits: Vec<String>,
    pub(crate) types: Vec<ResolvedTypeDeclaration>,
    pub(crate) interfaces: Vec<ResolvedInterface>,
    pub(crate) declaration_facts: BTreeMap<DeclarationId, LinkedDeclarationFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNativeRustImportCall {
    pub expression: ExpressionId,
    pub import: DeclarationId,
    pub args: Vec<ResolvedExpr>,
    pub result: ResolvedImportResultKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedHostCommandOperation {
    ArgsLen,
    ArgUtf8,
    StdinRead,
    StderrWrite,
    StdoutAppend,
    StderrAppend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedHostCommandCall {
    pub expression: ExpressionId,
    pub operation: ResolvedHostCommandOperation,
    pub args: Vec<ResolvedExpr>,
}

impl ResolvedProgram {
    pub fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        match instance {
            None => self
                .functions
                .iter()
                .find(|function| function.id == *callee),
            Some(instance) => self
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance && candidate.template == *callee)
                .map(|candidate| &candidate.function),
        }
    }
}

fn inline_array_payload_bytes(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<u32, Diagnostic> {
    let mut total = 0_u32;
    let mut pending = vec![ty.clone()];
    let mut expanded = 0_usize;
    while let Some(ty) = pending.pop() {
        expanded = expanded
            .checked_add(1)
            .ok_or_else(|| hir_error("inline-array type traversal overflowed"))?;
        if expanded > 65_536 {
            return Err(hir_error(
                "inline-array type traversal exceeds the compiler bound",
            ));
        }
        match ty {
            ResolvedType::ArrayU8(length) => {
                total = total
                    .checked_add(length)
                    .ok_or_else(|| hir_error("inline-array payload calculation overflowed"))?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                let declaration = program
                    .types
                    .iter()
                    .find(|candidate| candidate.id == declaration)
                    .ok_or_else(|| hir_error("inline-array slot references an unknown type"))?;
                let fields = match &declaration.kind {
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => fields.as_slice(),
                    ResolvedTypeDeclarationKind::Variant { .. }
                    | ResolvedTypeDeclarationKind::Resource { .. } => &[],
                };
                for field in fields.iter().rev() {
                    pending.push(substitute_type(&field.ty, &declaration.id, &arguments)?);
                }
            }
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8 => {}
            ResolvedType::TypeParameter { .. } => {
                return Err(hir_error(
                    "inline-array capacity cannot inspect an unresolved type parameter",
                ));
            }
        }
    }
    Ok(total)
}

fn push_array_slot(
    program: &ResolvedProgram,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    identity: String,
    kind: crate::byte_data_capacity::ArrayStorageKind,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    let length = inline_array_payload_bytes(program, ty)?;
    if length != 0 || matches!(ty, ResolvedType::ArrayU8(0)) {
        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
            identity,
            kind,
            length,
        });
    }
    Ok(())
}

fn push_array_pattern_slots(
    program: &ResolvedProgram,
    pattern: &ResolvedMatchPattern,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
) -> Result<(), Diagnostic> {
    match pattern {
        ResolvedMatchPattern::Binding(binding) => push_array_slot(
            program,
            slots,
            binding.id.as_str().to_owned(),
            crate::byte_data_capacity::ArrayStorageKind::Binding,
            &binding.ty,
        ),
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                push_array_slot(
                    program,
                    slots,
                    field.binding.id.as_str().to_owned(),
                    crate::byte_data_capacity::ArrayStorageKind::Binding,
                    &field.binding.ty,
                )?;
            }
            Ok(())
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            let mut pending = fields
                .iter()
                .rev()
                .map(|field| &field.pattern)
                .collect::<Vec<_>>();
            while let Some(pattern) = pending.pop() {
                match pattern {
                    ResolvedRecordMatchFieldPattern::Binding(binding) => push_array_slot(
                        program,
                        slots,
                        binding.id.as_str().to_owned(),
                        crate::byte_data_capacity::ArrayStorageKind::Binding,
                        &binding.ty,
                    )?,
                    ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| &field.pattern));
                    }
                    ResolvedRecordMatchFieldPattern::Wildcard => {}
                }
            }
            Ok(())
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Or(_) => Ok(()),
    }
}

fn byte_slice_transcript_source(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> crate::byte_data_capacity::TranscriptSource {
    use crate::byte_data_capacity::TranscriptSource;
    enum Frame<'a> {
        Visit(&'a ResolvedExpr),
        If,
    }
    let mut frames = vec![Frame::Visit(expression)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(expression) => match &expression.kind {
                ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
                    if let Some(fact) = program.declarations.byte_slice_provenance(&place.root) {
                        results.push(match fact.root_kind {
                            ByteSliceRootKind::CommandArguments => {
                                TranscriptSource::CommandArguments
                            }
                            ByteSliceRootKind::FixedArray => match fact.length {
                                ByteSliceExtent::Constant(length) => {
                                    TranscriptSource::Fixed(length)
                                }
                                ByteSliceExtent::ParameterLength | ByteSliceExtent::ValueLength => {
                                    TranscriptSource::Unknown
                                }
                            },
                            ByteSliceRootKind::OwnedBytes
                                if resolved_value_is_stdin(program, &fact.root) =>
                            {
                                TranscriptSource::Stdin
                            }
                            ByteSliceRootKind::FunctionParameter
                            | ByteSliceRootKind::OwnedBytes
                            | ByteSliceRootKind::BorrowedStr => TranscriptSource::Unknown,
                        });
                    } else {
                        results.push(resolved_value_type(program, &place.root).map_or(
                            TranscriptSource::Unknown,
                            |ty| match ty {
                                ResolvedType::ArrayU8(length) => {
                                    TranscriptSource::Fixed(u64::from(length))
                                }
                                _ => TranscriptSource::Unknown,
                            },
                        ));
                    }
                }
                ResolvedExprKind::Call { callee, args, .. }
                    if callee.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID =>
                {
                    results.push(args.first().map_or(TranscriptSource::Unknown, |argument| {
                        match argument.ty {
                            ResolvedType::ArrayU8(length) => {
                                TranscriptSource::Fixed(u64::from(length))
                            }
                            _ => TranscriptSource::Unknown,
                        }
                    }));
                }
                ResolvedExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    frames.push(Frame::If);
                    frames.push(Frame::Visit(else_branch));
                    frames.push(Frame::Visit(then_branch));
                }
                ResolvedExprKind::Block { tail, .. } => frames.push(Frame::Visit(tail)),
                _ => results.push(TranscriptSource::Unknown),
            },
            Frame::If => {
                let else_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                let then_source = results.pop().unwrap_or(TranscriptSource::Unknown);
                results.push(if then_source == else_source {
                    then_source
                } else {
                    TranscriptSource::Unknown
                });
            }
        }
    }
    results.pop().unwrap_or(TranscriptSource::Unknown)
}

pub(crate) fn push_resolved_expression_children_in_authored_order<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
) {
    match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
        ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev()),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end);
            pending.push(start);
            pending.push(source);
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => pending.push(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right);
            pending.push(left);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch);
            pending.push(then_branch);
            pending.push(condition);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value);
                if let Some(guard) = &arm.guard {
                    pending.push(guard);
                }
            }
            pending.push(scrutinee);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
            pending.push(base);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

fn resolved_value_is_stdin(program: &ResolvedProgram, value: &ValueId) -> bool {
    let in_expression = |root: &ResolvedExpr| {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let {
                        binding,
                        value: initializer,
                        ..
                    } = statement
                    {
                        if binding.id == *value {
                            return matches!(
                                &initializer.kind,
                                ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                                    operation: ResolvedHostCommandOperation::StdinRead,
                                    ..
                                })
                            );
                        }
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
        false
    };
    program
        .functions
        .iter()
        .any(|function| in_expression(&function.body))
        || program
            .function_instances
            .iter()
            .any(|instance| in_expression(&instance.function.body))
}

fn resolved_value_type(program: &ResolvedProgram, value: &ValueId) -> Option<ResolvedType> {
    let in_expression = |root: &ResolvedExpr| {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        if binding.id == *value {
                            return Some(binding.ty.clone());
                        }
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
        None
    };

    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .find_map(|function| {
            function
                .params
                .iter()
                .find(|parameter| parameter.id == *value)
                .map(|parameter| parameter.ty.clone())
                .or_else(|| in_expression(&function.body))
        })
}
fn byte_capacity_expression(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    slots: &mut Vec<crate::byte_data_capacity::ArrayStorageSlot>,
    direct_destination: bool,
) -> Result<crate::byte_data_capacity::CapacityFlow, Diagnostic> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow};

    enum Frame<'a> {
        Visit(&'a ResolvedExpr, bool),
        Argument(
            &'a ResolvedExpr,
            Option<(String, ArrayStorageKind, ResolvedType)>,
            bool,
        ),
        Sequence(usize),
        Alternative(usize),
        Loop,
        Match(usize),
        Emit(CapacityFlow),
    }
    fn sequence(children: Vec<CapacityFlow>) -> CapacityFlow {
        if children.is_empty() {
            CapacityFlow::Empty
        } else {
            CapacityFlow::Sequence(children)
        }
    }

    let mut frames = vec![Frame::Visit(expression, direct_destination)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Visit(expression, direct_destination) => {
                let payload = inline_array_payload_bytes(program, &expression.ty)?;
                if payload != 0 || matches!(expression.ty, ResolvedType::ArrayU8(0)) {
                    let kind = match &expression.kind {
                        ResolvedExprKind::Call { .. } => Some(ArrayStorageKind::CallStaging),
                        ResolvedExprKind::ArrayU8(_) | ResolvedExprKind::RepeatArrayU8 { .. }
                            if direct_destination =>
                        {
                            None
                        }
                        ResolvedExprKind::Place(_) | ResolvedExprKind::BorrowPlace { .. } => None,
                        ResolvedExprKind::ByteRange { .. } => None,
                        ResolvedExprKind::Block { .. }
                        | ResolvedExprKind::If { .. }
                        | ResolvedExprKind::Match { .. } => None,
                        _ => Some(ArrayStorageKind::Temporary),
                    };
                    if let Some(kind) = kind {
                        slots.push(crate::byte_data_capacity::ArrayStorageSlot {
                            identity: expression.id.as_str().to_owned(),
                            kind,
                            length: payload,
                        });
                    }
                }
                match &expression.kind {
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        ..
                    } => {
                        let effect = if callee.as_str() == crate::byte_ops::COPY_ID {
                            Some(CapacityFlow::BytesCopy {
                                site: expression.id.as_str().to_owned(),
                                conservative_payload_bytes:
                                    crate::byte_data_capacity::MAX_ARRAY_BYTES,
                            })
                        } else if callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID {
                            Some(CapacityFlow::StdoutWrite {
                                site: expression.id.as_str().to_owned(),
                                source: byte_slice_transcript_source(program, &args[0]),
                            })
                        } else if program
                            .resolve_call_target(callee, instance.as_ref())
                            .is_some()
                        {
                            Some(CapacityFlow::Call {
                                site: expression.id.as_str().to_owned(),
                                callee: instance
                                    .as_ref()
                                    .map_or_else(|| callee.as_str(), FunctionInstanceId::as_str)
                                    .to_owned(),
                            })
                        } else {
                            None
                        };
                        frames.push(Frame::Sequence(args.len() + usize::from(effect.is_some())));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for (index, argument) in args.iter().enumerate().rev() {
                            frames.push(Frame::Argument(
                                argument,
                                Some((
                                    format!("{}.arg.{index}", expression.id.as_str()),
                                    ArrayStorageKind::CallStaging,
                                    argument.ty.clone(),
                                )),
                                false,
                            ));
                        }
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frames.push(Frame::Sequence(call.args.len()));
                        for argument in call.args.iter().rev() {
                            frames.push(Frame::Visit(argument, false));
                        }
                    }
                    ResolvedExprKind::HostCommandCall(call) => {
                        let effect = if call.operation == ResolvedHostCommandOperation::StdinRead {
                            Some(CapacityFlow::StdinRead {
                                site: expression.id.as_str().to_owned(),
                                conservative_payload_bytes: crate::command_io_ops::MAX_INPUT_BYTES,
                            })
                        } else if call.operation == ResolvedHostCommandOperation::StderrWrite {
                            Some(CapacityFlow::StderrWrite {
                                site: expression.id.as_str().to_owned(),
                                source: byte_slice_transcript_source(program, &call.args[0]),
                            })
                        } else {
                            None
                        };
                        frames.push(Frame::Sequence(
                            call.args.len() + usize::from(effect.is_some()),
                        ));
                        if let Some(effect) = effect {
                            frames.push(Frame::Emit(effect));
                        }
                        for argument in call.args.iter().rev() {
                            frames.push(Frame::Visit(argument, false));
                        }
                    }
                    ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } => {
                        frames.push(Frame::Sequence(3));
                        frames.push(Frame::Visit(end, false));
                        frames.push(Frame::Visit(start, false));
                        frames.push(Frame::Visit(source, false));
                    }
                    ResolvedExprKind::Unary { value, .. }
                    | ResolvedExprKind::Try { operand: value, .. }
                    | ResolvedExprKind::TryOption { operand: value, .. }
                    | ResolvedExprKind::Project { base: value, .. }
                    | ResolvedExprKind::Upcast { source: value } => {
                        frames.push(Frame::Visit(value, false));
                    }
                    ResolvedExprKind::Binary { left, right, .. } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Visit(right, false));
                        frames.push(Frame::Visit(left, false));
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        frames.push(Frame::Sequence(statements.len() + 1));
                        frames.push(Frame::Visit(tail, direct_destination));
                        for statement in statements.iter().rev() {
                            match statement {
                                ResolvedStatement::Let { binding, value, .. } => {
                                    frames.push(Frame::Argument(
                                        value,
                                        Some((
                                            binding.id.as_str().to_owned(),
                                            ArrayStorageKind::Binding,
                                            binding.ty.clone(),
                                        )),
                                        true,
                                    ));
                                }
                                ResolvedStatement::Assign { value, .. } => {
                                    frames.push(Frame::Visit(value, true));
                                }
                                ResolvedStatement::Unsafe { body, .. } => {
                                    frames.push(Frame::Visit(body, true));
                                }
                                ResolvedStatement::While {
                                    condition, body, ..
                                } => {
                                    frames.push(Frame::Loop);
                                    frames.push(Frame::Visit(body, false));
                                    frames.push(Frame::Visit(condition, false));
                                }
                            }
                        }
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::Sequence(2));
                        frames.push(Frame::Alternative(2));
                        frames.push(Frame::Visit(else_branch, direct_destination));
                        frames.push(Frame::Visit(then_branch, direct_destination));
                        frames.push(Frame::Visit(condition, false));
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. } => {
                        frames.push(Frame::Sequence(fields.len()));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Visit(&field.value, false));
                        }
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        for arm in arms {
                            push_array_pattern_slots(program, &arm.pattern, slots)?;
                        }
                        frames.push(Frame::Match(arms.len()));
                        frames.push(Frame::Visit(scrutinee, false));
                        for arm in arms.iter().rev() {
                            frames.push(Frame::Sequence(1 + usize::from(arm.guard.is_some())));
                            frames.push(Frame::Visit(&arm.value, direct_destination));
                            if let Some(guard) = &arm.guard {
                                frames.push(Frame::Visit(guard, false));
                            }
                        }
                    }
                    ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                        frames.push(Frame::Sequence(1 + fields.len()));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Visit(&field.value, false));
                        }
                        frames.push(Frame::Visit(base, false));
                    }
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::Place(_)
                    | ResolvedExprKind::BorrowPlace { .. } => results.push(CapacityFlow::Empty),
                }
            }
            Frame::Argument(expression, slot, direct_destination) => {
                if let Some((identity, kind, ty)) = slot {
                    push_array_slot(program, slots, identity, kind, &ty)?;
                }
                frames.push(Frame::Visit(expression, direct_destination));
            }
            Frame::Sequence(count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let children = results.drain(start..).collect::<Vec<_>>();
                results.push(sequence(children));
            }
            Frame::Alternative(count) => {
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let children = results.drain(start..).collect::<Vec<_>>();
                results.push(CapacityFlow::Alternative(children));
            }
            Frame::Loop => {
                let body = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let condition = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                results.push(CapacityFlow::Loop {
                    condition: Box::new(condition),
                    body: Box::new(body),
                });
            }
            Frame::Match(arm_count) => {
                let scrutinee = results
                    .pop()
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let start = results
                    .len()
                    .checked_sub(arm_count)
                    .ok_or_else(|| hir_error("byte-capacity traversal stack underflowed"))?;
                let alternatives = results.drain(start..).collect::<Vec<_>>();
                results.push(sequence(vec![
                    scrutinee,
                    CapacityFlow::Alternative(alternatives),
                ]));
            }
            Frame::Emit(flow) => results.push(flow),
        }
    }
    if results.len() == 1 {
        results
            .pop()
            .ok_or_else(|| hir_error("byte-capacity traversal produced no result"))
    } else {
        Err(hir_error(
            "byte-capacity traversal produced an invalid result stack",
        ))
    }
}

pub(crate) fn byte_data_capacity_inputs(
    program: &ResolvedProgram,
) -> Result<Vec<crate::byte_data_capacity::FunctionCapacityInput>, Diagnostic> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow, FunctionCapacityInput};

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| (instance.id.as_str(), &instance.function)),
        );
    functions
        .map(|(identity, function)| {
            let mut slots = Vec::new();
            for parameter in &function.params {
                push_array_slot(
                    program,
                    &mut slots,
                    parameter.id.as_str().to_owned(),
                    ArrayStorageKind::Parameter,
                    &parameter.ty,
                )?;
            }
            push_array_slot(
                program,
                &mut slots,
                function.result_id.as_str().to_owned(),
                ArrayStorageKind::ProvisionalResult,
                &function.return_type,
            )?;
            let mut execution = function
                .requires
                .iter()
                .map(|expression| byte_capacity_expression(program, expression, &mut slots, false))
                .collect::<Result<Vec<_>, _>>()?;
            execution.push(byte_capacity_expression(
                program,
                &function.body,
                &mut slots,
                true,
            )?);
            execution.extend(
                function
                    .ensures
                    .iter()
                    .map(|expression| {
                        byte_capacity_expression(program, expression, &mut slots, false)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(FunctionCapacityInput {
                function: identity.to_owned(),
                array_slots: slots,
                execution: CapacityFlow::Sequence(execution),
            })
        })
        .collect()
}

pub(crate) fn analyze_byte_data_capacity(
    program: &ResolvedProgram,
) -> Result<crate::byte_data_capacity::ProgramCapacitySummary, Diagnostic> {
    let inputs = byte_data_capacity_inputs(program)?;
    crate::byte_data_capacity::analyze(&inputs)
        .map_err(|error| Diagnostic::io(error.diagnostic.code(), error.to_string()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub kind: ResolvedTypeDeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameterDeclaration {
    pub name: String,
    pub index: u32,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeDeclarationKind {
    Resource {
        drop: ResolvedResourceDrop,
    },
    Record {
        fields: Vec<ResolvedFieldDeclaration>,
    },
    Class {
        fields: Vec<ResolvedFieldDeclaration>,
        methods: Vec<DeclarationId>,
    },
    Variant {
        cases: Vec<ResolvedVariantCaseDeclaration>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVariantCaseDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResourceDrop {
    pub id: DeclarationId,
    pub kind: ResolvedResourceDropKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedResourceDropKind {
    Trivial,
    Imported {
        import: DeclarationId,
        import_key: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterface {
    pub id: DeclarationId,
    pub name: String,
    pub permits: Vec<String>,
    pub imports: Vec<ResolvedImport>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub id: DeclarationId,
    pub name: String,
    pub interface: DeclarationId,
    pub import_key: String,
    pub native_rust: bool,
    pub parameters: Vec<ResolvedImportParameter>,
    pub result: ResolvedImportResult,
    pub effects: Vec<String>,
    pub required_authority: Vec<String>,
    pub failure: ResolvedImportFailure,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportParameter {
    pub name: String,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub consumes_on_failure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportResult {
    pub kind: ResolvedImportResultKind,
    pub ownership: OwnershipMode,
    pub producer: &'static str,
    pub out_slot_initialization: &'static str,
    pub ownership_transfer: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportResultKind {
    Unit,
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportFailure {
    Infallible,
    Status {
        domain_id: String,
        normalization: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunction {
    pub id: DeclarationId,
    pub name: String,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub cleanup: CleanupInventory,
    pub cleanup_plan: CleanupPlan,
    pub loan_plan: LoanPlan,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionInstance {
    pub id: FunctionInstanceId,
    pub template: DeclarationId,
    pub type_arguments: Vec<ResolvedType>,
    pub function: ResolvedFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionTemplate {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParam {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinding {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpr {
    pub id: ExpressionId,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub kind: ResolvedExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExprKind {
    Int(i64),
    /// An `i32` literal held as its exact value.
    Int32(i32),
    /// A `char` literal held as its exact Unicode scalar value.
    Char(u32),
    /// A `u8` literal held as its exact value.
    Uint8(u8),
    /// A target-independent `usize` literal held as its exact u64 value.
    Usize(u64),
    ArrayU8(Vec<u8>),
    RepeatArrayU8 {
        value: u8,
        count: u32,
    },
    /// An `f32` literal held as its exact IEEE-754 bit pattern.
    Float32(u32),
    /// An `f64` literal held as its exact IEEE-754 bit pattern.
    Float64(u64),
    Bool(bool),
    /// A string literal held as its exact owned UTF-8 contents.
    String(String),
    Place(Place),
    /// A compiler-owned, non-consuming view of one exact authenticated place.
    BorrowPlace {
        operation: DeclarationId,
        place: Place,
    },
    /// A compiler-owned fallible half-open subview of one exact named slice.
    /// This is deliberately not an ordinary call: its borrowed result and
    /// parent provenance must be independently reconstructable from HIR.
    ByteRange {
        operation: DeclarationId,
        source: Box<ResolvedExpr>,
        start: Box<ResolvedExpr>,
        end: Box<ResolvedExpr>,
    },
    Call {
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        args: Vec<ResolvedExpr>,
    },
    NativeRustImportCall(ResolvedNativeRustImportCall),
    HostCommandCall(ResolvedHostCommandCall),
    Unary {
        op: UnaryOp,
        value: Box<ResolvedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<ResolvedExpr>,
        right: Box<ResolvedExpr>,
    },
    Block {
        statements: Vec<ResolvedStatement>,
        tail: Box<ResolvedExpr>,
    },
    If {
        condition: Box<ResolvedExpr>,
        then_branch: Box<ResolvedExpr>,
        else_branch: Box<ResolvedExpr>,
    },
    ConstructRecord {
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    ConstructVariant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Match {
        mode: ResolvedMatchMode,
        scrutinee: Box<ResolvedExpr>,
        arms: Vec<ResolvedMatchArm>,
    },
    Try {
        operand: Box<ResolvedExpr>,
        result: DeclarationId,
        ok_case: DeclarationId,
        ok_field: DeclarationId,
        err_case: DeclarationId,
        err_field: DeclarationId,
        residual_type: ResolvedType,
    },
    TryOption {
        operand: Box<ResolvedExpr>,
        option: DeclarationId,
        some_case: DeclarationId,
        some_field: DeclarationId,
        none_case: DeclarationId,
        residual_type: ResolvedType,
    },
    UpdateRecord {
        base: Box<ResolvedExpr>,
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Project {
        base: Box<ResolvedExpr>,
        field: DeclarationId,
    },
    /// Class Inheritance v1: implicit prefix upcast of an owned descendant
    /// class value to an ancestor class value. The source is consumed; its
    /// inherited leaves transfer into the ancestor-typed result, so the
    /// child-declared suffix must be cleanup-inert (checked at resolution).
    /// Backends copy the ancestor prefix field-by-field from the source.
    Upcast {
        source: Box<ResolvedExpr>,
    },
}

impl ResolvedMatchArm {
    /// Refutable Match v1 interpreter admission: literal and or-of-literal
    /// patterns are refutable; wildcard/binding are irrefutable; aggregate
    /// patterns never join the scalar profile.
    pub fn pattern_is_literal_or_irrefutable(&self) -> bool {
        match &self.pattern {
            ResolvedMatchPattern::Wildcard
            | ResolvedMatchPattern::Binding(_)
            | ResolvedMatchPattern::Literal(_) => true,
            ResolvedMatchPattern::Or(alternatives) => alternatives
                .iter()
                .all(|alternative| matches!(alternative, ResolvedMatchPattern::Literal(_))),
            ResolvedMatchPattern::Variant { .. } | ResolvedMatchPattern::Record { .. } => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchArm {
    pub pattern: ResolvedMatchPattern,
    /// Refutable Match v1: ordinary bool expression evaluated once after the
    /// pattern matches; a false result falls through to the following arms.
    /// `None` for every pre-feature arm.
    pub guard: Option<Box<ResolvedExpr>>,
    pub value: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedMatchPattern {
    Variant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedMatchPatternField>,
    },
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
    Wildcard,
    /// Refutable Match v1: one exact scalar literal compared against the
    /// scrutinee with exact equality. The literal's type equals the
    /// scrutinee type; floats are never admitted.
    Literal(PatternValue),
    /// Refutable Match v1: `a | b` flattened to same-typed literal
    /// alternatives. Never empty; nesting is rejected at resolution.
    Or(Vec<ResolvedMatchPattern>),
    /// Refutable Match v1: irrefutable whole-scrutinee binding of a Copy
    /// scalar.
    Binding(ResolvedBinding),
}

/// The exact scalar value carried by [`ResolvedMatchPattern::Literal`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternValue {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Usize(u64),
    Char(u32),
    Bool(bool),
}

impl PatternValue {
    pub fn from_ast(value: crate::ast::PatternLiteral) -> Self {
        match value {
            crate::ast::PatternLiteral::Int(inner) => Self::Int(inner),
            crate::ast::PatternLiteral::Int32(inner) => Self::Int32(inner),
            crate::ast::PatternLiteral::Uint8(inner) => Self::Uint8(inner),
            crate::ast::PatternLiteral::Usize(inner) => Self::Usize(inner),
            crate::ast::PatternLiteral::Char(inner) => Self::Char(inner),
            crate::ast::PatternLiteral::Bool(inner) => Self::Bool(inner),
        }
    }

    /// The scalar scrutinee type this literal compares against.
    pub fn ty(&self) -> ResolvedType {
        match self {
            Self::Int(_) => ResolvedType::I64,
            Self::Int32(_) => ResolvedType::I32,
            Self::Uint8(_) => ResolvedType::U8,
            Self::Usize(_) => ResolvedType::Usize,
            Self::Char(_) => ResolvedType::Char,
            Self::Bool(_) => ResolvedType::Bool,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchPatternField {
    pub field: DeclarationId,
    pub binding: ResolvedBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRecordMatchPatternField {
    pub field: DeclarationId,
    pub pattern: ResolvedRecordMatchFieldPattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedRecordMatchFieldPattern {
    Binding(ResolvedBinding),
    Wildcard,
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldInitializer {
    pub field: DeclarationId,
    pub value: ResolvedExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Let {
        binding: ResolvedBinding,
        /// Explicit Mutation v1: `true` when the source declared `let mut`.
        mutable: bool,
        value: ResolvedExpr,
        span: Span,
    },
    /// Explicit Mutation v1: `<binding> = <expr>;`. The target reuses the
    /// original binding's [`ValueId`]; no new value identity is created.
    /// Field Mutation v1: `field` names the one direct scalar field of a
    /// `<binding>.<field>` target; the store replaces that whole field.
    Assign {
        binding: ResolvedBinding,
        field: Option<DeclarationId>,
        value: ResolvedExpr,
        span: Span,
    },
    /// Unsafe Boundary Mechanics v1: `@audit("...") unsafe { ... }`. The body
    /// is an ordinary checked safe block expression; the audit summary is
    /// recorded verbatim. No raw pointers or memory operations exist.
    Unsafe {
        audit: String,
        body: Box<ResolvedExpr>,
        span: Span,
    },
    /// Bounded While-Loops v1: `while <condition> { <body> }`. The condition
    /// must be exactly `bool` and the body is an ordinary checked block whose
    /// value is discarded. The statement produces no value.
    While {
        condition: Box<ResolvedExpr>,
        body: Box<ResolvedExpr>,
        span: Span,
    },
}

impl ResolvedStatement {
    /// The statement's evaluated expression. While statements carry two
    /// evaluated expressions and must be traversed with
    /// [`ResolvedStatement::child`] instead.
    pub fn value(&self) -> &ResolvedExpr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// Mutable access to the statement's evaluated expression.
    pub fn value_mut(&mut self) -> &mut ResolvedExpr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// The statement's target or declared binding. Only `let` and assignment
    /// statements carry one.
    pub fn binding(&self) -> &ResolvedBinding {
        match self {
            Self::Let { binding, .. } | Self::Assign { binding, .. } => binding,
            Self::Unsafe { .. } | Self::While { .. } => {
                panic!("only let and assignment statements declare a binding")
            }
        }
    }

    /// The verbatim audit summary of an unsafe boundary statement.
    pub fn audit(&self) -> Option<&str> {
        match self {
            Self::Unsafe { audit, .. } => Some(audit),
            _ => None,
        }
    }

    /// `true` for assignment statements.
    pub fn is_assign(&self) -> bool {
        matches!(self, Self::Assign { .. })
    }

    /// Number of directly nested evaluated expressions. `let`, assignment,
    /// and unsafe statements contribute one; while statements contribute its
    /// condition then its body, in evaluation order.
    pub fn child_count(&self) -> usize {
        match self {
            Self::Let { .. } | Self::Assign { .. } | Self::Unsafe { .. } => 1,
            Self::While { .. } => 2,
        }
    }

    /// One directly nested evaluated expression in left-to-right order.
    pub fn child(&self, index: usize) -> Option<&ResolvedExpr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => (index == 0).then_some(value),
            Self::Unsafe { body, .. } => (index == 0).then_some(body.as_ref()),
            Self::While {
                condition, body, ..
            } => [condition.as_ref(), body.as_ref()].get(index).copied(),
        }
    }
}

fn derive_byte_slice_provenance(
    functions: &[ResolvedFunction],
    declarations: &DeclarationIndex,
) -> Result<BTreeMap<ValueId, ByteSliceProvenance>, Diagnostic> {
    let mut facts = BTreeMap::new();
    let mut root_types = BTreeMap::<ValueId, ResolvedType>::new();
    let command_argument_root =
        ValueId::intrinsic_parameter(crate::command_io_ops::ARG_UTF8_ID, usize::MAX);
    let mut command_argument_views = BTreeSet::<ValueId>::new();
    let mut aliases = Vec::<(&ResolvedBinding, bool, &ResolvedExpr)>::new();
    for function in functions {
        for parameter in &function.params {
            root_types.insert(parameter.id.clone(), parameter.ty.clone());
            if parameter.ty == ResolvedType::SliceU8 {
                facts.insert(
                    parameter.id.clone(),
                    ByteSliceProvenance {
                        root: parameter.id.clone(),
                        projections: Vec::new(),
                        projected_type: ResolvedType::SliceU8,
                        root_kind: ByteSliceRootKind::FunctionParameter,
                        root_length: ByteSliceExtent::ParameterLength,
                        offset: ByteSliceExtent::Constant(0),
                        length: ByteSliceExtent::ParameterLength,
                        producer: None,
                        ranges: Vec::new(),
                    },
                );
            }
        }
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ResolvedExprKind::Call { args, .. } => pending.extend(args),
                ResolvedExprKind::ByteRange {
                    source, start, end, ..
                } => {
                    pending.push(source);
                    pending.push(start);
                    pending.push(end);
                }
                ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
                ResolvedExprKind::HostCommandCall(call) => pending.extend(&call.args),
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Try { operand: value, .. }
                | ResolvedExprKind::TryOption { operand: value, .. }
                | ResolvedExprKind::Project { base: value, .. }
                | ResolvedExprKind::Upcast { source: value } => pending.push(value),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                ResolvedExprKind::Block { statements, tail } => {
                    pending.push(tail);
                    for statement in statements {
                        if let ResolvedStatement::Let {
                            binding,
                            mutable,
                            value,
                            ..
                        } = statement
                        {
                            root_types.insert(binding.id.clone(), binding.ty.clone());
                            if matches!(
                                &value.kind,
                                ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                                    operation: ResolvedHostCommandOperation::ArgUtf8,
                                    ..
                                })
                            ) {
                                command_argument_views.insert(binding.id.clone());
                            }
                            if binding.ty == ResolvedType::SliceU8 {
                                aliases.push((binding, *mutable, value));
                            }
                        }
                        for index in 0..statement.child_count() {
                            pending.push(
                                statement
                                    .child(index)
                                    .expect("statement child count is canonical"),
                            );
                        }
                    }
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(condition);
                    pending.push(then_branch);
                    pending.push(else_branch);
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    pending.extend(fields.iter().map(|field| &field.value));
                }
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    pending.push(scrutinee);
                    for arm in arms {
                        if let Some(guard) = &arm.guard {
                            pending.push(guard);
                        }
                        pending.push(&arm.value);
                    }
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    pending.push(base);
                    pending.extend(fields.iter().map(|field| &field.value));
                }
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::Place(_)
                | ResolvedExprKind::BorrowPlace { .. } => {}
            }
        }
    }
    let mut unresolved = aliases;
    loop {
        let before = unresolved.len();
        unresolved.retain(|(binding, mutable, value)| {
            if let ResolvedExprKind::BorrowPlace { operation, place } = &value.kind {
                if *mutable {
                    return true;
                }
                let Some(root_ty) = root_types.get(&place.root) else {
                    return true;
                };
                let (root_kind, root_length, projected_type) = match root_ty {
                    ResolvedType::Bytes => (
                        ByteSliceRootKind::OwnedBytes,
                        ByteSliceExtent::ValueLength,
                        ResolvedType::Bytes,
                    ),
                    ResolvedType::ArrayU8(length) => (
                        ByteSliceRootKind::FixedArray,
                        ByteSliceExtent::Constant(u64::from(*length)),
                        root_ty.clone(),
                    ),
                    ResolvedType::Str => {
                        if command_argument_views.contains(&place.root) {
                            (
                                ByteSliceRootKind::CommandArguments,
                                ByteSliceExtent::ValueLength,
                                ResolvedType::Str,
                            )
                        } else {
                            (
                                ByteSliceRootKind::BorrowedStr,
                                ByteSliceExtent::ValueLength,
                                ResolvedType::Str,
                            )
                        }
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } if arguments.is_empty() && place.projections.len() == 1 => {
                        let PlaceProjection::Field(field) = &place.projections[0] else {
                            return true;
                        };
                        let Some(fields) = declarations.record_fields(declaration) else {
                            return true;
                        };
                        let Some(resolved) = fields.iter().find(|candidate| candidate.id == *field)
                        else {
                            return true;
                        };
                        if resolved.ty != ResolvedType::Bytes {
                            return true;
                        }
                        (
                            ByteSliceRootKind::OwnedBytes,
                            ByteSliceExtent::ValueLength,
                            ResolvedType::Bytes,
                        )
                    }
                    _ => return true,
                };
                if place.projections.is_empty() != !matches!(root_ty, ResolvedType::Nominal { .. })
                {
                    return true;
                }
                let expected_operation = match &projected_type {
                    ResolvedType::Bytes => crate::byte_ops::BYTES_AS_SLICE_ID,
                    ResolvedType::ArrayU8(_) => crate::byte_ops::ARRAY_AS_SLICE_ID,
                    ResolvedType::Str => crate::byte_ops::STR_AS_BYTES_ID,
                    _ => return true,
                };
                if operation.as_str() != expected_operation {
                    return true;
                }
                facts.insert(
                    binding.id.clone(),
                    ByteSliceProvenance {
                        root: if root_kind == ByteSliceRootKind::CommandArguments {
                            command_argument_root.clone()
                        } else {
                            place.root.clone()
                        },
                        projections: place.projections.clone(),
                        projected_type,
                        root_kind,
                        root_length,
                        offset: ByteSliceExtent::Constant(0),
                        length: root_length,
                        producer: Some(value.id.clone()),
                        ranges: Vec::new(),
                    },
                );
                return false;
            }
            if let ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } = &value.kind
            {
                if *mutable
                    || operation.as_str() != crate::byte_ops::RANGE_ID
                    || value.ty != ResolvedType::SliceU8
                    || value.ownership != OwnershipMode::Borrow
                    || start.ty != ResolvedType::Usize
                    || end.ty != ResolvedType::Usize
                    || start.ownership != OwnershipMode::Value
                    || end.ownership != OwnershipMode::Value
                {
                    return true;
                }
                let ResolvedExprKind::Place(place) = &source.kind else {
                    return true;
                };
                if !place.projections.is_empty()
                    || source.ty != ResolvedType::SliceU8
                    || source.ownership != OwnershipMode::Borrow
                {
                    return true;
                }
                let Some(mut provenance) = facts.get(&place.root).cloned() else {
                    return true;
                };
                if provenance.ranges.len() >= crate::byte_ops::MAX_RANGE_DEPTH {
                    return true;
                }
                provenance.producer = Some(value.id.clone());
                provenance.ranges.push(ByteSliceRangeStep {
                    source: place.root.clone(),
                    producer: value.id.clone(),
                    start: start.id.clone(),
                    end: end.id.clone(),
                });
                facts.insert(binding.id.clone(), provenance);
                return false;
            }
            let ResolvedExprKind::Place(place) = &value.kind else {
                return true;
            };
            if *mutable || !place.projections.is_empty() {
                return true;
            }
            let Some(source) = facts.get(&place.root).cloned() else {
                return true;
            };
            facts.insert(binding.id.clone(), source);
            false
        });
        if unresolved.is_empty() {
            return Ok(facts);
        }
        if unresolved.len() == before {
            return Err(hir_error(
                "byte-slice alias lacks a canonical symbolic parameter root",
            ));
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub root: ValueId,
    pub projections: Vec<PlaceProjection>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlaceProjection {
    Field(DeclarationId),
    VariantField {
        case: DeclarationId,
        field: DeclarationId,
    },
}

#[derive(Clone)]
struct Binding {
    id: ValueId,
    ty: ResolvedType,
    ownership: OwnershipMode,
    /// Explicit Mutation v1: only local `let mut` bindings are mutable.
    mutable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::MaybeMoved
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidationBinding {
    ty: ResolvedType,
    ownership: OwnershipMode,
    availability: Availability,
    moved_places: BTreeMap<Vec<PlaceProjection>, Availability>,
    definitely_partial: BTreeSet<Vec<PlaceProjection>>,
    /// Exact shared loans currently protecting this storage root.
    active_loans: BTreeSet<LoanId>,
}

/// Restores the most recently published ownership scope on every early
/// validation return. Iterative continuations update the publication boundary
/// before entering a direct child; isolated Block/branch/arm children leave it
/// at their outer baseline.
struct ValidationScopePublication<'a> {
    target: &'a mut BTreeMap<ValueId, ValidationBinding>,
    published: BTreeMap<ValueId, ValidationBinding>,
    enabled: bool,
}

impl ValidationScopePublication<'_> {
    fn publish(&mut self, scope: &BTreeMap<ValueId, ValidationBinding>) {
        if self.enabled {
            self.published.clone_from(scope);
        }
    }
}

impl Drop for ValidationScopePublication<'_> {
    fn drop(&mut self) {
        std::mem::swap(self.target, &mut self.published);
    }
}

/// Verify and resolve a parsed program into deterministic HIR.
///
/// Verification errors are returned unchanged. This makes the HIR boundary
/// fail closed: no backend can accidentally resolve and execute an invalid AST.
pub fn resolve(program: &Program) -> Result<ResolvedProgram, Vec<Diagnostic>> {
    let Analysis {
        diagnostics,
        resolved,
    } = analyze(program);
    resolved.ok_or(diagnostics)
}

/// The source diagnostics and optional resolved meaning from one analysis.
///
/// Warnings do not prevent resolution. Any source error fails closed before the
/// resolver runs, so invalid source cannot leak internal HIR diagnostics.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved: Option<ResolvedProgram>,
}

/// Verify source once and resolve it when only warnings remain.
pub fn analyze(program: &Program) -> Analysis {
    let diagnostics = source_verify::verify(program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Analysis {
            diagnostics,
            resolved: None,
        };
    }

    let declarations = match DeclarationIndex::from_verified(program) {
        Ok(declarations) => declarations,
        Err(diagnostic) => {
            return Analysis {
                diagnostics: vec![diagnostic],
                resolved: None,
            };
        }
    };
    match (Resolver {
        program,
        declarations,
    })
    .resolve()
    {
        Ok(resolved) => Analysis {
            diagnostics,
            resolved: Some(resolved),
        },
        Err(diagnostic) => Analysis {
            // Preserve `resolve`'s established invariant-failure behavior: an
            // internal HIR diagnostic replaces otherwise non-fatal warnings.
            diagnostics: vec![diagnostic],
            resolved: None,
        },
    }
}

fn hir_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

struct Resolver<'a> {
    program: &'a Program,
    declarations: DeclarationIndex,
}

impl Resolver<'_> {
    fn resolve(self) -> Result<ResolvedProgram, Diagnostic> {
        let entrypoint = self
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .map(|function| DeclarationId::new(function.stable_id.clone()))
            .ok_or_else(|| {
                self.error(
                    "SPX-H005",
                    "verified program has no resolved entry point",
                    Span::default(),
                )
            })?;
        self.validate_record_layouts()?;
        let types = self
            .program
            .types
            .iter()
            .chain(crate::prelude::declarations())
            .map(|declaration| {
                let id = DeclarationId::new(declaration.stable_id.clone());
                let kind = match &declaration.kind {
                    TypeDeclarationKind::Resource { lifecycles } => {
                        let lifecycle = lifecycles.first().ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("resource `{id}` has no resolved lifecycle"),
                                declaration.span,
                            )
                        })?;
                        let lifecycle_id = DeclarationId::new(
                            lifecycle.stable_id.clone().ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("resource `{id}` lifecycle has no identity"),
                                    lifecycle.span,
                                )
                            })?,
                        );
                        let drop_kind = match &lifecycle.kind {
                            ResourceLifecycleKind::Trivial => ResolvedResourceDropKind::Trivial,
                            ResourceLifecycleKind::Imported { import_key } => {
                                let import = self
                                    .declarations
                                    .import_id(import_key)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            format!(
                                                "resource `{id}` lifecycle references unknown import key `{import_key}`"
                                            ),
                                            lifecycle.span,
                                        )
                                    })?;
                                ResolvedResourceDropKind::Imported {
                                    import,
                                    import_key: import_key.clone(),
                                }
                            }
                        };
                        ResolvedTypeDeclarationKind::Resource {
                            drop: ResolvedResourceDrop {
                                id: lifecycle_id,
                                kind: drop_kind,
                            },
                        }
                    }
                    TypeDeclarationKind::Record { .. } => {
                        let fields = self
                            .declarations
                            .record_fields(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("record `{id}` has no resolved fields"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Record { fields }
                    }
                    TypeDeclarationKind::Class { methods, .. } => {
                        let fields = self
                            .declarations
                            .record_fields(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("class `{id}` has no resolved fields"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        let methods = methods
                            .iter()
                            .map(|method| DeclarationId::new(method.stable_id.clone()))
                            .collect();
                        ResolvedTypeDeclarationKind::Class { fields, methods }
                    }
                    TypeDeclarationKind::Variant { .. } => {
                        let cases = self
                            .declarations
                            .variant_cases(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("variant `{id}` has no resolved cases"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Variant { cases }
                    }
                };
                Ok(ResolvedTypeDeclaration {
                    type_parameters: self
                        .declarations
                        .type_parameters(&id)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("type `{id}` has no parameter metadata"),
                                declaration.span,
                            )
                        })?
                        .to_vec(),
                    id,
                    name: declaration.name.clone(),
                    kind,
                    span: declaration.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let interfaces = self
            .program
            .interfaces
            .iter()
            .map(|interface| {
                let interface_id = DeclarationId::new(interface.stable_id.clone());
                let imports = interface
                    .imports
                    .iter()
                    .map(|import| {
                        let parameters = import
                            .params
                            .iter()
                            .map(|param| {
                                Ok(ResolvedImportParameter {
                                    name: param.name.clone(),
                                    ty: self.resolve_type(&param.ty, param.span)?,
                                    ownership: param.mode.into(),
                                    consumes_on_failure: param.name == import.consumes,
                                })
                            })
                            .collect::<Result<Vec<_>, Diagnostic>>()?;
                        let failure = match &import.failure {
                            ImportFailure::Infallible => ResolvedImportFailure::Infallible,
                            ImportFailure::Status { domain_id } => ResolvedImportFailure::Status {
                                domain_id: domain_id.clone(),
                                normalization: "semaprax.status.v1",
                            },
                        };
                        Ok(ResolvedImport {
                            id: DeclarationId::new(import.stable_id.clone()),
                            name: import.name.clone(),
                            interface: interface_id.clone(),
                            import_key: import.stable_id.clone(),
                            native_rust: import.native_rust,
                            parameters,
                            result: ResolvedImportResult {
                                kind: match import.result {
                                    crate::ast::ImportResult::Unit => {
                                        ResolvedImportResultKind::Unit
                                    }
                                    crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                                    crate::ast::ImportResult::Bool => {
                                        ResolvedImportResultKind::Bool
                                    }
                                },
                                ownership: OwnershipMode::Value,
                                producer: "callee",
                                out_slot_initialization: "success_only",
                                ownership_transfer: "final_zero_status_commit",
                            },
                            effects: import.effects.clone(),
                            required_authority: import.effects.clone(),
                            failure,
                            span: import.span,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                Ok(ResolvedInterface {
                    id: interface_id,
                    name: interface.name.clone(),
                    permits: interface.permits.clone(),
                    imports,
                    span: interface.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut functions: Vec<ResolvedFunction> = self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
            .map(|function| self.resolve_function(function))
            .collect::<Result<_, _>>()?;
        for decl in &self.program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &decl.kind {
                for method in methods {
                    if method.type_parameters.is_empty() {
                        functions.push(self.resolve_function(method)?);
                    }
                }
            }
        }
        let function_templates = self
            .program
            .functions
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| self.resolve_function_template(function))
            .collect::<Result<_, _>>()?;
        let function_instances = self.discover_function_instances()?;
        let byte_slice_roots = derive_byte_slice_provenance(&functions, &self.declarations)?;
        let mut declarations = self.declarations;
        declarations.byte_slice_roots = byte_slice_roots;
        let mut resolved = ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            entrypoint,
            declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        };
        analyze_byte_data_capacity(&resolved)?;
        let loan_plans = resolved
            .functions
            .iter()
            .map(|function| crate::loan_plan::build_plan(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, loan_plan) in resolved.functions.iter_mut().zip(loan_plans) {
            function.loan_plan = loan_plan;
        }
        let instance_loan_plans = resolved
            .function_instances
            .iter()
            .map(|instance| crate::loan_plan::build_plan(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, loan_plan) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_loan_plans)
        {
            instance.function.loan_plan = loan_plan;
        }
        let inventories = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup::build_inventory(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, inventory) in resolved.functions.iter_mut().zip(inventories) {
            function.cleanup = inventory;
        }
        let instance_inventories = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup::build_inventory(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, inventory) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_inventories)
        {
            instance.function.cleanup = inventory;
        }
        let cleanup_plans = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup_plan::build_plan(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, cleanup_plan) in resolved.functions.iter_mut().zip(cleanup_plans) {
            function.cleanup_plan = cleanup_plan;
        }
        let instance_cleanup_plans = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup_plan::build_plan(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, cleanup_plan) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_cleanup_plans)
        {
            instance.function.cleanup_plan = cleanup_plan;
        }
        validate(&resolved)?;
        Ok(resolved)
    }

    fn validate_record_layouts(&self) -> Result<(), Diagnostic> {
        for declaration in &self.program.types {
            if !matches!(
                &declaration.kind,
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. }
            ) {
                continue;
            }
            if !declaration.type_parameters.is_empty() {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(declaration.stable_id.clone()),
                arguments: Vec::new(),
            };
            if self.declarations.type_facts(&ty).is_none() {
                return Err(self.error(
                    "SPX-T217",
                    format!(
                        "record `{}` has an illegal by-value recursive layout",
                        declaration.name
                    ),
                    declaration.span,
                ));
            }
        }
        Ok(())
    }

    fn resolve_function(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let template_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(template_id.clone());
        self.resolve_function_in_scope(function, &function_scope, template_id)
    }

    fn resolve_function_template(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunctionTemplate, Diagnostic> {
        let function_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(function_id.clone());
        let type_parameters = function
            .type_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                Ok(ResolvedTypeParameterDeclaration {
                    name: parameter.name.clone(),
                    index: u32::try_from(index).map_err(|_| {
                        self.error(
                            "SPX-H006",
                            format!("function `{}` has too many type parameters", function.name),
                            parameter.span,
                        )
                    })?,
                    span: parameter.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_function_type(function, &param.ty, param.span)?;
                let id = ValueId::parameter(&function_scope, index);
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership: OwnershipMode::Value,
                        mutable: false,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership: OwnershipMode::Value,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type =
            self.resolve_function_type(function, &function.return_type, function.span)?;
        let result_id = ValueId::result(&function_scope);
        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(&function_scope, &function.body, &bindings, "body")?;
        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: OwnershipMode::Value,
                mutable: false,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(ResolvedFunctionTemplate {
            id: function_id,
            name: function.name.clone(),
            type_parameters,
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            span: function.span,
        })
    }

    fn discover_function_instances(&self) -> Result<Vec<ResolvedFunctionInstance>, Diagnostic> {
        let mut calls = Vec::new();
        for function in self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
        {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                expression.visit_call_instances(&mut |name, arguments, span| {
                    calls.push((name.to_owned(), arguments.to_vec(), span));
                });
            }
        }

        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        for (name, source_arguments, span) in calls {
            let Some(template) = self
                .program
                .functions
                .iter()
                .find(|function| function.name == name && !function.type_parameters.is_empty())
            else {
                continue;
            };
            let type_arguments = source_arguments
                .iter()
                .map(|argument| self.resolve_type(argument, span))
                .collect::<Result<Vec<_>, _>>()?;
            let template_id = DeclarationId::new(template.stable_id.clone());
            let id = FunctionInstanceId::derive(&template_id, &type_arguments);
            if !seen.insert(id.clone()) {
                continue;
            }
            let specialized =
                specialize_source_function(template, &source_arguments).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("generic function `{}` specialization failed", template.name),
                        span,
                    )
                })?;
            let execution = FunctionExecutionId::Generic(id.clone());
            let function =
                self.resolve_function_in_scope(&specialized, &execution, template_id.clone())?;
            instances.push(ResolvedFunctionInstance {
                id,
                template: template_id,
                type_arguments,
                function,
            });
        }
        Ok(instances)
    }

    fn resolve_function_in_scope(
        &self,
        function: &crate::ast::Function,
        function_scope: &FunctionExecutionId,
        function_id: DeclarationId,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_type(&param.ty, param.span)?;
                let id = ValueId::parameter(function_scope, index);
                // Owned strings are non-Copy: even a by-value `string`
                // parameter carries unique ownership.
                let ownership = if ty.is_uniquely_owned() {
                    OwnershipMode::Own
                } else if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8) {
                    if param.mode != ParamMode::Borrow {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved borrowed-view parameter must have borrow ownership",
                            param.span,
                        ));
                    }
                    OwnershipMode::Borrow
                } else {
                    param.mode.into()
                };
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
                        mutable: false,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type = self.resolve_type(&function.return_type, function.span)?;
        if return_type == ResolvedType::Str {
            return Err(self.error(
                "SPX-H006",
                "borrowed `str` cannot escape through a function result",
                function.span,
            ));
        }
        if return_type == ResolvedType::SliceU8 {
            return Err(self.error(
                "SPX-H006",
                "borrowed `Slice<u8>` cannot escape through a function result",
                function.span,
            ));
        }
        let result_id = ValueId::result(function_scope);

        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(function_scope, &function.body, &bindings, "body")?;

        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: self.expression_ownership(
                    &return_type,
                    OwnershipMode::Own,
                    function.span,
                )?,
                mutable: false,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(ResolvedFunction {
            id: function_id,
            name: function.name.clone(),
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            cleanup: CleanupInventory::unresolved(),
            cleanup_plan: CleanupPlan::unresolved(),
            loan_plan: LoanPlan::unresolved(),
            span: function.span,
        })
    }

    fn resolve_type(&self, ty: &Type, span: Span) -> Result<ResolvedType, Diagnostic> {
        enum Frame<'a> {
            Enter(&'a Type),
            Arguments {
                declaration: DeclarationId,
                arguments: &'a [Type],
                index: usize,
                resolved: Vec<ResolvedType>,
            },
        }
        let mut frames = vec![Frame::Enter(ty)];
        let mut result = None;
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(Type::I64) => result = Some(ResolvedType::I64),
                Frame::Enter(Type::I32) => result = Some(ResolvedType::I32),
                Frame::Enter(Type::Char) => result = Some(ResolvedType::Char),
                Frame::Enter(Type::U8) => result = Some(ResolvedType::U8),
                Frame::Enter(Type::Usize) => result = Some(ResolvedType::Usize),
                Frame::Enter(Type::ArrayU8(length)) => {
                    result = Some(ResolvedType::ArrayU8(*length));
                }
                Frame::Enter(Type::F32) => result = Some(ResolvedType::F32),
                Frame::Enter(Type::F64) => result = Some(ResolvedType::F64),
                Frame::Enter(Type::Bool) => result = Some(ResolvedType::Bool),
                Frame::Enter(Type::String) => result = Some(ResolvedType::String),
                Frame::Enter(Type::Bytes) => result = Some(ResolvedType::Bytes),
                Frame::Enter(Type::Str) => result = Some(ResolvedType::Str),
                Frame::Enter(Type::SliceU8) => result = Some(ResolvedType::SliceU8),
                Frame::Enter(Type::Named { name, arguments }) => {
                    let declaration =
                        self.declarations.type_id(name).cloned().ok_or_else(|| {
                            self.error("SPX-H001", format!("unresolved type `{name}`"), span)
                        })?;
                    frames.push(Frame::Arguments {
                        declaration,
                        arguments,
                        index: 0,
                        resolved: Vec::with_capacity(arguments.len()),
                    });
                }
                Frame::Arguments {
                    declaration,
                    arguments,
                    index,
                    mut resolved,
                } => {
                    if index != 0 {
                        resolved.push(result.take().expect("resolved child type retained"));
                    }
                    if let Some(argument) = arguments.get(index) {
                        frames.push(Frame::Arguments {
                            declaration,
                            arguments,
                            index: index + 1,
                            resolved,
                        });
                        frames.push(Frame::Enter(argument));
                    } else {
                        let parameters = self
                            .declarations
                            .type_parameters(&declaration)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("type `{declaration}` has no parameter metadata"),
                                    span,
                                )
                            })?;
                        if resolved.len() != parameters.len()
                            || (!admitted_owned_byte_prelude_instance(&declaration, &resolved)
                                && !resolved.is_empty()
                                && resolved.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                }))
                        {
                            return Err(self.error(
                                "SPX-H006",
                                format!("type `{declaration}` has invalid concrete arguments"),
                                span,
                            ));
                        }
                        result = Some(ResolvedType::Nominal {
                            declaration,
                            arguments: resolved,
                        });
                    }
                }
            }
        }
        Ok(result.expect("root type resolution produces a value"))
    }

    fn resolve_function_type(
        &self,
        function: &crate::ast::Function,
        ty: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                if let Some(index) = function
                    .type_parameters
                    .iter()
                    .position(|parameter| parameter.name == *name)
                {
                    return Ok(ResolvedType::TypeParameter {
                        owner: DeclarationId::new(function.stable_id.clone()),
                        index: u32::try_from(index).map_err(|_| {
                            self.error(
                                "SPX-H006",
                                format!(
                                    "function `{}` type parameter index does not fit u32",
                                    function.name
                                ),
                                span,
                            )
                        })?,
                    });
                }
            }
        }
        self.resolve_type(ty, span)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_record_match_pattern(
        &self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        type_name: &str,
        fields: &[crate::ast::RecordMatchPatternField],
        bindings: &mut BTreeMap<String, Binding>,
        path: &str,
        span: Span,
        mode: ResolvedMatchMode,
    ) -> Result<ResolvedMatchPattern, Diagnostic> {
        enum Frame<'a> {
            Enter {
                expected: ResolvedType,
                type_name: &'a str,
                fields: &'a [crate::ast::RecordMatchPatternField],
                path: String,
                span: Span,
            },
            Fields {
                expected: ResolvedType,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                templates: &'a [ResolvedFieldDeclaration],
                fields: &'a [crate::ast::RecordMatchPatternField],
                index: usize,
                resolved: Vec<ResolvedRecordMatchPatternField>,
                path: String,
            },
            AfterNested {
                expected: ResolvedType,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                templates: &'a [ResolvedFieldDeclaration],
                fields: &'a [crate::ast::RecordMatchPatternField],
                index: usize,
                resolved: Vec<ResolvedRecordMatchPatternField>,
                path: String,
                field: DeclarationId,
            },
        }
        let mut frames = vec![Frame::Enter {
            expected: expected.clone(),
            type_name,
            fields,
            path: path.to_owned(),
            span,
        }];
        let mut results = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter {
                    expected,
                    type_name,
                    fields,
                    path,
                    span,
                } => {
                    let ResolvedType::Nominal {
                        declaration: record,
                        arguments,
                    } = &expected
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            "record pattern has a non-record concrete instance",
                            span,
                        ));
                    };
                    if self.declarations.type_id(type_name) != Some(record)
                        || self
                            .declarations
                            .declaration(record)
                            .is_none_or(|item| item.kind != DeclarationKind::Record)
                    {
                        return Err(self.error(
                            "SPX-H001",
                            format!("record pattern `{type_name}` does not match `{record}`"),
                            span,
                        ));
                    }
                    let templates = self.declarations.record_fields(record).ok_or_else(|| {
                        self.error("SPX-H006", "record pattern has no fields", span)
                    })?;
                    let record = record.clone();
                    let arguments = arguments.clone();
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        arguments,
                        templates,
                        fields,
                        index: 0,
                        resolved: Vec::with_capacity(fields.len()),
                        path,
                    });
                }
                Frame::Fields {
                    expected,
                    record,
                    arguments,
                    templates,
                    fields,
                    index,
                    mut resolved,
                    path,
                } => {
                    let Some(field) = fields.get(index) else {
                        results.push(ResolvedMatchPattern::Record {
                            record,
                            instance: expected,
                            fields: resolved,
                        });
                        continue;
                    };
                    let field_id = self
                        .declarations
                        .field_id(&record, &field.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved record pattern field `{record}.{}`",
                                    field.name
                                ),
                                field.span,
                            )
                        })?;
                    let template = templates
                        .iter()
                        .find(|candidate| candidate.id == field_id)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("record pattern field `{field_id}` has no template"),
                                field.span,
                            )
                        })?;
                    let field_ty = substitute_type(&template.ty, &record, &arguments)?;
                    let field_path = format!("{path}.field.{index}");
                    match &field.pattern {
                        crate::ast::RecordMatchFieldPattern::Binding { name, span } => {
                            let field_facts =
                                self.declarations.type_facts(&field_ty).ok_or_else(|| {
                                    self.error(
                                        "SPX-H006",
                                        "record pattern field has no authenticated type facts",
                                        *span,
                                    )
                                })?;
                            let ownership = if field_facts.needs_drop {
                                match mode {
                                    ResolvedMatchMode::Own => OwnershipMode::Own,
                                    ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                    ResolvedMatchMode::Value => OwnershipMode::Value,
                                }
                            } else {
                                OwnershipMode::Value
                            };
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &format!("{field_path}.binding")),
                                name: name.clone(),
                                ownership,
                                ty: field_ty.clone(),
                                span: *span,
                            };
                            bindings.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: field_ty,
                                    ownership,
                                    mutable: false,
                                },
                            );
                            resolved.push(ResolvedRecordMatchPatternField {
                                field: field_id,
                                pattern: ResolvedRecordMatchFieldPattern::Binding(binding),
                            });
                            frames.push(Frame::Fields {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index: index + 1,
                                resolved,
                                path,
                            });
                        }
                        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {
                            resolved.push(ResolvedRecordMatchPatternField {
                                field: field_id,
                                pattern: ResolvedRecordMatchFieldPattern::Wildcard,
                            });
                            frames.push(Frame::Fields {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index: index + 1,
                                resolved,
                                path,
                            });
                        }
                        crate::ast::RecordMatchFieldPattern::Record {
                            type_name,
                            fields: nested,
                            span,
                            ..
                        } => {
                            frames.push(Frame::AfterNested {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index,
                                resolved,
                                path: path.clone(),
                                field: field_id,
                            });
                            frames.push(Frame::Enter {
                                expected: field_ty,
                                type_name,
                                fields: nested,
                                path: format!("{field_path}.record"),
                                span: *span,
                            });
                        }
                    }
                }
                Frame::AfterNested {
                    expected,
                    record,
                    arguments,
                    templates,
                    fields,
                    index,
                    mut resolved,
                    path,
                    field,
                } => {
                    let ResolvedMatchPattern::Record {
                        record: nested_record,
                        instance,
                        fields: nested_fields,
                    } = results.pop().expect("nested record result retained")
                    else {
                        unreachable!("nested resolver returns a record pattern")
                    };
                    resolved.push(ResolvedRecordMatchPatternField {
                        field,
                        pattern: ResolvedRecordMatchFieldPattern::Record {
                            record: nested_record,
                            instance,
                            fields: nested_fields,
                        },
                    });
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        arguments,
                        templates,
                        fields,
                        index: index + 1,
                        resolved,
                        path,
                    });
                }
            }
        }
        Ok(results.pop().expect("root record pattern result retained"))
    }

    fn resolve_expr(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        self.resolve_expr_iterative(function, expr, bindings, path)
    }

    /// Resolve one assignment target against the enclosing scope. The target
    /// must name an existing `let mut` local; parameters, contracts bindings,
    /// and immutable locals are rejected before the assigned value resolves.
    /// Resolve one assignment target against the enclosing scope. The target
    /// must name an existing `let mut` local; parameters, contracts bindings,
    /// and immutable locals are rejected before the assigned value resolves.
    /// Simple assignments report `SPX-U101`; Field Mutation v1 targets report
    /// `SPX-U107`.
    fn resolve_assign_target(
        &self,
        name: &str,
        name_span: Span,
        bindings: &BTreeMap<String, Binding>,
        immutable_code: &'static str,
    ) -> Result<ResolvedBinding, Diagnostic> {
        let binding = bindings.get(name).ok_or_else(|| {
            self.error("SPX-H002", format!("unresolved value `{name}`"), name_span)
        })?;
        if !binding.mutable {
            return Err(self.error(
                immutable_code,
                format!("cannot assign to immutable binding `{name}`; declare it with `let mut`"),
                name_span,
            ));
        }
        Ok(ResolvedBinding {
            id: binding.id.clone(),
            name: name.to_owned(),
            ownership: binding.ownership,
            ty: binding.ty.clone(),
            span: name_span,
        })
    }

    /// Field Mutation v1: resolve the one direct `<binding>.<field>` level.
    /// The base must be a record/class-typed mutable local and the field must
    /// be a checked Copy scalar; everything else fails closed before the
    /// assigned value resolves.
    fn resolve_assign_field_target(
        &self,
        binding: &ResolvedBinding,
        field: &crate::ast::FieldTarget,
    ) -> Result<(DeclarationId, ResolvedType), Diagnostic> {
        let ResolvedType::Nominal {
            declaration: owner,
            arguments,
        } = &binding.ty
        else {
            return Err(self.error(
                "SPX-U112",
                format!(
                    "cannot mutate a field of non-record value `{}`",
                    binding.ty.identity_key()
                ),
                field.span,
            ));
        };
        if self.declarations.declaration(owner).is_none_or(|item| {
            !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
        }) {
            return Err(self.error(
                "SPX-U112",
                format!(
                    "cannot mutate a field of non-record value `{}`",
                    binding.ty.identity_key()
                ),
                field.span,
            ));
        }
        let field_id = self
            .declarations
            .field_id(owner, &field.name)
            .cloned()
            .ok_or_else(|| {
                self.error(
                    "SPX-U108",
                    format!("record `{owner}` has no field `{}`", field.name),
                    field.span,
                )
            })?;
        let declared = self
            .declarations
            .record_fields(owner)
            .and_then(|fields| fields.iter().find(|item| item.id == field_id))
            .map(|item| item.ty.clone())
            .ok_or_else(|| {
                self.error(
                    "SPX-H001",
                    format!("field `{field_id}` has no resolved type"),
                    field.span,
                )
            })?;
        let field_ty = substitute_type(&declared, owner, arguments)?;
        if !is_scalar_resolved_type(&field_ty) {
            return Err(self.error(
                "SPX-U109",
                "field mutation v1 supports only direct scalar Copy record fields",
                field.span,
            ));
        }
        Ok((field_id, field_ty))
    }

    /// Bounded While-Loops v1 plus Indexed Byte Loop v2 admission profile: a
    /// loop condition or body may contain Copy-scalar operations — scalar
    /// literals, names, checked
    /// scalar arithmetic and comparisons, nested `if`s over scalars, blocks
    /// with scalar statements, scalar `let`/assignment statements, nested
    /// while loops, monomorphic calls to scalar-value functions, exact
    /// read-only `byte_len`/`byte_get`, and one guard-free direct
    /// `byte_get`/`Option<u8>` match. Every other construct (records, variants,
    /// general matches, `?`, projections, method
    /// calls, strings, unsafe boundaries, generic calls, non-scalar calls)
    /// is rejected fail-closed so loop cleanup stays edge-free.
    fn reject_while_disallowed(&self, expression: &Expr) -> Result<(), Diagnostic> {
        enum Item<'a> {
            Expression(&'a Expr),
            Statement(&'a Statement),
        }

        let mut pending = vec![Item::Expression(expression)];
        while let Some(item) = pending.pop() {
            let expression = match item {
                Item::Statement(statement) => match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => value,
                    Statement::Unsafe { span, .. } => {
                        return Err(self.error(
                            "SPX-T252",
                            "unsafe boundary statements are not yet admitted in while bodies",
                            *span,
                        ));
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        pending.push(Item::Expression(body));
                        pending.push(Item::Expression(condition));
                        continue;
                    }
                },
                Item::Expression(expression) => expression,
            };

            match &expression.kind {
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::Var(_) => {}
                ExprKind::String(_) => {
                    return Err(self.error(
                        "SPX-T252",
                        "string literals are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "fixed-array literals are not admitted in bounded while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Unary { value, .. } => pending.push(Item::Expression(value)),
                ExprKind::Binary { left, right, .. } => {
                    pending.push(Item::Expression(right));
                    pending.push(Item::Expression(left));
                }
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(Item::Expression(else_branch));
                    pending.push(Item::Expression(then_branch));
                    pending.push(Item::Expression(condition));
                }
                ExprKind::Block { statements, tail } => {
                    pending.push(Item::Expression(tail));
                    pending.extend(statements.iter().rev().map(Item::Statement));
                }
                ExprKind::Call {
                    type_arguments,
                    args,
                    name,
                    ..
                } => {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-T252",
                            "generic calls are not yet admitted in while bodies",
                            expression.span,
                        ));
                    }
                    if let Some(operation) = crate::byte_ops::by_name(name) {
                        if !matches!(
                            operation,
                            crate::byte_ops::ByteOp::Len
                                | crate::byte_ops::ByteOp::Get
                                | crate::byte_ops::ByteOp::Range
                        ) || args.len() != operation.arity()
                        {
                            return Err(self.error(
                                "SPX-T252",
                                format!(
                                    "byte operation `{name}` is not admitted in while bodies; only exact byte_len and byte_get reads qualify"
                                ),
                                expression.span,
                            ));
                        }
                    }
                    // Only calls that resolve to a monomorphic function with
                    // by-value scalar parameters and a scalar result keep the
                    // loop cleanup-edge-free; everything else is rejected
                    // before any argument in the same order as the recursive
                    // admission scan.
                    let declared = self
                        .program
                        .functions
                        .iter()
                        .find(|function| function.name == *name);
                    if let Some(declared) = declared {
                        let scalar_signature = declared.effects.is_empty()
                            && is_scalar_source_type(&declared.return_type)
                            && declared.params.iter().all(|param| {
                                (param.mode == ParamMode::Value && is_scalar_source_type(&param.ty))
                                    || (param.mode == ParamMode::Borrow
                                        && param.ty == Type::SliceU8)
                            });
                        if !scalar_signature {
                            return Err(self.error(
                                "SPX-T252",
                                format!(
                                    "call `{name}` is not admitted in while bodies; only scalar functions qualify"
                                ),
                                expression.span,
                            ));
                        }
                    }
                    pending.extend(args.iter().rev().map(Item::Expression));
                }
                ExprKind::MethodCall { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::SuperMethod { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "super method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Project { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record field projection is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ConstructRecord { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::ConstructVariant { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "variant construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::UpdateRecord { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "record updates are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Match {
                    scrutinee, arms, ..
                } if crate::byte_ops::is_indexed_byte_option_match_source(expression) => {
                    pending.extend(arms.iter().rev().map(|arm| Item::Expression(&arm.value)));
                    pending.push(Item::Expression(scrutinee));
                }
                ExprKind::Match { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "match expressions are not yet admitted in while bodies",
                        expression.span,
                    ));
                }
                ExprKind::Try { .. } => {
                    return Err(self.error(
                        "SPX-T252",
                        "postfix `?` propagation is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Refutable Match v1 admission over a Copy-scalar scrutinee: literal
    /// patterns must compare against exactly the scrutinee type
    /// (`SPX-T255`), or-patterns stay flat and same-typed (`SPX-M105`),
    /// aggregate patterns never mix with scalar scrutinees (`SPX-H001`), and
    /// a refutable match requires one trailing irrefutable guard-free
    /// catch-all arm (`SPX-T257`).
    fn validate_refutable_match_admission(
        &self,
        scrutinee: &ResolvedType,
        arms: &[crate::ast::MatchArm],
    ) -> Result<(), Diagnostic> {
        for arm in arms {
            match &arm.pattern {
                crate::ast::MatchPattern::Wildcard { .. }
                | crate::ast::MatchPattern::Binding { .. } => {}
                crate::ast::MatchPattern::Literal { value, span } => {
                    if PatternValue::from_ast(*value).ty() != *scrutinee {
                        return Err(self.error(
                            "SPX-T255",
                            format!(
                                "literal pattern of type `{}` cannot match a `{}` scrutinee; \
                                 pattern literals compare against exactly their own type",
                                value.type_text(),
                                scrutinee.identity_key()
                            ),
                            *span,
                        ));
                    }
                }
                crate::ast::MatchPattern::Or { alternatives, span } => {
                    let mut alternative_type: Option<&'static str> = None;
                    for alternative in alternatives {
                        let crate::ast::MatchPattern::Literal { value, span } = alternative else {
                            return Err(self.error(
                                "SPX-M105",
                                "or-patterns admit only literal alternatives in v1",
                                alternative.span(),
                            ));
                        };
                        if PatternValue::from_ast(*value).ty() != *scrutinee {
                            return Err(self.error(
                                "SPX-T255",
                                format!(
                                    "literal pattern of type `{}` cannot match a `{}` scrutinee; \
                                     pattern literals compare against exactly their own type",
                                    value.type_text(),
                                    scrutinee.identity_key()
                                ),
                                *span,
                            ));
                        }
                        let type_text = value.type_text();
                        match alternative_type {
                            None => alternative_type = Some(type_text),
                            Some(seen) if seen == type_text => {}
                            Some(seen) => {
                                return Err(self.error(
                                    "SPX-M105",
                                    format!(
                                        "or-pattern mixes `{seen}` and `{type_text}` literal \
                                         alternatives; all alternatives must share one type"
                                    ),
                                    *span,
                                ));
                            }
                        }
                    }
                    if alternatives.is_empty() {
                        return Err(self.error(
                            "SPX-M105",
                            "or-pattern needs at least one literal alternative",
                            *span,
                        ));
                    }
                }
                crate::ast::MatchPattern::Variant { span, .. }
                | crate::ast::MatchPattern::Record { span, .. } => {
                    return Err(self.error(
                        "SPX-H001",
                        "aggregate pattern has a Copy-scalar scrutinee",
                        *span,
                    ));
                }
            }
        }
        let last = arms.last().expect("match always has arm syntax");
        let catch_all = matches!(
            &last.pattern,
            crate::ast::MatchPattern::Wildcard { .. } | crate::ast::MatchPattern::Binding { .. }
        );
        if !catch_all || last.guard.is_some() {
            return Err(self.error(
                "SPX-T257",
                "refutable match requires a trailing irrefutable catch-all arm \
                 (`_` or a binding) without a guard",
                last.span,
            ));
        }
        Ok(())
    }

    fn resolve_expr_iterative(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        enum Frame<'expr> {
            Enter {
                expr: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
                path: String,
            },
            FinishNativeCall {
                span: Span,
                path: String,
                import: DeclarationId,
                argument_count: usize,
            },
            FinishCall {
                span: Span,
                path: String,
                callee: DeclarationId,
                type_arguments: Vec<ResolvedType>,
                instance: Option<FunctionInstanceId>,
                return_source_type: Type,
                target_span: Span,
                argument_count: usize,
            },
            FinishStringOp {
                span: Span,
                path: String,
                op: crate::string_ops::StringOp,
                argument_count: usize,
            },
            FinishStrOp {
                span: Span,
                path: String,
                op: crate::str_ops::StrOp,
                argument_count: usize,
            },
            FinishByteOp {
                span: Span,
                path: String,
                op: crate::byte_ops::ByteOp,
                argument_count: usize,
            },
            FinishHostIoOp {
                span: Span,
                path: String,
                op: crate::host_io_ops::HostIoOp,
                argument_count: usize,
            },
            FinishHostCommandOp {
                span: Span,
                path: String,
                op: ResolvedHostCommandOperation,
                argument_count: usize,
            },
            ChildNext {
                children: &'expr [Expr],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                path: String,
                segment: &'static str,
            },
            MethodArgNext {
                args: &'expr [Expr],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                path: String,
            },
            FinishUnary {
                span: Span,
                path: String,
                op: UnaryOp,
            },
            FinishBinary {
                span: Span,
                path: String,
                op: BinaryOp,
            },
            AfterBinaryLeft {
                span: Span,
                path: String,
                op: BinaryOp,
                right: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            BlockNext {
                span: Span,
                path: String,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
            },
            BlockAfterLet {
                span: Span,
                path: String,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
            },
            BlockAfterAssign {
                span: Span,
                path: String,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
                target: ResolvedBinding,
                /// Field Mutation v1: the resolved direct field and its
                /// substituted type when the target is `<binding>.<field>`.
                target_field: Option<(DeclarationId, ResolvedType)>,
            },
            BlockAfterUnsafe {
                span: Span,
                path: String,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
            },
            BlockWhileCondition {
                span: Span,
                path: String,
                condition: &'expr Expr,
                body: &'expr Expr,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
            },
            BlockWhileBody {
                span: Span,
                path: String,
                statements: &'expr [Statement],
                tail: &'expr Expr,
                index: usize,
                scope: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedStatement>,
                condition: Box<ResolvedExpr>,
                condition_span: Span,
            },
            FinishBlock {
                span: Span,
                path: String,
                statements: Vec<ResolvedStatement>,
            },
            FinishIf {
                span: Span,
                path: String,
            },
            AfterIfCondition {
                span: Span,
                path: String,
                then_branch: &'expr Expr,
                else_branch: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            AfterIfThen {
                span: Span,
                path: String,
                else_branch: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            RecordNext {
                span: Span,
                path: String,
                type_name: &'expr str,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
            },
            RecordAfterField {
                span: Span,
                path: String,
                type_name: &'expr str,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
                field: DeclarationId,
            },
            VariantNext {
                span: Span,
                path: String,
                type_name: &'expr str,
                case_name: &'expr str,
                variant: DeclarationId,
                case: DeclarationId,
                type_arguments: &'expr [Type],
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
            },
            VariantAfterField {
                span: Span,
                path: String,
                type_name: &'expr str,
                case_name: &'expr str,
                variant: DeclarationId,
                case: DeclarationId,
                type_arguments: &'expr [Type],
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
                field: DeclarationId,
            },
            AfterMatchScrutinee {
                span: Span,
                path: String,
                mode: ResolvedMatchMode,
                arms: &'expr [crate::ast::MatchArm],
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            MatchNext {
                span: Span,
                path: String,
                mode: ResolvedMatchMode,
                arms: &'expr [crate::ast::MatchArm],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                scrutinee: ResolvedExpr,
                matched_type: DeclarationId,
                instance_arguments: Vec<ResolvedType>,
                matched_kind: DeclarationKind,
                resolved: Vec<ResolvedMatchArm>,
            },
            MatchAfterArm {
                span: Span,
                path: String,
                mode: ResolvedMatchMode,
                arms: &'expr [crate::ast::MatchArm],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                scrutinee: ResolvedExpr,
                matched_type: DeclarationId,
                instance_arguments: Vec<ResolvedType>,
                matched_kind: DeclarationKind,
                resolved: Vec<ResolvedMatchArm>,
                pattern: ResolvedMatchPattern,
            },
            /// Refutable Match v1 decision chain over a Copy-scalar
            /// scrutinee. Arms resolve in order under the enclosing bindings;
            /// binding arms extend them for their own arm only.
            ScalarMatchNext {
                span: Span,
                path: String,
                mode: ResolvedMatchMode,
                arms: &'expr [crate::ast::MatchArm],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                scrutinee: ResolvedExpr,
                resolved: Vec<ResolvedMatchArm>,
            },
            ScalarMatchAfterArm {
                span: Span,
                path: String,
                mode: ResolvedMatchMode,
                arms: &'expr [crate::ast::MatchArm],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                scrutinee: ResolvedExpr,
                resolved: Vec<ResolvedMatchArm>,
                pattern: ResolvedMatchPattern,
            },
            FinishTry {
                span: Span,
                path: String,
            },
            AfterUpdateBase {
                span: Span,
                path: String,
                fields: &'expr [crate::ast::FieldInitializer],
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            UpdateNext {
                span: Span,
                path: String,
                base: ResolvedExpr,
                record: DeclarationId,
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
            },
            UpdateAfterField {
                span: Span,
                path: String,
                base: ResolvedExpr,
                record: DeclarationId,
                fields: &'expr [crate::ast::FieldInitializer],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                resolved: Vec<ResolvedFieldInitializer>,
                field: DeclarationId,
            },
            FinishProject {
                span: Span,
                path: String,
                field: &'expr str,
            },
            FinishMethodCall {
                span: Span,
                path: String,
                method: &'expr str,
                receiver: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
                type_arguments: Vec<ResolvedType>,
                args_len: usize,
            },
            FinishSuperMethod {
                span: Span,
                method_span: Span,
                path: String,
                method: &'expr str,
                holder: DeclarationId,
                callee: DeclarationId,
                args_len: usize,
            },
            StartUpcast {
                source: &'expr Expr,
                bindings: Rc<BTreeMap<String, Binding>>,
                slot_path: String,
                holder: DeclarationId,
                span: Span,
                resume: Box<Frame<'expr>>,
            },
            FinishUpcast {
                slot_path: String,
                holder: DeclarationId,
                span: Span,
            },
        }

        fn take_results(results: &mut Vec<ResolvedExpr>, count: usize) -> Vec<ResolvedExpr> {
            let start = results
                .len()
                .checked_sub(count)
                .expect("expression continuation retains every child result");
            results.split_off(start)
        }

        #[cfg(test)]
        fn frame_owned_capacity(
            frame: &Frame<'_>,
            seen_scopes: &mut std::collections::HashSet<*const BTreeMap<String, Binding>>,
        ) -> usize {
            let path = match frame {
                Frame::Enter { path, .. }
                | Frame::FinishNativeCall { path, .. }
                | Frame::FinishCall { path, .. }
                | Frame::FinishStringOp { path, .. }
                | Frame::FinishStrOp { path, .. }
                | Frame::FinishByteOp { path, .. }
                | Frame::FinishHostIoOp { path, .. }
                | Frame::FinishHostCommandOp { path, .. }
                | Frame::ChildNext { path, .. }
                | Frame::MethodArgNext { path, .. }
                | Frame::FinishUnary { path, .. }
                | Frame::FinishBinary { path, .. }
                | Frame::AfterBinaryLeft { path, .. }
                | Frame::BlockNext { path, .. }
                | Frame::BlockAfterLet { path, .. }
                | Frame::BlockAfterAssign { path, .. }
                | Frame::BlockAfterUnsafe { path, .. }
                | Frame::BlockWhileCondition { path, .. }
                | Frame::BlockWhileBody { path, .. }
                | Frame::FinishBlock { path, .. }
                | Frame::FinishIf { path, .. }
                | Frame::AfterIfCondition { path, .. }
                | Frame::AfterIfThen { path, .. }
                | Frame::RecordNext { path, .. }
                | Frame::RecordAfterField { path, .. }
                | Frame::VariantNext { path, .. }
                | Frame::VariantAfterField { path, .. }
                | Frame::AfterMatchScrutinee { path, .. }
                | Frame::MatchNext { path, .. }
                | Frame::MatchAfterArm { path, .. }
                | Frame::ScalarMatchNext { path, .. }
                | Frame::ScalarMatchAfterArm { path, .. }
                | Frame::FinishTry { path, .. }
                | Frame::AfterUpdateBase { path, .. }
                | Frame::UpdateNext { path, .. }
                | Frame::UpdateAfterField { path, .. }
                | Frame::FinishProject { path, .. }
                | Frame::FinishMethodCall { path, .. }
                | Frame::FinishSuperMethod { path, .. }
                | Frame::StartUpcast {
                    slot_path: path, ..
                }
                | Frame::FinishUpcast {
                    slot_path: path, ..
                } => path.capacity(),
            };
            // Continuations share immutable binding maps through `Rc`. Count
            // the owned map allocation once, not once per retaining frame.
            let mut unique_scope_capacity = |scope: &Rc<BTreeMap<String, Binding>>| {
                if seen_scopes.insert(Rc::as_ptr(scope)) {
                    resolver_scope_owned_capacity(scope)
                } else {
                    0
                }
            };
            let scope = match frame {
                Frame::Enter { bindings, .. }
                | Frame::ChildNext { bindings, .. }
                | Frame::MethodArgNext { bindings, .. }
                | Frame::StartUpcast { bindings, .. }
                | Frame::AfterBinaryLeft { bindings, .. }
                | Frame::AfterIfCondition { bindings, .. }
                | Frame::AfterIfThen { bindings, .. }
                | Frame::RecordNext { bindings, .. }
                | Frame::RecordAfterField { bindings, .. }
                | Frame::VariantNext { bindings, .. }
                | Frame::VariantAfterField { bindings, .. }
                | Frame::AfterMatchScrutinee { bindings, .. }
                | Frame::MatchNext { bindings, .. }
                | Frame::MatchAfterArm { bindings, .. }
                | Frame::ScalarMatchNext { bindings, .. }
                | Frame::ScalarMatchAfterArm { bindings, .. }
                | Frame::AfterUpdateBase { bindings, .. }
                | Frame::UpdateNext { bindings, .. }
                | Frame::UpdateAfterField { bindings, .. } => unique_scope_capacity(bindings),
                Frame::BlockNext { scope, .. }
                | Frame::BlockAfterLet { scope, .. }
                | Frame::BlockAfterAssign { scope, .. }
                | Frame::BlockAfterUnsafe { scope, .. }
                | Frame::BlockWhileCondition { scope, .. }
                | Frame::BlockWhileBody { scope, .. } => unique_scope_capacity(scope),
                _ => 0,
            };
            let retained = match frame {
                Frame::FinishMethodCall { type_arguments, .. } => {
                    type_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                        + type_arguments
                            .iter()
                            .map(resolved_type_owned_capacity)
                            .sum::<usize>()
                }
                Frame::FinishCall {
                    type_arguments,
                    return_source_type,
                    ..
                } => {
                    type_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                        + type_arguments
                            .iter()
                            .map(resolved_type_owned_capacity)
                            .sum::<usize>()
                        + match return_source_type {
                            Type::I64
                            | Type::I32
                            | Type::Char
                            | Type::U8
                            | Type::Usize
                            | Type::ArrayU8(_)
                            | Type::F32
                            | Type::F64
                            | Type::Bool => 0,
                            Type::String | Type::Bytes | Type::Str | Type::SliceU8 => 0,
                            Type::Named { name, arguments } => {
                                name.capacity() + arguments.capacity() * std::mem::size_of::<Type>()
                            }
                        }
                }
                Frame::BlockNext { resolved, .. }
                | Frame::BlockAfterLet { resolved, .. }
                | Frame::BlockAfterAssign { resolved, .. }
                | Frame::BlockAfterUnsafe { resolved, .. }
                | Frame::BlockWhileCondition { resolved, .. }
                | Frame::BlockWhileBody { resolved, .. }
                | Frame::FinishBlock {
                    statements: resolved,
                    ..
                } => {
                    resolved.capacity() * std::mem::size_of::<ResolvedStatement>()
                        + resolved
                            .iter()
                            .map(resolved_statement_owned_capacity)
                            .sum::<usize>()
                }
                Frame::RecordNext {
                    arguments,
                    resolved,
                    ..
                }
                | Frame::RecordAfterField {
                    arguments,
                    resolved,
                    ..
                } => {
                    arguments.capacity() * std::mem::size_of::<ResolvedType>()
                        + arguments
                            .iter()
                            .map(resolved_type_owned_capacity)
                            .sum::<usize>()
                        + resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                        + resolved
                            .iter()
                            .map(resolved_field_initializer_owned_capacity)
                            .sum::<usize>()
                }
                Frame::VariantNext { resolved, .. } | Frame::VariantAfterField { resolved, .. } => {
                    resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                        + resolved
                            .iter()
                            .map(resolved_field_initializer_owned_capacity)
                            .sum::<usize>()
                }
                Frame::MatchNext {
                    scrutinee,
                    instance_arguments,
                    resolved,
                    ..
                }
                | Frame::MatchAfterArm {
                    scrutinee,
                    instance_arguments,
                    resolved,
                    ..
                } => {
                    resolved_expr_owned_capacity(scrutinee)
                        + instance_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                        + instance_arguments
                            .iter()
                            .map(resolved_type_owned_capacity)
                            .sum::<usize>()
                        + resolved.capacity() * std::mem::size_of::<ResolvedMatchArm>()
                        + resolved
                            .iter()
                            .map(resolved_match_arm_owned_capacity)
                            .sum::<usize>()
                }
                Frame::ScalarMatchNext {
                    scrutinee,
                    resolved,
                    ..
                }
                | Frame::ScalarMatchAfterArm {
                    scrutinee,
                    resolved,
                    ..
                } => {
                    resolved_expr_owned_capacity(scrutinee)
                        + resolved.capacity() * std::mem::size_of::<ResolvedMatchArm>()
                        + resolved
                            .iter()
                            .map(resolved_match_arm_owned_capacity)
                            .sum::<usize>()
                }
                Frame::UpdateNext { base, resolved, .. }
                | Frame::UpdateAfterField { base, resolved, .. } => {
                    resolved_expr_owned_capacity(base)
                        + resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                        + resolved
                            .iter()
                            .map(resolved_field_initializer_owned_capacity)
                            .sum::<usize>()
                }
                _ => 0,
            };
            path.saturating_add(scope).saturating_add(retained)
        }

        // Refutable Match v1 grew `ResolvedMatchPattern` (Literal/Or/
        // Binding), which grows this frame's arm-pattern payload.
        const { assert!(std::mem::size_of::<Frame<'static>>() == 592) };
        let mut frames = vec![Frame::Enter {
            expr,
            bindings: Rc::new(bindings.clone()),
            path: path.to_owned(),
        }];
        let mut results = Vec::new();

        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            {
                let mut seen_scopes = std::collections::HashSet::new();
                let frame_owned = frames.iter().fold(0_usize, |total, candidate| {
                    total.saturating_add(frame_owned_capacity(candidate, &mut seen_scopes))
                });
                let current_owned = frame_owned_capacity(&frame, &mut seen_scopes);
                note_iterative_phase_capacity(
                    0,
                    frames.capacity() * std::mem::size_of::<Frame<'_>>()
                        + results.capacity() * std::mem::size_of::<ResolvedExpr>()
                        + results
                            .iter()
                            .map(resolved_expr_owned_capacity)
                            .sum::<usize>()
                        + frame_owned
                        + current_owned,
                );
            }
            match frame {
                Frame::Enter {
                    expr,
                    bindings,
                    path,
                } => match &expr.kind {
                    ExprKind::Int(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::I64,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Int(*value),
                        span: expr.span,
                    }),
                    ExprKind::String(value) => {
                        let ty = ResolvedType::String;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ownership: self.expression_ownership(
                                &ty,
                                OwnershipMode::Own,
                                expr.span,
                            )?,
                            ty,
                            kind: ResolvedExprKind::String(value.clone()),
                            span: expr.span,
                        });
                    }
                    ExprKind::Int32(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::I32,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Int32(*value),
                        span: expr.span,
                    }),
                    ExprKind::Char(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Char,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Char(*value),
                        span: expr.span,
                    }),
                    ExprKind::Uint8(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::U8,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Uint8(*value),
                        span: expr.span,
                    }),
                    ExprKind::Usize(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Usize,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Usize(*value),
                        span: expr.span,
                    }),
                    ExprKind::ArrayU8(values) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::ArrayU8(values.len() as u32),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::ArrayU8(values.clone()),
                        span: expr.span,
                    }),
                    ExprKind::RepeatArrayU8 { value, count } => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::ArrayU8(*count),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::RepeatArrayU8 {
                            value: *value,
                            count: *count,
                        },
                        span: expr.span,
                    }),
                    ExprKind::Float32(bits) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::F32,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Float32(*bits),
                        span: expr.span,
                    }),
                    ExprKind::Float64(bits) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::F64,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Float64(*bits),
                        span: expr.span,
                    }),
                    ExprKind::Bool(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Bool,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Bool(*value),
                        span: expr.span,
                    }),
                    ExprKind::Var(name) => {
                        let binding = bindings.get(name).ok_or_else(|| {
                            self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                        })?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            kind: ResolvedExprKind::Place(Place {
                                root: binding.id.clone(),
                                projections: Vec::new(),
                            }),
                            span: expr.span,
                        });
                    }
                    ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } => {
                        if let Some(import_id) =
                            self.declarations.native_rust_import_id(name).cloned()
                        {
                            let import = self
                                .program
                                .interfaces
                                .iter()
                                .flat_map(|interface| &interface.imports)
                                .find(|import| import.stable_id == import_id.as_str())
                                .expect("native Rust import index is built from source imports");
                            if !type_arguments.is_empty() || args.len() != import.params.len() {
                                return Err(self.error(
                                    "SPX-B107",
                                    "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishNativeCall {
                                span: expr.span,
                                path: path.clone(),
                                import: import_id,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "native-rust-arg",
                            });
                        } else if let Some(op) = crate::string_ops::by_name(name) {
                            // Compiler-owned string operations resolve to
                            // ordinary monomorphic calls carrying their
                            // reserved `core.string.*` identity; backends
                            // lower that identity intrinsically.
                            if !type_arguments.is_empty() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!("string operation `{name}` has type arguments"),
                                    expr.span,
                                ));
                            }
                            if args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!(
                                        "string operation `{name}` expects {} arguments, received {}",
                                        op.arity(),
                                        args.len()
                                    ),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishStringOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::str_ops::by_name(name) {
                            if !type_arguments.is_empty() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!(
                                        "borrowed string operation `{name}` has type arguments"
                                    ),
                                    expr.span,
                                ));
                            }
                            if args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!(
                                        "borrowed string operation `{name}` expects {} arguments, received {}",
                                        op.arity(),
                                        args.len()
                                    ),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishStrOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::byte_ops::by_name(name) {
                            if !type_arguments.is_empty() || args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!("invalid byte operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishByteOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::host_io_ops::by_name(name) {
                            if !type_arguments.is_empty() || args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-T269",
                                    format!("invalid host I/O operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishHostIoOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::command_io_ops::by_name(name) {
                            if !type_arguments.is_empty()
                                || args.len() != crate::command_io_ops::arity(op)
                            {
                                return Err(self.error(
                                    "SPX-T270",
                                    format!("invalid command I/O operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishHostCommandOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else {
                            let template = self
                                .declarations
                                .function_id(name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H003",
                                        format!("unresolved function `{name}`"),
                                        expr.span,
                                    )
                                })?;
                            let target = self
                                .program
                                .functions
                                .iter()
                                .find(|function| function.stable_id == template.as_str())
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H003",
                                        format!(
                                            "function identity `{template}` has no declaration"
                                        ),
                                        expr.span,
                                    )
                                })?;
                            let resolved_arguments = type_arguments
                                .iter()
                                .map(|argument| self.resolve_type(argument, expr.span))
                                .collect::<Result<Vec<_>, _>>()?;
                            let (instance, return_source_type) = if target
                                .type_parameters
                                .is_empty()
                            {
                                if !resolved_arguments.is_empty() {
                                    return Err(self.error(
                                        "SPX-H006",
                                        format!(
                                            "monomorphic function `{template}` has type arguments"
                                        ),
                                        expr.span,
                                    ));
                                }
                                (None, target.return_type.clone())
                            } else {
                                if resolved_arguments.len() != target.type_parameters.len()
                                    || resolved_arguments.iter().any(|argument| {
                                        !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                    })
                                {
                                    return Err(self.error(
                                            "SPX-H006",
                                            format!(
                                                "generic function `{template}` has invalid type arguments"
                                            ),
                                            expr.span,
                                        ));
                                }
                                let instance =
                                    FunctionInstanceId::derive(&template, &resolved_arguments);
                                let return_type = substitute_source_function_type(
                                        target,
                                        type_arguments,
                                        &target.return_type,
                                    )
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            format!(
                                                "generic function `{template}` return substitution failed"
                                            ),
                                            expr.span,
                                        )
                                    })?;
                                (Some(instance), return_type)
                            };
                            frames.push(Frame::FinishCall {
                                span: expr.span,
                                path: path.clone(),
                                callee: template,
                                type_arguments: resolved_arguments,
                                instance,
                                return_source_type,
                                target_span: target.span,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        }
                    }
                    ExprKind::Unary { op, value } => {
                        frames.push(Frame::FinishUnary {
                            span: expr.span,
                            path: path.clone(),
                            op: *op,
                        });
                        frames.push(Frame::Enter {
                            expr: value,
                            bindings,
                            path: format!("{path}.value"),
                        });
                    }
                    ExprKind::Binary { op, left, right } => {
                        frames.push(Frame::AfterBinaryLeft {
                            span: expr.span,
                            path: path.clone(),
                            op: *op,
                            right,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: left,
                            bindings,
                            path: format!("{path}.left"),
                        });
                    }
                    ExprKind::Block { statements, tail } => {
                        frames.push(Frame::BlockNext {
                            span: expr.span,
                            path,
                            statements,
                            tail,
                            index: 0,
                            scope: bindings,
                            resolved: Vec::with_capacity(statements.len()),
                        });
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::AfterIfCondition {
                            span: expr.span,
                            path: path.clone(),
                            then_branch,
                            else_branch,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: condition,
                            bindings,
                            path: format!("{path}.condition"),
                        });
                    }
                    ExprKind::ConstructRecord {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    } => {
                        let record =
                            self.declarations
                                .type_id(type_name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H001",
                                        format!("unresolved record `{type_name}`"),
                                        expr.span,
                                    )
                                })?;
                        if self.declarations.declaration(&record).is_none_or(|item| {
                            !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
                        }) {
                            return Err(self.error(
                                "SPX-H001",
                                format!(
                                    "constructor target `{type_name}` is not a record or class"
                                ),
                                expr.span,
                            ));
                        }
                        let arguments = type_arguments
                            .iter()
                            .map(|argument| self.resolve_type(argument, expr.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        let parameters =
                            self.declarations.type_parameters(&record).ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("record `{record}` has no parameter metadata"),
                                    expr.span,
                                )
                            })?;
                        if arguments.len() != parameters.len()
                            || arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            })
                        {
                            return Err(self.error(
                                "SPX-H006",
                                format!("record `{record}` has invalid concrete arguments"),
                                expr.span,
                            ));
                        }
                        frames.push(Frame::RecordNext {
                            span: expr.span,
                            path,
                            type_name,
                            record,
                            arguments,
                            fields,
                            index: 0,
                            bindings,
                            resolved: Vec::with_capacity(fields.len()),
                        });
                    }
                    ExprKind::ConstructVariant {
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        ..
                    } => {
                        let variant =
                            self.declarations
                                .type_id(type_name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H001",
                                        format!("unresolved variant `{type_name}`"),
                                        expr.span,
                                    )
                                })?;
                        if self
                            .declarations
                            .declaration(&variant)
                            .is_none_or(|item| item.kind != DeclarationKind::Variant)
                        {
                            return Err(self.error(
                                "SPX-H001",
                                format!("constructor target `{type_name}` is not a variant"),
                                expr.span,
                            ));
                        }
                        let case = self
                            .declarations
                            .case_id(&variant, case_name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H001",
                                    format!("unresolved case `{type_name}::{case_name}`"),
                                    expr.span,
                                )
                            })?;
                        frames.push(Frame::VariantNext {
                            span: expr.span,
                            path,
                            type_name,
                            case_name,
                            variant,
                            case,
                            type_arguments,
                            fields,
                            index: 0,
                            bindings,
                            resolved: Vec::with_capacity(fields.len()),
                        });
                    }
                    ExprKind::Match {
                        mode,
                        scrutinee,
                        arms,
                    } => {
                        frames.push(Frame::AfterMatchScrutinee {
                            span: expr.span,
                            path: path.clone(),
                            mode: (*mode).into(),
                            arms,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: scrutinee,
                            bindings,
                            path: format!("{path}.scrutinee"),
                        });
                    }
                    ExprKind::Try { operand } => {
                        frames.push(Frame::FinishTry {
                            span: expr.span,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: operand,
                            bindings,
                            path: format!("{path}.operand"),
                        });
                    }
                    ExprKind::UpdateRecord { base, fields } => {
                        frames.push(Frame::AfterUpdateBase {
                            span: expr.span,
                            path: path.clone(),
                            fields,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: base,
                            bindings,
                            path: format!("{path}.base"),
                        });
                    }
                    ExprKind::Project { base, field, .. } => {
                        frames.push(Frame::FinishProject {
                            span: expr.span,
                            path: path.clone(),
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: base,
                            bindings,
                            path: format!("{path}.base"),
                        });
                    }
                    ExprKind::MethodCall {
                        receiver,
                        method,
                        type_arguments,
                        args,
                        ..
                    } => {
                        if !type_arguments.is_empty() {
                            return Err(self.error(
                                "SPX-P106",
                                "method call generic arguments are not supported in this slice",
                                expr.span,
                            ));
                        }
                        let resolved_args = type_arguments
                            .iter()
                            .map(|a| self.resolve_type(a, expr.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        frames.push(Frame::FinishMethodCall {
                            span: expr.span,
                            path: path.clone(),
                            method,
                            receiver,
                            bindings: bindings.clone(),
                            type_arguments: resolved_args,
                            args_len: args.len(),
                        });
                        if !args.is_empty() {
                            frames.push(Frame::MethodArgNext {
                                args,
                                index: 0,
                                bindings: bindings.clone(),
                                path: path.clone(),
                            });
                        }
                        // The receiver lowers to the call's first argument, so
                        // it carries the canonical `.arg.0` identity slot.
                        frames.push(Frame::Enter {
                            expr: receiver,
                            bindings,
                            path: format!("{path}.arg.0"),
                        });
                    }
                    ExprKind::SuperMethod {
                        method,
                        method_span,
                        args,
                    } => {
                        // `super` resolves against the enclosing class-method's
                        // owner; the enclosing method's own receiver becomes
                        // the callee's `self` argument.
                        let FunctionExecutionId::Monomorphic(template) = function else {
                            return Err(self.error(
                                "SPX-T231",
                                "`super` is only allowed inside a class-method override",
                                *method_span,
                            ));
                        };
                        let owner = self
                            .declarations
                            .declaration(template)
                            .and_then(|item| item.owner.clone())
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-T231",
                                    "`super` is only allowed inside a class-method override",
                                    *method_span,
                                )
                            })?;
                        let parent = self
                            .declarations
                            .class_parent(&owner)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-T231",
                                    format!(
                                        "`super.{method}` requires a parent; the enclosing class has none"
                                    ),
                                    *method_span,
                                )
                            })?;
                        let (holder, callee) = self
                            .resolve_method_in_chain(&parent, method, *method_span)
                            .map_err(|_| {
                                self.error(
                                    "SPX-T231",
                                    format!("unresolved super method `{method}`"),
                                    *method_span,
                                )
                            })?;
                        frames.push(Frame::FinishSuperMethod {
                            span: expr.span,
                            method_span: *method_span,
                            path: path.clone(),
                            method,
                            holder: holder.clone(),
                            callee,
                            args_len: args.len(),
                        });
                        if !args.is_empty() {
                            frames.push(Frame::MethodArgNext {
                                args,
                                index: 0,
                                bindings: bindings.clone(),
                                path: path.clone(),
                            });
                        }
                        // The inherited receiver is the enclosing method's
                        // own `self` parameter. It is created here as the
                        // upcast source; the finish frame wraps it under the
                        // canonical `.arg.0` argument identity.
                        let owner_ty = ResolvedType::Nominal {
                            declaration: owner.clone(),
                            arguments: Vec::new(),
                        };
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0.source")),
                            ty: owner_ty.clone(),
                            ownership: self.expression_ownership(
                                &owner_ty,
                                OwnershipMode::Own,
                                expr.span,
                            )?,
                            kind: ResolvedExprKind::Place(Place {
                                root: ValueId::parameter(function, 0),
                                projections: Vec::new(),
                            }),
                            span: expr.span,
                        });
                    }
                },
                Frame::FinishNativeCall {
                    span,
                    path,
                    import,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    let source_import = self
                        .program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|candidate| candidate.stable_id == import.as_str())
                        .expect("native Rust import identity remains indexed");
                    for (argument, parameter) in args.iter().zip(&source_import.params) {
                        if argument.ty != self.resolve_type(&parameter.ty, parameter.span)? {
                            return Err(self.error(
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                argument.span,
                            ));
                        }
                    }
                    let result = match source_import.result {
                        crate::ast::ImportResult::Unit => ResolvedImportResultKind::Unit,
                        crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                        crate::ast::ImportResult::Bool => ResolvedImportResultKind::Bool,
                    };
                    let ty = match result {
                        ResolvedImportResultKind::Unit => ResolvedType::Unit,
                        ResolvedImportResultKind::I64 => ResolvedType::I64,
                        ResolvedImportResultKind::Bool => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::NativeRustImportCall(
                            ResolvedNativeRustImportCall {
                                expression: ExpressionId::new(function, &path),
                                import,
                                args,
                                result,
                            },
                        ),
                        span,
                    });
                }
                Frame::FinishCall {
                    span,
                    path,
                    callee,
                    type_arguments,
                    instance,
                    return_source_type,
                    target_span,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    let ty = self.resolve_type(&return_source_type, target_span)?;
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, target_span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishStringOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishStrOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "borrowed string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishByteOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "byte operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                    if op == crate::byte_ops::ByteOp::Range {
                        let mut args = args.into_iter();
                        let source = args.next().expect("range has a source");
                        if !matches!(source.kind, ResolvedExprKind::Place(ref place) if place.projections.is_empty())
                        {
                            return Err(self.error(
                                "SPX-T266",
                                "byte_range requires an exact named Slice<u8> source",
                                source.span,
                            ));
                        }
                        let start = args.next().expect("range has a start");
                        let end = args.next().expect("range has an end");
                        debug_assert!(args.next().is_none());
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::ByteRange {
                                operation: DeclarationId::new(crate::byte_ops::RANGE_ID),
                                source: Box::new(source),
                                start: Box::new(start),
                                end: Box::new(end),
                            },
                            span,
                        });
                        continue;
                    }
                    if op.is_view() {
                        let ResolvedExprKind::Place(place) = &args[0].kind else {
                            return Err(self.error(
                                "SPX-T266",
                                format!(
                                    "byte view `{}` requires an exact named storage root",
                                    op.name()
                                ),
                                args[0].span,
                            ));
                        };
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::BorrowPlace {
                                operation: DeclarationId::new(op.id()),
                                place: place.clone(),
                            },
                            span,
                        });
                        continue;
                    }
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishHostIoOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T269",
                                format!(
                                    "host I/O operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: op.return_type(),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishHostCommandOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if !crate::command_io_ops::accepts_resolved(op, index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T270",
                                format!(
                                    "command I/O operation `{}` argument {index} has the wrong type",
                                    crate::command_io_ops::name(op)
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let expression = ExpressionId::new(function, &path);
                    results.push(ResolvedExpr {
                        id: expression.clone(),
                        ty: crate::command_io_ops::return_type(op),
                        ownership: crate::command_io_ops::result_ownership(op),
                        kind: ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                            expression,
                            operation: op,
                            args,
                        }),
                        span,
                    });
                }
                Frame::ChildNext {
                    children,
                    index,
                    bindings,
                    path,
                    segment,
                } => {
                    if index < children.len() {
                        frames.push(Frame::ChildNext {
                            children,
                            index: index + 1,
                            bindings: bindings.clone(),
                            path: path.clone(),
                            segment,
                        });
                        frames.push(Frame::Enter {
                            expr: &children[index],
                            bindings,
                            path: format!("{path}.{segment}.{index}"),
                        });
                    }
                }
                Frame::MethodArgNext {
                    args,
                    index,
                    bindings,
                    path,
                } => {
                    if index < args.len() {
                        frames.push(Frame::MethodArgNext {
                            args,
                            index: index + 1,
                            bindings: bindings.clone(),
                            path: path.clone(),
                        });
                        // Method arguments lower to call slots shifted by one
                        // so the receiver owns `.arg.0`.
                        frames.push(Frame::Enter {
                            expr: &args[index],
                            bindings,
                            path: format!("{path}.arg.{}", index + 1),
                        });
                    }
                }
                Frame::FinishUnary { span, path, op } => {
                    let value = results.pop().expect("unary child result retained");
                    // Negation keeps the numeric operand type; the validator
                    // and backends fail closed on any other shape.
                    let ty = match (&op, &value.ty) {
                        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
                        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
                        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
                        (UnaryOp::Neg, _) => ResolvedType::I64,
                        (UnaryOp::Not, _) => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Unary {
                            op,
                            value: Box::new(value),
                        },
                        span,
                    });
                }
                Frame::FinishBinary { span, path, op } => {
                    let mut children = take_results(&mut results, 2).into_iter();
                    let left = children.next().expect("binary left result retained");
                    let right = children.next().expect("binary right result retained");
                    // Arithmetic keeps the numeric operand type; the validator
                    // and backends reject mixed or float-remainder shapes.
                    let ty = match (&op, &left.ty) {
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::I32,
                        ) => ResolvedType::I32,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::F32,
                        ) => ResolvedType::F32,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::F64,
                        ) => ResolvedType::F64,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::U8,
                        ) => ResolvedType::U8,
                        (
                            BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem,
                            ResolvedType::Usize,
                        ) => ResolvedType::Usize,
                        (
                            BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem,
                            _,
                        ) => ResolvedType::I64,
                        (
                            BinaryOp::Eq
                            | BinaryOp::Ne
                            | BinaryOp::Lt
                            | BinaryOp::Le
                            | BinaryOp::Gt
                            | BinaryOp::Ge
                            | BinaryOp::And
                            | BinaryOp::Or,
                            _,
                        ) => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Binary {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        span,
                    });
                }
                Frame::AfterBinaryLeft {
                    span,
                    path,
                    op,
                    right,
                    bindings,
                } => {
                    frames.push(Frame::FinishBinary {
                        span,
                        path: path.clone(),
                        op,
                    });
                    frames.push(Frame::Enter {
                        expr: right,
                        bindings,
                        path: format!("{path}.right"),
                    });
                }
                Frame::BlockNext {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    resolved,
                } => {
                    if index == statements.len() {
                        frames.push(Frame::FinishBlock {
                            span,
                            path: path.clone(),
                            statements: resolved,
                        });
                        frames.push(Frame::Enter {
                            expr: tail,
                            bindings: scope,
                            path: format!("{path}.tail"),
                        });
                    } else {
                        match &statements[index] {
                            Statement::Let { value, .. } => {
                                frames.push(Frame::BlockAfterLet {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: value,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            Statement::Assign {
                                name,
                                name_span,
                                field,
                                value,
                                ..
                            } => {
                                let immutable_code = if field.is_some() {
                                    "SPX-U107"
                                } else {
                                    "SPX-U101"
                                };
                                let target = self.resolve_assign_target(
                                    name,
                                    *name_span,
                                    &scope,
                                    immutable_code,
                                )?;
                                let target_field = match field {
                                    Some(field) => {
                                        Some(self.resolve_assign_field_target(&target, field)?)
                                    }
                                    None => None,
                                };
                                frames.push(Frame::BlockAfterAssign {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                    target,
                                    target_field,
                                });
                                frames.push(Frame::Enter {
                                    expr: value,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            Statement::Unsafe { body, .. } => {
                                // The body is an ordinary safe block; it
                                // resolves with the enclosing scope and its
                                // result is admitted (or rejected) when the
                                // boundary statement is assembled.
                                frames.push(Frame::BlockAfterUnsafe {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: body,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.body"),
                                });
                            }
                            Statement::While {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: admit only the
                                // Copy-scalar profile before resolving, so a
                                // loop can never introduce cleanup structure.
                                self.reject_while_disallowed(condition)?;
                                self.reject_while_disallowed(body)?;
                                frames.push(Frame::BlockWhileCondition {
                                    span,
                                    path: path.clone(),
                                    condition,
                                    body,
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: condition,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.condition"),
                                });
                            }
                        }
                    }
                }
                Frame::BlockAfterLet {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    mut scope,
                    mut resolved,
                } => {
                    let value = results.pop().expect("let value result retained");
                    let Statement::Let {
                        name,
                        name_span,
                        mutable,
                        declared,
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = format!("{path}.s{index}");
                    // Class Inheritance v1: an explicit declared type accepts
                    // either the value's exact type or an ancestor class; a
                    // descendant value is consumed through a prefix upcast
                    // whose source re-resolves at the canonical `.source`
                    // identity below the binding slot.
                    if let Some(declared_ast) = declared {
                        let declared_ty = self.resolve_type(declared_ast, *name_span)?;
                        if value.ty != declared_ty {
                            let ResolvedType::Nominal {
                                declaration: child_id,
                                ..
                            } = &value.ty
                            else {
                                return Err(self.error(
                                    "SPX-T232",
                                    format!(
                                        "declared binding type `{}` does not accept value type `{}`",
                                        declared_ty.identity_key(),
                                        value.ty.identity_key()
                                    ),
                                    *name_span,
                                ));
                            };
                            let ResolvedType::Nominal {
                                declaration: parent_id,
                                ..
                            } = &declared_ty
                            else {
                                return Err(self.error(
                                    "SPX-T232",
                                    format!(
                                        "declared binding type `{}` does not accept value type `{}`",
                                        declared_ty.identity_key(),
                                        value.ty.identity_key()
                                    ),
                                    *name_span,
                                ));
                            };
                            self.check_upcast_admissible(child_id, parent_id, *name_span)?;
                            let slot_path = format!("{statement_path}.value");
                            frames.push(Frame::StartUpcast {
                                source: match &statements[index] {
                                    Statement::Let { value, .. } => value,
                                    _ => unreachable!("let frame resumes at a let statement"),
                                },
                                bindings: scope.clone(),
                                slot_path,
                                holder: parent_id.clone(),
                                span: value.span,
                                resume: Box::new(Frame::BlockAfterLet {
                                    span,
                                    path,
                                    statements,
                                    tail,
                                    index,
                                    scope,
                                    resolved,
                                }),
                            });
                            continue;
                        }
                    }
                    let binding = ResolvedBinding {
                        id: ValueId::local(function, &statement_path),
                        name: name.clone(),
                        ownership: value.ownership,
                        ty: value.ty.clone(),
                        span: *name_span,
                    };
                    Rc::make_mut(&mut scope).insert(
                        name.clone(),
                        Binding {
                            id: binding.id.clone(),
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            mutable: *mutable,
                        },
                    );
                    resolved.push(ResolvedStatement::Let {
                        binding,
                        mutable: *mutable,
                        value,
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockAfterAssign {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                    target,
                    target_field,
                } => {
                    // The assigned value is fully evaluated before the store;
                    // exact-type and scalar-Copy admission are checked here so
                    // failure statuses propagate exactly like initializers.
                    let value = results.pop().expect("assign value result retained");
                    match &target_field {
                        Some((_, field_ty)) => {
                            if value.ty != *field_ty {
                                return Err(self.error(
                                    "SPX-U110",
                                    format!(
                                        "assigned value type `{}` does not exactly match field type `{}`",
                                        value.ty.identity_key(),
                                        field_ty.identity_key()
                                    ),
                                    value.span,
                                ));
                            }
                        }
                        None => {
                            if value.ty != target.ty {
                                return Err(self.error(
                                    "SPX-U102",
                                    format!(
                                        "assigned value type `{}` does not exactly match binding type `{}`",
                                        value.ty.identity_key(),
                                        target.ty.identity_key()
                                    ),
                                    value.span,
                                ));
                            }
                            if value.ownership != OwnershipMode::Value
                                || !is_scalar_resolved_type(&value.ty)
                            {
                                return Err(self.error(
                                    "SPX-U105",
                                    "explicit mutation v1 supports only scalar Copy values",
                                    value.span,
                                ));
                            }
                        }
                    }
                    let Statement::Assign {
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    resolved.push(ResolvedStatement::Assign {
                        binding: target,
                        field: target_field.map(|(field_id, _)| field_id),
                        value,
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockAfterUnsafe {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                } => {
                    // The body block resolved like any ordinary nested block.
                    // Boundary admission mirrors the mutation checks: the
                    // discarded body result must be a scalar Copy value so no
                    // cleanup or ownership semantics are introduced.
                    let body = results.pop().expect("unsafe body result retained");
                    if body.ownership != OwnershipMode::Value || !is_scalar_resolved_type(&body.ty)
                    {
                        return Err(self.error(
                            "SPX-N104",
                            "unsafe boundary bodies must produce a scalar Copy value",
                            body.span,
                        ));
                    }
                    let Statement::Unsafe {
                        audit,
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("unsafe frame resumes at an unsafe statement")
                    };
                    resolved.push(ResolvedStatement::Unsafe {
                        audit: audit.clone(),
                        body: Box::new(body),
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockWhileCondition {
                    span,
                    path,
                    condition,
                    body,
                    statements,
                    tail,
                    index,
                    scope,
                    resolved,
                } => {
                    // The condition is re-evaluated before every iteration and
                    // must be exactly `bool`.
                    let evaluated = results.pop().expect("while condition result retained");
                    if evaluated.ty != ResolvedType::Bool {
                        return Err(self.error(
                            "SPX-T251",
                            "`while` condition must be bool",
                            condition.span,
                        ));
                    }
                    frames.push(Frame::BlockWhileBody {
                        span,
                        path: path.clone(),
                        statements,
                        tail,
                        index,
                        scope: scope.clone(),
                        resolved,
                        condition: Box::new(evaluated),
                        condition_span: condition.span,
                    });
                    frames.push(Frame::Enter {
                        expr: body,
                        bindings: scope.clone(),
                        path: format!("{path}.s{index}.body"),
                    });
                }
                Frame::BlockWhileBody {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                    condition,
                    condition_span,
                } => {
                    // The body block resolved like any ordinary nested block;
                    // its value is discarded by the statement.
                    let body = results.pop().expect("while body result retained");
                    let Statement::While {
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("while frame resumes at a while statement")
                    };
                    resolved.push(ResolvedStatement::While {
                        condition,
                        body: Box::new(body),
                        span: condition_span.merge(*statement_span),
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::FinishBlock {
                    span,
                    path,
                    statements,
                } => {
                    let tail = results.pop().expect("block tail result retained");
                    let ty = tail.ty.clone();
                    let ownership = tail.ownership;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Block {
                            statements,
                            tail: Box::new(tail),
                        },
                        span,
                    });
                }
                Frame::FinishIf { span, path } => {
                    let mut children = take_results(&mut results, 3).into_iter();
                    let condition = children.next().expect("if condition retained");
                    let then_branch = children.next().expect("if then branch retained");
                    let else_branch = children.next().expect("if else branch retained");
                    let ty = then_branch.ty.clone();
                    let ownership = then_branch.ownership;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::If {
                            condition: Box::new(condition),
                            then_branch: Box::new(then_branch),
                            else_branch: Box::new(else_branch),
                        },
                        span,
                    });
                }
                Frame::AfterIfCondition {
                    span,
                    path,
                    then_branch,
                    else_branch,
                    bindings,
                } => {
                    frames.push(Frame::AfterIfThen {
                        span,
                        path: path.clone(),
                        else_branch,
                        bindings: bindings.clone(),
                    });
                    frames.push(Frame::Enter {
                        expr: then_branch,
                        bindings,
                        path: format!("{path}.then"),
                    });
                }
                Frame::AfterIfThen {
                    span,
                    path,
                    else_branch,
                    bindings,
                } => {
                    frames.push(Frame::FinishIf {
                        span,
                        path: path.clone(),
                    });
                    frames.push(Frame::Enter {
                        expr: else_branch,
                        bindings,
                        path: format!("{path}.else"),
                    });
                }
                Frame::RecordNext {
                    span,
                    path,
                    type_name,
                    record,
                    arguments,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let ty = ResolvedType::Nominal {
                            declaration: record.clone(),
                            arguments,
                        };
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::ConstructRecord {
                                record,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
                        let field = self
                            .declarations
                            .field_id(&record, &initializer.name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H001",
                                    format!(
                                        "unresolved field `{}.{}`",
                                        type_name, initializer.name
                                    ),
                                    initializer.name_span,
                                )
                            })?;
                        frames.push(Frame::RecordAfterField {
                            span,
                            path: path.clone(),
                            type_name,
                            record,
                            arguments,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::RecordAfterField {
                    span,
                    path,
                    type_name,
                    record,
                    arguments,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("record field result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::RecordNext {
                        span,
                        path,
                        type_name,
                        record,
                        arguments,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::VariantNext {
                    span,
                    path,
                    type_name,
                    case_name,
                    variant,
                    case,
                    type_arguments,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let arguments = type_arguments
                            .iter()
                            .map(|argument| self.resolve_type(argument, span))
                            .collect::<Result<Vec<_>, _>>()?;
                        let ty = ResolvedType::Nominal {
                            declaration: variant.clone(),
                            arguments,
                        };
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::ConstructVariant {
                                variant,
                                case,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
                        let field = self
                            .declarations
                            .field_id(&case, &initializer.name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H001",
                                    format!(
                                        "unresolved payload field `{type_name}::{case_name}.{}`",
                                        initializer.name
                                    ),
                                    initializer.name_span,
                                )
                            })?;
                        frames.push(Frame::VariantAfterField {
                            span,
                            path: path.clone(),
                            type_name,
                            case_name,
                            variant,
                            case,
                            type_arguments,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::VariantAfterField {
                    span,
                    path,
                    type_name,
                    case_name,
                    variant,
                    case,
                    type_arguments,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("variant field result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::VariantNext {
                        span,
                        path,
                        type_name,
                        case_name,
                        variant,
                        case,
                        type_arguments,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::AfterMatchScrutinee {
                    span,
                    path,
                    mode,
                    arms,
                    bindings,
                } => {
                    let scrutinee = results.pop().expect("match scrutinee retained");
                    // Refutable Match v1: Copy-scalar scrutinees take the
                    // literal/guard decision chain; every aggregate or
                    // non-scalar type keeps the exact pre-feature surface.
                    if matches!(
                        scrutinee.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::Char
                            | ResolvedType::Bool
                    ) {
                        if mode != ResolvedMatchMode::Value {
                            return Err(self.error(
                                "SPX-O117",
                                "explicit match ownership modes require a non-Copy record scrutinee",
                                span,
                            ));
                        }
                        if arms.is_empty() {
                            return Err(self.error("SPX-H006", "resolved match has no arms", span));
                        }
                        self.validate_refutable_match_admission(&scrutinee.ty, arms)?;
                        frames.push(Frame::ScalarMatchNext {
                            span,
                            path,
                            mode,
                            arms,
                            index: 0,
                            bindings,
                            scrutinee,
                            resolved: Vec::with_capacity(arms.len()),
                        });
                        continue;
                    }
                    let refutable_syntax = arms.iter().any(|arm| {
                        arm.guard.is_some()
                            || matches!(
                                &arm.pattern,
                                crate::ast::MatchPattern::Literal { .. }
                                    | crate::ast::MatchPattern::Or { .. }
                                    | crate::ast::MatchPattern::Binding { .. }
                            )
                    });
                    if refutable_syntax {
                        return Err(self.error(
                            "SPX-T254",
                            format!(
                                "guards and literal/or/binding patterns require a Copy-scalar \
                                 scrutinee (i64/i32/u8/char/bool); received {}",
                                scrutinee.ty.identity_key()
                            ),
                            span,
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration: matched_type,
                        arguments,
                    } = &scrutinee.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            "cannot resolve match on a non-record/non-variant value",
                            span,
                        ));
                    };
                    let matched_kind = self
                        .declarations
                        .declaration(matched_type)
                        .map(|item| item.kind)
                        .filter(|kind| {
                            matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                        })
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                "cannot resolve match on a non-record/non-variant value",
                                span,
                            )
                        })?;
                    let facts = self.declarations.type_facts(&scrutinee.ty).ok_or_else(|| {
                        self.error("SPX-H006", "match scrutinee has no type facts", span)
                    })?;
                    match (matched_kind, mode) {
                        (DeclarationKind::Variant, ResolvedMatchMode::Value)
                            if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                        (DeclarationKind::Variant, ResolvedMatchMode::Own)
                            if resolver_admits_flat_owned_byte_variant(
                                &self.declarations,
                                &scrutinee.ty,
                            ) && facts.needs_drop
                                && !facts.copy
                                && scrutinee.ownership == OwnershipMode::Own => {}
                        (DeclarationKind::Variant, ResolvedMatchMode::Borrow)
                            if resolver_admits_flat_owned_byte_variant(
                                &self.declarations,
                                &scrutinee.ty,
                            ) && facts.needs_drop
                                && !facts.copy
                                && matches!(
                                    scrutinee.ownership,
                                    OwnershipMode::Own | OwnershipMode::Borrow
                                )
                                && matches!(
                                    &scrutinee.kind,
                                    ResolvedExprKind::Place(place) if place.projections.is_empty()
                                ) => {}
                        (DeclarationKind::Variant, _) => {
                            return Err(self.error(
                                "SPX-O117",
                                "match ownership mode disagrees with the admitted variant scrutinee",
                                span,
                            ));
                        }
                        (DeclarationKind::Record, ResolvedMatchMode::Value)
                            if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                        (DeclarationKind::Record, ResolvedMatchMode::Own)
                            if facts.needs_drop
                                && !facts.copy
                                && scrutinee.ownership == OwnershipMode::Own => {}
                        (DeclarationKind::Record, ResolvedMatchMode::Borrow)
                            if facts.needs_drop
                                && !facts.copy
                                && matches!(
                                    scrutinee.ownership,
                                    OwnershipMode::Own | OwnershipMode::Borrow
                                )
                                && matches!(scrutinee.kind, ResolvedExprKind::Place(_)) => {}
                        (DeclarationKind::Record, _) => {
                            return Err(self.error(
                                "SPX-O117",
                                "match ownership mode disagrees with the record scrutinee",
                                span,
                            ));
                        }
                        _ => unreachable!("matched kind was restricted above"),
                    }
                    let matched_type = matched_type.clone();
                    let instance_arguments = arguments.clone();
                    frames.push(Frame::MatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: 0,
                        bindings,
                        scrutinee,
                        matched_type,
                        instance_arguments,
                        matched_kind,
                        resolved: Vec::with_capacity(arms.len()),
                    });
                }
                Frame::MatchNext {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    matched_type,
                    instance_arguments,
                    matched_kind,
                    resolved,
                } => {
                    if index == arms.len() {
                        let first = resolved.first().ok_or_else(|| {
                            self.error("SPX-H006", "resolved match has no arms", span)
                        })?;
                        let ty = first.value.ty.clone();
                        let ownership = first.value.ownership;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::Match {
                                mode,
                                scrutinee: Box::new(scrutinee),
                                arms: resolved,
                            },
                            span,
                        });
                    } else {
                        let arm = &arms[index];
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
                            MatchPattern::Wildcard { span }
                                if matched_kind == DeclarationKind::Variant
                                    && mode != ResolvedMatchMode::Value =>
                            {
                                return Err(self.error(
                                    "SPX-O117",
                                    "explicit ownership variant match requires every case pattern",
                                    *span,
                                ));
                            }
                            MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                            MatchPattern::Variant {
                                case_name, fields, ..
                            } => {
                                if matched_kind != DeclarationKind::Variant {
                                    return Err(self.error(
                                        "SPX-H001",
                                        "variant pattern has a record scrutinee",
                                        arm.span,
                                    ));
                                }
                                let case = self
                                    .declarations
                                    .case_id(&matched_type, case_name)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!(
                                                "unresolved case `{matched_type}::{case_name}`"
                                            ),
                                            arm.span,
                                        )
                                    })?;
                                let mut resolved_fields = Vec::with_capacity(fields.len());
                                for (field_index, field) in fields.iter().enumerate() {
                                    let field_id = self
                                        .declarations
                                        .field_id(&case, &field.name)
                                        .cloned()
                                        .ok_or_else(|| {
                                            self.error(
                                                "SPX-H001",
                                                format!(
                                                    "unresolved pattern field `{case}.{}`",
                                                    field.name
                                                ),
                                                field.span,
                                            )
                                        })?;
                                    let field_template = self
                                        .declarations
                                        .case_fields(&case)
                                        .and_then(|items| {
                                            items.iter().find(|item| item.id == field_id)
                                        })
                                        .map(|item| item.ty.clone())
                                        .ok_or_else(|| {
                                            self.error(
                                                "SPX-H001",
                                                format!("pattern field `{field_id}` has no type"),
                                                field.span,
                                            )
                                        })?;
                                    let field_ty = substitute_type(
                                        &field_template,
                                        &matched_type,
                                        &instance_arguments,
                                    )?;
                                    let field_facts = self
                                        .declarations
                                        .type_facts(&field_ty)
                                        .ok_or_else(|| {
                                            self.error(
                                                "SPX-H006",
                                                "variant pattern field has no authenticated type facts",
                                                field.span,
                                            )
                                        })?;
                                    let ownership = if field_facts.needs_drop {
                                        match mode {
                                            ResolvedMatchMode::Own => OwnershipMode::Own,
                                            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                            ResolvedMatchMode::Value => OwnershipMode::Value,
                                        }
                                    } else {
                                        OwnershipMode::Value
                                    };
                                    let binding = ResolvedBinding {
                                        id: ValueId::local(
                                            function,
                                            &format!("{path}.arm.{index}.binding.{field_index}"),
                                        ),
                                        name: field.binding.clone(),
                                        ownership,
                                        ty: field_ty.clone(),
                                        span: field.binding_span,
                                    };
                                    Rc::make_mut(&mut arm_bindings).insert(
                                        field.binding.clone(),
                                        Binding {
                                            id: binding.id.clone(),
                                            ty: field_ty,
                                            ownership,
                                            mutable: false,
                                        },
                                    );
                                    resolved_fields.push(ResolvedMatchPatternField {
                                        field: field_id,
                                        binding,
                                    });
                                }
                                ResolvedMatchPattern::Variant {
                                    variant: matched_type.clone(),
                                    case,
                                    fields: resolved_fields,
                                }
                            }
                            MatchPattern::Record {
                                type_name,
                                fields,
                                span: pattern_span,
                                ..
                            } => {
                                if matched_kind != DeclarationKind::Record {
                                    return Err(self.error(
                                        "SPX-H001",
                                        "record pattern has a variant scrutinee",
                                        arm.span,
                                    ));
                                }
                                self.resolve_record_match_pattern(
                                    function,
                                    &scrutinee.ty,
                                    type_name,
                                    fields,
                                    Rc::make_mut(&mut arm_bindings),
                                    &format!("{path}.arm.{index}.record"),
                                    *pattern_span,
                                    mode,
                                )?
                            }
                            // Refutable Match v1 patterns on aggregate
                            // scrutinees were rejected during admission
                            // (SPX-T254); the legacy chain never sees them.
                            MatchPattern::Literal { span, .. }
                            | MatchPattern::Or { span, .. }
                            | MatchPattern::Binding { span, .. } => {
                                return Err(self.error(
                                    "SPX-T254",
                                    "guards and literal/or/binding patterns require a \
                                     Copy-scalar scrutinee",
                                    *span,
                                ));
                            }
                        };
                        frames.push(Frame::MatchAfterArm {
                            span,
                            path: path.clone(),
                            mode,
                            arms,
                            index,
                            bindings,
                            scrutinee,
                            matched_type,
                            instance_arguments,
                            matched_kind,
                            resolved,
                            pattern,
                        });
                        frames.push(Frame::Enter {
                            expr: &arm.value,
                            bindings: arm_bindings,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                Frame::MatchAfterArm {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    matched_type,
                    instance_arguments,
                    matched_kind,
                    mut resolved,
                    pattern,
                } => {
                    let value = results.pop().expect("match arm value retained");
                    // Aggregate matches reject guards with SPX-T254 before
                    // any arm resolves, so pre-feature arms carry no guard.
                    resolved.push(ResolvedMatchArm {
                        pattern,
                        guard: None,
                        value,
                        span: arms[index].span,
                    });
                    frames.push(Frame::MatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: index + 1,
                        bindings,
                        scrutinee,
                        matched_type,
                        instance_arguments,
                        matched_kind,
                        resolved,
                    });
                }
                Frame::ScalarMatchNext {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    resolved,
                } => {
                    if index == arms.len() {
                        // SPX-T257 already guaranteed a trailing catch-all
                        // during admission, so at least one arm exists and
                        // all arm values unified to one type.
                        let ty = resolved[0].value.ty.clone();
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: OwnershipMode::Value,
                            kind: ResolvedExprKind::Match {
                                mode,
                                scrutinee: Box::new(scrutinee),
                                arms: resolved,
                            },
                            span,
                        });
                    } else {
                        let arm = &arms[index];
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
                            crate::ast::MatchPattern::Wildcard { .. } => {
                                ResolvedMatchPattern::Wildcard
                            }
                            crate::ast::MatchPattern::Binding { name, .. } => {
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{index}.binding"),
                                    ),
                                    name: name.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: scrutinee.ty.clone(),
                                    span: arm.pattern.span(),
                                };
                                Rc::make_mut(&mut arm_bindings).insert(
                                    name.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        mutable: false,
                                    },
                                );
                                ResolvedMatchPattern::Binding(binding)
                            }
                            crate::ast::MatchPattern::Literal { value, .. } => {
                                ResolvedMatchPattern::Literal(PatternValue::from_ast(*value))
                            }
                            crate::ast::MatchPattern::Or { alternatives, .. } => {
                                ResolvedMatchPattern::Or(
                                    alternatives
                                        .iter()
                                        .map(|alternative| match alternative {
                                            crate::ast::MatchPattern::Literal { value, .. } => {
                                                ResolvedMatchPattern::Literal(
                                                    PatternValue::from_ast(*value),
                                                )
                                            }
                                            // SPX-M105 rejected non-literal
                                            // alternatives during admission.
                                            _ => {
                                                unreachable!("or-pattern alternatives are literals")
                                            }
                                        })
                                        .collect(),
                                )
                            }
                            // Aggregate patterns on scalar scrutinees were
                            // rejected during admission.
                            crate::ast::MatchPattern::Variant { .. }
                            | crate::ast::MatchPattern::Record { .. } => {
                                return Err(self.error(
                                    "SPX-H001",
                                    "aggregate pattern has a Copy-scalar scrutinee",
                                    arm.span,
                                ));
                            }
                        };
                        frames.push(Frame::ScalarMatchAfterArm {
                            span,
                            path: path.clone(),
                            mode,
                            arms,
                            index,
                            bindings,
                            scrutinee,
                            resolved,
                            pattern,
                        });
                        if let Some(guard) = &arm.guard {
                            frames.push(Frame::Enter {
                                expr: guard.as_ref(),
                                bindings: arm_bindings.clone(),
                                path: format!("{path}.arm.{index}.guard"),
                            });
                        }
                        frames.push(Frame::Enter {
                            expr: &arm.value,
                            bindings: arm_bindings,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                Frame::ScalarMatchAfterArm {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    mut resolved,
                    pattern,
                } => {
                    // The guard's Enter resolved after the value's, so the
                    // guard's result sits on top of the results stack.
                    let guard = arms[index]
                        .guard
                        .is_some()
                        .then(|| Box::new(results.pop().expect("scalar match arm guard retained")));
                    let value = results.pop().expect("scalar match arm value retained");
                    if let Some(guard) = &guard {
                        if guard.ty != ResolvedType::Bool {
                            return Err(self.error(
                                "SPX-T256",
                                format!(
                                    "match guard must be bool; received {}",
                                    guard.ty.identity_key()
                                ),
                                arms[index].span,
                            ));
                        }
                    }
                    if let Some(first) = resolved.first() {
                        if first.value.ty != value.ty || first.value.ownership != value.ownership {
                            return Err(self.error(
                                "SPX-T259",
                                format!(
                                    "match arms disagree on the result type; expected {}",
                                    first.value.ty.identity_key()
                                ),
                                arms[index].span,
                            ));
                        }
                    }
                    resolved.push(ResolvedMatchArm {
                        pattern,
                        guard,
                        value,
                        span: arms[index].span,
                    });
                    frames.push(Frame::ScalarMatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: index + 1,
                        bindings,
                        scrutinee,
                        resolved,
                    });
                }
                Frame::FinishTry { span, path } => {
                    let operand = results.pop().expect("try operand retained");
                    let operand_type = operand.ty.clone();
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = &operand_type
                    else {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved `?` operand is not the ordinary Result",
                            span,
                        ));
                    };
                    let target = self
                        .program
                        .functions
                        .iter()
                        .find(|candidate| {
                            matches!(
                                function,
                                FunctionExecutionId::Monomorphic(declaration)
                                    if candidate.stable_id == declaration.as_str()
                            )
                        })
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("resolved `?` has unknown enclosing function `{function}`"),
                                span,
                            )
                        })?;
                    let residual_type = self.resolve_type(&target.return_type, target.span)?;
                    let (kind, ty) = match (declaration.as_str(), arguments.as_slice()) {
                        (crate::prelude::RESULT_ID, [ok_type, _]) => (
                            ResolvedExprKind::Try {
                                operand: Box::new(operand),
                                result: DeclarationId::new(crate::prelude::RESULT_ID),
                                ok_case: DeclarationId::new(crate::prelude::RESULT_OK_ID),
                                ok_field: DeclarationId::new(crate::prelude::RESULT_OK_VALUE_ID),
                                err_case: DeclarationId::new(crate::prelude::RESULT_ERR_ID),
                                err_field: DeclarationId::new(crate::prelude::RESULT_ERR_ERROR_ID),
                                residual_type,
                            },
                            ok_type.clone(),
                        ),
                        (crate::prelude::OPTION_ID, [some_type]) => (
                            ResolvedExprKind::TryOption {
                                operand: Box::new(operand),
                                option: DeclarationId::new(crate::prelude::OPTION_ID),
                                some_case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                                some_field: DeclarationId::new(
                                    crate::prelude::OPTION_SOME_VALUE_ID,
                                ),
                                none_case: DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                                residual_type,
                            },
                            some_type.clone(),
                        ),
                        _ => {
                            return Err(self.error(
                                "SPX-H006",
                                "resolved `?` operand is not an ordinary Result or Option",
                                span,
                            ));
                        }
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind,
                        span,
                    });
                }
                Frame::AfterUpdateBase {
                    span,
                    path,
                    fields,
                    bindings,
                } => {
                    let base = results.pop().expect("record update base retained");
                    let ResolvedType::Nominal {
                        declaration: record,
                        ..
                    } = &base.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            "cannot resolve a record update on a non-record value",
                            span,
                        ));
                    };
                    if self
                        .declarations
                        .declaration(record)
                        .is_none_or(|item| item.kind != DeclarationKind::Record)
                    {
                        return Err(self.error(
                            "SPX-H001",
                            "cannot resolve a record update on a non-record value",
                            span,
                        ));
                    }
                    let record = record.clone();
                    frames.push(Frame::UpdateNext {
                        span,
                        path,
                        base,
                        record,
                        fields,
                        index: 0,
                        bindings,
                        resolved: Vec::with_capacity(fields.len()),
                    });
                }
                Frame::UpdateNext {
                    span,
                    path,
                    base,
                    record,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let ty = base.ty.clone();
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::UpdateRecord {
                                base: Box::new(base),
                                record,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
                        let field = self
                            .declarations
                            .field_id(&record, &initializer.name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H001",
                                    format!(
                                        "unresolved replacement field `{}.{}`",
                                        record, initializer.name
                                    ),
                                    initializer.name_span,
                                )
                            })?;
                        frames.push(Frame::UpdateAfterField {
                            span,
                            path: path.clone(),
                            base,
                            record,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::UpdateAfterField {
                    span,
                    path,
                    base,
                    record,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("record replacement result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::UpdateNext {
                        span,
                        path,
                        base,
                        record,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::FinishProject { span, path, field } => {
                    let base = results.pop().expect("projection base retained");
                    let ResolvedType::Nominal {
                        declaration: owner,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve field `{field}` on a non-record value"),
                            span,
                        ));
                    };
                    if self.declarations.declaration(owner).is_none_or(|item| {
                        !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
                    }) {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve field `{field}` on a non-record value"),
                            span,
                        ));
                    }
                    let field_id = self
                        .declarations
                        .field_id(owner, field)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{field}` on `{owner}`"),
                                span,
                            )
                        })?;
                    let field_ty = self
                        .declarations
                        .record_fields(owner)
                        .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                        .map(|item| item.ty.clone())
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("field `{field_id}` has no resolved type"),
                                span,
                            )
                        })?;
                    let field_ty = substitute_type(&field_ty, owner, arguments)?;
                    let ownership = self.expression_ownership(&field_ty, base.ownership, span)?;
                    let kind = match &base.kind {
                        ResolvedExprKind::Place(place) => {
                            let mut place = place.clone();
                            place
                                .projections
                                .push(PlaceProjection::Field(field_id.clone()));
                            ResolvedExprKind::Place(place)
                        }
                        _ => ResolvedExprKind::Project {
                            base: Box::new(base),
                            field: field_id,
                        },
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: field_ty,
                        ownership,
                        kind,
                        span,
                    });
                }
                Frame::FinishMethodCall {
                    span,
                    path,
                    method,
                    receiver: receiver_ast,
                    bindings,
                    type_arguments,
                    args_len,
                } => {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-P106",
                            "method generic arguments are not supported in this slice",
                            span,
                        ));
                    }
                    let mut all = take_results(&mut results, args_len + 1);
                    let receiver = all.remove(0);
                    let args = all;
                    let receiver_class = match &receiver.ty {
                        ResolvedType::Nominal {
                            declaration: class_id,
                            arguments,
                        } => Some((class_id.clone(), arguments.clone())),
                        _ => None,
                    };
                    let Some((class_id, class_args)) = receiver_class else {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve method `{method}` on a non-class value"),
                            span,
                        ));
                    };
                    let class_decl = self.declarations.declaration(&class_id).ok_or_else(|| {
                        self.error("SPX-H001", format!("unknown class `{class_id}`"), span)
                    })?;
                    if class_decl.kind != DeclarationKind::Class {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` requires a class receiver, found `{class_id}`"
                            ),
                            span,
                        ));
                    }
                    // Class Inheritance v1: method resolution walks the
                    // declared receiver's ancestor chain nearest-first, so an
                    // override replaces the inherited symbol for receivers of
                    // its own class while unoverridden parents stay callable.
                    let (holder, method_id) =
                        self.resolve_method_in_chain(&class_id, method, span)?;
                    // An inherited receiver is consumed through a prefix
                    // upcast: re-enter the receiver expression at the
                    // canonical `.source` identity and wrap its result before
                    // the method-call continuation resumes.
                    if holder != class_id {
                        self.check_upcast_admissible(&class_id, &holder, receiver.span)?;
                        frames.push(Frame::StartUpcast {
                            source: receiver_ast,
                            bindings: bindings.clone(),
                            slot_path: format!("{path}.arg.0"),
                            holder: holder.clone(),
                            span: receiver.span,
                            resume: Box::new(Frame::FinishMethodCall {
                                span,
                                path,
                                method,
                                receiver: receiver_ast,
                                bindings,
                                type_arguments,
                                args_len,
                            }),
                        });
                        continue;
                    }
                    let holder_ast = self
                        .program
                        .types
                        .iter()
                        .find(|t| t.stable_id == holder.as_str())
                        .ok_or_else(|| {
                            self.error("SPX-H006", format!("class `{holder}` has no AST"), span)
                        })?;
                    let TypeDeclarationKind::Class { methods, .. } = &holder_ast.kind else {
                        return Err(self.error(
                            "SPX-H006",
                            format!("`{holder}` is not a class"),
                            span,
                        ));
                    };
                    let method_ast =
                        methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved method `{method}` on class `{holder}`"),
                                span,
                            )
                        })?;
                    if method_ast.params.is_empty() {
                        return Err(self.error(
                            "SPX-H001",
                            format!("method `{method}` has no self parameter"),
                            method_ast.span,
                        ));
                    }
                    let self_param = &method_ast.params[0];
                    let self_ty = self.resolve_type(&self_param.ty, self_param.span)?;
                    let expected_self = ResolvedType::Nominal {
                        declaration: holder.clone(),
                        arguments: class_args.clone(),
                    };
                    if self_ty != expected_self {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` self parameter type `{:?}` does not match class `{holder}`",
                                self_ty
                            ),
                            self_param.span,
                        ));
                    }
                    if method_ast.params.len() - 1 != args.len() {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` expects {} arguments, found {}",
                                method_ast.params.len() - 1,
                                args.len()
                            ),
                            span,
                        ));
                    }
                    for (arg, param) in args.iter().zip(method_ast.params.iter().skip(1)) {
                        let param_ty = self.resolve_type(&param.ty, param.span)?;
                        if arg.ty != param_ty {
                            return Err(self.error(
                                "SPX-H001",
                                format!(
                                    "method `{method}` argument `{}` expects type `{}`, found `{}`",
                                    param.name,
                                    param_ty.identity_key(),
                                    arg.ty.identity_key()
                                ),
                                arg.span,
                            ));
                        }
                    }
                    let return_ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                    let ownership =
                        self.expression_ownership(&return_ty, OwnershipMode::Own, span)?;
                    let callee = method_id;
                    let mut call_args = Vec::with_capacity(1 + args.len());
                    call_args.push(receiver);
                    call_args.extend(args);
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: return_ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments: Vec::new(),
                            instance: None,
                            args: call_args,
                        },
                        span,
                    });
                }
                Frame::FinishSuperMethod {
                    span,
                    method_span,
                    path,
                    method,
                    holder,
                    callee,
                    args_len,
                } => {
                    let mut all = take_results(&mut results, args_len + 1);
                    let receiver = all.remove(0);
                    let args = all;
                    // The inherited receiver is the enclosing override's own
                    // `self`, upcast to the declaring ancestor exactly like a
                    // declared-type binding. The source place is synthesized
                    // here, so both identities are canonical by construction.
                    let ResolvedType::Nominal {
                        declaration: self_class,
                        ..
                    } = &receiver.ty
                    else {
                        return Err(self.error(
                            "SPX-H006",
                            "super receiver is not a class value",
                            method_span,
                        ));
                    };
                    let receiver = if *self_class == holder {
                        receiver
                    } else {
                        self.check_upcast_admissible(self_class, &holder, method_span)?;
                        let source = ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0.source")),
                            ty: receiver.ty.clone(),
                            ownership: receiver.ownership,
                            kind: receiver.kind,
                            span: receiver.span,
                        };
                        ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0")),
                            ty: ResolvedType::Nominal {
                                declaration: holder.clone(),
                                arguments: Vec::new(),
                            },
                            ownership: self.expression_ownership(
                                &ResolvedType::Nominal {
                                    declaration: holder.clone(),
                                    arguments: Vec::new(),
                                },
                                OwnershipMode::Own,
                                span,
                            )?,
                            kind: ResolvedExprKind::Upcast {
                                source: Box::new(source),
                            },
                            span: receiver.span,
                        }
                    };
                    let holder_ast = self
                        .program
                        .types
                        .iter()
                        .find(|t| t.stable_id == holder.as_str())
                        .ok_or_else(|| {
                            self.error("SPX-H006", format!("class `{holder}` has no AST"), span)
                        })?;
                    let TypeDeclarationKind::Class { methods, .. } = &holder_ast.kind else {
                        return Err(self.error(
                            "SPX-H006",
                            format!("`{holder}` is not a class"),
                            span,
                        ));
                    };
                    let method_ast =
                        methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved super method `{method}` on `{holder}`"),
                                method_span,
                            )
                        })?;
                    if method_ast.params.is_empty() {
                        return Err(self.error(
                            "SPX-H001",
                            format!("super method `{method}` has no self parameter"),
                            method_ast.span,
                        ));
                    }
                    if method_ast.params.len() - 1 != args.len() {
                        return Err(self.error(
                            "SPX-T231",
                            format!(
                                "`super.{method}` expects {} arguments, found {}",
                                method_ast.params.len() - 1,
                                args.len()
                            ),
                            span,
                        ));
                    }
                    for (arg, param) in args.iter().zip(method_ast.params.iter().skip(1)) {
                        let param_ty = self.resolve_type(&param.ty, param.span)?;
                        if arg.ty != param_ty {
                            return Err(self.error(
                                "SPX-T231",
                                format!(
                                    "`super.{method}` argument `{}` expects type `{}`, found `{}`",
                                    param.name,
                                    param_ty.identity_key(),
                                    arg.ty.identity_key()
                                ),
                                arg.span,
                            ));
                        }
                    }
                    let return_ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                    let ownership =
                        self.expression_ownership(&return_ty, OwnershipMode::Own, span)?;
                    let mut call_args = Vec::with_capacity(1 + args.len());
                    call_args.push(receiver);
                    call_args.extend(args);
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: return_ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments: Vec::new(),
                            instance: None,
                            args: call_args,
                        },
                        span,
                    });
                }
                Frame::StartUpcast {
                    source,
                    bindings,
                    slot_path,
                    holder,
                    span,
                    resume,
                } => {
                    // Re-resolve the consumed expression at the canonical
                    // `.source` identity below its occupied slot, then wrap
                    // and resume the interrupted continuation.
                    frames.push(*resume);
                    frames.push(Frame::FinishUpcast {
                        slot_path: slot_path.clone(),
                        holder,
                        span,
                    });
                    frames.push(Frame::Enter {
                        expr: source,
                        bindings,
                        path: format!("{slot_path}.source"),
                    });
                }
                Frame::FinishUpcast {
                    slot_path,
                    holder,
                    span,
                } => {
                    let source = results.pop().expect("upcast source result retained");
                    let declared = ResolvedType::Nominal {
                        declaration: holder,
                        arguments: Vec::new(),
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &slot_path),
                        ty: declared.clone(),
                        ownership: self.expression_ownership(
                            &declared,
                            OwnershipMode::Own,
                            span,
                        )?,
                        kind: ResolvedExprKind::Upcast {
                            source: Box::new(source),
                        },
                        span,
                    });
                }
            }
        }

        if results.len() != 1 {
            return Err(self.error(
                "SPX-H006",
                "iterative expression resolver finished with an invalid result stack",
                expr.span,
            ));
        }
        results.pop().ok_or_else(|| {
            self.error(
                "SPX-H006",
                "iterative expression resolver lost its root result",
                expr.span,
            )
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn resolve_expr_recursive_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let id = ExpressionId::new(function, path);
        let (kind, ty, ownership) = match &expr.kind {
            ExprKind::Int(value) => (
                ResolvedExprKind::Int(*value),
                ResolvedType::I64,
                OwnershipMode::Value,
            ),
            ExprKind::Int32(value) => (
                ResolvedExprKind::Int32(*value),
                ResolvedType::I32,
                OwnershipMode::Value,
            ),
            ExprKind::Char(value) => (
                ResolvedExprKind::Char(*value),
                ResolvedType::Char,
                OwnershipMode::Value,
            ),
            ExprKind::Uint8(value) => (
                ResolvedExprKind::Uint8(*value),
                ResolvedType::U8,
                OwnershipMode::Value,
            ),
            ExprKind::Usize(value) => (
                ResolvedExprKind::Usize(*value),
                ResolvedType::Usize,
                OwnershipMode::Value,
            ),
            ExprKind::ArrayU8(values) => (
                ResolvedExprKind::ArrayU8(values.clone()),
                ResolvedType::ArrayU8(values.len() as u32),
                OwnershipMode::Value,
            ),
            ExprKind::RepeatArrayU8 { value, count } => (
                ResolvedExprKind::RepeatArrayU8 {
                    value: *value,
                    count: *count,
                },
                ResolvedType::ArrayU8(*count),
                OwnershipMode::Value,
            ),
            ExprKind::Float32(bits) => (
                ResolvedExprKind::Float32(*bits),
                ResolvedType::F32,
                OwnershipMode::Value,
            ),
            ExprKind::Float64(bits) => (
                ResolvedExprKind::Float64(*bits),
                ResolvedType::F64,
                OwnershipMode::Value,
            ),
            ExprKind::Bool(value) => (
                ResolvedExprKind::Bool(*value),
                ResolvedType::Bool,
                OwnershipMode::Value,
            ),
            ExprKind::String(value) => (
                ResolvedExprKind::String(value.clone()),
                ResolvedType::String,
                OwnershipMode::Own,
            ),
            ExprKind::Var(name) => {
                let binding = bindings.get(name).ok_or_else(|| {
                    self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                })?;
                (
                    ResolvedExprKind::Place(Place {
                        root: binding.id.clone(),
                        projections: Vec::new(),
                    }),
                    binding.ty.clone(),
                    binding.ownership,
                )
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                if let Some(import_id) = self.declarations.native_rust_import_id(name).cloned() {
                    let import = self
                        .program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|import| import.stable_id == import_id.as_str())
                        .expect("native Rust import index is built from source imports");
                    if !type_arguments.is_empty() || args.len() != import.params.len() {
                        return Err(self.error(
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.native-rust-arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (argument, parameter) in args.iter().zip(&import.params) {
                        if argument.ty != self.resolve_type(&parameter.ty, parameter.span)? {
                            return Err(self.error(
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                argument.span,
                            ));
                        }
                    }
                    let result = match import.result {
                        crate::ast::ImportResult::Unit => ResolvedImportResultKind::Unit,
                        crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                        crate::ast::ImportResult::Bool => ResolvedImportResultKind::Bool,
                    };
                    let ty = match result {
                        ResolvedImportResultKind::Unit => ResolvedType::Unit,
                        ResolvedImportResultKind::I64 => ResolvedType::I64,
                        ResolvedImportResultKind::Bool => ResolvedType::Bool,
                    };
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::NativeRustImportCall(
                            ResolvedNativeRustImportCall {
                                expression: ExpressionId::new(function, path),
                                import: import_id,
                                args,
                                result,
                            },
                        ),
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::string_ops::by_name(name) {
                    // Oracle parity: the recursive-reference resolver admits
                    // string operations exactly like the iterative resolver.
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("string operation `{name}` has type arguments"),
                            expr.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!(
                                "string operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::str_ops::by_name(name) {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("borrowed string operation `{name}` has type arguments"),
                            expr.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!(
                                "borrowed string operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "borrowed string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::byte_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("invalid byte operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "byte operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    if op == crate::byte_ops::ByteOp::Range {
                        let mut args = args.into_iter();
                        let source = args.next().expect("range has a source");
                        if !matches!(source.kind, ResolvedExprKind::Place(ref place) if place.projections.is_empty())
                        {
                            return Err(self.error(
                                "SPX-T266",
                                "byte_range requires an exact named Slice<u8> source",
                                source.span,
                            ));
                        }
                        let start = args.next().expect("range has a start");
                        let end = args.next().expect("range has an end");
                        debug_assert!(args.next().is_none());
                        return Ok(ResolvedExpr {
                            id,
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::ByteRange {
                                operation: DeclarationId::new(crate::byte_ops::RANGE_ID),
                                source: Box::new(source),
                                start: Box::new(start),
                                end: Box::new(end),
                            },
                            span: expr.span,
                        });
                    }
                    if op.is_view() {
                        let ResolvedExprKind::Place(place) = &args[0].kind else {
                            return Err(self.error(
                                "SPX-T266",
                                format!("byte view `{name}` requires an exact named storage root"),
                                args[0].span,
                            ));
                        };
                        return Ok(ResolvedExpr {
                            id,
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::BorrowPlace {
                                operation: DeclarationId::new(op.id()),
                                place: place.clone(),
                            },
                            span: expr.span,
                        });
                    }
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::host_io_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-T269",
                            format!("invalid host I/O operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T269",
                                format!(
                                    "host I/O operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    return Ok(ResolvedExpr {
                        id,
                        ty: op.return_type(),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::command_io_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != crate::command_io_ops::arity(op)
                    {
                        return Err(self.error(
                            "SPX-T270",
                            format!("invalid command I/O operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !crate::command_io_ops::accepts_resolved(op, index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T270",
                                format!("command I/O operation `{name}` argument {index} has the wrong type"),
                                argument.span,
                            ));
                        }
                    }
                    return Ok(ResolvedExpr {
                        id: id.clone(),
                        ty: crate::command_io_ops::return_type(op),
                        ownership: crate::command_io_ops::result_ownership(op),
                        kind: ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                            expression: id,
                            operation: op,
                            args,
                        }),
                        span: expr.span,
                    });
                }
                let template = self
                    .declarations
                    .function_id(name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("unresolved function `{name}`"),
                            expr.span,
                        )
                    })?;
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|function| function.stable_id == template.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("function identity `{template}` has no declaration"),
                            expr.span,
                        )
                    })?;
                let resolved_arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let (callee, instance, return_source_type) = if target.type_parameters.is_empty() {
                    if !resolved_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("monomorphic function `{template}` has type arguments"),
                            expr.span,
                        ));
                    }
                    (template.clone(), None, target.return_type.clone())
                } else {
                    if resolved_arguments.len() != target.type_parameters.len()
                        || resolved_arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        })
                    {
                        return Err(self.error(
                            "SPX-H006",
                            format!("generic function `{template}` has invalid type arguments"),
                            expr.span,
                        ));
                    }
                    let instance = FunctionInstanceId::derive(&template, &resolved_arguments);
                    let return_type = substitute_source_function_type(
                        target,
                        type_arguments,
                        &target.return_type,
                    )
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("generic function `{template}` return substitution failed"),
                            expr.span,
                        )
                    })?;
                    (template.clone(), Some(instance), return_type)
                };
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        self.resolve_expr_recursive_reference(
                            function,
                            argument,
                            bindings,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                let ty = self.resolve_type(&return_source_type, target.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, target.span)?;
                (
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments: resolved_arguments,
                        instance,
                        args,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::MethodCall {
                receiver,
                method,
                type_arguments,
                args,
                ..
            } => {
                if !type_arguments.is_empty() {
                    return Err(self.error(
                        "SPX-P106",
                        "method call generic arguments are not supported in this slice",
                        expr.span,
                    ));
                }
                let receiver = self.resolve_expr_recursive_reference(
                    function,
                    receiver,
                    bindings,
                    &format!("{path}.arg.0"),
                )?;
                let ResolvedType::Nominal {
                    declaration: class_id,
                    arguments: class_args,
                } = &receiver.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve method `{method}` on a non-class value"),
                        expr.span,
                    ));
                };
                let class_decl = self.declarations.declaration(class_id).ok_or_else(|| {
                    self.error("SPX-H001", format!("unknown class `{class_id}`"), expr.span)
                })?;
                if class_decl.kind != DeclarationKind::Class {
                    return Err(self.error(
                        "SPX-H001",
                        format!("method `{method}` requires a class receiver, found `{class_id}`"),
                        expr.span,
                    ));
                }
                let method_id = self
                    .declarations
                    .declarations()
                    .find(|decl| {
                        decl.kind == DeclarationKind::Function
                            && decl.name == *method
                            && decl.owner.as_ref() == Some(class_id)
                    })
                    .map(|decl| decl.id.clone())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved method `{method}` on class `{class_id}`"),
                            expr.span,
                        )
                    })?;
                let class_ast = self
                    .program
                    .types
                    .iter()
                    .find(|t| t.stable_id == class_id.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("class `{class_id}` has no AST"),
                            expr.span,
                        )
                    })?;
                let TypeDeclarationKind::Class { methods, .. } = &class_ast.kind else {
                    return Err(self.error(
                        "SPX-H006",
                        format!("`{class_id}` is not a class"),
                        expr.span,
                    ));
                };
                let method_ast = methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                    self.error(
                        "SPX-H001",
                        format!("unresolved method `{method}` on class `{class_id}`"),
                        expr.span,
                    )
                })?;
                let Some(self_param) = method_ast.params.first() else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("method `{method}` has no self parameter"),
                        method_ast.span,
                    ));
                };
                let self_ty = self.resolve_type(&self_param.ty, self_param.span)?;
                let expected_self = ResolvedType::Nominal {
                    declaration: class_id.clone(),
                    arguments: class_args.clone(),
                };
                if self_ty != expected_self {
                    return Err(self.error(
                        "SPX-H001",
                        format!(
                            "method `{method}` self parameter type does not match class `{class_id}`"
                        ),
                        self_param.span,
                    ));
                }
                if method_ast.params.len() - 1 != args.len() {
                    return Err(self.error(
                        "SPX-H001",
                        format!(
                            "method `{method}` expects {} arguments, found {}",
                            method_ast.params.len() - 1,
                            args.len()
                        ),
                        expr.span,
                    ));
                }
                let mut call_args = Vec::with_capacity(1 + args.len());
                call_args.push(receiver);
                for (index, (argument, param)) in args
                    .iter()
                    .zip(method_ast.params.iter().skip(1))
                    .enumerate()
                {
                    let resolved = self.resolve_expr_recursive_reference(
                        function,
                        argument,
                        bindings,
                        &format!("{path}.arg.{}", index + 1),
                    )?;
                    let param_ty = self.resolve_type(&param.ty, param.span)?;
                    if resolved.ty != param_ty {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` argument `{}` expects type mismatch",
                                param.name
                            ),
                            argument.span,
                        ));
                    }
                    call_args.push(resolved);
                }
                let ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::Call {
                        callee: method_id,
                        type_arguments: Vec::new(),
                        instance: None,
                        args: call_args,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Unary { op, value } => {
                // Peel this linear family without consuming resolver frames.
                // The general expression-frame conversion handles the other
                // recursive families separately; this fast path preserves the
                // exact canonical `.value` identity chain.
                let mut unary = Vec::new();
                unary.push((*op, expr.span, path.to_owned()));
                let mut leaf = value.as_ref();
                let mut leaf_path = format!("{path}.value");
                while let ExprKind::Unary { op, value } = &leaf.kind {
                    unary.push((*op, leaf.span, leaf_path.clone()));
                    leaf = value;
                    leaf_path.push_str(".value");
                }
                let mut resolved =
                    self.resolve_expr_recursive_reference(function, leaf, bindings, &leaf_path)?;
                for (op, span, unary_path) in unary.into_iter().rev() {
                    let ty = match (&op, &resolved.ty) {
                        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
                        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
                        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
                        (UnaryOp::Neg, _) => ResolvedType::I64,
                        (UnaryOp::Not, _) => ResolvedType::Bool,
                    };
                    resolved = ResolvedExpr {
                        id: ExpressionId::new(function, &unary_path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Unary {
                            op,
                            value: Box::new(resolved),
                        },
                        span,
                    };
                }
                return Ok(resolved);
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.resolve_expr_recursive_reference(
                    function,
                    left,
                    bindings,
                    &format!("{path}.left"),
                )?;
                let right = self.resolve_expr_recursive_reference(
                    function,
                    right,
                    bindings,
                    &format!("{path}.right"),
                )?;
                let ty = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => ResolvedType::I64,
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Block { statements, tail } => {
                let mut scope = bindings.clone();
                let mut resolved_statements = Vec::with_capacity(statements.len());
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        Statement::Let {
                            name,
                            name_span,
                            mutable,
                            declared: _,
                            value,
                            span,
                        } => {
                            let value = self.resolve_expr_recursive_reference(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &statement_path),
                                name: name.clone(),
                                ownership: value.ownership,
                                ty: value.ty.clone(),
                                span: *name_span,
                            };
                            scope.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    mutable: *mutable,
                                },
                            );
                            resolved_statements.push(ResolvedStatement::Let {
                                binding,
                                mutable: *mutable,
                                value,
                                span: *span,
                            });
                        }
                        Statement::Assign {
                            name,
                            name_span,
                            field,
                            value,
                            span,
                        } => {
                            let immutable_code = if field.is_some() {
                                "SPX-U107"
                            } else {
                                "SPX-U101"
                            };
                            let target = self.resolve_assign_target(
                                name,
                                *name_span,
                                &scope,
                                immutable_code,
                            )?;
                            let target_field = match field {
                                Some(field) => {
                                    Some(self.resolve_assign_field_target(&target, field)?)
                                }
                                None => None,
                            };
                            let value = self.resolve_expr_recursive_reference(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            match &target_field {
                                Some((_, field_ty)) => {
                                    if value.ty != *field_ty {
                                        return Err(self.error(
                                            "SPX-U110",
                                            format!(
                                                "assigned value type `{}` does not exactly match field type `{}`",
                                                value.ty.identity_key(),
                                                field_ty.identity_key()
                                            ),
                                            value.span,
                                        ));
                                    }
                                }
                                None => {
                                    if value.ty != target.ty {
                                        return Err(self.error(
                                            "SPX-U102",
                                            format!(
                                                "assigned value type `{}` does not exactly match binding type `{}`",
                                                value.ty.identity_key(),
                                                target.ty.identity_key()
                                            ),
                                            value.span,
                                        ));
                                    }
                                    if value.ownership != OwnershipMode::Value
                                        || !is_scalar_resolved_type(&value.ty)
                                    {
                                        return Err(self.error(
                                            "SPX-U105",
                                            "explicit mutation v1 supports only scalar Copy values",
                                            value.span,
                                        ));
                                    }
                                }
                            }
                            resolved_statements.push(ResolvedStatement::Assign {
                                binding: target,
                                field: target_field.map(|(field_id, _)| field_id),
                                value,
                                span: *span,
                            });
                        }
                        Statement::Unsafe {
                            audit, body, span, ..
                        } => {
                            let body = self.resolve_expr_recursive_reference(
                                function,
                                body,
                                &scope,
                                &format!("{statement_path}.body"),
                            )?;
                            if body.ownership != OwnershipMode::Value
                                || !is_scalar_resolved_type(&body.ty)
                            {
                                return Err(self.error(
                                    "SPX-N104",
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                    body.span,
                                ));
                            }
                            resolved_statements.push(ResolvedStatement::Unsafe {
                                audit: audit.clone(),
                                body: Box::new(body),
                                span: *span,
                            });
                        }
                        Statement::While {
                            condition,
                            body,
                            span,
                            ..
                        } => {
                            // Mirror the iterative admission and typing checks
                            // exactly, including path spellings.
                            self.reject_while_disallowed(condition)?;
                            self.reject_while_disallowed(body)?;
                            let resolved_condition = self.resolve_expr_recursive_reference(
                                function,
                                condition,
                                &scope,
                                &format!("{statement_path}.condition"),
                            )?;
                            if resolved_condition.ty != ResolvedType::Bool {
                                return Err(self.error(
                                    "SPX-T251",
                                    "`while` condition must be bool",
                                    condition.span,
                                ));
                            }
                            let resolved_body = self.resolve_expr_recursive_reference(
                                function,
                                body,
                                &scope,
                                &format!("{statement_path}.body"),
                            )?;
                            resolved_statements.push(ResolvedStatement::While {
                                condition: Box::new(resolved_condition),
                                body: Box::new(resolved_body),
                                span: condition.span.merge(*span),
                            });
                        }
                    }
                }
                let tail = self.resolve_expr_recursive_reference(
                    function,
                    tail,
                    &scope,
                    &format!("{path}.tail"),
                )?;
                let ty = tail.ty.clone();
                let ownership = tail.ownership;
                (
                    ResolvedExprKind::Block {
                        statements: resolved_statements,
                        tail: Box::new(tail),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.resolve_expr_recursive_reference(
                    function,
                    condition,
                    bindings,
                    &format!("{path}.condition"),
                )?;
                let then_branch = self.resolve_expr_recursive_reference(
                    function,
                    then_branch,
                    bindings,
                    &format!("{path}.then"),
                )?;
                let else_branch = self.resolve_expr_recursive_reference(
                    function,
                    else_branch,
                    bindings,
                    &format!("{path}.else"),
                )?;
                let ty = then_branch.ty.clone();
                let ownership = then_branch.ownership;
                (
                    ResolvedExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                let record = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved record `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self.declarations.declaration(&record).is_none_or(|item| {
                    !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
                }) {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a record or class"),
                        expr.span,
                    ));
                }
                let arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let parameters = self.declarations.type_parameters(&record).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("record `{record}` has no parameter metadata"),
                        expr.span,
                    )
                })?;
                if arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(self.error(
                        "SPX-H006",
                        format!("record `{record}` has invalid concrete arguments"),
                        expr.span,
                    ));
                }
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{}.{}`", type_name, initializer.name),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: record.clone(),
                    arguments,
                };
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::ConstructRecord {
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                case_name,
                fields,
                ..
            } => {
                let variant = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved variant `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self
                    .declarations
                    .declaration(&variant)
                    .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a variant"),
                        expr.span,
                    ));
                }
                let case = self
                    .declarations
                    .case_id(&variant, case_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved case `{type_name}::{case_name}`"),
                            expr.span,
                        )
                    })?;
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&case, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved payload field `{type_name}::{case_name}.{}`",
                                    initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: variant.clone(),
                    arguments: type_arguments
                        .iter()
                        .map(|argument| self.resolve_type(argument, expr.span))
                        .collect::<Result<Vec<_>, _>>()?,
                };
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                let mode = ResolvedMatchMode::from(*mode);
                let scrutinee = self.resolve_expr_recursive_reference(
                    function,
                    scrutinee,
                    bindings,
                    &format!("{path}.scrutinee"),
                )?;
                // Refutable Match v1: mirror of the iterative resolver's
                // Copy-scalar decision chain, producing identical identities.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    if mode != ResolvedMatchMode::Value {
                        return Err(self.error(
                            "SPX-O117",
                            "explicit match ownership modes require a non-Copy record scrutinee",
                            expr.span,
                        ));
                    }
                    if arms.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved match has no arms",
                            expr.span,
                        ));
                    }
                    self.validate_refutable_match_admission(&scrutinee.ty, arms)?;
                    let mut resolved_arms: Vec<ResolvedMatchArm> = Vec::with_capacity(arms.len());
                    for (arm_index, arm) in arms.iter().enumerate() {
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
                            MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                            MatchPattern::Binding { name, .. } => {
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding"),
                                    ),
                                    name: name.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: scrutinee.ty.clone(),
                                    span: arm.pattern.span(),
                                };
                                arm_bindings.insert(
                                    name.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        mutable: false,
                                    },
                                );
                                ResolvedMatchPattern::Binding(binding)
                            }
                            MatchPattern::Literal { value, .. } => {
                                ResolvedMatchPattern::Literal(PatternValue::from_ast(*value))
                            }
                            MatchPattern::Or { alternatives, .. } => ResolvedMatchPattern::Or(
                                alternatives
                                    .iter()
                                    .map(|alternative| match alternative {
                                        MatchPattern::Literal { value, .. } => {
                                            ResolvedMatchPattern::Literal(PatternValue::from_ast(
                                                *value,
                                            ))
                                        }
                                        _ => unreachable!("or-pattern alternatives are literals"),
                                    })
                                    .collect(),
                            ),
                            MatchPattern::Variant { .. } | MatchPattern::Record { .. } => {
                                return Err(self.error(
                                    "SPX-H001",
                                    "aggregate pattern has a Copy-scalar scrutinee",
                                    arm.span,
                                ));
                            }
                        };
                        let guard = match &arm.guard {
                            Some(guard) => {
                                let resolved_guard = self.resolve_expr_recursive_reference(
                                    function,
                                    guard.as_ref(),
                                    &arm_bindings,
                                    &format!("{path}.arm.{arm_index}.guard"),
                                )?;
                                if resolved_guard.ty != ResolvedType::Bool {
                                    return Err(self.error(
                                        "SPX-T256",
                                        format!(
                                            "match guard must be bool; received {}",
                                            resolved_guard.ty.identity_key()
                                        ),
                                        arm.span,
                                    ));
                                }
                                Some(Box::new(resolved_guard))
                            }
                            None => None,
                        };
                        let value = self.resolve_expr_recursive_reference(
                            function,
                            &arm.value,
                            &arm_bindings,
                            &format!("{path}.arm.{arm_index}.value"),
                        )?;
                        if let Some(first) = resolved_arms.first() {
                            if first.value.ty != value.ty
                                || first.value.ownership != value.ownership
                            {
                                return Err(self.error(
                                    "SPX-T259",
                                    format!(
                                        "match arms disagree on the result type; expected {}",
                                        first.value.ty.identity_key()
                                    ),
                                    arm.span,
                                ));
                            }
                        }
                        resolved_arms.push(ResolvedMatchArm {
                            pattern,
                            guard,
                            value,
                            span: arm.span,
                        });
                    }
                    return Ok(ResolvedExpr {
                        id: ExpressionId::new(function, path),
                        ty: resolved_arms[0].value.ty.clone(),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Match {
                            mode,
                            scrutinee: Box::new(scrutinee),
                            arms: resolved_arms,
                        },
                        span: expr.span,
                    });
                }
                let refutable_syntax = arms.iter().any(|arm| {
                    arm.guard.is_some()
                        || matches!(
                            &arm.pattern,
                            crate::ast::MatchPattern::Literal { .. }
                                | crate::ast::MatchPattern::Or { .. }
                                | crate::ast::MatchPattern::Binding { .. }
                        )
                });
                if refutable_syntax {
                    return Err(self.error(
                        "SPX-T254",
                        format!(
                            "guards and literal/or/binding patterns require a Copy-scalar \
                             scrutinee (i64/i32/u8/char/bool); received {}",
                            scrutinee.ty.identity_key()
                        ),
                        expr.span,
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: matched_type,
                    arguments,
                } = &scrutinee.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                };
                let matched_kind = self
                    .declarations
                    .declaration(matched_type)
                    .map(|item| item.kind);
                if !matches!(
                    matched_kind,
                    Some(DeclarationKind::Record | DeclarationKind::Variant)
                ) {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                }
                let matched_kind = matched_kind.expect("matched kind checked above");
                let facts = self.declarations.type_facts(&scrutinee.ty).ok_or_else(|| {
                    self.error("SPX-H006", "match scrutinee has no type facts", expr.span)
                })?;
                match (matched_kind, mode) {
                    (DeclarationKind::Variant, ResolvedMatchMode::Value)
                        if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                    (DeclarationKind::Variant, ResolvedMatchMode::Own)
                        if resolver_admits_flat_owned_byte_variant(
                            &self.declarations,
                            &scrutinee.ty,
                        ) && facts.needs_drop
                            && !facts.copy
                            && scrutinee.ownership == OwnershipMode::Own => {}
                    (DeclarationKind::Variant, ResolvedMatchMode::Borrow)
                        if resolver_admits_flat_owned_byte_variant(
                            &self.declarations,
                            &scrutinee.ty,
                        ) && facts.needs_drop
                            && !facts.copy
                            && matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            )
                            && matches!(
                                &scrutinee.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            ) => {}
                    (DeclarationKind::Variant, _) => {
                        return Err(self.error(
                            "SPX-O117",
                            "match ownership mode disagrees with the admitted variant scrutinee",
                            expr.span,
                        ));
                    }
                    (DeclarationKind::Record, ResolvedMatchMode::Value)
                        if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                    (DeclarationKind::Record, ResolvedMatchMode::Own)
                        if facts.needs_drop
                            && !facts.copy
                            && scrutinee.ownership == OwnershipMode::Own => {}
                    (DeclarationKind::Record, ResolvedMatchMode::Borrow)
                        if facts.needs_drop
                            && !facts.copy
                            && matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            )
                            && matches!(scrutinee.kind, ResolvedExprKind::Place(_)) => {}
                    (DeclarationKind::Record, _) => {
                        return Err(self.error(
                            "SPX-O117",
                            "match ownership mode disagrees with the record scrutinee",
                            expr.span,
                        ));
                    }
                    _ => unreachable!("matched kind was restricted above"),
                }
                let instance_arguments = arguments.clone();
                let matched_type = matched_type.clone();
                let mut resolved_arms = Vec::with_capacity(arms.len());
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_bindings = bindings.clone();
                    let pattern = match &arm.pattern {
                        MatchPattern::Wildcard { span }
                            if matched_kind == DeclarationKind::Variant
                                && mode != ResolvedMatchMode::Value =>
                        {
                            return Err(self.error(
                                "SPX-O117",
                                "explicit ownership variant match requires every case pattern",
                                *span,
                            ));
                        }
                        MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                        MatchPattern::Variant {
                            case_name, fields, ..
                        } => {
                            if matched_kind != DeclarationKind::Variant {
                                return Err(self.error(
                                    "SPX-H001",
                                    "variant pattern has a record scrutinee",
                                    arm.span,
                                ));
                            }
                            let case = self
                                .declarations
                                .case_id(&matched_type, case_name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H001",
                                        format!("unresolved case `{matched_type}::{case_name}`"),
                                        arm.span,
                                    )
                                })?;
                            let mut resolved_fields = Vec::with_capacity(fields.len());
                            for (field_index, field) in fields.iter().enumerate() {
                                let field_id = self
                                    .declarations
                                    .field_id(&case, &field.name)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!(
                                                "unresolved pattern field `{case}.{}`",
                                                field.name
                                            ),
                                            field.span,
                                        )
                                    })?;
                                let field_template = self
                                    .declarations
                                    .case_fields(&case)
                                    .and_then(|items| items.iter().find(|item| item.id == field_id))
                                    .map(|item| item.ty.clone())
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!("pattern field `{field_id}` has no type"),
                                            field.span,
                                        )
                                    })?;
                                let field_ty = substitute_type(
                                    &field_template,
                                    &matched_type,
                                    &instance_arguments,
                                )?;
                                let field_facts =
                                    self.declarations.type_facts(&field_ty).ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            "variant pattern field has no authenticated type facts",
                                            field.span,
                                        )
                                    })?;
                                let ownership = if field_facts.needs_drop {
                                    match mode {
                                        ResolvedMatchMode::Own => OwnershipMode::Own,
                                        ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                        ResolvedMatchMode::Value => OwnershipMode::Value,
                                    }
                                } else {
                                    OwnershipMode::Value
                                };
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding.{field_index}"),
                                    ),
                                    name: field.binding.clone(),
                                    ownership,
                                    ty: field_ty.clone(),
                                    span: field.binding_span,
                                };
                                arm_bindings.insert(
                                    field.binding.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: field_ty,
                                        ownership,
                                        mutable: false,
                                    },
                                );
                                resolved_fields.push(ResolvedMatchPatternField {
                                    field: field_id,
                                    binding,
                                });
                            }
                            ResolvedMatchPattern::Variant {
                                variant: matched_type.clone(),
                                case,
                                fields: resolved_fields,
                            }
                        }
                        MatchPattern::Record {
                            type_name,
                            fields,
                            span,
                            ..
                        } => {
                            if matched_kind != DeclarationKind::Record {
                                return Err(self.error(
                                    "SPX-H001",
                                    "record pattern has a variant scrutinee",
                                    arm.span,
                                ));
                            }
                            self.resolve_record_match_pattern(
                                function,
                                &scrutinee.ty,
                                type_name,
                                fields,
                                &mut arm_bindings,
                                &format!("{path}.arm.{arm_index}.record"),
                                *span,
                                mode,
                            )?
                        }
                        // Refutable Match v1 patterns on aggregate
                        // scrutinees were rejected during admission
                        // (SPX-T254); the legacy chain never sees them.
                        MatchPattern::Literal { span, .. }
                        | MatchPattern::Or { span, .. }
                        | MatchPattern::Binding { span, .. } => {
                            return Err(self.error(
                                "SPX-T254",
                                "guards and literal/or/binding patterns require a \
                                 Copy-scalar scrutinee",
                                *span,
                            ));
                        }
                    };
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &arm.value,
                        &arm_bindings,
                        &format!("{path}.arm.{arm_index}.value"),
                    )?;
                    resolved_arms.push(ResolvedMatchArm {
                        pattern,
                        // Aggregate matches reject guards with SPX-T254
                        // before any arm resolves.
                        guard: None,
                        value,
                        span: arm.span,
                    });
                }
                let first = resolved_arms.first().ok_or_else(|| {
                    self.error("SPX-H006", "resolved match has no arms", expr.span)
                })?;
                let ty = first.value.ty.clone();
                let ownership = first.value.ownership;
                (
                    ResolvedExprKind::Match {
                        mode,
                        scrutinee: Box::new(scrutinee),
                        arms: resolved_arms,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Try { operand } => {
                let operand = self.resolve_expr_recursive_reference(
                    function,
                    operand,
                    bindings,
                    &format!("{path}.operand"),
                )?;
                let operand_type = operand.ty.clone();
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &operand_type
                else {
                    return Err(self.error(
                        "SPX-H006",
                        "resolved `?` operand is not the ordinary Result",
                        expr.span,
                    ));
                };
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|candidate| {
                        matches!(
                            function,
                            FunctionExecutionId::Monomorphic(declaration)
                                if candidate.stable_id == declaration.as_str()
                        )
                    })
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("resolved `?` has unknown enclosing function `{function}`"),
                            expr.span,
                        )
                    })?;
                let residual_type = self.resolve_type(&target.return_type, target.span)?;
                match (declaration.as_str(), arguments.as_slice()) {
                    (crate::prelude::RESULT_ID, [ok_type, _]) => (
                        ResolvedExprKind::Try {
                            operand: Box::new(operand),
                            result: DeclarationId::new(crate::prelude::RESULT_ID),
                            ok_case: DeclarationId::new(crate::prelude::RESULT_OK_ID),
                            ok_field: DeclarationId::new(crate::prelude::RESULT_OK_VALUE_ID),
                            err_case: DeclarationId::new(crate::prelude::RESULT_ERR_ID),
                            err_field: DeclarationId::new(crate::prelude::RESULT_ERR_ERROR_ID),
                            residual_type,
                        },
                        ok_type.clone(),
                        OwnershipMode::Value,
                    ),
                    (crate::prelude::OPTION_ID, [some_type]) => (
                        ResolvedExprKind::TryOption {
                            operand: Box::new(operand),
                            option: DeclarationId::new(crate::prelude::OPTION_ID),
                            some_case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                            some_field: DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID),
                            none_case: DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                            residual_type,
                        },
                        some_type.clone(),
                        OwnershipMode::Value,
                    ),
                    _ => {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved `?` operand is not an ordinary Result or Option",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                let base = self.resolve_expr_recursive_reference(
                    function,
                    base,
                    bindings,
                    &format!("{path}.base"),
                )?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments: _,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                }
                let record = record.clone();
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved replacement field `{}.{}`",
                                    record, initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = base.ty.clone();
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::UpdateRecord {
                        base: Box::new(base),
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            // The test-only reference resolver never walks class-method
            // bodies; `super` resolution is owned by the iterative resolver.
            ExprKind::SuperMethod { method_span, .. } => {
                return Err(self.error(
                    "SPX-T231",
                    "`super` is only allowed inside a class-method override",
                    *method_span,
                ));
            }
            ExprKind::Project { base, field, .. } => {
                let base = self.resolve_expr_recursive_reference(
                    function,
                    base,
                    bindings,
                    &format!("{path}.base"),
                )?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                }
                let instance_arguments = arguments.clone();
                let field_id = self
                    .declarations
                    .field_id(record, field)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved field `{field}` on record `{record}`"),
                            expr.span,
                        )
                    })?;
                let field_ty = self
                    .declarations
                    .record_fields(record)
                    .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                    .map(|field| field.ty.clone())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("field `{field_id}` has no resolved type"),
                            expr.span,
                        )
                    })?;
                let field_ty = substitute_type(&field_ty, record, &instance_arguments)?;
                let ownership = self.expression_ownership(&field_ty, base.ownership, expr.span)?;
                let kind = match &base.kind {
                    ResolvedExprKind::Place(place) => {
                        let mut place = place.clone();
                        place
                            .projections
                            .push(PlaceProjection::Field(field_id.clone()));
                        ResolvedExprKind::Place(place)
                    }
                    _ => ResolvedExprKind::Project {
                        base: Box::new(base),
                        field: field_id,
                    },
                };
                (kind, field_ty, ownership)
            }
        };
        Ok(ResolvedExpr {
            id,
            ty,
            ownership,
            kind,
            span: expr.span,
        })
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic::error(code, message, span).at_path(&self.program.path)
    }

    /// Class Inheritance v1: finds the nearest ancestor of `start` (inclusive)
    /// declaring a method named `method`, returning the declaring class and
    /// the method's stable identity.
    fn resolve_method_in_chain(
        &self,
        start: &DeclarationId,
        method: &str,
        span: Span,
    ) -> Result<(DeclarationId, DeclarationId), Diagnostic> {
        let mut chain = vec![start.clone()];
        chain.extend(self.declarations.class_ancestors(start));
        for class in &chain {
            if let Some(declaration) = self
                .declarations
                .declarations()
                .find(|decl| {
                    decl.kind == DeclarationKind::Function
                        && decl.name == method
                        && decl.owner.as_ref() == Some(class)
                })
                .map(|decl| decl.id.clone())
            {
                return Ok((class.clone(), declaration));
            }
        }
        Err(self.error(
            "SPX-H001",
            format!("unresolved method `{method}` on class `{start}`"),
            span,
        ))
    }

    /// Class Inheritance v1: consumes `receiver` (a whole value of some
    /// descendant of `holder`) as a `holder`-typed value. Exact-type receivers
    /// pass through unchanged; descendants are consumed through the same
    /// prefix-upcast block a declared-type binding uses, which requires the
    /// child-declared suffix to introduce no cleanup leaves.
    /// Class Inheritance v1: admits an implicit upcast from class `child` to
    /// its ancestor `parent`. The prefix must be exactly the ancestor's
    /// effective layout, and the child-declared suffix must be cleanup-inert:
    /// consuming the child transfers its inherited leaves into the
    /// ancestor-typed result, so owned suffix state would otherwise leak.
    fn check_upcast_admissible(
        &self,
        child: &DeclarationId,
        parent: &DeclarationId,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !self.declarations.class_extends(child, parent) {
            return Err(self.error(
                "SPX-T232",
                format!("`{child}` does not inherit from `{parent}`"),
                span,
            ));
        }
        let child_fields = self.declarations.record_fields(child).ok_or_else(|| {
            self.error("SPX-H006", format!("class `{child}` has no fields"), span)
        })?;
        let parent_fields = self.declarations.record_fields(parent).ok_or_else(|| {
            self.error("SPX-H006", format!("class `{parent}` has no fields"), span)
        })?;
        if child_fields.len() < parent_fields.len()
            || child_fields[..parent_fields.len()]
                .iter()
                .zip(parent_fields.iter())
                .any(|(child_field, parent_field)| child_field.id != parent_field.id)
        {
            return Err(self.error(
                "SPX-H006",
                format!("class `{child}` prefix disagrees with ancestor `{parent}`"),
                span,
            ));
        }
        for field in &child_fields[parent_fields.len()..] {
            let drops = self
                .declarations
                .type_facts(&field.ty)
                .is_some_and(|facts| facts.needs_drop);
            if drops {
                return Err(self.error(
                    "SPX-T233",
                    format!(
                        "upcast from `{child}` to `{parent}` would discard owned field `{}`; only cleanup-inert child fields are admitted in this slice",
                        field.name
                    ),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn expression_ownership(
        &self,
        ty: &ResolvedType,
        non_copy_mode: OwnershipMode,
        span: Span,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy_mode
                }
            })
            .ok_or_else(|| {
                self.error(
                    "SPX-H004",
                    format!(
                        "semantic facts are unavailable for type `{}`",
                        ty.identity_key()
                    ),
                    span,
                )
            })
    }
}

#[cfg(test)]
mod iterative_validator_tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use super::*;
    use crate::{hir, parse};

    #[test]
    fn hostile_declaration_index_rejects_reserved_host_id_for_every_authored_kind() {
        let source = r#"
module test.reserved_host_index;
@id("token") resource Token { @id("token.drop") drop trivial; }
@id("pair") record Pair { @id("pair.x") x: i64, }
@id("choice") variant Choice { @id("choice.a") A { @id("choice.a.x") x: i64, }, }
@id("class") class Class { @id("class.x") x: i64, @id("class.value") fn value(self: Class) -> i64 { self.x } }
@id("host") interface Host permits {} {
    @id("host.echo") import rust fn echo(value: i64) -> i64 effects {} failure infallible;
}
@id("app.main") fn main() -> i64 { 0 }
"#;
        let parsed = parse(source, Path::new("reserved-host-index.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        for identity in [
            "token",
            "token.drop",
            "pair",
            "pair.x",
            "choice",
            "choice.a",
            "choice.a.x",
            "class",
            "class.x",
            "class.value",
            "host",
            "host.echo",
        ] {
            let mut hostile = resolved.clone();
            let original = DeclarationId::new(identity);
            let mut declaration = hostile
                .declarations
                .declarations
                .remove(&original)
                .expect("fixture declaration is indexed");
            declaration.id = DeclarationId::new(crate::host_io_ops::STDOUT_WRITE_ID);
            hostile
                .declarations
                .declarations
                .insert(declaration.id.clone(), declaration);
            let diagnostic = hir::validate(&hostile).unwrap_err();
            assert!(
                diagnostic
                    .message
                    .contains("aliases a compiler-owned host I/O operation"),
                "reserved hostile declaration kind `{identity}` was not rejected first: {diagnostic:?}"
            );
        }
    }

    #[test]
    fn iterative_resolver_matches_recursive_reference_outside_builder_accounting() {
        let source = r#"
module test.resolver_oracle;
permit { host.echo }
@id("choice")
variant Choice {
  @id("choice.a") A { @id("choice.a.v") v: i64, },
  @id("choice.b") B,
}

@id("pair")
record Pair {
  @id("pair.a") a: i64,
  @id("pair.b") b: i64,
}
@id("host.echo.interface")
interface HostEcho permits { host.echo } {
  @id("host.echo") import rust fn host_echo(value: i64) -> i64
    effects { host.echo }
    failure status "host.echo.v1";
}
@id("callee") fn callee(a: i64, b: i64) -> i64 { a + b }
@id("identity") fn identity<T>(value: T) -> T { value }
@id("option_use") fn option_use(value: Option<i64>) -> Option<bool> {
  let checked = value?;
  Option<bool>::Some { value: checked > 0 }
}
@id("result_use") fn result_use(value: Result<i64, bool>) -> Result<bool, bool> {
  let checked = value?;
  Result<bool, bool>::Ok { value: checked > 0 }
}
@id("match_value") fn match_value(value: i64) -> i64 {
  match value { _ => value, }
}
@id("match_own") fn match_own(value: Choice) -> i64 {
  match value { Choice::A { v } => v, Choice::B {} => 0, }
}
@id("match_borrow") fn match_borrow(value: Choice) -> i64 {
  match value { Choice::A { v } => v, Choice::B {} => 0, }
}
@id("exercise") fn exercise(flag: bool, choice: Choice, pair: Pair) -> i64
  uses { host.echo }
{
  let x = callee(1, 2);
  let mut total = x;
  total = total + x;
  let native = host_echo(identity<i64>(total));
  let rebuilt = if flag && !false { Choice::A { v: Pair { a: native, b: 3 }.a } } else { choice };
  let y = pair with { b: 4 }.b;
  match rebuilt { Choice::A { v } => y + v, Choice::B {} => -y, }
}
@id("main") fn main() -> i64 { 0 }
"#;
        let parsed = parse(source, Path::new("resolver-oracle.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let resolver = Resolver {
            program: &parsed,
            declarations: DeclarationIndex::from_verified(&parsed).unwrap(),
        };
        for source_function in &parsed.functions {
            let Some(resolved_function) = resolved
                .functions
                .iter()
                .find(|function| function.id.as_str() == source_function.stable_id)
            else {
                continue;
            };
            let execution = FunctionExecutionId::Monomorphic(resolved_function.id.clone());
            let bindings = source_function
                .params
                .iter()
                .zip(&resolved_function.params)
                .map(|(source, resolved)| {
                    (
                        source.name.clone(),
                        Binding {
                            id: resolved.id.clone(),
                            ty: resolved.ty.clone(),
                            ownership: resolved.ownership,
                            mutable: false,
                        },
                    )
                })
                .collect();
            let iterative = resolver.resolve_expr_iterative(
                &execution,
                &source_function.body,
                &bindings,
                "body",
            );
            let recursive = resolver.resolve_expr_recursive_reference(
                &execution,
                &source_function.body,
                &bindings,
                "body",
            );
            match (iterative, recursive) {
                (Ok(iterative), Ok(recursive)) => assert_eq!(iterative, recursive),
                (Err(iterative), Err(recursive)) => {
                    assert_eq!(iterative.code, recursive.code);
                    assert_eq!(iterative.severity, recursive.severity);
                    assert_eq!(iterative.message, recursive.message);
                    assert_eq!(iterative.path, recursive.path);
                    assert_eq!(iterative.span, recursive.span);
                    assert_eq!(iterative.help, recursive.help);
                }
                (iterative, recursive) => panic!(
                    "resolver oracle outcome differs: iterative={iterative:?}, recursive={recursive:?}"
                ),
            }
        }

        for (function_id, expected_mode) in [
            ("match_value", ResolvedMatchMode::Value),
            ("match_own", ResolvedMatchMode::Value),
            ("match_borrow", ResolvedMatchMode::Value),
        ] {
            let function = resolved
                .functions
                .iter()
                .find(|function| function.id.as_str() == function_id)
                .expect("match-mode fixture is resolved");
            let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
                panic!("match-mode fixture body is a block with a tail")
            };
            let ResolvedExprKind::Match { mode, .. } = &tail.kind else {
                panic!("match-mode fixture tail is a match")
            };
            assert_eq!(*mode, expected_mode);
        }

        let invalid = parse(
            "module test.resolver_invalid; @id(\"main\") fn main() -> i64 { missing }",
            Path::new("resolver-invalid.spx"),
        )
        .unwrap();
        let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("main"));
        let iterative = resolver.resolve_expr_iterative(
            &execution,
            &invalid.functions[0].body,
            &BTreeMap::new(),
            "body",
        );
        let recursive = resolver.resolve_expr_recursive_reference(
            &execution,
            &invalid.functions[0].body,
            &BTreeMap::new(),
            "body",
        );
        let (Err(iterative), Err(recursive)) = (iterative, recursive) else {
            panic!("unresolved-value oracle must fail in both evaluators")
        };
        assert_eq!(iterative.code, recursive.code);
        assert_eq!(iterative.severity, recursive.severity);
        assert_eq!(iterative.message, recursive.message);
        assert_eq!(iterative.path, recursive.path);
        assert_eq!(iterative.span, recursive.span);
        assert_eq!(iterative.help, recursive.help);
    }

    const SOURCE: &str = r#"
module test.validator_oracle_hostiles;
permit { host.echo }

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("owned.box")
record OwnedBox { @id("owned.box.token") token: Token, }

@id("choice.type")
variant Choice { @id("choice.a") A, @id("choice.b") B, }

@id("host.echo.interface")
interface HostEcho permits { host.echo } {
    @id("host.echo")
    import rust fn host_echo(value: i64) -> i64
        effects { host.echo }
        failure status "host.echo.v1";
}

@id("token.consume")
fn consume(token: own Token) -> i64 { 1 }

@id("token.consume_bool")
fn consume_bool(token: own Token) -> bool { true }

@id("hostile.call")
fn call_hostile(token: own Token) -> i64 { consume(token) }

@id("hostile.native")
fn native_hostile(token: own Token, value: i64) -> i64
    uses { host.echo }
{ host_echo(value) }

@id("hostile.construct")
fn construct_hostile(token: own Token) -> OwnedBox { OwnedBox { token: token } }

@id("hostile.update")
fn update_hostile(input: own OwnedBox, token: own Token) -> OwnedBox {
    input with { token: token }
}

@id("hostile.block_statement")
fn block_statement_hostile(token: own Token) -> i64 {
    let used = consume(token);
    used
}

@id("hostile.block_tail")
fn block_tail_hostile(token: own Token) -> i64 {
    let zero = 0;
    consume(token)
}

@id("hostile.if")
fn if_hostile(flag: bool, token: own Token) -> i64 {
    if flag { consume(token) } else { 0 }
}

@id("hostile.lazy")
fn lazy_hostile(flag: bool, token: own Token) -> bool {
    flag && consume_bool(token)
}

@id("hostile.match")
fn match_hostile(choice: Choice, token: own Token) -> i64 {
    match choice { Choice::A {} => consume(token), Choice::B {} => 0, }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        hir::resolve(&parse(SOURCE, Path::new("validator-oracle-hostiles.spx")).unwrap()).unwrap()
    }

    fn function_index(program: &ResolvedProgram, id: &str) -> usize {
        program
            .functions
            .iter()
            .position(|function| function.id.as_str() == id)
            .unwrap()
    }

    fn tail_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("fixture function body must remain a block")
        };
        tail
    }

    fn validation_scope(function: &ResolvedFunction) -> BTreeMap<ValueId, ValidationBinding> {
        function
            .params
            .iter()
            .map(|param| {
                (
                    param.id.clone(),
                    ValidationBinding {
                        ty: param.ty.clone(),
                        ownership: param.ownership,
                        availability: Availability::Available,
                        active_loans: BTreeSet::new(),
                        moved_places: BTreeMap::new(),
                        definitely_partial: BTreeSet::new(),
                    },
                )
            })
            .collect()
    }

    fn validate_expression_hostile(
        program: &ResolvedProgram,
        function_id: &str,
        expression: &ResolvedExpr,
        path: &str,
    ) -> BTreeMap<String, Availability> {
        let function = &program.functions[function_index(program, function_id)];
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let mut scope = validation_scope(function);
        let mut recursive_scope = scope.clone();
        let mut validator = HirValidator::new(program).unwrap();
        let mut recursive_validator = validator.clone();
        let allowed_effects = function.effects.iter().cloned().collect();
        let recursive = recursive_validator.validate_expr_recursive_reference(
            &execution,
            expression,
            &mut recursive_scope,
            path,
            true,
            Some(&allowed_effects),
        );
        let iterative = validator.validate_expr_iterative(
            &execution,
            expression,
            &mut scope,
            path,
            true,
            Some(&allowed_effects),
        );
        HirValidator::assert_validation_oracle(
            &iterative,
            &recursive,
            &validator,
            &recursive_validator,
            &scope,
            &recursive_scope,
            path,
        );
        let diagnostic = iterative.unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006", "{function_id}");
        function
            .params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    scope.get(&param.id).unwrap().availability,
                )
            })
            .collect()
    }

    #[test]
    fn validator_oracle_preserves_direct_child_scope_on_late_errors() {
        for (function_id, expected_token, expected_input) in [
            ("hostile.call", Availability::Moved, None),
            ("hostile.native", Availability::Available, None),
            ("hostile.construct", Availability::Moved, None),
            (
                "hostile.update",
                Availability::Moved,
                Some(Availability::Moved),
            ),
        ] {
            let mut hostile = program();
            let index = function_index(&hostile, function_id);
            tail_mut(&mut hostile.functions[index]).ownership = match function_id {
                "hostile.construct" | "hostile.update" => OwnershipMode::Value,
                "hostile.call" | "hostile.native" => OwnershipMode::Borrow,
                _ => unreachable!(),
            };
            let expression = tail_mut(&mut hostile.functions[index]).clone();
            let scope =
                validate_expression_hostile(&hostile, function_id, &expression, "body.tail");
            assert_eq!(scope["token"], expected_token);
            if let Some(expected) = expected_input {
                assert_eq!(scope["input"], expected);
            }
        }
    }

    #[test]
    fn validator_oracle_suppresses_failed_block_branch_lazy_and_match_child_scopes() {
        for function_id in [
            "hostile.block_statement",
            "hostile.block_tail",
            "hostile.if",
            "hostile.lazy",
            "hostile.match",
        ] {
            let mut hostile = program();
            let index = function_index(&hostile, function_id);
            let body = &mut hostile.functions[index].body;
            match function_id {
                "hostile.block_statement" => {
                    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
                        unreachable!()
                    };
                    let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
                        unreachable!("fixture statement is a let")
                    };
                    binding.ty = ResolvedType::Bool;
                }
                "hostile.block_tail" => {
                    tail_mut(&mut hostile.functions[index]).ownership = OwnershipMode::Borrow;
                }
                "hostile.if" => {
                    let ResolvedExprKind::If { then_branch, .. } =
                        &mut tail_mut(&mut hostile.functions[index]).kind
                    else {
                        unreachable!()
                    };
                    then_branch.ownership = OwnershipMode::Borrow;
                }
                "hostile.lazy" => {
                    let ResolvedExprKind::Binary { right, .. } =
                        &mut tail_mut(&mut hostile.functions[index]).kind
                    else {
                        unreachable!()
                    };
                    right.ownership = OwnershipMode::Borrow;
                }
                "hostile.match" => {
                    let ResolvedExprKind::Match { arms, .. } =
                        &mut tail_mut(&mut hostile.functions[index]).kind
                    else {
                        unreachable!()
                    };
                    arms[0].value.ownership = OwnershipMode::Borrow;
                }
                _ => unreachable!(),
            }
            let expression = hostile.functions[index].body.clone();
            let scope = validate_expression_hostile(&hostile, function_id, &expression, "body");
            assert_eq!(scope["token"], Availability::Available, "{function_id}");
        }
    }

    #[test]
    fn validator_oracle_handles_an_exact_depth_512_late_error_with_a_nonempty_scope() {
        fn run() {
            const UNARY_NODES: usize = 510;
            let source = format!(
                "module test.validator_depth; @id(\"token.type\") resource Token {{ @id(\"token.drop\") drop trivial; }} @id(\"token.consume\") fn consume(token: own Token) -> i64 {{ 1 }} @id(\"hostile.depth\") fn deep(token: own Token) -> i64 {{ {}consume(token) }} @id(\"app.main\") fn main() -> i64 {{ 0 }}",
                "-".repeat(UNARY_NODES)
            );
            let mut hostile =
                hir::resolve(&parse(&source, Path::new("validator-depth-hostile.spx")).unwrap())
                    .unwrap();
            let index = function_index(&hostile, "hostile.depth");
            let expression = tail_mut(&mut hostile.functions[index]);
            let mut depth = 0;
            let mut cursor = &*expression;
            loop {
                depth += 1;
                match &cursor.kind {
                    ResolvedExprKind::Unary { value, .. } => cursor = value,
                    ResolvedExprKind::Call { args, .. } => cursor = &args[0],
                    ResolvedExprKind::Place(_) => break,
                    _ => panic!("unexpected exact-depth fixture shape"),
                }
            }
            assert_eq!(depth, 512);
            expression.ownership = OwnershipMode::Borrow;
            let expression = expression.clone();
            let scope =
                validate_expression_hostile(&hostile, "hostile.depth", &expression, "body.tail");
            assert_eq!(scope["token"], Availability::Moved);
        }

        std::thread::Builder::new()
            .name("validator-depth-oracle".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn(run)
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod record_tests {
    use std::fmt::Write as _;
    use std::path::Path;

    use super::{validate, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
    use crate::{hir, parse};

    fn record_program() -> hir::ResolvedProgram {
        let source = r#"
module test.hostile_record_hir;
@id("node.type")
record Node {
    @id("node.value")
    value: i64,
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        hir::resolve(&parse(source, Path::new("hostile-record-hir.spx")).unwrap()).unwrap()
    }

    #[cfg(test)]
    mod identity_nul_tests {
        use std::path::Path;

        use super::super::{
            validate, DeclarationId, ExpressionId, ResolvedResourceDropKind,
            ResolvedTypeDeclarationKind, ValueId,
        };
        use crate::{codegen, hir, parse, wasm};

        fn identity_program() -> hir::ResolvedProgram {
            let source = r#"
module test.hostile_identity_nul;
@id("token.type")
resource Token {
    @id("token.drop")
    drop import "host.dispose";
}
@id("pair.type")
record Pair { @id("pair.value") value: i64, }
@id("host.interface")
interface Host permits {} {
    @id("host.dispose")
    import fn dispose(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}
@id("helper.function")
fn helper(value: i64) -> i64 { value }
@id("pair.make")
fn make_pair(value: i64) -> Pair { Pair { value: value } }
@id("pair.read")
fn read_pair(pair: Pair) -> i64 { pair.value }
@id("pair.read-temporary")
fn read_temporary() -> i64 { Pair { value: 1 }.value }
@id("token.discard")
fn discard(token: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { helper(1) }
"#;
            hir::resolve(&parse(source, Path::new("hostile-identity-nul.spx")).unwrap()).unwrap()
        }

        fn assert_nul_rejected(program: &hir::ResolvedProgram, kind: &str) {
            let diagnostic = validate(program).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-H006", "wrong code for {kind}");
            assert!(
                diagnostic.message.contains("contains NUL"),
                "wrong diagnostic for {kind}: {}",
                diagnostic.message
            );
        }

        fn function_index(program: &hir::ResolvedProgram, name: &str) -> usize {
            program
                .functions
                .iter()
                .position(|function| function.name == name)
                .unwrap()
        }

        fn tail(expression: &super::super::ResolvedExpr) -> &super::super::ResolvedExpr {
            match &expression.kind {
                super::super::ResolvedExprKind::Block { tail, .. } => tail,
                _ => expression,
            }
        }

        fn tail_mut(
            expression: &mut super::super::ResolvedExpr,
        ) -> &mut super::super::ResolvedExpr {
            if matches!(
                &expression.kind,
                super::super::ResolvedExprKind::Block { .. }
            ) {
                let super::super::ResolvedExprKind::Block { tail, .. } = &mut expression.kind
                else {
                    unreachable!()
                };
                tail
            } else {
                expression
            }
        }

        #[test]
        fn validator_rejects_nul_in_every_persistent_hir_identity_carrier() {
            let original = identity_program();

            let mut program = original.clone();
            program.entrypoint = DeclarationId::new("app.main\0forged");
            assert_nul_rejected(&program, "entry point");

            let mut program = original.clone();
            program.types[0].id = DeclarationId::new("token.type\0forged");
            assert_nul_rejected(&program, "resource");

            let mut program = original.clone();
            let ResolvedTypeDeclarationKind::Resource { drop } = &mut program.types[0].kind else {
                panic!("Token must be a resource")
            };
            drop.id = DeclarationId::new("token.drop\0forged");
            assert_nul_rejected(&program, "resource lifecycle");

            let mut program = original.clone();
            let record = program
                .types
                .iter_mut()
                .find(|declaration| declaration.name == "Pair")
                .unwrap();
            record.id = DeclarationId::new("pair.type\0forged");
            assert_nul_rejected(&program, "record");

            let mut program = original.clone();
            let record = program
                .types
                .iter_mut()
                .find(|declaration| declaration.name == "Pair")
                .unwrap();
            let ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
                panic!("Pair must be a record")
            };
            fields[0].id = DeclarationId::new("pair.value\0forged");
            assert_nul_rejected(&program, "field");

            let mut program = original.clone();
            program.interfaces[0].id = DeclarationId::new("host.interface\0forged");
            assert_nul_rejected(&program, "interface");

            let mut program = original.clone();
            program.interfaces[0].imports[0].id = DeclarationId::new("host.dispose\0forged");
            assert_nul_rejected(&program, "import");

            let mut program = original.clone();
            program.interfaces[0].imports[0].import_key = "host.dispose\0forged".to_owned();
            assert_nul_rejected(&program, "logical import key");

            let mut program = original;
            program.functions[0].id = DeclarationId::new("helper.function\0forged");
            assert_nul_rejected(&program, "function");
        }

        #[test]
        fn validator_rejects_nul_in_derived_expression_and_value_identities() {
            let original = identity_program();
            let helper_index = original
                .functions
                .iter()
                .position(|function| function.name == "helper")
                .unwrap();

            let mut program = original.clone();
            program.functions[helper_index].body.id = ExpressionId("expression\0forged".to_owned());
            assert_nul_rejected(&program, "expression");

            let mut program = original.clone();
            program.functions[helper_index].params[0].id = ValueId("value\0forged".to_owned());
            assert_nul_rejected(&program, "parameter value");

            let mut program = original;
            program.functions[helper_index].result_id = ValueId("result\0forged".to_owned());
            assert_nul_rejected(&program, "result value");
        }

        #[test]
        fn validator_normalizes_nul_across_core_hir_reference_carriers() {
            let original = identity_program();

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            program.functions[helper].params[0].ty = super::super::ResolvedType::Nominal {
                declaration: DeclarationId::new("type\0forged"),
                arguments: vec![super::super::ResolvedType::TypeParameter {
                    owner: DeclarationId::new("owner.safe"),
                    index: 0,
                }],
            };
            assert_nul_rejected(&program, "nominal type declaration");

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            program.functions[helper].params[0].ty = super::super::ResolvedType::TypeParameter {
                owner: DeclarationId::new("owner\0forged"),
                index: 0,
            };
            assert_nul_rejected(&program, "type-parameter owner");

            let mut program = original.clone();
            let main = function_index(&program, "main");
            let super::super::ResolvedExprKind::Call { callee, .. } =
                &mut tail_mut(&mut program.functions[main].body).kind
            else {
                panic!("main must call helper")
            };
            *callee = DeclarationId::new("callee\0forged");
            assert_nul_rejected(&program, "call target");

            let mut program = original.clone();
            let helper = function_index(&program, "helper");
            let super::super::ResolvedExprKind::Place(place) =
                &mut tail_mut(&mut program.functions[helper].body).kind
            else {
                panic!("helper must return a place")
            };
            place.root = ValueId("place\0forged".to_owned());
            assert_nul_rejected(&program, "place root");

            let mut program = original.clone();
            let reader = function_index(&program, "read_pair");
            let super::super::ResolvedExprKind::Place(place) =
                &mut tail_mut(&mut program.functions[reader].body).kind
            else {
                panic!("record parameter projection must remain a place")
            };
            place.projections[0] =
                super::super::PlaceProjection::Field(DeclarationId::new("field\0forged"));
            assert_nul_rejected(&program, "place projection");

            let mut program = original.clone();
            let maker = function_index(&program, "make_pair");
            let super::super::ResolvedExprKind::ConstructRecord { record, .. } =
                &mut tail_mut(&mut program.functions[maker].body).kind
            else {
                panic!("make_pair must construct a record")
            };
            *record = DeclarationId::new("record\0forged");
            assert_nul_rejected(&program, "record constructor");

            let mut program = original.clone();
            let maker = function_index(&program, "make_pair");
            let super::super::ResolvedExprKind::ConstructRecord { fields, .. } =
                &mut tail_mut(&mut program.functions[maker].body).kind
            else {
                panic!("make_pair must construct a record")
            };
            fields[0].field = DeclarationId::new("initializer\0forged");
            assert_nul_rejected(&program, "record initializer field");

            let mut program = original;
            let reader = function_index(&program, "read_temporary");
            let super::super::ResolvedExprKind::Project { field, .. } =
                &mut tail_mut(&mut program.functions[reader].body).kind
            else {
                panic!("temporary record projection must remain explicit")
            };
            *field = DeclarationId::new("projected\0forged");
            assert_nul_rejected(&program, "projected field");
        }

        #[test]
        fn validator_normalizes_nul_across_cleanup_inventory_and_plan_references() {
            let original = identity_program();
            let discard = function_index(&original, "discard");

            let mut program = original.clone();
            let crate::cleanup::CleanupStorageOrigin::Parameter { value, .. } =
                &mut program.functions[discard].cleanup.slots[0].origin
            else {
                panic!("discard must own parameter storage")
            };
            *value = ValueId("inventory\0forged".to_owned());
            assert_nul_rejected(&program, "inventory value");

            let mut program = original.clone();
            program.functions[discard].cleanup.flags[0]
                .place
                .projections
                .push(DeclarationId::new("inventory.projection\0forged"));
            assert_nul_rejected(&program, "inventory projection");

            let mut program = original.clone();
            program.functions[discard].cleanup_plan.slots[0].storage =
                crate::cleanup_plan::StorageId::CallArgument {
                    call: ExpressionId("plan.call\0forged".to_owned()),
                    parameter_index: 0,
                    value_expression: ExpressionId("plan.value".to_owned()),
                };
            assert_nul_rejected(&program, "plan call-argument storage");

            let mut program = original.clone();
            program.functions[discard]
                .cleanup_plan
                .entry_state
                .live_owned_parameters[0]
                .projections
                .push(DeclarationId::new("plan.projection\0forged"));
            assert_nul_rejected(&program, "plan place projection");

            let mut program = original.clone();
            let finalizer = program.functions[discard]
                .cleanup_plan
                .exits
                .iter_mut()
                .find_map(|exit| exit.finalize_in_order.first_mut())
                .expect("discard must finalize its parameter");
            finalizer.lifecycle_id = DeclarationId::new("plan.lifecycle\0forged");
            assert_nul_rejected(&program, "plan finalizer lifecycle");

            let mut program = original;
            let main = function_index(&program, "main");
            let source = program.functions[main]
                .cleanup_plan
                .status_sources
                .iter_mut()
                .find(|source| {
                    matches!(
                        &source.producer,
                        crate::cleanup_plan::StatusProducer::PropagatedCall { .. }
                    )
                })
                .expect("main call must have a propagated status source");
            let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } =
                &mut source.producer
            else {
                unreachable!()
            };
            *callee = DeclarationId::new("plan.callee\0forged");
            assert_nul_rejected(&program, "plan propagated callee");
        }

        #[test]
        fn native_and_wasm_reject_nul_before_backend_feature_gates() {
            let mut program = identity_program();
            let main = function_index(&program, "main");
            let super::super::ResolvedExprKind::Call { callee, .. } =
                &mut tail_mut(&mut program.functions[main].body).kind
            else {
                panic!("main must call helper")
            };
            *callee = DeclarationId::new("helper.function\0forged");

            let native = codegen::emit_hir_c(&program).unwrap_err();
            assert_eq!(native.code, "SPX-H006");
            assert!(native.message.contains("contains NUL"));

            let wasm = wasm::emit_resolved_module(&program).unwrap_err();
            assert_eq!(wasm.code, "SPX-H006");
            assert!(wasm.message.contains("contains NUL"));

            let mut cleanup_program = identity_program();
            let discard = function_index(&cleanup_program, "discard");
            let finalizer = cleanup_program.functions[discard]
                .cleanup_plan
                .exits
                .iter_mut()
                .find_map(|exit| exit.finalize_in_order.first_mut())
                .expect("discard must finalize its parameter");
            finalizer.lifecycle_id = DeclarationId::new("cleanup.lifecycle\0forged");

            let native = codegen::emit_hir_c(&cleanup_program).unwrap_err();
            assert_eq!(native.code, "SPX-H006");
            assert!(native.message.contains("contains NUL"));

            let wasm = wasm::emit_resolved_module(&cleanup_program).unwrap_err();
            assert_eq!(wasm.code, "SPX-H006");
            assert!(wasm.message.contains("contains NUL"));
        }

        #[test]
        fn valid_identity_program_keeps_its_existing_validation_result() {
            let program = identity_program();
            validate(&program).unwrap();
            let helper = function_index(&program, "helper");
            assert!(matches!(
                &tail(&program.functions[helper].body).kind,
                super::super::ResolvedExprKind::Place(_)
            ));
            let ResolvedTypeDeclarationKind::Resource { drop } = &program.types[0].kind else {
                panic!("Token must be a resource")
            };
            assert!(matches!(
                drop.kind,
                ResolvedResourceDropKind::Imported { .. }
            ));
        }
    }

    #[test]
    fn validator_rejects_a_forged_by_value_recursive_record_index() {
        let mut program = record_program();
        let recursive = ResolvedType::Nominal {
            declaration: DeclarationId::new("node.type"),
            arguments: Vec::new(),
        };
        let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
            panic!("Node must be a record");
        };
        fields[0].ty = recursive.clone();
        program
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("node.type"))
            .unwrap()[0]
            .ty = recursive;

        assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
    }

    #[test]
    fn validator_rejects_unit_in_an_ordinary_record_field_and_index() {
        let mut program = record_program();
        let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
            panic!("Node must be a record");
        };
        fields[0].ty = ResolvedType::Unit;
        program
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("node.type"))
            .unwrap()[0]
            .ty = ResolvedType::Unit;

        let error = validate(&program).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("uses Unit outside a native Rust import result"));
    }

    #[test]
    fn validator_rejects_a_field_owned_by_the_wrong_record() {
        let mut program = record_program();
        program
            .declarations
            .declarations
            .get_mut(&DeclarationId::new("node.value"))
            .unwrap()
            .owner = Some(DeclarationId::new("forged.owner"));

        assert_eq!(validate(&program).unwrap_err().code, "SPX-H006");
    }

    #[test]
    fn iterative_resolver_and_validator_report_allocated_vec_capacity() {
        let source = "module capacity.hir; @id(\"capacity.choose\") fn choose(value: i64) -> i64 { if value == 0 { value } else { value + 1 } } @id(\"app.main\") fn main() -> i64 { choose(0) }";
        let parsed = crate::parse(source, std::path::Path::new("capacity-hir.spx")).unwrap();
        crate::source_verify::reset_capacity_high_water();
        super::reset_iterative_phase_capacity_high_water();
        let resolved = super::resolve(&parsed).unwrap();
        validate(&resolved).unwrap();
        let water = super::iterative_phase_capacity_high_water();
        assert!(water[0] >= std::mem::size_of::<super::ResolvedExpr>());
        assert!(water[1] > 0);
        assert!(water[2] > 0);
        assert!(crate::source_verify::capacity_high_water() > 0);
    }

    #[test]
    fn type_facts_capacity_high_water_covers_layered_and_wide_hostiles() {
        use sha2::{Digest, Sha256};

        fn layered(resource: bool, levels: usize) -> String {
            let mut source = String::from("module capacity.typefacts.layers;\n\n");
            if resource {
                source.push_str(
                    "@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n",
                );
            } else {
                source.push_str(
                    "@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n",
                );
            }
            for level in 1..=levels {
                writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
            }
            source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
            source
        }

        fn resolve_type_facts_peak(source: &str, name: &str) -> (String, usize) {
            let parsed = crate::parse(source, std::path::Path::new(name)).unwrap();
            let canonical = crate::format::canonical(&parsed);
            super::reset_iterative_phase_capacity_high_water();
            super::resolve(&parsed).unwrap();
            (
                format!(
                    "sha256:{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
                ),
                super::iterative_phase_capacity_high_water()[2],
            )
        }

        let scalar = layered(false, 12);
        let resource = layered(true, 12);
        let mut wide = String::from("module capacity.typefacts.wide;\n\n");
        for index in 0..514 {
            writeln!(
                wide,
                "@id(\"wide.r{index}\")\nrecord R{index} {{\n    @id(\"wide.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
        }
        wide.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
        let mut chain = String::from(
            "module capacity.typefacts.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
        );
        for index in 1..514 {
            writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
        }
        chain.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");

        let observed = [
            resolve_type_facts_peak(&scalar, "typefacts-layered-scalar.spx"),
            resolve_type_facts_peak(&resource, "typefacts-layered-resource.spx"),
            resolve_type_facts_peak(&wide, "typefacts-wide.spx"),
            resolve_type_facts_peak(&chain, "typefacts-chain.spx"),
        ];
        let expected = [
            (
                "sha256:cfa16985be87d169c3fb81d5958126347ec82b4c1afed878e2d98d1fbfe72c80",
                1_741_515,
                669_965_618,
            ),
            (
                "sha256:461611e4315e312330af0285273568e5d09cd8e5770a35dcf66a82783aa15ae6",
                1_397_458,
                2_886_293_140,
            ),
            (
                "sha256:dc19474b86def3eaf6e3c60cc2224694e6aa7cf2811cca6115943c11102f95fc",
                96_838,
                122_429_248,
            ),
            (
                "sha256:d2692d4883957575ee95df8f9ee7057343599e1da945c386cedea714c716f66d",
                6_273_598,
                31_588_832_202,
            ),
        ];
        for ((digest, actual), (expected_digest, expected_actual, envelope)) in
            observed.into_iter().zip(expected)
        {
            assert_eq!(digest, expected_digest, "canonical hostile fixture drifted");
            assert_eq!(
                actual, expected_actual,
                "TypeFacts owned-capacity peak drifted"
            );
            assert!(
                actual <= envelope,
                "TypeFacts observed total exceeded retained_upper + TypeFacts phase"
            );
        }
    }

    #[test]
    fn useful_data_workspace_linker_reconstructs_and_rejects_hostile_slice_provenance() {
        let source = r#"
module test.useful_data_link;

@id("data.length")
fn length(value: borrow Slice<u8>) -> usize {
    let alias = value;
    byte_len(alias)
}

@id("data.count")
fn count(value: borrow Slice<u8>) -> usize {
    let mut index = 0usize;
    while index < byte_len(value) {
        index = index + 1usize;
        index < byte_len(value)
    }
    match byte_get(value, 0usize) {
        Option::Some { value: _ } => index,
        Option::None {} => index,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(source, "useful-data-link.spx").unwrap();
        let resolved = super::resolve(&parsed).unwrap();
        let entrypoint = resolved.entrypoint.clone();
        let linked_functions = resolved
            .functions
            .iter()
            .cloned()
            .map(|function| super::LinkedScalarFunction {
                function,
                origin: super::IdentityOrigin::Explicit,
            })
            .collect::<Vec<_>>();
        let linked = super::link_useful_data_workspace(
            resolved.module.clone(),
            entrypoint.clone(),
            resolved
                .functions
                .iter()
                .cloned()
                .map(|function| super::LinkedScalarFunction {
                    function,
                    origin: super::IdentityOrigin::Explicit,
                })
                .collect(),
        )
        .unwrap();
        let length = linked
            .functions
            .iter()
            .find(|function| function.id.as_str() == "data.length")
            .unwrap();
        let parameter = &length.params[0].id;
        let provenance = linked
            .declarations
            .byte_slice_provenance(parameter)
            .unwrap();
        assert_eq!(provenance.root, *parameter);
        assert_eq!(
            provenance.root_kind,
            super::ByteSliceRootKind::FunctionParameter
        );
        assert!(linked
            .functions
            .iter()
            .any(|function| function.id.as_str() == "data.count"));

        let mut hostile = linked_functions;
        let length = hostile
            .iter_mut()
            .find(|linked| linked.function.id.as_str() == "data.length")
            .unwrap();
        let super::ResolvedExprKind::Block { statements, .. } = &mut length.function.body.kind
        else {
            panic!("fixture function body is a block");
        };
        let super::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("fixture first statement is a let");
        };
        let super::ResolvedExprKind::Place(place) = &mut value.kind else {
            panic!("fixture slice alias is a place");
        };
        place.root = super::ValueId("hostile.missing-root".to_owned());
        let error =
            super::link_useful_data_workspace(resolved.module, entrypoint, hostile).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("byte-slice alias lacks a canonical symbolic parameter root"));
    }
}
