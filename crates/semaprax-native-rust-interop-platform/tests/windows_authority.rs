#![cfg(windows)]

use semaprax_native_rust_interop_platform::{
    clang_version_bounded, create_directory_new, discard_owned_stage_prepared, execute_harness,
    hold_directory, hold_executable, hold_regular_file, hold_settled_regular_file_prepared,
    inventory_exact_prepared, prepare_discard_inventory, prepare_inventory_exact,
    prepare_publish_directory, prepare_stage_name, publish_directory_new_prepared, read_exact,
    recheck_directory, same_directory_path, transition_regular_file_to_external_read_prepared,
    write_file_new, write_file_new_prepared, Error, HeldDirectory, HeldRegularFile,
};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Read as _;
use std::ops::Deref;
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn discard_one(
    parent: &HeldDirectory,
    stage: &HeldDirectory,
    stage_name: &'static str,
    file_name: &'static str,
    file: HeldRegularFile,
) -> Result<(), Error> {
    let stage_name = prepare_stage_name(OsStr::new(stage_name))?;
    let mut inventory = prepare_discard_inventory([OsStr::new(file_name)])?;
    inventory.attach(file_name, file)?;
    discard_owned_stage_prepared(parent, stage, &stage_name, &inventory)
}

struct OwnedRoot {
    path: PathBuf,
    authority: Option<HeldDirectory>,
}

impl Deref for OwnedRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let Some(authority) = self.authority.take() else {
            return;
        };
        let identity_matches = recheck_directory(&authority).is_ok()
            && same_directory_path(&authority, &self.path) == Ok(true);
        drop(authority);
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() && identity_matches {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
}

fn root(label: &str) -> OwnedRoot {
    let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
    for _ in 0..32 {
        let mut random = [0_u8; 16];
        File::open("NUL")
            .and_then(|mut file| file.read_exact(&mut random))
            .unwrap_or_else(|_| {
                random[..8].copy_from_slice(
                    &std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                        .to_le_bytes()[..8],
                );
            });
        let nonce = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = parent.join(format!(
            "semaprax-native-rust-platform-{label}-{}-{nonce}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                let authority = hold_directory(&path).unwrap();
                return OwnedRoot {
                    path,
                    authority: Some(authority),
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("create owned Windows test root: {error}"),
        }
    }
    panic!("could not allocate an owned Windows test root")
}

fn compile_c(root: &Path, name: &str, source: &str) -> PathBuf {
    let source_path = root.join(format!("{name}.c"));
    let executable = root.join(format!("{name}.exe"));
    fs::write(&source_path, source).unwrap();
    let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let output = Command::new(compiler)
        .env("TMP", root)
        .env("TEMP", root)
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-D_CRT_SECURE_NO_WARNINGS",
            "-O2",
        ])
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

#[test]
fn windows_junctions_and_same_path_directory_substitution_are_rejected() {
    let root = root("directory-authority");
    let real = root.join("real");
    fs::create_dir(&real).unwrap();
    let junction = root.join("junction");
    let linked = Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&real)
        .output()
        .unwrap();
    assert!(linked.status.success());
    assert_eq!(hold_directory(&junction).err(), Some(Error::Changed));

    let held = hold_directory(&real).unwrap();
    let displaced = root.join("displaced");
    fs::rename(&real, &displaced).unwrap();
    fs::create_dir(&real).unwrap();
    recheck_directory(&held).unwrap();
    let parent = hold_directory(&root).unwrap();
    let stage_name = prepare_stage_name(OsStr::new("real")).unwrap();
    let mut publish = prepare_publish_directory(OsStr::new("output")).unwrap();
    assert_eq!(
        publish_directory_new_prepared(
            &mut publish,
            &parent,
            &held,
            &stage_name,
            OsStr::new("output")
        )
        .err(),
        Some(Error::Changed)
    );
    assert!(displaced.is_dir());
    assert!(real.is_dir());
}

#[test]
fn windows_create_inventory_publish_and_exact_discard_are_no_clobber() {
    let root = root("publish-discard");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let file = write_file_new(&stage, OsStr::new("artifact"), b"authenticated", 0o600).unwrap();
    assert_eq!(read_exact(&file, 13).unwrap(), b"authenticated");
    assert_eq!(read_exact(&file, 12), Err(Error::OutputLimit));
    let mut inventory = prepare_discard_inventory([OsStr::new("artifact")]).unwrap();
    inventory
        .attach(
            "artifact",
            hold_regular_file(&stage, OsStr::new("artifact")).unwrap(),
        )
        .unwrap();
    drop(file);
    let mut exact = prepare_inventory_exact(&inventory).unwrap();
    inventory_exact_prepared(&mut exact, &stage, &inventory).unwrap();

    let foreign = root.join("foreign");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("sentinel"), b"foreign").unwrap();
    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    inventory.settle_for_publish().unwrap();
    let mut publish = prepare_publish_directory(OsStr::new("foreign")).unwrap();
    assert_eq!(
        publish_directory_new_prepared(
            &mut publish,
            &parent,
            &stage,
            &stage_name,
            OsStr::new("foreign")
        )
        .err(),
        Some(Error::Exists)
    );
    assert_eq!(fs::read(foreign.join("sentinel")).unwrap(), b"foreign");

    discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory).unwrap();
    assert!(!root.join("stage").exists());
}

#[test]
fn windows_prepared_publish_renames_the_authenticated_stage_without_clobber() {
    let root = root("publish-success");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let mut inventory = prepare_discard_inventory([OsStr::new("artifact")]).unwrap();
    write_file_new_prepared(&stage, &mut inventory, "artifact", b"authenticated", 0o600).unwrap();
    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    inventory.settle_for_publish().unwrap();
    assert_eq!(inventory.file("artifact").err(), Some(Error::Changed));
    let mut publish = prepare_publish_directory(OsStr::new("published")).unwrap();
    publish_directory_new_prepared(
        &mut publish,
        &parent,
        &stage,
        &stage_name,
        OsStr::new("published"),
    )
    .unwrap();
    assert!(!root.join("stage").exists());
    assert_eq!(
        fs::read(root.join("published/artifact")).unwrap(),
        b"authenticated"
    );
    let reopened = hold_settled_regular_file_prepared(&stage, &inventory, "artifact").unwrap();
    assert_eq!(read_exact(&reopened, 13).unwrap(), b"authenticated");
    let published_name = prepare_stage_name(OsStr::new("published")).unwrap();
    discard_owned_stage_prepared(&parent, &stage, &published_name, &inventory).unwrap();
    assert!(!root.join("published").exists());
}

#[test]
fn windows_settled_nested_inventory_publishes_after_descendant_authorities_close() {
    let root = root("nested-publish-success");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let source = create_directory_new(&stage, OsStr::new("src"), 0o700).unwrap();
    let native = create_directory_new(&stage, OsStr::new("native"), 0o700).unwrap();
    let mut root_files = prepare_discard_inventory([
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("sdk.json"),
    ])
    .unwrap();
    let mut source_files = prepare_discard_inventory([
        OsStr::new("lib.rs"),
        OsStr::new("safe.rs"),
        OsStr::new("ffi.rs"),
    ])
    .unwrap();
    let mut native_files = prepare_discard_inventory([
        OsStr::new("sdk.lib"),
        OsStr::new("descriptor.json"),
        OsStr::new("manifest.json"),
    ])
    .unwrap();
    for (name, bytes) in [
        ("Cargo.toml", b"cargo".as_slice()),
        ("build.rs", b"build".as_slice()),
        ("sdk.json", b"sdk".as_slice()),
    ] {
        write_file_new_prepared(&stage, &mut root_files, name, bytes, 0o600).unwrap();
    }
    for (name, bytes) in [
        ("lib.rs", b"lib".as_slice()),
        ("safe.rs", b"safe".as_slice()),
        ("ffi.rs", b"ffi".as_slice()),
    ] {
        write_file_new_prepared(&source, &mut source_files, name, bytes, 0o600).unwrap();
    }
    for (name, bytes) in [
        ("sdk.lib", b"archive".as_slice()),
        ("descriptor.json", b"descriptor".as_slice()),
        ("manifest.json", b"manifest".as_slice()),
    ] {
        write_file_new_prepared(&native, &mut native_files, name, bytes, 0o600).unwrap();
    }

    root_files.settle_for_publish().unwrap();
    source_files.settle_for_publish().unwrap();
    native_files.settle_for_publish().unwrap();
    drop((source, native));

    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    let mut publish = prepare_publish_directory(OsStr::new("published")).unwrap();
    publish_directory_new_prepared(
        &mut publish,
        &parent,
        &stage,
        &stage_name,
        OsStr::new("published"),
    )
    .unwrap();
    assert!(!root.join("stage").exists());
    assert_eq!(fs::read(root.join("published/src/lib.rs")).unwrap(), b"lib");
    assert_eq!(
        fs::read(root.join("published/native/sdk.lib")).unwrap(),
        b"archive"
    );
}

#[test]
fn settled_publish_inventory_rejects_foreign_name_substitution_before_cleanup() {
    let root = root("settled-publish-substitution");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let mut inventory = prepare_discard_inventory([OsStr::new("artifact")]).unwrap();
    write_file_new_prepared(&stage, &mut inventory, "artifact", b"authenticated", 0o600).unwrap();
    inventory.settle_for_publish().unwrap();

    fs::rename(root.join("stage/artifact"), root.join("displaced")).unwrap();
    fs::write(root.join("stage/artifact"), b"foreign").unwrap();
    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    assert_eq!(
        discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory),
        Err(Error::Changed)
    );
    assert_eq!(fs::read(root.join("stage/artifact")).unwrap(), b"foreign");
    assert_eq!(fs::read(root.join("displaced")).unwrap(), b"authenticated");
}

#[test]
fn hard_link_aliases_release_owned_access_before_external_read() {
    const FILE_SHARE_READ: u32 = 1;

    let root = root("hard-link-external-read");
    let parent = hold_directory(&root).unwrap();
    let source_stage = create_directory_new(&parent, OsStr::new("source"), 0o700).unwrap();
    let consumer_stage = create_directory_new(&parent, OsStr::new("consumer"), 0o700).unwrap();
    let mut source = prepare_discard_inventory([OsStr::new("module.obj")]).unwrap();
    write_file_new_prepared(
        &source_stage,
        &mut source,
        "module.obj",
        b"authenticated-object",
        0o600,
    )
    .unwrap();
    fs::hard_link(
        root.join("source/module.obj"),
        root.join("consumer/module_O2.o"),
    )
    .unwrap();
    let mut consumer = prepare_discard_inventory([OsStr::new("module_O2.o")]).unwrap();
    consumer
        .attach(
            "module_O2.o",
            hold_regular_file(&consumer_stage, OsStr::new("module_O2.o")).unwrap(),
        )
        .unwrap();

    transition_regular_file_to_external_read_prepared(
        &consumer_stage,
        &mut consumer,
        "module_O2.o",
    )
    .unwrap();
    assert!(OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(root.join("consumer/module_O2.o"))
        .is_err());

    transition_regular_file_to_external_read_prepared(&source_stage, &mut source, "module.obj")
        .unwrap();
    let external_reader = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(root.join("consumer/module_O2.o"))
        .unwrap();
    drop(external_reader);

    let consumer_name = prepare_stage_name(OsStr::new("consumer")).unwrap();
    discard_owned_stage_prepared(&parent, &consumer_stage, &consumer_name, &consumer).unwrap();
    let source_name = prepare_stage_name(OsStr::new("source")).unwrap();
    discard_owned_stage_prepared(&parent, &source_stage, &source_name, &source).unwrap();
}

#[test]
fn windows_discard_stops_on_inventory_and_stage_identity_drift() {
    let root = root("discard-hostile");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let file = write_file_new(&stage, OsStr::new("artifact"), b"authenticated", 0o600).unwrap();
    fs::write(root.join("stage/foreign-sentinel"), b"foreign").unwrap();
    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    let mut inventory = prepare_discard_inventory([OsStr::new("artifact")]).unwrap();
    inventory.attach("artifact", file).unwrap();
    assert_eq!(
        discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory),
        Err(Error::Changed)
    );
    assert_eq!(
        fs::read(root.join("stage/foreign-sentinel")).unwrap(),
        b"foreign"
    );
    inventory.settle_for_publish().unwrap();

    fs::rename(root.join("stage"), root.join("displaced-stage")).unwrap();
    fs::create_dir(root.join("stage")).unwrap();
    fs::write(root.join("stage/foreign-sentinel"), b"substitute").unwrap();
    assert_eq!(
        discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory),
        Err(Error::Changed)
    );
    assert_eq!(
        fs::read(root.join("stage/foreign-sentinel")).unwrap(),
        b"substitute"
    );
    assert_eq!(
        fs::read(root.join("displaced-stage/artifact")).unwrap(),
        b"authenticated"
    );

    let file_stage = create_directory_new(&parent, OsStr::new("file-stage"), 0o700).unwrap();
    let held = write_file_new(
        &file_stage,
        OsStr::new("artifact"),
        b"authenticated-file",
        0o600,
    )
    .unwrap();
    fs::rename(
        root.join("file-stage/artifact"),
        root.join("displaced-artifact"),
    )
    .unwrap();
    fs::write(root.join("file-stage/artifact"), b"foreign-file").unwrap();
    assert_eq!(
        discard_one(&parent, &file_stage, "file-stage", "artifact", held),
        Err(Error::Changed)
    );
    assert_eq!(
        fs::read(root.join("file-stage/artifact")).unwrap(),
        b"foreign-file"
    );
    assert_eq!(
        fs::read(root.join("displaced-artifact")).unwrap(),
        b"authenticated-file"
    );
}

#[test]
fn windows_held_executable_uses_held_identity_and_empty_environment() {
    let root = root("held-executable");
    let good = compile_c(
        &root,
        "good",
        "#include <stdlib.h>\nint main(void){return getenv(\"PATH\")!=0?8:0;}\n",
    );
    let bad = compile_c(&root, "bad", "int main(void){return 77;}\n");
    let probe = root.join("probe.exe");
    fs::rename(&good, &probe).unwrap();
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("probe.exe")).unwrap();
    fs::rename(&probe, root.join("displaced-probe.exe")).unwrap();
    fs::copy(&bad, &probe).unwrap();

    execute_harness(&held, &directory).unwrap();
    assert!(root.join("displaced-probe.exe").is_file());
    assert!(probe.is_file());
}

#[test]
fn windows_run_argv_handles_zero_and_small_stdout_at_normal_eof() {
    let root = root("normal-eof");
    let silent = compile_c(&root, "silent", "int main(void){return 0;}\n");
    let small = compile_c(
        &root,
        "small",
        "#include <stdio.h>\nint main(void){fputs(\"ok\",stdout);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let silent = hold_executable(&directory, silent.file_name().unwrap()).unwrap();
    let small = hold_executable(&directory, small.file_name().unwrap()).unwrap();
    assert_eq!(
        clang_version_bounded(&silent, &directory, 0)
            .unwrap()
            .bytes(),
        b""
    );
    assert_eq!(
        clang_version_bounded(&small, &directory, 2)
            .unwrap()
            .bytes(),
        b"ok"
    );
    assert_eq!(
        clang_version_bounded(&small, &directory, 1).err(),
        Some(Error::OutputLimit)
    );
}

#[test]
fn windows_names_are_exact_ascii_non_dos_and_casefold_no_clobber() {
    let root = root("names");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("s"), 0o700).unwrap();
    let one = write_file_new(&stage, OsStr::new("a"), b"one", 0o600).unwrap();
    assert_eq!(
        write_file_new(&stage, OsStr::new("A"), b"foreign", 0o600).err(),
        Some(Error::Exists)
    );
    assert_eq!(read_exact(&one, 3).unwrap(), b"one");
    for reserved in [
        "CON", "con.txt", "PRN", "AUX", "NUL", "CLOCK$", "COM1", "com9.log", "LPT1", "lpt9.bin",
    ] {
        assert_eq!(
            write_file_new(&stage, OsStr::new(reserved), b"x", 0o600).err(),
            Some(Error::Invalid),
            "reserved Windows name {reserved} was accepted"
        );
    }
    let mut inventory = prepare_discard_inventory([OsStr::new("a")]).unwrap();
    inventory
        .attach("a", hold_regular_file(&stage, OsStr::new("a")).unwrap())
        .unwrap();
    let mut exact = prepare_inventory_exact(&inventory).unwrap();
    inventory_exact_prepared(&mut exact, &stage, &inventory).unwrap();
    discard_one(&parent, &stage, "s", "a", one).unwrap();
}

#[test]
fn windows_descendant_held_stdout_is_quiesced_without_output_overflow() {
    let root = root("descendant-stdout");
    compile_c(
        &root,
        "quiet_tree",
        "#include <windows.h>\n#include <stdio.h>\n#include <string.h>\nint main(int argc,char **argv){if(argc==2&&strcmp(argv[1],\"child\")==0){Sleep(30000);return 0;}char path[MAX_PATH];if(!GetModuleFileNameA(NULL,path,MAX_PATH))return 4;char command[MAX_PATH+16];if(sprintf_s(command,sizeof(command),\"\\\"%s\\\" child\",path)<0)return 5;STARTUPINFOA startup={0};startup.cb=sizeof(startup);PROCESS_INFORMATION process={0};if(!CreateProcessA(NULL,command,NULL,NULL,TRUE,0,NULL,NULL,&startup,&process))return 6;FILE *file=fopen(\"descendant.pid\",\"w\");if(!file)return 7;fprintf(file,\"%lu\",(unsigned long)process.dwProcessId);fclose(file);CloseHandle(process.hThread);CloseHandle(process.hProcess);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("quiet_tree.exe")).unwrap();
    let started = Instant::now();
    execute_harness(&held, &directory).unwrap();
    assert!(started.elapsed() < Duration::from_secs(10));
    let descendant = fs::read_to_string(root.join("descendant.pid")).unwrap();
    let listed = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {descendant}"), "/FO", "CSV", "/NH"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&listed.stdout).contains(&format!("\"{descendant}\"")));
}

#[test]
fn windows_silent_timeout_is_bounded_and_reaps_the_leader() {
    let root = root("silent-timeout");
    compile_c(
        &root,
        "silent_timeout",
        "#include <windows.h>\n#include <stdio.h>\nint main(void){FILE *file=fopen(\"leader.pid\",\"w\");if(!file)return 3;fprintf(file,\"%lu\",(unsigned long)GetCurrentProcessId());fclose(file);Sleep(60000);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("silent_timeout.exe")).unwrap();
    let started = Instant::now();
    assert_eq!(execute_harness(&held, &directory), Err(Error::Spawn));
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(30));
    assert!(elapsed < Duration::from_secs(40));
    let leader = fs::read_to_string(root.join("leader.pid")).unwrap();
    let listed = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {leader}"), "/FO", "CSV", "/NH"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&listed.stdout).contains(&format!("\"{leader}\"")));
}

#[test]
fn windows_output_overflow_kills_and_reaps_the_process_tree_with_a_bounded_wait() {
    let root = root("bounded-kill");
    compile_c(
        &root,
        "noisy",
        "#include <windows.h>\n#include <stdio.h>\n#include <string.h>\nint main(int argc,char **argv){if(argc==2&&strcmp(argv[1],\"child\")==0){FILE *file=fopen(\"descendant.pid\",\"w\");if(!file)return 3;fprintf(file,\"%lu\",(unsigned long)GetCurrentProcessId());fclose(file);Sleep(30000);return 0;}char path[MAX_PATH];if(!GetModuleFileNameA(NULL,path,MAX_PATH))return 4;char command[MAX_PATH+16];if(sprintf_s(command,sizeof(command),\"\\\"%s\\\" child\",path)<0)return 5;STARTUPINFOA startup={0};startup.cb=sizeof(startup);PROCESS_INFORMATION process={0};if(!CreateProcessA(NULL,command,NULL,NULL,FALSE,0,NULL,NULL,&startup,&process))return 6;CloseHandle(process.hThread);CloseHandle(process.hProcess);while(GetFileAttributesA(\"descendant.pid\")==INVALID_FILE_ATTRIBUTES)Sleep(1);fputs(\"x\",stdout);fflush(stdout);Sleep(30000);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("noisy.exe")).unwrap();
    let started = Instant::now();
    assert_eq!(execute_harness(&held, &directory), Err(Error::OutputLimit));
    assert!(started.elapsed() < Duration::from_secs(10));

    let descendant = fs::read_to_string(root.join("descendant.pid")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let listed = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {descendant}"), "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        let output = String::from_utf8_lossy(&listed.stdout);
        if !output.contains(&format!("\"{descendant}\"")) {
            break;
        }
        if Instant::now() >= deadline {
            let _ = Command::new("taskkill")
                .args(["/PID", &descendant, "/F"])
                .output();
            panic!("owned Windows descendant remained observable after harness return");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn windows_external_consumer_cannot_extract_handles_or_reach_sys_quarantine() {
    let root = root("opacity");
    fs::create_dir(root.join("src")).unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname='windows-opacity-probe'\nversion='0.0.0'\nedition='2021'\n[dependencies]\nsemaprax-native-rust-interop-platform={{path={manifest_dir:?}}}\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"use semaprax_native_rust_interop_platform::{hold_directory,HeldDirectory};
use std::os::windows::io::AsRawHandle;
fn raw(directory:&HeldDirectory)->*mut core::ffi::c_void{directory.0.as_raw_handle()}
fn require_clone<T:Clone>(){}
fn clone_it(){require_clone::<HeldDirectory>();}
fn debug_it(directory:&HeldDirectory){let _=format!("{directory:?}");}
fn main(){let _=hold_directory(std::path::Path::new("C:\\"));let _=semaprax_native_rust_interop_platform_sys::Error::Invalid;}
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(["check", "--offline", "--quiet"])
        .current_dir(&*root)
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(stderr.contains("field `0` of struct `HeldDirectory` is private"));
    assert!(stderr.contains("Clone"));
    assert!(stderr.contains("Debug"));
    assert!(stderr.contains("semaprax_native_rust_interop_platform_sys"));
}
