//! Actual injected filesystem execution and failure settlement.
use semaprax::filesystem_provider::{FileFailure, FileProvider, FixtureFileProvider};
use semaprax::hosted_interpreter::execute_filesystem_command;
use semaprax::interpreter::{CommandEvaluation, CommandEvaluationOutcome};
use semaprax::{hir, parse, verify};

fn run(body: &str, provider: &mut dyn FileProvider) -> CommandEvaluation {
    let source = format!("module fs.app;\npermit {{ fs.read, fs.write }}\n@id(\"fs.run\") fn run() -> bool uses {{ fs.read, fs.write }} {{ {body} }}\n@id(\"main\") fn main() -> i64 {{ 0 }}\n");
    let ast = parse(&source, "filesystem-interpreter.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let hir = hir::resolve(&ast).unwrap();
    execute_filesystem_command(&hir, "fs.run", provider, 100_000).unwrap()
}
fn code(evaluation: CommandEvaluation) -> u32 {
    let CommandEvaluationOutcome::LanguageFailure(status) = evaluation.outcome else {
        panic!("expected language failure, got {:?}", evaluation.outcome);
    };
    assert_eq!(status.domain_id(), "semaprax.filesystem.v1");
    status.code()
}
#[derive(Default)]
struct Probe {
    reads: usize,
    writes: usize,
    settles: usize,
    oversize: bool,
    wrong_count: bool,
}
impl FileProvider for Probe {
    fn read(&mut self, _: &[u8], max: usize) -> Result<Vec<u8>, FileFailure> {
        self.reads += 1;
        Ok(vec![0; if self.oversize { max + 1 } else { max }])
    }
    fn write_new(&mut self, _: &[u8], bytes: &[u8]) -> Result<usize, FileFailure> {
        self.writes += 1;
        Ok(bytes.len() + usize::from(self.wrong_count))
    }
    fn settle(&mut self) {
        self.settles += 1;
    }
}
#[test]
fn filesystem_interpreter_binary_roundtrip_and_no_overwrite() {
    let mut provider = FixtureFileProvider::new([], true).unwrap();
    let body = "let path = [102u8]; let data = [0u8, 255u8]; let written = file_write_new(array_as_slice(path), 1usize, array_as_slice(data), 2usize); let read = file_read(array_as_slice(path), 1usize, 2usize); written == 2usize && byte_len(bytes_as_slice(read)) == 2usize";
    assert_eq!(
        run(body, &mut provider).outcome,
        CommandEvaluationOutcome::ReturnedBool(true)
    );
    assert_eq!(provider.files().get(b"f".as_slice()).unwrap(), &[0, 255]);
    assert_eq!(code(run(body, &mut provider)), 3);
    assert_eq!(provider.files().get(b"f".as_slice()).unwrap(), &[0, 255]);
    assert_eq!(provider.settlements(), 2);
}
#[test]
fn filesystem_interpreter_rejects_paths_before_provider_dispatch() {
    for bytes in [
        &b""[..],
        b"/x",
        b"x/",
        b"x//y",
        b".",
        b"..",
        b"a/../b",
        b"a/./b",
        b"a\\b",
        b"a:b",
        b"x\0y",
    ] {
        let array = if bytes.is_empty() {
            "[0u8]".to_owned()
        } else {
            format!(
                "[{}]",
                bytes
                    .iter()
                    .map(|b| format!("{b}u8"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let body = format!("let path = {array}; let bytes = file_read(array_as_slice(path), {}usize, 0usize); byte_len(bytes_as_slice(bytes)) == 0usize", bytes.len());
        let mut provider = Probe::default();
        assert_eq!(code(run(&body, &mut provider)), 1, "{bytes:?}");
        assert_eq!(
            (provider.reads, provider.writes, provider.settles),
            (0, 0, 1)
        );
    }
    let mut provider = Probe::default();
    assert_eq!(code(run("let path = [97u8]; let bytes = file_read(array_as_slice(path), 2usize, 0usize); byte_len(bytes_as_slice(bytes)) == 0usize", &mut provider)), 1);
    assert_eq!((provider.reads, provider.settles), (0, 1));
}
#[test]
fn filesystem_interpreter_validates_only_logical_prefix_and_allows_empty_files() {
    let mut provider = FixtureFileProvider::new([], true).unwrap();
    let body = "let path = [112u8, 0u8, 255u8]; let data = [65u8, 66u8]; let written = file_write_new(array_as_slice(path), 1usize, array_as_slice(data), 0usize); let read = file_read(array_as_slice(path), 1usize, 0usize); written == 0usize && byte_len(bytes_as_slice(read)) == 0usize";
    assert_eq!(
        run(body, &mut provider).outcome,
        CommandEvaluationOutcome::ReturnedBool(true)
    );
    assert!(provider.files().get(b"p".as_slice()).unwrap().is_empty());
}
#[test]
fn filesystem_interpreter_host_miscounts_fail_and_settle() {
    let mut provider = Probe {
        oversize: true,
        ..Probe::default()
    };
    assert_eq!(code(run("let path = [97u8]; let bytes = file_read(array_as_slice(path), 1usize, 1usize); byte_len(bytes_as_slice(bytes)) == 1usize", &mut provider)), 4);
    assert_eq!((provider.reads, provider.settles), (1, 1));
    let mut provider = Probe {
        wrong_count: true,
        ..Probe::default()
    };
    assert_eq!(code(run("let path = [97u8]; let data = [0u8]; file_write_new(array_as_slice(path), 1usize, array_as_slice(data), 0usize) == 0usize", &mut provider)), 5);
    assert_eq!((provider.writes, provider.settles), (1, 1));
}
#[test]
fn filesystem_interpreter_operation_limit_is_per_invocation() {
    let body = "let path = [97u8]; let data = [0u8]; let path_view = array_as_slice(path); let data_view = array_as_slice(data); let mut i = 0usize; while i < 65usize { let written = file_write_new(path_view, 1usize, data_view, 0usize); i = i + 1usize; true } true";
    let mut provider = Probe::default();
    for invocation in 1..=2 {
        assert_eq!(code(run(body, &mut provider)), 4);
        assert_eq!(provider.writes, 64 * invocation);
        assert_eq!(provider.settles, invocation);
    }
}

#[test]
fn filesystem_interpreter_reserves_cumulative_bytes_before_dispatch() {
    let body = "let path = [97u8]; let data = bytes_zeroed(65536usize); let path_view = array_as_slice(path); let data_view = bytes_as_slice(data); let mut i = 0usize; while i < 17usize { let written = file_write_new(path_view, 1usize, data_view, 65536usize); i = i + 1usize; true } true";
    let mut provider = Probe::default();
    assert_eq!(code(run(body, &mut provider)), 4);
    assert_eq!((provider.writes, provider.settles), (16, 1));
}
#[test]
fn filesystem_interpreter_later_failure_does_not_undo_created_file() {
    let mut provider = FixtureFileProvider::new([], true).unwrap();
    let body = "let path = [97u8]; let data = [0u8]; let written = file_write_new(array_as_slice(path), 1usize, array_as_slice(data), 0usize); 1usize / written == 0usize";
    let result = run(body, &mut provider);
    assert!(
        matches!(result.outcome, CommandEvaluationOutcome::LanguageFailure(_)),
        "{result:?}"
    );
    assert!(provider.files().get(b"a".as_slice()).unwrap().is_empty());
    assert_eq!(provider.settlements(), 1);
}

#[test]
fn filesystem_interpreter_failure_priority_is_reservation_then_path_then_payload() {
    for (max, expected) in [(65537usize, 1), (1048577usize, 4)] {
        let body = format!("let path = [0u8]; let data = file_read(array_as_slice(path), 1usize, {max}usize); byte_len(bytes_as_slice(data)) == 0usize");
        let mut provider = Probe::default();
        assert_eq!(code(run(&body, &mut provider)), expected);
        assert_eq!((provider.reads, provider.settles), (0, 1));
    }
}
