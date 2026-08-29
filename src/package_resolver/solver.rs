use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;

use super::catalog::{Catalog, Entry};
use super::model::ParsedRequirement;
use super::semver::{Range, Version};
use super::wire;
use super::{ResolutionInput, MAX_DECISIONS, MAX_DEPTH, MAX_EDGES, MAX_SELECTED_PACKAGES};

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum ConstraintTag {
    Root(usize),
    Dependency(Coordinate, usize),
}

#[derive(Clone, Copy)]
enum ConstraintValue {
    Range(Range),
    Exact(Version),
}

#[derive(Clone)]
struct Constraint {
    tag: ConstraintTag,
    value: ConstraintValue,
}

impl Constraint {
    fn admits(&self, version: Version) -> bool {
        match self.value {
            ConstraintValue::Range(range) => range.contains(version),
            ConstraintValue::Exact(expected) => expected == version,
        }
    }
}

#[derive(Clone)]
struct State<'catalog, 'input> {
    selected: BTreeMap<String, &'catalog Entry<'input>>,
    constraints: BTreeMap<String, Vec<Constraint>>,
    edges: BTreeSet<(Coordinate, Coordinate)>,
    depth: usize,
}

pub(super) struct Solved<'catalog, 'input> {
    pub(super) selected: BTreeMap<String, &'catalog Entry<'input>>,
    pub(super) edges: usize,
    pub(super) depth: usize,
    pub(super) decisions: usize,
}

struct Search<'catalog, 'input, 'context> {
    input: &'context ResolutionInput,
    catalog: &'catalog Catalog<'input>,
    allowed: BTreeSet<&'context str>,
    work: &'context mut usize,
    decisions: usize,
    saw_policy_rejection: bool,
    saw_structural_rejection: bool,
}

pub(super) fn solve<'catalog, 'input>(
    input: &ResolutionInput,
    requirements: &[ParsedRequirement],
    catalog: &'catalog Catalog<'input>,
    work: &mut usize,
) -> Result<Solved<'catalog, 'input>, Diagnostic> {
    let mut constraints = BTreeMap::<String, Vec<Constraint>>::new();
    for requirement in requirements {
        constraints
            .entry(requirement.package.clone())
            .or_default()
            .push(Constraint {
                tag: ConstraintTag::Root(requirement.row),
                value: ConstraintValue::Range(requirement.range),
            });
    }
    let initial = State {
        selected: BTreeMap::new(),
        constraints,
        edges: BTreeSet::new(),
        depth: 0,
    };
    let mut search = Search {
        input,
        catalog,
        allowed: input
            .allowed_capabilities
            .iter()
            .map(String::as_str)
            .collect(),
        work,
        decisions: 0,
        saw_policy_rejection: false,
        saw_structural_rejection: false,
    };
    let Some(state) = search.visit(initial)? else {
        return Err(if search.saw_structural_rejection {
            wire::resolution_error("no complete package graph satisfies all constraints")
        } else if search.saw_policy_rejection {
            wire::policy_error("no complete package graph satisfies target/capability policy")
        } else {
            wire::resolution_error("no package candidate satisfies the root requirements")
        });
    };
    Ok(Solved {
        selected: state.selected,
        edges: state.edges.len(),
        depth: state.depth,
        decisions: search.decisions,
    })
}

impl<'catalog, 'input, 'context> Search<'catalog, 'input, 'context> {
    fn visit(
        &mut self,
        state: State<'catalog, 'input>,
    ) -> Result<Option<State<'catalog, 'input>>, Diagnostic> {
        let unresolved = state
            .constraints
            .keys()
            .find(|package| !state.selected.contains_key(*package))
            .cloned();
        let Some(package) = unresolved else {
            return Ok(Some(state));
        };
        let Some(candidates) = self.catalog.by_package.get(&package) else {
            self.saw_structural_rejection = true;
            return Ok(None);
        };
        for index in candidates {
            if self.decisions >= MAX_DECISIONS {
                return Err(wire::limit_error("resolver decision bound exceeded"));
            }
            self.decisions += 1;
            wire::charge(self.work, 1)?;
            let candidate = &self.catalog.entries[*index];
            let mut branch = state.clone();
            let Some(constraints) = branch.constraints.get(&package) else {
                return Err(wire::resolution_error(
                    "resolver state lost an unresolved constraint",
                ));
            };
            let mut admitted = true;
            for constraint in constraints {
                wire::charge(self.work, 1)?;
                if !constraint.admits(candidate.version) {
                    admitted = false;
                    self.saw_structural_rejection = true;
                    break;
                }
            }
            if admitted {
                wire::charge(self.work, 1)?;
                if candidate
                    .subject
                    .targets
                    .get(&self.input.target)
                    .map(String::as_str)
                    != Some("available")
                {
                    admitted = false;
                    self.saw_policy_rejection = true;
                }
            }
            if admitted {
                for capability in &candidate.subject.capabilities {
                    wire::charge(self.work, 1)?;
                    if !self.allowed.contains(capability.as_str()) {
                        admitted = false;
                        self.saw_policy_rejection = true;
                        break;
                    }
                }
            }
            if admitted {
                branch.selected.insert(package.clone(), candidate);
                admitted = self.insert_dependencies(&mut branch, candidate)?;
            }
            if admitted {
                admitted = validate_graph(&mut branch);
                if !admitted {
                    self.saw_structural_rejection = true;
                }
            }
            if admitted {
                if let Some(result) = self.visit(branch)? {
                    return Ok(Some(result));
                }
            }
            wire::charge(self.work, 1)?;
        }
        Ok(None)
    }

    fn insert_dependencies(
        &mut self,
        branch: &mut State<'catalog, 'input>,
        candidate: &'catalog Entry<'input>,
    ) -> Result<bool, Diagnostic> {
        let dependent = candidate.subject.coordinate.clone();
        for (row, (dependency, version)) in candidate
            .subject
            .dependencies
            .iter()
            .zip(&candidate.dependency_versions)
            .enumerate()
        {
            wire::charge(self.work, 1)?;
            branch
                .constraints
                .entry(dependency.package.clone())
                .or_default()
                .push(Constraint {
                    tag: ConstraintTag::Dependency(dependent.clone(), row),
                    value: ConstraintValue::Exact(*version),
                });
            let Some(constraints) = branch.constraints.get_mut(&dependency.package) else {
                return Err(wire::resolution_error(
                    "resolver state lost an inserted dependency constraint",
                ));
            };
            constraints.sort_by(|left, right| left.tag.cmp(&right.tag));
            wire::charge(self.work, 1)?;
            branch.edges.insert((dependency.clone(), dependent.clone()));
            if !self
                .catalog
                .by_coordinate
                .contains_key(&(dependency.package.clone(), *version))
            {
                self.saw_structural_rejection = true;
                return Ok(false);
            }
            if branch
                .selected
                .get(&dependency.package)
                .is_some_and(|selected| selected.version != *version)
            {
                self.saw_structural_rejection = true;
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn validate_graph(state: &mut State<'_, '_>) -> bool {
    if state.constraints.len() > MAX_SELECTED_PACKAGES || !admit_edge_count(state.edges.len()) {
        return false;
    }
    let mut indegree = state
        .constraints
        .keys()
        .map(|package| (package.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for (dependency, dependent) in &state.edges {
        *indegree.entry(dependent.package.clone()).or_default() += 1;
        dependents
            .entry(dependency.package.clone())
            .or_default()
            .push(dependent.package.clone());
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(package, _)| package.clone())
        .collect::<BTreeSet<_>>();
    let mut seen = 0usize;
    let mut depths = indegree
        .keys()
        .map(|package| (package.clone(), 1usize))
        .collect::<BTreeMap<_, _>>();
    while let Some(package) = ready.pop_first() {
        seen += 1;
        for dependent in dependents.get(&package).into_iter().flatten() {
            let proposed = depths[&package].saturating_add(1);
            depths
                .entry(dependent.clone())
                .and_modify(|depth| *depth = (*depth).max(proposed));
            let Some(count) = indegree.get_mut(dependent) else {
                return false;
            };
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    let depth = depths.values().copied().max().unwrap_or(0);
    if seen != indegree.len() || !admit_depth(depth) {
        return false;
    }
    state.depth = depth;
    true
}

pub(super) fn admit_edge_count(count: usize) -> bool {
    count <= MAX_EDGES
}

pub(super) fn admit_depth(depth: usize) -> bool {
    depth <= MAX_DEPTH
}
