mod owned_moves;
pub(crate) mod public_generic_bridge;
use super::{
    backend_error, c_i32, c_i64, native_box, native_byte_data, native_bytes, native_command,
    native_command_io, native_host_output, native_iter, native_resource, native_runtime,
    native_source_command, native_vec, resource_lowering_gate, COutput, NATIVE_SCALAR_RUNTIME_C,
};
#[cfg(test)]
use super::{
    native_adapter_abi, native_cleanup, native_cleanup_emit, native_host_contract, native_trace,
    native_value,
};
use crate::aggregate_layout::{AggregateLayoutCache, AggregateTarget};
use crate::ast::BinaryOp;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, DeclarationKind, ExpressionId, FunctionExecutionId, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
    ValueId,
};
use crate::variant_layout::{VariantLayout, VariantLayoutCache, VariantTarget};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
mod closure;
mod compiler;
mod contract_status;
mod mutable;
mod once;
mod read_snapshot;
use contract_status::{contract_label, emit_contract_status};
mod expression;
mod filesystem_io;
mod filesystem_io_v2;
mod filesystem_io_v3;
mod function_value;
mod generic_record;
mod generic_variant;
mod guarded_variant;
mod http_io;
mod indexed_reads;
mod literals;
mod narrow_remainder;
mod native_list;
mod nested_owned;
mod network_io;
mod output_profile;
mod owned_strings;
mod scope_anchors;
mod string_collections;
mod map_collections;
mod string_ops;
mod string_ordering;
mod string_views;
mod symbols;
mod text_toolkit;
#[cfg(test)]
use compiler::write_and_compile_c_with_runner;
pub(super) use compiler::{
    write_and_compile_c, write_and_compile_c_with_curl, write_and_compile_c_with_mode,
    write_compile_and_publish_c, write_compile_and_publish_c_with_curl,
    write_compile_and_publish_c_with_threads,
};
pub use filesystem_io::{emit_c_with_filesystem_io, emit_hir_c_with_filesystem_io};
pub use filesystem_io_v2::{emit_c_with_filesystem_io_v2, emit_hir_c_with_filesystem_io_v2};
pub use filesystem_io_v3::{emit_c_with_filesystem_io_v3, emit_hir_c_with_filesystem_io_v3};
pub use http_io::{emit_c_with_https_io, emit_hir_c_with_https_io};
pub(super) use literals::c_string;
use nested_owned::{borrowed_aggregate_byte_paths, borrowed_aggregate_path_suffix};
pub use network_io::{emit_c_with_network_io, emit_hir_c_with_network_io};
pub(super) use output_profile::NativeOutputProfile;
use output_profile::StringRuntimeSelection;
pub(super) use symbols::{c_case_symbol, c_field_symbol, c_record_symbol, c_variant_symbol};
mod owned_carrier;
use owned_carrier::is_direct_plan_owned;
pub(in crate::codegen) use owned_carrier::is_native_owned_vec_type;
#[path = "../../native_scratch.rs"]
mod native_scratch;
#[cfg(all(test, any(unix, windows)))]
mod scratch_tests;
mod semantic_work;
pub(crate) use semantic_work::{
    NativeSemanticMetering, SEMANTIC_EVENT_CAPACITY, SEMANTIC_FUEL_STATUS_DOMAIN,
};
pub(super) fn emit_hir_c_with_labels(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
) -> Result<String, Diagnostic> {
    emit_hir_c_with_options(
        program,
        contract_labels,
        output_profile,
        selected_command,
        None,
    )
}
/// The Legacy native profile instrumented for Agent Stage Semantic Work v1.
pub(super) fn emit_hir_c_semantically_metered(
    program: &ResolvedProgram,
    metering: &NativeSemanticMetering,
) -> Result<String, Diagnostic> {
    let labels = HashMap::new();
    emit_hir_c_with_options(
        program,
        &labels,
        NativeOutputProfile::Legacy,
        None,
        Some(metering),
    )
}
fn emit_hir_c_with_options(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
    output_profile: NativeOutputProfile,
    selected_command: Option<&DeclarationId>,
    semantic: Option<&NativeSemanticMetering>,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    if !output_profile.is_stdin_stream() && crate::stdin_stream_ops::resolved_program_uses(program)
    {
        return Err(backend_error(
            "stdin reader requires the explicit native streaming-command profile",
        ));
    }
    if program.types.iter().any(|declaration| {
        matches!(
            declaration.kind,
            ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(resource_lowering_gate());
    }
    let resource_abi = native_resource::build_resource_abi(program)?;
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Native64)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Native64)?;
    let functions = function_index(program)?;
    debug_assert!(resource_abi.resources.is_empty());
    let mut output = crate::bounded_output::CappedString::new();
    // Feature-test macros must precede the first system include. The
    // `file_read_text` runtime needs `openat` and `O_NOFOLLOW`, and every
    // command entry opens its file root with `O_DIRECTORY | O_CLOEXEC`;
    // glibc declares these under strict C11 only with POSIX.1-2008 visibility.
    if output_profile.is_command()
        || crate::string_ops::program_uses_op(program, crate::string_ops::StringOp::FileReadText)
    {
        network_io::emit_feature_macros(&mut output);
    }
    if output_profile.is_command() {
        emit_native_prelude_without_public_failure(
            &mut output,
            &resource_abi,
            program,
            output_profile.is_language_command(),
            output_profile.string_runtime(),
        );
    } else {
        emit_native_prelude_profile(
            &mut output,
            &resource_abi,
            program,
            output_profile.string_runtime(),
        );
    }
    if output_profile == NativeOutputProfile::NetworkCommandIo {
        network_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIo {
        filesystem_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIoV2 {
        filesystem_io_v2::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::FilesystemCommandIoV3 {
        filesystem_io_v3::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::HttpsCommandIo {
        http_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::ProcessCommandIo {
        process_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::EnvironmentCommandIo {
        environment_io::emit_runtime(&mut output, program);
    } else if output_profile == NativeOutputProfile::LineCommandIo {
        native_host_output::emit_line_command_runtime(&mut output);
        native_command_io::emit_line_runtime(&mut output);
    } else if output_profile.is_stdin_stream() {
        native_host_output::emit_language_command_runtime(&mut output);
        native_command_io::emit_runtime(&mut output);
        super::native_stdin_stream::emit_runtime(&mut output);
        super::native_stdin_stream::emit_command_helper_table(&mut output);
    } else if output_profile == NativeOutputProfile::LanguageCommandIo {
        native_host_output::emit_language_command_runtime(&mut output);
        native_command_io::emit_runtime(&mut output);
    } else if output_profile == NativeOutputProfile::SourceCommand {
        native_source_command::emit_runtime(&mut output, program);
    } else if output_profile.supports_stdout_transcript() {
        native_host_output::emit_runtime(&mut output);
    }
    if let Some(metering) = semantic {
        semantic_work::emit_runtime(&mut output, metering);
    }
    emit_fixed_byte_array_declarations(&mut output, program)?;
    emit_aggregate_declarations(
        &mut output,
        program,
        &resource_abi,
        &record_layouts,
        &variant_layouts,
    )?;
    if hir::closure::requires_runtime_closures(program) {
        closure::emit_carrier_declarations(&mut output, program, &resource_abi)?;
    }
    function_value::emit_typedefs(&mut output, program, &resource_abi)?;
    emit_function_prototypes(&mut output, program, &functions, &resource_abi)?;
    if closure::enabled(program) {
        closure::emit_thunk_prototypes(&mut output, program, &resource_abi)?;
    }
    let emission = NativeEmissionContext {
        program,
        resource_abi: &resource_abi,
        functions: &functions,
        contract_labels,
        record_layouts: &record_layouts,
        variant_layouts: &variant_layouts,
        output_profile,
        semantic,
    };
    for function in &program.functions {
        emit_function(
            &mut output,
            function,
            &FunctionExecutionId::Monomorphic(function.id.clone()),
            &emission,
        )?;
    }
    for instance in &program.function_instances {
        emit_function(
            &mut output,
            &instance.function,
            &FunctionExecutionId::Generic(instance.id.clone()),
            &emission,
        )?;
    }
    if closure::enabled(program) {
        for function in closure::closure_functions(program)? {
            emit_function(
                &mut output,
                &function,
                &FunctionExecutionId::Monomorphic(function.id.clone()),
                &emission,
            )?;
        }
        closure::emit_thunks(&mut output, program, &emission)?;
    }
    if output_profile.is_command() {
        let command = selected_command
            .ok_or_else(|| backend_error("native command selection is unavailable"))?;
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(command.clone()))
            .ok_or_else(|| backend_error("selected native command is not indexed"))?
            .symbol;
        if output_profile == NativeOutputProfile::NetworkCommandIo {
            network_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIo {
            filesystem_io::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIoV2 {
            filesystem_io_v2::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::FilesystemCommandIoV3 {
            filesystem_io_v3::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::HttpsCommandIo {
            http_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else if output_profile == NativeOutputProfile::ProcessCommandIo {
            process_io::emit_runner(&mut output, symbol);
        } else if output_profile == NativeOutputProfile::EnvironmentCommandIo {
            environment_io::emit_runner(&mut output, symbol);
        } else if output_profile.is_stdin_stream() {
            if matches!(
                output_profile,
                NativeOutputProfile::StdinStreamExitCommandIo
                    | NativeOutputProfile::StdinStreamTextCommandIo
            ) {
                super::native_stdin_stream::exit_status::emit_runner(&mut output, symbol);
                super::native_stdin_stream::exit_status::emit_process_adapter(&mut output);
            } else {
                super::native_stdin_stream::emit_runner(&mut output, symbol);
                super::native_stdin_stream::emit_process_adapter(&mut output);
            }
        } else if output_profile.is_language_command() {
            native_command_io::emit_runner(&mut output, symbol);
            native_command_io::emit_process_adapter(&mut output);
        } else {
            native_command::emit_runner(&mut output, symbol);
            native_command::emit_process_adapter(&mut output);
        }
    } else {
        let main = program
            .functions
            .iter()
            .find(|function| function.id == program.entrypoint)
            .ok_or_else(|| backend_error("resolved native entry point is not indexed"))?;
        if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
            return Err(backend_error(
                "resolved native entry point must have type `fn main() -> i64`",
            ));
        }
        let symbol = &functions
            .get(&FunctionExecutionId::Monomorphic(main.id.clone()))
            .ok_or_else(|| backend_error("native entry point is not indexed"))?
            .symbol;
        if output_profile == NativeOutputProfile::StdoutTranscript {
            native_host_output::emit_root_wrapper(&mut output, symbol);
            return Ok(output.into_string());
        }
        if output_profile == NativeOutputProfile::SourceCommand {
            native_source_command::emit_process_adapter(&mut output, symbol);
            return Ok(output.into_string());
        }
        write!(
            output,
        "#ifndef SPX_NO_ENTRY_WRAPPER\n\
         #if defined(_WIN32)\n\
         #include <fcntl.h>\n\
         #include <io.h>\n\
         #endif\n\
         int main(void) {{\n\
             struct spx_status_entry spx_status_entries[UINT32_C(1)];\n\
             struct spx_context spx_ctx = {{0}};\n\
             if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: context initialization\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             int64_t result;\n\
             spx_status_token status = {symbol}(&spx_ctx, &result);\n\
             if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);\n\
             #if defined(_WIN32)\n\
             if (_setmode(_fileno(stdout), _O_BINARY) == -1) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: stdout binary mode\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             #endif\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n\
         }}\n\
         #endif\n"
        )
        .expect("writing to a string cannot fail");
    }
    Ok(output.into_string())
}
/// Emit one length-indexed, alignment-one C type for each reachable nonempty
/// fixed byte array. `[u8; 0]` is erased from physical C storage altogether;
/// its expressions use a compiler-only scalar sentinel that is never stored,
/// addressed, copied, or exposed through an ABI.
fn emit_fixed_byte_array_declarations(
    output: &mut impl COutput,
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let mut lengths = BTreeSet::new();
    let mut include_type = |ty: &ResolvedType| {
        if let ResolvedType::ArrayU8(length) = ty {
            lengths.insert(*length);
        }
    };
    for declaration in &program.types {
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields, .. }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    include_type(&field.ty);
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    include_type(&field.ty);
                }
            }
            ResolvedTypeDeclarationKind::Resource { .. } => {}
        }
    }
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        include_type(&function.return_type);
        for parameter in &function.params {
            include_type(&parameter.ty);
        }
        let mut pending = vec![&function.body];
        pending.extend(function.requires.iter().chain(&function.ensures));
        while let Some(expression) = pending.pop() {
            include_type(&expression.ty);
            pending.extend(resolved_expr_children(expression));
        }
    }
    let mut emitted = false;
    for length in lengths {
        if u64::from(length) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
            return Err(backend_error(format!(
                "fixed byte array length `{length}` exceeds the authenticated native bound"
            )));
        }
        if length == 0 {
            continue;
        }
        emitted = true;
        writeln!(
            output,
            "struct spx_array_u8_{length} {{ uint8_t spx_bytes[{length}]; }};"
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "_Static_assert(sizeof(struct spx_array_u8_{length}) == UINT32_C({length}), \"SEMAPRAX fixed byte array size\");"
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "_Static_assert(_Alignof(struct spx_array_u8_{length}) == UINT32_C(1), \"SEMAPRAX fixed byte array alignment\");"
        )
        .expect("writing to a string cannot fail");
    }
    if emitted {
        output.push('\n');
    }
    Ok(())
}
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "resource preflight seam is exercised by tests")
)]
pub(super) fn emit_native_prelude(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
) {
    emit_native_prelude_inner(
        output,
        resource_abi,
        program,
        false,
        false,
        StringRuntimeSelection::FROZEN,
    );
}
fn emit_native_prelude_profile(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
    strings: StringRuntimeSelection,
) {
    emit_native_prelude_inner(output, resource_abi, program, false, false, strings);
}
fn emit_native_prelude_without_public_failure(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
    command_carriers: bool,
    strings: StringRuntimeSelection,
) {
    emit_native_prelude_inner(
        output,
        resource_abi,
        program,
        true,
        command_carriers,
        strings,
    );
}
fn emit_native_prelude_inner(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
    omit_public_failure: bool,
    command_carriers: bool,
    strings: StringRuntimeSelection,
) {
    let needs_borrowed_str = command_carriers
        || strings.command_carriers
        || program_uses_borrowed_str(program, strings.include_instances);
    native_runtime::emit_status_runtime_for_profile(
        output,
        needs_borrowed_str || program_uses_byte_data(program) || strings.provider_carriers,
        native_vec::program_uses_vec(program) || native_iter::program_uses_iterator(program),
        native_box::program_uses_box(program),
        native_iter::program_uses_owned_runtime(program),
        crate::list_ops::resolved_program_uses_list(program),
    );
    output.push_str(&resource_abi.declarations);
    output.push_str("#include <stdio.h>\n\n");
    if omit_public_failure {
        let marker = "static __attribute__((unused)) int spx_public_failure(";
        let prefix = NATIVE_SCALAR_RUNTIME_C
            .split_once(marker)
            .map(|(prefix, _)| prefix)
            .expect("native public-failure marker must remain exact");
        output.push_str(prefix);
    } else {
        output.push_str(NATIVE_SCALAR_RUNTIME_C);
    }
    native_list::emit_runtime(output, program);
    if closure::enabled(program) || program_uses_u8_arithmetic(program) {
        output.push_str(NATIVE_U8_RUNTIME_C);
        narrow_remainder::emit_runtime(output, program);
    }
    if closure::enabled(program) || program_uses_usize_arithmetic(program) {
        // Portable usize is semantic u64 on every target. Keep its helpers
        // reachability-gated so programs without usize preserve exact bytes.
        output.push_str(NATIVE_USIZE_RUNTIME_C);
    }
    if program_uses_strings(program, strings.include_instances) {
        output.push_str(if strings.length_delimited {
            NATIVE_LENGTH_DELIMITED_STRING_RUNTIME_C
        } else {
            NATIVE_STRING_RUNTIME_C
        });
    }
    string_ordering::emit_runtime(output, program, strings);
    if program_uses_string_ops(program, strings.include_instances) {
        // String operation helpers stay out of programs that cannot reach
        // them, so existing projections keep their exact committed bytes.
        output.push_str(if strings.length_delimited {
            NATIVE_LENGTH_DELIMITED_STRING_OPS_RUNTIME_C
        } else {
            NATIVE_STRING_OPS_RUNTIME_C
        });
    }
    if program_uses_string_ops_v2(program, strings.include_instances) {
        // Breadth-v2 string operation helpers gate as their own group so
        // first-wave programs keep their exact committed bytes.
        output.push_str(if strings.length_delimited {
            NATIVE_LENGTH_DELIMITED_STRING_OPS_V2_RUNTIME_C
        } else {
            NATIVE_STRING_OPS_V2_RUNTIME_C
        });
    }
    if program_uses_numeric_text(program, strings.include_instances) {
        output.push_str(NATIVE_NUMERIC_TEXT_RUNTIME_C);
    }
    if strings.length_delimited
        && text_toolkit::program_uses_text_toolkit(program, strings.include_instances)
    {
        output.push_str(text_toolkit::RUNTIME_C);
    }
    string_collections::emit_runtime(output, program, strings);
    map_collections::emit_runtime(output, program);
    if needs_borrowed_str {
        // Borrowed text is a distinct length-aware carrier. Keep it behind a
        // reachability gate so every pre-text native projection is byte exact.
        output.push_str(NATIVE_BORROWED_STR_RUNTIME_C);
    }
    if string_views::program_uses_string_as_str(program, strings.include_instances) {
        output.push_str(if strings.length_delimited {
            string_views::LENGTH_DELIMITED_RUNTIME_C
        } else {
            string_views::TERMINATED_RUNTIME_C
        });
    }
    if program_uses_byte_data(program) || strings.provider_carriers || strings.command_carriers {
        if strings.stream_epochs {
            native_byte_data::emit_stream_epoch_runtime(output);
        } else if strings.reserved_bytes {
            native_byte_data::emit_reserved_runtime(output);
        } else {
            native_byte_data::emit_runtime(output);
        }
        if program_uses_additive_byte_operations(program) {
            if strings.stream_epochs {
                native_byte_data::emit_stream_epoch_additive_operations(output);
            } else {
                native_byte_data::emit_additive_operations(output);
            }
        }
    }
    if native_vec::program_uses_vec(program) || native_iter::program_uses_iterator(program) {
        native_vec::emit_runtime(output, program);
    }
    if native_iter::program_uses_iterator(program) {
        native_iter::emit_runtime(output, program);
    }
    if native_box::program_uses_box(program) {
        native_box::emit_runtime(output, program);
    }
}
fn program_uses_byte_data(program: &ResolvedProgram) -> bool {
    let affine = hir::closure::once::requires_bytes(program);
    if affine || crate::iterator_ops::resolved_program_uses_owned_iterator(program) {
        return true;
    }
    // Owned-record elements require byte runtime even without a named Bytes slot.
    if crate::hir::owned_record_collection::program_uses_profile(program) {
        return true;
    }
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(
            function.return_type,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) || function.params.iter().any(|param| {
            matches!(
                param.ty,
                ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
            )
        }) {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.ty,
            ResolvedType::SliceU8 | ResolvedType::Bytes | ResolvedType::ArrayU8(_)
        ) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::byte_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    crate::box_ops::resolved_program_uses_owned_payload(program)
        || crate::vec_ops::resolved_program_uses_owned_payload(program)
}

/// Store helpers added after the frozen byte runtime are emitted only for a
/// resolved call that can reach them. This keeps legacy native projections and
/// generated package pins byte exact while retaining the helper definitions
/// needed by the newer owned-buffer lowering paths.
fn program_uses_additive_byte_operations(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(
            &expression.kind,
            ResolvedExprKind::Call { callee, .. }
                if matches!(
                    crate::byte_ops::by_id(callee.as_str()),
                    Some(
                        crate::byte_ops::ByteOp::Set5
                            | crate::byte_ops::ByteOp::Set1Or5
                            | crate::byte_ops::ByteOp::Set1Or6Or48
                    )
                )
        ) {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}
/// Whether any resolved signature, body, or contract admits an owned string
/// value that lowers through the string runtime helpers.
fn program_uses_strings(program: &ResolvedProgram, include_instances: bool) -> bool {
    program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Variant { cases }
                if cases.iter().flat_map(|case| &case.fields)
                    .any(|field| matches!(field.ty, ResolvedType::String))
        ) || matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Record { fields }
                if fields.iter().any(|field| string_collections::owns_text(&field.ty))
        )
    }) || string_runtime_functions(program, include_instances).any(function_uses_strings)
}

fn string_runtime_functions(
    program: &ResolvedProgram,
    include_instances: bool,
) -> impl Iterator<Item = &ResolvedFunction> {
    program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .filter(move |_| include_instances)
            .map(|instance| &instance.function),
    )
}

fn function_uses_strings(function: &ResolvedFunction) -> bool {
    if string_collections::owns_text(&function.return_type)
        || function
            .params
            .iter()
            .any(|param| string_collections::owns_text(&param.ty))
    {
        return true;
    }
    let mut pending = vec![&function.body];
    pending.extend(function.requires.iter().chain(&function.ensures));
    while let Some(expression) = pending.pop() {
        if string_collections::owns_text(&expression.ty)
            || matches!(expression.kind, ResolvedExprKind::String(_))
        {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// string operation intrinsic.
fn program_uses_string_ops(program: &ResolvedProgram, include_instances: bool) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some_and(|op| op.touches_string()) {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a breadth-v2
/// compiler-owned string operation intrinsic.
fn program_uses_string_ops_v2(program: &ResolvedProgram, include_instances: bool) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_breadth_v2)
            {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether a resolved body or contract renders a copied integer as decimal text.
fn program_uses_numeric_text(program: &ResolvedProgram, include_instances: bool) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_numeric_text)
            {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// borrowed-text operation intrinsic.
fn program_uses_borrowed_str(program: &ResolvedProgram, include_instances: bool) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in string_runtime_functions(program, include_instances) {
        if matches!(function.return_type, ResolvedType::Str)
            || function
                .params
                .iter()
                .any(|param| matches!(param.ty, ResolvedType::Str))
        {
            return true;
        }
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.ty, ResolvedType::Str) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::str_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract contains checked u8
/// arithmetic that lowers through the u8 runtime helpers.
fn program_uses_u8_arithmetic(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Binary { op, left, right } = &expression.kind {
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return true;
            }
            pending.push(left);
            pending.push(right);
            continue;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

fn program_uses_usize_arithmetic(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter());
        pending.extend(function.ensures.iter());
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Binary { op, left, right } = &expression.kind {
            if matches!(left.ty, ResolvedType::Usize)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return true;
            }
            pending.push(left);
            pending.push(right);
            continue;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Every direct resolved child of an expression.
use function_value::resolved_expr_children;
fn emit_aggregate_declarations(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    record_layouts: &AggregateLayoutCache,
    variant_layouts: &VariantLayoutCache,
) -> Result<(), Diagnostic> {
    let records = record_layouts.layouts().collect::<Vec<_>>();
    let variants = variant_layouts.layouts().collect::<Vec<_>>();
    if records.is_empty() && variants.is_empty() {
        return Ok(());
    }

    output.push_str("#include <stddef.h>\n\n");
    for record in &records {
        writeln!(output, "struct {};", c_record_symbol(&record.instance))
            .expect("writing to a string cannot fail");
    }
    for variant in &variants {
        writeln!(output, "struct {};", c_variant_symbol(&variant.instance))
            .expect("writing to a string cannot fail");
    }
    output.push('\n');

    let mut visiting = BTreeSet::new();
    let mut emitted = BTreeSet::new();
    for record in &records {
        emit_record_declaration(
            output,
            program,
            resource_abi,
            record_layouts,
            &record.instance,
            &mut visiting,
            &mut emitted,
        )?;
    }
    for variant in &variants {
        emit_variant_declaration(output, program, resource_abi, variant)?;
    }
    Ok(())
}

fn emit_record_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layouts: &AggregateLayoutCache,
    instance: &ResolvedType,
    visiting: &mut BTreeSet<ResolvedType>,
    emitted: &mut BTreeSet<ResolvedType>,
) -> Result<(), Diagnostic> {
    if emitted.contains(instance) {
        return Ok(());
    }
    if !visiting.insert(instance.clone()) {
        return Err(backend_error(format!(
            "native aggregate instance `{}` is recursively embedded",
            instance.identity_key()
        )));
    }
    let layout = layouts.layout(instance)?.clone();
    layout.validate(program)?;
    for field in &layout.fields {
        if record_declaration_id(program, &field.ty)?.is_some() {
            emit_record_declaration(
                output,
                program,
                resource_abi,
                layouts,
                &field.ty,
                visiting,
                emitted,
            )?;
        }
    }

    let symbol = c_record_symbol(instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    if layout.fields.is_empty() {
        // The canonical empty product has a frozen one-byte representation
        // on every target. Keep that semantic byte distinct from the
        // physical carrier used by nonempty, semantically zero-sized records.
        output.push_str("    uint8_t spx_empty_record_padding;\n");
    } else if layout.size == 0 {
        // ISO C11 has no empty objects or empty structs. This byte is an ABI
        // carrier only: the authenticated SEMAPRAX layout remains size zero,
        // and every zero-sized semantic field stays erased below.
        output.push_str("    uint8_t spx_zero_sized_record_carrier;\n");
    } else {
        for field in layout.fields.iter().filter(|field| field.size != 0) {
            writeln!(
                output,
                "    {} {};",
                c_value_type(program, resource_abi, &field.ty)?,
                c_field_symbol(&field.field)
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push_str("};\n");
    if layout.size == 0 {
        writeln!(
            output,
            "_Static_assert(sizeof(struct {symbol}) == UINT32_C(1), \"SEMAPRAX zero-sized native aggregate carrier size\");"
        )
        .expect("writing to a string cannot fail");
    } else {
        writeln!(
            output,
            "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate size\");",
            layout.size
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    for field in layout.fields.iter().filter(|field| field.size != 0) {
        writeln!(
            output,
            "_Static_assert(offsetof(struct {symbol}, {}) == UINT32_C({}), \"SEMAPRAX native aggregate field offset\");",
            c_field_symbol(&field.field),
            field.offset
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    visiting.remove(instance);
    emitted.insert(instance.clone());
    Ok(())
}

fn emit_variant_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layout: &VariantLayout,
) -> Result<(), Diagnostic> {
    layout.validate(program)?;
    let symbol = c_variant_symbol(&layout.instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    output.push_str("    uint32_t spx_tag;\n    union {\n");
    for case in &layout.cases {
        output.push_str("        struct {\n");
        if case.fields.is_empty() {
            output.push_str("            uint8_t spx_empty_variant_payload;\n");
        } else {
            for field in case.fields.iter().filter(|field| field.size != 0) {
                writeln!(
                    output,
                    "            {} {};",
                    c_value_type(program, resource_abi, &field.ty)?,
                    c_field_symbol(&field.field)
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(output, "        }} {};", c_case_symbol(&case.case))
            .expect("writing to a string cannot fail");
    }
    output.push_str("    } spx_payload;\n};\n");
    writeln!(
        output,
        "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant size\");",
        layout.size
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_tag) == UINT32_C(0), \"SEMAPRAX native variant tag offset\");"
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_payload) == UINT32_C({}), \"SEMAPRAX native variant payload offset\");",
        layout.payload_offset
    )
    .expect("writing to a string cannot fail");
    for case in &layout.cases {
        let case_symbol = c_case_symbol(&case.case);
        for field in case.fields.iter().filter(|field| field.size != 0) {
            let absolute_offset = layout
                .payload_offset
                .checked_add(field.offset)
                .ok_or_else(|| backend_error("native variant field offset overflows u32"))?;
            writeln!(
                output,
                "_Static_assert(offsetof(struct {symbol}, spx_payload.{case_symbol}.{}) == UINT32_C({}), \"SEMAPRAX native variant field offset\");",
                c_field_symbol(&field.field),
                absolute_offset
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push('\n');
    Ok(())
}

use owned_carrier::{c_value_type, record_declaration_id, variant_declaration_id};

fn is_aggregate_type(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    Ok(matches!(ty, ResolvedType::ArrayU8(length) if *length != 0)
        || crate::iterator_ops::is_step(ty)
        || record_declaration_id(program, ty)?.is_some()
        || variant_declaration_id(program, ty)?.is_some())
}

pub(super) fn emit_function_prototypes(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Native64)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Native64)?;
    for (function, execution) in program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                &instance.function,
                FunctionExecutionId::Generic(instance.id.clone()),
            )
        }))
    {
        let bytes_plan = native_bytes::NativeBytesPlan::build(function)?;
        let metadata = functions
            .get(&execution)
            .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
        write!(
            output,
            "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
            metadata.symbol,
        )
        .expect("writing to a string cannot fail");
        for param in &function.params {
            let ty = c_value_type(program, resource_abi, &param.ty)?;
            if is_direct_plan_owned(program, &param.ty)
                && param.ownership == crate::hir::OwnershipMode::Borrow
            {
                write!(output, ", const {ty} *").expect("writing to a string cannot fail");
            } else if is_aggregate_type(program, &param.ty)? {
                let storage = crate::cleanup_plan::StorageId::Value(param.id.clone());
                let qualifier = if bytes_plan
                    .as_ref()
                    .is_some_and(|plan| plan.has_projected_leaves(&storage))
                {
                    ""
                } else {
                    "const "
                };
                write!(output, ", {qualifier}{ty} *").expect("writing to a string cannot fail");
            } else {
                write!(output, ", {ty}").expect("writing to a string cannot fail");
            }
            if param.ownership == crate::hir::OwnershipMode::Borrow {
                for path in borrowed_aggregate_byte_paths(
                    program,
                    &record_layouts,
                    &variant_layouts,
                    &param.ty,
                )? {
                    let pointer = nested_owned::borrowed_leaf_pointer_type(
                        program,
                        &record_layouts,
                        &variant_layouts,
                        &param.ty,
                        &path,
                    )?;
                    write!(output, ", {pointer}").expect("writing to a string cannot fail");
                }
            }
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            c_value_type(program, resource_abi, &function.return_type)?
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

#[cfg(test)]
pub(super) fn preflight_resource_lowering(
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let mut first_failure = None;
    for function in &program.functions {
        match native_cleanup::classify(program, function) {
            Ok(cleanup) => {
                match native_value::plan(program, function, &cleanup, resource_abi, contract_labels)
                {
                    Ok(mut values) => {
                        match crate::semantic_trace::build_semantic_event_dictionary(
                            program,
                            &function.id,
                        ) {
                            Ok(dictionary) => {
                                values.cleanup_bindings.semantic_events = Some(dictionary);
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                                continue;
                            }
                        }
                        match native_host_contract::derive_from_admitted(
                            program,
                            &function.id,
                            resource_abi,
                            &cleanup,
                            &values,
                        ) {
                            Ok(host_template) => {
                                match native_adapter_abi::derive(&host_template) {
                                    Ok(descriptor) => {
                                        let _descriptor_header =
                                            native_adapter_abi::emit_header(&descriptor);
                                        if let Err(diagnostic) = native_adapter_abi::emit_source(
                                            &descriptor,
                                            "semaprax_adapter_descriptor.h",
                                        ) {
                                            first_failure.get_or_insert(diagnostic);
                                        }
                                    }
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                                let _declarations = native_value::emit_declarations(&values);
                                match native_cleanup_emit::emit_with_block_prologues(
                                    &cleanup,
                                    &values.cleanup_bindings,
                                    |block, output| {
                                        output.push_str(&native_value::emit_block_prologue(
                                            &values, block,
                                        ));
                                        Ok(())
                                    },
                                ) {
                                    Ok(_cleanup_body) => {}
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                            }
                        }
                    }
                    Err(diagnostic) => {
                        first_failure.get_or_insert(diagnostic);
                    }
                }
            }
            Err(diagnostic) => {
                first_failure.get_or_insert(diagnostic);
            }
        }
        if let Err(diagnostic) = native_trace::required_event_capacity(program, function) {
            first_failure.get_or_insert(diagnostic);
        }
    }

    // Exercise the same ABI declaration/type/prototype order that the eventual
    // resource emitter will use, then discard it. The loop above separately
    // constructs the gated value/cleanup bodies; the exact conformance harness
    // composes those inside strict C functions. No resource artifact may escape
    // until a public host ownership boundary is defined and proven.
    let mut staged_output = crate::bounded_output::CappedString::new();
    emit_native_prelude(&mut staged_output, resource_abi, program);
    if let Err(diagnostic) =
        function_value::emit_typedefs(&mut staged_output, program, resource_abi)
    {
        first_failure.get_or_insert(diagnostic);
    }
    if let Err(diagnostic) =
        emit_function_prototypes(&mut staged_output, program, functions, resource_abi)
    {
        first_failure.get_or_insert(diagnostic);
    }
    first_failure.map_or(Ok(()), Err)
}

const NATIVE_STRING_RUNTIME_C: &str = r#"#include <stdlib.h>
#include <string.h>

static __attribute__((unused)) char *spx_string_from_literal(
    const char *spx_data, uint64_t spx_len
) {
    char *spx_copy = (char *)malloc((size_t)spx_len + 1u);
    if (spx_copy == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_copy, spx_data, (size_t)spx_len);
    spx_copy[spx_len] = '\0';
    return spx_copy;
}

static __attribute__((unused)) char *spx_string_clone(const char *spx_source) {
    return spx_string_from_literal(spx_source, (uint64_t)strlen(spx_source));
}

static __attribute__((unused)) bool spx_string_eq(const char *a, const char *b) {
    return strcmp(a, b) == 0;
}

static __attribute__((unused)) void spx_string_drop(char *spx_value) {
    free(spx_value);
}
"#;

// V10 and corrected ordinary/stdout/owned-data functions reuse this representation.
// The complete translation unit uses one allocator/header/drop convention;
// Frozen command profiles and the private callable prelude retain the terminated runtime.
// The trailing terminator is not a semantic length or equality boundary.
const NATIVE_LENGTH_DELIMITED_STRING_RUNTIME_C: &str = r#"#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

struct spx_string_v10 { uint64_t len; char data[]; };
static __attribute__((unused)) struct spx_string_v10 *spx_string_header_v10(const char *value) {
    return (struct spx_string_v10 *)((uint8_t *)value - offsetof(struct spx_string_v10, data));
}
static __attribute__((unused)) uint64_t spx_string_length_v10(const char *value) {
    return spx_string_header_v10(value)->len;
}
static __attribute__((unused)) char *spx_string_from_literal(
    const char *spx_data, uint64_t spx_len
) {
    if (spx_len > (uint64_t)SIZE_MAX - (uint64_t)offsetof(struct spx_string_v10, data) - UINT64_C(1))
        spx_runtime_invariant_failure("string allocation length overflow");
    struct spx_string_v10 *spx_value = (struct spx_string_v10 *)malloc(
        offsetof(struct spx_string_v10, data) + (size_t)spx_len + 1u
    );
    if (spx_value == NULL) spx_runtime_invariant_failure("string allocation failed");
    spx_value->len = spx_len;
    if (spx_len != UINT64_C(0)) memcpy(spx_value->data, spx_data, (size_t)spx_len);
    spx_value->data[spx_len] = '\0';
    return spx_value->data;
}
static __attribute__((unused)) char *spx_string_clone(const char *spx_source) {
    return spx_string_from_literal(spx_source, spx_string_length_v10(spx_source));
}
static __attribute__((unused)) bool spx_string_eq(const char *a, const char *b) {
    uint64_t a_len = spx_string_length_v10(a), b_len = spx_string_length_v10(b);
    return a_len == b_len && (a_len == UINT64_C(0) || memcmp(a, b, (size_t)a_len) == 0);
}
static __attribute__((unused)) void spx_string_drop(char *spx_value) {
    free(spx_string_header_v10(spx_value));
}
"#;

const NATIVE_STRING_OPS_RUNTIME_C: &str = r#"static __attribute__((unused)) int64_t spx_string_len(const char *spx_value) {
    return (int64_t)strlen(spx_value);
}

static __attribute__((unused)) bool spx_string_is_empty(const char *spx_value) {
    return spx_value[0] == '\0';
}

static __attribute__((unused)) char *spx_string_concat(
    char *spx_left, char *spx_right
) {
    uint64_t spx_left_len = (uint64_t)strlen(spx_left);
    uint64_t spx_right_len = (uint64_t)strlen(spx_right);
    char *spx_joined = (char *)malloc((size_t)(spx_left_len + spx_right_len) + 1u);
    if (spx_joined == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_joined, spx_left, (size_t)spx_left_len);
    memcpy(spx_joined + spx_left_len, spx_right, (size_t)spx_right_len);
    spx_joined[spx_left_len + spx_right_len] = '\0';
    return spx_joined;
}
"#;

const NATIVE_LENGTH_DELIMITED_STRING_OPS_RUNTIME_C: &str = r#"static __attribute__((unused)) int64_t spx_string_len(const char *spx_value) {
    uint64_t length = spx_string_length_v10(spx_value);
    if (length > (uint64_t)INT64_MAX) spx_runtime_invariant_failure("string length overflow");
    return (int64_t)length;
}
static __attribute__((unused)) bool spx_string_is_empty(const char *spx_value) {
    return spx_string_length_v10(spx_value) == UINT64_C(0);
}
static __attribute__((unused)) char *spx_string_concat(char *left, char *right) {
    uint64_t left_len = spx_string_length_v10(left), right_len = spx_string_length_v10(right);
    if (right_len > UINT64_MAX - left_len) spx_runtime_invariant_failure("string length overflow");
    uint64_t joined_len = left_len + right_len;
    char *joined;
    if (joined_len == UINT64_C(0)) joined = spx_string_from_literal("", UINT64_C(0));
    else {
        if (joined_len > (uint64_t)SIZE_MAX) spx_runtime_invariant_failure("string length overflow");
        char *temporary = (char *)malloc((size_t)joined_len);
        if (temporary == NULL) spx_runtime_invariant_failure("string allocation failed");
        if (left_len != UINT64_C(0)) memcpy(temporary, left, (size_t)left_len);
        if (right_len != UINT64_C(0)) memcpy(temporary + left_len, right, (size_t)right_len);
        joined = spx_string_from_literal(temporary, joined_len);
        free(temporary);
    }
    return joined;
}
"#;

const NATIVE_STRING_OPS_V2_RUNTIME_C: &str = r#"static __attribute__((unused)) bool spx_string_starts_with(
    const char *spx_value, const char *spx_prefix
) {
    return strncmp(spx_value, spx_prefix, strlen(spx_prefix)) == 0;
}

static __attribute__((unused)) bool spx_string_contains(
    const char *spx_value, const char *spx_needle
) {
    return strstr(spx_value, spx_needle) != NULL;
}

static __attribute__((unused)) int64_t spx_string_len_chars(const char *spx_value) {
    int64_t spx_count = 0;
    for (const unsigned char *spx_cursor = (const unsigned char *)spx_value;
         *spx_cursor != UINT8_C(0);
         ++spx_cursor) {
        if ((*spx_cursor & UINT8_C(0xC0)) != UINT8_C(0x80)) ++spx_count;
    }
    return spx_count;
}

static __attribute__((unused)) char *spx_string_from_char(uint32_t spx_scalar) {
    char spx_encoded[4];
    uint64_t spx_length;
    if (spx_scalar < UINT32_C(0x80)) {
        spx_encoded[0] = (char)(uint8_t)spx_scalar;
        spx_length = UINT64_C(1);
    } else if (spx_scalar < UINT32_C(0x800)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xC0) | (uint8_t)(spx_scalar >> 6));
        spx_encoded[1] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(2);
    } else if (spx_scalar < UINT32_C(0x10000)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xE0) | (uint8_t)(spx_scalar >> 12));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[2] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(3);
    } else {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xF0) | (uint8_t)(spx_scalar >> 18));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 12) & UINT32_C(0x3F)));
        spx_encoded[2] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[3] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(4);
    }
    return spx_string_from_literal(spx_encoded, spx_length);
}
"#;

const NATIVE_LENGTH_DELIMITED_STRING_OPS_V2_RUNTIME_C: &str = r#"static __attribute__((unused)) bool spx_string_starts_with(const char *value, const char *prefix) {
    uint64_t value_len = spx_string_length_v10(value), prefix_len = spx_string_length_v10(prefix);
    return prefix_len <= value_len && (prefix_len == UINT64_C(0) || memcmp(value, prefix, (size_t)prefix_len) == 0);
}
static __attribute__((unused)) bool spx_string_contains(const char *value, const char *needle) {
    uint64_t value_len = spx_string_length_v10(value), needle_len = spx_string_length_v10(needle);
    if (needle_len == UINT64_C(0)) return true;
    if (needle_len > value_len) return false;
    for (uint64_t offset = UINT64_C(0); offset <= value_len - needle_len; ++offset)
        if (memcmp(value + offset, needle, (size_t)needle_len) == 0) return true;
    return false;
}
static __attribute__((unused)) int64_t spx_string_len_chars(const char *value) {
    uint64_t length = spx_string_length_v10(value), count = UINT64_C(0);
    for (uint64_t offset = UINT64_C(0); offset < length; ++offset)
        if ((((const uint8_t *)value)[offset] & UINT8_C(0xc0)) != UINT8_C(0x80)) ++count;
    if (count > (uint64_t)INT64_MAX) spx_runtime_invariant_failure("string character length overflow");
    return (int64_t)count;
}
static __attribute__((unused)) char *spx_string_from_char(uint32_t scalar) {
    char encoded[4]; uint64_t length;
    if (scalar < UINT32_C(0x80)) { encoded[0] = (char)(uint8_t)scalar; length = UINT64_C(1); }
    else if (scalar < UINT32_C(0x800)) { encoded[0]=(char)(uint8_t)(UINT8_C(0xc0)|(uint8_t)(scalar>>6)); encoded[1]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)(scalar&UINT32_C(0x3f))); length=UINT64_C(2); }
    else if (scalar < UINT32_C(0x10000)) { encoded[0]=(char)(uint8_t)(UINT8_C(0xe0)|(uint8_t)(scalar>>12)); encoded[1]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)((scalar>>6)&UINT32_C(0x3f))); encoded[2]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)(scalar&UINT32_C(0x3f))); length=UINT64_C(3); }
    else { encoded[0]=(char)(uint8_t)(UINT8_C(0xf0)|(uint8_t)(scalar>>18)); encoded[1]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)((scalar>>12)&UINT32_C(0x3f))); encoded[2]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)((scalar>>6)&UINT32_C(0x3f))); encoded[3]=(char)(uint8_t)(UINT8_C(0x80)|(uint8_t)(scalar&UINT32_C(0x3f))); length=UINT64_C(4); }
    return spx_string_from_literal(encoded, length);
}
"#;

const NATIVE_NUMERIC_TEXT_RUNTIME_C: &str = r#"static __attribute__((unused)) char *spx_string_from_i64(int64_t value) {
    char text[21];
    int written = snprintf(text, sizeof text, "%lld", (long long)value);
    if (written < 1 || written >= (int)sizeof text) spx_runtime_invariant_failure("i64 text rendering failed");
    return spx_string_from_literal(text, (uint64_t)written);
}
static __attribute__((unused)) char *spx_string_from_usize(uint64_t value) {
    char text[21];
    int written = snprintf(text, sizeof text, "%llu", (unsigned long long)value);
    if (written < 1 || written >= (int)sizeof text) spx_runtime_invariant_failure("usize text rendering failed");
    return spx_string_from_literal(text, (uint64_t)written);
}
"#;

const NATIVE_BORROWED_STR_RUNTIME_C: &str = r#"#include <limits.h>
#include <stddef.h>
#include <string.h>

typedef struct {
    const uint8_t *data;
    uint64_t len;
} spx_str_v1;

#define SPX_BORROWED_STR_MAX_BYTES UINT64_C(65536)

static __attribute__((unused)) void spx_str_require_valid(spx_str_v1 value) {
    if (value.len != UINT64_C(0) && value.data == NULL) {
        spx_runtime_invariant_failure("borrowed str has null data with nonzero length");
    }
    if (value.len > SPX_BORROWED_STR_MAX_BYTES
        || value.len > (uint64_t)SIZE_MAX
        || value.len > (uint64_t)INT64_MAX) {
        spx_runtime_invariant_failure("borrowed str length exceeds native profile");
    }
    uint64_t offset = UINT64_C(0);
    while (offset < value.len) {
        const uint8_t first = value.data[offset];
        uint64_t width = UINT64_C(0);
        if (first <= UINT8_C(0x7f)) {
            width = UINT64_C(1);
        } else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            width = UINT64_C(2);
        } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            width = UINT64_C(3);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            width = UINT64_C(4);
        } else {
            spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
        }
        if (width > value.len - offset) {
            spx_runtime_invariant_failure("borrowed str has truncated UTF-8");
        }
        if (width >= UINT64_C(2)) {
            const uint8_t second = value.data[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80)
                || (first == UINT8_C(0xe0) && second < UINT8_C(0xa0))
                || (first == UINT8_C(0xed) && second > UINT8_C(0x9f))
                || (first == UINT8_C(0xf0) && second < UINT8_C(0x90))
                || (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail) {
            if ((value.data[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) {
                spx_runtime_invariant_failure("borrowed str is not canonical UTF-8");
            }
        }
        offset += width;
    }
}

static __attribute__((unused)) int64_t spx_str_len_bytes(spx_str_v1 value) {
    spx_str_require_valid(value);
    return (int64_t)value.len;
}

static __attribute__((unused)) bool spx_str_is_empty(spx_str_v1 value) {
    spx_str_require_valid(value);
    return value.len == UINT64_C(0);
}

static __attribute__((unused)) bool spx_str_starts_with(
    spx_str_v1 value, spx_str_v1 prefix
) {
    spx_str_require_valid(value);
    spx_str_require_valid(prefix);
    if (prefix.len == UINT64_C(0)) return true;
    if (prefix.len > value.len) return false;
    return memcmp(value.data, prefix.data, (size_t)prefix.len) == 0;
}

static __attribute__((unused)) bool spx_str_contains(
    spx_str_v1 value, spx_str_v1 needle
) {
    spx_str_require_valid(value);
    spx_str_require_valid(needle);
    if (needle.len == UINT64_C(0)) return true;
    if (needle.len > value.len) return false;

    /* Fixed-capacity KMP: every byte advances or shortens the matched prefix. */
    uint16_t prefix[SPX_BORROWED_STR_MAX_BYTES];
    prefix[0] = UINT16_C(0);
    uint64_t matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(1); index < needle.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != needle.data[index]) {
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        }
        if (needle.data[matched] == needle.data[index]) ++matched;
        prefix[index] = (uint16_t)matched;
    }

    matched = UINT64_C(0);
    for (uint64_t index = UINT64_C(0); index < value.len; ++index) {
        while (matched != UINT64_C(0) && needle.data[matched] != value.data[index]) {
            matched = (uint64_t)prefix[matched - UINT64_C(1)];
        }
        if (needle.data[matched] == value.data[index]) {
            ++matched;
            if (matched == needle.len) return true;
        }
    }
    return false;
}
"#;

const NATIVE_U8_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_u8_add(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a + (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_sub(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a - (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_mul(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a * (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_div(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    *result_out = (uint8_t)((int64_t)a / (int64_t)b);
    return SPX_STATUS_SUCCESS;
}
"#;

const NATIVE_USIZE_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_usize_add(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (a > UINT64_MAX - b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = a + b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_sub(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (a < b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = a - b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_mul(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b != UINT64_C(0) && a > UINT64_MAX / b) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = a * b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_div(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b == UINT64_C(0)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_usize_rem(
    struct spx_context *spx_ctx, uint64_t a, uint64_t b, uint64_t *result_out
) {
    if (b == UINT64_C(0)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}
"#;

pub(super) struct NativeEmissionContext<'a> {
    pub(super) program: &'a ResolvedProgram,
    pub(super) resource_abi: &'a native_resource::NativeResourceAbi,
    pub(super) functions: &'a HashMap<FunctionExecutionId, CFunction>,
    contract_labels: &'a HashMap<ExpressionId, String>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
    output_profile: NativeOutputProfile,
    semantic: Option<&'a NativeSemanticMetering>,
}

fn emit_function(
    output: &mut impl COutput,
    function: &ResolvedFunction,
    execution: &FunctionExecutionId,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    let program = emission.program;
    let resource_abi = emission.resource_abi;
    let functions = emission.functions;
    let contract_labels = emission.contract_labels;
    let owned = |ty: &ResolvedType| is_direct_plan_owned(program, ty);
    let has_try = expression_has_try(&function.body);
    let semantic_ordinal = emission.semantic.and_then(|meter| meter.ordinal(execution));
    let bytes_plan = native_bytes::NativeBytesPlan::build(function)?
        .map(|plan| plan.with_semantic_function(semantic_ordinal));
    let metadata = functions
        .get(execution)
        .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
    write!(
        output,
        "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
        metadata.symbol
    )
    .expect("writing to a string cannot fail");
    for (index, param) in function.params.iter().enumerate() {
        let ty = c_value_type(program, resource_abi, &param.ty)?;
        if owned(&param.ty) && param.ownership == crate::hir::OwnershipMode::Borrow {
            write!(output, ", const {ty} *spx_param_{index}")
                .expect("writing to a string cannot fail");
        } else if is_aggregate_type(program, &param.ty)? {
            let storage = crate::cleanup_plan::StorageId::Value(param.id.clone());
            let qualifier = if bytes_plan
                .as_ref()
                .is_some_and(|plan| plan.has_projected_leaves(&storage))
            {
                ""
            } else {
                "const "
            };
            write!(output, ", {qualifier}{ty} *spx_param_{index}")
                .expect("writing to a string cannot fail");
        } else {
            write!(output, ", {ty} spx_param_{index}").expect("writing to a string cannot fail");
        }
        if param.ownership == crate::hir::OwnershipMode::Borrow {
            for path in borrowed_aggregate_byte_paths(
                program,
                emission.record_layouts,
                emission.variant_layouts,
                &param.ty,
            )? {
                let suffix = borrowed_aggregate_path_suffix(&path)?;
                let pointer = nested_owned::borrowed_leaf_pointer_type(
                    program,
                    emission.record_layouts,
                    emission.variant_layouts,
                    &param.ty,
                    &path,
                )?;
                write!(output, ", {pointer}spx_param_{index}_borrow_{suffix}")
                    .expect("writing to a string cannot fail");
            }
        }
    }
    writeln!(
        output,
        ", {} *spx_result_out) {{",
        c_value_type(program, resource_abi, &function.return_type)?
    )
    .expect("writing to a string cannot fail");

    if let Some(plan) = &bytes_plan {
        output.push_str(&plan.declarations(
            function,
            emission.output_profile == NativeOutputProfile::OwnedDataProvider,
        ));
        for (index, parameter) in function.params.iter().enumerate() {
            if owned(&parameter.ty) && parameter.ownership == crate::hir::OwnershipMode::Own {
                output.push_str(&plan.initialize_parameter(
                    &crate::cleanup_plan::StorageId::Value(parameter.id.clone()),
                    &format!("spx_param_{index}"),
                )?);
            } else if parameter.ownership == crate::hir::OwnershipMode::Own
                && is_aggregate_type(program, &parameter.ty)?
            {
                let storage = crate::cleanup_plan::StorageId::Value(parameter.id.clone());
                if plan.has_projected_leaves(&storage) {
                    if plan.has_variant_leaves(&storage) {
                        let layout = emission.variant_layouts.layout(&parameter.ty)?;
                        output.push_str(&plan.initialize_variant_parameter(
                            &storage,
                            &format!("spx_param_{index}"),
                            layout,
                        )?);
                    } else {
                        output.push_str(&plan.initialize_record_parameter(
                            &storage,
                            &format!("spx_param_{index}"),
                        )?);
                    }
                }
            }
        }
    }
    let mut variables = HashMap::new();
    let mut borrowed_aggregate_bytes = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let name = if owned(&param.ty) {
            match param.ownership {
                crate::hir::OwnershipMode::Own => bytes_plan
                    .as_ref()
                    .ok_or_else(|| backend_error("owned Bytes parameter has no cleanup plan"))?
                    .value(&crate::cleanup_plan::StorageId::Value(param.id.clone()))?
                    .to_owned(),
                crate::hir::OwnershipMode::Borrow => format!("(*spx_param_{index})"),
                _ => {
                    return Err(backend_error(
                        "Bytes parameter lacks validated ownership classification",
                    ));
                }
            }
        } else if is_aggregate_type(program, &param.ty)?
            || (owned(&param.ty) && param.ownership == crate::hir::OwnershipMode::Borrow)
        {
            format!("(*spx_param_{index})")
        } else {
            format!("spx_param_{index}")
        };
        variables.insert(
            param.id.clone(),
            CBinding {
                name,
                ty: param.ty.clone(),
            },
        );
        if param.ownership == crate::hir::OwnershipMode::Borrow {
            for path in borrowed_aggregate_byte_paths(
                program,
                emission.record_layouts,
                emission.variant_layouts,
                &param.ty,
            )? {
                let suffix = borrowed_aggregate_path_suffix(&path)?;
                borrowed_aggregate_bytes.insert(
                    (param.id.clone(), path),
                    format!("(*spx_param_{index}_borrow_{suffix})"),
                );
            }
        }
    }
    // String values with cleanup-plan slots use that plan as their only owner.
    let track_strings = emission.output_profile.tracks_strings(function) && bytes_plan.is_none();
    let mut function_body = if track_strings {
        owned_strings::FunctionOutput::Staged(crate::bounded_output::CappedString::new())
    } else {
        owned_strings::FunctionOutput::Direct(&mut *output)
    };
    let mut emitter = CEmitter::new(
        &mut function_body,
        variables,
        function,
        emission,
        bytes_plan.as_ref(),
        borrowed_aggregate_bytes,
        track_strings,
    );
    if emitter.owned_strings.is_some() {
        for (index, param) in function.params.iter().enumerate() {
            if matches!(param.ty, ResolvedType::String) {
                if param.ownership != hir::OwnershipMode::Own {
                    return Err(backend_error(
                        "inline String parameter lacks validated owned classification",
                    ));
                }
                let name = format!("spx_param_{index}");
                emitter
                    .owned_strings
                    .as_mut()
                    .unwrap()
                    .register(&name, false)?;
                emitter.string_initialize(&name);
            }
        }
        if matches!(function.return_type, ResolvedType::String) {
            emitter
                .owned_strings
                .as_mut()
                .unwrap()
                .register("spx_result", false)?;
        }
    }
    emitter.line("spx_status_token spx_status = SPX_STATUS_SUCCESS;");
    emitter.line("bool spx_call_entered = false;");
    if has_try {
        emitter.line("bool spx_result_staged = false;");
    }
    emitter.line("(void)spx_ctx;");
    let borrowed_params = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| matches!(param.ty, ResolvedType::Str))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let borrowed_byte_params = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, param)| matches!(param.ty, ResolvedType::SliceU8))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if !borrowed_params.is_empty() || !borrowed_byte_params.is_empty() {
        emitter
            .line("const bool spx_borrowed_str_root = spx_ctx->borrowed_str_depth == UINT32_C(0);");
        emitter.line("if (spx_ctx->borrowed_str_depth == UINT32_MAX) spx_runtime_invariant_failure(\"borrowed str call depth exhausted\");");
        emitter.line("if (spx_borrowed_str_root) {");
        emitter.indent += 1;
        emitter.line("uint64_t spx_borrowed_root_bytes = UINT64_C(0);");
        if !borrowed_params.is_empty() {
            for index in &borrowed_params {
                emitter.line(&format!("spx_str_require_valid(spx_param_{index});"));
                emitter.line(&format!(
                    "if (spx_param_{index}.len > SPX_BORROWED_STR_MAX_BYTES - spx_borrowed_root_bytes) spx_runtime_invariant_failure(\"borrowed invocation exceeds cumulative root byte budget\");"
                ));
                emitter.line(&format!(
                    "spx_borrowed_root_bytes += spx_param_{index}.len;"
                ));
            }
        }
        if !borrowed_byte_params.is_empty() {
            for index in &borrowed_byte_params {
                emitter.line(&format!("spx_borrowed_root_bytes = spx_slice_u8_charge_root(spx_borrowed_root_bytes, spx_param_{index});"));
            }
        }
        emitter.indent -= 1;
        emitter.line("}");
        emitter.line("++spx_ctx->borrowed_str_depth;");
    }
    emitter.line("if (spx_ctx->call_depth >= SPX_MAX_CALL_DEPTH) {");
    emitter.indent += 1;
    emitter.line("spx_status = spx_rt_call_depth_failure(spx_ctx);");
    emitter.line("goto spx_epilogue;");
    emitter.indent -= 1;
    emitter.line("}");
    emitter.line("++spx_ctx->call_depth;");
    emitter.line("spx_call_entered = true;");
    emitter.semantic_metered = semantic_ordinal.is_some();
    emitter.semantic_charge();
    emitter.line(&format!(
        "{} spx_result = {{0}};",
        c_value_type(program, resource_abi, &function.return_type)?
    ));
    // An owned-Bytes result moves field by field, and a contract-failure lane
    // leaves even that unreached, so the slot can go unnamed in valid C.
    if owned(&function.return_type) || emitter.record_contains_owned_bytes(&function.return_type)? {
        emitter.line("(void)spx_result;");
    }
    emitter.mark_unused_parameter_aliases()?;
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "precondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emit_contract_status(
            &mut emitter,
            function,
            "SPX_STATUS_CONTRACT_REQUIRES_FALSE",
            "requires",
            contract_label(contract, contract_labels),
        );
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.try_target_enabled = true;
    let body = emitter.emit_expr(&function.body)?;
    emitter.try_target_enabled = false;
    emitter.require_type(&body.ty, &function.return_type, "function body")?;
    if matches!(body.ty, ResolvedType::String) && emitter.owned_strings.is_some() {
        emitter.string_move("spx_result", &body.code);
    } else if !owned(&body.ty) && emitter.record_contains_owned_bytes(&body.ty)? {
        emitter.move_owned_record_fields("spx_result", &body.code, &body.ty)?;
    } else if !owned(&body.ty) {
        emitter.line(&format!("spx_result = {};", body.code));
    }
    if has_try {
        emitter.line("spx_result_staged = true;");
        emitter.label("spx_postconditions");
    }

    if is_aggregate_type(program, &function.return_type)? {
        if let Some(plan) = &bytes_plan {
            for path in borrowed_aggregate_byte_paths(
                program,
                emission.record_layouts,
                emission.variant_layouts,
                &function.return_type,
            )? {
                if let Ok(alias) = plan.value_at(&crate::cleanup_plan::CleanupPlace {
                    storage: crate::cleanup_plan::StorageId::ProvisionalResult,
                    projections: path.clone(),
                }) {
                    emitter
                        .borrowed_aggregate_bytes
                        .insert((function.result_id.clone(), path), alias.to_owned());
                }
            }
        }
    }
    emitter.variables.insert(
        function.result_id.clone(),
        CBinding {
            name: if owned(&function.return_type) {
                bytes_plan
                    .as_ref()
                    .ok_or_else(|| backend_error("owned Bytes result has no cleanup plan"))?
                    .provisional()?
                    .0
                    .to_owned()
            } else {
                "spx_result".to_owned()
            },
            ty: function.return_type.clone(),
        },
    );
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "postcondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emit_contract_status(
            &mut emitter,
            function,
            "SPX_STATUS_CONTRACT_ENSURES_FALSE",
            "ensures",
            contract_label(contract, contract_labels),
        );
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.line("goto spx_epilogue;");
    let string_cells = emitter.owned_strings.take();
    drop(emitter);
    if let owned_strings::FunctionOutput::Staged(body) = function_body {
        if let Some(cells) = &string_cells {
            output.push_str(&cells.declarations());
        }
        output.push_str(&body.into_string());
    }
    output.push_str("spx_epilogue:\n");
    output.push_str("    if (spx_call_entered) {\n        if (spx_ctx->call_depth == UINT32_C(0)) spx_runtime_invariant_failure(\"call depth underflow\");\n        --spx_ctx->call_depth;\n    }\n");
    if crate::list_ops::resolved_program_uses_list(program) {
        output.push_str("    if (spx_call_entered && spx_ctx->call_depth == UINT32_C(0)) spx_list_release(spx_ctx);\n");
    }
    if !borrowed_params.is_empty() || !borrowed_byte_params.is_empty() {
        output.push_str("    if (spx_ctx->borrowed_str_depth == UINT32_C(0)) spx_runtime_invariant_failure(\"borrowed str call depth underflow\");\n");
        output.push_str("    --spx_ctx->borrowed_str_depth;\n");
    }
    // Callee-owned parameters free their storage on every exit path; a moved Bytes carrier is normalized by `spx_bytes_move`, making this exact-once. the
    // staged result is handed to the caller instead.
    if let Some(cells) = &string_cells {
        for name in cells.names() {
            let guard = if name == "spx_result" {
                "spx_status != SPX_STATUS_SUCCESS && "
            } else {
                ""
            };
            output.push_str(&format!("    if ({guard}{name}_live) {{ {name}_live = false; spx_string_drop({name}); {name} = NULL; }}\n"));
        }
    } else if bytes_plan.is_none() {
        for (index, param) in function.params.iter().enumerate() {
            if matches!(param.ty, ResolvedType::String) {
                output.push_str(&format!("    spx_string_drop(spx_param_{index});\n"));
            }
        }
    }
    if let Some(plan) = &bytes_plan {
        output.push_str(&plan.epilogue());
    }
    if has_try {
        output.push_str("    if (spx_status == SPX_STATUS_SUCCESS && !spx_result_staged) spx_runtime_invariant_failure(\"unstaged function result\");\n");
    }
    output.push_str("    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;\n");
    if owned(&function.return_type) {
        let (value, flag) = bytes_plan
            .as_ref()
            .ok_or_else(|| backend_error("owned Bytes result has no cleanup plan"))?
            .provisional()?;
        let move_call = owned_moves::owned_move(&function.return_type, value);
        output.push_str(&format!(
            "    if (!{flag}) spx_runtime_invariant_failure(\"dead owned provisional result\");\n    *spx_result_out = {move_call};\n    {flag} = false;\n"
        ));
    } else if is_aggregate_type(program, &function.return_type)?
        && bytes_plan.as_ref().is_some_and(|plan| {
            plan.has_projected_leaves(&crate::cleanup_plan::StorageId::ProvisionalResult)
        })
    {
        let plan = bytes_plan
            .as_ref()
            .expect("projected result check requires a plan");
        let publish = if variant_declaration_id(program, &function.return_type)?.is_some() {
            let layout = emission.variant_layouts.layout(&function.return_type)?;
            nested_owned::emit_owned_variant_shell(
                output,
                program,
                emission.record_layouts,
                layout,
                "(*spx_result_out)",
                "spx_result",
            )?;
            if !plan.has_variant_leaves(&crate::cleanup_plan::StorageId::ProvisionalResult) {
                return Err(backend_error(
                    "owned variant result disagrees with its cleanup plan",
                ));
            }
            plan.materialize_variant_carrier(
                &crate::cleanup_plan::StorageId::ProvisionalResult,
                "(*spx_result_out)",
                "spx_result",
                layout,
            )?
        } else {
            nested_owned::emit_owned_record_shell(
                output,
                program,
                emission.record_layouts,
                "(*spx_result_out)",
                "spx_result",
                &function.return_type,
                &mut BTreeSet::new(),
            )?;
            plan.publish_record_result("(*spx_result_out)")?
        };
        output.push_str(
            &publish
                .lines()
                .map(|line| format!("    {line}\n"))
                .collect::<String>(),
        );
        if variant_declaration_id(program, &function.return_type)?.is_some() {
            output.push_str("    (*spx_result_out).spx_tag = spx_result.spx_tag;\n");
        }
    } else {
        if string_cells.is_some() && matches!(function.return_type, ResolvedType::String) {
            output.push_str("    if (!spx_result_live) spx_runtime_invariant_failure(\"dead String result\");\n");
        }
        output.push_str("    *spx_result_out = spx_result;\n");
        if string_cells.is_some() && matches!(function.return_type, ResolvedType::String) {
            output.push_str("    spx_result_live = false;\n");
        }
    }
    output.push_str("    return SPX_STATUS_SUCCESS;\n");
    output.push_str("}\n\n");
    Ok(())
}

fn expression_has_try(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.kind,
            ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. }
        ) {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

#[derive(Clone)]
pub(super) struct CFunction {
    pub(super) symbol: String,
    pub(super) params: Vec<ResolvedType>,
    pub(super) param_ownerships: Vec<crate::hir::OwnershipMode>,
    pub(super) return_type: ResolvedType,
}

pub(super) fn function_index(
    program: &ResolvedProgram,
) -> Result<HashMap<FunctionExecutionId, CFunction>, Diagnostic> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        let declaration = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| {
                backend_error(format!(
                    "resolved function `{}` has no declaration",
                    function.id
                ))
            })?;
        if declaration.kind != DeclarationKind::Function {
            return Err(backend_error(format!(
                "resolved callable `{}` does not refer to a function declaration",
                function.id
            )));
        }
        let metadata = CFunction {
            symbol: c_function_symbol(&function.id),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            param_ownerships: function
                .params
                .iter()
                .map(|param| param.ownership)
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions
            .insert(
                FunctionExecutionId::Monomorphic(function.id.clone()),
                metadata,
            )
            .is_some()
        {
            return Err(backend_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
    }
    for instance in &program.function_instances {
        let function = &instance.function;
        let execution = FunctionExecutionId::Generic(instance.id.clone());
        let metadata = CFunction {
            symbol: c_function_execution_symbol(&execution),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            param_ownerships: function
                .params
                .iter()
                .map(|param| param.ownership)
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions.insert(execution, metadata).is_some() {
            return Err(backend_error(format!(
                "duplicate resolved function instance `{}`",
                instance.id
            )));
        }
    }
    for function in closure::closure_functions(program)? {
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let metadata = CFunction {
            symbol: c_function_symbol(&function.id),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            param_ownerships: function
                .params
                .iter()
                .map(|param| param.ownership)
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions.insert(execution, metadata).is_some() {
            return Err(backend_error(format!(
                "duplicate native closure identity `{}`",
                function.id
            )));
        }
    }
    Ok(functions)
}

fn c_function_execution_symbol(id: &FunctionExecutionId) -> String {
    let identity = match id {
        FunctionExecutionId::Monomorphic(declaration) => format!(
            "semaprax.function-execution.v1:monomorphic:{}:{}",
            declaration.as_str().len(),
            declaration
        ),
        FunctionExecutionId::Generic(instance) => format!(
            "semaprax.function-execution.v1:generic:{}:{}",
            instance.as_str().len(),
            instance
        ),
    };
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_exec_");
    for byte in identity.bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

pub(super) fn c_function_symbol(id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_decl_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

/// Refutable Match v1: exact C spelling of a literal pattern, using the same
/// conventions as the matching expression literals.
fn c_pattern_literal(value: hir::PatternValue) -> String {
    match value {
        hir::PatternValue::Int(value) => c_i64(value),
        hir::PatternValue::Int32(value) => c_i32(value),
        hir::PatternValue::Uint8(value) => format!("UINT8_C({value})"),
        hir::PatternValue::Usize(value) => format!("UINT64_C({value})"),
        hir::PatternValue::Char(value) => format!("UINT32_C(0x{value:x})"),
        hir::PatternValue::Bool(value) => value.to_string(),
    }
}

#[derive(Clone)]
struct CBinding {
    name: String,
    ty: ResolvedType,
}

struct CValue {
    code: String,
    ty: ResolvedType,
}

struct CEmitter<'a, O: COutput> {
    output: &'a mut O,
    program: &'a ResolvedProgram,
    resource_abi: &'a native_resource::NativeResourceAbi,
    variables: HashMap<ValueId, CBinding>,
    /// `let mut` bindings; a Copy read of one is snapshotted (issue #561).
    mutable_bindings: BTreeSet<ValueId>,
    function: &'a ResolvedFunction,
    functions: &'a HashMap<FunctionExecutionId, CFunction>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
    return_type: &'a ResolvedType,
    bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
    borrowed_aggregate_bytes: HashMap<(ValueId, Vec<DeclarationId>), String>,
    output_profile: NativeOutputProfile,
    semantic_metered: bool,
    owned_strings: Option<owned_strings::OwnedStrings>,
    try_target_enabled: bool,
    string_owner_moves: BTreeSet<hir::ExpressionId>,
    string_condition_reads: BTreeSet<hir::ExpressionId>,
    next_local: usize,
    indent: usize,
}

impl<'a, O: COutput> CEmitter<'a, O> {
    fn new(
        output: &'a mut O,
        variables: HashMap<ValueId, CBinding>,
        function: &'a ResolvedFunction,
        emission: &'a NativeEmissionContext<'a>,
        bytes_plan: Option<&'a native_bytes::NativeBytesPlan>,
        borrowed_aggregate_bytes: HashMap<(ValueId, Vec<DeclarationId>), String>,
        track_strings: bool,
    ) -> Self {
        Self {
            output,
            program: emission.program,
            resource_abi: emission.resource_abi,
            variables,
            mutable_bindings: BTreeSet::new(),
            function,
            functions: emission.functions,
            record_layouts: emission.record_layouts,
            variant_layouts: emission.variant_layouts,
            return_type: &function.return_type,
            bytes_plan,
            borrowed_aggregate_bytes,
            output_profile: emission.output_profile,
            semantic_metered: false,
            owned_strings: track_strings.then(owned_strings::OwnedStrings::default),
            try_target_enabled: false,
            string_owner_moves: crate::string_ops::same_owner_concat_operands(function),
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            next_local: 0,
            indent: 1,
        }
    }

    fn line(&mut self, value: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{value}").expect("writing to a string cannot fail");
    }

    fn label(&mut self, value: &str) {
        writeln!(self.output, "{value}: ;").expect("writing to a string cannot fail");
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
            Err(backend_error(format!(
                "inconsistent HIR type for {context}: expected `{}`, found `{}`",
                expected.identity_key(),
                actual.identity_key()
            )))
        }
    }
}
pub(super) mod environment_io;
pub(super) mod process_io;
