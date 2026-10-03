# Project Source Local Future v1

Status: bounded interpreter-only RI-09 Project profile. Ordinary native and
Core Wasm `yield` emission remains refused.

## Selection and authority

An extensible `semaprax.manifest.v1` Project may select
`profile = "source-local-future.v1"` with `[exports] web = []` and exactly one
`rust_async = ["stable.id"]`. The selector is separate from Web exports.
The Phase-A Project loader links the entry and selected declaration, validates
the checked HIR, derives the compiler-owned source effect signature, and
admits only one direct `i64` request and answer, one `i64` input, and one
`i64` result. Other source shapes fail before a host Future exists. The
canonical manifest and source bytes remain part of the Project revision;
neither a caller-provided digest nor a Rust closure can select another source
declaration.

`with_authenticated_project` holds the manifest and sources through the
operation and performs its final held-input recheck before releasing a
retained `Arc<ProjectRevision>`. `SourceLocalFuture::prepare_revision` accepts
only that Phase-A profile, replays its retained signature against the exact
linked program, then evaluates the pure source prefix. The caller explicitly
supplies the host Future and executor. The runtime retains the immutable
revision until the Future is dropped or settles; it does not retain a file
handle or grant new network authority. The constructor accepts a retained
revision; it does not itself acquire held files or prove filesystem provenance.
The focused gate obtains that revision through the authenticated route.

The host Future is local and `'static`; it is pinned on first poll and cannot
be moved to another thread through this adapter. The checked interpreter
resumes once with the exact state, binding, arguments and request. Handler
error, panic, language failure and fuel exhaustion remain distinct. Dropping
the Future discards its local pending work and does not reverse a request a
server may already have received. Nothing live is serialized into a durable
source checkpoint or journal.

This profile has no Web, npm, or ordinary native emitter. The Project lock
records the profile and exact Project revision; no public interface descriptor
or generated Rust SDK is claimed. Source-authenticated Rust import selection,
generated async SDK export and RI-08 retained callback registration still
require separate admission and physical execution gates.

## Focused gate

The `project::source_local_future::authenticated_project_selected_async_export_awaits_host_future_and_refuses_drift`
selector opens a real held-input Project, retains its admitted revision,
awaits a caller-owned Rust Future through the source interpreter, and checks
the source result. It also refuses Web/npm emission, wrong selected identity,
and source mutation before held-input release. This is a local interpreter
gate, not a reqwest or generated SDK gate.
