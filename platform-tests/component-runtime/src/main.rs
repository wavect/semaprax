//! Hosted typed Wasmtime Component Model evidence for private results v3/v4.

use std::{error::Error, io, path::Path};

use sha2::{Digest, Sha256};
use wasmtime::{
    Config, Engine, Instance, Module, Store,
    component::{Component, Linker},
};

wasmtime::component::bindgen!({
    path: "wit/semaprax-private-v1.wit",
    world: "semaprax-private-v1",
    ownership: Owning,
    additional_derives: [Eq, PartialEq],
});

mod v4_bindings {
    wasmtime::component::bindgen!({
        path: "wit/semaprax-private-v4.wit",
        world: "semaprax-private-v4",
        ownership: Owning,
        additional_derives: [Eq, PartialEq],
    });
}

mod v5_bindings {
    wasmtime::component::bindgen!({
        path: "wit/semaprax-private-v5.wit",
        world: "semaprax-private-v5",
        ownership: Owning,
        additional_derives: [Eq, PartialEq],
    });
}

mod v6_bindings {
    wasmtime::component::bindgen!({
        path: "wit/semaprax-private-v6.wit",
        world: "semaprax-private-v6",
        ownership: Owning,
        additional_derives: [Eq, PartialEq],
    });
}

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

const SOURCE_V4: &str = r#"module test.component_source_result_v4;

@id("component.source")
fn source(value: i64, reject: bool) -> Result<i64, bool>
{
    if reject { Result<i64, bool>::Err { error: value > 0 } } else { Result<i64, bool>::Ok { value: value } }
}

@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const SOURCE_V5: &str = include_str!("../v5.spx");
const SOURCE_V6: &str = include_str!("../v6.spx");

// These independent known answers are replaced only alongside a reviewed
// component-profile version change. Artifact accessor metadata is never used
// as runtime authority.
const EXPECTED_COMPONENT_SHA256: [u8; 32] = [
    0x7d, 0x86, 0x44, 0x38, 0x49, 0x48, 0xf5, 0x91, 0xd6, 0xfe, 0x79, 0x29, 0xd0, 0xcf, 0xed, 0x9e,
    0xab, 0x79, 0xd1, 0x75, 0x28, 0xca, 0x56, 0x10, 0x07, 0xee, 0x45, 0x58, 0xe2, 0x93, 0x30, 0x27,
];
const EXPECTED_GENERATED_CORE_SHA256: [u8; 32] = [
    0xd5, 0x5f, 0x76, 0xa0, 0xe6, 0x97, 0x47, 0x77, 0x5c, 0x32, 0x93, 0xfd, 0x61, 0x6e, 0x2f, 0xf0,
    0x3a, 0x92, 0x1c, 0x65, 0x0c, 0xbe, 0xa9, 0x05, 0x5e, 0xc8, 0xca, 0x97, 0x77, 0xb8, 0x73, 0x6c,
];
const EXPECTED_PROFILE_SHA256: [u8; 32] = [
    222, 215, 48, 247, 69, 152, 10, 90, 86, 167, 93, 149, 152, 80, 26, 184, 41, 24, 28, 36, 66,
    136, 84, 206, 88, 224, 108, 189, 68, 18, 50, 98,
];

// Independent v4 known answers. These are intentionally not read from
// artifact metadata and are replaced only with a reviewed v4 profile change.
const EXPECTED_COMPONENT_V4_SHA256: [u8; 32] = [
    0x3e, 0x7b, 0x9c, 0x2d, 0xdc, 0x8c, 0xa6, 0xfd, 0xfa, 0x80, 0x1e, 0xb5, 0x0a, 0xe3, 0xa2, 0x15,
    0x31, 0xfc, 0xe4, 0x46, 0x77, 0x34, 0x5d, 0xde, 0xa6, 0x8d, 0x20, 0x58, 0x1c, 0x79, 0xb2, 0x3b,
];
const EXPECTED_GENERATED_CORE_V4_SHA256: [u8; 32] = [
    0x54, 0xfa, 0x28, 0x22, 0xc5, 0x1a, 0x71, 0xce, 0xbf, 0xd8, 0x8d, 0x37, 0x9b, 0x45, 0xc3, 0x7f,
    0xfd, 0x3d, 0x0f, 0x0b, 0x28, 0x93, 0xcb, 0x4f, 0x29, 0x66, 0xf9, 0xe2, 0xdb, 0x6d, 0x5e, 0x5f,
];
const EXPECTED_SOURCE_REVISION_V4: &str =
    "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254";
const EXPECTED_COMPONENT_V5_SHA256: [u8; 32] = [
    0x6c, 0xeb, 0x9e, 0x30, 0x96, 0x94, 0xa5, 0xb9, 0x60, 0x94, 0x49, 0x58, 0xa4, 0xb0, 0x52, 0x7e,
    0x29, 0xef, 0xa6, 0xba, 0xe8, 0xf7, 0xfc, 0x27, 0xe9, 0x4a, 0xd0, 0x1a, 0x84, 0x7b, 0xad, 0xca,
];
const EXPECTED_GENERATED_CORE_V5_SHA256: [u8; 32] = [
    0x08, 0x25, 0xf2, 0x70, 0xcf, 0x2c, 0x94, 0xbd, 0x75, 0x19, 0x01, 0xd0, 0x5d, 0x74, 0x29, 0x3e,
    0x52, 0xb6, 0x9b, 0xda, 0x00, 0xa1, 0xaf, 0x99, 0xcd, 0xfb, 0xc4, 0x72, 0x53, 0x5a, 0xf3, 0x1b,
];
const EXPECTED_SOURCE_REVISION_V5: &str =
    "sha256:86411224efe3adace5ffdd410c243306859edc280dbe3342adcf830588b62259";
const EXPECTED_COMPONENT_V6_SHA256: [u8; 32] = [
    0xad, 0x40, 0x8a, 0x7a, 0x6a, 0x35, 0x96, 0xa0, 0x26, 0xeb, 0x73, 0xbc, 0x42, 0x3e, 0x59, 0xf3,
    0x03, 0x50, 0xc0, 0xe4, 0xf7, 0xcb, 0xc5, 0x07, 0xce, 0x60, 0x51, 0x0e, 0xff, 0x2b, 0x53, 0x0f,
];
const EXPECTED_GENERATED_CORE_V6_SHA256: [u8; 32] = [
    0x42, 0x83, 0x5d, 0xcb, 0xf9, 0x80, 0x78, 0xac, 0x24, 0xbf, 0xd3, 0x65, 0x68, 0xf1, 0xb6, 0x91,
    0x7b, 0x5b, 0x64, 0xca, 0x2d, 0x82, 0x65, 0xef, 0x4d, 0xed, 0x16, 0x1d, 0x26, 0x43, 0x8d, 0xa1,
];
const EXPECTED_SOURCE_REVISION_V6: &str =
    "sha256:d1fcbc45b3d86fa1d7910378578828df3c557dba92f90ed9459f928c5bf2fe8a";

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

type EvaluationV4 =
    Result<Result<bool, bool>, v4_bindings::exports::semaprax::private::evaluation::Status>;

#[derive(Clone, Copy)]
enum ExpectedV4 {
    Value(Result<bool, bool>),
    Status {
        domain: &'static str,
        code: u32,
        class: u8,
    },
}

#[derive(Clone, Copy)]
struct CaseV4 {
    name: &'static str,
    value: i64,
    reject: bool,
    divisor: i64,
    expected: ExpectedV4,
}

const CASES_V4: [CaseV4; 10] = [
    CaseV4 {
        name: "inner-ok-true",
        value: 83,
        reject: false,
        divisor: 2,
        expected: ExpectedV4::Value(Ok(true)),
    },
    CaseV4 {
        name: "inner-ok-false",
        value: -3,
        reject: false,
        divisor: 2,
        expected: ExpectedV4::Value(Ok(false)),
    },
    CaseV4 {
        name: "inner-err-true",
        value: 1,
        reject: true,
        divisor: 0,
        expected: ExpectedV4::Value(Err(true)),
    },
    CaseV4 {
        name: "inner-err-false",
        value: -1,
        reject: true,
        divisor: 0,
        expected: ExpectedV4::Value(Err(false)),
    },
    CaseV4 {
        name: "addition-overflow",
        value: i64::MAX,
        reject: false,
        divisor: 1,
        expected: ExpectedV4::Status {
            domain: "semaprax.arithmetic.v1",
            code: 1,
            class: 2,
        },
    },
    CaseV4 {
        name: "division-by-zero",
        value: 1,
        reject: false,
        divisor: 0,
        expected: ExpectedV4::Status {
            domain: "semaprax.arithmetic.v1",
            code: 4,
            class: 2,
        },
    },
    CaseV4 {
        name: "sticky-add-overflow-before-division-by-zero",
        value: i64::MAX,
        reject: false,
        divisor: 0,
        expected: ExpectedV4::Status {
            domain: "semaprax.arithmetic.v1",
            code: 1,
            class: 2,
        },
    },
    CaseV4 {
        name: "false-precondition",
        value: -99,
        reject: false,
        divisor: 1,
        expected: ExpectedV4::Status {
            domain: "semaprax.contract.v1",
            code: 1,
            class: 1,
        },
    },
    CaseV4 {
        name: "false-postcondition-after-ok",
        value: 1,
        reject: false,
        divisor: 13,
        expected: ExpectedV4::Status {
            domain: "semaprax.contract.v1",
            code: 2,
            class: 1,
        },
    },
    CaseV4 {
        name: "false-postcondition-after-err",
        value: 1,
        reject: true,
        divisor: 13,
        expected: ExpectedV4::Status {
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

fn assert_outcome_v4(case: CaseV4, outcome: &EvaluationV4) -> HostResult<()> {
    match (case.expected, outcome) {
        (ExpectedV4::Value(expected), Ok(actual)) if *actual == expected => Ok(()),
        (
            ExpectedV4::Status {
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
            "{} returned an unexpected typed nested result: {outcome:?}",
            case.name
        ))),
    }
}

fn invoke_v4(
    bindings: &v4_bindings::SemapraxPrivateV4,
    store: &mut Store<()>,
    case: CaseV4,
) -> HostResult<()> {
    let outcome = bindings.semaprax_private_evaluation().call_evaluate(
        store,
        case.value,
        case.reject,
        case.divisor,
    )?;
    assert_outcome_v4(case, &outcome)
}

fn instantiate_and_run_v4(
    engine: &Engine,
    component: &Component,
    cases: impl IntoIterator<Item = CaseV4>,
) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(10_000_000)?;
    let bindings = v4_bindings::SemapraxPrivateV4::instantiate(&mut store, component, &linker)?;
    for case in cases {
        invoke_v4(&bindings, &mut store, case)?;
    }
    Ok(())
}

fn prove_engine_failure_is_out_of_band_v4(
    engine: &Engine,
    component: &Component,
) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(1_000_000)?;
    let bindings = v4_bindings::SemapraxPrivateV4::instantiate(&mut store, component, &linker)?;
    store.set_fuel(0)?;
    let engine_failure = bindings
        .semaprax_private_evaluation()
        .call_evaluate(&mut store, 83, false, 2);
    if engine_failure.is_ok() {
        return Err(failure(
            "fuel exhaustion did not remain an out-of-band Wasmtime error for v4",
        ));
    }
    Ok(())
}

fn run_v3() -> HostResult<()> {
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

fn run_v4() -> HostResult<()> {
    let program = ::semaprax::check(SOURCE_V4, Path::new("component-source-result-v4.spx"))
        .map_err(|diagnostics| {
            failure(format!("v4 fixture verification failed: {diagnostics:?}"))
        })?;
    let artifact = ::semaprax::wit_component::emit_private_source_result_component_v4(&program)
        .map_err(|diagnostic| failure(format!("v4 component emission failed: {diagnostic:?}")))?;
    if artifact.wit() != include_str!("../wit/semaprax-private-v4.wit") {
        return Err(failure(
            "checked-in Wasmtime v4 WIT drifted from the compiler fixture",
        ));
    }

    let bytes: Box<[u8]> = artifact.bytes().to_vec().into_boxed_slice();
    let before: [u8; 32] = Sha256::digest(&bytes).into();
    if before != EXPECTED_COMPONENT_V4_SHA256 {
        return Err(failure(format!(
            "v4 component bytes failed the independent SHA-256 known answer: {before:02x?}"
        )));
    }
    let expected_revision = ::semaprax::graph::revision(&program);
    if expected_revision != EXPECTED_SOURCE_REVISION_V4 {
        return Err(failure(format!(
            "v4 fixture source revision drifted: {expected_revision}"
        )));
    }
    let validated = ::semaprax::wit_component::validate_private_source_result_component_v4(
        &bytes,
        &expected_revision,
        EXPECTED_GENERATED_CORE_V4_SHA256,
    )
    .map_err(|error| failure(format!("v4 component profile validation failed: {error:?}")))?;
    if validated.interface_export_name() != "semaprax:private/evaluation@0.2.0"
        || validated.function_export_name() != "evaluate"
        || validated.source_revision() != expected_revision
    {
        return Err(failure(
            "independent v4 component profile did not match the typed WIT",
        ));
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure(
            "private nested-result component requested ambient imports",
        ));
    }
    let after: [u8; 32] = Sha256::digest(&bytes).into();
    if after != before {
        return Err(failure(
            "authenticated v4 component bytes changed before compilation",
        ));
    }

    instantiate_and_run_v4(&engine, &component, CASES_V4.into_iter().chain(CASES_V4))?;
    for case in CASES_V4 {
        instantiate_and_run_v4(&engine, &component, [case])?;
    }
    prove_engine_failure_is_out_of_band_v4(&engine, &component)?;
    println!("semaprax-private-component-runtime-v4-ok");
    Ok(())
}

// This is deliberately a flat, reviewable six-export protocol matrix. Splitting
// it behind generic helpers would obscure the exact WIT export-to-carrier map.
#[allow(clippy::too_many_lines)]
fn run_v5_instance(engine: &Engine, component: &Component) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(30_000_000)?;
    let bindings = v5_bindings::SemapraxPrivateV5::instantiate(&mut store, component, &linker)?;
    let api = bindings.semaprax_private_scalar_algebra();

    macro_rules! expect_value {
        ($call:expr, $pattern:pat, $name:literal) => {
            match $call? {
                $pattern => {}
                _ => return Err(failure(concat!("unexpected v5 value for ", $name))),
            }
        };
    }
    macro_rules! expect_status {
        ($call:expr, $domain:literal, $code:literal, $class:literal, $name:literal) => {
            match $call? {
                Err(status)
                    if status.domain == $domain
                        && status.code == $code
                        && status.class == $class
                        && status.retryable == Some(false) => {}
                _ => return Err(failure(concat!("unexpected v5 status for ", $name))),
            }
        };
    }

    expect_value!(
        api.call_option_i64(&mut store, 83, true, 2),
        Ok(Some(42)),
        "option-i64 some"
    );
    expect_value!(
        api.call_option_i64(&mut store, 1, false, 0),
        Ok(None),
        "option-i64 none skip"
    );
    expect_value!(
        api.call_option_bool(&mut store, 83, true, 2),
        Ok(Some(true)),
        "option-bool true"
    );
    expect_value!(
        api.call_option_bool(&mut store, -3, true, 2),
        Ok(Some(false)),
        "option-bool false"
    );
    expect_value!(
        api.call_option_bool(&mut store, 1, false, 0),
        Ok(None),
        "option-bool none skip"
    );

    expect_value!(
        api.call_result_i64_i64(&mut store, 83, false, 2),
        Ok(Ok(42)),
        "result-i64-i64 ok"
    );
    expect_value!(
        api.call_result_i64_i64(&mut store, 83, true, 0),
        Ok(Err(83)),
        "result-i64-i64 err skip"
    );
    expect_value!(
        api.call_result_i64_bool(&mut store, 83, false, 2),
        Ok(Ok(42)),
        "result-i64-bool ok"
    );
    expect_value!(
        api.call_result_i64_bool(&mut store, 1, true, 0),
        Ok(Err(true)),
        "result-i64-bool err true"
    );
    expect_value!(
        api.call_result_i64_bool(&mut store, -1, true, 0),
        Ok(Err(false)),
        "result-i64-bool err false"
    );
    expect_value!(
        api.call_result_bool_i64(&mut store, 83, false, 2),
        Ok(Ok(true)),
        "result-bool-i64 ok true"
    );
    expect_value!(
        api.call_result_bool_i64(&mut store, -3, false, 2),
        Ok(Ok(false)),
        "result-bool-i64 ok false"
    );
    expect_value!(
        api.call_result_bool_i64(&mut store, 83, true, 0),
        Ok(Err(83)),
        "result-bool-i64 err skip"
    );
    expect_value!(
        api.call_result_bool_bool(&mut store, 83, false, 2),
        Ok(Ok(true)),
        "result-bool-bool ok true"
    );
    expect_value!(
        api.call_result_bool_bool(&mut store, -3, false, 2),
        Ok(Ok(false)),
        "result-bool-bool ok false"
    );
    expect_value!(
        api.call_result_bool_bool(&mut store, 1, true, 0),
        Ok(Err(true)),
        "result-bool-bool err true"
    );
    expect_value!(
        api.call_result_bool_bool(&mut store, -1, true, 0),
        Ok(Err(false)),
        "result-bool-bool err false"
    );

    expect_status!(
        api.call_option_i64(&mut store, i64::MAX, true, 0),
        "semaprax.arithmetic.v1",
        1,
        2,
        "sticky add"
    );
    expect_status!(
        api.call_option_bool(&mut store, 1, true, 0),
        "semaprax.arithmetic.v1",
        4,
        2,
        "division zero"
    );
    expect_status!(
        api.call_result_i64_i64(&mut store, -99, false, 1),
        "semaprax.contract.v1",
        1,
        1,
        "requires"
    );
    expect_status!(
        api.call_result_i64_bool(&mut store, 1, true, 13),
        "semaprax.contract.v1",
        2,
        1,
        "ensures after err"
    );
    expect_status!(
        api.call_result_bool_i64(&mut store, 1, false, 13),
        "semaprax.contract.v1",
        2,
        1,
        "ensures after ok"
    );
    expect_status!(
        api.call_result_bool_bool(&mut store, 1, false, 0),
        "semaprax.arithmetic.v1",
        4,
        2,
        "result division zero"
    );
    Ok(())
}

fn prove_engine_failure_is_out_of_band_v5(
    engine: &Engine,
    component: &Component,
) -> HostResult<()> {
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(1_000_000)?;
    let bindings = v5_bindings::SemapraxPrivateV5::instantiate(&mut store, component, &linker)?;
    store.set_fuel(0)?;
    if bindings
        .semaprax_private_scalar_algebra()
        .call_option_i64(&mut store, 83, true, 2)
        .is_ok()
    {
        return Err(failure("v5 fuel exhaustion became a typed status"));
    }
    Ok(())
}

fn run_v5() -> HostResult<()> {
    let program = ::semaprax::check(SOURCE_V5, Path::new("component-scalar-algebra-v5.spx"))
        .map_err(|diagnostics| {
            failure(format!("v5 fixture verification failed: {diagnostics:?}"))
        })?;
    let artifact = ::semaprax::wit_component::emit_private_scalar_algebra_component_v5(&program)
        .map_err(|diagnostic| failure(format!("v5 component emission failed: {diagnostic:?}")))?;
    if artifact.wit() != include_str!("../wit/semaprax-private-v5.wit") {
        return Err(failure("checked-in Wasmtime v5 WIT drifted"));
    }
    let bytes: Box<[u8]> = artifact.bytes().to_vec().into_boxed_slice();
    let before: [u8; 32] = Sha256::digest(&bytes).into();
    if before != EXPECTED_COMPONENT_V5_SHA256
        || artifact.generated_core_digest() != EXPECTED_GENERATED_CORE_V5_SHA256
    {
        return Err(failure("v5 independent component/core KAT changed"));
    }
    let expected_revision = ::semaprax::graph::revision(&program);
    if expected_revision != EXPECTED_SOURCE_REVISION_V5 {
        return Err(failure("v5 source revision KAT changed"));
    }
    let validated = ::semaprax::wit_component::validate_private_scalar_algebra_component_v5(
        &bytes,
        EXPECTED_SOURCE_REVISION_V5,
        EXPECTED_GENERATED_CORE_V5_SHA256,
    )
    .map_err(|error| failure(format!("v5 profile validation failed: {error:?}")))?;
    if validated.interface_export_name() != "semaprax:private/scalar-algebra@0.3.0"
        || validated.function_export_names()
            != [
                "option-i64",
                "option-bool",
                "result-i64-i64",
                "result-i64-bool",
                "result-bool-i64",
                "result-bool-bool",
            ]
        || validated.type_export_names()
            != [
                "maybe-i64",
                "maybe-bool",
                "language-result-i64-i64",
                "language-result-i64-bool",
                "language-result-bool-i64",
                "language-result-bool-bool",
            ]
        || <[u8; 32]>::from(Sha256::digest(validated.generated_core()))
            != EXPECTED_GENERATED_CORE_V5_SHA256
    {
        return Err(failure("v5 typed export table changed"));
    }
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure("v5 requested ambient imports"));
    }
    run_v5_instance(&engine, &component)?;
    run_v5_instance(&engine, &component)?;
    prove_engine_failure_is_out_of_band_v5(&engine, &component)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != before {
        return Err(failure("authenticated v5 bytes changed during execution"));
    }
    println!("semaprax-private-component-runtime-v5-ok");
    Ok(())
}

fn expect_v6_status(
    value: Result<
        v6_bindings::exports::semaprax::private::nested_records::Outer,
        v6_bindings::exports::semaprax::private::nested_records::Status,
    >,
    domain: &str,
    code: u32,
    class: u8,
    name: &str,
) -> HostResult<()> {
    match value {
        Err(status)
            if status.domain == domain
                && status.code == code
                && status.class == class
                && status.retryable == Some(false) =>
        {
            Ok(())
        }
        _ => Err(failure(format!("unexpected v6 status for {name}"))),
    }
}

// Keep the exact nested-record field and status matrix visible in one reviewable
// function; splitting it would obscure the frozen WIT-to-SEMAPRAX mapping.
#[allow(clippy::too_many_lines)]
fn run_v6_instance(engine: &Engine, component: &Component) -> HostResult<()> {
    use v6_bindings::exports::semaprax::private::nested_records::{Inner, Outer};

    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(20_000_000)?;
    let bindings = v6_bindings::SemapraxPrivateV6::instantiate(&mut store, component, &linker)?;
    let api = bindings.semaprax_private_nested_records();

    for flag in [true, false] {
        let input = Outer {
            inner: Inner { value: 18, flag },
            other: 22,
        };
        let output = api.call_transform(&mut store, input, 2)?;
        let expected = Outer {
            inner: Inner { value: 20, flag },
            other: 22,
        };
        if output != Ok(expected) {
            return Err(failure("unexpected v6 nested-record success"));
        }
    }

    expect_v6_status(
        api.call_transform(
            &mut store,
            Outer {
                inner: Inner {
                    value: i64::MAX,
                    flag: true,
                },
                other: 22,
            },
            1,
        )?,
        "semaprax.arithmetic.v1",
        1,
        2,
        "sticky nested add before later division by zero",
    )?;
    expect_v6_status(
        api.call_transform(
            &mut store,
            Outer {
                inner: Inner {
                    value: 18,
                    flag: true,
                },
                other: 22,
            },
            1,
        )?,
        "semaprax.arithmetic.v1",
        4,
        2,
        "standalone division by zero",
    )?;
    expect_v6_status(
        api.call_transform(
            &mut store,
            Outer {
                inner: Inner {
                    value: 18,
                    flag: true,
                },
                other: 22,
            },
            -99,
        )?,
        "semaprax.contract.v1",
        1,
        1,
        "requires",
    )?;
    expect_v6_status(
        api.call_transform(
            &mut store,
            Outer {
                inner: Inner {
                    value: 18,
                    flag: false,
                },
                other: 24,
            },
            13,
        )?,
        "semaprax.contract.v1",
        2,
        1,
        "ensures",
    )?;
    Ok(())
}

fn prove_raw_core_v6_poison_status_and_invalid_bool(
    engine: &Engine,
    core: &[u8],
) -> HostResult<()> {
    let module = Module::new(engine, core)?;
    if module.imports().next().is_some() {
        return Err(failure("v6 raw core requested ambient imports"));
    }
    let mut store = Store::new(engine, ());
    store.set_fuel(20_000_000)?;
    let instance = Instance::new(&mut store, &module, &[])?;
    let transform = instance.get_typed_func::<(i64, i32, i64, i64), i32>(
        &mut store,
        "cabi_transform_nested_record_v6",
    )?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| failure("v6 raw core memory export missing"))?;

    let pointer = transform.call(&mut store, (18, 1, 22, 2))?;
    let pointer =
        usize::try_from(pointer).map_err(|_| failure("v6 raw result pointer was negative"))?;
    if pointer != 256 {
        return Err(failure("v6 raw result pointer changed"));
    }
    let mut success = [0_u8; 32];
    memory.read(&store, pointer, &mut success)?;
    if success[0] != 0
        || i64::from_le_bytes(success[8..16].try_into()?) != 20
        || success[16] != 1
        || i64::from_le_bytes(success[24..32].try_into()?) != 22
    {
        return Err(failure("v6 raw success reconstruction changed"));
    }

    let pointer = transform.call(&mut store, (18, 1, 22, 1))?;
    let pointer =
        usize::try_from(pointer).map_err(|_| failure("v6 raw result pointer was negative"))?;
    if pointer != 256 {
        return Err(failure("v6 raw status pointer changed"));
    }
    let mut status = [0_u8; 32];
    memory.read(&store, pointer, &mut status)?;
    if status[0] != 1
        || u32::from_le_bytes(status[16..20].try_into()?) != 4
        || status[20] != 2
        || status[24..32] != [0xa5; 8]
    {
        return Err(failure("v6 raw status or stale-output poison changed"));
    }

    if transform.call(&mut store, (18, 2, 22, 2)).is_ok() {
        return Err(failure("v6 raw invalid bool did not trap"));
    }
    let mut poison = [0_u8; 32];
    memory.read(&store, 256, &mut poison)?;
    if poison != [0xa5; 32] {
        return Err(failure("v6 raw invalid-bool trap retained stale output"));
    }
    Ok(())
}

fn prove_engine_failure_is_out_of_band_v6(
    engine: &Engine,
    component: &Component,
) -> HostResult<()> {
    use v6_bindings::exports::semaprax::private::nested_records::{Inner, Outer};

    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(1_000_000)?;
    let bindings = v6_bindings::SemapraxPrivateV6::instantiate(&mut store, component, &linker)?;
    store.set_fuel(0)?;
    let input = Outer {
        inner: Inner {
            value: 18,
            flag: true,
        },
        other: 22,
    };
    if bindings
        .semaprax_private_nested_records()
        .call_transform(&mut store, input, 2)
        .is_ok()
    {
        return Err(failure("v6 fuel exhaustion became a typed status"));
    }
    Ok(())
}

fn run_v6() -> HostResult<()> {
    let program = ::semaprax::check(SOURCE_V6, Path::new("component-nested-record-v6.spx"))
        .map_err(|diagnostics| {
            failure(format!("v6 fixture verification failed: {diagnostics:?}"))
        })?;
    let artifact = ::semaprax::wit_component::emit_private_nested_record_component_v6(&program)
        .map_err(|diagnostic| failure(format!("v6 component emission failed: {diagnostic:?}")))?;
    if artifact.wit() != include_str!("../wit/semaprax-private-v6.wit") {
        return Err(failure("checked-in Wasmtime v6 WIT drifted"));
    }
    let bytes: Box<[u8]> = artifact.bytes().to_vec().into_boxed_slice();
    let before: [u8; 32] = Sha256::digest(&bytes).into();
    if before != EXPECTED_COMPONENT_V6_SHA256
        || artifact.generated_core_digest() != EXPECTED_GENERATED_CORE_V6_SHA256
    {
        return Err(failure("v6 independent component/core KAT changed"));
    }
    let expected_revision = ::semaprax::graph::revision(&program);
    if expected_revision != EXPECTED_SOURCE_REVISION_V6 {
        return Err(failure("v6 source revision KAT changed"));
    }
    let validated = ::semaprax::wit_component::validate_private_nested_record_component_v6(
        &bytes,
        EXPECTED_SOURCE_REVISION_V6,
        EXPECTED_GENERATED_CORE_V6_SHA256,
    )
    .map_err(|error| failure(format!("v6 profile validation failed: {error:?}")))?;
    if validated.interface_export_name() != "semaprax:private/nested-records@0.4.0"
        || validated.function_export_name() != "transform"
        || validated.type_export_names() != ["status", "inner", "outer"]
        || validated.source_revision() != EXPECTED_SOURCE_REVISION_V6
        || <[u8; 32]>::from(Sha256::digest(validated.generated_core()))
            != EXPECTED_GENERATED_CORE_V6_SHA256
    {
        return Err(failure("v6 typed export table changed"));
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure("v6 requested ambient imports"));
    }
    run_v6_instance(&engine, &component)?;
    run_v6_instance(&engine, &component)?;
    for _ in 0..2 {
        run_v6_instance(&engine, &component)?;
    }
    prove_raw_core_v6_poison_status_and_invalid_bool(&engine, validated.generated_core())?;
    prove_engine_failure_is_out_of_band_v6(&engine, &component)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != before {
        return Err(failure("authenticated v6 bytes changed during execution"));
    }
    println!("semaprax-private-component-runtime-v6-ok");
    Ok(())
}

fn run() -> HostResult<()> {
    run_v3()?;
    run_v4()?;
    run_v5()?;
    run_v6()
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
