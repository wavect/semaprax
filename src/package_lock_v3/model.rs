use std::collections::BTreeMap;

use super::{Coordinate, DependencyRequirement};

#[derive(Clone)]
pub(super) struct Subject {
    pub(super) coordinate: Coordinate,
    pub(super) digest: String,
    pub(super) bytes: usize,
    pub(super) report: String,
    pub(super) report_digest: String,
    pub(super) revision: String,
    pub(super) targets: BTreeMap<String, String>,
    pub(super) dependencies: Vec<DependencyRequirement>,
    pub(super) capabilities: Vec<String>,
}
