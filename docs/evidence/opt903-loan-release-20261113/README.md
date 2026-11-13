# OPT #903 paid loan reachability release

The focused paid-plan observer compared the complete saved planner against the current retained planner using 1, 3, and 17 overlapping loan uses. It used a library test build from `0a68f5420` with Rust 1.97.1 on macOS arm64. Each fixture ran with both its ordinary attached plan and a deliberately modified attached plan. The observed allocation tuples matched across those two variants.

| Loan uses | Requested bytes, both planners | Cumulative reservation, both planners | Peak live requested bytes, old → current |
| ---: | ---: | ---: | ---: |
| 1 | 5,960 | 22,369 | 4,234 → 4,122 |
| 3 | 13,628 | 52,997 | 8,946 → 8,738 |
| 17 | 85,272 | 285,361 | 51,034 → 50,154 |

Each planner freed all tracked backing on return. The peak reductions equal the released reachability row backing of 112, 208, and 880 bytes. The old and current receivers agreed on plans, exact work refusals, late overlap, and mixed work and physical failure order. The associated source audit authenticates the whole current retained planner and reverses its later canonical-output constructor before checking the original early-release edits.

The mode-six workspace forecast remains a separate open gate. On the four-module scalar fixture, the current forecast was 6,573,050 bytes and the independent predecessor forecast was 6,418,504 bytes. The early release measurements cover actual planner backing and do not establish a lower workspace forecast or application admission.
