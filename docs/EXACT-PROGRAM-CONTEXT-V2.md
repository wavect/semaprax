# Exact Program Context v2

Status: additive SEG-02 ProgramRoot-v3 selection contract; focused local
evidence passes. This is not ProgramRoot-v3 refresh, candidate identity,
execution, or authority.

Audience: compiler contributors, semantic-service implementers, and reviewers
of exact contract/test-fact selection.

`ExactProgramContextV2` retains and independently replays one complete
[Exact Program Context v1](EXACT-PROGRAM-CONTEXT-V1.md), its exact
[Contracts and Tests Facts v1](CONTRACTS-AND-TESTS-FACTS-V1.md), and the
[ProgramRoot v3](PROGRAM-ROOT-V3.md) that appends the descriptor for those same
facts. It is an additive typed in-memory selection layer. Exact-context v1,
ProgramRoot v1/v2/v3, facts, query, transaction, evidence, service receipt, and
history wire bytes are unchanged.

## Derivation, replay, and identity

`assemble` derives facts from the retained admitted Project, derives
ProgramRoot v3 from the complete v1 context, and passes both through the same
independent replay used by `derive`. `derive` first validates the exact enriched
workspace and ProgramRoot-v3 selectors, freshly replays context v1, facts, and
ProgramRoot v3, and rejects any cross-pairing. `replay` performs that selector
check before parsing submitted context-v2 bytes, then requires exact canonical
shape, self-authenticated identity, and byte-for-byte fresh derivation.

The schema is `semaprax.exact-program-context.v2`; the complete document is
capped at 96 KiB. Its closed descriptor contains the Project and enriched
workspace revisions; context-v1, facts, ProgramRoot-v1, ProgramRoot-v2, and
ProgramRoot-v3 digests; the fixed limit and nonclaims; and `context_v2_digest`.
The latter is lowercase SHA-256 over:

```text
"semaprax.exact-program-context.digest.v2\0"
|| u64le(byte_length)
|| exact_canonical_bytes_without_context_v2_digest
```

The document embeds none of the retained payloads or private Project Lock
bytes.

## Exact selection through existing operations

Every v2 exact route requires both the enriched workspace revision and
ProgramRoot-v3 digest before parsing or executing the existing operation:

- semantic query execution and replay retain exact ProgramRoot v2 and v3 on
  the typed result only;
- semantic transaction validation and replay retain the exact base ProgramRoot
  v2 and v3 on the typed artifacts only;
- service open, snapshot, query, query replay, transaction validation, and
  transaction replay retain the same context-v2 generation;
- exact history snapshot/query retain the same ProgramRoot v2 and v3 on typed
  results while serializing the unchanged history-v1 projection.

The appended ProgramRoot-v3 descriptor continues to name the exact retained
facts digest and byte count throughout. No route substitutes a newly derived or
caller-described fact association.

Exact transaction history continues to record the authenticated default
Project-derived base workspace identity from the unchanged transaction
artifacts. The enriched workspace and ProgramRoot v3 are selector associations,
not replacements for that base. Replay remains read-only and appends no history.

## Closed boundaries and diagnostics

An exact service generation cannot refresh: refresh still requires fresh
external-fact replay and fails closed for both context versions. Transaction
validation can construct the unchanged ordinary candidate artifacts, but there
is no candidate ProgramRoot v3, candidate context v2, or v3 refresh/adoption
route.

| Code | Meaning |
| --- | --- |
| `SPX-G576` | Malformed, noncanonical, internally inconsistent, unknown-field, invalid-digest, or over-bound context-v2 material. |
| `SPX-G577` | Stale or cross-paired workspace, ProgramRoot-v3, retained-product, context identity, or exact replay mismatch. |

Owning context-v1, facts, ProgramRoot, query, transaction, service, and history
diagnostics retain precedence when those layers reject after successful v2
selection. Failed selection creates no candidate, executes no query, appends no
history, and changes no generation or cache.

The exact nonclaims deny embedded payloads; contract proof, coverage, or test
results; changes to earlier identities; and filesystem, network, process,
execution, deployment, commit, or publication authority.

## Focused evidence

The three-case focused Workspace module passes locally. It covers exact
assembly/replay, retained descriptor identity, frozen context-v1 and
ProgramRoot-v1/v2 bytes, selector-first failure,
unknown-field rejection, self-consistent fact-digest remint rejection, and the
complete-document byte ceiling. Its lifecycle case keeps query/result,
transaction/evidence, and history bytes identical across ordinary, v1-exact,
and v2-exact routes; retains the same v2, v3, and facts association through
direct and service query/replay, transaction/replay, and history; preserves the
authenticated default base workspace in history; and shows stale/cross-paired
selectors winning before malformed operation bytes without appending history:

```sh
cargo test --locked -p semaprax --test workspace exact_program_context_v2::
```

This remains bounded in-memory association evidence. It does not establish
candidate-v3 derivation, exact refresh, a new service wire, execution, or
authority.
