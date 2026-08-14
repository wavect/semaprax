#![cfg(unix)]

#[cfg(target_os = "macos")]
use semaprax_native_rust_interop_platform::{
    clang_version, hold_external_executable, rustc_version,
};
use semaprax_native_rust_interop_platform::{
    create_directory_new, discard_owned_stage_prepared, hold_directory, inventory_exact_prepared,
    prepare_discard_inventory, prepare_inventory_exact, prepare_publish_directory,
    prepare_stage_name, prepared_publish_directory_remaining, publish_directory_new_prepared,
    read_exact, recheck_directory, write_file_new, write_file_new_prepared, Error, HeldDirectory,
    HeldRegularFile,
};
use semaprax_native_rust_interop_platform::{execute_harness, hold_executable};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::ops::Deref;
use std::os::unix::fs::MetadataExt as _;
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
    dev: u64,
    ino: u64,
}

impl Deref for OwnedRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for OwnedRoot {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && (metadata.dev(), metadata.ino()) == (self.dev, self.ino)
        {
            std::fs::remove_dir_all(&self.path).unwrap();
        }
    }
}

fn root(label: &str) -> OwnedRoot {
    for _ in 0..32 {
        let mut random = [0_u8; 16];
        File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut random)
            .unwrap();
        let mut nonce = String::with_capacity(random.len() * 2);
        for byte in random {
            write!(&mut nonce, "{byte:02x}").unwrap();
        }
        let path = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-platform-{label}-{}-{nonce}",
                std::process::id()
            ));
        match std::fs::create_dir(&path) {
            Ok(()) => {
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                return OwnedRoot {
                    path,
                    dev: metadata.dev(),
                    ino: metadata.ino(),
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("create owned test directory: {error}"),
        }
    }
    panic!("could not allocate an owned test directory")
}

#[cfg(target_os = "macos")]
fn resolved_tool(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("test PATH"))
        .map(|directory| directory.join(name))
        .find_map(|candidate| std::fs::canonicalize(candidate).ok())
        .expect("installed test tool")
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_installed_tools_are_suspended_vnode_attested_before_resume() {
    let root = root("darwin-attestation");
    let cwd = hold_directory(root.as_ref()).unwrap();
    let rustc_path = std::env::var_os("RUSTC")
        .map(PathBuf::from)
        .map(|path| std::fs::canonicalize(path).expect("resolved RUSTC image"))
        .unwrap_or_else(|| resolved_tool("rustc"));
    let rustc = hold_external_executable(&rustc_path).unwrap();
    let clang = hold_external_executable(&resolved_tool("clang")).unwrap();

    let rustc_output = rustc_version(&rustc, &cwd).unwrap();
    assert!(rustc_output.bytes().starts_with(b"rustc "));
    let clang_output = clang_version(&clang, &cwd).unwrap();
    assert!(clang_output.bytes().starts_with(b"Apple clang version "));
}

fn compile_c(root: &Path, name: &str, source: &str) -> PathBuf {
    let source_path = root.join(format!("{name}.c"));
    let executable = root.join(name);
    std::fs::write(&source_path, source).unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let output = Command::new(compiler)
        .env_clear()
        .env("TMPDIR", root)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
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
fn intermediate_symlinks_and_directory_identity_or_permission_drift_are_rejected() {
    use std::os::unix::fs::{symlink, PermissionsExt as _};

    let root = root("directory-authority");
    let real = root.join("real");
    std::fs::create_dir(&real).unwrap();
    let link = root.join("link");
    symlink(&real, &link).unwrap();
    let error = match hold_directory(&link) {
        Ok(_) => panic!("intermediate symlink was followed"),
        Err(error) => error,
    };
    assert_eq!(error, Error::Changed);

    let held = hold_directory(&real).unwrap();
    let original = std::fs::metadata(&real).unwrap().permissions();
    let mut changed = original.clone();
    changed.set_mode(0o700);
    std::fs::set_permissions(&real, changed).unwrap();
    assert_eq!(recheck_directory(&held), Err(Error::Changed));
    std::fs::set_permissions(&real, original).unwrap();

    let displaced = root.join("displaced");
    std::fs::rename(&real, &displaced).unwrap();
    std::fs::create_dir(&real).unwrap();
    recheck_directory(&held).unwrap();
    let parent = hold_directory(&root).unwrap();
    let stage_name = prepare_stage_name(OsStr::new("real")).unwrap();
    let mut publish = prepare_publish_directory(OsStr::new("output")).unwrap();
    let error = match publish_directory_new_prepared(
        &mut publish,
        &parent,
        &held,
        &stage_name,
        OsStr::new("output"),
    ) {
        Ok(_) => panic!("substituted stage path was published"),
        Err(error) => error,
    };
    assert_eq!(error, Error::Changed);
    assert!(displaced.is_dir());
    assert!(real.is_dir());
}

#[test]
fn handle_relative_create_inventory_and_publish_are_no_clobber() {
    let root = root("publish");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let mut inventory = prepare_discard_inventory([OsStr::new("artifact")]).unwrap();
    let mut exact = prepare_inventory_exact(&inventory).unwrap();
    write_file_new_prepared(&stage, &mut inventory, "artifact", b"authenticated", 0o600).unwrap();
    assert_eq!(
        read_exact(inventory.file("artifact").unwrap(), 13).unwrap(),
        b"authenticated"
    );
    assert_eq!(
        read_exact(inventory.file("artifact").unwrap(), 12),
        Err(Error::OutputLimit)
    );
    inventory_exact_prepared(&mut exact, &stage, &inventory).unwrap();
    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    let mut mismatched_publish = prepare_publish_directory(OsStr::new("planned")).unwrap();
    assert_eq!(
        publish_directory_new_prepared(
            &mut mismatched_publish,
            &parent,
            &stage,
            &stage_name,
            OsStr::new("different"),
        ),
        Err(Error::Invalid)
    );
    assert_eq!(prepared_publish_directory_remaining(&mismatched_publish), 1);
    assert!(root.join("stage").is_dir());

    let foreign = root.join("foreign");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("sentinel"), b"foreign").unwrap();
    let mut foreign_publish = prepare_publish_directory(OsStr::new("foreign")).unwrap();
    let error = match publish_directory_new_prepared(
        &mut foreign_publish,
        &parent,
        &stage,
        &stage_name,
        OsStr::new("foreign"),
    ) {
        Ok(_) => panic!("foreign output was replaced"),
        Err(error) => error,
    };
    assert_eq!(error, Error::Exists);
    assert_eq!(prepared_publish_directory_remaining(&foreign_publish), 0);
    assert_eq!(std::fs::read(foreign.join("sentinel")).unwrap(), b"foreign");
    assert_eq!(
        std::fs::read(root.join("stage/artifact")).unwrap(),
        b"authenticated"
    );

    let mut bundle_publish = prepare_publish_directory(OsStr::new("bundle")).unwrap();
    publish_directory_new_prepared(
        &mut bundle_publish,
        &parent,
        &stage,
        &stage_name,
        OsStr::new("bundle"),
    )
    .unwrap();
    assert_eq!(prepared_publish_directory_remaining(&bundle_publish), 0);
    inventory_exact_prepared(&mut exact, &stage, &inventory).unwrap();
    assert!(!root.join("stage").exists());
    assert_eq!(
        std::fs::read(root.join("bundle/artifact")).unwrap(),
        b"authenticated"
    );
}

#[test]
fn exact_owned_stage_discard_removes_only_the_authenticated_inventory() {
    let root = root("discard-exact");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let first = write_file_new(&stage, OsStr::new("first"), b"one", 0o600).unwrap();
    let second = write_file_new(&stage, OsStr::new("second"), b"two", 0o600).unwrap();

    let stage_name = prepare_stage_name(OsStr::new("stage")).unwrap();
    let mut inventory =
        prepare_discard_inventory([OsStr::new("first"), OsStr::new("second")]).unwrap();
    inventory.attach("first", first).unwrap();
    inventory.attach("second", second).unwrap();
    discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory).unwrap();

    assert!(!root.join("stage").exists());
    assert!(root.is_dir());
}

#[test]
fn owned_stage_discard_stops_on_inventory_or_file_identity_drift() {
    use std::os::unix::fs::symlink;

    let root = root("discard-hostile");
    let parent = hold_directory(&root).unwrap();

    let inventory_stage =
        create_directory_new(&parent, OsStr::new("inventory-stage"), 0o700).unwrap();
    let expected = write_file_new(
        &inventory_stage,
        OsStr::new("expected"),
        b"authenticated",
        0o600,
    )
    .unwrap();
    std::fs::write(root.join("inventory-stage/foreign-sentinel"), b"foreign").unwrap();
    assert_eq!(
        discard_one(
            &parent,
            &inventory_stage,
            "inventory-stage",
            "expected",
            expected,
        ),
        Err(Error::Changed)
    );
    assert_eq!(
        std::fs::read(root.join("inventory-stage/foreign-sentinel")).unwrap(),
        b"foreign"
    );
    assert_eq!(
        std::fs::read(root.join("inventory-stage/expected")).unwrap(),
        b"authenticated"
    );

    let file_stage = create_directory_new(&parent, OsStr::new("file-stage"), 0o700).unwrap();
    let held =
        write_file_new(&file_stage, OsStr::new("artifact"), b"authenticated", 0o600).unwrap();
    std::fs::rename(
        root.join("file-stage/artifact"),
        root.join("displaced-artifact"),
    )
    .unwrap();
    let foreign = root.join("foreign-target");
    std::fs::write(&foreign, b"foreign-sentinel").unwrap();
    symlink(&foreign, root.join("file-stage/artifact")).unwrap();
    assert_eq!(
        discard_one(&parent, &file_stage, "file-stage", "artifact", held),
        Err(Error::Changed)
    );
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign-sentinel");
    assert!(std::fs::symlink_metadata(root.join("file-stage/artifact"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::read(root.join("displaced-artifact")).unwrap(),
        b"authenticated"
    );
}

#[test]
fn owned_stage_discard_stops_on_same_path_directory_substitution() {
    let root = root("discard-stage-substitution");
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let held = write_file_new(&stage, OsStr::new("artifact"), b"authenticated", 0o600).unwrap();
    std::fs::rename(root.join("stage"), root.join("displaced-stage")).unwrap();
    std::fs::create_dir(root.join("stage")).unwrap();
    std::fs::write(root.join("stage/foreign-sentinel"), b"foreign").unwrap();

    assert_eq!(
        discard_one(&parent, &stage, "stage", "artifact", held),
        Err(Error::Changed)
    );
    assert_eq!(
        std::fs::read(root.join("stage/foreign-sentinel")).unwrap(),
        b"foreign"
    );
    assert_eq!(
        std::fs::read(root.join("displaced-stage/artifact")).unwrap(),
        b"authenticated"
    );
}

#[test]
fn held_executable_ignores_same_byte_path_substitution_and_clears_process_ambient_state() {
    let root = root("held-executable");
    let executable = compile_c(
        &root,
        "probe",
        "#include <fcntl.h>\n#include <stdlib.h>\nint main(void){if(getenv(\"PATH\")!=0)return 8;for(int fd=4;fd<1024;fd++){if(fcntl(fd,F_GETFD)!=-1)return 9;}return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("probe")).unwrap();
    let original = std::fs::read(&executable).unwrap();
    std::fs::rename(&executable, root.join("displaced-probe")).unwrap();
    std::fs::write(&executable, &original).unwrap();
    let permissions = std::fs::metadata(root.join("displaced-probe"))
        .unwrap()
        .permissions();
    std::fs::set_permissions(&executable, permissions).unwrap();

    execute_harness(&held, &directory).unwrap();
    assert_eq!(std::fs::read(&executable).unwrap(), original);
    assert!(root.join("displaced-probe").is_file());
}

#[test]
fn output_overflow_kills_and_reaps_the_child_with_a_bounded_wait() {
    let root = root("bounded-kill");
    compile_c(
        &root,
        "noisy",
        "#include <unistd.h>\nint main(void){(void)write(1,\"x\",1);sleep(30);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("noisy")).unwrap();
    let started = Instant::now();
    assert_eq!(execute_harness(&held, &directory), Err(Error::OutputLimit));
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn output_overflow_quiesces_the_owned_process_group_before_return() {
    let root = root("bounded-process-group");
    compile_c(
        &root,
        "forking-noisy",
        "#include <stdio.h>\n#include <unistd.h>\nint main(void){pid_t child=fork();if(child<0)return 2;if(child==0){FILE *file=fopen(\"descendant.pid\",\"w\");if(!file)_exit(3);fprintf(file,\"%ld\",(long)getpid());fclose(file);sleep(30);_exit(0);}while(access(\"descendant.pid\",F_OK)!=0)usleep(1000);(void)write(1,\"x\",1);sleep(30);return 0;}\n",
    );
    let directory = hold_directory(&root).unwrap();
    let held = hold_executable(&directory, OsStr::new("forking-noisy")).unwrap();
    let started = Instant::now();
    assert_eq!(execute_harness(&held, &directory), Err(Error::OutputLimit));
    assert!(started.elapsed() < Duration::from_secs(10));

    let descendant = std::fs::read_to_string(root.join("descendant.pid"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let pid = descendant.to_string();
        if !Command::new("/bin/kill")
            .args(["-0", pid.as_str()])
            .output()
            .unwrap()
            .status
            .success()
        {
            break;
        }
        if Instant::now() >= deadline {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", pid.as_str()])
                .output();
            panic!("owned descendant remained observable after execute_harness returned");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn external_consumer_cannot_extract_handles_or_reach_the_sys_quarantine() {
    let root = root("opacity");
    std::fs::create_dir(root.join("src")).unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname='opacity-probe'\nversion='0.0.0'\nedition='2021'\n[dependencies]\nsemaprax-native-rust-interop-platform={{path={:?}}}\n",
            manifest_dir
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        r#"use semaprax_native_rust_interop_platform::{hold_directory,HeldDirectory};
use std::os::fd::AsRawFd;
fn raw(directory:&HeldDirectory)->i32{directory.0.as_raw_fd()}
fn require_clone<T:Clone>(){}
fn clone_it(){require_clone::<HeldDirectory>();}
fn debug_it(directory:&HeldDirectory){let _=format!("{directory:?}");}
fn main(){let _=hold_directory(std::path::Path::new("/"));let _=semaprax_native_rust_interop_platform_sys::Error::Invalid;}
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(["check", "--offline", "--quiet"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(
        stderr.contains("field `0` of struct `HeldDirectory` is private"),
        "{stderr}"
    );
    assert!(stderr.contains("Clone"), "{stderr}");
    assert!(stderr.contains("doesn't implement `Debug`"), "{stderr}");
    assert!(
        stderr.contains("semaprax_native_rust_interop_platform_sys"),
        "{stderr}"
    );
    assert!(
        stderr.contains("failed to resolve") || stderr.contains("cannot find module or crate"),
        "{stderr}"
    );
}
