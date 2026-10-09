//! Explicit checked-source codec derivation; publication never overwrites a file.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;

static NEXT: AtomicU64 = AtomicU64::new(0);
const USAGE: &str =
    "json-codec <project> --source <module-path> --type <record-id> --output <new-file>";
pub(crate) const HELP: &str = "Derives checked ordinary source for an explicit flat i64/u8/usize/bool record.\nRequires declared std.data.json scan/token/digits/write dependencies. Publishes a new complete module replacement; never overwrites source.\nContract and typed failure codes: docs/APPLICATION-JSON-CODECS-V1.md\n";

pub(crate) struct Options {
    project: PathBuf,
    source: String,
    record: String,
    output: PathBuf,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, u8> {
    let fail = || {
        eprintln!("{USAGE}");
        2
    };
    if args.len() != 7 || args[0].is_empty() || args[0].starts_with('-') {
        return Err(fail());
    }
    let mut source = None;
    let mut record = None;
    let mut output = None;
    for pair in args[1..].chunks_exact(2) {
        if pair[1].starts_with('-') || pair[1].is_empty() {
            return Err(fail());
        }
        match pair[0].as_str() {
            "--source" if source.is_none() => source = Some(pair[1].clone()),
            "--type" if record.is_none() => record = Some(pair[1].clone()),
            "--output" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
            _ => return Err(fail()),
        }
    }
    Ok(Options {
        project: PathBuf::from(&args[0]),
        source: source.ok_or_else(fail)?,
        record: record.ok_or_else(fail)?,
        output: output.ok_or_else(fail)?,
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
        semaprax::project::derive_json_codec_source(
            &snapshot.retain_revision(),
            &options.source,
            &options.record,
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
