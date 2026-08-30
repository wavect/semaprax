# Semantic Target Evidence v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors.

Semantic Target Evidence v1 is a bounded, read-only projection of one admitted
single-file Semantic Patch v1/v2 operation set or the sole canonical Patch v3
identity rebase. It independently rebuilds exact base and candidate Graph JSON,
the compiler-derived capability manifest, production native C11 source, and a
structurally validated production Wasm core module. It reports digests, lengths,
and closed classifications; it does not execute a target or discover project
tests, and it grants no authority.

The authored [ordinary native String cleanup correction](NATIVE-INLINE-STRING-SETTLEMENT-V1.md)
intentionally changes String-bearing production C, and thus this report's
native digests and byte counts for those subjects. The schema and digest
domains do not change. String-free known answers remain frozen; this
correction does not refresh or establish execution of historical KATs.

## Command and public API

```text
semaprax target-evidence <file> <patch.spatch>
```

The public function is:

```rust
pub fn preview(source_path: &Path, patch_path: &Path)
    -> Result<String, Vec<Diagnostic>>
```

`semaprax::target_evidence::preview` returns the canonical report without a
terminal LF. The fixed-arity CLI prints those exact bytes followed by one LF.
Preview owns bounded source and patch bytes, uses the same pure Patch preflight,
checks parsed-AST work limits before HIR construction, and rechecks exact source
identity, bytes, revision, and size before returning. It never enters A0 and
writes no source or stage.

## Canonical report

The report is exactly one UTF-8 JSON line. Its top-level key order is:

```text
schema, source_graph_schema, base_revision, candidate_revision, source, patch,
graphs, capabilities, targets, limits, budget, nonclaims
```

Closed nested objects and their exact key order are:

```text
source: digest
patch: schema, digest
graphs: classification, base, candidate
graphs.base, graphs.candidate: digest, bytes
capabilities: schema, base_digest, candidate_digest, classification, added,
              removed
limits: max_source_bytes, max_patch_bytes, max_operations, max_declarations,
        max_callables, max_call_sites, max_graph_bytes, max_native_c11_bytes,
        max_wasm_core_bytes, max_output_bytes
budget: used_source_bytes, used_patch_bytes, used_operations,
        used_declarations, used_callables, used_call_sites,
        used_base_graph_bytes, used_candidate_graph_bytes,
        used_base_native_c11_bytes, used_candidate_native_c11_bytes,
        used_base_wasm_core_bytes, used_candidate_wasm_core_bytes,
        used_output_bytes
```

`schema` is `semaprax.semantic-target-evidence.v1`. The source Graph schema is
one of v10 through v14 and the Patch schema is v1, v2, or v3. Graph and target
classifications are the closed values `changed` or `unchanged`.

The `targets` array contains exactly these two ordered objects:

```text
native: kind, profile, base_digest, candidate_digest, base_bytes,
        candidate_bytes, classification
wasm:   kind, profile, validation, validator_version, validator_features,
        base_digest, candidate_digest, base_bytes, candidate_bytes,
        classification
```

The native values are `native_c11_source` and
`semaprax.native-c11.bootstrap.v1`. These bytes are production compiler-emitted
C11 source, not compiled objects, machine code, ABI evidence, or toolchain
attestation. The Wasm values are `wasm_core_module`, `semaprax.wasm-core.v1`,
`wasmparser_structural`, `0.256.0`, and `all`. Structural validation is not
runtime execution or multi-engine conformance.

## Graph and capability facts

Graph bytes are the exact canonical `graph::to_hir_json` projections. The
capability manifest has schema `semaprax.capability-manifest.v1` and key order
`schema, facts`; each fact has key order `kind, owner, value`. Facts use set
semantics and lexicographic structural order. The closed kinds are
`module_permit`, `function_effect`, `function_template_effect`,
`interface_permit`, `import_effect`, and `import_required_authority`.
Materialized generic instances must have exactly their template's effects and
carry no independent authority.

Base and candidate manifests must be byte-identical. Therefore
`capabilities.classification` is exactly `unchanged`, both digest fields are
equal, and `added` and `removed` are exactly empty arrays. A capability change
is outside this v1 domain and fails closed; the report is not a general
capability-flow proof.

## Digests, bounds, and diagnostics

Every digest has wire form `sha256:<64 lowercase hexadecimal digits>`. The
domains are the following ASCII bytes plus NUL:

```text
semaprax.semantic-target-evidence.graph-digest.v1\0
semaprax.semantic-target-evidence.capability-manifest-digest.v1\0
semaprax.semantic-target-evidence.native-c11-source-digest.v1\0
semaprax.semantic-target-evidence.wasm-core-module-digest.v1\0
semaprax.semantic-target-evidence.report-digest.v1\0
```

Each digest is
`SHA-256(domain || little_endian_u64(byte_length) || exact_bytes)`. Source and
processed-Patch bindings reuse Review v1's exact domains
`semaprax.semantic-review.source-digest.v1\0` and
`semaprax.semantic-review.patch-digest.v1\0` with the same framing.

| Limit | Value |
| --- | ---: |
| Source bytes | 16 MiB |
| Patch bytes | 4 MiB |
| Operations | 4,096 |
| Parsed declarations | 4,096 |
| Parsed callables | 1,024 |
| Parsed call sites | 65,536 |
| Graph bytes, each | 32 MiB |
| Native C11 source bytes, each | 32 MiB |
| Wasm core bytes, each | 16 MiB |
| Report bytes | 65,536 |

`used_output_bytes` is the exact API-return byte length and is rendered to a
fixed point. `SPX-G140` covers Target Evidence bounds, including Review G120
work bounds; `SPX-G141` covers typed projection invariants, including mapped
Review G121. Existing Patch/source diagnostics remain exact. `SPX-I208` is the
private final-check hook failure used by executable race evidence, not a new
public evidence-file input.

## KATs and executable evidence

Raw whole-report SHA-256 KATs, computed over the API report bytes without a
wire prefix or terminal LF, are:

| Patch schema | SHA-256 |
| --- | --- |
| v1 | `900ee398b20f8cb59d5e48be3c6b824ce9ede339d86f86403368e0f5b574cc95` |
| v2 | `ec432841ca9e4e6209b0b302ed6cfd1ab61810eeed903c7cf0e1e97d806c185f` |
| v3 | `dded215d3f185978788d72e3dfbef3d167264c37ac36a88f753ec458a56494e1` |

Target Evidence integration is 9/9 and its internal units are 4/4. The hosted
integration gate compiles and runs the exact candidate C at O0 and O2 and runs
the exact candidate Wasm through Node. Those executions validate the emitted
artifacts in CI only; they are not performed, recorded, or authorized by the
product report. The root library suite is 439/439; full workspace/all-target/
all-feature, release, host 11/11 and loader 26/26 doctests, rustdoc with
warnings denied, strict Clippy, formatting, diff, preservation, and security
gates are locally green. The exact
`fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793), with
[dependency job
93624123614](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123614),
[Ubuntu job
93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631),
[macOS job
93624123633](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123633),
[Windows job
93624123715](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123715),
[component job
93624123698](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123698),
and [MSRV job
93624123711](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123711);
all 12 jobs passed.

## Exact nonclaims

The report carries this ordered array verbatim:

```text
not_native_machine_code_or_toolchain_attestation
no_native_or_wasm_runtime_execution
no_project_test_discovery_or_execution
not_signature_or_authenticated_provenance
not_human_approval_or_policy
not_safe_compatible_or_abi_verified
not_target_verified_or_runtime_conformant
no_repository_or_multi_file_analysis
no_external_consumer_compatibility
no_general_capability_flow_proof
no_commit_authority
no_new_patch_graph_cleanup_or_runtime_semantics
```

This is not target verification, safety or compatibility proof, C object/
machine-code/ABI evidence, Wasm runtime or multi-engine evidence, test status,
signature, provenance, approval, token, Context, repository analysis,
multi-file transaction, consumer analysis, or new Patch/Graph/Cleanup/backend
semantics. It does not replace the planned multi-file architecture tranche.

The separate [Semantic Workspace Transaction
v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md) does not run Target Evidence, bind a
target report, or turn this projection into multi-file evidence. All report
bytes, KATs, execution/authority nonclaims, and compatibility boundaries above
remain unchanged.

[Semantic Workspace Patch Evidence
v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) explicitly carries
`no_target_evidence_or_evidence_v2_aggregation`; it neither invokes nor binds
this report. Target Evidence bytes, KATs, execution nonclaims, and no-authority
boundary remain unchanged.
