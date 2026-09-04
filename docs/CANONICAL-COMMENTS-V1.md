# Canonical comments v1

Status: authored and locally exercised; unpublished and unpromoted. The
completion matrix and release evidence own product status.

Audience: language users, coding agents, and compiler contributors.

## Purpose

Canonical formatting projected the program back to text from the syntax tree,
and the syntax tree never held comments, so `semaprax fmt` deleted every `//`
comment a person wrote. The agent quick reference had to warn that nothing
after `//` survives. This version keeps comments through formatting. It does
so by position rather than by changing the syntax tree: the lexer records
where each comment was, the formatter maps each position onto the item it
belongs to, and prints it there.

## What is preserved

Every `//` comment of a file that parses is printed exactly once, with its text
after `//` kept verbatim except for trailing whitespace. Formatting is
idempotent: formatting a formatted file changes nothing, so a canonical file
with comments passes `fmt --check`. A file without comments formats to exactly
the bytes it formatted to before this version.

## Where a comment goes

The formatter's items are module uses, declarations, record and class fields,
variant cases, resource lifecycles, class methods, block statements, and the
tail expression of every block. Items nest: a function's body is a sequence of
items inside the function; a `while` or `unsafe` body inside a statement.

1. A comment before the `module` line stays at the top of the file.
2. A comment on its own line before an item is printed above that item, above
   its `@id` attribute, indented to the item's depth.
3. A comment on the line directly after the previous item, or trailing a token
   on the same line, belongs to that previous item and is printed on its own
   line right after it. The blank line canonical formatting places between
   declarations therefore never separates a comment from the item it followed,
   and a same-line comment moves to the next line once, then stays.
4. A comment inside an item but outside its body, such as one inside a
   signature, a contract line, or a single-line expression, is printed above
   the item.
5. A comment after the last item of a block that is not on the line directly
   after that item is printed before the block's closing brace. One after the
   last declaration of the file is printed at the end of the file.

Because canonical formatting orders declarations by kind rather than by source
position, a comment travels with the declaration it leads or trails, not with
its line number.

## Non-claims

Only `fmt` restores comments. Semantic transactions that rewrite a source file
(`patch`, workspace and candidate publication) still emit comment-free
canonical text; comments in a file those routes rewrite are lost as before.
The semantic graph, HIR, diagnostics, and every backend ignore comments;
`graph` output is byte-identical with or without them. Comments are not
documentation attached to declarations, are not part of a declaration's
identity, and are not visible to `context`. Block comments (`/* */`) are not
part of the language.

## Evidence

Unit cases in `src/format/comments.rs` pin the exact placement of header,
leading, trailing, in-signature, block-end, field, method, and loop-body
comments and the idempotence and comment-free-identity properties.
`tests/projections/fmt_comments.rs` exercises the CLI: `fmt` rewrites a
commented file to the pinned bytes, `fmt --check` accepts the result, a second
`fmt` changes nothing, and `graph` of the commented and comment-free files is
byte-identical. The examples and documentation gates keep the comment-free
canonical form unchanged.
