//! Source bytes shared independently of parsed AST generations.
use super::SourceMetadata;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) enum SourceBytes {
    Owned(String),
    Current(Arc<SourceMetadata>),
}
impl SourceBytes {
    pub(crate) fn current(metadata: Arc<SourceMetadata>) -> Self {
        Self::Current(metadata)
    }
    #[cfg(test)]
    pub(crate) fn make_mut(&mut self) -> &mut String {
        if let Self::Current(metadata) = self {
            *self = Self::Owned(metadata.input().source.clone());
        }
        match self {
            Self::Owned(bytes) => bytes,
            Self::Current(_) => unreachable!(),
        }
    }
}
impl From<String> for SourceBytes {
    fn from(bytes: String) -> Self {
        Self::Owned(bytes)
    }
}
impl Deref for SourceBytes {
    type Target = String;
    fn deref(&self) -> &String {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Current(metadata) => &metadata.input().source,
        }
    }
}
impl AsRef<str> for SourceBytes {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl std::fmt::Debug for SourceBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}
impl<T: AsRef<str> + ?Sized> PartialEq<T> for SourceBytes {
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}
impl Eq for SourceBytes {}
impl serde::Serialize for SourceBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl std::fmt::Display for SourceBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}
impl From<SourceBytes> for String {
    fn from(bytes: SourceBytes) -> Self {
        match bytes {
            SourceBytes::Owned(bytes) => bytes,
            SourceBytes::Current(metadata) => metadata.input().source.clone(),
        }
    }
}
impl SourceBytes {
    /// Move the exact retained buffer out when this is its last owner. Since
    /// f7aeaf32c a current source is shared through `SourceMetadata`; a caller
    /// that has released every other holder receives the original allocation
    /// without a copy. A still-shared carrier keeps the cloning conversion.
    pub(crate) fn into_unshared_string(self) -> String {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Current(metadata) => match Arc::try_unwrap(metadata) {
                Ok(metadata) => metadata.input.source,
                Err(shared) => shared.input().source.clone(),
            },
        }
    }
}
#[cfg(test)]
impl std::ops::DerefMut for SourceBytes {
    fn deref_mut(&mut self) -> &mut String {
        self.make_mut()
    }
}
