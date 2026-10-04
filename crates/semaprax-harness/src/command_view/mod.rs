//! Authoritative command results versus model-facing views (HP-08).

pub fn cli_exec(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("exec")
}

pub fn cli_recover(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("recover")
}
