# External three-file semantic merge

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Full resolved files are under this directory's src/hir paths; manifest.json records exact hashes. ROOT was read only. These files start from the actual conflicted merge working copies, retain all automatically merged non-conflict regions, then resolve five conflict chunks. No Cargo, tests, builds, formatting tools or runtime execution occurred.

resolve_expr: retain the paid lexical byte_ancestry entry for BOTH while condition and body. MAIN's arbitrary current &scope body alternative would let a manually authored while manufacture byte ancestry. MAIN's strict condition alternative would incorrectly remove the established surrounding byte ancestry. The lexical entry exists only after EnterByteLoop and is unwound with loop depth; this is stronger contextual evidence than a scope containing a SliceU8.

resolve_for: retain our inherited entry and exact source binding-ID check, so same-spelling shadow/local views and nonbyte sources cannot inherit the outer profile. Scan a byte body against byte_entry.or(current scope) only for an admitted byte source; keep nonbyte Vec/record/owned callers on the contextless strict scanner. This preserves the whole body/value pre-scan and actual outermost binding availability.

resolve_statement: retain our documented byte_scope parameter and complete value+body pending pushes. Add MAIN's authored source_byte_ancestry conjunction verbatim as an additional requirement on a nested For branch. For actual monomorphic execution the scanner locates the exact authored expression in the original function body and requires an unbroken named-byte For ancestry, resetting at closure boundaries and rejecting mixed owned/borrow/Vec ancestry. If execution is absent, only callers with an explicit established byte entry can lend context, as before. The source predicate does not replace the current typed-binding/source-ID check or complete bounded-loop body scan.

Nonconflicted src/hir/resolve_for/bytes.rs now supplies source_byte_ancestry(root:&ast::Expr,target:&ast::Expr,scope:&BTreeMap<String,Binding>)->bool. Its admits Binding predicate matches the merged scanner and resolve_for signatures; the helper is pub(in crate::hir) and callable from the sibling. Exact target pointer matching, cache depth refusal, closure reset and mixed-loop denial are preserved. No API adaptation or helper edit is needed. Main88884 Census/Slot ptr::addr_eq fixes are in separate nonconflicted validation files: retain data-address context identity plus the exact function pointer; do not revert them to trait-object ptr::eq.

minimum-hir-selectors.json lists seven exact owning library selectors. They cover manual-while refusal, paid lexical ancestry/unwind, new context coercion identity, complete roundtrip, whole subtree freeze, nearest-loop exits and same-root release. Retain existing foreign/restored/floor/sticky and nested allocation controls in the parent's already selected set; do not run full suites. Existing shared-loan interpreter/native/Wasm owners remain the backend qualification boundary. No gate was executed here.
