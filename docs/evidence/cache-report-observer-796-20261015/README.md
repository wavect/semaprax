# Cached-prelude report observer distinction (#796)

Source-only test observer correction based on 2cf33c156735bc644b933cd5a4c1e0f891649b9b. No production behavior or accounting change; no Cargo, test, build or Clippy executed.

The d8 actual two cache attribution owners failed at the old no-acquisition assertion: 9 observed versus 6 before warm replay. The existing replay_cached_prelude actually acquires a report owner for each warm module. The earlier d20 observer repair made those existing acquisitions visible; hiding them would make the observer false. Both actual failed receipts/logs remain in /Volumes/Semaprax/provenance/closure-cache-d8fad-three-actual-20261015 and are not reclassified.

The total acquisition observer is unchanged. A separate fixed thread-local counter classifies only the actual successful cached-prelude report acquisition, immediately after that acquisition and beside the existing total observation. Warm replay must acquire exactly three such report receivers for its three hits. The original no-fresh-resolver/index predicate is then asserted as unchanged total-minus-classified counts. Every original graph/Image/work/stage/source assertion following this check remains byte-exact. No graph equality or counter padding is introduced.

All four complete before sources are retained under original/. current-to-2cf33.json is a protected runtime input and whole-file inverse table. The new first hop in combined_ready_input authenticates the exact current source before every immutable older joined route. The three changed existing source owners already belong to all three audit unions; no new Rust owner or test body is introduced. The same two existing lib selectors are pending runtime, listed in pending-selectors.json.

This is a test observation distinction, not an improvement or #796 closure claim. The original total counter remains truthful, and the original semantic cache graph/Image/stage predicates must still execute and pass.
