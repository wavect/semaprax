# Native disposal frame layout correction

The Ubuntu Public Native Rust SDK job at commit 12adce failed with E0080 on
the historical 56-byte disposal-frame pin. The closed compiler declaration
tag representation reduces DeclarationId from 24 to 16 bytes, ResolvedType
from 48 to 40 bytes, and FieldLivenessShape from 56 to 48 bytes on the
observed 64-bit host. The complete disposal frame is now 48 bytes.

Both production and the existing disposal regression retain an exact equality,
updated to the observed current size. All fixtures, disposal transitions,
preallocation/Drop controls and budget limits are unchanged. Both the census
and retained-byte ledger already multiply by the actual size_of of the frame;
there is no hard-coded 56-byte allocation tariff to change.

The bounded standalone layout probe retains all variants of the real inline
layout dependencies. Types occurring only behind Box or Vec use explicitly
opaque pointee declarations: this is a layout probe, not execution of disposal
or a full compiler build. Its independent historical identity/type/shape
alternatives still assert the old 56-byte frame layout. Whole prior files and
exact one-replacement inverses are retained beside this note. Full crate
compilation and disposal runtime qualification require current-head hosted CI.
