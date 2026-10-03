//! Real archive/package comparison against the immutable pre-RI05 renderer.
use super::*;
use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn owned_v1_historical_archive_bytes_and_linked_copy() {
    let root = std::env::temp_dir().join(format!("semaprax-v1-archive-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let _scratch = Scratch(root.clone());
    let rustc = std::env::var("RUSTC").expect("configured rustc");
    let original = include_bytes!("fixtures/owned_v1_archive_provider.c");
    let baseline = include_str!("fixtures/owned_v1_archive_baseline_digests.txt");
    let target = HostTarget::current().unwrap();
    assert!(
        baseline.contains(&format!("target {}\n", target.triple())),
        "requires the baseline capture target and toolchain"
    );
    let mut original_archive = None;
    for (label, provider, expected) in [
        ("current", original.to_vec(), true),
        (
            "flipped",
            String::from_utf8(original.to_vec())
                .unwrap()
                .replace(
                    "memset(payload,0xff,requested)",
                    "memset(payload,0xfe,requested)",
                )
                .into_bytes(),
            false,
        ),
    ] {
        let output = root.join(label);
        let bytes = descriptor_bytes("owned-bytes");
        build_and_publish(
            PackagePlan::new(
                bytes.clone(),
                descriptor_digest(&bytes),
                vec!["fixture.value".to_owned()],
                provider.clone(),
                provider_sha256(&provider),
                PackageMode::StandaloneEvidence,
            ),
            &output,
        )
        .unwrap();
        let archive = fs::read(output.join(target.archive_name())).unwrap();
        if expected {
            let mut rows = String::from("baseline 4dd73795021564dcc6d90ac6505b2814c1818cd1\n");
            rows.push_str(&format!(
                "target {}\nprovider {}\n",
                target.triple(),
                provider_sha256(&provider)
            ));
            let mut names = fs::read_dir(&output)
                .unwrap()
                .map(|p| p.unwrap().file_name().into_string().unwrap())
                .collect::<Vec<_>>();
            names.sort();
            for name in names {
                let bytes = fs::read(output.join(&name)).unwrap();
                rows.push_str(&format!("{name} {} {}\n", bytes.len(), raw_sha256(&bytes)));
            }
            assert_eq!(
                rows, baseline,
                "actual native archive and complete package bytes"
            );
            original_archive = Some(archive);
        } else {
            assert_ne!(
                Some(archive),
                original_archive,
                "flipped provider must alter actual native archive"
            );
        }
        fs::write(output.join("consumer.rs"), CONSUMER).unwrap();
        let binary = output.join("consumer");
        let compiled = Command::new(&rustc)
            .args(["--edition=2021"])
            .arg(output.join("consumer.rs"))
            .arg("-L")
            .arg(&output)
            .arg("-lstatic=semaprax_native_rust_owned_data_sdk")
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(binary).output().unwrap();
        assert_eq!(
            ran.status.success(),
            expected,
            "{label}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        if expected {
            assert_eq!(ran.stdout, b"native-archive-copy-ok\n");
        } else {
            assert!(String::from_utf8_lossy(&ran.stderr).contains("native-archive-payload"));
        }
    }
}

const CONSUMER: &str = r#"
#[path="lib.rs"]mod sdk;
unsafe extern "C" {fn spx_frozen_configure(len:u64);fn spx_frozen_count(kind:u32)->u64;}
fn main(){
 for len in [0u64,3,65536]{
  unsafe{spx_frozen_configure(len)};
  let mut sdk=sdk::NativeRustOwnedDataSdk::new().unwrap();let bytes=sdk.spx_fixture_dot_value().unwrap();
  assert_eq!(bytes.len(),len as usize);assert!(bytes.iter().all(|b|*b==0xff),"native-archive-payload");
  for kind in 0..3 {assert_eq!(unsafe{spx_frozen_count(kind)},1);}
  drop(sdk);
 }
 println!("native-archive-copy-ok");
}
"#;
