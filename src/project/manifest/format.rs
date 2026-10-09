use crate::diagnostic::Diagnostic;

use super::ProjectManifest;

impl ProjectManifest {
    /// Parse for the explicit formatter, relaxing only table text canonicality.
    #[doc(hidden)]
    pub fn parse_for_format(source: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::parse_mode(source, true)
    }
}
