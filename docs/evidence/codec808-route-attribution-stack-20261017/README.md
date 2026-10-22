# codec808-route-attribution-stack-20261017 README

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The private observer clears only packet header fields and copies only populated rows directly between TLS packets. Packet no longer implements Copy or Clone. The three constant TLS initializers retain Packet::EMPTY; each newly registered row still receives Row::EMPTY. Capacity, overflow, coordinates, first refusal, counters and public measured behavior are unchanged by the source edit. Complete current and predecessor module bytes and a whole-source inverse are retained. The original codec808 six-source inverses are unchanged. Source inspection only; no Cargo, runtime, stack measurement or application qualification was run.
