# Private Bounded Native Agent Runtime v1

Status: private proof tranche. The implementation is not publicly exported and
does not add language syntax, compiler semantics, a provider transport, a CLI,
or a backend.

## Boundary

The runtime admits one caller-authenticated canonical profile and task, routes
sequentially across an injected model catalog, and accepts only canonical final
or registered read-only tool actions. Model output is data, never authority.
Each tool is selected from the admitted catalog; its closed scalar-object
schema, read effect, capabilities, policy epoch, cancellation state, deadline,
and remaining budgets are checked immediately before its single invocation.
The runtime supplies the call identity. Tools and uncertain provider attempts
are never retried.

The injected `AgentHost` owns provider credentials and transport. Provider and
tool bytes enter runtime-owned bounded sinks incrementally. No built-in HTTP,
provider SDK, local process, environment, home-directory credential lookup,
filesystem mutation, durable memory, wallet, payment, signing, or asset
authority exists. Cancellation is cooperative, not forced preemption.

## Canonical documents

The private tranche implements the frozen schemas:

- `semaprax.agent-runtime-profile.v1`
- `semaprax.agent-runtime-task.v1`
- `semaprax.agent-runtime-action.v1`
- `semaprax.agent-runtime-provider-request.v1`
- `semaprax.agent-runtime-tool-result.v1`
- `semaprax.agent-runtime-trace.v1`
- `semaprax.agent-runtime-evidence.v1`

Documents are compact canonical UTF-8 JSON with one terminal LF, exact key and
array order, closed types, no duplicate or extra keys, and depth at most 16.
External digest strings are lowercase `sha256:` values over the frozen domain
and exact document bytes. Raw prompts, actions, tool results, provider errors,
and credentials are absent from Trace and Evidence; those artifacts retain
only bounded identities, decisions, lengths, usage, statuses, and digests.

The run loop is single-threaded (`max_concurrency` is exactly 1), bounded to 16
turns, 32 provider attempts, 32 tool calls, five minutes, and one decreasing
64 MiB builder budget. Effective profile limits may only reduce the production
caps. Checked arithmetic, exact terminal-artifact preflight, and runtime-owned
stream sinks prevent an external provider or tool call when its immediate
authenticated completion cannot fit.

## Diagnostics and operational Evidence

`SPX-G204` through `SPX-G209` cover canonical grammar, profile invariants,
routing eligibility, tool authorization, numeric bounds, and replay mismatch.
`SPX-I218` through `SPX-I221` cover the closed provider, tool, cancellation,
and deadline outcomes without appending attacker-controlled text. Profile/task
failures before an external attempt may return diagnostics alone. Once external
authority is crossed, every operational exit retains a terminal event, exact
usage, canonical Trace, and independently replayed Evidence.

## Executable evidence and nonclaims

Private deterministic tests pin the profile, task, tool action, final action,
Trace, and Evidence bytes/digests; canonical/depth and exact cap boundaries;
routing ties and permutation; streaming, UTF-8, malformed actions, uncertain
retry; tool schema/effect/capability/policy/result failures; cancellation and
deadline boundaries; replay mutations; secret-sentinel absence; no-write
inventory; 240-byte identities and JSON escaping; and cumulative builder
limits. CI is configured to run the fake-host corpus on Ubuntu, macOS, and
Windows; exact-head hosted evidence remains pending. There is no live-provider
or provider-quality claim.

The fixture documents have these executable raw SHA-256 known answers
(including the terminal LF):

- profile: `sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a`
- task: `sha256:b6be370dea6708b7b3f7c6bd46299061c8f146a684fdf9895c32dc7e50c9a425`
- tool action: `sha256:a7142d92a8d33130892472cfeafee44519fe7bbc9c52a12319638089583a5286`
- final action: `sha256:2b44a98bfc80bb89339c4a76c6d43637f3a65c5b0a65a9a5571d507289f6681a`
- Trace: `sha256:b418408ff16de76251e0b40eb2c7b68dd408bbae66b96e734138ad64f6f70eab`
- Evidence: `sha256:45da26349aa89514ca3066a0f14076d4220cb03560589b9f959f97e9564bd6ad`

The exact ordered nonclaims are carried in every profile, Trace, and Evidence:

`no_compiler_determinism_from_model_output`; `no_model_output_authority`;
`no_provider_identity_provenance_or_quality_truth`;
`no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content`;
`no_credential_prompt_state_trace_or_diagnostic_exposure`;
`no_ambient_network_filesystem_process_home_or_environment_authority`;
`no_write_apply_mutation_or_target_execution_tool_authority`;
`no_capability_minting_delegation_or_self_approval`;
`no_human_approval_ui_or_policy`; `no_semantic_prompt_injection_proof`;
`no_forced_cancellation_or_preemption`;
`no_exactly_once_provider_billing_or_retry`;
`no_durable_memory_persistence_recovery_or_resume`;
`no_crash_reboot_or_power_loss_durability`;
`no_distributed_or_parallel_execution`;
`no_model_quality_accuracy_or_completion_guarantee`;
`no_live_price_or_cost_accuracy_guarantee`;
`no_reusable_authorization_token`;
`no_signature_attestation_or_authenticated_provenance`;
`no_wallet_payment_signing_asset_or_economic_authority`;
`no_privacy_compliance_or_data_residency_guarantee`;
`no_general_formal_proof`;
`no_new_language_graph_cleanup_backend_or_runtime_semantics`;
`no_current_schema_api_or_kat_modification`.
