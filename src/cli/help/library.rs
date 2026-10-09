use std::fmt::Write as _;

use super::{LIBRARY_CATALOG, LIBRARY_INDEX};

pub(super) fn library_help(query: Option<&str>) -> Result<String, String> {
    match query {
        None => Ok(library_index()),
        Some("all") => Ok(LIBRARY_CATALOG.to_owned()),
        Some(query) => library_entry(query),
    }
}

pub(super) fn library_index() -> String {
    let catalog: serde_json::Value =
        serde_json::from_str(LIBRARY_INDEX).expect("generated standard-library JSON must parse");
    let modules = catalog["modules"]
        .as_array()
        .expect("generated standard-library JSON must contain modules");
    let mut output = String::new();
    writeln!(output, "Standard library modules ({}):", modules.len())
        .expect("writing to a string cannot fail");
    for module in modules {
        let id = module["module"]
            .as_str()
            .expect("generated standard-library module must have an identity");
        writeln!(output, "  {id}").expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "\nExact lookup: semaprax help library <module|name|stable-id>"
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "Full catalog: semaprax help library all")
        .expect("writing to a string cannot fail");
    output
}

pub(super) fn library_entry(query: &str) -> Result<String, String> {
    let catalog: serde_json::Value =
        serde_json::from_str(LIBRARY_INDEX).expect("generated standard-library JSON must parse");
    let modules = catalog["modules"]
        .as_array()
        .expect("generated standard-library JSON must contain modules");
    let mut output = String::new();
    for module in modules {
        let module_id = module["module"]
            .as_str()
            .expect("generated standard-library module must have an identity");
        let whole_module = query == module_id;
        for declaration in module["declarations"]
            .as_array()
            .expect("generated standard-library module must contain declarations")
        {
            let id = declaration["id"]
                .as_str()
                .expect("generated standard-library declaration must have an identity");
            let name = declaration["name"]
                .as_str()
                .expect("generated standard-library declaration must have a name");
            if !whole_module && query != id && query != name {
                continue;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            writeln!(output, "{id}").expect("writing to a string cannot fail");
            writeln!(
                output,
                "dependency {}",
                module["dependency"]
                    .as_str()
                    .expect("generated standard-library dependency must be text")
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "profile {}",
                module["required_profile"]
                    .as_str()
                    .expect("generated standard-library profile must be text")
            )
            .expect("writing to a string cannot fail");
            for line in declaration["head"]
                .as_array()
                .expect("generated standard-library declaration must have a head")
            {
                writeln!(
                    output,
                    "{}",
                    line.as_str()
                        .expect("generated standard-library head line must be text")
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
    if output.is_empty() {
        Err(format!("standard library has no exact match for `{query}`"))
    } else {
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_a_complete_module_index_and_all_selects_the_full_catalog() {
        let index = library_help(None).unwrap();
        assert!(index.starts_with("Standard library modules (51):\n"));
        assert!(index.contains("\n  std.int.decimal\n"));
        assert!(index.contains("\n  std.data.json.scan\n"));
        assert!(index.contains("semaprax help library <module|name|stable-id>"));
        assert!(index.contains("semaprax help library all"));
        assert!(index.len() <= 2_048);
        assert!(semaprax::agent_economics::lexical_tokens(&index) <= 256);
        assert_eq!(
            index
                .lines()
                .filter(|line| line.starts_with("  std."))
                .count(),
            51
        );
        assert_eq!(library_help(Some("all")).unwrap(), LIBRARY_CATALOG);
    }
}
