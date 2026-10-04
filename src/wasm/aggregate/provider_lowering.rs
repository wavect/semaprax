//! Closure lowering for the closed public-generic Core Wasm provider target.
//!
//! The ordinary aggregate profiles number their imported helpers before their
//! generated functions.  The provider has no imports: it emits equivalent
//! internal byte-runtime functions in those canonical slots instead.  This
//! module therefore owns the one place that offsets every generated direct
//! call without changing the legacy aggregate emitters.

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, FunctionExecutionId, ResolvedFunction, ResolvedProgram};
use crate::variant_layout::{VariantLayoutCache, VariantTarget};

use super::{
    emit_function, error, is_aggregate, resource_gate, scalar_wasm_type, SelectedAggregateLowering,
    Signature, BYTE_IMPORT_COUNT, I32, OWNED_BUFFER_IMPORT_COUNT, SCALAR_IMPORT_COUNT,
};

/// The first source-function index available to the closed provider module.
///
/// Slots `0..7` are the aggregate lane's scalar helper reservation and slots
/// `7..13` are its owned-byte runtime (`copy`, `get`, `drop`, `as_slice`,
/// `zeroed`, and `set`).  Unlike legacy modules these functions are emitted
/// by the provider itself, never imported from a browser host.  Provider ABI
/// helpers may occupy later slots, so callers supply the actual source base
/// and this lowering rejects a base that would overlap the runtime.
pub(in crate::wasm) const PUBLIC_GENERIC_PROVIDER_SOURCE_INDEX_MIN: u32 =
    SCALAR_IMPORT_COUNT + BYTE_IMPORT_COUNT + OWNED_BUFFER_IMPORT_COUNT;

/// Lower the exact reachable closure of one admitted monomorphic endpoint.
///
/// The public-generic boundary admits a monomorphic exported function whose
/// record types may be concrete generic instances.  Its implementation may
/// nevertheless call concrete instances of internal generic helpers.  The
/// ordinary selected-function lowerers handle only one of those execution
/// identity classes at a time; this target needs both in a single function
/// index map so direct calls preserve their checked HIR identity.
///
/// `source_function_index_base` is the absolute Wasm function index at which
/// the caller will place the first returned body.  Every returned direct-call
/// index, including `selected_index`, is absolute and ready for the final
/// module function section.
pub(in crate::wasm) fn lower_public_generic_provider_closure(
    program: &ResolvedProgram,
    selected: &DeclarationId,
    source_function_index_base: u32,
) -> Result<SelectedAggregateLowering, Diagnostic> {
    crate::hir::validate(program)?;
    if source_function_index_base < PUBLIC_GENERIC_PROVIDER_SOURCE_INDEX_MIN {
        return Err(error(format!(
            "public-generic provider source function base {source_function_index_base} overlaps the reserved internal byte runtime through index {}",
            PUBLIC_GENERIC_PROVIDER_SOURCE_INDEX_MIN - 1,
        )));
    }
    if program.types.iter().any(|item| {
        matches!(
            item.kind,
            crate::hir::ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(resource_gate());
    }
    if program
        .function_templates
        .iter()
        .any(|template| template.id == *selected)
    {
        return Err(error(format!(
            "public-generic provider endpoint `{selected}` must be monomorphic, not a function template"
        )));
    }
    if !program
        .functions
        .iter()
        .any(|function| function.id == *selected)
    {
        return Err(error(format!(
            "public-generic provider endpoint `{selected}` is not a resolved monomorphic function"
        )));
    }

    let ordered = reachable_execution_closure(program, selected)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let mut function_type_indexes = Vec::with_capacity(ordered.len());
    let mut function_indexes = HashMap::with_capacity(ordered.len());

    for (offset, execution) in ordered.iter().enumerate() {
        let function = function_for_execution(program, execution)?;
        let mut params = Vec::with_capacity(function.params.len() + 1);
        for param in &function.params {
            params.push(if is_aggregate(program, &param.ty)? {
                I32
            } else {
                scalar_wasm_type(program, &param.ty)?
            });
        }
        params.push(I32);
        function_type_indexes.push(super::intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
        let offset = u32::try_from(offset)
            .map_err(|_| error("public-generic provider function count overflows u32"))?;
        let index = source_function_index_base
            .checked_add(offset)
            .ok_or_else(|| error("public-generic provider function index overflows u32"))?;
        if function_indexes.insert(execution.clone(), index).is_some() {
            return Err(error(format!(
                "public-generic provider closure repeats `{}`",
                execution.identity_key()
            )));
        }
    }

    let selected_execution = FunctionExecutionId::Monomorphic(selected.clone());
    let selected_index = *function_indexes.get(&selected_execution).ok_or_else(|| {
        error(format!(
            "public-generic provider closure omits selected endpoint `{selected}`"
        ))
    })?;
    let bodies = ordered
        .iter()
        .map(|execution| {
            emit_function(
                program,
                function_for_execution(program, execution)?,
                &function_indexes,
                &HashMap::new(),
                &HashMap::new(),
                &variant_layouts,
                None,
                None,
                None,
                None,
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SelectedAggregateLowering {
        types,
        function_type_indexes,
        bodies,
        selected_index,
    })
}

fn reachable_execution_closure(
    program: &ResolvedProgram,
    selected: &DeclarationId,
) -> Result<Vec<FunctionExecutionId>, Diagnostic> {
    let selected_execution = FunctionExecutionId::Monomorphic(selected.clone());
    let mut reached = BTreeSet::from([selected_execution.clone()]);
    let mut pending = VecDeque::from([selected_execution]);

    while let Some(execution) = pending.pop_front() {
        let function = function_for_execution(program, &execution)?;
        for expression in function
            .requires
            .iter()
            .chain(function.ensures.iter())
            .chain(std::iter::once(&function.body))
        {
            crate::hir::visit_resolved_calls(expression, &mut |callee, instance, _| {
                // The aggregate emitter lowers byte intrinsics directly to
                // the provider's reserved in-module runtime slots.  Do not
                // accidentally index a standard-library declaration here:
                // `bytes_zeroed` and `bytes_set` intentionally fall back to
                // slots 11 and 12 when absent from `function_indexes`.
                if instance.is_none() && crate::byte_ops::by_id(callee.as_str()).is_some() {
                    return;
                }
                if program.resolve_call_target(callee, instance).is_none() {
                    // Other compiler intrinsics are validated by the checked
                    // HIR and dispatched directly by `emit_function`; they
                    // are not source-function closure edges.
                    return;
                }
                let target = instance.map_or_else(
                    || FunctionExecutionId::Monomorphic(callee.clone()),
                    |instance| FunctionExecutionId::Generic(instance.clone()),
                );
                if reached.insert(target.clone()) {
                    pending.push_back(target);
                }
            });
        }
    }

    // Discovery order follows expression traversal.  The module contract is
    // stronger: body/type order follows the canonical resolved inventories,
    // which are replayed and identity-checked by HIR validation.
    let mut ordered = Vec::with_capacity(reached.len());
    for function in &program.functions {
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        if reached.remove(&execution) {
            ordered.push(execution);
        }
    }
    for instance in &program.function_instances {
        if crate::hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
            != instance.id
        {
            return Err(error(format!(
                "public-generic provider generic helper instance `{}` has inconsistent identity",
                instance.id
            )));
        }
        let execution = FunctionExecutionId::Generic(instance.id.clone());
        if reached.remove(&execution) {
            ordered.push(execution);
        }
    }
    if let Some(unresolved) = reached.into_iter().next() {
        return Err(error(format!(
            "public-generic provider closure target `{}` is absent from the canonical resolved inventory",
            unresolved.identity_key()
        )));
    }
    Ok(ordered)
}

fn function_for_execution<'a>(
    program: &'a ResolvedProgram,
    execution: &FunctionExecutionId,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    match execution {
        FunctionExecutionId::Monomorphic(id) => program
            .functions
            .iter()
            .find(|function| function.id == *id)
            .ok_or_else(|| {
                error(format!(
                    "public-generic provider monomorphic helper `{id}` is missing"
                ))
            }),
        FunctionExecutionId::Generic(instance) => program
            .function_instances
            .iter()
            .find(|candidate| candidate.id == *instance)
            .map(|candidate| &candidate.function)
            .ok_or_else(|| {
                error(format!(
                    "public-generic provider generic helper instance `{instance}` is missing"
                ))
            }),
    }
}
