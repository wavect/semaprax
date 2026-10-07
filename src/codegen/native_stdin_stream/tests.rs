use std::process::Command;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn compile_and_run(body: &str, expect_success: bool) {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-native-stdin-stream-{}-{id}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    let source = format!("{}\n{}\n{}", C_STUBS, super::STDIN_STREAM_RUNTIME_C, body);
    std::fs::write(&c_path, source).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-unused-function",
            "-O2",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "native stdin stream fixture did not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&executable).output().unwrap();
    if expect_success {
        assert!(
            output.status.success(),
            "native stdin stream fixture had unexpected exit status:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert_eq!(
            output.status.code(),
            Some(86),
            "native stdin stream guard did not take the invariant-failure path:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_file(c_path);
    let _ = std::fs::remove_file(executable);
}

fn compile_runner(command: &str, tail: &str, include_process_adapter: bool) -> Option<PathBuf> {
    if Command::new("clang").arg("--version").output().is_err() {
        return None;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-native-stdin-stream-runner-{}-{id}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    let mut source = C_STUBS.to_owned();
    source.push('\n');
    super::emit_runtime(&mut source);
    source.push_str(command);
    source.push('\n');
    super::emit_runner(&mut source, "test_command");
    source.push_str(tail);
    source.push('\n');
    if include_process_adapter {
        super::emit_process_adapter(&mut source);
    }
    std::fs::write(&c_path, source).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-unused-function",
            "-O2",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "native stdin stream runner fixture did not compile:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let _ = std::fs::remove_file(c_path);
    Some(executable)
}

fn run_with_stdin(executable: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(executable)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

const C_STUBS: &str = r#"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t spx_status_token;
#define SPX_STATUS_SUCCESS UINT32_C(0)
#define SPX_COMMAND_ARGUMENT_LIMIT_V1 UINT32_C(16)
#define SPX_COMMAND_INPUT_CAPACITY_V1 UINT64_C(65536)
#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(65536)
#define SPX_STATUS_DOMAIN_MAX_BYTES UINT32_C(256)
typedef uint32_t spx_status_class;
typedef uint32_t spx_retryability;
typedef struct { uint8_t *data; uint64_t len; } spx_str_v1;
typedef struct { const uint8_t *ptr; uint64_t len; } spx_slice_u8_v1;
struct spx_command_output_staging_v1 {
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[8];
    uint8_t stderr_bytes[8];
};
struct spx_language_command_input_v1 {
    uint32_t argument_count;
    spx_str_v1 arguments[SPX_COMMAND_ARGUMENT_LIMIT_V1];
    spx_slice_u8_v1 stdin_snapshot;
};
struct spx_language_command_state_v1 {
    struct spx_command_output_staging_v1 output;
    const struct spx_language_command_input_v1 *input;
    bool stdin_consumed;
};
struct spx_language_command_result_v1 {
    bool semantic_success;
    bool matched;
    uint32_t status_code;
    spx_status_class status_class;
    spx_retryability status_retryability;
    char status_domain[SPX_STATUS_DOMAIN_MAX_BYTES];
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
    uint8_t stderr_bytes[SPX_COMMAND_OUTPUT_CAPACITY_V1];
};
struct spx_normalized_status {
    const char *domain_id;
    uint32_t code;
    spx_status_class status_class;
    spx_retryability retryability;
};
struct spx_status_detail { uint32_t unused; };
struct spx_status_entry {
    struct spx_normalized_status status;
    struct spx_status_detail detail;
};
struct spx_context {
    void *target_state;
    struct spx_status_entry *entries;
    uint32_t capacity;
    uint32_t length;
};

static bool spx_context_init(struct spx_context *context, uint64_t nonce,
                             struct spx_status_entry *entries, uint32_t capacity,
                             const void *imports, const void *capabilities,
                             void *target_state) {
    (void)nonce; (void)imports; (void)capabilities;
    context->target_state = target_state; context->entries = entries;
    context->capacity = capacity; context->length = UINT32_C(0);
    return context != NULL && entries != NULL && capacity != UINT32_C(0);
}

static bool spx_command_utf8_v1(const uint8_t *bytes, uint64_t length) {
    return length <= SPX_COMMAND_INPUT_CAPACITY_V1 &&
        (length == UINT64_C(0) ? bytes == NULL : bytes != NULL);
}

static bool spx_language_command_input_is_valid_v1(
    const struct spx_language_command_input_v1 *input
) {
    return input != NULL && input->argument_count <= SPX_COMMAND_ARGUMENT_LIMIT_V1 &&
        input->stdin_snapshot.ptr == NULL && input->stdin_snapshot.len == UINT64_C(0);
}

static const struct spx_normalized_status *spx_status_resolve(
    const struct spx_context *context, spx_status_token token
) {
    if (context == NULL || token == SPX_STATUS_SUCCESS || token > context->length) return NULL;
    return &context->entries[token - UINT32_C(1)].status;
}

static const struct spx_status_detail *spx_status_resolve_detail(
    const struct spx_context *context, spx_status_token token
) {
    if (context == NULL || token == SPX_STATUS_SUCCESS || token > context->length) return NULL;
    return &context->entries[token - UINT32_C(1)].detail;
}

static bool spx_status_domain_size(const char *domain, size_t *size_out) {
    if (domain == NULL || size_out == NULL) return false;
    *size_out = strlen(domain) + 1u;
    return true;
}

static void spx_runtime_invariant_failure(const char *reason) {
    (void)fprintf(stderr, "%s\n", reason);
    exit(86);
}

static spx_status_token spx_command_input_status_v1(
    struct spx_context *context, uint32_t code
) {
    if (context == NULL || context->entries == NULL || context->capacity == 0 ||
        context->length != 0 || code != UINT32_C(3)) {
        spx_runtime_invariant_failure("unexpected status recording request");
    }
    context->entries[0].status = (struct spx_normalized_status){
        .domain_id = "semaprax.command-input.v1",
        .code = code,
        .status_class = UINT32_C(5),
        .retryability = UINT32_C(1)
    };
    context->length = UINT32_C(1);
    return UINT32_C(1);
}
"#;

#[test]
fn provider_reuses_reader_for_short_chunks_and_idempotent_eof() {
    compile_and_run(
        r#"
struct scripted_provider {
    uint32_t reads;
    uint32_t drops;
    uint32_t settles;
};
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque;
    *token_out = (uintptr_t)77;
    return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)77 || capacity != UINT32_C(4096)) abort();
    if (provider->reads == UINT32_C(0)) {
        buffer[0] = 'a'; buffer[1] = 'b'; *length_out = UINT32_C(2);
        *eof_out = UINT32_C(0);
    } else if (provider->reads == UINT32_C(1)) {
        buffer[0] = 'c'; *length_out = UINT32_C(1); *eof_out = UINT32_C(0);
    } else {
        *length_out = UINT32_C(0); *eof_out = UINT32_C(1);
    }
    provider->reads += UINT32_C(1);
    return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)77) abort();
    provider->drops += UINT32_C(1);
}
static void settle_provider(void *opaque) {
    struct scripted_provider *provider = opaque;
    provider->settles += UINT32_C(1);
}
int main(void) {
    struct scripted_provider script = {0};
    struct spx_stdin_stream_state_v1 state = {0};
    state.provider = (struct spx_stdin_stream_provider_v1){
        .context = &script, .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
    struct spx_context context = { .target_state = &state };
    uintptr_t reader = (uintptr_t)0, next_reader = (uintptr_t)0;
    if (spx_host_stdin_stream_open_v1(&context, &reader) != SPX_STATUS_SUCCESS) return 1;
    if (reader != (uintptr_t)&state || spx_stdin_stream_eof_v1(&context, reader)) return 2;
    spx_slice_u8_v1 chunk = spx_stdin_stream_chunk_v1(&context, reader);
    if (chunk.len != UINT64_C(2) || memcmp(chunk.ptr, "ab", 2) != 0) return 3;
    if (spx_host_stdin_stream_next_v1(&context, reader, &next_reader) != SPX_STATUS_SUCCESS ||
        next_reader != reader) return 4;
    chunk = spx_stdin_stream_chunk_v1(&context, reader);
    if (chunk.len != UINT64_C(1) || chunk.ptr[0] != 'c') return 5;
    if (spx_host_stdin_stream_next_v1(&context, reader, &next_reader) != SPX_STATUS_SUCCESS ||
        next_reader != reader || !spx_stdin_stream_eof_v1(&context, reader)) return 6;
    if (spx_stdin_stream_chunk_v1(&context, reader).len != UINT64_C(0)) return 7;
    if (spx_host_stdin_stream_next_v1(&context, reader, &next_reader) != SPX_STATUS_SUCCESS ||
        next_reader != reader || script.reads != UINT32_C(3)) return 8;
    uintptr_t moved = spx_stdin_stream_move_v1(&context, &reader);
    if (reader != (uintptr_t)0 || moved != (uintptr_t)&state) return 9;
    spx_stdin_stream_drop_v1(&context, moved);
    spx_stdin_stream_settle_v1(&context);
    if (script.drops != UINT32_C(1) || script.settles != UINT32_C(1)) return 10;
    return 0;
}
"#,
        true,
    );
}

#[test]
fn failed_prefetch_and_refill_keep_status_and_release_once() {
    compile_and_run(
        r#"
struct scripted_provider {
    uint32_t reads;
    uint32_t fail_on_read;
    uint32_t drops;
    uint32_t settles;
};
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque; *token_out = (uintptr_t)55; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)55 || capacity != UINT32_C(4096)) abort();
    provider->reads += UINT32_C(1);
    if (provider->reads == provider->fail_on_read) return UINT32_C(3);
    buffer[0] = 'x'; *length_out = UINT32_C(1); *eof_out = UINT32_C(0);
    return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)55) abort();
    provider->drops += UINT32_C(1);
}
static void settle_provider(void *opaque) {
    ((struct scripted_provider *)opaque)->settles += UINT32_C(1);
}
static void configure(struct spx_stdin_stream_state_v1 *state,
                      struct scripted_provider *script) {
    state->provider = (struct spx_stdin_stream_provider_v1){
        .context = script, .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
}
int main(void) {
    struct scripted_provider first = { .fail_on_read = UINT32_C(1) };
    struct spx_stdin_stream_state_v1 first_state = {0}; configure(&first_state, &first);
    struct spx_context first_context = { .target_state = &first_state };
    uintptr_t reader = (uintptr_t)99;
    if (spx_host_stdin_stream_open_v1(&first_context, &reader) != UINT32_C(1) ||
        reader != (uintptr_t)0 || first_state.reader_live) return 1;
    spx_stdin_stream_settle_v1(&first_context);
    if (first.drops != UINT32_C(1) || first.settles != UINT32_C(1)) return 2;

    struct scripted_provider second = { .fail_on_read = UINT32_C(2) };
    struct spx_stdin_stream_state_v1 second_state = {0}; configure(&second_state, &second);
    struct spx_context second_context = { .target_state = &second_state };
    reader = (uintptr_t)0;
    if (spx_host_stdin_stream_open_v1(&second_context, &reader) != SPX_STATUS_SUCCESS ||
        reader != (uintptr_t)&second_state) return 3;
    uintptr_t published = (uintptr_t)99;
    if (spx_host_stdin_stream_next_v1(&second_context, reader, &published) != UINT32_C(1) ||
        published != (uintptr_t)0 || second_state.chunk_length != UINT32_C(0) ||
        second_state.chunk_bytes[0] != UINT8_C(0)) return 4;
    spx_stdin_stream_settle_v1(&second_context);
    if (second.drops != UINT32_C(1) || second.settles != UINT32_C(1)) return 5;
    return 0;
}
"#,
        true,
    );
}

#[test]
fn reader_from_another_live_invocation_is_rejected() {
    compile_and_run(
        r#"
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque; *token_out = (uintptr_t)11; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    (void)opaque; (void)token; (void)buffer; (void)capacity;
    *length_out = UINT32_C(0); *eof_out = UINT32_C(1); return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) { (void)opaque; (void)token; }
static void settle_provider(void *opaque) { (void)opaque; }
static void configure(struct spx_stdin_stream_state_v1 *state) {
    state->provider = (struct spx_stdin_stream_provider_v1){
        .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
}
int main(void) {
    struct spx_stdin_stream_state_v1 left = {0}, right = {0};
    configure(&left); configure(&right);
    struct spx_context left_context = { .target_state = &left };
    struct spx_context right_context = { .target_state = &right };
    uintptr_t reader = (uintptr_t)0;
    if (spx_host_stdin_stream_open_v1(&left_context, &reader) != SPX_STATUS_SUCCESS) return 1;
    (void)spx_stdin_stream_chunk_v1(&right_context, reader);
    return 2;
}
"#,
        false,
    );
}

#[test]
fn open_permission_remains_spent_after_reader_drop() {
    compile_and_run(
        r#"
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque; *token_out = (uintptr_t)22; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    (void)opaque; (void)token; (void)buffer; (void)capacity;
    *length_out = UINT32_C(0); *eof_out = UINT32_C(1); return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) { (void)opaque; (void)token; }
static void settle_provider(void *opaque) { (void)opaque; }
int main(void) {
    struct spx_stdin_stream_state_v1 state = {0};
    state.provider = (struct spx_stdin_stream_provider_v1){
        .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
    struct spx_context context = { .target_state = &state };
    uintptr_t reader = (uintptr_t)0;
    if (spx_host_stdin_stream_open_v1(&context, &reader) != SPX_STATUS_SUCCESS) return 1;
    spx_stdin_stream_drop_v1(&context, reader);
    (void)spx_host_stdin_stream_open_v1(&context, &reader);
    return 2;
}
"#,
        false,
    );
}

#[test]
fn malformed_length_and_eof_tuples_are_invariant_failures() {
    for (length, eof) in [(0, 0), (1, 1)] {
        let body = format!(
            r#"
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {{
    (void)opaque; *token_out = (uintptr_t)33; return UINT32_C(0);
}}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {{
    (void)opaque; (void)token; (void)capacity;
    *length_out = UINT32_C({length}); *eof_out = UINT32_C({eof});
    if ({length} != 0) buffer[0] = 'x';
    return UINT32_C(0);
}}
static void drop_provider(void *opaque, uintptr_t token) {{ (void)opaque; (void)token; }}
static void settle_provider(void *opaque) {{ (void)opaque; }}
int main(void) {{
    struct spx_stdin_stream_state_v1 state = {{0}};
    state.provider = (struct spx_stdin_stream_provider_v1){{
        .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    }};
    struct spx_context context = {{ .target_state = &state }};
    uintptr_t reader = (uintptr_t)0;
    (void)spx_host_stdin_stream_open_v1(&context, &reader);
    return 2;
}}
"#
        );
        compile_and_run(&body, false);
    }
}

#[test]
fn runner_streams_irregular_chunks_past_snapshot_bound_and_stops_at_eof() {
    let command = r#"
struct scripted_stream { uint64_t produced; uint32_t reads; uint32_t drops; uint32_t settles; };
static spx_status_token test_command(struct spx_context *context, bool *matched) {
    uintptr_t reader = (uintptr_t)0, next_reader = (uintptr_t)0;
    spx_status_token status = spx_host_stdin_stream_open_v1(context, &reader);
    if (status != SPX_STATUS_SUCCESS) return status;
    uint64_t total = UINT64_C(0);
    while (!spx_stdin_stream_eof_v1(context, reader)) {
        spx_slice_u8_v1 chunk = spx_stdin_stream_chunk_v1(context, reader);
        total += chunk.len;
        status = spx_host_stdin_stream_next_v1(context, reader, &next_reader);
        if (status != SPX_STATUS_SUCCESS) return status;
        reader = next_reader;
    }
    struct scripted_stream *script =
        (struct scripted_stream *)((struct spx_stdin_stream_state_v1 *)context->target_state)->provider.context;
    uint32_t reads_at_eof = script->reads;
    status = spx_host_stdin_stream_next_v1(context, reader, &next_reader);
    *matched = status == SPX_STATUS_SUCCESS && next_reader == reader &&
        script->reads == reads_at_eof && total == UINT64_C(70021);
    spx_stdin_stream_drop_v1(context, reader);
    return status;
}
"#;
    let tail = r#"
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque; *token_out = (uintptr_t)44; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    static const uint32_t pattern[] = { 17, 4096, 31, 2049, 7, 4095 };
    struct scripted_stream *script = opaque;
    if (token != (uintptr_t)44 || capacity != UINT32_C(4096)) abort();
    script->reads += UINT32_C(1);
    if (script->produced == UINT64_C(70021)) {
        *length_out = UINT32_C(0); *eof_out = UINT32_C(1); return UINT32_C(0);
    }
    uint32_t chunk = pattern[(script->reads - UINT32_C(1)) %
        (uint32_t)(sizeof(pattern) / sizeof(pattern[0]))];
    uint64_t remaining = UINT64_C(70021) - script->produced;
    if ((uint64_t)chunk > remaining) chunk = (uint32_t)remaining;
    memset(buffer, 'x', (size_t)chunk);
    script->produced += (uint64_t)chunk;
    *length_out = chunk; *eof_out = UINT32_C(0); return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) {
    struct scripted_stream *script = opaque;
    if (token != (uintptr_t)44) abort();
    script->drops += UINT32_C(1);
}
static void settle_provider(void *opaque) {
    ((struct scripted_stream *)opaque)->settles += UINT32_C(1);
}
int main(void) {
    struct scripted_stream script = {0};
    const struct spx_stdin_stream_provider_v1 provider = {
        .context = &script, .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
    struct spx_language_command_input_v1 input = {0};
    struct spx_language_command_result_v1 result = {0};
    if (!spx_language_command_stream_run_v1(&input, &provider, &result) ||
        !result.semantic_success || !result.matched ||
        script.produced != UINT64_C(70021) || script.drops != UINT32_C(1) ||
        script.settles != UINT32_C(1)) return 1;
    return 0;
}
"#;
    let executable = compile_runner(command, tail, false).expect("clang is required for this test");
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "stream runner failed irregular >64 KiB fixture:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_file(executable);
}

#[test]
fn runner_settles_failed_refill_before_publishing_input_failure() {
    let command = r#"
static spx_status_token test_command(struct spx_context *context, bool *matched) {
    (void)matched;
    uintptr_t reader = (uintptr_t)0, next_reader = (uintptr_t)0;
    spx_status_token status = spx_host_stdin_stream_open_v1(context, &reader);
    if (status != SPX_STATUS_SUCCESS) return status;
    return spx_host_stdin_stream_next_v1(context, reader, &next_reader);
}
"#;
    let tail = r#"
struct scripted_stream { uint32_t reads; uint32_t drops; uint32_t settles; };
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    (void)opaque; *token_out = (uintptr_t)45; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    struct scripted_stream *script = opaque;
    if (token != (uintptr_t)45 || capacity != UINT32_C(4096)) abort();
    script->reads += UINT32_C(1);
    if (script->reads == UINT32_C(2)) return UINT32_C(3);
    buffer[0] = 'z'; *length_out = UINT32_C(1); *eof_out = UINT32_C(0);
    return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) {
    struct scripted_stream *script = opaque;
    if (token != (uintptr_t)45) abort();
    script->drops += UINT32_C(1);
}
static void settle_provider(void *opaque) {
    ((struct scripted_stream *)opaque)->settles += UINT32_C(1);
}
int main(void) {
    struct scripted_stream script = {0};
    const struct spx_stdin_stream_provider_v1 provider = {
        .context = &script, .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
    struct spx_language_command_input_v1 input = {0};
    struct spx_language_command_result_v1 result = {0};
    if (!spx_language_command_stream_run_v1(&input, &provider, &result) ||
        result.semantic_success || result.status_code != UINT32_C(3) ||
        strcmp(result.status_domain, "semaprax.command-input.v1") != 0 ||
        result.stdout_length != UINT64_C(0) || result.stderr_length != UINT64_C(0) ||
        script.reads != UINT32_C(2) || script.drops != UINT32_C(1) ||
        script.settles != UINT32_C(1)) return 1;
    return 0;
}
"#;
    let executable = compile_runner(command, tail, false).expect("clang is required for this test");
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "stream runner did not settle failed refill exactly once:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_file(executable);
}

#[test]
fn process_adapter_reads_more_than_snapshot_capacity_without_preloading() {
    let command = r#"
static spx_status_token test_command(struct spx_context *context, bool *matched) {
    uintptr_t reader = (uintptr_t)0, next_reader = (uintptr_t)0;
    spx_status_token status = spx_host_stdin_stream_open_v1(context, &reader);
    if (status != SPX_STATUS_SUCCESS) return status;
    uint64_t total = UINT64_C(0);
    while (!spx_stdin_stream_eof_v1(context, reader)) {
        total += spx_stdin_stream_chunk_v1(context, reader).len;
        status = spx_host_stdin_stream_next_v1(context, reader, &next_reader);
        if (status != SPX_STATUS_SUCCESS) return status;
        reader = next_reader;
    }
    *matched = total == UINT64_C(70013);
    spx_stdin_stream_drop_v1(context, reader);
    return SPX_STATUS_SUCCESS;
}
"#;
    let executable = compile_runner(command, "", true).expect("clang is required for this test");
    let input = vec![b'q'; 70_013];
    let output = run_with_stdin(&executable, &input);
    assert!(
        output.status.success(),
        "stream process adapter failed >64 KiB stdin:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let _ = std::fs::remove_file(executable);
}

#[test]
fn stream_runner_rejects_a_legacy_snapshot() {
    let command = r#"
static spx_status_token test_command(struct spx_context *context, bool *matched) {
    (void)context; *matched = true; return SPX_STATUS_SUCCESS;
}
"#;
    let tail = r#"
struct counters { uint32_t opens; uint32_t settles; };
static uint32_t open_provider(void *opaque, uintptr_t *token_out) {
    struct counters *counts = opaque; counts->opens += UINT32_C(1);
    *token_out = (uintptr_t)1; return UINT32_C(0);
}
static uint32_t read_provider(void *opaque, uintptr_t token, uint8_t *buffer,
                              uint32_t capacity, uint32_t *length_out,
                              uint32_t *eof_out) {
    (void)opaque; (void)token; (void)buffer; (void)capacity;
    *length_out = UINT32_C(0); *eof_out = UINT32_C(1); return UINT32_C(0);
}
static void drop_provider(void *opaque, uintptr_t token) { (void)opaque; (void)token; }
static void settle_provider(void *opaque) {
    ((struct counters *)opaque)->settles += UINT32_C(1);
}
int main(void) {
    uint8_t byte = 'x'; struct counters counts = {0};
    const struct spx_stdin_stream_provider_v1 provider = {
        .context = &counts, .open = open_provider, .read = read_provider,
        .drop = drop_provider, .settle = settle_provider
    };
    struct spx_language_command_input_v1 input = {0};
    input.stdin_snapshot = (spx_slice_u8_v1){ .ptr = &byte, .len = UINT64_C(1) };
    struct spx_language_command_result_v1 result = {0};
    if (spx_language_command_stream_run_v1(&input, &provider, &result) ||
        result.semantic_success || counts.opens != UINT32_C(0) ||
        counts.settles != UINT32_C(1)) return 1;
    return 0;
}
"#;
    let executable = compile_runner(command, tail, false).expect("clang is required for this test");
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "stream runner admitted a legacy stdin snapshot:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_file(executable);
}
