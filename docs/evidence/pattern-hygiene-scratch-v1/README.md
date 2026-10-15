# Lazy fixed pattern scratch

The hosted `1e3a14ce7` integration-5 log
`116577327396` reports `SPX-P402` before resolver work exhaustion is reached.
The unchanged resolver fixture has 512 scalar payload declarations and one
main, each with a 230-byte function-name padding. Each pattern-free function
previously paid the complete 129-slot pattern stack during canonical hygiene.
This single eager term exceeds the unchanged 1 MiB canonical construction
limit across 513 functions, independently of the final source length.

The pattern dispatcher, slot count, frame pop/push order, bindings, markers,
readers, name sorting and collision decisions remain unchanged. A prepaid empty
vector owns scratch until the first pattern. The first pattern prepays the
complete original 129-slot layout, acquires exactly that capacity fallibly, and
initializes the same slots. Later patterns reuse this owner and authenticate
zero bytes before touching it. No depth limit, static source limit, cumulative
payer or test fixture changes. Matching code can gain a heap allocation and
pays its complete backing; pattern-free scopes acquire no pattern backing.

`expression_statement.before.rs.txt` retains the complete immediate previous
receiver. The existing independently compiled original receiver from
`format-hygiene-scratch-v1` and all its old controls remain. Its full original
SHA and the expression growth helper SHA are unchanged. Three additional
literal inverse operations reverse only the new pattern storage before the
existing complete-file inverse runs. A local Python execution of those exact
literal inverse tables reconstructs every original byte.

Authored controls add exact/one-short pattern backing, unchanged 129-slot
capacity, reuse and sticky refusal, allocator release, and the absence of
pattern backing for a pattern-free projection. Another control preserves all
512 padded resolver payloads plus main, binds their complete canonical bytes,
and requires them to fit the unchanged 1,048,576-byte construction limit.

Current runtime qualification is pending. Three local formatter compiles
were stopped at RAM cutoffs: 261,392 KiB sampled at a 240 MiB threshold,
226,320 KiB and 225,440 KiB at the lower 220 MiB threshold. They ran no tests.
A large compiler from a separate checkout was also observed. Full validation
is deferred to hosted CI; no full local Cargo/root compiler build was run.
The authored tests must pass before application or package/resolver fit is
claimed. The complete-file source inverse was independently executed in Python
and passed with both original source/helper hashes unchanged.
