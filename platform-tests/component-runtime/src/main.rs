//! Hosted typed Wasmtime Component Model evidence for private result v3.

use std::{error::Error, io, path::Path};

use sha2::{Digest, Sha256};
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};

wasmtime::component::bindgen!({
    path: "wit",
    world: "semaprax-private-v1",
    ownership: Owning,
    additional_derives: [Eq, PartialEq],
});

use exports::semaprax::private::evaluation::Status;

type HostResult<T> = Result<T, Box<dyn Error>>;
type Evaluation = Result<i64, Status>;

const SOURCE: &str = r#"module test.component_result_v3;

@id("component.evaluate")
fn evaluate(left: i64, right: i64) -> i64
    requires right != 7
    ensures result != 9
{
    (left + 1) / right
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

// These independent known answers are replaced only alongside a reviewed
// component-profile version change. Artifact accessor metadata is never used
// as runtime authority.
const EXPECTED_COMPONENT_SHA256: [u8; 32] = [
    0x4c, 0xc1, 0x69, 0xb2, 0x2b, 0xbf, 0x42, 0xf8, 0xfc, 0x2c, 0xbb, 0x8a, 0x37, 0x97, 0x0b, 0x11,
    0x67, 0x6b, 0xc3, 0x7d, 0xd7, 0xe3, 0x1a, 0x9f, 0x80, 0xd5, 0xa7, 0xdc, 0x76, 0x29, 0x7f, 0xb1,
];
const EXPECTED_GENERATED_CORE_SHA256: [u8; 32] = [
    0x57, 0x3a, 0xa1, 0xd8, 0x1b, 0x63, 0x4e, 0xf6, 0xe9, 0x7e, 0x54, 0x06, 0x2f, 0x97, 0xd8, 0x74,
    0x7e, 0x53, 0x99, 0x4f, 0xd9, 0xf8, 0x40, 0x36, 0xe3, 0xd0, 0x8d, 0x67, 0xe7, 0xab, 0xe2, 0xba,
];
const EXPECTED_PROFILE_SHA256: [u8; 32] = [
    222, 215, 48, 247, 69, 152, 10, 90, 86, 167, 93, 149, 152, 80, 26, 184, 41, 24, 28, 36, 66,
    136, 84, 206, 88, 224, 108, 189, 68, 18, 50, 98,
];

#[derive(Clone, Copy)]
enum Expected {
    Ok(i64),
    Err {
        domain: &'static str,
        code: u32,
        class: u8,
    },
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    left: i64,
    right: i64,
    expected: Expected,
}

const CASES: [Case; 6] = [
    Case {
        name: "success",
        left: 83,
        right: 2,
        expected: Expected::Ok(42),
    },
    Case {
        name: "addition-overflow",
        left: i64::MAX,
        right: 1,
        expected: Expected::Err {
            domain: "semaprax.arithmetic.v1",
            code: 1,
            class: 2,
        },
    },
    Case {
        name: "division-by-zero",
        left: 1,
        right: 0,
        expected: Expected::Err {
            domain: "semaprax.arithmetic.v1",
            code: 4,
            class: 2,
        },
    },
    Case {
        name: "sticky-add-overflow-before-division-by-zero",
        left: i64::MAX,
        right: 0,
        expected: Expected::Err {
            domain: "semaprax.arithmetic.v1",
            code: 1,
            class: 2,
        },
    },
    Case {
        name: "false-precondition",
        left: 1,
        right: 7,
        expected: Expected::Err {
            domain: "semaprax.contract.v1",
            code: 1,
            class: 1,
        },
    },
    Case {
        name: "false-postcondition",
        left: 17,
        right: 2,
        expected: Expected::Err {
            domain: "semaprax.contract.v1",
            code: 2,
            class: 1,
        },
    },
];

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}

fn assert_outcome(case: Case, outcome: &Evaluation) -> HostResult<()> {
    match (case.expected, outcome) {
        (Expected::Ok(expected), Ok(actual)) if *actual == expected => Ok(()),
        (
            Expected::Err {
                domain,
                code,
                class,
            },
            Err(status),
        ) if status.domain == domain
            && status.code == code
            && status.class == class
            && status.retryable == Some(false) =>
        {
            Ok(())
        }
        _ => Err(failure(format!(
            "{} returned an unexpected typed result: {outcome:?}",
            case.name
        ))),
    }
}

fn invoke(bindings: &SemapraxPrivateV1, store: &mut Store<()>, case: Case) -> HostResult<()> {
    // Wasmtime 47's generated typed call delegates to `TypedFunc::call`, which
    // completes canonical ABI post_return processing synchronously. Running
    // the complete sequence repeatedly on one instance proves that no pending
    // post-return state leaks across calls.
    let outcome = bindings
        .semaprax_private_evaluation()
        .call_evaluate(store, case.left, case.right)?;
    assert_outcome(case, &outcome)
}

fn instantiate_and_run(
    engine: &Engine,
    component: &Component,
    cases: impl IntoIterator<Item = Case>,
) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(10_000_000)?;
    let bindings = SemapraxPrivateV1::instantiate(&mut store, component, &linker)?;
    for case in cases {
        invoke(&bindings, &mut store, case)?;
    }
    Ok(())
}

fn prove_engine_failure_is_out_of_band(engine: &Engine, component: &Component) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(1_000_000)?;
    let bindings = SemapraxPrivateV1::instantiate(&mut store, component, &linker)?;
    store.set_fuel(0)?;
    let engine_failure = bindings
        .semaprax_private_evaluation()
        .call_evaluate(&mut store, 83, 2);
    if engine_failure.is_ok() {
        return Err(failure(
            "fuel exhaustion did not remain an out-of-band Wasmtime error",
        ));
    }
    Ok(())
}

fn run() -> HostResult<()> {
    let program = ::semaprax::check(SOURCE, Path::new("component-result-v3.spx"))
        .map_err(|diagnostics| failure(format!("fixture verification failed: {diagnostics:?}")))?;
    let artifact = ::semaprax::wit_component::emit_private_result_component_v3(&program)
        .map_err(|diagnostic| failure(format!("component emission failed: {diagnostic:?}")))?;
    if artifact.wit() != include_str!("../wit/semaprax-private-v1.wit") {
        return Err(failure(
            "checked-in Wasmtime WIT drifted from the compiler fixture",
        ));
    }
    if artifact.generated_core_digest() != EXPECTED_GENERATED_CORE_SHA256
        || artifact.profile_digest() != EXPECTED_PROFILE_SHA256
    {
        return Err(failure(format!(
            "compiler component provenance changed: generated={:02x?}, profile={:02x?}",
            artifact.generated_core_digest(),
            artifact.profile_digest()
        )));
    }

    let bytes: Box<[u8]> = artifact.bytes().to_vec().into_boxed_slice();
    let before: [u8; 32] = Sha256::digest(&bytes).into();
    if before != EXPECTED_COMPONENT_SHA256 {
        return Err(failure(format!(
            "component bytes failed the independent SHA-256 known answer: {before:02x?}"
        )));
    }
    let expected_revision = ::semaprax::graph::revision(&program);
    let validated = ::semaprax::wit_component::validate_private_result_component_v3(
        &bytes,
        &expected_revision,
        EXPECTED_GENERATED_CORE_SHA256,
    )
    .map_err(|error| failure(format!("component profile validation failed: {error:?}")))?;
    if validated.interface_export_name() != "semaprax:private/evaluation@0.1.0"
        || validated.function_export_name() != "evaluate"
        || validated.source_revision() != expected_revision
    {
        return Err(failure(
            "independent component profile did not match the typed WIT",
        ));
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure(
            "private result component requested ambient imports",
        ));
    }
    let after: [u8; 32] = Sha256::digest(&bytes).into();
    if after != before {
        return Err(failure(
            "authenticated component bytes changed before compilation",
        ));
    }

    // One instance must reset its fixed result/status area and complete
    // Wasmtime's synchronous post-return work across two full sequences.
    instantiate_and_run(&engine, &component, CASES.into_iter().chain(CASES))?;
    // Fresh stores/instances must produce exactly the same typed outcomes.
    for case in CASES {
        instantiate_and_run(&engine, &component, [case])?;
    }
    prove_engine_failure_is_out_of_band(&engine, &component)?;
    println!("semaprax-private-component-runtime-v3-ok");
    Ok(())
}

fn main() -> HostResult<()> {
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_engine_runtime_covers_success_and_every_frozen_status() -> HostResult<()> {
        run()
    }
}
