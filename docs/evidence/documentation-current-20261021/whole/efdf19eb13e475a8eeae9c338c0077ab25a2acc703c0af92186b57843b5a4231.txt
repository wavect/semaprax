# Application OsStr map spelling

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Replace exactly two zero-capture |name| OsStr::new(name) closures with the
OsStr::new function item. Both receive each &String from names.each_ref() and
return the same borrowed &OsStr; no clone, source-copy, ownership, lifetime,
lookup, error or publication route changes. These are array::map calls, so
their existing eager array traversal and order remain unchanged. No iterator
is newly evaluated and no existing laziness boundary moves.

The old closure captures no environment; the monomorphized function item is
zero-sized too. No function-pointer cast, stored environment, new local or
allocation is added. There is no reservation/fee change. Production source bytes
are changed and must be explicitly admitted by source assembly/V3; they are not
claimed byte-exact. Full immediate original and a two-site inverse are archived.
Existing selected application inventory/publication controls remain the owners.
No Cargo/build/tests/Clippy ran; no lint allowance or suppression is introduced.
