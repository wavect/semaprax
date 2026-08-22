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
    BinaryOp, Expr, ExprKind, ImportFailure, MatchPattern, ParamMode, Program,
    ResourceLifecycleKind, Span, Statement, Type, TypeDeclarationKind, UnaryOp,
};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;
use crate::source_verify;

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

mod validation;

pub(crate) use validation::validate_core;
#[cfg(test)]
use validation::HirValidator;

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
                    crate::ast::TypeDeclarationKind::Record { fields } => {
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
        | ResolvedType::Char
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
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
        ResolvedExprKind::Unary { value: operand, .. } => bytes += child(operand),
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
                let ResolvedStatement::Let { binding, value, .. } = statement;
                bytes += binding.id.as_str().len()
                    + binding.name.capacity()
                    + resolved_type_owned_capacity(&binding.ty)
                    + resolved_expr_owned_capacity(value);
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
        ResolvedExprKind::Match { scrutinee, arms } => {
            bytes += child(scrutinee);
            bytes += arms.capacity() * std::mem::size_of::<ResolvedMatchArm>();
            bytes += arms
                .iter()
                .map(|arm| {
                    resolved_match_pattern_owned_capacity(&arm.pattern)
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
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => {}
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
    let ResolvedStatement::Let { binding, value, .. } = statement;
    resolved_binding_owned_capacity(binding) + resolved_expr_owned_capacity(value)
}

#[cfg(test)]
fn resolved_field_initializer_owned_capacity(field: &ResolvedFieldInitializer) -> usize {
    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
}

#[cfg(test)]
fn resolved_match_arm_owned_capacity(arm: &ResolvedMatchArm) -> usize {
    resolved_match_pattern_owned_capacity(&arm.pattern) + resolved_expr_owned_capacity(&arm.value)
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
    fn new(function: &FunctionExecutionId, path: &str) -> Self {
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

/// A deterministic, display-name-to-identity index.
///
/// Types and values occupy distinct namespaces so future record/variant type
/// declarations can coexist with functions without ambiguous lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    declarations: BTreeMap<DeclarationId, Declaration>,
    types_by_name: BTreeMap<String, DeclarationId>,
    functions_by_name: BTreeMap<String, DeclarationId>,
    fields_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    record_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    cases_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    variant_cases: BTreeMap<DeclarationId, Vec<ResolvedVariantCaseDeclaration>>,
    case_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    type_parameters: BTreeMap<DeclarationId, Vec<ResolvedTypeParameterDeclaration>>,
    imports_by_key: BTreeMap<String, DeclarationId>,
    native_rust_imports_by_name: BTreeMap<String, DeclarationId>,
    type_facts_by_id: BTreeMap<String, TypeFacts>,
}

impl DeclarationIndex {
    #[cfg(test)]
    fn type_facts_layout_capacity(&self) -> usize {
        self.type_facts_by_id
            .values()
            .map(|facts| facts.layout_key.capacity())
            .sum()
    }

    #[cfg(test)]
    fn owned_capacity_for_private_contract(&self) -> usize {
        fn string_map_capacity<V>(map: &BTreeMap<String, V>) -> usize {
            map.len() * std::mem::size_of::<(String, V)>()
                + map.keys().map(String::capacity).sum::<usize>()
        }
        fn named_id_map_capacity(map: &BTreeMap<String, DeclarationId>) -> usize {
            string_map_capacity(map) + map.values().map(|id| id.as_str().len()).sum::<usize>()
        }
        fn owner_name_map_capacity(
            map: &BTreeMap<(DeclarationId, String), DeclarationId>,
        ) -> usize {
            map.len() * std::mem::size_of::<((DeclarationId, String), DeclarationId)>()
                + map
                    .iter()
                    .map(|((owner, name), value)| {
                        owner.as_str().len() + name.capacity() + value.as_str().len()
                    })
                    .sum::<usize>()
        }
        fn field_capacity(field: &ResolvedFieldDeclaration) -> usize {
            field.id.as_str().len()
                + field.name.capacity()
                + resolved_type_owned_capacity(&field.ty)
        }
        let declaration_bytes = self
            .declarations
            .iter()
            .map(|(id, declaration)| {
                id.as_str().len()
                    + declaration.id.as_str().len()
                    + declaration.name.capacity()
                    + declaration
                        .owner
                        .as_ref()
                        .map_or(0, |owner| owner.as_str().len())
            })
            .sum::<usize>();
        let field_bytes = self
            .record_fields
            .values()
            .chain(self.case_fields.values())
            .flatten()
            .map(field_capacity)
            .sum::<usize>();
        let case_bytes = self
            .variant_cases
            .values()
            .flatten()
            .map(|case| {
                case.id.as_str().len()
                    + case.name.capacity()
                    + case.fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
                    + case.fields.iter().map(field_capacity).sum::<usize>()
            })
            .sum::<usize>();
        let fact_bytes = self
            .type_facts_by_id
            .values()
            .map(|facts| facts.layout_key.capacity())
            .sum::<usize>();
        let declaration_map_backing =
            self.declarations.len() * std::mem::size_of::<(DeclarationId, Declaration)>();
        let record_field_maps = self
            .record_fields
            .iter()
            .chain(self.case_fields.iter())
            .map(|(owner, fields)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedFieldDeclaration>)>()
                    + owner.as_str().len()
                    + fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
            })
            .sum::<usize>();
        let variant_case_map = self
            .variant_cases
            .iter()
            .map(|(owner, cases)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedVariantCaseDeclaration>)>()
                    + owner.as_str().len()
                    + cases.capacity() * std::mem::size_of::<ResolvedVariantCaseDeclaration>()
            })
            .sum::<usize>();
        let type_parameter_map = self
            .type_parameters
            .iter()
            .map(|(owner, parameters)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedTypeParameterDeclaration>)>()
                    + owner.as_str().len()
                    + parameters.capacity()
                        * std::mem::size_of::<ResolvedTypeParameterDeclaration>()
                    + parameters
                        .iter()
                        .map(|parameter| parameter.name.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>();
        declaration_map_backing
            + record_field_maps
            + variant_case_map
            + type_parameter_map
            + declaration_bytes
            + field_bytes
            + case_bytes
            + fact_bytes
            + named_id_map_capacity(&self.types_by_name)
            + named_id_map_capacity(&self.functions_by_name)
            + named_id_map_capacity(&self.imports_by_key)
            + named_id_map_capacity(&self.native_rust_imports_by_name)
            + string_map_capacity(&self.type_facts_by_id)
            + owner_name_map_capacity(&self.fields_by_owner_name)
            + owner_name_map_capacity(&self.cases_by_owner_name)
    }

    pub(crate) fn workspace_declarations(&self) -> Vec<Declaration> {
        self.declarations.values().cloned().collect()
    }

    pub fn declaration(&self, id: &DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id)
    }

    pub fn type_id(&self, name: &str) -> Option<&DeclarationId> {
        self.types_by_name.get(name)
    }

    pub fn function_id(&self, name: &str) -> Option<&DeclarationId> {
        self.functions_by_name.get(name)
    }

    pub fn field_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.fields_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn record_fields(&self, owner: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.record_fields.get(owner).map(Vec::as_slice)
    }

    pub fn case_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.cases_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn variant_cases(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedVariantCaseDeclaration]> {
        self.variant_cases.get(owner).map(Vec::as_slice)
    }

    pub fn case_fields(&self, case: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.case_fields.get(case).map(Vec::as_slice)
    }

    pub fn type_parameters(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&[ResolvedTypeParameterDeclaration]> {
        self.type_parameters.get(declaration).map(Vec::as_slice)
    }

    pub fn import_id(&self, key: &str) -> Option<&DeclarationId> {
        self.imports_by_key.get(key)
    }

    pub fn native_rust_import_id(&self, name: &str) -> Option<&DeclarationId> {
        self.native_rust_imports_by_name.get(name)
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &Declaration> {
        self.declarations.values()
    }

    /// Computes the recursive semantic facts shared by ownership and backends.
    ///
    /// `None` is reserved for unresolved type parameters and future malformed
    /// HIR. Every type produced for today's verified language has facts.
    pub fn type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.type_facts_by_id
            .get(&ty.identity_key())
            .cloned()
            .or_else(|| self.recompute_type_facts(ty))
    }

    fn compute_type_facts(
        &self,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        memo: &mut BTreeMap<String, TypeFacts>,
    ) -> Option<TypeFacts> {
        enum Frame {
            Enter(ResolvedType),
            Finish {
                identity: String,
                declaration: DeclarationId,
                kind: DeclarationKind,
                child_count: usize,
            },
        }

        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame) -> usize {
            match frame {
                Frame::Enter(ty) => resolved_type_owned_capacity(ty),
                Frame::Finish {
                    identity,
                    declaration,
                    ..
                } => identity.capacity() + declaration.as_str().len(),
            }
        }

        #[cfg(test)]
        fn retained_capacity(
            frames: &Vec<Frame>,
            results: &Vec<TypeFacts>,
            memo: &BTreeMap<String, TypeFacts>,
            visiting: &BTreeSet<DeclarationId>,
        ) -> usize {
            type_facts_outer_baseline()
                + frames.capacity() * std::mem::size_of::<Frame>()
                + frames.iter().map(frame_owned_capacity).sum::<usize>()
                + results.capacity() * std::mem::size_of::<TypeFacts>()
                + results
                    .iter()
                    .map(|facts| facts.layout_key.capacity())
                    .sum::<usize>()
                + memo.len()
                    * (std::mem::size_of::<(String, TypeFacts)>()
                        + std::mem::size_of::<BTreeMap<String, TypeFacts>>())
                + memo
                    .iter()
                    .map(|(key, facts)| key.capacity() + facts.layout_key.capacity())
                    .sum::<usize>()
                + visiting.len()
                    * (std::mem::size_of::<DeclarationId>()
                        + std::mem::size_of::<BTreeSet<DeclarationId>>())
                + visiting.iter().map(|id| id.as_str().len()).sum::<usize>()
        }

        let mut frames = vec![Frame::Enter(ty.clone())];
        let mut results = Vec::<TypeFacts>::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                2,
                retained_capacity(&frames, &results, memo, visiting) + frame_owned_capacity(&frame),
            );
            match frame {
                Frame::Enter(ty) => {
                    let identity = ty.identity_key();
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + resolved_type_owned_capacity(&ty)
                            + identity.capacity(),
                    );
                    if let Some(facts) = memo.get(&identity) {
                        results.push(facts.clone());
                        continue;
                    }
                    let scalar = match &ty {
                        ResolvedType::Unit => {
                            Some((true, false, false, "native-rust-import-result:unit"))
                        }
                        ResolvedType::I64 => Some((true, false, false, "scalar:i64")),
                        ResolvedType::Char => Some((true, false, false, "scalar:char")),
                        ResolvedType::F32 => Some((true, false, false, "scalar:f32")),
                        ResolvedType::F64 => Some((true, false, false, "scalar:f64")),
                        ResolvedType::Bool => Some((true, false, false, "scalar:bool")),
                        ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => None,
                    };
                    if let Some((copy, contains_resource, needs_drop, key)) = scalar {
                        results.push(TypeFacts {
                            copy,
                            contains_resource,
                            sized: true,
                            needs_drop,
                            layout_key: key.to_owned(),
                        });
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return None;
                    };
                    let item = self.declaration(&declaration)?;
                    if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                        let facts = TypeFacts {
                            copy: false,
                            contains_resource: true,
                            sized: true,
                            needs_drop: true,
                            layout_key: format!("resource:{identity}"),
                        };
                        memo.insert(identity, facts.clone());
                        results.push(facts);
                        continue;
                    }
                    if !matches!(
                        item.kind,
                        DeclarationKind::Record | DeclarationKind::Variant
                    ) {
                        return None;
                    }
                    let parameters = self.type_parameters.get(&declaration)?;
                    if arguments.len() != parameters.len()
                        || arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        })
                        || !visiting.insert(declaration.clone())
                    {
                        return None;
                    }
                    let children = match item.kind {
                        DeclarationKind::Record => self
                            .record_fields
                            .get(&declaration)?
                            .iter()
                            .map(|field| substitute_type(&field.ty, &declaration, &arguments).ok())
                            .collect::<Option<Vec<_>>>()?,
                        DeclarationKind::Variant => self
                            .variant_cases
                            .get(&declaration)?
                            .iter()
                            .flat_map(|case| &case.fields)
                            .map(|field| substitute_type(&field.ty, &declaration, &arguments).ok())
                            .collect::<Option<Vec<_>>>()?,
                        _ => unreachable!(),
                    };
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + identity.capacity()
                            + declaration.as_str().len()
                            + children.capacity() * std::mem::size_of::<ResolvedType>()
                            + children
                                .iter()
                                .map(resolved_type_owned_capacity)
                                .sum::<usize>(),
                    );
                    frames.try_reserve(children.len() + 1).ok()?;
                    frames.push(Frame::Finish {
                        identity,
                        declaration,
                        kind: item.kind,
                        child_count: children.len(),
                    });
                    frames.extend(children.into_iter().rev().map(Frame::Enter));
                }
                Frame::Finish {
                    identity,
                    declaration,
                    kind,
                    child_count,
                } => {
                    #[cfg(test)]
                    let finish_identity_bytes = identity.capacity() + declaration.as_str().len();
                    let split = results.len().checked_sub(child_count)?;
                    let child_facts = results.drain(split..).collect::<Vec<_>>();
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + finish_identity_bytes
                            + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                            + child_facts
                                .iter()
                                .map(|facts| facts.layout_key.capacity())
                                .sum::<usize>(),
                    );
                    visiting.remove(&declaration);
                    let mut encoded = crate::bounded_output::CappedString::new();
                    match kind {
                        DeclarationKind::Record => {
                            let fields = self.record_fields.get(&declaration)?;
                            let mut copy = true;
                            let mut contains_resource = false;
                            let mut sized = true;
                            let mut needs_drop = false;
                            for (field, facts) in fields.iter().zip(&child_facts) {
                                copy &= facts.copy;
                                contains_resource |= facts.contains_resource;
                                sized &= facts.sized;
                                needs_drop |= facts.needs_drop;
                                write!(
                                    encoded,
                                    "{}:{}:{}:{}",
                                    field.id.as_str().len(),
                                    field.id,
                                    facts.layout_key.len(),
                                    facts.layout_key
                                )
                                .ok()?;
                            }
                            #[cfg(test)]
                            let encoded_capacity = encoded.allocated_capacity();
                            let facts = TypeFacts {
                                copy,
                                contains_resource,
                                sized,
                                needs_drop,
                                layout_key: format!(
                                    "record:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    fields.len(),
                                    encoded.into_string()
                                ),
                            };
                            #[cfg(test)]
                            note_iterative_phase_capacity(
                                2,
                                retained_capacity(&frames, &results, memo, visiting)
                                    + finish_identity_bytes
                                    + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                                    + child_facts
                                        .iter()
                                        .map(|facts| facts.layout_key.capacity())
                                        .sum::<usize>()
                                    + encoded_capacity
                                    + facts.layout_key.capacity(),
                            );
                            memo.insert(identity, facts.clone());
                            results.push(facts);
                        }
                        DeclarationKind::Variant => {
                            let cases = self.variant_cases.get(&declaration)?;
                            let mut facts_iter = child_facts.iter();
                            for case in cases {
                                write!(
                                    encoded,
                                    "{}:{}:{}:",
                                    case.id.as_str().len(),
                                    case.id,
                                    case.fields.len()
                                )
                                .ok()?;
                                for field in &case.fields {
                                    let facts = facts_iter.next()?;
                                    if !facts.copy || facts.contains_resource || facts.needs_drop {
                                        return None;
                                    }
                                    write!(
                                        encoded,
                                        "{}:{}:{}:{}",
                                        field.id.as_str().len(),
                                        field.id,
                                        facts.layout_key.len(),
                                        facts.layout_key
                                    )
                                    .ok()?;
                                }
                            }
                            #[cfg(test)]
                            let encoded_capacity = encoded.allocated_capacity();
                            let facts = TypeFacts {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                                layout_key: format!(
                                    "variant:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    cases.len(),
                                    encoded.into_string()
                                ),
                            };
                            #[cfg(test)]
                            note_iterative_phase_capacity(
                                2,
                                retained_capacity(&frames, &results, memo, visiting)
                                    + finish_identity_bytes
                                    + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                                    + child_facts
                                        .iter()
                                        .map(|facts| facts.layout_key.capacity())
                                        .sum::<usize>()
                                    + encoded_capacity
                                    + facts.layout_key.capacity(),
                            );
                            memo.insert(identity, facts.clone());
                            results.push(facts);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        (results.len() == 1).then(|| results.pop().expect("type fact count checked above"))
    }

    fn populate_type_facts(&mut self) -> bool {
        let mut memo = BTreeMap::new();
        for ty in [ResolvedType::I64, ResolvedType::Bool] {
            let Some(facts) = self.compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo) else {
                return false;
            };
            memo.insert(ty.identity_key(), facts);
        }
        let declarations = self.types_by_name.values().cloned().collect::<Vec<_>>();
        #[cfg(test)]
        let declarations_capacity = declarations.capacity() * std::mem::size_of::<DeclarationId>()
            + declarations
                .iter()
                .map(|id| id.as_str().len())
                .sum::<usize>();
        #[cfg(test)]
        TYPE_FACTS_OUTER_BASELINE.with(|baseline| baseline.set(declarations_capacity));
        for declaration in declarations {
            #[cfg(test)]
            note_iterative_phase_capacity(
                2,
                declarations_capacity.saturating_add(declaration.as_str().len()),
            );
            if self
                .type_parameters
                .get(&declaration)
                .is_some_and(|parameters| !parameters.is_empty())
            {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if self
                .compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo)
                .is_none()
            {
                return false;
            }
        }
        #[cfg(test)]
        TYPE_FACTS_OUTER_BASELINE.with(|baseline| baseline.set(0));
        self.type_facts_by_id = memo;
        true
    }

    fn recompute_type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.compute_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
    }

    fn from_verified(program: &Program) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            index.insert_top_level(
                declaration.name.clone(),
                DeclarationId::new(declaration.stable_id.clone()),
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let parameters = declaration
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("type `{}` has too many parameters", declaration.name),
                                declaration.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        for interface in &program.interfaces {
            let interface_id = DeclarationId::new(interface.stable_id.clone());
            index.insert_top_level(
                interface.name.clone(),
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id = DeclarationId::new(import.stable_id.clone());
                index
                    .imports_by_key
                    .insert(import.stable_id.clone(), import_id.clone());
                if import.native_rust {
                    index
                        .native_rust_imports_by_name
                        .insert(import.name.clone(), import_id.clone());
                }
                index.insert_owned_declaration(
                    interface_id.clone(),
                    import.name.clone(),
                    import_id,
                    DeclarationKind::Import,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
                continue;
            };
            if let [lifecycle] = lifecycles.as_slice() {
                let lifecycle_id = lifecycle
                    .stable_id
                    .as_ref()
                    .expect("verified lifecycle has an explicit identity");
                index.insert_owned_declaration(
                    DeclarationId::new(declaration.stable_id.clone()),
                    "drop".to_owned(),
                    DeclarationId::new(lifecycle_id.clone()),
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in &program.functions {
            let owner = DeclarationId::new(function.stable_id.clone());
            index.insert_top_level(
                function.name.clone(),
                owner.clone(),
                DeclarationKind::Function,
                if function.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let parameters = function
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("function `{}` has too many parameters", function.name),
                                function.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        if program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.native_rust)
            .any(|import| index.functions_by_name.contains_key(&import.name))
        {
            return Err(Diagnostic::error(
                "SPX-B107",
                "Native Rust Interop declaration set is unsupported: symbol collision",
                Span::default(),
            )
            .at_path(&program.path));
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(&program.path)
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("record `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index.record_fields.insert(owner, resolved_fields);
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_cases = Vec::with_capacity(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = DeclarationId::new(case.stable_id.clone());
                let case_index = u32::try_from(case_ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("variant `{}` has too many cases", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                index.insert_case(
                    owner.clone(),
                    case.name.clone(),
                    case_id.clone(),
                    if crate::prelude::is_compiler_owned_id(&case.stable_id) {
                        IdentityOrigin::CompilerOwned
                    } else if case.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                let mut resolved_fields = Vec::with_capacity(case.fields.len());
                for (field_ordinal, field) in case.fields.iter().enumerate() {
                    let ty = index
                        .resolve_source_type(&field.ty, Some(&owner))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "SPX-H001",
                                format!("unresolved case field type `{}`", field.ty),
                                field.span,
                            )
                            .at_path(&program.path)
                        })?;
                    let field_index = u32::try_from(field_ordinal).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!(
                                "variant case `{}::{}` has too many fields",
                                declaration.name, case.name
                            ),
                            case.span,
                        )
                        .at_path(&program.path)
                    })?;
                    let resolved = ResolvedFieldDeclaration {
                        id: DeclarationId::new(field.stable_id.clone()),
                        name: field.name.clone(),
                        index: field_index,
                        ty,
                        span: field.span,
                    };
                    index.insert_case_field(
                        case_id.clone(),
                        resolved.clone(),
                        if crate::prelude::is_compiler_owned_id(&field.stable_id) {
                            IdentityOrigin::CompilerOwned
                        } else if field.explicit_id {
                            IdentityOrigin::Explicit
                        } else {
                            IdentityOrigin::Automatic
                        },
                    );
                    resolved_fields.push(resolved);
                }
                index
                    .case_fields
                    .insert(case_id.clone(), resolved_fields.clone());
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: case.name.clone(),
                    index: case_index,
                    fields: resolved_fields,
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases);
        }
        if !index.populate_type_facts() {
            return Err(Diagnostic::error(
                "SPX-T217",
                "record declarations contain an illegal by-value recursive layout",
                Span::default(),
            )
            .at_path(&program.path));
        }
        Ok(index)
    }

    fn insert_top_level(
        &mut self,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        match kind {
            DeclarationKind::Resource | DeclarationKind::Record | DeclarationKind::Variant => {
                self.types_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Function => {
                self.functions_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Interface => {}
            DeclarationKind::ResourceDrop
            | DeclarationKind::Import
            | DeclarationKind::Field
            | DeclarationKind::VariantCase
            | DeclarationKind::CaseField => {
                unreachable!("owned declarations use owner-scoped insertion")
            }
        }
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: None,
            },
        );
    }

    fn insert_owned_declaration(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::Field,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        identity_origin: IdentityOrigin,
    ) {
        self.cases_by_owner_name
            .insert((owner.clone(), name.clone()), id.clone());
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind: DeclarationKind::VariantCase,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::CaseField,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn resolve_source_type(
        &self,
        ty: &Type,
        parameter_owner: Option<&DeclarationId>,
    ) -> Option<ResolvedType> {
        enum Frame<'a> {
            Enter(&'a Type),
            Finish(DeclarationId, usize),
        }
        let mut frames = vec![Frame::Enter(ty)];
        let mut resolved = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Type::I64 => resolved.push(ResolvedType::I64),
                    Type::Char => resolved.push(ResolvedType::Char),
                    Type::F32 => resolved.push(ResolvedType::F32),
                    Type::F64 => resolved.push(ResolvedType::F64),
                    Type::Bool => resolved.push(ResolvedType::Bool),
                    Type::Named { name, arguments } => {
                        if arguments.is_empty() {
                            if let Some(owner) = parameter_owner {
                                if let Some(parameter) = self
                                    .type_parameters(owner)?
                                    .iter()
                                    .find(|parameter| parameter.name == *name)
                                {
                                    resolved.push(ResolvedType::TypeParameter {
                                        owner: owner.clone(),
                                        index: parameter.index,
                                    });
                                    continue;
                                }
                            }
                        }
                        frames.push(Frame::Finish(self.type_id(name)?.clone(), arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(declaration, count) => {
                    let split = resolved.len().checked_sub(count)?;
                    let arguments = resolved.drain(split..).collect();
                    resolved.push(ResolvedType::Nominal {
                        declaration,
                        arguments,
                    });
                }
            }
        }
        (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedType {
    Unit,
    I64,
    /// One Unicode scalar value.
    Char,
    /// IEEE-754 single precision.
    F32,
    /// IEEE-754 double precision.
    F64,
    Bool,
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
    pub fn nominal_id(&self) -> Option<&DeclarationId> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::Unit
            | Self::I64
            | Self::Char
            | Self::F32
            | Self::F64
            | Self::Bool
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
                    Self::Char => keys.push("char".to_owned()),
                    Self::F32 => keys.push("f32".to_owned()),
                    Self::F64 => keys.push("f64".to_owned()),
                    Self::Bool => keys.push("bool".to_owned()),
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
                ResolvedType::Char => resolved.push(ResolvedType::Char),
                ResolvedType::F32 => resolved.push(ResolvedType::F32),
                ResolvedType::F64 => resolved.push(ResolvedType::F64),
                ResolvedType::Bool => resolved.push(ResolvedType::Bool),
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
                Type::Char => resolved.push(Type::Char),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
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
        ResolvedExprKind::Char(value) => ResolvedExprKind::Char(*value),
        ResolvedExprKind::Float32(bits) => ResolvedExprKind::Float32(*bits),
        ResolvedExprKind::Float64(bits) => ResolvedExprKind::Float64(*bits),
        ResolvedExprKind::Bool(value) => ResolvedExprKind::Bool(*value),
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
                            value,
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
        | ResolvedExprKind::Project { .. } => {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNativeRustImportCall {
    pub expression: ExpressionId,
    pub import: DeclarationId,
    pub args: Vec<ResolvedExpr>,
    pub result: ResolvedImportResultKind,
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
    /// A `char` literal held as its exact Unicode scalar value.
    Char(u32),
    /// An `f32` literal held as its exact IEEE-754 bit pattern.
    Float32(u32),
    /// An `f64` literal held as its exact IEEE-754 bit pattern.
    Float64(u64),
    Bool(bool),
    Place(Place),
    Call {
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        args: Vec<ResolvedExpr>,
    },
    NativeRustImportCall(ResolvedNativeRustImportCall),
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchArm {
    pub pattern: ResolvedMatchPattern,
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
        value: ResolvedExpr,
        span: Span,
    },
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

/// Assemble one backend-ready scalar program from real resolved workspace
/// functions. This is intentionally narrower than general cross-file linking:
/// callers must have already resolved the complete provider closure, and only
/// value `i64`/`bool` functions without effects are admitted.
///
pub(crate) fn link_scalar_workspace(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace scalar closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));

    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        let function = &linked.function;
        if !seen.insert(function.id.clone()) {
            return Err(link_error(format!(
                "workspace scalar closure duplicates function `{}`",
                function.id
            )));
        }
        if !function.effects.is_empty()
            || function
                .params
                .iter()
                .any(|parameter| parameter.ownership != OwnershipMode::Value)
            || !scalar_type(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| !scalar_type(&parameter.ty))
        {
            return Err(link_error(format!(
                "workspace function `{}` is outside the pure scalar linker profile",
                function.id
            )));
        }
        if function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if function.name != "main" {
                return Err(link_error(
                    "workspace scalar entry point is not an authored `main` function",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace scalar entry point must have an explicit authored identity",
        ));
    }

    let mut declarations = DeclarationIndex::default();
    for linked in &linked_functions {
        declarations.insert_top_level(
            linked.function.name.clone(),
            linked.function.id.clone(),
            DeclarationKind::Function,
            linked.origin,
        );
        declarations
            .type_parameters
            .insert(linked.function.id.clone(), Vec::new());
    }
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace scalar linker could not construct scalar type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: Vec::new(),
        entrypoint,
        declarations,
        types: Vec::new(),
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions: linked_functions
            .drain(..)
            .map(|linked| linked.function)
            .collect(),
        function_instances: Vec::new(),
    };
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

fn scalar_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}

fn link_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

fn rebuild_cleanup_metadata(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let inventories = program
        .functions
        .iter()
        .map(|function| crate::cleanup::build_inventory(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, inventory) in program.functions.iter_mut().zip(inventories) {
        function.cleanup = inventory;
    }
    let cleanup_plans = program
        .functions
        .iter()
        .map(|function| crate::cleanup_plan::build_plan(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, cleanup_plan) in program.functions.iter_mut().zip(cleanup_plans) {
        function.cleanup_plan = cleanup_plan;
    }
    Ok(())
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

/// Validate an identity-resolved program before a semantic consumer uses it.
///
/// Resolved HIR is intentionally public for agent and compiler integrations,
/// so callers can inspect or transform HIR produced by [`resolve`]. Every
/// backend calls this function and therefore fails closed when a transformation
/// breaks identities, lexical scope, or current type rules. A versioned wire
/// schema for constructing HIR outside the compiler is future work.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_core(program)?;
    validate_attached_identity_references(program)?;
    crate::cleanup::validate_program(program)?;
    crate::cleanup_plan::validate_program(program)?;
    Ok(())
}

fn validate_nul_free_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved entry point", program.entrypoint.as_str())?;

    for (key, declaration) in &program.declarations.declarations {
        reject_nul_identity("declaration index key", key.as_str())?;
        reject_nul_identity(
            declaration_identity_subject(declaration.kind),
            declaration.id.as_str(),
        )?;
        if let Some(owner) = &declaration.owner {
            reject_nul_identity("resolved declaration owner", owner.as_str())?;
        }
    }
    for id in program.declarations.types_by_name.values() {
        reject_nul_identity("resolved type lookup", id.as_str())?;
    }
    for id in program.declarations.functions_by_name.values() {
        reject_nul_identity("resolved function lookup", id.as_str())?;
    }
    for ((owner, _), field) in &program.declarations.fields_by_owner_name {
        reject_nul_identity("resolved field owner lookup", owner.as_str())?;
        reject_nul_identity("resolved field lookup", field.as_str())?;
    }
    for ((owner, _), case) in &program.declarations.cases_by_owner_name {
        reject_nul_identity("resolved variant owner lookup", owner.as_str())?;
        reject_nul_identity("resolved variant case lookup", case.as_str())?;
    }
    for (owner, fields) in &program.declarations.record_fields {
        reject_nul_identity("resolved record-field owner", owner.as_str())?;
        for field in fields {
            reject_nul_identity("resolved field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (owner, cases) in &program.declarations.variant_cases {
        reject_nul_identity("resolved variant-case owner", owner.as_str())?;
        for case in cases {
            reject_nul_identity("resolved variant case", case.id.as_str())?;
            for field in &case.fields {
                reject_nul_identity("resolved case field", field.id.as_str())?;
                audit_resolved_type(&field.ty)?;
            }
        }
    }
    for (case, fields) in &program.declarations.case_fields {
        reject_nul_identity("resolved case-field owner", case.as_str())?;
        for field in fields {
            reject_nul_identity("resolved case field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (key, import) in &program.declarations.imports_by_key {
        reject_nul_identity("resolved logical import key", key)?;
        reject_nul_identity("resolved import lookup", import.as_str())?;
    }

    for declaration in &program.types {
        let subject = match declaration.kind {
            ResolvedTypeDeclarationKind::Resource { .. } => "resolved resource",
            ResolvedTypeDeclarationKind::Record { .. } => "resolved record",
            ResolvedTypeDeclarationKind::Variant { .. } => "resolved variant",
        };
        reject_nul_identity(subject, declaration.id.as_str())?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                reject_nul_identity("resolved resource lifecycle", drop.id.as_str())?;
                if let ResolvedResourceDropKind::Imported { import, import_key } = &drop.kind {
                    reject_nul_identity("resolved lifecycle import", import.as_str())?;
                    reject_nul_identity("resolved lifecycle logical import key", import_key)?;
                }
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                for field in fields {
                    reject_nul_identity("resolved field", field.id.as_str())?;
                    audit_resolved_type(&field.ty)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    reject_nul_identity("resolved variant case", case.id.as_str())?;
                    for field in &case.fields {
                        reject_nul_identity("resolved case field", field.id.as_str())?;
                        audit_resolved_type(&field.ty)?;
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        reject_nul_identity("resolved interface", interface.id.as_str())?;
        for import in &interface.imports {
            reject_nul_identity("resolved import", import.id.as_str())?;
            reject_nul_identity("resolved import owner", import.interface.as_str())?;
            reject_nul_identity("resolved logical import key", &import.import_key)?;
            for parameter in &import.parameters {
                audit_resolved_type(&parameter.ty)?;
            }
        }
    }
    for function in &program.functions {
        reject_nul_identity("resolved function", function.id.as_str())?;
        for parameter in &function.params {
            reject_nul_identity("resolved value", parameter.id.as_str())?;
            audit_resolved_type(&parameter.ty)?;
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        audit_resolved_type(&function.return_type)?;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            audit_resolved_expression(expression)?;
        }
    }
    Ok(())
}

/// Reject target-neutral attached metadata containing identities that cannot
/// cross C-string-backed backend and trace boundaries losslessly.
///
/// This is intentionally narrower than semantic inventory/plan validation so
/// independent replayers can call it without trusting either canonical builder.
pub(crate) fn validate_attached_identity_references(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    for function in &program.functions {
        audit_cleanup_inventory(&function.cleanup)?;
        audit_cleanup_plan(&function.cleanup_plan)?;
    }
    Ok(())
}

fn audit_resolved_type(root: &ResolvedType) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::Char
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool => {}
            ResolvedType::TypeParameter { owner, .. } => {
                reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                reject_nul_identity("resolved nominal type", declaration.as_str())?;
                pending.extend(arguments);
            }
        }
    }
    Ok(())
}

fn audit_resolved_record_match_pattern(
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved record match", record.as_str())?;
    audit_resolved_type(instance)?;
    for field in fields {
        reject_nul_identity("resolved record match field", field.field.as_str())?;
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                reject_nul_identity("resolved record match binding", binding.id.as_str())?;
                audit_resolved_type(&binding.ty)?;
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => audit_resolved_record_match_pattern(record, instance, fields)?,
        }
    }
    Ok(())
}

fn audit_resolved_expression(root: &ResolvedExpr) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        audit_resolved_type(&expression.ty)?;
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place) => audit_hir_place(place)?,
            ResolvedExprKind::Call { callee, args, .. } => {
                reject_nul_identity("resolved call target", callee.as_str())?;
                pending.extend(args);
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                reject_nul_identity("resolved native Rust import target", call.import.as_str())?;
                if call.expression != expression.id {
                    return Err(hir_error(
                        "resolved native Rust import call identity is inconsistent",
                    ));
                }
                pending.extend(&call.args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    reject_nul_identity("resolved value", binding.id.as_str())?;
                    audit_resolved_type(&binding.ty)?;
                    pending.push(value);
                }
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
            ResolvedExprKind::ConstructRecord { record, fields } => {
                reject_nul_identity("resolved record constructor", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record initializer field", field.field.as_str())?;
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                reject_nul_identity("resolved variant constructor", variant.as_str())?;
                reject_nul_identity("resolved variant case", case.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved case initializer field", field.field.as_str())?;
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                for arm in arms.iter().rev() {
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            reject_nul_identity("resolved match variant", variant.as_str())?;
                            reject_nul_identity("resolved match case", case.as_str())?;
                            for field in fields {
                                reject_nul_identity("resolved match field", field.field.as_str())?;
                                reject_nul_identity(
                                    "resolved match binding",
                                    field.binding.id.as_str(),
                                )?;
                                audit_resolved_type(&field.binding.ty)?;
                            }
                        }
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => audit_resolved_record_match_pattern(record, instance, fields)?,
                    }
                    pending.push(&arm.value);
                }
                pending.push(scrutinee);
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
                reject_nul_identity("resolved `?` Result", result.as_str())?;
                reject_nul_identity("resolved `?` Ok case", ok_case.as_str())?;
                reject_nul_identity("resolved `?` Ok field", ok_field.as_str())?;
                reject_nul_identity("resolved `?` Err case", err_case.as_str())?;
                reject_nul_identity("resolved `?` Err field", err_field.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand);
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                reject_nul_identity("resolved Option `?` Option", option.as_str())?;
                reject_nul_identity("resolved Option `?` Some case", some_case.as_str())?;
                reject_nul_identity("resolved Option `?` Some field", some_field.as_str())?;
                reject_nul_identity("resolved Option `?` None case", none_case.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand);
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                reject_nul_identity("resolved record update", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record replacement field", field.field.as_str())?;
                    pending.push(&field.value);
                }
                pending.push(base);
            }
            ResolvedExprKind::Project { base, field } => {
                reject_nul_identity("resolved projected field", field.as_str())?;
                pending.push(base);
            }
        }
    }
    Ok(())
}

fn audit_hir_place(place: &Place) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved place root", place.root.as_str())?;
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => {
                reject_nul_identity("resolved place field", field.as_str())?;
            }
            PlaceProjection::VariantField { case, field } => {
                reject_nul_identity("resolved place variant case", case.as_str())?;
                reject_nul_identity("resolved place variant field", field.as_str())?;
            }
        }
    }
    Ok(())
}

fn audit_field_liveness_shape(root: &crate::cleanup::FieldLivenessShape) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(shape) = pending.pop() {
        match shape {
            crate::cleanup::FieldLivenessShape::NoDrop => {}
            crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
                reject_nul_identity("cleanup lifecycle", lifecycle.as_str())?;
            }
            crate::cleanup::FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                reject_nul_identity("cleanup record", declaration.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("cleanup field", field.field.as_str())?;
                    pending.push(&field.shape);
                }
            }
        }
    }
    Ok(())
}

fn audit_inventory_place(place: &crate::cleanup::CleanupPlace) -> Result<(), Diagnostic> {
    for projection in &place.projections {
        reject_nul_identity("cleanup inventory projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_cleanup_inventory(inventory: &CleanupInventory) -> Result<(), Diagnostic> {
    for slot in &inventory.slots {
        match &slot.origin {
            crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
            | crate::cleanup::CleanupStorageOrigin::Binding { value }
            | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                reject_nul_identity("cleanup inventory value", value.as_str())?;
            }
            crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                reject_nul_identity("cleanup inventory expression", expression.as_str())?;
            }
        }
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.shape)?;
    }
    for flag in &inventory.flags {
        audit_inventory_place(&flag.place)?;
        reject_nul_identity("cleanup inventory lifecycle", flag.lifecycle.as_str())?;
    }
    Ok(())
}

fn audit_plan_storage(storage: &crate::cleanup_plan::StorageId) -> Result<(), Diagnostic> {
    match storage {
        crate::cleanup_plan::StorageId::Value(value) => {
            reject_nul_identity("cleanup-plan value storage", value.as_str())?;
        }
        crate::cleanup_plan::StorageId::Temporary(expression) => {
            reject_nul_identity("cleanup-plan temporary storage", expression.as_str())?;
        }
        crate::cleanup_plan::StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => {
            reject_nul_identity("cleanup-plan call-argument call", call.as_str())?;
            reject_nul_identity(
                "cleanup-plan call-argument value",
                value_expression.as_str(),
            )?;
        }
        crate::cleanup_plan::StorageId::ProvisionalResult => {}
    }
    Ok(())
}

fn audit_plan_place(place: &crate::cleanup_plan::CleanupPlace) -> Result<(), Diagnostic> {
    audit_plan_storage(&place.storage)?;
    for projection in &place.projections {
        reject_nul_identity("cleanup-plan projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_status_source(source: &crate::cleanup_plan::StatusSourceId) -> Result<(), Diagnostic> {
    reject_nul_identity("cleanup-plan status expression", source.expression.as_str())
}

fn audit_result_source(
    source: &crate::cleanup_plan::CleanupResultSource,
) -> Result<(), Diagnostic> {
    match source {
        crate::cleanup_plan::CleanupResultSource::Scalar { expression } => {
            reject_nul_identity("cleanup-plan scalar result", expression.as_str())?;
        }
        crate::cleanup_plan::CleanupResultSource::Owned { storage } => {
            audit_plan_place(storage)?;
        }
    }
    Ok(())
}

fn audit_cleanup_plan(plan: &CleanupPlan) -> Result<(), Diagnostic> {
    for place in &plan.entry_state.live_owned_parameters {
        audit_plan_place(place)?;
    }
    for slot in &plan.slots {
        audit_plan_storage(&slot.storage)?;
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.field_liveness_shape)?;
    }
    for source in &plan.status_sources {
        audit_status_source(&source.id)?;
        if let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &source.producer {
            reject_nul_identity("cleanup-plan propagated callee", callee.as_str())?;
        }
    }
    for block in &plan.blocks {
        for transition in &block.transitions {
            match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { at, destination } => {
                    reject_nul_identity("cleanup-plan initialize expression", at.as_str())?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    reject_nul_identity("cleanup-plan transfer expression", at.as_str())?;
                    audit_plan_place(source)?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments } => {
                    reject_nul_identity("cleanup-plan committed call", call.as_str())?;
                    for argument in arguments {
                        audit_plan_place(&argument.source)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::SelectFailure { source } => {
                    audit_status_source(source)?;
                }
                crate::cleanup_plan::CleanupTransition::StageCopyResult { source } => {
                    match source {
                        crate::cleanup_plan::StagedCopyResultSource::Body {
                            expression,
                            instance,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged body expression",
                                expression.as_str(),
                            )?;
                            audit_resolved_type(instance)?;
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            result,
                            ok_case,
                            ok_field,
                            err_case,
                            err_field,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("Result", result),
                                ("Ok case", ok_case),
                                ("Ok field", ok_field),
                                ("Err case", err_case),
                                ("Err field", err_field),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged `?` {kind}"),
                                    declaration.as_str(),
                                )?;
                            }
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            option,
                            some_case,
                            some_field,
                            none_case,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("Option", option),
                                ("Some case", some_case),
                                ("Some field", some_field),
                                ("None case", none_case),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged Option `?` {kind}"),
                                    declaration.as_str(),
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            crate::cleanup_plan::EdgeCondition::Always => {}
            crate::cleanup_plan::EdgeCondition::BooleanResult(expression, _) => {
                reject_nul_identity("cleanup-plan boolean expression", expression.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::VariantCase {
                scrutinee, case, ..
            } => {
                reject_nul_identity("cleanup-plan match scrutinee", scrutinee.as_str())?;
                reject_nul_identity("cleanup-plan variant case", case.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::StatusZero(source)
            | crate::cleanup_plan::EdgeCondition::StatusNonzero(source) => {
                audit_status_source(source)?;
            }
        }
    }
    for region in &plan.regions {
        for storage in &region.slots {
            audit_plan_storage(storage)?;
        }
    }
    for exit in &plan.exits {
        for finalizer in &exit.finalize_in_order {
            audit_plan_place(&finalizer.source)?;
            reject_nul_identity(
                "cleanup-plan finalizer lifecycle",
                finalizer.lifecycle_id.as_str(),
            )?;
        }
        match &exit.continuation {
            crate::cleanup_plan::ExitContinuation::Continue(_)
            | crate::cleanup_plan::ExitContinuation::ReturnUnit => {}
            crate::cleanup_plan::ExitContinuation::CommitResult { source } => {
                audit_result_source(source)?;
            }
            crate::cleanup_plan::ExitContinuation::ReturnFailure { source } => {
                audit_status_source(source)?;
            }
        }
    }
    Ok(())
}

fn declaration_identity_subject(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Resource => "resolved resource declaration",
        DeclarationKind::ResourceDrop => "resolved resource lifecycle declaration",
        DeclarationKind::Record => "resolved record declaration",
        DeclarationKind::Field => "resolved field declaration",
        DeclarationKind::Variant => "resolved variant declaration",
        DeclarationKind::VariantCase => "resolved variant case declaration",
        DeclarationKind::CaseField => "resolved case field declaration",
        DeclarationKind::Interface => "resolved interface declaration",
        DeclarationKind::Import => "resolved import declaration",
        DeclarationKind::Function => "resolved function declaration",
    }
}

fn reject_nul_identity(subject: &str, value: &str) -> Result<(), Diagnostic> {
    if value.contains('\0') {
        Err(hir_error(format!("{subject} identity contains NUL")))
    } else {
        Ok(())
    }
}

fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

fn resolved_lifecycle_effects(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<BTreeSet<String>, Diagnostic> {
    fn collect(
        program: &ResolvedProgram,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        effects: &mut BTreeSet<String>,
    ) -> Result<(), Diagnostic> {
        let Some(id) = ty.nominal_id() else {
            return Ok(());
        };
        if !visiting.insert(id.clone()) {
            return Ok(());
        }
        let declaration = program
            .types
            .iter()
            .find(|item| item.id == *id)
            .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    let resolved = program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|item| item.id == *import)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resource `{id}` references missing import `{import}`"
                            ))
                        })?;
                    effects.extend(resolved.effects.iter().cloned());
                }
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                for field in fields {
                    collect(program, &field.ty, visiting, effects)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        collect(program, &field.ty, visiting, effects)?;
                    }
                }
            }
        }
        visiting.remove(id);
        Ok(())
    }

    let mut effects = BTreeSet::new();
    collect(program, ty, &mut BTreeSet::new(), &mut effects)?;
    Ok(effects)
}

fn visit_resolved_calls(
    expression: &ResolvedExpr,
    visit: &mut impl FnMut(&DeclarationId, Option<&FunctionInstanceId>, &[ResolvedType]),
) {
    match &expression.kind {
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            visit(callee, instance.as_ref(), type_arguments);
            for arg in args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. } => visit_resolved_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_resolved_calls(left, visit);
            visit_resolved_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { value, .. } => visit_resolved_calls(value, visit),
                }
            }
            visit_resolved_calls(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_resolved_calls(condition, visit);
            visit_resolved_calls(then_branch, visit);
            visit_resolved_calls(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            visit_resolved_calls(scrutinee, visit);
            for arm in arms {
                visit_resolved_calls(&arm.value, visit);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_resolved_calls(base, visit);
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_) => {}
    }
}

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_call_edges(
    program: &ResolvedProgram,
) -> BTreeSet<(DeclarationId, DeclarationId)> {
    let mut edges = BTreeSet::new();
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, _, _| {
                edges.insert((function.id.clone(), callee.clone()));
            });
        }
    }
    edges
}

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_expression_identity(owner: &DeclarationId, path: &str) -> String {
    ExpressionId::new(&FunctionExecutionId::Monomorphic(owner.clone()), path)
        .as_str()
        .to_owned()
}

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_call_sites(
    program: &ResolvedProgram,
) -> Vec<(DeclarationId, String, DeclarationId)> {
    fn walk(
        owner: &DeclarationId,
        expression: &ResolvedExpr,
        sites: &mut Vec<(DeclarationId, String, DeclarationId)>,
    ) {
        match &expression.kind {
            ResolvedExprKind::Call { callee, args, .. } => {
                sites.push((
                    owner.clone(),
                    expression.id.as_str().to_owned(),
                    callee.clone(),
                ));
                for argument in args {
                    walk(owner, argument, sites);
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                for argument in &call.args {
                    walk(owner, argument, sites);
                }
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. } => walk(owner, value, sites),
            ResolvedExprKind::Binary { left, right, .. } => {
                walk(owner, left, sites);
                walk(owner, right, sites);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { value, .. } => walk(owner, value, sites),
                    }
                }
                walk(owner, tail, sites);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                walk(owner, condition, sites);
                walk(owner, then_branch, sites);
                walk(owner, else_branch, sites);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    walk(owner, &field.value, sites);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                walk(owner, scrutinee, sites);
                for arm in arms {
                    walk(owner, &arm.value, sites);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                walk(owner, base, sites);
                for field in fields {
                    walk(owner, &field.value, sites);
                }
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::Place(_) => {}
        }
    }

    let mut sites = Vec::new();
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            walk(&function.id, expression, &mut sites);
        }
    }
    for template in &program.function_templates {
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            walk(&template.id, expression, &mut sites);
        }
    }
    sites
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
        let functions = self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
            .map(|function| self.resolve_function(function))
            .collect::<Result<_, _>>()?;
        let function_templates = self
            .program
            .functions
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| self.resolve_function_template(function))
            .collect::<Result<_, _>>()?;
        let function_instances = self.discover_function_instances()?;
        let mut resolved = ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            entrypoint,
            declarations: self.declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        };
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
            if !matches!(&declaration.kind, TypeDeclarationKind::Record { .. }) {
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
                let ownership = param.mode.into();
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
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
                Frame::Enter(Type::Char) => result = Some(ResolvedType::Char),
                Frame::Enter(Type::F32) => result = Some(ResolvedType::F32),
                Frame::Enter(Type::F64) => result = Some(ResolvedType::F64),
                Frame::Enter(Type::Bool) => result = Some(ResolvedType::Bool),
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
                            || (!resolved.is_empty()
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
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &format!("{field_path}.binding")),
                                name: name.clone(),
                                ownership: OwnershipMode::Value,
                                ty: field_ty.clone(),
                                span: *span,
                            };
                            bindings.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: field_ty,
                                    ownership: OwnershipMode::Value,
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
            ChildNext {
                children: &'expr [Expr],
                index: usize,
                bindings: Rc<BTreeMap<String, Binding>>,
                path: String,
                segment: &'static str,
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
                arms: &'expr [crate::ast::MatchArm],
                bindings: Rc<BTreeMap<String, Binding>>,
            },
            MatchNext {
                span: Span,
                path: String,
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
        }

        fn take_results(results: &mut Vec<ResolvedExpr>, count: usize) -> Vec<ResolvedExpr> {
            let start = results
                .len()
                .checked_sub(count)
                .expect("expression continuation retains every child result");
            results.split_off(start)
        }

        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame<'_>) -> usize {
            let path = match frame {
                Frame::Enter { path, .. }
                | Frame::FinishNativeCall { path, .. }
                | Frame::FinishCall { path, .. }
                | Frame::ChildNext { path, .. }
                | Frame::FinishUnary { path, .. }
                | Frame::FinishBinary { path, .. }
                | Frame::AfterBinaryLeft { path, .. }
                | Frame::BlockNext { path, .. }
                | Frame::BlockAfterLet { path, .. }
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
                | Frame::FinishTry { path, .. }
                | Frame::AfterUpdateBase { path, .. }
                | Frame::UpdateNext { path, .. }
                | Frame::UpdateAfterField { path, .. }
                | Frame::FinishProject { path, .. } => path.capacity(),
            };
            let scope = match frame {
                Frame::Enter { bindings, .. }
                | Frame::ChildNext { bindings, .. }
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
                | Frame::AfterUpdateBase { bindings, .. }
                | Frame::UpdateNext { bindings, .. }
                | Frame::UpdateAfterField { bindings, .. } => {
                    resolver_scope_owned_capacity(bindings)
                }
                Frame::BlockNext { scope, .. } | Frame::BlockAfterLet { scope, .. } => {
                    resolver_scope_owned_capacity(scope)
                }
                _ => 0,
            };
            let retained = match frame {
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
                            Type::I64 | Type::Char | Type::F32 | Type::F64 | Type::Bool => 0,
                            Type::Named { name, arguments } => {
                                name.capacity() + arguments.capacity() * std::mem::size_of::<Type>()
                            }
                        }
                }
                Frame::BlockNext { resolved, .. }
                | Frame::BlockAfterLet { resolved, .. }
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

        const { assert!(std::mem::size_of::<Frame<'static>>() == 552) };

        let mut frames = vec![Frame::Enter {
            expr,
            bindings: Rc::new(bindings.clone()),
            path: path.to_owned(),
        }];
        let mut results = Vec::new();

        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                0,
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + results.capacity() * std::mem::size_of::<ResolvedExpr>()
                    + results
                        .iter()
                        .map(resolved_expr_owned_capacity)
                        .sum::<usize>()
                    + frames.iter().map(frame_owned_capacity).sum::<usize>()
                    + frame_owned_capacity(&frame),
            );
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
                    ExprKind::Char(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Char,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Char(*value),
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
                        if self
                            .declarations
                            .declaration(&record)
                            .is_none_or(|item| item.kind != DeclarationKind::Record)
                        {
                            return Err(self.error(
                                "SPX-H001",
                                format!("constructor target `{type_name}` is not a record"),
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
                    ExprKind::Match { scrutinee, arms } => {
                        frames.push(Frame::AfterMatchScrutinee {
                            span: expr.span,
                            path: path.clone(),
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
                Frame::FinishUnary { span, path, op } => {
                    let value = results.pop().expect("unary child result retained");
                    // Negation keeps the numeric operand type; the validator
                    // and backends fail closed on any other shape.
                    let ty = match (&op, &value.ty) {
                        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
                        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
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
                            ResolvedType::F32,
                        ) => ResolvedType::F32,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::F64,
                        ) => ResolvedType::F64,
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
                        let Statement::Let { value, .. } = &statements[index];
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
                        span: statement_span,
                        ..
                    } = &statements[index];
                    let statement_path = format!("{path}.s{index}");
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
                        },
                    );
                    resolved.push(ResolvedStatement::Let {
                        binding,
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
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty: ResolvedType::Nominal {
                                declaration: variant.clone(),
                                arguments,
                            },
                            ownership: OwnershipMode::Value,
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
                    arms,
                    bindings,
                } => {
                    let scrutinee = results.pop().expect("match scrutinee retained");
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
                    let matched_type = matched_type.clone();
                    let instance_arguments = arguments.clone();
                    frames.push(Frame::MatchNext {
                        span,
                        path,
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
                                scrutinee: Box::new(scrutinee),
                                arms: resolved,
                            },
                            span,
                        });
                    } else {
                        let arm = &arms[index];
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
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
                                    let binding = ResolvedBinding {
                                        id: ValueId::local(
                                            function,
                                            &format!("{path}.arm.{index}.binding.{field_index}"),
                                        ),
                                        name: field.binding.clone(),
                                        ownership: OwnershipMode::Value,
                                        ty: field_ty.clone(),
                                        span: field.binding_span,
                                    };
                                    Rc::make_mut(&mut arm_bindings).insert(
                                        field.binding.clone(),
                                        Binding {
                                            id: binding.id.clone(),
                                            ty: field_ty,
                                            ownership: OwnershipMode::Value,
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
                                )?
                            }
                        };
                        frames.push(Frame::MatchAfterArm {
                            span,
                            path: path.clone(),
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
                    resolved.push(ResolvedMatchArm {
                        pattern,
                        value,
                        span: arms[index].span,
                    });
                    frames.push(Frame::MatchNext {
                        span,
                        path,
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
                        declaration: record,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve field `{field}` on a non-record value"),
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
                            format!("cannot resolve field `{field}` on a non-record value"),
                            span,
                        ));
                    }
                    let field_id = self
                        .declarations
                        .field_id(record, field)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{field}` on record `{record}`"),
                                span,
                            )
                        })?;
                    let field_ty = self
                        .declarations
                        .record_fields(record)
                        .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                        .map(|item| item.ty.clone())
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("field `{field_id}` has no resolved type"),
                                span,
                            )
                        })?;
                    let field_ty = substitute_type(&field_ty, record, arguments)?;
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
            ExprKind::Char(value) => (
                ResolvedExprKind::Char(*value),
                ResolvedType::Char,
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
                                },
                            );
                            resolved_statements.push(ResolvedStatement::Let {
                                binding,
                                value,
                                span: *span,
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
                if self
                    .declarations
                    .declaration(&record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a record"),
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
                (
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields: resolved_fields,
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee = self.resolve_expr_recursive_reference(
                    function,
                    scrutinee,
                    bindings,
                    &format!("{path}.scrutinee"),
                )?;
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
                let instance_arguments = arguments.clone();
                let matched_type = matched_type.clone();
                let mut resolved_arms = Vec::with_capacity(arms.len());
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_bindings = bindings.clone();
                    let pattern = match &arm.pattern {
                        MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                        MatchPattern::Variant {
                            case_name, fields, ..
                        } => {
                            if matched_kind != Some(DeclarationKind::Variant) {
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
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding.{field_index}"),
                                    ),
                                    name: field.binding.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: field_ty.clone(),
                                    span: field.binding_span,
                                };
                                arm_bindings.insert(
                                    field.binding.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: field_ty,
                                        ownership: OwnershipMode::Value,
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
                            if matched_kind != Some(DeclarationKind::Record) {
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
                            )?
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
@id("exercise") fn exercise(flag: bool, choice: Choice, pair: Pair) -> i64
  uses { host.echo }
{
  let x = callee(1, 2);
  let native = host_echo(identity<i64>(x));
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
                    let ResolvedStatement::Let { binding, .. } = &mut statements[0];
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
}
