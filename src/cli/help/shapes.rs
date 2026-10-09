use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::SHAPES_INDEX;

pub(super) fn kind_index() -> String {
    let catalog: serde_json::Value =
        serde_json::from_str(SHAPES_INDEX).expect("generated language-shapes JSON must parse");
    let entries = catalog["entries"]
        .as_array()
        .expect("generated language-shapes JSON must contain entries");
    let kinds: BTreeSet<_> = entries
        .iter()
        .map(|entry| {
            entry["kind"]
                .as_str()
                .expect("generated shape must have a kind")
        })
        .collect();

    let mut output = String::new();
    writeln!(output, "Language shape kinds ({}):", kinds.len())
        .expect("writing to a string cannot fail");
    for kind in kinds {
        writeln!(output, "  {kind}").expect("writing to a string cannot fail");
    }
    writeln!(output, "Exact exemplar: semaprax help shapes <kind>")
        .expect("writing to a string cannot fail");
    writeln!(output, "Full catalog: semaprax help shapes")
        .expect("writing to a string cannot fail");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND_INDEX_MAX_BYTES: usize = 2_048;
    const KIND_INDEX_MAX_UNITS: usize = 256;

    #[test]
    fn exact_kind_index_is_complete_collision_free_and_bounded() {
        let catalog: serde_json::Value = serde_json::from_str(SHAPES_INDEX).unwrap();
        let entries = catalog["entries"].as_array().unwrap();
        let expected: BTreeSet<_> = entries
            .iter()
            .map(|entry| entry["kind"].as_str().unwrap())
            .collect();
        let identities: BTreeSet<_> = entries
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect();
        assert!(!expected.contains("kinds"));
        assert!(!identities.contains("kinds"));

        let index = kind_index();
        assert_eq!(index, kind_index());
        assert!(index.starts_with(&format!(
            "Language shape kinds ({}):\n",
            expected.len()
        )));
        let listed: Vec<_> = index
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .collect();
        assert_eq!(listed, expected.iter().copied().collect::<Vec<_>>());
        assert!(index.ends_with(
            "Exact exemplar: semaprax help shapes <kind>\nFull catalog: semaprax help shapes\n"
        ));
        assert!(index.len() <= KIND_INDEX_MAX_BYTES, "{} bytes", index.len());
        let units = semaprax::agent_economics::lexical_tokens(&index);
        assert!(units <= KIND_INDEX_MAX_UNITS, "{units} lexical units");
    }
}
