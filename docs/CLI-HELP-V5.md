# CLI Help v5: bounded authoring guidance

Status: implemented source; current-head execution of the owning help harnesses
is pending the grouped verification batch.

Audience: CLI users, coding agents, and compiler contributors.

V5 changes the default `semaprax help language` response. It now prints a
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

## Bounded application discovery

`help language author:routes` lists the complete application-route inventory.
Exact `author:stdin-json`, `author:file-text`, and `author:source-web` selectors
give the admitted profile/target, build and test commands, required capabilities,
starter, limitations, and exact selectors for checked library prerequisites.
These routes are not aliases for the existing seventeen language topics.
Unknown routes refuse with exit two and no stdout.

`help language find:<word>:<offset>` searches the embedded generated library
catalog. A word has 1..64 ASCII letters, digits, dots, underscores or hyphens;
matching is case-sensitive substring matching over module, identity, display
name and checked signature. The offset is canonical unsigned decimal. Results
are ordered by stable identity. Each response includes at most 2,048 UTF-8 bytes,
whole declarations with their exact dependency, required profile and signature,
and either complete matching coverage or an exact next-page command. No result
is silently truncated. An entry larger than a page refuses with its exact
`help library` selector. Empty matches, malformed queries and out-of-range
offsets refuse. The fixed page limit is not a runtime-language capacity.

Search uses the existing catalog, not a competing semantic index. No selector
reads a user project, compiles source, writes files or gains a capability.
Focused tests cover deterministic route output, malformed selectors, complete
multi-page traversal without duplicates, exact byte budget and one-short
refusal. Smaller help bytes alone do not establish live-agent savings.
