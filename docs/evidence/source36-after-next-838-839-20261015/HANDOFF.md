# Source36 successor after 838/839 and warning corrections

Basis `5f1fecd42694c52fbdcec67f9ef3d3fd0b7d7f3b`. Source-only; no Rust compilation/execution or global composition.

The four-view successor changes only the complete current `tests/hir_module_boundaries.rs` inverse to its same immutable94c392 checkpoint. The other three records remain object-identical. Both prior evidence packets, their JSON, runners, proofs and original archives are byte-identical; `prior-artifact-hashes.json` records every tracked artifact and the verifier compares each with the basis.

Only the actual Rust helper's manifest literal changes. a.rs remains byte-identical1500 lines; b.rs remains byte-identical, preserving every original undo/count/assertion. `source-inverses.json` supplies one complete immediate helper inverse with full original archive. The old four-view JSON and local successor-addition inverse remain separately under metadata-history, outside active source rows. No scope_checkpoint view is introduced: it is not a direct Source36 input.

The outboard `verify_source36.py` is the old runner with only its explicit manifest literal replaced. All36 original full-source Python assertions pass, including the original constructor/key/substitute/storage tail checks. `verify_adapter.py` authenticates the complete four views, helper-only edit and old artifact preservation. This is source proof, not runtime qualification.

Composition inputs: this packet's `source-inverses.json` contains the sole changed Rust row. The new source-projections JSON is an actual include_str input of the helper and has a separate local addition inverse; preserve this metadata join without recursively composing history JSON. Parent compositor owns global active rows and final source freeze.
