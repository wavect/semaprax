//! Embedding API v1: a small, versioned Rust entry point for checking one
//! caller-supplied SEMAPRAX compilation unit, built for issue #203 ("publish
//! a small stable Semaprax embedding API with explicit host capabilities").
//!
//! # Relationship to `src/semantic_embedding/` (issue #203's other tranche)
//!
//! [`crate::semantic_embedding`] also cites issue #203, but it is a vector-
//! embedding boundary (turning bytes into an `f32` vector for retrieval), an
//! entirely different sense of the word "embedding" from the one issue #203
//! actually asks for: *embedding the compiler itself* into a host process
//! ("editors, build systems, services, and applications embed parsing,
//! checking, interpretation, semantic query, candidate validation, and
//! selected execution without spawning the CLI"). That module's own doc
//! says so plainly ("Issue #203 asks for a much larger surface... None of
//! that lives in this module"). This module is the first real slice of
//! *that* surface: compiler/session-shaped, not vector-shaped.
//!
//! # What this module is
//!
//! [`check_source`] parses and statically analyzes one caller-supplied
//! source string and returns a closed, versioned [`CheckOutcome`] — never
//! the internal [`crate::ast::Program`] or [`crate::hir::Analysis`] (whose
//! `resolved` field carries [`crate::hir::ResolvedProgram`]). Both stay
//! compiler-owned, exactly as issue #203's "Validated internals remain
//! compiler-owned" acceptance criterion requires: an embedder gets
//! diagnostics and a revision hash, never a value whose shape this crate is
//! free to change without notice.
//!
//! No capability is required to call [`check_source`]: checking is a pure
//! function of the bytes the caller passes in (module-level "in scope"
//! bullets "Source/Project load" and "Check"). It opens no file, spawns no
//! process, and reaches no network — the only input is `source`, and
//! `unit_name` is used solely to label diagnostics, never to read a path
//! (`unit_name_is_never_read_from_disk` below proves this against a path
//! that does not exist on this machine). A future slice that reaches
//! "Deterministic interpreter execution for admitted profiles" would need
//! its own explicit capability, mirroring
//! [`crate::live_invocation::model_invoke::ModelInvokeCapability`] and
//! [`crate::semantic_embedding::capability::EmbeddingCapability`]'s shape;
//! this module does not attempt execution and grants none.
//!
//! # What this module is not
//!
//! It is not a session or a handle: there is no `open`/`close` lifecycle,
//! no persisted state between calls, and no `Project`/multi-file workspace
//! load (`docs/PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md`'s
//! `SemanticWorkspaceService` already owns that, for its own operation
//! surface). It is not a C ABI. It does not run the deterministic
//! interpreter. It does not do candidate validate/replay. See
//! `docs/EMBEDDING-API-V1.md` for the complete scope statement and
//! nonclaims this module is honestly bounded by.
//!
//! # Panic normalization
//!
//! A parser or analyzer defect must never unwind across this API boundary
//! into a host's own call stack. [`check_source`] therefore runs the actual
//! analysis inside [`std::panic::catch_unwind`] and converts a caught panic
//! into a [`crate::diagnostic::Diagnostic`] carrying
//! [`PANIC_NORMALIZED_DIAGNOSTIC_CODE`], a code reserved for exactly this
//! case and never produced by parsing or analyzing real source. Because a
//! real compiler defect could not be manufactured honestly for this test
//! suite, the panic path is proven with an internal test-only
//! [`SourceChecker`] double that panics on purpose
//! (`embedding_boundary_normalizes_a_panic_into_a_diagnostic_never_
//! propagating_the_unwind`) — the same seam-substitution pattern
//! `src/semantic_embedding/fixture.rs`'s `ScriptedEmbeddingProvider` uses to
//! prove paths the real fixture cannot reach.

use crate::diagnostic::{Diagnostic, Severity};

/// A diagnostic code reserved for [`check_source`]'s panic-normalization
/// path. No parser or analyzer diagnostic uses this code; it names an
/// embedding-boundary defect, never a property of the checked program.
pub const PANIC_NORMALIZED_DIAGNOSTIC_CODE: &str = "SPX-EMB001";

/// This embedding surface's own compatibility version — unrelated to any
/// checked program's semantics. See "Compatibility policy" in
/// `docs/EMBEDDING-API-V1.md`.
pub const EMBEDDING_API_VERSION: EmbeddingApiVersion = EmbeddingApiVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

/// A semantic version for this Rust embedding surface. Two builds are
/// compatible for this module's calls exactly when their `major` values
/// are equal; `minor`/`patch` are informational only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingApiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl EmbeddingApiVersion {
    /// Whether a caller that was built against `requested_major` can rely on
    /// this build's `check_source` contract. Only the major version is
    /// checked: `minor`/`patch` differences never change accepted inputs or
    /// [`CheckOutcome`]'s shape (see the compatibility policy in
    /// `docs/EMBEDDING-API-V1.md`), while a differing major version is
    /// explicitly refused rather than silently assumed compatible.
    pub fn is_compatible_with(&self, requested_major: u16) -> bool {
        self.major == requested_major
    }
}

/// The closed outcome of checking one caller-supplied compilation unit.
/// Deliberately does not carry [`crate::ast::Program`] or any other
/// internal compiler value: only diagnostics and the canonical revision
/// hash [`crate::graph::revision`] already publishes for the same purpose
/// elsewhere in this crate.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    /// Echoes the `unit_name` the caller passed in; never read from disk.
    pub unit_name: String,
    /// `true` exactly when no diagnostic in `diagnostics` is
    /// [`Severity::Error`] — matching the `severity.is_error()` rule the CLI
    /// `check` command and [`crate::hir::analyze`] both already use.
    pub ok: bool,
    /// Every diagnostic the parser and analyzer produced, warnings
    /// included. Unlike the crate-root [`crate::check`] helper (which
    /// discards non-error diagnostics on success), this module always
    /// returns the full set so a host does not silently lose warnings.
    pub diagnostics: Vec<Diagnostic>,
    /// [`crate::graph::revision`] of the checked program, present only when
    /// `ok` is `true` — an unresolvable program has no canonical revision to
    /// report.
    pub revision: Option<String>,
}

/// One parsed-and-analyzed unit, before being rendered into the public
/// [`CheckOutcome`]. Kept private so no internal compiler type crosses this
/// module's boundary.
struct UnitAnalysis {
    diagnostics: Vec<Diagnostic>,
    ok: bool,
    revision: Option<String>,
}

/// Internal seam behind [`check_source`], not part of the public API.
/// Exists only so the panic-normalization boundary in [`check_with`] can be
/// exercised by a test double that panics on purpose, without depending on
/// discovering an actual parser/analyzer defect.
trait SourceChecker {
    fn analyze(&self, unit_name: &str, source: &str) -> Result<UnitAnalysis, Diagnostic>;
}

/// The only [`SourceChecker`] this crate ships for real use: the ordinary
/// parse-then-analyze pipeline, over caller-supplied bytes only.
struct StandardChecker;

impl SourceChecker for StandardChecker {
    fn analyze(&self, unit_name: &str, source: &str) -> Result<UnitAnalysis, Diagnostic> {
        // `crate::parse` reads only `source`; `unit_name` labels diagnostics
        // and is never opened as a path (see `parser::Parser::new`, and
        // `unit_name_is_never_read_from_disk` below).
        let program = crate::parse(source, unit_name)?;
        let diagnostics = crate::hir::analyze(&program).diagnostics;
        let ok = !diagnostics.iter().any(|item| item.severity.is_error());
        let revision = ok.then(|| crate::graph::revision(&program));
        Ok(UnitAnalysis {
            diagnostics,
            ok,
            revision,
        })
    }
}

fn check_with(checker: &dyn SourceChecker, unit_name: &str, source: &str) -> CheckOutcome {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        checker.analyze(unit_name, source)
    }));
    match outcome {
        Ok(Ok(analysis)) => CheckOutcome {
            unit_name: unit_name.to_owned(),
            ok: analysis.ok,
            diagnostics: analysis.diagnostics,
            revision: analysis.revision,
        },
        Ok(Err(parse_failure)) => CheckOutcome {
            unit_name: unit_name.to_owned(),
            ok: false,
            diagnostics: vec![parse_failure],
            revision: None,
        },
        Err(_panic_payload) => CheckOutcome {
            unit_name: unit_name.to_owned(),
            ok: false,
            diagnostics: vec![Diagnostic {
                code: PANIC_NORMALIZED_DIAGNOSTIC_CODE,
                severity: Severity::Error,
                message: format!(
                    "the embedding check boundary caught a panic while analyzing {unit_name:?} \
                     and normalized it to this diagnostic instead of letting the unwind cross \
                     the embedding API boundary"
                ),
                path: Some(unit_name.to_owned()),
                span: None,
                help: Some(
                    "this names an embedding-boundary defect, not a property of the checked \
                     source; report it against the compiler"
                        .to_owned(),
                ),
            }],
            revision: None,
        },
    }
}

/// Check one caller-supplied SEMAPRAX compilation unit.
///
/// `source` is the exact bytes to check; `unit_name` only labels
/// diagnostics and never names a path this function reads. No capability is
/// required: this is a pure, read-only function of its two arguments, and
/// it panics never (a panic inside the pipeline is caught and normalized
/// into [`PANIC_NORMALIZED_DIAGNOSTIC_CODE`] instead of unwinding out of
/// this call). See the module documentation for what this deliberately does
/// not do (session lifecycle, multi-file Project load, execution, a C ABI).
pub fn check_source(unit_name: &str, source: &str) -> CheckOutcome {
    check_with(&StandardChecker, unit_name, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = "module app.hello;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    42\n}\n";

    #[test]
    fn valid_source_checks_ok_with_no_diagnostics_and_a_revision() {
        let outcome = check_source("hello.spx", HELLO);
        assert!(outcome.ok, "expected ok, got {:?}", outcome.diagnostics);
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.revision.is_some());
        assert_eq!(outcome.unit_name, "hello.spx");
    }

    #[test]
    fn a_declaration_missing_id_still_checks_ok_but_keeps_its_warning() {
        // `crate::check` (the crate-root helper) discards non-error
        // diagnostics on success; this module must not repeat that, or a
        // host embedding it would silently lose every warning on a
        // successful check.
        let source = "module app.warned;\n\nfn main() -> i64\n{\n    42\n}\n";
        let outcome = check_source("warned.spx", source);
        assert!(outcome.ok, "expected ok, got {:?}", outcome.diagnostics);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|item| item.code == "SPX-S103"),
            "expected the missing-@id warning to survive a successful check, got {:?}",
            outcome.diagnostics
        );
        assert!(outcome
            .diagnostics
            .iter()
            .all(|item| !item.severity.is_error()));
    }

    #[test]
    fn malformed_source_fails_with_the_specific_parser_diagnostic() {
        // A module with no function at all is refused by the parser with
        // SPX-P101, before any HIR analysis runs.
        let outcome = check_source("empty.spx", "module app.empty;\n");
        assert!(!outcome.ok);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].code, "SPX-P101");
        assert!(outcome.diagnostics[0].severity.is_error());
        assert!(outcome.revision.is_none());
        // Distinguish this genuine parser refusal from the unrelated
        // panic-normalization path: neither code's text appears in the
        // other's diagnostic.
        assert_ne!(
            outcome.diagnostics[0].code,
            PANIC_NORMALIZED_DIAGNOSTIC_CODE
        );
        assert!(!outcome.diagnostics[0]
            .message
            .contains(PANIC_NORMALIZED_DIAGNOSTIC_CODE));
    }

    #[test]
    fn unit_name_is_never_read_from_disk() {
        // A path that certainly does not exist on this machine must not
        // cause an I/O failure: `unit_name` only labels diagnostics.
        let outcome = check_source("/definitely/does/not/exist/on/this/machine/unit.spx", HELLO);
        assert!(
            outcome.ok,
            "checking must depend only on `source`, not on whether `unit_name` \
             names a real file; got {:?}",
            outcome.diagnostics
        );
    }

    struct PanickingChecker;

    impl SourceChecker for PanickingChecker {
        fn analyze(&self, _unit_name: &str, _source: &str) -> Result<UnitAnalysis, Diagnostic> {
            panic!("deliberate test panic: proving it never crosses the embedding boundary");
        }
    }

    #[test]
    fn embedding_boundary_normalizes_a_panic_into_a_diagnostic_never_propagating_the_unwind() {
        let outcome = check_with(&PanickingChecker, "panicking.spx", "irrelevant");
        assert!(!outcome.ok);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].code,
            PANIC_NORMALIZED_DIAGNOSTIC_CODE
        );
        assert!(outcome.diagnostics[0].severity.is_error());
        assert!(outcome.revision.is_none());
        // Distinguish this internal-defect path from a genuine parser
        // refusal: the panic diagnostic never mentions a real parser code.
        assert!(!outcome.diagnostics[0].message.contains("SPX-P101"));
    }

    #[test]
    fn version_negotiation_accepts_matching_major_and_refuses_a_different_one() {
        assert!(EMBEDDING_API_VERSION.is_compatible_with(1));
        assert!(!EMBEDDING_API_VERSION.is_compatible_with(2));
        assert!(!EMBEDDING_API_VERSION.is_compatible_with(0));
    }
}
