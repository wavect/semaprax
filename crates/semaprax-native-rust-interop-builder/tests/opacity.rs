use std::fs::{self, File};
use std::io::Read as _;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax_native_rust_interop_platform::{
    hold_directory, recheck_directory, same_directory_path, HeldDirectory,
};

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
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        let identity_matches = recheck_directory(&authority).is_ok()
            && same_directory_path(&authority, &self.path) == Ok(true);
        drop(authority);
        if metadata.is_dir() && !metadata.file_type().is_symlink() && identity_matches {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
}

fn owned_root() -> OwnedRoot {
    let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
    for _ in 0..32 {
        let mut random = [0_u8; 16];
        File::open(if cfg!(windows) { "NUL" } else { "/dev/urandom" })
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
        use std::fmt::Write as _;
        let nonce = random.iter().fold(String::new(), |mut nonce, byte| {
            write!(nonce, "{byte:02x}").expect("write to string");
            nonce
        });
        let path = parent.join(format!(
            "semaprax-native-rust-interop-opacity-{}-{nonce}",
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
            Err(error) => panic!("create owned opacity root: {error}"),
        }
    }
    panic!("could not create owned opacity root")
}

#[test]
fn external_consumer_cannot_reach_private_preparation_build_or_facts() {
    let root = owned_root();
    fs::create_dir(root.join("src")).unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname='native-rust-interop-opacity-probe'\nversion='0.0.0'\nedition='2021'\n[dependencies]\nsemaprax-native-rust-interop={{path={manifest_dir:?}}}\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"use semaprax_native_rust_interop::{build,prepare,Bundle,Prepared};
fn main(){
    let _ = core::mem::size_of::<Prepared>();
    let _ = core::mem::size_of::<Bundle>();
    let _ = prepare;
    let _ = build;
}
"#,
    )
    .unwrap();
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .env("CARGO_NET_OFFLINE", "true")
        .current_dir(&root.path)
        .args(["check", "--offline", "--quiet"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    for name in ["build", "prepare", "Bundle", "Prepared"] {
        assert!(
            stderr.contains(name),
            "compiler did not reject `{name}`: {stderr}"
        );
    }
    assert!(!root
        .join("target/debug/native-rust-interop-opacity-probe")
        .exists());

    fs::write(
        root.join("src/main.rs"),
        "fn main(){let _=semaprax_native_rust_interop::implementation::prepare_native_rust_interop;}\n",
    )
    .unwrap();
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .env("CARGO_NET_OFFLINE", "true")
        .current_dir(&root.path)
        .args(["check", "--offline", "--quiet"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("implementation"));
}
