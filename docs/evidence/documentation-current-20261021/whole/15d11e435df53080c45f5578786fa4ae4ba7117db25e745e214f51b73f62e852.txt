# Scoped production Clippy correction

Source-only patch from exactdeec161142b2f3e882e50b4450338c69c137e586. No Cargo/compiler/tests. The private worktree contains only the two selected tracked files; all other baseline entries are skip-worktree rather than copied. ROOT and the active runtime source are untouched.

At owned_suffix.rs308/386/451 each View already stores a borrowed Observations. Remove only the immediately dereferenced extra ampersand. The callee still receives exactly &Observations; no ownership/move/Drop/evaluation order or receiving price changes. We make no fee reduction or savings claim from the removed coercion syntax and do not repurpose a paid role.

SORT_CALL's eight usize receiver contributions remain exactly eight scalar widths. size_of::<[usize;8]>() is a type-only byte inventory (array size is eight element sizes); no runtime array, bit count, new value or allocation is introduced. usize::BITS would mean the wrong unit. No blanket lint allow, quota, cap, baseline or test/assertion changes.

Both complete immediate-before and candidate-after files and exact inverse are retained. Parent must compose exact physical source ingress for these two current files before unchanged877/878 historical owners; this patch does not retarget old packet rows or run source generation.
