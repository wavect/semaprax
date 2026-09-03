use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, MAX_CXX_OWNED_DATA_PACKAGE_BYTES};

#[path = "../support/owned_result_product.rs"]
mod result_subject;

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

#[test]
fn exact_input_and_output_limit_succeeds_plus_one_rejects_and_recovers() {
    let root = Temporary::new();
    let manifest = result_subject::write_project(&root);
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
    fs::write(
        root.join("limits.cpp"),
        format!(
            r#"#include "semaprax_owned_data.hpp"
#include <stdexcept>
int main(){{
  using namespace semaprax::owned_data_v1;
  Client client;
  Bytes input(65537,UINT8_C(7));
  bool rejected=false;
  try{{(void)client.{method}(ByteView(input.data(),UINT64_C(65537)));}}
  catch(const std::length_error&){{rejected=true;}}
  if(!rejected)return 1;
  auto first=client.{method}(ByteView(input.data(),UINT64_C(65536)));
  if(!std::holds_alternative<Bytes>(first)||std::get<Bytes>(first).size()!=65536)return 2;
  auto second=client.{method}(ByteView(input.data(),UINT64_C(65536)));
  return std::holds_alternative<Bytes>(second)&&std::get<Bytes>(second)==std::get<Bytes>(first)?0:3;
}}
"#
        ),
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for optimization in ["-O0", "-O2"] {
        let provider = format!("limit-provider-{optimization}.o");
        let consumer = format!("limit-consumer-{optimization}.o");
        let executable = format!("limit-{optimization}");
        for (compiler, args) in [
            (
                &clang,
                vec![
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-c",
                    "provider.c",
                    "-o",
                    provider.as_str(),
                ],
            ),
            (
                &cxx,
                vec![
                    "-std=c++17",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-c",
                    "limits.cpp",
                    "-o",
                    consumer.as_str(),
                ],
            ),
        ] {
            let output = Command::new(compiler)
                .current_dir(&root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let output = Command::new(&cxx)
            .current_dir(&root)
            .args([
                provider.as_str(),
                consumer.as_str(),
                "-o",
                executable.as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(root.join(executable))
            .status()
            .unwrap()
            .success());
    }
}

#[test]
fn synthetic_provider_exercises_poison_handles_and_settlement_uncertainty() {
    let root = Temporary::new();
    let manifest = result_subject::write_project(&root);
    let (package, method) = with_authenticated_project(&manifest, |snapshot| {
        let method = snapshot.public_api_descriptor()?.exports()[0]
            .rust_method_name()
            .to_owned();
        Ok((snapshot.cxx_owned_data_package_v1()?, method))
    })
    .unwrap();
    fs::write(root.join("semaprax_owned_data.h"), package.c_header()).unwrap();
    fs::write(root.join("semaprax_owned_data.hpp"), package.cxx_header()).unwrap();
    fs::write(root.join("fault.c"), format!(r#"#include "semaprax_owned_data.h"
#include <stddef.h>
#include <string.h>
struct spx_owned_data_context_v1{{uint64_t marker;uint64_t serial;uint8_t live;}};
static uint32_t mode;
void set_mode(uint32_t value){{mode=value;}}
uint64_t spx_owned_data_context_size_v1(void){{return sizeof(spx_context_v1);}}
uint64_t spx_owned_data_context_align_v1(void){{return _Alignof(spx_context_v1);}}
spx_owned_data_status_v1 spx_owned_data_context_init_v1(void* p,uint64_t n){{if(!p||n!=sizeof(spx_context_v1))return 2;spx_context_v1*c=p;memset(c,0,n);c->marker=7;return 0;}}
spx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1*c){{if(mode==6)return 5;if(!c||c->marker!=7||c->live)return 5;c->marker=0;return 0;}}
spx_owned_data_status_v1 spx_owned_bytes_len_v1(spx_context_v1*c,uint64_t h,uint64_t*n){{if(!c||c->marker!=7||!c->live||h!=c->serial)return 3;*n=mode==8?UINT64_C(65537):UINT64_C(2);return 0;}}
spx_owned_data_status_v1 spx_owned_bytes_copy_v1(spx_context_v1*c,uint64_t h,uint8_t*d,uint64_t n){{if(!c||!c->live||h!=c->serial)return 3;if(mode==4)return 4;if(n!=2||!d)return 4;d[0]=111;d[1]=107;return 0;}}
spx_owned_data_status_v1 spx_owned_bytes_drop_v1(spx_context_v1*c,uint64_t h){{if(!c||!c->live||h!=c->serial)return 3;if(mode==5)return 5;c->live=0;return 0;}}
spx_owned_data_status_v1 spx_owned_data_call_{method}_v1(spx_context_v1*c,const uint8_t*p,uint64_t n,uint32_t*t,uint64_t*h,int64_t*e){{(void)p;(void)n;if(mode==1)return 1;if(mode==2){{*t=0;return 1;}}if(!c||c->marker!=7||c->live)return 2;c->serial+=1;c->live=1;*t=mode==3?99:0;*h=c->serial;*e=0;return 0;}}
"#)).unwrap();
    fs::write(root.join("fault.cpp"), format!(r#"#include "semaprax_owned_data.hpp"
#include <cstdlib>
extern "C" void set_mode(uint32_t);
int main(int argc,char**argv){{using namespace semaprax::owned_data_v1;unsigned m=argc>1?(unsigned)std::strtoul(argv[1],nullptr,10):0;set_mode(m);uint8_t x=1;
if(m==7){{auto n=spx_owned_data_context_size_v1(),a=spx_owned_data_context_align_v1();void*p=::operator new((size_t)n,std::align_val_t((size_t)a));void*q=::operator new((size_t)n,std::align_val_t((size_t)a));auto*c1=(spx_context_v1*)p;auto*c2=(spx_context_v1*)q;if(spx_owned_data_context_init_v1(p,n)||spx_owned_data_context_init_v1(q,n))return 20;uint32_t t=UINT32_MAX;uint64_t h=0;int64_t e=INT64_MIN;if(spx_owned_data_call_{method}_v1(c1,&x,1,&t,&h,&e))return 21;uint64_t len=0;if(spx_owned_bytes_len_v1(c2,h,&len)!=SPX_OWNED_DATA_INVALID_HANDLE)return 22;if(spx_owned_bytes_drop_v1(c1,h))return 23;if(spx_owned_bytes_len_v1(c1,h,&len)!=SPX_OWNED_DATA_INVALID_HANDLE||spx_owned_bytes_drop_v1(c1,h)!=SPX_OWNED_DATA_INVALID_HANDLE)return 24;if(spx_owned_data_context_drop_v1(c1)||spx_owned_data_context_drop_v1(c2))return 25;::operator delete(p,std::align_val_t((size_t)a));::operator delete(q,std::align_val_t((size_t)a));return 0;}}
Client c;
try{{auto r=c.{method}(ByteView(&x,1));if(m==1||m==4||m==8)return 10;return std::holds_alternative<Bytes>(r)?0:11;}}catch(const Failure&f){{if((m==1&&f.status()==1)||(m==4&&f.status()==4)||(m==8&&f.status()==2)){{set_mode(0);auto r=c.{method}(ByteView(&x,1));return std::holds_alternative<Bytes>(r)?0:12;}}return 13;}}
}}
"#)).unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for (compiler, args) in [
        (
            &clang,
            vec![
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "fault.c", "-o", "fault.o",
            ],
        ),
        (
            &cxx,
            vec![
                "-std=c++17",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
                "fault.cpp",
                "-o",
                "fault-cxx.o",
            ],
        ),
    ] {
        let output = Command::new(compiler)
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new(&cxx)
        .current_dir(&root)
        .args(["fault.o", "fault-cxx.o", "-o", "fault"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for mode in [0, 1, 4, 7, 8] {
        assert!(Command::new(root.join("fault"))
            .arg(mode.to_string())
            .status()
            .unwrap()
            .success());
    }
    for mode in [2, 3, 5, 6] {
        assert!(!Command::new(root.join("fault"))
            .arg(mode.to_string())
            .status()
            .unwrap()
            .success());
    }
}
