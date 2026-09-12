# project/incremental/snapshot.rs

- Snapshot · struct · L6-L12 — struct Snapshot
- Entry · struct · L13-L20 — struct Entry
- manifest · function · L37-L55 — fn manifest(context: &str) -> Result<ProjectManifest>
- encode_snapshot · function · L57-L97 — pub(crate) fn encode_snapshot(cache: &ProjectFrontendCache) -> Result<Vec<u8>>
- decode_snapshot · function · L101-L180 — pub(crate) fn decode_snapshot(bytes: &[u8]) -> Result<ProjectFrontendCache>
- require_warm · function · L182-L193 — fn require_warm(build: &ProjectFrontendBuild) -> Result<()>
