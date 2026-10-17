# Reuse paid status insertion scratch

Status: local implementation and component evidence; hosted current-head CI remains required.

Audience: compiler contributors and reviewers.

The Windows unit-heavy job [117423478630](https://github.com/wavect/semaprax/actions/runs/39041964674/job/117423478630)
on `99f1f06d361e16bd937f4a6eda19703e78f0e525` reached the status identity directory's
comparison reservation in retry mode 2. Its five actual attempts ended in mode 6;
the original two-attempt assertion remains unchanged.

The ordinary active builder now reserves one boxed `IndexSearch` before insertion.
Each subsequent row resets every field in place. The complete original bit traversal,
full-byte/lane comparison, neighbour search, append, links and AVL rebalance still
run. The comparison and index commit are inline, so they create no owning search
result or nested comparison result. Every new borrowed/scalar local and both new
index nodes remain paid. The original comparison, search and commit stay compiled
as independent receivers, and other payers keep those routes. All current row,
producer, binding, invocation, sticky-refusal and publication checks remain.

Three new owning tests compare every link and key pointer, prove the held search
address is stable, retain all original work plus the three new alias operations,
measure reservation savings and final release, and exercise exact, one-short,
zero and reserved-floor backing admission. The 20 previous status-directory tests
remain complete. Component verification uses actual current implementation and
ReplayBudget bodies with an explicit HIR fixture and bounded-output adapter;
it does not establish a full current compiler or hosted CI result.

`sources.json` authenticates five complete predecessors and unique inverses,
including the complete previous source controllers and all three audit inventories.
Their old assertions, archived inputs and inverse prefixes are retained.
No builder/work quota, fixture size, attempt counter or expected result changes.

The serialized component checks passed all 23 complete owning test bodies on
Rust 1.88 and 1.98 (46 runs). The 512-row comparison reserves 1,255,520 bytes on
the owning route and 771,116 bytes with reuse, saving 484,404 bytes. These are
cumulative ledger reservations, not RSS. Compiler probes remain under the
unchanged 200 MiB process-tree cap; no full local Cargo build was attempted.
