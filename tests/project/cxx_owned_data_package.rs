use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, MAX_CXX_OWNED_DATA_PACKAGE_BYTES};

use crate::owned_result_product as result_subject;

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
fn maximum_export_inventory_fits_the_provider_wrapper_reserve() {
    let root = Temporary::new();
    fs::create_dir(root.join("src")).unwrap();
    let mut source = String::from("module maximum.app;\n");
    let mut exports = Vec::new();
    for index in 0..32 {
        let id = format!("maximum.value{index:02}");
        exports.push(format!("\"{id}\""));
        source.push_str(&format!("@id(\"{id}\") fn value{index:02}() -> Bytes {{ let value = [{index}u8]; bytes_copy(array_as_slice(value)) }}\n"));
    }
    source.push_str("@id(\"maximum.main\") fn main() -> i64 { 0 }\n");
    let checked = semaprax::check(&source, "maximum.spx").unwrap();
    fs::write(
        root.join("src/app.spx"),
        semaprax::format::canonical(&checked),
    )
    .unwrap();
    let tests = semaprax::check(
        "module maximum.tests; @id(\"maximum.tests.main\") fn main() -> i64 { 0 }",
        "tests.spx",
    )
    .unwrap();
    fs::write(
        root.join("src/tests.spx"),
        semaprax::format::canonical(&tests),
    )
    .unwrap();
    let manifest = root.join("semaprax.toml");
    fs::write(&manifest, format!("schema = \"semaprax.project.v8\"\nname = \"maximum-cxx\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"maximum.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [{}]\ntests = [\"maximum.tests\"]\n", exports.join(", "))).unwrap();
    let package =
        with_authenticated_project(&manifest, |snapshot| snapshot.cxx_owned_data_package_v1())
            .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(package.descriptor()).unwrap()["exports"]
            .as_array()
            .unwrap()
            .len(),
        32
    );
    assert!(
        package.provider_c().len() <= 2 * 1024 * 1024,
        "maximum admitted export inventory exceeded the reserved structural provider budget"
    );
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
fn generated_provider_and_c11_consumer_compile_link_and_settle_owned_bytes() {
    let root = Temporary::new();
    let manifest = crate::owned_mixed_arity_product::write_project(&root, 4);
    let (package, method) = with_authenticated_project(&manifest, |snapshot| {
        let method = snapshot.public_api_descriptor()?.exports()[4]
            .rust_method_name()
            .to_owned();
        Ok((snapshot.cxx_owned_data_package_v1()?, method))
    })
    .unwrap();
    fs::write(root.join("semaprax_owned_data.h"), package.c_header()).unwrap();
    fs::write(root.join("provider.c"), package.provider_c()).unwrap();
    fs::write(
        root.join("consumer.c"),
        format!(
            r#"#include "semaprax_owned_data.h"
#include <stddef.h>
#include <stdint.h>
#include <string.h>

union aligned_context {{ max_align_t alignment; uint8_t bytes[UINT64_C(1) << 20]; }};

int main(void) {{
    union aligned_context storage;
    uint64_t size = spx_owned_data_context_size_v1();
    uint64_t align = spx_owned_data_context_align_v1();
    if (size == 0 || size > sizeof(storage.bytes) || align == 0 || align > _Alignof(max_align_t)) return 1;
    if (spx_owned_data_context_init_v1(storage.bytes, size) != SPX_OWNED_DATA_SUCCESS) return 2;
    spx_context_v1 *context = (spx_context_v1 *)storage.bytes;
    const uint8_t text[] = {{ 'f', 'o', 'u', 'r' }};
    const uint8_t input[] = {{ 1, 2, 3 }};
    uint32_t tag = UINT32_MAX;
    spx_owned_bytes_handle_v1 handle = UINT64_C(0);
    int64_t error = INT64_MIN;
    if (spx_owned_data_call_{method}_v1(context, INT64_C(-13), UINT8_C(2), text, sizeof(text), input, sizeof(input), &tag, &handle, &error) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 3;
    if (tag != UINT32_MAX || handle != UINT64_C(0) || error != INT64_MIN) return 4;
    if (spx_owned_data_call_{method}_v1(context, INT64_C(-13), UINT8_C(1), text, sizeof(text), input, sizeof(input), &tag, &handle, &error) != SPX_OWNED_DATA_SUCCESS) return 5;
    if (tag != UINT32_C(0) || handle == UINT64_C(0) || error != INT64_C(0)) return 6;
    uint64_t length = UINT64_MAX;
    if (spx_owned_bytes_len_v1(context, handle, &length) != SPX_OWNED_DATA_SUCCESS || length != UINT64_C(2)) return 7;
    uint8_t output[2] = {{ 0, 0 }};
    if (spx_owned_bytes_copy_v1(context, handle, output, length) != SPX_OWNED_DATA_SUCCESS || memcmp(output, "ok", 2) != 0) return 8;
    if (spx_owned_bytes_drop_v1(context, handle) != SPX_OWNED_DATA_SUCCESS) return 9;
    if (spx_owned_bytes_len_v1(context, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE) return 10;
    if (spx_owned_data_context_drop_v1(context) != SPX_OWNED_DATA_SUCCESS) return 11;
    return 0;
}}
"#
        ),
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in ["-O0", "-O2"] {
        let provider = format!("c-provider-{optimization}.o");
        let consumer = format!("c-consumer-{optimization}.o");
        let executable = format!("c-consumer-{optimization}");
        for (input, output) in [
            ("provider.c", provider.as_str()),
            ("consumer.c", consumer.as_str()),
        ] {
            let result = Command::new(&clang)
                .current_dir(&root)
                .args([
                    "-std=c11",
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
                .expect("Clang is required for C package evidence");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let result = Command::new(&clang)
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
spx_owned_data_status_v1 spx_owned_data_call_{method}_v1(spx_context_v1*c,const uint8_t*p,uint64_t n,uint32_t*t,uint64_t*h,int64_t*e){{(void)p;(void)n;if(mode==1)return 1;if(mode==2){{*t=0;return 1;}}if(mode==9)return 99;if(!c||c->marker!=7||c->live)return 2;c->serial+=1;c->live=1;*t=mode==3?99:0;*h=c->serial;*e=0;return 0;}}
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
    for mode in [2, 3, 5, 6, 9] {
        assert!(!Command::new(root.join("fault"))
            .arg(mode.to_string())
            .status()
            .unwrap()
            .success());
    }
}

#[test]
fn synthetic_scalar_provider_preserves_poison_and_rejects_unknown_status() {
    let root = Temporary::new();
    fs::create_dir(root.join("src")).unwrap();
    let source = "module scalar.app; @id(\"scalar.value\") fn value() -> i64 { 7 } @id(\"scalar.main\") fn main() -> i64 { 0 }";
    let checked = semaprax::check(source, "scalar.spx").unwrap();
    fs::write(
        root.join("src/app.spx"),
        semaprax::format::canonical(&checked),
    )
    .unwrap();
    fs::write(
        root.join("src/tests.spx"),
        semaprax::format::canonical(
            &semaprax::check(
                "module scalar.tests; @id(\"scalar.tests.main\") fn main() -> i64 { 0 }",
                "tests.spx",
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let manifest = root.join("semaprax.toml");
    fs::write(&manifest,"schema = \"semaprax.project.v8\"\nname = \"scalar-cxx\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"scalar.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"scalar.value\"]\ntests = [\"scalar.tests\"]\n").unwrap();
    let (package, method) = with_authenticated_project(&manifest, |snapshot| {
        let method = snapshot.public_api_descriptor()?.exports()[0]
            .rust_method_name()
            .to_owned();
        Ok((snapshot.cxx_owned_data_package_v1()?, method))
    })
    .unwrap();
    fs::write(root.join("semaprax_owned_data.h"), package.c_header()).unwrap();
    fs::write(root.join("semaprax_owned_data.hpp"), package.cxx_header()).unwrap();
    fs::write(root.join("scalar.c"),format!(r#"#include "semaprax_owned_data.h"
#include <string.h>
struct spx_owned_data_context_v1{{uint64_t marker;}};static uint32_t mode;void set_mode(uint32_t m){{mode=m;}}uint64_t spx_owned_data_context_size_v1(void){{return sizeof(spx_context_v1);}}uint64_t spx_owned_data_context_align_v1(void){{return _Alignof(spx_context_v1);}}uint32_t spx_owned_data_context_init_v1(void*p,uint64_t n){{if(!p||n!=sizeof(spx_context_v1))return 2;memset(p,0,n);((spx_context_v1*)p)->marker=7;return 0;}}uint32_t spx_owned_data_context_drop_v1(spx_context_v1*c){{if(!c||c->marker!=7)return 5;c->marker=0;return 0;}}uint32_t spx_owned_bytes_len_v1(spx_context_v1*c,uint64_t h,uint64_t*n){{(void)c;(void)h;(void)n;return 3;}}uint32_t spx_owned_bytes_copy_v1(spx_context_v1*c,uint64_t h,uint8_t*d,uint64_t n){{(void)c;(void)h;(void)d;(void)n;return 3;}}uint32_t spx_owned_bytes_drop_v1(spx_context_v1*c,uint64_t h){{(void)c;(void)h;return 3;}}uint32_t spx_owned_data_call_{method}_v1(spx_context_v1*c,int64_t*out){{if(!c||c->marker!=7)return 2;if(mode==1)return 1;if(mode==2){{*out=0;return 1;}}if(mode==9)return 99;*out=7;return 0;}}
"#)).unwrap();
    fs::write(root.join("scalar.cpp"),format!(r#"#include "semaprax_owned_data.hpp"
#include <cstdlib>
extern "C" void set_mode(uint32_t);int main(int argc,char**argv){{unsigned m=argc>1?(unsigned)std::strtoul(argv[1],nullptr,10):0;set_mode(m);semaprax::owned_data_v1::Client c;try{{auto value=c.{method}();if(m==1)return 10;return value==7?0:11;}}catch(const semaprax::owned_data_v1::Failure&f){{if(m==1&&f.status()==1){{set_mode(0);return c.{method}()==7?0:12;}}return 13;}}}}
"#)).unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let cxx = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for (compiler, args) in [
        (
            &clang,
            vec![
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "scalar.c", "-o", "scalar.o",
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
                "scalar.cpp",
                "-o",
                "scalar-cxx.o",
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
        .args(["scalar.o", "scalar-cxx.o", "-o", "scalar"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for mode in [0, 1] {
        assert!(Command::new(root.join("scalar"))
            .arg(mode.to_string())
            .status()
            .unwrap()
            .success());
    }
    for mode in [2, 9] {
        assert!(!Command::new(root.join("scalar"))
            .arg(mode.to_string())
            .status()
            .unwrap()
            .success());
    }
}
