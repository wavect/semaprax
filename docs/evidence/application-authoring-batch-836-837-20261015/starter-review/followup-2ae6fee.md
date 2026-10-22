# OPT837 P2 follow-up — source-only

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Reviewed `604e7fb05203a6701be6bdbcda8a181c5b969df2..2ae6fee203fb726980524885ec36da268d5361bd` only for the prior input-opening counterexample and preservation of its content/publication boundary. No source edits, builds, tests, Clippy, native execution, or paid calls were performed.

The prior P2 counterexample is closed in source. `new_project/application.rs:34–49` now acquires a held parent, opens its final component through `platform::hold_regular_file_bounded`, reads through that exact held regular file, and rechecks the named binding. The initial path metadata is explicitly advisory. Unix owner `platform-sys/src/unix/handles.rs:323–340` opens relative to the held directory with `O_NOFOLLOW | O_NONBLOCK`; its bounded authenticator at 284–304 checks the opened handle is a regular file before digesting. Thus a post-inspection symlink refuses at open, and a FIFO opens nonblocking and refuses classification before any content read. The former following `Handle::from_path`/`File::open` sequence is gone.

The existing owner also binds length, identity, mode and digest when rechecking the named file (`unix/handles.rs:249–278`). `read_exact` at 884–904 surrounds bounded handle reads with regular-file rechecks. The caller checks the held parent's ambient binding at `application.rs:50–53`. Final pre-staging plan and activation reads remain byte-compared against the selected originals at 147–154, using this same strengthened reader. The fix introduces no stage or publication authority into input reading.

The authored regression deterministically swaps each selected input (plan and activation) between inspection and held open, once to a symlink and once to a FIFO, and asserts refusal, no destination/stage, and preservation of the outside file. Stable regular bytes, bounds, and initial symlink controls are also authored. Tiny typed native-selector success, function-effect/module-permit refusal, undeclared root, and legacy native-target refusal controls have been added separately from the broad starter journey. Inspection confirms these target the requested boundaries; their execution remains unproven.

Disposition: prior P2 addressed by the inspected source change. This is narrowly a source review of that fix, not blanket approval or executable qualification of OPT837.
