//! Native Box v2 owns one Bytes carrier while retaining scalar-only v1 output.
use super::*;

pub(super) fn emit_runtime(output: &mut impl super::super::COutput) {
    // Only this added profile widens the common carrier validator. Scalar
    // allocation/get/extraction retain their own eight-tag admission below.
    let runtime = NATIVE_BOX_RUNTIME_C.replacen(
        "tag > UINT32_C(8) || value->ptr == NULL",
        "tag > UINT32_C(9) || value->ptr == NULL", 1,
    ).replacen(
        "(void)spx_box_require_valid(spx_ctx, value, tag); return *value->ptr;",
        "if (tag == UINT32_C(9)) spx_runtime_invariant_failure(\"owned Box payload cannot be copied\");\n    (void)spx_box_require_valid(spx_ctx, value, tag); return *value->ptr;", 1,
    ).replacen(
        "struct spx_box_authority_entry *entry = spx_box_require_valid(spx_ctx, value, tag);\n    uint64_t bits",
        "if (tag == UINT32_C(9)) spx_runtime_invariant_failure(\"owned Box payload requires consuming Bytes extraction\");\n    struct spx_box_authority_entry *entry = spx_box_require_valid(spx_ctx, value, tag);\n    uint64_t bits", 1,
    ).replacen(
        "uint64_t *payload = value->ptr;\n    *entry = (struct spx_box_authority_entry){0}; *value = (spx_box_v1){0}; free(payload);",
        "uint64_t *payload = value->ptr;\n    if (value->type_tag == UINT32_C(9)) spx_bytes_drop((spx_bytes_v1 *)(void *)payload);\n    *entry = (struct spx_box_authority_entry){0}; *value = (spx_box_v1){0}; free(payload);", 1,
    );
    output.push_str(&runtime);
    output.push_str(OWNED_PAYLOAD_C);
}

const OWNED_PAYLOAD_C: &str = r#"
static __attribute__((unused)) spx_status_token spx_box_bytes_new(
    struct spx_context *spx_ctx, spx_bytes_v1 *source, spx_box_v1 *result
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || source == NULL || result == NULL)
        spx_runtime_invariant_failure("invalid owned Bytes Box allocation");
    spx_bytes_require_valid(*source);
    *result = (spx_box_v1){0};
    spx_bytes_v1 *payload = (spx_bytes_v1 *)malloc(sizeof(spx_bytes_v1));
    if (payload == NULL) return spx_box_failure(spx_ctx);
    uint64_t generation = spx_box_next_generation(spx_ctx);
    uint32_t authority = spx_box_register(spx_ctx, (uint64_t *)(void *)payload, UINT32_C(9), generation);
    if (authority == UINT32_C(0)) { free(payload); return spx_box_failure(spx_ctx); }
    *payload = spx_bytes_move(source);
    result->ptr = (uint64_t *)(void *)payload; result->generation = generation;
    result->type_tag = UINT32_C(9); result->authority = authority;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_bytes_v1 spx_box_bytes_into_inner(
    struct spx_context *spx_ctx, spx_box_v1 *source
) {
    struct spx_box_authority_entry *entry = spx_box_require_valid(spx_ctx, source, UINT32_C(9));
    spx_bytes_v1 *payload = (spx_bytes_v1 *)(void *)source->ptr;
    spx_bytes_v1 result = spx_bytes_move(payload);
    *entry = (struct spx_box_authority_entry){0}; *source = (spx_box_v1){0};
    free(payload); return result;
}
"#;
