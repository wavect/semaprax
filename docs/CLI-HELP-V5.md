# CLI Help v5: bounded authoring guidance

Status: implemented help surface; exact behavior is checked by both CLI help
surface harnesses and the compiler's language-reference test.

Audience: CLI users, coding agents, and compiler contributors.

V5 changes only the default `semaprax help language` response. It now prints a
short authoring guide with supported source and Project routes, profile
distinctions, and exact selectors for deeper help. The complete
`AGENT-QUICK-REFERENCE.md` remains available byte-for-byte through
`semaprax help language all`. Existing `topics` and exact topic selectors keep
their behavior. Diagnostics, library catalogs, capability rules, and profile
admission do not change.

The authoring guide is a checked-in document embedded in the compiler. The
default response is bounded to 2,048 bytes and 400 lexical units. Both
standalone and full-capability CLI tests check its exact bytes, the explicit
full-reference bytes, exact topic behavior, usage grammar, and inert execution
in an empty working directory.

The guide distinguishes single-file web source profiles from Project profiles.
It names only the admitted `internal-strings-v1` and `text-toolkit-v1` source
profiles and identifies `source-command-file-text` as a native Project starter
with its declared profile, input, and target. Generated project documentation
remains authoritative for each starter's dependencies and commands.

This is guidance, not an admission mechanism. A profile or capability is
available only when the ordinary compiler route accepts it.
