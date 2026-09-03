# Graph-operational Phase 0 execution evidence v3

Status: runner and closed evidence contract are authored at the current source
head; the v3 aggregate has **not been executed**. Historical v1 and v2 bundles
retain their exact-subject claims.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract reruns the selected Phase 0 evidence at one clean exact local
commit. It updates the aggregate for the v2 VS Code task-control scenario and
makes evidence status classes explicit in one canonical envelope.

```sh
python3 scripts/graph-operational-phase0-evidence.py \
  --node /absolute/path/to/node \
  --tsc /absolute/path/to/tsc \
  --typescript-package-root /absolute/path/to/typescript-5.8.3 \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --mcp-python /absolute/path/to/python-with-mcp-1.27.0
```

The aggregate fails unless every executed child binds the same commit and tree,
the repository stays clean and unchanged, all selected tools remain identical,
and every archived byte replays to its recorded digest and bundle identity.

## Closed evidence classes

The v3 envelope separates five classes. No consumer may infer one class from
another.

| Class | Meaning |
| --- | --- |
| `current_head` | Selected canonical-Git, client/MCP, generated product-workflow, VS Code, and independent MCP SDK components freshly executed against the one exact local `HEAD` captured by the envelope. |
| `exact_tag` | Tags observed at that commit; tag evidence is `not_selected` and no release-tag claim follows. |
| `provisioned` | Selected executions that use explicitly bound local Node, TypeScript, Visual Studio Code, or Python MCP SDK inputs. |
| `default_ignored` | Default-ignored tests are excluded from ordinary counts and named only when separately selected and passed. |
| `authored_unrun` | Present slices outside this aggregate, including the packaged TypeScript workflow over MCP; they remain unexecuted by this report. |

## Updated selected inventory

The canonical Git, managed publication, generated-client/MCP, and independent
Python MCP SDK dimensions remain the v2 selections. A new child selects the
closed generated Python/Rust/TypeScript review-to-publication workflow and its
ten hostile transitions; its TypeScript row is separately provisioned and its
transport remains the recorded direct v5 harness. It is not packaged-SDK-over-
MCP evidence. The editor component now
requires 57 standalone controller rows plus one real Extension Host row that
observes fixed startup policy, immediate zero-step cancellation, no released
report or source authority, dirty-buffer invalidation, and rejection of the late
task result.

The VS Code child bundle uses the v2 schema and v2 bundle domain. A v1 child can
never satisfy the v3 aggregate by retaining a compatible-looking payload.

## Nonclaims

Even after a successful v3 run, the aggregate does not establish an exact
release tag, remote-main status, hosted or cross-platform execution, OS network
isolation, Marketplace/VSIX behavior, full MCP conformance, packaged SDK-over-MCP
execution, native/Wasm runtime equivalence, full quality, completion-matrix
promotion, or graph-operational programme completion.
