# Test source cache unwind isolation

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The test-only source reader previously held its global cache mutex while running the complete source-restoration assertions. A rejected restoration left the cache unchanged but poisoned the mutex, causing later independent lookups to panic with PoisonError. The new reader checks for a hit under the lock, reconstructs the complete String outside the lock, then locks only to publish one immutable value. Concurrent misses discard the losing String normally.

Every old source authentication assertion and existing test is retained. The fixed before-root archive and complete old/new entry archives reconstruct every byte of the previous source file. No poisoned mutex is silently recovered; the original authentication panic still propagates and a failed restoration publishes no value.

The new focused regression uses the exact current cache helper and unchanged Cache alias with std imports in a small standalone Rust test. It observes the mutex during restoration, catches an authentication refusal, verifies the cache is empty and unpoisoned, and then checks successful publication and the identical cached pointer on a hit. Focused Clippy treats every warning as an error. These are component checks, not the complete compiler unit suite or current CI success.

All execution, temporary files, caches and evidence stay on the external SSD. The unchanged local guard caps the complete process tree at 200 MiB; full compiler builds remain remote. Original source-role and hostile-input checks are recorded separately.
