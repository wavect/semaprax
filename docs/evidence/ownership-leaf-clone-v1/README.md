# Ownership classifier leaf cloning

Status: private component evidence; current-main hosted qualification pending.

Audience: compiler maintainers and allocation reviewers.

[GEN-05B job 116955308912](https://github.com/wavect/semaprax/actions/runs/38895413747/job/116955308912)
refused an 8,224-byte ownership clone-census cursor with 5,032 unreserved bytes
remaining. The builder allowance and its reserved floor remain unchanged.

The ownership payer now authenticates its existing ledger, then recognizes the
complete set of leaf type variants. Scalars and type parameters have no recursive
backing; an argument-free nominal retains its shared declaration identity and
clones its empty vector without allocation. This branch calls the same derived
`Clone` directly. It never invokes the fixed-array payload census, acquires no
new owned carrier, and leaves the cursor-paid flag untouched. Callable types and
nominals with arguments retain every original census, depth limit, backing
reservation and clone operation. No semantic cache or diagnostic precedence
changes. A later recursive clone must still pay its cursor exactly once.

`function.json` freezes the complete immediate predecessor. Its compiled method
in `ownership_flags/before.rs` changes only its name and visibility. A fixed hash
and full-body source control permit only the new guarded branch. Every existing
ownership test and original saved receiver remains a byte-exact prefix.

The bounded local probe compiles the actual type enum and derived clone, shared
identity representation and operations, complete type payload census, current
payer, frozen original clone method, current budget core, allocation observer
and five actual test bodies. It first reproduced the old cursor debit. The new
leaf control covers every leaf variant, Unicode/NUL identity bytes, an exact
setup-only account, short/zero accounts and sticky refusal. Existing compound
and depth-514/515 controls retain every assertion; a mixed control verifies that
leaf cloning cannot prepay a later recursive clone. Source controls pin the
whole old method and unchanged fallback. Static standard-library execution
observes no leaf allocation and complete paid Drop for recursive clones.

The probe has explicit uncalled BTree sizing and diagnostic failure adapters;
it does not compile the complete HIR or reproduce the GEN-05B application.
Current-main CI, that original application and the full quality profile remain
required. Local compiler processes retain the 200 MiB child-tree cap and SSD
outputs.
