//! Generate three target-bound private Apple Swift ownership fixtures.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semaprax::codegen::{
    emit_private_native_callable_v3_ios_fixture, PrivateNativeCallableV3Fixture,
    PrivateNativeCallableV3IosTarget,
};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

fn main() -> Result<(), Box<dyn Error>> {
    let outputs: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if outputs.len() != 3
        || outputs.iter().any(|path| !path.is_absolute())
        || outputs[0] == outputs[1]
        || outputs[0] == outputs[2]
        || outputs[1] == outputs[2]
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"expected three distinct absolute create-new output paths: device-arm64, simulator-arm64, simulator-x86_64").into());
    }
    let corpus = build_owned_resource_corpus_v1()
        .map_err(|e| io::Error::other(format!("build corpus: {e:?}")))?;
    for ((path, target), tag) in outputs
        .iter()
        .zip([
            PrivateNativeCallableV3IosTarget::DeviceArm64,
            PrivateNativeCallableV3IosTarget::SimulatorArm64,
            PrivateNativeCallableV3IosTarget::SimulatorX86_64,
        ])
        .zip([1_u32, 2, 3])
    {
        let artifact = emit_private_native_callable_v3_ios_fixture(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .map_err(|e| io::Error::other(format!("emit Apple fixture: {e:?}")))?;
        write_new(
            path,
            render(
                artifact.source(),
                artifact.descriptor().len(),
                artifact.getter_symbol(),
                artifact.execute_symbol(),
                artifact.settle_symbol(),
                tag,
            )
            .as_bytes(),
        )?;
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

fn render(
    provider: &str,
    descriptor_len: usize,
    getter: &str,
    execute: &str,
    settle: &str,
    target: u32,
) -> String {
    let mut source = String::new();
    write!(source,r#"{provider}

#include <stdint.h>

typedef const uint8_t *(*spx_getter_fn)(void);
typedef uint32_t (*spx_execute_fn)(const uint8_t *,uint32_t,uint8_t *,uint32_t,uint8_t *,uint32_t);
typedef uint32_t (*spx_settle_fn)(uint8_t *,uint32_t,const uint8_t *,uint32_t,uint8_t *,uint32_t);
typedef uint32_t (*spx_reset_fn)(void);
typedef uint32_t (*spx_snapshot_fn)(uint32_t *,uint32_t *,uint64_t *,uint32_t);

__attribute__((visibility("hidden"))) extern uint64_t spx_private_apple_swift_fixture_register_v1(uint32_t,const uint8_t *,uint32_t,spx_getter_fn,spx_execute_fn,spx_settle_fn);

static _Thread_local uint32_t spx_swift_count;
static _Thread_local uint32_t spx_swift_ordinals[2];
static _Thread_local uint64_t spx_swift_payloads[2];

static void spx_v3_generated_finalize(uint32_t ordinal,uint64_t payload){{
  uint32_t index=spx_swift_count;
  if(index<UINT32_C(2)){{spx_swift_ordinals[index]=ordinal;spx_swift_payloads[index]=payload;}}
  spx_swift_count=index+UINT32_C(1);
}}
__attribute__((visibility("hidden"))) uint32_t spx_private_apple_swift_fixture_reset_v1(void){{spx_swift_count=0;spx_swift_ordinals[0]=spx_swift_ordinals[1]=0;spx_swift_payloads[0]=spx_swift_payloads[1]=0;return 0;}}
__attribute__((visibility("hidden"))) uint32_t spx_private_apple_swift_fixture_snapshot_v1(uint32_t *count,uint32_t *ordinals,uint64_t *payloads,uint32_t capacity){{
  if(count==0||ordinals==0||payloads==0||capacity!=2)return 1;
  *count=spx_swift_count;ordinals[0]=spx_swift_ordinals[0];ordinals[1]=spx_swift_ordinals[1];payloads[0]=spx_swift_payloads[0];payloads[1]=spx_swift_payloads[1];return 0;
}}
__attribute__((visibility("default"))) uint64_t spx_private_apple_swift_fixture_v1_open(void){{
  return spx_private_apple_swift_fixture_register_v1(UINT32_C({target}),{getter}(),UINT32_C({descriptor_len}),{getter},{execute},{settle});
}}
"#).expect("string write");
    source
}

#[cfg(test)]
mod tests {
    use super::render;
    #[test]
    fn generated_fixture_freezes_closed_abi() {
        let source = render("/*provider*/", 731, "getter", "execute", "settle", 2);
        for required in [
            "/*provider*/",
            "spx_private_apple_swift_fixture_v1_open",
            "spx_private_apple_swift_fixture_register_v1(UINT32_C(2),getter(),UINT32_C(731)",
            "static _Thread_local uint32_t spx_swift_count",
            "spx_v3_generated_finalize",
            "visibility(\"hidden\"))) uint32_t spx_private_apple_swift_fixture_reset_v1",
            "visibility(\"hidden\"))) uint32_t spx_private_apple_swift_fixture_snapshot_v1",
        ] {
            assert!(source.contains(required), "missing `{required}`");
        }
        assert!(!source.contains("spx_private_apple_swift_v1_open"));
        assert!(!source.contains("spx_reset_fn,spx_snapshot_fn"));
    }
}
