//! Immutable module-owned decoded text. The carrier layout is the same pinned
//! `Arc<String>` allocation as ExpressionId; equal bytes do not prove sharing.
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticUtf8(Option<Arc<String>>);
impl StaticUtf8 {
    pub const OWNED_ALLOCATION_CARRIER_BYTES: usize =
        super::super::ExpressionId::OWNED_ALLOCATION_CARRIER_BYTES;

    pub(crate) fn from_text(value: &str) -> Self {
        let Some(bytes) = Self::OWNED_ALLOCATION_CARRIER_BYTES.checked_add(value.len()) else {
            return Self(None);
        };
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Self(None);
        }
        Self(Some(Arc::new(value.to_owned())))
    }

    pub(crate) fn from_owned(value: String) -> Self {
        if !crate::bounded_output::reserve_active_required(Self::OWNED_ALLOCATION_CARRIER_BYTES) {
            return Self(None);
        }
        Self(Some(Arc::new(value)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_deref().map_or("", String::as_str)
    }
    pub fn owned_allocation_bytes(&self) -> Option<usize> {
        Self::OWNED_ALLOCATION_CARRIER_BYTES.checked_add(self.0.as_deref()?.capacity())
    }
    pub(crate) fn cached_text(&self) -> Option<&String> {
        self.0.as_deref()
    }
    pub(crate) fn is_backed(&self) -> bool {
        self.0.is_some()
    }
}
impl Deref for StaticUtf8 {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for StaticUtf8 {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
