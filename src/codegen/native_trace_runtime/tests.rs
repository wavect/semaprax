use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_TEST_BINARY: AtomicU64 = AtomicU64::new(0);

#[test]
fn emission_is_deterministic_and_defines_every_v1_event_tag() {
    let mut first = String::new();
    let mut second = String::new();
    emit_trace_runtime(&mut first);
    emit_trace_runtime(&mut second);

    assert_eq!(first, second);
    assert_eq!(first, TRACE_RUNTIME_C);
    for marker in [
        "#define SPX_CONFORMANCE_TRACE_SCHEMA_V1 \"semaprax.conformance-trace.v1\"",
        "#define SPX_TRACE_INITIALIZE UINT32_C(1)",
        "#define SPX_TRACE_TRANSFER UINT32_C(2)",
        "#define SPX_TRACE_CALL_COMMIT UINT32_C(3)",
        "#define SPX_TRACE_IMPORT_BEGIN UINT32_C(4)",
        "#define SPX_TRACE_IMPORT_END UINT32_C(5)",
        "#define SPX_TRACE_SELECT_FAILURE UINT32_C(6)",
        "#define SPX_TRACE_FINALIZE_BEGIN UINT32_C(7)",
        "#define SPX_TRACE_FINALIZE_END UINT32_C(8)",
        "#define SPX_TRACE_RESULT_COMMIT UINT32_C(9)",
        "struct spx_trace_place_descriptor {",
        "struct spx_trace_status_source_descriptor {",
        "struct spx_trace_normalized_status {",
        "struct spx_trace_call_argument {",
        "struct spx_trace_import_site {",
        "struct spx_trace_result_source {",
        "event->invocation_expression_ids != NULL",
        "case SPX_TRACE_STORAGE_CALL_ARGUMENT:",
        "case SPX_TRACE_CALL_COMMIT:",
        "case SPX_TRACE_IMPORT_BEGIN:",
        "case SPX_TRACE_IMPORT_END:",
        "spx_trace_event_shape_is_valid(event)",
        "#define SPX_TRACE_BUFFER_READY UINT32_C(0x53505852)",
        "#define SPX_TRACE_BUFFER_ATTACHED UINT32_C(0x53505841)",
        "spx_trace_attach_preflight(",
        "context->trace != NULL",
        "context->trace->state != SPX_TRACE_BUFFER_ATTACHED",
        "event->storage_owner != NULL",
        "slot->storage_owner != buffer",
        "context->trace->owner_context != context",
        "context->state != SPX_CONTEXT_TRACE_ATTACHED",
    ] {
        assert!(first.contains(marker), "missing `{marker}`");
    }
}

#[test]
fn emitted_storage_excludes_physical_payload_and_status_token_fields() {
    for forbidden in [
        "status_token",
        "uintptr_t",
        "payload",
        "result_out",
        "stack_offset",
        "context_nonce",
        "host_exception",
    ] {
        assert!(!TRACE_RUNTIME_C.contains(forbidden), "found `{forbidden}`");
    }
    assert!(!TRACE_RUNTIME_C.contains("malloc("));
    assert!(!TRACE_RUNTIME_C.contains("realloc("));
    assert!(!TRACE_RUNTIME_C.contains("memmove("));
    assert!(!TRACE_RUNTIME_C.contains("qsort("));
}

#[test]
fn emitted_runtime_compiles_preserves_order_and_aborts_on_overflow() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }

    let suffix = NEXT_TEST_BINARY.fetch_add(1, Ordering::Relaxed);
    let binary = std::env::temp_dir().join(format!(
        "semaprax-native-trace-runtime-{}-{suffix}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    let mut combined_runtime = String::new();
    super::super::native_runtime::emit_status_runtime(&mut combined_runtime);
    emit_trace_runtime(&mut combined_runtime);
    let source = format!(
        "{combined_runtime}{}",
        r#"#include <string.h>

static struct spx_trace_event event(
    spx_trace_event_kind kind,
    const char *function_id
) {
    struct spx_trace_event value = {0};
    value.kind = kind;
    value.function_id = function_id;
    return value;
}

static struct spx_trace_event failure_event(
    const char *domain_id,
    uint32_t code,
    spx_trace_status_class status_class,
    spx_trace_retryability retryability
) {
    struct spx_trace_event failure = event(
        SPX_TRACE_SELECT_FAILURE, "app.main"
    );
    failure.data.select_failure.source.expression_id = "expression.failure";
    failure.data.select_failure.source.lane =
        SPX_TRACE_STATUS_OPERATION_FAILURE;
    failure.data.select_failure.status.schema = "semaprax.status.v1";
    failure.data.select_failure.status.domain_id = domain_id;
    failure.data.select_failure.status.code = code;
    failure.data.select_failure.status.status_class = status_class;
    failure.data.select_failure.status.retryability = retryability;
    return failure;
}

static void retain_combined_status_runtime(void) {
    (void)spx_status_resolve;
    (void)spx_status_attach_detail;
    (void)spx_status_resolve_detail;
    (void)spx_status_record_requires_false;
    (void)spx_status_record_ensures_false;
    (void)spx_status_record_arithmetic;
}

int main(int argc, char **argv) {
    retain_combined_status_runtime();
    /* The original seven-argument scalar initializer remains the only context
       construction API. Trace attachment is a separate preflight. */
    struct spx_status_entry status_entries[UINT32_C(1)];
    struct spx_context context = {0};
    if (!spx_context_init(
            &context, UINT64_C(41), status_entries, UINT32_C(1),
            NULL, NULL, NULL)) return 9;
    if (context.trace != NULL) return 39;

    struct spx_trace_event entries[UINT32_C(2)] = {0};
    struct spx_trace_buffer buffer = {0};
    if (!spx_trace_buffer_init(&buffer, entries, UINT32_C(2))) return 10;
    if (buffer.length != UINT32_C(0)) return 11;

    if (argc == 2 && strcmp(argv[1], "insufficient-capacity") == 0) {
        struct spx_trace_event small_entries[UINT32_C(1)] = {0};
        struct spx_trace_buffer small = {0};
        if (!spx_trace_buffer_init(&small, small_entries, UINT32_C(1))) return 40;
        struct spx_trace_event *before_events = small.events;
        if (spx_trace_attach_preflight(&context, &small, UINT32_C(2))) return 41;
        if (context.trace != NULL || small.events != before_events ||
            small.capacity != UINT32_C(1) || small.length != UINT32_C(0) ||
            small.state != SPX_TRACE_BUFFER_READY) return 42;
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "not-attached") == 0) {
        struct spx_trace_event unattached = {0};
        unattached.kind = SPX_TRACE_INITIALIZE;
        unattached.function_id = "app.main";
        unattached.data.initialize.at_expression_id = "expression.initialize";
        unattached.data.initialize.destination.storage.kind =
            SPX_TRACE_STORAGE_TEMPORARY;
        unattached.data.initialize.destination.storage.expression_id =
            "expression.temporary";
        spx_trace_push(&context, &unattached);
        return 43;
    }
    if (argc == 2 && strcmp(argv[1], "alias-storage") == 0) {
        struct spx_trace_buffer alias = {0};
        if (spx_trace_buffer_init(&alias, entries, UINT32_C(2))) return 53;
        if (alias.state != UINT32_C(0) || alias.events != NULL ||
            alias.generation != UINT64_C(0) || context.trace != NULL ||
            entries[0].storage_owner != &buffer ||
            entries[1].storage_owner != &buffer) return 54;
        struct spx_status_entry alias_status_entries[UINT32_C(1)];
        struct spx_context alias_context = {0};
        if (!spx_context_init(
                &alias_context, UINT64_C(43), alias_status_entries,
                UINT32_C(1), NULL, NULL, NULL)) return 55;
        if (spx_trace_attach_preflight(
                &alias_context, &alias, UINT32_C(2))) return 56;
        if (alias_context.trace != NULL || context.trace != NULL) return 57;
        return 0;
    }
    if (!spx_trace_attach_preflight(&context, &buffer, UINT32_C(2))) return 44;
    if (context.trace != &buffer || buffer.state != SPX_TRACE_BUFFER_ATTACHED ||
        buffer.length != UINT32_C(0)) return 45;
    if (argc == 2 && strcmp(argv[1], "double-attach") == 0) {
        struct spx_trace_event replacement_entries[UINT32_C(2)] = {0};
        struct spx_trace_buffer replacement = {0};
        if (!spx_trace_buffer_init(
                &replacement, replacement_entries, UINT32_C(2))) return 46;
        if (spx_trace_attach_preflight(
                &context, &replacement, UINT32_C(2))) return 47;
        if (context.trace != &buffer || buffer.state != SPX_TRACE_BUFFER_ATTACHED ||
            buffer.length != UINT32_C(0) ||
            replacement.length != UINT32_C(0) ||
            replacement.state != SPX_TRACE_BUFFER_READY) return 48;
        struct spx_status_entry other_status_entries[UINT32_C(1)];
        struct spx_context other_context = {0};
        if (!spx_context_init(
                &other_context, UINT64_C(42), other_status_entries, UINT32_C(1),
                NULL, NULL, NULL)) return 50;
        if (spx_trace_attach_preflight(
                &other_context, &buffer, UINT32_C(2))) return 51;
        if (other_context.trace != NULL || context.trace != &buffer ||
            buffer.state != SPX_TRACE_BUFFER_ATTACHED ||
            buffer.length != UINT32_C(0)) return 52;
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "reinit-buffer") == 0) {
        if (spx_trace_buffer_init(&buffer, entries, UINT32_C(2))) return 58;
        if (context.trace != &buffer || buffer.state != SPX_TRACE_BUFFER_ATTACHED ||
            buffer.owner_context != &context ||
            buffer.owner_context_generation != context.generation ||
            entries[0].storage_owner != &buffer ||
            entries[1].storage_owner != &buffer) return 59;
        struct spx_status_entry next_status_entries[UINT32_C(1)];
        struct spx_context next_context = {0};
        if (!spx_context_init(
                &next_context, UINT64_C(44), next_status_entries,
                UINT32_C(1), NULL, NULL, NULL)) return 60;
        if (spx_trace_attach_preflight(
                &next_context, &buffer, UINT32_C(2))) return 61;
        if (next_context.trace != NULL || context.trace != &buffer) return 62;
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "reinit-context") == 0) {
        if (spx_context_init(
                &context, UINT64_C(45), status_entries, UINT32_C(1),
                NULL, NULL, NULL)) return 63;
        if (context.state != SPX_CONTEXT_TRACE_ATTACHED ||
            context.trace != &buffer ||
            context.trace_generation != buffer.generation ||
            buffer.owner_context != &context ||
            buffer.owner_context_generation != context.generation) return 64;
        return 0;
    }

    struct spx_trace_event initialize = event(SPX_TRACE_INITIALIZE, "app.main");
    initialize.data.initialize.at_expression_id = "expression.initialize";
    initialize.data.initialize.destination.storage.kind =
        SPX_TRACE_STORAGE_TEMPORARY;
    initialize.data.initialize.destination.storage.expression_id =
        "expression.temporary";

    if (argc == 2 && strcmp(argv[1], "invalid-kind") == 0) {
        initialize.kind = UINT32_C(99);
        spx_trace_push(&context, &initialize);
        return 20;
    }
    if (argc == 2 && strcmp(argv[1], "invocation-pointer") == 0) {
        static const char *const path[] = {"expression.call"};
        initialize.invocation_expression_ids = path;
        spx_trace_push(&context, &initialize);
        return 21;
    }
    if (argc == 2 && strcmp(argv[1], "invocation-count") == 0) {
        initialize.invocation_count = UINT32_C(1);
        spx_trace_push(&context, &initialize);
        return 22;
    }
    if (argc == 2 && strcmp(argv[1], "projection-count") == 0) {
        initialize.data.initialize.destination.projection_count = UINT32_C(1);
        spx_trace_push(&context, &initialize);
        return 23;
    }
    if (argc == 2 && strcmp(argv[1], "projection-pointer") == 0) {
        static const char *const fields[] = {"token.field"};
        initialize.data.initialize.destination.projection_ids = fields;
        spx_trace_push(&context, &initialize);
        return 24;
    }
    if (argc == 2 && strcmp(argv[1], "call-argument-storage") == 0) {
        initialize.data.initialize.destination.storage.kind =
            SPX_TRACE_STORAGE_CALL_ARGUMENT;
        initialize.data.initialize.destination.storage.call_id = "expression.call";
        initialize.data.initialize.destination.storage.value_expression_id =
            "expression.argument";
        spx_trace_push(&context, &initialize);
        return 25;
    }
    if (argc == 2 && strcmp(argv[1], "unsupported-import") == 0) {
        struct spx_trace_event import_event = event(
            SPX_TRACE_IMPORT_BEGIN, "app.main"
        );
        spx_trace_push(&context, &import_event);
        return 26;
    }
    if (argc == 2 && strcmp(argv[1], "valid-contract") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.contract.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_CONTRACT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return buffer.length == UINT32_C(1) ? 0 : 28;
    }
    if (argc == 2 && strcmp(argv[1], "valid-arithmetic") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.arithmetic.v1", UINT32_C(8),
            SPX_TRACE_STATUS_CLASS_ARITHMETIC, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return buffer.length == UINT32_C(1) ? 0 : 29;
    }
    if (argc == 2 && strcmp(argv[1], "invalid-status") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.contract.v1", UINT32_C(0),
            SPX_TRACE_STATUS_CLASS_CONTRACT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 27;
    }
    if (argc == 2 && strcmp(argv[1], "forged-class") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.contract.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_ADAPTER, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 30;
    }
    if (argc == 2 && strcmp(argv[1], "import-class") == 0) {
        struct spx_trace_event failure = failure_event(
            "host.error.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_IMPORT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 35;
    }
    if (argc == 2 && strcmp(argv[1], "explicit-close-class") == 0) {
        struct spx_trace_event failure = failure_event(
            "host.error.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_EXPLICIT_CLOSE,
            SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 36;
    }
    if (argc == 2 && strcmp(argv[1], "forged-domain") == 0) {
        struct spx_trace_event failure = failure_event(
            "host.error.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_CONTRACT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 31;
    }
    if (argc == 2 && strcmp(argv[1], "reserved-domain-mismatch") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.arithmetic.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_CONTRACT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 37;
    }
    if (argc == 2 && strcmp(argv[1], "unknown-contract-code") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.contract.v1", UINT32_C(3),
            SPX_TRACE_STATUS_CLASS_CONTRACT, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 32;
    }
    if (argc == 2 && strcmp(argv[1], "unknown-arithmetic-code") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.arithmetic.v1", UINT32_C(9),
            SPX_TRACE_STATUS_CLASS_ARITHMETIC, SPX_TRACE_RETRYABILITY_FALSE
        );
        spx_trace_push(&context, &failure);
        return 33;
    }
    if (argc == 2 && strcmp(argv[1], "retryable-compiler-status") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.arithmetic.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_ARITHMETIC, SPX_TRACE_RETRYABILITY_TRUE
        );
        spx_trace_push(&context, &failure);
        return 34;
    }
    if (argc == 2 && strcmp(argv[1], "unknown-retryability") == 0) {
        struct spx_trace_event failure = failure_event(
            "semaprax.contract.v1", UINT32_C(1),
            SPX_TRACE_STATUS_CLASS_CONTRACT,
            SPX_TRACE_RETRYABILITY_UNKNOWN
        );
        spx_trace_push(&context, &failure);
        return 38;
    }
    spx_trace_push(&context, &initialize);

    struct spx_trace_event finalize = event(SPX_TRACE_FINALIZE_END, "app.main");
    finalize.data.finalize.source.storage.kind = SPX_TRACE_STORAGE_VALUE;
    finalize.data.finalize.source.storage.value_id = "value.token";
    finalize.data.finalize.lifecycle_id = "token.drop";
    finalize.data.finalize.guard_flag = UINT32_C(7);
    spx_trace_push(&context, &finalize);

    if (buffer.length != UINT32_C(2)) return 12;
    if (buffer.events[0].kind != SPX_TRACE_INITIALIZE ||
        strcmp(buffer.events[0].data.initialize.at_expression_id,
               "expression.initialize") != 0) return 13;
    if (buffer.events[1].kind != SPX_TRACE_FINALIZE_END ||
        strcmp(buffer.events[1].data.finalize.lifecycle_id,
               "token.drop") != 0 ||
        buffer.events[1].data.finalize.guard_flag != UINT32_C(7)) return 14;

    struct spx_trace_buffer empty = {0};
    if (!spx_trace_buffer_init(&empty, NULL, UINT32_C(0))) return 15;
    if (spx_trace_buffer_init(&empty, NULL, UINT32_C(1))) return 16;
    if (spx_trace_buffer_init(&empty, entries, UINT32_C(0))) return 49;
    if (spx_trace_buffer_init(&empty, entries, UINT32_MAX)) return 17;

    if (argc == 2 && strcmp(argv[1], "overflow") == 0) {
        struct spx_trace_event excess = event(SPX_TRACE_RESULT_COMMIT, "app.main");
        spx_trace_push(&context, &excess);
        return 18;
    }
    return 0;
}
"#
    );

    let mut compiler = Command::new("clang")
        .args(["-x", "c", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
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
        .expect("write native trace runtime to clang");
    let compiled = compiler.wait().expect("wait for clang");
    assert!(compiled.success(), "generated trace runtime must compile");

    let normal = Command::new(&binary).status().expect("run trace runtime");
    assert!(normal.success(), "trace runtime exited {normal}");
    for valid in [
        "valid-contract",
        "valid-arithmetic",
        "insufficient-capacity",
        "double-attach",
        "alias-storage",
        "reinit-buffer",
        "reinit-context",
    ] {
        let result = Command::new(&binary)
            .arg(valid)
            .status()
            .unwrap_or_else(|error| panic!("run {valid} probe: {error}"));
        assert!(result.success(), "{valid} must be accepted");
    }
    for hostile in [
        "not-attached",
        "overflow",
        "invalid-kind",
        "invocation-pointer",
        "invocation-count",
        "projection-count",
        "projection-pointer",
        "call-argument-storage",
        "unsupported-import",
        "invalid-status",
        "forged-class",
        "import-class",
        "explicit-close-class",
        "forged-domain",
        "reserved-domain-mismatch",
        "unknown-contract-code",
        "unknown-arithmetic-code",
        "retryable-compiler-status",
        "unknown-retryability",
    ] {
        let result = Command::new(&binary)
            .arg(hostile)
            .status()
            .unwrap_or_else(|error| panic!("run {hostile} probe: {error}"));
        assert!(!result.success(), "{hostile} must abort");
    }
    std::fs::remove_file(&binary).expect("remove generated trace runtime fixture");
}
