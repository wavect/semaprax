#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::codegen::emit_native_adapter_admission;
use semaprax::conformance::{NormalizedStatus, Retryability, StatusClass};
use semaprax::hir::{self, DeclarationId};
use semaprax_native_host::{
    AdmissionError, CallRejection, DescriptorRejection, NativeHost, OwnedExecution,
    ScalarExecution, ScalarValue,
};
use semaprax_native_loader::{OpenError, MAX_DESCRIPTOR_BYTES};

const HEADER_NAME: &str = "semaprax_adapter_descriptor.h";
const SOURCE: &str = r#"module test.physical_host;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.discard")
fn discard(value: own Token) -> i64 { 7 }

@id("token.scalar-mix")
fn scalar_mix(value: own Token, delta: i64, condition: bool) -> i64 {
    0
}

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token {
    second
}

@id("test.main")
fn main() -> i64 { 0 }
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("panic payload destructor must never run outside containment");
    }
}

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    unload_marker: PathBuf,
    descriptor: Vec<u8>,
    getter_symbol: String,
}

impl Fixture {
    fn build(function: &str) -> Self {
        let program = semaprax::parse(SOURCE, Path::new("physical-host.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let artifact =
            emit_native_adapter_admission(&resolved, &DeclarationId::new(function), HEADER_NAME)
                .unwrap();
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-native-host-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create isolated host fixture directory");
        let directory = fs::canonicalize(directory).expect("canonical host fixture directory");
        let header = directory.join(HEADER_NAME);
        let source = directory.join("provider.c");
        let library = directory.join(library_filename());
        let unload_marker = directory.join("unloaded.marker");
        fs::write(&header, artifact.header()).expect("write generated descriptor header");
        let provider = format!(
            "{}\n#include <stdio.h>\n\
             __attribute__((destructor)) static void spx_host_unload(void) {{\n\
                 FILE *marker = fopen({}, \"wb\");\n\
                 if (marker != NULL) {{ fputs(\"unloaded\", marker); fclose(marker); }}\n\
             }}\n",
            artifact.provider_source(),
            c_string_literal(&unload_marker)
        );
        fs::write(&source, provider).expect("write generated descriptor provider");

        let mut compiler = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
        #[cfg(target_os = "macos")]
        compiler.args(["-dynamiclib", "-fPIC"]);
        #[cfg(not(target_os = "macos"))]
        compiler.args(["-shared", "-fPIC"]);
        let output = compiler
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-fvisibility=hidden",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .expect("compile runtime host fixture");
        assert!(
            output.status.success(),
            "host fixture compiler failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let library = fs::canonicalize(library).expect("canonical host fixture library");
        Self {
            directory,
            library,
            unload_marker,
            descriptor: artifact.descriptor().to_vec(),
            getter_symbol: artifact.getter_symbol().to_owned(),
        }
    }

    unsafe fn open(&self) -> NativeHost {
        // SAFETY: This fixture compiles the exact compiler-generated provider in
        // a private canonical directory. Its sole exported getter has the
        // declared ABI, returns immutable static bytes, and cannot unwind.
        unsafe {
            NativeHost::open_admitted_exact(
                &self.library,
                self.getter_symbol.as_bytes(),
                &self.descriptor,
            )
        }
        .expect("admit generated physical host fixture")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove isolated host fixture directory");
    }
}

fn library_filename() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libhost-provider.dylib"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "libhost-provider.so"
    }
}

fn c_string_literal(path: &Path) -> String {
    let mut escaped = String::from("\"");
    for character in path.to_str().expect("fixture path is UTF-8").chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            value => escaped.push(value),
        }
    }
    escaped.push('"');
    escaped
}

fn adapter_failure() -> NormalizedStatus {
    NormalizedStatus::try_new(
        "test.native-host.adapter.v1",
        17,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .unwrap()
}

fn expect_owned_execution(
    result: Result<OwnedExecution, semaprax_native_host::RejectedCall>,
) -> OwnedExecution {
    match result {
        Ok(execution) => execution,
        Err(rejection) => panic!("unexpected rejection: {:?}", rejection.rejection()),
    }
}

fn expect_scalar_execution(
    result: Result<ScalarExecution, semaprax_native_host::RejectedCall>,
) -> ScalarExecution {
    match result {
        Ok(execution) => execution,
        Err(rejection) => panic!("unexpected rejection: {:?}", rejection.rejection()),
    }
}

#[test]
fn real_owned_identity_rotates_authority_and_retains_exact_module() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: `Fixture::open` documents the exact generated provider.
    let mut host = unsafe { fixture.open() };
    let instance = host.module_instance_id();
    // SAFETY: Zero is a valid opaque resource payload, not a liveness sentinel,
    // and this test creates exactly one exclusive owner for it.
    let owner = unsafe { host.adopt_trusted_owner(0, 0) }.unwrap();
    assert_eq!(owner.module_instance_id(), instance);

    // SAFETY: The test closure is the exact identity template and does not
    // retain or reinterpret its opaque payload.
    let owner = match expect_owned_execution(unsafe {
        host.execute_owned_with(vec![owner], |payloads| {
            assert_eq!(payloads, [0]);
            Ok(())
        })
    }) {
        OwnedExecution::Success(owner) => owner,
        OwnedExecution::Failure(status) => panic!("unexpected failure: {status:?}"),
    };
    assert_eq!(owner.module_instance_id(), instance);
    drop(host);
    assert!(
        !fixture.unload_marker.exists(),
        "the live result owner must retain the exact platform module"
    );
    drop(owner);
    assert!(
        fixture.unload_marker.exists(),
        "the final owner release makes the platform handle eligible for unload"
    );
}

#[test]
fn separate_equal_descriptors_reject_cross_instance_without_consumption() {
    let first_fixture = Fixture::build("token.identity");
    let second_fixture = Fixture::build("token.identity");
    // SAFETY: Both fixtures are exact generated providers in distinct paths.
    let mut first = unsafe { first_fixture.open() };
    // SAFETY: Both fixtures are exact generated providers in distinct paths.
    let mut second = unsafe { second_fixture.open() };
    assert_ne!(first.module_instance_id(), second.module_instance_id());
    // SAFETY: Maximum `u64` is a valid opaque payload and is adopted once.
    let owner = unsafe { first.adopt_trusted_owner(0, u64::MAX) }.unwrap();

    // SAFETY: The closure cannot execute because exact-instance preflight must
    // reject this credential before ingress commit.
    let rejection = match unsafe {
        second.execute_owned_with(vec![owner], |_| panic!("cross-instance executor ran"))
    } {
        Ok(_) => panic!("cross-instance credential was accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::WrongModuleInstance);
    let owner = rejection.into_owners().pop().unwrap();

    // SAFETY: The owner is returned unchanged to its original exact host.
    let execution = expect_owned_execution(unsafe {
        first.execute_owned_with(vec![owner], |payloads| {
            assert_eq!(payloads, [u64::MAX]);
            Ok(())
        })
    });
    assert!(matches!(execution, OwnedExecution::Success(_)));
}

#[test]
fn draining_rejects_creation_and_calls_while_existing_owner_keeps_pin() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { host.adopt_trusted_owner(0, 41) }.unwrap();
    host.begin_draining();
    assert!(host.is_draining());
    // SAFETY: Draining rejects before considering payload adoption.
    assert!(matches!(
        unsafe { host.adopt_trusted_owner(0, 42) },
        Err(CallRejection::Draining)
    ));
    // SAFETY: Draining rejects before the executor can run.
    let rejection = match unsafe {
        host.execute_owned_with(vec![owner], |_| panic!("draining executor ran"))
    } {
        Ok(_) => panic!("draining call executed"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::Draining);
    let owner = rejection.into_owners().pop().unwrap();
    drop(host);
    assert!(!fixture.unload_marker.exists());
    drop(owner);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn scalar_execution_consumes_owner_on_success_and_failure() {
    for fail in [false, true] {
        let fixture = Fixture::build("token.discard");
        // SAFETY: The fixture is an exact generated provider.
        let mut host = unsafe { fixture.open() };
        // SAFETY: The payload has one exclusive owner.
        let owner = unsafe { host.adopt_trusted_owner(0, 99) }.unwrap();
        // SAFETY: The closure is the admitted scalar template test executor and
        // does not retain or reinterpret the payload.
        let execution = expect_scalar_execution(unsafe {
            host.execute_scalar_with(vec![owner], |payloads| {
                assert_eq!(payloads, [99]);
                if fail {
                    Err(adapter_failure())
                } else {
                    Ok(7)
                }
            })
        });
        if fail {
            assert_eq!(execution, ScalarExecution::Failure(adapter_failure()));
        } else {
            assert_eq!(execution, ScalarExecution::Success(7));
        }
        drop(host);
        assert!(fixture.unload_marker.exists());
    }
}

#[test]
fn owned_execution_failure_publishes_no_owner_and_releases_inputs() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { host.adopt_trusted_owner(0, 5) }.unwrap();
    // SAFETY: The trusted test executor reports a normalized failure without
    // retaining or reinterpreting the payload.
    let execution = expect_owned_execution(unsafe {
        host.execute_owned_with(vec![owner], |_| Err(adapter_failure()))
    });
    assert!(matches!(execution, OwnedExecution::Failure(_)));
    drop(host);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn dropping_live_owner_retires_ledger_before_releasing_its_pin() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { host.adopt_trusted_owner(0, 77) }.unwrap();
    assert_eq!(host.live_owner_count(), 1);
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
    assert!(
        !fixture.unload_marker.exists(),
        "the host still owns its pin"
    );
    drop(host);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn panicking_trusted_executor_is_an_executed_failure_not_unwind_or_rejection() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { host.adopt_trusted_owner(0, 88) }.unwrap();
    // SAFETY: This deliberately hostile trusted executor panics without
    // retaining the payload; the host must contain the Rust unwind.
    let execution = expect_owned_execution(unsafe {
        host.execute_owned_with(vec![owner], |_| panic!("hostile executor panic"))
    });
    assert!(matches!(execution, OwnedExecution::Failure(_)));
    assert_eq!(host.live_owner_count(), 0);
    drop(host);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn panicking_drop_payload_is_contained_for_scalar_and_owned_execution() {
    let scalar_fixture = Fixture::build("token.discard");
    // SAFETY: The fixture is an exact generated descriptor provider.
    let mut scalar_host = unsafe { scalar_fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let scalar_owner = unsafe { scalar_host.adopt_trusted_owner(0, 89) }.unwrap();
    // SAFETY: This deliberately hostile trusted executor proves that the host
    // never runs an attacker-controlled panic-payload destructor after
    // containing the unwind.
    let scalar = expect_scalar_execution(unsafe {
        scalar_host.execute_scalar_with(vec![scalar_owner], |_| std::panic::panic_any(PanicOnDrop))
    });
    assert!(matches!(scalar, ScalarExecution::Failure(_)));
    assert_eq!(scalar_host.live_owner_count(), 0);

    let owned_fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated descriptor provider.
    let mut owned_host = unsafe { owned_fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owned_owner = unsafe { owned_host.adopt_trusted_owner(0, 90) }.unwrap();
    // SAFETY: This exercises the equivalent owned-result containment branch.
    let owned = expect_owned_execution(unsafe {
        owned_host.execute_owned_with(vec![owned_owner], |_| std::panic::panic_any(PanicOnDrop))
    });
    assert!(matches!(owned, OwnedExecution::Failure(_)));
    assert_eq!(owned_host.live_owner_count(), 0);
}

#[test]
fn captured_unrelated_owner_can_retire_while_an_invocation_is_active() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: Both payloads have independent exclusive owners.
    let input = unsafe { host.adopt_trusted_owner(0, 91) }.unwrap();
    // SAFETY: Both payloads have independent exclusive owners.
    let captured = unsafe { host.adopt_trusted_owner(0, 92) }.unwrap();
    assert_eq!(host.live_owner_count(), 2);

    // SAFETY: The identity executor observes only its committed input. The
    // unrelated owner is deliberately retired during execution to prove that
    // no registry borrow spans trusted code.
    let result = match expect_owned_execution(unsafe {
        host.execute_owned_with(vec![input], move |payloads| {
            assert_eq!(payloads, [91]);
            drop(captured);
            Ok(())
        })
    }) {
        OwnedExecution::Success(owner) => owner,
        OwnedExecution::Failure(status) => panic!("unexpected failure: {status:?}"),
    };
    assert_eq!(host.live_owner_count(), 1);
    drop(result);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn captured_owner_drop_during_executor_unwind_is_contained_without_leak() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: Both payloads have independent exclusive owners.
    let input = unsafe { host.adopt_trusted_owner(0, 93) }.unwrap();
    // SAFETY: Both payloads have independent exclusive owners.
    let captured = unsafe { host.adopt_trusted_owner(0, 94) }.unwrap();

    // SAFETY: The deliberate adapter panic is contained. Moving the unrelated
    // owner into a local forces its destructor to run during that unwind.
    let execution = expect_owned_execution(unsafe {
        host.execute_owned_with(vec![input], move |_| {
            let _drop_during_unwind = captured;
            panic!("adapter panic with captured owner")
        })
    });
    assert!(matches!(execution, OwnedExecution::Failure(_)));
    assert_eq!(host.live_owner_count(), 0);
    drop(host);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn scalar_bearing_descriptor_validates_typed_values_before_owner_commit() {
    let fixture = Fixture::build("token.scalar-mix");
    // SAFETY: The fixture is the exact generated descriptor provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: This test creates one exclusive owner for the opaque payload.
    let owner = unsafe { host.adopt_trusted_owner(0, 71) }.unwrap();

    // SAFETY: The compatibility executor must reject the missing scalars before
    // it can run this closure or commit the owner.
    let rejection = match unsafe {
        host.execute_scalar_with(vec![owner], |_| panic!("missing-scalar executor ran"))
    } {
        Ok(_) => panic!("missing scalar arguments were accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(
        rejection.rejection(),
        CallRejection::ScalarInputCountMismatch
    );
    let owner = rejection.into_owners().pop().unwrap();

    // SAFETY: A kind mismatch is another precommit rejection and the returned
    // owner remains the exact reusable credential.
    let rejection = match unsafe {
        host.execute_scalar_with_values(
            vec![owner],
            vec![ScalarValue::Bool(true), ScalarValue::I64(41)],
            |_, _| panic!("wrong-kind executor ran"),
        )
    } {
        Ok(_) => panic!("wrong scalar kinds were accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::ScalarKindMismatch);
    let owner = rejection.into_owners().pop().unwrap();

    // SAFETY: The closure observes the exact admitted scalar order and does not
    // retain or reinterpret the opaque owner payload.
    let execution = expect_scalar_execution(unsafe {
        host.execute_scalar_with_values(
            vec![owner],
            vec![ScalarValue::I64(i64::MIN), ScalarValue::Bool(false)],
            |payloads, scalars| {
                assert_eq!(payloads, [71]);
                assert_eq!(
                    scalars,
                    [ScalarValue::I64(i64::MIN), ScalarValue::Bool(false)]
                );
                Ok(0)
            },
        )
    });
    assert_eq!(execution, ScalarExecution::Success(0));
    assert_eq!(host.live_owner_count(), 0);

    // SAFETY: A second independent owner exercises the other scalar boundary
    // values through the same exact admitted signature.
    let owner = unsafe { host.adopt_trusted_owner(0, 72) }.unwrap();
    let execution = expect_scalar_execution(unsafe {
        host.execute_scalar_with_values(
            vec![owner],
            vec![ScalarValue::I64(i64::MAX), ScalarValue::Bool(true)],
            |payloads, scalars| {
                assert_eq!(payloads, [72]);
                assert_eq!(
                    scalars,
                    [ScalarValue::I64(i64::MAX), ScalarValue::Bool(true)]
                );
                Ok(1)
            },
        )
    });
    assert_eq!(execution, ScalarExecution::Success(1));
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn scalar_bearing_owned_result_preserves_order_reuse_and_execution_atomicity() {
    let fixture = Fixture::build("token.choose-second");
    // SAFETY: The fixture is the exact generated descriptor provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: These are independent exclusive payload owners.
    let first = unsafe { host.adopt_trusted_owner(0, 81) }.unwrap();
    // SAFETY: These are independent exclusive payload owners.
    let second = unsafe { host.adopt_trusted_owner(1, 82) }.unwrap();

    // SAFETY: Missing scalars reject before owner commit.
    let rejection = match unsafe {
        host.execute_owned_with_values(vec![first, second], Vec::new(), |_, _| {
            panic!("missing-scalar executor ran")
        })
    } {
        Ok(_) => panic!("missing scalar was accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(
        rejection.rejection(),
        CallRejection::ScalarInputCountMismatch
    );
    let mut owners = rejection.into_owners().into_iter();
    let first = owners.next().unwrap();
    let second = owners.next().unwrap();
    assert!(owners.next().is_none());

    // SAFETY: Wrong scalar kind also rejects without consuming either owner.
    let rejection = match unsafe {
        host.execute_owned_with_values(
            vec![first, second],
            vec![ScalarValue::Bool(false)],
            |_, _| panic!("wrong-kind executor ran"),
        )
    } {
        Ok(_) => panic!("wrong scalar kind was accepted"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::ScalarKindMismatch);
    let mut owners = rejection.into_owners().into_iter();
    let first = owners.next().unwrap();
    let second = owners.next().unwrap();
    assert!(owners.next().is_none());

    // SAFETY: The closure observes exact signature order and implements the
    // admitted choose-second result mapping without retaining payloads.
    let selected = match expect_owned_execution(unsafe {
        host.execute_owned_with_values(
            vec![first, second],
            vec![ScalarValue::I64(i64::MIN)],
            |payloads, scalars| {
                assert_eq!(payloads, [81, 82]);
                assert_eq!(scalars, [ScalarValue::I64(i64::MIN)]);
                Ok(())
            },
        )
    }) {
        OwnedExecution::Success(owner) => owner,
        OwnedExecution::Failure(status) => panic!("unexpected failure: {status:?}"),
    };
    assert_eq!(host.live_owner_count(), 1);
    drop(selected);
    assert_eq!(host.live_owner_count(), 0);

    // SAFETY: Executed failure happens after commit and consumes both inputs.
    let first = unsafe { host.adopt_trusted_owner(0, 83) }.unwrap();
    // SAFETY: These are independent exclusive payload owners.
    let second = unsafe { host.adopt_trusted_owner(1, 84) }.unwrap();
    let execution = expect_owned_execution(unsafe {
        host.execute_owned_with_values(
            vec![first, second],
            vec![ScalarValue::I64(i64::MAX)],
            |payloads, scalars| {
                assert_eq!(payloads, [83, 84]);
                assert_eq!(scalars, [ScalarValue::I64(i64::MAX)]);
                Err(adapter_failure())
            },
        )
    });
    assert!(matches!(execution, OwnedExecution::Failure(_)));
    assert_eq!(host.live_owner_count(), 0);

    // SAFETY: A trusted-adapter panic is contained after commit; all inputs
    // are abandoned and no owned result is published.
    let first = unsafe { host.adopt_trusted_owner(0, 85) }.unwrap();
    // SAFETY: These are independent exclusive payload owners.
    let second = unsafe { host.adopt_trusted_owner(1, 86) }.unwrap();
    let execution = expect_owned_execution(unsafe {
        host.execute_owned_with_values(vec![first, second], vec![ScalarValue::I64(0)], |_, _| {
            panic!("contained scalar-bearing owned panic")
        })
    });
    assert!(matches!(execution, OwnedExecution::Failure(_)));
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn descriptor_length_bounds_are_enforced_before_parsing_or_loading() {
    let fixture = Fixture::build("token.identity");
    for descriptor in [Vec::new(), vec![0; MAX_DESCRIPTOR_BYTES + 1]] {
        // SAFETY: The length gate rejects before parsing or loading, so the
        // contents and native fixture cannot be observed.
        let error = match unsafe {
            NativeHost::open_admitted_exact(
                &fixture.library,
                fixture.getter_symbol.as_bytes(),
                &descriptor,
            )
        } {
            Ok(_) => panic!("out-of-bound descriptor was admitted"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            AdmissionError::Loader(OpenError::InvalidExpectedDescriptorLength {
                actual,
                maximum: MAX_DESCRIPTOR_BYTES,
            }) if actual == descriptor.len()
        ));
        assert!(!fixture.unload_marker.exists());
    }
}

#[test]
fn getter_symbol_mismatch_is_rejected_before_native_loading() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: Host-side canonical-symbol validation rejects before invoking
    // the native loader or any provider initializer.
    let error = match unsafe {
        NativeHost::open_admitted_exact(
            &fixture.library,
            b"spx_wrong_descriptor_getter",
            &fixture.descriptor,
        )
    } {
        Ok(_) => panic!("wrong descriptor getter was admitted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        AdmissionError::Descriptor(DescriptorRejection::NonCanonical)
    ));
    assert!(!fixture.unload_marker.exists());
}

#[test]
fn wrong_execution_shape_returns_the_exact_owner_for_reuse() {
    let owned_fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut owned_host = unsafe { owned_fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { owned_host.adopt_trusted_owner(0, 101) }.unwrap();
    // SAFETY: Shape rejection occurs before this executor can run.
    let rejection = match unsafe {
        owned_host.execute_scalar_with(vec![owner], |_| panic!("wrong-shape executor ran"))
    } {
        Ok(_) => panic!("owned-result descriptor accepted scalar execution"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::WrongShape);
    let owner = rejection.into_owners().pop().unwrap();
    // SAFETY: Rejection returned the still-live exact wrapper.
    let result =
        expect_owned_execution(unsafe { owned_host.execute_owned_with(vec![owner], |_| Ok(())) });
    assert!(matches!(result, OwnedExecution::Success(_)));

    let scalar_fixture = Fixture::build("token.discard");
    // SAFETY: The fixture is an exact generated provider.
    let mut scalar_host = unsafe { scalar_fixture.open() };
    // SAFETY: The payload has one exclusive owner.
    let owner = unsafe { scalar_host.adopt_trusted_owner(0, 102) }.unwrap();
    // SAFETY: Shape rejection occurs before this executor can run.
    let rejection = match unsafe {
        scalar_host.execute_owned_with(vec![owner], |_| panic!("wrong-shape executor ran"))
    } {
        Ok(_) => panic!("scalar descriptor accepted owned execution"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::WrongShape);
    let owner = rejection.into_owners().pop().unwrap();
    // SAFETY: Rejection returned the still-live exact wrapper.
    assert_eq!(
        expect_scalar_execution(unsafe { scalar_host.execute_scalar_with(vec![owner], |_| Ok(7)) }),
        ScalarExecution::Success(7)
    );
}

#[test]
fn input_count_rejection_returns_all_owners_in_parameter_order() {
    let fixture = Fixture::build("token.identity");
    // SAFETY: The fixture is an exact generated provider.
    let mut host = unsafe { fixture.open() };
    // SAFETY: Both payloads have independent exclusive owners.
    let first = unsafe { host.adopt_trusted_owner(0, 111) }.unwrap();
    // SAFETY: Both payloads have independent exclusive owners.
    let second = unsafe { host.adopt_trusted_owner(0, 112) }.unwrap();
    // SAFETY: Count preflight rejects before this executor can run.
    let rejection = match unsafe {
        host.execute_owned_with(vec![first, second], |_| {
            panic!("count-mismatch executor ran")
        })
    } {
        Ok(_) => panic!("input-count mismatch was executed"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.rejection(), CallRejection::InputCountMismatch);
    let mut owners = rejection.into_owners().into_iter();
    let first = owners.next().unwrap();
    let second = owners.next().unwrap();
    assert!(owners.next().is_none());

    // SAFETY: The first parameter-ordered wrapper remains live and reusable.
    let result = match expect_owned_execution(unsafe {
        host.execute_owned_with(vec![first], |payloads| {
            assert_eq!(payloads, [111]);
            Ok(())
        })
    }) {
        OwnedExecution::Success(owner) => owner,
        OwnedExecution::Failure(status) => panic!("unexpected failure: {status:?}"),
    };
    drop(second);
    drop(result);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn malformed_descriptor_is_rejected_before_native_loading() {
    let fixture = Fixture::build("token.identity");
    let mut descriptor = fixture.descriptor.clone();
    descriptor[0] ^= 1;
    // SAFETY: Descriptor parsing rejects the mutated magic before the loader is
    // invoked, so no foreign code can execute.
    let error = match unsafe {
        NativeHost::open_admitted_exact(
            &fixture.library,
            fixture.getter_symbol.as_bytes(),
            &descriptor,
        )
    } {
        Ok(_) => panic!("mutated descriptor was admitted"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        AdmissionError::Descriptor(DescriptorRejection::UnsupportedSchema)
    ));
    assert!(
        !fixture.unload_marker.exists(),
        "descriptor rejection must precede native loading"
    );
}

#[test]
fn target_schema_length_and_trailing_mutations_all_precede_loading() {
    let fixture = Fixture::build("token.identity");
    let target_length = u32::from_le_bytes(fixture.descriptor[20..24].try_into().unwrap()) as usize;
    let schema_offset = 24 + target_length;
    let target_fingerprint_offset = schema_offset + 32;
    let mut cases = Vec::new();

    let mut wrong_schema = fixture.descriptor.clone();
    wrong_schema[schema_offset] ^= 1;
    cases.push((wrong_schema, DescriptorRejection::UnsupportedSchema));

    let mut wrong_target = fixture.descriptor.clone();
    wrong_target[target_fingerprint_offset] ^= 1;
    cases.push((wrong_target, DescriptorRejection::WrongTarget));

    let mut wrong_length = fixture.descriptor.clone();
    wrong_length[16..20].copy_from_slice(&1_u32.to_le_bytes());
    cases.push((wrong_length, DescriptorRejection::Malformed));

    let mut trailing = fixture.descriptor.clone();
    trailing.push(0);
    cases.push((trailing, DescriptorRejection::Malformed));

    for (descriptor, expected) in cases {
        // SAFETY: Strict descriptor parsing rejects each mutation before the
        // platform loader is invoked.
        let error = match unsafe {
            NativeHost::open_admitted_exact(
                &fixture.library,
                fixture.getter_symbol.as_bytes(),
                &descriptor,
            )
        } {
            Ok(_) => panic!("hostile descriptor was admitted"),
            Err(error) => error,
        };
        assert!(matches!(error, AdmissionError::Descriptor(actual) if actual == expected));
        assert!(!fixture.unload_marker.exists());
    }
}
