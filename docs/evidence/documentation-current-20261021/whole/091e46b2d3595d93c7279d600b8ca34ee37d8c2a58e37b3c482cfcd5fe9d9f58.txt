# Compiled source-audit transport V1

Status: source-only NEXT #798 component; executable qualification pending.

The HIR boundary, workspace projection and native interop layout audits retain
all live Rust snapshots as compile-time include_str! inputs. Their complete
original include occurrence order, repetitions, separators, forbidden predicates,
role classifier and static negatives are unchanged. No runtime filesystem read
of Rust source or authority is added.

A small local audit_source_join macro holds immutable &[&str] pieces and joins
at the same existing unmeasured audit invocation. Nested joins borrow temporary
Strings until the enclosing join finishes. Owning locals keep those strings
alive across the existing borrowed tuple/facade calls. No const/static consumer
requires a flattened literal. No new test binary, generic runtime cache or
framework exists. Original full source before-images and exact transport
substitutions are externally replayed; no flattened old corpus is embedded as
a binary oracle. Tiny cross-piece UTF8/NUL/CRLF equality assertions live in the
same original three owners.

Authority metadata generation normalizes only the exact declared test macro
for its existing literal-inventory grammar. Actual current source bytes remain
unmodified in the metadata and at runtime. The original classifier, production
forbidden predicates and six negative controls remain; chunk paths/literals,
multiplicity and production/test roles must equal the complete prior manifest.
Every production file length/hash remains exact. Existing source inverse domains
require a complete next->component view before prior-source assertions.

The direct include occurrence sums (before macro alias repetition) were
10,425,492 HIR bytes, 8,078,363 workspace bytes and 15,244,999 interop bytes at
b5eca55b96890e1a815f0d8d9e7b42c78d743b07. They belong to integration/interop
harness compilations, not the library unit artifact. These sums explain no
previous unit RSS refusal. New runtime String allocations happen only in the
standalone source/layout audit owners, outside ACTIVE and allocation observers;
this is no production cost or savings claim. Actual build memory comparison,
resource policy, all benchmark gates and full help remain separate and unchanged.
