//! Explicit checked-source codec derivation; publication never overwrites a file.

mod profile;

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;

static NEXT: AtomicU64 = AtomicU64::new(0);
const USAGE: &str = "json-codec <project> --source <module-path> --type <record-id> --output <new-file> [--profile <selector>] [--max-string-bytes <1..64>] [--max-array-items <1..256>]";
pub(crate) const HELP: &str = "Derives checked ordinary source for explicit scalar records; opt-in identifier-views.v1, request-views.v1 and stream-request-views.v1 add bounded identifier/array views; owned-request.v1 and stream-owned-request.v1 materialize bounded identifier String/record collections under an owning runtime profile. UTF-8 owned request profiles bound each decoded string with --max-string-bytes (1..64 UTF-8 bytes). bounded-collection-response.v1 selects an encode-only nested collection view and also requires that bound. bounded-nested-request.v1 requires both --max-string-bytes (1..64) and --max-array-items (1..256); direct decoding only, under the independently admitted caller Project profile. bounded-stream-nested-request.v1 uses the same bounds, original schema stdin permit and a computed normalized spelling envelope; it adds no raw-input or whitespace cap. bounded-nested-response.v1 requires the same two bounds and derives encode-only borrowed finite nested records; primitive Vec<string> refuses. Contract: docs/APPLICATION-JSON-NESTED-RESPONSE-V1.md.\nRequires declared std.data.json scan/token/digits/write dependencies. Publishes a new complete module replacement; never overwrites source.\nContract and typed failure codes: docs/APPLICATION-JSON-CODECS-V1.md\n";

pub(crate) struct Options {
    project: PathBuf,
    source: String,
    record: String,
    output: PathBuf,
    profile: semaprax::project::JsonCodecProfile,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, u8> {
    let fail = || {
        eprintln!("{USAGE}");
        2
    };
    if !matches!(args.len(), 7 | 9 | 11 | 13) || args[0].is_empty() || args[0].starts_with('-') {
        return Err(fail());
    }
    let mut source = None;
    let mut record = None;
    let mut output = None;
    let mut profile = None;
    let mut max_string_bytes = None;
    let mut max_array_items = None;
    for pair in args[1..].chunks_exact(2) {
        if pair[1].starts_with('-') || pair[1].is_empty() {
            return Err(fail());
        }
        match pair[0].as_str() {
            "--source" if source.is_none() => source = Some(pair[1].clone()),
            "--type" if record.is_none() => record = Some(pair[1].clone()),
            "--output" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
            "--profile" if profile.is_none() => profile = Some(pair[1].as_str()),
            "--max-string-bytes" if max_string_bytes.is_none() => {
                max_string_bytes = Some(pair[1].as_str())
            }
            "--max-array-items" if max_array_items.is_none() => {
                max_array_items = Some(pair[1].as_str())
            }
            _ => return Err(fail()),
        }
    }
    Ok(Options {
        project: PathBuf::from(&args[0]),
        source: source.ok_or_else(fail)?,
        record: record.ok_or_else(fail)?,
        output: output.ok_or_else(fail)?,
        profile: profile::parse(profile, max_string_bytes, max_array_items).ok_or_else(fail)?,
    })
}

pub(crate) fn dispatch(
    command: super::help::CommandId,
    args: &[String],
    report: impl Fn(&[Diagnostic]) -> u8,
) -> Result<(), u8> {
    if command == super::help::CommandId::Doc {
        return super::doc::run(super::doc::parse(args)?, report);
    }
    run_codec(parse(args)?, report)
}

fn run_codec(options: Options, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let project = super::project::resolve_positional(options.project);
    // with_authenticated_project performs its final held-input recheck before
    // returning. Only a complete verified source artifact reaches publication.
    let source = semaprax::project::with_authenticated_project(&project, |snapshot| {
        semaprax::project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            &options.source,
            &options.record,
            options.profile,
        )
    })
    .map_err(|errors| report(&errors))?;
    publish_new(&options.output, source.as_bytes()).map_err(|error| report(&[error]))
}

fn publish_new(output: &Path, source: &[u8]) -> Result<(), Diagnostic> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = output
        .file_name()
        .ok_or_else(|| error("output must name a new file"))?;
    let temporary = parent.join(format!(
        ".{}.json-codec-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|failure| error(format!("cannot create private codec output: {failure}")))?;
    let result = file
        .write_all(source)
        .and_then(|()| file.sync_all())
        // Hard-link publication is atomic and refuses an existing destination;
        // unlike rename it cannot overwrite an authored or competing file.
        .and_then(|()| fs::hard_link(&temporary, output));
    drop(file);
    let _ = fs::remove_file(&temporary);
    result.map_err(|failure| error(format!("cannot publish new codec file: {failure}")))
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J181", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_nested_codec_cli_requires_closed_bounds_before_authority() {
        let args = [
            "missing-project",
            "--source",
            "src/schema.spx",
            "--type",
            "orders.request",
            "--output",
            "derived.spx",
            "--profile",
            "bounded-stream-nested-request.v1",
            "--max-string-bytes",
            "16",
            "--max-array-items",
            "8",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            parse(&args)
                .unwrap_or_else(|_| panic!("valid stream bounds"))
                .profile,
            semaprax::project::JsonCodecProfile::StreamNestedRequest {
                max_string_bytes: 16,
                max_array_items: 8,
            }
        );
        for (index, invalid) in [
            (10, "0"),
            (10, "65"),
            (10, "016"),
            (10, "+16"),
            (12, "0"),
            (12, "257"),
            (12, "08"),
            (12, "８"),
        ] {
            let mut changed = args.clone();
            changed[index] = invalid.to_owned();
            assert_eq!(
                dispatch(
                    super::super::help::CommandId::JsonCodec,
                    &changed,
                    |_| panic!("malformed stream bounds reached authoritative generation")
                ),
                Err(2)
            );
        }
        for removed in [7, 9, 11] {
            let mut changed = args.clone();
            changed.drain(removed..removed + 2);
            assert!(parse(&changed).is_err());
        }
        for index in [9, 11] {
            let mut changed = args.clone();
            changed[index] = "--source".to_owned();
            assert!(parse(&changed).is_err());
        }
    }

    #[test]
    fn nested_codec_cli_bounds_are_canonical_closed_and_profile_specific() {
        let args = |bytes: &str, items: &str| {
            [
                "semaprax.toml",
                "--source",
                "src/schema.spx",
                "--type",
                "app.root",
                "--output",
                "derived.spx",
                "--profile",
                "bounded-nested-request.v1",
                "--max-string-bytes",
                bytes,
                "--max-array-items",
                items,
            ]
            .map(str::to_owned)
            .to_vec()
        };
        for selector in [
            "bounded-nested-request.v1",
            "bounded-stream-nested-request.v1",
            "bounded-nested-response.v1",
        ] {
            let selected_args = |bytes: &str, items: &str| {
                let mut selected = args(bytes, items);
                selected[8] = selector.to_owned();
                selected
            };
            for (bytes, items) in [("1", "1"), ("64", "256"), ("16", "8")] {
                let parsed = parse(&selected_args(bytes, items))
                    .unwrap_or_else(|_| panic!("valid nested bounds"));
                let bounds = (bytes.parse().unwrap(), items.parse().unwrap());
                let expected = match selector {
                    "bounded-nested-request.v1" => {
                        semaprax::project::JsonCodecProfile::NestedRequest {
                            max_string_bytes: bounds.0,
                            max_array_items: bounds.1,
                        }
                    }
                    "bounded-stream-nested-request.v1" => {
                        semaprax::project::JsonCodecProfile::StreamNestedRequest {
                            max_string_bytes: bounds.0,
                            max_array_items: bounds.1,
                        }
                    }
                    "bounded-nested-response.v1" => {
                        semaprax::project::JsonCodecProfile::NestedResponse {
                            max_string_bytes: bounds.0,
                            max_array_items: bounds.1,
                        }
                    }
                    _ => unreachable!(),
                };
                assert_eq!(parsed.profile, expected);
            }
            for (bytes, items) in [
                ("0", "1"),
                ("65", "1"),
                ("1", "0"),
                ("1", "257"),
                ("01", "1"),
                ("1", "0256"),
                ("+1", "1"),
                ("1", "+1"),
                (" 1", "1"),
                ("1", "1 "),
                ("١", "1"),
                ("1", "２５６"),
                ("18446744073709551616", "1"),
                ("1", "18446744073709551616"),
            ] {
                assert!(
                    parse(&selected_args(bytes, items)).is_err(),
                    "{bytes}/{items}"
                );
            }
            for removed in [7, 9, 11] {
                let mut incomplete = selected_args("64", "256");
                incomplete.drain(removed..removed + 2);
                assert!(parse(&incomplete).is_err());
            }
            let mut reordered = selected_args("64", "256");
            reordered.swap(9, 11);
            reordered.swap(10, 12);
            assert!(parse(&reordered).is_ok());
            let mut duplicate = selected_args("64", "256");
            duplicate[11] = "--max-string-bytes".to_owned();
            assert!(parse(&duplicate).is_err());
            let mut unknown = selected_args("64", "256");
            unknown[11] = "--maximum-array-items".to_owned();
            assert!(parse(&unknown).is_err());
            // Usage rejection must precede Project access and generation, even
            // when the positional Project path does not exist.
            let mut refused = selected_args("64", "257");
            refused[0] = "not-an-existing-authorized-project".to_owned();
            assert_eq!(
                dispatch(super::super::help::CommandId::JsonCodec, &refused, |_| {
                    panic!("malformed bounds reached authoritative generation")
                }),
                Err(2)
            );
        }
        for old in [
            "identifier-views.v1",
            "request-views.v1",
            "stream-request-views.v1",
            "owned-request.v1",
            "stream-owned-request.v1",
            "utf8-owned-request.v1",
            "stream-utf8-owned-request.v1",
            "bounded-collection-response.v1",
        ] {
            let mut wrong_profile = args("64", "256");
            wrong_profile[8] = old.to_owned();
            assert!(parse(&wrong_profile).is_err(), "{old}");
        }
    }

    #[test]
    fn codec_cli_grammar_is_exact_and_publication_never_overwrites() {
        assert!(super::super::help::scoped("json-codec", false)
            .unwrap()
            .contains("docs/APPLICATION-JSON-CODECS-V1.md"));
        let good = [
            "semaprax.toml",
            "--source",
            "src/schema.spx",
            "--type",
            "app.row",
            "--output",
            "new.spx",
        ];
        let strings = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        assert!(parse(&strings(&good)).is_ok());
        for profile in [
            "identifier-views.v1",
            "request-views.v1",
            "stream-request-views.v1",
            "owned-request.v1",
            "stream-owned-request.v1",
        ] {
            let mut args = strings(&good);
            args.extend(["--profile".to_owned(), profile.to_owned()]);
            assert!(parse(&args).is_ok());
        }
        let mut utf8 = strings(&good);
        utf8.extend([
            "--profile".to_owned(),
            "utf8-owned-request.v1".to_owned(),
            "--max-string-bytes".to_owned(),
            "64".to_owned(),
        ]);
        assert!(parse(&utf8).is_ok());
        let mut stream_utf8 = strings(&good);
        stream_utf8.extend([
            "--profile".to_owned(),
            "stream-utf8-owned-request.v1".to_owned(),
            "--max-string-bytes".to_owned(),
            "64".to_owned(),
        ]);
        assert!(parse(&stream_utf8).is_ok());
        let mut collection_response = strings(&good);
        collection_response.extend([
            "--profile".to_owned(),
            "bounded-collection-response.v1".to_owned(),
            "--max-string-bytes".to_owned(),
            "64".to_owned(),
        ]);
        assert!(parse(&collection_response).is_ok());
        for invalid in ["0", "65", "01", "+1", " 1"] {
            for profile in [
                "utf8-owned-request.v1",
                "stream-utf8-owned-request.v1",
                "bounded-collection-response.v1",
            ] {
                let mut args = strings(&good);
                args.extend([
                    "--profile".to_owned(),
                    profile.to_owned(),
                    "--max-string-bytes".to_owned(),
                    invalid.to_owned(),
                ]);
                assert!(parse(&args).is_err());
            }
        }
        let mut missing_bound = strings(&good);
        missing_bound.extend(["--profile".to_owned(), "utf8-owned-request.v1".to_owned()]);
        assert!(parse(&missing_bound).is_err());
        let mut stream_missing_bound = strings(&good);
        stream_missing_bound.extend([
            "--profile".to_owned(),
            "stream-utf8-owned-request.v1".to_owned(),
        ]);
        assert!(parse(&stream_missing_bound).is_err());
        let mut collection_response_missing_bound = strings(&good);
        collection_response_missing_bound.extend([
            "--profile".to_owned(),
            "bounded-collection-response.v1".to_owned(),
        ]);
        assert!(parse(&collection_response_missing_bound).is_err());
        let mut old_with_bound = strings(&good);
        old_with_bound.extend([
            "--profile".to_owned(),
            "owned-request.v1".to_owned(),
            "--max-string-bytes".to_owned(),
            "8".to_owned(),
        ]);
        assert!(parse(&old_with_bound).is_err());
        let mut unknown = strings(&good);
        unknown.extend(["--profile".to_owned(), "open-json".to_owned()]);
        assert!(parse(&unknown).is_err());
        assert!(parse(&strings(&good[..6])).is_err());
        assert!(parse(&strings(&[
            "p", "--type", "x", "--type", "y", "--output", "z"
        ]))
        .is_err());
        let root = std::env::temp_dir().join(format!(
            "spx-json-codec-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let output = root.join("output.spx");
        publish_new(&output, b"complete\n").unwrap();
        assert_eq!(
            publish_new(&output, b"replacement").unwrap_err().code,
            "SPX-J181"
        );
        assert_eq!(fs::read(&output).unwrap(), b"complete\n");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let raced = root.join("raced.spx");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let workers = [b"first".as_slice(), b"second".as_slice()].map(|bytes| {
            let barrier = barrier.clone();
            let output = raced.clone();
            std::thread::spawn(move || {
                barrier.wait();
                publish_new(&output, bytes)
            })
        });
        let outcomes = workers.map(|worker| worker.join().unwrap());
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|result| result.as_ref().is_err_and(|error| error.code == "SPX-J181"))
                .count(),
            1
        );
        assert!(matches!(
            fs::read(&raced).unwrap().as_slice(),
            b"first" | b"second"
        ));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
