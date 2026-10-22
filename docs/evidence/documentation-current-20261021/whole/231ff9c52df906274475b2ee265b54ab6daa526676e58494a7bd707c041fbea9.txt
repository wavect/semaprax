# Checked instance-key format completion (#853)

Source-only repair from a82e00cc83f99b16249181c45710d962598577db. Rust
compilation, owning runtime tests and application qualification are pending.
No budget cap or first omitted-byte expectation is increased or reduced.

The exact actual344 owner log is retained as `actual344-owning-selected-93.log`.
Its assertion at old tests.rs:215 reported requested=484 instead of child=8208.
The old fixture used the complete comparison bill and reserved only child_bytes
as its floor, then inspected the first receipt after two comparisons. A whole
bill includes later nominal rematerialization and comparison writes; it does
not identify the pre-child frontier. The retry could select an earlier
metadata boundary after the first attempt's silent ordinary-format refusal.

There is also a production completeness hole: `identity_key_checked` used the
ordinary bounded formatter for its zero-argument nominal shortcut and final
nominal rematerialization, as well as array, parameter and function headers.
That formatter returns an empty String on silent floor refusal without
updating the root completeness destination. A retained forged empty key could
therefore become identity authority. The new regression executes the complete
old encoder under its exact key byte bill with floor=1 and requires this old
empty/complete/nonsticky witness before testing the repaired comparator.

All six checked-key format sites now use the existing CheckedCappedString.
Every constructor admits its existing concrete CONTROLS before forming any
writer or completion reference. Every callback pays the unchanged ENTRY
controls before requesting its actual bytes. The existing first-omission bank
selects the actual omitted byte request and site; sticky later callbacks cannot
replace it. Constructor-admission failure sets completeness false and executes
the original formatter once. Public `ResolvedType::identity_key`, allocating
`FunctionInstanceId::derive`, comparator, comparison controls and global string
sink remain byte-exact. No successful key grammar changes.

Checked formatting replaces the ordinary formatter's direct reserve call.
The checked sink's extra active/captured Rc/Ref/Option payer selection is
therefore inventoried explicitly in SINK_SELECTION_CONTROLS and paid as part
of every real ENTRY admission. Existing nominal callbacks also pay this finite
component. No per-write mode or guessed formatter callback count is added;
this is additional charged control work, not claimed credit or a cap change.

The checked nominal branch must release its inner completion borrow before
constructing the rematerialization writer. Its existing constructor admission
also pays `size_of::<String>()` for that newly named receiving slot. It is
consumed exactly once; the failure branch retains its original inline String
receiver. No scratch size is guessed and no old debit is refunded.

A cfg(test), allocation-free observer records the exact remaining budget after
the real checked ENTRY admission and before the selected byte request. It has
no reservation, floor mutation or production field. The owner first succeeds
under the measured complete bill and captures this frontier R. It then uses
the same bill and floor R-request+1, which leaves exactly one byte too little
for that actual write. Earlier requests remain admitted because they were
observed before this frontier. The original child still requests exactly8208,
fits absolute remaining bytes, refuses only against the floor, and selects the
checked_string.rs site. The owner asserts sticky refusal before retry and
unchanged remaining bytes after retry, alongside all original zero/exact/short,
active-floor, full key-producer, allocating oracle and Drop controls.

The separate forged-empty regression retains the full8192-byte declaration
request, exact pre-write calibration, sticky retry and complete producers.
The previous checked method remains executable under cfg(test), with its body
byte-exact and only its method identity/signature formatting changed.

`before/` retains complete exact Git preimages for all four modified files.
`source-inverses.json` binds every current byte back to those preimages. The
source-only verifier checks all six mechanical format substitutions, exact
old traversal body, constructor ordering, unchanged public producer/comparator/
sink, original all-forms and graph/diagnostic owners, observer isolation and
all three existing complete raw-source inventory joins:

```sh
python3 docs/evidence/instance-key-format-completion-853-20261016/verify-source.py --repo /absolute/checkout
```

This packet does not rewrite the earlier #853 packet or its original claims.
The old packet is historical source evidence; a new joined source needs its
current endpoint/authority bindings and actual owning runtime qualification.
No measured saving, issue closure or current application-fit claim is made.
