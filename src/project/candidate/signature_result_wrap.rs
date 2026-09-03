//! Exact owned result wrapping for `change_function_signature`.
//!
//! Authentication is complete before this plan exposes either mutation. The
//! provider constructs one monomorphic record and each direct local caller
//! immediately moves its sole field back out. Ordinary Project replay owns
//! cleanup and target-profile validation. External consumer replay remains a
//! separate API and is not implied by this transformation.

use super::super::{aggregate, object, text, Result};
use crate::ast::{
    Expr, ExprKind, FieldInitializer, Function, ModuleUseKind, Param, ParamMode, Program, Type,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::project::ProjectRevision;
use serde_json::Value;

pub(super) struct Plan {
    record_name: String,
    field_name: String,
    expected_callers: usize,
}

pub(super) fn shape(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G494", message)]
}

fn authentication(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G495", message)]
}

fn caller(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G496", message)]
}

pub(super) fn authenticate(
    revision: &ProjectRevision,
    programs: &[Program],
    owner: usize,
    function_index: usize,
    target: &str,
    request: &Value,
) -> Result<Plan> {
    object(request, &["record", "field"])?;
    let record = text(request, "record")?;
    let field = text(request, "field")?;
    let function = &programs[owner].functions[function_index];
    if !function.explicit_id
        || !function.type_parameters.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !matches!(function.return_type, Type::Bytes | Type::String)
    {
        return Err(shape(
            "owned result wrapping requires one monomorphic explicit function with a whole owning Bytes or String result and no contracts",
        ));
    }
    if revision.entry_program().entrypoint.as_str() == target
        || revision.test_program().entrypoint.as_str() == target
        || revision
            .manifest()
            .web_exports()
            .iter()
            .any(|id| id == target)
    {
        return Err(shape(
            "owned result wrapping excludes entrypoints and manifest exports",
        ));
    }

    let expected = match function.return_type {
        Type::Bytes => ResolvedType::Bytes,
        Type::String => ResolvedType::String,
        _ => unreachable!(),
    };
    let mut checked_function = None;
    let mut checked_record = None;
    for module in revision.semantic.image_modules() {
        for candidate in module
            .functions()
            .iter()
            .filter(|item| item.id.as_str() == target)
        {
            if checked_function.replace((module, candidate)).is_some() {
                return Err(authentication(
                    "owned result provider identity is ambiguous in retained HIR",
                ));
            }
        }
        for candidate in module
            .types()
            .iter()
            .filter(|item| item.id.as_str() == record)
        {
            if checked_record.replace((module, candidate)).is_some() {
                return Err(authentication(
                    "owned result wrapper identity is ambiguous in retained HIR",
                ));
            }
        }
    }
    let (function_module, checked_function) = checked_function.ok_or_else(|| {
        authentication("owned result provider is absent from retained checked HIR")
    })?;
    if checked_function.return_type != expected
        || checked_function.name != function.name
        || checked_function.span != function.span
        || checked_function.params.len() != function.params.len()
        || !checked_function.requires.is_empty()
        || !checked_function.ensures.is_empty()
    {
        return Err(authentication(
            "owned result provider source and checked signature disagree",
        ));
    }
    let mut parameter_work = 0usize;
    for (source, checked) in function.params.iter().zip(&checked_function.params) {
        authenticate_parameter(&programs[owner], source, checked, &mut parameter_work)?;
    }
    let authenticated_function_source = revision
        .sources()
        .iter()
        .find(|source| source.path() == function_module.path())
        .ok_or_else(|| authentication("owned result provider source is absent"))?;
    let authenticated_function_program = crate::parse(
        authenticated_function_source.source(),
        authenticated_function_source.path(),
    )
    .map_err(|error| vec![error])?;
    if !authenticated_function_program
        .functions
        .iter()
        .any(|candidate| candidate == function)
    {
        return Err(authentication(
            "owned result provider AST differs from authenticated source",
        ));
    }
    let (record_module, checked_record) = checked_record.ok_or_else(|| {
        shape("owned result wrapper must name an existing explicit checked record")
    })?;
    if !checked_record.type_parameters.is_empty() {
        return Err(shape("owned result wrapper record must be monomorphic"));
    }
    let ResolvedTypeDeclarationKind::Record { fields } = &checked_record.kind else {
        return Err(shape("owned result wrapper must be a record"));
    };
    if fields.len() != 1 || fields[0].id.as_str() != field || fields[0].ty != expected {
        return Err(shape(
            "owned result wrapper must have exactly one selected field owning the original result type",
        ));
    }
    let wrapper_ty = ResolvedType::Nominal {
        declaration: checked_record.id.clone(),
        arguments: Vec::new(),
    };
    let (_, facts) = record_module
        .signature_type_facts(&wrapper_ty)
        .ok_or_else(|| authentication("owned result wrapper has no retained TypeFacts"))?;
    if facts.copy || !facts.sized || facts.contains_resource || !facts.needs_drop {
        return Err(shape(
            "owned result wrapper must be sized, resource-free, non-Copy, and cleanup-owning",
        ));
    }

    let projection = aggregate::projection_plan(revision, &programs[owner], field, None)?;
    let Type::Named { name, arguments } = projection.owner_type else {
        return Err(authentication(
            "owned result wrapper binding is not nominal",
        ));
    };
    if !arguments.is_empty() || projection.field_name != fields[0].name {
        return Err(authentication(
            "owned result wrapper source binding disagrees with retained HIR",
        ));
    }
    let source_records = programs
        .iter()
        .flat_map(|program| &program.types)
        .filter(|ty| {
            ty.stable_id == record
                && ty.explicit_id
                && matches!(&ty.kind, crate::ast::TypeDeclarationKind::Record { fields }
                if fields.len() == 1 && fields[0].stable_id == field && fields[0].explicit_id
                    && fields[0].ty == function.return_type)
        })
        .collect::<Vec<_>>();
    if source_records.len() != 1 {
        return Err(authentication(
            "owned result wrapper source declaration disagrees with retained HIR",
        ));
    }
    let authenticated_record_source = revision
        .sources()
        .iter()
        .find(|source| source.path() == record_module.path())
        .ok_or_else(|| authentication("owned result wrapper source is absent"))?;
    let authenticated_record_program = crate::parse(
        authenticated_record_source.source(),
        authenticated_record_source.path(),
    )
    .map_err(|error| vec![error])?;
    if !authenticated_record_program
        .types
        .iter()
        .any(|candidate| candidate == source_records[0])
    {
        return Err(authentication(
            "owned result wrapper AST differs from authenticated source",
        ));
    }

    // Every source call is already bound to an exact stable ID by the checked
    // revision. Reject contract occurrences before mutation; body occurrences
    // are migrated through those same independently reconstructed bindings.
    let mut body_calls = 0usize;
    for program in programs {
        let bindings = super::super::call_bindings(program)?;
        for callable in program
            .functions
            .iter()
            .chain(program.types.iter().flat_map(|ty| match &ty.kind {
                crate::ast::TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
                _ => &[],
            }))
        {
            let count = |expr: &Expr| count_calls(expr, &bindings, target);
            if callable.requires.iter().map(count).sum::<usize>() != 0
                || callable.ensures.iter().map(count).sum::<usize>() != 0
            {
                return Err(caller(
                    "owned result wrapping excludes provider calls from contracts",
                ));
            }
            body_calls = body_calls
                .checked_add(count(&callable.body))
                .ok_or_else(|| caller("owned result caller inventory overflow"))?;
        }
    }
    if body_calls == 0 {
        return Err(caller(
            "owned result wrapping requires at least one authenticated local body caller",
        ));
    }

    Ok(Plan {
        record_name: name,
        field_name: fields[0].name.clone(),
        expected_callers: body_calls,
    })
}

fn authenticate_parameter(
    program: &Program,
    source: &Param,
    checked: &ResolvedParam,
    work: &mut usize,
) -> Result<()> {
    let ownership = if source.mode == ParamMode::Value && source.ty == Type::String {
        OwnershipMode::Own
    } else {
        OwnershipMode::from(source.mode)
    };
    let ty = source_type(program, &source.ty, work, 0)?;
    if source.name != checked.name
        || source.span != checked.span
        || ownership != checked.ownership
        || ty != checked.ty
    {
        return Err(authentication(
            "owned result provider parameter disagrees with retained checked name, span, ownership, or type",
        ));
    }
    Ok(())
}

fn source_type(
    program: &Program,
    source: &Type,
    work: &mut usize,
    depth: usize,
) -> Result<ResolvedType> {
    *work = work
        .checked_add(1)
        .ok_or_else(|| authentication("owned result provider parameter type work overflow"))?;
    if *work > super::super::MAX_WALK_NODES || depth > 64 {
        return Err(authentication(
            "owned result provider parameter type exceeds its authentication bound",
        ));
    }
    Ok(match source {
        Type::I64 => ResolvedType::I64,
        Type::I32 => ResolvedType::I32,
        Type::Char => ResolvedType::Char,
        Type::U8 => ResolvedType::U8,
        Type::Usize => ResolvedType::Usize,
        Type::ArrayU8(length) => ResolvedType::ArrayU8(*length),
        Type::F32 => ResolvedType::F32,
        Type::F64 => ResolvedType::F64,
        Type::Bool => ResolvedType::Bool,
        Type::String => ResolvedType::String,
        Type::Bytes => ResolvedType::Bytes,
        Type::Str => ResolvedType::Str,
        Type::SliceU8 => ResolvedType::SliceU8,
        Type::Named { name, arguments } => {
            *work = work
                .checked_add(program.types.len() + program.module_uses.len())
                .ok_or_else(|| authentication("owned result nominal binding work overflow"))?;
            if *work > super::super::MAX_WALK_NODES {
                return Err(authentication(
                    "owned result nominal binding inventory exceeds its authentication bound",
                ));
            }
            let mut identities = std::collections::BTreeSet::new();
            for declaration in &program.types {
                if declaration.name == *name {
                    identities.insert(declaration.stable_id.as_str());
                }
            }
            for binding in &program.module_uses {
                if binding.kind == ModuleUseKind::Type && binding.alias == *name {
                    identities.insert(binding.persistent_id.as_str());
                }
            }
            if name == "Option" {
                identities.insert(crate::prelude::OPTION_ID);
            }
            if name == "Result" {
                identities.insert(crate::prelude::RESULT_ID);
            }
            if identities.len() != 1 {
                return Err(authentication(
                    "owned result provider parameter has an ambiguous nominal source binding",
                ));
            }
            let declaration = DeclarationId::new(*identities.iter().next().expect("one identity"));
            let mut resolved = Vec::with_capacity(arguments.len());
            for argument in arguments {
                resolved.push(source_type(program, argument, work, depth + 1)?);
            }
            ResolvedType::Nominal {
                declaration,
                arguments: resolved,
            }
        }
    })
}

impl Plan {
    pub(super) fn wrap_caller(&self, expression: &mut Expr) {
        let span = expression.span;
        let old = std::mem::replace(&mut expression.kind, ExprKind::Int(0));
        expression.kind = ExprKind::Project {
            base: Box::new(Expr { kind: old, span }),
            field: self.field_name.clone(),
            field_span: span,
        };
    }

    pub(super) fn wrap_provider(&self, function: &mut Function) {
        let span = function.body.span;
        let old = std::mem::replace(&mut function.body.kind, ExprKind::Int(0));
        function.body.kind = ExprKind::ConstructRecord {
            type_name: self.record_name.clone(),
            type_span: span,
            type_arguments: Vec::new(),
            fields: vec![FieldInitializer {
                name: self.field_name.clone(),
                name_span: span,
                value: Expr { kind: old, span },
                span,
            }],
        };
        function.return_type = Type::Named {
            name: self.record_name.clone(),
            arguments: Vec::new(),
        };
    }

    pub(super) fn authenticate_migrated_count(&self, actual: usize) -> Result<()> {
        if actual != self.expected_callers {
            return Err(caller(
                "owned result caller rewrite disagrees with the authenticated body-call inventory",
            ));
        }
        Ok(())
    }
}

fn count_calls(
    expression: &Expr,
    bindings: &std::collections::BTreeMap<String, String>,
    target: &str,
) -> usize {
    let mut count = 0usize;
    expression.visit_calls(&mut |name, _| {
        if bindings.get(name).is_some_and(|id| id == target) {
            count += 1;
        }
    });
    count
}
