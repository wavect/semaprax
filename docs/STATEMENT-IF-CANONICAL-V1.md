# Statement If Canonical Source v1

Audience: language users, parser contributors, and formatter contributors.

Status: implemented source projection with focused local source/graph, interpreter, native, Core Wasm and formatter/cache accounting evidence.

Audience: compiler contributors maintaining canonical source and cache projections.

Canonical formatting preserves an authored statement `if`, its `else if`
chain, its explicit `else`, and its original branch values. It does not print
the generated discard binding, missing-else branch, or generated zero tails.
The existing Language Ergonomics v1 grammar and verified lowering remain the
semantic contract; this profile changes source projection only.

```spx
module statement_if_source;
@id("statement.main") fn main()->i64 {
    let mut count = 0;
    if true { count = count + 1; }
    if false { count = 0; } else { 0 }
    let _if1 = if true { 7 } else { 8 };
    count
}
```

The last two forms remain distinct: explicit `else { 0 }` is retained, and an
authored `let`, including one whose name looks like `_if1`, stays a `let`.
Value `if` expressions retain their ordinary canonical rendering. An all-valued
`if` followed by another statement is still the existing discarded statement
form; one at the block tail is still a value expression. Existing expression
statement and missing-final-value diagnostics remain unchanged.

## Provenance and normalization

A parsed statement `if` remains an ordinary normalized AST `Let` containing
its existing value `If`. `LetSyntax::StatementIf` records the original branch
tail categories and presence of the final alternative, with no alternate
expression tree. The parser also marks its generated nonliteral branch-tail
discards; authored lets carry `LetSyntax::Authored`. Only the parser constructs
the statement marker. Semantic visitors, source verifiers, resolver, loan and
cleanup proofs, HIR and every backend consume the same normalized value tree
as before. Syntax metadata grants no effect, ownership or runtime authority.

The formatter authenticates the complete marker against the normalized shape
before projecting it: absent tails must be literal zero, missing alternatives
must be empty zero blocks, chain wrappers must be empty blocks around `If`,
and discarded nonliteral tails must have the parser's discard marker. If a
program transformation invalidates that normal form, the formatter emits the
complete ordinary let/value expression. It never infers provenance from a
reserved-looking name or omits a changed else value.

Synthetic names still skip every identifier anywhere in the source file.
Since canonical statement spelling removes only generated names, reparsing
recreates the same hygienic normalized bindings and expression paths. Nested
statement `if`s, ordinary loop bodies and consuming-loop bodies use this same
normalization; no additional HIR statement, intrinsic, backend primitive,
prelude, graph or cleanup schema is introduced.

## Determinism and accounting

Canonical source is idempotent. Source-based graph revisions now bind the
preserved statement spelling. An explicitly authored lowered value-discard
program has its own canonical bytes and revision; it retains the same checked
behavior. Source drift still fails exact graph verification.

Both multiline statement rendering and inline expression blocks use the same
iterative formatter frames. Measured rendering records visible `If` and branch
frames and zero-byte erased normalization nodes, so the existing capacity
census continues to cover every normalized expression without recursively
rerendering hidden subtrees. The unchanged frame stack enforces its ordinary
capacity bound. Syntax metadata carries no extra expressions or runtime work.

The owning gates are `language::statement_if` (exact canonical source, graph
round-trip, diagnostics, authored-name/explicit-zero distinction and
interpreter/native/Core Wasm behavior) and
`format::iterative_tests::statement_if_measurement_accounts_for_erased_normalization_nodes`.
The shared native Rust builder compiles the same formatter and its unit gate.
No conformance claim is made before those focused checks pass.
