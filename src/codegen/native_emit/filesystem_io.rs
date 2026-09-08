//! Native C11 lowering for the closed, callback-backed filesystem profile.
//!
//! Semantic functions receive only `spx_context`. The runner is the sole
//! holder of the host callbacks, so this translation unit never opens a path,
//! consults a working directory, or imports a platform filesystem API.

use std::collections::HashMap;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::filesystem_ops as ops;
use crate::hir::{self, ResolvedProgram};

use super::super::{
    backend_error, first_backend_diagnostic, native_byte_data, reject_native_rust_for_native,
    COutput,
};
use super::{emit_hir_c_with_labels, NativeOutputProfile};

const ADMITTED_PERMITS: [&str; 2] = [ops::READ_EFFECT, ops::WRITE_EFFECT];

/// Resolve source and emit the callback-backed Filesystem I/O v1 profile.
pub fn emit_c_with_filesystem_io(
    program: &Program,
    command_id: &str,
) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_hir_c_with_filesystem_io(&resolved, command_id)
}

/// Emit checked HIR for one explicit zero-argument boolean filesystem command.
pub fn emit_hir_c_with_filesystem_io(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    reject_native_rust_for_native(program)?;
    if program
        .permits
        .iter()
        .any(|permit| !ADMITTED_PERMITS.contains(&permit.as_str()))
        || !program
            .permits
            .iter()
            .any(|permit| ops::FILESYSTEM_EFFECTS.contains(&permit.as_str()))
    {
        return Err(backend_error(
            "filesystem command permits must contain only fs.read/fs.write and include one",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| {
            backend_error(format!(
                "selected filesystem command `{command_id}` is absent"
            ))
        })?;
    if program
        .declarations
        .declaration(&command.id)
        .is_none_or(|declaration| {
            declaration.identity_origin != crate::hir::IdentityOrigin::Explicit
        })
        || !command.params.is_empty()
        || command.return_type != crate::hir::ResolvedType::Bool
    {
        return Err(backend_error(
            "selected filesystem command must be an explicit stable-ID `fn () -> bool`",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::FilesystemV1,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::FilesystemCommandIo,
        Some(&command.id),
    )
}

/// The profile runtime and callback ABI. The host owns callback context and
/// provides the sole physical filesystem authority for one invocation. Read
/// callbacks write into a generated allocation, so no host allocation enters
/// the semantic owned-Bytes carrier.
pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if !super::program_uses_byte_data(program) {
        native_byte_data::emit_runtime(output);
    }
    emit_constants(output);
    output.push_str(FILESYSTEM_RUNTIME_C);
}

fn emit_constants(output: &mut impl COutput) {
    writeln!(
        output,
        "#define SPX_FILESYSTEM_STATUS_DOMAIN_V1 \"{}\"\n\\
         #define SPX_FILESYSTEM_INVALID_PATH_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_NOT_FOUND_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_ALREADY_EXISTS_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_IO_FAILURE_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_AUTHORITY_DENIED_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_INVALID_FILE_TYPE_V1 UINT32_C({})\n\\
         #define SPX_FILESYSTEM_MAX_PATH_BYTES_V1 UINT64_C({})\n\\
         #define SPX_FILESYSTEM_MAX_FILE_BYTES_V1 UINT64_C({})\n\\
         #define SPX_FILESYSTEM_MAX_TOTAL_BYTES_V1 UINT64_C({})\n\\
         #define SPX_FILESYSTEM_MAX_OPERATIONS_V1 UINT64_C({})",
        ops::STATUS_DOMAIN,
        ops::INVALID_PATH,
        ops::NOT_FOUND,
        ops::ALREADY_EXISTS,
        ops::CAPACITY_EXCEEDED,
        ops::IO_FAILURE,
        ops::AUTHORITY_DENIED,
        ops::INVALID_FILE_TYPE,
        ops::MAX_PATH_BYTES,
        ops::MAX_FILE_BYTES,
        ops::MAX_TOTAL_BYTES,
        ops::MAX_OPERATIONS,
    )
    .expect("writing filesystem constants cannot fail");
}

/// Emit the public runner. `settle` runs precisely once after the semantic
/// entry, before a status or bool result becomes visible to the host.
pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    let runner = FILESYSTEM_RUNNER_C
        .replace("{command_symbol}", command_symbol)
        .replace("{{", "{")
        .replace("}}", "}");
    output.push_str(&runner);
}

const FILESYSTEM_RUNTIME_C: &str = r#"
struct spx_filesystem_callbacks_v1 {
    void *context;
    uint32_t (*read)(
        void *context,
        spx_slice_u8_v1 path_storage,
        uint64_t path_length,
        uint8_t *destination,
        uint64_t destination_capacity,
        uint64_t *written_out
    );
    uint32_t (*write_new)(
        void *context,
        spx_slice_u8_v1 path_storage,
        uint64_t path_length,
        spx_slice_u8_v1 data,
        uint64_t data_length,
        uint64_t *result_out
    );
    void (*settle)(void *context);
};

struct spx_filesystem_command_result_v1 {
    bool semantic_success;
    bool matched;
    char status_domain[64];
    uint32_t status_code;
    uint32_t status_class;
    uint32_t status_retryability;
};

struct spx_filesystem_command_state_v1 {
    const struct spx_filesystem_callbacks_v1 *callbacks;
    uint64_t charged_bytes;
    uint64_t operations;
};

static __attribute__((unused)) struct spx_filesystem_command_state_v1 *
spx_filesystem_command_state_v1(struct spx_context *spx_ctx) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("filesystem state is unavailable");
    }
    return (struct spx_filesystem_command_state_v1 *)spx_ctx->target_state;
}

static __attribute__((unused)) spx_status_token spx_filesystem_status_v1(
    struct spx_context *spx_ctx,
    uint32_t code
) {
    if (code < SPX_FILESYSTEM_INVALID_PATH_V1 ||
        code > SPX_FILESYSTEM_INVALID_FILE_TYPE_V1) {
        spx_runtime_invariant_failure("filesystem status is outside the closed table");
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        SPX_FILESYSTEM_STATUS_DOMAIN_V1,
        code,
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("filesystem status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_filesystem_charge_v1(
    struct spx_context *spx_ctx,
    uint64_t bytes
) {
    struct spx_filesystem_command_state_v1 *state =
        spx_filesystem_command_state_v1(spx_ctx);
    if (state->operations >= SPX_FILESYSTEM_MAX_OPERATIONS_V1 ||
        bytes > SPX_FILESYSTEM_MAX_TOTAL_BYTES_V1 - state->charged_bytes) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1);
    }
    state->operations += UINT64_C(1);
    state->charged_bytes += bytes;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) bool spx_filesystem_path_is_shaped_v1(
    spx_slice_u8_v1 path_storage,
    uint64_t path_length
) {
    if (path_storage.ptr == NULL || path_length == UINT64_C(0) ||
        path_length > path_storage.len ||
        path_length > SPX_FILESYSTEM_MAX_PATH_BYTES_V1) return false;
    uint64_t component = UINT64_C(0);
    for (uint64_t index = UINT64_C(0); index < path_length; ++index) {
        uint8_t byte = path_storage.ptr[index];
        if (byte == UINT8_C(0) || byte == UINT8_C(92) || byte == UINT8_C(58)) return false;
        if (byte != UINT8_C(47)) { component += UINT64_C(1); continue; }
        if (component == UINT64_C(0) ||
            (component == UINT64_C(1) && path_storage.ptr[index - UINT64_C(1)] == UINT8_C(46)) ||
            (component == UINT64_C(2) && path_storage.ptr[index - UINT64_C(1)] == UINT8_C(46) &&
             path_storage.ptr[index - UINT64_C(2)] == UINT8_C(46))) return false;
        component = UINT64_C(0);
    }
    if (component == UINT64_C(0)) return false;
    if (component == UINT64_C(1) && path_storage.ptr[path_length - UINT64_C(1)] == UINT8_C(46)) return false;
    return component != UINT64_C(2) ||
        path_storage.ptr[path_length - UINT64_C(1)] != UINT8_C(46) ||
        path_storage.ptr[path_length - UINT64_C(2)] != UINT8_C(46);
}

static __attribute__((unused)) spx_status_token spx_host_file_read_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 path_storage,
    uint64_t path_length,
    uint64_t max,
    spx_bytes_v1 *result_out
) {
    if (result_out == NULL) spx_runtime_invariant_failure("file_read result slot is unavailable");
    *result_out = (spx_bytes_v1){ .ptr = NULL, .len = UINT64_C(0) };
    spx_status_token charged = spx_filesystem_charge_v1(spx_ctx, max);
    if (charged != SPX_STATUS_SUCCESS) return charged;
    if (!spx_filesystem_path_is_shaped_v1(path_storage, path_length)) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_INVALID_PATH_V1);
    }
    if (max > SPX_FILESYSTEM_MAX_FILE_BYTES_V1) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1);
    }
    struct spx_filesystem_command_state_v1 *state = spx_filesystem_command_state_v1(spx_ctx);
    if (state->callbacks == NULL || state->callbacks->read == NULL) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_AUTHORITY_DENIED_V1);
    }
    uint8_t *destination = NULL;
    if (max != UINT64_C(0)) {
        destination = (uint8_t *)malloc((size_t)max);
        if (destination == NULL) {
            return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_IO_FAILURE_V1);
        }
    }
    uint64_t written = UINT64_C(0);
    uint32_t code = state->callbacks->read(
        state->callbacks->context,
        path_storage,
        path_length,
        destination,
        max,
        &written
    );
    if (code != UINT32_C(0)) {
        if (destination != NULL) free(destination);
        return spx_filesystem_status_v1(spx_ctx, code);
    }
    if (written > max || written > SPX_FILESYSTEM_MAX_FILE_BYTES_V1) {
        if (destination != NULL) free(destination);
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1);
    }
    if (written == UINT64_C(0) && destination != NULL) {
        free(destination);
        destination = NULL;
    }
    result_out->ptr = destination;
    result_out->len = written;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_host_file_write_new_v1(
    struct spx_context *spx_ctx,
    spx_slice_u8_v1 path_storage,
    uint64_t path_length,
    spx_slice_u8_v1 data,
    uint64_t data_length,
    uint64_t *result_out
) {
    if (result_out == NULL) spx_runtime_invariant_failure("file_write_new result slot is unavailable");
    *result_out = UINT64_C(0);
    spx_status_token charged = spx_filesystem_charge_v1(spx_ctx, data_length);
    if (charged != SPX_STATUS_SUCCESS) return charged;
    if (!spx_filesystem_path_is_shaped_v1(path_storage, path_length)) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_INVALID_PATH_V1);
    }
    if (data_length > SPX_FILESYSTEM_MAX_FILE_BYTES_V1) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1);
    }
    if (data_length > data.len || (data_length != UINT64_C(0) && data.ptr == NULL)) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V1);
    }
    struct spx_filesystem_command_state_v1 *state = spx_filesystem_command_state_v1(spx_ctx);
    if (state->callbacks == NULL || state->callbacks->write_new == NULL) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_AUTHORITY_DENIED_V1);
    }
    uint32_t code = state->callbacks->write_new(
        state->callbacks->context, path_storage, path_length, data, data_length, result_out
    );
    if (code != UINT32_C(0)) return spx_filesystem_status_v1(spx_ctx, code);
    if (*result_out != data_length) {
        return spx_filesystem_status_v1(spx_ctx, SPX_FILESYSTEM_IO_FAILURE_V1);
    }
    return SPX_STATUS_SUCCESS;
}
"#;

const FILESYSTEM_RUNNER_C: &str = r#"
int spx_run_filesystem_command_v1(
    const struct spx_filesystem_callbacks_v1 *callbacks,
    struct spx_filesystem_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_filesystem_command_state_v1 state = {{ .callbacks = callbacks }};
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, &state
    )) {{
        if (callbacks != NULL && callbacks->settle != NULL) callbacks->settle(callbacks->context);
        return 0;
    }}
    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, &matched);
    if (callbacks != NULL && callbacks->settle != NULL) callbacks->settle(callbacks->context);
    if (status != SPX_STATUS_SUCCESS) {{
        const struct spx_normalized_status *failure = spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        if (failure == NULL || failure->domain_id == NULL) {{
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        size_t domain_size = 0;
        if (!spx_status_domain_size(failure->domain_id, &domain_size) ||
            domain_size > sizeof(result_out->status_domain)) {{
            memset(result_out, 0, sizeof(*result_out));
            return 0;
        }}
        memcpy(result_out->status_domain, failure->domain_id, domain_size);
        result_out->status_code = failure->code;
        result_out->status_class = failure->status_class;
        result_out->status_retryability = failure->retryability;
        return 1;
    }}
    result_out->semantic_success = true;
    result_out->matched = matched;
    return 1;
}}
"#;
