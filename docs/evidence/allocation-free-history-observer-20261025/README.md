# Allocation-free plan-history observer

The first `std::sync::Mutex` acquisition on this macOS/Rust host allocates 64 bytes that remain owned by the static lock. The original cold allocator frontier therefore reports 64 live bytes belonging to the test observer. A direct standard-library allocation probe reproduces the allocation.

Use `std::sync::RwLock` only through exclusive write leases. Its first write acquisition allocates zero bytes on this host; observations remain serialized, inactive callbacks take no lease, and TLS destruction releases the lease. No unsafe code or dependency is added. `prepare` remains the original TLS-only operation: first reset still works without preparation.

The complete original frontier and initialization test files are unchanged. The fresh-thread/no-prepare initialization test and the complete original allocator-frontier test each pass in a fresh process. All 16 focused current-head history tests pass, including full owned-event parity, malformed links, terminal/multiplicity mutations, exact/short/zero and floor bounds, sticky refusal, inactive foreign callbacks, repeated snapshots, capacity refusal, and final allocation teardown. The added regression verifies repeated preparation preserves an existing lease and its complete event.

The focused compiler uses actual main IR, types, defaults and verification; only private test registration/visibility and fixture path acquisition are adapted. These results do not include the unrelated private runtime budget experiments. The original source-boundary verifier and its six hostile controls pass. Both complete source-domain manifests retain every original role, chunk and acquisition; only this helper pin and registration byte offsets change. Exact complete predecessor bytes are retained.

All builds, caches, temporary files and logs stay on the external SSD. Device available physical RAM is monitored; no process RSS cap is used. This fixes observer initialization, not the remaining compiler builder-budget failures.
