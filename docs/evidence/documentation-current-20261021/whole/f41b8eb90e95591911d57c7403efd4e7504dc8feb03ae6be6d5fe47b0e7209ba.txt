# borrowed-single-cold-equation-fix-20261017 README

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only correction to the00c20 exact owning equations. cold_dead_id publishes ids/states but never enters id_inner, so completed state count alone is not an Exit count. Each unobserved logical owner is freshly constructed and invoked once; scratch_paid=false therefore identifies the cold-dead publication only, with no preexisting cache. Gate E/N/M on scratch_paid. Weight2/MAX width0 noncold entries retain scratch_paid=true; an overflow with no completed state has E=N=M=0 while the existing local admission remains counted. All physical assertions, source-role equations and exact/short/zero cases remain. Complete immediate files and exact inverses are retained. No Cargo/tests/builds/binders or ROOT writes.
