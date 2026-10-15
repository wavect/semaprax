# Private prelude refusal receipt correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual current350 failure 304 compared the raw private diagnostic after a zero-byte sticky reservation against a rich public diagnostic rendered outside that refused ledger. The production diagnostic formatter is budgeted; this test-only correction retains the complete rich public comparison, exact/short/zero numeric controls and empty-selection census. It compares all six raw diagnostic fields (code, severity, message, path, span, help) against the static empty-text SPX-G171 receipt and checks zero owned message capacity. Separately it asserts the authoritative first reservation requested=1, remaining=0, floor=0, stage=None, refused=true and paid=0. The original failure selection and subsequent zero-byte refusal remain unchanged. No dynamic expectation is constructed inside the sticky ledger, no production or forecast changes, and no new TLS budget identifier occurrences.

The complete old fixture and actual historical failed log are preserved; the inverse restores the original bytes. No Cargo/build/runtime ran here. ROOT owns the later exact receiver rerun and existing rich public report gates.
