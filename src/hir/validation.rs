//! Fail-closed validation of identity-resolved HIR.
//!
//! This leaf validates canonical identities, types, lexical ownership, effects,
//! and call meaning. Stable-ID construction and source resolution remain in
//! the parent module.

use super::*;

/// Validate resolved meaning without consulting attached cleanup metadata.
/// Independent cleanup-plan replayers use this boundary to avoid circularly
/// trusting the canonical cleanup-plan builder as their oracle.
pub(crate) fn validate_core(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    HirValidator::new(program)?.validate()
}

/// `true` when the resolved expression tree contains an unsafe boundary
/// statement anywhere inside its blocks, branches, arms, or nested bodies.
fn contains_unsafe_boundary(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| match statement {
                ResolvedStatement::Unsafe { .. } => true,
                _ => (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(contains_unsafe_boundary)),
            }) || contains_unsafe_boundary(tail)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(contains_unsafe_boundary),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(contains_unsafe_boundary)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => contains_unsafe_boundary(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            contains_unsafe_boundary(left) || contains_unsafe_boundary(right)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_unsafe_boundary(condition)
                || contains_unsafe_boundary(then_branch)
                || contains_unsafe_boundary(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| contains_unsafe_boundary(&field.value)),
        ResolvedExprKind::Match { scrutinee, arms } => {
            contains_unsafe_boundary(scrutinee)
                || arms.iter().any(|arm| contains_unsafe_boundary(&arm.value))
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            contains_unsafe_boundary(base)
                || fields
                    .iter()
                    .any(|field| contains_unsafe_boundary(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => false,
    }
}

#[derive(Clone)]
pub(super) struct HirValidator<'a> {
    program: &'a ResolvedProgram,
    functions: BTreeMap<DeclarationId, &'a ResolvedFunction>,
    expression_ids: BTreeSet<ExpressionId>,
    value_ids: BTreeSet<ValueId>,
}

impl<'a> HirValidator<'a> {
    fn execution_function(&self, id: &FunctionExecutionId) -> Option<&ResolvedFunction> {
        match id {
            FunctionExecutionId::Monomorphic(declaration) => {
                self.functions.get(declaration).copied()
            }
            FunctionExecutionId::Generic(instance) => self
                .program
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance)
                .map(|candidate| &candidate.function),
        }
    }

    pub(super) fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        validate_nul_free_identities(program)?;
        let mut functions = BTreeMap::new();
        for function in &program.functions {
            if functions.insert(function.id.clone(), function).is_some() {
                return Err(hir_error(format!(
                    "duplicate resolved function identity `{}`",
                    function.id
                )));
            }
            match program.declarations.declaration(&function.id) {
                Some(declaration)
                    if declaration.kind == DeclarationKind::Function
                        && declaration.name == function.name => {}
                Some(_) => {
                    return Err(hir_error(format!(
                        "resolved function `{}` disagrees with its declaration index entry",
                        function.id
                    )));
                }
                None => {
                    return Err(hir_error(format!(
                        "resolved function `{}` is absent from the declaration index",
                        function.id
                    )));
                }
            }
        }
        Ok(Self {
            program,
            functions,
            expression_ids: BTreeSet::new(),
            value_ids: BTreeSet::new(),
        })
    }

    fn validate(mut self) -> Result<(), Diagnostic> {
        let entrypoint = self
            .functions
            .get(&self.program.entrypoint)
            .ok_or_else(|| hir_error("resolved entry point is not indexed"))?;
        if !entrypoint.params.is_empty() || entrypoint.return_type != ResolvedType::I64 {
            return Err(hir_error(
                "resolved entry point must have type `fn main() -> i64`",
            ));
        }

        let type_ids = self
            .program
            .types
            .iter()
            .map(|declaration| declaration.id.clone())
            .collect::<BTreeSet<_>>();
        if type_ids.len() != self.program.types.len() {
            return Err(hir_error("duplicate resolved type declaration identity"));
        }
        let mut interface_ids = BTreeSet::new();
        let mut import_ids = BTreeSet::new();
        let mut imports = BTreeMap::new();
        for interface in &self.program.interfaces {
            if !interface_ids.insert(interface.id.clone()) {
                return Err(hir_error(format!(
                    "duplicate resolved interface identity `{}`",
                    interface.id
                )));
            }
            match self.program.declarations.declaration(&interface.id) {
                Some(item)
                    if item.kind == DeclarationKind::Interface
                        && item.name == interface.name
                        && item.owner.is_none() => {}
                _ => {
                    return Err(hir_error(format!(
                        "resolved interface `{}` disagrees with its declaration index entry",
                        interface.id
                    )));
                }
            }
            let permits = interface.permits.iter().collect::<BTreeSet<_>>();
            if permits.len() != interface.permits.len() {
                return Err(hir_error(format!(
                    "interface `{}` has duplicate permits",
                    interface.id
                )));
            }
            for import in &interface.imports {
                if !import_ids.insert(import.id.clone())
                    || imports.insert(import.id.clone(), import).is_some()
                {
                    return Err(hir_error(format!(
                        "duplicate resolved import identity `{}`",
                        import.id
                    )));
                }
                if import.interface != interface.id
                    || self.program.declarations.import_id(&import.import_key) != Some(&import.id)
                {
                    return Err(hir_error(format!(
                        "import `{}` has an invalid owner or logical key",
                        import.id
                    )));
                }
                match self.program.declarations.declaration(&import.id) {
                    Some(item)
                        if item.kind == DeclarationKind::Import
                            && item.name == import.name
                            && item.owner.as_ref() == Some(&interface.id) => {}
                    _ => {
                        return Err(hir_error(format!(
                            "resolved import `{}` disagrees with its declaration index entry",
                            import.id
                        )));
                    }
                }
                let native_shape = import.native_rust
                    && import.parameters.len() <= 8
                    && import.parameters.iter().all(|parameter| {
                        parameter.ownership == OwnershipMode::Value
                            && !parameter.consumes_on_failure
                            && matches!(parameter.ty, ResolvedType::I64 | ResolvedType::Bool)
                    })
                    && matches!(
                        import.result.kind,
                        ResolvedImportResultKind::Unit
                            | ResolvedImportResultKind::I64
                            | ResolvedImportResultKind::Bool
                    );
                let lifecycle_shape = !import.native_rust
                    && import.parameters.len() == 1
                    && import.parameters[0].ownership == OwnershipMode::Own
                    && import.parameters[0].consumes_on_failure
                    && import.result.kind == ResolvedImportResultKind::Unit;
                if (!native_shape && !lifecycle_shape)
                    || import.result.ownership != OwnershipMode::Value
                    || import.result.producer != "callee"
                    || import.result.out_slot_initialization != "success_only"
                    || import.result.ownership_transfer != "final_zero_status_commit"
                    || import.required_authority != import.effects
                {
                    return Err(hir_error(format!(
                        "import `{}` has an invalid ownership or result contract",
                        import.id
                    )));
                }
                for parameter in &import.parameters {
                    self.validate_type(&parameter.ty)?;
                }
                let parameter_is_resource = import.parameters.first().is_some_and(|parameter| {
                    parameter
                        .ty
                        .nominal_id()
                        .and_then(|id| self.program.declarations.declaration(id))
                        .is_some_and(|item| item.kind == DeclarationKind::Resource)
                });
                let effects = import.effects.iter().collect::<BTreeSet<_>>();
                let authority = import.required_authority.iter().collect::<BTreeSet<_>>();
                if (!import.native_rust && !parameter_is_resource)
                    || effects.len() != import.effects.len()
                    || authority.len() != import.required_authority.len()
                {
                    return Err(hir_error(format!(
                        "import `{}` has a noncanonical resource or authority contract",
                        import.id
                    )));
                }
                if import
                    .effects
                    .iter()
                    .any(|effect| !permits.contains(effect))
                {
                    return Err(hir_error(format!(
                        "import `{}` exceeds interface `{}` authority",
                        import.id, interface.id
                    )));
                }
                if let ResolvedImportFailure::Status {
                    domain_id,
                    normalization,
                } = &import.failure
                {
                    let native_domain_valid = || {
                        let bytes = domain_id.as_bytes();
                        (2..=STATUS_DOMAIN_MAX_BYTES_V1).contains(&bytes.len())
                            && bytes.first().is_some_and(|byte| {
                                byte.is_ascii_lowercase() || byte.is_ascii_digit()
                            })
                            && bytes.last().is_some_and(|byte| {
                                byte.is_ascii_lowercase() || byte.is_ascii_digit()
                            })
                            && bytes.iter().all(|byte| {
                                byte.is_ascii_lowercase()
                                    || byte.is_ascii_digit()
                                    || matches!(byte, b'.' | b'-')
                            })
                    };
                    if (import.native_rust && !native_domain_valid())
                        || (!import.native_rust
                            && (domain_id.is_empty()
                                || domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1
                                || domain_id.contains('\0')))
                        || *normalization != "semaprax.status.v1"
                    {
                        return Err(hir_error(format!(
                            "import `{}` has an invalid status contract",
                            import.id
                        )));
                    }
                }
            }
        }
        let mut template_ids = BTreeSet::new();
        for template in &self.program.function_templates {
            if !template_ids.insert(template.id.clone())
                || self.functions.contains_key(&template.id)
            {
                return Err(hir_error(format!(
                    "duplicate or executable generic template `{}`",
                    template.id
                )));
            }
            match self.program.declarations.declaration(&template.id) {
                Some(declaration)
                    if declaration.kind == DeclarationKind::Function
                        && declaration.name == template.name => {}
                _ => {
                    return Err(hir_error(format!(
                        "generic template `{}` disagrees with its declaration",
                        template.id
                    )));
                }
            }
            if !(1..=2).contains(&template.type_parameters.len()) || !template.effects.is_empty() {
                return Err(hir_error(format!(
                    "generic template `{}` is outside the bounded slice",
                    template.id
                )));
            }
            if self.program.declarations.type_parameters(&template.id)
                != Some(template.type_parameters.as_slice())
            {
                return Err(hir_error(format!(
                    "generic template `{}` has non-canonical type-parameter metadata",
                    template.id
                )));
            }
            for (index, parameter) in template.type_parameters.iter().enumerate() {
                if usize::try_from(parameter.index) != Ok(index) {
                    return Err(hir_error(format!(
                        "generic template `{}` has non-canonical parameter indices",
                        template.id
                    )));
                }
            }
            let execution = FunctionExecutionId::Monomorphic(template.id.clone());
            for (index, parameter) in template.params.iter().enumerate() {
                if parameter.id != ValueId::parameter(&execution, index)
                    || parameter.ownership != OwnershipMode::Value
                {
                    return Err(hir_error(format!(
                        "generic template `{}` has invalid parameter identity or ownership",
                        template.id
                    )));
                }
                self.validate_function_template_type(template, &parameter.ty)?;
            }
            self.validate_function_template_type(template, &template.return_type)?;
            if template.result_id != ValueId::result(&execution) {
                return Err(hir_error(format!(
                    "generic template `{}` has invalid result identity",
                    template.id
                )));
            }
            self.validate_template_expressions(template, &execution)?;
            for arguments in resolved_scalar_substitutions(template.type_parameters.len()) {
                let materialized = materialize_function_template(template, &arguments)?;
                let saved_expression_ids = self.expression_ids.clone();
                let saved_value_ids = self.value_ids.clone();
                self.validate_function(
                    &materialized,
                    &FunctionExecutionId::Generic(FunctionInstanceId::derive(
                        &template.id,
                        &arguments,
                    )),
                )?;
                self.expression_ids = saved_expression_ids;
                self.value_ids = saved_value_ids;
            }
        }
        let mut resource_drop_ids = BTreeSet::new();
        let mut field_ids = BTreeSet::new();
        let mut variant_case_ids = BTreeSet::new();
        let mut case_field_ids = BTreeSet::new();
        for declaration in &self.program.types {
            let expected_kind = match &declaration.kind {
                ResolvedTypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                ResolvedTypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                ResolvedTypeDeclarationKind::Class { .. } => DeclarationKind::Class,
                ResolvedTypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            match self.program.declarations.declaration(&declaration.id) {
                Some(item)
                    if item.kind == expected_kind
                        && item.name == declaration.name
                        && item.owner.is_none() => {}
                Some(_) => {
                    return Err(hir_error(format!(
                        "resolved type `{}` disagrees with its declaration index entry",
                        declaration.id
                    )));
                }
                None => {
                    return Err(hir_error(format!(
                        "resolved type `{}` is absent from the declaration index",
                        declaration.id
                    )));
                }
            }
            let indexed_parameters = self
                .program
                .declarations
                .type_parameters(&declaration.id)
                .ok_or_else(|| {
                    hir_error(format!(
                        "type `{}` has no indexed parameter sequence",
                        declaration.id
                    ))
                })?;
            if indexed_parameters != declaration.type_parameters.as_slice()
                || declaration
                    .type_parameters
                    .iter()
                    .enumerate()
                    .any(|(index, parameter)| usize::try_from(parameter.index) != Ok(index))
                || declaration
                    .type_parameters
                    .iter()
                    .map(|parameter| &parameter.name)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != declaration.type_parameters.len()
                || (matches!(
                    declaration.kind,
                    ResolvedTypeDeclarationKind::Resource { .. }
                ) && !declaration.type_parameters.is_empty())
            {
                return Err(hir_error(format!(
                    "type `{}` has invalid generic parameter metadata",
                    declaration.id
                )));
            }
            if let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind {
                if !resource_drop_ids.insert(drop.id.clone()) {
                    return Err(hir_error(format!(
                        "duplicate resolved resource lifecycle identity `{}`",
                        drop.id
                    )));
                }
                match self.program.declarations.declaration(&drop.id) {
                    Some(item)
                        if item.kind == DeclarationKind::ResourceDrop
                            && item.name == "drop"
                            && item.owner.as_ref() == Some(&declaration.id) => {}
                    _ => {
                        return Err(hir_error(format!(
                            "resource `{}` has an invalid lifecycle declaration `{}`",
                            declaration.id, drop.id
                        )));
                    }
                }
                if let ResolvedResourceDropKind::Imported { import, import_key } = &drop.kind {
                    let resolved_import = imports.get(import).ok_or_else(|| {
                        hir_error(format!(
                            "resource `{}` lifecycle references unknown import `{import}`",
                            declaration.id
                        ))
                    })?;
                    let expected_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    if resolved_import.import_key != *import_key
                        || resolved_import.parameters[0].ty != expected_ty
                        || !matches!(resolved_import.failure, ResolvedImportFailure::Infallible)
                    {
                        return Err(hir_error(format!(
                            "resource `{}` lifecycle is incompatible with import `{import}`",
                            declaration.id
                        )));
                    }
                }
            }
            if let ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } = &declaration.kind
            {
                let indexed = self
                    .program
                    .declarations
                    .record_fields(&declaration.id)
                    .ok_or_else(|| {
                        hir_error(format!(
                            "record `{}` has no indexed field sequence",
                            declaration.id
                        ))
                    })?;
                if indexed.len() != fields.len() {
                    return Err(hir_error(format!(
                        "record `{}` field sequence disagrees with its declaration index",
                        declaration.id
                    )));
                }
                for (position, (field, indexed_field)) in fields.iter().zip(indexed).enumerate() {
                    // Class Inheritance v1: inherited members reappear in a
                    // child's effective sequence; global identity uniqueness
                    // remains scoped to each declaring class.
                    let declared_here = self
                        .program
                        .declarations
                        .declaration(&field.id)
                        .and_then(|item| item.owner.clone())
                        .is_some_and(|owner| owner == declaration.id);
                    if declared_here && !field_ids.insert(field.id.clone()) {
                        return Err(hir_error(format!(
                            "duplicate resolved field identity `{}`",
                            field.id
                        )));
                    }
                    if field.id != indexed_field.id
                        || field.name != indexed_field.name
                        || usize::try_from(field.index) != Ok(position)
                        || field.index != indexed_field.index
                        || field.ty != indexed_field.ty
                    {
                        return Err(hir_error(format!(
                            "field {position} of record `{}` disagrees with its declaration index",
                            declaration.id
                        )));
                    }
                    // An effective member is either declared by this class or
                    // owned by one of its ancestors.
                    let mut owner_ok = self
                        .program
                        .declarations
                        .declaration(&field.id)
                        .and_then(|item| item.owner.clone())
                        .map(|owner| owner == declaration.id)
                        .unwrap_or(false);
                    if !owner_ok {
                        let mut ancestors =
                            self.program.declarations.class_ancestors(&declaration.id);
                        owner_ok = ancestors.iter().any(|ancestor| {
                            self.program
                                .declarations
                                .declaration(&field.id)
                                .and_then(|item| item.owner.clone())
                                .is_some_and(|owner| owner == *ancestor)
                        });
                        let _ = &mut ancestors;
                    }
                    match self.program.declarations.declaration(&field.id) {
                        Some(item)
                            if item.kind == DeclarationKind::Field
                                && item.name == field.name
                                && owner_ok
                                && self
                                    .program
                                    .declarations
                                    .field_id(&declaration.id, &field.name)
                                    == Some(&field.id) => {}
                        _ => {
                            return Err(hir_error(format!(
                                "field `{}` is not indexed under record `{}`",
                                field.id, declaration.id
                            )));
                        }
                    }
                    if declaration.type_parameters.is_empty() {
                        if field.ty == ResolvedType::Unit {
                            return Err(hir_error(format!(
                                "field `{}` uses Unit outside a native Rust import result",
                                field.id
                            )));
                        }
                        self.validate_type(&field.ty)?;
                        if let ResolvedType::Nominal {
                            declaration: field_declaration,
                            arguments,
                        } = &field.ty
                        {
                            if !arguments.is_empty()
                                && self
                                    .program
                                    .declarations
                                    .declaration(field_declaration)
                                    .is_some_and(|item| item.kind == DeclarationKind::Record)
                            {
                                return Err(hir_error(format!(
                                    "field `{}` nests a generic record instance outside the admitted slice",
                                    field.id
                                )));
                            }
                        }
                    } else {
                        match &field.ty {
                            ResolvedType::I64 | ResolvedType::Bool => {}
                            ResolvedType::I32
                            | ResolvedType::Char
                            | ResolvedType::U8
                            | ResolvedType::F32
                            | ResolvedType::F64
                            | ResolvedType::String => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy record template",
                                    field.id
                                )));
                            }
                            ResolvedType::TypeParameter { owner, index }
                                if owner == &declaration.id
                                    && declaration
                                        .type_parameters
                                        .get(usize::try_from(*index).map_err(|_| {
                                            hir_error("type parameter index does not fit usize")
                                        })?)
                                        .is_some() => {}
                            ResolvedType::Unit
                            | ResolvedType::TypeParameter { .. }
                            | ResolvedType::Nominal { .. } => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy record template",
                                    field.id
                                )));
                            }
                        }
                    }
                }
                if declaration.type_parameters.is_empty() {
                    let record_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    let cached = self.program.declarations.type_facts(&record_ty);
                    let recomputed = self.program.declarations.recompute_type_facts(&record_ty);
                    if cached.is_none() || cached != recomputed {
                        return Err(hir_error(format!(
                            "record `{}` has invalid or stale recursive type facts",
                            declaration.id
                        )));
                    }
                }
            }
            if let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind {
                if cases.is_empty() {
                    return Err(hir_error(format!(
                        "variant `{}` has no cases",
                        declaration.id
                    )));
                }
                let indexed = self
                    .program
                    .declarations
                    .variant_cases(&declaration.id)
                    .ok_or_else(|| {
                        hir_error(format!(
                            "variant `{}` has no indexed case sequence",
                            declaration.id
                        ))
                    })?;
                if indexed.len() != cases.len() {
                    return Err(hir_error(format!(
                        "variant `{}` case sequence disagrees with its declaration index",
                        declaration.id
                    )));
                }
                for (case_position, (case, indexed_case)) in cases.iter().zip(indexed).enumerate() {
                    if !variant_case_ids.insert(case.id.clone())
                        || case.id != indexed_case.id
                        || case.name != indexed_case.name
                        || usize::try_from(case.index) != Ok(case_position)
                        || case.index != indexed_case.index
                    {
                        return Err(hir_error(format!(
                            "case {case_position} of variant `{}` disagrees with its declaration index",
                            declaration.id
                        )));
                    }
                    match self.program.declarations.declaration(&case.id) {
                        Some(item)
                            if item.kind == DeclarationKind::VariantCase
                                && item.name == case.name
                                && item.owner.as_ref() == Some(&declaration.id)
                                && self
                                    .program
                                    .declarations
                                    .case_id(&declaration.id, &case.name)
                                    == Some(&case.id) => {}
                        _ => {
                            return Err(hir_error(format!(
                                "case `{}` is not indexed under variant `{}`",
                                case.id, declaration.id
                            )));
                        }
                    }
                    let indexed_fields = self
                        .program
                        .declarations
                        .case_fields(&case.id)
                        .ok_or_else(|| {
                            hir_error(format!("case `{}` has no indexed field sequence", case.id))
                        })?;
                    if indexed_fields.len() != case.fields.len() {
                        return Err(hir_error(format!(
                            "case `{}` field sequence disagrees with its declaration index",
                            case.id
                        )));
                    }
                    for (field_position, (field, indexed_field)) in
                        case.fields.iter().zip(indexed_fields).enumerate()
                    {
                        if !case_field_ids.insert(field.id.clone())
                            || field.id != indexed_field.id
                            || field.name != indexed_field.name
                            || usize::try_from(field.index) != Ok(field_position)
                            || field.index != indexed_field.index
                            || field.ty != indexed_field.ty
                            || !matches!(
                                field.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::Bool
                                    | ResolvedType::TypeParameter { .. }
                            )
                        {
                            return Err(hir_error(format!(
                                "field {field_position} of case `{}` is invalid or disagrees with its declaration index",
                                case.id
                            )));
                        }
                        match self.program.declarations.declaration(&field.id) {
                            Some(item)
                                if item.kind == DeclarationKind::CaseField
                                    && item.name == field.name
                                    && item.owner.as_ref() == Some(&case.id)
                                    && self
                                        .program
                                        .declarations
                                        .field_id(&case.id, &field.name)
                                        == Some(&field.id) => {}
                            _ => {
                                return Err(hir_error(format!(
                                    "field `{}` is not indexed under case `{}`",
                                    field.id, case.id
                                )));
                            }
                        }
                        match &field.ty {
                            ResolvedType::I64 | ResolvedType::Bool => {}
                            ResolvedType::I32
                            | ResolvedType::Char
                            | ResolvedType::U8
                            | ResolvedType::F32
                            | ResolvedType::F64
                            | ResolvedType::String => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy payload template",
                                    field.id
                                )));
                            }
                            ResolvedType::TypeParameter { owner, index }
                                if owner == &declaration.id
                                    && declaration
                                        .type_parameters
                                        .get(usize::try_from(*index).map_err(|_| {
                                            hir_error("type parameter index does not fit usize")
                                        })?)
                                        .is_some() => {}
                            ResolvedType::Unit
                            | ResolvedType::TypeParameter { .. }
                            | ResolvedType::Nominal { .. } => {
                                return Err(hir_error(format!(
                                    "field `{}` has an invalid generic copy payload template",
                                    field.id
                                )));
                            }
                        }
                    }
                }
                if declaration.type_parameters.is_empty() {
                    let variant_ty = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    let cached = self.program.declarations.type_facts(&variant_ty);
                    let recomputed = self.program.declarations.recompute_type_facts(&variant_ty);
                    if cached.is_none()
                        || cached != recomputed
                        || cached.as_ref().is_none_or(|facts| {
                            !facts.copy
                                || facts.contains_resource
                                || facts.needs_drop
                                || !facts.sized
                        })
                    {
                        return Err(hir_error(format!(
                            "variant `{}` has invalid or stale type facts",
                            declaration.id
                        )));
                    }
                }
            }
        }
        for declaration in self.program.declarations.declarations() {
            match declaration.kind {
                DeclarationKind::Resource
                | DeclarationKind::Record
                | DeclarationKind::Class
                | DeclarationKind::Variant
                    if !type_ids.contains(&declaration.id) =>
                {
                    return Err(hir_error(format!(
                        "type `{}` has no resolved type declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Field if !field_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "field `{}` has no resolved field declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::VariantCase if !variant_case_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "variant case `{}` has no resolved case declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::CaseField if !case_field_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "case field `{}` has no resolved field declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Function
                    if !self.functions.contains_key(&declaration.id)
                        && !template_ids.contains(&declaration.id) =>
                {
                    return Err(hir_error(format!(
                        "function `{}` has no resolved function body",
                        declaration.id
                    )));
                }
                DeclarationKind::ResourceDrop if !resource_drop_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "resource lifecycle `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Interface if !interface_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "interface `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Import if !import_ids.contains(&declaration.id) => {
                    return Err(hir_error(format!(
                        "import `{}` has no resolved declaration",
                        declaration.id
                    )));
                }
                DeclarationKind::Resource
                | DeclarationKind::ResourceDrop
                | DeclarationKind::Record
                | DeclarationKind::Class
                | DeclarationKind::Field
                | DeclarationKind::Variant
                | DeclarationKind::VariantCase
                | DeclarationKind::CaseField
                | DeclarationKind::Interface
                | DeclarationKind::Import
                | DeclarationKind::Function => {}
            }
        }

        for function in &self.program.functions {
            self.validate_function(
                function,
                &FunctionExecutionId::Monomorphic(function.id.clone()),
            )?;
        }
        let mut instance_ids = BTreeSet::new();
        let expected_instances = self.reachable_function_instances()?;
        if expected_instances.len() != self.program.function_instances.len()
            || expected_instances
                .iter()
                .zip(&self.program.function_instances)
                .any(|((id, template, arguments), actual)| {
                    id != &actual.id
                        || template != &actual.template
                        || arguments != &actual.type_arguments
                })
        {
            return Err(hir_error(
                "materialized generic function instances are not the exact reachable sequence",
            ));
        }
        for instance in &self.program.function_instances {
            if !instance_ids.insert(instance.id.clone())
                || FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
                    != instance.id
                || instance.function.id != instance.template
                || instance
                    .type_arguments
                    .iter()
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(hir_error(
                    "generic function instance identity is inconsistent",
                ));
            }
            let template = self
                .program
                .function_templates
                .iter()
                .find(|template| template.id == instance.template)
                .ok_or_else(|| hir_error("generic function instance has no template"))?;
            if template.type_parameters.len() != instance.type_arguments.len()
                || template.params.len() != instance.function.params.len()
            {
                return Err(hir_error("generic function instance has invalid arity"));
            }
            for (template_param, concrete_param) in
                template.params.iter().zip(&instance.function.params)
            {
                let expected =
                    substitute_type(&template_param.ty, &template.id, &instance.type_arguments)?;
                if concrete_param.ty != expected {
                    return Err(hir_error(
                        "generic function instance parameter substitution is inconsistent",
                    ));
                }
            }
            let expected_return = substitute_type(
                &template.return_type,
                &template.id,
                &instance.type_arguments,
            )?;
            if instance.function.return_type != expected_return {
                return Err(hir_error(
                    "generic function instance result substitution is inconsistent",
                ));
            }
            let expected_function =
                materialize_function_template(template, &instance.type_arguments)?;
            if !same_function_meaning(&expected_function, &instance.function) {
                return Err(hir_error(
                    "generic function instance is not the exact template substitution",
                ));
            }
            self.validate_function(
                &instance.function,
                &FunctionExecutionId::Generic(instance.id.clone()),
            )?;
        }
        Ok(())
    }

    fn validate_function_template_type(
        &self,
        template: &ResolvedFunctionTemplate,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool => Ok(()),
            ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::String => Err(hir_error(format!(
                "generic template `{}` has an invalid direct-scalar signature slot",
                template.id
            ))),
            ResolvedType::TypeParameter { owner, index }
                if owner == &template.id
                    && usize::try_from(*index)
                        .ok()
                        .is_some_and(|index| index < template.type_parameters.len()) =>
            {
                Ok(())
            }
            ResolvedType::Unit
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::Nominal { .. } => Err(hir_error(format!(
                "generic template `{}` has an invalid direct-scalar signature slot",
                template.id
            ))),
        }
    }

    fn validate_template_expressions(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        let mut values = BTreeMap::new();
        for parameter in &template.params {
            self.insert_value(&parameter.id)?;
            values.insert(parameter.id.clone(), parameter.ty.clone());
        }
        for (index, expression) in template.requires.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("requires.{index}"),
            )?;
        }
        self.validate_template_expr(template, execution, &template.body, &mut values, "body")?;
        self.insert_value(&template.result_id)?;
        values.insert(template.result_id.clone(), template.return_type.clone());
        for (index, expression) in template.ensures.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("ensures.{index}"),
            )?;
        }
        Ok(())
    }

    fn validate_template_expr(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if expression.id != ExpressionId::new(execution, path)
            || !self.expression_ids.insert(expression.id.clone())
            || expression.ownership != OwnershipMode::Value
        {
            return Err(hir_error(format!(
                "generic template `{}` has invalid expression identity or ownership",
                template.id
            )));
        }
        self.validate_function_template_type(template, &expression.ty)?;
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_) => {}
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() || values.get(&place.root) != Some(&expression.ty)
                {
                    return Err(hir_error(
                        "generic template place is out of scope or has the wrong type",
                    ));
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if instance.is_some()
                    || !type_arguments.is_empty()
                    || self
                        .program
                        .functions
                        .iter()
                        .all(|target| target.id != *callee)
                {
                    return Err(hir_error(
                        "generic template call is not a monomorphic executable target",
                    ));
                }
                for (index, argument) in args.iter().enumerate() {
                    self.validate_template_expr(
                        template,
                        execution,
                        argument,
                        values,
                        &format!("{path}.arg.{index}"),
                    )?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(_) => {
                return Err(hir_error(
                    "generic templates cannot call native Rust imports",
                ));
            }
            ResolvedExprKind::Unary { value, .. } => self.validate_template_expr(
                template,
                execution,
                value,
                values,
                &format!("{path}.value"),
            )?,
            ResolvedExprKind::Binary { left, right, .. } => {
                self.validate_template_expr(
                    template,
                    execution,
                    left,
                    values,
                    &format!("{path}.left"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    right,
                    values,
                    &format!("{path}.right"),
                )?;
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut block_values = values.clone();
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            self.validate_template_expr(
                                template,
                                execution,
                                value,
                                &mut block_values,
                                &format!("{statement_path}.value"),
                            )?;
                            if binding.id != ValueId::local(execution, &statement_path)
                                || binding.ownership != OwnershipMode::Value
                                || binding.ty != value.ty
                            {
                                return Err(hir_error("generic template binding is not canonical"));
                            }
                            self.validate_function_template_type(template, &binding.ty)?;
                            self.insert_value(&binding.id)?;
                            block_values.insert(binding.id.clone(), binding.ty.clone());
                        }
                        ResolvedStatement::Assign { .. } => {
                            return Err(hir_error(
                                "generic template statements cannot assign to local bindings",
                            ));
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            self.validate_template_expr(
                                template,
                                execution,
                                body,
                                &mut block_values,
                                &format!("{statement_path}.body"),
                            )?;
                        }
                        ResolvedStatement::While { .. } => {
                            return Err(hir_error("generic templates cannot contain while loops"));
                        }
                    }
                }
                self.validate_template_expr(
                    template,
                    execution,
                    tail,
                    &mut block_values,
                    &format!("{path}.tail"),
                )?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_template_expr(
                    template,
                    execution,
                    condition,
                    &mut values.clone(),
                    &format!("{path}.condition"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    then_branch,
                    &mut values.clone(),
                    &format!("{path}.then"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    else_branch,
                    &mut values.clone(),
                    &format!("{path}.else"),
                )?;
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. } => {
                return Err(hir_error(
                    "generic template expression is outside the direct-scalar slice",
                ));
            }
        }
        Ok(())
    }

    /// Bounded While-Loops v1 admission re-check at the HIR trust boundary.
    /// Loop conditions and bodies may contain only Copy-scalar operations;
    /// anything else fails closed as malformed HIR because source resolution
    /// already rejected it with `SPX-T252`.
    fn validate_while_admission(&self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::Place(_) => Ok(()),
            ResolvedExprKind::String(_) => {
                Err(hir_error("while loops cannot contain string literals"))
            }
            ResolvedExprKind::Unary { value, .. } => self.validate_while_admission(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                self.validate_while_admission(left)?;
                self.validate_while_admission(right)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_while_admission(condition)?;
                self.validate_while_admission(then_branch)?;
                self.validate_while_admission(else_branch)
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        let child = statement
                            .child(index)
                            .ok_or_else(|| hir_error("while statement child is missing"))?;
                        self.validate_while_admission(child)?;
                    }
                }
                self.validate_while_admission(tail)
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(hir_error("while loops cannot contain generic calls"));
                }
                let target = self
                    .program
                    .resolve_call_target(callee, None)
                    .ok_or_else(|| {
                        hir_error(format!("while loop call `{callee}` is not indexed"))
                    })?;
                let scalar_signature = crate::hir::is_scalar_resolved_type(&target.return_type)
                    && target.params.iter().all(|param| {
                        param.ownership == OwnershipMode::Value
                            && crate::hir::is_scalar_resolved_type(&param.ty)
                    });
                if !scalar_signature {
                    return Err(hir_error(format!(
                        "while loop call `{callee}` is not a scalar-value function"
                    )));
                }
                for argument in args {
                    self.validate_while_admission(argument)?;
                }
                Ok(())
            }
            ResolvedExprKind::NativeRustImportCall(_) => Err(hir_error(
                "while loops cannot contain native Rust import calls",
            )),
            ResolvedExprKind::Upcast { .. } => {
                Err(hir_error("while loops cannot contain inheritance upcasts"))
            }
            ResolvedExprKind::Project { .. } => {
                Err(hir_error("while loops cannot project record fields"))
            }
            ResolvedExprKind::ConstructRecord { .. } => {
                Err(hir_error("while loops cannot construct records"))
            }
            ResolvedExprKind::ConstructVariant { .. } => {
                Err(hir_error("while loops cannot construct variants"))
            }
            ResolvedExprKind::UpdateRecord { .. } => {
                Err(hir_error("while loops cannot update records"))
            }
            ResolvedExprKind::Match { .. } => {
                Err(hir_error("while loops cannot contain match expressions"))
            }
            ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => Err(hir_error(
                "while loops cannot contain postfix `?` propagation",
            )),
        }
    }

    fn reachable_function_instances(
        &self,
    ) -> Result<Vec<(FunctionInstanceId, DeclarationId, Vec<ResolvedType>)>, Diagnostic> {
        let mut seen = BTreeSet::new();
        let mut reachable = Vec::new();
        for function in &self.program.functions {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                    let Some(instance) = instance else {
                        return;
                    };
                    if seen.insert(instance.clone()) {
                        reachable.push((instance.clone(), callee.clone(), arguments.to_vec()));
                    }
                });
            }
        }
        for (instance, template, arguments) in &reachable {
            if FunctionInstanceId::derive(template, arguments) != *instance {
                return Err(hir_error(
                    "reachable generic function instance identity is inconsistent",
                ));
            }
        }
        Ok(reachable)
    }

    fn validate_function(
        &mut self,
        function: &ResolvedFunction,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        if function.return_type == ResolvedType::Unit {
            return Err(hir_error(
                "ordinary resolved functions cannot declare a unit result",
            ));
        }
        self.validate_type(&function.return_type)?;
        let permits = self
            .program
            .permits
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for effect in &function.effects {
            if !permits.contains(effect.as_str()) {
                return Err(hir_error(format!(
                    "function `{}` declares effect `{effect}` which the module does not permit",
                    function.id
                )));
            }
        }
        if contains_unsafe_boundary(&function.body) && !permits.contains("unsafe") {
            return Err(hir_error(format!(
                "function `{}` contains an unsafe boundary but the module does not declare `permit {{ unsafe }}`",
                function.id
            )));
        }
        let declared_effects = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        let mut required_lifecycle_effects = BTreeSet::new();
        for param in &function.params {
            if param.ownership == OwnershipMode::Own {
                required_lifecycle_effects
                    .extend(resolved_lifecycle_effects(self.program, &param.ty)?);
            }
        }
        required_lifecycle_effects.extend(resolved_lifecycle_effects(
            self.program,
            &function.return_type,
        )?);
        let mut callees = Vec::new();
        visit_resolved_calls(&function.body, &mut |callee, instance, _| {
            callees.push((callee.clone(), instance.cloned()));
        });
        for (callee, instance) in callees {
            if instance.is_none() && crate::string_ops::by_id(callee.as_str()).is_some() {
                // String operations carry no authored declaration and their
                // scalar/string results contribute no lifecycle effects.
                continue;
            }
            let target = self
                .program
                .resolve_call_target(&callee, instance.as_ref())
                .ok_or_else(|| hir_error(format!("function `{callee}` is not indexed")))?;
            required_lifecycle_effects.extend(resolved_lifecycle_effects(
                self.program,
                &target.return_type,
            )?);
        }
        if let Some(effect) = required_lifecycle_effects
            .iter()
            .find(|effect| !declared_effects.contains(*effect))
        {
            return Err(hir_error(format!(
                "function `{}` omits lifecycle effect `{effect}`",
                function.id
            )));
        }
        let mut scope = BTreeMap::new();
        for (index, param) in function.params.iter().enumerate() {
            if param.ty == ResolvedType::Unit {
                return Err(hir_error(
                    "ordinary resolved functions cannot declare a unit parameter",
                ));
            }
            reject_nul_identity("resolved value", param.id.as_str())?;
            let expected = ValueId::parameter(execution, index);
            if param.id != expected {
                return Err(hir_error(format!(
                    "parameter {} of `{}` has a non-canonical identity",
                    index, function.id
                )));
            }
            self.insert_value(&param.id)?;
            self.validate_type(&param.ty)?;
            self.validate_declared_ownership(&param.ty, param.ownership)?;
            scope.insert(
                param.id.clone(),
                ValidationBinding {
                    ty: param.ty.clone(),
                    ownership: param.ownership,
                    availability: Availability::Available,
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                },
            );
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        if function.result_id != ValueId::result(execution) {
            return Err(hir_error(format!(
                "function `{}` has a non-canonical result identity",
                function.id
            )));
        }
        self.insert_value(&function.result_id)?;

        for (index, contract) in function.requires.iter().enumerate() {
            let mut contract_scope = scope.clone();
            self.validate_expr(
                execution,
                contract,
                &mut contract_scope,
                &format!("requires.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "precondition")?;
        }
        self.validate_expr(
            execution,
            &function.body,
            &mut scope,
            "body",
            true,
            Some(&declared_effects),
        )?;
        self.require_type(&function.body.ty, &function.return_type, "function body")?;
        let returned = self.expected_ownership(&function.return_type, OwnershipMode::Own)?;
        if function.body.ownership != returned {
            return Err(hir_error(format!(
                "function `{}` body has invalid return ownership",
                function.id
            )));
        }

        let mut ensures_scope = scope;
        ensures_scope.insert(
            function.result_id.clone(),
            ValidationBinding {
                ty: function.return_type.clone(),
                ownership: returned,
                availability: Availability::Available,
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        );
        for (index, contract) in function.ensures.iter().enumerate() {
            let mut contract_scope = ensures_scope.clone();
            self.validate_expr(
                execution,
                contract,
                &mut contract_scope,
                &format!("ensures.{index}"),
                false,
                None,
            )?;
            self.require_type(&contract.ty, &ResolvedType::Bool, "postcondition")?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_record_match_pattern(
        &mut self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[ResolvedRecordMatchPatternField],
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter {
                expected: ResolvedType,
                record: &'a DeclarationId,
                instance: &'a ResolvedType,
                fields: &'a [ResolvedRecordMatchPatternField],
                path: String,
            },
            Fields {
                expected: ResolvedType,
                record: &'a DeclarationId,
                fields: &'a [ResolvedRecordMatchPatternField],
                declared_fields: &'a [ResolvedFieldDeclaration],
                index: usize,
                seen: BTreeSet<DeclarationId>,
                path: String,
            },
        }
        let mut frames = vec![Frame::Enter {
            expected: expected.clone(),
            record,
            instance,
            fields,
            path: path.to_owned(),
        }];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter {
                    expected,
                    record,
                    instance,
                    fields,
                    path,
                } => {
                    if instance != &expected {
                        return Err(hir_error(
                            "resolved record pattern has the wrong concrete instance",
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments: _,
                    } = &expected
                    else {
                        return Err(hir_error("resolved record pattern instance is not nominal"));
                    };
                    if declaration != record
                        || self
                            .program
                            .declarations
                            .declaration(record)
                            .is_none_or(|item| item.kind != DeclarationKind::Record)
                    {
                        return Err(hir_error(
                            "resolved record pattern references a foreign record",
                        ));
                    }
                    let facts =
                        self.program
                            .declarations
                            .type_facts(&expected)
                            .ok_or_else(|| {
                                hir_error("resolved record pattern has no exact type facts")
                            })?;
                    if !facts.copy || facts.contains_resource || facts.needs_drop {
                        return Err(hir_error("resolved record pattern is not Copy"));
                    }
                    let declared_fields = self
                        .program
                        .declarations
                        .record_fields(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?;
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        fields,
                        declared_fields,
                        index: 0,
                        seen: BTreeSet::new(),
                        path,
                    });
                }
                Frame::Fields {
                    expected,
                    record,
                    fields,
                    declared_fields,
                    index,
                    mut seen,
                    path,
                } => {
                    let Some(field) = fields.get(index) else {
                        if seen.len() != declared_fields.len() {
                            return Err(hir_error("resolved record pattern is missing fields"));
                        }
                        continue;
                    };
                    let declared = declared_fields
                        .iter()
                        .find(|candidate| candidate.id == field.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resolved record pattern contains foreign field `{}`",
                                field.field
                            ))
                        })?;
                    if !seen.insert(field.field.clone()) {
                        return Err(hir_error(
                            "resolved record pattern contains a duplicate field",
                        ));
                    }
                    let ResolvedType::Nominal { arguments, .. } = &expected else {
                        unreachable!("validated record instance remains nominal")
                    };
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    let field_path = format!("{path}.field.{index}");
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        fields,
                        declared_fields,
                        index: index + 1,
                        seen,
                        path,
                    });
                    match &field.pattern {
                        ResolvedRecordMatchFieldPattern::Binding(binding) => {
                            if binding.id
                                != ValueId::local(function, &format!("{field_path}.binding"))
                                || binding.ty != field_ty
                                || binding.ownership != OwnershipMode::Value
                            {
                                return Err(hir_error(
                                    "resolved record pattern binding is not canonical",
                                ));
                            }
                            self.insert_value(&binding.id)?;
                            self.validate_type(&binding.ty)?;
                            if scope.contains_key(&binding.id) {
                                return Err(hir_error(
                                    "resolved record pattern binding shadows an existing value",
                                ));
                            }
                            scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    ty: binding.ty.clone(),
                                    ownership: OwnershipMode::Value,
                                    availability: Availability::Available,
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            );
                        }
                        ResolvedRecordMatchFieldPattern::Wildcard => {}
                        ResolvedRecordMatchFieldPattern::Record {
                            record,
                            instance,
                            fields,
                        } => frames.push(Frame::Enter {
                            expected: field_ty,
                            record,
                            instance,
                            fields,
                            path: format!("{field_path}.record"),
                        }),
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_expr(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        self.validate_expr_iterative(
            function,
            expression,
            scope,
            path,
            allow_moves,
            allowed_effects,
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn assert_validation_oracle(
        iterative: &Result<(), Diagnostic>,
        recursive: &Result<(), Diagnostic>,
        iterative_validator: &Self,
        recursive_validator: &Self,
        iterative_scope: &BTreeMap<ValueId, ValidationBinding>,
        recursive_scope: &BTreeMap<ValueId, ValidationBinding>,
        path: &str,
    ) {
        match (iterative, recursive) {
            (Ok(()), Ok(())) => {}
            (Err(left), Err(right)) => {
                assert_eq!(left.code, right.code, "validator code differs at {path}");
                assert_eq!(
                    left.severity, right.severity,
                    "validator severity differs at {path}"
                );
                assert_eq!(
                    left.message, right.message,
                    "validator message differs at {path}"
                );
                assert_eq!(left.path, right.path, "validator path differs at {path}");
                assert_eq!(left.span, right.span, "validator span differs at {path}");
                assert_eq!(left.help, right.help, "validator help differs at {path}");
            }
            outcomes => panic!("validator outcomes differ at {path}: {outcomes:?}"),
        }
        assert_eq!(
            iterative_validator.expression_ids, recursive_validator.expression_ids,
            "validator expression IDs differ at {path}"
        );
        assert_eq!(
            iterative_validator.value_ids, recursive_validator.value_ids,
            "validator value IDs differ at {path}"
        );
        assert_eq!(
            iterative_scope, recursive_scope,
            "validator scope differs at {path}"
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_expr_iterative(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        enum Frame<'e> {
            RestorePublication(bool),
            Enter {
                expression: &'e ResolvedExpr,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            Unary {
                expression: &'e ResolvedExpr,
                op: UnaryOp,
            },
            BinaryLeft {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                right: &'e ResolvedExpr,
                path: String,
            },
            BinaryRight {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                left: &'e ResolvedExpr,
                baseline: Option<(Vec<ValueId>, BTreeMap<ValueId, ValidationBinding>)>,
            },
            IfCondition {
                expression: &'e ResolvedExpr,
                then_branch: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: String,
            },
            IfThen {
                expression: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: String,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            IfElse {
                expression: &'e ResolvedExpr,
                then_scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            Project {
                expression: &'e ResolvedExpr,
                field: &'e DeclarationId,
            },
            Upcast {
                expression: &'e ResolvedExpr,
            },
            Try {
                expression: &'e ResolvedExpr,
                path: String,
                option: bool,
            },
            CallNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: Vec<ResolvedParam>,
                return_type: ResolvedType,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            CallAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: Vec<ResolvedParam>,
                return_type: ResolvedType,
                index: usize,
                path: String,
            },
            NativeNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: Vec<ResolvedImportParameter>,
                result: ResolvedType,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            NativeAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: Vec<ResolvedImportParameter>,
                result: ResolvedType,
                index: usize,
                path: String,
            },
            BlockNext {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
            },
            BlockAfterLet {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
            },
            BlockAfterAssign {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
            },
            BlockAfterUnsafe {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
            },
            BlockAfterWhileCondition {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
                body: &'e ResolvedExpr,
            },
            BlockAfterWhileBody {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: String,
                entry: BTreeMap<ValueId, ValidationBinding>,
            },
            BlockTail {
                expression: &'e ResolvedExpr,
                outer_ids: Vec<ValueId>,
                outer: BTreeMap<ValueId, ValidationBinding>,
            },
            RecordNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            RecordAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: String,
            },
            VariantNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                variant: DeclarationId,
                case: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            VariantAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                variant: DeclarationId,
                case: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: String,
            },
            UpdateBase {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [ResolvedFieldInitializer],
                path: String,
            },
            UpdateNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
                ownership: OwnershipMode,
            },
            UpdateAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: Vec<ResolvedFieldDeclaration>,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: String,
                ownership: OwnershipMode,
            },
            MatchScrutinee {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                path: String,
            },
            RecordMatchArm {
                expression: &'e ResolvedExpr,
                arm: &'e ResolvedMatchArm,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            VariantMatchNext {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                cases: Vec<ResolvedVariantCaseDeclaration>,
                variant: DeclarationId,
                arguments: Vec<ResolvedType>,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: String,
            },
            VariantMatchAfterArm {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                cases: Vec<ResolvedVariantCaseDeclaration>,
                variant: DeclarationId,
                arguments: Vec<ResolvedType>,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: String,
            },
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == 288) };
        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame<'_>) -> usize {
            let ids = |values: &Vec<ValueId>| {
                values.capacity() * std::mem::size_of::<ValueId>()
                    + values.iter().map(|id| id.as_str().len()).sum::<usize>()
            };
            let types = |values: &Vec<ResolvedType>| {
                values.capacity() * std::mem::size_of::<ResolvedType>()
                    + values
                        .iter()
                        .map(resolved_type_owned_capacity)
                        .sum::<usize>()
            };
            let scope = |scope: &BTreeMap<ValueId, ValidationBinding>| {
                validation_scope_owned_capacity(scope)
            };
            let path = match frame {
                Frame::Enter { path, .. }
                | Frame::BinaryLeft { path, .. }
                | Frame::IfCondition { path, .. }
                | Frame::IfThen { path, .. }
                | Frame::Try { path, .. }
                | Frame::CallNext { path, .. }
                | Frame::CallAfterArg { path, .. }
                | Frame::NativeNext { path, .. }
                | Frame::NativeAfterArg { path, .. }
                | Frame::BlockNext { path, .. }
                | Frame::BlockAfterLet { path, .. }
                | Frame::BlockAfterAssign { path, .. }
                | Frame::BlockAfterUnsafe { path, .. }
                | Frame::BlockAfterWhileCondition { path, .. }
                | Frame::BlockAfterWhileBody { path, .. }
                | Frame::RecordNext { path, .. }
                | Frame::RecordAfterField { path, .. }
                | Frame::VariantNext { path, .. }
                | Frame::VariantAfterField { path, .. }
                | Frame::UpdateBase { path, .. }
                | Frame::UpdateNext { path, .. }
                | Frame::UpdateAfterField { path, .. }
                | Frame::MatchScrutinee { path, .. }
                | Frame::VariantMatchNext { path, .. }
                | Frame::VariantMatchAfterArm { path, .. } => path.capacity(),
                _ => 0,
            };
            let retained = match frame {
                Frame::Enter { scope: value, .. } => scope(value),
                Frame::BinaryRight { baseline, .. } => baseline
                    .as_ref()
                    .map_or(0, |(outer_ids, value)| ids(outer_ids) + scope(value)),
                Frame::IfThen {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::IfElse {
                    then_scope,
                    outer,
                    outer_ids,
                    ..
                } => scope(then_scope) + scope(outer) + ids(outer_ids),
                Frame::CallNext {
                    params,
                    return_type,
                    scope: value,
                    ..
                } => {
                    params.capacity() * std::mem::size_of::<ResolvedParam>()
                        + params
                            .iter()
                            .map(|param| {
                                param.id.as_str().len()
                                    + param.name.capacity()
                                    + resolved_type_owned_capacity(&param.ty)
                            })
                            .sum::<usize>()
                        + resolved_type_owned_capacity(return_type)
                        + scope(value)
                }
                Frame::CallAfterArg {
                    params,
                    return_type,
                    ..
                } => {
                    params.capacity() * std::mem::size_of::<ResolvedParam>()
                        + params
                            .iter()
                            .map(|param| {
                                param.id.as_str().len()
                                    + param.name.capacity()
                                    + resolved_type_owned_capacity(&param.ty)
                            })
                            .sum::<usize>()
                        + resolved_type_owned_capacity(return_type)
                }
                Frame::NativeNext {
                    params,
                    result,
                    scope: value,
                    ..
                } => {
                    params.capacity() * std::mem::size_of::<ResolvedImportParameter>()
                        + params
                            .iter()
                            .map(|param| {
                                param.name.capacity() + resolved_type_owned_capacity(&param.ty)
                            })
                            .sum::<usize>()
                        + resolved_type_owned_capacity(result)
                        + scope(value)
                }
                Frame::NativeAfterArg { params, result, .. } => {
                    params.capacity() * std::mem::size_of::<ResolvedImportParameter>()
                        + params
                            .iter()
                            .map(|param| {
                                param.name.capacity() + resolved_type_owned_capacity(&param.ty)
                            })
                            .sum::<usize>()
                        + resolved_type_owned_capacity(result)
                }
                Frame::BlockNext {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                }
                | Frame::BlockAfterAssign {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                } => scope(value) + scope(outer) + ids(outer_ids),
                Frame::BlockAfterLet {
                    outer, outer_ids, ..
                }
                | Frame::BlockTail {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::RecordNext {
                    expected,
                    arguments,
                    seen,
                    scope: value,
                    ..
                }
                | Frame::VariantNext {
                    expected,
                    arguments,
                    seen,
                    scope: value,
                    ..
                }
                | Frame::UpdateNext {
                    expected,
                    arguments,
                    seen,
                    scope: value,
                    ..
                } => {
                    expected.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
                        + expected
                            .iter()
                            .map(resolved_field_declaration_owned_capacity)
                            .sum::<usize>()
                        + types(arguments)
                        + seen.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + scope(value)
                }
                Frame::RecordAfterField {
                    expected,
                    arguments,
                    seen,
                    ..
                }
                | Frame::VariantAfterField {
                    expected,
                    arguments,
                    seen,
                    ..
                }
                | Frame::UpdateAfterField {
                    expected,
                    arguments,
                    seen,
                    ..
                } => {
                    expected.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
                        + expected
                            .iter()
                            .map(resolved_field_declaration_owned_capacity)
                            .sum::<usize>()
                        + types(arguments)
                        + seen.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                Frame::RecordMatchArm {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::VariantMatchNext {
                    cases,
                    arguments,
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    result,
                    ..
                }
                | Frame::VariantMatchAfterArm {
                    cases,
                    arguments,
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    result,
                    ..
                } => {
                    cases.capacity() * std::mem::size_of::<ResolvedVariantCaseDeclaration>()
                        + cases
                            .iter()
                            .map(resolved_variant_case_owned_capacity)
                            .sum::<usize>()
                        + types(arguments)
                        + scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity()
                            * std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                        + covered.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + covered.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + result
                            .as_ref()
                            .map_or(0, |(ty, _)| resolved_type_owned_capacity(ty))
                }
                _ => 0,
            };
            path.saturating_add(retained)
        }
        let initial_scope = std::mem::take(scope);
        let mut publication = ValidationScopePublication {
            target: scope,
            published: initial_scope.clone(),
            enabled: true,
        };
        let mut frames = vec![Frame::Enter {
            expression,
            scope: initial_scope,
            path: path.to_owned(),
        }];
        let mut scopes = Vec::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                1,
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + scopes.capacity()
                        * std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>()
                    + scopes
                        .iter()
                        .map(validation_scope_owned_capacity)
                        .sum::<usize>()
                    + validation_scope_owned_capacity(&publication.published)
                    + frames.iter().map(frame_owned_capacity).sum::<usize>()
                    + frame_owned_capacity(&frame)
                    + self.expression_ids.len()
                        * (std::mem::size_of::<ExpressionId>()
                            + std::mem::size_of::<BTreeSet<ExpressionId>>())
                    + self
                        .expression_ids
                        .iter()
                        .map(|id| id.as_str().len())
                        .sum::<usize>()
                    + self.value_ids.len()
                        * (std::mem::size_of::<ValueId>()
                            + std::mem::size_of::<BTreeSet<ValueId>>())
                    + self
                        .value_ids
                        .iter()
                        .map(|id| id.as_str().len())
                        .sum::<usize>()
                    + self.functions.len()
                        * (std::mem::size_of::<(DeclarationId, &ResolvedFunction)>()
                            + std::mem::size_of::<BTreeMap<DeclarationId, &ResolvedFunction>>())
                    + self
                        .functions
                        .keys()
                        .map(|id| id.as_str().len())
                        .sum::<usize>(),
            );
            match frame {
                Frame::RestorePublication(enabled) => publication.enabled = enabled,
                Frame::Enter {
                    expression,
                    scope,
                    path,
                } => {
                    reject_nul_identity("resolved expression", expression.id.as_str())?;
                    if expression.id != ExpressionId::new(function, &path) {
                        return Err(hir_error(format!(
                            "expression `{}` has a non-canonical identity",
                            expression.id
                        )));
                    }
                    if !self.expression_ids.insert(expression.id.clone()) {
                        return Err(hir_error(format!(
                            "duplicate resolved expression identity `{}`",
                            expression.id
                        )));
                    }
                    self.validate_type(&expression.ty)?;
                    match &expression.kind {
                        ResolvedExprKind::Int(_) => {
                            self.finish_expr(expression, &ResolvedType::I64, OwnershipMode::Value)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Int32(_) => {
                            self.finish_expr(expression, &ResolvedType::I32, OwnershipMode::Value)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Char(value) => {
                            if char::from_u32(*value).is_none() {
                                return Err(hir_error(
                                    "char literal bits are not a Unicode scalar value",
                                ));
                            }
                            self.finish_expr(
                                expression,
                                &ResolvedType::Char,
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Uint8(_) => {
                            self.finish_expr(expression, &ResolvedType::U8, OwnershipMode::Value)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Float32(bits) => {
                            self.validate_finite_f32(*bits)?;
                            self.finish_expr(expression, &ResolvedType::F32, OwnershipMode::Value)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Float64(bits) => {
                            self.validate_finite_f64(*bits)?;
                            self.finish_expr(expression, &ResolvedType::F64, OwnershipMode::Value)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Bool(_) => {
                            self.finish_expr(
                                expression,
                                &ResolvedType::Bool,
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::String(_) => {
                            self.finish_expr(
                                expression,
                                &ResolvedType::String,
                                OwnershipMode::Own,
                            )?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Place(place) => {
                            let binding = scope.get(&place.root).ok_or_else(|| {
                                hir_error(format!(
                                    "resolved value `{}` is out of scope",
                                    place.root
                                ))
                            })?;
                            match (place.projections.is_empty(), binding.availability) {
                                (true, Availability::Available) => {
                                    match Self::place_availability(binding, &[]) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(hir_error(format!(
                                                "resolved value `{}` is partially moved",
                                                place.root
                                            )))
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(hir_error(format!(
                                                "resolved value `{}` may be partially moved",
                                                place.root
                                            )))
                                        }
                                    }
                                }
                                (true, Availability::Moved) => {
                                    return Err(hir_error(format!(
                                        "resolved value `{}` is used after it was moved",
                                        place.root
                                    )))
                                }
                                (true, Availability::MaybeMoved) => {
                                    return Err(hir_error(format!(
                                        "resolved value `{}` may have been moved",
                                        place.root
                                    )))
                                }
                                (false, _) => {
                                    match Self::place_availability(binding, &place.projections) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(hir_error(format!(
                                                "resolved place rooted at `{}` is partially moved",
                                                place.root
                                            )))
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(hir_error(format!(
                                        "resolved place rooted at `{}` may be conditionally moved",
                                        place.root
                                    )))
                                        }
                                    }
                                }
                            }
                            let (ty, ownership) = self.resolve_place(place, binding)?;
                            self.finish_expr(expression, &ty, ownership)?;
                            scopes.push(scope);
                        }
                        ResolvedExprKind::Unary { op, value } => {
                            frames.push(Frame::Unary {
                                expression,
                                op: *op,
                            });
                            frames.push(Frame::Enter {
                                expression: value,
                                scope,
                                path: format!("{path}.value"),
                            });
                        }
                        ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } => {
                            match instance {
                                None if !type_arguments.is_empty() => return Err(hir_error("monomorphic resolved call carries generic type arguments")),
                                Some(actual) if FunctionInstanceId::derive(callee, type_arguments) != *actual => return Err(hir_error("resolved call instance disagrees with its template and arguments")),
                                Some(_) if type_arguments.is_empty() => return Err(hir_error("generic resolved call has no concrete type arguments")),
                                None | Some(_) => {}
                            }
                            if type_arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            }) {
                                return Err(hir_error(
                                    "resolved call has a non-scalar generic type argument",
                                ));
                            }
                            let (params, return_type) = if let Some(op) =
                                crate::string_ops::by_id(callee.as_str())
                            {
                                // Compiler-owned string operations carry
                                // their reserved identity instead of an
                                // authored declaration; their synthetic
                                // parameters keep the ordinary argument
                                // ownership and transfer machinery.
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "string operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(hir_error(format!(
                                            "string operation `{}` expects {} arguments but received {}",
                                            op.name(),
                                            op.arity(),
                                            args.len()
                                        )));
                                }
                                (crate::string_ops::resolved_params(op), op.return_type())
                            } else {
                                let target = self
                                    .program
                                    .resolve_call_target(callee, instance.as_ref())
                                    .ok_or_else(|| {
                                        hir_error(format!(
                                            "resolved callee `{callee}` is not indexed"
                                        ))
                                    })?;
                                if args.len() != target.params.len() {
                                    return Err(hir_error(format!(
                                        "call to `{callee}` has {} arguments but expects {}",
                                        args.len(),
                                        target.params.len()
                                    )));
                                }
                                match allowed_effects {
                                    Some(allowed) => {
                                        for effect in &target.effects {
                                            if !allowed.contains(effect) {
                                                return Err(hir_error(format!("call to `{callee}` requires undeclared effect `{effect}`")));
                                            }
                                        }
                                    }
                                    None if !target.effects.is_empty() => {
                                        return Err(hir_error(format!(
                                            "contract calls effectful function `{callee}`"
                                        )))
                                    }
                                    None => {}
                                }
                                (target.params.clone(), target.return_type.clone())
                            };
                            frames.push(Frame::CallNext {
                                expression,
                                args,
                                params,
                                return_type,
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            if call.expression != expression.id {
                                return Err(hir_error("native Rust import call has a non-canonical expression identity"));
                            }
                            let import = self
                                .program
                                .interfaces
                                .iter()
                                .flat_map(|interface| &interface.imports)
                                .find(|import| import.id == call.import && import.native_rust)
                                .ok_or_else(|| {
                                    hir_error("native Rust import call has an unknown target")
                                })?;
                            if import.parameters.len() != call.args.len()
                                || import.result.kind != call.result
                            {
                                return Err(hir_error(
                                    "native Rust import call disagrees with its declaration",
                                ));
                            }
                            match allowed_effects {
                                Some(allowed)
                                    if import
                                        .effects
                                        .iter()
                                        .any(|effect| !allowed.contains(effect)) =>
                                {
                                    return Err(hir_error(
                                        "native Rust import call requires an undeclared effect",
                                    ))
                                }
                                None if !import.effects.is_empty() => {
                                    return Err(hir_error(
                                        "contract calls an effectful native Rust import",
                                    ))
                                }
                                _ => {}
                            }
                            let result = match call.result {
                                ResolvedImportResultKind::Unit => ResolvedType::Unit,
                                ResolvedImportResultKind::I64 => ResolvedType::I64,
                                ResolvedImportResultKind::Bool => ResolvedType::Bool,
                            };
                            frames.push(Frame::NativeNext {
                                expression,
                                args: &call.args,
                                params: import.parameters.clone(),
                                result,
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::Binary { op, left, right } => {
                            frames.push(Frame::BinaryLeft {
                                expression,
                                op: *op,
                                right,
                                path: path.clone(),
                            });
                            frames.push(Frame::Enter {
                                expression: left,
                                scope,
                                path: format!("{path}.left"),
                            });
                        }
                        ResolvedExprKind::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            frames.push(Frame::IfCondition {
                                expression,
                                then_branch,
                                else_branch,
                                path: path.clone(),
                            });
                            frames.push(Frame::Enter {
                                expression: condition,
                                scope,
                                path: format!("{path}.condition"),
                            });
                        }
                        ResolvedExprKind::Block { statements, tail } => {
                            let outer_ids = scope.keys().cloned().collect();
                            let outer = scope.clone();
                            frames.push(Frame::BlockNext {
                                expression,
                                statements,
                                tail,
                                index: 0,
                                scope,
                                outer,
                                outer_ids,
                                path,
                            });
                        }
                        ResolvedExprKind::ConstructRecord { record, fields } => {
                            let declaration = self
                                .program
                                .declarations
                                .declaration(record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` is not indexed"))
                                })?;
                            if !matches!(
                                declaration.kind,
                                DeclarationKind::Record | DeclarationKind::Class
                            ) {
                                return Err(hir_error(format!(
                                    "constructor target `{record}` is not a record or class"
                                )));
                            }
                            let expected = self
                                .program
                                .declarations
                                .record_fields(record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` has no fields"))
                                })?
                                .to_vec();
                            let ResolvedType::Nominal {
                                declaration: instance,
                                arguments,
                            } = &expression.ty
                            else {
                                return Err(hir_error("record constructor result is not nominal"));
                            };
                            let parameters = self
                                .program
                                .declarations
                                .type_parameters(record)
                                .ok_or_else(|| {
                                    hir_error(format!("record `{record}` has no parameters"))
                                })?;
                            if instance != record
                                || arguments.len() != parameters.len()
                                || arguments.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                })
                            {
                                return Err(hir_error(format!(
                                    "constructor for `{record}` has an invalid concrete instance"
                                )));
                            }
                            frames.push(Frame::RecordNext {
                                expression,
                                fields,
                                expected,
                                record: record.clone(),
                                arguments: arguments.clone(),
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::ConstructVariant {
                            variant,
                            case,
                            fields,
                        } => {
                            let ResolvedType::Nominal {
                                declaration: instance,
                                arguments,
                            } = &expression.ty
                            else {
                                return Err(hir_error(
                                    "variant constructor has a non-nominal result",
                                ));
                            };
                            if instance != variant {
                                return Err(hir_error(
                                    "variant constructor result disagrees with its declaration",
                                ));
                            }
                            let declaration =
                                self.program.declarations.declaration(variant).ok_or_else(
                                    || hir_error(format!("variant `{variant}` is not indexed")),
                                )?;
                            if declaration.kind != DeclarationKind::Variant {
                                return Err(hir_error(format!(
                                    "constructor target `{variant}` is not a variant"
                                )));
                            }
                            let expected = self.program.declarations.variant_cases(variant).and_then(|cases| cases.iter().find(|item| item.id == *case)).ok_or_else(|| hir_error(format!("constructor for `{variant}` contains foreign case `{case}`")))?.fields.clone();
                            frames.push(Frame::VariantNext {
                                expression,
                                fields,
                                expected,
                                variant: variant.clone(),
                                case: case.clone(),
                                arguments: arguments.clone(),
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            });
                        }
                        ResolvedExprKind::UpdateRecord {
                            base,
                            record,
                            fields,
                        } => {
                            frames.push(Frame::UpdateBase {
                                expression,
                                record,
                                fields,
                                path: path.clone(),
                            });
                            frames.push(Frame::Enter {
                                expression: base,
                                scope,
                                path: format!("{path}.base"),
                            });
                        }
                        ResolvedExprKind::Match { scrutinee, arms } => {
                            frames.push(Frame::MatchScrutinee {
                                expression,
                                arms,
                                path: path.clone(),
                            });
                            frames.push(Frame::Enter {
                                expression: scrutinee,
                                scope,
                                path: format!("{path}.scrutinee"),
                            });
                        }
                        ResolvedExprKind::Project { base, field } => {
                            if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                                return Err(hir_error(
                                    "place field projections must use a resolved place path",
                                ));
                            }
                            frames.push(Frame::Project { expression, field });
                            frames.push(Frame::Enter {
                                expression: base,
                                scope,
                                path: format!("{path}.base"),
                            });
                        }
                        // Class Inheritance v1: independently re-derive the
                        // upcast contract from the resolved source before the
                        // ancestor-typed result is accepted.
                        ResolvedExprKind::Upcast { source } => {
                            frames.push(Frame::Upcast { expression });
                            frames.push(Frame::Enter {
                                expression: source,
                                scope,
                                path: format!("{path}.source"),
                            });
                        }
                        ResolvedExprKind::Try { operand, .. } => {
                            frames.push(Frame::Try {
                                expression,
                                path: path.clone(),
                                option: false,
                            });
                            frames.push(Frame::Enter {
                                expression: operand,
                                scope,
                                path: format!("{path}.operand"),
                            });
                        }
                        ResolvedExprKind::TryOption { operand, .. } => {
                            frames.push(Frame::Try {
                                expression,
                                path: path.clone(),
                                option: true,
                            });
                            frames.push(Frame::Enter {
                                expression: operand,
                                scope,
                                path: format!("{path}.operand"),
                            });
                        }
                    }
                }
                Frame::Unary { expression, op } => {
                    let scope = scopes.pop().expect("unary scope retained");
                    let ResolvedExprKind::Unary { value, .. } = &expression.kind else {
                        unreachable!()
                    };
                    // Negation keeps a numeric operand type; i64 negation is
                    // checked while IEEE-754 negation is total.
                    if matches!(op, UnaryOp::Neg)
                        && !matches!(
                            &value.ty,
                            ResolvedType::I64
                                | ResolvedType::I32
                                | ResolvedType::F32
                                | ResolvedType::F64
                        )
                    {
                        return Err(hir_error("unary operand has inconsistent resolved types"));
                    }
                    let expected = match op {
                        UnaryOp::Neg => value.ty.clone(),
                        UnaryOp::Not => ResolvedType::Bool,
                    };
                    self.require_type(&value.ty, &expected, "unary operand")?;
                    self.finish_expr(expression, &expected, OwnershipMode::Value)?;
                    scopes.push(scope);
                }
                Frame::CallNext {
                    expression,
                    args,
                    params,
                    return_type,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        let ownership =
                            self.expected_ownership(&return_type, OwnershipMode::Own)?;
                        self.finish_expr(expression, &return_type, ownership)?;
                        scopes.push(scope);
                    } else {
                        frames.push(Frame::CallAfterArg {
                            expression,
                            args,
                            params,
                            return_type,
                            index,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: format!("{path}.arg.{index}"),
                        });
                    }
                }
                Frame::CallAfterArg {
                    expression,
                    args,
                    params,
                    return_type,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("call argument scope retained");
                    publication.publish(&scope);
                    let argument = &args[index];
                    let param = &params[index];
                    self.require_type(&argument.ty, &param.ty, "call argument")?;
                    self.validate_argument_ownership(argument.ownership, param)?;
                    if self.argument_transfers(param)? {
                        if !allow_moves {
                            let ResolvedExprKind::Call { callee, .. } = &expression.kind else {
                                unreachable!()
                            };
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, &mut scope)?;
                        publication.publish(&scope);
                    }
                    frames.push(Frame::CallNext {
                        expression,
                        args,
                        params,
                        return_type,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                Frame::NativeNext {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        self.finish_expr(expression, &result, OwnershipMode::Value)?;
                        scopes.push(scope);
                    } else {
                        frames.push(Frame::NativeAfterArg {
                            expression,
                            args,
                            params,
                            result,
                            index,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: format!("{path}.native-rust-arg.{index}"),
                        });
                    }
                }
                Frame::NativeAfterArg {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    path,
                } => {
                    let scope = scopes.pop().expect("native argument scope retained");
                    publication.publish(&scope);
                    let argument = &args[index];
                    let parameter = &params[index];
                    self.require_type(&argument.ty, &parameter.ty, "native Rust import argument")?;
                    if argument.ownership != OwnershipMode::Value
                        || parameter.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error(
                            "native Rust import arguments must use value ownership",
                        ));
                    }
                    frames.push(Frame::NativeNext {
                        expression,
                        args,
                        params,
                        result,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                Frame::BinaryLeft {
                    expression,
                    op,
                    right,
                    path,
                } => {
                    let left_scope = scopes.pop().expect("binary left scope retained");
                    publication.publish(&left_scope);
                    let baseline = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        Some((left_scope.keys().cloned().collect(), left_scope.clone()))
                    } else {
                        None
                    };
                    frames.push(Frame::BinaryRight {
                        expression,
                        op,
                        left: match &expression.kind {
                            ResolvedExprKind::Binary { left, .. } => left,
                            _ => unreachable!(),
                        },
                        baseline,
                    });
                    if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled));
                    }
                    frames.push(Frame::Enter {
                        expression: right,
                        scope: left_scope,
                        path: format!("{path}.right"),
                    });
                }
                Frame::BinaryRight {
                    expression,
                    op,
                    left,
                    baseline,
                } => {
                    let mut scope = scopes.pop().expect("binary right scope retained");
                    let direct = baseline.is_none();
                    if let Some((ids, mut parent)) = baseline {
                        Self::join_conditional(&mut parent, &scope, &ids);
                        scope = parent;
                    }
                    if direct || matches!(op, BinaryOp::And | BinaryOp::Or) {
                        publication.publish(&scope);
                    }
                    let ResolvedExprKind::Binary { right, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let output = match op {
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem => {
                            // Arithmetic keeps the operand type: i64 and u8
                            // arithmetic is checked, IEEE-754 float
                            // arithmetic is total and never selects a status.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::U8
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                            ) || (matches!(op, BinaryOp::Rem) && left.ty != ResolvedType::I64)
                            {
                                return Err(hir_error(
                                    "binary operand has inconsistent resolved types",
                                ));
                            }
                            self.require_type(&left.ty, &right.ty, "binary operand")?;
                            left.ty.clone()
                        }
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                            // Ordered comparison compares scalar values: i64,
                            // IEEE-754 numerics, and Unicode scalar chars.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::Char
                                    | ResolvedType::U8
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                            ) {
                                return Err(hir_error(
                                    "comparison operand has inconsistent resolved types",
                                ));
                            }
                            self.require_type(&left.ty, &right.ty, "comparison operand")?;
                            ResolvedType::Bool
                        }
                        BinaryOp::And | BinaryOp::Or => {
                            self.require_type(&left.ty, &ResolvedType::Bool, "boolean operand")?;
                            self.require_type(&right.ty, &ResolvedType::Bool, "boolean operand")?;
                            ResolvedType::Bool
                        }
                        BinaryOp::Eq | BinaryOp::Ne => {
                            self.require_type(&left.ty, &right.ty, "equality operands")?;
                            ResolvedType::Bool
                        }
                    };
                    self.finish_expr(expression, &output, OwnershipMode::Value)?;
                    scopes.push(scope);
                }
                Frame::IfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    path,
                } => {
                    let outer = scopes.pop().expect("if condition scope retained");
                    publication.publish(&outer);
                    let ResolvedExprKind::If { condition, .. } = &expression.kind else {
                        unreachable!()
                    };
                    self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                    let outer_ids = outer.keys().cloned().collect();
                    frames.push(Frame::IfThen {
                        expression,
                        else_branch,
                        path: path.clone(),
                        outer: outer.clone(),
                        outer_ids,
                    });
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled));
                    frames.push(Frame::Enter {
                        expression: then_branch,
                        scope: outer,
                        path: format!("{path}.then"),
                    });
                }
                Frame::IfThen {
                    expression,
                    else_branch,
                    path,
                    outer,
                    outer_ids,
                } => {
                    let then_scope = scopes.pop().expect("if then scope retained");
                    frames.push(Frame::IfElse {
                        expression,
                        then_scope,
                        outer: outer.clone(),
                        outer_ids,
                    });
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled));
                    frames.push(Frame::Enter {
                        expression: else_branch,
                        scope: outer,
                        path: format!("{path}.else"),
                    });
                }
                Frame::IfElse {
                    expression,
                    then_scope,
                    mut outer,
                    outer_ids,
                } => {
                    let else_scope = scopes.pop().expect("if else scope retained");
                    Self::join_branches(&mut outer, &then_scope, &else_scope, &outer_ids);
                    publication.publish(&outer);
                    let ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    self.require_type(&then_branch.ty, &else_branch.ty, "if branches")?;
                    if then_branch.ownership != else_branch.ownership {
                        return Err(hir_error("if branches have inconsistent ownership"));
                    }
                    self.finish_expr(expression, &then_branch.ty, then_branch.ownership)?;
                    scopes.push(outer);
                }
                Frame::BlockNext {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    if index == statements.len() {
                        frames.push(Frame::BlockTail {
                            expression,
                            outer_ids,
                            outer,
                        });
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled));
                        frames.push(Frame::Enter {
                            expression: tail,
                            scope,
                            path: format!("{path}.tail"),
                        });
                    } else {
                        match &statements[index] {
                            ResolvedStatement::Let { value, .. } => {
                                frames.push(Frame::BlockAfterLet {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path: path.clone(),
                                });
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled));
                                frames.push(Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            ResolvedStatement::Assign { binding, value, .. } => {
                                // The target must be a previously declared
                                // mutable scalar binding in this block's scope.
                                if !scope.contains_key(&binding.id) {
                                    return Err(hir_error(format!(
                                        "assignment target `{}` has no enclosing binding",
                                        binding.id
                                    )));
                                }
                                let target_scope = scope.clone();
                                frames.push(Frame::BlockAfterAssign {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    scope: target_scope,
                                    outer,
                                    outer_ids,
                                    path: path.clone(),
                                });
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled));
                                frames.push(Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            ResolvedStatement::Unsafe { body, .. } => {
                                // Contract expressions stay pure; ordinary
                                // blocks resolve the body like any nested
                                // block and bind nothing outside it.
                                if !allow_moves {
                                    return Err(hir_error(
                                        "contract expressions cannot contain unsafe boundary statements",
                                    ));
                                }
                                frames.push(Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path: path.clone(),
                                });
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled));
                                frames.push(Frame::Enter {
                                    expression: body,
                                    scope,
                                    path: format!("{path}.s{index}.body"),
                                });
                            }
                            ResolvedStatement::While {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: re-check admission
                                // at the trust boundary and require an exact
                                // `bool` condition before validating the body
                                // like any nested block.
                                self.validate_while_admission(condition)?;
                                self.validate_while_admission(body)?;
                                if condition.ty != ResolvedType::Bool {
                                    return Err(hir_error("`while` condition must be bool"));
                                }
                                frames.push(Frame::BlockAfterWhileCondition {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path: path.clone(),
                                    body,
                                });
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled));
                                frames.push(Frame::Enter {
                                    expression: condition,
                                    scope,
                                    path: format!("{path}.s{index}.condition"),
                                });
                            }
                        }
                    }
                }
                Frame::BlockAfterUnsafe {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    // The body block validated like any nested block; its
                    // merged scope continues into the enclosing block.
                    let scope = scopes.pop().expect("unsafe body scope retained");
                    let ResolvedStatement::Unsafe { body, .. } = &statements[index] else {
                        unreachable!("unsafe frame resumes at an unsafe statement")
                    };
                    if body.ownership != OwnershipMode::Value
                        || !crate::hir::is_scalar_resolved_type(&body.ty)
                    {
                        return Err(hir_error(
                            "unsafe boundary bodies must produce a scalar Copy value",
                        ));
                    }
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                Frame::BlockAfterWhileCondition {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    body,
                } => {
                    // The condition validated like any expression; it must be
                    // exactly `bool` because it re-evaluates before every
                    // iteration.
                    let scope = scopes.pop().expect("while condition scope retained");
                    let ResolvedStatement::While { condition, .. } = &statements[index] else {
                        unreachable!("while condition frame resumes at a while statement")
                    };
                    if condition.ty != ResolvedType::Bool {
                        return Err(hir_error("`while` condition must be bool"));
                    }
                    frames.push(Frame::BlockAfterWhileBody {
                        expression,
                        statements,
                        tail,
                        index,
                        outer,
                        outer_ids,
                        path: path.clone(),
                        entry: scope.clone(),
                    });
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled));
                    frames.push(Frame::Enter {
                        expression: body,
                        scope,
                        path: format!("{path}.s{index}.body"),
                    });
                }
                Frame::BlockAfterWhileBody {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    entry,
                } => {
                    // The body block validated like any nested block. Because
                    // zero or more iterations run, its merged ownership state
                    // must equal the loop-entry state exactly; admission keeps
                    // every loop binding Copy-scalar so nothing can drift.
                    let after = scopes.pop().expect("while body scope retained");
                    if after != entry {
                        return Err(hir_error("while loop body changes ownership liveness"));
                    }
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope: entry,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                Frame::BlockAfterLet {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("block let scope retained");
                    let ResolvedStatement::Let { binding, value, .. } = &statements[index] else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = format!("{path}.s{index}");
                    if binding.id != ValueId::local(function, &statement_path) {
                        return Err(hir_error(format!(
                            "local `{}` has a non-canonical identity",
                            binding.id
                        )));
                    }
                    self.insert_value(&binding.id)?;
                    self.require_type(&binding.ty, &value.ty, "local binding")?;
                    if binding.ownership != value.ownership {
                        return Err(hir_error(format!(
                            "local `{}` has inconsistent ownership",
                            binding.id
                        )));
                    }
                    self.validate_declared_ownership(&binding.ty, binding.ownership)?;
                    if self.is_owned_resource(&binding.ty, binding.ownership)? {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a local binding",
                            ));
                        }
                        self.mark_value_sources_moved(value, &mut scope)?;
                    }
                    scope.insert(
                        binding.id.clone(),
                        ValidationBinding {
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            availability: Availability::Available,
                            moved_places: BTreeMap::new(),
                            definitely_partial: BTreeSet::new(),
                        },
                    );
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                Frame::BlockAfterAssign {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    scopes.pop().expect("block assign scope retained");
                    let ResolvedStatement::Assign {
                        binding,
                        field,
                        value: assigned,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    let target = scope.get(&binding.id).cloned();
                    let Some(target) = target else {
                        return Err(hir_error(format!(
                            "assignment target `{}` has no enclosing binding",
                            binding.id
                        )));
                    };
                    match field {
                        Some(field) => {
                            if target.ownership != OwnershipMode::Value {
                                return Err(hir_error(
                                    "field assignment base is not a value-owned aggregate",
                                ));
                            }
                            self.validate_assign_field(&target.ty, field, assigned)?;
                        }
                        None => {
                            self.require_type(&target.ty, &assigned.ty, "assignment")?;
                            if target.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&target.ty)
                            {
                                return Err(hir_error(
                                    "explicit mutation v1 supports only scalar Copy values",
                                ));
                            }
                        }
                    }
                    if target.availability != Availability::Available {
                        return Err(hir_error(format!(
                            "assignment target `{}` is not available",
                            binding.id
                        )));
                    }
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    });
                }
                Frame::BlockTail {
                    expression,
                    outer_ids,
                    mut outer,
                } => {
                    let block_scope = scopes.pop().expect("block tail scope retained");
                    Self::merge_availability(&mut outer, &block_scope, &outer_ids);
                    publication.publish(&outer);
                    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
                        unreachable!()
                    };
                    self.finish_expr(expression, &tail.ty, tail.ownership)?;
                    scopes.push(outer);
                }
                Frame::RecordNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(hir_error(format!(
                                "constructor for `{record}` is missing required fields"
                            )));
                        }
                        let ownership =
                            self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        self.finish_expr(expression, &expression.ty, ownership)?;
                        scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "constructor for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push(Frame::RecordAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::RecordAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("record field scope retained");
                    publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, &record, &arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let ownership = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(hir_error(format!(
                            "field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope);
                    }
                    frames.push(Frame::RecordNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                Frame::VariantNext {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(hir_error(format!(
                                "constructor for `{case}` is missing required payload fields"
                            )));
                        }
                        self.finish_expr(expression, &expression.ty, OwnershipMode::Value)?;
                        scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "constructor for `{case}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push(Frame::VariantAfterField {
                            expression,
                            fields,
                            expected,
                            variant,
                            case,
                            arguments,
                            seen,
                            index,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::VariantAfterField {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let scope = scopes.pop().expect("variant field scope retained");
                    publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("variant field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, &variant, &arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "variant payload field")?;
                    if initializer.value.ownership != OwnershipMode::Value {
                        return Err(hir_error(format!(
                            "variant payload field `{}` is not a Copy value",
                            initializer.field
                        )));
                    }
                    frames.push(Frame::VariantNext {
                        expression,
                        fields,
                        expected,
                        variant,
                        case,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                Frame::UpdateBase {
                    expression,
                    record,
                    fields,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("update base scope retained");
                    publication.publish(&scope);
                    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let declaration = self
                        .program
                        .declarations
                        .declaration(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                    if declaration.kind != DeclarationKind::Record {
                        return Err(hir_error(format!(
                            "record update target `{record}` is not a record"
                        )));
                    }
                    let ResolvedType::Nominal {
                        declaration: instance,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(hir_error("record update base is not nominal"));
                    };
                    let parameters = self
                        .program
                        .declarations
                        .type_parameters(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                    if instance != record
                        || arguments.len() != parameters.len()
                        || arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        })
                    {
                        return Err(hir_error(format!(
                            "record update for `{record}` has an invalid concrete instance"
                        )));
                    }
                    let ownership = self.expected_ownership(&base.ty, OwnershipMode::Own)?;
                    if base.ownership != ownership {
                        return Err(hir_error(format!(
                            "record update base for `{record}` has incompatible ownership"
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership from a record update base",
                            ));
                        }
                        self.mark_value_sources_moved(base, &mut scope)?;
                        publication.publish(&scope);
                    }
                    let expected = self
                        .program
                        .declarations
                        .record_fields(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?
                        .to_vec();
                    frames.push(Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record: record.clone(),
                        arguments: arguments.clone(),
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                        ownership,
                    });
                }
                Frame::UpdateNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                    ownership,
                } => {
                    if index == fields.len() {
                        let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                            unreachable!()
                        };
                        self.finish_expr(expression, &base.ty, ownership)?;
                        scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "update for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push(Frame::UpdateAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path: path.clone(),
                            ownership,
                        });
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::UpdateAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                    ownership,
                } => {
                    let mut scope = scopes.pop().expect("update field scope retained");
                    publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("update field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, &record, &arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record replacement")?;
                    let expected_ownership =
                        self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected_ownership {
                        return Err(hir_error(format!(
                            "replacement field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected_ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record replacement",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope);
                    }
                    frames.push(Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                        ownership,
                    });
                }
                Frame::MatchScrutinee {
                    expression,
                    arms,
                    path,
                } => {
                    let outer = scopes.pop().expect("match scrutinee scope retained");
                    publication.publish(&outer);
                    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let ResolvedType::Nominal {
                        declaration: matched,
                        arguments,
                    } = &scrutinee.ty
                    else {
                        return Err(hir_error("resolved match scrutinee is not nominal"));
                    };
                    let kind = self
                        .program
                        .declarations
                        .declaration(matched)
                        .map(|item| item.kind);
                    let outer_ids = outer.keys().cloned().collect::<Vec<_>>();
                    if kind == Some(DeclarationKind::Record) {
                        if scrutinee.ownership != OwnershipMode::Value {
                            return Err(hir_error("resolved record match scrutinee is not Copy"));
                        }
                        let [arm] = arms else {
                            return Err(hir_error(
                                "resolved irrefutable record match must have exactly one arm",
                            ));
                        };
                        let mut arm_scope = outer.clone();
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {}
                            ResolvedMatchPattern::Record {
                                record,
                                instance,
                                fields,
                            } => self.validate_record_match_pattern(
                                function,
                                &scrutinee.ty,
                                record,
                                instance,
                                fields,
                                &mut arm_scope,
                                &format!("{path}.arm.0.record"),
                            )?,
                            ResolvedMatchPattern::Variant { .. } => {
                                return Err(hir_error(
                                    "resolved variant pattern has a record scrutinee",
                                ))
                            }
                        }
                        frames.push(Frame::RecordMatchArm {
                            expression,
                            arm,
                            outer,
                            outer_ids,
                        });
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled));
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: format!("{path}.arm.0.value"),
                        });
                    } else {
                        if scrutinee.ownership != OwnershipMode::Value
                            || kind != Some(DeclarationKind::Variant)
                        {
                            return Err(hir_error(
                                "resolved match scrutinee is not a concrete Copy variant",
                            ));
                        }
                        let cases = self
                            .program
                            .declarations
                            .variant_cases(matched)
                            .ok_or_else(|| hir_error(format!("variant `{matched}` has no cases")))?
                            .to_vec();
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        frames.push(Frame::VariantMatchNext {
                            expression,
                            arms,
                            cases,
                            variant: matched.clone(),
                            arguments: arguments.clone(),
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: Vec::with_capacity(arms.len()),
                            covered: BTreeSet::new(),
                            wildcard_seen: false,
                            result: None,
                            path,
                        });
                    }
                }
                Frame::RecordMatchArm {
                    expression,
                    arm,
                    mut outer,
                    outer_ids,
                } => {
                    let arm_scope = scopes.pop().expect("record match arm scope retained");
                    if !matches!(arm.value.ty, ResolvedType::I64 | ResolvedType::Bool) {
                        return Err(hir_error(
                            "resolved record match arm must produce i64 or bool",
                        ));
                    }
                    for id in outer_ids {
                        if let Some(state) = arm_scope.get(&id) {
                            outer.insert(id, state.clone());
                        }
                    }
                    publication.publish(&outer);
                    self.finish_expr(expression, &arm.value.ty, arm.value.ownership)?;
                    scopes.push(outer);
                }
                Frame::VariantMatchNext {
                    expression,
                    arms,
                    cases,
                    variant,
                    arguments,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    mut covered,
                    mut wildcard_seen,
                    result,
                    path,
                } => {
                    if index == arms.len() {
                        if !wildcard_seen && covered.len() != cases.len() {
                            return Err(hir_error("resolved match is not exhaustive"));
                        }
                        let (ty, ownership) =
                            result.ok_or_else(|| hir_error("resolved match has no result"))?;
                        let mut final_scope = outer;
                        if let Some((first, rest)) = arm_scopes.split_first() {
                            let mut joined = first.clone();
                            for arm_scope in rest {
                                Self::join_conditional(&mut joined, arm_scope, &outer_ids);
                            }
                            Self::merge_availability(&mut final_scope, &joined, &outer_ids);
                        }
                        publication.publish(&final_scope);
                        self.finish_expr(expression, &ty, ownership)?;
                        scopes.push(final_scope);
                    } else {
                        let arm = &arms[index];
                        let mut arm_scope = outer.clone();
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {
                                if wildcard_seen || covered.len() == cases.len() {
                                    return Err(hir_error(
                                        "resolved match has an unreachable wildcard",
                                    ));
                                }
                                wildcard_seen = true;
                            }
                            ResolvedMatchPattern::Variant {
                                variant: pattern_variant,
                                case,
                                fields,
                            } => {
                                if wildcard_seen
                                    || pattern_variant != &variant
                                    || !covered.insert(case.clone())
                                {
                                    return Err(hir_error(
                                        "resolved match has an unreachable or foreign case pattern",
                                    ));
                                }
                                let declared_case = cases
                                    .iter()
                                    .find(|item| item.id == *case)
                                    .ok_or_else(|| {
                                        hir_error(format!(
                                            "resolved match references foreign case `{case}`"
                                        ))
                                    })?;
                                let mut seen = BTreeSet::new();
                                for (field_index, field) in fields.iter().enumerate() {
                                    let declared = declared_case
                                        .fields
                                        .iter()
                                        .find(|item| item.id == field.field)
                                        .ok_or_else(|| {
                                            hir_error(format!(
                                                "resolved pattern contains foreign field `{}`",
                                                field.field
                                            ))
                                        })?;
                                    let binding_ty =
                                        substitute_type(&declared.ty, &variant, &arguments)?;
                                    if !seen.insert(field.field.clone())
                                        || field.binding.id
                                            != ValueId::local(
                                                function,
                                                &format!(
                                                    "{path}.arm.{index}.binding.{field_index}"
                                                ),
                                            )
                                        || field.binding.ty != binding_ty
                                        || field.binding.ownership != OwnershipMode::Value
                                    {
                                        return Err(hir_error(
                                            "resolved match pattern field or binding is invalid",
                                        ));
                                    }
                                    self.insert_value(&field.binding.id)?;
                                    self.validate_type(&field.binding.ty)?;
                                    if arm_scope.contains_key(&field.binding.id) {
                                        return Err(hir_error("resolved match pattern binding shadows an existing value"));
                                    }
                                    arm_scope.insert(
                                        field.binding.id.clone(),
                                        ValidationBinding {
                                            ty: field.binding.ty.clone(),
                                            ownership: OwnershipMode::Value,
                                            availability: Availability::Available,
                                            moved_places: BTreeMap::new(),
                                            definitely_partial: BTreeSet::new(),
                                        },
                                    );
                                }
                                if seen.len() != declared_case.fields.len() {
                                    return Err(hir_error(
                                        "resolved match pattern is missing payload fields",
                                    ));
                                }
                            }
                            ResolvedMatchPattern::Record { .. } => {
                                return Err(hir_error(
                                    "resolved record pattern has a variant scrutinee",
                                ))
                            }
                        }
                        frames.push(Frame::VariantMatchAfterArm {
                            expression,
                            arms,
                            cases,
                            variant,
                            arguments,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            covered,
                            wildcard_seen,
                            result,
                            path: path.clone(),
                        });
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled));
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                Frame::VariantMatchAfterArm {
                    expression,
                    arms,
                    cases,
                    variant,
                    arguments,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    covered,
                    wildcard_seen,
                    mut result,
                    path,
                } => {
                    let arm_scope = scopes.pop().expect("variant match arm scope retained");
                    let arm = &arms[index];
                    if let Some((ty, ownership)) = &result {
                        self.require_type(&arm.value.ty, ty, "match arm")?;
                        if arm.value.ownership != *ownership {
                            return Err(hir_error(
                                "resolved match arms have inconsistent ownership",
                            ));
                        }
                    } else {
                        result = Some((arm.value.ty.clone(), arm.value.ownership));
                    }
                    arm_scopes.push(arm_scope);
                    frames.push(Frame::VariantMatchNext {
                        expression,
                        arms,
                        cases,
                        variant,
                        arguments,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        covered,
                        wildcard_seen,
                        result,
                        path,
                    });
                }
                Frame::Project { expression, field } => {
                    let scope = scopes.pop().expect("projection scope retained");
                    let ResolvedExprKind::Project { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let projected = self.field_type_for_type(&base.ty, field)?;
                    let ownership = self.expected_ownership(&projected, base.ownership)?;
                    self.finish_expr(expression, &projected, ownership)?;
                    scopes.push(scope);
                }
                Frame::Upcast { expression } => {
                    let scope = scopes.pop().expect("upcast scope retained");
                    let ResolvedExprKind::Upcast { source } = &expression.kind else {
                        unreachable!()
                    };
                    self.validate_upcast(expression, source)?;
                    let ownership = self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                    self.finish_expr(expression, &expression.ty, ownership)?;
                    scopes.push(scope);
                }
                Frame::Try {
                    expression,
                    path,
                    option,
                } => {
                    let scope = scopes.pop().expect("try scope retained");
                    self.finish_try_expr(function, expression, &scope, &path, option)?;
                    scopes.push(scope);
                }
            }
        }
        if scopes.len() != 1 {
            return Err(hir_error("iterative HIR validator lost its scope stack"));
        }
        publication.publish(&scopes.pop().expect("root validation scope retained"));
        Ok(())
    }

    fn finish_expr(
        &self,
        expression: &ResolvedExpr,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        self.require_type(&expression.ty, ty, "expression")?;
        if expression.ownership != ownership {
            return Err(hir_error(format!(
                "expression `{}` has inconsistent ownership",
                expression.id
            )));
        }
        Ok(())
    }

    /// Float literals must stay finite so canonical source projection and
    /// every backend agree on the exact value; infinities and NaNs cannot be
    /// written as literals and hostile HIR is rejected here.
    fn validate_finite_f32(&self, bits: u32) -> Result<(), Diagnostic> {
        if f32::from_bits(bits).is_finite() {
            Ok(())
        } else {
            Err(hir_error(
                "f32 literal bits are not a finite IEEE-754 value",
            ))
        }
    }

    fn validate_finite_f64(&self, bits: u64) -> Result<(), Diagnostic> {
        if f64::from_bits(bits).is_finite() {
            Ok(())
        } else {
            Err(hir_error(
                "f64 literal bits are not a finite IEEE-754 value",
            ))
        }
    }

    fn finish_try_expr(
        &self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        option_profile: bool,
    ) -> Result<(), Diagnostic> {
        if option_profile {
            let ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } = &expression.kind
            else {
                unreachable!()
            };
            if !path.starts_with("body") {
                return Err(hir_error(
                    "resolved Option `?` is outside the executable function body",
                ));
            }
            if scope.values().any(|binding| {
                self.program
                    .declarations
                    .type_facts(&binding.ty)
                    .is_some_and(|facts| facts.contains_resource)
            }) {
                return Err(hir_error(
                    "resolved Option `?` has a live resource binding in the bounded Copy-only profile",
                ));
            }
            if option.as_str() != crate::prelude::OPTION_ID
                || some_case.as_str() != crate::prelude::OPTION_SOME_ID
                || some_field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID
                || none_case.as_str() != crate::prelude::OPTION_NONE_ID
            {
                return Err(hir_error(
                    "resolved Option `?` does not authenticate the compiler-owned Option shape",
                ));
            }
            for id in [option, some_case, some_field, none_case] {
                if self
                    .program
                    .declarations
                    .declaration(id)
                    .is_none_or(|declaration| {
                        declaration.identity_origin != IdentityOrigin::CompilerOwned
                    })
                {
                    return Err(hir_error(format!(
                        "resolved Option `?` identity `{id}` is not compiler-owned"
                    )));
                }
            }
            let (
                ResolvedType::Nominal {
                    declaration: operand_option,
                    arguments: operand_arguments,
                },
                ResolvedType::Nominal {
                    declaration: residual_option,
                    arguments: residual_arguments,
                },
            ) = (&operand.ty, residual_type)
            else {
                return Err(hir_error(
                    "resolved Option `?` operand or residual is not nominal Option",
                ));
            };
            if operand_option != option
                || residual_option != option
                || operand_arguments.len() != 1
                || residual_arguments.len() != 1
                || operand_arguments
                    .iter()
                    .chain(residual_arguments)
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(hir_error(
                    "resolved Option `?` has invalid concrete Option instances",
                ));
            }
            let enclosing = self
                .execution_function(function)
                .map(|candidate| &candidate.return_type)
                .ok_or_else(|| hir_error("resolved Option `?` has no enclosing function"))?;
            self.require_type(residual_type, enclosing, "Option `?` residual")?;
            self.require_type(
                &expression.ty,
                &operand_arguments[0],
                "Option `?` success value",
            )?;
            if expression.ownership != OwnershipMode::Value {
                return Err(hir_error("resolved Option `?` success value is not Copy"));
            }
            Ok(())
        } else {
            let ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } = &expression.kind
            else {
                unreachable!()
            };
            if !path.starts_with("body") {
                return Err(hir_error(
                    "resolved `?` is outside the executable function body",
                ));
            }
            if scope.values().any(|binding| {
                self.program
                    .declarations
                    .type_facts(&binding.ty)
                    .is_some_and(|facts| facts.contains_resource)
            }) {
                return Err(hir_error(
                    "resolved `?` has a live resource binding in the bounded Copy-only profile",
                ));
            }
            if result.as_str() != crate::prelude::RESULT_ID
                || ok_case.as_str() != crate::prelude::RESULT_OK_ID
                || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
                || err_case.as_str() != crate::prelude::RESULT_ERR_ID
                || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
            {
                return Err(hir_error(
                    "resolved `?` does not authenticate the compiler-owned Result shape",
                ));
            }
            let (
                ResolvedType::Nominal {
                    declaration: operand_result,
                    arguments: operand_arguments,
                },
                ResolvedType::Nominal {
                    declaration: residual_result,
                    arguments: residual_arguments,
                },
            ) = (&operand.ty, residual_type)
            else {
                return Err(hir_error(
                    "resolved `?` operand or residual is not nominal Result",
                ));
            };
            if operand_result != result
                || residual_result != result
                || operand_arguments.len() != 2
                || residual_arguments.len() != 2
                || operand_arguments
                    .iter()
                    .chain(residual_arguments)
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            {
                return Err(hir_error(
                    "resolved `?` has invalid concrete Result instances",
                ));
            }
            let enclosing = self
                .execution_function(function)
                .map(|candidate| &candidate.return_type)
                .ok_or_else(|| hir_error("resolved `?` has no enclosing function"))?;
            self.require_type(residual_type, enclosing, "`?` residual")?;
            self.require_type(&expression.ty, &operand_arguments[0], "`?` success value")?;
            self.require_type(
                &operand_arguments[1],
                &residual_arguments[1],
                "`?` residual error",
            )?;
            if expression.ownership != OwnershipMode::Value {
                return Err(hir_error("resolved `?` success value is not Copy"));
            }
            Ok(())
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn validate_expr_recursive_reference(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        if matches!(expression.kind, ResolvedExprKind::Unary { .. }) {
            let mut unary = Vec::new();
            let mut current = expression;
            let mut current_path = path.to_owned();
            while let ResolvedExprKind::Unary { op, value } = &current.kind {
                reject_nul_identity("resolved expression", current.id.as_str())?;
                if current.id != ExpressionId::new(function, &current_path) {
                    return Err(hir_error(format!(
                        "expression `{}` has a non-canonical identity",
                        current.id
                    )));
                }
                if !self.expression_ids.insert(current.id.clone()) {
                    return Err(hir_error(format!(
                        "duplicate resolved expression identity `{}`",
                        current.id
                    )));
                }
                self.validate_type(&current.ty)?;
                unary.push((current, *op));
                current = value;
                current_path.push_str(".value");
            }
            self.validate_expr_recursive_reference(
                function,
                current,
                scope,
                &current_path,
                allow_moves,
                allowed_effects,
            )?;
            let mut operand = current;
            for (expression, op) in unary.into_iter().rev() {
                if matches!(op, UnaryOp::Neg)
                    && !matches!(
                        &operand.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::F32
                            | ResolvedType::F64
                    )
                {
                    return Err(hir_error("unary operand has inconsistent resolved types"));
                }
                let expected = match op {
                    UnaryOp::Neg => operand.ty.clone(),
                    UnaryOp::Not => ResolvedType::Bool,
                };
                self.require_type(&operand.ty, &expected, "unary operand")?;
                self.require_type(&expression.ty, &expected, "expression")?;
                if expression.ownership != OwnershipMode::Value {
                    return Err(hir_error(format!(
                        "expression `{}` has inconsistent ownership",
                        expression.id
                    )));
                }
                operand = expression;
            }
            return Ok(());
        }
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        if expression.id != ExpressionId::new(function, path) {
            return Err(hir_error(format!(
                "expression `{}` has a non-canonical identity",
                expression.id
            )));
        }
        if !self.expression_ids.insert(expression.id.clone()) {
            return Err(hir_error(format!(
                "duplicate resolved expression identity `{}`",
                expression.id
            )));
        }
        self.validate_type(&expression.ty)?;

        let (ty, ownership) = match &expression.kind {
            ResolvedExprKind::String(_) => (ResolvedType::String, OwnershipMode::Own),
            ResolvedExprKind::Int(_) => (ResolvedType::I64, OwnershipMode::Value),
            ResolvedExprKind::Int32(_) => (ResolvedType::I32, OwnershipMode::Value),
            ResolvedExprKind::Char(value) => {
                if char::from_u32(*value).is_none() {
                    return Err(hir_error(
                        "char literal bits are not a Unicode scalar value",
                    ));
                }
                (ResolvedType::Char, OwnershipMode::Value)
            }
            ResolvedExprKind::Uint8(_) => (ResolvedType::U8, OwnershipMode::Value),
            ResolvedExprKind::Float32(bits) => {
                self.validate_finite_f32(*bits)?;
                (ResolvedType::F32, OwnershipMode::Value)
            }
            ResolvedExprKind::Float64(bits) => {
                self.validate_finite_f64(*bits)?;
                (ResolvedType::F64, OwnershipMode::Value)
            }
            ResolvedExprKind::Bool(_) => (ResolvedType::Bool, OwnershipMode::Value),
            ResolvedExprKind::Place(place) => {
                let binding = scope.get(&place.root).ok_or_else(|| {
                    hir_error(format!("resolved value `{}` is out of scope", place.root))
                })?;
                match (place.projections.is_empty(), binding.availability) {
                    (true, Availability::Available) => {
                        match Self::place_availability(binding, &[]) {
                            Availability::Available => {}
                            Availability::Moved => {
                                return Err(hir_error(format!(
                                    "resolved value `{}` is partially moved",
                                    place.root
                                )));
                            }
                            Availability::MaybeMoved => {
                                return Err(hir_error(format!(
                                    "resolved value `{}` may be partially moved",
                                    place.root
                                )));
                            }
                        }
                    }
                    (true, Availability::Moved) => {
                        return Err(hir_error(format!(
                            "resolved value `{}` is used after it was moved",
                            place.root
                        )));
                    }
                    (true, Availability::MaybeMoved) => {
                        return Err(hir_error(format!(
                            "resolved value `{}` may have been moved",
                            place.root
                        )));
                    }
                    (false, _) => match Self::place_availability(binding, &place.projections) {
                        Availability::Available => {}
                        Availability::Moved => {
                            return Err(hir_error(format!(
                                "resolved place rooted at `{}` is partially moved",
                                place.root
                            )));
                        }
                        Availability::MaybeMoved => {
                            return Err(hir_error(format!(
                                "resolved place rooted at `{}` may be conditionally moved",
                                place.root
                            )));
                        }
                    },
                }
                self.resolve_place(place, binding)?
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                match instance {
                    None if !type_arguments.is_empty() => {
                        return Err(hir_error(
                            "monomorphic resolved call carries generic type arguments",
                        ));
                    }
                    Some(instance)
                        if FunctionInstanceId::derive(callee, type_arguments) != *instance =>
                    {
                        return Err(hir_error(
                            "resolved call instance disagrees with its template and arguments",
                        ));
                    }
                    Some(_) if type_arguments.is_empty() => {
                        return Err(hir_error(
                            "generic resolved call has no concrete type arguments",
                        ));
                    }
                    None | Some(_) => {}
                }
                for argument in type_arguments {
                    if !matches!(argument, ResolvedType::I64 | ResolvedType::Bool) {
                        return Err(hir_error(
                            "resolved call has a non-scalar generic type argument",
                        ));
                    }
                }
                let intrinsic = if instance.is_none() {
                    crate::string_ops::by_id(callee.as_str())
                } else {
                    None
                };
                let (params, return_type, target_effects) = if let Some(op) = intrinsic {
                    // String operations carry their reserved identity instead
                    // of an authored declaration.
                    if args.len() != op.arity() {
                        return Err(hir_error(format!(
                            "string operation `{}` expects {} arguments but received {}",
                            op.name(),
                            op.arity(),
                            args.len()
                        )));
                    }
                    (
                        crate::string_ops::resolved_params(op),
                        op.return_type(),
                        Vec::new(),
                    )
                } else {
                    let target = self
                        .program
                        .resolve_call_target(callee, instance.as_ref())
                        .ok_or_else(|| {
                            hir_error(format!("resolved callee `{callee}` is not indexed"))
                        })?;
                    (
                        target.params.clone(),
                        target.return_type.clone(),
                        target.effects.clone(),
                    )
                };
                if args.len() != params.len() {
                    return Err(hir_error(format!(
                        "call to `{callee}` has {} arguments but expects {}",
                        args.len(),
                        params.len()
                    )));
                }
                match allowed_effects {
                    Some(allowed) => {
                        for effect in &target_effects {
                            if !allowed.contains(effect) {
                                return Err(hir_error(format!(
                                    "call to `{callee}` requires undeclared effect `{effect}`"
                                )));
                            }
                        }
                    }
                    None if !target_effects.is_empty() => {
                        return Err(hir_error(format!(
                            "contract calls effectful function `{callee}`"
                        )));
                    }
                    None => {}
                }
                for (index, (argument, param)) in args.iter().zip(&params).enumerate() {
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &format!("{path}.arg.{index}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &param.ty, "call argument")?;
                    self.validate_argument_ownership(argument.ownership, param)?;
                    if self.argument_transfers(param)? {
                        if !allow_moves {
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                let ownership = self.expected_ownership(&return_type, OwnershipMode::Own)?;
                (return_type, ownership)
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                if call.expression != expression.id {
                    return Err(hir_error(
                        "native Rust import call has a non-canonical expression identity",
                    ));
                }
                let import = self
                    .program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|import| import.id == call.import && import.native_rust)
                    .ok_or_else(|| hir_error("native Rust import call has an unknown target"))?;
                if import.parameters.len() != call.args.len() || import.result.kind != call.result {
                    return Err(hir_error(
                        "native Rust import call disagrees with its declaration",
                    ));
                }
                match allowed_effects {
                    Some(allowed) => {
                        if import
                            .effects
                            .iter()
                            .any(|effect| !allowed.contains(effect))
                        {
                            return Err(hir_error(
                                "native Rust import call requires an undeclared effect",
                            ));
                        }
                    }
                    None if !import.effects.is_empty() => {
                        return Err(hir_error("contract calls an effectful native Rust import"));
                    }
                    None => {}
                }
                for (index, (argument, parameter)) in
                    call.args.iter().zip(&import.parameters).enumerate()
                {
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &format!("{path}.native-rust-arg.{index}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &parameter.ty, "native Rust import argument")?;
                    if argument.ownership != OwnershipMode::Value
                        || parameter.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error(
                            "native Rust import arguments must use value ownership",
                        ));
                    }
                }
                let result = match call.result {
                    ResolvedImportResultKind::Unit => ResolvedType::Unit,
                    ResolvedImportResultKind::I64 => ResolvedType::I64,
                    ResolvedImportResultKind::Bool => ResolvedType::Bool,
                };
                (result, OwnershipMode::Value)
            }
            ResolvedExprKind::Unary { .. } => unreachable!("unary chain handled above"),
            ResolvedExprKind::Binary { op, left, right } => {
                self.validate_expr_recursive_reference(
                    function,
                    left,
                    scope,
                    &format!("{path}.left"),
                    allow_moves,
                    allowed_effects,
                )?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let baseline_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut conditional_scope = scope.clone();
                    self.validate_expr_recursive_reference(
                        function,
                        right,
                        &mut conditional_scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    Self::join_conditional(scope, &conditional_scope, &baseline_ids);
                } else {
                    self.validate_expr_recursive_reference(
                        function,
                        right,
                        scope,
                        &format!("{path}.right"),
                        allow_moves,
                        allowed_effects,
                    )?;
                }
                let output = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {
                        self.require_type(&left.ty, &ResolvedType::I64, "binary operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "binary operand")?;
                        ResolvedType::I64
                    }
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        self.require_type(&left.ty, &ResolvedType::I64, "comparison operand")?;
                        self.require_type(&right.ty, &ResolvedType::I64, "comparison operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        self.require_type(&left.ty, &ResolvedType::Bool, "boolean operand")?;
                        self.require_type(&right.ty, &ResolvedType::Bool, "boolean operand")?;
                        ResolvedType::Bool
                    }
                    BinaryOp::Eq | BinaryOp::Ne => {
                        self.require_type(&left.ty, &right.ty, "equality operands")?;
                        ResolvedType::Bool
                    }
                };
                (output, OwnershipMode::Value)
            }
            ResolvedExprKind::Block { statements, tail } => {
                let mut block_scope = scope.clone();
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let statement_path = format!("{path}.s{index}");
                            self.validate_expr_recursive_reference(
                                function,
                                value,
                                &mut block_scope,
                                &format!("{statement_path}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if binding.id != ValueId::local(function, &statement_path) {
                                return Err(hir_error(format!(
                                    "local `{}` has a non-canonical identity",
                                    binding.id
                                )));
                            }
                            self.insert_value(&binding.id)?;
                            self.require_type(&binding.ty, &value.ty, "local binding")?;
                            if binding.ownership != value.ownership {
                                return Err(hir_error(format!(
                                    "local `{}` has inconsistent ownership",
                                    binding.id
                                )));
                            }
                            self.validate_declared_ownership(&binding.ty, binding.ownership)?;
                            if self.is_owned_resource(&binding.ty, binding.ownership)? {
                                if !allow_moves {
                                    return Err(hir_error(
                                        "contract cannot transfer ownership into a local binding",
                                    ));
                                }
                                self.mark_value_sources_moved(value, &mut block_scope)?;
                            }
                            block_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    availability: Availability::Available,
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            );
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            ..
                        } => {
                            let statement_path = format!("{path}.s{index}");
                            self.validate_expr_recursive_reference(
                                function,
                                assigned,
                                &mut block_scope,
                                &format!("{statement_path}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            let target = block_scope.get(&binding.id).cloned();
                            let Some(target) = target else {
                                return Err(hir_error(format!(
                                    "assignment target `{}` has no enclosing binding",
                                    binding.id
                                )));
                            };
                            match field {
                                Some(field) => {
                                    if target.ownership != OwnershipMode::Value {
                                        return Err(hir_error(
                                            "field assignment base is not a value-owned aggregate",
                                        ));
                                    }
                                    self.validate_assign_field(&target.ty, field, assigned)?;
                                }
                                None => {
                                    self.require_type(&target.ty, &assigned.ty, "assignment")?;
                                    if target.ownership != OwnershipMode::Value
                                        || !crate::hir::is_scalar_resolved_type(&target.ty)
                                    {
                                        return Err(hir_error(
                                            "explicit mutation v1 supports only scalar Copy values",
                                        ));
                                    }
                                }
                            }
                            if target.availability != Availability::Available {
                                return Err(hir_error(format!(
                                    "assignment target `{}` is not available",
                                    binding.id
                                )));
                            }
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            if !allow_moves {
                                return Err(hir_error(
                                    "contract expressions cannot contain unsafe boundary statements",
                                ));
                            }
                            let statement_path = format!("{path}.s{index}.body");
                            self.validate_expr_recursive_reference(
                                function,
                                body,
                                &mut block_scope,
                                &statement_path,
                                allow_moves,
                                allowed_effects,
                            )?;
                            let ResolvedExprKind::Block { tail, .. } = &body.kind else {
                                unreachable!("unsafe bodies always parse as blocks")
                            };
                            if tail.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&tail.ty)
                            {
                                return Err(hir_error(
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                ));
                            }
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1: mirror the iterative
                            // admission re-check, condition typing, body
                            // validation, and exact entry-state equality.
                            self.validate_while_admission(condition)?;
                            self.validate_while_admission(body)?;
                            let entry_scope = block_scope.clone();
                            self.validate_expr_recursive_reference(
                                function,
                                condition,
                                &mut block_scope,
                                &format!("{path}.s{index}.condition"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if condition.ty != ResolvedType::Bool {
                                return Err(hir_error("`while` condition must be bool"));
                            }
                            self.validate_expr_recursive_reference(
                                function,
                                body,
                                &mut block_scope,
                                &format!("{path}.s{index}.body"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if block_scope != entry_scope {
                                return Err(hir_error(
                                    "while loop body changes ownership liveness",
                                ));
                            }
                        }
                    }
                }
                self.validate_expr_recursive_reference(
                    function,
                    tail,
                    &mut block_scope,
                    &format!("{path}.tail"),
                    allow_moves,
                    allowed_effects,
                )?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                Self::merge_availability(scope, &block_scope, &outer_ids);
                (tail.ty.clone(), tail.ownership)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_expr_recursive_reference(
                    function,
                    condition,
                    scope,
                    &format!("{path}.condition"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut then_scope = scope.clone();
                let mut else_scope = scope.clone();
                self.validate_expr_recursive_reference(
                    function,
                    then_branch,
                    &mut then_scope,
                    &format!("{path}.then"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_expr_recursive_reference(
                    function,
                    else_branch,
                    &mut else_scope,
                    &format!("{path}.else"),
                    allow_moves,
                    allowed_effects,
                )?;
                Self::join_branches(scope, &then_scope, &else_scope, &outer_ids);
                self.require_type(&then_branch.ty, &else_branch.ty, "if branches")?;
                if then_branch.ownership != else_branch.ownership {
                    return Err(hir_error("if branches have inconsistent ownership"));
                }
                (then_branch.ty.clone(), then_branch.ownership)
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let declaration = self
                    .program
                    .declarations
                    .declaration(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                if !matches!(
                    declaration.kind,
                    DeclarationKind::Record | DeclarationKind::Class
                ) {
                    return Err(hir_error(format!(
                        "constructor target `{record}` is not a record or class"
                    )));
                }
                let expected_fields = self
                    .program
                    .declarations
                    .record_fields(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?
                    .to_vec();
                let ResolvedType::Nominal {
                    declaration: instance_record,
                    arguments,
                } = &expression.ty
                else {
                    return Err(hir_error("record constructor result is not nominal"));
                };
                let parameters = self
                    .program
                    .declarations
                    .type_parameters(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                if instance_record != record
                    || arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(format!(
                        "constructor for `{record}` has an invalid concrete instance"
                    )));
                }
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "constructor for `{record}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let expected = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected {
                        return Err(hir_error(format!(
                            "field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, scope)?;
                    }
                }
                if seen.len() != expected_fields.len() {
                    return Err(hir_error(format!(
                        "constructor for `{record}` is missing required fields"
                    )));
                }
                let ty = expression.ty.clone();
                let ownership = self.expected_ownership(&ty, OwnershipMode::Own)?;
                (ty, ownership)
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let ResolvedType::Nominal {
                    declaration: instance_variant,
                    arguments,
                } = &expression.ty
                else {
                    return Err(hir_error("variant constructor has a non-nominal result"));
                };
                if instance_variant != variant {
                    return Err(hir_error(
                        "variant constructor result disagrees with its declaration",
                    ));
                }
                let declaration = self
                    .program
                    .declarations
                    .declaration(variant)
                    .ok_or_else(|| hir_error(format!("variant `{variant}` is not indexed")))?;
                if declaration.kind != DeclarationKind::Variant {
                    return Err(hir_error(format!(
                        "constructor target `{variant}` is not a variant"
                    )));
                }
                let declared_case = self
                    .program
                    .declarations
                    .variant_cases(variant)
                    .and_then(|cases| cases.iter().find(|item| item.id == *case))
                    .ok_or_else(|| {
                        hir_error(format!(
                            "constructor for `{variant}` contains foreign case `{case}`"
                        ))
                    })?;
                let expected_fields = declared_case.fields.clone();
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "constructor for `{case}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, variant, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "variant payload field")?;
                    if initializer.value.ownership != OwnershipMode::Value {
                        return Err(hir_error(format!(
                            "variant payload field `{}` is not a Copy value",
                            initializer.field
                        )));
                    }
                }
                if seen.len() != expected_fields.len() {
                    return Err(hir_error(format!(
                        "constructor for `{case}` is missing required payload fields"
                    )));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                self.validate_expr_recursive_reference(
                    function,
                    scrutinee,
                    scope,
                    &format!("{path}.scrutinee"),
                    allow_moves,
                    allowed_effects,
                )?;
                let ResolvedType::Nominal {
                    declaration: matched_type,
                    arguments,
                } = &scrutinee.ty
                else {
                    return Err(hir_error("resolved match scrutinee is not nominal"));
                };
                let matched_kind = self
                    .program
                    .declarations
                    .declaration(matched_type)
                    .map(|item| item.kind);
                if matched_kind == Some(DeclarationKind::Record) {
                    if scrutinee.ownership != OwnershipMode::Value {
                        return Err(hir_error("resolved record match scrutinee is not Copy"));
                    }
                    let [arm] = arms.as_slice() else {
                        return Err(hir_error(
                            "resolved irrefutable record match must have exactly one arm",
                        ));
                    };
                    let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut arm_scope = scope.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => self.validate_record_match_pattern(
                            function,
                            &scrutinee.ty,
                            record,
                            instance,
                            fields,
                            &mut arm_scope,
                            &format!("{path}.arm.0.record"),
                        )?,
                        ResolvedMatchPattern::Variant { .. } => {
                            return Err(hir_error(
                                "resolved variant pattern has a record scrutinee",
                            ));
                        }
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &arm.value,
                        &mut arm_scope,
                        &format!("{path}.arm.0.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    if !matches!(arm.value.ty, ResolvedType::I64 | ResolvedType::Bool) {
                        return Err(hir_error(
                            "resolved record match arm must produce i64 or bool",
                        ));
                    }
                    for id in outer_ids {
                        if let Some(state) = arm_scope.get(&id) {
                            scope.insert(id, state.clone());
                        }
                    }
                    self.require_type(&expression.ty, &arm.value.ty, "record match expression")?;
                    if expression.ownership != arm.value.ownership {
                        return Err(hir_error(
                            "resolved record match expression has inconsistent ownership",
                        ));
                    }
                    return Ok(());
                }
                let variant = matched_type;
                if scrutinee.ownership != OwnershipMode::Value
                    || self
                        .program
                        .declarations
                        .declaration(variant)
                        .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(hir_error(
                        "resolved match scrutinee is not a concrete Copy variant",
                    ));
                }
                let cases = self
                    .program
                    .declarations
                    .variant_cases(variant)
                    .ok_or_else(|| hir_error(format!("variant `{variant}` has no cases")))?
                    .to_vec();
                if arms.is_empty() {
                    return Err(hir_error("resolved match has no arms"));
                }
                let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                let mut arm_scopes = Vec::with_capacity(arms.len());
                let mut covered = BTreeSet::new();
                let mut wildcard_seen = false;
                let mut result = None::<(ResolvedType, OwnershipMode)>;
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_scope = scope.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {
                            if wildcard_seen || covered.len() == cases.len() {
                                return Err(hir_error(
                                    "resolved match has an unreachable wildcard",
                                ));
                            }
                            wildcard_seen = true;
                        }
                        ResolvedMatchPattern::Variant {
                            variant: pattern_variant,
                            case,
                            fields,
                        } => {
                            if wildcard_seen
                                || pattern_variant != variant
                                || !covered.insert(case.clone())
                            {
                                return Err(hir_error(
                                    "resolved match has an unreachable or foreign case pattern",
                                ));
                            }
                            let declared_case =
                                cases.iter().find(|item| item.id == *case).ok_or_else(|| {
                                    hir_error(format!(
                                        "resolved match references foreign case `{case}`"
                                    ))
                                })?;
                            let mut seen_fields = BTreeSet::new();
                            for (field_index, pattern_field) in fields.iter().enumerate() {
                                let declared_field = declared_case
                                    .fields
                                    .iter()
                                    .find(|item| item.id == pattern_field.field)
                                    .ok_or_else(|| {
                                        hir_error(format!(
                                            "resolved pattern contains foreign field `{}`",
                                            pattern_field.field
                                        ))
                                    })?;
                                let binding_ty =
                                    substitute_type(&declared_field.ty, variant, arguments)?;
                                if !seen_fields.insert(pattern_field.field.clone())
                                    || pattern_field.binding.id
                                        != ValueId::local(
                                            function,
                                            &format!(
                                                "{path}.arm.{arm_index}.binding.{field_index}"
                                            ),
                                        )
                                    || pattern_field.binding.ty != binding_ty
                                    || pattern_field.binding.ownership != OwnershipMode::Value
                                {
                                    return Err(hir_error(
                                        "resolved match pattern field or binding is invalid",
                                    ));
                                }
                                self.insert_value(&pattern_field.binding.id)?;
                                self.validate_type(&pattern_field.binding.ty)?;
                                if arm_scope.contains_key(&pattern_field.binding.id) {
                                    return Err(hir_error(
                                        "resolved match pattern binding shadows an existing value",
                                    ));
                                }
                                arm_scope.insert(
                                    pattern_field.binding.id.clone(),
                                    ValidationBinding {
                                        ty: pattern_field.binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        availability: Availability::Available,
                                        moved_places: BTreeMap::new(),
                                        definitely_partial: BTreeSet::new(),
                                    },
                                );
                            }
                            if seen_fields.len() != declared_case.fields.len() {
                                return Err(hir_error(
                                    "resolved match pattern is missing payload fields",
                                ));
                            }
                        }
                        ResolvedMatchPattern::Record { .. } => {
                            return Err(hir_error(
                                "resolved record pattern has a variant scrutinee",
                            ));
                        }
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &arm.value,
                        &mut arm_scope,
                        &format!("{path}.arm.{arm_index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    if let Some((expected_ty, expected_ownership)) = &result {
                        self.require_type(&arm.value.ty, expected_ty, "match arm")?;
                        if arm.value.ownership != *expected_ownership {
                            return Err(hir_error(
                                "resolved match arms have inconsistent ownership",
                            ));
                        }
                    } else {
                        result = Some((arm.value.ty.clone(), arm.value.ownership));
                    }
                    arm_scopes.push(arm_scope);
                }
                if !wildcard_seen && covered.len() != cases.len() {
                    return Err(hir_error("resolved match is not exhaustive"));
                }
                if let Some((first, rest)) = arm_scopes.split_first() {
                    let mut joined = first.clone();
                    for arm_scope in rest {
                        Self::join_conditional(&mut joined, arm_scope, &outer_ids);
                    }
                    Self::merge_availability(scope, &joined, &outer_ids);
                }
                result.ok_or_else(|| hir_error("resolved match has no result"))?
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
                self.validate_expr_recursive_reference(
                    function,
                    operand,
                    scope,
                    &format!("{path}.operand"),
                    allow_moves,
                    allowed_effects,
                )?;
                if !path.starts_with("body") {
                    return Err(hir_error(
                        "resolved `?` is outside the executable function body",
                    ));
                }
                if scope.values().any(|binding| {
                    self.program
                        .declarations
                        .type_facts(&binding.ty)
                        .is_some_and(|facts| facts.contains_resource)
                }) {
                    return Err(hir_error(
                        "resolved `?` has a live resource binding in the bounded Copy-only profile",
                    ));
                }
                if result.as_str() != crate::prelude::RESULT_ID
                    || ok_case.as_str() != crate::prelude::RESULT_OK_ID
                    || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
                    || err_case.as_str() != crate::prelude::RESULT_ERR_ID
                    || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
                {
                    return Err(hir_error(
                        "resolved `?` does not authenticate the compiler-owned Result shape",
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: operand_result,
                    arguments: operand_arguments,
                } = &operand.ty
                else {
                    return Err(hir_error("resolved `?` operand is not nominal Result"));
                };
                let ResolvedType::Nominal {
                    declaration: residual_result,
                    arguments: residual_arguments,
                } = residual_type
                else {
                    return Err(hir_error("resolved `?` residual is not nominal Result"));
                };
                if operand_result != result
                    || residual_result != result
                    || operand_arguments.len() != 2
                    || residual_arguments.len() != 2
                    || operand_arguments
                        .iter()
                        .chain(residual_arguments)
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(
                        "resolved `?` has invalid concrete Result instances",
                    ));
                }
                let enclosing_return = self
                    .execution_function(function)
                    .map(|candidate| &candidate.return_type)
                    .ok_or_else(|| hir_error("resolved `?` has no enclosing function"))?;
                self.require_type(residual_type, enclosing_return, "`?` residual")?;
                self.require_type(&expression.ty, &operand_arguments[0], "`?` success value")?;
                self.require_type(
                    &operand_arguments[1],
                    &residual_arguments[1],
                    "`?` residual error",
                )?;
                if expression.ownership != OwnershipMode::Value {
                    return Err(hir_error("resolved `?` success value is not Copy"));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                self.validate_expr_recursive_reference(
                    function,
                    operand,
                    scope,
                    &format!("{path}.operand"),
                    allow_moves,
                    allowed_effects,
                )?;
                if !path.starts_with("body") {
                    return Err(hir_error(
                        "resolved Option `?` is outside the executable function body",
                    ));
                }
                if scope.values().any(|binding| {
                    self.program
                        .declarations
                        .type_facts(&binding.ty)
                        .is_some_and(|facts| facts.contains_resource)
                }) {
                    return Err(hir_error(
                        "resolved Option `?` has a live resource binding in the bounded Copy-only profile",
                    ));
                }
                if option.as_str() != crate::prelude::OPTION_ID
                    || some_case.as_str() != crate::prelude::OPTION_SOME_ID
                    || some_field.as_str() != crate::prelude::OPTION_SOME_VALUE_ID
                    || none_case.as_str() != crate::prelude::OPTION_NONE_ID
                {
                    return Err(hir_error(
                        "resolved Option `?` does not authenticate the compiler-owned Option shape",
                    ));
                }
                for id in [option, some_case, some_field, none_case] {
                    if self
                        .program
                        .declarations
                        .declaration(id)
                        .is_none_or(|declaration| {
                            declaration.identity_origin != IdentityOrigin::CompilerOwned
                        })
                    {
                        return Err(hir_error(format!(
                            "resolved Option `?` identity `{id}` is not compiler-owned"
                        )));
                    }
                }
                let ResolvedType::Nominal {
                    declaration: operand_option,
                    arguments: operand_arguments,
                } = &operand.ty
                else {
                    return Err(hir_error(
                        "resolved Option `?` operand is not nominal Option",
                    ));
                };
                let ResolvedType::Nominal {
                    declaration: residual_option,
                    arguments: residual_arguments,
                } = residual_type
                else {
                    return Err(hir_error(
                        "resolved Option `?` residual is not nominal Option",
                    ));
                };
                if operand_option != option
                    || residual_option != option
                    || operand_arguments.len() != 1
                    || residual_arguments.len() != 1
                    || operand_arguments
                        .iter()
                        .chain(residual_arguments)
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(
                        "resolved Option `?` has invalid concrete Option instances",
                    ));
                }
                let enclosing_return = self
                    .execution_function(function)
                    .map(|candidate| &candidate.return_type)
                    .ok_or_else(|| hir_error("resolved Option `?` has no enclosing function"))?;
                self.require_type(residual_type, enclosing_return, "Option `?` residual")?;
                self.require_type(
                    &expression.ty,
                    &operand_arguments[0],
                    "Option `?` success value",
                )?;
                if expression.ownership != OwnershipMode::Value {
                    return Err(hir_error("resolved Option `?` success value is not Copy"));
                }
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                self.validate_expr_recursive_reference(
                    function,
                    base,
                    scope,
                    &format!("{path}.base"),
                    allow_moves,
                    allowed_effects,
                )?;
                let declaration = self
                    .program
                    .declarations
                    .declaration(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                if declaration.kind != DeclarationKind::Record {
                    return Err(hir_error(format!(
                        "record update target `{record}` is not a record"
                    )));
                }
                let ty = base.ty.clone();
                let ResolvedType::Nominal {
                    declaration: instance_record,
                    arguments,
                } = &ty
                else {
                    return Err(hir_error("record update base is not nominal"));
                };
                let parameters = self
                    .program
                    .declarations
                    .type_parameters(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                if instance_record != record
                    || arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(hir_error(format!(
                        "record update for `{record}` has an invalid concrete instance"
                    )));
                }
                self.require_type(&base.ty, &ty, "record update base")?;
                let ownership = self.expected_ownership(&ty, OwnershipMode::Own)?;
                if base.ownership != ownership {
                    return Err(hir_error(format!(
                        "record update base for `{record}` has incompatible ownership"
                    )));
                }
                if ownership == OwnershipMode::Own {
                    if !allow_moves {
                        return Err(hir_error(
                            "contract cannot transfer ownership from a record update base",
                        ));
                    }
                    self.mark_value_sources_moved(base, scope)?;
                }

                let expected_fields = self
                    .program
                    .declarations
                    .record_fields(record)
                    .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?
                    .to_vec();
                let mut seen = BTreeSet::new();
                for (index, initializer) in fields.iter().enumerate() {
                    let field = expected_fields
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            ))
                        })?;
                    if !seen.insert(initializer.field.clone()) {
                        return Err(hir_error(format!(
                            "update for `{record}` repeats field `{}`",
                            initializer.field
                        )));
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &initializer.value,
                        scope,
                        &format!("{path}.field.{index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let field_ty = substitute_type(&field.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record replacement")?;
                    let expected = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected {
                        return Err(hir_error(format!(
                            "replacement field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record replacement",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, scope)?;
                    }
                }
                (ty, ownership)
            }
            ResolvedExprKind::Project { base, field } => {
                if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                    return Err(hir_error(
                        "place field projections must use a resolved place path",
                    ));
                }
                self.validate_expr_recursive_reference(
                    function,
                    base,
                    scope,
                    &format!("{path}.base"),
                    allow_moves,
                    allowed_effects,
                )?;
                let projected = self.field_type_for_type(&base.ty, field)?;
                let ownership = self.expected_ownership(&projected, base.ownership)?;
                (projected, ownership)
            }
            ResolvedExprKind::Upcast { source } => {
                self.validate_expr_recursive_reference(
                    function,
                    source,
                    scope,
                    &format!("{path}.source"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_upcast(expression, source)?;
                let ownership = self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                (expression.ty.clone(), ownership)
            }
        };

        self.require_type(&expression.ty, &ty, "expression")?;
        if expression.ownership != ownership {
            return Err(hir_error(format!(
                "expression `{}` has inconsistent ownership",
                expression.id
            )));
        }
        Ok(())
    }

    fn resolve_place(
        &self,
        place: &Place,
        binding: &ValidationBinding,
    ) -> Result<(ResolvedType, OwnershipMode), Diagnostic> {
        let mut ty = binding.ty.clone();
        let mut ownership = binding.ownership;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => {
                    ty = self.field_type_for_type(&ty, field)?;
                    ownership = self.expected_ownership(&ty, ownership)?;
                }
                PlaceProjection::VariantField { .. } => {
                    return Err(hir_error(
                        "variant-field projections are not valid before variant HIR lands",
                    ));
                }
            }
        }
        Ok((ty, ownership))
    }

    fn field_type_for_type(
        &self,
        ty: &ResolvedType,
        field: &DeclarationId,
    ) -> Result<ResolvedType, Diagnostic> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Err(hir_error(format!(
                "field `{field}` projects from a non-record type"
            )));
        };
        if self
            .program
            .declarations
            .declaration(declaration)
            .is_none_or(|item| {
                !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
            })
        {
            return Err(hir_error(format!(
                "field `{field}` projects from a non-record nominal type"
            )));
        }
        let parameters = self
            .program
            .declarations
            .type_parameters(declaration)
            .ok_or_else(|| hir_error(format!("record `{declaration}` has no parameters")))?;
        if arguments.len() != parameters.len()
            || arguments
                .iter()
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
        {
            return Err(hir_error(format!(
                "field `{field}` projects from an invalid concrete record instance"
            )));
        }
        let template = self
            .program
            .declarations
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|candidate| candidate.id == *field))
            .ok_or_else(|| {
                hir_error(format!(
                    "field `{field}` does not belong to record `{declaration}`"
                ))
            })?;
        substitute_type(&template.ty, declaration, arguments)
    }

    fn argument_transfers(&self, param: &ResolvedParam) -> Result<bool, Diagnostic> {
        self.is_owned_resource(&param.ty, param.ownership)
    }

    fn is_owned_resource(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<bool, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| !facts.copy && ownership == OwnershipMode::Own)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn mark_value_sources_moved(
        &self,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedExpr, usize),
            AfterThen {
                else_branch: &'a ResolvedExpr,
                parent: usize,
                then_scope: usize,
                ids: Vec<ValueId>,
            },
            AfterElse {
                parent: usize,
                else_scope: usize,
                ids: Vec<ValueId>,
                then_bindings: BTreeMap<ValueId, ValidationBinding>,
            },
            AfterMatchArm {
                arms: &'a [ResolvedMatchArm],
                index: usize,
                parent: usize,
                arm_scope: usize,
                ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
            },
        }
        let root = std::mem::take(scope);
        let mut scopes = vec![root];
        let mut frames = vec![Frame::Enter(expression, 0)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(expression, scope_index) => match &expression.kind {
                    ResolvedExprKind::Place(place) => {
                        let Some(binding) = scopes[scope_index].get(&place.root) else {
                            continue;
                        };
                        let (place_ty, place_ownership) = self.resolve_place(place, binding)?;
                        let should_move = self.is_owned_resource(&place_ty, place_ownership)?
                            && Self::place_availability(binding, &place.projections)
                                == Availability::Available;
                        if should_move {
                            let binding =
                                scopes[scope_index].get_mut(&place.root).ok_or_else(|| {
                                    hir_error(format!(
                                    "resolved value `{}` disappeared during ownership validation",
                                    place.root
                                ))
                                })?;
                            if place.projections.is_empty() {
                                binding.availability = Availability::Moved;
                            } else {
                                binding
                                    .moved_places
                                    .insert(place.projections.clone(), Availability::Moved);
                            }
                        }
                    }
                    ResolvedExprKind::Block { tail, .. } => {
                        frames.push(Frame::Enter(tail, scope_index));
                    }
                    ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        let ids = scopes[scope_index].keys().cloned().collect::<Vec<_>>();
                        let then_scope = scopes.len();
                        scopes.push(scopes[scope_index].clone());
                        frames.push(Frame::AfterThen {
                            else_branch,
                            parent: scope_index,
                            then_scope,
                            ids,
                        });
                        frames.push(Frame::Enter(then_branch, then_scope));
                    }
                    ResolvedExprKind::Match { arms, .. } => {
                        if let Some(first) = arms.first() {
                            let ids = scopes[scope_index].keys().cloned().collect::<Vec<_>>();
                            let arm_scope = scopes.len();
                            scopes.push(scopes[scope_index].clone());
                            frames.push(Frame::AfterMatchArm {
                                arms,
                                index: 0,
                                parent: scope_index,
                                arm_scope,
                                ids,
                                arm_scopes: Vec::with_capacity(arms.len()),
                            });
                            frames.push(Frame::Enter(&first.value, arm_scope));
                        }
                    }
                    ResolvedExprKind::Project { base, .. }
                    | ResolvedExprKind::Upcast { source: base } => {
                        frames.push(Frame::Enter(base, scope_index));
                    }
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::ConstructRecord { .. }
                    | ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::UpdateRecord { .. } => {}
                },
                Frame::AfterThen {
                    else_branch,
                    parent,
                    then_scope,
                    ids,
                } => {
                    debug_assert_eq!(then_scope + 1, scopes.len());
                    let then_bindings = scopes.pop().expect("active move branch retained");
                    let else_scope = scopes.len();
                    scopes.push(scopes[parent].clone());
                    frames.push(Frame::AfterElse {
                        parent,
                        else_scope,
                        ids,
                        then_bindings,
                    });
                    frames.push(Frame::Enter(else_branch, else_scope));
                }
                Frame::AfterElse {
                    parent,
                    else_scope,
                    ids,
                    then_bindings,
                } => {
                    debug_assert_eq!(else_scope + 1, scopes.len());
                    let else_bindings = scopes.pop().expect("active move branch retained");
                    Self::join_branches(&mut scopes[parent], &then_bindings, &else_bindings, &ids);
                }
                Frame::AfterMatchArm {
                    arms,
                    index,
                    parent,
                    arm_scope,
                    ids,
                    mut arm_scopes,
                } => {
                    debug_assert_eq!(arm_scope + 1, scopes.len());
                    arm_scopes.push(scopes.pop().expect("active match move branch retained"));
                    let next = index + 1;
                    if let Some(arm) = arms.get(next) {
                        let arm_scope = scopes.len();
                        scopes.push(scopes[parent].clone());
                        frames.push(Frame::AfterMatchArm {
                            arms,
                            index: next,
                            parent,
                            arm_scope,
                            ids,
                            arm_scopes,
                        });
                        frames.push(Frame::Enter(&arm.value, arm_scope));
                    } else if let Some((first, rest)) = arm_scopes.split_first() {
                        let mut joined = first.clone();
                        for arm_scope in rest {
                            Self::join_conditional(&mut joined, arm_scope, &ids);
                        }
                        Self::merge_availability(&mut scopes[parent], &joined, &ids);
                    }
                }
            }
        }
        *scope = scopes.pop().expect("root move scope retained");
        Ok(())
    }

    fn merge_availability(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        source: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(source)) = (target.get_mut(id), source.get(id)) {
                target.availability = source.availability;
                target.moved_places.clone_from(&source.moved_places);
                target
                    .definitely_partial
                    .clone_from(&source.definitely_partial);
            }
        }
    }

    fn join_conditional(
        baseline: &mut BTreeMap<ValueId, ValidationBinding>,
        conditional: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(baseline), Some(conditional)) = (baseline.get_mut(id), conditional.get(id))
            {
                let moved_places = Self::join_moved_places(baseline, conditional);
                let definitely_partial = Self::join_definitely_partial(baseline, conditional);
                baseline.availability = baseline.availability.join(conditional.availability);
                baseline.moved_places = moved_places;
                baseline.definitely_partial = definitely_partial;
            }
        }
    }

    fn join_branches(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        then_scope: &BTreeMap<ValueId, ValidationBinding>,
        else_scope: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(then_value), Some(else_value)) =
                (target.get_mut(id), then_scope.get(id), else_scope.get(id))
            {
                target.availability = then_value.availability.join(else_value.availability);
                target.moved_places = Self::join_moved_places(then_value, else_value);
                target.definitely_partial = Self::join_definitely_partial(then_value, else_value);
            }
        }
    }

    fn place_availability(
        binding: &ValidationBinding,
        requested: &[PlaceProjection],
    ) -> Availability {
        if binding.availability != Availability::Available {
            return binding.availability;
        }
        let mut maybe_moved = false;
        for (moved, state) in &binding.moved_places {
            if path_is_prefix(moved, requested) || path_is_prefix(requested, moved) {
                if *state == Availability::Moved {
                    return Availability::Moved;
                }
                maybe_moved = true;
            }
        }
        if binding
            .definitely_partial
            .iter()
            .any(|partial| path_is_prefix(requested, partial))
        {
            return Availability::Moved;
        }
        if maybe_moved {
            Availability::MaybeMoved
        } else {
            Availability::Available
        }
    }

    fn join_moved_places(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeMap<Vec<PlaceProjection>, Availability> {
        left.moved_places
            .keys()
            .chain(right.moved_places.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|path| {
                let left = left
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let right = right
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let state = left.join(right);
                (state != Availability::Available).then_some((path, state))
            })
            .collect()
    }

    fn join_definitely_partial(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeSet<Vec<PlaceProjection>> {
        let mut candidates = BTreeSet::new();
        for path in left
            .moved_places
            .keys()
            .chain(right.moved_places.keys())
            .chain(left.definitely_partial.iter())
            .chain(right.definitely_partial.iter())
        {
            for length in 0..=path.len() {
                candidates.insert(path[..length].to_vec());
            }
        }
        candidates
            .into_iter()
            .filter(|path| {
                Self::place_availability(left, path) == Availability::Moved
                    && Self::place_availability(right, path) == Availability::Moved
            })
            .collect()
    }

    fn validate_type(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a ResolvedType),
        }
        let mut frames = vec![Frame::Enter(ty)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(
                    ResolvedType::Unit
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
                    | ResolvedType::String,
                ) => {}
                Frame::Enter(ResolvedType::TypeParameter { .. }) => {
                    return Err(hir_error(
                        "uninstantiated type parameters are not valid in executable HIR",
                    ));
                }
                Frame::Enter(
                    ty @ ResolvedType::Nominal {
                        declaration,
                        arguments,
                    },
                ) => {
                    let kind = self
                        .program
                        .declarations
                        .declaration(declaration)
                        .map(|item| item.kind)
                        .filter(|kind| {
                            matches!(
                                kind,
                                DeclarationKind::Resource
                                    | DeclarationKind::Record
                                    | DeclarationKind::Class
                                    | DeclarationKind::Variant
                            )
                        })
                        .ok_or_else(|| {
                            hir_error(format!(
                                "nominal type `{declaration}` is not a resolved type declaration"
                            ))
                        })?;
                    let parameters = self
                        .program
                        .declarations
                        .type_parameters(declaration)
                        .ok_or_else(|| {
                            hir_error(format!("nominal type `{declaration}` has no parameters"))
                        })?;
                    if arguments.len() != parameters.len() {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has incorrect argument arity"
                        )));
                    }
                    if !arguments.is_empty()
                        && (!matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                            || arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            }))
                    {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has unsupported generic arguments"
                        )));
                    }
                    frames.push(Frame::Finish(ty));
                    for argument in arguments.iter().rev() {
                        frames.push(Frame::Enter(argument));
                    }
                }
                Frame::Finish(ty) => {
                    self.program.declarations.type_facts(ty).ok_or_else(|| {
                        hir_error(format!(
                            "type `{}` has no semantic facts",
                            ty.identity_key()
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }

    fn validate_declared_ownership(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        let facts = self.program.declarations.type_facts(ty).ok_or_else(|| {
            hir_error(format!(
                "type `{}` has no semantic facts",
                ty.identity_key()
            ))
        })?;
        if (facts.copy && ownership != OwnershipMode::Value)
            || (!facts.copy && ownership == OwnershipMode::Value)
        {
            return Err(hir_error(format!(
                "type `{}` has an invalid ownership mode",
                ty.identity_key()
            )));
        }
        Ok(())
    }

    fn validate_argument_ownership(
        &self,
        actual: OwnershipMode,
        param: &ResolvedParam,
    ) -> Result<(), Diagnostic> {
        let facts = self
            .program
            .declarations
            .type_facts(&param.ty)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    param.ty.identity_key()
                ))
            })?;
        let valid = if facts.copy {
            actual == OwnershipMode::Value && param.ownership == OwnershipMode::Value
        } else {
            match param.ownership {
                OwnershipMode::Own => actual == OwnershipMode::Own,
                OwnershipMode::Borrow => true,
                OwnershipMode::Shared => actual == OwnershipMode::Shared,
                OwnershipMode::Value => false,
            }
        };
        if valid {
            Ok(())
        } else {
            Err(hir_error(format!(
                "argument ownership is incompatible with parameter `{}`",
                param.id
            )))
        }
    }

    /// Class Inheritance v1: independent re-derivation of the upcast
    /// contract. The source must be a descendant class value whose effective
    /// field sequence extends the ancestor's exactly, with a cleanup-inert
    /// child-declared suffix.
    fn validate_upcast(
        &self,
        expression: &ResolvedExpr,
        source: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let (
            ResolvedType::Nominal {
                declaration: child_id,
                arguments: child_arguments,
            },
            ResolvedType::Nominal {
                declaration: parent_id,
                arguments: parent_arguments,
            },
        ) = (&source.ty, &expression.ty)
        else {
            return Err(hir_error(
                "resolved upcast operands are not nominal classes",
            ));
        };
        if !child_arguments.is_empty() || !parent_arguments.is_empty() {
            return Err(hir_error("resolved upcast has generic class arguments"));
        }
        if !self.program.declarations.class_extends(child_id, parent_id) {
            return Err(hir_error(format!(
                "resolved upcast `{child_id}` does not inherit from `{parent_id}`"
            )));
        }
        let child_fields = self
            .program
            .declarations
            .record_fields(child_id)
            .ok_or_else(|| hir_error(format!("class `{child_id}` has no fields")))?;
        let parent_fields = self
            .program
            .declarations
            .record_fields(parent_id)
            .ok_or_else(|| hir_error(format!("class `{parent_id}` has no fields")))?;
        if child_fields.len() < parent_fields.len()
            || child_fields[..parent_fields.len()]
                .iter()
                .zip(parent_fields.iter())
                .any(|(child_field, parent_field)| child_field.id != parent_field.id)
        {
            return Err(hir_error(format!(
                "resolved upcast `{child_id}` prefix disagrees with ancestor `{parent_id}`"
            )));
        }
        for field in &child_fields[parent_fields.len()..] {
            let drops = self
                .program
                .declarations
                .type_facts(&field.ty)
                .is_some_and(|facts| facts.needs_drop);
            if drops {
                return Err(hir_error(format!(
                    "resolved upcast from `{child_id}` would discard owned field `{}`",
                    field.name
                )));
            }
        }
        let _ = source;
        Ok(())
    }

    fn expected_ownership(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        self.program
            .declarations
            .type_facts(ty)
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy
                }
            })
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(hir_error(format!(
                "{context} has inconsistent resolved types"
            )))
        }
    }

    /// Field Mutation v1 oracle check: the targeted field must exist on a
    /// record/class base, stay a direct scalar Copy field, and match the
    /// assigned value's type exactly.
    fn validate_assign_field(
        &self,
        target_ty: &ResolvedType,
        field: &DeclarationId,
        assigned: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let ResolvedType::Nominal {
            declaration: owner,
            arguments,
        } = target_ty
        else {
            return Err(hir_error("field assignment base is not a record"));
        };
        if self
            .program
            .declarations
            .declaration(owner)
            .is_none_or(|item| {
                !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
            })
        {
            return Err(hir_error("field assignment base is not a record"));
        }
        let declared = self
            .program
            .declarations
            .record_fields(owner)
            .and_then(|fields| fields.iter().find(|item| &item.id == field))
            .map(|item| item.ty.clone())
            .ok_or_else(|| {
                hir_error(format!(
                    "record `{owner}` has no assignment field `{field}`"
                ))
            })?;
        let field_ty =
            crate::hir::substitute_type(&declared, owner, arguments).map_err(|diagnostic| {
                hir_error(format!(
                    "assignment field type substitution failed: {diagnostic}"
                ))
            })?;
        self.require_type(&field_ty, &assigned.ty, "field assignment")?;
        if !crate::hir::is_scalar_resolved_type(&field_ty) {
            return Err(hir_error(
                "field mutation v1 supports only direct scalar Copy record fields",
            ));
        }
        Ok(())
    }

    fn insert_value(&mut self, id: &ValueId) -> Result<(), Diagnostic> {
        reject_nul_identity("resolved value", id.as_str())?;
        if self.value_ids.insert(id.clone()) {
            Ok(())
        } else {
            Err(hir_error(format!(
                "duplicate resolved value identity `{id}`"
            )))
        }
    }
}
