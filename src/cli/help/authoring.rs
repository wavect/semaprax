//! Bounded, authority-free discovery over the embedded checked library catalog.
use std::fmt::Write as _;

const MAX_BYTES: usize = 2_048;
const ROUTES: &str = "Authoring routes (complete):\n  author:stdin-json               v27 bounded native stream command\n  author:stream-data-v2           v29 private record/Vec stream command\n  author:owned-data               v30 private owned-leaf collections\n  author:file-text                native UTF-8 file command\n  author:source-web               single-source web build\n  author:literal-format           checked literal String rendering\n  author:copy-record-vec          flat Copy-record vectors\n  author:json-codec               checked source JSON codec derivation\n  author:json-identifier-views    one identifier as a token view\n  author:json-request-views       bounded identifier and record arrays\n  author:json-stream-request-views bounded native stream request view\n  author:json-owned-request      bounded owning request collections\n  author:json-utf8-owned-request bounded UTF-8 string values\nLibrary-only search: help language find:<word>:0\nExact card sections: help language topics\n";

#[cfg(test)]
pub(super) fn assert_guide_contract() {
    let guide = super::AUTHORING_GUIDE;
    assert_eq!(
        guide,
        include_str!("../../../docs/AGENT-AUTHORING-GUIDE.md")
    );
    assert!(guide.len() <= 2_048);
    assert!(semaprax::agent_economics::lexical_tokens(guide) <= 400);
    assert!(guide.contains("internal-strings-v1` or `text-toolkit-v1`"));
    assert!(guide.contains("Project profiles are distinct"));
    assert!(guide.contains("source-command.v1"));
    assert!(guide.contains("help language all"));
}

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
            "Search bundled declarations: help language find:json:0. For v29 private record helpers: help language author:stream-data-v2.\n",
            "Web, Wasm and npm targets refuse this command profile before emission. Help grants no provider authority.\n"
        ).to_owned()),
        "author:stream-data-v2" => Ok(concat!(
            "Project v29 source implementation; focused and hosted qualification pending.\n",
            "Select [package] profile = \"language-command-io.stream-data.v2\" with input argv-utf8+stdin-stream.v1; native64 selected command only.\n",
            "The command and entry remain fn() -> i64. Private helpers admit explicit flat Copy records, owned/borrowed Vec<R>, and the exact codec/stream/collection outcomes.\n",
            "The v27 profile stays scalar-vector-only. No public record ABI, generic collection helper, or provider grant is added.\n",
            "Generate checked request views with help language author:json-codec; use source-owned codec helpers and keep Ready bytes live through decoding.\n",
            "Exact shapes and target boundary: docs/STREAM-DATA-COMMAND-V2.md; Copy Vec: help language author:copy-record-vec.\n"
        ).to_owned()),
        "author:owned-data" => Ok(concat!(
            "Project v30 source implementation; focused current-head qualification pending.\n",
            "Select [package] profile = \"language-command-io.owned-data.v1\" with input argv-utf8+stdin-stream.v1; native64 selected command only.\n",
            "Entry and command remain fn() -> i64 with the same four explicit command grants. Private helpers admit checked owned-leaf Vec/Iter carriers alongside v29's Copy records and codec outcomes.\n",
            "Choose v30 when private helpers need owned-leaf Vec/Iter runtime; v29 covers private Copy-record Vec helpers and codec outcomes, while v27 remains scalar-Vec-only. Schema declarations alone grant no carrier.\n",
            "vec_clone_at deep-copies an element; vec_replace, vec_reserve_owned and vec_sort_owned transfer the collection owner. Consuming traversal uses for own and vec_into_iter.\n",
            "Source spells String as lowercase string. Write an owning parameter as text: string (not text: own string, SPX-O002); give each user record and field its own @id. Vec<string> push consumes the String.\n",
            "Bytes-bearing allocation/deep copy stays refused in loops, including vec_clone_at<Record> when Record has Bytes and transitive helpers (SPX-T267). Stage payloads and clones outside loops; checked String-bearing loops remain distinct.\n",
            "V27 and v29 stay closed to these owned carriers, including unused helpers. No public nominal ABI, ambient grant or Web/Wasm/npm command route is added.\n",
            "Exact source shapes, admission, cleanup and gates: docs/STREAM-OWNED-DATA-COMMAND-V1.md and docs/OWNED-LEAF-COLLECTIONS-V1.md.\n"
        ).to_owned()),
        "author:literal-format" => Ok(concat!(
            "Checked literal format source implementation; current-head executable qualification pending.\n",
            "string_format(\"id={}\", 7) -> owned string. The first argument is a source literal: <=65536 decoded UTF-8 bytes, <=32 sequential {} fields; {{ and }} escape braces.\n",
            "Fields are i64, u8, usize, bool, or owned string, evaluated left to right. Runtime templates, borrowed strings, floats, JSON escaping and generic/closure bodies are refused.\n",
            "Use only where an existing String backend/profile admits the ordinary function. Exact failure and cleanup contract: docs/CHECKED-LITERAL-FORMAT-V1.md.\n"
        ).to_owned()),
        "author:copy-record-vec" => Ok(concat!(
            "Flat Copy-record Vec source implementation; cross-engine and application qualification pending.\n",
            "R is an explicit monomorphic record with 1..8 direct Copy-scalar fields. Vec<R> uses typed vec_*<R> operations; get copies R, mutations transfer the Vec owner.\n",
            "Capacity is bounded by 8192 scalar words divided by R's field count. Private pure helpers may borrow Vec<R> and pass R by value; generic wrappers, nested/owned records and for traversal remain closed.\n",
            "Core interpreter/native/Wasm contracts and exact status: docs/COPY-RECORD-COLLECTIONS-V1.md. Native v29 private command transport: help language author:stream-data-v2.\n"
        ).to_owned()),
        "author:json-codec" => Ok(concat!(
            "Checked source generator implementation; focused/application qualification pending.\n",
            "semaprax json-codec <project> --source <module-path> --type <record-id> --output <new-file> [--profile <selector>]\n",
            "utf8-owned-request.v1 also requires --max-string-bytes 1..64; see author:json-utf8-owned-request.\n",
            "Omit --profile for the default flat scalar record (1..8 i64/u8/usize/bool fields). Choose by source shape: help language author:json-identifier-views, author:json-request-views or author:json-stream-request-views.\n",
            "Declare std.data.json.scan/token/digits/write. Output is a new complete module, never an overwrite. Generated helpers are ordinary checked source.\n",
            "Declared Vec<string> in a request-view schema is description only: runtime carries Copy views and Vec<View>, not owned String collections. For owning requests: help language author:json-owned-request. This is not a generic JSON codec.\n",
            "View implementations exist; owning combined gates are pending. Stream errors use raw input offsets; post-Ready request/schema errors use normalized-input offsets. Exact profiles, limits and gates: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-identifier-views" => Ok(concat!(
            "Select --profile identifier-views.v1 for a flat record with exactly one string identifier and up to six i64/u8/usize/bool fields.\n",
            "The generated view stores source-relative token bounds; retain the original input while using it. std.data.json.query must be available from scan's declared dependency closure or as an explicit dependency.\n",
            "Source implementation; owning combined gates pending. Invocation and limits: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-request-views" => Ok(concat!(
            "Select --profile request-views.v1 for a request record whose fields are Vec<string> then Vec<IdentifierRecord>. Runtime collections carry Copy views, not owned String values.\n",
            "Bounds are 0..8 server IDs and 0..256 records; IDs are unique 1..16 ASCII letters, digits, underscore or hyphen. Zero servers is valid only with zero records. Source implementation; owning combined gates pending.\n",
            "Invocation and exact schema: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-stream-request-views" => Ok(concat!(
            "Select --profile stream-request-views.v1 for request views used by a native v29 streaming command. The original schema module must already declare process.stdin.read; derivation grants no capability.\n",
            "The source implementation exists; owning composition gates remain pending. The current composition fixture rejects extra chunks. Exact requirements: docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-utf8-owned-request" => Ok(concat!(
            "UTF-8 owned request source implementation; focused current-head qualification pending.\n",
            "Select --profile utf8-owned-request.v1 and --max-string-bytes N, canonical decimal 1..64 per decoded string in either array. It is not a stream selector.\n",
            "Use Vec<string> (0..8) then Vec<Row> (0..256); Row has one string identifier and up to six i64/u8/usize/bool fields with explicit IDs. Empty and duplicate values are accepted; the second array may be nonempty when the first is empty.\n",
            "Values may contain Unicode, including NUL. Raw input remains subject to the existing 65,536-byte borrowed-root limit; this profile does not raise it. Source field identifiers remain ASCII. Runtime owns decoded strings independently; retain declared JSON dependencies and select private owned-data-api.v1 or native v30.\n",
            "Invocation and exact source policy: help language author:json-codec and docs/APPLICATION-JSON-CODECS-V1.md.\n"
        ).to_owned()),
        "author:json-owned-request" => Ok(concat!(
            "Owned request source implementation; focused cross-backend and application qualification pending.\n",
            "Select --profile owned-request.v1, or stream-owned-request.v1 with the original schema's process.stdin.read permit and the manifest grant. Derivation grants no capability.\n",
            "Request fields are Vec<string> then Vec<Row>; Row has one string identifier and up to six i64/u8/usize/bool fields with explicit record/field IDs. Names are authored, order is declaration order.\n",
            "Bounds: 0..8 first-array identifiers, 0..256 rows, unique identifiers of 1..16 ASCII letters/digits/underscore/hyphen; a nonempty second array needs a nonempty first array. Arbitrary Unicode, recursion and nullable values are outside this policy.\n",
            "Actual runtime collections own independent Strings and Rows. Select private owned-data-api.v1 or native v30 language-command-io.owned-data.v1; v29 stays closed to owned carriers.\n",
            "json_<Request>_owned_decode takes borrow Slice<u8> and returns an owning Decoded or Error{code,offset,field}; malformed input is an application outcome. Stream normalization is separate: decode Ready's normalized slice, then release its Bytes after decode.\n",
            "json_<Request>_owned_encoded_len and _owned_encode borrow both Vecs; encode also takes output_limit. Exact source, cleanup, limits and ordinary failure contracts: docs/APPLICATION-JSON-CODECS-V1.md.\n"
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
            "author:stream-data-v2",
            "author:owned-data",
            "author:file-text",
            "author:source-web",
            "author:literal-format",
            "author:copy-record-vec",
            "author:json-codec",
            "author:json-identifier-views",
            "author:json-request-views",
            "author:json-stream-request-views",
            "author:json-owned-request",
            "author:json-utf8-owned-request",
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
        let codec = lookup("author:json-codec").unwrap();
        assert!(codec
            .contains("--profile <selector>"));
        assert!(codec.contains("--max-string-bytes 1..64"));
        assert!(codec.contains("Omit --profile for the default flat scalar record"));
        assert!(codec.contains("author:json-identifier-views"));
        assert!(codec.contains("Declared Vec<string> in a request-view schema is description only"));
        assert!(codec.contains("post-Ready request/schema errors use normalized-input offsets"));
        let identifier_views = lookup("author:json-identifier-views").unwrap();
        assert!(identifier_views.contains("exactly one string identifier"));
        assert!(identifier_views.contains("combined gates pending"));
        let request_views = lookup("author:json-request-views").unwrap();
        assert!(request_views.contains("Vec<IdentifierRecord>"));
        assert!(request_views.contains("0..256 records"));
        let stream_request_views = lookup("author:json-stream-request-views").unwrap();
        assert!(stream_request_views.contains("process.stdin.read"));
        assert!(stream_request_views.contains("composition fixture rejects extra chunks"));
        let utf8_request = lookup("author:json-utf8-owned-request").unwrap();
        for fact in [
            "utf8-owned-request.v1",
            "--max-string-bytes",
            "per decoded string",
            "0..8",
            "0..256",
            "Unicode",
        ] {
            assert!(utf8_request.contains(fact), "UTF-8 request guidance omits {fact}");
        }
        let owned_request = lookup("author:json-owned-request").unwrap();
        for fact in [
            "owned-request.v1",
            "stream-owned-request.v1",
            "0..256 rows",
            "1..16 ASCII",
            "v29 stays closed",
            "normalized slice",
            "qualification pending",
        ] {
            assert!(
                owned_request.contains(fact),
                "owned request guidance omits {fact}"
            );
        }
        let stream = lookup("author:stream-data-v2").unwrap();
        assert!(stream.contains("language-command-io.stream-data.v2"));
        assert!(stream.contains("v27 profile stays scalar-vector-only"));
        let owned = lookup("author:owned-data").unwrap();
        assert!(owned.contains("language-command-io.owned-data.v1"));
        assert!(owned.contains("including unused helpers"));
        assert!(owned.contains("SPX-T267"));
        assert!(owned.contains("fn() -> i64"));
        assert!(owned.contains("Choose v30 when private helpers need owned-leaf Vec/Iter runtime"));
        let routes = lookup("author:routes").unwrap();
        for selector in [
            "author:json-identifier-views",
            "author:json-request-views",
            "author:json-stream-request-views",
            "author:json-owned-request",
            "author:json-utf8-owned-request",
        ] {
            assert!(routes.contains(selector), "route list omits {selector}");
        }
        let mut query = "find:std.data.json.:0".to_owned();
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
            .filter(|declaration| {
                declaration["id"]
                    .as_str()
                    .unwrap()
                    .contains("std.data.json.")
            })
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
