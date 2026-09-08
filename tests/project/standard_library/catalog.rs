use super::*;

pub(super) fn render_catalogs() -> (String, String) {
    let mut human = String::new();
    human.push_str("# Standard library catalog\n\n");
    human.push_str(
        "Status: generated from `std/` through the `semaprax doc` documentation model by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.\n\n",
    );
    human.push_str("Audience: agents and humans choosing a standard-library declaration.\n\n");
    human.push_str(
        "Every declaration below is verified, canonical, and executed by its package's conformance module on the interpreter, native C11, and Core Wasm lanes. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the contract; `std/catalog.json` is the same catalog for tools.\n\nConsume a package from an installed compiler by adding its dependency line to the extensible manifest, then importing the selected stable identity: `[dependencies] std.num = \"^0.1.0\"` and `use function @id(\"std.num.abs\") from std.num as abs;`. Set `[package] profile` to the package's required profile below; `scalar` means omit the profile key. The compiler supplies the closed bundled package without a source checkout, cache, or network access.\n",
    );
    let mut modules = Vec::new();
    for package in packages() {
        let (library, _, _) = package_sources(&package);
        let profile = required_consumer_profile(&package);
        human.push_str(&format!(
            "\n## `{}`\n\nPackage `std/{}`, tier `{}`, status {}. Required project profile: `{profile}`. Dependency: `{} = \"^0.1.0\"`. Targets: {}.\n",
            package.module,
            package.directory,
            package.tier,
            package.status,
            package.module,
            package
                .targets
                .iter()
                .map(|target| format!("`{target}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        // The catalog is a projection of the same documentation model that
        // `semaprax doc` renders, so the bundled skill cannot drift from the
        // graph; the source-text slice below cross-checks every signature.
        let (program, comments) =
            semaprax::parse_with_comments(&library.source, &library.path).unwrap();
        let document = semaprax::doc::document(&program, &comments);
        let mut declarations = Vec::new();
        for entry in document
            .entries
            .iter()
            .filter(|entry| matches!(entry.kind, "function" | "record" | "variant"))
        {
            let head: Vec<String> = entry
                .signature
                .lines()
                .filter(|line| !line.trim_start().starts_with("@id("))
                .map(str::to_owned)
                .collect();
            if entry.kind == "function" {
                assert_eq!(
                    head,
                    declaration_head(&library.source, &entry.id),
                    "{}: the documentation signature must equal the source text",
                    entry.id
                );
            }
            let function = library
                .program
                .functions
                .iter()
                .find(|function| function.stable_id == entry.id);
            human.push_str(&format!("\n### `{}`\n\n", entry.id));
            for line in &entry.description {
                human.push_str(line);
                human.push('\n');
            }
            if !entry.description.is_empty() {
                human.push('\n');
            }
            human.push_str(&format!("```semaprax\n{}\n```\n", head.join("\n")));
            declarations.push(serde_json::json!({
                "id": entry.id,
                "kind": entry.kind,
                "name": entry.name,
                "description": entry.description,
                "head": head,
                "effects": function.map(|f| f.effects.clone()).unwrap_or_default(),
                "requires": function.map_or(0, |f| f.requires.len()),
                "ensures": function.map_or(0, |f| f.ensures.len()),
            }));
        }
        modules.push(serde_json::json!({
            "module": package.module,
            "package": format!("std/{}", package.directory),
            "dependency": format!("{} = \"^0.1.0\"", package.module),
            "required_profile": profile,
            "tier": package.tier,
            "targets": package.targets,
            "status": package.status,
            "declarations": declarations,
        }));
    }
    let agent = serde_json::json!({
        "schema": CATALOG_SCHEMA,
        "modules": modules,
    });
    (
        human,
        format!("{}\n", serde_json::to_string_pretty(&agent).unwrap()),
    )
}
