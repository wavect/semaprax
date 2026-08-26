//! Generate nine target-bound private Apple Swift ownership fixtures.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semaprax::codegen::{
    emit_private_native_callable_v3_ios_corpus_fixture,
    emit_private_native_callable_v3_ios_fixture, PrivateNativeCallableV3Fixture,
    PrivateNativeCallableV3IosTarget,
};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

const FINALIZE_DECLARATION: &str =
    "static void spx_v3_generated_finalize(uint32_t owner_ordinal, uint64_t payload);\n";
const FINALIZE_EXTERN_DECLARATION: &str =
    "extern void spx_v3_generated_finalize(uint32_t,uint64_t);\n";

fn main() -> Result<(), Box<dyn Error>> {
    let outputs: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if outputs.len() != 9
        || outputs.iter().any(|path| !path.is_absolute())
        || (0..outputs.len())
            .any(|left| (left + 1..outputs.len()).any(|right| outputs[left] == outputs[right]))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"expected nine distinct absolute create-new output paths: device-arm64, simulator-arm64, simulator-x86_64, device-requires-false, simulator-arm64-requires-false, simulator-x86_64-requires-false, device-identity-max, simulator-arm64-identity-max, simulator-x86_64-identity-max").into());
    }
    let corpus = build_owned_resource_corpus_v1()
        .map_err(|e| io::Error::other(format!("build corpus: {e:?}")))?;
    let requires_false = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "requires-false")
        .ok_or_else(|| io::Error::other("requires-false corpus case is absent"))?;
    let requires_false_function = DeclarationId::new(requires_false.function_id);
    let identity_max = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "identity-max")
        .ok_or_else(|| io::Error::other("identity-max corpus case is absent"))?;
    if identity_max.expected_owned_result_ordinal != Some(0) {
        return Err(io::Error::other("identity-max corpus result ordinal diverged").into());
    }
    let identity_max_function = DeclarationId::new(identity_max.function_id);
    for (((discard_path, requires_false_path), identity_max_path), (target, tag)) in outputs[..3]
        .iter()
        .zip(outputs[3..6].iter())
        .zip(outputs[6..].iter())
        .zip([
            (PrivateNativeCallableV3IosTarget::DeviceArm64, 1_u32),
            (PrivateNativeCallableV3IosTarget::SimulatorArm64, 2),
            (PrivateNativeCallableV3IosTarget::SimulatorX86_64, 3),
        ])
    {
        let artifact = emit_private_native_callable_v3_ios_fixture(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .map_err(|e| io::Error::other(format!("emit Apple fixture: {e:?}")))?;
        let bound_discard = shared_trace_provider(artifact.source())
            .map_err(|e| io::Error::other(format!("bind discard finalize: {e}")))?;
        write_new(
            discard_path,
            render(
                &bound_discard,
                artifact.descriptor().len(),
                artifact.getter_symbol(),
                artifact.execute_symbol(),
                artifact.settle_symbol(),
                tag,
            )
            .as_bytes(),
        )?;
        let requires_false_artifact = emit_private_native_callable_v3_ios_corpus_fixture(
            &corpus.program,
            &requires_false_function,
            &requires_false.arguments,
            requires_false.expected_owned_result_ordinal,
            &requires_false.reference,
            target,
        )
        .map_err(|e| io::Error::other(format!("emit Apple requires-false fixture: {e:?}")))?;
        let bound_requires_false = shared_trace_provider(requires_false_artifact.source())
            .map_err(|e| io::Error::other(format!("bind requires-false finalize: {e}")))?;
        write_new(
            requires_false_path,
            render_requires_false(
                &bound_requires_false,
                requires_false_artifact.descriptor().len(),
                requires_false_artifact.getter_symbol(),
                requires_false_artifact.execute_symbol(),
                requires_false_artifact.settle_symbol(),
                tag,
            )
            .as_bytes(),
        )?;
        let identity_max_artifact = emit_private_native_callable_v3_ios_corpus_fixture(
            &corpus.program,
            &identity_max_function,
            &identity_max.arguments,
            identity_max.expected_owned_result_ordinal,
            &identity_max.reference,
            target,
        )
        .map_err(|e| io::Error::other(format!("emit Apple identity-max fixture: {e:?}")))?;
        let bound_identity_max = shared_trace_provider(identity_max_artifact.source())
            .map_err(|e| io::Error::other(format!("bind identity-max finalize: {e}")))?;
        write_new(
            identity_max_path,
            render_identity_max(
                &bound_identity_max,
                identity_max_artifact.descriptor().len(),
                identity_max_artifact.getter_symbol(),
                identity_max_artifact.execute_symbol(),
                identity_max_artifact.settle_symbol(),
                tag,
            )
            .as_bytes(),
        )?;
    }
    Ok(())
}

/// Both providers share one hidden trace-hook definition exported by the
/// discard object, so each static finalize forward declaration must become a
/// plain extern reference before the wrapper definition is appended.
fn shared_trace_provider(provider: &str) -> io::Result<String> {
    let offset = provider.find(FINALIZE_DECLARATION).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "finalize declaration absent")
    })?;
    let mut bound = String::with_capacity(provider.len());
    bound.push_str(&provider[..offset]);
    bound.push_str(FINALIZE_EXTERN_DECLARATION);
    bound.push_str(&provider[offset + FINALIZE_DECLARATION.len()..]);
    Ok(bound)
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

__attribute__((visibility("hidden"))) void spx_v3_generated_finalize(uint32_t ordinal,uint64_t payload){{
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

fn render_requires_false(
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

__attribute__((visibility("hidden"))) extern uint64_t spx_private_apple_swift_fixture_register_v1(uint32_t,const uint8_t *,uint32_t,spx_getter_fn,spx_execute_fn,spx_settle_fn);

__attribute__((visibility("default"))) uint64_t spx_private_apple_swift_fixture_rf_v1_open(void){{
  return spx_private_apple_swift_fixture_register_v1(UINT32_C({target}),{getter}(),UINT32_C({descriptor_len}),{getter},{execute},{settle});
}}
"#).expect("string write");
    source
}

fn render_identity_max(
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

__attribute__((visibility("hidden"))) extern uint64_t spx_private_apple_swift_fixture_register_v1(uint32_t,const uint8_t *,uint32_t,spx_getter_fn,spx_execute_fn,spx_settle_fn);

__attribute__((visibility("default"))) uint64_t spx_private_apple_swift_fixture_id_v1_open(void){{
  return spx_private_apple_swift_fixture_register_v1(UINT32_C({target}),{getter}(),UINT32_C({descriptor_len}),{getter},{execute},{settle});
}}
"#).expect("string write");
    source
}

#[cfg(test)]
mod tests {
    use super::{
        render, render_identity_max, render_requires_false, shared_trace_provider,
        FINALIZE_DECLARATION,
    };

    #[test]
    fn generated_fixture_freezes_closed_abi() {
        let provider = format!("/*provider*/\n{FINALIZE_DECLARATION}");
        let bound = shared_trace_provider(&provider).unwrap();
        let source = render(&bound, 731, "getter", "execute", "settle", 2);
        for required in [
            "/*provider*/",
            "spx_private_apple_swift_fixture_v1_open",
            "spx_private_apple_swift_fixture_register_v1(UINT32_C(2),getter(),UINT32_C(731)",
            "static _Thread_local uint32_t spx_swift_count",
            "__attribute__((visibility(\"hidden\"))) void spx_v3_generated_finalize",
            "visibility(\"hidden\"))) uint32_t spx_private_apple_swift_fixture_reset_v1",
            "visibility(\"hidden\"))) uint32_t spx_private_apple_swift_fixture_snapshot_v1",
        ] {
            assert!(source.contains(required), "missing `{required}`");
        }
        assert!(!source.contains("static void spx_v3_generated_finalize"));
        assert!(!source.contains("spx_private_apple_swift_v1_open"));
        assert!(!source.contains("spx_reset_fn,spx_snapshot_fn"));
    }

    #[test]
    fn requires_false_fixture_shares_hidden_trace_hooks() {
        let provider = format!("/*provider*/\n{FINALIZE_DECLARATION}");
        let bound = shared_trace_provider(&provider).unwrap();
        let source = render_requires_false(&bound, 734, "rf_getter", "rf_execute", "rf_settle", 2);
        for required in [
            "/*provider*/",
            "extern void spx_v3_generated_finalize(uint32_t,uint64_t);",
            "spx_private_apple_swift_fixture_rf_v1_open",
            "spx_private_apple_swift_fixture_register_v1(UINT32_C(2),rf_getter(),UINT32_C(734),rf_getter,rf_execute,rf_settle)",
        ] {
            assert!(source.contains(required), "missing `{required}`");
        }
        assert!(!source.contains("spx_private_apple_swift_fixture_v1_open"));
        assert!(!source.contains("spx_private_apple_swift_fixture_reset_v1"));
        assert!(!source.contains("spx_private_apple_swift_fixture_snapshot_v1"));
        assert!(!source.contains("static _Thread_local uint32_t spx_swift_count"));
        assert!(!source.contains("static void spx_v3_generated_finalize"));
    }

    #[test]
    fn identity_max_fixture_shares_hidden_trace_hooks() {
        let provider = format!("/*provider*/\n{FINALIZE_DECLARATION}");
        let bound = shared_trace_provider(&provider).unwrap();
        let source = render_identity_max(&bound, 738, "id_getter", "id_execute", "id_settle", 3);
        for required in [
            "/*provider*/",
            "extern void spx_v3_generated_finalize(uint32_t,uint64_t);",
            "spx_private_apple_swift_fixture_id_v1_open",
            "spx_private_apple_swift_fixture_register_v1(UINT32_C(3),id_getter(),UINT32_C(738),id_getter,id_execute,id_settle)",
        ] {
            assert!(source.contains(required), "missing `{required}`");
        }
        assert!(!source.contains("spx_private_apple_swift_fixture_v1_open"));
        assert!(!source.contains("spx_private_apple_swift_fixture_rf_v1_open"));
        assert!(!source.contains("spx_private_apple_swift_fixture_reset_v1"));
        assert!(!source.contains("spx_private_apple_swift_fixture_snapshot_v1"));
        assert!(!source.contains("static _Thread_local uint32_t spx_swift_count"));
        assert!(!source.contains("static void spx_v3_generated_finalize"));
    }

    #[test]
    fn providers_rebind_finalize_exactly_once() {
        let provider = format!("before\n{FINALIZE_DECLARATION}after\n");
        let bound = shared_trace_provider(&provider).unwrap();
        assert_eq!(
            bound,
            "before\nextern void spx_v3_generated_finalize(uint32_t,uint64_t);\nafter\n"
        );
        assert_eq!(
            shared_trace_provider("no declaration here")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}
