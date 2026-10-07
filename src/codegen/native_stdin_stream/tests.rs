use std::process::Command;
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

const C_STUBS: &str = r#"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t spx_status_token;
#define SPX_STATUS_SUCCESS UINT32_C(0)
typedef struct { const uint8_t *ptr; uint64_t len; } spx_slice_u8_v1;
struct spx_command_output_staging_v1 {
    uint64_t stdout_length;
    uint64_t stderr_length;
    uint8_t stdout_bytes[8];
    uint8_t stderr_bytes[8];
};
struct spx_language_command_input_v1 { spx_slice_u8_v1 stdin_snapshot; };
struct spx_language_command_state_v1 {
    struct spx_command_output_staging_v1 output;
    const struct spx_language_command_input_v1 *input;
    bool stdin_consumed;
};
struct spx_context { void *target_state; };

static void spx_runtime_invariant_failure(const char *reason) {
    (void)fprintf(stderr, "%s\n", reason);
    exit(86);
}

static spx_status_token spx_command_input_status_v1(
    struct spx_context *context, uint32_t code
) {
    (void)context;
    if (code != UINT32_C(3)) spx_runtime_invariant_failure("unexpected status code");
    return code;
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
                              uint32_t capacity, uint32_t *length_out) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)77 || capacity != UINT32_C(4096)) abort();
    if (provider->reads == UINT32_C(0)) {
        buffer[0] = 'a'; buffer[1] = 'b'; *length_out = UINT32_C(2);
    } else if (provider->reads == UINT32_C(1)) {
        buffer[0] = 'c'; *length_out = UINT32_C(1);
    } else {
        *length_out = UINT32_C(0);
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
                              uint32_t capacity, uint32_t *length_out) {
    struct scripted_provider *provider = opaque;
    if (token != (uintptr_t)55 || capacity != UINT32_C(4096)) abort();
    provider->reads += UINT32_C(1);
    if (provider->reads == provider->fail_on_read) return UINT32_C(3);
    buffer[0] = 'x'; *length_out = UINT32_C(1); return UINT32_C(0);
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
    if (spx_host_stdin_stream_open_v1(&first_context, &reader) != UINT32_C(3) ||
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
    if (spx_host_stdin_stream_next_v1(&second_context, reader, &published) != UINT32_C(3) ||
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
                              uint32_t capacity, uint32_t *length_out) {
    (void)opaque; (void)token; (void)buffer; (void)capacity;
    *length_out = UINT32_C(0); return UINT32_C(0);
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
                              uint32_t capacity, uint32_t *length_out) {
    (void)opaque; (void)token; (void)buffer; (void)capacity;
    *length_out = UINT32_C(0); return UINT32_C(0);
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
