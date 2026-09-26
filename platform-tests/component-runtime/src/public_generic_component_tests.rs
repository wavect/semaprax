use std::{fmt::Write as _, path::Path};

use sha2::{Digest, Sha256};
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};

use super::{HostResult, failure, public_generic_component_v1_bindings::PublicGenericComponentV1};

mod parity;

// Independent known answers for the checked-in, explicitly acquired project.
// Replay must not accept identity claims supplied by the emitter under test.
// R07's Core Wasm provider fixes (owned-byte runtime, admission before
// memory.grow, the 128 KiB payload window, and ABI v2 status 14/SPX-PG803)
// changed the compiled provider's bytes: only the provider/component
// digests below moved. The descriptor digests are a pure function of the
// checked source and are unchanged.
const EXPECTED_PUBLIC_GENERIC_COMPONENT_DIGEST: &str =
    "sha256:55c42169da109f2a78dd67ea364513adba3d9106924ab9c55fb642a81e6927df";
const EXPECTED_PUBLIC_GENERIC_DESCRIPTOR_DIGEST: &str =
    "sha256:52473587274784c87a62e109cd8640bf337306117f8fa943a6b930aeb6a75b1a";
const EXPECTED_PUBLIC_GENERIC_PROVIDER_DIGEST: &str =
    "sha256:632afbf2067355cfb41bc8badec7c9e26ed0d3260a3c706925009b32e204af65";
const EXPECTED_PUBLIC_GENERIC_COMPONENT_SHA256: &str =
    "a8d19baea7ed0fe59518d337f3efea54cd00ea3810e8e6ddd9ee39a3b8fde630";
const EXPECTED_CONTRACT_FAILURE_COMPONENT_DIGEST: &str =
    "sha256:30f479592233897ca0db6a9371f0014eca58fc3a66ffee59c2d457eb851b9605";
const EXPECTED_CONTRACT_FAILURE_DESCRIPTOR_DIGEST: &str =
    "sha256:1cef20213f00dce6e80b9cc1eb977065018986bc5568263d6ab4cd04ba9c5d49";
const EXPECTED_CONTRACT_FAILURE_PROVIDER_DIGEST: &str =
    "sha256:7a5b8e2b33133c6963a1b9529b1bed270ad2359be54bb2cd2ff1515236b3ab9e";
const EXPECTED_CONTRACT_FAILURE_COMPONENT_SHA256: &str =
    "3b927698a0255f83461bb730c745ba2f92b5b461c09bc0b0e5f7c11003a15810";

const MAX_LIST_BYTES: usize = 65_536;
const REUSE_CYCLES: usize = 200;
const PREVIOUS_STAGING_CAPACITY_BYTES: usize = 12_355_336;
const _: () = assert!(REUSE_CYCLES * MAX_LIST_BYTES > PREVIOUS_STAGING_CAPACITY_BYTES);

fn retained_component_bytes() -> HostResult<Vec<u8>> {
    use semaprax::project::with_authenticated_project;

    let manifest = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/public-generic-v1/semaprax.toml"
    ));
    let artifact_bytes = with_authenticated_project(manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let endpoint = revision.public_generic_wasm_provider_endpoint_v1()?;
        if endpoint.subject().input().owned_leaves.len() != 2 {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "runtime fixture did not retain exactly two owned Bytes leaves",
            )]);
        }
        let input_leaves = endpoint.subject().input().owned_leaves.clone();
        let leaf_paths = input_leaves
            .iter()
            .map(|path| path.split_once('/'))
            .collect::<Option<Vec<_>>>();
        let has_left_then_right = leaf_paths.as_ref().is_some_and(|paths| {
            paths.len() == 2
                && paths[0].0 == paths[1].0
                && paths[0].0.ends_with("provider.envelope.payload")
                && paths[0].1.ends_with("provider.leaf-pair.left")
                && paths[1].1.ends_with("provider.leaf-pair.right")
                && !paths[0].1.contains('/')
                && !paths[1].1.contains('/')
        });
        if !has_left_then_right {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "retained input Bytes leaves must bind payload.left then payload.right",
            )]);
        }
        let artifact = revision.public_generic_wasm_component_artifact_v1()?;
        if artifact.descriptor_digest() != EXPECTED_PUBLIC_GENERIC_DESCRIPTOR_DIGEST
            || endpoint.descriptor().descriptor_digest()
                != EXPECTED_PUBLIC_GENERIC_DESCRIPTOR_DIGEST
            || artifact.provider_digest() != EXPECTED_PUBLIC_GENERIC_PROVIDER_DIGEST
        {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "Component descriptor/provider identity differs from pinned fixture",
            )]);
        }
        let mut tampered = artifact.bytes().to_vec();
        let final_byte = tampered.last_mut().ok_or_else(|| {
            vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "retained Component artifact is empty",
            )]
        })?;
        *final_byte ^= 1;
        if revision
            .replay_public_generic_wasm_component_v1(
                &tampered,
                EXPECTED_PUBLIC_GENERIC_COMPONENT_DIGEST,
                EXPECTED_PUBLIC_GENERIC_DESCRIPTOR_DIGEST,
                EXPECTED_PUBLIC_GENERIC_PROVIDER_DIGEST,
            )
            .is_ok()
        {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "tampered Component unexpectedly replayed",
            )]);
        }
        let replayed = revision.replay_public_generic_wasm_component_v1(
            artifact.bytes(),
            EXPECTED_PUBLIC_GENERIC_COMPONENT_DIGEST,
            EXPECTED_PUBLIC_GENERIC_DESCRIPTOR_DIGEST,
            EXPECTED_PUBLIC_GENERIC_PROVIDER_DIGEST,
        )?;
        if replayed != artifact {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "authentic retained Component failed replay after tamper refusal",
            )]);
        }
        Ok(replayed.bytes().to_vec())
    })
    .map_err(|errors| failure(format!("retained project admission failed: {errors:?}")))?;

    Ok(artifact_bytes)
}

pub(super) fn run_public_generic_component_v1() -> HostResult<()> {
    let artifact_bytes = retained_component_bytes()?;
    let before = Sha256::digest(&artifact_bytes);
    let mut raw_digest = String::with_capacity(64);
    for byte in before {
        write!(raw_digest, "{byte:02x}")?;
    }
    if raw_digest != EXPECTED_PUBLIC_GENERIC_COMPONENT_SHA256 {
        return Err(failure(
            "retained Component bytes differ from pinned fixture",
        ));
    }
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &artifact_bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure(
            "public-generic Component requested ambient imports",
        ));
    }
    let linker = Linker::<()>::new(&engine);
    let mut store = Store::new(&engine, ());
    store.set_fuel(1_000_000_000)?;
    let bindings = PublicGenericComponentV1::instantiate(&mut store, &component, &linker)?;

    prove_maximum_leaf_transfer(&bindings, &mut store)?;
    prove_constructor_reuse(&bindings, &mut store)?;
    prove_transfer_reentry(&bindings, &mut store)?;
    prove_closed_and_transferred_handles_refuse(&bindings, &mut store)?;
    if Sha256::digest(&artifact_bytes) != before {
        return Err(failure(
            "authenticated retained Component bytes changed during execution",
        ));
    }
    Ok(())
}

pub(super) fn run_public_generic_component_contract_failure_v1() -> HostResult<()> {
    use semaprax::project::with_authenticated_project;

    let manifest = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/public-generic-contract-failure-v1/semaprax.toml"
    ));
    let artifact = with_authenticated_project(manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let endpoint = revision.public_generic_wasm_provider_endpoint_v1()?;
        let artifact = revision.public_generic_wasm_component_artifact_v1()?;
        if endpoint.descriptor().descriptor_digest() != EXPECTED_CONTRACT_FAILURE_DESCRIPTOR_DIGEST
            || artifact.descriptor_digest() != EXPECTED_CONTRACT_FAILURE_DESCRIPTOR_DIGEST
            || artifact.provider_digest() != EXPECTED_CONTRACT_FAILURE_PROVIDER_DIGEST
        {
            return Err(vec![semaprax::diagnostic::Diagnostic::io(
                "SPX-W121",
                "contract-failure fixture descriptor/provider differs from pinned bytes",
            )]);
        }
        revision.replay_public_generic_wasm_component_v1(
            artifact.bytes(),
            EXPECTED_CONTRACT_FAILURE_COMPONENT_DIGEST,
            EXPECTED_CONTRACT_FAILURE_DESCRIPTOR_DIGEST,
            EXPECTED_CONTRACT_FAILURE_PROVIDER_DIGEST,
        )
    })
    .map_err(|errors| {
        failure(format!(
            "contract-failure project admission failed: {errors:?}"
        ))
    })?;

    verify_contract_failure_component_bytes(artifact.bytes())?;

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, artifact.bytes())?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure(
            "contract-failure Component requested ambient imports",
        ));
    }
    let linker = Linker::<()>::new(&engine);
    let mut store = Store::new(&engine, ());
    store.set_fuel(1_000_000_000)?;
    let bindings = PublicGenericComponentV1::instantiate(&mut store, &component, &linker)?;
    let adapter = bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    let left = bytes.call_constructor(&mut store, &[1, 2, 3])?;
    let right = bytes.call_constructor(&mut store, &[4, 5])?;
    let outcome = adapter.call_invoke(&mut store, left, right)?;
    if !matches!(
        outcome,
        Err(super::public_generic_component_v1_bindings::exports::semaprax::public_generic_component::adapter::Failure::ContractViolation)
    ) {
        return Err(failure(format!(
            "checked contract failure did not return the typed Component error: {outcome:?}"
        )));
    }

    // The two consumed input slots must be available again, not merely absent
    // from the result: keep all 64 fixed-arena resources live simultaneously.
    let mut live = Vec::with_capacity(64);
    for index in 0..64 {
        live.push(bytes.call_constructor(&mut store, &[index])?);
    }
    for resource in live {
        resource.resource_drop(&mut store)?;
    }
    let fresh = bytes.call_constructor(&mut store, &[3, 2, 1])?;
    if bytes.call_read(&mut store, fresh)? != [3, 2, 1] {
        return Err(failure(
            "Component did not recover after resource saturation",
        ));
    }
    fresh.resource_drop(&mut store)?;

    // A constructor trap leaves this Wasmtime instance unable to run a guest
    // destructor; isolate the cap control in a disposable Store, after the
    // positive settlement and re-entry observations above have completed.
    let mut saturated = Store::new(&engine, ());
    saturated.set_fuel(1_000_000_000)?;
    let saturated_bindings =
        PublicGenericComponentV1::instantiate(&mut saturated, &component, &linker)?;
    let saturated_bytes = saturated_bindings
        .semaprax_public_generic_component_adapter()
        .owned_bytes();
    let mut saturated_live = Vec::with_capacity(64);
    for index in 0..64 {
        saturated_live.push(saturated_bytes.call_constructor(&mut saturated, &[index])?);
    }
    if saturated_bytes
        .call_constructor(&mut saturated, &[255])
        .is_ok()
    {
        return Err(failure("Component admitted a 65th live resource"));
    }
    drop(saturated_live);
    drop(saturated);
    Ok(())
}

fn verify_contract_failure_component_bytes(bytes: &[u8]) -> HostResult<()> {
    let mut raw_digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(raw_digest, "{byte:02x}")?;
    }
    if raw_digest != EXPECTED_CONTRACT_FAILURE_COMPONENT_SHA256 {
        return Err(failure(
            "contract-failure Component differs from pinned bytes",
        ));
    }
    Ok(())
}

fn prove_maximum_leaf_transfer(
    bindings: &PublicGenericComponentV1,
    store: &mut Store<()>,
) -> HostResult<()> {
    let adapter = bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    let sentinel_input = vec![0xa5; 257];
    let sentinel = bytes
        .call_constructor(&mut *store, &sentinel_input)
        .map_err(|error| failure(format!("sentinel constructor trapped: {error}")))?;
    let left_input = (0..MAX_LIST_BYTES)
        .map(|index| {
            u8::try_from((index * 17 + 3) & 0xff)
                .expect("the 0xff mask bounds the value to one byte")
        })
        .collect::<Vec<_>>();
    let right_input = (0..MAX_LIST_BYTES)
        .map(|index| {
            u8::try_from((index * 29 + 11) & 0xff)
                .expect("the 0xff mask bounds the value to one byte")
        })
        .collect::<Vec<_>>();
    let left = bytes
        .call_constructor(&mut *store, &left_input)
        .map_err(|error| failure(format!("left max-leaf constructor trapped: {error}")))?;
    let right = bytes
        .call_constructor(&mut *store, &right_input)
        .map_err(|error| failure(format!("right max-leaf constructor trapped: {error}")))?;
    let (left_output, right_output) = adapter
        .call_invoke(&mut *store, left, right)
        .map_err(|error| failure(format!("max-leaf invoke trapped: {error}")))?
        .map_err(|status| failure(format!("component refused valid transform: {status:?}")))?;
    let left_observed = bytes
        .call_read(&mut *store, left_output)
        .map_err(|error| failure(format!("left max-leaf output read trapped: {error}")))?;
    let right_observed = bytes
        .call_read(&mut *store, right_output)
        .map_err(|error| failure(format!("right max-leaf output read trapped: {error}")))?;
    if left_observed != left_input || right_observed != right_input {
        return Err(failure(
            "descriptor-bound owned Bytes leaves differ from identity oracle",
        ));
    }
    let sentinel_observed = bytes
        .call_read(&mut *store, sentinel)
        .map_err(|error| failure(format!("sentinel read trapped after invoke: {error}")))?;
    if sentinel_observed != sentinel_input {
        return Err(failure(
            "invocation changed an unrelated retained Component resource",
        ));
    }
    left_output
        .resource_drop(&mut *store)
        .map_err(|error| failure(format!("left output drop trapped: {error}")))?;
    right_output
        .resource_drop(&mut *store)
        .map_err(|error| failure(format!("right output drop trapped: {error}")))?;
    sentinel
        .resource_drop(&mut *store)
        .map_err(|error| failure(format!("sentinel drop trapped: {error}")))?;

    Ok(())
}

fn prove_constructor_reuse(
    bindings: &PublicGenericComponentV1,
    store: &mut Store<()>,
) -> HostResult<()> {
    let adapter = bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    // These maximum-size cycles exceed the former staging tail; each must
    // reclaim its constructor allocation while resource storage stays live
    // only for the duration of the individual resource.
    let cycle_payload = vec![0x6d; MAX_LIST_BYTES];
    for cycle in 0..REUSE_CYCLES {
        let resource = bytes
            .call_constructor(&mut *store, &cycle_payload)
            .map_err(|error| {
                failure(format!("reuse cycle {cycle} constructor trapped: {error}"))
            })?;
        let observed = bytes
            .call_read(&mut *store, resource)
            .map_err(|error| failure(format!("reuse cycle {cycle} read trapped: {error}")))?;
        if observed != cycle_payload {
            return Err(failure(format!(
                "Component resource changed during reuse cycle {cycle}"
            )));
        }
        resource
            .resource_drop(&mut *store)
            .map_err(|error| failure(format!("reuse cycle {cycle} drop trapped: {error}")))?;
    }

    Ok(())
}

fn prove_transfer_reentry(
    bindings: &PublicGenericComponentV1,
    store: &mut Store<()>,
) -> HostResult<()> {
    let adapter = bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    // A second full transfer proves recovery after the tampered candidate
    // was refused and that the output resources were explicitly closed.
    let left = bytes
        .call_constructor(&mut *store, &[1, 2, 3])
        .map_err(|error| failure(format!("recovery left constructor trapped: {error}")))?;
    let right = bytes
        .call_constructor(&mut *store, &[4, 5])
        .map_err(|error| failure(format!("recovery right constructor trapped: {error}")))?;
    let (left_output, right_output) = adapter
        .call_invoke(&mut *store, left, right)
        .map_err(|error| failure(format!("recovery invoke trapped: {error}")))?
        .map_err(|status| failure(format!("component recovery call refused: {status:?}")))?;
    let left_observed = bytes
        .call_read(&mut *store, left_output)
        .map_err(|error| failure(format!("recovery left output read trapped: {error}")))?;
    let right_observed = bytes
        .call_read(&mut *store, right_output)
        .map_err(|error| failure(format!("recovery right output read trapped: {error}")))?;
    if left_observed != [1, 2, 3] || right_observed != [4, 5] {
        return Err(failure("component recovery call changed owned Bytes"));
    }
    left_output
        .resource_drop(&mut *store)
        .map_err(|error| failure(format!("recovery left output drop trapped: {error}")))?;
    right_output
        .resource_drop(&mut *store)
        .map_err(|error| failure(format!("recovery right output drop trapped: {error}")))?;

    Ok(())
}

fn prove_closed_and_transferred_handles_refuse(
    bindings: &PublicGenericComponentV1,
    store: &mut Store<()>,
) -> HostResult<()> {
    let adapter = bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    let closed = bytes.call_constructor(&mut *store, &[9, 8, 7])?;
    let stale_closed = closed;
    closed.resource_drop(&mut *store)?;
    if bytes.call_read(&mut *store, stale_closed).is_ok()
        || stale_closed.resource_drop(&mut *store).is_ok()
    {
        return Err(failure("closed Component resource was accepted again"));
    }

    let left = bytes.call_constructor(&mut *store, &[1])?;
    let right = bytes.call_constructor(&mut *store, &[2])?;
    let stale_left = left;
    let stale_right = right;
    let (left_result, right_result) = adapter
        .call_invoke(&mut *store, left, right)?
        .map_err(|status| failure(format!("valid transfer refused: {status:?}")))?;
    if bytes.call_read(&mut *store, stale_left).is_ok()
        || bytes.call_read(&mut *store, stale_right).is_ok()
        || stale_left.resource_drop(&mut *store).is_ok()
        || stale_right.resource_drop(&mut *store).is_ok()
    {
        return Err(failure("transferred Component input was accepted again"));
    }
    left_result.resource_drop(&mut *store)?;
    right_result.resource_drop(&mut *store)?;

    let fresh = bytes.call_constructor(&mut *store, &[6, 5, 4])?;
    if bytes.call_read(&mut *store, fresh)? != [6, 5, 4] {
        return Err(failure(
            "Component resource failed re-entry after stale refusals",
        ));
    }
    fresh.resource_drop(&mut *store)?;
    Ok(())
}

#[test]
fn retained_public_generic_component_rejects_foreign_instance_resource_owner() -> HostResult<()> {
    let artifact_bytes = retained_component_bytes()?;
    let mut actual_digest = String::with_capacity(64);
    for byte in Sha256::digest(&artifact_bytes) {
        write!(actual_digest, "{byte:02x}")?;
    }
    if actual_digest != EXPECTED_PUBLIC_GENERIC_COMPONENT_SHA256 {
        return Err(failure(
            "retained Component bytes differ from pinned fixture",
        ));
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, &artifact_bytes)?;
    if component.component_type().imports(&engine).len() != 0 {
        return Err(failure(
            "public-generic Component requested ambient imports",
        ));
    }
    let linker = Linker::<()>::new(&engine);
    let mut store = Store::new(&engine, ());
    store.set_fuel(1_000_000_000)?;
    let instance_a = PublicGenericComponentV1::instantiate(&mut store, &component, &linker)?;
    let instance_b = PublicGenericComponentV1::instantiate(&mut store, &component, &linker)?;
    let bytes_a = instance_a
        .semaprax_public_generic_component_adapter()
        .owned_bytes();
    let bytes_b = instance_b
        .semaprax_public_generic_component_adapter()
        .owned_bytes();

    let original = [0x31, 0xa7, 0x5c];
    let owner_a = bytes_a.call_constructor(&mut store, &original)?;
    let mismatch = bytes_b
        .call_read(&mut store, owner_a)
        .expect_err("instance B must refuse instance A's resource type");
    let mismatch_chain = format!("{mismatch:#}");
    if !mismatch_chain.contains("mismatched resource types") {
        return Err(failure(format!(
            "foreign-instance handle refusal was not a resource type mismatch: {mismatch_chain}"
        )));
    }

    if bytes_a.call_read(&mut store, owner_a)? != original {
        return Err(failure(
            "foreign-instance refusal changed or consumed instance A's resource",
        ));
    }
    owner_a.resource_drop(&mut store)?;

    let fresh_b = bytes_b.call_constructor(&mut store, &[0x90, 0x42])?;
    if bytes_b.call_read(&mut store, fresh_b)? != [0x90, 0x42] {
        return Err(failure(
            "instance B did not recover after foreign-resource refusal",
        ));
    }
    fresh_b.resource_drop(&mut store)?;
    Ok(())
}
