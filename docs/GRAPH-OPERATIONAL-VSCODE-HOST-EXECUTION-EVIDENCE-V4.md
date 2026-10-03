# Graph-operational VS Code host execution evidence v4

V4 extends the local, exact-subject installed-VSIX scenario from V3. It does
not replace the historical v1, v2, or v3 receipts, which remain evidence only
for their recorded subjects.

Run a clean committed subject with a provisioned Visual Studio Code product:

```sh
python3 scripts/graph-operational-vscode-host-evidence.py \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --node /absolute/path/node
```

The runner executes the eight owning Node controller files (97 tests), packs a
deterministic VSIX from the checked-out extension files, installs that VSIX in
a fresh `--extensions-dir`, and starts one actual Extension Host. The host test
rejects a loaded extension path other than that isolated installed package. It
opens current-project, selected-declaration, and candidate Explorer panels and
waits for each actual webview to report its completed initial render after its
checked summary and page reads.

The runner also executes the standalone HTML and focused Markdown commands from
the Explorer guide against the built compiler. It requires each installed VSIX
viewer asset, its manifest digest, and its corresponding embedded standalone
HTML asset to have identical bytes.

The generated, Git-ignored v4 envelope records the exact commit/tree, product
and tool hashes, installed VSIX bytes, Node controller TAP, compiler build log,
Extension Host log, generated guide outputs, and one closed host observation. It
proves this selected local installed-VSIX path and the exact shared viewer assets
in that package and standalone output only. It does not claim Marketplace
publication, hosted or cross-platform coverage, network isolation, or full
quality.
