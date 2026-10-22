# Explicit reference-control sizes

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Fourteen Clippy size_of_ref findings are replaced with explicit size_of::<T>(),
where T is the actual reference binding type formerly inferred by
size_of_val::<T>(&binding). Every T is Sized, including fat references to str
and slices. Lifetimes affect validity, not layout; the parameter-slice constructor
retains its explicit 'a lifetime in the spelling. No dereference, referent size,
host-pointer constant, lint allowance, helper or charge exemption is introduced.

Thirteen sites are in generic_type_import/receiving.rs and one is in its
parameter_names/receiving.rs. The nonreference Option layout control remains
size_of_val(&layout). All arithmetic/operator ordering, paid bytes, errors,
producer bodies, ownership and Drop boundaries are unchanged. Imports are updated
to match actual calls. Production source bytes DO change: source-only V3 and
custody assembly must explicitly admit these two spelling changes, not claim
production blobs are byte-exact.

The per-site ledger records all 14 old expressions, binding/reference types and
new calls. Both complete immediate sources are archived with full current/before
hashes and inverses. Old runtime metadata remains immutable. The already selected
old/current frontier and full-source owners remain necessary. No selector was
added; no Cargo/build/test/Clippy ran. This is not a Clippy PASS receipt.
