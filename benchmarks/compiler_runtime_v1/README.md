# Shared-runtime compiler backend v1 — private next-batch checkpoint

Status: source implementation in progress, NOT admitted or runtime-qualified.
The existing codex-exec campaign and all its labels remain untouched.
`compiler_execution_broker.run_shared_trial` is an explicit new backend entry;
it requires the exact reserved `ExecutionAuthority` type and live checks.
Static provisioning is not authority. Unsupported admission refuses, without a
proxy, unsandboxed execution, or legacy-label fallback.

The protocol snapshot is a finite reachable-definition projection of the
installed retained schemas; sources.json records the original hashes. It is
compatibility evidence, not proof of effective enforcement. No runtime, model,
compiler, RPC, Cargo or live benchmark has run in this lane.

Pure checks were run once on the initial bounded implementation: seven passed,
one found common ServerNotification fields were not composed with variants.
That validator handling has been repaired in source; rerun and expanded
broker/settlement controls remain pending. Current code is a private WIP and
must not be joined or represented as a completed feature.

Remaining integration: compose the separate provisioning module, finish pure
controls and independent review, qualify actual authority/configuration and
protected artifacts, then explicitly opt both benchmark arms into a newly
versioned matched configuration. Never retarget prior CLI evidence.
