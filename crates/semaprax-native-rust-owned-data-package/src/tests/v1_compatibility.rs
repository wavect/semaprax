use super::*;
use std::{fs, path::PathBuf, process::Command};

const BASELINE: &str = "4dd73795021564dcc6d90ac6505b2814c1818cd1";

// These rows cover every deterministic text artifact and its manifest across
// the five declared targets. The archive is fixed input, not native-code proof.
fn frozen_rows() -> String {
    let mut output = format!("baseline {BASELINE}\narchive-input RI05 frozen archive input\\n\n");
    for (name, target) in [
        ("x86_64-linux", HostTarget::X86_64LinuxGnu),
        ("aarch64-linux", HostTarget::Aarch64LinuxGnu),
        ("x86_64-darwin", HostTarget::X86_64Darwin),
        ("aarch64-darwin", HostTarget::Aarch64Darwin),
        ("x86_64-windows", HostTarget::X86_64WindowsMsvc),
    ] {
        for result in [
            "owned-bytes",
            "option-owned-bytes",
            "result-owned-bytes-i64",
        ] {
            let bytes = descriptor_bytes(result);
            let digest = descriptor_digest(&bytes);
            let descriptor =
                descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap();
            let sources =
                render::render_sources(&descriptor, target, PackageMode::StandaloneEvidence);
            let files = [
                ("Cargo.toml", sources.cargo_toml.as_bytes()),
                ("build.rs", sources.build_rs.as_bytes()),
                ("lib.rs", sources.lib_rs.as_bytes()),
                ("owned_data_ffi.rs", sources.ffi_rs.as_bytes()),
                (
                    target.archive_name(),
                    b"RI05 frozen archive input\n".as_slice(),
                ),
                ("descriptor.json", bytes.as_slice()),
            ];
            let manifest = render::render_manifest(
                target,
                &bytes,
                &digest,
                target.archive_name(),
                PackageMode::StandaloneEvidence,
                "sha256:provider",
                files,
            );
            for (file, data) in files
                .into_iter()
                .chain([("manifest.json", manifest.as_bytes())])
            {
                output.push_str(&format!(
                    "{name} {result} {file} {} {}\n",
                    data.len(),
                    raw_sha256(data)
                ));
            }
        }
    }
    output
}

#[test]
fn owned_v1_frozen_bytes_and_physical_output_copy() {
    assert_eq!(
        frozen_rows(),
        include_str!("fixtures/owned_v1_baseline_digests.txt")
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-v1-frozen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let rustc = std::env::var("RUSTC").expect("configured rustc is required");
    assert!(std::path::Path::new(&rustc).is_absolute());
    for (kind, result, normalize) in [
        (0, "owned-bytes", "Some(raw.unwrap())"),
        (1, "option-owned-bytes", "raw.unwrap()"),
        (2, "result-owned-bytes-i64", "match raw.unwrap(){Ok(value)=>Some(value),Err(code)=>{assert_eq!(code,i64::MIN);None}}"),
    ] {
        let bytes = descriptor_bytes(result);
        let descriptor = descriptor::replay(&bytes, &descriptor_digest(&bytes), &["fixture.value".to_owned()]).unwrap();
        let sources = render::render_sources(&descriptor, HostTarget::current().unwrap(), PackageMode::StandaloneEvidence);
        fs::write(root.join("sdk.rs"), &sources.lib_rs).unwrap();
        let fixture = include_str!("fixtures/owned_v1_copy.rs").replace("@KIND@", &kind.to_string()).replace("@NORMALIZE@", normalize);
        fs::write(root.join("main.rs"), fixture).unwrap();
        let copy = "unsafe{spx_owned_bytes_copy_v1(guard.context.raw.as_ptr(),handle,pointer,length as u64)}";
        assert_eq!(sources.ffi_rs.matches(copy).count(), 1);
        for (label, opt, ffi, expected) in [
            ("o0", "0", sources.ffi_rs.clone(), true),
            ("o2", "2", sources.ffi_rs.clone(), true),
            ("skipped-copy", "2", sources.ffi_rs.replace(copy, "0u32"), false),
        ] {
            fs::write(root.join("owned_data_ffi.rs"), ffi).unwrap();
            let executable = root.join(format!("{kind}-{label}{}", std::env::consts::EXE_SUFFIX));
            let compiled = Command::new(&rustc).args(["--edition=2021", "-C", &format!("opt-level={opt}")])
                .arg(root.join("main.rs")).arg("-o").arg(&executable).output().unwrap();
            assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
            let ran = Command::new(executable).output().unwrap();
            assert_eq!(ran.status.success(), expected, "{kind}/{label}: {}", String::from_utf8_lossy(&ran.stderr));
            if expected { assert_eq!(ran.stdout, b"owned-v1-copy-ok\n"); }
            else { assert!(String::from_utf8_lossy(&ran.stderr).contains("copy-call-count")); }
        }
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        // This fresh, uniquely named directory contains only files made here.
        let _ = fs::remove_dir_all(&self.0);
    }
}
