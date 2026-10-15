# README (nested-byte-build-correction-a33-20261015)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The frozen a33 focused no-run build compiled the production library, then stopped with five relocated test-control errors: three include paths and two oracle imports. This patch fixes only those lexical adapters, preserving the complete old receiver, original fixture bytes, test bodies and original oracle. The old oracle is imported under the relocated receiver’s existing local name.

The production warnings are corrected together: Refused is visible only to its owning byte-traversal module, matching the already exposed Failure variant; the Exit source-reference field is explicitly ignored without changing its type/layout or construction; the two old standalone draft wrappers remain compiled for test controls while production uses draft_bytes_in. No algorithm, work/debit, resource limit or backend semantics changes. No new tests or Cargo command ran in this correction lane; the coherent batch still requires its focused build/runtime/Clippy campaign.
