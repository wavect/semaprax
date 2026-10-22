# Compact-Node actual layout and nested-floor controls

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Basis35cc6325790e00f7bcbd0c0bedf51518939f9b47. Actual175 logs00 and05 are genuine failures and remain unchanged. This test-only correction is unexecuted. No production code, fee, limit, allocator probe or independent old receiver changes.

Layout: the owning actual Rust test measured compactNode24bytes, not the earlier predicted32; oldtwo-VecNode48 remains correct. Derive the inline payload using independent Label and usize Layout constituents/alignment; assert exact24/current equality, exactold48 and strictsmaller. Production continues using actual size_of<Node>; whole physical owners remain unchanged.

Floor: original childfloor4096 prevented all positive spend, so its refusal correctly left outerfloor0 unrefused. Install an actualparentfloor8192, pay one real childbyte, then floor the remaining4095. Both old/current node construction must refuse; child restoration really consumes1across the parentfloor. Preserve complete old/current result/Diagnostic/receipt/work equality and all previous sticky/refusal assertions, adding exactchild/parentdebit and floor receipt assertions.

The complete original file is retained with a unique whole-source inverse. It reconstructs exactGit35cc; only these two existing test bodies change. No new selector or role. Integration requires a new fullcurrent→35cc input view before the old compact844 sourceguard domains; immutable oldtables/hashes stay unchanged. Targeted rustfmt1.9.0 completed. No Rust compile/tests or Cargo executed.
