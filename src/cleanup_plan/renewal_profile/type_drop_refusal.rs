//! Closed selected inventory type/drop failure data; raw legacy text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeDropRefusal {
    UnsupportedCleanupType,
    UnauthenticatedOption,
}
impl TypeDropRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::UnsupportedCleanupType => {
                "selected physical inventory does not support this cleanup type"
            }
            Self::UnauthenticatedOption => "selected Option metadata is unauthenticated",
        }
    }
}
