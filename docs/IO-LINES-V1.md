# IO Lines v1

Status: implemented bounded source profile; hosted-green on the named
`std-library-depth` CI job at 1dfe12a6, outside the v0.4.0 release baseline. The broader
Everyday profile and streaming `std.io` scope remain incomplete.

Audience: language users, compiler contributors, standard-library authors, and
backend implementers.

This profile is the bundled `std.io.lines` package: line meaning over the
`std.io` Reader and Writer cursors. [IO Cursors v1](IO-CURSORS-V1.md) owns
those shapes and their existing transitions; nothing here changes an existing
signature, contract, identity, or byte. Line processing composes the same
records through an ordinary `[dependencies] std.io = "=0.1.0"` import: it
introduces no new type, no allocation, and no host authority.

Line processing is a sibling package rather than more of `std.io` for the
reason [Standard Library v1](STANDARD-LIBRARY-V1.md) already records for
`std.data.json` and `std.path.value`: one library module holding both halves
pushes an ordinary three-package consumer past the `SPX-G171` workspace-graph
pre-bound. The split is a packaging consequence of that bound, not a semantic
boundary; identities are `std.io.lines.*` and the cursor identities stay
exactly as released.

## Line meaning

A line is delimited by one line feed, `0x0A`. `line_end(view, start)` is the
absolute offset of the first line feed at or after `start`, or the view's length
when no further terminator exists, so it is total over every byte sequence and
never reports a position outside the view. `line_terminated(view, start)`
distinguishes a complete line from an unterminated tail by exactly that
comparison.

A line's *content* excludes its terminator and one immediately preceding
carriage return, so canonical LF and CRLF inputs of the same text yield
byte-identical content. `line_content_len` is that length. A carriage return not
immediately followed by a line feed is ordinary content: `a\rb` is one
unterminated three-byte line, not two lines. This is the whole policy; the
profile performs no other rewriting, no UTF-8 interpretation, and no trimming.

Empty lines are represented exactly: `\nab` has a zero-length first line
followed by an unterminated `ab`, and `\r\n` is one zero-length complete line.

## Cursor transitions

`reader_line_len` and `reader_line_complete` are borrowed observers over the
Reader's live position; both preserve the owner and reject a forged position
beyond the buffer through the same precondition as the existing observers.

`reader_line_into(borrow Reader, own Writer) -> Writer` copies the current
line's content into caller-supplied Writer capacity. Its precondition
preflights the exact content length against the writer's remaining capacity, so
a short buffer fails before any byte is written and the borrowed Reader keeps
its position and bytes. The copy starts at the Writer's live cursor: a Writer
that already holds output keeps its prefix. Bytes beyond the advanced cursor
retain their previous values, exactly as `writer_finish` already specifies.

`reader_next_line(own Reader) -> Reader` is the consuming transition past the
current line and its terminator, clamped to the buffer length for an
unterminated tail. Repeated application therefore reaches an exhausted cursor
and stays there; an exhausted cursor reports a zero-length, unterminated line.

Line transitions return records, so a `while` body cannot step them
(`SPX-T252`). A bounded caller unrolls the walk, as the executed two-line case
does. General streaming and mutable in-loop cursor replacement remain open.

## Boundaries

This profile adds no buffered reader, no stream, no standard-stream authority,
no file or socket, no public export, and no descriptor. It does not widen a
public nominal or generic surface, and it grants no ambient filesystem,
process, or network authority. Existing `Bytes`, contract, cleanup, graph, and
backend meanings are unchanged outside the admitted transitions. Line policy is
lexical: it does not claim text, Unicode, or platform newline conversion, which
remain with `std.text` and the open `std.io` scope.

## Focused local evidence

```sh
cargo test --locked -p semaprax --test project standard_library::io_lines
cargo test --locked -p semaprax --test project standard_library::io_lines::io_lines_execute_on_all_three_backends
cargo test --locked -p semaprax --test project standard_library::io_cursors
```

Eight named cases in `tests/project/standard_library/io_lines_cases.spx` run as
individual bounded projects, each with its own local call closure, on the
interpreter, native C11 at `-O0` and `-O2`, and repeated Core Wasm under Node
with an exact live-`Bytes` bound of two: CRLF and LF lines, an empty line, a
bare `\r\n`, a bare carriage return, an unterminated tail, an unrolled
two-line walk, and a prefixed Writer. The shipped package's own examples and
conformance modules execute on the same three backends, and the unchanged
`std.io` package keeps its own hosted-green corpus.

A graph check derives the projection for one fixture, replays it, and pins the
selected cleanup schema per shape: the line copy and the record observers
select the existing `semaprax.cleanup-plan.v5` contract schema, the pure view
helpers select `v2`, the copy carries exactly the caller's Writer as its one
owned parameter with the `core.bytes.drop` leaf lifecycle, and the borrowed
Reader never enters an owned inventory. A reminted field identity and a
one-byte source drift each fail replay.

Nine hostile interpreter cases reject before execution or any write:
insufficient writer capacity, a live cursor that leaves too little capacity, a
forged Reader position through each line observer and both transitions, and a
view offset past the end through each view helper. Each fails with the exact
`requires`-false contract status. Existing borrow-escape, no-public-descriptor,
and bundled-dependency checks continue to pass unchanged.

This is source-level line processing over caller-supplied buffers. It is not
evidence of a stream, a physical file, a hosted provider, or any public API.
