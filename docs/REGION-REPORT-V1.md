# Region Structure Report v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax region-report <file.spx> [--max-bytes N]` is a deterministic,
read-only projection that describes one verified module's lifetime structure.
It is the first executable slice of the completion-matrix row "Regions/arenas"
under Language and safety, moving that row from Missing to **Partial**. It
implements no region inference, adds no region annotation syntax, introduces
no arena type, performs no bulk release, changes no destructor behavior,
executes nothing, and changes no source.

## Command

```sh
semaprax region-report <file> [--max-bytes N]
```

- There is no selection flag: the report always describes the whole module,
  so two runs over the same bytes are byte-identical.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-L102`; output is
  never truncated or repaired.
- The output is one canonical compact JSON envelope plus one trailing
  newline.

## Admission model

The admission profile mirrors Canonical ABI Report v1 exactly: a function is
admitted only when it has an explicit stable identity, is monomorphic,
declares no effects, has only by-value direct `i64`/`bool` parameters, and
returns direct `i64`/`bool`. Every other function of the module is recorded
as an exclusion with one closed reason: `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, or `unsupported_result_type`. Exclusions never
abort generation; a module without admitted functions yields a valid empty
inventory.

## Reported facts

All facts derive from what the existing borrow/move checking already proves;
nothing is inferred beyond today's checker. The payload carries, in fixed
key order: `schema`, `source` (`path`, `revision`, domain-separated source
digest), `limits`, `module` (`name`, `functions_total`,
`functions_admitted`, `functions_excluded`), `functions`, `exclusions`, and
the fixed `nonclaims` list. Each admitted function entry carries:

- `bindings` — every value binding (parameters, `let`/`let mut` locals, and
  match pattern bindings), ordered bytewise by binding id, where each id is
  the real resolved-HIR [`ValueId`]. Each binding reports its display name,
  `kind` (`param`, `local`, `match_pattern`), `mutable`, ownership mode,
  canonical type key, `def_offset` (the binding name token), 
  `last_use_offset` (the effective live-range end: the end of the innermost
  statement or block tail containing its last read, assignment, contract
  clause, or own-consumption; equal to the definition offset when the
  binding is never used — replay rejects any disagreement with `use_count`),
  and `use_count`.
- `regions` — the canonical region clusters: a partition of the bindings
  under the rule that overlapping live ranges `[def_offset,
  last_use_offset]` can never share one region, greedily clustered in the
  canonical binding-id order so the partition itself is deterministic.
  Disjoint ranges reuse the lowest compatible cluster.
- `escape` — per-function escape facts derived from parameter ownership:
  borrowed/shared parameter counts (always zero under today's admitted
  profile, which excludes view modes outright), their total, the identical
  non-escaping count, and the named enforcing check `SPX-O104`
  ("return-position borrow escape is rejected: a function cannot return a
  borrowed or shared resource as owned"). Every borrow that exists in the
  language today is provably non-escaping because of exactly that check.
- `moves` — consumption sites recomputed from the resolved call graph (a
  place passed to an `own` callee parameter, ordered by binding id then
  offset) plus the derived distinct `moved_bindings`. Under the admitted
  profile this section is empty today: `own` parameters are resource-typed
  and resources cannot be constructed inside admitted scalar functions, but
  the derivation and its replay stay fully general.
- `release_groups` — bulk-release grouping candidates: maximal sets of at
  least two bindings whose effective live-range ends coincide (co-dying
  bindings inside one statement or tail expression), ordered by end offset
  with members in binding-id order.

## Envelope and verification

`region_report::generate` returns canonical compact JSON with fixed key
order: outer wrapper `{"schema","digest","bytes","payload"}` where `digest`
is the domain-separated SHA-256 of the exact payload bytes
(`semaprax.region-report.payload.v1`) and `bytes` is their length.

`region_report::verify_envelope` independently recomputes the outer payload
digest over the exact serialized payload bytes, re-checks the declared byte
count, replays the module counts against the listed inventories, checks every
exclusion reason against the closed vocabulary, verifies strict stable-id and
binding-id ordering, re-derives the complete greedy region clustering from
the reported live ranges (so even a conflict-free but non-canonical
assignment fails), re-derives the escape totals against the reported
parameter ownership, re-checks move-fact ordering and derivation, and
re-derives the exact bulk-release groupings. `verify_envelope_against_source`
additionally binds the current source bytes to the embedded source digest.
Any mutation anywhere invalidates verification, and mutations of derived
sections fail replay even when the outer digest was re-minted around the
forgery.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-L1xx` family: `SPX-L101` options, `SPX-L102` budget exhaustion,
`SPX-L103` envelope/consistency failures.

## Nonclaims

This tranche makes no region-inference implementation claim, adds no region
annotation syntax, introduces no arena type, performs no bulk release at
runtime, changes no destructor behavior, executes no target, and changes no
source. Regions here are a checked structural report over existing
borrow/move facts — not runtime storage management.

## Evidence

Executable evidence lives in `tests/region_report_v1.rs` plus module tests in
`src/region_report.rs`: pinned golden envelope KATs over
`examples/calculator.spx` (`sha256:cdde79b66a970e57cf86c13bfcac02cdd6782d5c1ceda7949270f344d80ee1e1`)
and `examples/meaning.spx`
(`sha256:b18fcfcad70e4d71a1de7cc472782af86d08cb15224662930c526b048c890946`),
byte-identical double runs, every exclusion reason exercised against real
programs, cross-consistency proving every reported binding id equals the real
resolved-HIR inventory on four examples plus match-pattern fixtures,
clustering/release-grouping unit proofs (overlapping ranges never share a
region; disjoint ranges reuse one; co-dying ends form maximal groups),
tamper rejection per digest field including forged-but-re-signed forgeries of
region assignment, escape totals, enforcing check, counts, release groups,
move facts, and use/end agreement caught by closed replay, fail-closed budget
exhaustion, source-drift binding, CLI exit-code contracts, and the fixed
nonclaims. No region inference, arena runtime, destructor change, or target
execution is involved, and hosted promotion is not claimed.

See also [PACKAGE-REPORT-V1.md](PACKAGE-REPORT-V1.md) for the sibling
read-only package descriptor whose admission profile this report mirrors, and
[EXPLICIT-MUTATION-V1.md](EXPLICIT-MUTATION-V1.md) for the mutation surface
whose assignments extend reported live ranges.
