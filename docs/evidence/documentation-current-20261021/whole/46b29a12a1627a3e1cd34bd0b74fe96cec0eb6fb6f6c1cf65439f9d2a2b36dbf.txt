# Test-only canonical entry owner ordering repair

The actual frozen3d4b singleton foreign/nested owner control failed because the
single-request cfg(test) id wrapper acquired a new ClosureReceiving before the
held Memo rejected the foreign scope. It therefore queried and charged a payer
that the unchanged original test requires untouched.

Authenticate the existing retained Memo first and publish its existing sticky
failure before constructing a new slot. Successful calls still acquire and pay
the complete real slot, and id_with_receiving retains its own validation and
authentication. No fee is deleted; no producer, work request, ledger/floor
predicate or production method changes. The added warm check is the existing
unit authenticator, not a Diagnostic-bearing Result wrapper. The original test
and all of its restoration/no-allocation assertions are unchanged.

The only Rust diff is inside the existing #[cfg(test)] id method. Source outside
that entire method is byte-exact. Complete immediate canonical source and its
full inverse are retained here; parent custody assembly must add this actual
current stage before immutable predecessor views. The preceding observer repair
is a dependency, not folded into this inverse. No Cargo/build/tests/Clippy ran.
The existing selected singleton owner is the regression; no selector was added.
Physical receiving-layer comparison failures are not addressed by this repair.
