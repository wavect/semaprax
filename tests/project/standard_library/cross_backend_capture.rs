//! Issue #102: helpers that capture a backend's *actual* computed value
//! instead of independently checking it against a hardcoded sentinel. Three
//! backends each individually asserting they returned `0` proves nothing
//! about equivalence between them; a real comparison must read one
//! backend's computed value and check it against another's.

use std::path::Path;
use std::process::Command;

use semaprax::project::ProjectExecutionOutcome;

/// Runs a binary produced by the entry wrapper `codegen::emit_hir_c` emits
/// and returns the full-fidelity `i64` it printed to stdout (the wrapper
/// always `printf("%lld\n", result)` before returning process exit code
/// `0`), instead of only checking that printed value against a literal.
/// Callers use this to compare the actual computed value across backends
/// rather than independently checking each backend against a hardcoded
/// sentinel.
pub(super) fn run_and_capture_i64(path: &Path) -> i64 {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{} failed", path.display());
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().parse::<i64>().unwrap_or_else(|error| {
        panic!(
            "{} printed a non-i64 result {stdout:?}: {error}",
            path.display()
        )
    })
}

/// The `i64` a project execution returned, or a panic naming `context` if it
/// did not run to a return at all (a language failure, fuel exhaustion, or a
/// call-depth violation). Used to thread the interpreter's actual computed
/// value into a cross-backend comparison instead of asserting each backend
/// individually returned a hardcoded sentinel.
pub(super) fn interpreter_i64(outcome: &ProjectExecutionOutcome, context: &str) -> i64 {
    match outcome {
        ProjectExecutionOutcome::Returned(value) => *value,
        other => panic!("{context}: {other:?}"),
    }
}
