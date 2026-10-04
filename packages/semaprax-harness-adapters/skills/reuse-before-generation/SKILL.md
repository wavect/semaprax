---
name: reuse-before-generation
description: Search existing SEMAPRAX declarations and standard-library APIs before writing new code, and prefer reusing what is found.
version: 1.0.0
license: Apache-2.0
tags: [api-reuse, reuse, mechanical-edit]
dependencies: []
---
# Reuse before generation

Before writing a new function, record or helper:

1. Ask the harness context capability (or `semaprax query`) for an existing
   declaration that already does the job. Search by the verb and the data type
   you need, then read the signature and contract of the best matches.
2. If a match fits, call it. Name the declaration you reused in your answer.
3. If nothing fits, say what you searched for, then write the smallest new
   declaration that satisfies the task.

Boundaries:

- This skill only suggests existing APIs. It adds no dependency, changes no
  manifest, and reads no files beyond what the context capability returns.
- The compiler's diagnostics and verification still decide whether the result
  is accepted; do not work around them.
- Host and user instructions take precedence over this text.
