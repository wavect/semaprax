# Typed Project Declaration Change v1

Audience: compiler contributors and agents constructing Project candidates.

Status: authored, unrun implementation and regression evidence. Local compiler,
test, and long quality gates were deliberately skipped at the user's request;
this document makes no verified completion or target-execution claim.

The additive `add_declaration` intention creates one explicit, monomorphic,
non-`main` top-level function in an existing Project module. It travels through
the existing [Semantic Change and Candidate](PROJECT-CANDIDATES-V1.md) envelope,
revision checks, full source reconstruction, verifier, Project profile admission,
target projection, and exact replay. It does not edit canonical Git source or
change the Project manifest, imports, exports, capabilities, or module permits.
Existing intentions and image-v1 serialization are unchanged.

## Closed constructor

Every illustrated object key is required. There are no implicit parameters,
contracts, effects, source paths, or source-text defaults.

```json
{
  "kind": "add_declaration",
  "target": "calculator.add",
  "declaration": {
    "id": "calculator.increment",
    "name": "increment",
    "parameters": [{"name": "value", "type": "i64", "mode": "value"}],
    "return_type": "i64",
    "effects": [],
    "requires": [],
    "ensures": [{"kind": "bool", "value": true}],
    "body": {
      "kind": "call",
      "target": "calculator.add",
      "arguments": [{"kind": "place", "name": "value"}, {"kind": "i64", "value": 1}]
    }
  }
}
```

`target` authenticates one existing explicit monomorphic top-level anchor. A
`main` anchor is allowed because its signature and body remain unchanged. The
new function is appended after the module's existing function declarations;
the anchor does not grant a filesystem path or arbitrary insertion position.

The new ID is 1–128 ASCII bytes from `[a-z0-9._-]`. It must be unused in the
complete retained declaration graph and module bindings, including IDs from
other modules and nested declaration kinds. The display name is one bounded
ordinary identifier, cannot be `main`, and cannot collide with a function,
type, interface, protocol, or imported alias in the destination module.

| Parameter type | Required mode | Allowed result |
| --- | --- | --- |
| `i64`, `i32`, `u8`, `usize`, `bool` | `value` | Yes |
| `Bytes` | `own` | Yes |
| `str`, `Slice<u8>` | `borrow` | No |

Parameters must have distinct names and cannot be named `result`. There are at
most 64 parameters, 64 effects, 64 preconditions, and 64 postconditions. Effects
are sorted and unique, and every effect must already occur in both the anchor's
declared effect budget and the destination module's permits. Creation cannot
widen either budget. These list limits supplement existing bounded change
bytes, aggregate JSON nodes, constructor depth, and complete Project limits;
they are not a claim that arbitrary HIR memory is bounded by report size.

Bodies and contracts use the existing closed typed expression constructors:
bounded scalar literals, parameter places, stable-ID calls, unary/binary
operators, and conditionals. Requires predicates see parameters; ensures
predicates additionally see `result`. Calls resolve only through the
destination module's admitted local and imported function bindings. Initial
construction cannot call the function being created, and cannot introduce a
new import or directly name an inaccessible function. Actual types, argument
modes, borrow escape, ownership, cleanup, effects, contracts, and target support
remain the real compiler's responsibility. A syntactically valid expression
does not establish that its contract is mathematically true.

The ordinary candidate path formats and reparses these compiler-owned ASTs,
rebuilds all held Project sources, and checks exact preserved facts. The only
new declaration identity permitted is the planned explicit function at its
authenticated path/module. All prior explicit identities and their ownership,
all prior effect/contract inventory facts, module permits, and the manifest
must remain unchanged. Failed construction exposes no candidate and leaves
both the original candidate and source files unchanged.

## Reports, composition, and remaining boundaries

Only new-operation summaries add `new_declaration` with `id`, `name`, `path`,
and `module`. Impact includes the new identity with a null base-side report;
null denotes absence from the original source revision, not missing proof for
an existing function. Later candidate intentions can address this stable ID.
Semantic rebase and merge replay creation and subsequent changes in order,
check collisions on the new base, and retain the original comparison base and
parent bindings. Exact candidate replay reconstructs the declarations from
the retained source base and typed intentions; serialized AST/HIR never gains
admission authority.

`SPX-G225` reports malformed or unsupported declaration constructors, ID/name
collisions, inaccessible scope, and disallowed effect/mode requests.
`SPX-G226` reports constructor list capacity. Full source verification and
Project admission retain their ordinary diagnostics; stale candidate/change
digests retain the existing candidate diagnostics.

`src/project/candidate/declaration.rs` owns construction and the shared internal
`append_function` helper used by compiler-derived extraction. Parent candidate
dispatch and invariant/identity checks live in `candidate/mod.rs`; semantic
composition lives in `candidate/rebase.rs`. Authored regressions in
`tests/project_candidate_declaration_v1.rs` cover canonical replay without
source writes, creation followed by rename/body change and merge, a `main`
placement anchor with existing imports, ID/name collisions, unauthorized
effects, invalid ownership modes, result scope, raw-source fields, malformed
bodies, list bounds, and borrowed-byte forwarding to an owned-byte result.
These regressions have not been run.

This is function creation only. Creating types, records, variants, interfaces,
protocols, methods, generic functions, modules, public exports, new imports,
arbitrary structured types, or package entries remains outside this slice.
General recursive creation, new authority, independently verified target
execution, and full programme completion require additional evidence.
