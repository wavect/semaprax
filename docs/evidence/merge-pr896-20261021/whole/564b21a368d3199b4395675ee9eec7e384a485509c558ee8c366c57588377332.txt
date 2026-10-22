//! Fail-closed validation of identity-resolved HIR.
//!
//! Validates HIR meaning; source resolution remains in
//! the parent module.
mod function_lookup;
mod function_scope;
mod identity_directory;
mod identity_insert;
mod identity_scratch;
#[cfg(test)]
mod owned_iterative_oracle;
mod scope_checkpoint;
use scope_checkpoint::{Read as ScopeRead, Write as ScopeWrite};
mod type_requirement;
use super::workspace_link::native_owner::admitted_ri06_regex_result as regex_result;
use super::*;
mod aggregate_metadata;
mod assign_field;
mod continuation_path;
mod path_label;
use crate::loan_plan::{LoanCause, LoanId, LoanPointPhase};
mod borrowed_argument;
mod borrowed_str;
mod box_intrinsic;
mod branch_merge;
mod byte_range;
pub(in crate::hir) mod byte_traversal;
mod call_parameters;
mod call_signature;
mod callable_types;
mod canonical_loans;
mod closure;
pub(in crate::hir) mod constructor_seed;
mod contract_owners;
mod effect_keys;
mod executable_profiles;
mod generic_record_composition;
mod generic_template;
mod held_profiles;
mod host_command;
mod lexical_exit;
mod native_borrow;
mod nominal_facts;
mod owned_buffer;
mod owned_result_try;
pub(super) mod owner_renewal;
mod ownership_flags;
mod proof_return;
mod record_pattern;
mod record_traversal;
mod row_loan;
mod stdin_stream;
mod string_intrinsic;
mod type_profiles;
mod unsafe_scan;
mod upcast;
mod vec_field;
mod vec_intrinsic;
use borrowed_argument::{hir_diagnostic_at_span, hir_error_at_span};
use call_parameters::CallParameters;
#[cfg(test)]
pub(crate) use owner_renewal::validate_core;
pub(crate) use type_profiles::resolved_type_contains_owned_bytes;
use type_profiles::{
    generic_instance_arguments_are_admitted, resolved_type_is_direct_owned_variant,
    resolved_type_is_flat_owned_byte_variant, template_contains_nested_owned_record_type,
    template_has_owned_record_slot, template_ownership, validate_nested_update_base_shape,
};
#[cfg(test)]
use unsafe_scan::contains_unsafe_boundary;
#[derive(Clone)]
pub(super) struct HirValidator<'a> {
    program: &'a ResolvedProgram,
    static_text: std::cell::Cell<(bool, usize)>,
    row_loan_observer: Option<&'a std::cell::Cell<bool>>,
    byte_traversal_observer: Option<&'a std::cell::Cell<bool>>,
    byte_helpers: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    nominal_facts: nominal_facts::Lookup<'a>,
    executable_profiles: executable_profiles::Lookup<'a>,
    functions: function_lookup::Lookup<'a>,
    expression_ids: identity_directory::Set<ExpressionId>,
    value_ids: identity_directory::Set<ValueId>,
    byte_slice_aliases: BTreeMap<ValueId, Place>,
    /// Expression identities of the `bytes_set` calls that are the right-hand
    /// side of a same-owner re-open, `buffer = bytes_set(buffer, index,
    /// value)`. Block statement scheduling records one before its right-hand
    /// side is validated, exactly as source verification does, so hostile HIR
    /// cannot present a `bytes_set` whose buffer operand is a second owner of
    /// an already frozen buffer.
    buffer_reopen_sites: BTreeSet<ExpressionId>,
    borrowed_str_aliases: BTreeMap<ValueId, Place>,
    proof_generic_calls: bool,
    proof_return: Option<(FunctionInstanceId, ResolvedType)>,
    canonical_loans: canonical_loans::Lookup<'a>,
    /// Joined `FusedConstructionV1` selection; `None` on every ordinary path.
    selected: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
}
impl<'a> HirValidator<'a> {
    fn exact_loan_id(
        &self,
        execution: &FunctionExecutionId,
        expression: &ExpressionId,
        cause: &LoanCause,
    ) -> Result<Option<LoanId>, Diagnostic> {
        let _ = execution;
        Ok(self
            .canonical_loans
            .id(&(expression.clone(), cause.clone()))
            .copied())
    }
    #[cfg(test)]
    pub(super) fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        Self::with_directory(program, None)
    }
    #[cfg(test)]
    pub(super) fn with_directory(
        program: &'a ResolvedProgram,
        directory: Option<&'a super::function_directory::Directory<'a>>,
    ) -> Result<Self, Diagnostic> {
        let seed = constructor_seed::ConstructorSeed::authenticate(program, directory)?;
        let canonical_loans = canonical_loans::Lookup::current(&seed)?;
        Self::from_constructor(seed, canonical_loans, None)
    }
    pub(in crate::hir) fn with_storage(
        seed: constructor_seed::ConstructorSeed<'a>,
        storage: &'a super::canonical_loan_storage::Storage<'a>,
    ) -> Result<Self, Diagnostic> {
        if !storage.is_current(seed.program) {
            return Err(hir_error(
                "canonical shared-loan storage does not belong to the current program",
            ));
        }
        Self::from_constructor(seed, canonical_loans::Lookup::Borrowed(storage), None)
    }
    // A nested closure keeps its original constructor order, while the primary
    // invocation owns the one classifier directory through every validator clone.
    fn with_profiles(
        program: &'a ResolvedProgram,
        directory: Option<&'a super::function_directory::Directory<'a>>,
        storage: Option<canonical_loans::Lookup<'a>>,
        profiles: executable_profiles::Lookup<'a>,
    ) -> Result<Self, Diagnostic> {
        let seed = constructor_seed::ConstructorSeed::authenticate(program, directory)?;
        let canonical_loans = if let Some(storage) = storage {
            if !storage.is_current(seed.program) {
                return Err(hir_error(
                    "canonical shared-loan storage does not belong to the current program",
                ));
            }
            storage
        } else {
            canonical_loans::Lookup::current(&seed)?
        };
        Self::from_constructor(seed, canonical_loans, Some(profiles))
    }
    fn with_shared_profiles(
        program: &'a ResolvedProgram,
        functions: std::rc::Rc<super::function_directory::Directory<'a>>,
        storage: Option<canonical_loans::Lookup<'a>>,
        profiles: executable_profiles::Lookup<'a>,
    ) -> Result<Self, Diagnostic> {
        let seed = constructor_seed::ConstructorSeed::authenticate_shared(program, functions)?;
        let canonical_loans = if let Some(storage) = storage {
            if !storage.is_current(seed.program) {
                return Err(hir_error(
                    "canonical shared-loan storage does not belong to the current program",
                ));
            }
            storage
        } else {
            canonical_loans::Lookup::current(&seed)?
        };
        Self::from_constructor(seed, canonical_loans, Some(profiles))
    }
    fn from_constructor(
        seed: constructor_seed::ConstructorSeed<'a>,
        canonical_loans: canonical_loans::Lookup<'a>,
        profiles: Option<executable_profiles::Lookup<'a>>,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            program: seed.program,
            static_text: std::cell::Cell::new((false, 0)),
            row_loan_observer: None,
            byte_traversal_observer: None,
            byte_helpers: None,
            nominal_facts: nominal_facts::Lookup::default(),
            executable_profiles: match profiles {
                Some(profiles) if profiles.is_current(&seed.program.declarations) => profiles,
                Some(_) => {
                    return Err(hir_error_static(
                        "executable profile observations do not belong to the current declaration index",
                    ));
                }
                None => executable_profiles::Lookup::for_program(seed.program)?,
            },
            functions: seed.functions,
            expression_ids: identity_directory::Set::new(),
            value_ids: identity_directory::Set::new(),
            byte_slice_aliases: BTreeMap::new(),
            buffer_reopen_sites: BTreeSet::new(),
            borrowed_str_aliases: BTreeMap::new(),
            proof_generic_calls: false,
            proof_return: None,
            canonical_loans,
            selected: None,
        })
    }
    fn validate(mut self) -> Result<bool, Diagnostic> {
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
            .map(|declaration| &declaration.id)
            .collect::<identity_scratch::Ids>();
        if type_ids.len() != self.program.types.len() {
            return Err(hir_error("duplicate resolved type declaration identity"));
        }
        let mut interface_ids = identity_scratch::Ids::new();
        let mut import_ids = identity_scratch::Ids::new();
        let mut imports = identity_scratch::Imports::new();
        for interface in &self.program.interfaces {
            if !interface_ids.insert(&interface.id) {
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
                if import.selected_receiver.as_deref().is_some_and(|receiver| {
                    !import.index_selected || match receiver {
                        "shared" => {
                            let scalar = matches!(import.parameters.first(), Some(p)
                                if p.ty == ResolvedType::I64 && p.ownership == OwnershipMode::Value);
                            let regex_loan = import.rust_path.as_deref()
                                == Some("regex_alias::Regex::is_match")
                                && matches!(import.parameters.as_slice(), [receiver, text]
                                    if receiver.ownership == OwnershipMode::Borrow
                                        && matches!(receiver.ty, ResolvedType::Nominal { ref declaration, ref arguments }
                                            if arguments.is_empty() && self.program.declarations.declaration(declaration)
                                                .is_some_and(|item| item.kind == DeclarationKind::Resource && item.name == "Regex"))
                                        && text.ownership == OwnershipMode::Borrow
                                        && text.ty == ResolvedType::String);
                            !scalar && !regex_loan && !native_borrow::admitted(self.program, import)
                        },
                        "owned" => !matches!(import.parameters.first(), Some(p) if matches!(p.ty, ResolvedType::Nominal { .. }) && p.ownership == OwnershipMode::Own),
                        _ => true,
                    }
                }) {
                    return Err(hir_error(
                        "selected Rust method receiver has an invalid ownership projection",
                    ));
                }
                if import.index_selected
                    && import
                        .selected_index_digest
                        .as_ref()
                        .is_none_or(|digest| !digest.starts_with("sha256:") || digest.len() != 71)
                {
                    return Err(hir_error(
                        "selected Rust import has no authenticated index digest",
                    ));
                }
                if import.rust_path.as_ref().is_some_and(|path| {
                    !import.native_rust || !crate::native_rust_binding::valid_rust_api_path(path)
                }) {
                    return Err(hir_error(
                        "resolved Rust API path has an invalid declaration shape",
                    ));
                }
                if crate::host_io_ops::by_id(import.id.as_str()).is_some() {
                    return Err(hir_error(format!(
                        "resolved import `{}` aliases a compiler-owned host I/O operation",
                        import.id
                    )));
                }
                if !import_ids.insert(&import.id) || imports.insert(&import.id, import).is_some() {
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
                let ri06_regex_borrow_shape = native_borrow::admitted(self.program, import);
                let native_shape = import.native_rust
                    && (!matches!(import.result.kind, ResolvedImportResultKind::BorrowedStr { .. }) || ri06_regex_borrow_shape)
                    && import.parameters.len() <= 8
                    && (ri06_regex_borrow_shape || import.parameters.iter().all(|parameter| {
                        (parameter.ownership == OwnershipMode::Value
                            && matches!(parameter.ty, ResolvedType::I64 | ResolvedType::Bool))
                            || (parameter.ownership == OwnershipMode::Own
                                && (parameter.ty == ResolvedType::String || ResolvedImportResultKind::is_owned_container_type(&parameter.ty, &self.program.declarations) || matches!(parameter.ty, ResolvedType::Nominal { ref declaration, ref arguments }
                                    if arguments.is_empty() && self.program.declarations.declaration(declaration)
                                        .is_some_and(|item| item.kind == DeclarationKind::Resource))))
                    }))
                    && matches!(
                        &import.result.kind,
                        ResolvedImportResultKind::Unit
                            | ResolvedImportResultKind::I64
                            | ResolvedImportResultKind::Bool
                            | ResolvedImportResultKind::ResultI64I64
                            | ResolvedImportResultKind::BorrowedStr { .. } | ResolvedImportResultKind::OwnedResource { .. } | ResolvedImportResultKind::OwnedResultResourceI64 { .. } | ResolvedImportResultKind::OwnedString | ResolvedImportResultKind::OwnedOptionString | ResolvedImportResultKind::OwnedResultStringI64 | ResolvedImportResultKind::OwnedResultStringOptionI64
                    )
                    && (!matches!(
                        import.result.kind,
                        ResolvedImportResultKind::ResultI64I64
                            | ResolvedImportResultKind::OwnedResultResourceI64 { .. }
                    ) || (import.index_selected && import.selected_index_digest.is_some()));
                let lifecycle_shape = !import.native_rust
                    && import.parameters.len() == 1
                    && import.parameters[0].ownership == OwnershipMode::Own
                    && import.parameters[0].consumes_on_failure
                    && import.result.kind == ResolvedImportResultKind::Unit;
                if (!native_shape && !lifecycle_shape)
                    || import.result.ownership != import.result.kind.ownership()
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
            let transparent_owned_wrapper =
                box_intrinsic::authenticate_owned_wrapper(self.program, template)?;
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
                if !parameter.id.matches_parameter(&execution, index)
                    || parameter.ownership
                        != template_ownership(self.program, template, &parameter.ty)
                {
                    return Err(hir_error(format!(
                        "generic template `{}` has invalid parameter identity or ownership",
                        template.id
                    )));
                }
                self.validate_function_template_type(template, &parameter.ty)?;
            }
            self.validate_function_template_type(template, &template.return_type)?;
            if !template.result_id.matches_result(&execution) {
                return Err(hir_error(format!(
                    "generic template `{}` has invalid result identity",
                    template.id
                )));
            }
            self.validate_template_expressions(template, &execution)?;
            if template_contains_nested_owned_record_type(self.program, template)
                && !template_has_owned_record_slot(self.program, template)
            {
                return Err(hir_error(format!(
                    "generic template `{}` must transfer exactly one bounded owned-record parameter into an identical result type",
                    template.id
                )));
            }
            let substitutions =
                generic_template::substitutions(self.program, template, transparent_owned_wrapper);
            for arguments in substitutions {
                let materialized = materialize_function_template(template, &arguments)?;
                let saved_expression_ids = self.expression_ids.clone();
                let saved_value_ids = self.value_ids.clone();
                self.proof_generic_calls = true;
                self.proof_return = Some((
                    FunctionInstanceId::derive(&template.id, &arguments),
                    materialized.return_type.clone(),
                ));
                let validation = self.validate_function(
                    &materialized,
                    &FunctionExecutionId::Generic(FunctionInstanceId::derive(
                        &template.id,
                        &arguments,
                    )),
                );
                self.proof_generic_calls = false;
                self.proof_return = None;
                validation?;
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
            match &declaration.kind {
                ResolvedTypeDeclarationKind::Record { fields: _ } => {
                    let root = ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    };
                    if resolved_type_contains_owned_bytes(self.program, &root)
                        && !super::type_reachability::is_admitted_nested_owned_byte_record(
                            &self.program.declarations,
                            &root,
                        )
                        && !self.executable_profiles.runtime_admitted(&root)?
                    {
                        return Err(hir_error(
                            "resolved owned-Bytes record is outside the bounded acyclic nested profile",
                        ));
                    }
                }
                ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    if fields.iter().any(|field| {
                        field.ty == ResolvedType::Bytes
                            || resolved_type_contains_owned_bytes(self.program, &field.ty)
                    }) {
                        return Err(hir_error(
                            "resolved class contains compiler-owned Bytes outside flat v1",
                        ));
                    }
                }
                ResolvedTypeDeclarationKind::Variant { cases } => {
                    let fields = cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .collect::<Vec<_>>();
                    let has_direct_bytes =
                        fields.iter().any(|field| field.ty == ResolvedType::Bytes);
                    let has_direct_string =
                        fields.iter().any(|field| field.ty == ResolvedType::String);
                    if has_direct_bytes
                        && (!declaration.type_parameters.is_empty()
                            || fields.iter().any(|field| {
                                field.ty != ResolvedType::Bytes
                                && !super::type_reachability::nested_record_copy_scalar_is_admitted(
                                    &field.ty,
                                )
                            }))
                    {
                        return Err(hir_error(
                            "resolved owned-Bytes variant is not flat and monomorphic",
                        ));
                    }
                    let nested_outcome = super::collection_outcome::nested::admitted(
                        &self.program.declarations,
                        &ResolvedType::Nominal {
                            declaration: declaration.id.clone(),
                            arguments: Vec::new(),
                        },
                    );
                    if !has_direct_bytes
                        && !nested_outcome
                        && fields.iter().any(|field| {
                            resolved_type_contains_owned_bytes(self.program, &field.ty)
                        })
                    {
                        return Err(hir_error(
                            "resolved variant contains compiler-owned Bytes outside flat v1",
                        ));
                    }
                    // Site 4 of the five admitted-variant-payload-profile
                    // classifiers named in issue #261. Shares its field
                    // admission test with site 3,
                    // `type_reachability::is_admitted_owned_string_variant`,
                    // through the one owning resolved-level ("ResolvedType")
                    // predicate `type_reachability::
                    // is_admitted_owned_string_variant_field`. The
                    // source-level ("Type") twin,
                    // `TypeTable::is_admitted_owned_string_variant_field`, is
                    // consulted by sites 1 and 2 before resolution; site 5,
                    // `Interpreter::value_has_type` (`SPX-F105`), checks a
                    // runtime value rather than a field type but consults the
                    // same underlying `is_admitted_copy_aggregate_variant_field`.
                    if has_direct_string
                        && !has_direct_bytes
                        && (!declaration.type_parameters.is_empty()
                            || fields.iter().any(|field| {
                                !super::type_reachability::is_admitted_owned_string_variant_field(
                                    &self.program.declarations,
                                    &field.ty,
                                )
                            }))
                    {
                        return Err(hir_error(
                            "resolved owned-string variant is not direct and monomorphic",
                        ));
                    }
                }
                ResolvedTypeDeclarationKind::Resource { .. } => {}
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
                        if field.ty == ResolvedType::Str {
                            return Err(hir_error(format!(
                                "field `{}` cannot store borrowed `str`",
                                field.id
                            )));
                        }
                        if field.ty == ResolvedType::SliceU8 {
                            return Err(hir_error(format!(
                                "field `{}` cannot store borrowed `Slice<u8>`",
                                field.id
                            )));
                        }
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
                                && !crate::map_ops::is_collection(&field.ty)
                                && !type_profiles::record_field_storage_instance_is_admitted(
                                    self.program,
                                    &declaration.id,
                                    &field.ty,
                                )
                                && self
                                    .program
                                    .declarations
                                    .declaration(field_declaration)
                                    .is_some_and(|item| item.kind == DeclarationKind::Record)
                                && !super::type_reachability::is_admitted_nested_owned_byte_record(
                                    &self.program.declarations,
                                    &field.ty,
                                )
                            {
                                return Err(hir_error(format!(
                                    "field `{}` nests a generic record instance outside the admitted slice",
                                    field.id
                                )));
                            }
                        }
                    } else {
                        match &field.ty {
                            ResolvedType::Never => {
                                return Err(hir_error(
                                    "compiler-only completion marker cannot be a field type",
                                ));
                            }
                            ResolvedType::I64 | ResolvedType::Bool => {}
                            ResolvedType::I32
                            | ResolvedType::Char
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::ArrayU8(_)
                            | ResolvedType::F32
                            | ResolvedType::F64
                            | ResolvedType::String
                            | ResolvedType::Bytes
                            | ResolvedType::Str
                            | ResolvedType::OnceFunction
                            | ResolvedType::OnceFunctionI64
                            | ResolvedType::OnceFunctionI64Pair
                            | ResolvedType::MutFunctionI64
                            | ResolvedType::Function { .. }
                            | ResolvedType::SliceU8
                            | ResolvedType::StringMap => {
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
                let collection_outcome = super::collection_outcome::runtime_admitted(
                    &self.program.declarations,
                    &ResolvedType::Nominal {
                        declaration: declaration.id.clone(),
                        arguments: Vec::new(),
                    },
                );
                let owned_byte_variant = declaration.type_parameters.is_empty()
                    && cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .any(|field| field.ty == ResolvedType::Bytes);
                let owned_string_variant = declaration.type_parameters.is_empty()
                    && !owned_byte_variant
                    && cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .any(|field| field.ty == ResolvedType::String);
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
                            || !(super::type_reachability::nested_record_copy_scalar_is_admitted(
                                &field.ty,
                            ) || matches!(field.ty, ResolvedType::TypeParameter { .. })
                                || (owned_byte_variant && field.ty == ResolvedType::Bytes)
                                || (owned_string_variant && field.ty == ResolvedType::String)
                                || (collection_outcome && (super::collection_outcome::field_admitted(&self.program.declarations,&field.ty) || super::collection_outcome::nested::record_payload_admitted(&self.program.declarations, &field.ty)))
                                || (declaration.type_parameters.is_empty()
                                    && super::type_reachability::is_admitted_copy_aggregate_variant_field(
                                        &self.program.declarations,
                                        &field.ty,
                                    ))
                                || crate::iterator_ops::is_step_rest_field(
                                    &declaration.id,
                                    &case.id,
                                    field,
                                )
                                || crate::list_ops::is_step_tail_field(
                                    &declaration.id,
                                    &case.id,
                                    field,
                                ))
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
                        if owned_byte_variant
                            || collection_outcome
                            || owned_string_variant
                            || crate::iterator_ops::is_step_rest_field(
                                &declaration.id,
                                &case.id,
                                field,
                            )
                            || crate::list_ops::is_step_tail_field(
                                &declaration.id,
                                &case.id,
                                field,
                            )
                            || super::type_reachability::nested_record_copy_scalar_is_admitted(
                                &field.ty,
                            )
                            || (declaration.type_parameters.is_empty()
                                && super::type_reachability::is_admitted_copy_aggregate_variant_field(
                                    &self.program.declarations,
                                    &field.ty,
                                ))
                        {
                            continue;
                        }
                        match &field.ty {
                            ResolvedType::Never => {
                                return Err(hir_error(
                                    "compiler-only completion marker cannot be a field type",
                                ));
                            }
                            ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::Char
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::F32
                            | ResolvedType::F64
                            | ResolvedType::Bool
                            | ResolvedType::ArrayU8(_)
                            | ResolvedType::String
                            | ResolvedType::Bytes
                            | ResolvedType::Str
                            | ResolvedType::OnceFunction
                            | ResolvedType::OnceFunctionI64
                            | ResolvedType::OnceFunctionI64Pair
                            | ResolvedType::MutFunctionI64
                            | ResolvedType::Function { .. }
                            | ResolvedType::SliceU8
                            | ResolvedType::StringMap => {
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
                    let valid_facts = cached.as_ref().is_some_and(|facts| {
                        if owned_byte_variant || owned_string_variant || collection_outcome {
                            !facts.copy
                                && !facts.contains_resource
                                && facts.needs_drop
                                && facts.sized
                        } else {
                            facts.copy
                                && !facts.contains_resource
                                && !facts.needs_drop
                                && facts.sized
                        }
                    });
                    if cached.is_none() || cached != recomputed || !valid_facts {
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
                    *id != &actual.id
                        || *template != &actual.template
                        || *arguments != actual.type_arguments.as_slice()
                })
        {
            return Err(hir_error(
                "materialized generic function instances are not the exact reachable sequence",
            ));
        }
        for instance in &self.program.function_instances {
            if !instance_ids.insert(instance.id.clone())
                || !super::ids::instance_match::instance_matches!(
                    &instance.id,
                    &instance.template,
                    &instance.type_arguments
                )
                || instance.function.id != instance.template
                || !generic_instance_arguments_are_admitted(
                    self.program,
                    &instance.template,
                    &instance.type_arguments,
                )
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
        let expected_byte_roots =
            derive_byte_slice_provenance(&self.program.functions, &self.program.declarations)?;
        if self.program.declarations.byte_slice_roots != expected_byte_roots {
            return Err(hir_error(
                "byte-slice symbolic provenance is missing, forged, or non-canonical",
            ));
        }
        // Rebuild the target-neutral storage/call-flow projection from the
        // hostile HIR rather than trusting Graph or backend-attached totals.
        self.analyze_capacity()?;
        self.finish_static_pool()
    }

    fn validate_template_expr_with_context<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
        aggregate_context: (bool, bool),
    ) -> Result<(), Diagnostic> {
        let (_allow_record_reconstruction, allow_aggregate_root) = aggregate_context;
        if !expression.id.matches(execution, path)
            || !identity_insert::expression(&mut self.expression_ids, &expression.id)?
            || !generic_template::expression_ownership_is_valid(self.program, template, expression)
        {
            return Err(hir_error(format!(
                "generic template `{}` has invalid expression identity or ownership",
                template.id
            )));
        }
        if generic_template::body_type_requires_validation(self.program, template, &expression.ty) {
            self.validate_function_template_type(template, &expression.ty)?;
        }
        if generic_variant::handles(self.program, template, expression) {
            return self
                .validate_template_authored_variant(template, execution, expression, values, path);
        }
        if generic_template::is_owned_record_expression(expression)
            && !generic_template::has_owned_record_composition(self.program, template)
        {
            return Err(hir_error(
                "generic aggregate expression requires an exact owned-record relay template",
            ));
        }
        match &expression.kind {
            ResolvedExprKind::LexicalExit { value, .. } => self.validate_template_lexical_exit(
                template,
                execution,
                expression,
                value.as_deref(),
                values,
                path,
            )?,

            ResolvedExprKind::Closure { .. } => {
                self.validate_template_closure(template, execution, expression, values, path)?
            }
            ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. } => {
                self.validate_template_callable(template, execution, expression, values, path)?
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
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::String(value) => self.record_static_pool(value.len(), false)?,
            ResolvedExprKind::BorrowedUtf8Literal(value) => {
                self.finish_static_text(expression, value)?
            }
            ResolvedExprKind::Place(place) => {
                if *generic_template::projected_place_type(self.program, place, values)?
                    != expression.ty
                {
                    return Err(hir_error(
                        "generic template place is out of scope or has the wrong type",
                    ));
                }
                generic_template::validate_template_place(
                    self.program,
                    template,
                    expression,
                    place,
                    values,
                )?;
            }
            ResolvedExprKind::ByteRange { .. } => {
                return Err(hir_error(
                    "generic templates cannot construct dynamic byte ranges",
                ));
            }
            ResolvedExprKind::LiteralFormat { .. } | ResolvedExprKind::VecFieldRead { .. } => {
                return Err(hir_error(
                    "generic templates cannot construct literal formats or scoped vector reads",
                ));
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                let transparent_vec_call = generic_template::is_vec_wrapper_call(
                    self.program,
                    template,
                    callee,
                    type_arguments,
                    instance,
                );
                let forwarded_generic_call = generic_template::is_forwarded_call(
                    self.program,
                    template,
                    callee,
                    type_arguments,
                    instance,
                );
                if (!transparent_vec_call && !forwarded_generic_call)
                    && (instance.is_some() || !type_arguments.is_empty())
                    || (!transparent_vec_call
                        && !forwarded_generic_call
                        && crate::string_ops::by_id(callee.as_str()).is_none()
                        && crate::str_ops::by_id(callee.as_str()).is_none()
                        && self
                            .program
                            .functions
                            .iter()
                            .all(|target| target.id != *callee))
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
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                super::vec_constructor::authenticate(expression)?;
                let mut block_values = values.checkpoint()?;
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            self.validate_template_expr_with_context(
                                template,
                                execution,
                                value,
                                &mut block_values,
                                &format!("{statement_path}.value"),
                                (false, allow_aggregate_root),
                            )?;
                            if !binding.id.matches_local(execution, &statement_path)
                                || binding.ownership
                                    != template_ownership(self.program, template, &binding.ty)
                                || binding.ty != value.ty
                            {
                                return Err(hir_error("generic template binding is not canonical"));
                            }
                            // Char and the rooted `str` view are admitted body
                            // types but never signature slots, exactly as in
                            // the expression check above.
                            if binding.ty != ResolvedType::Char && binding.ty != ResolvedType::Str {
                                self.validate_function_template_type(template, &binding.ty)?;
                            }
                            self.insert_value(&binding.id)?;
                            block_values.insert(binding.id.clone(), &binding.ty)?;
                        }
                        ResolvedStatement::CopyDiscard { .. } => {
                            return Err(hir_error(
                                "generic templates do not admit lexical discard statements",
                            ));
                        }
                        ResolvedStatement::Assign { .. } => self.validate_template_mutation(
                            template,
                            execution,
                            statement,
                            &mut block_values,
                            &statement_path,
                        )?,
                        ResolvedStatement::Unsafe { body, .. } => {
                            self.validate_template_expr(
                                template,
                                execution,
                                body,
                                &mut block_values,
                                &format!("{statement_path}.body"),
                            )?;
                        }
                        ResolvedStatement::While { .. }
                        | ResolvedStatement::LexicalWhile { .. } => self
                            .validate_template_mutation(
                                template,
                                execution,
                                statement,
                                &mut block_values,
                                &statement_path,
                            )?,
                    }
                }
                self.validate_template_expr_with_context(
                    template,
                    execution,
                    tail,
                    &mut block_values,
                    &format!("{path}.tail"),
                    (false, allow_aggregate_root),
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
                    &mut values.checkpoint()?,
                    &format!("{path}.condition"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    then_branch,
                    &mut values.checkpoint()?,
                    &format!("{path}.then"),
                )?;
                self.validate_template_expr(
                    template,
                    execution,
                    else_branch,
                    &mut values.checkpoint()?,
                    &format!("{path}.else"),
                )?;
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Match { .. } => self
                .validate_composed_template_record(template, execution, expression, values, path)?,
            ResolvedExprKind::ConstructVariant { .. } | ResolvedExprKind::Try { .. } => self
                .validate_template_result_expression(
                    template, execution, expression, values, path,
                )?,
            // Resumable Effects v1 (issue #204): unreachable in practice --
            // `hir::resolve_yield` refuses `yields` on any generic function
            // before a template is ever built -- but still outside this
            // slice's direct-scalar admission if it were ever reached.
            ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::RowLoan { .. }
            | ResolvedExprKind::Yield { .. } => {
                return Err(hir_error(
                    "generic template expression is outside the direct-scalar slice",
                ));
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                // The one admitted template view is the rooted owned-String
                // view, mirroring the reference validator's "one unprojected
                // named storage root" rule for `ByteOp::StringAsStr`. Every
                // projected byte-field view, every other byte operation, and
                // every other result type stays outside the slice.
                if crate::byte_ops::by_id(operation.as_str())
                    != Some(crate::byte_ops::ByteOp::StringAsStr)
                    || !place.projections.is_empty()
                    || values.get(&place.root) != Some(&ResolvedType::String)
                    || expression.ty != ResolvedType::Str
                {
                    return Err(hir_error(
                        "generic template byte view is outside the direct-scalar slice",
                    ));
                }
            }
            ResolvedExprKind::HostCommandCall(_) => {
                return Err(hir_error(
                    "generic template command I/O is outside the direct-scalar slice",
                ));
            }
        }
        Ok(())
    }
    fn reachable_function_instances(&self) -> Result<generic_template::Reachable<'a>, Diagnostic> {
        generic_template::reachable_instances(self.program)
    }
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "retained original map-entry adapter for independent test receivers"
    )]
    fn validate_expr(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<effect_keys::Allowed<'_>>,
    ) -> Result<(), Diagnostic> {
        self.validate_expr_iterative_effects(
            function,
            expression,
            scope,
            path.into(),
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
    fn validate_expr_iterative_effects(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        path: path_label::Label<'_>,
        allow_moves: bool,
        allowed_effects: Option<effect_keys::Allowed<'_>>,
    ) -> Result<(), Diagnostic> {
        let program = self.program;
        enum Frame<'e> {
            LexicalReturn {
                expression: &'e ResolvedExpr,
                operand: &'e ResolvedExpr,
            },
            InvokeNext {
                expression: &'e ResolvedExpr,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
            },
            InvokeAfter {
                expression: &'e ResolvedExpr,
                index: usize,
                path: &'e str,
            },
            RestorePublication(bool),
            Enter {
                expression: &'e ResolvedExpr,
                scope: scope_checkpoint::Scope,
                path: path_label::Label<'e>,
            },
            Unary {
                expression: &'e ResolvedExpr,
                op: UnaryOp,
            },
            ByteRangeAfterSource {
                expression: &'e ResolvedExpr,
                start: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
                path: &'e str,
            },
            ByteRangeAfterStart {
                expression: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
                path: &'e str,
            },
            ByteRangeAfterEnd {
                expression: &'e ResolvedExpr,
            },
            BinaryLeft {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                right: &'e ResolvedExpr,
                path: &'e str,
            },
            BinaryRight {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                left: &'e ResolvedExpr,
                baseline: Option<(scope_checkpoint::IdSnapshot, scope_checkpoint::Scope)>,
            },
            IfCondition {
                expression: &'e ResolvedExpr,
                then_branch: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: &'e str,
            },
            IfThen {
                expression: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: &'e str,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
            },
            IfElse {
                expression: &'e ResolvedExpr,
                then_scope: scope_checkpoint::Scope,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
            },
            Project {
                expression: &'e ResolvedExpr,
                field: &'e DeclarationId,
            },
            Upcast {
                expression: &'e ResolvedExpr,
            },
            /// Resumable Effects v1 (issue #204). See `hir::resolve_yield`.
            Yield {
                expression: &'e ResolvedExpr,
            },
            Try {
                expression: &'e ResolvedExpr,
                path: &'e str,
                option: bool,
            },
            CallNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: CallParameters<'e>,
                return_type: call_signature::ResultType<'e>,
                return_ownership: OwnershipMode,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
            },
            CallAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: CallParameters<'e>,
                return_type: call_signature::ResultType<'e>,
                return_ownership: OwnershipMode,
                index: usize,
                path: &'e str,
            },
            NativeNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: &'e [ResolvedImportParameter],
                result: ResolvedType,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
            },
            NativeAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: &'e [ResolvedImportParameter],
                result: ResolvedType,
                index: usize,
                path: &'e str,
            },
            BlockNext {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: scope_checkpoint::Scope,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
            },
            BlockAfterLet {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
            },
            BlockAfterAssign {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: scope_checkpoint::Scope,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
            },
            BlockAfterUnsafe {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
            },
            BlockAfterWhileCondition {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
                body: &'e ResolvedExpr,
                entry: scope_checkpoint::Scope,
            },
            BlockAfterWhileBody {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                path: &'e str,
                entry: scope_checkpoint::Scope,
            },
            BlockTail {
                expression: &'e ResolvedExpr,
                outer_ids: scope_checkpoint::IdSnapshot,
                outer: scope_checkpoint::Scope,
            },
            RecordNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
            },
            RecordAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
            },
            VariantNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                variant: &'e DeclarationId,
                case: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
            },
            VariantAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                variant: &'e DeclarationId,
                case: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
            },
            UpdateBase {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [ResolvedFieldInitializer],
                path: &'e str,
            },
            UpdateNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: scope_checkpoint::Scope,
                path: &'e str,
                ownership: OwnershipMode,
            },
            UpdateAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
                ownership: OwnershipMode,
            },
            MatchScrutinee {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                path: &'e str,
            },
            RecordMatchArm {
                expression: &'e ResolvedExpr,
                arm: &'e ResolvedMatchArm,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                borrowed_root: Option<ValueId>,
            },
            VariantMatchNext {
                expression: &'e ResolvedExpr,
                cases: &'e [ResolvedVariantCaseDeclaration],
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
                covered: variant_or::Covered<'e>,
                wildcard_seen: bool,
                result: Option<&'e ResolvedExpr>,
                path: &'e str,
            },
            VariantMatchAfterArm {
                expression: &'e ResolvedExpr,
                cases: &'e [ResolvedVariantCaseDeclaration],
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
                covered: variant_or::Covered<'e>,
                wildcard_seen: bool,
                result: Option<&'e ResolvedExpr>,
                path: &'e str,
            },
            /// Refutable Match v1 decision-chain twins. They mirror the
            /// resolver's Copy-scalar admission structurally, fail-closed.
            ScalarMatchNext {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
                result: Option<&'e ResolvedExpr>,
                path: &'e str,
            },
            ScalarMatchAfterGuard {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
                result: Option<&'e ResolvedExpr>,
                path: &'e str,
            },
            ScalarMatchAfterArm {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: scope_checkpoint::Scope,
                outer_ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Scope>,
                result: Option<&'e ResolvedExpr>,
                path: &'e str,
            },
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() <= super::VALIDATOR_FRAME_BYTES) };
        #[cfg(test)]
        lexical_exit::observe_frame_layout::<Frame<'static>>(
            std::mem::size_of::<Frame<'static>>(),
            std::mem::align_of::<Frame<'static>>(),
        );
        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame<'_>) -> usize {
            let ids = |values: &scope_checkpoint::IdSnapshot| values.retained_capacity();
            let scope = |scope: &scope_checkpoint::Scope| scope.retained_capacity();
            match frame {
                Frame::InvokeNext { scope: value, .. } | Frame::Enter { scope: value, .. } => {
                    scope(value)
                }
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
                    params.owned_capacity()
                        + call_signature::owned_capacity(return_type)
                        + scope(value)
                }
                Frame::CallAfterArg {
                    params,
                    return_type,
                    ..
                } => params.owned_capacity() + call_signature::owned_capacity(return_type),
                Frame::NativeNext {
                    result,
                    scope: value,
                    ..
                } => resolved_type_owned_capacity(result) + scope(value),
                Frame::NativeAfterArg { result, .. } => resolved_type_owned_capacity(result),
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
                    seen, scope: value, ..
                }
                | Frame::VariantNext {
                    seen, scope: value, ..
                }
                | Frame::UpdateNext {
                    seen, scope: value, ..
                } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + scope(value)
                }
                Frame::RecordAfterField { seen, .. }
                | Frame::VariantAfterField { seen, .. }
                | Frame::UpdateAfterField { seen, .. } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                Frame::RecordMatchArm {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::VariantMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    ..
                }
                | Frame::VariantMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity() * std::mem::size_of::<scope_checkpoint::Scope>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                        + covered.owned_capacity()
                }
                Frame::ScalarMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                }
                | Frame::ScalarMatchAfterGuard {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                }
                | Frame::ScalarMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity() * std::mem::size_of::<scope_checkpoint::Scope>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                }
                _ => 0,
            }
        }
        let mut publication = scope_checkpoint::Publication::take(scope)?;
        let initial_scope = publication.published.clone();
        let mut frames = scope_checkpoint::Storage::new();
        frames.push(Frame::Enter {
            expression,
            scope: initial_scope,
            path,
        })?;
        let mut scopes = scope_checkpoint::Storage::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                1,
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + scopes.capacity() * std::mem::size_of::<scope_checkpoint::Scope>()
                    + scopes
                        .iter()
                        .map(scope_checkpoint::Scope::retained_capacity)
                        .sum::<usize>()
                    + publication.published.retained_capacity()
                    + frames.iter().map(frame_owned_capacity).sum::<usize>()
                    + frame_owned_capacity(&frame)
                    + self.nominal_facts.owned_capacity()
                    + self.executable_profiles.owned_capacity()
                    + self
                        .byte_helpers
                        .map_or(0, |index| index.owned_capacity_bytes())
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
                Frame::InvokeNext {
                    expression,
                    index,
                    mut scope,
                    path,
                } => {
                    let ResolvedExprKind::Invoke { callable, args } = &expression.kind else {
                        unreachable!()
                    };
                    let child = if index == 0 {
                        Some(callable.as_ref())
                    } else {
                        args.get(index - 1)
                    };
                    if let Some(child) = child {
                        let child_path = if index == 0 {
                            path_label::Label::child(path, ".callable", [0, 0])?
                        } else {
                            path_label::Label::child(path, ".arg.{}", [index - 1, 0])?
                        };
                        frames.push(Frame::InvokeAfter {
                            expression,
                            index,
                            path,
                        })?;
                        frames.push(Frame::Enter {
                            expression: child,
                            scope,
                            path: child_path,
                        })?;
                    } else {
                        self.finish_affine_invocation(expression, &mut scope, allow_moves)?;
                        scopes.push(scope)?;
                    }
                }
                Frame::InvokeAfter {
                    expression,
                    index,
                    path,
                } => {
                    let scope = scopes.pop().expect("invocation child scope");
                    frames.push(Frame::InvokeNext {
                        expression,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                Frame::RestorePublication(enabled) => publication.enabled = enabled,
                Frame::Enter {
                    expression,
                    mut scope,
                    path,
                } => {
                    reject_nul_identity("resolved expression", expression.id.as_str())?;
                    if !expression.id.matches_display(function, path.len(), path) {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "expression `{}` has a non-canonical identity",
                            expression.id
                        )));
                    }
                    if !identity_insert::expression(&mut self.expression_ids, &expression.id)? {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "duplicate resolved expression identity `{}`",
                            expression.id
                        )));
                    }
                    self.validate_type(&expression.ty)?;
                    let expected_path = path;
                    let path = continuation_path::after_header(expression, expected_path.len());
                    match &expression.kind {
                        ResolvedExprKind::LexicalExit { value, .. } => {
                            if let Some(operand) = value {
                                frames.push(Frame::LexicalReturn {
                                    expression,
                                    operand,
                                })?;
                                frames.push(Frame::Enter {
                                    expression: operand,
                                    scope,
                                    path: path_label::Label::child(path, ".value", [0, 0])?,
                                })?;
                            } else {
                                self.finish_expr(
                                    expression,
                                    &ResolvedType::Never,
                                    OwnershipMode::Value,
                                )?;
                                scopes.push(scope)?;
                            }
                        }
                        ResolvedExprKind::Closure { .. } => {
                            self.validate_closure(
                                function,
                                expression,
                                &mut scope,
                                path,
                                allow_moves,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::FunctionReference { target } => {
                            super::function_value::validate_reference(
                                self.program,
                                target,
                                &expression.ty,
                            )?;
                            self.finish_expr(expression, &expression.ty, OwnershipMode::Value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Invoke { .. } => {
                            super::function_value::validate_invocation(expression)?;
                            frames.push(Frame::InvokeNext {
                                expression,
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::Int(_) => {
                            self.finish_expr(expression, &ResolvedType::I64, OwnershipMode::Value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Int32(_) => {
                            self.finish_expr(expression, &ResolvedType::I32, OwnershipMode::Value)?;
                            scopes.push(scope)?;
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
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Uint8(_) => {
                            self.finish_expr(expression, &ResolvedType::U8, OwnershipMode::Value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Usize(_) => {
                            self.finish_expr(
                                expression,
                                &ResolvedType::Usize,
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::BorrowPlace { operation, place } => {
                            let ty = self.validate_borrowed_view(
                                operation,
                                place,
                                expression.span,
                                &scope,
                            )?;
                            self.finish_expr(expression, &ty, OwnershipMode::Borrow)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::ByteRange {
                            operation,
                            source,
                            start,
                            end,
                        } => {
                            if operation.as_str() != crate::byte_ops::RANGE_ID {
                                return Err(hir_error(
                                    "byte range has an invalid compiler-owned operation",
                                ));
                            }
                            self.byte_range_origin(source, &scope)?;
                            frames.push(Frame::ByteRangeAfterSource {
                                expression,
                                start,
                                end,
                                path,
                            })?;
                            frames.push(Frame::Enter {
                                expression: source,
                                scope,
                                path: path_label::Label::child(path, ".arg.0", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::ArrayU8(values) => {
                            let length = u32::try_from(values.len()).map_err(|_| {
                                hir_error("fixed-array literal length does not fit u32")
                            })?;
                            self.finish_expr(
                                expression,
                                &ResolvedType::ArrayU8(length),
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::RepeatArrayU8 { count, .. } => {
                            if u64::from(*count) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
                                return Err(hir_error(
                                    "fixed-array repeat exceeds the exact byte bound",
                                ));
                            }
                            self.finish_expr(
                                expression,
                                &ResolvedType::ArrayU8(*count),
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Float32(bits) => {
                            self.validate_finite_f32(*bits)?;
                            self.finish_expr(expression, &ResolvedType::F32, OwnershipMode::Value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Float64(bits) => {
                            self.validate_finite_f64(*bits)?;
                            self.finish_expr(expression, &ResolvedType::F64, OwnershipMode::Value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Bool(_) => {
                            self.finish_expr(
                                expression,
                                &ResolvedType::Bool,
                                OwnershipMode::Value,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::BorrowedUtf8Literal(value) => {
                            self.finish_static_text(expression, value)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::String(value) => {
                            self.record_static_pool(value.len(), false)?;
                            self.finish_expr(
                                expression,
                                &ResolvedType::String,
                                OwnershipMode::Own,
                            )?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Place(place) => {
                            let binding = scope.get(&place.root).ok_or_else(|| {
                                scope_checkpoint::paths::diagnostic(format_args!(
                                    "resolved value `{}` is out of scope",
                                    place.root
                                ))
                            })?;
                            match (place.projections.is_empty(), binding.availability) {
                                (true, Availability::Available) => {
                                    match Self::place_availability(binding, &[]) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(scope_checkpoint::paths::diagnostic(
                                                format_args!(
                                                    "resolved value `{}` is partially moved",
                                                    place.root
                                                ),
                                            ));
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(scope_checkpoint::paths::diagnostic(
                                                format_args!(
                                                    "resolved value `{}` may be partially moved",
                                                    place.root
                                                ),
                                            ));
                                        }
                                    }
                                }
                                (true, Availability::Moved) => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved value `{}` is used after it was moved",
                                        place.root
                                    )));
                                }
                                (true, Availability::MaybeMoved) => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved value `{}` may have been moved",
                                        place.root
                                    )));
                                }
                                (false, _) => {
                                    match Self::place_availability(binding, &place.projections) {
                                        Availability::Available => {}
                                        Availability::Moved => {
                                            return Err(scope_checkpoint::paths::diagnostic(
                                                format_args!(
                                                "resolved place rooted at `{}` is partially moved",
                                                place.root
                                            ),
                                            ));
                                        }
                                        Availability::MaybeMoved => {
                                            return Err(scope_checkpoint::paths::diagnostic(
                                                format_args!(
                                                    "resolved place rooted at `{}` may be conditionally moved",
                                                    place.root
                                                ),
                                            ));
                                        }
                                    }
                                }
                            }
                            let (ty, ownership) = self.resolve_place(place, binding)?;
                            self.finish_expr(expression, &ty, ownership)?;
                            scopes.push(scope)?;
                        }
                        ResolvedExprKind::Unary { op, value } => {
                            frames.push(Frame::Unary {
                                expression,
                                op: *op,
                            })?;
                            frames.push(Frame::Enter {
                                expression: value,
                                scope,
                                path: path_label::Label::child(path, ".value", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } => {
                            let owned_generic_intrinsic = box_intrinsic::is_call(callee, instance);
                            match instance {
                                None if !type_arguments.is_empty() && !owned_generic_intrinsic => {
                                    return Err(hir_error(
                                        "monomorphic resolved call carries generic type arguments",
                                    ));
                                }
                                Some(actual)
                                    if FunctionInstanceId::derive(callee, type_arguments)
                                        != *actual =>
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
                            if !owned_generic_intrinsic
                                && !generic_instance_arguments_are_admitted(
                                    self.program,
                                    callee,
                                    type_arguments,
                                )
                            {
                                return Err(hir_error(
                                    "resolved call has a generic type argument outside the direct-scalar or owned-record relay profile",
                                ));
                            }
                            let mut intrinsic_parameters = None;
                            let mut borrowed_parameters = None;
                            let mut held_parameters = None;
                            let retained_program = self.program;
                            let (params, return_type) = if let Some(signature) =
                                self.intrinsic_signature(callee, type_arguments, instance, args)?
                            {
                                (signature.0, call_signature::ResultType::Owned(signature.1))
                            } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                                string_intrinsic::take_shape(expression, allow_moves)?;
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "string operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "string operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                intrinsic_parameters = Some(CallParameters::String(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "borrowed string operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "borrowed string operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                intrinsic_parameters = Some(CallParameters::Str(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "byte operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "byte operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                owned_buffer::require_admitted_chain(
                                    op,
                                    args,
                                    self.buffer_reopen_sites.contains(&expression.id),
                                )?;
                                intrinsic_parameters = Some(CallParameters::Byte(op));
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                                if instance.is_some() || !type_arguments.is_empty() {
                                    return Err(hir_error(
                                        "host I/O operation call must be monomorphic",
                                    ));
                                }
                                if args.len() != op.arity() {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "host I/O operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                                }
                                match allowed_effects {
                                    Some(allowed) if !allowed.contains(op.effect()) => {
                                        return Err(scope_checkpoint::paths::diagnostic(
                                            format_args!(
                                            "call to `{callee}` requires undeclared effect `{}`",
                                            op.effect()
                                        ),
                                        ));
                                    }
                                    None => {
                                        return Err(hir_error(
                                            "contract calls effectful host I/O operation",
                                        ));
                                    }
                                    _ => {}
                                }
                                (
                                    crate::host_io_ops::resolved_params(op),
                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else {
                                let retained_target = if self.executable_profiles.is_bound() {
                                    held_parameters =
                                        self.executable_profiles.call(callee, instance.as_ref());
                                    held_parameters.map(|call| call.function())
                                } else {
                                    retained_program.resolve_call_target(callee, instance.as_ref())
                                };
                                let (params, return_type, target_effects) = if let Some(target) =
                                    retained_target
                                {
                                    borrowed_parameters = Some(target.params.as_slice());
                                    (
                                        Vec::new(),
                                        call_signature::ResultType::Borrowed(&target.return_type),
                                        std::borrow::Cow::Borrowed(target.effects.as_slice()),
                                    )
                                } else if self.proof_generic_calls {
                                    let (params, result, effects) =
                                        generic_template::proof_signature(
                                            self.program,
                                            callee,
                                            instance.as_ref(),
                                            type_arguments,
                                        )?;
                                    (
                                        params,
                                        call_signature::ResultType::Owned(result),
                                        std::borrow::Cow::Owned(effects),
                                    )
                                } else {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved callee `{callee}` is not indexed"
                                    )));
                                };
                                let parameter_count = borrowed_parameters
                                    .map_or(params.len(), |parameters| parameters.len());
                                if args.len() != parameter_count {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "call to `{callee}` has {} arguments but expects {}",
                                        args.len(),
                                        parameter_count
                                    )));
                                }
                                match allowed_effects {
                                    Some(allowed) => {
                                        for effect in target_effects.iter() {
                                            if !allowed.contains(effect) {
                                                return Err(scope_checkpoint::paths::diagnostic(
                                                    format_args!(
                                                        "call to `{callee}` requires undeclared effect `{effect}`"
                                                    ),
                                                ));
                                            }
                                        }
                                    }
                                    None if !target_effects.is_empty() => {
                                        return Err(scope_checkpoint::paths::diagnostic(
                                            format_args!(
                                                "contract calls effectful function `{callee}`"
                                            ),
                                        ));
                                    }
                                    None => {}
                                }
                                (params, return_type)
                            };
                            let params = if let Some(parameters) = borrowed_parameters {
                                held_parameters.map_or(
                                    CallParameters::Borrowed(parameters),
                                    CallParameters::Held,
                                )
                            } else {
                                intrinsic_parameters.unwrap_or(CallParameters::Owned(params))
                            };
                            let return_ownership = if matches!(
                                callee.as_str(),
                                crate::vec_ops::string_slot::ID
                                    | crate::vec_ops::string_slot::BYTES_ID
                            ) {
                                OwnershipMode::Borrow
                            } else {
                                self.expected_ownership(&return_type, OwnershipMode::Own)?
                            };
                            frames.push(Frame::CallNext {
                                expression,
                                args,
                                params,
                                return_type,
                                return_ownership,
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::VecFieldRead { args, .. }
                        | ResolvedExprKind::RowLoan { args, .. } => {
                            let (return_type, return_ownership, _bytes) =
                                self.validate_scoped_vec_node(function, expression, &scope)?;
                            frames.push(Frame::CallNext {
                                expression,
                                args,
                                params: CallParameters::VecField(args),
                                return_type: call_signature::ResultType::Owned(return_type),
                                return_ownership,
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::LiteralFormat { template, args } => {
                            self.record_static_pool(template.len(), false)?;
                            self.validate_literal_format_arguments(function, template, args)?;
                            frames.push(Frame::CallNext {
                                expression,
                                args,
                                params: CallParameters::LiteralFormat(args),
                                return_type: call_signature::ResultType::Owned(
                                    ResolvedType::String,
                                ),
                                return_ownership: OwnershipMode::Own,
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::HostCommandCall(call) => {
                            if call.expression != expression.id
                                || call.args.len() != crate::command_io_ops::arity(call.operation)
                            {
                                return Err(hir_error(
                                    "host-command call has a non-canonical identity or shape",
                                ));
                            }
                            host_command::require_effects_view(call.operation, allowed_effects)?;
                            frames.push(Frame::CallNext {
                                expression,
                                args: &call.args,
                                params: CallParameters::Owned(
                                    crate::command_io_ops::resolved_params(call.operation),
                                ),
                                return_type: call_signature::ResultType::Owned(
                                    crate::command_io_ops::return_type(call.operation),
                                ),
                                return_ownership: crate::command_io_ops::result_ownership(
                                    call.operation,
                                ),
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::NativeRustImportCall(call) => {
                            if call.expression != expression.id {
                                return Err(hir_error(
                                    "native Rust import call has a non-canonical expression identity",
                                ));
                            }
                            let retained_program = self.program;
                            let import = retained_program
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
                                    ));
                                }
                                None if !import.effects.is_empty() => {
                                    return Err(hir_error(
                                        "contract calls an effectful native Rust import",
                                    ));
                                }
                                _ => {}
                            }
                            let result = call.result.value_type(&self.program.declarations)?;
                            frames.push(Frame::NativeNext {
                                expression,
                                args: &call.args,
                                params: import.parameters.as_slice(),
                                result,
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::Binary { op, left, right } => {
                            frames.push(Frame::BinaryLeft {
                                expression,
                                op: *op,
                                right,
                                path,
                            })?;
                            frames.push(Frame::Enter {
                                expression: left,
                                scope,
                                path: path_label::Label::child(path, ".left", [0, 0])?,
                            })?;
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
                                path,
                            })?;
                            frames.push(Frame::Enter {
                                expression: condition,
                                scope,
                                path: path_label::Label::child(path, ".condition", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::Block { statements, tail }
                        | ResolvedExprKind::VecConstructor {
                            statements, tail, ..
                        } => {
                            super::vec_constructor::authenticate(expression)?;
                            self.validate_record_traversal(expression, &scope)?;
                            let outer_ids = scope.ids();
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
                            })?;
                        }
                        ResolvedExprKind::ConstructRecord { record, fields } => {
                            if crate::map_ops::is_declaration(record.as_str())
                                || record.as_str() == crate::iterator_ops::ITER_ID
                                || record.as_str() == crate::list_ops::LIST_ID
                            {
                                return Err(hir_error(
                                    "iterator owner cannot be authored as a record",
                                ));
                            }
                            let declaration = self
                                .program
                                .declarations
                                .declaration(record)
                                .ok_or_else(|| {
                                    scope_checkpoint::paths::diagnostic(format_args!(
                                        "record `{record}` is not indexed"
                                    ))
                                })?;
                            if !matches!(
                                declaration.kind,
                                DeclarationKind::Record | DeclarationKind::Class
                            ) {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "constructor target `{record}` is not a record or class"
                                )));
                            }
                            let expected = aggregate_metadata::record_fields(program, record)
                                .ok_or_else(|| {
                                    scope_checkpoint::paths::diagnostic(format_args!(
                                        "record `{record}` has no fields"
                                    ))
                                })?;
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
                                    scope_checkpoint::paths::diagnostic(format_args!(
                                        "record `{record}` has no parameters"
                                    ))
                                })?;
                            if instance != record
                                || arguments.len() != parameters.len()
                                || !super::type_reachability::record_args_ok(
                                    &self.program.declarations,
                                    record,
                                    arguments,
                                )
                            {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "constructor for `{record}` has an invalid concrete instance"
                                )));
                            }
                            frames.push(Frame::RecordNext {
                                expression,
                                fields,
                                expected,
                                record,
                                arguments,
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            })?;
                        }
                        ResolvedExprKind::ConstructVariant {
                            variant,
                            case,
                            fields,
                        } => {
                            if expression.ty.is_compiler_byte_option() {
                                return Err(hir_error(
                                    "Option<u8> may be produced only by compiler-owned byte_get",
                                ));
                            }
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
                                    || {
                                        scope_checkpoint::paths::diagnostic(format_args!(
                                            "variant `{variant}` is not indexed"
                                        ))
                                    },
                                )?;
                            if declaration.kind != DeclarationKind::Variant {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "constructor target `{variant}` is not a variant"
                                )));
                            }
                            let expected = aggregate_metadata::variant_cases(program, variant)
                                .and_then(|cases| cases.iter().find(|item| item.id == *case))
                                .ok_or_else(|| {
                                    scope_checkpoint::paths::diagnostic(format_args!(
                                        "constructor for `{variant}` contains foreign case `{case}`"
                                    ))
                                })?
                                .fields
                                .as_slice();
                            frames.push(Frame::VariantNext {
                                expression,
                                fields,
                                expected,
                                variant,
                                case,
                                arguments,
                                seen: BTreeSet::new(),
                                index: 0,
                                scope,
                                path,
                            })?;
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
                                path,
                            })?;
                            frames.push(Frame::Enter {
                                expression: base,
                                scope,
                                path: path_label::Label::child(path, ".base", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::Match {
                            scrutinee, arms, ..
                        } => {
                            frames.push(Frame::MatchScrutinee {
                                expression,
                                arms,
                                path,
                            })?;
                            frames.push(Frame::Enter {
                                expression: scrutinee,
                                scope,
                                path: path_label::Label::child(path, ".scrutinee", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::Project { base, field } => {
                            if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                                return Err(hir_error(
                                    "place field projections must use a resolved place path",
                                ));
                            }
                            frames.push(Frame::Project { expression, field })?;
                            frames.push(Frame::Enter {
                                expression: base,
                                scope,
                                path: path_label::Label::child(path, ".base", [0, 0])?,
                            })?;
                        }
                        // Class Inheritance v1: independently re-derive the
                        // upcast contract from the resolved source before the
                        // ancestor-typed result is accepted.
                        ResolvedExprKind::Upcast { source } => {
                            frames.push(Frame::Upcast { expression })?;
                            frames.push(Frame::Enter {
                                expression: source,
                                scope,
                                path: path_label::Label::child(path, ".source", [0, 0])?,
                            })?;
                        }
                        // Resumable Effects v1 (issue #204): the request
                        // resolves like any other child expression; its
                        // own type and the whole node's ownership are
                        // independently re-derived on the way back up,
                        // matching every other node here.
                        ResolvedExprKind::Yield { request } => {
                            frames.push(Frame::Yield { expression })?;
                            frames.push(Frame::Enter {
                                expression: request,
                                scope,
                                path: path_label::Label::child(path, ".request", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::Try { operand, .. } => {
                            frames.push(Frame::Try {
                                expression,
                                path,
                                option: false,
                            })?;
                            frames.push(Frame::Enter {
                                expression: operand,
                                scope,
                                path: path_label::Label::child(path, ".operand", [0, 0])?,
                            })?;
                        }
                        ResolvedExprKind::TryOption { operand, .. } => {
                            frames.push(Frame::Try {
                                expression,
                                path,
                                option: true,
                            })?;
                            frames.push(Frame::Enter {
                                expression: operand,
                                scope,
                                path: path_label::Label::child(path, ".operand", [0, 0])?,
                            })?;
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
                    scopes.push(scope)?;
                }
                Frame::ByteRangeAfterSource {
                    expression,
                    start,
                    end,
                    path,
                } => {
                    let scope = scopes.pop().expect("byte range source scope retained");
                    frames.push(Frame::ByteRangeAfterStart {
                        expression,
                        end,
                        path,
                    })?;
                    frames.push(Frame::Enter {
                        expression: start,
                        scope,
                        path: path_label::Label::child(path, ".arg.1", [0, 0])?,
                    })?;
                }
                Frame::ByteRangeAfterStart {
                    expression,
                    end,
                    path,
                } => {
                    let scope = scopes.pop().expect("byte range start scope retained");
                    frames.push(Frame::ByteRangeAfterEnd { expression })?;
                    frames.push(Frame::Enter {
                        expression: end,
                        scope,
                        path: path_label::Label::child(path, ".arg.2", [0, 0])?,
                    })?;
                }
                Frame::ByteRangeAfterEnd { expression } => {
                    let scope = scopes.pop().expect("byte range end scope retained");
                    let ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    self.require_type(&source.ty, &ResolvedType::SliceU8, "byte range source")?;
                    self.require_type(&start.ty, &ResolvedType::Usize, "byte range start")?;
                    self.require_type(&end.ty, &ResolvedType::Usize, "byte range end")?;
                    if source.ownership != OwnershipMode::Borrow
                        || start.ownership != OwnershipMode::Value
                        || end.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error("byte range operands have invalid ownership"));
                    }
                    self.finish_expr(expression, &ResolvedType::SliceU8, OwnershipMode::Borrow)?;
                    scopes.push(scope)?;
                }
                Frame::CallNext {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        self.finish_expr(expression, &return_type, return_ownership)?;
                        scopes.push(scope)?;
                    } else {
                        frames.push(Frame::CallAfterArg {
                            expression,
                            args,
                            params,
                            return_type,
                            return_ownership,
                            index,
                            path,
                        })?;
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: path_label::Label::argument(expression, path, index)?,
                        })?;
                    }
                }
                Frame::CallAfterArg {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("call argument scope retained");
                    publication.publish(&scope)?;
                    let argument = &args[index];
                    let param = params.parameter(index);
                    self.require_type(&argument.ty, param.ty, "call argument")?;
                    self.validate_argument_ownership_view(argument, param)?;
                    let param = params.parameter(index);
                    self.validate_borrowed_bytes_call_argument_fields(
                        expression,
                        argument,
                        (param.ty, param.ownership),
                        index,
                        &scope,
                    )?;
                    if *param.ty == ResolvedType::SliceU8 {
                        self.validate_byte_slice_call_argument(argument, &scope)?;
                    }
                    if self.is_owned_resource(param.ty, param.ownership)? {
                        if !allow_moves && !contract_owners::local(argument, &scope) {
                            let callee = match &expression.kind {
                                ResolvedExprKind::Call { callee, .. } => callee.as_str(),
                                ResolvedExprKind::LiteralFormat { .. } => crate::literal_format::ID,
                                ResolvedExprKind::VecFieldRead { .. } => crate::vec_field::ID,
                                _ => unreachable!(),
                            };
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push(Frame::CallNext {
                        expression,
                        args,
                        params,
                        return_type,
                        return_ownership,
                        index: index + 1,
                        scope,
                        path,
                    })?;
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
                        let ownership = if let ResolvedExprKind::NativeRustImportCall(call) =
                            &expression.kind
                        {
                            call.result.ownership()
                        } else {
                            OwnershipMode::Value
                        };
                        self.finish_expr(expression, &result, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        frames.push(Frame::NativeAfterArg {
                            expression,
                            args,
                            params,
                            result,
                            index,
                            path,
                        })?;
                        frames.push(Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: path_label::Label::child(
                                path,
                                ".native-rust-arg.{}",
                                [index, 0],
                            )?,
                        })?;
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
                    let mut scope = scopes.pop().expect("native argument scope retained");
                    publication.publish(&scope)?;
                    let argument = &args[index];
                    let parameter = &params[index];
                    self.require_type(&argument.ty, &parameter.ty, "native Rust import argument")?;
                    if argument.ownership != parameter.ownership
                        && !(parameter.ownership == OwnershipMode::Borrow
                            && argument.ownership == OwnershipMode::Own
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
                    {
                        return Err(hir_error(
                            "native Rust import argument ownership disagrees with its declaration",
                        ));
                    }
                    if parameter.ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership to native Rust",
                            ));
                        }
                        self.mark_value_sources_moved(argument, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push(Frame::NativeNext {
                        expression,
                        args,
                        params,
                        result,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                Frame::BinaryLeft {
                    expression,
                    op,
                    right,
                    path,
                } => {
                    let left_scope = scopes.pop().expect("binary left scope retained");
                    publication.publish(&left_scope)?;
                    let baseline = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        Some((left_scope.ids(), left_scope.clone()))
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
                    })?;
                    if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                    }
                    frames.push(Frame::Enter {
                        expression: right,
                        scope: left_scope,
                        path: path_label::Label::child(path, ".right", [0, 0])?,
                    })?;
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
                        scope_checkpoint::merge::conditional(&mut parent, &scope, &ids)?;
                        scope = parent;
                    }
                    if direct || matches!(op, BinaryOp::And | BinaryOp::Or) {
                        publication.publish(&scope)?;
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
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                            ) || (matches!(op, BinaryOp::Rem)
                                && !matches!(
                                    left.ty,
                                    ResolvedType::I64
                                        | ResolvedType::I32
                                        | ResolvedType::U8
                                        | ResolvedType::Usize
                                ))
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
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                                    | ResolvedType::String
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
                            if left.ty.is_callable() || right.ty.is_callable() {
                                return Err(hir_error(
                                    "function value equality is outside the admitted profile",
                                ));
                            }
                            self.require_type(&left.ty, &right.ty, "equality operands")?;
                            ResolvedType::Bool
                        }
                    };
                    self.finish_expr(expression, &output, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                Frame::LexicalReturn {
                    expression,
                    operand,
                } => {
                    let mut scope = scopes.pop().expect("return operand scope retained");
                    self.validate_lexical_return_operand(operand, &mut scope)?;
                    self.finish_expr(expression, &ResolvedType::Never, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                Frame::IfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    path,
                } => {
                    let outer = scopes.pop().expect("if condition scope retained");
                    publication.publish(&outer)?;
                    let ResolvedExprKind::If { condition, .. } = &expression.kind else {
                        unreachable!()
                    };
                    self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                    let outer_ids = outer.ids();
                    frames.push(Frame::IfThen {
                        expression,
                        else_branch,
                        path,
                        outer: outer.clone(),
                        outer_ids,
                    })?;
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled))?;
                    frames.push(Frame::Enter {
                        expression: then_branch,
                        scope: outer,
                        path: path_label::Label::child(path, ".then", [0, 0])?,
                    })?;
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
                    })?;
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled))?;
                    frames.push(Frame::Enter {
                        expression: else_branch,
                        scope: outer,
                        path: path_label::Label::child(path, ".else", [0, 0])?,
                    })?;
                }
                Frame::IfElse {
                    expression,
                    then_scope,
                    mut outer,
                    outer_ids,
                } => {
                    let else_scope = scopes.pop().expect("if else scope retained");
                    let ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    self.finish_lexical_if_checkpoint(
                        expression,
                        then_branch,
                        else_branch,
                        &mut outer,
                        &then_scope,
                        &else_scope,
                        &outer_ids,
                    )?;
                    publication.publish(&outer)?;
                    scopes.push(outer)?;
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
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                        frames.push(Frame::Enter {
                            expression: tail,
                            scope,
                            path: path_label::Label::child(path, ".tail", [0, 0])?,
                        })?;
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
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled))?;
                                frames.push(Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::CopyDiscard { value, .. } => {
                                frames.push(Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled))?;
                                frames.push(Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::Assign { binding, value, .. } => {
                                if crate::byte_ops::is_same_owner_set_hir(value, &binding.id) {
                                    self.buffer_reopen_sites.insert(value.id.clone());
                                }
                                // The target must be a previously declared
                                // mutable scalar binding in this block's scope.
                                if !scope.contains_key(&binding.id) {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled))?;
                                frames.push(Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
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
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled))?;
                                frames.push(Frame::Enter {
                                    expression: body,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.body", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::While {
                                condition, body, ..
                            }
                            | ResolvedStatement::LexicalWhile {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: re-check admission
                                // at the trust boundary and require an exact
                                // `bool` condition before validating the body
                                // like any nested block.
                                self.validate_loop_pair(condition, body)?;
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
                                    path,
                                    body,
                                    entry: scope.clone(),
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push(Frame::RestorePublication(enabled))?;
                                frames.push(Frame::Enter {
                                    expression: condition,
                                    scope,
                                    path: path_label::Label::child(
                                        path,
                                        ".s{}.condition",
                                        [index, 0],
                                    )?,
                                })?;
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
                    match &statements[index] {
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            super::copy_discard::require(value)?
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            if body.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&body.ty)
                            {
                                return Err(hir_error(
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                ));
                            }
                        }
                        _ => unreachable!("no-binding statement continuation"),
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
                    })?;
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
                    entry,
                } => {
                    // The condition validated like any expression; it must be
                    // exactly `bool` because it re-evaluates before every
                    // iteration.
                    let scope = scopes.pop().expect("while condition scope retained");
                    let (ResolvedStatement::While { condition, .. }
                    | ResolvedStatement::LexicalWhile { condition, .. }) = &statements[index]
                    else {
                        unreachable!("while condition frame resumes at a while statement")
                    };
                    if scope != entry {
                        return Err(hir_error("while condition changes ownership liveness"));
                    }
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
                        path,
                        entry: scope.clone(),
                    })?;
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::RestorePublication(enabled))?;
                    frames.push(Frame::Enter {
                        expression: body,
                        scope,
                        path: path_label::Label::child(path, ".s{}.body", [index, 0])?,
                    })?;
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
                    // Zero or more iterations run: every route reaching the
                    // continuation must preserve the loop-entry ownership state.
                    let after = scopes.pop().expect("while body scope retained");
                    let body = statements[index].child(1).expect("loop body");
                    if !super::lexical_exit::returns_only(body) && after != entry {
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
                    })?;
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
                    let ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = path_label::Label::child(path, ".s{}", [index, 0])?;
                    if !binding.id.matches_local_display(
                        function,
                        statement_path.len(),
                        statement_path,
                    ) {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "local `{}` has a non-canonical identity",
                            binding.id
                        )));
                    }
                    self.insert_value(&binding.id)?;
                    self.require_type(&binding.ty, &value.ty, "local binding")?;
                    if binding.ownership != value.ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "local `{}` has inconsistent ownership",
                            binding.id
                        )));
                    }
                    self.validate_declared_ownership(&binding.ty, binding.ownership)?;
                    if binding.ty == ResolvedType::SliceU8 {
                        let (place, origin) = match &value.kind {
                            ResolvedExprKind::Place(place) => {
                                let origin = self.byte_slice_aliases.get(&place.root).cloned().ok_or_else(|| {
                                    hir_error("byte-slice local alias lacks symbolic parameter-root provenance")
                                })?;
                                (place, origin)
                            }
                            ResolvedExprKind::BorrowPlace { place, .. } => (place, place.clone()),
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                let place = crate::vec_ops::string_slot::place(value)
                                    .expect("authenticated slot");
                                (place, place.clone())
                            }
                            ResolvedExprKind::VecFieldRead { args, .. } => {
                                let ResolvedExprKind::Place(place) = &args[0].kind else {
                                    return Err(hir_error("vector view has no carrier place"));
                                };
                                (place, place.clone())
                            }
                            ResolvedExprKind::ByteRange { source, .. } => {
                                self.byte_range_origin(source, &scope)?
                            }
                            _ => {
                                return Err(hir_error(
                                    "byte-slice local must be a direct immutable alias or authenticated view",
                                ));
                            }
                        };
                        let is_authenticated_view = crate::vec_ops::string_slot::inspect(value)
                            .is_some()
                            || matches!(
                                &value.kind,
                                ResolvedExprKind::BorrowPlace { .. }
                                    | ResolvedExprKind::VecFieldRead { .. }
                            );
                        if *mutable || (!is_authenticated_view && !place.projections.is_empty()) {
                            return Err(hir_error(
                                "byte-slice local alias must be immutable and unprojected",
                            ));
                        }
                        self.byte_slice_aliases
                            .insert(binding.id.clone(), origin.clone());
                        let borrowed_origin = match &value.kind {
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                Some(origin)
                            }
                            ResolvedExprKind::BorrowPlace { .. }
                            | ResolvedExprKind::VecFieldRead { .. }
                            | ResolvedExprKind::ByteRange { .. } => Some(origin),
                            _ => None,
                        };
                        if let Some(origin) = borrowed_origin {
                            let owner = scope.get_mut(&origin.root)?.ok_or_else(|| {
                                hir_error("byte view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend(self.exact_loan_id(
                                function,
                                &value.id,
                                &LoanCause::SliceView,
                            )?);
                        }
                    } else if binding.ty == ResolvedType::Str {
                        let (origin, creates_loan) =
                            self.borrowed_str_let_origin(value, binding)?;
                        if *mutable {
                            return Err(hir_error(
                                "borrowed-str local alias must be immutable and unprojected",
                            ));
                        }
                        self.borrowed_str_aliases
                            .insert(binding.id.clone(), origin.clone());
                        if creates_loan {
                            let owner = scope.get_mut(&origin.root)?.ok_or_else(|| {
                                hir_error("String view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend(self.exact_loan_id(
                                function,
                                &value.id,
                                &LoanCause::StrView,
                            )?);
                        }
                    }
                    let contract_local = !allow_moves && contract_owners::fresh(value, &scope);
                    if self.is_owned_resource(&binding.ty, binding.ownership)? {
                        if !allow_moves && !contract_owners::fresh(value, &scope) {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a local binding",
                            ));
                        }
                        self.mark_value_sources_moved(value, &mut scope)?;
                    }
                    scope_checkpoint::charge::clone_type(&binding.ty)?;
                    scope.insert(
                        binding.id.clone(),
                        ValidationBinding {
                            contract_local,
                            mutable: *mutable,
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            availability: Availability::Available,
                            active_loans: BTreeSet::new(),
                            moved_places: BTreeMap::new(),
                            definitely_partial: BTreeSet::new(),
                        },
                    )?;
                    frames.push(Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    })?;
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
                    let assigned_scope = scopes.pop().expect("block assign scope retained");
                    let mut scope = scope;
                    let ResolvedStatement::Assign {
                        binding,
                        field,
                        value: assigned,
                        span: assignment_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    if crate::stdin_stream_ops::hir_reopen(assigned, &binding.id) {
                        scope = assigned_scope;
                        stdin_stream::reopen_reader(&mut scope, &binding.id)?;
                    } else if crate::hir::iterator_loop::is_step_reassignment(assigned, &binding.id)
                    {
                        scope = assigned_scope;
                        iterator_loops::reopen_step(&mut scope, &binding.id)?;
                    } else if crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id) {
                        scope = assigned_scope;
                        iterator_loops::reopen_string(&mut scope, &binding.id)?;
                    } else if crate::string_ops::replacement::admitted(binding, assigned) {
                        if scope
                            .get(&binding.id)
                            .is_none_or(|target| target.availability != Availability::Available)
                        {
                            return Err(hir_error(
                                "String replacement starts with an unavailable owner",
                            ));
                        }
                        scope = assigned_scope;
                        self.mark_value_sources_moved(assigned, &mut scope)?;
                        string_replacement::reopen(&mut scope, &binding.id)?;
                    }
                    let target = scope.get(&binding.id).cloned();
                    let Some(target) = target else {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "assignment target `{}` has no enclosing binding",
                            binding.id
                        )));
                    };
                    let assigned_place = Place {
                        root: binding.id.clone(),
                        projections: field.iter().cloned().map(PlaceProjection::Field).collect(),
                    };
                    if self.binding_has_live_overlapping_loans(
                        &target,
                        &assigned_place,
                        &assigned.id,
                        LoanPointPhase::After,
                    )? {
                        return Err(hir_error(
                            "SPX-T265: assignment would replace lexically borrowed byte storage",
                        ));
                    }
                    match field {
                        Some(field) => {
                            if target.ownership != OwnershipMode::Value {
                                return Err(hir_error(
                                    "field assignment base is not a value-owned aggregate",
                                ));
                            }
                            self.validate_assign_field(
                                &target.ty,
                                field,
                                assigned,
                                *assignment_span,
                            )?;
                        }
                        None => {
                            self.require_type(&target.ty, &assigned.ty, "assignment")?;
                            if (target.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&target.ty))
                                && !crate::string_ops::replacement::admitted(binding, assigned)
                                && !owner_renewal::assignment_is_admitted(
                                    self.program,
                                    self.byte_helpers,
                                    binding,
                                    assigned,
                                )
                            {
                                return Err(hir_error(
                                    "explicit mutation v1 supports only scalar Copy values",
                                ));
                            }
                        }
                    }
                    if target.availability != Availability::Available {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                    })?;
                }
                Frame::BlockTail {
                    expression,
                    outer_ids,
                    mut outer,
                } => {
                    let block_scope = scopes.pop().expect("block tail scope retained");
                    scope_checkpoint::merge::availability(&mut outer, &block_scope, &outer_ids)?;
                    publication.publish(&outer)?;
                    let (ResolvedExprKind::Block { tail, .. }
                    | ResolvedExprKind::VecConstructor { tail, .. }) = &expression.kind
                    else {
                        unreachable!()
                    };
                    self.finish_expr(expression, &tail.ty, tail.ownership)?;
                    scopes.push(outer)?;
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
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{record}` is missing required fields"
                            )));
                        }
                        let ownership =
                            self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        self.finish_expr(expression, &expression.ty, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                            path,
                        })?;
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
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
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let ownership = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                        publication.publish(&scope)?;
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
                    })?;
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
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{case}` is missing required payload fields"
                            )));
                        }
                        let ownership =
                            self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        self.finish_expr(expression, &expression.ty, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                            path,
                        })?;
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
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
                    let mut scope = scopes.pop().expect("variant field scope retained");
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("variant field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, variant, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "variant payload field")?;
                    let ownership = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "variant payload field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a variant",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope)?;
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
                    })?;
                }
                Frame::UpdateBase {
                    expression,
                    record,
                    fields,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("update base scope retained");
                    publication.publish(&scope)?;
                    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let declaration =
                        self.program
                            .declarations
                            .declaration(record)
                            .ok_or_else(|| {
                                scope_checkpoint::paths::diagnostic(format_args!(
                                    "record `{record}` is not indexed"
                                ))
                            })?;
                    if declaration.kind != DeclarationKind::Record {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` has no parameters"
                            ))
                        })?;
                    if instance != record
                        || arguments.len() != parameters.len()
                        || !super::type_reachability::record_args_ok(
                            &self.program.declarations,
                            record,
                            arguments,
                        )
                    {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "record update for `{record}` has an invalid concrete instance"
                        )));
                    }
                    validate_nested_update_base_shape(self.program, base, expression.span)?;
                    let ownership = self.expected_ownership(&base.ty, OwnershipMode::Own)?;
                    if base.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                        publication.publish(&scope)?;
                    }
                    let expected =
                        aggregate_metadata::record_fields(program, record).ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` has no fields"
                            ))
                        })?;
                    frames.push(Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                        ownership,
                    })?;
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
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                            path,
                            ownership,
                        })?;
                        frames.push(Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
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
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("update field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    self.require_type(&initializer.value.ty, &field_ty, "record replacement")?;
                    let expected_ownership =
                        self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected_ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
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
                        publication.publish(&scope)?;
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
                    })?;
                }
                Frame::MatchScrutinee {
                    expression,
                    arms,
                    path,
                } => {
                    let mut outer = scopes.pop().expect("match scrutinee scope retained");
                    publication.publish(&outer)?;
                    let ResolvedExprKind::Match {
                        mode, scrutinee, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    // Refutable Match v1: Copy-scalar scrutinees validate
                    // through the decision-chain twins below.
                    if matches!(
                        scrutinee.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::Char
                            | ResolvedType::Bool
                    ) {
                        if *mode != ResolvedMatchMode::Value
                            || scrutinee.ownership != OwnershipMode::Value
                        {
                            return Err(hir_error(
                                "resolved refutable match scrutinee is not Copy",
                            ));
                        }
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        let outer_ids = outer.ids();
                        frames.push(Frame::ScalarMatchNext {
                            expression,
                            arms,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: scope_checkpoint::Storage::with_capacity(arms.len())?,
                            result: None,
                            path,
                        })?;
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration: matched,
                        arguments: _,
                    } = &scrutinee.ty
                    else {
                        return Err(hir_error("resolved match scrutinee is not nominal"));
                    };
                    let kind = self
                        .program
                        .declarations
                        .declaration(matched)
                        .map(|item| item.kind);
                    let outer_ids = outer.ids();
                    if kind == Some(DeclarationKind::Record) {
                        let facts = self
                            .program
                            .declarations
                            .type_flags(&scrutinee.ty)
                            .ok_or_else(|| hir_error("record match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own
                                    && (!super::type_reachability::is_nested_nonflat_owned_byte_record(
                                        &self.program.declarations,
                                        &scrutinee.ty,
                                    ) || matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    )) =>
                            {
                                self.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    ) => {}
                            _ => {
                                return Err(hir_error(
                                    "resolved record match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let [arm] = arms else {
                            return Err(hir_error(
                                "resolved irrefutable record match must have exactly one arm",
                            ));
                        };
                        let mut arm_scope = outer.clone();
                        let borrowed_root = if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!("borrow mode authenticated a place above")
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root)? {
                                owner.active_loans.extend(self.exact_loan_id(
                                    function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow { arm: 0 },
                                )?);
                            }
                            Some(place.root.clone())
                        } else {
                            None
                        };
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard if *mode != ResolvedMatchMode::Value => {
                                return Err(hir_error(
                                    "resolved explicit ownership match lacks an exact record pattern",
                                ));
                            }
                            ResolvedMatchPattern::Wildcard => {}
                            ResolvedMatchPattern::Record {
                                record,
                                instance,
                                fields,
                            } => self
                                .validate_record_match_pattern(
                                    function,
                                    &scrutinee.ty,
                                    record,
                                    instance,
                                    fields,
                                    &mut arm_scope,
                                    path_label::Label::child(path, ".arm.0.record", [0, 0])?,
                                    *mode,
                                )
                                .map_err(|diagnostic| {
                                    hir_diagnostic_at_span(diagnostic, expression.span)
                                })?,
                            ResolvedMatchPattern::Variant { .. } => {
                                return Err(hir_error(
                                    "resolved variant pattern has a record scrutinee",
                                ));
                            }
                            ResolvedMatchPattern::Literal(_)
                            | ResolvedMatchPattern::Or(_)
                            | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate record \
                                     scrutinee",
                                ));
                            }
                        }
                        frames.push(Frame::RecordMatchArm {
                            expression,
                            arm,
                            outer,
                            outer_ids,
                            borrowed_root,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.0.value", [0, 0])?,
                        })?;
                    } else {
                        if kind != Some(DeclarationKind::Variant) {
                            return Err(hir_error(
                                "resolved match scrutinee is not a concrete variant",
                            ));
                        }
                        let facts = self
                            .program
                            .declarations
                            .type_flags(&scrutinee.ty)
                            .ok_or_else(|| hir_error("variant match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if (resolved_type_is_flat_owned_byte_variant(
                                    self.program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    self.program,
                                    &scrutinee.ty,
                                )) && facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own =>
                            {
                                self.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if (resolved_type_is_flat_owned_byte_variant(
                                    self.program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    self.program,
                                    &scrutinee.ty,
                                ) || regex_result(self.program, &scrutinee.ty))
                                    && facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(&scrutinee.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()) =>
                                {}
                            _ => {
                                return Err(hir_error(
                                    "resolved variant match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let cases = aggregate_metadata::variant_cases(program, matched)
                            .ok_or_else(|| {
                                scope_checkpoint::paths::diagnostic(format_args!(
                                    "variant `{matched}` has no cases"
                                ))
                            })?;
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        frames.push(Frame::VariantMatchNext {
                            expression,
                            cases,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: scope_checkpoint::Storage::with_capacity(arms.len())?,
                            covered: variant_or::Covered::new(),
                            wildcard_seen: false,
                            result: None,
                            path,
                        })?;
                    }
                }
                Frame::RecordMatchArm {
                    expression,
                    arm,
                    mut outer,
                    outer_ids,
                    borrowed_root,
                } => {
                    let mut arm_scope = scopes.pop().expect("record match arm scope retained");
                    if let Some(root) = borrowed_root {
                        if let Some(owner) = arm_scope.get_mut(&root)? {
                            if let Some(loan) = self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow { arm: 0 },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    self.validate_generic_record_match_result(function, expression, arm)?;
                    for id in scope_checkpoint::Ids::ids(&outer_ids) {
                        outer.copy_binding_from(&arm_scope, id)?;
                    }
                    publication.publish(&outer)?;
                    self.finish_expr(expression, &arm.value.ty, arm.value.ownership)?;
                    scopes.push(outer)?;
                }
                Frame::VariantMatchNext {
                    expression,
                    cases,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    mut covered,
                    mut wildcard_seen,
                    result,
                    path,
                } => {
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("variant", lexical_exit::VARIANT_VIEWS);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::VARIANT_VIEWS))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("variant");
                    let ResolvedExprKind::Match {
                        scrutinee,
                        mode,
                        arms,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    let ResolvedType::Nominal {
                        declaration: variant,
                        arguments,
                    } = &scrutinee.ty
                    else {
                        unreachable!("variant continuation follows validated nominal scrutinee")
                    };
                    let arms: &[ResolvedMatchArm] = arms;
                    let arguments: &[ResolvedType] = arguments;
                    if index == arms.len() {
                        if !wildcard_seen
                            && covered.len() != cases.len()
                            && !self.observed_partial_byte_receiver(expression)
                        {
                            return Err(hir_error("resolved match is not exhaustive"));
                        }
                        #[cfg(test)]
                        lexical_exit::observe_witness_frontier(
                            "finish",
                            lexical_exit::WITNESS_FINISH,
                        );
                        scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_FINISH))?;
                        #[cfg(test)]
                        lexical_exit::observe_witness_role("finish");
                        let witness =
                            result.ok_or_else(|| hir_error("resolved match has no result"))?;
                        let mut final_scope = outer;
                        if let Some((first, rest)) = arm_scopes.split_first() {
                            let mut joined = first.clone();
                            for arm_scope in rest {
                                scope_checkpoint::merge::conditional(
                                    &mut joined,
                                    arm_scope,
                                    &outer_ids,
                                )?;
                            }
                            scope_checkpoint::merge::availability(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                            scope_checkpoint::merge::borrows(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                        }
                        publication.publish(&final_scope)?;
                        self.finish_expr(expression, &witness.ty, witness.ownership)?;
                        scopes.push(final_scope)?;
                    } else {
                        let arm = &arms[index];
                        let mut arm_scope = outer.clone();
                        if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!()
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root)? {
                                owner.active_loans.extend(self.exact_loan_id(
                                    function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow {
                                        arm: u16::try_from(index).map_err(|_| {
                                            hir_error("match loan arm index overflows")
                                        })?,
                                    },
                                )?);
                            }
                        }
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {
                                if *mode != ResolvedMatchMode::Value {
                                    return Err(hir_error(
                                        "resolved explicit ownership match cannot discard an owned variant",
                                    ));
                                }
                                if wildcard_seen || covered.len() == cases.len() {
                                    return Err(hir_error(
                                        "resolved match has an unreachable wildcard",
                                    ));
                                }
                                wildcard_seen = arm.guard.is_none();
                            }
                            ResolvedMatchPattern::Variant {
                                variant: pattern_variant,
                                case,
                                fields,
                            } => {
                                if wildcard_seen
                                    || pattern_variant != variant
                                    || covered.contains(case)
                                    || (arm.guard.is_none() && !covered.insert(case)?)
                                {
                                    return Err(hir_error(
                                        "resolved match has an unreachable or foreign case pattern",
                                    ));
                                }
                                let declared_case = cases
                                    .iter()
                                    .find(|item| item.id == *case)
                                    .ok_or_else(|| {
                                        scope_checkpoint::paths::diagnostic(format_args!(
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
                                            scope_checkpoint::paths::diagnostic(format_args!(
                                                "resolved pattern contains foreign field `{}`",
                                                field.field
                                            ))
                                        })?;
                                    let binding_ty =
                                        substitute_type(&declared.ty, variant, arguments)?;
                                    let facts = self
                                        .program
                                        .declarations
                                        .type_flags(&binding_ty)
                                        .ok_or_else(|| {
                                        hir_error("variant binding has no exact type facts")
                                    })?;
                                    let binding_ownership = if facts.copy {
                                        OwnershipMode::Value
                                    } else if *mode == ResolvedMatchMode::Own {
                                        OwnershipMode::Own
                                    } else if *mode == ResolvedMatchMode::Borrow {
                                        OwnershipMode::Borrow
                                    } else {
                                        return Err(hir_error(
                                            "Copy variant pattern contains an owned payload",
                                        ));
                                    };
                                    if !seen.insert(field.field.clone())
                                        || !field.binding.id.matches_local_label(
                                            function,
                                            path_label::Label::child(
                                                path,
                                                ".arm.{}.binding.{}",
                                                [index, field_index],
                                            )?,
                                        )
                                        || field.binding.ty != binding_ty
                                        || field.binding.ownership != binding_ownership
                                    {
                                        return Err(hir_error(
                                            "resolved match pattern field or binding is invalid",
                                        ));
                                    }
                                    self.insert_value(&field.binding.id)?;
                                    self.validate_type(&field.binding.ty)?;
                                    if arm_scope.contains_key(&field.binding.id) {
                                        return Err(hir_error(
                                            "resolved match pattern binding shadows an existing value",
                                        ));
                                    }
                                    scope_checkpoint::charge::clone_type(&field.binding.ty)?;
                                    arm_scope.insert(
                                        field.binding.id.clone(),
                                        ValidationBinding {
                                            contract_local: false,
                                            mutable: false,
                                            ty: field.binding.ty.clone(),
                                            ownership: binding_ownership,
                                            availability: Availability::Available,
                                            active_loans: BTreeSet::new(),
                                            moved_places: BTreeMap::new(),
                                            definitely_partial: BTreeSet::new(),
                                        },
                                    )?;
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
                                ));
                            }
                            ResolvedMatchPattern::Or(alternatives) => {
                                let mut candidate = covered.checkpoint()?;
                                variant_or::cover_borrowed(
                                    alternatives,
                                    variant,
                                    cases,
                                    *mode,
                                    wildcard_seen,
                                    &mut candidate,
                                )?;
                                if arm.guard.is_none() {
                                    covered = candidate;
                                }
                            }
                            ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate variant \
                                     scrutinee",
                                ));
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            if !crate::variant_guards::admitted(
                                &self.program.declarations,
                                &scrutinee.ty,
                                *mode,
                                &arm.pattern,
                                guard,
                            ) {
                                return Err(hir_error(
                                    "resolved Copy variant guard is outside its ordinary Boolean profile",
                                ));
                            }
                            self.validate_expr_iterative_effects(
                                function,
                                guard,
                                &mut arm_scope,
                                path_label::Label::child(path, ".arm.{}.guard", [index, 0])?,
                                allow_moves,
                                allowed_effects,
                            )?;
                            if guard.ty != ResolvedType::Bool {
                                return Err(hir_error("resolved match guard is not bool"));
                            }
                            variant_guard_state::unchanged(&outer, &arm_scope)?;
                        }
                        frames.push(Frame::VariantMatchAfterArm {
                            expression,
                            cases,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            covered,
                            wildcard_seen,
                            result,
                            path,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.{}.value", [index, 0])?,
                        })?;
                    }
                }
                Frame::VariantMatchAfterArm {
                    expression,
                    cases,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    covered,
                    wildcard_seen,
                    mut result,
                    path,
                } => {
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("arm", lexical_exit::VARIANT_ARM_VIEW);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::VARIANT_ARM_VIEW))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("arm");
                    let ResolvedExprKind::Match { arms, .. } = &expression.kind else {
                        unreachable!("variant arm continuation retains its match")
                    };
                    let arms: &[ResolvedMatchArm] = arms;
                    let mut arm_scope = scopes.pop().expect("variant match arm scope retained");
                    if let ResolvedExprKind::Match {
                        scrutinee,
                        mode: ResolvedMatchMode::Borrow,
                        ..
                    } = &expression.kind
                    {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!()
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root)? {
                            if let Some(loan) = self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow {
                                    arm: u16::try_from(index)
                                        .map_err(|_| hir_error("match loan arm index overflows"))?,
                                },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    let arm = &arms[index];
                    if arm.value.ty != ResolvedType::Never
                        && !super::collection_outcome::match_arm_execution(
                            self.program,
                            function,
                            expression,
                            &arm.value,
                        )
                    {
                        return Err(hir_error(
                            "resolved owned variant match arm must produce a Copy scalar",
                        ));
                    }
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("join", lexical_exit::WITNESS_JOIN);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_JOIN))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("join");
                    self.join_lexical_match_witness(&mut result, &arm.value)?;
                    #[cfg(test)]
                    lexical_exit::witness_unwind_point();
                    if !super::lexical_exit::returns_only(&arm.value) {
                        arm_scopes.push(arm_scope)?;
                    }
                    frames.push(Frame::VariantMatchNext {
                        expression,
                        cases,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        covered,
                        wildcard_seen,
                        result,
                        path,
                    })?;
                }
                Frame::ScalarMatchNext {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    path,
                } => {
                    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
                        unreachable!()
                    };
                    if index == arms.len() {
                        // Admission already required the trailing catch-all;
                        // re-check it here against hostile HIR.
                        let last = arms.last().expect("non-empty checked above");
                        let catch_all = matches!(
                            &last.pattern,
                            ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Binding(_)
                        );
                        if !catch_all || last.guard.is_some() {
                            return Err(hir_error(
                                "resolved refutable match lacks a trailing irrefutable \
                                 guard-free catch-all",
                            ));
                        }
                        #[cfg(test)]
                        lexical_exit::observe_witness_frontier(
                            "finish",
                            lexical_exit::WITNESS_FINISH,
                        );
                        scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_FINISH))?;
                        #[cfg(test)]
                        lexical_exit::observe_witness_role("finish");
                        let witness =
                            result.ok_or_else(|| hir_error("resolved match has no result"))?;
                        let mut final_scope = outer;
                        if let Some((first, rest)) = arm_scopes.split_first() {
                            let mut joined = first.clone();
                            for arm_scope in rest {
                                scope_checkpoint::merge::conditional(
                                    &mut joined,
                                    arm_scope,
                                    &outer_ids,
                                )?;
                            }
                            scope_checkpoint::merge::availability(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                            scope_checkpoint::merge::borrows(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                        }
                        publication.publish(&final_scope)?;
                        self.finish_expr(expression, &witness.ty, witness.ownership)?;
                        scopes.push(final_scope)?;
                        continue;
                    }
                    let arm = &arms[index];
                    let mut arm_scope = outer.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Binding(binding) => {
                            if !binding.id.matches_local_label(
                                function,
                                path_label::Label::child(path, ".arm.{}.binding", [index, 0])?,
                            ) || binding.ty != scrutinee.ty
                                || binding.ownership != OwnershipMode::Value
                            {
                                return Err(hir_error("resolved scalar match binding is invalid"));
                            }
                            self.insert_value(&binding.id)?;
                            self.validate_type(&binding.ty)?;
                            if arm_scope.contains_key(&binding.id) {
                                return Err(hir_error(
                                    "resolved match pattern binding shadows an existing value",
                                ));
                            }
                            scope_checkpoint::charge::clone_type(&binding.ty)?;
                            arm_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    contract_local: false,
                                    mutable: false,
                                    ty: binding.ty.clone(),
                                    ownership: OwnershipMode::Value,
                                    availability: Availability::Available,
                                    active_loans: BTreeSet::new(),
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            )?;
                        }
                        ResolvedMatchPattern::Literal(value) => {
                            if value.ty() != scrutinee.ty {
                                return Err(hir_error(
                                    "resolved literal pattern disagrees with its scrutinee type",
                                ));
                            }
                        }
                        ResolvedMatchPattern::Or(alternatives) => {
                            if alternatives.is_empty() {
                                return Err(hir_error("resolved or-pattern has no alternatives"));
                            }
                            for alternative in alternatives {
                                let ResolvedMatchPattern::Literal(value) = alternative else {
                                    return Err(hir_error(
                                        "resolved or-pattern contains a non-literal alternative",
                                    ));
                                };
                                if value.ty() != scrutinee.ty {
                                    return Err(hir_error(
                                        "resolved or-pattern alternative disagrees with its \
                                         scrutinee type",
                                    ));
                                }
                            }
                        }
                        ResolvedMatchPattern::Variant { .. }
                        | ResolvedMatchPattern::Record { .. } => {
                            return Err(hir_error(
                                "resolved aggregate pattern has a Copy-scalar scrutinee",
                            ));
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        frames.push(Frame::ScalarMatchAfterGuard {
                            expression,
                            arms,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            result,
                            path,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                        frames.push(Frame::Enter {
                            expression: guard.as_ref(),
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.{}.guard", [index, 0])?,
                        })?;
                    } else {
                        frames.push(Frame::ScalarMatchAfterArm {
                            expression,
                            arms,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            result,
                            path,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push(Frame::RestorePublication(enabled))?;
                        frames.push(Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.{}.value", [index, 0])?,
                        })?;
                    }
                }
                Frame::ScalarMatchAfterGuard {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    path,
                } => {
                    let arm = &arms[index];
                    let Some(guard) = &arm.guard else {
                        unreachable!("guard continuation retains a guarded arm")
                    };
                    let scope_after_guard =
                        scopes.pop().expect("scalar match guard scope retained");
                    if guard.ty != ResolvedType::Bool || guard.ownership != OwnershipMode::Value {
                        return Err(hir_error("resolved match guard is not exactly bool"));
                    }
                    // The arm value observes the guard's moves within this
                    // arm only, so it resolves under the post-guard scope.
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push(Frame::ScalarMatchAfterArm {
                        expression,
                        arms,
                        index,
                        outer,
                        outer_ids,
                        arm_scopes,
                        result,
                        path,
                    })?;
                    frames.push(Frame::RestorePublication(enabled))?;
                    frames.push(Frame::Enter {
                        expression: &arm.value,
                        scope: scope_after_guard,
                        path: path_label::Label::child(path, ".arm.{}.value", [index, 0])?,
                    })?;
                }
                Frame::ScalarMatchAfterArm {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    mut result,
                    path,
                } => {
                    let arm_scope = scopes.pop().expect("scalar match arm scope retained");
                    let arm = &arms[index];
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("join", lexical_exit::WITNESS_JOIN);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_JOIN))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("join");
                    self.join_lexical_match_witness(&mut result, &arm.value)?;
                    #[cfg(test)]
                    lexical_exit::witness_unwind_point();
                    if !super::lexical_exit::returns_only(&arm.value) {
                        arm_scopes.push(arm_scope)?;
                    }
                    frames.push(Frame::ScalarMatchNext {
                        expression,
                        arms,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        result,
                        path,
                    })?;
                }
                Frame::Project { expression, field } => {
                    let scope = scopes.pop().expect("projection scope retained");
                    let ResolvedExprKind::Project { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let projected = self.field_type_for_type(&base.ty, field)?;
                    let ownership = self.expected_ownership(&projected, base.ownership)?;
                    self.finish_expr(expression, &projected, ownership)?;
                    scopes.push(scope)?;
                }
                Frame::Upcast { expression } => {
                    let scope = scopes.pop().expect("upcast scope retained");
                    let ResolvedExprKind::Upcast { source } = &expression.kind else {
                        unreachable!()
                    };
                    self.validate_upcast(expression, source)?;
                    let ownership = self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                    self.finish_expr(expression, &expression.ty, ownership)?;
                    scopes.push(scope)?;
                }
                Frame::Yield { expression } => {
                    let scope = scopes.pop().expect("yield scope retained");
                    if !matches!(expression.kind, ResolvedExprKind::Yield { .. }) {
                        unreachable!()
                    }
                    // Re-derive `hir::resolve_yield`'s own admission: scalar,
                    // or (issue #296 R20) a bounded aggregate.
                    let (d, ty) = (&self.program.declarations, &expression.ty);
                    let bounded = matches!(ty, ResolvedType::Nominal { .. })
                        && super::yield_aggregate::bounded_aggregate_refusal(d, ty).is_ok();
                    if !crate::hir::is_scalar_resolved_type(ty) && !bounded {
                        return Err(hir_error("resolved `yield` type is not scalar or bounded"));
                    }
                    let ownership =
                        self.expected_ownership(&expression.ty, OwnershipMode::Value)?;
                    self.finish_expr(expression, &expression.ty, ownership)?;
                    scopes.push(scope)?;
                }
                Frame::Try {
                    expression,
                    path,
                    option,
                } => {
                    let mut scope = scopes.pop().expect("try scope retained");
                    if !option {
                        let ResolvedExprKind::Try { operand, .. } = &expression.kind else {
                            unreachable!()
                        };
                        if owned_result_try::expression_owns_exact_operand(operand) {
                            self.mark_value_sources_moved(operand, &mut scope)?;
                        }
                    }
                    self.finish_try_expr(function, expression, &scope, path, option)?;
                    scopes.push(scope)?;
                }
            }
        }
        if scopes.len() != 1 {
            return Err(hir_error("iterative HIR validator lost its scope stack"));
        }
        publication.publish(&scopes.pop().expect("root validation scope retained"))?;
        Ok(())
    }

    fn finish_try_expr(
        &self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &dyn ScopeRead,
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
                    .type_flags(&binding.ty)
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
            ) = (&operand.ty, &**residual_type)
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
                .execution_return_type(function)
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
                binding.availability != Availability::Moved
                    && self
                        .program
                        .declarations
                        .type_flags(&binding.ty)
                        .is_some_and(|facts| facts.needs_drop)
            }) {
                return Err(hir_error(
                    "resolved `?` has an unrelated live owned binding",
                ));
            }
            let profile = owned_result_try::validate_shape(
                expression,
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            )?;
            let enclosing = self
                .execution_return_type(function)
                .ok_or_else(|| hir_error("resolved `?` has no enclosing function"))?;
            self.require_type(residual_type, enclosing, "`?` residual")?;
            self.require_type(
                &expression.ty,
                &profile.operand_arguments[0],
                "`?` success value",
            )?;
            self.require_type(
                &profile.operand_arguments[1],
                &profile.residual_arguments[1],
                "`?` residual error",
            )?;
            Ok(())
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn validate_expr_recursive_reference_body(
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
            ResolvedExprKind::LexicalExit { value, .. } => {
                if let Some(operand) = value {
                    self.validate_expr_recursive_reference(
                        function,
                        operand,
                        scope,
                        &format!("{path}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.validate_lexical_return_operand(operand, scope)?;
                }
                (ResolvedType::Never, OwnershipMode::Value)
            }

            ResolvedExprKind::Closure { .. } => {
                self.validate_closure_owned_reference(
                    function,
                    expression,
                    scope,
                    path,
                    allow_moves,
                )?;
                (expression.ty.clone(), expression.ownership)
            }
            ResolvedExprKind::FunctionReference { target } => {
                super::function_value::validate_reference(self.program, target, &expression.ty)?;
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::Invoke { callable, args } => {
                super::function_value::validate_invocation(expression)?;
                self.validate_expr_recursive_reference(
                    function,
                    callable,
                    scope,
                    &format!("{path}.callable"),
                    allow_moves,
                    allowed_effects,
                )?;
                for (i, arg) in args.iter().enumerate() {
                    self.validate_expr_recursive_reference(
                        function,
                        arg,
                        scope,
                        &format!("{path}.arg.{i}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                }
                self.finish_affine_invocation(expression, scope, allow_moves)?;
                (expression.ty.clone(), OwnershipMode::Value)
            }
            ResolvedExprKind::BorrowedUtf8Literal(value) => {
                self.finish_static_text(expression, value)?;
                (ResolvedType::Str, OwnershipMode::Borrow)
            }
            ResolvedExprKind::String(value) => {
                self.record_static_pool(value.len(), false)?;
                (ResolvedType::String, OwnershipMode::Own)
            }
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
            ResolvedExprKind::Usize(_) => (ResolvedType::Usize, OwnershipMode::Value),
            ResolvedExprKind::ArrayU8(values) => (
                ResolvedType::ArrayU8(
                    u32::try_from(values.len())
                        .map_err(|_| hir_error("fixed-array literal length does not fit u32"))?,
                ),
                OwnershipMode::Value,
            ),
            ResolvedExprKind::RepeatArrayU8 { count, .. } => {
                if u64::from(*count) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
                    return Err(hir_error("fixed-array repeat exceeds the exact byte bound"));
                }
                (ResolvedType::ArrayU8(*count), OwnershipMode::Value)
            }
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
            ResolvedExprKind::BorrowPlace { operation, place } => (
                self.validate_borrowed_view(operation, place, expression.span, scope)?,
                OwnershipMode::Borrow,
            ),
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(hir_error(
                        "byte range has an invalid compiler-owned operation",
                    ));
                }
                self.byte_range_origin(source, scope)?;
                self.validate_expr_recursive_reference(
                    function,
                    source,
                    scope,
                    &format!("{path}.arg.0"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_expr_recursive_reference(
                    function,
                    start,
                    scope,
                    &format!("{path}.arg.1"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.validate_expr_recursive_reference(
                    function,
                    end,
                    scope,
                    &format!("{path}.arg.2"),
                    allow_moves,
                    allowed_effects,
                )?;
                self.require_type(&source.ty, &ResolvedType::SliceU8, "byte range source")?;
                self.require_type(&start.ty, &ResolvedType::Usize, "byte range start")?;
                self.require_type(&end.ty, &ResolvedType::Usize, "byte range end")?;
                if source.ownership != OwnershipMode::Borrow
                    || start.ownership != OwnershipMode::Value
                    || end.ownership != OwnershipMode::Value
                {
                    return Err(hir_error("byte range operands have invalid ownership"));
                }
                (ResolvedType::SliceU8, OwnershipMode::Borrow)
            }
            ResolvedExprKind::VecFieldRead { args, .. }
            | ResolvedExprKind::RowLoan { args, .. } => {
                let (ty, ownership, bytes) =
                    self.validate_scoped_vec_node(function, expression, scope)?;
                for (index, argument) in args.iter().enumerate() {
                    let child = if bytes {
                        format!("{path}.arg.0.arg.{index}")
                    } else {
                        format!("{path}.arg.{index}")
                    };
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &child,
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.validate_argument_ownership_view(
                        argument,
                        CallParameters::VecField(args).parameter(index),
                    )?;
                }
                (ty, ownership)
            }
            ResolvedExprKind::LiteralFormat { template, args } => {
                self.record_static_pool(template.len(), false)?;
                let pieces = crate::literal_format::scan(template)
                    .map_err(|reason| hir_error(reason.message()))?;
                if crate::literal_format::field_count(&pieces) != args.len() {
                    return Err(hir_error(
                        "literal format field count does not match arguments",
                    ));
                }
                if function.instance().is_some()
                    || function
                        .monomorphic_declaration()
                        .is_none_or(|id| self.program.declarations.declaration(id).is_none())
                {
                    return Err(hir_error(
                        "literal format is not admitted in generic or closure bodies",
                    ));
                }
                let parameters = CallParameters::LiteralFormat(args);
                for (index, argument) in args.iter().enumerate() {
                    if !crate::literal_format::accepts_hir_type(&argument.ty) {
                        return Err(hir_error("literal format has an unsupported argument type"));
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &format!("{path}.arg.{}", index + 1),
                        allow_moves,
                        allowed_effects,
                    )?;
                    let parameter = parameters.parameter(index);
                    self.validate_argument_ownership_view(argument, parameter)?;
                    let parameter = parameters.parameter(index);
                    if self.is_owned_resource(parameter.ty, parameter.ownership)? {
                        if !allow_moves && !contract_owners::local(argument, scope) {
                            return Err(hir_error(
                                "contract cannot transfer ownership to literal format",
                            ));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                (ResolvedType::String, OwnershipMode::Own)
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                let owned_generic_intrinsic = box_intrinsic::is_call(callee, instance);
                match instance {
                    None if !type_arguments.is_empty() && !owned_generic_intrinsic => {
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
                if !owned_generic_intrinsic
                    && !generic_instance_arguments_are_admitted(
                        self.program,
                        callee,
                        type_arguments,
                    )
                {
                    return Err(hir_error(
                        "resolved call has a generic type argument outside the direct-scalar or owned-record relay profile",
                    ));
                }
                let string_intrinsic = instance
                    .is_none()
                    .then(|| crate::string_ops::by_id(callee.as_str()))
                    .flatten();
                let str_intrinsic = instance
                    .is_none()
                    .then(|| crate::str_ops::by_id(callee.as_str()))
                    .flatten();
                let byte_intrinsic = instance
                    .is_none()
                    .then(|| crate::byte_ops::by_id(callee.as_str()))
                    .flatten();
                let host_io_intrinsic = instance
                    .is_none()
                    .then(|| crate::host_io_ops::by_id(callee.as_str()))
                    .flatten();
                let (params, return_type, target_effects) = if let Some((params, return_type)) =
                    self.intrinsic_signature(callee, type_arguments, instance, args)?
                {
                    (params, return_type, Vec::new())
                } else if let Some(op) = string_intrinsic {
                    string_intrinsic::take_shape(expression, allow_moves)?;
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
                        op.effect().into_iter().map(str::to_owned).collect(),
                    )
                } else if let Some(op) = str_intrinsic {
                    if args.len() != op.arity() {
                        return Err(hir_error(format!(
                            "borrowed string operation `{}` expects {} arguments but received {}",
                            op.name(),
                            op.arity(),
                            args.len()
                        )));
                    }
                    (
                        crate::str_ops::resolved_params(op),
                        op.return_type(),
                        Vec::new(),
                    )
                } else if let Some(op) = byte_intrinsic {
                    if args.len() != op.arity() {
                        return Err(hir_error(format!(
                            "byte operation `{}` expects {} arguments but received {}",
                            op.name(),
                            op.arity(),
                            args.len()
                        )));
                    }
                    owned_buffer::require_admitted_chain(
                        op,
                        args,
                        self.buffer_reopen_sites.contains(&expression.id),
                    )?;
                    (
                        crate::byte_ops::resolved_params(op),
                        op.return_type(),
                        Vec::new(),
                    )
                } else if let Some(op) = host_io_intrinsic {
                    if args.len() != op.arity() || !type_arguments.is_empty() {
                        return Err(hir_error(format!(
                            "invalid host I/O operation `{}` call shape",
                            op.name()
                        )));
                    }
                    (
                        crate::host_io_ops::resolved_params(op),
                        op.return_type(),
                        vec![op.effect().to_owned()],
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
                    let child = crate::vec_ops::string_slot::argument_path(expression, path, index);
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &child,
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &param.ty, "call argument")?;
                    self.validate_argument_ownership(argument, param)?;
                    self.validate_borrowed_bytes_call_argument(
                        expression, argument, param, index, scope,
                    )?;
                    if param.ty == ResolvedType::SliceU8 {
                        self.validate_byte_slice_call_argument(argument, scope)?;
                    }
                    if self.argument_transfers(param)? {
                        if !allow_moves && !contract_owners::local(argument, scope) {
                            return Err(hir_error(format!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                let ownership = if matches!(
                    callee.as_str(),
                    crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
                ) {
                    OwnershipMode::Borrow
                } else {
                    self.expected_ownership(&return_type, OwnershipMode::Own)?
                };
                (return_type, ownership)
            }
            ResolvedExprKind::HostCommandCall(call) => {
                if call.expression != expression.id
                    || call.args.len() != crate::command_io_ops::arity(call.operation)
                {
                    return Err(hir_error(
                        "host-command call has a non-canonical identity or shape",
                    ));
                }
                host_command::require_effects(call.operation, allowed_effects)?;
                let params = crate::command_io_ops::resolved_params(call.operation);
                for (index, (argument, param)) in call.args.iter().zip(&params).enumerate() {
                    self.validate_expr_recursive_reference(
                        function,
                        argument,
                        scope,
                        &format!("{path}.arg.{index}"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    self.require_type(&argument.ty, &param.ty, "host-command argument")?;
                    self.validate_argument_ownership(argument, param)?;
                    if param.ty == ResolvedType::SliceU8
                        && !matches!(
                            &argument.kind,
                            ResolvedExprKind::Place(place)
                                if place.projections.is_empty()
                                    && self.byte_slice_aliases.contains_key(&place.root)
                        )
                        && !matches!(
                            &argument.kind,
                            ResolvedExprKind::BorrowPlace { place, .. }
                                if place.projections.is_empty()
                        )
                    {
                        return Err(hir_error(
                            "host-command byte slice lacks authenticated root provenance",
                        ));
                    }
                }
                (
                    crate::command_io_ops::return_type(call.operation),
                    crate::command_io_ops::result_ownership(call.operation),
                )
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
                    if argument.ownership != parameter.ownership
                        && !(parameter.ownership == OwnershipMode::Borrow
                            && argument.ownership == OwnershipMode::Own
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
                    {
                        return Err(hir_error(
                            "native Rust import argument ownership disagrees with its declaration",
                        ));
                    }
                    if parameter.ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership to native Rust",
                            ));
                        }
                        self.mark_value_sources_moved(argument, scope)?;
                    }
                }
                let result = call.result.value_type(&self.program.declarations)?;
                let ownership = call.result.ownership();
                (result, ownership)
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
                        if !matches!(left.ty, ResolvedType::I64 | ResolvedType::String) {
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
                        if left.ty.is_callable() || right.ty.is_callable() {
                            return Err(hir_error(
                                "function value equality is outside the admitted profile",
                            ));
                        }
                        self.require_type(&left.ty, &right.ty, "equality operands")?;
                        ResolvedType::Bool
                    }
                };
                (output, OwnershipMode::Value)
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                super::vec_constructor::authenticate(expression)?;
                self.validate_record_traversal(expression, scope)?;
                let mut block_scope = scope.clone();
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        ResolvedStatement::Let {
                            binding,
                            mutable,
                            value,
                            ..
                        } => {
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
                            if binding.ty == ResolvedType::SliceU8 {
                                let (place, origin) = match &value.kind {
                                    ResolvedExprKind::Place(place) => {
                                        let origin = self.byte_slice_aliases.get(&place.root).cloned().ok_or_else(|| {
                                            hir_error("byte-slice local alias lacks symbolic parameter-root provenance")
                                        })?;
                                        (place, origin)
                                    }
                                    ResolvedExprKind::BorrowPlace { place, .. } => {
                                        (place, place.clone())
                                    }
                                    ResolvedExprKind::Call { .. }
                                        if crate::vec_ops::string_slot::inspect(value)
                                            .is_some() =>
                                    {
                                        let place = crate::vec_ops::string_slot::place(value)
                                            .expect("authenticated slot");
                                        (place, place.clone())
                                    }
                                    ResolvedExprKind::VecFieldRead { args, .. } => {
                                        let ResolvedExprKind::Place(place) = &args[0].kind else {
                                            return Err(hir_error(
                                                "vector view has no carrier place",
                                            ));
                                        };
                                        (place, place.clone())
                                    }
                                    ResolvedExprKind::ByteRange { source, .. } => {
                                        self.byte_range_origin(source, &block_scope)?
                                    }
                                    _ => {
                                        return Err(hir_error(
                                            "byte-slice local must be a direct immutable alias or authenticated view",
                                        ));
                                    }
                                };
                                let is_authenticated_view =
                                    crate::vec_ops::string_slot::inspect(value).is_some()
                                        || matches!(
                                            &value.kind,
                                            ResolvedExprKind::BorrowPlace { .. }
                                                | ResolvedExprKind::VecFieldRead { .. }
                                        );
                                if *mutable
                                    || (!is_authenticated_view && !place.projections.is_empty())
                                {
                                    return Err(hir_error(
                                        "byte-slice local alias must be immutable and unprojected",
                                    ));
                                }
                                self.byte_slice_aliases
                                    .insert(binding.id.clone(), origin.clone());
                                let borrowed_origin = match &value.kind {
                                    ResolvedExprKind::Call { .. }
                                        if crate::vec_ops::string_slot::inspect(value)
                                            .is_some() =>
                                    {
                                        Some(origin)
                                    }
                                    ResolvedExprKind::BorrowPlace { .. }
                                    | ResolvedExprKind::VecFieldRead { .. }
                                    | ResolvedExprKind::ByteRange { .. } => Some(origin),
                                    _ => None,
                                };
                                if let Some(origin) = borrowed_origin {
                                    let owner =
                                        block_scope.get_mut(&origin.root).ok_or_else(|| {
                                            hir_error(
                                                "byte view owner disappeared before lexical borrow",
                                            )
                                        })?;
                                    owner.active_loans.extend(self.exact_loan_id(
                                        function,
                                        &value.id,
                                        &LoanCause::SliceView,
                                    )?);
                                }
                            } else if binding.ty == ResolvedType::Str {
                                let (origin, creates_loan) =
                                    self.borrowed_str_let_origin(value, binding)?;
                                if *mutable {
                                    return Err(hir_error(
                                        "borrowed-str local alias must be immutable and unprojected",
                                    ));
                                }
                                self.borrowed_str_aliases
                                    .insert(binding.id.clone(), origin.clone());
                                if creates_loan {
                                    let owner =
                                        block_scope.get_mut(&origin.root).ok_or_else(|| {
                                            hir_error(
                                            "String view owner disappeared before lexical borrow",
                                        )
                                        })?;
                                    owner.active_loans.extend(self.exact_loan_id(
                                        function,
                                        &value.id,
                                        &LoanCause::StrView,
                                    )?);
                                }
                            }
                            let contract_local =
                                !allow_moves && contract_owners::fresh(value, &block_scope);
                            if self.is_owned_resource(&binding.ty, binding.ownership)? {
                                if !allow_moves && !contract_owners::fresh(value, &block_scope) {
                                    return Err(hir_error(
                                        "contract cannot transfer ownership into a local binding",
                                    ));
                                }
                                self.mark_value_sources_moved(value, &mut block_scope)?;
                            }
                            block_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    contract_local,
                                    mutable: *mutable,
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    availability: Availability::Available,
                                    active_loans: BTreeSet::new(),
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            );
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            span: assignment_span,
                            ..
                        } => {
                            if crate::byte_ops::is_same_owner_set_hir(assigned, &binding.id) {
                                self.buffer_reopen_sites.insert(assigned.id.clone());
                            }
                            let statement_path = format!("{path}.s{index}");
                            if crate::string_ops::replacement::admitted(binding, assigned)
                                && block_scope.get(&binding.id).is_none_or(|target| {
                                    target.availability != Availability::Available
                                })
                            {
                                return Err(hir_error(
                                    "String replacement starts with an unavailable owner",
                                ));
                            }
                            self.validate_expr_recursive_reference(
                                function,
                                assigned,
                                &mut block_scope,
                                &format!("{statement_path}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if crate::hir::iterator_loop::is_step_reassignment(
                                assigned,
                                &binding.id,
                            ) {
                                iterator_loops::reopen_step(&mut block_scope, &binding.id)?;
                            }
                            if crate::stdin_stream_ops::hir_reopen(assigned, &binding.id) {
                                stdin_stream::reopen_reader(&mut block_scope, &binding.id)?;
                            }
                            if crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id) {
                                iterator_loops::reopen_string(&mut block_scope, &binding.id)?;
                            } else if crate::string_ops::replacement::admitted(binding, assigned) {
                                self.mark_value_sources_moved(assigned, &mut block_scope)?;
                                string_replacement::reopen(&mut block_scope, &binding.id)?;
                            }
                            let target = block_scope.get(&binding.id).cloned();
                            let Some(target) = target else {
                                return Err(hir_error(format!(
                                    "assignment target `{}` has no enclosing binding",
                                    binding.id
                                )));
                            };
                            let assigned_place = Place {
                                root: binding.id.clone(),
                                projections: field
                                    .iter()
                                    .cloned()
                                    .map(PlaceProjection::Field)
                                    .collect(),
                            };
                            if self.binding_has_live_overlapping_loans(
                                &target,
                                &assigned_place,
                                &assigned.id,
                                LoanPointPhase::After,
                            )? {
                                return Err(hir_error(
                                    "SPX-T265: assignment would replace lexically borrowed byte storage",
                                ));
                            }
                            match field {
                                Some(field) => {
                                    if target.ownership != OwnershipMode::Value {
                                        return Err(hir_error(
                                            "field assignment base is not a value-owned aggregate",
                                        ));
                                    }
                                    self.validate_assign_field(
                                        &target.ty,
                                        field,
                                        assigned,
                                        *assignment_span,
                                    )?;
                                }
                                None => {
                                    self.require_type(&target.ty, &assigned.ty, "assignment")?;
                                    if (target.ownership != OwnershipMode::Value
                                        || !crate::hir::is_scalar_resolved_type(&target.ty))
                                        && !crate::string_ops::replacement::admitted(
                                            binding, assigned,
                                        )
                                        && !owner_renewal::assignment_is_admitted(
                                            self.program,
                                            self.byte_helpers,
                                            binding,
                                            assigned,
                                        )
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
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            self.validate_expr_recursive_reference(
                                function,
                                value,
                                &mut block_scope,
                                &format!("{path}.s{index}.value"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            super::copy_discard::require(value)?;
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
                        }
                        | ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1: mirror the iterative
                            // admission re-check, condition typing, body
                            // validation, and exact entry-state equality.
                            self.validate_loop_pair(condition, body)?;
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
                            if block_scope != entry_scope {
                                return Err(hir_error(
                                    "while condition changes ownership liveness",
                                ));
                            }
                            self.validate_expr_recursive_reference(
                                function,
                                body,
                                &mut block_scope,
                                &format!("{path}.s{index}.body"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if !super::lexical_exit::returns_only(body)
                                && block_scope != entry_scope
                            {
                                return Err(hir_error(
                                    "while loop body changes ownership liveness",
                                ));
                            }
                            block_scope = entry_scope;
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
                self.finish_lexical_if(
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                    &then_scope,
                    &else_scope,
                    &outer_ids,
                )?;
                (expression.ty.clone(), expression.ownership)
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                if crate::map_ops::is_declaration(record.as_str())
                    || record.as_str() == crate::iterator_ops::ITER_ID
                    || record.as_str() == crate::list_ops::LIST_ID
                {
                    return Err(hir_error("iterator owner cannot be authored as a record"));
                }
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
                    || !super::type_reachability::record_args_ok(
                        &self.program.declarations,
                        record,
                        arguments,
                    )
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
                if expression.ty.is_compiler_byte_option() {
                    return Err(hir_error(
                        "Option<u8> may be produced only by compiler-owned byte_get",
                    ));
                }
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
                    let ownership = self.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(hir_error(format!(
                            "variant payload field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a variant",
                            ));
                        }
                        self.mark_value_sources_moved(&initializer.value, scope)?;
                    }
                }
                if seen.len() != expected_fields.len() {
                    return Err(hir_error(format!(
                        "constructor for `{case}` is missing required payload fields"
                    )));
                }
                let ownership = self.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                (expression.ty.clone(), ownership)
            }
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                self.validate_expr_recursive_reference(
                    function,
                    scrutinee,
                    scope,
                    &format!("{path}.scrutinee"),
                    allow_moves,
                    allowed_effects,
                )?;
                // Refutable Match v1: mirror of the iterative decision-chain
                // twins over a Copy-scalar scrutinee.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    if *mode != ResolvedMatchMode::Value
                        || scrutinee.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error("resolved refutable match scrutinee is not Copy"));
                    }
                    if arms.is_empty() {
                        return Err(hir_error("resolved match has no arms"));
                    }
                    for (arm_index, arm) in arms.iter().enumerate() {
                        let mut arm_scope = scope.clone();
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {}
                            ResolvedMatchPattern::Binding(binding) => {
                                if binding.id
                                    != ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding"),
                                    )
                                    || binding.ty != scrutinee.ty
                                    || binding.ownership != OwnershipMode::Value
                                {
                                    return Err(hir_error(
                                        "resolved scalar match binding is invalid",
                                    ));
                                }
                                self.insert_value(&binding.id)?;
                                self.validate_type(&binding.ty)?;
                                if arm_scope.contains_key(&binding.id) {
                                    return Err(hir_error(
                                        "resolved match pattern binding shadows an existing value",
                                    ));
                                }
                                arm_scope.insert(
                                    binding.id.clone(),
                                    ValidationBinding {
                                        contract_local: false,
                                        mutable: false,
                                        ty: binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        availability: Availability::Available,
                                        active_loans: BTreeSet::new(),
                                        moved_places: BTreeMap::new(),
                                        definitely_partial: BTreeSet::new(),
                                    },
                                );
                            }
                            ResolvedMatchPattern::Literal(value) => {
                                if value.ty() != scrutinee.ty {
                                    return Err(hir_error(
                                        "resolved literal pattern disagrees with its scrutinee \
                                         type",
                                    ));
                                }
                            }
                            ResolvedMatchPattern::Or(alternatives) => {
                                if alternatives.is_empty() {
                                    return Err(hir_error(
                                        "resolved or-pattern has no alternatives",
                                    ));
                                }
                                for alternative in alternatives {
                                    let ResolvedMatchPattern::Literal(value) = alternative else {
                                        return Err(hir_error(
                                            "resolved or-pattern contains a non-literal \
                                             alternative",
                                        ));
                                    };
                                    if value.ty() != scrutinee.ty {
                                        return Err(hir_error(
                                            "resolved or-pattern alternative disagrees with its \
                                             scrutinee type",
                                        ));
                                    }
                                }
                            }
                            ResolvedMatchPattern::Variant { .. }
                            | ResolvedMatchPattern::Record { .. } => {
                                return Err(hir_error(
                                    "resolved aggregate pattern has a Copy-scalar scrutinee",
                                ));
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            self.validate_expr_recursive_reference(
                                function,
                                guard.as_ref(),
                                &mut arm_scope,
                                &format!("{path}.arm.{arm_index}.guard"),
                                allow_moves,
                                allowed_effects,
                            )?;
                            if guard.ty != ResolvedType::Bool
                                || guard.ownership != OwnershipMode::Value
                            {
                                return Err(hir_error("resolved match guard is not exactly bool"));
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
                    }
                    let last = arms.last().expect("empty checked above");
                    let catch_all = matches!(
                        &last.pattern,
                        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Binding(_)
                    );
                    if !catch_all || last.guard.is_some() {
                        return Err(hir_error(
                            "resolved refutable match lacks a trailing irrefutable guard-free \
                             catch-all",
                        ));
                    }
                    for arm in arms.iter().skip(1) {
                        self.require_type(&arm.value.ty, &arms[0].value.ty, "match arm")?;
                        if arm.value.ownership != arms[0].value.ownership {
                            return Err(hir_error(
                                "resolved match arms have inconsistent ownership",
                            ));
                        }
                    }
                    self.require_type(&expression.ty, &arms[0].value.ty, "match expression")?;
                    if expression.ownership != arms[0].value.ownership {
                        return Err(hir_error(
                            "resolved match expression has inconsistent ownership",
                        ));
                    }
                    return Ok(());
                }
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
                    let facts = self
                        .program
                        .declarations
                        .type_facts(&scrutinee.ty)
                        .ok_or_else(|| hir_error("record match has no exact type facts"))?;
                    match mode {
                        ResolvedMatchMode::Value
                            if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                        ResolvedMatchMode::Own
                            if facts.needs_drop
                                && !facts.copy
                                && scrutinee.ownership == OwnershipMode::Own
                                && (!super::type_reachability::is_nested_nonflat_owned_byte_record(
                                    &self.program.declarations,
                                    &scrutinee.ty,
                                )
                                    || matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    )) =>
                        {
                            self.mark_value_sources_moved(scrutinee, scope)?;
                        }
                        ResolvedMatchMode::Borrow
                            if facts.needs_drop
                                && !facts.copy
                                && matches!(
                                    scrutinee.ownership,
                                    OwnershipMode::Own | OwnershipMode::Borrow
                                )
                                && matches!(
                                    &scrutinee.kind,
                                    ResolvedExprKind::Place(place)
                                        if place.projections.is_empty()
                                ) => {}
                        _ => {
                            return Err(hir_error(
                                "resolved record match mode disagrees with its scrutinee",
                            ));
                        }
                    }
                    let [arm] = arms.as_slice() else {
                        return Err(hir_error(
                            "resolved irrefutable record match must have exactly one arm",
                        ));
                    };
                    let outer_ids = scope.keys().cloned().collect::<Vec<_>>();
                    let mut arm_scope = scope.clone();
                    let borrowed_root = if *mode == ResolvedMatchMode::Borrow {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!("borrow mode authenticated a place above")
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root) {
                            owner.active_loans.extend(self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow { arm: 0 },
                            )?);
                        }
                        Some(place.root.clone())
                    } else {
                        None
                    };
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard if *mode != ResolvedMatchMode::Value => {
                            return Err(hir_error(
                                "resolved explicit ownership match lacks an exact record pattern",
                            ));
                        }
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => self
                            .validate_record_match_pattern_owned_reference(
                                function,
                                &scrutinee.ty,
                                record,
                                instance,
                                fields,
                                &mut arm_scope,
                                &format!("{path}.arm.0.record"),
                                *mode,
                            )
                            .map_err(|diagnostic| {
                                hir_diagnostic_at_span(diagnostic, expression.span)
                            })?,
                        ResolvedMatchPattern::Variant { .. } => {
                            return Err(hir_error(
                                "resolved variant pattern has a record scrutinee",
                            ));
                        }
                        ResolvedMatchPattern::Literal(_)
                        | ResolvedMatchPattern::Or(_)
                        | ResolvedMatchPattern::Binding(_) => {
                            return Err(hir_error(
                                "resolved refutable pattern has an aggregate record scrutinee",
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
                    if let Some(root) = borrowed_root {
                        if let Some(owner) = arm_scope.get_mut(&root) {
                            if let Some(loan) = self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow { arm: 0 },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    self.validate_generic_record_match_result(function, expression, arm)?;
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
                if self
                    .program
                    .declarations
                    .declaration(variant)
                    .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(hir_error(
                        "resolved match scrutinee is not a concrete variant",
                    ));
                }
                let facts = self
                    .program
                    .declarations
                    .type_facts(&scrutinee.ty)
                    .ok_or_else(|| hir_error("variant match has no exact type facts"))?;
                match mode {
                    ResolvedMatchMode::Value
                        if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                    ResolvedMatchMode::Own
                        if (resolved_type_is_flat_owned_byte_variant(
                            self.program,
                            &scrutinee.ty,
                        ) || resolved_type_is_direct_owned_variant(
                            self.program,
                            &scrutinee.ty,
                        )) && facts.needs_drop
                            && !facts.copy
                            && scrutinee.ownership == OwnershipMode::Own =>
                    {
                        self.mark_value_sources_moved(scrutinee, scope)?;
                    }
                    ResolvedMatchMode::Borrow
                        if (resolved_type_is_flat_owned_byte_variant(
                            self.program,
                            &scrutinee.ty,
                        ) || resolved_type_is_direct_owned_variant(
                            self.program,
                            &scrutinee.ty,
                        ) || regex_result(self.program, &scrutinee.ty))
                            && facts.needs_drop
                            && !facts.copy
                            && matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            )
                            && matches!(&scrutinee.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()) =>
                        {}
                    _ => {
                        return Err(hir_error(
                            "resolved variant match mode disagrees with its scrutinee",
                        ));
                    }
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
                    if *mode == ResolvedMatchMode::Borrow {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!()
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root) {
                            owner.active_loans.extend(self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow {
                                    arm:
                                        u16::try_from(arm_index).map_err(|_| {
                                            hir_error("match loan arm index overflows")
                                        })?,
                                },
                            )?);
                        }
                    }
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {
                            if *mode != ResolvedMatchMode::Value {
                                return Err(hir_error(
                                    "resolved explicit ownership match cannot discard an owned variant",
                                ));
                            }
                            if wildcard_seen || covered.len() == cases.len() {
                                return Err(hir_error(
                                    "resolved match has an unreachable wildcard",
                                ));
                            }
                            wildcard_seen = arm.guard.is_none();
                        }
                        ResolvedMatchPattern::Variant {
                            variant: pattern_variant,
                            case,
                            fields,
                        } => {
                            if wildcard_seen
                                || pattern_variant != variant
                                || covered.contains(case)
                                || (arm.guard.is_none() && !covered.insert(case.clone()))
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
                                let facts = self
                                    .program
                                    .declarations
                                    .type_facts(&binding_ty)
                                    .ok_or_else(|| {
                                        hir_error("variant binding has no exact type facts")
                                    })?;
                                let binding_ownership = if facts.copy {
                                    OwnershipMode::Value
                                } else if *mode == ResolvedMatchMode::Own {
                                    OwnershipMode::Own
                                } else if *mode == ResolvedMatchMode::Borrow {
                                    OwnershipMode::Borrow
                                } else {
                                    return Err(hir_error(
                                        "Copy variant pattern contains an owned payload",
                                    ));
                                };
                                if !seen_fields.insert(pattern_field.field.clone())
                                    || pattern_field.binding.id
                                        != ValueId::local(
                                            function,
                                            &format!(
                                                "{path}.arm.{arm_index}.binding.{field_index}"
                                            ),
                                        )
                                    || pattern_field.binding.ty != binding_ty
                                    || pattern_field.binding.ownership != binding_ownership
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
                                        contract_local: false,
                                        mutable: false,
                                        ty: pattern_field.binding.ty.clone(),
                                        ownership: binding_ownership,
                                        availability: Availability::Available,
                                        active_loans: BTreeSet::new(),
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
                        ResolvedMatchPattern::Or(alternatives) => {
                            let mut candidate = covered.clone();
                            variant_or::cover_variant_or_pattern(
                                alternatives,
                                variant,
                                &cases,
                                *mode,
                                wildcard_seen,
                                &mut candidate,
                            )?;
                            if arm.guard.is_none() {
                                covered = candidate;
                            }
                        }
                        ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Binding(_) => {
                            return Err(hir_error(
                                "resolved refutable pattern has an aggregate variant scrutinee",
                            ));
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        if !crate::variant_guards::admitted(
                            &self.program.declarations,
                            &scrutinee.ty,
                            *mode,
                            &arm.pattern,
                            guard,
                        ) {
                            return Err(hir_error(
                                "resolved Copy variant guard is outside its ordinary Boolean profile",
                            ));
                        }
                        self.validate_expr_recursive_reference(
                            function,
                            guard,
                            &mut arm_scope,
                            &format!("{path}.arm.{arm_index}.guard"),
                            allow_moves,
                            allowed_effects,
                        )?;
                        if guard.ty != ResolvedType::Bool {
                            return Err(hir_error("resolved match guard is not bool"));
                        }
                        variant_guard_state::unchanged(scope, &arm_scope)?;
                    }
                    self.validate_expr_recursive_reference(
                        function,
                        &arm.value,
                        &mut arm_scope,
                        &format!("{path}.arm.{arm_index}.value"),
                        allow_moves,
                        allowed_effects,
                    )?;
                    if *mode == ResolvedMatchMode::Borrow {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!()
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root) {
                            if let Some(loan) = self.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow {
                                    arm: u16::try_from(arm_index)
                                        .map_err(|_| hir_error("match loan arm index overflows"))?,
                                },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    if arm.value.ty != ResolvedType::Never
                        && !super::collection_outcome::match_arm_execution(
                            self.program,
                            function,
                            expression,
                            &arm.value,
                        )
                    {
                        return Err(hir_error(
                            "resolved owned variant match arm must produce a Copy scalar",
                        ));
                    }
                    self.join_lexical_match(&mut result, &arm.value)?;
                    if !super::lexical_exit::returns_only(&arm.value) {
                        arm_scopes.push(arm_scope);
                    }
                }
                if !wildcard_seen
                    && covered.len() != cases.len()
                    && !self.observed_partial_byte_receiver(expression)
                {
                    return Err(hir_error("resolved match is not exhaustive"));
                }
                if let Some((first, rest)) = arm_scopes.split_first() {
                    let mut joined = first.clone();
                    for arm_scope in rest {
                        Self::join_conditional(&mut joined, arm_scope, &outer_ids);
                    }
                    Self::merge_availability(scope, &joined, &outer_ids);
                    Self::merge_lexical_borrows(scope, &joined, &outer_ids);
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
                if owned_result_try::expression_owns_exact_operand(operand) {
                    self.mark_value_sources_moved(operand, scope)?;
                }
                if !path.starts_with("body") {
                    return Err(hir_error(
                        "resolved `?` is outside the executable function body",
                    ));
                }
                if scope.values().any(|binding| {
                    binding.availability != Availability::Moved
                        && self
                            .program
                            .declarations
                            .type_facts(&binding.ty)
                            .is_some_and(|facts| facts.needs_drop)
                }) {
                    return Err(hir_error(
                        "resolved `?` has an unrelated live owned binding",
                    ));
                }
                let profile = owned_result_try::validate_shape(
                    expression,
                    operand,
                    result,
                    ok_case,
                    ok_field,
                    err_case,
                    err_field,
                    residual_type,
                )?;
                let enclosing_return = self
                    .execution_return_type(function)
                    .ok_or_else(|| hir_error("resolved `?` has no enclosing function"))?;
                self.require_type(residual_type, enclosing_return, "`?` residual")?;
                self.require_type(
                    &expression.ty,
                    &profile.operand_arguments[0],
                    "`?` success value",
                )?;
                self.require_type(
                    &profile.operand_arguments[1],
                    &profile.residual_arguments[1],
                    "`?` residual error",
                )?;
                (expression.ty.clone(), expression.ownership)
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
                } = &**residual_type
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
                    .execution_return_type(function)
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
                    || !super::type_reachability::record_args_ok(
                        &self.program.declarations,
                        record,
                        arguments,
                    )
                {
                    return Err(hir_error(format!(
                        "record update for `{record}` has an invalid concrete instance"
                    )));
                }
                validate_nested_update_base_shape(self.program, base, expression.span)?;
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
            // Resumable Effects v1 (issue #204): mirrors `Frame::Yield` in
            // the iterative validator above -- the request is an ordinary
            // child, and the whole node's type/ownership were already
            // fixed up to the declared response type by
            // `hir::resolve_yield`.
            ResolvedExprKind::Yield { request } => {
                self.validate_expr_recursive_reference(
                    function,
                    request,
                    scope,
                    &format!("{path}.request"),
                    allow_moves,
                    allowed_effects,
                )?;
                let ownership = self.expected_ownership(&expression.ty, OwnershipMode::Value)?;
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
            || !super::type_reachability::record_args_ok(
                &self.program.declarations,
                declaration,
                arguments,
            )
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

    #[cfg(test)]
    fn argument_transfers(&self, param: &ResolvedParam) -> Result<bool, Diagnostic> {
        self.is_owned_resource(&param.ty, param.ownership)
    }

    fn is_owned_resource(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<bool, Diagnostic> {
        if *ty == ResolvedType::Never {
            return Ok(false);
        }
        #[cfg(test)]
        if ownership_flags::original_selected() {
            return self.is_owned_resource_before(ty, ownership);
        }
        self.borrowed_ownership_copy(ty)?
            .map(|facts| !facts && ownership == OwnershipMode::Own)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }

    fn mark_value_sources_moved_owned_reference(
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
                    ResolvedExprKind::LexicalExit { value, .. } => {
                        if let Some(value) = value {
                            frames.push(Frame::Enter(value, scope_index));
                        }
                    }
                    ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::FunctionReference { .. } => {}
                    ResolvedExprKind::Invoke { args, .. } => {
                        for arg in args.iter().rev() {
                            frames.push(Frame::Enter(arg, scope_index));
                        }
                    }
                    ResolvedExprKind::Place(place) => {
                        let Some(binding) = scopes[scope_index].get(&place.root) else {
                            continue;
                        };
                        let (place_ty, place_ownership) = self.resolve_place(place, binding)?;
                        let should_move = self.is_owned_resource(&place_ty, place_ownership)?
                            && Self::place_availability(binding, &place.projections)
                                == Availability::Available;
                        if should_move {
                            if self.binding_has_live_overlapping_loans(
                                binding,
                                place,
                                &expression.id,
                                LoanPointPhase::Before,
                            )? {
                                return Err(hir_error(
                                    "SPX-T265: move or transfer would invalidate a lexical byte view",
                                ));
                            }
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
                    ResolvedExprKind::Block { tail, .. }
                    | ResolvedExprKind::VecConstructor { tail, .. } => {
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
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::BorrowedUtf8Literal(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::BorrowPlace { .. }
                    | ResolvedExprKind::ByteRange { .. }
                    | ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::VecFieldRead { .. }
                    | ResolvedExprKind::RowLoan { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::ConstructRecord { .. }
                    | ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::Yield { .. }
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

    /// Class Inheritance v1: independent re-derivation of the upcast
    /// contract. The source must be a descendant class value whose effective
    /// field sequence extends the ancestor's exactly, with a cleanup-inert
    /// child-declared suffix.
    fn expected_ownership(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        if *ty == ResolvedType::Never {
            return Ok(OwnershipMode::Value);
        }
        #[cfg(test)]
        if ownership_flags::original_selected() {
            return self.expected_ownership_before(ty, non_copy);
        }
        self.borrowed_ownership_copy(ty)?
            .map(|facts| {
                if facts {
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

    fn insert_value(&mut self, id: &ValueId) -> Result<(), Diagnostic> {
        reject_nul_identity("resolved value", id.as_str())?;
        if identity_insert::value(&mut self.value_ids, id)? {
            Ok(())
        } else {
            Err(hir_error(format!(
                "duplicate resolved value identity `{id}`"
            )))
        }
    }
}

#[cfg(test)]
#[path = "validation/owned_byte_record_hostile_tests.rs"]
mod owned_byte_record_hostile_tests;

#[cfg(test)]
#[path = "validation/borrowed_bytes_call_tests.rs"]
mod borrowed_bytes_call_tests;

#[cfg(test)]
#[path = "validation/iterative_while_admission_tests.rs"]
mod iterative_while_admission_tests;

#[cfg(test)]
mod literal_format_result_tests;

mod fused_selection;
mod generic_variant;
mod iterator_loops;
mod place_resolution;
mod string_replacement;
mod variant_guard_state;
mod variant_or;
