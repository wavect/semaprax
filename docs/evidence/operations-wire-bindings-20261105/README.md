# Complete Operations wire bindings

Status: Captured CI inputs and local receipt templates; current original tests remain authoritative.
Audience: Compiler maintainers and CI reviewers.

The twelve complete bodies retain every semantic field, limit, refusal and
nonclaim. The manifest retains the five previous literal expectations. Current
derivation, change, child-evidence and Operations-evidence bodies were captured
from the unchanged public fixtures on all three hosts at main 221101989.
Linux and Windows produced identical complete bodies. macOS has the previously
measured native builder layout difference.

The two macOS receipt templates were captured locally from the ordinary public
API. Portable verification and application bodies are reconstructed from those
complete templates by changing exactly three authenticated reference digests.
These receipt schemas contain no builder counters. Their lengths, revisions,
sources, results, limits, usage, nonclaims and every other byte remain unchanged.
The verifier checks the entire reconstruction and all reference digests and
lengths. This reconstruction is expected data, not a claim that the two portable
receipts were captured in CI. The original API/CLI tests compare their complete
actual reports, perform verification and application, and retain every existing
read-only, lock and hostile-input assertion.

Run the independent data proof:

```sh
python3 docs/evidence/operations-wire-bindings-20261105/verify.py
```
