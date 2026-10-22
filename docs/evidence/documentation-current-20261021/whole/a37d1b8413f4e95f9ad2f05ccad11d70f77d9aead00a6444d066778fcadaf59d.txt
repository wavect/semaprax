# Keep dormant test observation buffers out of static TLS

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The Linux OPT1 signal capture at 599d98f stops in the lexical scan prologue. All captured caller frames together consume only roughly 8 KiB before its 12 KiB scan array, while the stopped thread control block is roughly 279 KiB above the bottom caller. The unchanged 4096-event frontier trace and 128-row pair census previously occupied roughly 270 KiB of inline static TLS. Linux pthread allocation includes static TLS, leaving very little stack for a request smaller than that storage.

Only the Vec carriers now occupy TLS. Frontier instrumentation is prepared before the three original measured wrappers; pair rows are prepared in their existing reset before measured comparison. The same fixed capacities, all counters, overflow handling, generations, original budget/work charges, allocator observation, and every original test assertion remain. Production validation and the original exact 512-depth / 128 KiB regression are unchanged.

The local component exercises the complete real observer bodies with the real global allocation observer, all original census hostile controls (minimal subject and ledger fixtures), both capacity boundaries, event order, and zero observer allocations during measured operations. It is an observer component proof, not a full compiler or Linux regression pass. Complete source inverse comparisons retain all earlier source guards; full current-head CI is still required.
