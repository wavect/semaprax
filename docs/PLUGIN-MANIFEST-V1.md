# Plugin Manifest Projection v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax plugin-manifest <file.spx>` is a deterministic, read-only
projection that derives one canonical digest-authenticated envelope
(`semaprax.plugin-manifest.v1`) describing a capability-limited plugin
descriptor for one verified module. It is the first executable slice of the
completion-matrix row "Plugins" under Application platforms. It performs no
Component Model runtime or packaging, no host loading or lifecycle
management, no versioning negotiation, no resource-limit enforcement, and no
hostile-plugin execution testing; it compiles nothing beyond the production
native projection used for verbatim signatures, executes nothing, and
changes no source.

## Command

```sh
semaprax plugin-manifest <file> [--max-bytes N]
```

- There is no selection flag: the manifest always describes the whole
  module, so two runs over the same bytes are byte-identical.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte
  limits) bounds the whole envelope. Overflow fails closed with `SPX-Q103`;
  output is never truncated or repaired.
- The output is one canonical compact JSON envelope plus one trailing
  newline.

## Admission model

The export admission profile mirrors Canonical ABI Report v1 and Interface
Package Report v1 exactly: a function is admitted as a provided export only
when it has an explicit stable identity, is monomorphic, declares no
effects, has only by-value direct `i64`/`bool` parameters, and returns
direct `i64`/`bool`. Every other function of the module is recorded as an
exclusion with one closed reason: `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, or `unsupported_result_type`. Exclusions
never abort generation; a module without admitted exports yields a valid
empty inventory. If at least one export is admitted, the production native
C11 lane must succeed; any backend diagnostic fails the whole command
closed.

## Plugin identity

The language has no version metadata convention today, so the descriptor
sources its identity fields from what exists plus one documented derivation:

- `name` — the module declaration name (`module examples.calculator;` →
  `"examples.calculator"`).
- `identity` — the domain-separated SHA-256 digest of the exact source bytes
  under `semaprax.plugin-manifest.identity.v1`.
- `version` — the first 16 lowercase hex characters of the identity digest:
  a build-hash-style version with no semver semantics and no
  versioning-negotiation machinery.

Independent replay re-checks that `version` equals exactly the leading 16
hex characters of `identity`, so neither field can be forged alone, and
`verify_envelope_against_source` re-derives both from the current source
bytes.

## Required host capabilities

The `required_capabilities` section is derived exactly like Build
Capability Manifest v1 derives its ambient-authority assertion and reuses
its helpers: module permits plus every declared function effect and every
interface-import effect form the required token inventory (an interface
permit that no import consumes is still fail-closed-checked but never marks
a domain declared). Every capability token anywhere in the module must sit
inside the closed five-domain vocabulary (`filesystem`, `home`, `network`,
`process`, `secrets`) or the whole command fails closed with `SPX-Q102`
before any bytes are emitted. The section asserts `none` or `declared` per
domain over that fixed vocabulary; the listed `capability_tokens` inventory
lets independent replay re-derive the section and reject any forgery even
when the outer digest was re-minted around it.

## Resource limits

The `resource_limits` section is explicitly empty by default and canonical:
`{"fuel":null,"memory_bytes":null,"table_elements":null}`. No limit can be
declared by this schema version and none is enforced; replay rejects any
other shape. This is a declaration of absence, not an enforcement mechanism.

## Envelope and verification

`plugin_manifest::generate` returns canonical compact JSON with fixed key
order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is
  the domain-separated SHA-256 of the exact payload bytes
  (`semaprax.plugin-manifest.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `plugin` (`name`, `identity`,
  `version`), `descriptor` (`functions_total`, `exports_admitted`,
  `exports_excluded`), `capability_tokens`, `required_capabilities`,
  `resource_limits`, `exports` (each with `stable_id`, `name`, interface
  `parameters`/`result`, verbatim `native64` `symbol`/`signature` under its
  own per-export domain-separated digest, and canonically rendered
  `requires`/`ensures` clauses), `exclusions`, `unavailable_sections`, and
  fixed `nonclaims`.

Each embedded `native64.signature` is extracted verbatim from the actual
production native C11 projection — exactly one prototype line must exist
per admitted symbol or the command fails with `SPX-Q104` — so every reported
signature matches the ABI the backend really emits.

`unavailable_sections` is the closed bytewise-sorted inventory of descriptor
sections this projection does not provide: `component_model_packaging`,
`host_lifecycle`, `hostile_plugin_execution_tests`,
`resource_limit_enforcement`, and `versioning_negotiation`.

`nonclaims` is the fixed honest-boundary statement:
`descriptor_projection_only`, `no_component_model_runtime_or_packaging`,
`no_host_loading_or_lifecycle`, `no_versioning_negotiation_machinery`,
`no_resource_limit_enforcement_or_declared_limits`,
`no_hostile_plugin_execution_tests`, `no_target_execution`, and
`read_only_no_source_changes`.

`plugin_manifest::verify_envelope` independently recomputes the outer
payload digest over the exact serialized payload bytes, re-checks the
declared byte count, replays the counts against the listed inventories,
checks every exclusion reason against the closed vocabulary, verifies
strict stable-id ordering, re-authenticates every embedded export-signature
digest, re-checks the capability vocabulary over every listed token,
compares `required_capabilities` with its re-derivation from those tokens,
compares `resource_limits` and `unavailable_sections` against their closed
canonical forms, and re-checks the internal identity/version consistency
before returning the verified summaries. Any mutation anywhere in the
envelope invalidates verification, and mutations of the closed sections
fail replay even when the outer digest was re-minted around the forgery.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-Q1xx` family: `SPX-Q101` options, `SPX-Q102` out-of-vocabulary
capability, `SPX-Q103` budget exhaustion, `SPX-Q104` envelope/backend
consistency.

## Evidence

Executable evidence lives in `tests/projections/plugin_manifest.rs` plus module
tests in `src/plugin_manifest.rs`: pinned golden envelope KATs over
`examples/calculator.spx`
(`sha256:5b70733d21c171280c236377e1c30bdd02b7aeda4a70e5ca6cf940cfb447f957`)
and `examples/meaning.spx`
(`sha256:135e4320d5a777eee4167ee43eb5dc75802d0460b8fefd94ff4602e7c5a105f9`),
byte-identical double runs, every exclusion reason exercised against real
programs, independent recomputation of the embedded export-signature
digest, tamper rejection per digest field including forged-but-re-signed
signatures, versions, capabilities, resource limits, sections, counts, and
reasons caught by closed replay, out-of-vocabulary capability rejection at
generation time, source-drift failure through both embedded digests,
fail-closed budget exhaustion, CLI exit-code contracts, cross-consistency
proving that `required_capabilities` equals the ambient-authority section
that `semaprax capability-manifest` embeds for the same program, and
cross-consistency proving that the listed exports carry byte-equal native
symbols/signatures to what `semaprax abi-report` admits. No Component Model
runtime or packaging, host loading or lifecycle hooks, versioning
negotiation, resource-limit enforcement, hostile-plugin execution test, or
any target execution is involved, and hosted promotion remains pending.

See also [CAPABILITY-MANIFEST-V1.md](CAPABILITY-MANIFEST-V1.md) for the
capability derivation this manifest reuses,
[PACKAGE-REPORT-V1.md](PACKAGE-REPORT-V1.md) for the sibling package
descriptor whose admission profile it mirrors, and
[ABI-REPORT-V1.md](ABI-REPORT-V1.md) for the native signature surface.
