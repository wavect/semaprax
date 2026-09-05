use std::path::Path;

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

const SOURCE: &str = r#"
module test.network_wasm;
permit { network.connect, network.read, network.write, process.stdout.write }
@id("test.network_wasm.run")
fn run() -> bool uses { network.connect, network.read, network.write, process.stdout.write } {
    let host = [104u8, 111u8, 115u8, 116u8];
    let handle = net_connect(array_as_slice(host), 80usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let first = net_stream_stdout(handle, 4096usize);
    let second = net_stream_stdout(handle, 4096usize);
    let tail = net_stream_stdout(handle, 4096usize);
    net_close(handle) == 0usize && sent == 5usize && tail == 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;

#[test]
fn network_lane_names_every_import_after_the_command_imports_and_exports_its_marker() {
    let program =
        crate::hir::resolve(&crate::parse(SOURCE, Path::new("network-wasm.spx")).unwrap()).unwrap();
    let wasm =
        super::emit_resolved_language_network_io_v1(&program, "test.network_wasm.run").unwrap();
    assert_eq!(
        wasm,
        super::emit_resolved_language_network_io_v1(&program, "test.network_wasm.run").unwrap()
    );
    let mut cursor = 0;
    for name in [
        "spx_command_owned_bytes_validate_v1",
        "spx_network_connect_v1",
        "spx_network_send_v1",
        "spx_network_recv_v1",
        "spx_network_stream_stdout_v1",
        "spx_network_wait_v1",
        "spx_network_close_v1",
        "spx_network_settle_v1",
    ] {
        let position = wasm[cursor..]
            .windows(name.len())
            .position(|window| window == name.as_bytes())
            .unwrap_or_else(|| panic!("{name} is absent or out of order"));
        cursor += position + name.len();
    }
    assert!(contains_bytes(&wasm, super::STATUS_EXPORT.as_bytes()));
    assert!(contains_bytes(
        &wasm,
        super::super::command_io::INPUT_STATUS_EXPORT.as_bytes()
    ));
    assert!(!contains_bytes(
        &wasm,
        super::super::line_command_io::OUTPUT_STATUS_EXPORT.as_bytes()
    ));
}

#[test]
fn command_lanes_reject_network_programs_and_emit_no_network_import() {
    let program =
        crate::hir::resolve(&crate::parse(SOURCE, Path::new("network-wasm-lane.spx")).unwrap())
            .unwrap();
    for lane in [
        super::super::emit_resolved_language_command_io_v1,
        super::super::emit_resolved_line_command_io_v1,
    ] {
        let error = lane(&program, "test.network_wasm.run")
            .expect_err("command lanes must not lower network operations");
        assert_eq!(error.code, "SPX-W114", "{error:?}");
    }

    let legacy = r#"
module test.network_wasm_legacy;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("test.network_wasm_legacy.run")
fn run() -> bool uses { process.stdin.read, process.stdout.write } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    stdout_write(view) == byte_len(view)
}
@id("main") fn main() -> i64 { 0 }
"#;
    let legacy_program =
        crate::hir::resolve(&crate::parse(legacy, Path::new("network-wasm-legacy.spx")).unwrap())
            .unwrap();
    let wasm = super::super::emit_resolved_language_command_io_v1(
        &legacy_program,
        "test.network_wasm_legacy.run",
    )
    .unwrap();
    assert!(!contains_bytes(&wasm, b"spx_network_"));
    assert!(!contains_bytes(&wasm, super::STATUS_EXPORT.as_bytes()));
    let rejected = super::emit_resolved_language_network_io_v1(
        &legacy_program,
        "test.network_wasm_legacy.run",
    )
    .expect_err("the network lane requires a network permit");
    assert_eq!(rejected.code, "SPX-W114", "{rejected:?}");
}

#[test]
fn network_permits_must_stay_within_the_admitted_seven() {
    let widened = SOURCE.replace(
        "permit { network.connect, network.read, network.write, process.stdout.write }",
        "permit { network.connect, network.read, network.write, process.stdout.write, fs.read }",
    );
    if let Ok(parsed) = crate::parse(&widened, Path::new("network-wasm-widened.spx")) {
        if let Ok(program) = crate::hir::resolve(&parsed) {
            let error =
                super::emit_resolved_language_network_io_v1(&program, "test.network_wasm.run")
                    .expect_err("a foreign permit must fail admission");
            assert_eq!(error.code, "SPX-W114", "{error:?}");
        }
    }
    assert!(super::check_permits(&["fs.read".to_owned()]).is_err());
    assert!(super::check_permits(&["process.stdout.write".to_owned()]).is_err());
    assert!(super::check_permits(&["network.read".to_owned()]).is_ok());
}
