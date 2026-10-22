# Optional bounded source mask memoization

Explicit opt-in source-tool dispatch only. The ordinary default entry and all seven original parser/generator/verifier scripts retain their entire bytes. `bindings.json` pins those scripts; `originals/` retains every full preimage. `entry.py`, `memoizer.py`, bindings and focused checks are byte-identical to the externally qualified prototype.

The cache keys complete built-in input strings, retains at most4MiB via32 LRU slots, and never caches filesystem reads, paths, Git blobs, membership or outputs. Misses execute the unchanged original mask. Retained accounting is not a process-RSS promise. Source drift is read freshly; exact script drift refuses. Use only a clean explicit frozen HEAD and fresh external SSD outputs/stats.

`qualification/` retains the actual aaeb whole-output comparison, source closure bindings, negative controls and resource observations. Complete manifests/domains/boundary/preservation proofs matched without normalization. The original historical global-purity verifier failure remains preserved; it is not claimed PASS. All original boundary controls plus six original static source negatives ran. Cached phases were approximately21/22/23seconds; no paired speedup ratio was measured.

Run `check-pure-masks.py` for the small offline whole-mask corpus. `entry.py --help` documents opt-in dispatch. Per-entry stats intentionally do not self-claim full parity; the separate head-bound qualification receipt carries that result. A later HEAD must not inherit the historical receipt's source identity.

Common refresh opts in with `--memoize-source-masks`. The refresh-only adapter records dirty intermediate state honestly and permits only the exact paths in refresh-write-paths.json; unexpected paths refuse. Its one exact standalone-entry delta is archived in refresh-entry-delta.json. Relocated adapter reproduced all four full outputs again at checkpoint9894ca88e, with original boundary negatives unchanged; packaged-result.json retains that exact subject. The default dispatch and complete original refresh body restore byte for byte. No full common refresh was run in this lane.
