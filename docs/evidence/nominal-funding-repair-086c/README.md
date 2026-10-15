# Real constructor funding failure checkpoint

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual086c control03 observed203 allocated bytes versus139 at the selected error:64 additional bytes after refusal. Exact installed nightly2026-10-02 std source proves its pthread Mutex lazily boxes pal::Mutex on the first lock; pal::Mutex contains only COpaque<libc::pthread_mutex_t>. The new constructor pays that exact native Layout on pthread Unix targets before allocating, and primes its lock before publishing rollback guards. The inline futex backend on Linux/Android/etc has no lazy Box and receives no extra native backing charge. Source snapshots and hashes are retained. No ambient unsafe allocation hook, quota change, cap change or refund is introduced.

This addresses one concrete delayed allocation site. The existing unchanged every-acquisition physical test must requalify it; no Cargo/runtime was executed here. Full constructor physical funding remains OPEN: actual control04 reports27765 allocated versus18621 paid. Legacy declaration/name maps, temporary source resolution/inheritance/type-fact construction require genuine site-owned payment; this mutex checkpoint does not claim they are funded or that the two failed controls pass.
