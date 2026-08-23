# Interface Package Report v1

`semaprax package-report <file.spx>` is a deterministic, read-only projection
that describes one verified module as an interface-first package descriptor.
It is the first executable slice of the completion-matrix row
"Interface-first packages and target matrices" under Ecosystem
interoperability. It resolves no dependencies, writes no lockfile, maintains
no dependency model, hosts no package registry, runs no version-compatibility
engine or conformance suite, attaches no provenance, signatures, licenses, or
SBOM, executes nothing, and changes no source.

## Command

```sh
semaprax package-report <file> [--max-bytes N]
```

- There is no selection flag: the report always describes the whole module,
  so two runs over the same bytes are byte-identical.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-P302`; output is
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
abort generation; a module without admitted exports yields a valid empty
inventory. If at least one export is admitted, the production native C11 lane
must succeed; any backend diagnostic fails the whole command closed.

## Reported facts

The payload carries, in fixed key order:

- `package` — the module name plus `functions_total`, `exports_admitted`,
  and `exports_excluded`.
- `targets` — the complete target availability matrix: exactly
  `{"target":"native64","available":true}` and
  `{"target":"wasm32","available":true}`. No third target can appear, and
  independent replay rejects any demotion, addition, or reordering.
- `exports` — the sorted admitted export inventory, ordered bytewise by
  stable identity. Each entry carries the interface-first facts (display
  name, persistent stable ID, language-type `parameters` and `result`,
  canonically rendered `requires` and `ensures` clauses, declared
  `effects`) plus the exact machine signature: `native64.signature` is the
  prototype line extracted verbatim from the production native C11
  projection (`codegen::emit_c`); exactly one must exist per admitted symbol
  or the command fails with `SPX-P303`, so every reported signature matches
  the ABI the backend really emits, authenticated by its own
  domain-separated `signature_sha256`.
- `exclusions` — every non-admitted function with its closed reason.
- `unavailable_capabilities` — the explicit closed inventory this report does
  not provide: `compatibility_engine`, `conformance_tests`,
  `dependency_model`, `licenses`, `lockfile`, `package_registry`,
  `provenance`, `resolver`, `sbom`, and `signatures`.
- `nonclaims` — the fixed honest-boundary statement, including
  `report_descriptor_only`, `no_resolver`, `no_lockfile_or_dependency_model`,
  `no_package_registry_or_hosting`, `no_version_compatibility_engine`,
  `no_conformance_tests`, `no_provenance_signatures_licenses_or_sbom`,
  `no_target_execution`, and `read_only_no_source_changes`.

## Envelope and verification

`package_report::generate` returns canonical compact JSON with fixed key
order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.package-report.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `package`, `targets`,
  `exports`, `exclusions`, `unavailable_capabilities`, and `nonclaims`.

`package_report::verify_envelope` independently recomputes the outer payload
digest over the exact serialized payload bytes, re-checks the declared byte
count, replays the package counts against the listed inventories, compares
the target matrix and the unavailable-capability list against their closed
canonical forms, checks every exclusion reason against the closed vocabulary,
verifies strict stable-id ordering, and re-authenticates every embedded
export-signature digest before returning the export summaries. Any mutation
anywhere in the envelope invalidates verification, and mutations of the
closed sections fail replay even when the outer digest was re-minted around
the forgery.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-P3xx` family: `SPX-P301` options, `SPX-P302` budget exhaustion,
`SPX-P303` envelope/backend consistency.

## Evidence

Executable evidence lives in `tests/package_report_v1.rs` plus module tests
in `src/package_report.rs`: pinned golden envelope KATs over
`examples/calculator.spx` and `examples/meaning.spx`, byte-identical double
runs, every exclusion reason exercised against real programs, independent
recomputation of the embedded export-signature digest, tamper rejection per
digest field including forged-but-re-signed targets, capabilities, counts,
and reasons caught by closed replay, fail-closed budget exhaustion, drift
between generations, CLI exit-code contracts, and cross-consistency proving
that the listed exports equal exactly what `semaprax abi-report` admits for
the same selections (with byte-equal native prototypes) and what
`semaprax openapi` publishes as operations for the same program. No
resolver, lockfile, registry, compatibility engine, conformance test, SBOM
tooling, compiler, Node runtime, or any other target execution is involved,
and hosted promotion remains pending.

See also [ABI-REPORT-V1.md](ABI-REPORT-V1.md) for the sibling read-only ABI
descriptor whose admission profile this report mirrors, and
[CAPABILITY-MANIFEST-V1.md](CAPABILITY-MANIFEST-V1.md) for the capability
vocabulary of the same modules.
