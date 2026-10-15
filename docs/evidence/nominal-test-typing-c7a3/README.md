# Four test compilation repairs

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Complete actual c7a3 build: production library passed; harness compilation failed with four errors and zero runtime tests. The two probe pointer assertions still compare exact Vec addresses via explicit borrowed dereference. The record comparison still compares every field through the full borrowed slice. The affinity test retains its exact typed Detached assertion using a cfg(test)-only reexport of the existing ProducerFailure enum; no production visibility changes.

No Cargo or runtime execution in this repair lane. Rust formatting, full-source byte inverses, unchanged archive hashes and the existing 36 source checks are static checks only. v6 retains all v5 originals unchanged, adds two exact c7a3 test originals, and binds 65 current sources. The host-mutator packet appends an exact receiving bridge before its original reverse-order inverses; every original splice and archive remains unchanged.
