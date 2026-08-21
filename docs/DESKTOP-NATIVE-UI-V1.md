# Private native desktop UI v1

Status: implemented, source-locked, and hosted green for the bounded AppKit and
Win32 packages: macOS in [run 31338834586, job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230)
and Windows in [run 31343897595, job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480).

This milestone composes the existing private callable-v3 desktop engine with a
small real OS-native frontend. It remains outside the public compiler and is
not a SEMAPRAX UI dialect or application API.

## Platform fixtures

- macOS uses an Objective-C/AppKit `NSApplication`, visible titled `NSWindow`,
  native `NSButton`, explicit accessibility label, main event loop, delayed
  native control action, close delegate, and application termination callback.
- Windows uses a GUI-subsystem Win32 executable, visible top-level window,
  native `BUTTON`, `IAccessible` name query, `WM_TIMER` to `BM_CLICK` event,
  `GetMessageW` loop, `WM_DESTROY`, and `PostQuitMessage`.

Both frontends execute the package-bound sibling `SemapraxPrivateEngine` rather
than reimplementing ownership semantics. The packager publishes a canonical
lowercase SHA-256 manifest, and each frontend hashes the exact engine bytes and
rejects a mismatch before process launch. Windows retains its read handle
through `CreateProcessW`; AppKit verifies immediately before `NSTask` launch.
They then accept only the engine's exact two-call, generation-rotation, and
receipt-replay output. Success is published to a new caller-selected result file
only after the complete native lifecycle reaches termination.

## Package and evidence contract

The native UI packagers consume a separately completed v1 engine package. They
use the same exact platform Clang, linker, SDK, deployment, MSVC, and
import-library identities plus the canonical Visual Studio 18 installation.
Each UI executable is linked twice under the same basename in independent
directories and must be byte-identical. Cargo and the engine packagers remain
offline.

The macOS result is a foreground `APPL` bundle with no `LSBackgroundOnly` key,
an exact AppKit framework allowlist, `LC_UUID`, exact build-version metadata,
and a closed inventory. AppKit replaces the unbounded task wait with a five
second deadline, one-second termination grace, and final `SIGKILL`; a hostile
digest-valid `/usr/bin/yes` engine proves the timeout path returns without
publishing success. The Windows result is an x64 PE32+ GUI-subsystem application
with exact non-reparse compiler/linker/library roots, explicit `/nodefaultlib`
archives, an exact seven-DLL import set, no export directory, named exports, or
ordinal-only exports, an effective external `asInvoker` manifest, and a closed
inventory. Both platforms append an executable-preserving byte to a copied
engine and require digest rejection before result publication. Both packages
reject build-local load paths and run the native lifecycle in the ordinary
macOS/Windows CI matrix. Hostile source-lock tests remove each UI,
accessibility, lifecycle, digest, timeout, toolchain, and package gate.

## Explicit nonclaims

This is one private automated window and button, not general desktop support.
It provides no SEMAPRAX UI syntax, state/update/view model, layout abstraction,
SwiftUI, WinUI, menus, navigation, document handling, localization, theming,
screen-reader conformance suite, keyboard-navigation audit, high-contrast or
reduced-motion audit, reopen/suspend/session/power lifecycle, sandboxing,
entitlements, signing/notarization, installer/MSIX, Store distribution,
auto-update, or public application/native admission surface. The fixed engine
still covers one direct-trivial owned identity. The colocated digest manifest is
package-internal consistency evidence, not a signature, notarization, secure
update channel, or protection against an attacker replacing both engine and
manifest; `SPX-B104` remains closed.
