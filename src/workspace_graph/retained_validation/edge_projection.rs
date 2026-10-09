//! Borrow immutable edge bytes for exact transient validation projections.
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{
    limit_error, reserve_builder_structure, WorkspaceEdge, MAX_CROSS_FILE_EDGES,
};

pub(super) fn push_edge_reference<'a>(
    edges: &mut Vec<&'a WorkspaceEdge>,
    edge: &'a WorkspaceEdge,
) -> Result<(), Vec<Diagnostic>> {
    if edges.len() == MAX_CROSS_FILE_EDGES {
        return Err(vec![limit_error(
            "resolved_cross_file_edges",
            MAX_CROSS_FILE_EDGES,
        )]);
    }
    reserve_builder_structure(std::mem::size_of::<&WorkspaceEdge>())?;
    edges.push(edge);
    Ok(())
}

pub(super) fn type_site(
    (caller, expression, path, target): &(String, String, String, String),
) -> (&str, &str, &str, &str) {
    (
        caller.as_str(),
        expression.as_str(),
        path.as_str(),
        target.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use crate::workspace_graph::{build_owned, retained_validation, WorkspaceSource};
    use std::path::Path;

    #[test]
    fn borrowed_call_projection_checks_the_complete_occurrence_multiset() {
        let sources = vec![
            WorkspaceSource {
                path: "app/main.spx".to_owned(),
                source: "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    answer() + answer()\n}\n".to_owned(),
            },
            WorkspaceSource {
                path: "lib/core.spx".to_owned(),
                source: "module lib.core;\n\n@id(\"lib.answer\")\nfn answer() -> i64\n{\n    42\n}\n".to_owned(),
            },
        ];
        let programs = sources
            .iter()
            .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
            .collect::<Vec<_>>();
        let built = build_owned(sources).unwrap();
        let validate = |edges: &[crate::workspace_graph::WorkspaceEdge]| {
            retained_validation::validate_retained_facts(&programs, &built.hir.modules, edges)
        };
        validate(&built.edges).unwrap();
        assert_eq!(
            built.edges.iter().filter(|edge| edge.kind == "call").count(),
            2
        );
        for field in [
            "caller_path", "caller", "target_path", "target", "site", "expression", "ast_path",
            "alias", "ordinal", "duplicate",
        ] {
            let mut edges = built.edges.clone();
            let index = edges.iter().position(|edge| edge.kind == "call").unwrap();
            let edge = &mut edges[index];
            match field {
                "caller_path" => edge.caller_path.push_str(".forged"),
                "caller" => edge.caller.push_str(".forged"),
                "target_path" => edge.target_path.push_str(".forged"),
                "target" => edge.target.push_str(".forged"),
                "site" => edge.site = "requires",
                "expression" => edge.expression.push_str(".forged"),
                "ast_path" => edge.ast_path.push_str(".forged"),
                "alias" => edge.alias.push_str(".forged"),
                "ordinal" => edge.ordinal += 1,
                "duplicate" => {
                    let duplicate = edge.clone();
                    edges.push(duplicate);
                }
                _ => unreachable!(),
            }
            let errors = validate(&edges).expect_err(field);
            assert_eq!(errors[0].code, "SPX-G173", "{field}");
        }
    }
}
