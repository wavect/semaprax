use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, MAX_CXX_OWNED_DATA_PACKAGE_BYTES};

struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "semaprax-cxx-owned-data-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path.canonicalize().unwrap()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("temporary directory: {error}"),
            }
        }
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
impl std::ops::Deref for Temporary {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl AsRef<std::path::Path> for Temporary {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

#[test]
fn package_is_exact_replayed_bounded_and_v8_only() {
    let root = Temporary::new();
    let manifest = crate::owned_mixed_arity_product::write_project(&root, 4);
    with_authenticated_project(&manifest, |snapshot| {
        let package = snapshot.cxx_owned_data_package_v1()?;
        assert!(package.canonical_bytes().len() <= MAX_CXX_OWNED_DATA_PACKAGE_BYTES);
        assert!(package.c_header().contains("extern \"C\""));
        assert!(package
            .cxx_header()
            .contains("Client(const Client&)=delete"));
        assert!(package
            .cxx_header()
            .contains("if(tag!=UINT32_MAX||handle!=0||error!=INT64_MIN)std::terminate()"));
        let replay = snapshot
            .replay_cxx_owned_data_package_v1(package.canonical_bytes(), package.digest())?;
        assert_eq!(replay, package);
        let mut reminted = package.canonical_bytes().to_vec();
        *reminted.last_mut().unwrap() ^= 1;
        assert!(snapshot
            .replay_cxx_owned_data_package_v1(&reminted, package.digest())
            .is_err());
        assert!(snapshot
            .replay_cxx_owned_data_package_v1(package.canonical_bytes(), &"0".repeat(64))
            .is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn generated_provider_and_wrapper_compile_separately_at_o0_and_o2() {
    let root = Temporary::new();
    let manifest = crate::owned_mixed_arity_product::write_project(&root, 4);
    let (package, method) = with_authenticated_project(&manifest, |snapshot| {
        let method = snapshot.public_api_descriptor()?.exports()[0]
            .rust_method_name()
            .to_owned();
        Ok((snapshot.cxx_owned_data_package_v1()?, method))
    })
    .unwrap();
    fs::write(root.join("semaprax_owned_data.h"), package.c_header()).unwrap();
    fs::write(root.join("semaprax_owned_data.hpp"), package.cxx_header()).unwrap();
    fs::write(root.join("provider.c"), package.provider_c()).unwrap();
    fs::write(root.join("consumer.cpp"), format!("#include \"semaprax_owned_data.hpp\"\nint main(){{semaprax::owned_data_v1::Client client;auto value=client.{method}();return value==semaprax::owned_data_v1::Bytes{{111,107}}?0:1;}}\n")).unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for optimization in ["-O0", "-O2"] {
        let provider_object = format!("provider{optimization}.o");
        let consumer_object = format!("consumer{optimization}.o");
        for (compiler, language, input, output) in [
            (&clang, "-std=c11", "provider.c", provider_object.as_str()),
            (&cxx, "-std=c++17", "consumer.cpp", consumer_object.as_str()),
        ] {
            let result = Command::new(compiler)
                .current_dir(&root)
                .args([
                    language,
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-c",
                    input,
                    "-o",
                    output,
                ])
                .output()
                .expect("Clang/Clang++ are required for C++ package evidence");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let executable = format!("consumer{optimization}");
        let result = Command::new(&cxx)
            .current_dir(&root)
            .args([
                provider_object.as_str(),
                consumer_object.as_str(),
                "-o",
                executable.as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(Command::new(root.join(executable))
            .status()
            .unwrap()
            .success());
    }
}
