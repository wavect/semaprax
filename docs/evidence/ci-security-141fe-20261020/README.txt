Security patch maintenance: RUSTSEC-2026-0382, RUSTSEC-2026-0383, RUSTSEC-2026-0381, RUSTSEC-2026-0378 and RUSTSEC-2026-0379.

Both unchanged dependency policies pass all four checks with cargo-deny 0.20.2. Four actual component CI contract tests pass with locked offline Cargo and one build job. Full pinned Linux Wasmtime execution remains a hosted gate; earlier runtime evidence does not qualify the new patch.

Companion locks retain their complete feature-specific dependency edges. Only registry-verified package versions/checksums change; patched registry dependency/features/link definitions are equal and the pinned Rust 1.97.1 admits their MSRV. Generated-source examples whose local manifests are absent have not been rebuilt here. Historical evidence locks and measurements remain exact.
