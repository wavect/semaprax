# Graph-operational VS Code host execution evidence v4

V4 is the reproducible local gate for the installed VSIX Explorer journey. It
binds one checked-out source subject, one freshly built public `semaprax`
binary, one locally provisioned Visual Studio Code product, and the extension
bytes extracted from the VSIX that the runner installed. It does not replace
the historical v1–v3 envelopes, which remain evidence only for their recorded
subjects.

Run it only from a clean committed checkout with a private Cargo target
directory and a provisioned Visual Studio Code application:

```sh
python3 scripts/graph-operational-vscode-host-evidence.py \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --node /absolute/path/node \
  --build-target /absolute/path/inside-this-checkout/target/exp10
```

The runner refuses a dirty subject before it starts and rechecks the commit,
tree, tracked inputs, compiler bytes, tool bytes, installed VSIX bytes, and
fixture source bytes before it writes evidence. The target directory must be
private to this checkout. Do not point it at another worktree's `target/`.

## What runs

The runner executes these three local journeys in fresh temporary user-data,
extension, policy, and project directories:

1. Eight authored VS Code controller files, requiring exactly 99 Node tests
   with no failures or skips. These are controller evidence and are labelled as
   such; they are not represented as Extension Host execution.
2. The public compiler built with `cargo build --locked --offline -p semaprax
   --bin semaprax`. It creates an offline HTML overview and focused Markdown
   report from `examples/calculator-project`, using the commands in the
   [Semantic Explorer guide](SEMANTIC-EXPLORER-V1.md#standalone-first).
3. A deterministic VSIX made from the extension checkout, installed through
   the selected VS Code CLI into an isolated extensions directory. The actual
   Extension Host loads the extracted installed bytes, explicitly starts a
   saved-source session using machine-scoped compiler, manifest, and policy
   settings, renders current overview and focused Explorer panels, opens a
   candidate panel, validates the virtual before/after diff, then verifies
   dirty-buffer invalidation and session-controlled task cancellation.

The runner compares every viewer asset in the source tree, VSIX archive,
installed extension, and generated HTML report. The manifest hashes must agree,
so the installed extension and standalone report cannot pass while using a
different viewer version. It also records the report and VSIX as compact
artifacts in the envelope.

## Evidence envelope

The default destination is the Git-ignored directory
`.semaprax/evidence/graph-operational-vscode-host/<commit>/<bundle-id>/`. The
canonical envelope schema is
`semaprax.graph-operational-vscode-host-execution-evidence.v4`. It includes
the exact test inventories, product and tool identities, source input hashes,
the closed Extension Host observation, and these artifacts:

- `controller-node.tap`;
- `compiler-build-cargo.log`;
- `vscode-extension-host.log`;
- `vscode-host-observation.json`;
- `installed-extension.vsix`;
- `semantic-explorer.html`;
- `app-main.md`.

The output is local evidence. It does not claim Marketplace publication,
hosted execution, vendor signature verification, network isolation, manual UI
or accessibility review, browser execution, Firefox or Chromium coverage,
cross-platform support, full quality, or programme completion. Run the
generated-report browser selector separately when provisioned Chromium and
Firefox are available; record an unavailable browser as unverified rather than
as a passing journey.

## Explorer 10 local execution

The following local execution ran on the exact detached subject
`170a297e9007c613058c7cbfc66161f604868dc0` on Darwin arm64. Its compact
[canonical envelope](evidence/explorer-v4/170a297e9007c613058c7cbfc66161f604868dc0/evidence.json)
has bundle ID
`dfac6cd960e778c2a69a9401cf64d17229ae7604327d647e6b5c45ee6f30b998`.
It binds the compiler, VSIX, shared-asset, Visual Studio Code, source-input,
and command-log artifact identities. The captured
[offline Chromium view](evidence/explorer-v4/170a297e9007c613058c7cbfc66161f604868dc0/offline-chromium.png)
supplements the assertions below.

1. The runner built the public compiler offline, packaged and installed the
   VSIX, and passed 99 controller tests, one real installed-VSIX Extension Host
   scenario, and two standalone guide exports. The selected product was Visual
   Studio Code 1.140.0 on Darwin arm64. The Extension Host rendered current
   overview, current focused, and candidate overview panels; it also observed
   a verified virtual diff, explicit cancellation, dirty-buffer invalidation,
   and unchanged source bytes.
2. The same clean source subject was installed with
   `cargo install --path . --locked --offline --bin semaprax --root
   /private/tmp/exp10-install --profile dev --target-dir
   /private/tmp/semaprax-351-evidence/target/exp10`. Its installed binary
   printed the closed `explore` grammar and generated the documented HTML,
   JSON, and focused Markdown reports. Each was byte-identical to the matching
   fresh-build artifact. The source-free JSON snapshot digest was
   `sha256:0acc621b1aef9f1cefc56e1b1c691abe24fc84e09b4bddc04a0ee10527d3cac1`.
3. With network disabled, the generated HTML and JSON opened in Chromium
   151.0.7922.34 and Firefox 153.0. Each browser passed 15 assertions: exact
   embedded snapshot identity, source-free state, restrictive CSP, inert
   executable surfaces, a usable module/inspector interaction, and no page
   errors. Chromium observed one `file:` request for the opened document;
   Firefox observed none.
4. The focused compiler integration selector
   `cargo test --locked --offline -p semaprax --test project explore_cli:: --
   --nocapture` passed all 11 selected cases in 8.24 seconds. Those cases cover
   source-free and explicit source-inclusive output, replayed candidate rename
   and move, removed base-side review, all four export formats, and rejected
   foreign, tampered, stale, oversized, or unsafe output cases.

This is local Darwin evidence for the recorded subject. It does not establish
other operating systems, browser versions, hosted execution, Marketplace
publication, or manual accessibility review.
