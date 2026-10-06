# Command catalog

Every `semaprax` command in 0.9.0, grouped by what you want to do, with the page
that teaches it. `semaprax help all` prints the exact grammar of each command on
your installed build; `semaprax help <command>` prints one.

`<input>` means a `.spx` file, a project directory or `semaprax.toml`.
`semaprax harness` exists only in release archives and `semaprax-full`
([Harness](../tools/harness.md)); everything else is in the standalone build.

## Write, check and run

| Command | Does | Page |
| --- | --- | --- |
| `new <dest> [--name n] [--template calculator\|library\|service]` | Creates a project in an unused directory. | [First project](../getting-started/first-project.md) |
| `project-scaffold --name n [--template t] [--layout frozen\|tables]` | Prints the same project as one JSON capsule; writes nothing. | [Manifests](../projects/manifests.md) |
| `check [<input>] [--json]` | Parses, resolves, type-checks, verifies. | [First program](../getting-started/first-program.md) |
| `fmt <input> [--check]` | Rewrites source to canonical form. | [Style](../practices/style.md) |
| `run <input> [--native] [--json] [--max-steps N] [--max-bytes N]` | Runs `main` in the interpreter; `--native` runs a single file as C11. | [First program](../getting-started/first-program.md) |
| `test [<dir>\|semaprax.toml] [--json]` | Runs every `test_` function in the project's test modules. | [Testing](../practices/testing.md) |
| `build <input> --target native\|native-callable\|web\|wasm\|npm\|oci` | Emits an artifact. | [Targets](../projects/targets.md) |
| `network-run <project> --fixture f.json [--arg s] [--stdin p]` | Runs a project against recorded network replies. | [Input and output](../language/io.md) |
| `dev <semaprax.toml> --jsonl\|--human` | Hot-reload session for the interpreter. | [Targets](../projects/targets.md#edit-and-re-run-hot-reload) |
| `interpret`, `interpret-strings <file> --function f --arg v` | Runs one function and prints a JSON report. | [Specialist commands](../tools/specialist-commands.md) |

## Inspect meaning

| Command | Does | Page |
| --- | --- | --- |
| `graph <file>` | The whole semantic graph as JSON. | [Explore](../practices/explorer.md) |
| `context <input> <id> [--depth N] [--max-bytes N] [--filters ...]` | Bounded facts about one declaration. With `--rust-index` it answers about a Rust import. | [Agents](../practices/agents.md), [Integrations](../projects/integrations.md) |
| `doc <file> [--json]` | Documentation generated from the graph. | [Explore](../practices/explorer.md) |
| `query <input> ...` | Finds declarations by kind, name, id, effect or call edge. Subforms: `declarations`, `symbol`, `context`, `impact`, `available-operations`, `--capabilities`. | [Agents](../practices/agents.md), [Shipping](../projects/shipping.md#change-with-review) |
| `explore <manifest> --format html\|json\|markdown\|svg --output p` | A visual project map. | [Explore](../practices/explorer.md) |
| `compact graph\|agent-definition\|context\|task-context\|api-surface\|candidate-diff` | Smaller replayable encodings of the same answers. | [Context performance](../practices/context-performance.md) |
| `context-benchmark <manifest>` | Measures the context size of maintenance questions. | [Context performance](../practices/context-performance.md#benchmark-context-size) |
| `explain <SPX-code> [--json]` | Confirms a code exists in this compiler. | [Diagnostics](diagnostics.md) |
| `help [<command>\|all\|language [topic]\|library [name]\|shapes [kind]\|diagnostic [code]]` | Offline help. | [Debugging](../practices/debugging.md) |
| `skills get <agent\|language\|graph\|stdlib\|packages\|effects>` | Version-matched agent guides as JSON. | [Agents](../practices/agents.md) |
| `fix --plan`, `repairs`, `repair` | Plan, then apply, the one repair offered (add a missing `@id`). | [Debugging](../practices/debugging.md#apply-a-one-step-repair) |

## Change by meaning

| Command | Does | Page |
| --- | --- | --- |
| `impact`, `review`, `patch <file> <patch.spatch>` | Preview, review and apply a one-file patch. | [Shipping](../projects/shipping.md#single-file-patches) |
| `patch-evidence[-v2]`, `verify-patch-evidence[-v2]`, `patch-with-evidence[-v2]`, `target-evidence` | Produce, verify and apply with replayed evidence. | [Shipping](../projects/shipping.md#single-file-patches) |
| `change preview\|rebase\|merge <project> ...` | Semantic changes to a project. | [Shipping](../projects/shipping.md#change-with-review) |
| `review <project> <transaction.json> [--evidence]` | Reviews a transaction file. | [Shipping](../projects/shipping.md#change-with-review) |
| `verify <subject> <change> <capsule.json>` | Replays an evidence capsule; the schema picks the verifier. | [Shipping](../projects/shipping.md#change-with-review) |
| `patch-receipt <project> render\|verify\|refusal\|verify-refusal\|compare\|evidence-summary\|evidence-page` | Receipts for transactions. | [Shipping](../projects/shipping.md#keep-candidates-and-images) |
| `semantic-workspace-init`, `workspace-snapshot\|graph\|context\|impact\|review` | Read a managed workspace of 2 to 32 files. | [Shipping](../projects/shipping.md#change-a-managed-workspace) |
| `semantic-workspace-change-preview`, `semantic-workspace-change-evidence`, `verify-semantic-workspace-change-evidence`, `apply-semantic-workspace-change-evidence` | Preview, evidence, verify, apply a workspace change. | [Shipping](../projects/shipping.md#change-a-managed-workspace) |
| `semantic-workspace-structural-change-preview`, `semantic-workspace-structural-change-evidence`, `verify-semantic-workspace-structural-change-evidence`, `apply-semantic-workspace-structural-change-evidence` | The same four steps for structural changes. | [Shipping](../projects/shipping.md#change-a-managed-workspace) |
| `semantic-workspace-operations-derive`, `semantic-workspace-operations-change-proposal`, `semantic-workspace-operations-evidence`, `verify-semantic-workspace-operations-evidence`, `apply-semantic-workspace-operations-evidence` | Derive operations, then evidence, verify, apply. | [Shipping](../projects/shipping.md#change-a-managed-workspace) |
| `workspace-init\|preview\|apply\|patch-evidence`, `verify-workspace-patch-evidence`, `workspace-apply-with-evidence` | The older `.wspatch` route. | [Shipping](../projects/shipping.md#change-a-managed-workspace) |
| `project-image`, `project-image-store`, `project-image-load`, `project-image-verify`, `project-symbol` | Disposable semantic images. | [Shipping](../projects/shipping.md#keep-candidates-and-images) |
| `project-candidate-preview`, `project-candidate-export`, `project-candidate-restore`, `project-candidate-persist`, `project-candidate-load`, `project-draft-persist`, `project-draft-load`, `project-candidate-git-publish` | Candidates, drafts and local Git publication. | [Shipping](../projects/shipping.md#keep-candidates-and-images) |
| `hygienic-gen <file>` | Prints generated constructors and accessors. | [Specialist commands](../tools/specialist-commands.md) |

## Serve

| Command | Does | Page |
| --- | --- | --- |
| `serve <file>` | One file over JSON-RPC. | [Specialist commands](../tools/specialist-commands.md#serve-one-file) |
| `service <project> [--mcp]` | One project over JSON-RPC or MCP. | [Shipping](../projects/shipping.md#serve-a-project-to-tools) |
| `serve-image`, `serve-candidates`, `serve-test-candidates`, `serve-diagnostics`, `serve-diagnostics-tested <manifest>` | Image protocol v1 to v4. | [Shipping](../projects/shipping.md#serve-a-project-to-tools) |
| `serve-workspace`, `serve-workspace-mcp <manifest> <host-policy.json>` | Image protocol v5 under a host policy. | [Shipping](../projects/shipping.md#serve-a-project-to-tools) |

## Agents

| Command | Does | Page |
| --- | --- | --- |
| `agent inspect <definition.json> [--profile]` | Prints an agent definition's graph. | [Agent programs](../agents/programs.md) |
| `agent run <definition> <task> <transcript> [--evidence\|--trace]` | Runs an agent against a recorded transcript. | [Agent programs](../agents/programs.md#run-a-recorded-transcript) |
| `agent replay <definition> <task> <transcript> <evidence>` | Replays and checks evidence. | [Recovery](../agents/recovery.md) |
| `agent skill [--require-schema s]` | Prints the installed agent skill. | [Agents](../practices/agents.md) |
| `verify <definition> <profile> <graph>`, `verify <manifest> <image.json>` | Replays agent and image evidence. | [Agent programs](../agents/programs.md) |
| `workflow inspect\|validate\|checkpoint\|dispatch` | Reads typed workflow files; runs nothing. | [Shipping](../projects/shipping.md#check-an-audit-capsule-or-a-workflow) |
| `harness <verb>` | The development harness (archive and full build). | [Harness](../tools/harness.md) |

## Package, lock and ship

| Command | Does | Page |
| --- | --- | --- |
| `add <dir> <package> <range>` | Adds a dependency row. | [Shipping](../projects/shipping.md#resolve-dependencies) |
| `lock [<input>] --write\|--verify\|--compare f\|--emit-interface\|--compare-interface f` | Pins and compares a project. | [Shipping](../projects/shipping.md#lock-the-interface) |
| `resolve <input> --target native64\|wasm32 --cache dir --write\|--verify` | Pins per-target dependency choices. | [Shipping](../projects/shipping.md#resolve-dependencies) |
| `fetch [--lock l] <cache> <subject.json>...` | Fills a local cache from subjects you hold. | [Shipping](../projects/shipping.md#resolve-dependencies) |
| `package report\|lock\|resolve` (aliases `package-report`, `package-lock`, `package-resolve`) | Package descriptor, lock and resolution for explicit inputs. | [Shipping](../projects/shipping.md#resolve-dependencies) |
| `registry search\|add\|lock\|fetch\|verify\|publish` | Offline registry document operations. | [Shipping](../projects/shipping.md#use-a-package-registry-file) |
| `audit inspect\|verify\|diff` | Audit capsules. | [Shipping](../projects/shipping.md#check-an-audit-capsule-or-a-workflow) |
| `release verify <dir>`, `doctor verify-release <dir> --trusted-root-sha256 h` | Verifies a downloaded release offline. | [Shipping](../projects/shipping.md#verify-a-release-you-downloaded) |

How far each of these is trusted: [What Semaprax verifies](../tools/trust.md).

## Interfaces and analyses

| Command | Does | Page |
| --- | --- | --- |
| `openapi`, `openapi-compat`, `c-header`, `abi-report`, `cxx-shim`, `cxx-package`, `freestanding-object` | Descriptions and headers for other systems. | [Integrations](../projects/integrations.md) |
| `plugin-manifest`, `ui-schema` | Read-only module descriptions. | [Specialist commands](../tools/specialist-commands.md) |
| `capability-manifest`, `protocol-check`, `simd-report`, `region-report` | Read-only analyses. | [Specialist commands](../tools/specialist-commands.md) |
| `assurance-policy`, `assurance-diff`, `assurance-manifest`, `project-assurance-manifest`, `properties`, `project-proof-check` | Proof and evidence accounting. | [Shipping](../projects/shipping.md#check-assurance-and-proofs), [Laws](../language/laws.md) |
| `semantic-cache-*` | Reuse checked analysis across processes. | [Context performance](../practices/context-performance.md#reuse-compiler-work-with-a-semantic-cache) |
| `retention-metadata-inventory`, `retention-metadata-plan`, `retention-metadata-persist`, `retention-metadata-load` | Plan and store retained-analysis metadata. | [Specialist commands](../tools/specialist-commands.md) |

## Toolchain

| Command | Does | Page |
| --- | --- | --- |
| `doctor [--profile id] [--target native\|web\|all] [--json]` | Reports the toolchain, offline. | [Targets](../projects/targets.md#check-the-environment) |
| `version [--json]`, `--version` | Version and maturity. | [Specialist commands](../tools/specialist-commands.md#version-and-contributor-gates) |
| `quality-plan quick\|changed\|full` | Prints the contributor gate plan. | [Specialist commands](../tools/specialist-commands.md#version-and-contributor-gates) |
