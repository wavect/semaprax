//! Four foreign toolchains read the same public generic metadata and agree,
//! byte for byte, on what it says — and on refusing everything else.
//!
//! This is the executable half of the metadata consumers described in
//! `docs/PUBLIC-GENERIC-CONSUMERS-V1.md`. Each generated consumer is compiled
//! and run for real: once on the metadata it embeds, once on the same bytes
//! from a file, and once per hostile document. The cross-language case is the
//! point of the gate: a refusal has to mean the same thing in Rust, in a Wasm
//! host's TypeScript, in C11, and in C++, or the grammar is not implementable
//! as a shared contract.
//!
//! No generated consumer calls a SEMAPRAX export. There is no public generic
//! calling convention, and this gate claims none.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::public_generic_consumer::{
    generate, ConsumerLanguage, ConsumerMetadata, GeneratedConsumer, Refusal,
};
use semaprax::public_generic_surface::CandidateSurface;
use semaprax::{hir, parse};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.public_generic_consumers;

@id("consumers.leaf")
record Leaf {
    @id("consumers.leaf.head")
    head: Bytes,
    @id("consumers.leaf.tag")
    tag: i64,
}

@id("consumers.pair")
record Pair<T, U> {
    @id("consumers.pair.left")
    left: T,
    @id("consumers.pair.right")
    right: U,
}

@id("consumers.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload, tag: tag }, right: present } =>
            if present && tag > 0 && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("consumers.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input), tag: 7 },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

struct Workspace(PathBuf);

impl Workspace {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-public-generic-consumers-{}-{}-{label}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn surface(exports: &[&str]) -> CandidateSurface {
    let parsed = parse(SOURCE, Path::new("consumers.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let selection = exports
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    CandidateSurface::derive(&program, &selection).unwrap()
}

fn command_available(command: &OsString) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn tool(variable: &str, fallback: &str) -> OsString {
    std::env::var_os(variable).unwrap_or_else(|| OsString::from(fallback))
}

/// Build the consumer for one language and return the command that runs it,
/// or `None` when the toolchain is absent.
fn build(consumer: &GeneratedConsumer, workspace: &Workspace) -> Option<(PathBuf, Vec<OsString>)> {
    for (name, source) in consumer.files() {
        workspace.write(name, source);
    }
    let executable = workspace.path("consumer");
    match consumer.language() {
        ConsumerLanguage::Rust => {
            let rustc = tool("RUSTC", "rustc");
            if !command_available(&rustc) {
                return None;
            }
            compile(
                &rustc,
                &["--edition", "2021", "-D", "warnings", "-C", "debuginfo=0"],
                &workspace.path("consumer.rs"),
                &executable,
                "rustc",
            );
            Some((executable, Vec::new()))
        }
        ConsumerLanguage::C => {
            let cc = tool("CC", "cc");
            if !command_available(&cc) {
                return None;
            }
            compile(
                &cc,
                &["-std=c11", "-O1", "-Wall", "-Wextra", "-Werror"],
                &workspace.path("consumer.c"),
                &executable,
                "cc",
            );
            Some((executable, Vec::new()))
        }
        ConsumerLanguage::Cxx => {
            let cxx = tool("CXX", "c++");
            if !command_available(&cxx) {
                return None;
            }
            compile(
                &cxx,
                &["-std=c++20", "-O1", "-Wall", "-Wextra", "-Werror"],
                &workspace.path("consumer.cpp"),
                &executable,
                "c++",
            );
            Some((executable, Vec::new()))
        }
        ConsumerLanguage::TypeScript => {
            let node = tool("NODE", "node");
            if !command_available(&node) {
                return None;
            }
            Some((
                PathBuf::from(node),
                vec![workspace.path("consumer.mjs").into()],
            ))
        }
    }
}

fn compile(tool: &OsString, flags: &[&str], input: &Path, output: &Path, label: &str) {
    let result = Command::new(tool)
        .args(flags)
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        result.status.success(),
        "{label} rejected the generated consumer:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{label} warned about the generated consumer:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// Run a built consumer, optionally against a metadata file, and return its
/// exact stdout line.
fn run(runner: &(PathBuf, Vec<OsString>), metadata: Option<&Path>) -> (String, Option<i32>) {
    let mut command = Command::new(&runner.0);
    command.args(&runner.1);
    if let Some(path) = metadata {
        command.arg(path);
    }
    let output = command.output().expect("run the generated consumer");
    assert!(
        output.stderr.is_empty(),
        "the consumer wrote to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        output.status.code(),
    )
}

/// The hostile documents every consumer must refuse, with the closed reason
/// each one must report. The stale case is a genuinely different surface, not
/// a corruption: two documents that are each perfectly well formed.
fn hostile(canonical: &str, stale: &str) -> Vec<(&'static str, String, Refusal)> {
    let mut records = ConsumerMetadata::parse(canonical)
        .unwrap()
        .records()
        .to_vec();
    records.swap(0, 1);
    let mut reordered = String::from("spxpgcm1;");
    for record in &records {
        reordered.push_str(&format!("|{};", record.len()));
        for field in record {
            reordered.push_str(&format!("{}:{field};", field.len()));
        }
    }
    vec![
        ("empty", String::new(), Refusal::Malformed),
        (
            "wrong magic",
            canonical.replacen("spxpgcm1;", "spxpgcm2;", 1),
            Refusal::Malformed,
        ),
        (
            "truncated",
            canonical[..canonical.len() - 3].to_owned(),
            Refusal::Malformed,
        ),
        (
            "leading zero count",
            canonical.replacen("|3;", "|03;", 1),
            Refusal::Malformed,
        ),
        (
            "field count beyond the record",
            canonical.replacen("|3;", "|8;", 1),
            Refusal::Malformed,
        ),
        (
            "forged term length",
            canonical.replacen("@14:consumers.leaf<>", "@13:consumers.leaf<>", 1),
            Refusal::Term,
        ),
        ("reordered records", reordered, Refusal::Mismatch),
        (
            "appended record",
            format!("{canonical}|2;1:D;1:x;"),
            Refusal::Mismatch,
        ),
        ("stale surface", stale.to_owned(), Refusal::Mismatch),
    ]
}

/// Every generated consumer compiles warning-free, accepts exactly the
/// metadata it embeds, and refuses every hostile document with the same closed
/// reason as the Rust reference reader and as the other three languages.
#[test]
fn four_generated_consumers_agree_on_public_generic_metadata() {
    let stale = ConsumerMetadata::of(&surface(&["consumers.take"]))
        .unwrap()
        .render();
    let selected = surface(&["consumers.take", "consumers.make"]);
    let reference = ConsumerMetadata::of(&selected).unwrap();
    let canonical = reference.render();
    assert_ne!(canonical, stale);

    let cases = hostile(&canonical, &stale);
    for (label, document, expected) in &cases {
        assert_eq!(
            reference.accepts(document).unwrap_err(),
            *expected,
            "the reference reader must refuse {label} with {}",
            expected.text()
        );
    }

    let mut observed: BTreeMap<&str, BTreeMap<&str, String>> = BTreeMap::new();
    let mut exercised = Vec::new();
    for language in ConsumerLanguage::ALL {
        let workspace = Workspace::new(language.text());
        let consumer = generate(&selected, language).unwrap();
        assert_eq!(consumer.metadata(), canonical);
        let Some(runner) = build(&consumer, &workspace) else {
            continue;
        };
        exercised.push(language.text());

        assert_eq!(
            run(&runner, None),
            ("ok".to_owned(), Some(0)),
            "{} must accept its embedded metadata",
            language.text()
        );
        let accepted = workspace.write("accepted.spxpgcm", &canonical);
        assert_eq!(
            run(&runner, Some(&accepted)),
            ("ok".to_owned(), Some(0)),
            "{} must accept the same bytes from a file",
            language.text()
        );

        let mut refusals = BTreeMap::new();
        for (label, document, expected) in &cases {
            let path = workspace.write("hostile.spxpgcm", document);
            let (line, code) = run(&runner, Some(&path));
            assert_eq!(
                code,
                Some(3),
                "{} must exit 3 on {label}, printed {line}",
                language.text()
            );
            assert_eq!(
                line,
                format!("refused:{}", expected.text()),
                "{} refused {label} with the wrong reason",
                language.text()
            );
            refusals.insert(*label, line);
        }
        observed.insert(language.text(), refusals);
    }

    println!("exercised consumer toolchains: {exercised:?}");
    assert!(
        !exercised.is_empty(),
        "no consumer toolchain was available; this gate proves nothing"
    );
    let (first, reference) = observed.iter().next().expect("one exercised language");
    for (language, refusals) in &observed {
        assert_eq!(
            refusals, reference,
            "{language} and {first} disagreed about a refusal: {observed:?}"
        );
    }
}

/// Regeneration is byte-identical, so a generated consumer can be committed
/// and diffed.
#[test]
fn generated_consumer_files_are_byte_deterministic() {
    let surface = surface(&["consumers.take", "consumers.make"]);
    for language in ConsumerLanguage::ALL {
        let first = generate(&surface, language).unwrap();
        let second = generate(&surface, language).unwrap();
        assert_eq!(first.files(), second.files(), "{}", language.text());
    }
}
