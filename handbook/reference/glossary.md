# Glossary

Use this page when a word stops you. Each entry says how the handbook uses the
term and, where useful, where to learn more.

| Term | Meaning |
| --- | --- |
| ABI | The agreement about values, ownership, failure and calling conventions across a compiled interface. |
| Adapter | A separate program that gives the [harness](../tools/harness.md) one capability, such as repository context or command-output views. |
| Agent | A program with explicit task, state, proposal, authorization, operation and result roles. See [Agent programs](../agents/programs.md). |
| Artifact | A produced file or package, such as a binary, web package or OCI layout. |
| Audit capsule | One JSON manifest of digests tying together the evidence for a decision. See [Shipping](../projects/shipping.md#check-an-audit-capsule-or-a-workflow). |
| Authority | Permission to do something. A document with `"authority": false` or `none` grants none. |
| Backend | What executes or lowers checked code: the interpreter, the C11 native route or the Core Wasm route. |
| Binding | A name attached to a value, as in `let count = 3;`. |
| Borrow | Temporary read access to a value without taking ownership of it. |
| Bridge | The harness surface that lets an outside coding agent, such as Claude Code, use the harness. |
| Candidate | A proposed project revision, kept as data, that can be inspected, tested and stored before anyone publishes it. |
| Canonical | The one representation a format's rules select. `fmt` writes canonical source. |
| Capability | Explicit authority supplied for one operation. In the harness, a kind of service such as `context.repository`. |
| Capsule | A package of revision-bound data for inspection or replay. |
| Checkpoint | Saved execution state used for recovery. |
| Class | A type with fields and methods that can inherit from another class. Records have no methods. |
| Cleanup | Releasing owned values in the checked order when their lifetimes end. |
| Contract | A function's `requires` and `ensures` clauses. |
| Copy scalar | A basic value such as an integer or boolean, copied without consuming its owner. |
| Declaration | A definition that introduces a named thing: function, type, field, law. |
| Diagnostic | A compiler message with a stable `SPX-...` code. See [Diagnostics](diagnostics.md). |
| Digest | A hash identifying exact bytes. A digest is never permission by itself. |
| Doctor | `semaprax doctor`, the offline toolchain report. It never searches `PATH`. |
| Draft | A candidate that is not finished, stored so you can resume it. |
| Effect | An operation category a function declares with `uses`, such as `process.stdout.write`. A module allows effects with `permit`. |
| Entry point | The function where execution starts: `fn main() -> i64`. |
| Evidence | Data produced or checked for one claim about one subject and revision. It carries no authority. |
| Export | A declaration made available through a package interface. |
| Fail closed | Stop with a code and change nothing, instead of guessing. |
| Fixture | Fixed test input, or a controlled stand-in, used to make a run repeatable. |
| Generation | One complete immutable published state of a managed workspace. |
| Harness | Optional tooling that runs an agent-proposed repair through compiler checks. See [Harness](../tools/harness.md). |
| Hot reload | Swapping a checked revision into a running interpreter session between calls (`semaprax dev`). |
| HIR | The compiler's high-level representation after names and types are resolved. |
| Host | The environment that supplies runtime services, tools, storage or operation handlers. |
| Image | A disposable semantic summary derived from a project. It is never source. |
| Immutable | Not reassigned through the binding in question. |
| Import | A declaration selected from another module with `use function @id("...") from ... as ...;`. |
| Interface | A declaration of host operations (`import fn`) with their effects and failure mode. |
| Journal | An ordered record of progress used for recovery. |
| JSON-RPC | The request and response framing the servers use, one JSON object per line. |
| Law | A named rule tracked independently of any implementation. See [Laws and proofs](../language/laws.md). |
| LawSet | The selected laws and evidence requirements a project must account for. |
| Lock | `semaprax.lock`: the pinned identity, digests and interface of a project. |
| Manifest | `semaprax.toml`: a project's modules, tests, exports and dependencies. |
| MCP | Model Context Protocol, a standard way for an assistant to call tools. `service --mcp` offers one. |
| Module | A named group of declarations. A `.spx` file begins with its `module` line. |
| Move | Transfer ownership so the old binding cannot be used. |
| Nonclaims | A list in a report of what it does not establish. |
| Owned value | A value with one tracked owner responsible for its transfer and cleanup. |
| Patch | A `.spatch` file naming a graph revision and edits by stable id. |
| Postcondition | A promise about a result, written with `ensures`. |
| Precondition | A requirement on inputs, written with `requires`. |
| Profile | The rules a project selects for types, ownership, execution or packaging, such as `scalar` or `useful-data.v1`. See [Profiles](../projects/profiles.md). |
| Proposal | Typed input describing a requested action, before authorization. |
| Provider | In the harness, an adapter that supplies a capability. In `network-run`, the host side that answers network calls. |
| Reducer | Checked logic that combines state and an outcome to choose the next agent step. |
| Registry | A file listing packages and versions. Semaprax reads it offline. |
| Replay | Rechecking retained data against the subject and rules that give it meaning. |
| Resource | A value with a declared end of life, such as a handle. |
| Revision | The identity of one source or project snapshot, as a `sha256:` digest. |
| Scalar | One basic value: a number, boolean or character. |
| Semantic graph | Structured facts about declarations, types, effects, contracts and relationships. |
| Session protocol | A declared state machine for an interaction, checked and then erased. |
| Skill | Passive instruction text for an agent. It is data, not code. |
| Stable ID | The persistent identity written with `@id`, separate from the display name. |
| Stale | Based on an older revision than the current source. Stale input is refused. |
| Subject | The exact thing an evidence document is about, such as a package or a patch. |
| Tail expression | The last expression of a block, which gives the block its value. |
| Target | The selected output form: `native`, `web`, `wasm`, `npm` or `oci`. |
| Transaction | A canonical, revision-bound set of semantic edits that is validated before it is applied. |
| Typed hole | A marked incomplete part of a candidate with a known type, filled later under checks. |
| UTF-8 | The byte encoding of text. One character can take more than one byte. |
| Variant | A type whose value is one of several named cases. |
| Workspace | Several `.spx` files read or changed together as one managed set. |

**Return to:** [Essentials](../language/essentials.md),
[project profiles](../projects/profiles.md) or [Agent programs](../agents/programs.md).
