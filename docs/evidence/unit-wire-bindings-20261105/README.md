# Complete unit report fixtures

Status: Local repair; hosted CI remains authoritative.
Audience: Compiler maintainers and CI reviewers.

These 22 complete report bodies bind ten original literal expectations to the
two native builder layouts used by CI. The original literal identifiers remain
in their owning tests and this manifest. Each replacement is a closed literal
pair; actual report hashing, domain, replay, order, no-write, lock and mutation
assertions remain independent and unchanged.

macOS bodies were captured from the existing owning unit fixtures using Rust
1.97.1, with the compiler package at test opt-level zero and dependencies at
one. The first failures also match the opt-level-one local baseline and macOS
CI. Portable expected bodies come from the complete public fixture captures.
Current Linux CI anchors the entire Change hash array and the Operations
derivation, child evidence and application hashes. Their other references are
checked against the complete parent bodies. Full portable unit execution is
still required; these fixtures do not claim a passing hosted run.

The verifier authenticates every body and literal tuple, checks all Operations
parent references, and compares every field across layouts. Only the declared
builder charges and their bound digests differ.

```sh
python3 docs/evidence/unit-wire-bindings-20261105/verify.py
```
