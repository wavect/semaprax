//! Generated C callback ABI coverage for Filesystem I/O v2.

use std::path::Path;
use std::process::Command;

use semaprax::{codegen, hir, parse, verify};

const SOURCE: &str = r#"
module test.filesystem_v2_native;
permit { fs.read, fs.write }
@id("filesystem-v2-native.run")
fn run() -> bool uses { fs.read, fs.write } {
    let root = [0u8];
    let directory = [100u8];
    let file = [100u8, 47u8, 102u8];
    let data = [80u8, 73u8, 78u8, 71u8];
    let made = file_create_dir(array_as_slice(directory), 1usize);
    let listed = file_list(array_as_slice(root), 0usize, 8usize);
    let first = file_write_new(array_as_slice(file), 3usize, array_as_slice(data), 4usize);
    let replaced = file_write_atomic(array_as_slice(file), 3usize, array_as_slice(data), 4usize);
    let metadata = file_stat(array_as_slice(file), 3usize);
    let read = file_read(array_as_slice(file), 3usize, 4usize);
    let removed_file = file_remove(array_as_slice(file), 3usize);
    let removed_directory = file_remove(array_as_slice(directory), 1usize);
    made == 0usize && byte_len(bytes_as_slice(listed)) == 2usize && first == 4usize &&
        replaced == 4usize && metadata == 17usize && byte_len(bytes_as_slice(read)) == 4usize &&
        removed_file == 0usize && removed_directory == 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;

const HARNESS: &str = r#"
static uint32_t read_cb(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t cap, uint64_t *out) { uint64_t *n=p; ++*n; if(length!=3||cap!=4||!data||!out||path.ptr[0]!=100)return 5; memcpy(data,"PING",4);*out=4;return 0; }
static uint32_t write_cb(void *p, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t size, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=3||size!=4||data.ptr[0]!=80)return 5;*out=4;return 0; }
static uint32_t stat_cb(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=3)return 5;*out=17;return 0; }
static uint32_t list_cb(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t cap, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length||cap<2||!data)return 5;data[0]=100;data[1]=0;*out=2;return 0; }
static uint32_t mkdir_cb(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=1)return 5;*out=0;return 0; }
static uint32_t remove_cb(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=1&&length!=3)return 5;*out=0;return 0; }
static uint32_t atomic_cb(void *p, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t size, uint64_t *out) { return write_cb(p,path,length,data,size,out); }
static void settle_cb(void *p) { ++*(uint64_t *)p; }
int main(void) { uint64_t calls=0; struct spx_filesystem_callbacks_v2 cb={.context=&calls,.read=read_cb,.write_new=write_cb,.stat=stat_cb,.list=list_cb,.create_dir=mkdir_cb,.remove=remove_cb,.write_atomic=atomic_cb,.settle=settle_cb}; struct spx_filesystem_command_result_v2 result; return spx_run_filesystem_command_v2(&cb,&result)==1&&result.semantic_success&&result.matched&&calls==9?0:1; }
"#;

const MALFORMED_LIST_HARNESS: &str = r#"
static uint32_t bad_list(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t cap, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length||cap<2)return 5;data[0]=100;data[1]=101;*out=2;return 0; }
static uint32_t mkdir_ok(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=1)return 5;*out=0;return 0; }
static void settle_bad(void *p) { ++*(uint64_t *)p; }
int main(void) { uint64_t calls=0; struct spx_filesystem_callbacks_v2 cb={.context=&calls,.list=bad_list,.create_dir=mkdir_ok,.settle=settle_bad}; struct spx_filesystem_command_result_v2 result; return spx_run_filesystem_command_v2(&cb,&result)==1&&!result.semantic_success&&result.status_code==5&&calls==3?0:1; }
"#;

const ROOT_NULL_HARNESS: &str = r#"
static uint32_t root_stat(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { ++*(uint64_t *)p; if(path.ptr!=NULL||length!=0)return 5;*out=2;return 0; }
static uint32_t root_list(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t capacity, uint64_t *out) { ++*(uint64_t *)p; if(path.ptr!=NULL||length!=0||data!=NULL||capacity!=0)return 5;*out=0;return 0; }
int main(void) { uint64_t calls=0,stat=0;spx_bytes_v1 list={0};struct spx_filesystem_callbacks_v2 cb={.context=&calls,.stat=root_stat,.list=root_list};struct spx_filesystem_command_state_v2 state={.callbacks=&cb};struct spx_status_entry entries[1];struct spx_context context={0};spx_slice_u8_v1 root={.ptr=NULL,.len=0};if(!spx_context_init(&context,1,entries,1,NULL,NULL,&state))return 1;if(spx_host_file_list_v2(&context,root,0,0,&list)!=0||list.ptr!=NULL||list.len!=0)return 1;if(spx_host_file_stat_v2(&context,root,0,&stat)!=0||stat!=2)return 1;return calls==2?0:1; }
"#;

// Issue #228 regression: pins today's collapse of a post-publication
// `file_write_atomic` failure into the same undifferentiated abort shape as
// any other filesystem failure. `create_dir` and `write_new` succeed, then
// `write_atomic` reports failure the same way a real provider would report
// "the rename step itself failed, outcome unknown" -- and the generated
// carrier gives the caller no way to tell that apart from `bad_atomic`
// failing before touching anything: both surface as the identical
// `!semantic_success && status_code==5`, and no later operation (`file_stat`,
// `file_read`, `file_remove`) ever runs, matching the "aborts the enclosing
// invocation on failure instead of returning an inspectable value" rule
// `docs/DURABLE-JOBS-V1.md`'s "#228 boundary" section names. This is exactly
// the gap `docs/HOST-OPERATION-OUTCOME-V1.md` designs a fix for and does not
// yet lower; a future value-typed `write_atomic` outcome must change this
// test's expectations deliberately, not by accident.
const FAILING_ATOMIC_HARNESS: &str = r#"
static uint32_t mkdir_ok2(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=1)return 5;*out=0;return 0; }
static uint32_t list_ok2(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t cap, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length||cap<2||!data)return 5;data[0]=100;data[1]=0;*out=2;return 0; }
static uint32_t write_ok2(void *p, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t size, uint64_t *out) { uint64_t *n=p;++*n;(void)path;if(length!=3||size!=4||data.ptr[0]!=80)return 5;*out=4;return 0; }
static uint32_t bad_atomic(void *p, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t size, uint64_t *out) { uint64_t *n=p;++*n;(void)path;(void)length;(void)data;(void)size;(void)out;return 5; }
static uint32_t stat_unreached(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { (void)path;(void)length;(void)out;++*(uint64_t *)p;return 0; }
static uint32_t read_unreached(void *p, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t cap, uint64_t *out) { (void)path;(void)length;(void)data;(void)cap;(void)out;++*(uint64_t *)p;return 0; }
static uint32_t remove_unreached(void *p, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) { (void)path;(void)length;(void)out;++*(uint64_t *)p;return 0; }
static void settle_bad_atomic(void *p) { ++*(uint64_t *)p; }
int main(void) { uint64_t calls=0; struct spx_filesystem_callbacks_v2 cb={.context=&calls,.read=read_unreached,.write_new=write_ok2,.stat=stat_unreached,.list=list_ok2,.create_dir=mkdir_ok2,.remove=remove_unreached,.write_atomic=bad_atomic,.settle=settle_bad_atomic}; struct spx_filesystem_command_result_v2 result; return spx_run_filesystem_command_v2(&cb,&result)==1&&!result.semantic_success&&result.status_code==5&&calls==5?0:1; }
"#;

fn generated() -> String {
    let ast = parse(SOURCE, Path::new("filesystem-v2-native.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    codegen::emit_hir_c_with_filesystem_io_v2(
        &hir::resolve(&ast).unwrap(),
        "filesystem-v2-native.run",
    )
    .unwrap()
}

#[test]
fn filesystem_v2_generated_c_runs_all_callbacks_and_settles() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let source = std::env::temp_dir().join(format!("spx-fsv2-{}.c", std::process::id()));
    let executable = source.with_extension("bin");
    std::fs::write(&source, format!("{}\n{}", generated(), HARNESS)).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(&executable).status().unwrap().success());
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
}

#[test]
fn filesystem_v2_rejects_malformed_list_wire_before_publication() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let generated = generated();
    assert!(generated.contains("spx_filesystem_list_is_shaped_v2"));
    assert!(generated.contains("spx_run_filesystem_command_v2"));
    assert!(!generated.contains("openat("));
    let source = std::env::temp_dir().join(format!("spx-fsv2-bad-{}.c", std::process::id()));
    let executable = source.with_extension("bin");
    std::fs::write(&source, format!("{generated}\n{MALFORMED_LIST_HARNESS}")).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(&executable).status().unwrap().success());
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
}

#[test]
fn filesystem_v2_accepts_null_empty_carrier_only_for_root_list_and_stat() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let source = std::env::temp_dir().join(format!("spx-fsv2-root-{}.c", std::process::id()));
    let executable = source.with_extension("bin");
    std::fs::write(&source, format!("{}\n{}", generated(), ROOT_NULL_HARNESS)).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(&executable).status().unwrap().success());
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
}

/// Issue #228 regression, see `FAILING_ATOMIC_HARNESS`: a `write_atomic`
/// failure that stands in for "the atomic replace step itself failed,
/// outcome unknown" collapses into the exact same `!semantic_success &&
/// status_code == 5` shape as any ordinary pre-publication validation
/// failure, and no operation issued after it ever runs. Today's generated
/// carrier has no third status class and no way to hand a checked SEMAPRAX
/// caller anything more than "the invocation aborted" -- exactly the gap
/// `docs/HOST-OPERATION-OUTCOME-V1.md` records and does not yet lower.
#[test]
fn filesystem_v2_write_atomic_failure_collapses_into_undifferentiated_abort() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let generated = generated();
    let source =
        std::env::temp_dir().join(format!("spx-fsv2-atomic-fail-{}.c", std::process::id()));
    let executable = source.with_extension("bin");
    std::fs::write(&source, format!("{generated}\n{FAILING_ATOMIC_HARNESS}")).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(&executable).status().unwrap().success());
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
}
