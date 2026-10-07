# CSV Record Decoding v1

Status: bounded source profile with focused local gates; hosted promotion is separate.

`std.data.csv` frames comma-separated logical records over a borrowed byte view.
CRLF and LF end a record only outside a quoted field. Embedded CRLF/LF stays
in decoded field data; bare CR is data. Quotes may open only at a field start,
and a closing quote must be followed by a comma, terminator, or EOF. Doubled
quotes inside a quoted field decode to one quote. Bytes have no implicit UTF-8
interpretation. Dialect selection, typed fields and asynchronous I/O are outside
this profile. Existing single-record raw-span functions retain their meanings.

`csv_record_end` excludes the terminator; `csv_record_next` skips it. Empty input
has zero records; an empty terminated line has one empty field. EOF without a
terminator completes an unquoted record. `csv_record_error_kind` returns 0 for
success, 1 for a quote outside a field start, 2 for junk after a closing quote,
and 3 for an incomplete quoted record. `csv_record_error_offset` gives the exact
first bad source byte, or input length for incomplete EOF and success.

Record starts must be in bounds. Field offsets must be actual field boundaries
within a fully validated logical record. Consecutive and trailing commas produce
empty fields; `csv_record_field_has_next` distinguishes a trailing empty field
from exhaustion. Decoded length excludes the outer quotes and collapses doubled
quotes. `csv_record_field_into` preflights the complete record, field offset and
remaining Writer capacity before mutation, borrows its input, consumes exactly
the supplied Writer, preserves its prefix/suffix, and returns the advanced
Writer without allocation. Invalid inputs and insufficient capacity select the
ordinary contract failure, with no partial successful result.

The internal import lane permits a borrowed byte view alongside an explicitly
imported resource-free owned byte record. Public Project signature and ABI
admission remains unchanged. Persistent identities use `std.data.csv.record-*`;
canonical source and graph derive from the same checked library bodies.

Focused gates live in `project::standard_library::csv` and the package's
`src/tests.spx`. They cover interpreter, native C11 O0/O2, repeated Core Wasm
calls, ordinary dependency linking, exact decoded bytes, malformed offsets and
capacity preflight. These are local executable gates, not hosted support claims.
