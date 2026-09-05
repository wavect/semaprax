# Installed Fix Plan v1

Status: additive authority-free planning projection; focused local integration
evidence passes 5/5.

Audience: compiler contributors, coding agents, CLI users, and reviewers of
diagnostic-repair planning.

Installed Fix Plan v1 exposes two closed read-only forms: an installed catalog
of plan kinds and one exact current-source plan. Version 1 advertises only the
existing Bounded Diagnostic Repair v1 response to `SPX-S103`: assigning a
persistent identity to one eligible automatic function. It does not guess a
target, select an identity, instantiate a patch, or apply a repair.

## API and commands

`src/installed_fix_plan.rs` owns two schemas and hard limits:

```text
semaprax.installed-fix-plan-catalog.v1       1 MiB
semaprax.current-source-fix-plan.v1         64 MiB
```

`installed_fix_plan_catalog()` returns exact JSON and its digest.
`current_source_fix_plan(path, request)` accepts only
`FixPlanRequest::assign_function_id`, and returns exact JSON/digest plus the
unchanged embedded Diagnostic Repair report and its separate digest.
`FixPlan::replay_current_source` freshly reruns source-bound discovery and
exact-compares the plan.

The exact CLI grammar is:

```text
semaprax fix --plan
semaprax fix <file> assign-function-id <automatic-function-id> --plan
```

The first prints the exact installed catalog. The second prints the exact
current-source plan after Diagnostic Repair's bounded held-source read and
final drift check. Missing, reordered, unknown, or extra operands fail as CLI
grammar with status 2. There are no aliases, implicit source lookup, ranking
flags, persistent-ID input, apply flag, or output path.

Existing `repairs`, `repair`, and `patch` commands are unchanged. Only
`repair` accepts a caller-selected persistent ID and returns a candidate
preview; `patch` remains the separately authorized commit route.

## Installed catalog and current plan

The installed catalog payload contains `authority`, `compiler`, `operations`,
`limits`, and `nonclaims`. Version 1 has exactly one operation:

- kind `assign_function_id` for diagnostic `SPX-S103`;
- classification `breaking_identity_rebase`;
- availability requiring exact current source and an automatic function ID;
- source report schema `semaprax.diagnostic-repair.v1`; and
- one required later `persistent_declaration_id` named `persistent_id`.

Catalog presence is installed support metadata, not a claim that any current
source or target is eligible.

The current plan embeds the complete installed catalog and exact unchanged
Diagnostic Repair report. The report must bind the same operation, target, and
`SPX-S103`. `source_binding` repeats its base revision and source digest and
records `diagnostic_repair_held_source_final_recheck`. Plan status is
`repair_available_requires_explicit_instantiation_input`.

The plan contains no candidate source or Semantic Patch and validates no
proposed persistent ID. A caller must separately instantiate and review the
existing repair preview.

## Canonical bytes and identity

Both artifacts are recursively key-sorted compact JSON terminated by one LF:

```json
{"digest":"sha256:...","payload":{},"schema":"..."}
```

The lowercase SHA-256 digest binds the canonical payload including its LF:

```text
domain || u64le(payload_byte_length) || payload_bytes
```

The domains are:

```text
semaprax.installed-fix-plan-catalog.payload.digest.v1\0
semaprax.current-source-fix-plan.payload.digest.v1\0
```

The unchanged embedded repair report is independently bound using domain
`semaprax.current-source-fix-plan.repair-report.digest.v1\0`, followed by its
little-endian u64 byte length and exact bytes.

Every payload carries `authority: false` and compiler package/version binding,
an optional 40-lowercase-hex build commit, and
`binary_identity_claimed: false`. This is not binary attestation, signing, or
reproducible-build evidence.

Exact replay rejects over-limit input before parsing, then requires canonical
digest grammar, JSON, schema and bytes, a matching payload digest, and a
byte-identical fresh plan from the current source and request.

## Diagnostics

- `SPX-G544`: invalid digest, JSON, canonical bytes, schema, embedded report,
  compiler binding, or document construction.
- `SPX-G545`: catalog, plan, or embedded-report capacity exceeded.
- `SPX-G546`: a well-formed embedded report does not match the selected plan.
- `SPX-G547`: digest mismatch or exact current-source replay failure.

Existing Diagnostic Repair admission/input failures retain `SPX-R101` and
`SPX-R102`; planning does not translate them into availability or success.

## Authority, compatibility, and nonclaims

Catalog construction is inert installed metadata. Current planning reads only
the selected source through existing bounded Diagnostic Repair authentication.
Neither form writes source or artifacts, changes a workspace/service/cache,
runs code or tests, starts a process, accesses the network, reads secrets,
chooses a repair, or acquires commit, publication, or host authority.

Version 1 is not general repair, automatic selection or ranking, source-wide
diagnosis, a claim that every diagnostic has a plan, a persistent-ID success
guarantee, or a source edit. It adds no Project/multi-file planner, Universal
Semantic Transaction operation, MCP, LSP, editor, daemon, hosted service, or
generated SDK.

This feature is additive. Existing Diagnostic Repair reports/previews and
`repairs`/`repair` CLI bytes remain unchanged, as do Diagnostic rendering,
Semantic Patch application, Project/workspace/image/query/transaction/service
artifacts, and frozen transport bytes.

## Focused evidence

`tests/semantic/installed_fix_plan.rs`, registered only in the existing
semantic harness, covers the exact one-operation catalog; canonical LF bytes,
digests, limits and compiler version binding; exact current-source embedding,
source binding and replay; byte-identical core/CLI output for both forms;
malformed, unavailable, noncanonical, tampered, oversized and stale rejection;
no writes; and exact preservation of existing core and CLI repair output.

```sh
CARGO_TARGET_DIR=target/installed-fix-plan-v1 \
  cargo test --locked -p semaprax --test semantic \
  installed_fix_plan --no-fail-fast
```

That command passes 5/5 in this checkout.
