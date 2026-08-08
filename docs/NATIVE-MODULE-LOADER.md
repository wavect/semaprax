# Native module loader quarantine

Status: private, workspace-only experiment. It is not used by the compiler,
the staged capability authority, or any public adapter, and it does not change
`SPX-B104`.

`crates/semaprax-native-loader` isolates the unavoidable unsafe operations for
opening a trusted native library, resolving one fixed C descriptor getter,
calling it, and reading and comparing an exactly bounded expected byte range. The main
`semaprax` crate remains `unsafe_code = "forbid"`. The loader is unpublished,
has one exact-pinned `libloading` dependency, exposes no generic symbol lookup,
raw handle, pointer, callable symbol, or manual close, and returns only an
opaque `Arc`-backed lease with explicit retention and exact logical-admission
identity. Leases are deliberately neither `Send` nor `Sync`, keeping potential
native terminator execution on the opening thread until a future module
contract can prove cross-thread teardown safe.

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

Current executable evidence uses a plain generated C provider on Linux and
macOS. It proves canonical-path and input bounds, exact-byte comparison, missing
path/symbol rejection, null rejection, logical admission separation, explicit
lease retention, and release of SEMAPRAX's loader reference after the last
lease. It does not prove immediate physical unmapping, same-root-image symbol
provenance on every Unix loader, hardened dependency search, Windows DLL
loading, iOS dynamic loading, Android device execution, callback or finalizer
quiescence, hot reload, fork recovery, signed code admission, or callable
resource safety. Those remain gates before this crate can connect to the
fake-backed authority topology or a public adapter.
