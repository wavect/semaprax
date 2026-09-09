/* Explicit callbacks are authority, not an operating-system discovery API. */
struct spx_process_callbacks_v1 {
    void *context;
    uint32_t (*run)(void *, uint64_t, spx_slice_u8_v1, uint64_t,
        spx_slice_u8_v1, uint64_t, uint64_t, uint64_t, uint64_t,
        uint8_t *, uint64_t, uint64_t *);
    uint32_t (*settle)(void *);
};
struct spx_process_command_state_v1 {
    struct spx_environment_command_state_v1 environment;
    const struct spx_process_callbacks_v1 *callbacks;
    uint64_t operations;
    uint64_t charged_bytes;
};
_Static_assert(offsetof(struct spx_process_command_state_v1, environment) == 0,
    "process preserves environment and command state prefixes");
static spx_status_token spx_process_status_v1(struct spx_context *ctx, uint32_t code) {
    if (code < 1 || code > 7) spx_runtime_invariant_failure("process status outside closed table");
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx, "semaprax.process.v1", code,
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("process status could not be recorded");
    return token;
}
static uint64_t spx_process_u64_v1(const uint8_t *p) {
    uint64_t value = 0;
    for (uint32_t i = 0; i < 8; ++i) value |= (uint64_t)p[i] << (8 * i);
    return value;
}
static uint32_t spx_process_u32_v1(const uint8_t *p) {
    uint32_t value = 0;
    for (uint32_t i = 0; i < 4; ++i) value |= (uint32_t)p[i] << (8 * i);
    return value;
}
static uint32_t spx_process_argv_v1(spx_slice_u8_v1 argv, uint64_t length) {
    if (length < 4 || length > argv.len || argv.ptr == NULL) return 1;
    uint32_t count = spx_process_u32_v1(argv.ptr);
    if (count > SPX_PROCESS_MAX_ARGUMENTS_V1) return 5;
    uint64_t offset = 4;
    for (uint32_t i = 0; i < count; ++i) {
        if (length - offset < 4) return 1;
        uint64_t size = spx_process_u32_v1(argv.ptr + offset);
        offset += 4;
        if (size > length - offset) return 1;
        for (uint64_t j = 0; j < size; ++j) if (argv.ptr[offset+j] == 0) return 1;
        offset += size;
    }
    return offset == length ? 0 : 1;
}
static uint32_t spx_process_wire_v1(const uint8_t *wire, uint64_t length,
    uint64_t stdout_max, uint64_t stderr_max) {
    if (length < SPX_PROCESS_HEADER_BYTES_V1 || spx_process_u64_v1(wire) != 1) return 6;
    uint64_t termination = spx_process_u64_v1(wire+8);
    uint64_t kind = termination & 3, code = termination >> 2;
    if (!((kind == 0 && code <= UINT32_MAX) || (kind == 1 && code >= 1 && code <= 255))) return 6;
    uint64_t out = spx_process_u64_v1(wire+16), err = spx_process_u64_v1(wire+24);
    if (out > stdout_max || err > stderr_max) return 5;
    return length == SPX_PROCESS_HEADER_BYTES_V1 + out + err ? 0 : 6;
}
static __attribute__((unused)) spx_status_token spx_host_process_run_v1(
    struct spx_context *ctx, uint64_t tool, spx_slice_u8_v1 argv, uint64_t argv_length,
    spx_slice_u8_v1 input, uint64_t input_length, uint64_t timeout_ms,
    uint64_t stdout_max, uint64_t stderr_max, spx_bytes_v1 *out
) {
    if (out == NULL) spx_runtime_invariant_failure("process result slot absent");
    *out = (spx_bytes_v1){NULL, 0};
    spx_slice_u8_require_valid(argv);
    spx_slice_u8_require_valid(input);
    if (argv_length > argv.len || input_length > input.len ||
        timeout_ms == 0 || timeout_ms > SPX_PROCESS_MAX_WAIT_MILLIS_V1)
        return spx_process_status_v1(ctx, 1);
    if (argv_length > SPX_PROCESS_MAX_INPUT_BYTES_V1 ||
        input_length > SPX_PROCESS_MAX_INPUT_BYTES_V1 - argv_length ||
        stdout_max > SPX_PROCESS_MAX_OUTPUT_BYTES_V1 - SPX_PROCESS_HEADER_BYTES_V1 ||
        stderr_max > SPX_PROCESS_MAX_OUTPUT_BYTES_V1 - SPX_PROCESS_HEADER_BYTES_V1 - stdout_max)
        return spx_process_status_v1(ctx, 5);
    uint32_t argument_status = spx_process_argv_v1(argv, argv_length);
    if (argument_status != 0) return spx_process_status_v1(ctx, argument_status);
    uint64_t capacity = SPX_PROCESS_HEADER_BYTES_V1 + stdout_max + stderr_max;
    uint64_t charge = argv_length + input_length + capacity;
    (void)spx_environment_state_v1(ctx);
    struct spx_process_command_state_v1 *state = (struct spx_process_command_state_v1 *)ctx->target_state;
    if (state->operations >= SPX_PROCESS_MAX_OPERATIONS_V1 ||
        charge > SPX_PROCESS_MAX_TOTAL_BYTES_V1 - state->charged_bytes)
        return spx_process_status_v1(ctx, 5);
    state->operations += 1;
    state->charged_bytes += charge;
    if (state->callbacks == NULL || state->callbacks->run == NULL)
        return spx_process_status_v1(ctx, 2);
    uint8_t *destination = (uint8_t *)malloc((size_t)capacity);
    if (destination == NULL) return spx_process_status_v1(ctx, 6);
    memset(destination, 0, (size_t)capacity);
    memset(destination, 255, (size_t)SPX_PROCESS_HEADER_BYTES_V1);
    uint64_t written = UINT64_MAX;
    uint32_t code = state->callbacks->run(state->callbacks->context, tool,
        argv, argv_length, input, input_length, timeout_ms, stdout_max, stderr_max,
        destination, capacity, &written);
    if (code != 0) {
        free(destination);
        return spx_process_status_v1(ctx, code);
    }
    if (written == UINT64_MAX) {
        free(destination);
        return spx_process_status_v1(ctx, 6);
    }
    if (written > capacity) {
        free(destination);
        return spx_process_status_v1(ctx, 5);
    }
    uint32_t wire_status = spx_process_wire_v1(destination, written, stdout_max, stderr_max);
    if (wire_status != 0) {
        free(destination);
        return spx_process_status_v1(ctx, wire_status);
    }
    out->ptr = destination;
    out->len = written;
    return SPX_STATUS_SUCCESS;
}
