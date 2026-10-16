# Receiver source custody

Status: private test evidence; full compiler qualification remains pending.

Audience: compiler maintainers and source-custody reviewers.

Three whole-file formatting records predate later receiver changes. Their
original input hashes, formatting inverses, and predecessor hashes remain
unchanged. The separately named `semantic_receivers` test helper authenticates
the complete live inputs, reverses only the explicit receiver edits, and passes
the complete original inputs to those existing formatting proofs.

The packet covers the three replay caller annotations and branch clone hook,
the storage-backed oracle error propagation, and the additional live HIR source
inventory entries. Each inverse also reconstructs its complete archived input
from commit `87612d690`. Tests reject a changed current byte and a forged
predecessor digest for every record. Existing receiver implementations, source
assertions, and semantic regression tests remain compiled at their live paths.
The live source inventories still inspect the current modules and children.

The minimum path-array receiver adds explicit inverses for its four capacity
hooks and registration, plus three entries in each source inventory. The second
inventory has no formatting repair; its fourth named custody record directly
preserves the complete input from `091c0d6de`. Every original formatting record
and archive remains unchanged.

A fifth record preserves the complete `0ab1bb6cc` source-verifier declaration
receiver across its one presentation hook. The live receiver now supplements
the causal capacity witness with its already selected authoring repair. The
independent original public-signature verifier stays unchanged and compiled.
