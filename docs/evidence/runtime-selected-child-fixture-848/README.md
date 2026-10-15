# Authenticated runtime evidence pilot (#848)

Source-only private pilot based on c37b7eea8b680ca4cb63c85758c6d7713745e8d3.
Excluded from the current qualification batch. No build or test body has run;
there is no memory, application-cap, token or speed benefit claim.

The existing complete selected-child source owner reads the immutable 5,196,870
byte repository fixture at one compiled path. Its compiled SHA256 remains
2ff3e371c8bfde876ede4f5e7445a26067858e4a0d84610cca94d5b3ccc1240e.
One opened handle supplies regular-file metadata and bytes. Exact metadata length
and the hard cap precede buffer acquisition; exact read, EOF, SHA256 and UTF-8
admission precede all original JSON/source assertions. There is no discovery,
cache, subprocess, build-time IO, production authority or cap change. Loading
remains outside compiler budget/allocation probes.

The test binary now requires the fixture at its compiled repository path at TEST
runtime. It is no longer self-contained for this owner. The five original
decoder/helper bodies and every hostile decoder assertion remain byte exact.
The obsolete whole-gzip digest constant is removed. All five existing test
dependencies remain for matched decoder controls; the old #841 experiment is
unchanged.

Exactly two owning selectors are in `pending-selectors.json`: the existing
complete source/old-receiver owner and one new IO owner. The latter has normal
and empty successes plus ten hostile controls: missing path, cap overflow,
directory, both metadata-length mismatches, actual same-length file tampering,
deterministic stream truncation and growth, stream cap overflow, and invalid
UTF-8. Directory refusal is strictly Input or Bound because platforms can refuse
opening directories or reject their metadata. Cleanup ownership is installed
only after successful exclusive directory creation.

`verify.py` passed the two full current-to-c37 inverses, original decoder-body
equality, immutable raw bytes and hash, unchanged parent assertions, five test
dependencies and all three existing complete audit includes. It also replayed
the unchanged raw-default predecessor proof in its complete named old domain,
preserving all 41 original assertion rows and earlier source histories. This is
source evidence, not compilation or runtime qualification.

`current-to-c37.json` is protected runtime data. The same IO test authenticates
both complete inverses. A future compositor must supply an independent full
final-to-this-checkpoint view at `source_reflows_merge::runtime848_input`;
inherited guards use the separate `combined_ready_input` prefix before the
immutable c37 chain. Preserve this JSON and derive aliases rather than changing
old hashes or assertions.

Qualification remains pending: the two exact owning tests and one fresh matched
same-source/profile no-run build pair with existing one-worker/private-SSD
safeguards. Preserve phase/time/RSS/artifact/pressure receipts and order/cache
limitations. Keep only for meaningful measured benefit; do not fish pairs.
Bootstrap setup and live-agent application costs remain separate evidence.
