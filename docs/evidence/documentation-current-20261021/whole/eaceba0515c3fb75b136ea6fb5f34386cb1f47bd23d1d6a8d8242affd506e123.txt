# Fatal warm/cold graph field diagnostic

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

At exact77f90da5e00d533e40714d42369daed90661156a, the existing fatal refresh
diagnostic already rebuilds its warm/cold graphs and parses both as JSON. Add
only two observations over those existing values: first differing JSON pointer,
and first differing pointer excluding only the already-reported root graph_digest.
The full first difference is still reported; no digest or graph assertion is
ignored. Fatal refresh behavior, errors, comparison fields, application rebuilds
and subsequent original assertions remain byte-exact.

A generated-JSON scanner traverses object keys in their existing deterministic
map order and array elements in index order. JSON Pointer escapes ~ and /.
Limits are32768 visited nodes,64 depth,512 path bytes. It reports scan_limit or
pointer_limit rather than falsely claiming equality or an exact difference when
bounded. String previews contain at most80 Unicode scalar values plus original
byte length; arrays/objects report lengths without subtree serialization. Missing
fields are explicit. It performs no extra application build or graph clone.

This changes one existing test file and adds zero test bodies. Complete immediate
source and full inverse are included; new source stage needs parent composition.
No ROOT edits, Cargo, builds, tests or Clippy ran. This is diagnostic preparation,
not an explanation or fix for the actual SPX-G577 parity failure.
