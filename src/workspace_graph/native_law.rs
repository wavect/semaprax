//! Internal inert Graph carrier for an authored native-law source.
//!
//! The returned Program is never a source projection. The managed generation
//! retains the original law bytes and exact native source facts.

use super::*;

pub(super) fn inert_program(
    source: &WorkspaceSource,
    remaining: usize,
) -> Result<Option<(Program, usize)>, Vec<Diagnostic>> {
    let Ok(law) = crate::native_law_source::parse(&source.source, &source.path) else {
        return Ok(None);
    };
    let (canonical, overflowed) =
        crate::bounded_output::with_limit(remaining, || crate::native_law_source::canonical(&law));
    if overflowed {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    if canonical != source.source {
        return Err(vec![graph_error(
            "SPX-G170",
            format!(
                "workspace native-law source `{}` is not canonical",
                source.path
            ),
        )]);
    }
    // The Workspace HIR has no executable representation for a law.
    // Resolve an internal empty carrier solely to retain module/path
    // identity. Recovered sources and manifest facts keep the exact
    // authored law bytes, never this carrier's text.
    let inert = Program {
        path: source.path.clone(),
        module: law.module_id,
        module_uses: Vec::new(),
        permits: Vec::new(),
        types: Vec::new(),
        interfaces: Vec::new(),
        protocols: Vec::new(),
        implementations: Vec::new(),
        session_protocols: Vec::new(),
        agents: Vec::new(),
        functions: Vec::new(),
    };
    Ok(Some((inert, canonical.len())))
}
