#[test]
fn complete_old_functions_are_compiled_and_only_capacity_hooks_change() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/minimum-path-array-v1/functions.json"
    )))
    .unwrap();
    let root = crate::source_reflows_merge::maina3_input(
        "src/cleanup_plan/replay.rs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay.rs"
        )),
    );
    let compiled = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/path_array/main8a/before.rs"
    ));
    for record in records.as_array().unwrap() {
        let name = record["name"].as_str().unwrap();
        let body = record["body"].as_str().unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
            ),
            record["sha256"]
        );
        assert!(compiled.contains(&body.replacen(
            &format!("fn {name}("),
            &format!("pub(super) fn {name}("),
            1
        )));
        let mut current = body.to_owned();
        match name {
            "sequence_skeleton_paths" => current = current.replacen("    let mut combined = Vec::new();\n", "    let mut combined = Vec::new();\n    if !suffixes.is_empty() {\n        path_array::minimum(\n            &mut combined,\n            prefixes.len(),\n            work,\n            \"sequenced path capacity\",\n        )?;\n    }\n", 1),
            "append_expr_paths" => current = current.replacen("    for path in paths {", "    path_array::minimum(target, paths.len(), work, phase)?;\n    for path in paths {", 1),
            "split_status_paths" => current = current.replacen("    let mut results = Vec::new();\n", "    let mut results = Vec::new();\n    path_array::minimum(&mut results, paths.len(), work, \"status path capacity\")?;\n", 1),
            "split_contract" => current = current.replacen("    let mut results = Vec::new();\n", "    let mut results = Vec::new();\n    path_array::minimum(&mut results, paths.len(), work, \"contract path capacity\")?;\n", 1),
            _ => panic!("unregistered receiver"),
        }
        if name == "sequence_skeleton_paths" {
            // Compose the later independently frozen prefix-transfer receiver;
            // keep this complete predecessor and every original assertion.
            let intermediate = crate::source_reflows_merge::maina3_input(
                "src/cleanup_plan/replay/sequence_prefix/before.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/sequence_prefix/before.rs"
                )),
            );
            assert!(intermediate.contains(&current.replacen(
                "fn sequence_skeleton_paths(",
                "pub(super) fn sequence_skeleton_paths(",
                1
            )));
            current = current.replacen(
                "            let observations =\n                work.clone_observations(&prefix.observations, \"skeleton prefix clone\")?;",
                "            let observations = match sequence_prefix::try_take(\n                &mut prefix.observations,\n                &suffix.observations,\n                suffixes.len(),\n                work.function,\n                work.budget,\n                \"skeleton prefix clone\",\n            )? {\n                Some(observations) => observations,\n                None => work.clone_observations(&prefix.observations, \"skeleton prefix clone\")?,\n            };", 1
            );
        }
        if name == "sequence_skeleton_paths" {
            // Compose the pure-probe follow-up without changing either full
            // predecessor, the earlier inverse, or the final exact match.
            current = current.replacen("            let observations = match sequence_prefix::try_take(\n                &mut prefix.observations,\n                &suffix.observations,\n                suffixes.len(),\n                work.function,\n                work.budget,\n                \"skeleton prefix clone\",\n            )? {\n                Some(observations) => observations,\n                None => work.clone_observations(&prefix.observations, \"skeleton prefix clone\")?,\n            };","            let observations = if sequence_prefix::can_take(sequence_prefix::Candidate {\n                prefix: &prefix.observations,\n                suffix: &suffix.observations,\n                suffix_count: suffixes.len(),\n                budget: work.budget,\n            }) {\n                // Retain the original clone's work and materialization, while\n                // funding the pure probe before transferring any owned field.\n                work.charge(1, \"skeleton prefix clone\")?;\n                work.budget.reserve_array::<u8>(\n                    work.function,\n                    sequence_prefix::RECEIVING_BYTES,\n                    \"skeleton prefix clone\",\n                )?;\n                note_skeleton_materialization();\n                std::mem::take(&mut prefix.observations)\n            } else {\n                work.clone_observations(&prefix.observations, \"skeleton prefix clone\")?\n            };", 1);
        }
        if name == "sequence_skeleton_paths" {
            current = current.replacen("        for suffix in suffixes.iter() {", "        // This is the original for-loop's Iter/Option state. Earlier\n        // successors borrow the prefix; the last one can take it intact.\n        let mut remaining_suffixes = suffixes.iter();\n        while let Some(suffix) = remaining_suffixes.next() {", 1);
            current = current.replacen(
                "                suffix_count: suffixes.len(),",
                "                remaining_suffixes: remaining_suffixes.len(),",
                1,
            );
        }
        if name == "sequence_skeleton_paths" {
            current = current.replacen(
                "    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n",
                "    let share_prefix = work.shares_prefix_tail(&suffixes)?;\n    if sequence_vector::can_reuse(sequence_vector::Candidate {\n        prefixes: &prefixes,\n        suffixes: &suffixes,\n        budget: work.budget,\n    }) {\n        work.budget.reserve_array::<u8>(\n            work.function,\n            sequence_vector::receiving_bytes(),\n            \"single-successor receiving frame\",\n        )?;\n        return sequence_vector::single(prefixes, &suffixes, work, share_prefix);\n    }\n",
                1,
            );
        }
        assert_eq!(root.matches(&current).count(), 1, "{name}");
    }
}
