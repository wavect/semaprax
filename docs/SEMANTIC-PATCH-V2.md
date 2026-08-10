# Bounded Semantic Patch v2

Semantic Patch v2 is an additive, single-file transaction format for exact
identity-scoped source changes. Schema-less patches retain the v1 behavior and
the legacy `rename` domain: explicitly identified functions and resources only.

## Frozen grammar

The first non-comment v2 instruction is exactly:

```text
schema semaprax.semantic-patch.v2
```

It is followed by one `base`, zero or more operations, and optional requirements:

```text
base <sha256-revision>
rename <function-or-resource-id> to <identifier>
rename-member owner <record-or-variant-case-id> member <field-id> to <identifier>
rename-case owner <variant-id> case <case-id> to <identifier>
replace-call-type-argument expression <expression-id> template <template-id> old-instance <instance-id> index <canonical-u32> from <i64|bool> to <i64|bool>
require no-new-effects
```

Unknown schemas, a v2 operation without the schema line, a schema line after
another instruction, duplicate bases/requirements/selectors, non-canonical
indices, conflicts, and overlapping source spans reject before staging.

## Identity and transaction contract

All selectors resolve against the same verified pre-edit AST/HIR. Record fields
are addressed by `(record ID, field ID)`, payload fields by `(case ID, field
ID)`, cases by `(variant ID, case ID)`, and generic arguments by the complete
expression/template/old-instance/index/from tuple. Member and case owners and
targets must have explicit persistent IDs. Compiler-owned `Option`/`Result`
declarations and automatic identities are outside the writable domain.

Construction and record-update grammar has no shorthand, so only the exact
resolved label changes. Record and variant pattern shorthand is expanded from
`{ old }` to `{ new: old }`; the binding name, `ValueId`, and every `Place` root
remain unchanged. Every edit is computed before any text changes, applied in
reverse byte order, parsed and verified once, and committed through the existing
authenticated A0 staging/held-handle/final-reparse path.

The mandatory post-HIR semantic-delta gate compares Graph meaning after removing
only the exact admitted display-name fields, exact addressed call instance/type
arguments, and exact selected old/new materialized instance declarations. All
other declaration IDs, owner/index/type facts, binding names and IDs, places,
layouts, call facts, and CleanupPlan bytes must remain equal. Generic edits must
produce the grouped final argument vector and derived instance, and no other
reachable instance may change. The Graph lattice maximum remains v14,
lower-schema programs remain v10-v13, and CleanupPlan selection is unchanged.

## Stable diagnostics

- `SPX-G106`: duplicate, conflicting, no-op, or overlapping v2 edit.
- `SPX-G107`: wrong owner/kind/persistence domain or compiler-owned identity.
- `SPX-G108`: stale generic-call tuple, source/HIR index mismatch, or excessive
  post-HIR semantic delta.
- Existing v1 parse, name, stale-revision, effect, verification, and A0 I/O
  diagnostics retain their codes and behavior.

## Threat model and nonclaims

The source snapshot, lock, staging handle, sibling paths, final source reparse,
and atomic rename retain the A0 transaction checks. Failed and stale operations
leave source unchanged. The patch file itself is trusted input: A0 authenticates
the source and stage, not a concurrently replaced `read_to_string(patch_path)`;
callers that require patch provenance must snapshot/authenticate it externally.

This milestone does not add construction/update shorthand, full type renames,
aggregate/resource generic arguments, generic-template-to-generic-template
composition, transitive generic instance materialization, multi-file commits,
Graph v15, or a new CleanupPlan schema. The admitted v14 resolver materializes
generic instances only from monomorphic source calls, so the bounded reachable
delta is the directly selected old/new instance set.

## Evidence

`tests/semantic_patch_v2.rs` covers schema confusion, legacy v1 success,
noncanonical exact spans, every member use site (declaration, construction,
update, projection, flat/nested record and variant patterns), shorthand binding
identity, case/payload changes, same-name owner isolation, a single mixed atomic
batch, two indices on one call, unrelated same-instance calls, every tuple
dimension, automatic/compiler-owned/wrong-kind/cross-owner targets, collisions,
and no-write failures. The mixed transaction's canonical post-edit revision KAT
is `sha256:f2f344c5a19591dfde2aa65ffd21918464be0848f526d6a59b977af9394805a7`.
Its exact old/new call-instance KATs are
`semaprax.function-instance.v1:14:generic.marker:2:3:i644:bool` and
`semaprax.function-instance.v1:14:generic.marker:2:4:bool3:i64`. The focused
suite also applies a v2 call patch and executes the changed source under Clang
`-O0`/`-O2` and Node/Wasm (256 re-entries); this is local executable evidence,
not a new hosted-platform claim.

Focused gates:

```text
cargo test --locked -p semaprax --all-features --test semantic_patch_v2
cargo test --locked -p semaprax --all-features --test patch
cargo test --locked -p semaprax --all-features --test generic_functions
cargo test --locked -p semaprax --all-features --test executable_generic_function_backends
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```
