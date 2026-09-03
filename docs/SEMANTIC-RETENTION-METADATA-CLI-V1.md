# Semantic retention metadata CLI v1

Status: **Partial, authored/unrun**.

This contract exposes the immutable semantic retention metadata store through
two explicit command-line operations. It makes an authenticated checkpoint and
its exact companion plan durable and restores that metadata under caller-held
selectors. It does not make a retained subject current or actionable.

## Commands and capabilities

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

Both commands are in the typed public CLI catalogue and therefore available in
the standalone and full command surfaces. Arguments are exact positional
operands; options, omitted selectors and implicit predecessor selection are not
admitted. `none` is the sole spelling for an absent predecessor.

## Authority boundary

The explicit paths authorize only the ordinary reads and immutable metadata
publication named by the selected command. Receipts and restored values report
`authority: "none"` and false GC, source, approval and publication authority.
They carry no root or file handle.

Neither command can:

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
but intentionally not executed. The completion status remains Partial until
the completion matrix's required executable gate is run and recorded.
