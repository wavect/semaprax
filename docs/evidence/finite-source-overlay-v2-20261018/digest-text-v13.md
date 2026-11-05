# V13 digest text

V13 stores the same 256-bit pin digests as canonical 43-character URL-safe
base64 without padding. Only the existing positional digest slots and digest
dictionary change; every other field stays literal. Decoding restores the full
V12 object before its original V10/V9/whole-packet validation.

The one-page limit remains 262144 bytes. Source, name-materialization and
digest-materialization limits remain unchanged. Old V12 inputs keep their
original wire cap and decoder. V13 checks its own wire cap before restoring
its bounded logical data; it rejects noncanonical tail bits, widths, alphabets,
unknown envelopes and all original table/index/refusal cases.

`pin-digest-fixtures-v13.json` freezes the entire previous V12 page and policy,
the corresponding V13 page, and 258 independent Python base64 test vectors.
The Rust tests compare both complete decoded metadata and the entire composed
packet, preserve opaque data, and exercise both original materialization caps.
The current Python decoder retains its existing 65536-byte source cap; compact
operator whitespace and shorter new private names keep it within that bound.
