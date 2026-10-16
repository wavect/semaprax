//! Closed complete selected slot/region/leaf semantic labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralRefusal {
    NoRootRegion,
    RegionIdsNoncontiguous,
    InvalidRegionRoot,
    ParentAfterChild,
    UnknownRegionStorage,
    RepeatedRegionStorage,
    UnknownNormalExit,
    UnassignedStorage,
    TooManySlots,
    TooManyRegions,
    TooManyFlags,
    NoncontiguousFlags,
    LeafCount,
    FieldIndex,
    NestedLeaf,
    ConditionalOwnership,
    RepeatedFlag,
    RepeatedPlace,
}
impl StructuralRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::NoRootRegion => "cleanup plan has no root region",
            Self::RegionIdsNoncontiguous => "cleanup region IDs are not contiguous",
            Self::InvalidRegionRoot => "cleanup region tree has an invalid root",
            Self::ParentAfterChild => "cleanup region parent must precede its child",
            Self::UnknownRegionStorage => "cleanup region references unknown storage",
            Self::RepeatedRegionStorage => "cleanup storage is assigned to more than one region",
            Self::UnknownNormalExit => "cleanup region references an unknown normal-scope exit",
            Self::UnassignedStorage => {
                "cleanup plan has storage not assigned to exactly one region"
            }
            Self::TooManySlots => "too many cleanup slots",
            Self::TooManyRegions => "too many cleanup regions",
            Self::TooManyFlags => "too many liveness flags",
            Self::NoncontiguousFlags => "cleanup liveness flags are not contiguous from zero",
            Self::LeafCount => "selected record leaf count is outside direct shape",
            Self::FieldIndex => "selected record field index is noncanonical",
            Self::NestedLeaf => "selected record has a nested or conditional leaf",
            Self::ConditionalOwnership => "selected record has conditional ownership",
            Self::RepeatedFlag => "selected liveness flag is repeated",
            Self::RepeatedPlace => "selected cleanup leaf place is repeated",
        }
    }
}
