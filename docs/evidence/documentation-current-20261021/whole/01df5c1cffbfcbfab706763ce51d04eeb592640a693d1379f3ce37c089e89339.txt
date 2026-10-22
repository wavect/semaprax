# d6bf capacity fixture, source boundary and control-slot correction

Basis: `d6bf8151942190a956eb57350fa8b8892a8c68e5`. Source authored only. No Cargo, tests, Clippy, compiler, Node, Claude, or review execution. The archived actual failures describe the basis, not this correction.

Actual selectors 59 and 60 failed while parsing their common imported-descendant fixture: the detail module had only type declarations, while the grammar requires a function. A minimal unimported zero-argument scalar function makes that module valid. It adds no types or signature roots. The original sparse padding, all observations, all assertions, and complete old capacity receiver remain unchanged. This is a fixture correction, not evidence of an allocation improvement.

Actual selector 80 rejected two `use super::*;` imports in cfg(test) children included in its full production source inventory. Explicit imports replace those globs. No source is excluded and the audit itself is unchanged.

Production Clippy reported 61 reference-to-reference size diagnostics and one drop_non_drop diagnostic. Each reference-slot charge is expressed as size_of of the exact declared reference type, preserving thin/fat pointer widths, mutable reference types, operand order, checked calls, and refusal ordering. `reference-slots.json` maps every actual diagnostic to its replacement. Pointee measurements and value slots remain untouched. The temporary Cost borrow now ends through a lexical block after the same census and total read; no destructor or operation is added.

All existing cost/frontier/foreign/restored/full-Drop controls and assertion bodies are preserved. Their exact pending selectors are recorded separately. No corrective runtime or Clippy result is claimed.

`original/00.before` through `03.before` archive the four immediate whole sources. Local full-source inverses restore those exact bytes. Three directly owning guard metadata files receive only current-to-immediate-before prefixes; every prior prefix and original archive remains intact. Their 27 complete inverse rows pass the source-only Python proof. The original metadata files are also archived in full. Global 68-packet custody composition belongs to the parent at integration and was not performed here.

`verify_source.py` performs only source, hash, inverse, unchanged-body and inventory checks. It reads missing sparse paths directly from the immutable basis Git objects. The worktree deliberately omits historical evidence checkout to avoid a multi-gigabyte copy.
