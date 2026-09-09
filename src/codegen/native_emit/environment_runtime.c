#define SPX_ENVIRONMENT_ENTRY_LIMIT_V1 UINT32_C(256)
#define SPX_ENVIRONMENT_STATUS_DOMAIN_V1 "semaprax.environment-input.v1"
struct spx_environment_entry_v1 { spx_str_v1 name; spx_str_v1 value; };
struct spx_environment_snapshot_v1 {
    uint32_t count;
    struct spx_environment_entry_v1 entries[SPX_ENVIRONMENT_ENTRY_LIMIT_V1];
};
struct spx_environment_command_state_v1 {
    struct spx_language_command_state_v1 command;
    const struct spx_environment_snapshot_v1 *environment;
};
_Static_assert(offsetof(struct spx_environment_command_state_v1, command) == 0,
    "environment command retains the frozen command-state prefix");

static bool spx_environment_name_v1(spx_str_v1 name) {
    if (name.len == 0 || !spx_command_argument_is_valid_v1(name)) return false;
    for (uint64_t i = 0; i < name.len; ++i) if (name.data[i] == '=') return false;
    return true;
}
static bool spx_environment_key_before_v1(spx_str_v1 a, spx_str_v1 b) {
    uint64_t length = a.len < b.len ? a.len : b.len;
    int order = length == 0 ? 0 : memcmp(a.data, b.data, (size_t)length);
    return order < 0 || (order == 0 && a.len < b.len);
}
static bool spx_environment_input_is_valid_v1(
    const struct spx_language_command_input_v1 *input,
    const struct spx_environment_snapshot_v1 *environment
) {
    if (!spx_language_command_input_is_valid_v1(input)) return false;
    if (environment == NULL) return true;
    if (environment->count > SPX_ENVIRONMENT_ENTRY_LIMIT_V1) return false;
    uint64_t total = input->stdin_snapshot.len;
    for (uint32_t i = 0; i < input->argument_count; ++i) total += input->arguments[i].len;
    for (uint32_t i = 0; i < environment->count; ++i) {
        spx_str_v1 name = environment->entries[i].name, value = environment->entries[i].value;
        if (!spx_environment_name_v1(name) || !spx_command_argument_is_valid_v1(value) ||
            name.len > SPX_COMMAND_INPUT_CAPACITY_V1 - total) return false;
        total += name.len;
        if (value.len > SPX_COMMAND_INPUT_CAPACITY_V1 - total) return false;
        total += value.len;
        if (i != 0 && !spx_environment_key_before_v1(environment->entries[i-1].name, name)) return false;
    }
    return true;
}
static spx_status_token spx_environment_status_v1(struct spx_context *ctx, uint32_t code) {
    if (code < 1 || code > 4) spx_runtime_invariant_failure("environment status outside closed table");
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx, SPX_ENVIRONMENT_STATUS_DOMAIN_V1, code,
        SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("environment status could not be recorded");
    return token;
}
static struct spx_environment_command_state_v1 *spx_environment_state_v1(struct spx_context *ctx) {
    (void)spx_command_state_v1(ctx);
    return (struct spx_environment_command_state_v1 *)ctx->target_state;
}
static __attribute__((unused)) spx_status_token spx_host_env_len_v1(struct spx_context *ctx, uint64_t *out) {
    if (out == NULL) spx_runtime_invariant_failure("environment result slot absent");
    *out = 0;
    const struct spx_environment_snapshot_v1 *env = spx_environment_state_v1(ctx)->environment;
    if (env == NULL) return spx_environment_status_v1(ctx, 4);
    if (env->count > SPX_ENVIRONMENT_ENTRY_LIMIT_V1) return spx_environment_status_v1(ctx, 3);
    *out = env->count;
    return SPX_STATUS_SUCCESS;
}
static spx_status_token spx_environment_lookup_v1(struct spx_context *ctx, uint64_t index, bool name, spx_str_v1 *out) {
    if (out == NULL) spx_runtime_invariant_failure("environment result slot absent");
    *out = (spx_str_v1){ .data = NULL, .len = 0 };
    struct spx_environment_command_state_v1 *state = spx_environment_state_v1(ctx);
    const struct spx_environment_snapshot_v1 *env = state->environment;
    if (env == NULL) return spx_environment_status_v1(ctx, 4);
    if (env->count > SPX_ENVIRONMENT_ENTRY_LIMIT_V1) return spx_environment_status_v1(ctx, 3);
    if (index >= env->count) return spx_environment_status_v1(ctx, 1);
    spx_str_v1 value = name ? env->entries[index].name : env->entries[index].value;
    if (!(name ? spx_environment_name_v1(value) : spx_command_argument_is_valid_v1(value)))
        return spx_environment_status_v1(ctx, 2);
    *out = value;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_host_env_name_utf8_v1(struct spx_context *ctx, uint64_t index, spx_str_v1 *out) {
    return spx_environment_lookup_v1(ctx, index, true, out);
}
static __attribute__((unused)) spx_status_token spx_host_env_value_utf8_v1(struct spx_context *ctx, uint64_t index, spx_str_v1 *out) {
    return spx_environment_lookup_v1(ctx, index, false, out);
}
