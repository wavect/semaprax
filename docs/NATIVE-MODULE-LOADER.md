# Native module loader quarantine

Status: private, workspace-only quarantine. It is used by the unpublished
`semaprax-native-host` physical ownership stage and its private generated
callable path, but not by ordinary compiler preflight or any public adapter,
and it does not change `SPX-B104`.

`crates/semaprax-native-loader` isolates the unavoidable unsafe operations for
opening a trusted native library, resolving one fixed C descriptor getter,
calling it, and reading and comparing an exactly bounded expected byte range.
Its separately versioned callable-v2 constructor additionally resolves one
exact C byte-wire function after descriptor equality. Unix opens use
`RTLD_NOW | RTLD_LOCAL`, so dependency relocations fail during admission
rather than at a later first call. The main `semaprax` crate remains
`unsafe_code = "forbid"`. The loader is unpublished, has one exact-pinned
`libloading` dependency, exposes no generic symbol lookup, raw handle, raw
pointer, callable pointer, or manual close, and returns only opaque
`Arc`-backed leases with explicit retention and exact logical-admission
identity. Leases are deliberately neither `Send` nor `Sync`, keeping
potential native terminator execution on the opening thread until a future
module contract can prove cross-thread teardown safe.

The constructor is intentionally `unsafe`. Loading executes the selected
image's and dependencies' initializers before descriptor validation and may run
termination routines when the last SEMAPRAX loader reference is released. The
caller must already trust the exact artifact, its module directory, dependency
search behavior, getter ABI, immutable returned byte range, and absence of
foreign unwind. Canonical paths are diagnostic metadata; descriptor equality
proves only that the resolved getter returned the caller's expected bytes. It
does not prove that the getter belongs to the root image, that the root image is
compatible, or establish file identity, signature, provenance, or code safety.
The unsafe contract therefore also requires the root path, module directory,
and dependency-search namespace to remain non-adversarially stable throughout
the load. This is not a sandbox or a malicious-plugin boundary.

Current executable evidence uses plain C fixtures on Linux and macOS. The
descriptor-only lane proves canonical-path and input bounds, exact-byte
comparison, missing path/symbol rejection, null rejection, logical-admission
separation, explicit lease retention, and release of SEMAPRAX's loader reference
after the last lease. The callable foundation separately proves v1 rejection
before loading, bounded and distinct getter/callable names, exact v2 bytes,
eager unresolved-import failure on Unix, one exact resolved echo callable,
preallocated bounded request/response storage, one-shot invocation, and
cross-instance prepared-call rejection.

The unpublished native host additionally proves that its real callable-v2
lease is retained by its same-thread authority and every live owner/result
credential, that equal descriptor bytes from separate opens do not establish
instance identity, and that draining rejects new work while existing owners
keep their pins. The compiler now derives deterministic
[`SPXNABI2` admission metadata](NATIVE-CALLABLE-ABI-V2.md), and the host has an
independent strict staged decoder with cross-crate exact acceptance and
every-byte, truncation, and trailing-data rejection. The host connects that
decoder and callable lease to its authority and ledger; real generated O0/O2
providers execute the complete 14-case corpus through safe host calls.

The standalone loader retains narrow bounded-call fixtures, while the
ownership-host integration exercises generated SEMAPRAX resource code. The
Windows loader excludes current-directory/legacy-PATH dependency search and
admits the root-image directory plus default safe directories. A mandatory
Windows-only fixture now places a same-name malicious dependency in both the
process current directory and legacy `PATH`: the sibling dependency must win,
and removing that sibling must fail as `LibraryOpen` rather than falling back
to the malicious image. CI also names the complete generated O0/O2 callable
corpus as an explicit Windows gate. Until those committed gates pass in public
CI, they are implementation intent rather than confirmed Windows runtime
evidence. The current evidence also does not prove
immediate physical unmapping, same-root-image callable provenance, Windows
dependency isolation on an unobserved host, sanitizer instrumentation of the
Rust host, iOS dynamic/static admission, Android device execution,
callback/finalizer quiescence, hot reload, fork recovery, signed code admission,
or callable resource safety. Those remain gates before any public native
adapter or `SPX-B104` change.

The Linux
[ASan/UBSan generated-provider job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 cases loaded through this quarantine and the Rust
host. It did not instrument the Rust host, and unrelated Clippy/GCC failures
kept the overall workflow run red; it is not Windows evidence.
