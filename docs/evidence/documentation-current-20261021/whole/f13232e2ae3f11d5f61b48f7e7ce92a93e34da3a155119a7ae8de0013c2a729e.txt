# Interrupted first post-pin build at bcd831

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The first unit-only no-run command at exact `bcd83107ccda58800e9f2b848950cece03979de6` was interrupted after55.747s when the outer guardian observed kernel-critical pressure(level4), free9%, and swap-used growth1,271,726,080 bytes over20.24s. Cargo/unit controller exit130; zero test bodies ran, and the remaining three commands did not run. No Rust diagnostic or runtime PASS was produced.

The original stop receipt has unconfirmed group settlement: a killpg operation returned PermissionError and a later membership observation timed out. That original failure remains intact. A SEPARATE later process snapshot found owned group3027 empty and kernel pressure normal1; it does not retroactively repair the earlier receipt. Unrelated host processes were not terminated.

The same-source retry uses a distinct external round with the same jobs1/profile64/8GiB guard and exact21 selection. This failed attempt is never counted as a build PASS or consumer execution, nor used as memory-efficiency evidence. The cause of the transient host pressure is not established.
