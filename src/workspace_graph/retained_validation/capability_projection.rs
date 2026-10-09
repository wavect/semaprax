//! Reconstruct permit authority without copying immutable retained identities.
use super::edge_projection::push_edge_reference;
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{
    graph_error, limit_error, reserve_builder_structure, WorkspaceEdge, WorkspaceResolvedModule,
    MAX_CROSS_FILE_EDGES,
};

// This order is the complete derived Ord/Eq field order of WorkspaceEdge.
type EdgeIdentity<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    usize,
);

struct PermitAuthority<'a> {
    module_path: &'a str,
    module: &'a str,
    permit: &'a str,
    path: String,
    ordinal: usize,
}

impl PermitAuthority<'_> {
    fn identity(&self) -> EdgeIdentity<'_> {
        (
            self.module_path,
            self.module,
            self.module_path,
            self.permit,
            "capability_authority",
            "module",
            &self.path,
            &self.path,
            "",
            self.ordinal,
        )
    }
}

fn edge_identity(edge: &WorkspaceEdge) -> EdgeIdentity<'_> {
    (
        &edge.caller_path,
        &edge.caller,
        &edge.target_path,
        &edge.target,
        edge.kind,
        edge.site,
        &edge.expression,
        &edge.ast_path,
        &edge.alias,
        edge.ordinal,
    )
}

pub(super) fn validate(
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut expected = Vec::new();
    for module in modules {
        for (ordinal, permit) in module.permits.iter().enumerate() {
            if expected.len() == MAX_CROSS_FILE_EDGES {
                return Err(vec![limit_error(
                    "resolved_cross_file_edges",
                    MAX_CROSS_FILE_EDGES,
                )]);
            }
            reserve_builder_structure(std::mem::size_of::<PermitAuthority<'_>>())?;
            let path = crate::bounded_output::budgeted_format(format_args!("permit.{ordinal}"));
            expected.push(PermitAuthority {
                module_path: &module.path,
                module: &module.module,
                permit,
                path,
                ordinal,
            });
        }
    }
    let mut actual = Vec::new();
    for edge in edges
        .iter()
        .filter(|edge| edge.kind == "capability_authority")
    {
        push_edge_reference(&mut actual, edge)?;
    }
    expected.sort_unstable_by(|left, right| left.identity().cmp(&right.identity()));
    actual.sort_unstable();
    if !actual
        .into_iter()
        .map(edge_identity)
        .eq(expected.iter().map(PermitAuthority::identity))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace capability-authority edges disagree with retained module permits",
        )]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_graph::{build_owned, WorkspaceSource};
    use std::path::Path;

    fn fixture() -> (Vec<WorkspaceResolvedModule>, Vec<WorkspaceEdge>) {
        let sources = [
            ("z/app.spx", "module cap.app;\npermit { audit.write, clock.read }\n@id(\"cap.app.main\") fn main() -> i64 { 0 }\n"),
            ("a/provider.spx", "module cap.provider;\npermit { audit.write }\n@id(\"cap.provider.main\") fn main() -> i64 { 0 }\n"),
        ].into_iter().map(|(path, source)| {
            let program = crate::parse(source, Path::new(path)).unwrap();
            WorkspaceSource {
                path: path.to_owned(),
                source: crate::format::canonical(&program),
            }
        }).collect();
        let built = build_owned(sources).unwrap();
        (built.hir.modules, built.edges)
    }

    // Independent original owned-edge reconstruction checks every field and
    // occurrence, rather than sharing the production projection's identity.
    fn owned_reference(modules: &[WorkspaceResolvedModule], edges: &[WorkspaceEdge]) -> bool {
        let mut expected = Vec::new();
        for module in modules {
            for (ordinal, permit) in module.permits.iter().enumerate() {
                let path = format!("permit.{ordinal}");
                expected.push(WorkspaceEdge {
                    caller_path: module.path.clone(),
                    caller: module.module.clone(),
                    target_path: module.path.clone(),
                    target: permit.clone(),
                    kind: "capability_authority",
                    site: "module",
                    expression: path.clone(),
                    ast_path: path,
                    alias: String::new(),
                    ordinal,
                });
            }
        }
        let mut actual = edges
            .iter()
            .filter(|edge| edge.kind == "capability_authority")
            .cloned()
            .collect::<Vec<_>>();
        expected.sort();
        actual.sort();
        expected == actual
    }

    #[test]
    fn capability_projection_matches_full_owned_multisets_and_rejects_forgery() {
        let (modules, edges) = fixture();
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == "capability_authority")
                .count(),
            3
        );
        for field in [
            "valid",
            "reordered",
            "caller_path",
            "caller",
            "target_path",
            "target",
            "kind",
            "site",
            "expression",
            "ast_path",
            "alias",
            "ordinal",
            "missing",
            "duplicate",
        ] {
            let mut candidate = edges.clone();
            let index = candidate
                .iter()
                .position(|edge| edge.kind == "capability_authority")
                .unwrap();
            let edge = &mut candidate[index];
            match field {
                "valid" => {}
                "reordered" => candidate.reverse(),
                "caller_path" => edge.caller_path.push_str(".forged"),
                "caller" => edge.caller.push_str(".forged"),
                "target_path" => edge.target_path.push_str(".forged"),
                "target" => edge.target.push_str(".forged"),
                "kind" => edge.kind = "call",
                "site" => edge.site = "body",
                "expression" => edge.expression.push_str(".forged"),
                "ast_path" => edge.ast_path.push_str(".forged"),
                "alias" => edge.alias.push_str(".forged"),
                "ordinal" => edge.ordinal += 1,
                "missing" => {
                    candidate.remove(index);
                }
                "duplicate" => {
                    let duplicate = edge.clone();
                    candidate.push(duplicate);
                }
                _ => unreachable!(),
            }
            let result = validate(&modules, &candidate);
            assert_eq!(
                result.is_ok(),
                owned_reference(&modules, &candidate),
                "{field}"
            );
            if field == "valid" || field == "reordered" {
                result.unwrap();
            } else {
                let errors = result.expect_err(field);
                assert_eq!(errors[0].code, "SPX-G173", "{field}");
                assert_eq!(
                    errors[0].message,
                    "workspace capability-authority edges disagree with retained module permits"
                );
            }
        }
        let mut no_authority = modules;
        for module in &mut no_authority {
            module.permits.clear();
        }
        validate(&no_authority, &[]).unwrap();
        let errors =
            validate(&no_authority, &edges).expect_err("no permits authorize forged edges");
        assert_eq!(errors[0].code, "SPX-G173");
    }

    #[test]
    fn capability_projection_charges_carriers_and_generated_paths_at_the_exact_limit() {
        let (modules, edges) = fixture();
        let expected_bytes = modules
            .iter()
            .map(|module| module.permits.len())
            .sum::<usize>()
            * (std::mem::size_of::<PermitAuthority<'_>>() + std::mem::size_of::<&WorkspaceEdge>())
            + "permit.0".len() * 3;
        let (result, overflow, used) =
            crate::bounded_output::with_limit_usage(expected_bytes, || validate(&modules, &edges));
        result.unwrap();
        assert!(!overflow);
        assert_eq!(used, expected_bytes);
        let (result, overflow, _) =
            crate::bounded_output::with_limit_usage(expected_bytes - 1, || {
                validate(&modules, &edges)
            });
        assert!(overflow);
        assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
        let (result, overflow, used) =
            crate::bounded_output::with_limit_usage(0, || validate(&[], &[]));
        result.unwrap();
        assert!(!overflow);
        assert_eq!(used, 0);
    }
}
