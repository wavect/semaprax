use std::collections::{BTreeMap, BTreeSet};

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};

use super::model::Subject;
use super::subject::{parse_subject, render_coordinate};
use super::wire::{charge, confusion_error, cycle_error, limit_error, render_wrapper};
use super::{
    Coordinate, LockOptions, LOCK_DOMAIN, MAX_CAPABILITIES, MAX_DEPENDENCIES, MAX_DEPTH, MAX_EDGES,
    MAX_OUTPUT_BYTES, MAX_PACKAGES, MAX_SUBJECT_BYTES, MAX_TOTAL_SUBJECT_BYTES, MAX_WORK_UNITS,
    SCHEMA,
};

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

pub(super) fn build(subjects: &[String], options: &LockOptions) -> Result<String, Diagnostic> {
    super::validate_options(options)?;
    if subjects.is_empty() || subjects.len() > MAX_PACKAGES {
        return Err(limit_error("semantic lock package count is outside bounds"));
    }
    let mut work = 0usize;
    let mut total_bytes = 0usize;
    let mut map = BTreeMap::new();
    for bytes in subjects {
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| limit_error("subject byte accounting overflow"))?;
        if bytes.len() > MAX_SUBJECT_BYTES || total_bytes > MAX_TOTAL_SUBJECT_BYTES {
            return Err(limit_error("semantic lock subject byte bound exceeded"));
        }
        charge(&mut work, 1)?;
        let subject = parse_subject(bytes, &mut work)?;
        if map.insert(subject.coordinate.clone(), subject).is_some() {
            return Err(confusion_error("duplicate semantic package coordinate"));
        }
    }
    let mut identities = BTreeMap::<&str, &str>::new();
    for coordinate in map.keys() {
        if identities
            .insert(&coordinate.package, &coordinate.version)
            .is_some()
        {
            return Err(confusion_error(
                "multiple semantic package versions share one identity",
            ));
        }
    }
    let mut edges = Vec::new();
    let mut indegree = map
        .keys()
        .cloned()
        .map(|key| (key, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<Coordinate, Vec<Coordinate>>::new();
    for (dependent, subject) in &map {
        for dependency in &subject.dependencies {
            if !map.contains_key(dependency) {
                return Err(confusion_error(
                    "missing or version-confused semantic dependency",
                ));
            }
            *indegree.get_mut(dependent).expect("present") += 1;
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(dependent.clone());
            edges.push((dependency.clone(), dependent.clone()));
            if edges.len() > MAX_EDGES {
                return Err(limit_error("semantic dependency edges exceed limit"));
            }
        }
    }
    for rows in dependents.values_mut() {
        rows.sort();
    }
    edges.sort();
    let mut ready = indegree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(c, _)| c.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    let mut depth = map
        .keys()
        .cloned()
        .map(|c| (c, 1usize))
        .collect::<BTreeMap<_, _>>();
    while let Some(next) = ready.pop_first() {
        order.push(next.clone());
        for dependent in dependents.get(&next).into_iter().flatten() {
            let proposed = depth[&next]
                .checked_add(1)
                .ok_or_else(|| limit_error("dependency depth overflow"))?;
            depth
                .entry(dependent.clone())
                .and_modify(|d| *d = (*d).max(proposed));
            if depth[dependent] > MAX_DEPTH {
                return Err(limit_error("semantic dependency depth exceeds limit"));
            }
            let count = indegree.get_mut(dependent).expect("present");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
            charge(&mut work, 1)?;
        }
    }
    if order.len() != map.len() {
        return Err(cycle_error("semantic dependency graph contains a cycle"));
    }
    let mut closures = BTreeMap::<Coordinate, BTreeSet<String>>::new();
    for coordinate in &order {
        let subject = &map[coordinate];
        let mut closure = subject
            .capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for dependency in &subject.dependencies {
            closure.extend(closures[dependency].iter().cloned());
        }
        if closure.len() > MAX_CAPABILITIES {
            return Err(limit_error("transitive capability closure exceeds limit"));
        }
        charge(&mut work, closure.len())?;
        closures.insert(coordinate.clone(), closure);
    }
    let targets = aggregate_targets(map.values())?;
    let edge_count = edges.len();
    let (lock, overflowed) = bounded_output::with_limit(64 * 1024 * 1024, || {
        let packages = order
            .iter()
            .map(|coordinate| render_package(&map[coordinate], &closures[coordinate]))
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let edge_json = edges
            .iter()
            .map(|(dependency, dependent)| {
                bf!(
                    "{{\"dependency\":{},\"dependent\":{}}}",
                    render_coordinate(dependency),
                    render_coordinate(dependent)
                )
            })
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let target_json = targets
            .iter()
            .map(|(target, status)| {
                bf!(
                    "{{\"target\":{},\"status\":{}}}",
                    quote_json(target),
                    quote_json(status)
                )
            })
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let payload = bf!("{{\"schema\":{},\"packages\":[{}],\"edges\":[{}],\"target_matrix\":[{}],\"limits\":{{\"max_packages\":{MAX_PACKAGES},\"max_subject_bytes\":{MAX_SUBJECT_BYTES},\"max_total_subject_bytes\":{MAX_TOTAL_SUBJECT_BYTES},\"max_dependencies\":{MAX_DEPENDENCIES},\"max_edges\":{MAX_EDGES},\"max_depth\":{MAX_DEPTH},\"max_capabilities\":{MAX_CAPABILITIES},\"max_work_units\":{MAX_WORK_UNITS},\"max_output_bytes\":{MAX_OUTPUT_BYTES},\"requested_max_bytes\":{}}},\"budget\":{{\"used_packages\":{},\"used_subject_bytes\":{},\"used_edges\":{},\"used_depth\":{},\"used_work_units\":{}}},\"nonclaims\":[\"offline_source_authenticated_lock\",\"no_resolver_registry_fetch_build_execution_or_publication\",\"capabilities_are_integrity_bound_claims_not_enforcement\",\"lock_is_evidence_not_authority\"]}}", quote_json(SCHEMA), packages, edge_json, target_json, options.max_bytes, map.len(), total_bytes, edge_count, depth.values().copied().max().unwrap_or(0), work);
        render_wrapper(SCHEMA, LOCK_DOMAIN, &payload)
    });
    if overflowed {
        return Err(limit_error("semantic lock render budget exceeded"));
    }
    if lock.len() > options.max_bytes || lock.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("semantic lock output bound exceeded"));
    }
    Ok(lock)
}

fn render_package(subject: &Subject, closure: &BTreeSet<String>) -> String {
    bf!("{{\"package\":{},\"version\":{},\"subject_digest\":{},\"subject_bytes\":{},\"report_digest\":{},\"report_bytes\":{},\"revision\":{},\"targets\":[{}],\"dependencies\":[{}],\"capabilities\":[{}],\"capability_closure\":[{}]}}", quote_json(&subject.coordinate.package), quote_json(&subject.coordinate.version), quote_json(&subject.digest), subject.bytes, quote_json(&subject.report_digest), subject.report.len(), quote_json(&subject.revision), subject.targets.iter().map(|(t,s)| bf!("{{\"target\":{},\"status\":{}}}",quote_json(t),quote_json(s))).collect::<Vec<_>>().budgeted_join(","), subject.dependencies.iter().map(render_coordinate).collect::<Vec<_>>().budgeted_join(","), subject.capabilities.iter().map(|v| quote_json(v)).collect::<Vec<_>>().budgeted_join(","), closure.iter().map(|v| quote_json(v)).collect::<Vec<_>>().budgeted_join(","))
}

pub(super) fn aggregate_targets<'a>(
    subjects: impl Iterator<Item = &'a Subject>,
) -> Result<BTreeMap<String, String>, Diagnostic> {
    let subjects = subjects.collect::<Vec<_>>();
    let keys = subjects
        .first()
        .ok_or_else(|| confusion_error("empty target inventory"))?
        .targets
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut result = BTreeMap::new();
    for key in keys {
        let mut status = "available";
        for subject in &subjects {
            let value = subject
                .targets
                .get(&key)
                .ok_or_else(|| confusion_error("target inventory disagreement"))?;
            status = match (status, value.as_str()) {
                (_, "unproven") => "unproven",
                ("unproven", _) => "unproven",
                (_, "unavailable") => "unavailable",
                (current, "available") => current,
                _ => return Err(confusion_error("unknown target status")),
            };
        }
        result.insert(key, status.to_owned());
    }
    if subjects.iter().any(|s| s.targets.len() != result.len()) {
        return Err(confusion_error("target inventory disagreement"));
    }
    Ok(result)
}
