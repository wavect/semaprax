# Specialist commands

After this page you know the commands that no other chapter teaches, what each
one prints, and which ones are not products yet. Most people never need these;
they exist for tool builders, CI authors and compiler contributors.

Every analysis below reads one source file, changes nothing, and prints one JSON
envelope with a `schema`, a `digest` and usually `nonclaims` that list what the
output does not say. The output is data and never grants permission. Add
`--max-bytes N` to cap it; an overflow fails instead of truncating.

## Ask about one module

| Question | Command | What you get |
| --- | --- | --- |
| What authority must a build grant this module? | `capability-manifest <file>` | The capabilities it needs. Only filesystem, home, network, process and secrets are admitted; a module that uses `clock.read` fails with `SPX-K202`. |
| Which `protocol` declarations exist and what conforms? | `protocol-check <file>` | Declared protocols, signature rules and conformance. A file with none prints `"protocols_total":0`. |
| Which functions could be vectorized? | `simd-report <file>` | Eligibility facts for effect-free scalar functions. It emits no vector code. |
| What would typed generation add? | `hygienic-gen <file> [--templates default-constructor,field-accessors]` | Generated constructors and accessors, as output only. It never edits your file. |
| What does this function return for these arguments? | `interpret <file> --function <name\|id> --arg 1 --arg 2` | A JSON report of one interpreted call. `interpret-strings` does the same for functions that use strings. |
| How are values created and released? | `region-report <file>` | Lifetime structure. It adds no regions or arenas to the language. |

```sh
semaprax simd-report examples/meaning.spx
```

```text
{"schema":"semaprax.simd-report.v1","digest":"sha256:0c7487af...","bytes":2549,"payload":{...}}
```

For proofs, property tests and assurance policy see [Shipping](../projects/shipping.md)
and [Laws and proofs](../language/laws.md).

## Describe a module for other systems

| Command | Prints | Status |
| --- | --- | --- |
| `plugin-manifest <file>` | A read-only description of the module's compiled exports. | It loads and runs nothing. There is no plugin system yet. |
| `ui-schema <file>` | The schema and facts of the UI dialect for one module. | The UI runtime targets (iOS, Android, desktop, Linux) are not shipped. |

**WIT and components.** The compiler can derive a scalar WIT interface
(`semaprax:project-scalar@1.0.0`) from a project's web exports, but only as a library
call. There is no command for it, and a Component binary exists only behind a private
feature that is off by default. For WebAssembly use the Core Wasm web package
([Targets](../projects/targets.md)). Specs:
[Public scalar WIT](https://github.com/wavect/semaprax/blob/main/docs/PUBLIC-SCALAR-WIT-INTERFACE-V1.md),
[private WIT boundary](https://github.com/wavect/semaprax/blob/main/docs/WIT-COMPONENT-BOUNDARY-V1.md).

Java/Kotlin (JNI) and Swift/Apple bridges, and public generic signatures, stay
private or unsupported in 0.9.0. The
[completion matrix](https://github.com/wavect/semaprax/blob/main/docs/COMPLETION-MATRIX.md)
gives each row's status.

## Serve one file

```sh
semaprax serve examples/meaning.spx
```

```text
-> {"jsonrpc":"2.0","id":1,"method":"ping"}
<- {"jsonrpc":"2.0","id":1,"result":{"pong":true}}
-> {"jsonrpc":"2.0","id":2,"method":"context","params":{"symbol":"math.add","depth":1}}
<- {"jsonrpc":"2.0","id":2,"result":{"context":{"schema":"semaprax.agent-context.v1",...}}}
-> {"jsonrpc":"2.0","id":3,"method":"shutdown"}
<- {"jsonrpc":"2.0","id":3,"result":{"ok":true}}
```

`serve` checks one file once, then answers many newline-delimited JSON-RPC
requests on stdin and stdout. Methods: `protocol`, `graph`, `context`,
`context_v2`, `ping`, `shutdown`. Each result is byte-identical to the matching
CLI command. A request without an `id` is a notification and gets no reply. For a
whole project use `semaprax service` ([Shipping](../projects/shipping.md#serve-a-project-to-tools)).

Release archives also ship `semapraxd`. `semapraxd --stdio [--manifest-path
semaprax.toml]` is the older project session: it authenticates one project once and
answers graph, context and test requests over the same framing. Prefer `service`
for new work. Specs:
[Agent Transport](https://github.com/wavect/semaprax/blob/main/docs/AGENT-TRANSPORT-V1.md),
[Project Agent Transport v2](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-AGENT-TRANSPORT-V2.md).

`@semaprax/agent-workflow` (`packages/semaprax-agent-workflow` in a checkout) is a
Node package that drives one bounded workflow, review then publish a function
signature change, over a generated codec and an MCP transport your host supplies.
It opens no files, starts no processes and holds no secrets.

## Keep derived data

These commands store analysis on disk. Inputs are explicit, and what they
restore carries no authority.

| Commands | Purpose |
| --- | --- |
| `semantic-cache-*` (eight commands) | Reuse checked project analysis across processes. Covered in [Context performance](../practices/context-performance.md). |
| `retention-metadata-inventory`, `-plan`, `-persist`, `-load` | Decide which retained analysis subjects to keep, store the plan and its checkpoint, and restore them by exact digest. |

Spec: [Retention metadata CLI](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-RETENTION-METADATA-CLI-V1.md).

## Version and contributor gates

```sh
semaprax version --json
semaprax quality-plan quick
```

```text
{"schema":"semaprax.version.v1","version":"0.9.0","commit":null,"maturity":"beta","rust_min":"1.88"}
```

`commit` is `null` for a build that recorded none. `quality-plan quick|changed|full
[changed-path ...]` prints the gate list that the repository's `scripts/quality.sh`
runs for that profile. It is for people changing the compiler.

**Next:** [What Semaprax verifies and what it does not](trust.md).
