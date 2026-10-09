# Stream Data Command v2 (Project v29)

Status: implementation and focused gates authored, not yet executed or hosted
qualified. OPT-723 and OPT-724 remain open through application acceptance and
matched agent benchmark evidence.

Select `[package] profile = "language-command-io.stream-data.v2"` in the
canonical `semaprax.manifest.v1` table manifest. It lowers to
`semaprax.project.v29`. Project v27 / `language-command-io.stream-data.v1`
retains its exact scalar-vector-only contract.

## Application boundary

The entry and selected command remain explicit `fn() -> i64`. The external
stream ABI, input `argv-utf8+stdin-stream.v1`, capability inventory, provider
limits, output staging, exit status rules and native-only command route are
unchanged. Web, Wasm package and npm publication retain `SPX-W120` refusal;
this profile adds no foreign interface or record host payload.

Authenticated private helpers additionally admit:

- By-value parameters/results of the explicit monomorphic flat Copy records
  defined in [Copy Record Collections v1](COPY-RECORD-COLLECTIONS-V1.md).
- Owned or borrowed `Vec<R>` parameters and owned `Vec<R>` results. Indexed
  mutation in while loops retains the core same-owner intrinsic update rules;
  private owner-returning helpers do not gain a new loop-renewal exemption.
- An explicit monomorphic two-case Copy codec outcome whose success case has
  exactly one such record and whose error case has exactly `(i64, usize, i64)`
  fields in declaration order.
- An explicit monomorphic two-case owned codec outcome whose success case has
  exactly one `string` and whose refusal case has exactly one `usize`.
- The direct collection outcome in [Copy Record Collections v1](COPY-RECORD-COLLECTIONS-V1.md):
  exactly two cases, 1–8 fields per case, one or two direct `Vec<R>` fields
  across the declaration, and direct scalar siblings. This supports the real
  decoder result with patient and identifier-span collections. Its private
  parameters use Own or Borrow, and its result is owned.

Either case order is admitted. Names and a generator's provenance confer no
special authority; types, field order, ownership and persistent declarations
are independently checked. Codec outcomes use Value and Own boundaries
respectively; direct collection outcomes are affine. No generic, arbitrary nested-record, arbitrary variant, or
owned-record collection profile is implied. Record invariants whose synthesized
helper closure falls outside explicit authored functions remain refused here.

## Source, cache and cleanup authority

The existing owned-data workspace linker retains the exact reachable declaration
closure and rebuilds type, loan and cleanup facts. The selected v29 validator in
`hir/workspace_link/stream_record.rs` then checks every helper signature and
all authored nominal references against its closed record/codec shapes. It runs
HIR validation first, preserves explicit identities, rejects interfaces and
generic templates/instances, and checks effects. The entry, command and named
test links use this same reconstruction. A fallback link without nominal
facts refuses; it cannot silently use v27's scalar linker.

Retained/prepared interpreter closure selection uses the manifest-selected v29
profile. Native emission independently rechecks the exact closure before using
the unchanged stream runtime. No cached graph or manifest text alone grants
source, capability, publication or raw carrier authority. Old manifests, profile
names, graph/report envelopes and diagnostics retain their existing meaning;
v29 has a distinct manifest schema, profile name and digest input.

## Required gates

`project::tests::stdin_stream_command::stream_records` authors an actual streamed
native command with imported six-field records, private owned/borrowed Vec
transport, Copy record returns and deterministic ordering. Its pure entry,
named tests and prepared interpreter must agree. It also checks exact graph
replay, fresh source field-type changes, forged ownership, old-profile rejection
and package backend refusal. `hir::workspace_link::stream_record::tests` checks
exact codec shape/type/ownership admission and hostile declaration facts.
The JSON codec lane adds composition with its ordinary generated source.

Execute those gates with the Copy-record cross-engine corpus, existing v27
stream-data and v25 stream-text profiles, and source/cache replay regressions.
Then run unchanged CLI 49, ShiftSim 15 and TeamDesk 912 acceptance and matched
five-runs-per-arm agent campaigns. Report turns, net input, authored tokens,
conditional accepted-task cost, wall time and fixed harness context separately;
unavailable provider billing remains null. A passed compiler gate establishes
behavior, not a token or cost advantage.
