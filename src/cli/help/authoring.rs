//! Bounded, authority-free discovery over the embedded checked library catalog.
use std::fmt::Write as _;

const MAX_BYTES: usize = 2_048;
const ROUTES: &str = "Authoring routes (complete):\n  author:stdin-json  bounded streaming native data command\n  author:file-text   native UTF-8 file command\n  author:source-web  single-source web build\nLibrary discovery: help language find:<word>:0\nExact syntax: help language topics\n";

pub(super) fn lookup(query: &str) -> Result<String, String> {
    match query {
        "author:routes" => Ok(ROUTES.to_owned()),
        "author:stdin-json" => Ok(concat!(
            "Native stream data route (complete guidance; exact library lookup follows):\n",
            "new app --template stdin-stream-data\n",
            "Project profile language-command-io.stream-data.v1; input argv-utf8+stdin-stream.v1; native64 only.\n",
            "Capabilities: process.args.read, process.stderr.write, process.stdin.read, process.stdout.write.\n",
            "Follow generated semaprax.toml and README; check app; test app; build app --target native -o app-bin.\n",
            "Public entry and command stay fn() -> i64. Private borrow Vec<Copy scalar> helpers are admitted; authored nominal helper closure is refused by this profile.\n",
            "JSON is parsed by ordinary declared source dependencies; scan bounded input views, preserve the reader owner and chunk lifetime.\n",
            "Exact prerequisites/signatures: help library std.data.json.scan; help library std.data.json.token.\n",
            "Search: help language find:json:0. Shapes: help shapes lists; syntax: help language ownership.\n",
            "Web, Wasm and npm targets refuse this command profile before emission. Help grants no provider authority.\n"
        ).to_owned()),
        "author:file-text" => Ok(concat!(
            "Native file text route (complete guidance; exact library lookup follows):\n",
            "new app --template source-command-file-text\n",
            "Project profile source-command.v1; input argv-utf8+file-text.v1; native64 only.\n",
            "Capabilities: fs.read, process.args.read, process.stderr.write, process.stdout.write.\n",
            "The template declares std.int.decimal and includes a sample file; follow generated README for invocation.\n",
            "check app; test app --target native; build app --target native -o app-bin. Invoke ./app-bin <relative-file>.\n",
            "Project interpreter invocation, Web, Wasm and npm targets refuse this command route.\n",
            "Exact dependency/profile/signatures: help library std.int.decimal; source imports: help language projects.\n",
            "Search: help language find:decimal:0. This route grants no ambient filesystem authority.\n"
        ).to_owned()),
        "author:source-web" => Ok(concat!(
            "Single-source web route (complete guidance):\n",
            "fmt app.spx; check app.spx; build app.spx --target web --profile internal-strings-v1 -o web.\n",
            "text-toolkit-v1 is the other explicit single-source web profile; help build gives exact grammar.\n",
            "These are source build profiles, not Project manifest profiles. Project web builds follow their declared profile and export closure.\n",
            "Read exact boundary limits with help language web; choose checked examples with help shapes kinds.\n",
            "A generated web package does not grant a filesystem, process, network or secret capability.\n"
        ).to_owned()),
        _ if query.starts_with("find:") => search(query, MAX_BYTES),
        _ => Err("unknown authoring route; use `help language author:routes`".to_owned()),
    }
}

fn search(query: &str, budget: usize) -> Result<String, String> {
    let mut parts = query.split(':');
    let (Some("find"), Some(word), Some(offset), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("library search requires find:<word>:<offset>".to_owned());
    };
    if word.is_empty()
        || word.len() > 64
        || !word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(
            "library search word must be 1..64 ASCII letters, digits, dots, underscores or hyphens"
                .to_owned(),
        );
    }
    let start = offset
        .parse::<usize>()
        .ok()
        .filter(|value| value.to_string() == offset)
        .ok_or_else(|| "library search offset must be canonical unsigned decimal".to_owned())?;
    let catalog: serde_json::Value = serde_json::from_str(super::LIBRARY_INDEX)
        .expect("checked standard library catalog must parse");
    let mut matches = Vec::new();
    let modules = catalog["modules"].as_array().expect("catalog modules");
    for module in modules {
        let module_id = module["module"].as_str().expect("catalog module id");
        for declaration in module["declarations"]
            .as_array()
            .expect("catalog declarations")
        {
            let id = declaration["id"].as_str().expect("catalog declaration id");
            let name = declaration["name"]
                .as_str()
                .expect("catalog declaration name");
            let head = declaration["head"]
                .as_array()
                .expect("catalog declaration head");
            if module_id.contains(word)
                || id.contains(word)
                || name.contains(word)
                || head
                    .iter()
                    .any(|line| line.as_str().expect("catalog head line").contains(word))
            {
                let mut row = format!(
                    "{id}\ndependency {}\nprofile {}\n",
                    module["dependency"].as_str().expect("catalog dependency"),
                    module["required_profile"]
                        .as_str()
                        .expect("catalog profile")
                );
                for line in head {
                    writeln!(row, "{}", line.as_str().expect("catalog head line")).unwrap();
                }
                matches.push((id, row));
            }
        }
    }
    matches.sort_by(|left, right| left.0.cmp(right.0));
    if matches.is_empty() {
        return Err(format!("library search has no match for `{word}`"));
    }
    if start >= matches.len() {
        return Err("library search offset is outside its matching inventory".to_owned());
    }
    let mut output = format!(
        "Library matches: {}; start: {start}; budget: {budget} UTF-8 bytes\n",
        matches.len()
    );
    let mut next = start;
    while next < matches.len() {
        let footer = footer(word, next + 1, matches.len());
        let row = &matches[next].1;
        if output.len() + row.len() + 1 + footer.len() > budget {
            break;
        }
        output.push('\n');
        output.push_str(row);
        next += 1;
    }
    if next == start {
        return Err(format!(
            "library search entry exceeds page budget; use `help library {}`",
            matches[start].0
        ));
    }
    output.push_str(&footer(word, next, matches.len()));
    Ok(output)
}

fn footer(word: &str, next: usize, total: usize) -> String {
    if next == total {
        "\nCoverage: complete matching inventory.\n".to_owned()
    } else {
        format!("\nCoverage: partial; next: help language find:{word}:{next}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_and_search_are_bounded_exact_and_have_complete_continuations() {
        for route in [
            "author:routes",
            "author:stdin-json",
            "author:file-text",
            "author:source-web",
        ] {
            let output = lookup(route).unwrap();
            assert!(output.len() <= MAX_BYTES);
            assert_eq!(output, lookup(route).unwrap());
        }
        for bad in [
            "author:unknown",
            "find::0",
            "find:json:00",
            "find:json:-1",
            "find:json:0:extra",
            "find:json:999999",
            "find:JSON_NOT_REAL:0",
            "find:json space:0",
        ] {
            assert!(lookup(bad).is_err(), "{bad}");
        }
        let mut query = "find:json:0".to_owned();
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let page = lookup(&query).unwrap();
            assert!(page.len() <= MAX_BYTES);
            for line in page
                .lines()
                .filter(|line| line.starts_with("std.") && !line.contains(' '))
            {
                assert!(ids.insert(line.to_owned()), "duplicate page entry {line}");
                assert!(super::super::library::library_entry(line).is_ok());
            }
            if let Some(next) = page
                .lines()
                .find_map(|line| line.strip_prefix("Coverage: partial; next: help language "))
            {
                assert_ne!(next, query);
                query = next.to_owned();
            } else {
                assert!(page.ends_with("Coverage: complete matching inventory.\n"));
                break;
            }
        }
        let catalog: serde_json::Value = serde_json::from_str(super::super::LIBRARY_INDEX).unwrap();
        let expected = catalog["modules"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|module| module["declarations"].as_array().unwrap().iter())
            .filter(|declaration| declaration["id"].as_str().unwrap().contains("json"))
            .map(|declaration| declaration["id"].as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids, expected);
    }

    #[test]
    fn exact_page_budget_and_one_short_preserve_all_entry_bytes() {
        let query = "find:std.data.json.scan.strict_end:0";
        let mut budget = 256;
        while search(query, budget).is_err() {
            budget += 1;
            assert!(budget < MAX_BYTES);
        }
        let page = search(query, budget).unwrap();
        assert_eq!(page.len(), budget);
        assert!(search(query, budget - 1).is_err());
        assert!(page.contains("dependency ") && page.contains("profile "));
    }
}
