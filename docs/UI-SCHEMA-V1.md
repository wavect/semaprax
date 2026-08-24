# UI Dialect Schema Projection v1

`semaprax ui-schema <file.spx>` is a deterministic, read-only projection that
turns one verified module into one canonical compact JSON envelope
(`semaprax.ui-dialect-schema.v1`) describing its typed application schema. It
is the first executable slice of the completion-matrix row "First-class
application/state/UI dialect". It performs no rendering, provides no runtime,
touches no DOM, adds no typed update/view language constructs, no semantic
controls, no accessibility, navigation, localization, assets, platform blocks,
or custom rendering, executes nothing, and changes no source.

## Command

```sh
semaprax ui-schema <file> [--max-bytes N]
```

- There is no selection flag: the whole verified module is projected.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-U102`; output is
  never truncated or repaired. Malformed or out-of-bounds values fail with
  `SPX-U101`.
- The output is one canonical compact JSON envelope plus one trailing newline.

## Admission model

Every top-level type declaration and function is classified; nothing is left
implicit.

A record becomes a state-shape descriptor only when it has an explicit stable
identity, no type parameters, and fields that are all direct `i64`/`bool`
scalars. Every other declaration is recorded as an exclusion with one closed
reason: `automatic_identity`, `generic_type`, `resource_type`,
`variant_type`, or `mixed_field_types`.

A function becomes an action descriptor under exactly the Canonical ABI Report
v1 profile: explicit stable identity, monomorphic, effect-free, by-value
direct `i64`/`bool` parameters, direct `i64`/`bool` result. Every other
function gets one of the shared reasons `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, or `unsupported_result_type`. Exclusions never
abort generation.

State-shape offsets, sizes, and alignments come exclusively from the checked
Native64 compiler layouts (`aggregate_layout`); any missing layout entry fails
the whole command closed with `SPX-U103`. State shapes, actions, and
exclusions are ordered bytewise by stable identity.

## Envelope content

Payload members in fixed order:

- `schema`, `source` (`path`, `revision`, domain-separated source digest),
  `limits`;
- `module` (`name`, `records_total`, `functions_total`) and `inventory`
  (`state_shapes_admitted`, `actions_admitted`, `excluded`);
- `state_shapes`, each carrying `stable_id`, `name`, the embedded `layout`
  object (per-field `index`, `name`, `type`, `offset`, `size_bytes`,
  `align_bytes`, plus record-level `size_bytes`/`align_bytes`), and a
  domain-separated `layout_sha256` over the exact canonical layout bytes;
- `actions`, each carrying `stable_id`, `name`, `role: "action"`, the embedded
  `signature` object (parameter name/type list plus result type), and a
  domain-separated `signature_sha256` over the exact canonical signature
  bytes;
- `exclusions` with `kind` (`record`/`function`), `stable_id`, `name`, and
  `reason`;
- the explicit empty-by-default reserved UI section `controls`,
  `accessibility`, `navigation` — always empty arrays in this schema version,
  present as nonclaim fields so downstream consumers can detect their absence
  of meaning rather than infer it;
- fixed `nonclaims`: schema projection only, no typed update/view language
  constructs, no semantic controls, no accessibility, navigation,
  localization, assets, platform blocks, or custom rendering, no target
  execution, read-only/no source changes.

The outer wrapper is `{"schema","digest","bytes","payload"}` where `digest`
is the domain-separated SHA-256 (`semaprax.ui-dialect-schema.payload.v1`) of
the exact payload bytes and `bytes` is their length.

## Verification

`ui_schema::verify_envelope` independently replays one envelope: exact outer
key set and schema, declared byte count, recomputed payload digest, presence
and emptiness of every reserved UI section, and re-authentication of every
embedded state-shape layout digest and action signature digest by rebuilding
the canonical bytes from the parsed payload values. Any mutation anywhere —
including forgeries whose outer digest was consistently re-minted — fails
closed with `SPX-U103`. All diagnostics use the previously unused `SPX-U1xx`
family: `SPX-U101` options, `SPX-U102` budget exhaustion, `SPX-U103`
consistency/replay.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. The embedded source digest binds the
envelope to its exact input bytes.

## Evidence

Executable evidence lives in `tests/ui_schema_v1.rs` plus module tests in
`src/ui_schema.rs`: pinned golden envelope digests over
`examples/meaning.spx`, `examples/calculator.spx`, and `examples/records.spx`,
the pinned Point state shape equal to the checked Native64 layout (fields at
offsets 0/8/16, sizes 8/8/1, record 24/8) with in-crate cross-consistency
against `aggregate_layout` itself, action descriptors agreeing with Canonical
ABI Report v1 signatures for the same program, determinism double runs, every
record and function exclusion reason exercised against real programs, per-
digest-field tamper rejection including re-minted forgeries, budget
exhaustion, and CLI exit-code contracts. No rendering, runtime, DOM, or target
execution is claimed; hosted promotion remains pending.

See also [ABI-REPORT-V1.md](ABI-REPORT-V1.md) for the sibling read-only
projection tranche whose function admission profile this slice mirrors.

## Widened scalar profile (2026-08-24)

Schema Scalar Widening v1 admits state-shape records whose fields are any mix
of the full Copy-scalar surface — `i64`, `i32`, `u8`, `f32`, `f64`, `char`,
`bool` — and action descriptors whose parameters/results use the same
widened profile, mirroring the widened package-report/openapi admission
style. Field offsets, sizes, and alignments continue to come exclusively from
the checked Native64 compiler layouts (`aggregate_layout`), which already
define every widened scalar (4/4 for i32, f32, and char; 1/1 for u8 and bool;
8/8 for i64 and f64), and a missing layout entry still fails closed with
`SPX-U103`. The envelope schema stays `semaprax.ui-dialect-schema.v1`: no
additive bump was required because verification rebuilds canonical layout and
signature bytes from the parsed values and replays digests rather than
checking any closed type vocabulary, so pre-widening envelopes replay
unchanged and all prior pinned KATs remain green untouched. Record exclusion
reasons (including `mixed_field_types`) and function exclusion reasons are
unchanged and still closed.

Remaining nonclaims: strings, nested/named field types, variants, resources,
and generics stay outside the widened record profile; reserved UI sections
remain explicitly empty; no rendering, runtime, DOM, typed update/view
constructs, semantic controls, accessibility, navigation, localization,
assets, platform blocks, custom rendering, or target execution; read-only
with no source changes. Widened-type evidence lives in
`tests/schema_scalar_widen_v1.rs`, with in-crate unit tests comparing widened
state shapes directly against `aggregate_layout`.
