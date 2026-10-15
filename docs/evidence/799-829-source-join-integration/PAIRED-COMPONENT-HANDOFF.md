# Exact generic signature component correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

A missing root such as `Missing<T>` changes its actual binding to `Missing<i64>` during validation specialization. The previous borrowed-template route compared that actual binding against the authored type and therefore could retain dependent body and contract failures. The new producer-origin receiver checks the exact current parameter type and mode while rechecking the original receipt against its exact paired authored parameter and generic environment. The complete preceding `Recovery::signature` function remains unchanged and compiled.

A generic missing-root return produced T220 while the old return path recognized only T001. The same opaque origin now authenticates the full original return diagnostic before suppressing its dependent T103. An unresolved root also defers its generic return shape profile; known outer return carriers retain their independent T224. Postcondition `result` acquires a dependency only for the actual current return carrier by pointer identity. The requires and ordinary body entries gain no result availability.

Three authored owners exercise the actual full old/current public pipeline, generic missing-root arguments across body/requires/ensures, generic missing-root returns with independent literal and missing-name contract failures, and full six-field return forgery plus foreign/stale origins and altered actual bindings. Existing six-field parameter and foreign/stale controls remain required.

Source-only verification: nine complete production/predecessor inverse comparisons, the extended-component self inverse, the unchanged preceding receiver, and complete original recovery/entry source reconstruction pass mechanically (13 checks). New full originals are archived separately; all prior archives and fixed hashes remain unchanged. Rustfmt parsed changed current code; final existing receiver bytes were preserved verbatim. No Cargo, build, Clippy, tests or runtime execution occurred here. Error scratch/directory acquisitions remain ordinary unpaid storage, with no physical funding or ABI claim. Three selectors are pending in the existing registration packet.
