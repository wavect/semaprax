//! Versioned graph projection of the explicitly selected byte partition profile.
use super::*;

pub(super) const SCHEMA: &str = "semaprax.graph.v86";

pub(super) fn append_contract(output: &mut crate::bounded_output::CappedString) {
    use crate::construction_fusion::partition::{operation, ID};
    let [width_zero, extent_overflow, extent_mismatch, metadata_capacity] = operation::STATUS_CODES;
    write!(
        output,
        ",\"byte_partition\":{{\"operation\":{},\"status_domain\":{},\"status_codes\":{{\"width_zero\":{width_zero},\"extent_overflow\":{extent_overflow},\"extent_mismatch\":{extent_mismatch},\"metadata_capacity\":{metadata_capacity}}},\"max_backing_events\":{},\"max_payload_bytes\":{},\"max_vector_capacity\":{},\"refund_on_drop\":false}}",
        quote_json(ID),
        quote_json(operation::STATUS_DOMAIN),
        crate::byte_data_capacity::MAX_BYTES_COPY_SITES,
        crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES,
        crate::vec_ops::MAX_CAPACITY,
    )
    .expect("writing to a string cannot fail");
}

/// Render the complete graph of the explicitly selected byte partition source.
/// The caller's graph-work counter funds the current partition proof and the
/// output-byte bound limits the complete projection.
pub fn to_partition_json(
    program: &Program,
    work: &mut usize,
    max_bytes: usize,
) -> Result<String, Diagnostic> {
    let result = (|| {
        let owner = hir::byte_partition::ValidatedPartitionProgram::compile_source(program, work)?;
        let resolved = owner.program();
        let (rendered, overflowed) = crate::bounded_output::with_limit(max_bytes, || {
            let selected_functions = resolved.functions.iter().map(|f| f.id.clone()).collect();
            let selected_types = resolved.types.iter().map(|t| t.id.clone()).collect();
            render_graph_json(
                resolved,
                &revision(program),
                &selected_functions,
                &selected_types,
                &GraphView::Module { static_text: false },
                GraphProjection::Partition,
            )
        });
        if overflowed {
            return Err(Diagnostic::io(
                "SPX-G171",
                "partition graph exceeds its builder budget",
            ));
        }
        rendered
    })();
    if result.is_err() {
        *work = 0;
    }
    result
}

/// Verify exact graph bytes by regenerating the selected graph from held source.
/// Graph JSON never supplies executable ownership or backend authority.
pub fn verify_partition_json(
    program: &Program,
    submitted: &str,
    work: &mut usize,
    max_bytes: usize,
) -> Result<(), Diagnostic> {
    if submitted.len() <= max_bytes && to_partition_json(program, work, max_bytes)? == submitted {
        Ok(())
    } else {
        *work = 0;
        Err(Diagnostic::io(
            "SPX-G411",
            "partition graph differs from exact retained source replay",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../../tests/fixtures/byte-partition-private-iterator.spx");
    const LIMIT: usize = 8_388_608;

    #[test]
    fn partition_graph_binds_exact_operands_cleanup_and_current_source() {
        let source = crate::parse(SOURCE, "partition-graph.spx").unwrap();
        let mut work = usize::MAX;
        let graph = to_partition_json(&source, &mut work, LIMIT).unwrap();
        let used = usize::MAX - work;
        let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(document["schema"], SCHEMA);
        assert_eq!(document["revision"], revision(&source));
        assert_eq!(
            document["byte_partition"]["operation"],
            "core.bytes.partition"
        );
        assert_eq!(document["byte_partition"]["max_backing_events"], 32);
        assert_eq!(document["byte_partition"]["max_payload_bytes"], 2_097_152);
        assert_eq!(
            document["byte_partition"]["status_codes"]["extent_mismatch"],
            3
        );
        let nodes = document["nodes"].as_array().unwrap();
        let entry = nodes
            .iter()
            .find(|node| node["id"] == "partition_iterator.main")
            .unwrap();
        assert!(nodes
            .iter()
            .any(|node| node["id"] == crate::prelude::VEC_ID));
        assert!(entry["cleanup"].is_object() && entry["loans"].is_object());
        assert!(entry["body"]
            .to_string()
            .contains("\"callee\":\"core.bytes.partition\""));

        let mut exact = used;
        verify_partition_json(&source, &graph, &mut exact, LIMIT).unwrap();
        assert_eq!(exact, 0);
        let mut short = used - 1;
        assert!(to_partition_json(&source, &mut short, LIMIT).is_err());
        assert_eq!(short, 0);
        let mut limited = usize::MAX;
        assert_eq!(
            to_partition_json(&source, &mut limited, graph.len() - 1)
                .unwrap_err()
                .code,
            "SPX-G171"
        );
        assert_eq!(limited, 0);
        for forged in [
            graph.replace(SCHEMA, "semaprax.graph.v85"),
            graph.replace("\"max_backing_events\":32", "\"max_backing_events\":33"),
            graph.replacen("\"value\":\"4096\"", "\"value\":\"4095\"", 1),
            format!("{graph}\n"),
        ] {
            assert_ne!(forged, graph);
            let mut work = usize::MAX;
            assert_eq!(
                verify_partition_json(&source, &forged, &mut work, LIMIT)
                    .unwrap_err()
                    .code,
                "SPX-G411"
            );
            assert_eq!(work, 0);
        }
        let changed = crate::parse(
            &SOURCE.replace("4096usize", "4095usize"),
            "partition-graph.spx",
        )
        .unwrap();
        assert!(verify_partition_json(&changed, &graph, &mut { usize::MAX }, LIMIT).is_err());
        assert!(super::super::to_json(&source).is_err());
        assert!(super::super::verify_json(&source, &graph).is_err());
        assert!(super::super::to_legacy_json(&source).is_err());
        assert_eq!(
            super::super::reject_evidence_schema(SCHEMA)
                .unwrap_err()
                .code,
            "SPX-G410"
        );
    }
}
