# Optional scalar-helper source work

The existing Explorer source-inclusive 16 MiB snapshot regression reaches a
G410 capture refusal first: scalar-helper capture charges the entire defining
source, including comments, against its 1,048,576-unit work allowance. The
calculator provider has no static literal, but its function imports demand an
optional helper capture. The same code is unchanged between the failing hosted
9279411a0 subject and e021a932d.

A cached earlier compiler independently reproduced the refusal after adding
1,052,672 comment bytes to the calculator provider; the unchanged baseline
checked successfully. This is prior-head evidence, not current-head
qualification.

The new selector computes and pays the exact original hash work first. An
insufficient allowance returns no inventory before digest/cursor allocation or
hashing. It does not increase the work bound, omit hash bytes from a proof, or
credit any paid storage. Checked arithmetic overflow and the existing held
source lookup guards retain their original refusals. Every admitted capture
still performs the complete original hash and declaration/body authentication.
The selected receiving gate remains unchanged and requires the full inventory.

The complete pre-change module is retained here. An executable source inverse
restores every byte using only removal of the new work helper and early decline
plus reinstatement of the original debit block. The existing independently
compiled legacy capture oracle and all admitted-inventory parity tests remain
unchanged.

New tests cover exact, spare and every short work allowance with UTF-8 bytes and
actual zero-allocation observation. A paired authored-source regression checks
that an ordinary scalar import builds without helper facts while the same
padded provider still gives the exact G410 refusal to a static-source receiver.
The original 16 MiB Explorer fixture and assertions are unchanged. Full Project
and receiving regressions require hosted execution; focused local work checks
alone do not qualify them.
