//! Production composition of the callable-v2 wrapper with the verified native
//! value, cleanup, status, and semantic-trace emitters.
//!
//! The generated hook is a direct C11 execution path. It does not consult the
//! Rust conformance executor, retain request pointers, allocate, invoke a
//! callback, or reconstruct cleanup from source-level syntax.

use std::fmt::Write as _;

use crate::conformance::TraceEventKind;
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedFunction, ResolvedProgram, ResolvedType};
use crate::semantic_trace::SemanticEventDictionary;

use super::native_callable_abi::NativeCallableDescriptor;
use super::native_callable_provider::{
    self, NativeCallableProvider, NativeCallableProviderSpec, ProviderParameter, ProviderResult,
};
use super::native_cleanup::NativeCleanupIndex;
use super::native_resource::NativeResourceAbi;
use super::native_value::{NativeValueDeclaration, NativeValuePlan};

const NORMALIZED_CALLABLE_SYMBOL: &str = "spx_callable_projection";
const NORMALIZED_CONTRACT: [u8; 32] = [0xa5; 32];
const NORMALIZED_TARGET_GUARDS: &str = "/* semaprax.native-callable-provider-target-guards.v1 */\n";
const STATUS_CAPACITY: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutionParameter {
    provider: ProviderParameter,
    c_type: String,
}

/// Sealed compiler facts required to instantiate both the normalized template
/// and the concrete descriptor-bound provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableExecutionPlan {
    parameters: Vec<ExecutionParameter>,
    result: ProviderResult,
    owned_result_parameter_index: Option<usize>,
    result_c_type: String,
    declarations: String,
    resource_bindings: Vec<String>,
    cleanup_body: String,
    max_event_count: u32,
    dictionary_entries: u32,
}

pub(super) struct ConcreteCallableProvider {
    pub(super) source: String,
    pub(super) codec_profile_fingerprint: [u8; 32],
    pub(super) normalized_projection: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    cleanup: &NativeCleanupIndex<'_>,
    values: &NativeValuePlan,
    resource_abi: &NativeResourceAbi,
    dictionary: &SemanticEventDictionary,
    declarations: String,
    cleanup_body: String,
) -> Result<NativeCallableExecutionPlan, Diagnostic> {
    if usize::BITS != 64 {
        return Err(execution_error(
            "opaque owned payloads require an audited 64-bit uintptr_t target",
        ));
    }
    if !values.belongs_to(function, cleanup) || dictionary.function() != &function.id {
        return Err(execution_error(
            "value, cleanup, and dictionary evidence belong to different functions",
        ));
    }
    if values.required_event_capacity == 0
        || values.required_event_capacity > native_callable_provider::MAX_PROVIDER_STACK_EVENTS
    {
        return Err(execution_error(format!(
            "required event capacity {} exceeds the audited {}-event callable stack ceiling",
            values.required_event_capacity,
            native_callable_provider::MAX_PROVIDER_STACK_EVENTS
        )));
    }
    let dictionary_entries = u32::try_from(dictionary.entries().len())
        .map_err(|_| execution_error("semantic dictionary entry count exceeds u32"))?;
    let result_commit_ordinal = unique_result_commit_ordinal(dictionary)?;
    let mut next_owner = 0_u32;
    let mut parameters = Vec::with_capacity(function.params.len());
    for parameter in &function.params {
        let provider = match (&parameter.ty, parameter.ownership) {
            (ResolvedType::I64, OwnershipMode::Value) => ProviderParameter::I64,
            (ResolvedType::Bool, OwnershipMode::Value) => ProviderParameter::Bool,
            (ResolvedType::Nominal { .. }, OwnershipMode::Own) => {
                let owner_ordinal = next_owner;
                next_owner = next_owner
                    .checked_add(1)
                    .ok_or_else(|| execution_error("owned parameter ordinal overflow"))?;
                ProviderParameter::Owned { owner_ordinal }
            }
            _ => {
                return Err(execution_error(format!(
                    "parameter `{}` is outside the callable-v2 direct execution slice",
                    parameter.id
                )))
            }
        };
        parameters.push(ExecutionParameter {
            provider,
            c_type: resource_abi.c_type(program, &parameter.ty)?.to_owned(),
        });
    }
    let (result, owned_result_parameter_index) = match values.result() {
        super::native_value::NativeValueResult::ScalarI64 => {
            if function.return_type != ResolvedType::I64 {
                return Err(execution_error(
                    "scalar value result disagrees with the function result type",
                ));
            }
            (
                ProviderResult::ScalarI64 {
                    result_commit_ordinal,
                },
                None,
            )
        }
        super::native_value::NativeValueResult::OwnedInput {
            parameter_index,
            parameter,
            owner_ordinal,
        } => {
            let admitted = function
                .params
                .get(*parameter_index)
                .ok_or_else(|| execution_error("owned result selects a missing parameter index"))?;
            let exact_owner = u32::try_from(*owner_ordinal)
                .map_err(|_| execution_error("owned result ordinal exceeds u32"))?;
            if &admitted.id != parameter
                || admitted.ownership != OwnershipMode::Own
                || parameters.get(*parameter_index).map(|entry| entry.provider)
                    != Some(ProviderParameter::Owned {
                        owner_ordinal: exact_owner,
                    })
            {
                return Err(execution_error(
                    "owned value result does not exactly select an admitted owner",
                ));
            }
            (
                ProviderResult::OwnedInput {
                    owner_ordinal: exact_owner,
                    result_commit_ordinal,
                },
                Some(*parameter_index),
            )
        }
    };
    let result_c_type = resource_abi
        .c_type(program, &function.return_type)?
        .to_owned();
    let resource_bindings = values
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            NativeValueDeclaration::ResourceStorage { binding, .. } => Some(binding.clone()),
            NativeValueDeclaration::Scalar { .. } | NativeValueDeclaration::Status { .. } => None,
        })
        .collect();
    Ok(NativeCallableExecutionPlan {
        parameters,
        result,
        owned_result_parameter_index,
        result_c_type,
        declarations,
        resource_bindings,
        cleanup_body,
        max_event_count: values.required_event_capacity,
        dictionary_entries,
    })
}

impl NativeCallableExecutionPlan {
    fn provider_spec(
        &self,
        callable_symbol: String,
        call_contract: [u8; 32],
    ) -> Result<NativeCallableProviderSpec, Diagnostic> {
        NativeCallableProviderSpec::new(
            callable_symbol,
            call_contract,
            self.parameters.iter().map(|entry| entry.provider).collect(),
            self.result,
            self.max_event_count,
            self.dictionary_entries,
        )
    }

    pub(super) fn normalized_projection(&self) -> Result<(String, [u8; 32]), Diagnostic> {
        let provider = native_callable_provider::emit(
            &self.provider_spec(NORMALIZED_CALLABLE_SYMBOL.to_owned(), NORMALIZED_CONTRACT)?,
        )?;
        let hook = self.emit_hook(&provider.hook_symbol)?;
        let projection = format!("{}{}", provider.source, hook);
        Ok((
            replace_required(
                &projection,
                &provider.target_guards,
                NORMALIZED_TARGET_GUARDS,
                "physical target guards",
            )?,
            provider.codec_profile_fingerprint,
        ))
    }

    pub(super) fn emit_concrete(
        &self,
        descriptor: &NativeCallableDescriptor,
        expected_projection: &str,
    ) -> Result<ConcreteCallableProvider, Diagnostic> {
        let provider = native_callable_provider::emit(
            &self.provider_spec(descriptor.callable_symbol.clone(), descriptor.call_contract)?,
        )?;
        if provider.request_bytes != descriptor.max_request_bytes
            || provider.response_bytes != descriptor.max_response_bytes
        {
            return Err(execution_error(
                "provider capacities disagree with the sealed callable descriptor",
            ));
        }
        let hook = self.emit_hook(&provider.hook_symbol)?;
        let concrete_projection = format!("{}{}", provider.source, hook);
        let normalized = normalize_concrete_projection(
            &concrete_projection,
            &provider,
            descriptor.call_contract,
        )?;
        if normalized != expected_projection {
            return Err(execution_error(
                "concrete wrapper/hook does not instantiate its authenticated projection",
            ));
        }
        Ok(ConcreteCallableProvider {
            source: concrete_projection,
            codec_profile_fingerprint: provider.codec_profile_fingerprint,
            normalized_projection: normalized,
        })
    }

    fn emit_hook(&self, hook_symbol: &str) -> Result<String, Diagnostic> {
        if hook_symbol.is_empty()
            || !hook_symbol
                .bytes()
                .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        {
            return Err(execution_error(
                "generated hook symbol is not a C identifier",
            ));
        }
        let execute_symbol = format!("{hook_symbol}_execute");
        let mut output = String::new();
        output.push_str(
            "SPX_PROVIDER_STATIC_ASSERT(sizeof(uintptr_t) == 8, \"SEMAPRAX callable owned payloads require 64-bit uintptr_t\");\n\
             SPX_PROVIDER_STATIC_ASSERT(UINTPTR_MAX == UINT64_MAX, \"SEMAPRAX callable owned payloads require full uint64_t range\");\n",
        );
        writeln!(
            output,
            "static spx_status_token {execute_symbol}(struct spx_context *spx_bind_context"
        )
        .expect("writing cannot fail");
        for (index, parameter) in self.parameters.iter().enumerate() {
            writeln!(output, "    , {} spx_bind_param_{index}", parameter.c_type)
                .expect("writing cannot fail");
        }
        writeln!(
            output,
            "    , {} *spx_bind_result_out) {{",
            self.result_c_type
        )
        .expect("writing cannot fail");
        for line in self.declarations.lines() {
            writeln!(output, "    {line}").expect("writing cannot fail");
        }
        for binding in &self.resource_bindings {
            writeln!(output, "    (void){binding};").expect("writing cannot fail");
        }
        output.push_str(&self.cleanup_body);
        output.push_str("}\n");

        writeln!(
            output,
            "static uint32_t SPX_PROVIDER_CALL {hook_symbol}(uint64_t spx_invocation"
        )
        .expect("writing cannot fail");
        for (index, parameter) in self.parameters.iter().enumerate() {
            let wire_type = match parameter.provider {
                ProviderParameter::I64 => "int64_t",
                ProviderParameter::Bool => "bool",
                ProviderParameter::Owned { .. } => "uint64_t",
            };
            writeln!(output, "    , {wire_type} spx_arg_{index}").expect("writing cannot fail");
        }
        output.push_str("    , struct spx_provider_execution *spx_execution) {\n");
        output.push_str("    if (spx_execution == NULL) return UINT32_C(1);\n");
        output.push_str("    (void)spx_status_attach_detail; (void)spx_status_resolve_detail; (void)spx_status_record_requires_false; (void)spx_status_record_ensures_false; (void)spx_status_record_arithmetic;\n");
        for (index, parameter) in self.parameters.iter().enumerate() {
            match parameter.provider {
                ProviderParameter::Owned { .. } => {
                    writeln!(
                        output,
                        "    {} spx_bind_param_{index} = {{(uintptr_t)spx_arg_{index}}};",
                        parameter.c_type
                    )
                    .expect("writing cannot fail");
                    writeln!(output, "    if ((uint64_t)spx_bind_param_{index}.payload != spx_arg_{index}) return UINT32_C(1);")
                        .expect("writing cannot fail");
                }
                ProviderParameter::I64 | ProviderParameter::Bool => {
                    writeln!(
                        output,
                        "    {} spx_bind_param_{index} = spx_arg_{index};",
                        parameter.c_type
                    )
                    .expect("writing cannot fail");
                }
            }
        }
        writeln!(
            output,
            "    struct spx_status_entry spx_status_entries[UINT32_C({STATUS_CAPACITY})] = {{0}};"
        )
        .expect("writing cannot fail");
        output.push_str("    struct spx_context spx_context = {0};\n");
        writeln!(output, "    if (!spx_context_init(&spx_context, spx_invocation, spx_status_entries, UINT32_C({STATUS_CAPACITY}), NULL, NULL, NULL)) return UINT32_C(1);")
            .expect("writing cannot fail");
        writeln!(
            output,
            "    struct spx_trace_event spx_events[UINT32_C({})] = {{0}};",
            self.max_event_count
        )
        .expect("writing cannot fail");
        output.push_str("    struct spx_trace_buffer spx_trace = {0};\n");
        writeln!(output, "    if (!spx_trace_buffer_init(&spx_trace, spx_events, UINT32_C({}))) return UINT32_C(1);", self.max_event_count)
            .expect("writing cannot fail");
        writeln!(output, "    if (!spx_trace_attach_preflight(&spx_context, &spx_trace, UINT32_C({}))) return UINT32_C(1);", self.max_event_count)
            .expect("writing cannot fail");
        writeln!(output, "    {} spx_result = {{0}};", self.result_c_type)
            .expect("writing cannot fail");
        write!(
            output,
            "    spx_status_token spx_status = {execute_symbol}(&spx_context"
        )
        .expect("writing cannot fail");
        for index in 0..self.parameters.len() {
            write!(output, ", spx_bind_param_{index}").expect("writing cannot fail");
        }
        output.push_str(", &spx_result);\n");
        output.push_str("    if (spx_trace.length == UINT32_C(0) || spx_trace.length > SPX_PROVIDER_MAX_EVENTS) return UINT32_C(1);\n");
        output.push_str("    uint32_t spx_selected = UINT32_C(0); const struct spx_trace_normalized_status *spx_selected_status = NULL;\n");
        output.push_str("    for (uint32_t i = UINT32_C(0); i < spx_trace.length; ++i) { spx_execution->event_ordinals[i] = spx_events[i].semantic_ordinal; if (spx_events[i].kind == SPX_TRACE_SELECT_FAILURE) { if (spx_selected != UINT32_C(0)) return UINT32_C(1); spx_selected = spx_events[i].semantic_ordinal; spx_selected_status = &spx_events[i].data.select_failure.status; } }\n");
        output.push_str("    spx_execution->event_count = spx_trace.length;\n");
        output.push_str("    if (spx_status == SPX_STATUS_SUCCESS) { if (spx_selected != UINT32_C(0)) return UINT32_C(1); spx_execution->outcome = SPX_OUTCOME_SUCCESS;");
        match self.result {
            ProviderResult::ScalarI64 { .. } => {
                output.push_str(" spx_execution->scalar_result = spx_result;");
            }
            ProviderResult::OwnedInput { owner_ordinal, .. } => {
                let parameter_index = self.owned_result_parameter_index.ok_or_else(|| {
                    execution_error("owned result lacks its selected physical input")
                })?;
                writeln!(
                    output,
                    " if ((uint64_t)spx_result.payload != spx_arg_{parameter_index} || spx_result.payload != spx_bind_param_{parameter_index}.payload) return UINT32_C(1); spx_execution->owned_result_ordinal = UINT32_C({owner_ordinal});"
                )
                .expect("writing cannot fail");
            }
        }
        output.push_str(" } else {\n");
        output.push_str("        const struct spx_normalized_status *spx_returned = spx_status_resolve(&spx_context, spx_status);\n");
        output.push_str("        if (spx_selected == UINT32_C(0) || spx_selected_status == NULL || spx_returned == NULL || strcmp(spx_returned->schema, spx_selected_status->schema) != 0 || strcmp(spx_returned->domain_id, spx_selected_status->domain_id) != 0 || spx_returned->code != spx_selected_status->code || spx_returned->status_class != spx_selected_status->status_class || spx_returned->retryability != spx_selected_status->retryability) return UINT32_C(1);\n");
        output.push_str("        spx_execution->outcome = SPX_OUTCOME_FAILURE; spx_execution->selected_failure_ordinal = spx_selected;\n    }\n    return UINT32_C(0);\n}\n");
        Ok(output)
    }
}

fn unique_result_commit_ordinal(dictionary: &SemanticEventDictionary) -> Result<u32, Diagnostic> {
    let mut matches = dictionary
        .entries()
        .iter()
        .filter(|entry| matches!(entry.event, TraceEventKind::ResultCommit { .. }))
        .map(|entry| entry.ordinal);
    let ordinal = matches
        .next()
        .ok_or_else(|| execution_error("semantic dictionary has no result-commit event"))?;
    if matches.next().is_some() {
        return Err(execution_error(
            "semantic dictionary has more than one result-commit event",
        ));
    }
    Ok(ordinal)
}

fn normalize_concrete_projection(
    concrete: &str,
    provider: &NativeCallableProvider,
    contract: [u8; 32],
) -> Result<String, Diagnostic> {
    let normalized_hook = format!("{NORMALIZED_CALLABLE_SYMBOL}_generated_hook");
    let mut normalized = replace_required(
        concrete,
        &provider.target_guards,
        NORMALIZED_TARGET_GUARDS,
        "physical target guards",
    )?;
    normalized = replace_required(
        &normalized,
        &provider.hook_symbol,
        &normalized_hook,
        "generated hook symbol",
    )?;
    normalized = replace_required(
        &normalized,
        provider
            .hook_symbol
            .strip_suffix("_generated_hook")
            .ok_or_else(|| execution_error("hook symbol lacks canonical suffix"))?,
        NORMALIZED_CALLABLE_SYMBOL,
        "callable symbol",
    )?;
    normalized = replace_required(
        &normalized,
        &contract_declaration(contract),
        &contract_declaration(NORMALIZED_CONTRACT),
        "call-contract declaration",
    )?;
    Ok(normalized)
}

fn replace_required(
    source: &str,
    from: &str,
    to: &str,
    context: &str,
) -> Result<String, Diagnostic> {
    if from == to || !source.contains(from) {
        return Err(execution_error(format!(
            "concrete projection lacks a distinct {context}"
        )));
    }
    Ok(source.replace(from, to))
}

fn contract_declaration(contract: [u8; 32]) -> String {
    let mut output = String::from("    static const uint8_t spx_call_contract[32] = {");
    for byte in contract {
        write!(output, "0x{byte:02x},").expect("writing cannot fail");
    }
    output.push_str("};\n");
    output
}

fn execution_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native callable execution: {}", message.into()),
    )
}
