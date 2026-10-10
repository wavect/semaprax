//! Source and metadata for the additive Project v31 collection-record starter.

pub const TEMPLATE: &str = "stdin-stream-collection-record";
pub(super) const FILE_COUNT: usize = 6;
pub(super) const INVENTORY: [&str; FILE_COUNT] = [
    "README.md",
    "AGENTS.md",
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

pub(super) const README: &str = "# {{name}}\n\nA native stdin command starter for Project v31 collection records. It keeps the ordinary command route and demonstrates a borrowed `Report` with a projected `Vec<string>` length.\n\n```sh\nsemaprax check .\nsemaprax test .\nsemaprax build --manifest-path semaprax.toml --target native --output app\nprintf 'sample\\n' | ./app\n```\n\nThe sample consumes stdin in bounded chunks and returns a small status. The v31 profile selects native64; web, Wasm, npm, and interpreter command execution are refused. Runtime qualification for this additive profile remains pending. Read `AGENTS.md` before editing the source.\n";

pub(super) const GUIDE: &str = "\n## Native collection-record command\n\nThis project selects Project v31 profile `language-command-io.collection-record.v1`, schema `semaprax.project.v31`, and input `argv-utf8+stdin-stream.v1`. The command and entry/test roots are separate stable-ID functions. The four command grants are unchanged from the stdin-stream-data starter.\n\n`core.spx` declares `Metrics` and `Report`; `inspect` immutably borrows `Report` and reads `vec_len<string>(value.items)` plus the nested scalar. For the bounded authoring card, run `semaprax help language author:collection-records`. This starter demonstrates the source shape only. The Project v31 current-head execution qualification is pending. Native64 is the selected target; web, Wasm, npm, and interpreter command execution are refused.\n";

pub(super) const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"{{name}}\"\nversion = \"0.1.0\"\nprofile = \"language-command-io.collection-record.v1\"\n\n[modules]\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"{{module}}.tests\"]\n\n[exports]\nweb = [\"{{command}}\"]\n\n[command]\nfunction = \"{{command}}\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n\n[targets]\nmatrix = [\"native64\"]\n";

pub(super) const APP: &str = r#"module {{module}}.app;
use function @id("{{name}}.verify") from {{module}}.core as verify;

permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }

@id("{{command}}")
fn command() -> i64
    uses { process.stdin.read, process.stdout.write }
{
    let mut reader = stdin_stream_open();
    let mut saw_input = false;
    while !stdin_stream_eof(reader) {
        let chunk = stdin_stream_chunk(reader);
        saw_input = saw_input || byte_len(chunk) > 0usize;
        reader = stdin_stream_next(reader);
        0
    }
    let score = verify();
    if score == 12 && saw_input {
        let output = string_from_i64(score);
        let view = string_as_str(output);
        let ignored = stdout_write(str_as_bytes(view));
        0
    } else { 1 }
}

@id("{{name}}.app.main")
fn main() -> i64
{
    if verify() == 12 { 0 } else { 1 }
}
"#;

pub(super) const CORE: &str = r#"module {{module}}.core;

@id("{{name}}.metrics")
record Metrics { @id("{{name}}.metrics.selected") selected: i64, }

@id("{{name}}.report")
record Report {
    @id("{{name}}.report.items") items: Vec<string>,
    @id("{{name}}.report.metrics") metrics: Metrics,
}

@id("{{name}}.inspect")
fn inspect(value: borrow Report) -> i64
{
    i64_from_usize(vec_len<string>(value.items)) + value.metrics.selected
}

@id("{{name}}.verify")
fn verify() -> i64
{
    let empty = vec_with_capacity<string>(2usize);
    let one = vec_push<string>(empty, "left");
    let two = vec_push<string>(one, "right");
    let report = Report { items: two, metrics: Metrics { selected: 10 } };
    inspect(report)
}
"#;

pub(super) const TESTS: &str = r#"module {{module}}.tests;
use function @id("{{name}}.verify") from {{module}}.core as verify;

@id("{{name}}.tests.test_report")
fn test_report() -> i64
{
    if verify() == 12 { 0 } else { 1 }
}

@id("{{name}}.tests.main")
fn main() -> i64
{
    if verify() == 12 { 0 } else { 1 }
}
"#;
