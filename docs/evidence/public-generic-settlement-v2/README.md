# Local settlement evidence supplement

Frozen conformance subject: `888ac18e416303c3a78544194cf3937d9ff4c5ff`.
This supplements the release-evidence ticket without selecting a new public
release candidate or granting support/publication authority.

The owning selector passed **5/5** on a clean macOS aarch64 checkout. Frozen
v1 is **105 pass / 0 known defect / 21 N/A**; additive v2 is **123 pass / 0 known
defect / 30 N/A**. Actual local ASan, generated consumers, strict physical fault
receipts, skipped-release negative control and inventory/provenance negatives
executed. The subjects use checked source HIR emitted into authenticated native
providers; descriptor provenance is not a substitute for the compiled body.

## Complete inventory

[Download the complete compressed inventory](888ac18e-inventory.json.gz).
Decompression yields exactly 4785149 bytes, including the full matrix,
three source/descriptor/binding subjects, 210 artifact entries, six tool roles,
command arguments/effective cwd/explicit env, receipts and provenance. It is an
inventory, not a retained distribution or standalone replay bundle. Scratch
builds were removed and Cargo caches are excluded.

- Raw JSON SHA-256: `bb4565c7ee987b67d3ab12921f2ce6c94e140f9858b03359fb868c33d9954576`
- Gzip SHA-256: `fd3f0af238f2ab60bf3cfc4a06dfb56a4896302f34d4c42a6faec20f6ad8d888` (69320 bytes)
- Manifest status: `matrix_asserted`; Git status: empty.
- Rust: 1.98.0, Homebrew, `aarch64-apple-darwin` (exact observation in manifest).

## Scope

[Corpus §13](../../PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md) owns the fixed N/A
reasons. The existing v1 engine applicability is unchanged. Three additive
native ABI fault rows do not have injection seams in interpreter/Core Wasm/
TypeScript; these nine cells are explicitly N/A. They are not hidden failures
or substitute model execution. Private, unpublished identity/refusing/allocating
profiles and local macOS execution remain the evidence scope. This does not
complete unrestricted R07 native support, hosted evidence, PG-9 release approval
or public support/publication.

## Owning command

```sh
CARGO_TARGET_DIR=target/private SPX_PG_MATRIX_EVIDENCE=/absolute/chosen/evidence cargo test --locked --offline -j 1 -p semaprax --test public_generic_native_adapter_v1 settlement_matrix:: -- --nocapture --test-threads=1
```

## Complete v2 matrix

```text
semaprax.public-generic.settlement-matrix.v2
success-small                          interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
success-empty-leaves                   interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
success-2k-boundary                    interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
success-over-2k                        interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
success-max-leaf                       interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
repeated-lifecycle                     interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
export-short-capacity                  interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=N/A generated-rust=N/A generated-c++17=N/A generated-typescript=N/A
prepare-wrong-leaf-path                interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=N/A generated-rust=N/A generated-c++17=N/A generated-typescript=N/A
prepare-refusal-effect-free            interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=N/A generated-rust=N/A generated-c++17=N/A generated-typescript=N/A
injected-export-and-release-failure    interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=N/A generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=N/A
injected-prepare-allocation-failure    interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=N/A generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=N/A
contract-failure                       interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
contract-failure-repeated              interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
allocating-success                     interpreter=PASS native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=PASS generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=PASS
injected-input-transfer-commit-failure interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=N/A generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=N/A
injected-result-commit-failure         interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=N/A generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=N/A
injected-result-root-acquisition-failure interpreter=N/A native-c11-O0=PASS native-c11-O2=PASS native-c11-asan(local)=PASS core-wasm=N/A generated-c11=PASS generated-rust=PASS generated-c++17=PASS generated-typescript=N/A
```
