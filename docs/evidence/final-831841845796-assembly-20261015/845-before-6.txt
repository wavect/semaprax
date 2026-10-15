# Canonical hygiene scratch

The source formatter still performs the complete expression-statement hygiene
census, including all roots, caller bindings, nested names and match patterns.
Only the expression-frame storage changes: each complete replacement allocation
and its receiving controls are prepaid before acquisition, while the independent
513-frame admission bound and the fixed 129-frame pattern census remain intact.
Backing is released normally; no allocation credit or refund is granted. Deep
walks can pay more cumulative scratch than the previous single fixed array.

`expression_statement.before.rs.txt` is the complete original source, compiled
as a test module. The source-custody test authenticates its full SHA-256 and
reconstructs every original byte after only the declared storage substitutions;
it separately authenticates the complete new growth helper. The original name,
pattern, provenance and traversal logic is retained verbatim.

Six focused tests pass in an SSD-local harness using the actual AST carriers,
old and new hygiene receivers, bounded-output payer, allocation observer and
canonical formatter. They cover hostile markers and readers, binding/pattern
collisions, shared roots, every growth frontier, exact/short/zero funding,
release accounting and the unchanged 513/514 boundary. A 32-function scalar
canonicalization passes with the unchanged 1,048,576-byte limit. Substituting
the complete old receiver reproduces its reservation refusal at that limit.

The focused harness omits unrelated AST workspace/agent admission methods and
uses the Rust reference bytes for the optional Kernel-0 candidate renderer;
this does not qualify candidate replay or the whole compiler. Compiles were
sequential with a 240 MiB sampled process-RSS stop threshold. An expanded
verification compile hit that cutoff and was terminated; its remaining funding
assertions were checked separately in a smaller harness. Full owning
package-report, language/backend, MSRV, Clippy and quality profiles require
hosted CI; no full local compiler build was run.
