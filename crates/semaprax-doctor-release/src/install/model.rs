pub const MAX_GENERATIONS_LIMIT: u16 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreExpectation {
    pub maximum_generations: u16,
}

impl Default for StoreExpectation {
    fn default() -> Self {
        Self {
            maximum_generations: 8,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(pub(super) String);

impl GenerationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallReceipt {
    pub generation: GenerationId,
    pub installed_new: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryReceipt {
    pub removed_generation: Option<GenerationId>,
}
