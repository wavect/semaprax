# Graph-operational VS Code host execution evidence v3

V3 is the local, exact-subject runner for the installed VSIX variation of the
saved-source Extension Host scenario. It does not replace the historical v2
receipt, which remains evidence only for its recorded development-path subject.

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
opens current-project, selected-declaration, and candidate Explorer panels in
the host, while retaining the existing real compiler, saved-source, typed
intent, cancellation, and dirty-buffer invalidation observations.

The generated, Git-ignored v3 envelope records the exact commit/tree, product
and tool hashes, installed VSIX bytes, Node controller TAP, compiler build log,
Extension Host log, and one closed host observation. It proves this selected
local installed-VSIX path only. It does not claim Marketplace publication,
hosted or cross-platform coverage, network isolation, or full quality.
