# Everyday Agent validation product

A bounded, honestly-scoped slice of the "Everyday Agent end-to-end
validation product" requested by issue ABI-09A.17.
[`docs/EVERYDAY-AGENT-VALIDATION-PRODUCT-V1.md`](../../docs/EVERYDAY-AGENT-VALIDATION-PRODUCT-V1.md)
is the versioned product-profile specification (`semaprax.everyday-agent-public-generic-validation.v1`)
pinning the exact schema, bounds, checkpoint rules, evidence shape, and
nonclaims this README explains in prose. It composes two real,
already-shipped SEMAPRAX foundations rather than inventing new ones:

1. **`src/manifest_json.spx`** — a real typed-filesystem-and-bounded-JSON
   workflow: reads `fixtures/input.json`, validates and classifies a fixed
   three-record manifest, and writes a canonical, no-clobber report to
   `fixtures/report.json`.
2. **`src/agent.spx`** — a source-declared Agent (`everyday.agent`) exercised
   through the existing durable checkpoint/crash-recovery lifecycle.

They are two files in **one project**, not two products, but they are
deliberately **not wired together** into one combined Agent-effect-calls-fs
pipeline. See "Scope and nonclaims" for exactly why, with reproductions.

## Commands

```sh
# The data/JSON/typed-fs slice: compiles and unit-tests under the reference
# interpreter (no filesystem authority — see "Scope and nonclaims").
semaprax check examples/everyday-agent-project
semaprax test  examples/everyday-agent-project

# The source-declared Agent is also admitted by the Project. This standalone
# check remains useful when inspecting the Agent module on its own:
semaprax check examples/everyday-agent-project/src/agent.spx
```

Real execution — a real `fs.read` of `fixtures/input.json`, real JSON
validation/classification, and a real `fs.write-new` of `fixtures/report.json`
— and the source-declared Agent's durable checkpoint/crash-recovery lifecycle
both require a host that binds an explicit capability (a scoped filesystem
provider, or a checkpoint store and read operation), because this language
grants **no ambient filesystem, network, or process authority** to compiled
code or to the plain CLI. `semaprax run`/`semaprax agent run` therefore
cannot exercise either slice for real; this repository's own examples
(`std.fs`'s tests) and this product both provide that host as a small,
explicitly-labeled validation harness in
`tests/agent_runtime_v1/everyday_agent_product.rs`:

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 everyday_agent_product
```

That harness:

- executes `everyday.manifest.review-ok` for real, through the same
  `semaprax::project::with_authenticated_project` /
  `execute_filesystem_command` Project route `std.fs`'s own tests use
  (`tests/project/standard_library/filesystem_v2.rs`), against a real
  temporary directory seeded with a copy of `fixtures/input.json`, and
  asserts the exact bytes of the resulting `fixtures/report.json`;
- compiles `src/agent.spx` via `compile_source_agent_lifecycle`, binds it
  through the existing `bind_durable_agent` durable-checkpoint machinery,
  and drives it through a happy path, an authorization refusal, an effect
  failure, and crash injection at all five durable boundaries
  (`CrashPoint::BeforeIntent` / `AfterIntent` / `AfterEffect` /
  `AfterSettlement` / `BeforeDelivery`), asserting that a resume never
  re-reads the real fixture and that an uncertain delivery
  (`AfterIntent`/`AfterEffect`) stays `DurableStatus::Unknown` until an
  explicit host reconciliation settles it — never a blind retry;
- proves the checkpoint document's self-verifying codec rejects a
  truncated, reordered, or byte-injected checkpoint, that `resume()`
  refuses a revoked policy epoch and reports a display-only source rename
  as `source_drift` (ProgramRoot drift), and that a self-consistent remint
  of a non-chain-covered budget field decodes anyway — the checkpoint
  module's own documented boundary, proven rather than assumed.

## The bounded manifest and report

`fixtures/input.json` is a fixed, flat, three-record manifest:

```json
{"schema":"everyday-agent-manifest.v1","record_count":3,
 "rec0_id":"m1","rec0_tag":"ok","rec0_payload":"alpha",
 "rec1_id":"m2","rec1_tag":"warn","rec1_payload":"beta",
 "rec2_id":"m3","rec2_tag":"ok","rec2_payload":"gamma"}
```

Each record has an identifier, a Copy-scalar tag classification (`"ok"`
accepts; anything else, including `"warn"`, is rejected), and an owned
text payload bounded at 16 bytes. `record_count` is pinned at exactly 3;
this is the whole bound, not an example of a larger one — see "Scope and
nonclaims" for why.

The report is the 7-byte canonical `{"a":<accepted_digit>}`. Because
`record_count` is pinned at 3, a reader recovers `rejected_count` as
`3 - accepted_count` without needing a second field. For the fixture above
that is `{"a":2}` (two `"ok"` tags accepted, one `"warn"` tag rejected).

## Scope and nonclaims

This product delivers, for real and verified by the commands above:

- canonical `.spx` source and an exact ProgramRoot (via `semaprax check`);
- bounded, typed `fs.read` / `fs.write-new` (no-clobber) filesystem
  interaction;
- bounded JSON validation, member lookup, and classification over a fixed
  flat manifest (`everyday.manifest.flat-object-valid`/`member`/`token-is`);
- a canonical, bounded, deterministic report;
- a source-declared Agent, admitted through the authenticated Project source
  set and compiled/selected by stable ID (`compile_source_agent_lifecycle`);
- explicit authorization before the external boundary
  (`everyday.agent.fn.authorize`, refused in
  `everyday_agent_refuses_before_the_external_read_when_authorization_is_refused`);
- durable per-operation checkpoints and crash/restart recovery without a
  duplicate external read, including genuine "uncertain, no blind retry"
  semantics at the two boundaries where the read may or may not have
  happened;
- replayable evidence (`DurableRun::evidence()`) that binds identities,
  digests, and counts and is asserted not to carry the fixture bytes, the
  authorization seal, or the task payload;
- checkpoint tamper detection: `AgentCheckpoint::decode` rejects a
  truncated, reordered, or byte-injected (structurally valid but
  non-canonical) checkpoint document
  (`everyday_agent_checkpoint_decode_rejects_truncation_reorder_and_injected_bytes`),
  and `resume()` refuses a revoked policy epoch
  (`everyday_agent_resume_refuses_a_revoked_policy_epoch`, reason
  `policy_epoch_revoked`) and a display-only source rename as ProgramRoot
  drift (`everyday_agent_resume_reports_a_display_only_rename_as_source_drift`,
  reason `source_drift` — the *last*-checked `CheckpointBinding` field, so
  this proves the comment-only edit is not silently absorbed by an earlier,
  broader drift check). The checkpoint codec's own honest boundary is
  proven too, not glossed over: a self-consistent remint of a
  non-chain-covered field (`budgets.effect_grants_remaining`) decodes
  successfully
  (`everyday_agent_checkpoint_decode_does_not_authenticate_a_self_consistent_budget_remint`),
  exactly matching the checkpoint module's own documented nonclaim ("no
  checkpoint integrity or authenticity without the caller's storage
  contract") — this is a real gap this product does not claim to close, not
  an oversight.

It does **not** deliver, and does not claim to deliver:

- **A real public-generic descriptor/provider/carrier call.** The Agent's
  `execute` effect is backed by this test's own `std::fs::read` of the real
  fixture, standing in for "the host executes the explicitly selected
  generated consumer" — it is a validation-harness substitute, not a call
  through a generated Public Generic Descriptor v1 consumer. The actual
  generated-consumer assets under `src/public_generic_consumer/` are
  explicitly test-only template fixtures (`scripts/public_generic_consumer_fixture.py`
  says so directly: "This is not a descriptor producer/verifier... Canonical
  framing != compiler-derived authenticity: no PG-7 claim") bound to a
  fixture provider explicitly named `unsupported-unpublished`. Issues #162
  (cross-engine settlement corpus) and #163 (hosted evidence refresh) — both
  P0 and open at the time this product was built — own turning that into a
  real, supported call; this product does not pre-empt or imply their
  outcome.
- Native or Wasm **provider** execution of that call, for the same reason.
- Evidence binding descriptor/provider/carrier/result-carrier digests,
  since there is no real descriptor/provider/carrier here to bind.
- The Agent's operations (`initialize`/`observe`/`authorize`/`reduce`)
  themselves performing `fs.read`/`fs.write`. They cannot: only `execute` is
  an `effect fn`, and it is realized by the host, not by compiled `.spx`
  code with `uses { fs.* }` — this is the existing Agent-runtime authority
  boundary, not a limit this product introduces.
- The iterative, multi-turn `AGENT-ITERATIVE-LIFECYCLE-V2` profile and the
  source-live repair route. `src/agent.spx` is now an authenticated Project
  source, satisfying repair's exact-source admission prerequisite, but its
  `reduce` operation returns the Lifecycle v1 `Report`. V2 requires one
  authored `Step` variant and a `reduce` operation returning that `Step`; one
  declared Agent operation cannot have both result contracts. Converting this
  product therefore requires a real V1-to-V2 lifecycle migration, including
  replacement of its durable V1 checkpoint evidence, rather than treating the
  existing V1 fixture as a repair-ready V2 source.
- State migration across a retained ProgramRoot revision.
  `AGENT-STATE-MIGRATION-V3` is an iterative-lifecycle (V2) facility
  (`src/agent_lifecycle/iterative/source_live/migration.rs`); this product
  uses the non-iterative Lifecycle v1 durable machine (see the V2 scope cut
  above), so migrating *this* product's checkpoints across a ProgramRoot
  revision would mean adopting V2, which is future work, not something this
  slice's own machine can exercise as an incremental addition. (ProgramRoot
  drift and policy-epoch revocation themselves are delivered — see above.)
- Hostile cross-paired-provider rejection, since there is no real provider
  pairing here to cross.
- The Agent's own `.spx` operations reading real JSON. They receive the
  manifest only as an opaque `Bytes` byte count for the Agent's own
  bookkeeping. The genuine JSON validation/classification lives entirely in
  `src/manifest_json.spx`, run separately.

### Why the manifest is exactly 3 records and the report is exactly 7 bytes

Both bounds are real, reproducible **language/tooling limits this product
ran into**, not arbitrary round numbers:

- **`SPX-G171` (Workspace Semantic Graph `builder_bytes`, pre-bound at
  18,874,368 bytes).** `std.fs` is a "hosted" multi-backend-tier package
  (native C11 and Core Wasm, per `std/packages.json`). Depending on it
  *and* on `std.data.json.doc` (a second package) in the same project
  exceeds this budget even for a two-line command body — reproducible by
  adding `std.data.json.doc = "^0.1.0"` back to `semaprax.toml`. Re-deriving
  the small bounded-JSON scanning this product needs as local functions
  (`everyday.manifest.json.*`, a flat-object-only rewrite of
  `std.data.json.doc`'s general recursive scanner) avoids the *dependency*
  cost, but a large enough scanner combined with `std.fs` hits the same
  budget from source size alone — reproducible by restoring the general
  recursive scanner (`document_end`/`step_action`/`next_state`, the
  depth-bounded state machine `std.data.json.doc` itself uses) in place of
  the flat-only one this product actually ships.
- **`SPX-T267` ("bytes_copy path reaches N sites; limit is 16"), a
  deliberate, documented cap** (`MAX_BYTES_COPY_SITES` in
  `src/byte_data_capacity.rs`) on the total interprocedural count of
  `bytes_copy`/`bytes_set`/`writer_write_u8`-family write-once operations
  reachable from one function — not per lexically-visible function body,
  but the whole call graph `everyday.manifest.review` reaches. Its two
  `Path` constructions already spend most of that budget; a
  `writer_write_u8`-chained report (the pattern
  `std.fs.examples.roundtrip` itself uses) is capped at 8 further calls,
  reproducible at exactly the 9th. A `while`-loop `bytes_set` writer (the
  pattern `std.data.json.write.count_into` uses for longer output) does not
  raise this particular count, but running one in this same project
  reproduces the `SPX-G171` budget above instead. `record_count = 3` and a
  7-byte, one-field report are the largest bounds this product could fit
  under *both* constraints at once with a real `fs.write`.

A maintainer, not this product, should decide whether either budget should
move for genuinely bounded, small workloads like this one — see the handoff
report for exactly this ask.

## Support/publication standing

Unsupported, unpublished. This is a validation product per ABI-09A.17, not a
support or publication decision; PG-9's own decision is unaffected and takes
precedence over any impression this product's README might otherwise give.
