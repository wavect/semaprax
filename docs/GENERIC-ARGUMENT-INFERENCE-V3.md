# Generic Argument Inference v3

Status: implemented private profile; **HOSTED GREEN** under the
[accepted v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Public generic ABI and support promotion remain separately gated.

Historical local witness: fifteen language checks, six direct inference/precheck
checks, five graph mapping checks, three private ProgramRoot replays and three
owned runtime corpora passed. Interpreter, native O0/O2 and Core Wasm agreed on
success, failure cleanup and evaluation-once probes. These counts describe that
local execution, not a new test run or the current evidence ceiling.

Audience: compiler contributors and reviewers.

This extends [v2](GENERIC-ARGUMENT-INFERENCE-V2.md) with nested omitted-call
result evidence and inference inside admitted generic functions. It preserves
the existing explicit substitution domains, value and ownership admission,
concrete instance closure, and [forwarding contract](GENERIC-EXPLICIT-FORWARDING-V1.md).

## Nested calls

A generic call without an authored type vector can provide evidence for an
enclosing call. The compiler derives its complete vector from its own arguments,
then substitutes that vector into its declared result type. It never executes
the call or treats the declared result as evidence that its arguments, effects,
contracts, or ownership are valid. Those checks still happen once in the
ordinary verification and resolution path.

The outer evidence walk shares its 4,096-node budget and 128-level depth limit
with nested omitted calls. Each visited call expression counts as one node;
its inspected argument expressions consume the same remaining budget at the
next depth. An explicitly typed or monomorphic call continues to supply its
declared result without inspecting its arguments for inference. Ordinary
checking still visits all arguments, including nested omitted calls there.

## Symbolic forwarding

A generic caller may omit the callee's complete vector when argument facts
uniquely determine every slot. In HIR, a symbolic result is identified by the
caller's persistent declaration identity and parameter index. An index from a
foreign owner or outside the caller's parameter list cannot be converted into
a caller type argument. Callee parameters are solved in declaration order;
source spelling does not establish owner identity.

The inferred vector must satisfy the existing explicit forwarding predicate.
Permutation, repetition, differing arity and admitted concrete arguments retain
their exact meaning. Inference adds no composite symbolic mapping operands,
constraints, return-context inference, or implicit ownership conversion.

Before concrete template checking, the source forwarding precheck derives
omitted vectors from lexical type facts and authenticates them just as it does
explicit vectors. Function parameters seed that environment; scoped bindings
must be derived from declared or supported static type facts. Unknown facts do
not justify a guessed vector. A shadowing declaration removes the previous
binding even when its replacement type cannot be derived. Pattern and loop
binders similarly hide outer facts; this version does not derive their new
symbolic types for the forwarding precheck. Calls depending on those unknown
facts retain the explicit-vector requirement. The precheck traverses at most
128 lexical expression levels and rejects omitted generic calls left beyond
that boundary, while existing explicit mappings retain their ordinary check. The unchanged call graph still rejects direct,
indirect and transitive generic cycles. Every existing admitted substitution is
checked, including combinations not reached by the entry point.

Neither the source precheck nor HIR evidence collection checks or evaluates
expressions speculatively. A binding's type grants no permission to reuse a
moved owner. Argument evaluation and staging remain left to right; ownership
transfers at the same checked boundary as an explicit call.

## Graph, roots and replay

No new graph schema is needed. Inferred identity forwarding retains v34;
nonidentity mappings select v35 from the checked symbolic template. Graph
mapping is associated by structural expression path, not by a search for equal
concrete types. Identity and swapped mappings remain different even when both
specialize to `<i64, i64>`. Unused templates retain their symbolic mapping facts.

Canonical source keeps the authored omission. Exact graph and ProgramRoot
replay rederive meaning from that retained source. Equivalent concrete instance
facts do not make explicit and inferred source revisions interchangeable.
Edited or reminted symbolic owners, indices, mappings and instance identities
fail the existing independent replay checks.

## Focused evidence

The named Linux inference selector exercises:

```sh
cargo test --locked --offline -p semaprax --lib generic_inference
cargo test --locked --offline -p semaprax --test language generic_argument_inference
cargo test --locked --offline -p semaprax --test language generic_function_hostiles_are_stable_and_fail_closed
cargo test --locked --offline -p semaprax --test ir graph_generic_mapping
cargo test --locked --offline -p semaprax --test workspace inferred_generic_instance
cargo test --locked --offline -p semaprax --test owned_data generic_owned_function_runtime::inference
```

Public generic descriptors and signatures remain separately gated. The accepted
v0.4.0 baseline supersedes the former local-only release status; it does not
establish public support or evidence for later code changes. Historical local
counts and any exact workflow records retain their original execution scope.
