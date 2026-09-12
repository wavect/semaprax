# codegen/native_emit/environment_runtime.c

- spx_environment_entry_v1 · class · L3-L3 — struct spx_environment_entry_v1 { spx_str_v1 name; spx_str_v1 value; };
- spx_environment_snapshot_v1 · class · L4-L7 — struct spx_environment_snapshot_v1
- spx_environment_command_state_v1 · class · L8-L11 — struct spx_environment_command_state_v1
- spx_environment_name_v1 · function · L15-L19 — static bool spx_environment_name_v1(spx_str_v1 name)
- spx_environment_key_before_v1 · function · L20-L24 — static bool spx_environment_key_before_v1(spx_str_v1 a, spx_str_v1 b)
- spx_environment_input_is_valid_v1 · function · L25-L44 — static bool spx_environment_input_is_valid_v1(
- spx_environment_status_v1 · function · L45-L52 — static spx_status_token spx_environment_status_v1(struct spx_context *ctx, uint32_t code)
- spx_environment_state_v1 · function · L53-L53 — static struct spx_environment_command_state_v1 *spx_environment_state_v1(struct spx_context *ctx)
- spx_host_env_len_v1 · function · L57-L65 — static __attribute__((unused)) spx_status_token spx_host_env_len_v1(struct spx_context *ctx, uint64_t *out)
- spx_environment_lookup_v1 · function · L66-L79 — static spx_status_token spx_environment_lookup_v1(struct spx_context *ctx, uint64_t index, bool name, spx_str_v1 *out)
- spx_host_env_name_utf8_v1 · function · L80-L82 — static __attribute__((unused)) spx_status_token spx_host_env_name_utf8_v1(struct spx_context *ctx, uint64_t index, spx_str_v1 *out)
- spx_host_env_value_utf8_v1 · function · L83-L85 — static __attribute__((unused)) spx_status_token spx_host_env_value_utf8_v1(struct spx_context *ctx, uint64_t index, spx_str_v1 *out)
