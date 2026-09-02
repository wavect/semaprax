//! Frozen tool environment, toolchain planning, and toolchain
//! authentication for private phase B.

use super::*;

#[cfg(test)]
pub(super) struct TestTool {
    pub(super) path: PathBuf,
}

#[cfg(test)]
pub(super) fn same_file_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    if left.len() != right.len()
        || left.is_file() != right.is_file()
        || left.modified().ok() != right.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
pub(super) fn configured_tool(variable: &str) -> Result<TestTool, Diagnostic> {
    if let Some(value) = std::env::var_os(variable) {
        let path = std::fs::canonicalize(value).map_err(|_| b110())?;
        return Ok(TestTool { path });
    }
    let name = if variable == "RUSTC" {
        if cfg!(windows) {
            "rustc.exe"
        } else {
            "rustc"
        }
    } else if cfg!(windows) {
        "clang.exe"
    } else {
        "clang"
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join(name);
            let Ok(path) = std::fs::canonicalize(candidate) else {
                continue;
            };
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                return Ok(TestTool { path });
            }
        }
    }
    Err(b110())
}

#[cfg(test)]
pub(super) fn bind_test_tool_environment(command: &mut std::process::Command) {
    #[cfg(windows)]
    for variable in ["INCLUDE", "LIB"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
pub(super) fn bind_test_rust_linker(command: &mut std::process::Command, _clang: &TestTool) {
    #[cfg(windows)]
    let linker = {
        let configured =
            std::env::var_os("SEMAPRAX_LINKER").expect("configured absolute Windows linker");
        let configured = PathBuf::from(configured);
        assert!(configured.is_absolute(), "Windows linker must be absolute");
        let linker = std::fs::canonicalize(configured).expect("canonical Windows linker");
        let metadata = std::fs::symlink_metadata(&linker).expect("stat canonical Windows linker");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Windows linker must be a regular non-symlink file"
        );
        linker
    };
    #[cfg(not(windows))]
    let linker = _clang.path.clone();
    command
        .arg("-C")
        .arg(format!("linker={}", linker.display()));
    #[cfg(target_os = "linux")]
    command.args(["-C", "link-arg=--ld-path=/usr/bin/ld"]);
}

pub(super) struct RustcVersion {
    pub(super) storage: String,
    pub(super) boundaries: [usize; 5],
}

impl RustcVersion {
    pub(super) fn prepared() -> Result<Self, PhaseBLocalError> {
        let storage = String::with_capacity(PHASE_B_TOOL_VERSION_CAPACITY);
        if storage.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(Self {
            storage,
            boundaries: [0; 5],
        })
    }

    pub(super) fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    fn field(&self, index: usize) -> &str {
        &self.storage[self.boundaries[index]..self.boundaries[index + 1]]
    }

    pub(super) fn release(&self) -> &str {
        self.field(0)
    }

    pub(super) fn commit_hash(&self) -> &str {
        self.field(1)
    }

    pub(super) fn host(&self) -> &str {
        self.field(2)
    }

    pub(super) fn llvm_version(&self) -> &str {
        self.field(3)
    }

    pub(super) fn store(&mut self, values: [&str; 4]) -> Result<(), PhaseBLocalError> {
        if self.capacity() != PHASE_B_TOOL_VERSION_CAPACITY
            || !self.storage.is_empty()
            || self.boundaries != [0; 5]
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        let total = values
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()));
        if total.is_none_or(|total| total > self.capacity()) {
            return Err(PhaseBLocalError::Unsupported);
        }
        for (index, value) in values.into_iter().enumerate() {
            self.storage.push_str(value);
            self.boundaries[index + 1] = self.storage.len();
        }
        if self.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn from_fields(values: [&str; 4]) -> Self {
        let mut version = Self::prepared().unwrap();
        version.store(values).unwrap();
        version
    }
}

struct FrozenToolEnvironment {
    clang: Option<OsString>,
    rustc: Option<OsString>,
    linker: Option<OsString>,
    vctools: Option<OsString>,
    path: Option<OsString>,
    sanitizer: Option<OsString>,
    include: Option<OsString>,
    libraries: Option<OsString>,
    budget: TemporaryBudget,
}

pub(super) struct AuthorizedProcessArena {
    arena: Option<platform::PreparedProcessArena>,
    budget: Option<TemporaryBudget>,
}

impl AuthorizedProcessArena {
    pub(super) fn new(arena: platform::PreparedProcessArena, budget: TemporaryBudget) -> Self {
        Self {
            arena: Some(arena),
            budget: Some(budget),
        }
    }

    pub(super) fn arena(&self) -> Result<&platform::PreparedProcessArena, PhaseBLocalError> {
        self.arena.as_ref().ok_or(PhaseBLocalError::BuilderBudget)
    }

    pub(super) fn arena_mut(
        &mut self,
    ) -> Result<&mut platform::PreparedProcessArena, PhaseBLocalError> {
        self.arena.as_mut().ok_or(PhaseBLocalError::BuilderBudget)
    }

    pub(super) fn authorized_capacity(&self) -> Result<usize, PhaseBLocalError> {
        self.budget
            .as_ref()
            .map(TemporaryBudget::maximum)
            .ok_or(PhaseBLocalError::BuilderBudget)
    }
}

impl Drop for AuthorizedProcessArena {
    fn drop(&mut self) {
        if let Some(arena) = self.arena.take() {
            drop(arena);
            #[cfg(test)]
            {
                PHASE_B_PROCESS_ARENA_DROPS.with(|drops| drops.set(drops.get() + 1));
                note_phase_b_process_arena_drop(1);
            }
        }
        if let Some(budget) = self.budget.take() {
            drop(budget);
            #[cfg(test)]
            {
                PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(|drops| drops.set(drops.get() + 1));
                note_phase_b_process_arena_drop(2);
            }
        }
    }
}

pub(super) struct PreparedToolchainPlan {
    environment: FrozenToolEnvironment,
    path_budget: TemporaryBudget,
    linker_resolver: Option<platform::PreparedToolResolver>,
    linker_resolver_budget: Option<TemporaryBudget>,
    discovery_output_budget: TemporaryBudget,
    direct_sysroot_output_budget: TemporaryBudget,
    rustc_output_budget: TemporaryBudget,
    clang_output_budget: TemporaryBudget,
    command_budget: TemporaryBudget,
    clang_resolver: platform::PreparedToolResolver,
    rustc_resolver: platform::PreparedToolResolver,
    discovery_invocation: platform::PreparedSysrootInvocation,
    direct_sysroot_invocation: platform::PreparedSysrootInvocation,
    rustc_invocation: platform::PreparedRustcVersionInvocation,
    clang_invocation: platform::PreparedVersionInvocation,
    process_arena: AuthorizedProcessArena,
    rustc_version: RustcVersion,
}

pub(super) struct ToolchainFacts {
    pub(super) rustc: platform::HeldDirectRustc,
    pub(super) clang: platform::HeldTool,
    pub(super) linker: Option<platform::HeldTool>,
    pub(super) process_arena: Option<AuthorizedProcessArena>,
    pub(super) rustc_version: RustcVersion,
    pub(super) clang_version: String,
}

fn freeze_tool_environment() -> Result<FrozenToolEnvironment, PhaseBLocalError> {
    #[cfg(test)]
    let invalid_tool_environment = PHASE_B_INVALID_TOOL_ENV_INJECTION.with(std::cell::Cell::get);
    #[cfg(not(test))]
    let invalid_tool_environment = false;
    let clang = if invalid_tool_environment {
        Some(OsString::from("__semaprax_missing_clang__"))
    } else {
        std::env::var_os("CLANG")
    };
    let rustc = if invalid_tool_environment {
        Some(OsString::from("__semaprax_missing_rustc__"))
    } else {
        std::env::var_os("RUSTC")
    };
    let linker = if cfg!(windows) {
        std::env::var_os("SEMAPRAX_LINKER")
    } else {
        None
    };
    let vctools = if cfg!(windows) {
        std::env::var_os("SEMAPRAX_VCTOOLS")
    } else {
        None
    };
    let path = std::env::var_os("PATH");
    let sanitizer = std::env::var_os("SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS");
    let include = if cfg!(windows) {
        std::env::var_os("INCLUDE")
    } else {
        None
    };
    let libraries = if cfg!(windows) {
        std::env::var_os("LIB")
    } else {
        None
    };
    let capacity = [
        &clang, &rustc, &linker, &vctools, &path, &sanitizer, &include, &libraries,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        total.checked_add(value.as_ref().map_or(0, OsString::capacity))
    })
    .ok_or(PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(capacity)?;
    Ok(FrozenToolEnvironment {
        clang,
        rustc,
        linker,
        vctools,
        path,
        sanitizer,
        include,
        libraries,
        budget,
    })
}

pub(super) fn prepare_toolchain_plan() -> Result<PreparedToolchainPlan, PhaseBLocalError> {
    let mut environment = freeze_tool_environment()?;
    let process_arena = prepare_process_arena_authorized(
        environment.include.as_deref(),
        environment.libraries.as_deref(),
    )?;
    let include = environment.include.take();
    let libraries = environment.libraries.take();
    drop(include);
    drop(libraries);
    let retained_environment = [
        &environment.clang,
        &environment.rustc,
        &environment.linker,
        &environment.vctools,
        &environment.path,
        &environment.sanitizer,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        total.checked_add(value.as_ref().map_or(0, OsString::capacity))
    })
    .ok_or(PhaseBLocalError::BuilderBudget)?;
    shrink_phase_b(&mut environment.budget, retained_environment)?;
    let clang_name = if cfg!(windows) { "clang.exe" } else { "clang" };
    let path_budget = reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)?;
    let discovery_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let direct_sysroot_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let rustc_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let clang_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let command_budget = reserve_phase_b(
        PHASE_B_VERSION_COMMAND_CAPACITY
            .checked_mul(4)
            .ok_or(PhaseBLocalError::BuilderBudget)?,
    )?;
    let clang_resolver = platform::prepare_tool_resolver(clang_name, PHASE_B_TOOL_PATH_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let resolver_owned = platform::prepared_tool_resolver_owned_capacity(&clang_resolver);
    if resolver_owned > path_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let rustc_name = if cfg!(windows) { "rustc.exe" } else { "rustc" };
    let rustc_resolver = platform::prepare_tool_resolver(rustc_name, PHASE_B_TOOL_PATH_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let rustc_resolver_owned = platform::prepared_tool_resolver_owned_capacity(&rustc_resolver);
    if rustc_resolver_owned > path_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let (linker_resolver_budget, linker_resolver) = if cfg!(windows) {
        let budget = reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)?;
        let resolver = platform::prepare_tool_resolver("link.exe", PHASE_B_TOOL_PATH_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
        if platform::prepared_tool_resolver_owned_capacity(&resolver) > budget.maximum() {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        (Some(budget), Some(resolver))
    } else {
        (None, None)
    };
    let discovery_invocation = platform::prepare_sysroot_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let direct_sysroot_invocation =
        platform::prepare_sysroot_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let rustc_invocation =
        platform::prepare_rustc_version_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let clang_invocation =
        platform::prepare_version_invocation("--version", PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let command_owned = platform::prepared_sysroot_owned_capacity(&discovery_invocation)
        .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
        .and_then(|discovery| {
            platform::prepared_sysroot_owned_capacity(&direct_sysroot_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|direct| discovery.checked_add(direct))
        })
        .and_then(|total| {
            platform::prepared_rustc_version_owned_capacity(&rustc_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|rustc| total.checked_add(rustc))
        })
        .and_then(|total| {
            platform::prepared_version_owned_capacity(&clang_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|clang| total.checked_add(clang))
        })
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    if command_owned > command_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let persistent = PHASE_B_TOOL_VERSION_CAPACITY;
    let persistent_budget = reserve_phase_b(persistent)?;
    let rustc_version = RustcVersion::prepared()?;
    if rustc_version.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    retain_phase_b(persistent_budget, rustc_version.capacity())?;
    Ok(PreparedToolchainPlan {
        environment,
        path_budget,
        linker_resolver_budget,
        discovery_output_budget,
        direct_sysroot_output_budget,
        rustc_output_budget,
        clang_output_budget,
        command_budget,
        clang_resolver,
        rustc_resolver,
        linker_resolver,
        discovery_invocation,
        direct_sysroot_invocation,
        rustc_invocation,
        clang_invocation,
        process_arena,
        rustc_version,
    })
}

pub(super) fn authenticate_toolchain(
    plan: PreparedToolchainPlan,
    target: &Target,
    cwd: &platform::HeldDirectory,
) -> Result<ToolchainFacts, PhaseBLocalError> {
    let PreparedToolchainPlan {
        environment,
        path_budget,
        linker_resolver_budget,
        discovery_output_budget,
        direct_sysroot_output_budget,
        rustc_output_budget,
        clang_output_budget,
        command_budget,
        clang_resolver,
        rustc_resolver,
        linker_resolver,
        discovery_invocation,
        direct_sysroot_invocation,
        rustc_invocation,
        clang_invocation,
        mut process_arena,
        mut rustc_version,
        ..
    } = plan;
    let FrozenToolEnvironment {
        clang: configured_clang,
        rustc: configured_rustc,
        linker: configured_linker,
        vctools: configured_vctools,
        path,
        sanitizer,
        include,
        libraries,
        budget: environment_budget,
    } = environment;
    match sanitizer {
        None => {}
        Some(value) if value == "1" && cfg!(target_os = "linux") => {}
        Some(_) => return Err(PhaseBLocalError::Unsupported),
    }
    if cfg!(windows)
        && !valid_windows_link_environment(
            configured_linker.as_deref(),
            configured_vctools.as_deref(),
        )
    {
        return Err(PhaseBLocalError::Unsupported);
    }
    let (clang, _clang_resolver) = platform::resolve_and_hold_tool_reusing_prepared(
        clang_resolver,
        configured_clang.as_deref(),
        path.as_deref(),
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    let linker = if cfg!(windows) {
        let configured = configured_linker
            .as_deref()
            .filter(|path| std::path::Path::new(path).is_absolute())
            .ok_or(PhaseBLocalError::Unsupported)?;
        let resolver = linker_resolver.ok_or(PhaseBLocalError::BuilderBudget)?;
        let (linker, _) =
            platform::resolve_and_hold_tool_reusing_prepared(resolver, Some(configured), None)
                .map_err(|_| PhaseBLocalError::Unsupported)?;
        if std::path::Path::new(platform::tool_path(&linker)).as_os_str() != configured {
            return Err(PhaseBLocalError::Unsupported);
        }
        #[cfg(test)]
        PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
        Some(linker)
    } else {
        if configured_linker.is_some()
            || configured_vctools.is_some()
            || linker_resolver.is_some()
            || linker_resolver_budget.is_some()
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        None
    };
    let (rustc, rustc_resolver) = if let Some(rustc) = configured_rustc.as_deref() {
        if std::path::Path::new(rustc).is_absolute() {
            platform::resolve_and_hold_tool_reusing_prepared(
                rustc_resolver,
                Some(rustc),
                path.as_deref(),
            )
            .map_err(|_| PhaseBLocalError::Unsupported)?
        } else {
            platform::resolve_and_hold_tool_reusing_prepared(rustc_resolver, None, path.as_deref())
                .map_err(|_| PhaseBLocalError::Unsupported)?
        }
    } else {
        platform::resolve_and_hold_tool_reusing_prepared(rustc_resolver, None, path.as_deref())
            .map_err(|_| PhaseBLocalError::Unsupported)?
    };
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    let configured_rustc = platform::tool_path(&rustc).to_owned();
    let discovery =
        platform::hold_rustc_discovery_prepared(rustc_resolver, OsStr::new(&configured_rustc))
            .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(rustc);
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(configured_clang);
    drop(configured_rustc);
    drop(configured_linker);
    drop(configured_vctools);
    drop(include);
    drop(libraries);
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let discovery_sysroot = platform::rustc_discovery_output_prepared(
        &discovery,
        cwd,
        discovery_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if discovery_sysroot.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut rustc = platform::hold_direct_rustc_prepared(discovery, discovery_sysroot.bytes())
        .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(discovery_sysroot);
    drop(discovery_output_budget);
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let direct_sysroot = platform::direct_rustc_output_prepared(
        &rustc,
        cwd,
        direct_sysroot_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if direct_sysroot.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    platform::direct_rustc_reproduces_sysroot(&mut rustc, direct_sysroot.bytes())
        .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    if PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(std::cell::Cell::get) {
        return Err(PhaseBLocalError::Unsupported);
    }
    drop(direct_sysroot);
    drop(direct_sysroot_output_budget);
    drop(path);
    drop(environment_budget);
    retain_phase_b(path_budget, platform::tool_path_capacity(&clang))?;
    if let Some(linker) = linker.as_ref() {
        retain_phase_b(
            linker_resolver_budget.ok_or(PhaseBLocalError::BuilderBudget)?,
            platform::tool_path_capacity(linker),
        )?;
    }
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let rustc_text = platform::direct_rustc_version_prepared(
        &rustc,
        cwd,
        rustc_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if rustc_text.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let rustc_bytes = rustc_text.into_bytes();
    let rustc_text = std::str::from_utf8(&rustc_bytes)
        .map_err(|_| PhaseBLocalError::Unsupported)?
        .trim();
    parse_rustc_version(rustc_text, &mut rustc_version)?;
    drop(rustc_bytes);
    drop(rustc_output_budget);
    if rustc_version.host() != target.triple {
        return Err(PhaseBLocalError::Unsupported);
    }
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let clang_text =
        platform::tool_version_prepared(&clang, cwd, clang_invocation, process_arena.arena_mut()?)
            .map_err(|_| PhaseBLocalError::Unsupported)?;
    if clang_text.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut clang_version =
        String::from_utf8(clang_text.into_bytes()).map_err(|_| PhaseBLocalError::Unsupported)?;
    let trimmed = clang_version.trim();
    let start = trimmed.as_ptr() as usize - clang_version.as_ptr() as usize;
    let end = start + trimmed.len();
    clang_version.truncate(end);
    clang_version.drain(..start);
    if clang_version.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    retain_phase_b(clang_output_budget, clang_version.capacity())?;
    drop(command_budget);
    if clang_version.is_empty() {
        return Err(PhaseBLocalError::Unsupported);
    }
    if platform::prepared_process_arena_remaining(process_arena.arena()?)
        != PHASE_B_PROCESS_INVOCATIONS - 4
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok(ToolchainFacts {
        rustc,
        clang,
        linker,
        process_arena: Some(process_arena),
        rustc_version,
        clang_version,
    })
}

pub(super) fn planned_sanitizers(plan: &PreparedToolchainPlan) -> bool {
    cfg!(target_os = "linux")
        && plan.environment.sanitizer.as_deref() == Some(std::ffi::OsStr::new("1"))
}

pub(super) fn planned_linker(plan: &PreparedToolchainPlan) -> Option<&OsStr> {
    if cfg!(windows) {
        if valid_windows_link_environment(
            plan.environment.linker.as_deref(),
            plan.environment.vctools.as_deref(),
        ) {
            plan.environment.linker.as_deref()
        } else {
            Some(OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER))
        }
    } else {
        None
    }
}

pub(super) fn planned_vctools(plan: &PreparedToolchainPlan) -> Option<&OsStr> {
    if cfg!(windows) {
        if valid_windows_link_environment(
            plan.environment.linker.as_deref(),
            plan.environment.vctools.as_deref(),
        ) {
            plan.environment.vctools.as_deref()
        } else {
            Some(OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS))
        }
    } else {
        None
    }
}

fn valid_windows_link_environment(linker: Option<&OsStr>, vctools: Option<&OsStr>) -> bool {
    let Some(linker) = linker.map(std::path::Path::new) else {
        return false;
    };
    let Some(vctools) = vctools.map(std::path::Path::new) else {
        return false;
    };
    linker.is_absolute()
        && vctools.is_absolute()
        && linker.strip_prefix(vctools).ok()
            == Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
}

pub(super) fn parse_rustc_version(
    source: &str,
    output: &mut RustcVersion,
) -> Result<(), PhaseBLocalError> {
    if output.capacity() != PHASE_B_TOOL_VERSION_CAPACITY
        || !output.storage.is_empty()
        || output.boundaries != [0; 5]
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut lines = source.lines();
    let header = lines
        .next()
        .filter(|line| line.starts_with("rustc ") && line.len() > 6)
        .ok_or(PhaseBLocalError::Unsupported)?;
    let mut values = [None; 4];
    let mut binary_seen = false;
    let mut date_seen = false;
    for line in lines {
        let (key, value) = line.split_once(": ").ok_or(PhaseBLocalError::Unsupported)?;
        let slot = match key {
            "release" => Some(0),
            "commit-hash" => Some(1),
            "host" => Some(2),
            "LLVM version" => Some(3),
            "binary" if !binary_seen => {
                binary_seen = true;
                None
            }
            "commit-date" if !date_seen => {
                date_seen = true;
                None
            }
            _ => return Err(PhaseBLocalError::Unsupported),
        };
        if let Some(slot) = slot {
            if values[slot].replace(value).is_some() {
                return Err(PhaseBLocalError::Unsupported);
            }
        }
    }
    let [Some(release), Some(commit_hash), Some(host), Some(llvm_version)] = values else {
        return Err(PhaseBLocalError::Unsupported);
    };
    if release.is_empty()
        || !header.contains(release)
        || commit_hash.len() < 7
        || !commit_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || host.is_empty()
        || llvm_version.is_empty()
        || [release, commit_hash, host, llvm_version]
            .iter()
            .any(|value| value.len() > PHASE_B_TOOL_VERSION_CAPACITY)
    {
        return Err(PhaseBLocalError::Unsupported);
    }
    output.store([release, commit_hash, host, llvm_version])
}
