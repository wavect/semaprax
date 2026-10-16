//! V6 fixed task/profile routes; embedded checked descriptors carry no authority.
use serde_json::Value;
use std::fmt::Write as _;

const DESCRIPTORS: &str = include_str!("tasks/catalog.json");
const INDEX: &str = concat!(
    "Task guidance v6; closed checked-starter subset, not profile-wide compatibility.\n",
    "help language author:task:file-text:source-command.v1:native64:0\n",
    "help language author:task:stdin-stream:language-command-io.stream-data.v1:native64:0\n",
    "help language author:task:stdin-json:language-command-io.collection-record.v1:native64:0\n",
    "JSON specimen: ordinary compiler admission and executable qualification pending.\n",
    "Other profiles/tasks: outside this pilot; help language author:routes.\n",
    "Full/exact catalog remains: help library all; help library <module|name|stable-id>.\n"
);

pub(super) fn lookup(query: &str, budget: usize) -> Result<String, String> {
    if query == "author:tasks" {
        return if INDEX.len() <= budget {
            Ok(INDEX.to_owned())
        } else {
            Err("task index exceeds byte budget".to_owned())
        };
    }
    let parts: Vec<_> = query.split(':').collect();
    let ["author", "task", task, profile, target, offset] = parts.as_slice() else {
        return Err(
            "task guidance requires author:task:<task>:<Project-profile>:<target>:<offset>"
                .to_owned(),
        );
    };
    let start = offset
        .parse::<usize>()
        .ok()
        .filter(|n| n.to_string() == *offset)
        .ok_or_else(|| "task offset must be canonical unsigned decimal".to_owned())?;
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).expect("checked task descriptors");
    if *task == "stdin-json" {
        let route = &descriptors["json_task"];
        if text(route, "project_profile") != *profile || text(route, "target") != *target {
            return Err("JSON task pilot requires author:task:stdin-json:language-command-io.collection-record.v1:native64:0; other profiles: author:routes".to_owned());
        }
        let catalog: Value =
            serde_json::from_str(super::super::LIBRARY_INDEX).expect("checked library catalog");
        return json_page(route, &catalog, start, budget);
    }
    let route = descriptors["tasks"]
        .as_array()
        .expect("task inventory")
        .iter()
        .find(|route| text(route, "task") == *task)
        .ok_or_else(|| {
            "task is outside this pilot; use help language author:tasks or author:routes".to_owned()
        })?;
    if text(route, "project_profile") != *profile || text(route, "target") != *target {
        return Err(format!("unsupported task/profile/target pair in this pilot; use help language author:task:{}:{}:{}:0; other shapes: help language author:routes", text(route,"task"), text(route,"project_profile"), text(route,"target")));
    }
    let catalog: Value =
        serde_json::from_str(super::super::LIBRARY_INDEX).expect("checked library catalog");
    page(route, &catalog, start, budget)
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .expect("checked descriptor/catalog string")
}
fn list(value: &Value) -> String {
    value
        .as_array()
        .expect("checked string array")
        .iter()
        .map(|v| v.as_str().expect("checked string"))
        .collect::<Vec<_>>()
        .join(", ")
}
fn capsule(route: &Value) -> String {
    let mut out = format!("Task v6: {}\nChecked starter subset; not whole-profile/library admission.\nnew app --template {}\nProject profile {}; target {}; input {}\nManifest semaprax.toml (tables); sources {}\nCapabilities (declare AND grant): {}\n",
        text(route,"task"), text(route,"template"), text(route,"project_profile"), text(route,"target"),text(route,"input"),list(&route["sources"]),list(&route["capabilities"]));
    for (name, version) in route["dependencies"].as_object().expect("dependencies") {
        writeln!(
            out,
            "Manifest dependency {name} = {:?}",
            version.as_str().expect("dependency version")
        )
        .unwrap();
    }
    if route["dependencies"].as_object().unwrap().is_empty() {
        out.push_str(
            "Manifest dependencies: none; transport/count example, not JSON composition.\n",
        );
    }
    out.push_str("Example bytes: project-scaffold --name task-demo --layout tables --template ");
    out.push_str(text(route, "template"));
    out.push('\n');
    for file in route["files"].as_array().expect("file bindings") {
        writeln!(
            out,
            "{} sha256 {}",
            text(file, "path"),
            text(file, "sha256")
        )
        .unwrap();
    }
    out.push_str(if text(route, "task") == "file-text" {
        "Next: check app; test app --target native; build app --target native -o app-bin\n"
    } else {
        "Next: check app; test app; build app --target native -o app-bin\n"
    });
    writeln!(
        out,
        "Detail: help language {}; full/exact library: help library all / <stable-id>",
        text(route, "card")
    )
    .unwrap();
    out
}
fn declaration(catalog: &Value, id: &str) -> Result<String, String> {
    let mut found = Vec::new();
    for module in catalog["modules"].as_array().expect("catalog modules") {
        for declaration in module["declarations"].as_array().expect("declarations") {
            if text(declaration, "id") == id {
                found.push((module, declaration));
            }
        }
    }
    let [(module, declaration)] = found.as_slice() else {
        return Err(format!(
            "task descriptor has stale/ambiguous declaration `{id}`; use help library {id}"
        ));
    };
    let mut row = format!("\n{id}\nLibrary dependency {}; library required-profile {} (distinct from Project profile); status {}; targets {}\n",
        text(module,"dependency"),text(module,"required_profile"),text(module,"status"),list(&module["targets"]));
    for line in declaration["description"].as_array().expect("description") {
        writeln!(row, "{}", line.as_str().expect("description line")).unwrap();
    }
    for line in declaration["head"].as_array().expect("signature") {
        writeln!(row, "{}", line.as_str().expect("signature line")).unwrap();
    }
    writeln!(
        row,
        "Effects: {}; requires {}; ensures {}; kind {}; detail: help library {id}",
        list(&declaration["effects"]),
        declaration["requires"],
        declaration["ensures"],
        text(declaration, "kind")
    )
    .unwrap();
    Ok(row)
}
fn continuation(route: &Value, next: usize, total: usize) -> String {
    if next == total {
        "\nCoverage: complete checked-starter imported declaration subset.\n".to_owned()
    } else {
        format!("\nCoverage: partial checked-starter subset; next: help language author:task:{}:{}:{}:{next}\n",text(route,"task"),text(route,"project_profile"),text(route,"target"))
    }
}
fn page(route: &Value, catalog: &Value, start: usize, budget: usize) -> Result<String, String> {
    let ids = route["declarations"]
        .as_array()
        .expect("imported declarations");
    if !ids.windows(2).all(|pair| {
        pair[0].as_str().expect("stable identity") < pair[1].as_str().expect("stable identity")
    }) {
        return Err("task descriptor identities are not strictly ordered and unique".to_owned());
    }
    if start >= ids.len() && !(start == 0 && ids.is_empty()) {
        return Err("task offset is outside its checked declaration subset".to_owned());
    }
    // Resolve every fixed identity first: a later stale row cannot hide behind pagination.
    let rows = ids
        .iter()
        .map(|id| declaration(catalog, id.as_str().expect("stable identity")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = capsule(route);
    let mut next = start;
    while next < rows.len() {
        if output.len() + rows[next].len() + continuation(route, next + 1, rows.len()).len()
            > budget
        {
            break;
        }
        output.push_str(&rows[next]);
        next += 1;
    }
    if next == start && !rows.is_empty() {
        return Err(format!("task prerequisite capsule plus whole declaration exceeds byte budget; use help library {}",ids[start].as_str().unwrap()));
    }
    output.push_str(&continuation(route, next, rows.len()));
    if output.len() > budget {
        return Err("complete task prerequisite capsule exceeds byte budget".to_owned());
    }
    Ok(output)
}

#[cfg(test)]
#[path = "tasks/tests.rs"]
mod tests;

fn json_capsule(route: &Value) -> String {
    format!("Task v6: stdin-json request -> decision -> response\nProfile {}; target {}; input {}\nCapabilities (declare AND grant): {}\nFixed ShiftSim specimen; source implementation, executable qualification pending. Ordinary admission (including SPX-G171) may refuse; help grants no authority.\n",
        text(route,"project_profile"),text(route,"target"),text(route,"input"),list(&route["capabilities"]))
}
fn json_items(route: &Value, catalog: &Value) -> Result<Vec<String>, String> {
    let ids = route["declarations"]
        .as_array()
        .expect("dependency catalog identities");
    if !ids
        .windows(2)
        .all(|pair| pair[0].as_str().unwrap() < pair[1].as_str().unwrap())
    {
        return Err("JSON task identities are not strictly ordered and unique".to_owned());
    }
    // Resolve the entire declared dependency catalog before any page succeeds.
    let rows = ids
        .iter()
        .map(|id| declaration(catalog, id.as_str().unwrap()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut items = vec![
        format!("Minimal route, from the unchanged specimen checkout:\np={}\nsemaprax-full new fresh-app --from \"$p\" --codec-plan \"$p/codec-plan.json\" --command-profile language-command-io.collection-record.v1 --command-path src/app.spx --command-source \"$p/src/app.command.spx\"\nsemaprax check fresh-app\nsemaprax test fresh-app --target native-collection-record-pure.v1\nsemaprax build fresh-app --target native -o app-bin\n./app-bin < \"$p/fixtures/request.json\"\nNative64/full-host installation only; no Web/Wasm or implicit capabilities. No source overwrite or runtime execution during installation. Old v31 --target native test refusal remains.\n",text(route,"specimen")),
        format!("Specimen: {} (requires its unchanged source checkout)\nComplete authored source inventory: {}\nPipeline: normalize/decode Request; schedule decision; encode Report; publish one JSON line.\n",text(route,"specimen"),list(&route["sources"])),
        format!("Paired plan (exact selectors/bounds, not admission): {}\nExternal review remains available: json-codec <project> --plan <plan-json> --output <new-bundle>; install only through ordinary source/managed authority.\n",serde_json::to_string(&route["plan"]).unwrap()),
        "Prerequisites/limits: native64 only; original v31 ownership, fuel, input/output and builder bounds remain. Fixed specimen identifiers are nonempty ASCII <=16 bytes, <=8 servers and <=256 patients; numeric/application validation remains. No general JSON schema, arbitrary Unicode identifiers, Wasm/web, foreign profiles or capacity increase is implied.\nDetail: help language author:collection-records; author:json-request-response; author:json-collection-response; docs/APPLICATION-STARTER-V1.md; docs/APPLICATION-JSON-CODEC-PLAN-V1.md.\n".to_owned(),
        format!("Direct manifest dependencies: {}\nComplete transitive bundled module closure: {}\nResolved bundled versions: {}\nFollowing exact catalog signatures cover this dependency closure, not a claim every declaration is callable under the consumer Project profile. Library required-profile remains a separate role.\n",serde_json::to_string(&route["dependencies"]).unwrap(),list(&route["dependency_closure"]),serde_json::to_string(&route["dependency_versions"]).unwrap()),
    ];
    for file in route["files"]
        .as_array()
        .expect("complete specimen input bindings")
    {
        items.push(format!(
            "Specimen input {} sha256 {}\n",
            text(file, "path"),
            text(file, "sha256")
        ));
    }
    for stage in route["pipeline"]
        .as_array()
        .expect("actual pipeline documentation")
    {
        items.push(format!(
            "Pipeline declaration {} in {}\n{}\n",
            text(stage, "id"),
            text(stage, "source"),
            stage["head"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    items.extend(rows);
    Ok(items)
}
fn json_continuation(next: usize, total: usize) -> String {
    if next == total {
        "\nCoverage: complete JSON task prerequisites, input inventory and dependency-closure catalog; executable qualification pending.\n".to_owned()
    } else {
        format!("\nCoverage: partial JSON task ({next}/{total} entries); next: help language author:task:stdin-json:language-command-io.collection-record.v1:native64:{next}\n")
    }
}
fn json_page(
    route: &Value,
    catalog: &Value,
    start: usize,
    budget: usize,
) -> Result<String, String> {
    let items = json_items(route, catalog)?;
    if start >= items.len() {
        return Err("JSON task offset is outside its complete entry inventory".to_owned());
    }
    let mut output = json_capsule(route);
    let mut next = start;
    while next < items.len()
        && output.len() + items[next].len() + json_continuation(next + 1, items.len()).len()
            <= budget
    {
        output.push_str(&items[next]);
        next += 1;
    }
    if next == start {
        return Err("JSON task prerequisite capsule plus whole entry exceeds byte budget; use help library <stable-id> or docs/CLI-HELP-V6.md".to_owned());
    }
    output.push_str(&json_continuation(next, items.len()));
    Ok(output)
}
