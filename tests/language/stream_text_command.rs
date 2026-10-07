//! Explicit stream-text profile: private String boundaries, settlement, and refusals.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::{codegen, format, graph, hir, parse, verify};
use std::path::Path;

pub(super) const SOURCE: &str = r#"module stream.text;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("text.cut")
fn cut(text: string, end: i64) -> string { string_slice(text, 0, end) }
@id("text.keep")
fn keep(text: string) -> string { text }
@id("text.clone")
fn clone_text(text: borrow str) -> string { string_from_str(text) }
@id("stream.command")
fn command() -> i64 uses { process.args.read, process.stdin.read, process.stdout.write } {
    let map_count = { let mut values = map_new(1usize); values = map_set(values, "count", 3); map_get_or(values, "count", 0) };
    let mut reader = stdin_stream_open();
    let mut total = 0;
    while !stdin_stream_eof(reader) {
        let chunk_size = { let chunk = stdin_stream_chunk(reader); byte_len(chunk) };
        let text = keep("é\u{0}");
        let copy = { let view = string_as_str(text); clone_text(view) };
        let end = if args_len() == 0usize { 3 } else { 4 };
        let piece = cut(copy, end);
        total = total + string_len(piece);
        reader = stdin_stream_next(reader);
        0
    }
    let output = keep("é\u{0}");
    let view = string_as_str(output);
    let written = stdout_write(str_as_bytes(view));
    if total == map_count { 0 } else { 1 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn stream_text_has_exact_projection_and_old_native_profile_refusal() {
    let ast = parse(SOURCE, Path::new("stream-text.spx")).unwrap();
    assert!(verify::verify(&ast).is_empty());
    let canonical = format::canonical(&ast);
    let reparsed = parse(&canonical, Path::new("stream-text.spx")).unwrap();
    assert_eq!(
        graph::to_json(&ast).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(
        codegen::emit_hir_c_with_stdin_stream_exit_status(&program, "stream.command")
            .unwrap_err()
            .code,
        "SPX-B103"
    );
    let emitted = codegen::emit_hir_c_with_stdin_stream_text(&program, "stream.command").unwrap();
    assert_eq!(
        emitted,
        codegen::emit_hir_c_with_stdin_stream_text(&program, "stream.command").unwrap()
    );
    assert!(emitted.contains("spx_string_length_v10"));
    assert!(emitted.contains("spx_language_command_stream_run_v2"));
    assert!(!emitted.contains("spx_host_file_read_text"));
    let fs_source = SOURCE.replace(
        "permit { process.args.read",
        "permit { fs.read, process.args.read",
    );
    let fs_ast = parse(&fs_source, Path::new("filesystem.spx")).unwrap();
    let fs_program = hir::resolve(&fs_ast).unwrap();
    assert_eq!(
        codegen::emit_hir_c_with_stdin_stream_text(&fs_program, "stream.command")
            .unwrap_err()
            .code,
        "SPX-B103"
    );
}

#[test]
fn stream_text_native_owned_calls_and_text_failure_settle_at_o0_and_o2() {
    if std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let ast = parse(SOURCE, Path::new("stream-text.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();
    let generated = codegen::emit_hir_c_with_stdin_stream_text(&program, "stream.command").unwrap();
    let probe=format!("{}\n#define main spx_process_main\n{generated}\n#undef main\n#undef malloc\n#undef free\n{}", include_str!("../native_owned_utf8_settlement_v1/allocations.c"), PROBE);
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), "ok");
    }
    fixture.cleanup();
}

const PROBE: &str = r#"
struct fixture_input { unsigned opens, reads, drops, settles; };
static uint32_t fixture_open(void *context, uintptr_t *token) {
    struct fixture_input *f=context; ++f->opens; *token=1; return 0;
}
static uint32_t fixture_read(void *context, uintptr_t token, uint8_t *buffer, uint32_t capacity, uint32_t *length, uint32_t *eof) {
    struct fixture_input *f=context; REQUIRE(token==1 && capacity==4096);
    ++f->reads; *eof=f->reads==1?0:1; *length=*eof?0:1; if(*length) buffer[0]='x'; return 0;
}
static void fixture_drop(void *context, uintptr_t token) {
    struct fixture_input *f=context; REQUIRE(token==1); ++f->drops;
}
static void fixture_settle(void *context) { ++((struct fixture_input*)context)->settles; }
int main(void) {
    for(unsigned bad=0;bad<2;++bad) {
        struct fixture_input state={0};
        const struct spx_stdin_stream_provider_v1 provider={&state,fixture_open,fixture_read,fixture_drop,fixture_settle};
        struct spx_language_command_input_v1 input={0};
        input.argument_count=bad;
        if(bad) input.arguments[0]=(spx_str_v1){ .data=(const uint8_t*)"bad", .len=3 };
        struct spx_language_command_stream_result_v2 result={0};
        REQUIRE(spx_language_command_stream_run_v2(&input,&provider,&result));
        REQUIRE(state.opens==1 && state.drops==1 && state.settles==1);
        if(!bad) {
            REQUIRE(result.semantic_success && result.application_status==0);
            REQUIRE(result.stdout_length==3 && result.stderr_length==0);
            REQUIRE(memcmp(result.stdout_bytes,"\xc3\xa9\0",3)==0);
            REQUIRE(state.reads==2);
        } else {
            REQUIRE(!result.semantic_success && result.status_code==1);
            REQUIRE(strcmp(result.status_domain,"semaprax.text.v1")==0);
            REQUIRE(result.stdout_length==0 && result.stderr_length==0);
            REQUIRE(state.reads==1);
        }
        REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    }
    /* Keep the probe marker independent of Windows CRT newline translation. */
    REQUIRE(fwrite("ok", 1, 2, stdout)==2); return 0;
}
"#;
