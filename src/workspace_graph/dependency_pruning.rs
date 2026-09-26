//! Skipping HIR construction for bundled-dependency functions that no call
//! site anywhere in the workspace can reach.
//!
//! `builder_bytes` charges every declaration of every admitted module, so a
//! bundled standard-library package costs its whole translation unit however
//! little of it a consumer uses. Issue #124 measured that on
//! `examples/catalog-normalizer-project`: of `std.data.json.dec`'s 27
//! functions only 7 are imported and 20 are in the internal-call closure, and
//! `std.io` is imported for its *types* only, so all 11 of its functions are
//! dead weight. This module removes exactly those functions from the parsed
//! program before the builder forecasts or resolves anything, so the saving
//! is a reduction in real resolver work rather than a narrowing of the
//! forecast.
//!
//! Three properties bound the risk.
//!
//! * **Only bundled dependency modules.** A module whose logical path is not
//!   under `dependencies/` is never touched, so every declaration a project's
//!   own source contains stays fully resolved and reviewable.
//! * **Reachability is over-approximated from canonical source text.** A
//!   function is retained when any identifier token in the source region of a
//!   retained function — its leading trivia, `@id`, signature, contracts and
//!   body — spells its name. That is a superset of every way one declaration
//!   can name another (call, method call, variable, type, contract,
//!   closure), so the fixpoint can only retain too much, never too little.
//!   Anything outside every function region (the module header, `use` lines,
//!   types, interfaces, protocols, implementations, session protocols,
//!   agents) seeds the roots, and so does every function a session protocol
//!   names by persistent id in a `via` clause.
//! * **Reaching a pruned declaration re-checks it.** The retained set is a
//!   pure function of the whole workspace's source bytes, so the moment any
//!   module names a pruned function it is retained again and resolved in
//!   full. `reaching_a_dependency_function_later_still_type_checks_it` proves
//!   that end to end.
//!
//! The scan's own working memory is transient and bounded by a small multiple
//! of the workspace source bytes, which `MAX_TOTAL_SOURCE_BYTES` already caps;
//! it is released before the builder charges its pre-bound, so it never
//! competes with the retained graph for `builder_bytes`.

use super::WorkspaceSource;
use crate::ast::{Function, Program};
use std::collections::BTreeSet;

/// Bundled-dependency functions removed from their parsed programs, with the
/// exact position each was removed from so the full program can be restored
/// byte-for-byte before it is handed to any cache that is keyed per file.
#[derive(Debug, Default)]
pub(super) struct PrunedDependencyFunctions {
    entries: Vec<(usize, Vec<(usize, Function)>)>,
}

impl PrunedDependencyFunctions {
    /// Total number of functions withheld from HIR construction.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn count(&self) -> usize {
        self.entries.iter().map(|(_, removed)| removed.len()).sum()
    }

    /// Put every withheld function back at its original index. The per-file
    /// frontend cache stores a parsed program keyed only by that file's
    /// bytes, but the retained set depends on the whole workspace, so the
    /// cache must never see a pruned program.
    pub(super) fn restore(self, programs: &mut [Program]) {
        for (index, removed) in self.entries {
            let program = &mut programs[index];
            for (position, function) in removed {
                program.functions.insert(position, function);
            }
        }
    }
}

/// Every ASCII identifier token in `bytes`. Non-ASCII bytes cannot start or
/// continue an identifier, so a region boundary that splits a multi-byte
/// character can only end a token early, never invent one.
fn ascii_identifiers(bytes: &[u8], out: &mut BTreeSet<String>) {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            out.insert(String::from_utf8_lossy(&bytes[start..index]).into_owned());
        } else {
            index += 1;
        }
    }
}

/// The source region owned by each function, in declaration order: from the
/// end of the nearest preceding declaration of any kind (so leading doc
/// comments and the `@id` attribute belong to the function they introduce,
/// not to the module) through the end of the function's own span. Returns
/// `None` when any span does not address `len` bytes, in which case the
/// caller retains the whole module.
fn function_regions(program: &Program, len: usize) -> Option<Vec<(usize, usize)>> {
    let mut boundaries: Vec<usize> = Vec::new();
    boundaries.extend(program.module_uses.iter().map(|item| item.span.end));
    boundaries.extend(program.types.iter().map(|item| item.span.end));
    boundaries.extend(program.interfaces.iter().map(|item| item.span.end));
    boundaries.extend(program.protocols.iter().map(|item| item.span.end));
    boundaries.extend(program.implementations.iter().map(|item| item.span.end));
    boundaries.extend(program.session_protocols.iter().map(|item| item.span.end));
    boundaries.extend(program.agents.iter().map(|item| item.span.end));
    boundaries.extend(program.functions.iter().map(|item| item.span.end));
    boundaries.retain(|end| *end <= len);
    boundaries.sort_unstable();
    let mut regions = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        let (start, end) = (function.span.start, function.span.end);
        if end > len || start >= end {
            return None;
        }
        let region_start = boundaries
            .iter()
            .rev()
            .find(|boundary| **boundary <= start)
            .copied()
            .unwrap_or(0);
        regions.push((region_start, end));
    }
    Some(regions)
}

/// Remove from every bundled-dependency module the functions no retained
/// declaration anywhere in the workspace can name. Deterministic: the result
/// is a pure function of the workspace's sorted source bytes, and every
/// intermediate set is a `BTreeSet` or a Vec walked in declaration order, so
/// it cannot depend on which module happened to be resolved first.
pub(super) fn prune(
    programs: &mut [Program],
    sources: &[WorkspaceSource],
) -> PrunedDependencyFunctions {
    if programs.len() != sources.len() {
        return PrunedDependencyFunctions::default();
    }
    // A declaration named by any `use` is part of the workspace's authorized
    // surface and is always retained, whatever kind of `use` names it.
    let externally_named: BTreeSet<String> = programs
        .iter()
        .flat_map(|program| program.module_uses.iter())
        .map(|module_use| module_use.persistent_id.clone())
        .collect();
    let mut entries = Vec::new();
    for index in 0..programs.len() {
        if !sources[index].path.starts_with("dependencies/") {
            continue;
        }
        let retained = {
            let program = &programs[index];
            if program.path != sources[index].path || program.functions.is_empty() {
                continue;
            }
            let bytes = sources[index].source.as_bytes();
            let Some(regions) = function_regions(program, bytes.len()) else {
                continue;
            };
            let mut masked = bytes.to_vec();
            for (start, end) in &regions {
                for byte in &mut masked[*start..*end] {
                    *byte = b' ';
                }
            }
            let mut outside = BTreeSet::new();
            ascii_identifiers(&masked, &mut outside);
            drop(masked);
            let mentions: Vec<BTreeSet<String>> = regions
                .iter()
                .map(|(start, end)| {
                    let mut set = BTreeSet::new();
                    ascii_identifiers(&bytes[*start..*end], &mut set);
                    set
                })
                .collect();
            let names: Vec<&str> = program
                .functions
                .iter()
                .map(|function| function.name.as_str())
                .collect();
            // A session protocol names its realizing functions by persistent
            // id (`via "<id>"`), not by display name, so the identifier scan
            // alone could miss one; every `via` target is a root.
            let via_targets: BTreeSet<&str> = program
                .session_protocols
                .iter()
                .flat_map(|protocol| &protocol.transitions)
                .filter_map(|transition| transition.via.as_ref())
                .map(|via| via.name.as_str())
                .collect();
            let mut retained = vec![false; program.functions.len()];
            let mut pending = Vec::new();
            for (position, function) in program.functions.iter().enumerate() {
                // `main` is an entry point the graph synthesizes when absent,
                // so removing a real one would change module identity rather
                // than only skip work.
                if function.name == "main"
                    || externally_named.contains(function.stable_id.as_str())
                    || via_targets.contains(function.stable_id.as_str())
                    || outside.contains(function.name.as_str())
                {
                    retained[position] = true;
                    pending.push(position);
                }
            }
            while let Some(position) = pending.pop() {
                for (candidate, name) in names.iter().enumerate() {
                    if !retained[candidate] && mentions[position].contains(*name) {
                        retained[candidate] = true;
                        pending.push(candidate);
                    }
                }
            }
            retained
        };
        if retained.iter().all(|flag| *flag) {
            continue;
        }
        let program = &mut programs[index];
        let mut removed = Vec::new();
        for position in (0..program.functions.len()).rev() {
            if !retained[position] {
                removed.push((position, program.functions.remove(position)));
            }
        }
        removed.reverse();
        entries.push((index, removed));
    }
    PrunedDependencyFunctions { entries }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_graph::{build_owned, WorkspaceSource};
    use std::path::Path;

    /// `build_owned` requires canonical source text, and these fixtures are
    /// hand written, so route each through one parse/format round trip.
    fn source(path: &str, text: &str) -> WorkspaceSource {
        WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&crate::parse(text, Path::new(path)).unwrap()),
        }
    }

    const APP_WITHOUT_CALL: &str = concat!(
        "module app;\n",
        "use function @id(\"dep.used\") from dep as used;\n",
        "@id(\"app.main\") fn main() -> i64 { used(1) }\n",
    );
    const APP_WITH_CALL: &str = concat!(
        "module app;\n",
        "use function @id(\"dep.used\") from dep as used;\n",
        "use function @id(\"dep.broken\") from dep as broken;\n",
        "@id(\"app.main\") fn main() -> i64 { used(1) + broken(2) }\n",
    );
    /// `dep.broken` does not type check: it declares `i64` and returns the
    /// `bool` its comparison produces. Nothing in this module names it.
    const DEP: &str = concat!(
        "module dep;\n",
        "@id(\"dep.helper\") fn helper(input: i64) -> i64 { input + 1 }\n",
        "@id(\"dep.used\") fn used(input: i64) -> i64 { helper(input) }\n",
        "@id(\"dep.broken\") fn broken(input: i64) -> i64 { input > 0 }\n",
    );

    /// The correctness bound on skipping work: a bundled dependency function
    /// that no call site reaches is not built into HIR and so is not checked,
    /// but the moment any module reaches it, it is resolved in full and its
    /// error is reported. A lazy resolver that silently never checked a
    /// reached declaration would be a soundness bug far worse than the
    /// capacity ceiling this pruning exists to relieve.
    #[test]
    fn reaching_a_dependency_function_later_still_type_checks_it() {
        let unreached = vec![
            source("src/app.spx", APP_WITHOUT_CALL),
            source("dependencies/dep/0.1.0/dep.spx", DEP),
        ];
        build_owned(unreached).expect(
            "an unreached bundled dependency declaration is never built into HIR, so its \
             body is never checked",
        );

        let reached = vec![
            source("src/app.spx", APP_WITH_CALL),
            source("dependencies/dep/0.1.0/dep.spx", DEP),
        ];
        let errors = build_owned(reached)
            .err()
            .expect("reaching the same declaration must check it and report its error");
        assert!(
            errors.iter().any(|error| error.code.starts_with("SPX-")),
            "{errors:?}"
        );
    }

    /// Only bundled dependency modules are pruned. The identical unreached,
    /// ill-typed declaration in a project's own source is still resolved and
    /// still refused, so nothing a reviewer can read in the project goes
    /// unchecked.
    #[test]
    fn a_project_module_is_never_pruned() {
        let sources = vec![
            source("src/app.spx", APP_WITHOUT_CALL),
            source("src/dep.spx", DEP),
        ];
        let errors = build_owned(sources)
            .err()
            .expect("project source is always fully resolved");
        assert!(
            errors.iter().any(|error| error.code.starts_with("SPX-")),
            "{errors:?}"
        );
    }

    /// The retained set is a pure function of the workspace source bytes, in
    /// declaration order, so two builds of the same input agree exactly.
    #[test]
    fn pruning_is_deterministic_and_reaches_through_internal_calls() {
        let sources = vec![
            source("src/app.spx", APP_WITHOUT_CALL),
            source("dependencies/dep/0.1.0/dep.spx", DEP),
        ];
        let first = build_owned(sources.clone()).unwrap();
        let second = build_owned(sources.clone()).unwrap();
        assert_eq!(first.usage.builder_bytes, second.usage.builder_bytes);
        assert_eq!(first.edges, second.edges);
        assert_eq!(first.hir.declarations, second.hir.declarations);
        // `dep.helper` is named only by `dep.used`'s body, and `dep.used` is
        // imported, so the internal-call closure must retain it.
        assert!(first.hir.declarations.contains_key("dep.helper"));
        assert!(first.hir.declarations.contains_key("dep.used"));
        assert!(!first.hir.declarations.contains_key("dep.broken"));
    }

    /// Fault-injection guard for the scan itself: a declaration named only
    /// from a retained function's body is a mention, and the scan must find
    /// it. Prunes the two-function module down to exactly the closure.
    #[test]
    fn only_unreached_dependency_functions_are_withheld() {
        let mut programs = vec![
            crate::parse(
                &crate::format::canonical(
                    &crate::parse(APP_WITHOUT_CALL, Path::new("src/app.spx")).unwrap(),
                ),
                Path::new("src/app.spx"),
            )
            .unwrap(),
            crate::parse(
                &crate::format::canonical(
                    &crate::parse(DEP, Path::new("dependencies/dep/0.1.0/dep.spx")).unwrap(),
                ),
                Path::new("dependencies/dep/0.1.0/dep.spx"),
            )
            .unwrap(),
        ];
        let sources = vec![
            source("src/app.spx", APP_WITHOUT_CALL),
            source("dependencies/dep/0.1.0/dep.spx", DEP),
        ];
        let pruned = prune(&mut programs, &sources);
        assert_eq!(pruned.count(), 1);
        assert_eq!(programs[0].functions.len(), 1, "project module untouched");
        let retained: Vec<&str> = programs[1]
            .functions
            .iter()
            .map(|function| function.stable_id.as_str())
            .collect();
        assert_eq!(retained, vec!["dep.helper", "dep.used"]);
        pruned.restore(&mut programs);
        let restored: Vec<&str> = programs[1]
            .functions
            .iter()
            .map(|function| function.stable_id.as_str())
            .collect();
        assert_eq!(restored, vec!["dep.helper", "dep.used", "dep.broken"]);
    }

    /// A session protocol's `via` names its realizer by persistent id, whose
    /// dotted segments need not spell the function's display name. The
    /// realizer must still be retained, or the protocol's HIR binding would
    /// refuse a function the pruner removed.
    #[test]
    fn a_session_protocol_via_target_is_retained() {
        const PROTOCOL_DEP: &str = concat!(
            "module dep;\n",
            "@id(\"dep.used\") fn used(input: i64) -> i64 { input }\n",
            "@id(\"dep.opaque.target\") fn realize() -> i64 { 1 }\n",
            "@id(\"dep.unused\") fn unused() -> i64 { 2 }\n",
            "@id(\"dep.protocol\")\n",
            "session protocol \"dep-order-v1\" {\n",
            "    states { Ready, Done }\n",
            "    initial Ready;\n",
            "    terminal Done cleanup { release }\n",
            "    on Ready go: send Unit via \"dep.opaque.target\" -> Done;\n",
            "    on Ready abort: fail Unit -> Done;\n",
            "}\n",
        );
        let path = "dependencies/dep/0.1.0/dep.spx";
        let mut programs = vec![
            crate::parse(
                &crate::format::canonical(
                    &crate::parse(APP_WITHOUT_CALL, Path::new("src/app.spx")).unwrap(),
                ),
                Path::new("src/app.spx"),
            )
            .unwrap(),
            crate::parse(
                &crate::format::canonical(&crate::parse(PROTOCOL_DEP, Path::new(path)).unwrap()),
                Path::new(path),
            )
            .unwrap(),
        ];
        let sources = vec![
            source("src/app.spx", APP_WITHOUT_CALL),
            source(path, PROTOCOL_DEP),
        ];
        let pruned = prune(&mut programs, &sources);
        let retained: Vec<&str> = programs[1]
            .functions
            .iter()
            .map(|function| function.stable_id.as_str())
            .collect();
        assert_eq!(retained, vec!["dep.used", "dep.opaque.target"]);
        assert_eq!(pruned.count(), 1);
    }
}
