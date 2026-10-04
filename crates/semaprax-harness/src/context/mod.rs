//! Native-first context broker with revision-safe caches (HP-05).

pub fn cli_context(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("context")
}
