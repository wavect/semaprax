# Reproducible typed application setup

`benchmarks/typed_application_setup.py` installs the two reviewed public v31
specimens, `shiftsim` and `catalog`. This is an operator workflow for manual
source qualification. It never installs application source in a paid campaign,
changes a prompt, executes a model, or alters the original 15/23/49 acceptance
requirements or the TypeScript baseline.

Use a compiler from the exact checked-out source revision after its owning
compiler gate. The build receipt uses the existing retained build-log format
`semaprax.loglens.compiler-build.v1`; its source/binary/command/log fields establish
byte bindings. Provider build and verification provenance must be independently
reviewed by the operator. The helper cannot manufacture that proof from metadata.

```sh
python3 benchmarks/typed_application_setup.py --application catalog \
  --repo "$EXACT_SOURCE_REPO" --compiler "$VERIFIED_COMPILER" \
  --compiler-source "$VERIFIED_SOURCE_COMMIT" \
  --compiler-sha256 "$VERIFIED_BINARY_SHA256" \
  --compiler-build-receipt "$RETAINED_BUILD_RECEIPT" \
  --output "$NEW_EXTERNAL_SETUP_DIRECTORY"
```

All supplied paths must have real directory ancestors; source, binary and receipt
files must be regular. The output directory must be new, outside the checkout and
owned by the operator, with no concurrent candidate/model writers. Unknown
bundled resolver/version/registry shapes fail closed; setup never downloads a
package or copies caches. The selected original manifest and source bytes are
checked against Git, then retained using `compiler_output_provenance.capture_inputs`.
Both generator invocations and their byte-exact replays use the unchanged private
bootstrap. Only after all four succeed are both module replacements and the
reviewed `app.command.spx` installed in another private tree.

The helper checks canonical source and the complete installed Project, records
its full source snapshot and an exclusive completion receipt, then prints the
regular `installed-project` path. There is no symlink candidate or atomic-directory
publication claim. Failed commands retain their status, stdout/stderr hashes and
partial generator artifacts without a completion receipt. These artifacts are
not successful source installation or acceptance evidence.

Generated complete modules retain the authored schemas. They are not wholly
compiler-authored files. The selected-input closure is explicit and incomplete
for the full compiler; the declared bundled-library closure is separately closed.
Original inputs, generated outputs, installed files, and the operator's three
build/test/run scripts have separate inventories. Model-authored tokens, fixed
context tokens and billing remain unknown. If this scaffolding is offered in a
future paid experiment, disclose it in that cohort's fixed harness context for
both arms; this helper does not change any current campaign setup.

Fresh qualification is a separate operation against the printed regular tree:

```sh
python3 benchmarks/catalog-tokens-v1/codex_campaign.py qualify \
  --repo "$EXACT_SOURCE_REPO" --compiler-source-ref "$VERIFIED_SOURCE_COMMIT" \
  --semaprax-bin "$VERIFIED_COMPILER" --candidate "$CATALOG_INSTALLED_PROJECT" \
  --authoring-profile semaprax-project-v31-collection-record-v1 \
  --output "$NEW_CATALOG_QUALIFICATION_DIRECTORY"

python3 benchmarks/event-sim-tokens-v1/campaign.py qualify-v6 \
  --repo "$EXACT_SOURCE_REPO" --compiler-source-ref "$VERIFIED_SOURCE_COMMIT" \
  --semaprax-bin "$VERIFIED_COMPILER" --candidate "$SHIFTSIM_INSTALLED_PROJECT" \
  --output "$NEW_SHIFTSIM_QUALIFICATION_DIRECTORY"
```

These explicit v31 successors retain the original functional corpus/oracles;
they do not relabel original v30/historical qualification. Setup checks source
only. Actual native acceptance, backend parity, maximum-shape execution, and
current compiler evidence remain their existing owning gates. LogLens49 and its
strong TypeScript setup are unaffected.

Owning mocked guard harness: `python3 -m unittest discover -s benchmarks -p
test_typed_application_setup.py`. No compiler/model execution occurs in its tests.
