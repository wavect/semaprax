# Compact borrowed child cursor

Status: private implementation; hosted constructor and full quality qualification pending.

Audience: compiler contributors and reviewers.

The actual Linux diagnostic run 38951879634 at main b4523cc7c stopped on
SIGSEGV in `hir::lexical_exit::walk::scan`, called by loan-plan construction
inside `HirValidator::with_directory`. The original 512-level indexed-match
case still used its original 128 KiB thread. A successful debugger job records
a captured failure; it does not qualify the test.

The cursor now obtains its variant from its immutable borrowed expression and
stores two full-width progress indexes. The unused maximum inner index marks
a fresh cursor: statement child counts are at most two, and match guard phases
are zero or one. No outer index, language depth, work limit or scan bound is
reduced. On 64-bit hosts the cursor is 24 bytes instead of 40, and the unchanged
513-entry scan array is 12,312 bytes instead of 20,520. Enumeration stays borrowed
and has no heap backing. The child dispatcher and scan implementations are
unchanged.

The original cursor is compiled beside the successor. Owning tests compare
borrowed child pointers, complete first-visit/statement/guard progress, fused
exhaustion and every work-refusal frontier. Every original test and assertion
remains; its direct state access uses a test-only projection. Complete source
archives and inverse edits bind both implementations, the original tests and
the composed historical source-custody packet.

Focused static-std tests compile both complete walk modules against explicit
thin HIR interfaces. They cover wide blocks and guarded matches, and presence
results and diagnostic equality at depths 0, 1, 256, 257, 511, 512, 513 and 514.
These component checks do not qualify the real constructor, full HIR/backend
behavior or hosted current-head CI. The unchanged original low-stack case and
all full quality gates remain required.
