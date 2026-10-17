# Shared-runtime compiler backend v1

Status: private next-batch source implementation, NOT admitted or runtime-qualified.
The existing codex-exec campaign, source/proxy validator and labels are unchanged.

`compiler_execution_broker.run_shared_trial(authority, configuration, arm, prompt,
candidate, evidence)` is the explicit opt-in runner entry. `candidate` must be the
fixed qualified workspace root. Both arms bind the same closed configuration:

```json
{
  "backend": "semaprax.shared-runtime-compiler.v1",
  "model": "EXACT_MODEL_FROM_NEW_MATCHED_SPEC",
  "effort": "medium",
  "arms": ["semaprax", "typescript"],
  "runtime_contracts_sha256": {"semaprax": "64_HEX", "typescript": "64_HEX"},
  "runtime_sha256": "64_HEX",
  "compiler_sha256": "64_HEX",
  "compiler_tool_arms": ["semaprax"],
  "allow_model_fallback": false
}
```

The placeholders are illustrative, invalid input. No model or price is chosen
by this example. Separate actual trial-root contracts are explicit; runtime,
compiler, model and effort remain common. The SEM-only codec tool is declared,
not hidden. These two digest slots and fixed_policy establish declared-shape
consistency, not matched effective environment/config/loader proof: HOME,
managed layers and dependency inventories can still differ between contracts.
A future qualifier must bind their common normalized authority fingerprint and
role-equivalent scope before admission; that comparison is not implemented here.
This backend is a new transport/configuration and cannot relabel an
old CLI run, reuse its calibration or claim a current token advantage.

The client requires the exact reserved `ExecutionAuthority` class and calls
`check_current` before launch and EVERY RPC. Static `ProvisioningPlan` values,
caller booleans, hashes and nonces cannot grant admission. The composed authority
module currently refuses ALL admission; no live grant has been manufactured in
tests. Unsupported authority has no old-proxy or unsandboxed fallback.

The runner owns a private stdio app-server connection, with bounded JSON frames,
requests, transcripts, stderr and elapsed waits. It negotiates the exact retained
installed schema, explicitly sets thread/turn/command policy, and refuses unknown
fields or schema-valid authority widening. Policy admits one fixed writable root,
no network, no implicit /tmp or TMPDIR writes; process/socket/loader/effective
policy proof remains a separate required authority obligation.

Compiler requests accept only the existing retained-codec selector grammar.
The runner injects a fresh nonce and invokes the protected compiler ONCE via
`command/exec`, observing the actual connection-scoped response/status/output.
Mandatory `capReached` refuses truncation. The stdout stream must contain one
complete <=512-byte publication seal matching the no-follow retained artifact.
The full original v1 structural inventory/derivation kernel is retained separately
from the unchanged proxy-origin adapter; all three inventories remain distinct.
No fake proxy observation is constructed. Semantic replay, authored subtraction,
billing, independent repeat and ratio eligibility are not automatically promoted.

`Session.close` checks settlement only of its runner-owned server process group.
It does not claim that detached child groups were enumerated. Actual per-command
whole-child settlement is an unresolved enforcement-qualification obligation.
Trial result publication happens after server-group settlement.

Eighteen focused transport/broker/protocol controls and four composed static
provisioning controls pass using synthetic metadata, in-memory protocol/process
interfaces and the existing offline receipt fixture. They launch no runtime,
compiler, RPC, model/provider or live benchmark and mint no live grant. Initial
WIP failure/old logs remain in external provenance; the common notification-field
composition defect is repaired. Actual enforcement and runtime qualification are
pending: see [QUALIFICATION.md](QUALIFICATION.md).

The protocol snapshot is a finite reachable-definition projection of the retained
installed schemas. sources.json records all original hashes; Protocol pins the
complete projected bytes. This compatibility evidence does not prove effective
sandbox enforcement. Production campaign CLI selection, fresh token/trace
calibration and hosted/live claims remain outside this private checkpoint.
