# LAW-16 local process measurements

Each JSON document retains exactly 30 child-process samples, command bytes,
exit status, output digests, and p50/p95/mean/population deviation. The three
`balance-z3-*` documents are separate installed-Z3 calls for debit, credit, and
total postconditions. They form a three-declaration proof batch and are not a
single invocation comparable to either Bend route.

Cold isolation, peak RSS, proof-writing effort, provider cost, and cache-work
are unavailable in these child-process measurements.

Peak RSS uses a separate `/usr/bin/time -l` corpus in
`../law16-peak-rss-v1/`; it is wrapper-inclusive and is not merged with these
wall-time statistics. Normal checking, Bend verdict, SEMAPRAX `check`, and
project-bound Z3 remain separate routes. No file reports a winner.
