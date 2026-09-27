//! Mid-call interruption for the Component and Core-provider engines (issue
//! #292): a Wasmtime fuel budget that runs out partway through the
//! Component's `invoke` or the Core provider's `spx_pg_v1_call` is a trap,
//! not a typed failure. No partial result is ever published for either
//! engine: the trapped `Store` (and, with it, every resource the interrupted
//! call held) is discarded before the call could return, and a fresh
//! instance of the identical bytes is then usable. Both tests measure one
//! full call's own fuel cost first and interrupt a fresh call at roughly
//! half that budget, so the cut lands inside the call itself rather than
//! merely refusing to start it.
//!
//! See `docs/PUBLIC-GENERIC-WASM-COMPONENT-V1.md`'s "Cancellation and
//! mid-call interruption" section for the full cross-engine comparison,
//! including the interpreter's own step-budget analogue
//! (`RetainedCallOutcome::FuelExhausted`, `docs/INTERPRETER-V1.md`) and why
//! the native profile has no interruption primitive at all to compare.

use wasmtime::TypedFunc;

use super::{
    CORE_SCRATCH, CORE_SCRATCH_BYTES, CarrierLeaf, Engine, FUEL, HostResult, Instance, LeafKind,
    Module, Outcome, PARITY_MANIFEST, PARITY_PINS, Store, Subject, WITNESS_LEFT, WITNESS_RIGHT,
    acquire, as_i32, as_u32, component_call, core_call, engine, failure, instantiate, offset,
    prove_no_live_resources, split_lane,
};

#[test]
fn component_mid_call_fuel_interruption_discards_store_without_publishing() -> HostResult<()> {
    let subject = acquire(PARITY_MANIFEST, PARITY_PINS, &[], &[])?;
    let engine = engine()?;

    // Measure: a full successful call's own fuel cost from a known, ample
    // budget, on a Store used for nothing else.
    let (mut baseline, baseline_bindings) = instantiate(&engine, &subject.component)?;
    let before = baseline.get_fuel()?;
    let outcome = component_call(
        &baseline_bindings,
        &mut baseline,
        &WITNESS_LEFT,
        &WITNESS_RIGHT,
    )?;
    if outcome != Outcome::Leaves(WITNESS_RIGHT.to_vec(), WITNESS_LEFT.to_vec()) {
        return Err(failure("baseline component call did not swap as expected"));
    }
    let consumed = before
        .checked_sub(baseline.get_fuel()?)
        .filter(|&consumed| consumed > 0)
        .ok_or_else(|| failure("baseline component call consumed no measurable fuel"))?;
    drop(baseline);

    // Interrupt: a fresh instance, with its two owned inputs already
    // constructed, must trap on `invoke` when roughly half that budget
    // remains -- the cut lands inside the call, not before it starts.
    let (mut trapped, trapped_bindings) = instantiate(&engine, &subject.component)?;
    let adapter = trapped_bindings.semaprax_public_generic_component_adapter();
    let bytes = adapter.owned_bytes();
    let left = bytes.call_constructor(&mut trapped, &WITNESS_LEFT)?;
    let right = bytes.call_constructor(&mut trapped, &WITNESS_RIGHT)?;
    trapped.set_fuel(consumed / 2)?;
    if adapter.call_invoke(&mut trapped, left, right).is_ok() {
        return Err(failure(
            "halved fuel budget did not interrupt the component call; increase the margin",
        ));
    }
    // A trap is not a typed failure: no result was published for this call,
    // and the trapped Store -- with both owned inputs it still held -- is
    // discarded here, never reused for another call.
    drop(trapped);

    // Recovery: a fresh instance of the identical Component bytes still
    // executes the checked call, and its full fixed resource arena is
    // available, proving nothing the interrupted call held was retained.
    let (mut store, bindings) = instantiate(&engine, &subject.component)?;
    let outcome = component_call(&bindings, &mut store, &WITNESS_LEFT, &WITNESS_RIGHT)?;
    if outcome != Outcome::Leaves(WITNESS_RIGHT.to_vec(), WITNESS_LEFT.to_vec()) {
        return Err(failure(
            "fresh Component instance did not recover after mid-call fuel interruption",
        ));
    }
    prove_no_live_resources(&bindings, &mut store)
}

/// A live provider `Store`, its `call` export and the two i32 handles
/// `spx_pg_v1_call` takes.
type PreparedCall = (Store<()>, TypedFunc<(i32, i32), i64>, i32, i32);

/// Drive the standalone compiled Core provider up to (not including) its own
/// `spx_pg_v1_call`, exactly as [`super::core_call`] does for its own
/// checked-in witness case, returning the live `Store`, the `call` export,
/// and the two i32 handles `spx_pg_v1_call` itself takes.
fn core_open_and_prepare(engine: &Engine, subject: &Subject) -> HostResult<PreparedCall> {
    let module = Module::new(engine, &subject.provider_wasm)?;
    let mut store = Store::new(engine, ());
    store.set_fuel(FUEL)?;
    let instance = Instance::new(&mut store, &module, &[])?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| failure("compiled Core provider memory export missing"))?;
    let reserve = instance.get_typed_func::<i32, i64>(&mut store, "spx_pg_v1_scratch_reserve")?;
    let open =
        instance.get_typed_func::<(i32, i32, i32, i32), i64>(&mut store, "spx_pg_v1_open")?;
    let prepare =
        instance.get_typed_func::<(i32, i32, i32), i64>(&mut store, "spx_pg_v1_input_prepare")?;
    let call = instance.get_typed_func::<(i32, i32), i64>(&mut store, "spx_pg_v1_call")?;

    let scratch = as_i32(CORE_SCRATCH)?;
    let (status, pointer) = split_lane(reserve.call(&mut store, as_i32(CORE_SCRATCH_BYTES)?)?)?;
    if status != 0 || pointer != CORE_SCRATCH {
        return Err(failure(
            "compiled Core provider scratch reservation changed",
        ));
    }
    let descriptor = &subject.provider_descriptor;
    let binding = &subject.provider_binding;
    memory.write(&mut store, offset(CORE_SCRATCH)?, descriptor)?;
    memory.write(
        &mut store,
        offset(CORE_SCRATCH)? + descriptor.len(),
        binding,
    )?;
    let descriptor_len = as_u32(descriptor.len())?;
    let (status, provider) = split_lane(open.call(
        &mut store,
        (
            scratch,
            as_i32(descriptor_len)?,
            as_i32(CORE_SCRATCH + descriptor_len)?,
            as_i32(as_u32(binding.len())?)?,
        ),
    )?)?;
    if status != 0 {
        return Err(failure(format!("Core provider refused open: {status}")));
    }
    let provider = as_i32(provider)?;
    let input = subject
        .input_binding
        .frame_with_leaves(
            subject
                .input_binding
                .leaf_paths()
                .iter()
                .zip([&WITNESS_LEFT[..], &WITNESS_RIGHT[..]])
                .map(|(path, payload)| {
                    CarrierLeaf::new(path.clone(), LeafKind::Bytes, payload.to_vec())
                })
                .collect(),
        )
        .encode();
    memory.write(&mut store, offset(CORE_SCRATCH)?, &input)?;
    let (status, value) = split_lane(prepare.call(
        &mut store,
        (provider, scratch, as_i32(as_u32(input.len())?)?),
    )?)?;
    if status != 0 {
        return Err(failure(format!(
            "compiled Core provider refused input: {status}"
        )));
    }
    Ok((store, call, provider, as_i32(value)?))
}

#[test]
fn core_provider_mid_call_fuel_interruption_discards_store_without_publishing() -> HostResult<()> {
    let subject = acquire(PARITY_MANIFEST, PARITY_PINS, &[], &[])?;
    let engine = engine()?;

    // Measure: `spx_pg_v1_call` itself's own fuel cost on a fully prepared,
    // otherwise unconstrained Store.
    let (mut baseline, call, provider, value) = core_open_and_prepare(&engine, &subject)?;
    let before = baseline.get_fuel()?;
    let (status, _result) = split_lane(call.call(&mut baseline, (provider, value))?)?;
    if status != 0 {
        return Err(failure(format!(
            "baseline Core provider call status changed: {status}"
        )));
    }
    let consumed = before
        .checked_sub(baseline.get_fuel()?)
        .filter(|&consumed| consumed > 0)
        .ok_or_else(|| failure("baseline Core provider call consumed no measurable fuel"))?;
    drop(baseline);

    // Interrupt: a fresh, identically prepared Store must trap on
    // `spx_pg_v1_call` itself when roughly half that budget remains.
    let (mut trapped, call, provider, value) = core_open_and_prepare(&engine, &subject)?;
    trapped.set_fuel(consumed / 2)?;
    if call.call(&mut trapped, (provider, value)).is_ok() {
        return Err(failure(
            "halved fuel budget did not interrupt the Core provider call; increase the margin",
        ));
    }
    // A trap here never reaches `spx_pg_v1_provider_close`: the trapped
    // Store -- the one Wasm linear memory everything this provider call
    // allocated lived in -- is discarded, never reused.
    drop(trapped);

    // Recovery: a fresh module instance (via the shared `core_call` helper,
    // which opens, prepares, calls, exports and closes on its own fresh
    // Store) completes the full checked call.
    let outcome = core_call(
        &engine,
        &subject,
        &subject.provider_descriptor,
        &WITNESS_LEFT,
        &WITNESS_RIGHT,
    )?
    .map_err(|status| {
        failure(format!(
            "Core provider refused open after recovery: {status}"
        ))
    })?;
    if outcome != Outcome::Leaves(WITNESS_RIGHT.to_vec(), WITNESS_LEFT.to_vec()) {
        return Err(failure(
            "fresh Core provider instance did not recover after mid-call fuel interruption",
        ));
    }
    Ok(())
}
