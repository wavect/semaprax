//! Private phase-B failure vocabulary: bounded messages, the process
//! arena reservation, sticky diagnostic carriers, and local errors.

use super::*;

/// Private phase-B static bundle construction. The output directory is
/// create-new and never merged with existing content.
pub(super) const PHASE_B_PUBLICATION_MESSAGE: &str =
    "Native Rust Interop output publication failed";
pub(super) const PHASE_B_COMPILE_MESSAGE: &str = "Native Rust Interop Clang compilation failed";
const PHASE_B_LINK_MESSAGE: &str = "Native Rust Interop Rust compilation or link failed";
pub(super) const PHASE_B_UNSUPPORTED_MESSAGE: &str =
    "Native Rust Interop target or toolchain is unsupported";
const PHASE_B_REPLAY_MESSAGE: &str = "Native Rust Interop generated artifact replay failed";
const PHASE_B_BUILDER_BUDGET_MESSAGE: &str =
    "Native Rust Interop max_builder_bytes exceeds 33554432";
pub(super) const PHASE_B_MANIFEST_BUDGET_MESSAGE: &str =
    "Native Rust Interop max_manifest_bytes exceeds 1048576";
pub(super) const PHASE_B_TOOL_VERSION_CAPACITY: usize = 65_536;
pub(super) const PHASE_B_TOOL_PATH_CAPACITY: usize = 32_768;
pub(super) const PHASE_B_MISSING_WINDOWS_LINKER: &str =
    r"C:\__semaprax_missing_vctools__\bin\Hostx64\x64\link.exe";
pub(super) const PHASE_B_MISSING_WINDOWS_VCTOOLS: &str = r"C:\__semaprax_missing_vctools__";
pub(super) const PHASE_B_VERSION_COMMAND_CAPACITY: usize = 256;
pub(super) const PHASE_B_TOOL_RESOLVER_CAPACITY: usize = PHASE_B_TOOL_PATH_CAPACITY * 7 + 256;
pub(super) const PHASE_B_PROCESS_INVOCATIONS: usize = 12;
#[cfg(windows)]
pub(super) const PHASE_B_PROCESS_ARENA_MAX_CAPACITY: usize = 1_245_188;
#[cfg(unix)]
pub(super) const PHASE_B_PROCESS_ARENA_MAX_CAPACITY: usize = 0;

pub(super) fn prepare_process_arena_authorized(
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<AuthorizedProcessArena, PhaseBLocalError> {
    if cfg!(windows) && (include.is_none() || libraries.is_none()) {
        return Err(PhaseBLocalError::Unsupported);
    }
    let plan = platform::prepare_process_arena_plan_with_environment(
        PHASE_B_PROCESS_INVOCATIONS,
        include,
        libraries,
    )
    .map_err(|error| match error {
        platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
        platform::Error::Invalid
        | platform::Error::Unsupported
        | platform::Error::Exists
        | platform::Error::Changed
        | platform::Error::Spawn
        | platform::Error::Exit => PhaseBLocalError::Unsupported,
    })?;
    let required = platform::prepared_process_arena_plan_capacity(&plan);
    if required > PHASE_B_PROCESS_ARENA_MAX_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let budget = reserve_phase_b(required)?;
    let arena = platform::materialize_process_arena_with_environment(plan, include, libraries)
        .map_err(|error| match error {
            platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
            platform::Error::Invalid
            | platform::Error::Unsupported
            | platform::Error::Exists
            | platform::Error::Changed
            | platform::Error::Spawn
            | platform::Error::Exit => PhaseBLocalError::Unsupported,
        })?;
    if platform::prepared_process_arena_owned_capacity(&arena) != required {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok(AuthorizedProcessArena::new(arena, budget))
}

#[cfg(test)]
pub(super) fn note_phase_b_process_arena_drop(value: u8) {
    PHASE_B_PROCESS_ARENA_DROP_ORDER.with(|order| {
        PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(|length| {
            let index = length.get();
            if index < 2 {
                let mut values = order.get();
                values[index] = value;
                order.set(values);
                length.set(index + 1);
            }
        });
    });
}

#[cfg(test)]
pub(super) fn reset_phase_b_process_arena_drop_observer() {
    PHASE_B_PROCESS_ARENA_DROPS.with(|drops| drops.set(0));
    PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(|drops| drops.set(0));
    PHASE_B_PROCESS_ARENA_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
pub(super) fn reset_phase_b_error_materialization_observer() {
    PHASE_B_EFFECT_STARTED.with(|started| started.set(false));
    PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn reset_phase_b_native_stage_arena_observer() {
    PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(|count| count.set(0));
    PHASE_B_NATIVE_STAGE_ARENA_SETS.with(|count| count.set(0));
    PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn reset_phase_b_object_authority_observer() {
    assert!(!PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
    let prior_length = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
    assert!(prior_length == 0 || prior_length == 2);
    PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_DROPS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(|count| count.set(0));
    PHASE_B_OBJECT_BYTES_DROPS.with(|count| count.set(0));
    PHASE_B_OBJECT_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
pub(super) fn assert_phase_b_object_drop_order(expected: usize) {
    assert_eq!(
        PHASE_B_OBJECT_BYTES_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        expected.saturating_mul(2),
    );
    assert_eq!(
        PHASE_B_OBJECT_DROP_ORDER.with(std::cell::Cell::get),
        if expected == 0 { [0, 0] } else { [1, 2] },
    );
    assert!(!PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
}

#[cfg(test)]
pub(super) fn reset_phase_b_manifest_authority_observer() {
    assert!(!PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
    let prior_length = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
    assert!(prior_length == 0 || prior_length == 2);
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES));
    PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(|count| count.set(0));
    PHASE_B_MANIFEST_ARENA_GROWTHS.with(|count| count.set(0));
    PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(|count| count.set(0));
    PHASE_B_MANIFEST_AUTHORITY_DROPS.with(|count| count.set(0));
    PHASE_B_MANIFEST_BYTES_DROPS.with(|count| count.set(0));
    PHASE_B_MANIFEST_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
pub(super) fn assert_phase_b_manifest_drop_order(expected: usize) {
    assert_eq!(
        PHASE_B_MANIFEST_BYTES_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        expected.saturating_mul(2)
    );
    assert_eq!(
        PHASE_B_MANIFEST_DROP_ORDER.with(std::cell::Cell::get),
        if expected == 0 { [0, 0] } else { [1, 2] }
    );
    assert!(!PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
}

#[cfg(not(test))]
pub(super) fn reset_phase_b_error_materialization_observer() {}

#[cfg(test)]
pub(super) fn mark_phase_b_effect_started() {
    PHASE_B_EFFECT_STARTED.with(|started| started.set(true));
}

#[cfg(not(test))]
pub(super) fn mark_phase_b_effect_started() {}

#[cfg(test)]
pub(super) fn observe_phase_b_error_materialization() {
    if PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get) {
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
    }
}

#[cfg(not(test))]
pub(super) fn observe_phase_b_error_materialization() {}

#[allow(
    clippy::vec_init_then_push,
    reason = "the one-element public diagnostic carrier requires an observed exact capacity"
)]
pub(super) fn diagnostic_vector(error: Diagnostic) -> Vec<Diagnostic> {
    observe_phase_b_error_materialization();
    let mut errors = Vec::with_capacity(1);
    errors.push(error);
    errors
}

pub(super) struct BundleBuildSuccess {
    pub(super) facts: NativeRustInteropBundleFacts,
    pub(super) overflow: Vec<Diagnostic>,
}

pub(super) enum BundleBuildError {
    Diagnostic(Diagnostic),
    Prepared {
        selected: Vec<Diagnostic>,
        overflow: Option<Vec<Diagnostic>>,
    },
}

impl From<Diagnostic> for BundleBuildError {
    fn from(error: Diagnostic) -> Self {
        Self::Diagnostic(error)
    }
}

impl BundleBuildError {
    pub(super) fn into_diagnostics(self, overflowed: bool) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostic(error) => {
                if overflowed {
                    diagnostic_vector(b109("max_builder_bytes", MAX_BUILDER_BYTES))
                } else {
                    diagnostic_vector(error)
                }
            }
            Self::Prepared { selected, overflow } => {
                if overflowed {
                    overflow.unwrap_or(selected)
                } else {
                    selected
                }
            }
        }
    }
}

struct StickyDiagnosticCarrier {
    errors: Option<Vec<Diagnostic>>,
}

impl StickyDiagnosticCarrier {
    #[allow(
        clippy::vec_init_then_push,
        reason = "the pre-effect sticky diagnostic carrier requires an observed exact capacity"
    )]
    fn prepare(code: &'static str, message: &'static str) -> Result<Self, Diagnostic> {
        let maximum = message
            .len()
            .checked_add(std::mem::size_of::<Diagnostic>())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(maximum)?;
        observe_phase_b_error_materialization();
        let diagnostic = Diagnostic::io(code, message);
        let mut errors = Vec::with_capacity(1);
        errors.push(diagnostic);
        let retained = errors[0]
            .message
            .capacity()
            .checked_add(
                errors
                    .capacity()
                    .checked_mul(std::mem::size_of::<Diagnostic>())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if errors.capacity() != 1 || retained > maximum {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            errors: Some(errors),
        })
    }

    fn take(&mut self) -> Vec<Diagnostic> {
        self.errors
            .take()
            .expect("sticky phase-B diagnostic is consumed once")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PhaseBLocalError {
    BuilderBudget,
    ManifestBudget,
    Unsupported,
    Replay,
    Compile,
    Link,
    Publication,
}

pub(super) struct PreparedManifestPlan {
    file_names: [&'static str; 6],
    manifest: AuthorizedManifest,
}

impl PreparedManifestPlan {
    pub(super) fn prepare(object_name: &'static str) -> Result<Self, PhaseBLocalError> {
        let file_names = canonical_manifest_file_names();
        if object_name != file_names[2] {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        let capacity = PHASE_B_MANIFEST_PLAN_CAPACITY.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let capacity = MAX_MANIFEST_BYTES;
        let authority = reserve_phase_b(capacity)?;
        let arena = String::with_capacity(capacity);
        #[cfg(test)]
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(|count| count.set(count.get().saturating_add(1)));
        if arena.capacity() != MAX_MANIFEST_BYTES {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        Ok(Self {
            file_names,
            manifest: AuthorizedManifest::new(arena, authority)?,
        })
    }

    #[allow(clippy::too_many_arguments, reason = "manifest inputs remain explicit")]
    pub(super) fn render(
        mut self,
        prepared: &PreparedNativeRustInterop,
        files: &[(&str, &[u8]); 6],
        clang_path: &str,
        clang_version: &str,
        rustc: &RustcVersion,
        target: &str,
    ) -> Result<AuthorizedManifest, PhaseBLocalError> {
        if files
            .iter()
            .zip(self.file_names)
            .any(|((actual, _), expected)| *actual != expected)
            || self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        self.manifest.check()?;
        let mut count = CountingSink {
            bytes: 0,
            maximum: MAX_MANIFEST_BYTES,
            overflowed: false,
        };
        write_manifest(
            &mut count,
            prepared,
            files,
            clang_path,
            clang_version,
            rustc,
            target,
        )
        .map_err(|_| PhaseBLocalError::ManifestBudget)?;
        #[cfg(test)]
        if PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(std::cell::Cell::get) {
            count.overflowed = true;
        }
        if count.overflowed {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        write_manifest(
            &mut self.manifest.manifest.bytes,
            prepared,
            files,
            clang_path,
            clang_version,
            rustc,
            target,
        )
        .map_err(|_| PhaseBLocalError::ManifestBudget)?;
        #[cfg(test)]
        if self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES {
            PHASE_B_MANIFEST_ARENA_GROWTHS.with(|count| count.set(count.get().saturating_add(1)));
        }
        if self.manifest.manifest.bytes.len() != count.bytes
            || self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        self.manifest.check()?;
        Ok(self.manifest)
    }
}

impl PhaseBLocalError {
    pub(super) const fn index(self) -> usize {
        match self {
            Self::BuilderBudget => 0,
            Self::ManifestBudget => 1,
            Self::Unsupported => 2,
            Self::Replay => 3,
            Self::Compile => 4,
            Self::Link => 5,
            Self::Publication => 6,
        }
    }

    pub(super) const fn diagnostic(self) -> (&'static str, &'static str) {
        match self {
            Self::BuilderBudget => ("SPX-B109", PHASE_B_BUILDER_BUDGET_MESSAGE),
            Self::ManifestBudget => ("SPX-B109", PHASE_B_MANIFEST_BUDGET_MESSAGE),
            Self::Unsupported => ("SPX-B110", PHASE_B_UNSUPPORTED_MESSAGE),
            Self::Replay => ("SPX-B111", PHASE_B_REPLAY_MESSAGE),
            Self::Compile => ("SPX-I230", PHASE_B_COMPILE_MESSAGE),
            Self::Link => ("SPX-I231", PHASE_B_LINK_MESSAGE),
            Self::Publication => ("SPX-I232", PHASE_B_PUBLICATION_MESSAGE),
        }
    }
}

pub(super) fn debit_phase_b(bytes: usize) -> Result<(), PhaseBLocalError> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(PhaseBLocalError::BuilderBudget)
    }
}

pub(super) fn reserve_phase_b(maximum: usize) -> Result<TemporaryBudget, PhaseBLocalError> {
    let remaining = crate::bounded_output::remaining_active().unwrap_or(MAX_BUILDER_BYTES);
    if maximum > remaining {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    debit_phase_b(maximum)?;
    Ok(TemporaryBudget { reserved: maximum })
}

pub(super) fn shrink_phase_b(
    authority: &mut TemporaryBudget,
    actual: usize,
) -> Result<(), PhaseBLocalError> {
    if actual > authority.reserved {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    crate::bounded_output::release_active(authority.reserved - actual);
    authority.reserved = actual;
    Ok(())
}

pub(super) fn retain_phase_b(
    mut authority: TemporaryBudget,
    actual: usize,
) -> Result<(), PhaseBLocalError> {
    if actual > authority.reserved {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    crate::bounded_output::release_active(authority.reserved - actual);
    authority.reserved = 0;
    Ok(())
}

pub(super) struct PhaseBErrorCarriers {
    carriers: [StickyDiagnosticCarrier; 7],
}

pub(super) fn finish_bounded_bundle(
    result: Result<BundleBuildSuccess, BundleBuildError>,
    overflowed: bool,
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    match result {
        Ok(success) if overflowed => Err(success.overflow),
        Ok(success) => Ok(success.facts),
        Err(error) => Err(error.into_diagnostics(overflowed)),
    }
}

impl PhaseBErrorCarriers {
    pub(super) fn prepare() -> Result<Self, Diagnostic> {
        let kinds = [
            PhaseBLocalError::BuilderBudget,
            PhaseBLocalError::ManifestBudget,
            PhaseBLocalError::Unsupported,
            PhaseBLocalError::Replay,
            PhaseBLocalError::Compile,
            PhaseBLocalError::Link,
            PhaseBLocalError::Publication,
        ];
        let carriers = kinds.map(|kind| {
            let (code, message) = kind.diagnostic();
            StickyDiagnosticCarrier::prepare(code, message)
        });
        let [Ok(builder), Ok(manifest), Ok(unsupported), Ok(replay), Ok(compile), Ok(link), Ok(publication)] =
            carriers
        else {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        };
        let carriers = [
            builder,
            manifest,
            unsupported,
            replay,
            compile,
            link,
            publication,
        ];
        #[cfg(test)]
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(|identities| {
            identities.set(std::array::from_fn(|index| {
                carriers[index].errors.as_ref().expect("prepared")[0]
                    .message
                    .as_ptr() as usize
            }));
        });
        Ok(Self { carriers })
    }

    pub(super) fn take(&mut self, kind: PhaseBLocalError) -> Vec<Diagnostic> {
        self.carriers[kind.index()].take()
    }

    pub(super) fn error(&mut self, kind: PhaseBLocalError) -> BundleBuildError {
        let selected = self.take(kind);
        let overflow = if kind == PhaseBLocalError::BuilderBudget {
            None
        } else {
            Some(self.take(PhaseBLocalError::BuilderBudget))
        };
        BundleBuildError::Prepared { selected, overflow }
    }
}
