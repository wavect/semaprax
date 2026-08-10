# Bounded Diagnostic Repair v1 and Semantic Patch v3

This document freezes the first executable diagnostic-repair contract. It is
deliberately narrow: Phase A discovers and instantiates one repair for an exact
`SPX-S103` automatic function identity, and Phase B applies the resulting one-
operation Semantic Patch v3 through the unchanged single-file A0 transaction.
The repair classification is exactly `breaking_identity_rebase`.

This tranche is useful but partial. It is not general diagnostic repair, typed
holes, repair ranking, repository-wide change, or stable-identity-preserving
refactoring.

## Commands and public API

Discovery is read-only:

```text
semaprax repairs <file> assign-function-id <automatic-function-id>
```

Instantiation is also read-only and requires a caller-selected persistent ID:

```text
semaprax repair <file> <repair-id> --persistent-id <persistent-id>
```

The corresponding Rust API is
`DiagnosticRepairQuery::assign_function_id`,
`PersistentDeclarationId::new`, `repair::query`, and
`repair::instantiate`. Discovery returns compact canonical
`semaprax.diagnostic-repair.v1` JSON. Instantiation independently proves the
candidate and returns compact canonical
`semaprax.diagnostic-repair-preview.v1` JSON containing one exact
`semaprax.semantic-patch.v3` source string. Neither command writes source,
creates A0 artifacts, or commits the candidate.

The existing command

```text
semaprax patch <file> <patch.spatch>
```

does have commit authority. For v3 it accepts only the exact operation frozen
below, reruns the repair-domain and candidate-rebase gates, and then uses the
unchanged A0 lock, bounded staging, final source/stage rechecks, and atomic
rename. A successful apply returns the same candidate revision reported by
instantiation. Applying the same revision-bound patch again fails stale as
`SPX-G409` without changing source.

## Closed repair domain

The only query kind and operation kind are `assign_function_id`. The target
must be exactly one function that:

- has automatic identity;
- has an exact `SPX-S103` diagnostic on its function-name span;
- is neither `main` nor the resolved entrypoint; and
- does not already have an explicit persistent ID.

The complete source program must select `semaprax.graph.v10` and must have no
types, interfaces, permits, function templates, or function instances. Every
function is monomorphic, acyclic, effect-free, and contract-free. Parameters
are value-mode `i64` or `bool`, results are `i64` or `bool`, and expressions
are limited to scalar literals, places, nongeneric calls, unary and binary
expressions, blocks with scalar `let` statements, and `if`.

Records, variants, construction, projection, update, matching, postfix `?`,
aggregates, resources, generics, cycles, effects, contracts, interfaces,
imports, permits, and capabilities are outside this repair domain and fail
closed as `SPX-R101`.

The supplied persistent ID is a closed value type:

- length is 1 through 255 ASCII bytes;
- syntax is `[A-Za-z0-9][A-Za-z0-9._:-]*`;
- reserved prefixes are `auto:`, `core.`, `semaprax.`, `declaration:`,
  `function-execution:`, `parameter:`, and `nominal:`;
- reserved complete values are `bool` and `i64`; and
- the ID must not already occur in the declaration table.

Invalid, reserved, or colliding input fails as `SPX-R102`. Ordinary IDs such
as `helper` and `operation_failure` are admitted; executable evidence proves
that these strings do not confuse Graph property names or cleanup enum names.

## Repair identity and digests

Every digest uses SHA-256. Text components are encoded as an unsigned little-
endian 64-bit byte length followed by the exact bytes.

Every digest-valued protocol field, including repair IDs and source, patch, and
derived-rebase digests, uses the wire string
`sha256:<64 lowercase hexadecimal digits>`. Base and candidate Graph revisions
use the existing identical algorithm-tagged form. By contrast, the whole-
artifact SHA-256 KATs below are raw 64-character lowercase hexadecimal values
with no `sha256:` prefix.

The repair ID hashes this sequence:

1. domain `semaprax.diagnostic-repair-id.v1\0`;
2. base revision;
3. `SPX-S103`;
4. `function`; and
5. the automatic target ID.

Source and patch digests hash their domain, one little-endian 64-bit total byte
length, and the exact bytes. Their domains are respectively
`semaprax.diagnostic-repair.source-digest.v1\0` and
`semaprax.diagnostic-repair.patch-digest.v1\0`.

The derived-rebase digest starts with
`semaprax.diagnostic-repair.derived-rebase.v1\0`, then hashes every canonical
`kind`, `before`, and `after` text triple in ordered rebase-entry order, with
each text length-prefixed as above. Entries use set semantics and are sorted
lexicographically by the tuple `(kind, before, after)`. The closed `kind` enum
is `binding`, `expression`, `parameter`, or `result`. With zero entries, the
digest hashes the domain bytes alone. A repair ID from a different revision,
diagnostic, declaration domain, or target is unknown or stale and fails as
`SPX-R101`.

## Canonical report schema

The compact UTF-8 JSON report has top-level keys in this exact order:

```text
schema, source_graph_schema, base_revision, source, limits, budget,
query, diagnostic, repair
```

Its fixed values are `schema = semaprax.diagnostic-repair.v1` and
`source_graph_schema = semaprax.graph.v10`. Nested keys are ordered as follows:

```text
source: digest
limits: max_source_bytes, max_functions, max_call_sites, max_output_bytes
budget: used_source_bytes, used_functions, used_call_sites, used_output_bytes
query: kind, target
repair: id, kind, classification, applicability, input, operation
input: name, type, required, constraints
constraints: min_bytes, max_bytes, pattern, forbidden_prefixes,
             forbidden_values
operation: schema, kind, repair_id, diagnostic, target, name, to
to: input
diagnostic: code, severity, message, path, location, help
diagnostic location: line, column, start, end
```

The query and repair kind are `assign_function_id`; `classification` is
`breaking_identity_rebase`; `applicability` is `requires_input`; input name is
`persistent_id`; input type is `persistent_declaration_id`; and operation
schema is `semaprax.semantic-patch.v3`. The embedded diagnostic retains the
canonical order above and is the exact target `SPX-S103`. `location` is either
`null` or the ordered location object; `path` and `help` retain the canonical
diagnostic string-or-`null` representation.

## Canonical instantiation-preview schema

The compact UTF-8 JSON preview has top-level keys in this exact order:

```text
schema, source_graph_schema, base_revision, candidate_revision, source,
candidate_source, limits, budget, query, diagnostic, repair, patch,
identity_rebase
```

Its fixed values are `schema = semaprax.diagnostic-repair-preview.v1` and
`source_graph_schema = semaprax.graph.v10`. Nested keys are ordered as follows:

```text
source: digest
candidate_source: digest
limits: max_source_bytes, max_functions, max_call_sites, max_output_bytes
budget: used_source_bytes, used_functions, used_call_sites, used_output_bytes
query: kind, target
repair: id, kind, classification, input
input: persistent_id
patch: schema, digest, source
identity_rebase: before_id, after_id, name, direct_callers,
                 derived_id_count, derived_id_digest
direct caller: id, identity_origin, site_count
```

Direct callers are ordered by caller ID. They are direct source call sites,
not transitive impact. Their closed `identity_origin` enum is `explicit` or
`automatic`. `derived_id_count` and `derived_id_digest` bind the
complete admitted internal identity rebase; they are not a general proof-
carrying-patch or semantic-review artifact.

## Work and output limits

The contract has four hard limits:

| Limit | Value |
| --- | ---: |
| Source bytes | 16 MiB |
| Functions | 1,024 |
| Call sites | 65,536 |
| Report or preview bytes | 32 MiB |

These are fail-closed work/output limits, not truncation. No partial result is
returned. `used_output_bytes` is computed to a fixed point and equals the
exact compact JSON byte length; the newline printed by the CLI is excluded.

Immediately after parsing the AST and before any HIR resolution, both Phase A
and parsed-v3 preflight enforce the 1,024-function bound and structurally walk
all `requires`, body, and `ensures` expressions to enforce the 65,536-call-site
bound. The walk includes otherwise excluded expression shapes, so a hostile
program with unresolved callees cannot force HIR work past either bound;
overflow fails `SPX-R101` first.

A0 selects bounded source reads only after the patch text has parsed as v3.
For parsed v3, the initial source snapshot and both final source rechecks each
use the 16 MiB bound. Patch v1/v2 read behavior remains unchanged. An initially
oversized v3 source fails `SPX-R101`; concurrent same-identity growth beyond 16
MiB at either final boundary fails `SPX-I207`, preserves the grown source, and
cleans only owned A0 artifacts. Executable evidence covers initial oversize and
greater-than-16-MiB final growth without an unbounded reread.

## Breaking identity-rebase proof

Instantiation and v3 preflight independently admit exactly one canonical
source edit: insert `@id("<persistent-id>")` before the selected function.
The implementation also constructs the expected candidate by changing the
selected function's cloned AST identity and explicit-ID flag, canonically
formats it, reparses it, resolves HIR, and independently validates the HIR.

The structural comparison permits only:

1. the selected function declaration changing from the automatic ID to the
   supplied explicit persistent ID;
2. every revision-scoped derived identity owned by that function rebasing
   bijectively; and
3. direct call sites changing their callee reference from the old automatic ID
   to the new persistent ID.

Everything else, including names, types, ownership, control flow, values,
effects, contracts, entrypoint, templates, and instances, must remain exact.
The implementation serializes the before and candidate Graphs independently,
normalizes the candidate revision, selected identity-origin/persistent fields,
and admitted rebased identity fields back to the before values, and requires
exact JSON equality. An excessive candidate delta fails as `SPX-G112`.

This is intentionally a breaking identity operation. It does not preserve the
old automatic declaration ID or the revision-scoped IDs below the selected
function, and it makes no backward-compatibility claim.

The candidate therefore changes Graph-v10 revision and identity-bearing
content, including the selected declaration, direct callee references, and
derived IDs. Identity-bearing CleanupPlan content may rebase with those IDs.
The gate admits no Graph or CleanupPlan schema/version or semantic-shape
widening, no Graph v11-v14 repair domain, and no backend/runtime semantic
change.

## Semantic Patch v3 Phase B

The only admitted v3 file is exactly three LF-terminated lines with canonical
single-space separators and one final LF:

```text
schema semaprax.semantic-patch.v3
base <base-revision>
assign-function-id repair <repair-id> diagnostic SPX-S103 target <automatic-id> name <name> to <persistent-id>
```

Comments, blank or extra lines, CRLF, a missing final LF, doubled separators,
v1/v2 schema confusion, other instructions, and multiple operations fail as
`SPX-G101`. V3 preflight authenticates the base, repair ID, diagnostic, target,
target name, persistent ID, complete reduced repair domain, and exact identity-
rebase candidate before A0 gains commit authority. Selector/input failures use
`SPX-R101`/`SPX-R102`; stale base uses `SPX-G409`; excessive candidate delta
uses `SPX-G112`.

The A0 source/staging contract is unchanged. It authenticates the canonical
regular source, serializes cooperating writers with a create-new sibling lock,
uses bounded create-new staging, and rechecks exact source/stage identity and
bytes at both final commit boundaries before atomic rename. Unix device/inode
identity is exact. Windows holds same-file handles and compares volume plus the
available 64-bit file index; this does not claim ReFS 128-bit or hostile non-
unique-index uniqueness. Patch-file path/content provenance remains trusted
input, just as for Patch v1/v2.

Semantic Impact v1 remains exactly a Patch v1/v2 preview. It rejects every
syntactically valid, canonical v3 as `SPX-G110` before semantic selector
interpretation. Malformed or noncanonical v3 remains `SPX-G101`. Its v1/v2
output and frozen bytes are unchanged.

## Diagnostics

- `SPX-S103`: the exact existing automatic-function warning being repaired.
- `SPX-R101`: invalid query, ineligible or stale target/repair ID, closed-domain
  violation, or repair work/output bound.
- `SPX-R102`: invalid, reserved, or colliding persistent ID.
- `SPX-G101`: noncanonical or confused Semantic Patch v3 grammar.
- `SPX-G112`: candidate identity-rebase or output-accounting invariant failure.
- `SPX-G409`: stale patch base.
- `SPX-G110`: Semantic Impact v1 rejects syntactically valid, canonical Patch
  v3 before semantic selector interpretation; malformed or noncanonical v3
  remains `SPX-G101`.
- `SPX-I207`: final source identity, byte, or revision drift.

Existing parser, HIR, verifier, backend, and A0 diagnostics remain
authoritative where those layers reject first.

## Known-answer tests and evidence

The frozen raw SHA-256 known answers are:

| Artifact | SHA-256 |
| --- | --- |
| Canonical query JSON | `ef689fed2c742dea6cedb0b8ec3d449e5facd8748dd00cb8a8f2e6115be82075` |
| Canonical instantiation-preview JSON | `ae779749b252e5d9661172dfebcd3317211b97310eed57a0a6b7a692be1053e4` |
| Independently authored candidate Graph v10 JSON | `d255c0e88ff497436ca0737ffd139cf47c2c142cf1b4f2da071514c0515ad2b3` |

Local evidence is green: Diagnostic Repair Phase A integration is 13/13; the
Semantic Patch v3 Phase B semantic integration corpus is 7/7; v3 A0 hook units
are 4/4; aggregate v3 integration-plus-hook evidence is 9/9; and the library
suite is 404/404. The full local suite and preservation gates are green, and
independent security review is clean. The
focused suites are:

The 9/9 aggregate is defined as the seven Phase B semantic integration cases
plus two bounded-work integration-hook cases. It does not include or replace
the separate 4/4 internal v3 A0 hook-unit result.

```sh
cargo test --locked -p semaprax --all-features --test diagnostic_repair_v1
cargo test --locked -p semaprax --all-features --test semantic_patch_v3
cargo test --locked -p semaprax --all-features --lib patch::commit_tests::v3
cargo test --locked -p semaprax --all-features --lib
```

Evidence includes exact read-only inventories, final source drift/replacement/
growth failures, parsed-AST pre-HIR function/call hard bounds, parsed-v3-only
bounded initial and two-final source reads, greater-than-16-MiB concurrent
growth, canonical grammar and selector confusion, stale/failure no-write
behavior, exact generated/handwritten Graph and candidate revision
equality, strict Native C11 O0/O2 and Node/Wasm behavior when those tools are
available, CLI rejection, A0 artifact cleanup, Impact-v3 rejection, and Patch
v1/v2 plus Impact byte-preservation coverage.

Hosted evidence is pending. Local and security gates must not be described as
cross-platform hosted proof until an exact green workflow run exists.

## Explicit nonclaims

This milestone does not implement typed holes; repairs for other diagnostics,
declaration kinds, members, cases, types, resources, aggregates, generics,
effects, contracts, interfaces, imports, permits, or capabilities; repair
ranking, composition, automatic application, proof-carrying patches, semantic
review, repository-wide or multi-file repair; authenticated patch-file
provenance; Graph or CleanupPlan schema/version or semantic-shape widening;
Graph v11-v14 repair admission; or backend/runtime semantic change. The
admitted Graph-v10 revision/identity/callee/derived-ID rebase, including any
identity-bearing CleanupPlan rebase, is the operation rather than a nonclaim.

Patch v3 adds no other operation, batching, comments, optional whitespace, v2
operation composition, or general repair language. It retains A0's predictable
sibling collision/stale-lock denial-of-service, crash-left-lock, trusted final
portable directory window, parent-directory sync, power-loss durability, and
platform file-identity nonclaims. Direct callers are not transitive impact,
and a derived-rebase digest is not general semantic equivalence evidence.
