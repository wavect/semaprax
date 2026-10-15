# Paid base prelude source packet

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Baseline 4ec48420b6e78b1035a66cc406893c46c3028464. This packet adds only a new unregistered producer/overlay module, independently retained cfg(test) AST constructor bodies, source-only controls and Paid Invocation Base Prelude v1. No existing source is replaced or relocated. The complete existing pre-schema constructor owner and current prelude owner remain archived with raw-byte hashes; no historical whole-wire or execution receipt is reconstructed. The new cfg(test) original helper bodies remain independent of base_schema.

The canonical #827 lane owns NominalPayload and all shared HIR/index/resolver/linker roots. The agreed interface consumes a funded Vec only after fallible reservations via a borrowed &mut Vec and FnMut reservation callback. This packet requires that interface and is not registered against the baseline's older HIR carrier types. Ordinary legacy index construction remains the full-copy route; the full-payload controls call the independently saved pre827 `DeclarationIndex::nominal_original_with_prelude` entry point supplied by that lane. Current P remains unchanged.

No Cargo/build/runtime executed. Allocation observer controls are authored, not successful physical receipts. No cache/backend/application parity or allocation/highwater savings is asserted. ROOT must compose the production replacement and execute the complete owning qualification before claiming support or adjusting forecasts. Cumulative construction and peak retained backing are separate quantities.
