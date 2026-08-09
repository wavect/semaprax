# Native module loader quarantine

Status: private, workspace-only quarantine. It is used by the unpublished
`semaprax-native-host` physical ownership stage and its private generated
callable path, but not by ordinary compiler preflight or any public adapter,
and it does not change `SPX-B104`.

`crates/semaprax-native-loader` isolates the unavoidable unsafe operations for
opening a trusted native library, resolving one fixed C descriptor getter,
calling it, and reading and comparing an exactly bounded expected byte range.
Its separately versioned callable-v2 constructor additionally resolves one
exact C byte-wire function after descriptor equality. The private settlement-v3
constructor consumes only a structurally bounded `SPXNABI3` projection already
accepted by the independent host decoder, resolves its getter plus six-argument
execute and settle entries eagerly, and admits them only when every function
address and the returned descriptor address belong to the canonical root
image, then retains its own immutable copy of the admitted bytes. Unix proves
this with `dladdr` plus canonical path equality; Windows
uses address-to-module allocation-base resolution plus canonical module-path
equality without adding another image reference. Unix opens use
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
foreign unwind. For descriptor-only and callable-v2 admission, canonical paths
are diagnostic metadata and descriptor equality proves only that the resolved
getter returned the caller's expected bytes. Settlement-v3 additionally proves
that its getter, execute, settle, and returned descriptor storage share one
root-image allocation and canonical path, and retains an immutable byte copy;
continued immutability of provider-owned storage remains part of the unsafe
caller contract. None of these checks establish file
identity, signature validity, provider compatibility, or code safety.
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
cross-instance prepared-call rejection. The v3 lane separately proves exact
descriptor-derived capacity equations, pairwise-distinct names and resolved
addresses, dependency-owned execute and descriptor-address rejection, missing
entry rejection, five disjoint preallocations, separate one-shot execute and
settle stages, cross-instance rejection, explicit retention, and final loader
release on the currently observed Unix host. The equivalent v3 Windows dynamic
runtime is green in [hosted run
31313341303](https://github.com/wavect/semaprax/actions/runs/31313341303).

The v3 lease retains exactly one platform `Library`; provenance queries do not
increment the native reference count. It also retains the exact admitted
descriptor bytes under the existing 64 KiB ceiling and exposes only a narrow
byte-equality check, so an independent host parse cannot be substituted with a
different same-capacity descriptor. Its request, recovery frame,
execute-response, decision, and candidate-receipt buffers are all allocated at
their exact authenticated capacities before execute. No allocation, symbol
lookup, generic lookup, or handle access occurs in either provider call. The
loader owns no poison, draining, quarantine, receipt-authentication, ledger, or
physical-finalizer policy; those remain host responsibilities. The v3 surface
is private and does not change public native admission or `SPX-B104`.

The loader's standalone plain-C fixtures remain separate provenance evidence.
A private joint test additionally compiles generated providers for all 14
authoritative normal scenarios, admits their exact descriptor and three entry
points through this constructor, and executes them through the host receipt
ledger at `-O0`/`-O2`. Seven failure/interruption fixtures also cross loader and
host; canonical pre-execute unwind skips execute and transitions directly to
settlement. Fatal allocator/process-crash evidence remains open.

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

The private [`SPXNPRF1` settlement proof](NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md)
is not a loadable descriptor. A dedicated regression requires the callable-v2
constructor to reject its magic during input validation, before attempting to
open any path. The unpublished host may parse proof bytes independently for
consistency evidence, but no loader constructor or symbol surface accepts them.

The standalone loader retains narrow bounded-call fixtures, while the
ownership-host integration exercises generated SEMAPRAX resource code. The
Windows loader excludes current-directory/legacy-PATH dependency search and
admits the root-image directory plus default safe directories. A mandatory
Windows-only fixture now places a same-name malicious dependency in both the
process current directory and legacy `PATH`: the sibling dependency must win,
and removing that sibling must fail as `LibraryOpen` rather than falling back
to the malicious image. CI also names the complete generated O0/O2 callable
corpus as an explicit Windows gate. Both passed in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).
The current evidence still does not prove
immediate physical unmapping, broader Windows
application-platform completion, an iOS-target loader/host build or runtime,
Android device execution,
callback/finalizer quiescence, hot reload, fork recovery, signed code admission,
or callable resource safety. Those remain gates before any public native
adapter or `SPX-B104` change. Bounded Linux Rust-host ASan evidence is green in
[public job 93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).

The Linux
[ASan/UBSan generated-provider job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 cases loaded through this quarantine and the Rust
host. It did not instrument the Rust host, and unrelated Clippy/GCC failures
kept that historical overall workflow run red; the later Windows evidence is
linked above.
