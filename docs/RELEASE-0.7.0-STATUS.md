# v0.7.0 release status

Status: published prerelease on 2026-10-01. The earlier failed tag attempts
below remain historical evidence.

Audience: release reviewers, maintainers, and readers checking hosted claims.

The [GitHub release](https://github.com/wavect/semaprax/releases/tag/v0.7.0)
lists three platform archives, `SHA256SUMS`, and release provenance assets.
The initial failed attempts described below preceded that publication.

The source package version and dated changelog are prepared for v0.7.0. At the
user's request, the annotated `v0.7.0` tag was first pushed before main CI
completed at `5b8b74f384e77ff9f850e0ace5f5c431c4bff51b`. After its gate
failed, the user authorized moving the still-unpublished tag to the repair
commit. The duplicate main CI runs were cancelled. Tagging does not certify
publication.

The [first exact-tag CI attempt](https://github.com/wavect/semaprax/actions/runs/36862427432/attempts/1)
had 76 successful jobs, but the macOS private desktop UI job was cancelled at
its four-hour limit. Its hostile engine test printed the expected digest
rejection, then the UI event loop did not return. The `release-gate` rejected
the cancelled upstream job; artifact and publication jobs were skipped. A
second attempt on that commit was cancelled after the same macOS step remained
in progress for more than 90 minutes. The tag had no GitHub Release or
published asset inventory when it was moved. The repair wakes the macOS UI
event loop after a rejected engine; it passed local digest-rejection, engine
timeout, successful UI lifecycle, and source-contract tests. The moved tag
requires its own green exact-tag gate and publication verification.

The [v0.6.0 gate record](RELEASE-0.6.0-STATUS.md) remains evidence for its own
failed tag runs. Its partial successful jobs do not certify v0.7.0. See the
[release process](RELEASE-PROCESS.md) and
[signing policy](RELEASE-SIGNING-POLICY-V1.md) for the acceptance rules.
