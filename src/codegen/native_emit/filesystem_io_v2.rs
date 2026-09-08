//! Native C11 lowering for the closed callback-backed Filesystem I/O v2 ABI.
//!
//! The generated translation unit has no filesystem imports.  A host supplies
//! every operation through this one invocation-scoped callback table.

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

pub fn emit_c_with_filesystem_io_v2(
    program: &Program,
    command_id: &str,
) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_hir_c_with_filesystem_io_v2(&resolved, command_id)
}

pub fn emit_hir_c_with_filesystem_io_v2(
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
        crate::command_io_ops::CommandOperationProfile::FilesystemV2,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::FilesystemCommandIoV2,
        Some(&command.id),
    )
}

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    if !super::program_uses_byte_data(program) {
        native_byte_data::emit_runtime(output);
    }
    writeln!(
        output,
        "#define SPX_FILESYSTEM_STATUS_DOMAIN_V2 \"{}\"\n\
         #define SPX_FILESYSTEM_INVALID_PATH_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_NOT_FOUND_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_ALREADY_EXISTS_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_CAPACITY_EXCEEDED_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_IO_FAILURE_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_AUTHORITY_DENIED_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_INVALID_FILE_TYPE_V2 UINT32_C({})\n\
         #define SPX_FILESYSTEM_MAX_PATH_BYTES_V2 UINT64_C({})\n\
         #define SPX_FILESYSTEM_MAX_FILE_BYTES_V2 UINT64_C({})\n\
         #define SPX_FILESYSTEM_MAX_TOTAL_BYTES_V2 UINT64_C({})\n\
         #define SPX_FILESYSTEM_MAX_OPERATIONS_V2 UINT64_C({})",
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
    output.push_str(FILESYSTEM_RUNTIME_C);
}

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    output.push_str(
        &FILESYSTEM_RUNNER_C
            .replace("{command_symbol}", command_symbol)
            .replace("{{", "{")
            .replace("}}", "}"),
    );
}

const FILESYSTEM_RUNTIME_C: &str = r#"
struct spx_filesystem_callbacks_v2 {
    void *context;
    uint32_t (*read)(void *, spx_slice_u8_v1, uint64_t, uint8_t *, uint64_t, uint64_t *);
    uint32_t (*write_new)(void *, spx_slice_u8_v1, uint64_t, spx_slice_u8_v1, uint64_t, uint64_t *);
    uint32_t (*stat)(void *, spx_slice_u8_v1, uint64_t, uint64_t *);
    uint32_t (*list)(void *, spx_slice_u8_v1, uint64_t, uint8_t *, uint64_t, uint64_t *);
    uint32_t (*create_dir)(void *, spx_slice_u8_v1, uint64_t, uint64_t *);
    uint32_t (*remove)(void *, spx_slice_u8_v1, uint64_t, uint64_t *);
    uint32_t (*write_atomic)(void *, spx_slice_u8_v1, uint64_t, spx_slice_u8_v1, uint64_t, uint64_t *);
    void (*settle)(void *);
};
struct spx_filesystem_command_result_v2 { bool semantic_success; bool matched; char status_domain[64]; uint32_t status_code; uint32_t status_class; uint32_t status_retryability; };
struct spx_filesystem_command_state_v2 { const struct spx_filesystem_callbacks_v2 *callbacks; uint64_t charged_bytes; uint64_t operations; };
static __attribute__((unused)) struct spx_filesystem_command_state_v2 *spx_filesystem_command_state_v2(struct spx_context *ctx) { if (ctx == NULL || ctx->target_state == NULL) spx_runtime_invariant_failure("filesystem state is unavailable"); return (struct spx_filesystem_command_state_v2 *)ctx->target_state; }
static __attribute__((unused)) spx_status_token spx_filesystem_status_v2(struct spx_context *ctx, uint32_t code) { if (code < SPX_FILESYSTEM_INVALID_PATH_V2 || code > SPX_FILESYSTEM_INVALID_FILE_TYPE_V2) spx_runtime_invariant_failure("filesystem status is outside the closed table"); spx_status_token token = SPX_STATUS_SUCCESS; if (!spx_status_record_adapter(ctx, SPX_FILESYSTEM_STATUS_DOMAIN_V2, code, SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token)) spx_runtime_invariant_failure("filesystem status could not be recorded"); return token; }
static __attribute__((unused)) spx_status_token spx_filesystem_charge_v2(struct spx_context *ctx, uint64_t bytes) { struct spx_filesystem_command_state_v2 *state = spx_filesystem_command_state_v2(ctx); if (state->operations >= SPX_FILESYSTEM_MAX_OPERATIONS_V2 || bytes > SPX_FILESYSTEM_MAX_TOTAL_BYTES_V2 - state->charged_bytes) return spx_filesystem_status_v2(ctx, SPX_FILESYSTEM_CAPACITY_EXCEEDED_V2); state->operations++; state->charged_bytes += bytes; return SPX_STATUS_SUCCESS; }
static __attribute__((unused)) bool spx_filesystem_path_is_shaped_v2(spx_slice_u8_v1 path, uint64_t length, bool root_allowed) { if (length > path.len || length > SPX_FILESYSTEM_MAX_PATH_BYTES_V2) return false; if (length == UINT64_C(0)) return root_allowed; if (path.ptr == NULL) return false; uint64_t component = 0; for (uint64_t i = 0; i < length; ++i) { uint8_t byte = path.ptr[i]; if (byte == 0 || byte == 92 || byte == 58) return false; if (byte != 47) { component++; continue; } if (component == 0 || (component == 1 && path.ptr[i-1] == 46) || (component == 2 && path.ptr[i-1] == 46 && path.ptr[i-2] == 46)) return false; component = 0; } return component != 0 && !(component == 1 && path.ptr[length-1] == 46) && !(component == 2 && path.ptr[length-1] == 46 && path.ptr[length-2] == 46); }
static __attribute__((unused)) spx_status_token spx_filesystem_checked_path_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, bool root_allowed) { return spx_filesystem_path_is_shaped_v2(path, length, root_allowed) ? SPX_STATUS_SUCCESS : spx_filesystem_status_v2(ctx, SPX_FILESYSTEM_INVALID_PATH_V2); }
static __attribute__((unused)) bool spx_filesystem_list_is_shaped_v2(const uint8_t *data, uint64_t count) { if (count == 0) return true; if (data == NULL) return false; uint64_t start = 0, names = 0, previous_start = 0, previous_size = 0; for (uint64_t i = 0; i < count; ++i) { if (data[i] == 0) { uint64_t size = i - start; if (size == 0 || size > SPX_FILESYSTEM_MAX_PATH_BYTES_V2 || (size == 1 && data[start] == 46) || (size == 2 && data[start] == 46 && data[start + 1] == 46)) return false; for (uint64_t j = start; j < i; ++j) if (data[j] == 47) return false; if (names >= UINT64_C(1024)) return false; if (names != 0) { uint64_t common = previous_size < size ? previous_size : size; uint64_t k = 0; while (k < common && data[previous_start + k] == data[start + k]) ++k; if (k == common ? previous_size >= size : data[previous_start + k] >= data[start + k]) return false; } ++names; previous_start = start; previous_size = size; start = i + 1; } } return start == count; }
static __attribute__((unused)) spx_status_token spx_host_file_read_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t max, spx_bytes_v1 *out) { *out=(spx_bytes_v1){0}; spx_status_token s=spx_filesystem_charge_v2(ctx,max); if(s) return s; if((s=spx_filesystem_checked_path_v2(ctx,path,length,false)))return s; if(max>SPX_FILESYSTEM_MAX_FILE_BYTES_V2)return spx_filesystem_status_v2(ctx,4); struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx); if(!state->callbacks||!state->callbacks->read)return spx_filesystem_status_v2(ctx,6); uint8_t *data=max?(uint8_t*)malloc((size_t)max):NULL; if(max&&!data)return spx_filesystem_status_v2(ctx,5); uint64_t written=0; uint32_t code=state->callbacks->read(state->callbacks->context,path,length,data,max,&written); if(code||written>max){free(data);return spx_filesystem_status_v2(ctx,code?code:4);} if(!written){free(data);data=NULL;} out->ptr=data;out->len=written;return 0; }
static __attribute__((unused)) spx_status_token spx_host_file_list_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t max, spx_bytes_v1 *out) { *out=(spx_bytes_v1){0}; spx_status_token s=spx_filesystem_charge_v2(ctx,max); if(s)return s; if((s=spx_filesystem_checked_path_v2(ctx,path,length,true)))return s; if(max>SPX_FILESYSTEM_MAX_FILE_BYTES_V2)return spx_filesystem_status_v2(ctx,4); struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->list)return spx_filesystem_status_v2(ctx,6);uint8_t *data=max?(uint8_t*)malloc((size_t)max):NULL;if(max&&!data)return spx_filesystem_status_v2(ctx,5);uint64_t written=0;uint32_t code=state->callbacks->list(state->callbacks->context,path,length,data,max,&written);if(code||written>max){free(data);return spx_filesystem_status_v2(ctx,code?code:4);}if(!spx_filesystem_list_is_shaped_v2(data,written)){free(data);return spx_filesystem_status_v2(ctx,5);}if(!written){free(data);data=NULL;}out->ptr=data;out->len=written;return 0; }
static __attribute__((unused)) spx_status_token spx_host_file_stat_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { *out=0;spx_status_token s=spx_filesystem_charge_v2(ctx,0);if(s)return s;if((s=spx_filesystem_checked_path_v2(ctx,path,length,true)))return s;struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->stat)return spx_filesystem_status_v2(ctx,6);uint32_t code=state->callbacks->stat(state->callbacks->context,path,length,out);if(code)return spx_filesystem_status_v2(ctx,code);uint64_t kind=*out&UINT64_C(3), size=*out>>2;if((kind!=1&&kind!=2)||(kind==2&&size!=0)||size>UINT64_MAX/4)return spx_filesystem_status_v2(ctx,5);return 0; }
static __attribute__((unused)) spx_status_token spx_host_file_write_new_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t data_length, uint64_t *out) { *out=0;spx_status_token s=spx_filesystem_charge_v2(ctx,data_length);if(s)return s;if((s=spx_filesystem_checked_path_v2(ctx,path,length,false)))return s;if(data_length>SPX_FILESYSTEM_MAX_FILE_BYTES_V2||data_length>data.len||(data_length&&data.ptr==NULL))return spx_filesystem_status_v2(ctx,4);struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->write_new)return spx_filesystem_status_v2(ctx,6);uint32_t code=state->callbacks->write_new(state->callbacks->context,path,length,data,data_length,out);if(code)return spx_filesystem_status_v2(ctx,code);return *out==data_length?0:spx_filesystem_status_v2(ctx,5); }
static __attribute__((unused)) spx_status_token spx_host_file_write_atomic_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t data_length, uint64_t *out) { *out=0;spx_status_token s=spx_filesystem_charge_v2(ctx,data_length);if(s)return s;if((s=spx_filesystem_checked_path_v2(ctx,path,length,false)))return s;if(data_length>SPX_FILESYSTEM_MAX_FILE_BYTES_V2||data_length>data.len||(data_length&&data.ptr==NULL))return spx_filesystem_status_v2(ctx,4);struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->write_atomic)return spx_filesystem_status_v2(ctx,6);uint32_t code=state->callbacks->write_atomic(state->callbacks->context,path,length,data,data_length,out);if(code)return spx_filesystem_status_v2(ctx,code);return *out==data_length?0:spx_filesystem_status_v2(ctx,5); }
static __attribute__((unused)) spx_status_token spx_host_file_create_dir_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { *out=0;spx_status_token s=spx_filesystem_charge_v2(ctx,0);if(s)return s;if((s=spx_filesystem_checked_path_v2(ctx,path,length,false)))return s;struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->create_dir)return spx_filesystem_status_v2(ctx,6);uint32_t code=state->callbacks->create_dir(state->callbacks->context,path,length,out);if(code)return spx_filesystem_status_v2(ctx,code);return *out==0?0:spx_filesystem_status_v2(ctx,5); }
static __attribute__((unused)) spx_status_token spx_host_file_remove_v2(struct spx_context *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { *out=0;spx_status_token s=spx_filesystem_charge_v2(ctx,0);if(s)return s;if((s=spx_filesystem_checked_path_v2(ctx,path,length,false)))return s;struct spx_filesystem_command_state_v2 *state=spx_filesystem_command_state_v2(ctx);if(!state->callbacks||!state->callbacks->remove)return spx_filesystem_status_v2(ctx,6);uint32_t code=state->callbacks->remove(state->callbacks->context,path,length,out);if(code)return spx_filesystem_status_v2(ctx,code);return *out==0?0:spx_filesystem_status_v2(ctx,5); }
"#;

const FILESYSTEM_RUNNER_C: &str = r#"
int spx_run_filesystem_command_v2(const struct spx_filesystem_callbacks_v2 *callbacks, struct spx_filesystem_command_result_v2 *out) {{ if (!out) return 0; memset(out,0,sizeof(*out)); struct spx_status_entry entries[1]; struct spx_filesystem_command_state_v2 state={{.callbacks=callbacks}}; struct spx_context ctx={{0}}; if(!spx_context_init(&ctx,1,entries,1,NULL,NULL,&state)){{if(callbacks&&callbacks->settle)callbacks->settle(callbacks->context);return 0;}} bool matched=false; spx_status_token status={command_symbol}(&ctx,&matched); if(callbacks&&callbacks->settle)callbacks->settle(callbacks->context);if(status){{const struct spx_normalized_status *failure=spx_status_resolve(&ctx,status);(void)spx_status_resolve_detail(&ctx,status);if(!failure||!failure->domain_id)return 0;size_t n=0;if(!spx_status_domain_size(failure->domain_id,&n)||n>sizeof(out->status_domain))return 0;memcpy(out->status_domain,failure->domain_id,n);out->status_code=failure->code;out->status_class=failure->status_class;out->status_retryability=failure->retryability;return 1;}}out->semantic_success=true;out->matched=matched;return 1; }}
"#;
