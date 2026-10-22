# OPT838 sticky retry observation repair

Source-only successor to frozen `3d4b996c9a8cd2b100a3c594e741314c108b5591`.
Actual owning check 159 panicked because its sticky retry read current_metrics
outside an active allocation observation. The retry and both original metric
assertions now run inside measure; its returned metrics also require zero
allocation, no invalid observation and zero live bytes. This scope borrows the
existing weights and budget; their construction, previous growth, refusal and
final Drop frontiers remain unchanged. No producer, charge, physical bound or
expected diagnostic is changed. Existing whole-owner allocation/Drop tests
remain independent.

The complete immediate test source is archived and authenticated by a full
current-to-3d4b inverse. Prior runtime JSON and histories are untouched. Parent
assembly must authenticate this new current stage before older literal-source
guards. The existing exact owner remains selected; no new selector is added.
No Cargo, tests, Clippy or build was run. The actual failed frozen-round result
remains historical evidence, not a passing result for this repair.
