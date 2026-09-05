//! Deterministic C11 scaffolding for the native status/out ABI.
//!
//! This module deliberately emits only the invocation context and normalized
//! status arena. Cleanup-plan lowering remains gated in `codegen.rs`; adding
//! this scaffold does not make resource-bearing programs executable.

/// Append the target-independent native status runtime to generated C.
///
/// The emitted arena is caller-provided, invocation-local storage. Token zero
/// is success, nonzero tokens are immutable one-based record indices, and
/// allocation failure is reported outside the language status-token channel.
pub(super) fn emit_status_runtime(output: &mut impl super::COutput) {
    output.push_str(STATUS_RUNTIME_C);
}

/// Emit the additive borrowed-text context extension. The ordinary emitter is
/// intentionally byte-frozen for every pre-text native projection.
pub(super) fn emit_status_runtime_with_borrowed_str(output: &mut impl super::COutput) {
    let runtime = STATUS_RUNTIME_C
        .replacen(
            "    uint64_t trace_generation;\n};",
            "    uint64_t trace_generation;\n    uint32_t borrowed_str_depth;\n};",
            1,
        )
        .replacen(
            "        context->trace_generation == UINT64_C(0);",
            "        context->trace_generation == UINT64_C(0) &&\n        context->borrowed_str_depth == UINT32_C(0);",
            1,
        )
        .replacen(
            "    context->trace_generation = UINT64_C(0);\n    return true;",
            "    context->trace_generation = UINT64_C(0);\n    context->borrowed_str_depth = UINT32_C(0);\n    return true;",
            1,
        );
    output.push_str(&runtime);
}

const STATUS_RUNTIME_C: &str = r#"#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define SPX_STATUS_SCHEMA_V1 "semaprax.status.v1"
#define SPX_STATUS_SUCCESS UINT32_C(0)
#define SPX_STATUS_CONTRACT_REQUIRES_FALSE UINT32_C(1)
#define SPX_STATUS_CONTRACT_ENSURES_FALSE UINT32_C(2)
#define SPX_STATUS_ARITHMETIC_ADD_OVERFLOW UINT32_C(1)
#define SPX_STATUS_ARITHMETIC_SUB_OVERFLOW UINT32_C(2)
#define SPX_STATUS_ARITHMETIC_MUL_OVERFLOW UINT32_C(3)
#define SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO UINT32_C(4)
#define SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW UINT32_C(5)
#define SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO UINT32_C(6)
#define SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW UINT32_C(7)
#define SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW UINT32_C(8)
/* Includes the terminating NUL. Longer domain IDs are rejected before append. */
#define SPX_STATUS_DOMAIN_MAX_BYTES UINT32_C(256)

typedef uint32_t spx_status_token;
typedef uint32_t spx_status_class;
typedef uint32_t spx_retryability;

#define SPX_STATUS_CLASS_CONTRACT UINT32_C(1)
#define SPX_STATUS_CLASS_ARITHMETIC UINT32_C(2)
#define SPX_STATUS_CLASS_IMPORT UINT32_C(3)
#define SPX_STATUS_CLASS_EXPLICIT_CLOSE UINT32_C(4)
#define SPX_STATUS_CLASS_ADAPTER UINT32_C(5)

#define SPX_RETRYABILITY_UNKNOWN UINT32_C(0)
#define SPX_RETRYABILITY_FALSE UINT32_C(1)
#define SPX_RETRYABILITY_TRUE UINT32_C(2)

#define SPX_CONTEXT_ZERO UINT32_C(0)
#define SPX_CONTEXT_INITIALIZED UINT32_C(0x53505843)
#define SPX_CONTEXT_TRACE_ATTACHED UINT32_C(0x53505854)
#define SPX_MAX_CALL_DEPTH UINT32_C(256)
#define SPX_STATUS_ARGUMENTS_MAX_BYTES UINT32_C(1024)

struct spx_normalized_status {
    const char *schema;
    const char *domain_id;
    uint32_t code;
    spx_status_class status_class;
    spx_retryability retryability;
};

struct spx_status_detail {
    const char *failure_kind;
    const char *failure_function;
    const char *failure_expression;
    const char *failure_operation;
    char failure_arguments[SPX_STATUS_ARGUMENTS_MAX_BYTES];
};

struct spx_status_entry {
    struct spx_normalized_status status;
    struct spx_status_detail detail;
    char domain_storage[SPX_STATUS_DOMAIN_MAX_BYTES];
    bool detail_attached;
};

struct spx_status_arena {
    struct spx_status_entry *entries;
    uint32_t capacity;
    uint32_t length;
};

struct spx_import_table;
struct spx_capability_table;
struct spx_trace_buffer;

struct spx_context {
    uint32_t state;
    uint64_t generation;
    uint64_t invocation_nonce;
    struct spx_status_arena status_arena;
    const struct spx_import_table *imports;
    const struct spx_capability_table *capabilities;
    void *target_state;
    struct spx_trace_buffer *trace;
    uint64_t trace_generation;
    uint32_t call_depth;
};

static inline bool spx_context_is_canonical_zero(
    const struct spx_context *context
) {
    return context != NULL && context->state == SPX_CONTEXT_ZERO &&
        context->generation == UINT64_C(0) &&
        context->invocation_nonce == UINT64_C(0) &&
        context->status_arena.entries == NULL &&
        context->status_arena.capacity == UINT32_C(0) &&
        context->status_arena.length == UINT32_C(0) &&
        context->imports == NULL && context->capabilities == NULL &&
        context->target_state == NULL && context->trace == NULL &&
        context->trace_generation == UINT64_C(0) &&
        context->call_depth == UINT32_C(0);
}

static inline bool spx_context_init(
    struct spx_context *context,
    uint64_t invocation_nonce,
    struct spx_status_entry *status_entries,
    uint32_t status_capacity,
    const struct spx_import_table *imports,
    const struct spx_capability_table *capabilities,
    void *target_state
) {
    /* Context storage is one-shot and must be canonically zero-initialized.
       Reading an indeterminate C object would itself be undefined behavior. */
    if (!spx_context_is_canonical_zero(context) ||
        status_entries == NULL ||
        status_capacity == 0 || status_capacity == UINT32_MAX) {
        return false;
    }
    context->state = SPX_CONTEXT_INITIALIZED;
    context->generation = UINT64_C(1);
    context->invocation_nonce = invocation_nonce;
    context->status_arena.entries = status_entries;
    context->status_arena.capacity = status_capacity;
    context->status_arena.length = 0;
    context->imports = imports;
    context->capabilities = capabilities;
    context->target_state = target_state;
    context->trace = NULL;
    context->trace_generation = UINT64_C(0);
    context->call_depth = UINT32_C(0);
    return true;
}

static inline bool spx_status_domain_size(
    const char *domain_id,
    size_t *size_out
) {
    if (domain_id == NULL || size_out == NULL) {
        return false;
    }
    for (size_t length = 0; length < (size_t)SPX_STATUS_DOMAIN_MAX_BYTES; ++length) {
        if (domain_id[length] == '\0') {
            if (length == 0) {
                return false;
            }
            *size_out = length + 1;
            return true;
        }
    }
    return false;
}

static inline bool spx_status_domain_is_utf8(
    const char *domain_id,
    size_t byte_length
) {
    const unsigned char *bytes = (const unsigned char *)domain_id;
    size_t index = 0;
    while (index < byte_length) {
        unsigned char first = bytes[index];
        if (first <= UINT8_C(0x7f)) {
            index += 1;
            continue;
        }
        if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            if (index + 1 >= byte_length ||
                bytes[index + 1] < UINT8_C(0x80) ||
                bytes[index + 1] > UINT8_C(0xbf)) return false;
            index += 2;
            continue;
        }
        if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            if (index + 2 >= byte_length) return false;
            unsigned char second = bytes[index + 1];
            unsigned char third = bytes[index + 2];
            if (third < UINT8_C(0x80) || third > UINT8_C(0xbf)) return false;
            if (first == UINT8_C(0xe0)) {
                if (second < UINT8_C(0xa0) || second > UINT8_C(0xbf)) return false;
            } else if (first == UINT8_C(0xed)) {
                if (second < UINT8_C(0x80) || second > UINT8_C(0x9f)) return false;
            } else if (second < UINT8_C(0x80) || second > UINT8_C(0xbf)) {
                return false;
            }
            index += 3;
            continue;
        }
        if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            if (index + 3 >= byte_length) return false;
            unsigned char second = bytes[index + 1];
            unsigned char third = bytes[index + 2];
            unsigned char fourth = bytes[index + 3];
            if (third < UINT8_C(0x80) || third > UINT8_C(0xbf) ||
                fourth < UINT8_C(0x80) || fourth > UINT8_C(0xbf)) return false;
            if (first == UINT8_C(0xf0)) {
                if (second < UINT8_C(0x90) || second > UINT8_C(0xbf)) return false;
            } else if (first == UINT8_C(0xf4)) {
                if (second < UINT8_C(0x80) || second > UINT8_C(0x8f)) return false;
            } else if (second < UINT8_C(0x80) || second > UINT8_C(0xbf)) {
                return false;
            }
            index += 4;
            continue;
        }
        return false;
    }
    return true;
}

static inline bool spx_status_shape_is_valid(
    const struct spx_normalized_status *status
) {
    if (status == NULL || status->schema == NULL || status->domain_id == NULL) {
        return false;
    }
    if (strcmp(status->schema, SPX_STATUS_SCHEMA_V1) != 0 ||
        status->domain_id[0] == '\0' || status->code == 0) {
        return false;
    }
    if (status->status_class < SPX_STATUS_CLASS_CONTRACT ||
        status->status_class > SPX_STATUS_CLASS_ADAPTER) {
        return false;
    }
    size_t domain_size = 0;
    return status->retryability <= SPX_RETRYABILITY_TRUE &&
        spx_status_domain_size(status->domain_id, &domain_size) &&
        spx_status_domain_is_utf8(status->domain_id, domain_size - 1);
}

static inline bool spx_status_arena_push(
    struct spx_context *context,
    struct spx_normalized_status status,
    spx_status_token *token_out
) {
    if (context == NULL || token_out == NULL ||
        !spx_status_shape_is_valid(&status)) {
        return false;
    }
    struct spx_status_arena *arena = &context->status_arena;
    if (arena->length >= arena->capacity || arena->entries == NULL) {
        return false;
    }
    size_t domain_size = 0;
    if (!spx_status_domain_size(status.domain_id, &domain_size)) {
        return false;
    }
    uint32_t index = arena->length;
    spx_status_token token = index + UINT32_C(1);
    struct spx_status_entry *entry = &arena->entries[index];
    memcpy(entry->domain_storage, status.domain_id, domain_size);
    entry->status = status;
    entry->status.schema = SPX_STATUS_SCHEMA_V1;
    entry->status.domain_id = entry->domain_storage;
    entry->detail = (struct spx_status_detail){
        .failure_kind = NULL,
        .failure_function = NULL,
        .failure_expression = NULL,
        .failure_operation = NULL,
        .failure_arguments = {0}
    };
    entry->detail_attached = false;
    arena->length = index + UINT32_C(1);
    *token_out = token;
    return true;
}

static inline const struct spx_normalized_status *spx_status_resolve(
    const struct spx_context *context,
    spx_status_token token
) {
    if (context == NULL || token == SPX_STATUS_SUCCESS ||
        token > context->status_arena.length ||
        context->status_arena.entries == NULL) {
        return NULL;
    }
    return &context->status_arena.entries[token - UINT32_C(1)].status;
}

static inline bool spx_status_attach_detail(
    struct spx_context *context,
    spx_status_token token,
    struct spx_status_detail detail
) {
    if (context == NULL || token == SPX_STATUS_SUCCESS ||
        token > context->status_arena.length ||
        context->status_arena.entries == NULL) {
        return false;
    }
    struct spx_status_entry *entry =
        &context->status_arena.entries[token - UINT32_C(1)];
    if (entry->detail_attached) {
        return false;
    }
    entry->detail = detail;
    entry->detail_attached = true;
    return true;
}

static inline const struct spx_status_detail *spx_status_resolve_detail(
    const struct spx_context *context,
    spx_status_token token
) {
    if (context == NULL || token == SPX_STATUS_SUCCESS ||
        token > context->status_arena.length ||
        context->status_arena.entries == NULL) {
        return NULL;
    }
    const struct spx_status_entry *entry =
        &context->status_arena.entries[token - UINT32_C(1)];
    return entry->detail_attached ? &entry->detail : NULL;
}

static inline bool spx_status_record_compiler(
    struct spx_context *context,
    const char *domain_id,
    uint32_t code,
    spx_status_class status_class,
    spx_status_token *token_out
) {
    struct spx_normalized_status status = {
        SPX_STATUS_SCHEMA_V1,
        domain_id,
        code,
        status_class,
        SPX_RETRYABILITY_FALSE
    };
    return spx_status_arena_push(context, status, token_out);
}

static inline bool spx_status_record_requires_false(
    struct spx_context *context,
    spx_status_token *token_out
) {
    return spx_status_record_compiler(
        context,
        "semaprax.contract.v1",
        SPX_STATUS_CONTRACT_REQUIRES_FALSE,
        SPX_STATUS_CLASS_CONTRACT,
        token_out
    );
}

static inline bool spx_status_record_ensures_false(
    struct spx_context *context,
    spx_status_token *token_out
) {
    return spx_status_record_compiler(
        context,
        "semaprax.contract.v1",
        SPX_STATUS_CONTRACT_ENSURES_FALSE,
        SPX_STATUS_CLASS_CONTRACT,
        token_out
    );
}

static inline bool spx_status_record_arithmetic(
    struct spx_context *context,
    uint32_t code,
    spx_status_token *token_out
) {
    if (code < SPX_STATUS_ARITHMETIC_ADD_OVERFLOW ||
        code > SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW) {
        return false;
    }
    return spx_status_record_compiler(
        context,
        "semaprax.arithmetic.v1",
        code,
        SPX_STATUS_CLASS_ARITHMETIC,
        token_out
    );
}

static inline __attribute__((unused)) bool spx_status_record_adapter(
    struct spx_context *context,
    const char *domain_id,
    uint32_t code,
    spx_status_class status_class,
    spx_retryability retryability,
    spx_status_token *token_out
) {
    if (domain_id == NULL || domain_id[0] == '\0' || code == 0 ||
        status_class < SPX_STATUS_CLASS_IMPORT ||
        status_class > SPX_STATUS_CLASS_ADAPTER ||
        retryability > SPX_RETRYABILITY_TRUE) {
        return false;
    }
    if (strcmp(domain_id, "semaprax.contract.v1") == 0 ||
        strcmp(domain_id, "semaprax.arithmetic.v1") == 0) {
        return false;
    }
    struct spx_normalized_status status = {
        SPX_STATUS_SCHEMA_V1,
        domain_id,
        code,
        status_class,
        retryability
    };
    return spx_status_arena_push(context, status, token_out);
}

"#;

#[cfg(test)]
#[path = "native_runtime/tests.rs"]
mod tests;
