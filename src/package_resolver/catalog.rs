use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::{self, ResolutionSubject};

use super::model;
use super::semver::{self, compare_coordinates, Version};
use super::wire;
use super::{
    ResolutionInput, MAX_SUBJECTS, MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES,
    MAX_VERSIONS_PER_PACKAGE,
};

pub(super) struct Entry<'a> {
    pub(super) subject: ResolutionSubject,
    pub(super) version: Version,
    pub(super) dependency_versions: Vec<Version>,
    pub(super) bytes: &'a str,
}

pub(super) struct Catalog<'a> {
    pub(super) entries: Vec<Entry<'a>>,
    pub(super) by_package: BTreeMap<String, Vec<usize>>,
    pub(super) by_coordinate: BTreeMap<(String, Version), usize>,
    pub(super) target_inventory: BTreeSet<String>,
    pub(super) total_bytes: usize,
    pub(super) digest: String,
}

pub(super) fn authenticate<'a>(
    input: &'a ResolutionInput,
    work: &mut usize,
) -> Result<Catalog<'a>, Diagnostic> {
    if input.subjects.is_empty() || input.subjects.len() > MAX_SUBJECTS {
        return Err(wire::limit_error("catalog subject count is outside bounds"));
    }
    let mut total_bytes = 0usize;
    let mut entries = Vec::with_capacity(input.subjects.len());
    for bytes in &input.subjects {
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| wire::limit_error("catalog byte accounting overflow"))?;
        if bytes.len() > MAX_SUBJECT_BYTES || total_bytes > MAX_TOTAL_SUBJECT_BYTES {
            return Err(wire::limit_error("catalog subject byte bound exceeded"));
        }
        wire::charge(work, 1)?;
        let subject = package_lock_v2::authenticate_subject_for_resolution(bytes, work)
            .map_err(|error| wire::map_subject_error(&error))?;
        model::validate_identity(&subject.coordinate.package, "subject package")?;
        let version = semver::parse_version(&subject.coordinate.version)?;
        let dependency_versions = subject
            .dependencies
            .iter()
            .map(|dependency| {
                model::validate_identity(&dependency.package, "dependency package")?;
                semver::parse_version(&dependency.version)
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        entries.push(Entry {
            subject,
            version,
            dependency_versions,
            bytes,
        });
    }
    entries.sort_by(|left, right| {
        compare_coordinates(
            &left.subject.coordinate.package,
            left.version,
            &right.subject.coordinate.package,
            right.version,
        )
    });
    let mut by_package = BTreeMap::<String, Vec<usize>>::new();
    let mut by_coordinate = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let key = (entry.subject.coordinate.package.clone(), entry.version);
        if by_coordinate.insert(key, index).is_some() {
            return Err(wire::resolution_error("duplicate catalog coordinate"));
        }
        let versions = by_package
            .entry(entry.subject.coordinate.package.clone())
            .or_default();
        versions.push(index);
        if versions.len() > MAX_VERSIONS_PER_PACKAGE {
            return Err(wire::limit_error(
                "catalog versions per package exceed limit",
            ));
        }
    }
    for versions in by_package.values_mut() {
        versions.sort_by_key(|index| std::cmp::Reverse(entries[*index].version));
    }
    let target_inventory = entries
        .first()
        .map(|entry| {
            entry
                .subject
                .targets
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .ok_or_else(|| wire::limit_error("empty catalog"))?;
    for entry in &entries {
        for key in entry.subject.targets.keys() {
            wire::charge(work, 1)?;
            if !target_inventory.contains(key) {
                return Err(wire::policy_error("catalog target inventory disagrees"));
            }
        }
        if entry.subject.targets.len() != target_inventory.len() {
            return Err(wire::policy_error("catalog target inventory disagrees"));
        }
    }
    let digest = model::catalog_digest(entries.iter().map(|entry| entry.bytes), entries.len());
    Ok(Catalog {
        entries,
        by_package,
        by_coordinate,
        target_inventory,
        total_bytes,
        digest,
    })
}
