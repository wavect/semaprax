//! Reachability-gated C11 runtime for internal Owned Bounded Box v1.

mod owned_payload;

pub(super) fn emit_runtime(
    output: &mut impl super::COutput,
    program: &crate::hir::ResolvedProgram,
) {
    if crate::box_ops::resolved_program_uses_owned_payload(program) {
        owned_payload::emit_runtime(output);
        return;
    }
    output.push_str(NATIVE_BOX_RUNTIME_C);
}

pub(super) fn program_uses_box(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            crate::cleanup::is_owned_bounded_box_type(&function.return_type)
                || function
                    .params
                    .iter()
                    .any(|param| crate::cleanup::is_owned_bounded_box_type(&param.ty))
                || std::iter::once(&function.body)
                    .chain(function.requires.iter())
                    .chain(function.ensures.iter())
                    .any(|root| {
                        let mut found = false;
                        crate::hir::visit_resolved_calls(
                            root,
                            &mut |callee, instance, arguments| {
                                found |= instance.is_none()
                                    && arguments.len() == 1
                                    && crate::box_ops::by_id(callee.as_str()).is_some();
                            },
                        );
                        found
                    })
        })
}

const NATIVE_BOX_RUNTIME_C: &str = r#"#include <stdlib.h>

typedef struct {
    uint64_t *ptr;
    uint64_t generation;
    uint32_t type_tag;
    uint32_t authority;
} spx_box_v1;

static __attribute__((unused)) uint64_t spx_box_f32_bits(float value) {
    union { float value; uint32_t bits; } carrier = { .value = value };
    return (uint64_t)carrier.bits;
}
static __attribute__((unused)) uint64_t spx_box_f64_bits(double value) {
    union { double value; uint64_t bits; } carrier = { .value = value };
    return carrier.bits;
}
static __attribute__((unused)) float spx_box_bits_f32(uint64_t bits) {
    union { float value; uint32_t bits; } carrier = { .bits = (uint32_t)bits };
    return carrier.value;
}
static __attribute__((unused)) double spx_box_bits_f64(uint64_t bits) {
    union { double value; uint64_t bits; } carrier = { .bits = bits };
    return carrier.value;
}

static __attribute__((unused)) struct spx_box_authority_entry *spx_box_require_valid(
    struct spx_context *spx_ctx, const spx_box_v1 *value, uint32_t tag
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || value == NULL
        || tag < UINT32_C(1) || tag > UINT32_C(8) || value->ptr == NULL
        || value->type_tag != tag || value->generation == UINT64_C(0)
        || value->authority == UINT32_C(0)
        || value->authority > SPX_BOX_AUTHORITY_CAPACITY) {
        spx_runtime_invariant_failure("invalid owned bounded Box carrier");
    }
    struct spx_box_authority_entry *entry =
        &spx_ctx->box_authority[value->authority - UINT32_C(1)];
    if (!entry->live || entry->ptr != value->ptr || entry->generation != value->generation
        || entry->type_tag != value->type_tag) {
        spx_runtime_invariant_failure("stale or forged owned bounded Box carrier");
    }
    return entry;
}

static __attribute__((unused)) uint64_t spx_box_next_generation(struct spx_context *spx_ctx) {
    if (spx_ctx->box_next_generation == UINT64_C(0)
        || spx_ctx->box_next_generation == UINT64_MAX) {
        spx_runtime_invariant_failure("bounded Box generation authority exhausted");
    }
    return spx_ctx->box_next_generation++;
}

static __attribute__((unused)) uint32_t spx_box_register(
    struct spx_context *spx_ctx, uint64_t *ptr, uint32_t tag, uint64_t generation
) {
    for (uint32_t index = UINT32_C(0); index < SPX_BOX_AUTHORITY_CAPACITY; ++index) {
        struct spx_box_authority_entry *entry = &spx_ctx->box_authority[index];
        if (!entry->live) {
            entry->ptr = ptr; entry->generation = generation; entry->type_tag = tag;
            entry->live = true; return index + UINT32_C(1);
        }
    }
    return UINT32_C(0);
}

static __attribute__((unused)) spx_status_token spx_box_failure(struct spx_context *spx_ctx) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, "semaprax.box.v1", UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token)) {
        spx_runtime_invariant_failure("bounded Box status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_box_new(
    struct spx_context *spx_ctx, uint32_t tag, uint64_t bits, spx_box_v1 *result
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || result == NULL
        || tag < UINT32_C(1) || tag > UINT32_C(8)) {
        spx_runtime_invariant_failure("invalid bounded Box allocation request");
    }
    *result = (spx_box_v1){0};
    uint64_t *payload = (uint64_t *)malloc(sizeof(uint64_t));
    if (payload == NULL) return spx_box_failure(spx_ctx);
    uint64_t generation = spx_box_next_generation(spx_ctx);
    uint32_t authority = spx_box_register(spx_ctx, payload, tag, generation);
    if (authority == UINT32_C(0)) { free(payload); return spx_box_failure(spx_ctx); }
    *payload = bits;
    result->ptr = payload; result->generation = generation; result->type_tag = tag;
    result->authority = authority;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) uint64_t spx_box_get(
    struct spx_context *spx_ctx, const spx_box_v1 *value, uint32_t tag
) {
    (void)spx_box_require_valid(spx_ctx, value, tag); return *value->ptr;
}

static __attribute__((unused)) uint64_t spx_box_into_inner(
    struct spx_context *spx_ctx, spx_box_v1 *value, uint32_t tag
) {
    struct spx_box_authority_entry *entry = spx_box_require_valid(spx_ctx, value, tag);
    uint64_t bits = *value->ptr; uint64_t *payload = value->ptr;
    *entry = (struct spx_box_authority_entry){0}; *value = (spx_box_v1){0};
    free(payload); return bits;
}

static __attribute__((unused)) spx_box_v1 spx_box_move(
    struct spx_context *spx_ctx, spx_box_v1 *source
) {
    if (source == NULL) spx_runtime_invariant_failure("invalid bounded Box move source");
    (void)spx_box_require_valid(spx_ctx, source, source->type_tag);
    spx_box_v1 moved = *source; *source = (spx_box_v1){0}; return moved;
}

static __attribute__((unused)) void spx_box_drop(
    struct spx_context *spx_ctx, spx_box_v1 *value
) {
    if (value == NULL) spx_runtime_invariant_failure("invalid bounded Box drop source");
    struct spx_box_authority_entry *entry = spx_box_require_valid(spx_ctx, value, value->type_tag);
    uint64_t *payload = value->ptr;
    *entry = (struct spx_box_authority_entry){0}; *value = (spx_box_v1){0}; free(payload);
}
"#;
