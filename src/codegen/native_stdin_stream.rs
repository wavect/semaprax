//! Standalone native provider foundation for bounded streaming command stdin.
//!
//! This fragment deliberately does not choose a command profile or access a
//! process file descriptor. An invocation supplies a synchronous provider
//! table through its target-state carrier. Profile routing and the compiler's
//! `StdinReader` value lowering are integrated separately.

use super::COutput;

pub(super) fn emit_runtime(output: &mut impl COutput) {
    output.push_str(STDIN_STREAM_RUNTIME_C);
}

const STDIN_STREAM_RUNTIME_C: &str = r#"#define SPX_STDIN_STREAM_CHUNK_CAPACITY_V1 UINT32_C(4096)

/* Provider tokens are opaque values. The runtime authenticates its own reader
   identity against target_state and never dereferences either token. */
struct spx_stdin_stream_provider_v1 {
    void *context;
    /* open returns only 0 (with a nonzero token) or 3 (with token still zero). */
    uint32_t (*open)(void *context, uintptr_t *provider_token_out);
    /* read returns only 0 (length 1..4096, or 0 for EOF) or 3 (length
       untouched). A positive short read is always a non-EOF chunk. */
    uint32_t (*read)(void *context, uintptr_t provider_token,
                     uint8_t *buffer, uint32_t capacity,
                     uint32_t *length_out);
    /* Cleanup cannot replace the selected command status. */
    void (*drop)(void *context, uintptr_t provider_token);
    void (*settle)(void *context);
};

/* The command output prefix is intentional. Existing output helpers
   authenticate target_state through that first member. */
struct spx_stdin_stream_state_v1 {
    struct spx_language_command_state_v1 command;
    struct spx_stdin_stream_provider_v1 provider;
    uintptr_t provider_token;
    uint64_t chunk_generation;
    uint32_t chunk_length;
    uint8_t chunk_bytes[SPX_STDIN_STREAM_CHUNK_CAPACITY_V1];
    bool open_attempted;
    bool provider_token_live;
    bool reader_live;
    bool eof;
    bool in_callback;
    bool poisoned;
    bool settled;
};

_Static_assert(
    offsetof(struct spx_stdin_stream_state_v1, command) == 0,
    "stdin stream command state must remain the target-state prefix"
);

_Static_assert(
    offsetof(struct spx_language_command_state_v1, output) == 0,
    "command output staging must remain the target-state prefix"
);

static struct spx_stdin_stream_state_v1 *spx_stdin_stream_state_v1(
    struct spx_context *spx_ctx
) {
    if (spx_ctx == NULL || spx_ctx->target_state == NULL) {
        spx_runtime_invariant_failure("stdin stream invocation state is unavailable");
    }
    return (struct spx_stdin_stream_state_v1 *)spx_ctx->target_state;
}

static void spx_stdin_stream_require_operable_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    if (state->poisoned || state->in_callback || state->settled) {
        spx_runtime_invariant_failure("stdin stream invocation is poisoned or settled");
    }
}

static bool spx_stdin_stream_provider_valid_v1(
    const struct spx_stdin_stream_provider_v1 *provider
) {
    return provider->open != NULL && provider->read != NULL &&
        provider->drop != NULL && provider->settle != NULL;
}

static void spx_stdin_stream_advance_generation_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    state->chunk_generation += UINT64_C(1);
    if (state->chunk_generation == UINT64_C(0)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream chunk generation overflow");
    }
}

static uint32_t spx_stdin_stream_read_v1(
    struct spx_stdin_stream_state_v1 *state
) {
    uint32_t length = UINT32_MAX;
    spx_stdin_stream_advance_generation_v1(state);
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    state->in_callback = true;
    uint32_t outcome = state->provider.read(
        state->provider.context,
        state->provider_token,
        state->chunk_bytes,
        SPX_STDIN_STREAM_CHUNK_CAPACITY_V1,
        &length
    );
    state->in_callback = false;
    if (outcome == UINT32_C(3)) {
        if (length != UINT32_MAX) {
            state->poisoned = true;
            spx_runtime_invariant_failure("stdin stream failed read published a length");
        }
        memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
        return UINT32_C(3);
    }
    if (outcome != UINT32_C(0) || length == UINT32_MAX ||
        length > SPX_STDIN_STREAM_CHUNK_CAPACITY_V1) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream provider violated its read protocol");
    }
    if (length == UINT32_C(0)) {
        state->eof = true;
        return UINT32_C(0);
    }
    state->chunk_length = length;
    return UINT32_C(0);
}

static spx_status_token spx_host_stdin_stream_open_v1(
    struct spx_context *spx_ctx,
    uintptr_t *reader_out
) {
    if (reader_out == NULL) {
        spx_runtime_invariant_failure("stdin stream open result slot is unavailable");
    }
    *reader_out = (uintptr_t)0;
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (state->open_attempted || state->provider_token_live || state->reader_live ||
        !spx_stdin_stream_provider_valid_v1(&state->provider)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream open is unavailable or repeated");
    }
    state->open_attempted = true;
    uintptr_t provider_token = (uintptr_t)0;
    state->in_callback = true;
    uint32_t outcome = state->provider.open(state->provider.context, &provider_token);
    state->in_callback = false;
    if (outcome == UINT32_C(3)) {
        if (provider_token != (uintptr_t)0) {
            state->poisoned = true;
            spx_runtime_invariant_failure("stdin stream failed open published a token");
        }
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    if (outcome != UINT32_C(0) || provider_token == (uintptr_t)0) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream provider violated its open protocol");
    }
    state->provider_token = provider_token;
    state->provider_token_live = true;
    if (spx_stdin_stream_read_v1(state) != UINT32_C(0)) {
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    state->reader_live = true;
    *reader_out = (uintptr_t)state;
    return SPX_STATUS_SUCCESS;
}

static spx_status_token spx_host_stdin_stream_next_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader,
    uintptr_t *reader_out
) {
    if (reader_out == NULL) {
        spx_runtime_invariant_failure("stdin stream next result slot is unavailable");
    }
    *reader_out = (uintptr_t)0;
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream reader is foreign or not live");
    }
    if (!state->eof && spx_stdin_stream_read_v1(state) != UINT32_C(0)) {
        return spx_command_input_status_v1(spx_ctx, UINT32_C(3));
    }
    *reader_out = reader;
    return SPX_STATUS_SUCCESS;
}

static uintptr_t spx_stdin_stream_move_v1(
    struct spx_context *spx_ctx,
    uintptr_t *source
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (source == NULL || *source == (uintptr_t)0 ||
        *source != (uintptr_t)state || !state->reader_live ||
        !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream owned move source is invalid");
    }
    uintptr_t reader = *source;
    *source = (uintptr_t)0;
    return reader;
}

static spx_slice_u8_v1 spx_stdin_stream_chunk_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream chunk reader is foreign or not live");
    }
    if (state->chunk_length == UINT32_C(0)) {
        return (spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) };
    }
    return (spx_slice_u8_v1){
        .ptr = state->chunk_bytes,
        .len = (uint64_t)state->chunk_length
    };
}

static bool spx_stdin_stream_eof_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream EOF reader is foreign or not live");
    }
    return state->eof;
}

static void spx_stdin_stream_drop_v1(
    struct spx_context *spx_ctx,
    uintptr_t reader
) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    spx_stdin_stream_require_operable_v1(state);
    if (!state->reader_live || reader == (uintptr_t)0 ||
        reader != (uintptr_t)state || !state->provider_token_live) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream drop reader is foreign or not live");
    }
    uintptr_t provider_token = state->provider_token;
    state->chunk_generation += UINT64_C(1);
    state->reader_live = false;
    state->provider_token_live = false;
    state->provider_token = (uintptr_t)0;
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    state->in_callback = true;
    state->provider.drop(state->provider.context, provider_token);
    state->in_callback = false;
}

static void spx_stdin_stream_settle_v1(struct spx_context *spx_ctx) {
    struct spx_stdin_stream_state_v1 *state = spx_stdin_stream_state_v1(spx_ctx);
    if (state->settled || state->in_callback) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream settlement is repeated or reentrant");
    }
    if (!spx_stdin_stream_provider_valid_v1(&state->provider)) {
        state->poisoned = true;
        spx_runtime_invariant_failure("stdin stream settlement provider is unavailable");
    }
    state->settled = true;
    spx_stdin_stream_advance_generation_v1(state);
    state->reader_live = false;
    state->chunk_length = UINT32_C(0);
    state->eof = false;
    memset(state->chunk_bytes, 0, sizeof(state->chunk_bytes));
    if (state->provider_token_live) {
        uintptr_t provider_token = state->provider_token;
        state->provider_token_live = false;
        state->provider_token = (uintptr_t)0;
        state->in_callback = true;
        state->provider.drop(state->provider.context, provider_token);
        state->in_callback = false;
    }
    state->in_callback = true;
    state->provider.settle(state->provider.context);
    state->in_callback = false;
}
"#;

#[cfg(test)]
mod tests;
