# v0.7.0 release status

Status: tagged, unpublished; no v0.7.0 archive or signed release is published.

Audience: release reviewers, maintainers, and readers checking hosted claims.

The source package version and dated changelog are prepared for v0.7.0. At the
user's request, the annotated `v0.7.0` tag was pushed before main CI completed;
it resolves to `5b8b74f384e77ff9f850e0ace5f5c431c4bff51b`. The duplicate
main CI run was cancelled. Tagging does not certify publication.

The [first exact-tag CI attempt](https://github.com/wavect/semaprax/actions/runs/36862427432/attempts/1)
had 76 successful jobs, but the macOS private desktop UI job was cancelled at
its four-hour limit. Its hostile engine test printed the expected digest
rejection, then the UI event loop did not return. The `release-gate` rejected
the cancelled upstream job; artifact and publication jobs were skipped. A
second attempt on the same tag was in progress when this failure was recorded.
The tag has no GitHub Release or published asset inventory. A subsequent fix
on `main` does not alter the tagged commit or certify this exact-tag gate.

The [v0.6.0 gate record](RELEASE-0.6.0-STATUS.md) remains evidence for its own
failed tag runs. Its partial successful jobs do not certify v0.7.0. See the
[release process](RELEASE-PROCESS.md) and
[signing policy](RELEASE-SIGNING-POLICY-V1.md) for the acceptance rules.
