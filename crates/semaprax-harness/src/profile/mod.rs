//! Per-project configuration, frozen lock, trust store and provider resolution (HP-02).

pub fn cli_status(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("status")
}

pub fn cli_explain(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("explain")
}

pub fn cli_resolve(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("resolve")
}

pub fn cli_adopt(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("adopt")
}

pub fn cli_trust(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("trust")
}

pub fn cli_revoke(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("revoke")
}

pub fn cli_inspect(_args: &[String], _env: &crate::cli::Environment) -> crate::cli::Outcome {
    crate::cli::unimplemented_verb("inspect")
}
