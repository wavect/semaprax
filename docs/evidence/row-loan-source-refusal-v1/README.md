# Owning provider row arguments are source refusals

The hosted Project semantic-cache regression on `9279411` fails only its
`row: own LoanRow` mutation with `SPX-G173`; its cold/warm diagnostic equality
already passes. The owning Scoped Record Row Loan v1 contract requires
`SPX-T284` for an authored owning parameter. The source-slot collector was
combining that invalid current source signature with retained-HIR mismatches.

The collector now rejects an actual current-source `own` parameter with the
existing row-escape message, caller argument span and source path before it
inspects a retained descriptor. Its exact source-callee and import resolution
still runs first. Every former borrowed-type, generic, declaration identity,
loan callee/parameter, vector/index, slot and loop check remains unchanged.
`row_loan.before.rs.txt` and `source-receipt.json` retain the complete original
module; removing only the inserted branch reconstructs every original byte.

The existing real-source unit control now checks valid borrowed-provider replay,
current owning-provider source against the old retained consumer, and a fresh
owning-provider build. Its four retained-descriptor attacks remain present and
now require G173 explicitly. The original Project cache regression remains
unchanged and requires cold/warm parity, T284, and recovery through all its
provider-body and signature mutations. These actual runtime gates are pending
hosted CI; no full local compiler was built or test result inferred from source
inspection. Pinned formatting and complete-file source inversion pass locally.

The actual installed diagnostic generator still yields 1,513 static codes.
Only the two new T284 source paths change its normalized identity. Removing
those paths reproduces the entire previous identity. Dynamic-constructor paths
are unchanged. `catalog-receipt.json` and `bundle.before.json` retain the complete
prior bundle and old/new inner/outer digests; the payload remains 5,401 bytes.
