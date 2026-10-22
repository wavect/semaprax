# Visibility-only correction against immutable 912a

`ProviderProofs` and its `new` constructor already use the `expected_projection` ownership boundary. Its `authorized` method remained `pub(super)`, which admits only the `import_stub` subtree. The typed caller in sibling `borrowed_imports/typed.rs:200` therefore receives production E0624.

`visibility-only.patch` changes that single method to `pub(in crate::workspace_graph::expected_projection)`. Complete prior helper bytes are archived and the exact inverse reconstructs them. All method body bytes, authentication order, carriers and layouts remain unchanged; no source guard exemption is introduced. The retained independent original helper is untouched. External preparation performed no Cargo, tests, review or ROOT mutation.

Parent should apply only after the active frozen run terminates, archive its actual failure, and include this source in the one final custody refresh. Existing source audit owner remains `semantic_graph::expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root` (workspace harness); existing 825 typed tests cover the sibling route. No unrelated reruns requested.


Integrated after actual 912a build termination: exit101, 58.373725 seconds, one E0624 compiler error, zero tests executed. Complete actual receipts are preserved in `../failed-build-912a-20261015/`; every archived file hash is recorded in `failed-build-preservation.json`.

The final metadata refresh proves all existing 54 packets/565 rows plus the new complete helper inverse: 55 packets/566 rows, all 36 full original source reconstructions. Prior proofs/archives and dispatcher hooks remain intact. The source-only static check retains 91 private-layout sources, all 6246 Rust file size checks, three mirror assertions and the four typed projection audit joins. The same 136 selected owners remain required; no Cargo, compiler, runtime tests, gates or reviews ran for this correction. Queued directory packet95 was not integrated.
