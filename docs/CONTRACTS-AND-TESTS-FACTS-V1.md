# Contracts and Tests Facts v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: compiler contributors, semantic-workspace implementers, and
reviewers of contract and declared-test identity.

Contracts and Tests Facts v1 is an authority-free, content-addressed inventory
derived from one already-admitted immutable `ProjectRevision`. It closes the
first bounded contract/test association gap without changing the frozen
`ContractsAndTests` node of Canonical Semantic Workspace Revision v1.

This object records compiler-admitted declarations and their contract clauses,
plus declarations selected by the Project's test-module convention. It is not
contract proof, test coverage, test execution, or a test result.

## Public API and bounds

`src/project/contracts_and_tests_facts.rs` exports:

```rust
pub const CONTRACTS_AND_TESTS_FACTS_SCHEMA: &str =
    "semaprax.contracts-and-tests-facts.v1";
pub const MAX_DECLARED_CONTRACT_FUNCTIONS: usize = 4096;
pub const MAX_DECLARED_CONTRACT_CLAUSES: usize = 16_384;
pub const MAX_DECLARED_TESTS: usize = 4096;
pub const MAX_CONTRACT_SOURCE_FACT_BYTES: usize = 256 * 1024;
pub const MAX_CONTRACTS_AND_TESTS_FACTS_BYTES: usize = 8 * 1024 * 1024;

pub struct ContractSourceFact { /* opaque */ }
pub struct DeclaredFunctionContractFacts { /* opaque */ }
pub struct DeclaredTestFact { /* opaque */ }
pub struct ContractsAndTestsFacts { /* opaque */ }
```

`derive` accepts an `Arc<ProjectRevision>` and its exact expected Project
revision. It reads only retained admitted compiler facts. `replay` additionally
accepts the expected fact digest and submitted canonical bytes; it returns a
fresh derivation only after exact Project, digest, shape, and byte agreement.

## Contract association inventory

The `functions` array contains every retained function and function template,
sorted by stable-identity bytes. Each entry binds:

- `stable_id` and declaring `module`;
- `declaration_kind`, exactly `function` or `function_template`;
- ordered `requires` and `ensures` arrays.

Each clause fact binds its phase, zero-based position within that phase,
revision-scoped expression identity, checked type identity, and the compiler's
canonical structured expression fact. Clause order is declaration order and is
never sorted or repaired. Empty arrays mean that the admitted declaration has
no clause in that phase; they are not proof that an external contract is
absent.

The structured expression fact is a compiler projection of checked HIR, not
the original source substring and not executable authority. Display-only
formatting is not independently authenticated by this object.

## Declared test inventory

The `tests` array is sorted by stable-identity bytes and is restricted to the
manifest-declared test module. It contains the admitted `main` declaration as
`test_main` and each compiler-admitted named test declaration as `named_test`.
A named test follows the ordinary Project test selector: an explicitly
identified, zero-parameter function returning `i64` whose display name begins
with `test_`. Helpers or skipped `test_` candidates of another shape are not
silently promoted into test cases.

This is a declaration inventory. It does not say that a test was run, passed,
failed, reached a declaration, covered a clause, or proves a behavior.

## Canonical document and identity

The top-level document contains exact keys:

```text
coverage_claimed
execution_claimed
functions
limits
nonclaims
project_graph_digest
project_revision
schema
source_authority
tests
workspace_revision
```

`coverage_claimed`, `execution_claimed`, and `source_authority` are always
`false`. The Project revision, legacy workspace revision, and semantic-graph
digest bind the inventory to the same admitted subject.

JSON is compact, recursively key-sorted, UTF-8, and terminated by one LF. The
fact digest is lowercase SHA-256 over:

```text
"semaprax.contracts-and-tests-facts.digest.v1\0"
|| u64le(byte_length)
|| exact_canonical_bytes
```

Arrays retain their specified semantic order. Replay rejects malformed,
noncanonical, stale, cross-Project, reordered, substituted, truncated,
extended, oversized, or self-consistently reminted submissions.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-G574` | Invalid digest syntax, malformed/noncanonical facts, invalid fixed claims or inventory, duplicate identity, or exceeded bound. |
| `SPX-G575` | Stale Project selector, fact digest, Project association, or exact replay mismatch. |

Existing Project-admission and checked-HIR diagnostics retain precedence when
the input Project itself cannot be admitted.

## Compatibility and nonclaims

The exact ordered nonclaims are:

```text
not_a_program_root_segment
no_contract_proof_or_coverage_claim
no_test_execution_or_result_claim
no_source_execution_or_publication_authority
```

The standalone facts object is not itself a ProgramRoot segment. The additive
[ProgramRoot v3](PROGRAM-ROOT-V3.md) binds its schema, digest, and byte count by
descriptor; it does not embed or trust submitted fact payload bytes.

This version changes no Canonical Semantic Workspace Revision v1,
`ContractsAndTests` v1 node, ProgramRoot v1/v2, Project, managed Workspace,
Semantic Workspace Image, HIR, Graph, source, test report, execution result,
query, transaction, service, or target artifact byte or digest.

## Focused local evidence

The current three-case Workspace selector passes locally. It exercises exact
derivation and replay; function/template and ordered requires/ensures
association; ordinary named-test selection including exclusion of a
parameterized `test_` helper; a contract change and one combined
function/test-body-only change; stale selection; noncanonical trailing bytes;
phase mutation; a self-consistent
fact remint; the complete-document byte ceiling; and the fixed false
coverage/execution/authority claims:

```sh
cargo test --locked -p semaprax --test workspace \
  contracts_and_tests_facts::
```

The gate establishes only the bounded association and declared inventory in
this document. It does not establish contract proof, coverage, test execution,
test results, or any authority.
