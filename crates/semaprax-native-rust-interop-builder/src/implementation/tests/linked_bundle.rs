//! End-to-end private phase-B builds and the linked Rust bridge round
//! trips they produce.

use super::*;

#[test]
fn private_b_builds_exact_static_inventory_without_clobber() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-interop-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("semaprax_native_rust_interop.h"),
        &prepared.generated_header,
    )
    .unwrap();
    std::fs::write(root.join("module.c"), &prepared.generated_c).unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let probe_object = if cfg!(windows) {
        "probe.obj"
    } else {
        "probe.o"
    };
    let mut probe = Command::new(&clang.path);
    probe.env_clear().current_dir(&root).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-c",
        "module.c",
        "-o",
        probe_object,
    ]);
    bind_test_tool_environment(&mut probe);
    let probe = probe.output().unwrap();
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let output = root.join("bundle");
    let facts = build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();
    assert_eq!(facts.output_directory, output);
    assert!(facts.object_path.is_file());
    assert!(facts.descriptor_path.is_file());
    assert!(facts.manifest_path.is_file());
    assert!(facts.manifest_digest.starts_with("sha256:"));
    let manifest = std::fs::read_to_string(&facts.manifest_path).unwrap();
    assert!(manifest.ends_with('\n'));
    assert_eq!(
        domain_digest(BUNDLE_DIGEST_DOMAIN, manifest.as_bytes()),
        facts.manifest_digest
    );
    let value: Value = serde_json::from_str(&manifest).unwrap();
    let row = value.as_object().unwrap();
    assert_eq!(row.len(), 6);
    assert_eq!(
        row.get("schema").and_then(Value::as_str),
        Some(BUNDLE_SCHEMA)
    );
    let descriptor = row.get("descriptor").and_then(Value::as_object).unwrap();
    assert_eq!(descriptor.len(), 3);
    assert_eq!(
        descriptor.get("schema").and_then(Value::as_str),
        Some(DESCRIPTOR_SCHEMA)
    );
    assert_eq!(
        descriptor.get("digest").and_then(Value::as_str),
        Some(prepared.descriptor_digest.as_str())
    );
    assert_eq!(
        descriptor.get("bytes").and_then(Value::as_u64),
        u64::try_from(prepared.descriptor.len()).ok()
    );
    let files = row.get("files").and_then(Value::as_array).unwrap();
    let paths = files
        .iter()
        .map(|file| file.get("path").and_then(Value::as_str).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "descriptor.json",
            "module.c",
            if cfg!(windows) {
                "module.obj"
            } else {
                "module.o"
            },
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ]
    );
    for file in files {
        let file = file.as_object().unwrap();
        assert_eq!(file.len(), 3);
        let path = file.get("path").and_then(Value::as_str).unwrap();
        let bytes = std::fs::read(output.join(path)).unwrap();
        let digest = raw_digest(&bytes);
        assert_eq!(
            file.get("bytes").and_then(Value::as_u64),
            u64::try_from(bytes.len()).ok()
        );
        assert_eq!(
            file.get("sha256").and_then(Value::as_str),
            Some(digest.as_str())
        );
    }
    let toolchain = row.get("toolchain").and_then(Value::as_object).unwrap();
    assert_eq!(
        toolchain.get("target").and_then(Value::as_str),
        Some(prepared.target.triple.as_str())
    );
    assert_eq!(
        row.get("nonclaims")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect::<Vec<_>>(),
        NONCLAIMS
    );
    let retry = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &output) {
        Ok(_) => panic!("existing output was overwritten"),
        Err(error) => error,
    };
    assert_eq!(retry[0].code, "SPX-I232");

    let foreign_file = root.join("foreign-file");
    std::fs::write(&foreign_file, b"foreign-file-sentinel").unwrap();
    let error = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_file) {
        Ok(_) => panic!("foreign file was overwritten"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        std::fs::read(&foreign_file).unwrap(),
        b"foreign-file-sentinel"
    );

    let foreign_directory = root.join("foreign-directory");
    std::fs::create_dir(&foreign_directory).unwrap();
    let sentinel = foreign_directory.join("sentinel");
    std::fs::write(&sentinel, b"foreign-directory-sentinel").unwrap();
    let error =
        match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_directory) {
            Ok(_) => panic!("foreign directory was overwritten"),
            Err(error) => error,
        };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"foreign-directory-sentinel"
    );

    #[cfg(unix)]
    {
        let foreign_target = root.join("foreign-symlink-target");
        std::fs::write(&foreign_target, b"foreign-symlink-sentinel").unwrap();
        let foreign_link = root.join("foreign-symlink");
        std::os::unix::fs::symlink(&foreign_target, &foreign_link).unwrap();
        let error = match build_native_rust_interop_bundle(&program, spec.as_bytes(), &foreign_link)
        {
            Ok(_) => panic!("foreign symlink was followed"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(
            std::fs::read(&foreign_target).unwrap(),
            b"foreign-symlink-sentinel"
        );
        assert!(std::fs::symlink_metadata(&foreign_link)
            .unwrap()
            .file_type()
            .is_symlink());
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn held_file_matching_rejects_symlink_identity_and_permission_drift() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-held-file-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let target = root.join("target");
    let link = root.join("link");
    std::fs::write(&target, b"authenticated-bytes").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let error = match_regular_file(&link, b"authenticated-bytes").unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"authenticated-bytes");

    let permissions = std::fs::metadata(&target).unwrap().permissions();
    let mut denied = permissions.clone();
    denied.set_mode(0o0);
    std::fs::set_permissions(&target, denied).unwrap();
    let result = match_regular_file(&target, b"authenticated-bytes");
    std::fs::set_permissions(&target, permissions).unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn held_stage_rejects_same_path_directory_and_reparse_substitution() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-held-stage-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();

    let parent = hold_stage(root.clone()).unwrap();
    let slot = StageSlot::new(&root, "sha256:held-stage", "identity").unwrap();
    let inventory = platform::prepare_discard_inventory([]).unwrap();
    let stage = create_stage(&parent, slot, &inventory).unwrap();
    stage.recheck().unwrap();
    let displaced = root.join("displaced-stage");
    std::fs::rename(&stage.path, &displaced).unwrap();
    std::fs::create_dir(&stage.path).unwrap();
    let error = stage.recheck().unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert!(displaced.is_dir());

    std::fs::remove_dir(&stage.path).unwrap();
    std::os::unix::fs::symlink(&displaced, &stage.path).unwrap();
    let error = stage.recheck().unwrap_err();
    assert_eq!(error.code, "SPX-I232");
    assert_eq!(
        error.message,
        "Native Rust Interop output publication failed"
    );
    assert!(std::fs::symlink_metadata(&stage.path)
        .unwrap()
        .file_type()
        .is_symlink());

    std::fs::remove_file(&stage.path).unwrap();
    std::fs::remove_dir_all(&displaced).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn linked_bridge_round_trips_rust_to_semaprax_to_rust_and_closes_failures() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let parsed_spec = parse_spec(&program, spec.as_bytes()).unwrap();
    let export = &prepared.exports[0];
    let import = &prepared.imports[0];
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-roundtrip-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();

    let harness = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use core::num::NonZeroU32;
use semaprax_native_rust_interop::*;
struct Host{{mode:u8,panicked:bool}}
impl NativeRustImports for Host{{
fn {import_method}(&mut self,arg_0:i64,_arg_1:i64)->NativeRustImportResult<i64>{{
match self.mode{{
0=>NativeRustImportResult::Success(arg_0),
1=>NativeRustImportResult::Status{{code:NonZeroU32::new(7).unwrap(),class:NativeRustStatusClass::Import,retryable:true}},
2=>NativeRustImportResult::HostFailure,
6=>if self.panicked{{NativeRustImportResult::Success(arg_0)}}else{{self.panicked=true;panic!("panic once")}},
4=>NativeRustImportResult::Status{{code:NonZeroU32::new(7).unwrap(),class:NativeRustStatusClass::Semantic,retryable:false}},
_=>panic!("private sentinel must not cross the FFI boundary")
}}
}}
}}
fn bridge(mode:u8)->NativeRustBridge<Host>{{
let caps=NativeRustCapabilities::new(&["host.math"]).unwrap_or_else(|_|std::process::exit(10));
NativeRustBridge::new(Host{{mode,panicked:false}},caps)
}}
fn main(){{
std::panic::set_hook(Box::new(|_|{{}}));
if NativeRustCapabilities::new(&["wrong.capability"]).is_ok(){{std::process::exit(11)}}
let mut success=bridge(0);
match success.{export_method}(20,22){{Ok(42)=>{{}},_=>std::process::exit(12)}}
let mut semantic=bridge(0);
match semantic.{export_method}(i64::MAX,1){{
Err(NativeRustCallError::Semantic{{domain_id:"semaprax.native-rust-semantics.v1",code,class:NativeRustStatusClass::Semantic,retryable:false}}) if code.get()==2=>{{}},
_=>std::process::exit(19)
}}
let mut status=bridge(1);
match status.{export_method}(1,2){{
Err(NativeRustCallError::Semantic{{domain_id:"host.math.v1",code,class:NativeRustStatusClass::Import,retryable:true}}) if code.get()==7=>{{}},
_=>std::process::exit(13)
}}
let mut failed=bridge(2);
match failed.{export_method}(1,2){{Err(NativeRustCallError::HostFailed)=>{{}},_=>std::process::exit(14)}}
let mut panicked=bridge(3);
match panicked.{export_method}(1,2){{Err(NativeRustCallError::HostPanicked)=>{{}},_=>std::process::exit(15)}}
let mut panic_once=bridge(6);match panic_once.{export_method}(1,2){{Err(NativeRustCallError::HostPanicked)=>{{}},_=>std::process::exit(23)}}match panic_once.{export_method}(1,2){{Ok(3)=>{{}},_=>std::process::exit(24)}}
let mut wrong_class=bridge(4);
match wrong_class.{export_method}(1,2){{Err(NativeRustCallError::AdapterRejected)=>{{}},_=>std::process::exit(18)}}
let mut bounded=bridge(0);
for _ in 0..2048{{if !matches!(bounded.{export_method}(1,2),Ok(3)){{std::process::exit(16)}}}}
match bounded.{export_method}(1,2){{Err(NativeRustCallError::AdapterRejected)=>{{}},_=>std::process::exit(17)}}
}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    let active = prepared
            .generated_rust
            .find("||core::mem::replace(&mut self.active,true){return Err(NativeRustCallError::AdapterRejected)}")
            .unwrap();
    let effect = prepared.generated_rust[active..]
        .find("super::ffi::")
        .unwrap();
    assert!(
        effect > 0,
        "reentry must reject before allocating an FFI result slot or performing an import effect"
    );
    assert!(prepared
        .generated_rust
        .contains("impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}"));
    let harness_path = output.join("roundtrip.rs");
    std::fs::write(&harness_path, harness).unwrap();
    let executable = if cfg!(windows) {
        "roundtrip.exe"
    } else {
        "roundtrip"
    };
    let object = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let rustc = configured_tool("RUSTC").unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let sanitizers = sanitizer_mode().unwrap();
    let o0_object = if cfg!(windows) {
        "module_hostile_O0.obj"
    } else {
        "module_hostile_O0.o"
    };
    let mut o0_compile = Command::new(&clang.path);
    o0_compile.env_clear().current_dir(&output).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O0",
        "-c",
        "module.c",
        "-o",
        o0_object,
    ]);
    bind_test_tool_environment(&mut o0_compile);
    if sanitizers {
        o0_compile.args(REQUIRED_NATIVE_RUST_SANITIZER_FLAGS);
    }
    assert!(o0_compile.status().unwrap().success());
    for (linked_object, linked_executable) in [(o0_object, "roundtrip_O0"), (object, executable)] {
        let mut roundtrip_compile = Command::new(&rustc.path);
        roundtrip_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "-C",
            "panic=unwind",
            "-C",
            &format!("link-arg={linked_object}"),
            "roundtrip.rs",
            "-o",
            linked_executable,
        ]);
        bind_test_tool_environment(&mut roundtrip_compile);
        bind_test_rust_linker(&mut roundtrip_compile, &clang);
        if sanitizers {
            roundtrip_compile.args([
                "-C",
                "link-arg=-fsanitize=address,undefined",
                "-C",
                "link-arg=-fno-sanitize-recover=all",
            ]);
        }
        assert!(roundtrip_compile.status().unwrap().success());
        let mut roundtrip_run = Command::new(output.join(linked_executable));
        roundtrip_run.env_clear().current_dir(&output);
        if sanitizers {
            roundtrip_run
                .env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        assert!(roundtrip_run.status().unwrap().success());
    }

    let capability_hex = capability_digest(&parsed_spec.capabilities)
        .strip_prefix("sha256:")
        .unwrap()
        .to_owned();
    let capability_bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    let abi_harness = format!(
        r#"#![allow(unsafe_code)]
use core::ffi::c_void;
type Callback=unsafe extern "C" fn(*mut c_void,i64,i64,*mut i64)->u64;
#[repr(C)]struct Imports{{abi_version:u32,size:u32,callback:Option<Callback>}}
#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}
unsafe extern "C"{{fn {export_symbol}(ctx:*const Context,arg_0:i64,arg_1:i64,result_out:*mut i64)->u64;}}
unsafe extern "C" fn callback(userdata:*mut c_void,left:i64,_right:i64,out:*mut i64)->u64{{
let injected=if userdata.is_null(){{0}}else{{unsafe{{*(userdata.cast::<u64>())}}}};
if injected!=0{{return injected}}unsafe{{*out=left}};0}}
fn adapter(code:u64)->u64{{(65535u64<<48)|(4u64<<32)|code}}
fn status(domain:u64,class:u64,retry:u64,code:u64)->u64{{(domain<<48)|(retry<<40)|(class<<32)|code}}
macro_rules! rejected{{($context:expr,$wire:expr)=>{{let mut poisoned=0x5a5a_6b6b_7c7c_8d8di64;assert_eq!({export_symbol}($context,1,2,&mut poisoned),$wire);assert_eq!(poisoned,0x5a5a_6b6b_7c7c_8d8di64);}}}}
fn main(){{unsafe{{
let imports=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};
let mut context=Context{{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&imports,capabilities_digest:[{capability_bytes}],call_depth:0,reserved:0}};
let mut out=0i64;
assert_eq!({export_symbol}(&context,20,22,&mut out),0);assert_eq!(out,42);
rejected!(core::ptr::null(),adapter(1));
context.abi_version=2;rejected!(&context,adapter(1));context.abi_version=1;
context.size=0;rejected!(&context,adapter(1));context.size=core::mem::size_of::<Context>() as u32;
context.reserved=1;rejected!(&context,adapter(1));context.reserved=0;
context.imports=core::ptr::null();rejected!(&context,adapter(2));context.imports=&imports;
let bad_imports=Imports{{abi_version:2,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};context.imports=&bad_imports;rejected!(&context,adapter(2));context.imports=&imports;
let missing_callback=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:None}};context.imports=&missing_callback;rejected!(&context,adapter(2));context.imports=&imports;
context.capabilities_digest[0]^=1;rejected!(&context,adapter(3));context.capabilities_digest[0]^=1;
context.call_depth=31;assert_eq!({export_symbol}(&context,1,2,&mut out),0);assert_eq!(out,3);
context.call_depth=32;rejected!(&context,adapter(7));context.call_depth=0;
let mut injected=status(65534,4,0,1);context.userdata=(&mut injected as *mut u64).cast();rejected!(&context,injected);
injected=status(65534,4,0,2);rejected!(&context,injected);
injected=status(65535,4,0,3);rejected!(&context,injected);
for forged in [status(65534,4,0,0),status(65534,4,0,3),status(65534,3,0,1),status(65534,4,1,1),status(65535,4,0,0),status(65535,4,0,9),status(65535,3,0,3),status(65535,4,1,3),status(65535,4,0,3)|(1u64<<41),status(0,4,0,1)]{{injected=forged;context.userdata=core::hint::black_box((&mut injected as *mut u64).cast());rejected!(&context,adapter(8));}}
context.userdata=core::ptr::null_mut();
assert_eq!({export_symbol}(&context,1,2,core::ptr::null_mut()),adapter(5));
let mut result_bytes=[0x5au8;16];let before_result_bytes=result_bytes;let misaligned=result_bytes.as_mut_ptr().add(1).cast::<i64>();assert_eq!({export_symbol}(&context,1,2,misaligned),adapter(5));assert_eq!(result_bytes,before_result_bytes);
let mut context_bytes=[0u8;128];let misaligned_context=context_bytes.as_mut_ptr().add(1).cast::<Context>();rejected!(misaligned_context,adapter(1));
}}}}
"#,
        export_symbol = export.c_symbol,
        capability_bytes = capability_bytes,
    );
    std::fs::write(output.join("abi_hostile.rs"), abi_harness).unwrap();
    let abi_executable = if cfg!(windows) {
        "abi_hostile.exe"
    } else {
        "abi_hostile"
    };
    for (linked_object, linked_executable) in
        [(o0_object, "abi_hostile_O0"), (object, abi_executable)]
    {
        let mut abi_compile = Command::new(&rustc.path);
        abi_compile.env_clear().current_dir(&output).args([
            "--edition=2021",
            "-C",
            "panic=abort",
            "-C",
            &format!("link-arg={linked_object}"),
            "abi_hostile.rs",
            "-o",
            linked_executable,
        ]);
        bind_test_tool_environment(&mut abi_compile);
        bind_test_rust_linker(&mut abi_compile, &clang);
        if sanitizers {
            abi_compile.args([
                "-C",
                "link-arg=-fsanitize=address,undefined",
                "-C",
                "link-arg=-fno-sanitize-recover=all",
            ]);
        }
        assert!(abi_compile.status().unwrap().success());
        let mut abi_run = Command::new(output.join(linked_executable));
        abi_run.env_clear().current_dir(&output);
        if sanitizers {
            abi_run
                .env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        assert!(abi_run.status().unwrap().success());
    }

    let cross_thread = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use semaprax_native_rust_interop::*;
struct Host;
impl NativeRustImports for Host{{fn {import_method}(&mut self,left:i64,_right:i64)->NativeRustImportResult<i64>{{NativeRustImportResult::Success(left)}}}}
fn main(){{let caps=NativeRustCapabilities::new(&["host.math"]).unwrap_or_else(|_|std::process::exit(1));let mut bridge=NativeRustBridge::new(Host,caps);std::thread::spawn(move||{{let _=bridge.{export_method}(1,2);}}).join().unwrap();}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("cross_thread.rs"), cross_thread).unwrap();
    let cross_thread_executable = if cfg!(windows) {
        "cross_thread.exe"
    } else {
        "cross_thread"
    };
    let mut compile = Command::new(&rustc.path);
    compile.env_clear().current_dir(&output).args([
        "--edition=2021",
        "-C",
        "panic=unwind",
        "-C",
        &format!("link-arg={object}"),
        "cross_thread.rs",
        "-o",
        cross_thread_executable,
    ]);
    bind_test_tool_environment(&mut compile);
    bind_test_rust_linker(&mut compile, &clang);
    let compile = compile.output().unwrap();
    assert!(!compile.status.success());
    assert!(!output.join(cross_thread_executable).exists());
    assert!(
        String::from_utf8_lossy(&compile.stderr).contains("cannot be sent between threads safely")
    );

    let nested_borrow = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use semaprax_native_rust_interop::*;
struct Host;
impl NativeRustImports for Host{{fn {import_method}(&mut self,left:i64,_right:i64)->NativeRustImportResult<i64>{{NativeRustImportResult::Success(left)}}}}
fn nested(bridge:&mut NativeRustBridge<Host>){{let borrow=&mut *bridge;let first=bridge.{export_method}(1,2);let second=borrow.{export_method}(1,2);let _=(first,second);}}
fn main(){{}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("nested_borrow.rs"), nested_borrow).unwrap();
    let nested = Command::new(&rustc.path)
        .env_clear()
        .current_dir(&output)
        .args([
            "--edition=2021",
            "--crate-type",
            "lib",
            "nested_borrow.rs",
            "-o",
            if cfg!(windows) {
                "nested_borrow.rlib"
            } else {
                "libnested_borrow.rlib"
            },
        ])
        .output()
        .unwrap();
    assert!(!nested.status.success());
    let nested_stderr = String::from_utf8_lossy(&nested.stderr);
    assert!(
        nested_stderr.contains("cannot borrow `*bridge` as mutable more than once at a time"),
        "{nested_stderr}"
    );

    let ffi_sibling = String::from(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
mod sibling{pub fn forge(){let _=super::semaprax_native_rust_interop::ffi::capabilities_digest();}}
fn main(){sibling::forge();}
"#,
    );
    std::fs::write(output.join("ffi_sibling.rs"), ffi_sibling).unwrap();
    let ffi_executable = if cfg!(windows) {
        "ffi_sibling.exe"
    } else {
        "ffi_sibling"
    };
    let mut compile = Command::new(&rustc.path);
    compile.env_clear().current_dir(&output).args([
        "--edition=2021",
        "-C",
        "panic=unwind",
        "-C",
        &format!("link-arg={object}"),
        "ffi_sibling.rs",
        "-o",
        ffi_executable,
    ]);
    bind_test_tool_environment(&mut compile);
    bind_test_rust_linker(&mut compile, &clang);
    let compile = compile.output().unwrap();
    assert!(!compile.status.success());
    assert!(!output.join(ffi_executable).exists());
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(stderr.contains("module `ffi` is private"), "{stderr}");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn bool_and_infallible_import_abi_is_exact_at_o0_and_o2() {
    const BOOL_SOURCE: &str = r#"module interop.bool_fixture;

@id("host.bool")
interface HostBool
    permits {  }
{
    @id("host.bool.invert")
    import rust fn invert(value: bool) -> bool
        effects {  }
        failure infallible;
}

@id("interop.bool")
fn call_invert(value: bool) -> bool
{
    invert(value)
}

@id("interop.bool.main")
fn main() -> i64
{
    0
}
"#;
    let program = crate::parse(BOOL_SOURCE, Path::new("native-rust-bool.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let spec = Spec {
        module: program.module.clone(),
        source_revision: Some(domain_digest(SOURCE_DOMAIN, canonical.as_bytes())),
        target: current_target().unwrap(),
        exports: vec!["interop.bool".to_owned()],
        imports: vec!["host.bool.invert".to_owned()],
        capabilities: Vec::new(),
    };
    let spec = render_spec(&spec);
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let parsed_spec = parse_spec(&program, spec.as_bytes()).unwrap();
    let export = &prepared.exports[0];
    let import = &prepared.imports[0];
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-bool-test-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    build_native_rust_interop_bundle(&program, spec.as_bytes(), &output).unwrap();
    let rustc = configured_tool("RUSTC").unwrap();
    let clang = configured_tool("CLANG").unwrap();
    let sanitizers = sanitizer_mode().unwrap();
    let object = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let o0_object = if cfg!(windows) {
        "module_bool_O0.obj"
    } else {
        "module_bool_O0.o"
    };
    let mut o0_compile = Command::new(&clang.path);
    o0_compile.env_clear().current_dir(&output).args([
        "-std=c11",
        "-target",
        &prepared.target.triple,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O0",
        "-c",
        "module.c",
        "-o",
        o0_object,
    ]);
    bind_test_tool_environment(&mut o0_compile);
    if sanitizers {
        o0_compile.args(REQUIRED_NATIVE_RUST_SANITIZER_FLAGS);
    }
    assert!(o0_compile.status().unwrap().success());

    let safe_harness = format!(
        r#"#[path="semaprax_native_rust_interop.rs"]mod semaprax_native_rust_interop;
use core::num::NonZeroU32;
use semaprax_native_rust_interop::*;
struct Host{{mode:u8}}
impl NativeRustImports for Host{{fn {import_method}(&mut self,value:bool)->NativeRustImportResult<bool>{{match self.mode{{0=>NativeRustImportResult::Success(!value),_=>NativeRustImportResult::Status{{code:NonZeroU32::new(9).unwrap(),class:NativeRustStatusClass::Import,retryable:false}}}}}}}}
fn bridge(mode:u8)->NativeRustBridge<Host>{{NativeRustBridge::new(Host{{mode}},NativeRustCapabilities::new(&[]).unwrap_or_else(|_|std::process::exit(10)))}}
fn main(){{let code=NonZeroU32::new(1).unwrap();let _=NativeRustImportResult::<bool>::HostFailure;let probe=NativeRustCallError::Semantic{{domain_id:"semaprax.native-rust-semantics.v1",code,class:NativeRustStatusClass::Semantic,retryable:false}};if let NativeRustCallError::Semantic{{domain_id,code,class,retryable}}=probe{{let _=(domain_id,code,class,retryable);}}let mut success=bridge(0);if !matches!(success.{export_method}(false),Ok(true))||!matches!(success.{export_method}(true),Ok(false)){{std::process::exit(11)}}let mut rejected=bridge(1);if !matches!(rejected.{export_method}(false),Err(NativeRustCallError::AdapterRejected)){{std::process::exit(12)}}}}
"#,
        import_method = import.rust_method,
        export_method = export.rust_method,
    );
    std::fs::write(output.join("bool_safe.rs"), safe_harness).unwrap();
    let capability_hex = capability_digest(&parsed_spec.capabilities)
        .strip_prefix("sha256:")
        .unwrap()
        .to_owned();
    let capability_bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    let raw_harness = format!(
        r#"#![allow(unsafe_code)]
use core::ffi::c_void;
type Callback=unsafe extern "C" fn(*mut c_void,u8,*mut u8)->u64;
#[repr(C)]struct Imports{{abi_version:u32,size:u32,callback:Option<Callback>}}
#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}
unsafe extern "C"{{fn {export_symbol}(ctx:*const Context,arg_0:u8,result_out:*mut u8)->u64;}}
fn adapter(code:u64)->u64{{(65535u64<<48)|(4u64<<32)|code}}
unsafe extern "C" fn callback(userdata:*mut c_void,value:u8,out:*mut u8)->u64{{let mode=unsafe{{*(userdata.cast::<u8>())}};match mode{{0=>{{unsafe{{*out=u8::from(value==0)}};0}},1=>{{unsafe{{*out=2}};0}},_=>adapter(3)}}}}
fn main(){{unsafe{{let imports=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,callback:Some(callback)}};let mut mode=0u8;let context=Context{{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:(&mut mode as *mut u8).cast(),imports:&imports,capabilities_digest:[{capability_bytes}],call_depth:0,reserved:0}};let mut out=0u8;assert_eq!({export_symbol}(&context,0,&mut out),0);assert_eq!(out,1);assert_eq!({export_symbol}(&context,1,&mut out),0);assert_eq!(out,0);let mut poison=0x5au8;assert_eq!({export_symbol}(&context,2,&mut poison),adapter(4));assert_eq!(poison,0x5a);mode=1;core::hint::black_box(&mode);assert_eq!({export_symbol}(&context,0,&mut poison),adapter(4));assert_eq!(poison,0x5a);mode=2;core::hint::black_box(&mode);assert_eq!({export_symbol}(&context,0,&mut poison),adapter(3));assert_eq!(poison,0x5a);}}}}
"#,
        export_symbol = export.c_symbol,
        capability_bytes = capability_bytes,
    );
    std::fs::write(output.join("bool_raw.rs"), raw_harness).unwrap();
    for (linked_object, suffix) in [(o0_object, "O0"), (object, "O2")] {
        for source in ["bool_safe.rs", "bool_raw.rs"] {
            let executable = format!(
                "{}_{}{}",
                source.trim_end_matches(".rs"),
                suffix,
                if cfg!(windows) { ".exe" } else { "" }
            );
            let mut compile = Command::new(&rustc.path);
            compile.env_clear().current_dir(&output).args([
                "--edition=2021",
                "-Dwarnings",
                "-C",
                "panic=unwind",
                "-C",
                &format!("link-arg={linked_object}"),
                source,
                "-o",
                &executable,
            ]);
            bind_test_tool_environment(&mut compile);
            bind_test_rust_linker(&mut compile, &clang);
            if sanitizers {
                compile.args([
                    "-C",
                    "link-arg=-fsanitize=address,undefined",
                    "-C",
                    "link-arg=-fno-sanitize-recover=all",
                ]);
            }
            assert!(compile.status().unwrap().success());
            let mut run = Command::new(output.join(&executable));
            run.env_clear().current_dir(&output);
            if sanitizers {
                run.env(
                    "ASAN_OPTIONS",
                    "detect_leaks=0:halt_on_error=1:abort_on_error=1",
                )
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
            }
            assert!(run.status().unwrap().success());
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}
