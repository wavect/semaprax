# Graph-operational VS Code host execution evidence v2

Status: authored and unrun. The earlier v1 bundle remains the strongest
executed local VS Code evidence until this complete v2 runner succeeds on one
clean exact subject.

Audience: editor integrators, compiler contributors, and programme reviewers.

This contract extends the focused saved-source Extension Host scenario with one
real compiler-backed candidate-test task. It proves the selected local editor
path only. It does not complete the graph-operational programme.

## Exact runner

Run a clean committed subject with an externally provisioned Visual Studio Code
application and absolute Node executable:

```sh
python3 scripts/graph-operational-vscode-host-evidence.py \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --node /absolute/path/node
```

The runner requires `product.json` to identify Visual Studio Code. It binds the
product executable, CLI, product metadata, exact Extension Host executable and
version. It creates fresh user-data, extension, policy, and calculator fixture
directories, then builds the exact-subject `semaprax` binary locked and offline
in a fresh target directory. Other extensions, updates, workspace-trust prompts,
and GPU use are disabled.

The startup v7 policy selects candidate preparation and exactly these reference
interpreter limits:

```json
{"max_execution_bytes":65536,"max_report_bytes":262144,"max_steps":100000}
```

It selects no build capability and no Git commit host. The extension contributes
no build, commit, publish, or source-write command. This is evidence that the
scenario lacks those authorities, rather than evidence that a hostile operating
system process was sandboxed.

## Extension Host scenario

The runner first executes the eight standalone Node controller files and
requires exactly 97 passes, including the candidate-test task, check-on-save,
and navigation controllers. These remain standalone Node evidence. It then
starts an actual Extension Host using `--extensionDevelopmentPath` and
`--extensionTestsPath`.

The host must:

1. discover and activate extension version 0.1.0 and the exact contributed
   command inventory — all 37 commands, in manifest order, each registered with
   VS Code, with no registered `semaprax.` command the manifest does not
   declare and no build, commit, publish, approve, Git-commit, package-install,
   or native-run command contributed or registered;
2. select the global compiler, manifest, and startup policy paths;
3. check a saved source outside the fixture workspace whose reported token
   follows a supplementary character, and require the published diagnostic to
   underline exactly that token in the editor's own UTF-16 columns;
4. require a compiler run whose output the adapter cannot classify — malformed
   output with status 1, and status 0 without a verified record — to retain the
   previous diagnostics rather than report a clean project, and require a
   believable verified run to clear them;
5. resolve declarations, callers, and code lenses for the importing module
   `src/app.spx` through the project that owns it, reach all three project
   sources, open the authenticated file a selection names, refuse both on a
   dirty buffer, and refuse a project-owned safe rename in favour of the
   session's replay-checked typed intent;
6. launch the freshly built compiler directly as `serve-workspace-mcp`;
7. open a candidate, apply the catalogued `rename_declaration` intention, and
   verify the read-only source diff without changing saved source;
8. invoke Run Candidate Tests and immediately invoke Cancel Candidate Tests;
9. observe the compiler's sticky cooperative cancellation, expose no passing
   report, and preserve every saved source byte;
10. start a fresh session, launch another test task, then dirty a real `.spx`
    buffer while the task is pending;
11. require the source epoch to invalidate the candidate and late task result,
    revert the buffer, and again verify that all saved fixture bytes are
    unchanged.

Steps 3 to 5 write only under the operating system temporary directory; the
runner independently re-hashes every fixture file before and after the run.

The task start response is held in `queued` state by the compiler. Immediate
editor cancellation therefore reaches the compiler cancel method before the
prepared evaluator executes its first charged node. The dirty-buffer case uses
the same monotonic cancellation request but rejects the terminal outcome because
the editor epoch changed. Neither path turns cancellation into a passing report.

## Envelope and boundaries

The private default destination is
`.semaprax/evidence/graph-operational-vscode-host/<commit>/<bundle-id>/`; it is
Git-ignored. The canonical `evidence.json` schema is
`semaprax.graph-operational-vscode-host-execution-evidence.v2`. It binds exact
commit/tree/current-head/tag state, repository inputs, host/tool/product bytes,
the two execution inventories, the closed v2 in-host observation, and four
authenticated artifacts:

- `controller-node.tap`;
- `compiler-build-cargo.log`;
- `vscode-extension-host.log`;
- `vscode-host-observation.json`.

The v2 observation shape is unchanged by the added steps: they either pass or
abort the host before it prints its single result marker. The observation closes
over the startup test limits, all-false editor authority,
explicit cooperative cancellation, pending-task dirty-buffer invalidation,
verified virtual diff, and unchanged source bytes. The runner refuses a dirty
subject, repository drift, tool drift, an unexpected command inventory, or a
different observation shape.

No v2 execution is claimed by this authored contract. A passing local run would
still not prove VSIX or Marketplace packaging, manual UI or accessibility,
minimum-version compatibility, remote/web hosts, hosted or cross-platform
behavior, MCP Tasks conformance, network isolation, target-runtime behavior,
source publication, full quality, task economics, or programme completion. The
test-only API exists only under `ExtensionMode.Test` and adds no production
command, policy choice, transport method, or authority.
