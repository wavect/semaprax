# Copy Scalar Sort v1

Status: authored; focused verification is pending the complete OPT implementation batch.

`vec_sort<T>(values: own Vec<T>) -> Vec<T>` sorts the eight admitted Copy
scalar element types in ascending order. It consumes the input authority and
returns the same backing allocation with its next authenticated generation.
Length and capacity are unchanged; sorting allocates no payloads. Mutable
bindings admit `values = vec_sort<T>(values)` in ordinary bodies and admitted
loops, using the existing same-owner cleanup reservation. Values are staged
left to right; commit and cleanup follow the authenticated plan. Borrowed,
aliased or already moved inputs are rejected by their existing ownership rules.

Integers compare numerically, characters by Unicode scalar value, and booleans
with false before true. Floats use IEEE total ordering (Rust `total_cmp`):
negative NaNs, negative infinity, negative finite values, negative zero,
positive zero, positive finite values, positive infinity, positive NaNs.
Equal elements have no stability guarantee. Bytes, String and authored owned
payload elements remain outside this sorting operation; source admission and
independent HIR validation reject them.

The operation identity is `core.vec.sort`. Programs reaching the operation
select `semaprax.prelude.v12`; contracts v1–v10 remain byte-for-byte frozen.
Interpreter sorting and native in-place heapsort use the same ordering. The
aggregate Wasm lane appends an optional `env.spx_vec_sort_v3` import after the
existing Vec imports, before iterator, Box and String imports. Its signature
is `(i64 owner, i32 element_tag) -> i64 owner`; the provider validates the
carrier and tag, sorts its Copy scalar storage, invalidates the input token
and returns a distinct token for the next generation. Zero is an invalid
result. The import grants no filesystem, network or process authority.

String `<`, `<=`, `>`, and `>=` compare unsigned UTF-8 bytes lexicographically.
They borrow both operands, evaluate them left to right, and return bool.
Prefixes sort before longer strings, NUL is an ordinary byte, and supplementary
Unicode scalars follow their UTF-8 encoding rather than JavaScript UTF-16
order. Operand types must match (`SPX-T208`). String arithmetic stays closed
with `SPX-T250`; concatenate with `string_concat`. `string_compare` retains its
existing -1/0/1 result. The native comparator is reachability-gated so older
programs retain their generated runtime text; the web projection uses the
same unsigned-byte rule. Wasm adds a selected String comparison import.

Authored regressions live in the existing `owned_data::vec_sort` and
`language::string_ordering` harness modules. They cover all Copy types, duplicate
values, empty vectors, loop renewal, capacity preservation, UTF-8 prefixes,
NUL and supplementary scalars, ownership refusal and independent HIR rejection.
These are authored cases, not a claim of current-head successful execution.
