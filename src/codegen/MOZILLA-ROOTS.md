# Mozilla root bundle provenance

`mozilla-roots.pem` is the PEM projection embedded in the generated source of
the pinned `webpki-roots 1.0.3` crate. It contains 146 certificates, uses one
terminal LF, and has SHA-256
`d839471cd89ace6cb060941d0cc880d79bded8230768d838900fcaa53f335b50`.

The upstream data is generated from Mozilla's Included CA Certificate Report.
The data is licensed under CDLA-Permissive-2.0. Its complete license text is
retained beside the projection in [`MOZILLA-ROOTS-LICENSE`](MOZILLA-ROOTS-LICENSE)
and originates from the pinned crate's Cargo source distribution.

When `webpki-roots` changes, regenerate this file from the PEM comment blocks
in that crate's `src/lib.rs`, update the count and digest here, and rerun the
native HTTPS loopback and public-endpoint opt-in gates. Generated C embeds these
exact bytes and supplies them with `CURLOPT_CAINFO_BLOB`; it never consults a
host path for production trust roots.
