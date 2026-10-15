# PRODUCTION EQUALITY REPAIR (paired-review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The b2 return receiver now compares code, severity, message, path, span and help explicitly, matching the complete preceding parameter receiver. Diagnostic derives Eq/PartialEq only under cfg(test), so whole-Diagnostic equality was not a valid production operation. No new field, weakened comparator, receipt inference or diagnostic change is introduced. Existing six-field forgeries remain the owning unexecuted controls. This is a source-only typing correction; no Cargo or runtime was run here. The original b2 review snapshot and raw result remain unchanged.
