# Exact hosted calculator graph invoice update

Status: focused verification passed; full CI remains pending.

Audience: compiler maintainers and browser CI reviewers.

Complete Linux Rust 1.88 graphs and compiler provenance are captured from run 40178348803, job 121124361356 at a2814be651. Both graphs retain every semantic field and budget limit; restoring only the builder invoice and its self-digest reproduces the entire prior graph byte for byte. The invoice falls from 2,999,812 to 2,987,780 bytes. The shipping hosted pins use these exact observed digests; native profile pins use complete graphs measured with the actual current native compiler (invoice 2,987,996 bytes). Both native graphs likewise recover their complete prior captures, and the hosted graphs, by restoring only the invoice and self-digest.

Every original graph, receiver and hostile test remains. Additional controls replay the new complete hosted graphs through all previous receivers, reject rehashed bill, semantic and limit mutations, wrong roles and partial wires, and invert the complete changed receiver and runner to their retained predecessors.

The actual current compiler generated all three local fixtures; their real TypeScript consumers and three serial Chromium tests pass. All 235 exact registered Rust browser controls pass with unchanged source and binary bindings across device-guarded runs. Device RAM interruptions released only the owned small test process; completed cases were retained and every remaining case executed. This is not a claim of one uninterrupted suite run. Clippy with the Cargo lint settings and denied warnings, formatting, documentation metadata, and the unchanged source verifier with all six hostile checks pass. Build, cache and temporary outputs stay on the external SSD, with actual device-memory monitoring and no process RSS cap. The remaining compiler builder-budget failures are outside this fixture correction; full CI is not claimed green.
