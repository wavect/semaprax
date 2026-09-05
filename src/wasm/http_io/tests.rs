use std::path::Path;

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

const SOURCE: &str = r#"
module test.https_wasm;
permit { network.http, process.stdout.write }
@id("test.https_wasm.run")
fn run() -> bool uses { network.http, process.stdout.write } {
    let url = [104u8, 116u8, 116u8, 112u8, 115u8, 58u8, 47u8, 47u8, 101u8, 120u8, 97u8, 109u8, 112u8, 108u8, 101u8, 46u8, 116u8, 101u8, 115u8, 116u8, 47u8];
    let response = https_get(array_as_slice(url), 1024usize);
    stdout_append(bytes_as_slice(response)) > 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;

#[test]
fn https_lane_names_its_single_import_and_status_marker() {
    let program =
        crate::hir::resolve(&crate::parse(SOURCE, Path::new("https-wasm.spx")).unwrap()).unwrap();
    let wasm = super::emit_resolved_https_command_io_v1(&program, "test.https_wasm.run").unwrap();
    assert_eq!(
        wasm,
        super::emit_resolved_https_command_io_v1(&program, "test.https_wasm.run").unwrap()
    );
    assert!(contains_bytes(&wasm, super::IMPORT_NAME.as_bytes()));
    assert!(contains_bytes(&wasm, super::STATUS_EXPORT.as_bytes()));
    assert!(contains_bytes(
        &wasm,
        super::super::command_io::INPUT_STATUS_EXPORT.as_bytes()
    ));
    assert!(!contains_bytes(&wasm, b"spx_network_connect_v1"));
    assert!(!contains_bytes(&wasm, b"__spx_network_status_v1"));
}

#[test]
fn https_lane_requires_the_exact_http_permit_family() {
    assert!(super::check_permits(&["network.http".to_owned()]).is_ok());
    assert!(super::check_permits(&["network.connect".to_owned()]).is_err());
    assert!(super::check_permits(&["network.http".to_owned(), "network.tls".to_owned()]).is_err());

    let program =
        crate::hir::resolve(&crate::parse(SOURCE, Path::new("https-wasm-lanes.spx")).unwrap())
            .unwrap();
    for lane in [
        super::super::emit_resolved_language_command_io_v1,
        super::super::emit_resolved_line_command_io_v1,
        super::super::emit_resolved_language_network_io_v1,
    ] {
        let error = lane(&program, "test.https_wasm.run").unwrap_err();
        assert_eq!(error.code, "SPX-W114");
    }
}
