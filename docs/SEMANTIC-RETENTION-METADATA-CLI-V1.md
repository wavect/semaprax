# Semantic retention metadata CLI v1

Status: **Partial, authored/unrun**.

Audience: compiler contributors, CLI integrators, and retention-store hosts.

This contract exposes authority-neutral retention planning and the immutable
semantic retention metadata store through four explicit command-line
operations. It derives a checkpoint and its exact companion plan from
caller-declared metadata, can make that pair durable, and restores it under
caller-held selectors. It does not make a retained subject current or
actionable.

## Declaration inventory construction

Canonical planner input can be constructed without reproducing the compiler's
subject-digest domain:

```text
semaprax retention-metadata-inventory <declarations.json>
```

`declarations.json` is exact canonical
`semaprax.semantic-retention-declaration-inventory.v1` JSON with a terminal
newline. Its closed top-level fields are `schema`, `declarations` and
`nonclaims`. Each declaration row contains exactly `subject` and nonzero
`stored_bytes`; no `subject_digest`, path, store handle or receipt is admitted.
The subject is one closed image, candidate or draft identity under the same
existing digest and 134,217,728-byte per-subject accounting bounds.

Rows are unique and strictly ordered by their visible canonical subject JSON
bytes. This order can be reproduced without the subject hash algorithm. The
compiler authenticates each subject, derives its domain-separated subject
digest, sorts the output rows by that derived digest, and emits raw exact
`semaprax.semantic-retention-observation-inventory.v1` JSON. That output is the
file format accepted directly by `retention-metadata-plan`; it is not wrapped in
a receipt or another payload.

The declaration input and emitted observation inventory are each bounded to
1,048,576 bytes and 96 rows. Fixed declaration nonclaims state that input rows
are caller declarations rather than store receipts or filesystem discovery;
their visible ordering does not depend on subject digests; compiler code derives
rather than trusts those digests; the declarations prove no presence,
freshness, validation or approval; and they grant no source, candidate, image,
GC or publication authority. The conversion reads only the explicit input file
and performs no discovery, persistence, planning, GC application or deletion.

## Canonical planning

Planning is:

```text
semaprax retention-metadata-plan <inventory.json> <sequence> <max-subjects> <max-bytes> <protected-generations> <previous-checkpoint.json|none> <previous-digest|none> <previous-predecessor-digest|none>
```

`inventory.json` must be exact canonical
`semaprax.semantic-retention-observation-inventory.v1` JSON with a terminal
newline. Its closed top-level fields are `schema`, `observations` and
`nonclaims`. Each observation binds `subject_digest`, the closed image,
candidate or draft `subject`, and its exact nonzero `stored_bytes`. Rows are
strictly sorted by derived subject digest, cannot repeat, and carry identities
rather than paths or store handles. The fixed nonclaims state that the rows are
caller declarations rather than store/filesystem discovery; do not prove
presence, freshness, validation or approval; and grant no source, candidate,
image, GC or publication authority.

The inventory is at most 1,048,576 bytes and 96 observations. Each observation
is at most 134,217,728 bytes of accounting. Existing policy bounds remain 1–96
subjects, 1–8,589,934,592 total bytes and 0–32 protected generations.
`sequence`, `max-subjects`, `max-bytes` and `protected-generations` are canonical
unsigned decimal operands: digits only, with no leading zero except the value
zero itself.

An initial checkpoint requires all three previous operands to be `none`. A
chained checkpoint requires an explicit previous-checkpoint file, its exact
checkpoint digest, and that checkpoint's own predecessor digest or `none`.
This three-part tuple lets ordinary checkpoint restoration authenticate the
file and its lineage before planning. The CLI never selects a previous, newest
or current checkpoint from a store.

The canonical output binds `checkpoint_digest`, exact `checkpoint_json`,
`plan_digest` and exact `plan_json`. It reports `authority: "none"` and false GC,
source, approval and publication authority. Planning reads only the named
inventory and optional prior-checkpoint files. It does not persist metadata,
scan a store, execute or apply the GC plan, delete a subject, or restore source,
candidate or image state.

## Persistence and load

Publication is:

```text
semaprax retention-metadata-persist <store-root> <checkpoint.json> <checkpoint-digest> <previous-digest|none> <plan.json> <plan-digest>
```

The caller supplies three separate capabilities: an existing store root, a
checkpoint file and a plan file. The CLI does not create or discover the root.
It opens the two files as explicit bounded, no-follow regular-file inputs,
restores their canonical semantic values under the exact checkpoint,
predecessor and plan selectors, and only then calls the immutable store pivot.
The checkpoint and plan limits remain 1,048,576 bytes each.

Exact restoration is:

```text
semaprax retention-metadata-load <store-root> <checkpoint-digest> <previous-digest|none> <plan-digest>
```

Load has no input-file capability. It addresses only the one pair named by all
three independently supplied selectors. The store repeats ordinary canonical
checkpoint and plan restoration while holding the selected entry. Output binds
the restored checkpoint and plan digests and carries their exact canonical JSON
bytes as `checkpoint_json` and `plan_json` strings. A missing, malformed, stale,
substituted or tampered selector/pair fails closed.

All four commands are in the typed public CLI catalogue and therefore
available in the standalone and full command surfaces. Arguments are exact
positional operands; options, omitted selectors and implicit predecessor
selection are not admitted. `none` is the sole spelling for an absent
predecessor.

## Authority boundary

The explicit paths authorize only the ordinary reads and immutable metadata
publication named by the selected command. Inventory conversion, planner
output, receipts and restored values carry no GC, source, approval or
publication authority. They carry no root or file handle.

No command can:

- list or discover roots, entries, subjects or selectors;
- choose a latest, newest or fresh checkpoint;
- initialize, repair, overwrite, adopt or delete store contents;
- execute a GC plan or delete an image, candidate or draft;
- restore source, image, candidate, draft or environment state;
- grant approval, deployment, execution or publication authority; or
- add a protocol request, daemon route or ambient filesystem capability.

Load is metadata inspection, not evidence that the selected checkpoint is
current. Persist settles only the exact checkpoint/plan envelope; a successful
pivot does not approve the plan or authorize a later effect.

## Evidence status

The existing semantic retention store harness contains an authored CLI
round-trip that supplies separate checkpoint, plan and root paths, checks the
authority-neutral receipt and exact restored bytes, and rejects a wrong plan
selector without removing the stored pair. That regression has been compiled
but intentionally not executed. The declaration conversion, canonical
observation-inventory parser, typed planner dispatch and closed help-catalogue
gate are authored and have compile-only validation; no inventory-conversion or
planner test was executed. The completion status remains Partial until the
completion matrix's required executable gate is run and recorded.
