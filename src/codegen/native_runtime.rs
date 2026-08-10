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
        context->trace_generation == UINT64_C(0);
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
    entry->detail = (struct spx_status_detail){NULL, NULL, NULL, NULL};
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
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_BINARY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn emission_is_deterministic_and_self_contained() {
        let mut first = String::new();
        let mut second = String::new();
        emit_status_runtime(&mut first);
        emit_status_runtime(&mut second);

        assert_eq!(first, second);
        assert_eq!(first, STATUS_RUNTIME_C);
        assert!(first.starts_with("#include <stdbool.h>\n"));
        assert!(first.ends_with("}\n\n"));
    }

    #[test]
    fn emitted_abi_has_exact_status_identity_and_codes() {
        for marker in [
            "#define SPX_STATUS_SCHEMA_V1 \"semaprax.status.v1\"",
            "#define SPX_STATUS_SUCCESS UINT32_C(0)",
            "#define SPX_STATUS_CONTRACT_REQUIRES_FALSE UINT32_C(1)",
            "#define SPX_STATUS_CONTRACT_ENSURES_FALSE UINT32_C(2)",
            "#define SPX_STATUS_ARITHMETIC_ADD_OVERFLOW UINT32_C(1)",
            "#define SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW UINT32_C(8)",
            "#define SPX_STATUS_DOMAIN_MAX_BYTES UINT32_C(256)",
            "typedef uint32_t spx_status_token;",
            "typedef uint32_t spx_status_class;",
            "typedef uint32_t spx_retryability;",
            "const char *schema;",
            "const char *domain_id;",
            "struct spx_status_entry {",
            "char domain_storage[SPX_STATUS_DOMAIN_MAX_BYTES];",
        ] {
            assert!(STATUS_RUNTIME_C.contains(marker), "missing `{marker}`");
        }
        assert!(!STATUS_RUNTIME_C.contains("enum spx_status_class"));
        assert!(!STATUS_RUNTIME_C.contains("enum spx_retryability"));
    }

    #[test]
    fn emitted_arena_is_context_local_and_fail_closed() {
        for marker in [
            "uint64_t invocation_nonce;",
            "struct spx_status_arena status_arena;",
            "struct spx_trace_buffer *trace;",
            "context->trace = NULL;",
            "spx_context_is_canonical_zero(context)",
            "context->state = SPX_CONTEXT_INITIALIZED;",
            "context->generation = UINT64_C(1);",
            "arena->length >= arena->capacity",
            "token == SPX_STATUS_SUCCESS",
            "token - UINT32_C(1)",
            "status->code == 0",
            "status_capacity == 0 || status_capacity == UINT32_MAX",
            "spx_status_attach_detail(",
            "spx_status_resolve_detail(",
            "strcmp(domain_id, \"semaprax.contract.v1\") == 0",
            "strcmp(domain_id, \"semaprax.arithmetic.v1\") == 0",
        ] {
            assert!(STATUS_RUNTIME_C.contains(marker), "missing `{marker}`");
        }
        assert!(!STATUS_RUNTIME_C.contains("current_context"));
        assert!(!STATUS_RUNTIME_C.contains("malloc("));
        assert!(!STATUS_RUNTIME_C.contains("realloc("));
    }

    #[test]
    fn emitted_runtime_executes_the_status_arena_contract() {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }

        let suffix = NEXT_TEST_BINARY.fetch_add(1, Ordering::Relaxed);
        let extension = if cfg!(windows) { ".exe" } else { "" };
        let binary = std::env::temp_dir().join(format!(
            "semaprax-native-runtime-{}-{suffix}{extension}",
            std::process::id()
        ));
        let source = format!(
            "{STATUS_RUNTIME_C}{}",
            r#"int main(void) {
    struct spx_status_entry first_entries[2];
    struct spx_context first = {0};
    if (!spx_context_init(&first, UINT64_C(11), first_entries, UINT32_C(2), NULL, NULL, NULL)) return 1;
    if (first.state != SPX_CONTEXT_INITIALIZED || first.generation != UINT64_C(1) ||
        first.trace != NULL || first.trace_generation != UINT64_C(0)) return 39;
    if (spx_context_init(
            &first, UINT64_C(99), first_entries, UINT32_C(2),
            NULL, NULL, NULL)) return 40;
    if (first.invocation_nonce != UINT64_C(11) ||
        first.state != SPX_CONTEXT_INITIALIZED) return 41;
    if (spx_status_resolve(&first, SPX_STATUS_SUCCESS) != NULL) return 2;

    spx_status_token token = UINT32_C(99);
    if (!spx_status_record_requires_false(&first, &token) || token != UINT32_C(1)) return 3;
    const struct spx_normalized_status *requires = spx_status_resolve(&first, token);
    if (requires == NULL || strcmp(requires->schema, SPX_STATUS_SCHEMA_V1) != 0) return 4;
    if (strcmp(requires->domain_id, "semaprax.contract.v1") != 0 || requires->code != UINT32_C(1)) return 5;
    if (requires->status_class != SPX_STATUS_CLASS_CONTRACT || requires->retryability != SPX_RETRYABILITY_FALSE) return 6;
    struct spx_status_detail requires_detail = {"contract", "main", "value > 0", NULL};
    if (spx_status_resolve_detail(&first, token) != NULL) return 27;
    if (!spx_status_attach_detail(&first, token, requires_detail)) return 28;
    if (spx_status_attach_detail(&first, token, requires_detail)) return 29;

    if (!spx_status_record_arithmetic(&first, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, &token)) return 7;
    if (token != UINT32_C(2) || first.status_arena.length != UINT32_C(2)) return 8;
    struct spx_status_detail arithmetic_detail = {"arithmetic", "helper", NULL, "negation overflow"};
    if (!spx_status_attach_detail(&first, token, arithmetic_detail)) return 30;
    const struct spx_status_detail *first_detail = spx_status_resolve_detail(&first, UINT32_C(1));
    const struct spx_status_detail *second_detail = spx_status_resolve_detail(&first, UINT32_C(2));
    if (first_detail == NULL || second_detail == NULL || first_detail == second_detail) return 31;
    if (strcmp(first_detail->failure_expression, "value > 0") != 0) return 32;
    if (strcmp(second_detail->failure_operation, "negation overflow") != 0) return 33;
    token = UINT32_C(99);
    if (spx_status_record_ensures_false(&first, &token)) return 9;
    if (token != UINT32_C(99) || first.status_arena.length != UINT32_C(2)) return 10;
    if (spx_status_resolve(&first, UINT32_C(3)) != NULL) return 11;

    struct spx_status_entry second_entries[1];
    struct spx_context second = {0};
    if (!spx_context_init(&second, UINT64_C(11), second_entries, UINT32_C(1), NULL, NULL, NULL)) return 12;
    token = UINT32_C(88);
    if (spx_status_record_adapter(&second, "semaprax.contract.v1", UINT32_C(7), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_UNKNOWN, &token)) return 13;
    if (spx_status_record_adapter(&second, "host.error.v1", UINT32_C(7), SPX_STATUS_CLASS_CONTRACT, SPX_RETRYABILITY_UNKNOWN, &token)) return 14;
    if (spx_status_record_adapter(&second, "host.error.v1", UINT32_C(0), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_UNKNOWN, &token)) return 15;
    if (token != UINT32_C(88) || second.status_arena.length != UINT32_C(0)) return 16;
    char stack_domain[] = "host.error.v1";
    if (!spx_status_record_adapter(&second, stack_domain, UINT32_C(7), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_UNKNOWN, &token)) return 17;
    if (token != UINT32_C(1)) return 18;
    stack_domain[0] = 'X';

    const struct spx_normalized_status *first_one = spx_status_resolve(&first, UINT32_C(1));
    const struct spx_normalized_status *second_one = spx_status_resolve(&second, UINT32_C(1));
    if (first_one == NULL || second_one == NULL) return 19;
    if (strcmp(first_one->domain_id, "semaprax.contract.v1") != 0) return 20;
    if (strcmp(second_one->domain_id, "host.error.v1") != 0) return 21;

    struct spx_context invalid = {0};
    struct spx_status_entry invalid_entries[1];
    if (spx_context_init(&invalid, UINT64_C(12), invalid_entries, UINT32_C(0), NULL, NULL, NULL)) return 22;
    if (spx_context_init(&invalid, UINT64_C(12), NULL, UINT32_C(1), NULL, NULL, NULL)) return 23;
    if (spx_context_init(&invalid, UINT64_C(12), invalid_entries, UINT32_MAX, NULL, NULL, NULL)) return 24;
    if (!spx_context_init(&invalid, UINT64_C(12), invalid_entries, UINT32_C(1), NULL, NULL, NULL)) return 25;
    char unterminated_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
    memset(unterminated_domain, 'x', sizeof(unterminated_domain));
    token = UINT32_C(77);
    if (spx_status_record_adapter(&invalid, unterminated_domain, UINT32_C(1), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_FALSE, &token)) return 26;
    if (token != UINT32_C(77) || invalid.status_arena.length != UINT32_C(0)) return 34;
    char bad_continuation[] = {(char)0xc3, (char)0x28, '\0'};
    char overlong_encoding[] = {(char)0xc0, (char)0xaf, '\0'};
    char surrogate_encoding[] = {(char)0xed, (char)0xa0, (char)0x80, '\0'};
    char above_unicode_max[] = {(char)0xf4, (char)0x90, (char)0x80, (char)0x80, '\0'};
    const char *invalid_utf8[] = {
        bad_continuation,
        overlong_encoding,
        surrogate_encoding,
        above_unicode_max
    };
    for (size_t index = 0; index < sizeof(invalid_utf8) / sizeof(invalid_utf8[0]); ++index) {
        if (spx_status_record_adapter(&invalid, invalid_utf8[index], UINT32_C(1), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_FALSE, &token)) return 37;
        if (token != UINT32_C(77) || invalid.status_arena.length != UINT32_C(0)) return 38;
    }
    char maximum_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
    memset(maximum_domain, 'm', sizeof(maximum_domain));
    maximum_domain[SPX_STATUS_DOMAIN_MAX_BYTES - UINT32_C(1)] = '\0';
    if (!spx_status_record_adapter(&invalid, maximum_domain, UINT32_C(1), SPX_STATUS_CLASS_IMPORT, SPX_RETRYABILITY_FALSE, &token)) return 35;
    const struct spx_normalized_status *maximum = spx_status_resolve(&invalid, token);
    if (maximum == NULL || strlen(maximum->domain_id) != SPX_STATUS_DOMAIN_MAX_BYTES - UINT32_C(1)) return 36;
    return 0;
}
"#
        );

        let mut compiler = Command::new("clang")
            .args(["-x", "c", "-std=c11", "-Wall", "-Wextra", "-Werror"])
            .arg("-o")
            .arg(&binary)
            .arg("-")
            .stdin(Stdio::piped())
            .spawn()
            .expect("clang was available during the version probe");
        compiler
            .stdin
            .as_mut()
            .expect("piped clang stdin exists")
            .write_all(source.as_bytes())
            .expect("write generated C to clang");
        let compiled = compiler.wait().expect("wait for clang");
        assert!(compiled.success(), "generated status runtime must compile");

        let executed = Command::new(&binary)
            .status()
            .expect("run generated status runtime fixture");
        let cleanup = std::fs::remove_file(&binary);
        assert!(executed.success(), "status arena fixture exited {executed}");
        cleanup.expect("remove generated status runtime fixture");
    }
}
