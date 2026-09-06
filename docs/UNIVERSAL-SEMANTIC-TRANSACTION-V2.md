# Universal Semantic Transaction v2

Status: additive bounded implementation; focused local evidence is required
before promotion.

Audience: compiler contributors, agent-tool authors, and reviewers of semantic
change evidence.

Universal Semantic Transaction v2 adds one authority-free
`ReplaceExpression` transaction over an exact immutable canonical workspace.
It reuses the authenticated expression catalogue and complete
`ProjectCandidate::replace_expression` rebuild. It neither changes nor accepts
the frozen Universal Semantic Transaction v1 envelope or artifact bytes.

## Exact envelope and operation

The intent schema is `semaprax.semantic-transaction.v2`. It retains the v1
top-level fields, one-operation cardinality, requested validation set,
`requested_authority: "none"`, canonical JSON rules, and size bounds. Its sole
operation is:

```json
{
  "expected_old_expression": "left + right",
  "expression_id": "<revision-scoped retained HIR identity>",
  "kind": "replace_expression",
  "replacement": {"kind":"i64","value":7},
  "target": "calculator.add"
}
```

`target` selects one explicit monomorphic top-level source function, including
an explicit monomorphic `main` declaration.
`expression_id` must be an actual replaceable body-expression identity from
that exact revision's Project expression catalogue. Methods, generic or
synthetic functions, contract expressions, implicit HIR nodes, ambiguous
source joins, and caller-invented identities remain closed.
`expected_old_expression` is the exact authenticated canonical source slice at
the selected span. Both the workspace revision and this old slice must match
before candidate construction.

`replacement` uses the existing closed Project Candidate typed-expression
constructor grammar. The compiler supplies no raw source or byte offset and
adds no imports, declarations, effects, or authority. Complete candidate
reparse and Project admission recheck type, expected ownership, effects,
contracts, loans, cleanup, native emission, and Wasm emission.

## Preservation and artifacts

After rebuilding, v2 independently reselects the corresponding authored AST
position and requires the same resolved type and ownership. Every source other
than the target source is byte-identical. In the target source, bytes outside
the authenticated expression span are identical; the new exact canonical
expression slice is reported in the impact and result.

V2 uses separately versioned intent, impact, review, result, evidence schemas
and digest domains. Evidence is deterministic and replayable but carries no
authority. Validation and replay do not mutate the retained generation,
candidate, or filesystem. Validation may append only the ordinary bounded
in-memory semantic-service history fact; replay appends no history.

The persistent semantic workspace service accepts and replays exact canonical
v2 bytes against its active immutable workspace revision. Its additive exact
routes select the retained workspace plus ProgramRoot-v2 or ProgramRoot-v3
digest before parsing, capacity accounting, or history work; validation appends
one ordinary bounded history fact and replay appends none. The one-shot route is:

```text
semaprax change preview <project> replace-expression <stable-id> <expression-id> <replacement-json> [--revision <digest>] [--evidence|--structural-diff]
```

The adapter checks the requested/current revision before catalogue lookup,
derives the old source slice itself, delegates validation to the persistent
service core, and prints the exact core result, evidence, or Candidate-derived
structural diff. It performs no commit, source write, refresh, or publication.

## Focused gate

```sh
cargo test --locked -p semaprax --test project_candidate \
  universal_semantic_transaction_v2 --no-fail-fast
cargo test --locked -p semaprax --test workspace \
  universal_semantic_transaction_v2_cli --no-fail-fast
```

The gate must cover exact ProjectCandidate/core/service/CLI parity, canonical
intent and artifact replay, stale workspace/expression/old-source rejection,
closed constructor and CLI grammar, unavailable or nonreplaceable identities,
type and ownership mismatch, exact outside-span preservation, result/evidence/
structural-diff output, zero filesystem writes, and unchanged v1 transaction
and command bytes.

## Nonclaims

This is not arbitrary graph editing, raw-source replacement, contract
replacement, generic or synthetic editing, comment/trivia-preserving editing,
behavioral equivalence, multi-operation composition, rebase or merge semantics,
publication, commit authority, or a general universal transaction algebra.
Universal Semantic Transaction v1 and its composition, query, review, service
transport, MCP, and legacy CLI wires remain unchanged.
