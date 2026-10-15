#include <stdbool.h>
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

#include <stdio.h>

#include <stdlib.h>

static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(
    const char *message
) {
    fprintf(stderr, "SEMAPRAX native runtime invariant failure: %s\n", message);
    abort();
}

static __attribute__((unused)) spx_status_token spx_rt_arithmetic_failure(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *operation
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_arithmetic(spx_ctx, code, &token)) {
        spx_runtime_invariant_failure("status arena exhaustion");
    }
    struct spx_status_detail detail = {
        .failure_kind = NULL,
        .failure_function = NULL,
        .failure_expression = NULL,
        .failure_operation = operation,
        .failure_arguments = {0}
    };
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("arithmetic status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract_with_arguments(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression,
    const char *arguments
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    bool recorded = code == SPX_STATUS_CONTRACT_REQUIRES_FALSE
        ? spx_status_record_requires_false(spx_ctx, &token)
        : code == SPX_STATUS_CONTRACT_ENSURES_FALSE
            ? spx_status_record_ensures_false(spx_ctx, &token)
            : false;
    if (!recorded) spx_runtime_invariant_failure("status arena exhaustion");
    struct spx_status_detail detail = {
        .failure_kind = kind,
        .failure_function = function,
        .failure_expression = expression,
        .failure_operation = NULL,
        .failure_arguments = {0}
    };
    int arguments_written = snprintf(
        detail.failure_arguments,
        sizeof detail.failure_arguments,
        "%s",
        arguments
    );
    if (arguments_written < 0 ||
        (size_t)arguments_written >= sizeof detail.failure_arguments) {
        spx_runtime_invariant_failure("contract argument detail overflow");
    }
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("contract status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression
) {
    return spx_rt_contract_with_arguments(
        spx_ctx, code, kind, function, expression, "none"
    );
}

static __attribute__((unused)) spx_status_token spx_rt_call_depth_failure(
    struct spx_context *spx_ctx
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.runtime.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("call-depth status arena exhaustion");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_add(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_add_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_sub_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_mul_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_add_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a + (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a - (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a * (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg_i32(
    struct spx_context *spx_ctx, int32_t value, int32_t *result_out
) {
    if (value == INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg(
    struct spx_context *spx_ctx, int64_t value, int64_t *result_out
) {
    if (value == INT64_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) int spx_public_failure(
    const struct spx_context *spx_ctx,
    spx_status_token token
) {
    const struct spx_normalized_status *status = spx_status_resolve(spx_ctx, token);
    if (status == NULL) {
        fputs("SEMAPRAX native runtime invariant failure: invalid status token\n", stderr);
        return 72;
    }
    const struct spx_status_detail *detail = spx_status_resolve_detail(spx_ctx, token);
    if (status->status_class == SPX_STATUS_CLASS_CONTRACT) {
        if (detail == NULL || detail->failure_kind == NULL ||
            detail->failure_function == NULL || detail->failure_expression == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing contract detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX contract failure\n  contract: %s %s in %s\n  arguments: %s\n",
            detail->failure_kind,
            detail->failure_expression,
            detail->failure_function,
            detail->failure_arguments
        );
        return 70;
    }
    if (status->status_class == SPX_STATUS_CLASS_ARITHMETIC) {
        if (detail == NULL || detail->failure_operation == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing arithmetic detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX checked arithmetic failure: %s\n",
            detail->failure_operation
        );
        return 71;
    }
    if (strcmp(status->domain_id, "semaprax.runtime.v1") == 0 &&
        status->code == UINT32_C(1)) {
        fprintf(
            stderr,
            "SEMAPRAX runtime failure: call depth exceeded (%u frames)\n",
            (unsigned int)SPX_MAX_CALL_DEPTH
        );
        return 73;
    }
    const char *conversion_help = "";
    if (strcmp(status->domain_id, "semaprax.convert.v1") == 0) {
        if (status->code == UINT32_C(1)) conversion_help = " (conversion out of range)";
        else if (status->code == UINT32_C(2)) conversion_help = " (conversion of NaN)";
    }
    fprintf(
        stderr,
        "SEMAPRAX operation failure: %s/%u%s\n",
        status->domain_id,
        status->code,
        conversion_help
    );
    return 73;
}

static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e6d61696e(struct spx_context *spx_ctx, int64_t *spx_result_out);
static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e746573745f615f7265667573616c(struct spx_context *spx_ctx, int64_t *spx_result_out);
static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e746573745f7a5f6166746572(struct spx_context *spx_ctx, int64_t *spx_result_out);

static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e6d61696e(struct spx_context *spx_ctx, int64_t *spx_result_out) {
    spx_status_token spx_status = SPX_STATUS_SUCCESS;
    bool spx_call_entered = false;
    (void)spx_ctx;
    int64_t spx_result = {0};
    if (spx_ctx->call_depth >= SPX_MAX_CALL_DEPTH) {
        spx_status = spx_rt_call_depth_failure(spx_ctx);
        goto spx_epilogue;
    }
    ++spx_ctx->call_depth;
    spx_call_entered = true;
    spx_result = INT64_C(0);
    goto spx_epilogue;
spx_epilogue:
    if (spx_call_entered) {
        if (spx_ctx->call_depth == UINT32_C(0)) spx_runtime_invariant_failure("call depth underflow");
        --spx_ctx->call_depth;
    }
    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;
    *spx_result_out = spx_result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e746573745f615f7265667573616c(struct spx_context *spx_ctx, int64_t *spx_result_out) {
    spx_status_token spx_status = SPX_STATUS_SUCCESS;
    bool spx_call_entered = false;
    (void)spx_ctx;
    int64_t spx_result = {0};
    if (spx_ctx->call_depth >= SPX_MAX_CALL_DEPTH) {
        spx_status = spx_rt_call_depth_failure(spx_ctx);
        goto spx_epilogue;
    }
    ++spx_ctx->call_depth;
    spx_call_entered = true;
    for (;;) {
        if (!(true)) break;
        (void)(INT64_C(0));
    }
    spx_result = INT64_C(0);
    goto spx_epilogue;
spx_epilogue:
    if (spx_call_entered) {
        if (spx_ctx->call_depth == UINT32_C(0)) spx_runtime_invariant_failure("call depth underflow");
        --spx_ctx->call_depth;
    }
    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;
    *spx_result_out = spx_result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_decl_646563696d616c2e74657374732e746573745f7a5f6166746572(struct spx_context *spx_ctx, int64_t *spx_result_out) {
    spx_status_token spx_status = SPX_STATUS_SUCCESS;
    bool spx_call_entered = false;
    (void)spx_ctx;
    int64_t spx_result = {0};
    if (spx_ctx->call_depth >= SPX_MAX_CALL_DEPTH) {
        spx_status = spx_rt_call_depth_failure(spx_ctx);
        goto spx_epilogue;
    }
    ++spx_ctx->call_depth;
    spx_call_entered = true;
    spx_result = INT64_C(0);
    goto spx_epilogue;
spx_epilogue:
    if (spx_call_entered) {
        if (spx_ctx->call_depth == UINT32_C(0)) spx_runtime_invariant_failure("call depth underflow");
        --spx_ctx->call_depth;
    }
    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;
    *spx_result_out = spx_result;
    return SPX_STATUS_SUCCESS;
}

#ifndef SPX_NO_ENTRY_WRAPPER
#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif
int main(void) {
struct spx_status_entry spx_status_entries[UINT32_C(1)];
struct spx_context spx_ctx = {0};
if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {
fputs("SEMAPRAX native runtime invariant failure: context initialization\n", stderr);
return 72;
}
int64_t result;
spx_status_token status = spx_decl_646563696d616c2e74657374732e746573745f7a5f6166746572(&spx_ctx, &result);
if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);
#if defined(_WIN32)
if (_setmode(_fileno(stdout), _O_BINARY) == -1) {
fputs("SEMAPRAX native runtime invariant failure: stdout binary mode\n", stderr);
return 72;
}
#endif
printf("%lld\n", (long long)result);
return 0;
}
#endif
