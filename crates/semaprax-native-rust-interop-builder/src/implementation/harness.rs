//! Generated Rust harness rendering and the prepared, exactly bounded
//! phase-B build invocations.

use super::*;

fn render_rust_harness(
    output: &mut impl std::fmt::Write,
    prepared: &PreparedNativeRustInterop,
) -> std::fmt::Result {
    output.write_str(
        "#[path=\"semaprax_native_rust_interop.rs\"]mod semaprax_native_rust_interop;\nuse semaprax_native_rust_interop::*;\nstruct Host;\nimpl NativeRustImports for Host{\n",
    )?;
    for import in &prepared.imports {
        write!(
            output,
            "fn {}(&mut self{}",
            import.rust_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
        )?;
        for (index, parameter) in import.parameters.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            write!(output, "_arg_{index}: {}", rust_type(parameter.ty))?;
        }
        write!(
            output,
            ")->NativeRustImportResult<{}>{{{} }}\n",
            rust_type(import.result),
            match import.result {
                ScalarType::Unit => "NativeRustImportResult::Success(())",
                ScalarType::Bool => "NativeRustImportResult::Success(false)",
                ScalarType::I64 => "NativeRustImportResult::Success(0)",
            }
        )?;
    }
    output.write_str("}\n#[no_mangle]pub extern \"C\" fn spxnr1_rust_harness_run()->i32{let code=core::num::NonZeroU32::new(1).unwrap();")?;
    if !prepared.imports.is_empty() {
        output.write_str("let _=NativeRustImportResult::<()>::Status{code,class:NativeRustStatusClass::Import,retryable:false};let _=NativeRustImportResult::<()>::HostFailure;")?;
    }
    output.write_str("let probe=NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class:NativeRustStatusClass::Semantic,retryable:false};if let NativeRustCallError::Semantic{domain_id,code,class,retryable}=probe{let _=(domain_id,code,class,retryable);}let caps=match NativeRustCapabilities::new(&[")?;
    let mut previous = None;
    let mut first = true;
    loop {
        let mut selected = None;
        for capability in prepared
            .imports
            .iter()
            .flat_map(|import| &import.capabilities)
            .map(String::as_str)
        {
            if previous.is_none_or(|prior| capability > prior)
                && selected.is_none_or(|current| capability < current)
            {
                selected = Some(capability);
            }
        }
        let Some(capability) = selected else {
            break;
        };
        if !first {
            output.write_char(',')?;
        }
        write_json_string(output, capability)?;
        previous = Some(capability);
        first = false;
    }
    output.write_str(
        "]){Ok(value)=>value,Err(_)=>return 2};let mut bridge=NativeRustBridge::new(Host,caps);",
    )?;
    for export in &prepared.exports {
        write!(output, "let _closed_result=bridge.{}(", export.rust_method)?;
        for (index, parameter) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.write_char(',')?;
            }
            output.write_str(match parameter.ty {
                ScalarType::I64 => "0",
                ScalarType::Bool => "false",
                ScalarType::Unit => "()",
            })?;
        }
        output.write_str(");")?;
    }
    output.write_str("0}\n")
}

#[derive(Default)]
struct HarnessCount {
    length: usize,
}

impl std::fmt::Write for HarnessCount {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.length = self
            .length
            .checked_add(value.len())
            .ok_or(std::fmt::Error)?;
        Ok(())
    }
}

pub(super) fn prepare_rust_harness(
    prepared: &PreparedNativeRustInterop,
) -> Result<(String, TemporaryBudget), PhaseBLocalError> {
    let mut count = HarnessCount::default();
    render_rust_harness(&mut count, prepared).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(count.length)?;
    let mut output = String::with_capacity(count.length);
    if output.capacity() != count.length {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    render_rust_harness(&mut output, prepared).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if output.len() != count.length || output.capacity() != count.length {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok((output, budget))
}

const PHASE_B_INVOCATION_ARGUMENT_CAPACITY: usize = 16_384;

pub(super) struct PreparedBuildInvocations {
    pub(super) c_o0: (platform::PreparedCCompileInvocation, TemporaryBudget),
    pub(super) c_o2: (platform::PreparedCCompileInvocation, TemporaryBudget),
    pub(super) rust: (platform::PreparedRustCompileInvocation, TemporaryBudget),
    pub(super) c_main: (platform::PreparedCCompileInvocation, TemporaryBudget),
    pub(super) link_o0: (platform::PreparedLinkInvocation, TemporaryBudget),
    pub(super) run_o0: (platform::PreparedRunInvocation, TemporaryBudget),
    pub(super) link_o2: (platform::PreparedLinkInvocation, TemporaryBudget),
    pub(super) run_o2: (platform::PreparedRunInvocation, TemporaryBudget),
}

fn prepare_invocation<T>(
    maximum: usize,
    prepare: impl FnOnce() -> Result<T, platform::Error>,
    capacity: impl FnOnce(&T) -> usize,
) -> Result<(T, TemporaryBudget), PhaseBLocalError> {
    let mut budget = reserve_phase_b(maximum)?;
    let invocation = prepare().map_err(|error| match error {
        platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
        platform::Error::Invalid
        | platform::Error::Unsupported
        | platform::Error::Exists
        | platform::Error::Changed
        | platform::Error::Spawn
        | platform::Error::Exit => PhaseBLocalError::Unsupported,
    })?;
    let owned = capacity(&invocation);
    if owned > budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    shrink_phase_b(&mut budget, owned)?;
    #[cfg(test)]
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((invocation, budget))
}

pub(super) fn consume_invocation<T>(plan: (T, TemporaryBudget)) -> (T, TemporaryBudget) {
    #[cfg(test)]
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    plan
}

pub(super) fn prepare_build_invocations(
    prepared: &PreparedNativeRustInterop,
    sanitizers: bool,
    linker: Option<&OsStr>,
    vctools: Option<&OsStr>,
) -> Result<PreparedBuildInvocations, PhaseBLocalError> {
    let c_maximum = MAX_GENERATED_C_BYTES
        .checked_add(PHASE_B_INVOCATION_ARGUMENT_CAPACITY)
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    let command_maximum = PHASE_B_INVOCATION_ARGUMENT_CAPACITY;
    let link_command_maximum = if cfg!(windows) {
        command_maximum
            .checked_add(
                PHASE_B_TOOL_RESOLVER_CAPACITY
                    .checked_mul(2)
                    .ok_or(PhaseBLocalError::BuilderBudget)?,
            )
            .ok_or(PhaseBLocalError::BuilderBudget)?
    } else {
        command_maximum
    };
    let staticlib_name = if cfg!(windows) {
        "semaprax_bridge.lib"
    } else {
        "libsemaprax_bridge.a"
    };
    let link_o0_name = if cfg!(windows) {
        "__semaprax_native_rust_link_O0.exe"
    } else {
        "__semaprax_native_rust_link_O0"
    };
    let link_o2_name = if cfg!(windows) {
        "__semaprax_native_rust_link_O2.exe"
    } else {
        "__semaprax_native_rust_link_O2"
    };
    Ok(PreparedBuildInvocations {
        c_o0: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "module.c".as_ref(),
                    0,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        c_o2: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "module.c".as_ref(),
                    2,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        rust: prepare_invocation(
            command_maximum,
            || {
                platform::prepare_rust_compile_invocation(
                    &prepared.target.triple,
                    "__semaprax_native_rust_link.rs".as_ref(),
                    staticlib_name.as_ref(),
                )
            },
            platform::prepared_rust_compile_owned_capacity,
        )?,
        c_main: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "__semaprax_native_rust_main.c".as_ref(),
                    2,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        link_o0: prepare_invocation(
            link_command_maximum,
            || {
                platform::prepare_link_invocation(
                    &prepared.target.triple,
                    linker,
                    vctools,
                    "__semaprax_native_rust_main.o".as_ref(),
                    "module_O0.o".as_ref(),
                    staticlib_name.as_ref(),
                    link_o0_name.as_ref(),
                    sanitizers,
                )
            },
            platform::prepared_link_owned_capacity,
        )?,
        run_o0: prepare_invocation(
            command_maximum,
            platform::prepare_run_invocation,
            platform::prepared_run_owned_capacity,
        )?,
        link_o2: prepare_invocation(
            link_command_maximum,
            || {
                platform::prepare_link_invocation(
                    &prepared.target.triple,
                    linker,
                    vctools,
                    "__semaprax_native_rust_main.o".as_ref(),
                    "module_O2.o".as_ref(),
                    staticlib_name.as_ref(),
                    link_o2_name.as_ref(),
                    sanitizers,
                )
            },
            platform::prepared_link_owned_capacity,
        )?,
        run_o2: prepare_invocation(
            command_maximum,
            platform::prepare_run_invocation,
            platform::prepared_run_owned_capacity,
        )?,
    })
}
