# Graph-operational VS Code host execution evidence v1

Status: runner and Extension Host scenario authored, unrun.

Audience: editor integrators, compiler contributors, and programme reviewers.

This contract defines a focused local evidence bundle for the saved-source VS
Code adapter. It is independent of the canonical-Git and generated-client/MCP
bundles. Passing it does not complete the graph-operational programme.

## Exact runner

Run a clean committed subject with an externally provisioned official VS Code
application and absolute Node executable:

```sh
python3 scripts/graph-operational-vscode-host-evidence.py \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --node /absolute/path/node
```

The runner requires `product.json` to identify `Visual Studio Code`, binds the
product executable, CLI, product metadata, exact Extension Host executable and
version, and creates fresh user-data, extension, policy, and calculator fixture
directories. It builds the exact-subject `semaprax` binary locked and offline.
The VS Code process receives only absolute paths and runs with other extensions,
updates, workspace trust prompts, and GPU use disabled. This is local provisioned
evidence; it is not a vendor-signature, network-isolation, minimum-version, or
cross-platform claim.

The runner first executes the five standalone Node controller files and requires
exactly 50 passes. It then starts an actual Extension Host using
`--extensionDevelopmentPath` and `--extensionTestsPath`. The host discovers and
activates extension version 0.1.0, verifies all 26 contributed commands, selects
global path settings, and launches the freshly built compiler directly as
`serve-workspace-mcp` under a v7 candidate-only policy.

The host opens a candidate, selects `calculator.add`, creates and applies the
catalogued `rename_declaration` typed intent, obtains a compiler-verified
read-only virtual source diff, and checks base `add` versus candidate `addition`.
It then dirties a real `.spx` editor buffer and requires candidate and virtual
views to invalidate while every saved fixture byte remains unchanged. Stop must
terminate the session and release virtual documents.

## Envelope and boundaries

The private default destination is
`.semaprax/evidence/graph-operational-vscode-host/<commit>/<bundle-id>/`. It is
Git-ignored. The canonical `evidence.json` schema is
`semaprax.graph-operational-vscode-host-execution-evidence.v1`; it binds exact
commit/tree/current-head/tag state, repository inputs, host/tool/product bytes,
two execution inventories, the closed in-host observation, and four artifacts:

- `controller-node.tap`;
- `compiler-build-cargo.log`;
- `vscode-extension-host.log`;
- `vscode-host-observation.json`.

The evidence does not claim that standalone controllers ran inside VS Code. It
does not prove VSIX or Marketplace packaging, manual UI usability, accessibility,
remote/web hosts, cancellation, hosted or cross-platform behavior, network
isolation, source publication, target runtime, full quality, or programme
completion. The test-only API exists only when VS Code supplies
`ExtensionMode.Test`; it queues deterministic picker values, invokes the same
internal command implementations, and exposes a read-only state snapshot. It
adds no production command, configuration, transport method, or authority.
