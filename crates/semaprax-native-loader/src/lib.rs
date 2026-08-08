//! A narrow quarantine around native module loading for SEMAPRAX hosts.
//!
//! Loading a native library executes trusted platform code, including its
//! initializers and terminators. Exact descriptor equality establishes only
//! that the resolved getter returned the caller's expected bytes; it does not
//! prove root-image provenance, authenticate code, or make code trustworthy.

#![cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "windows"
))]

use libloading::Library;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Largest descriptor this quarantine will read from a native module.
pub const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;

/// Largest getter symbol accepted by the loader.
pub const MAX_GETTER_SYMBOL_BYTES: usize = 1024;

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// Process-local identity of one exact successful library open.
///
/// Opening the same canonical path twice produces distinct identities. Retains
/// of one lease preserve its identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ModuleInstanceId(NonZeroU64);

impl ModuleInstanceId {
    /// Return the process-local nonzero logical admission identifier.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// An opaque strong lease on one exact loaded native module instance.
///
/// This type deliberately does not implement [`Clone`] or [`Debug`]. Call
/// [`Self::retain`] when another owner must keep the module loaded. The module
/// is eligible for platform unload only after the last lease is dropped.
///
/// ```compile_fail
/// use semaprax_native_loader::NativeModuleLease;
///
/// fn clone_is_not_implicit(lease: NativeModuleLease) {
///     let _duplicate = lease.clone();
/// }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeModuleLease;
///
/// fn secret_loader_state_is_not_formatted(lease: NativeModuleLease) {
///     let _rendered = format!("{lease:?}");
/// }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeModuleLease;
///
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeModuleLease>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeModuleLease;
///
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeModuleLease>();
/// ```
pub struct NativeModuleLease {
    inner: Arc<LoadedModule>,
    // A final drop can execute arbitrary native terminators. Until the module
    // contract contains an explicit cross-thread teardown guarantee, every
    // retain remains confined to the thread that opened the module.
    _same_thread: PhantomData<Rc<()>>,
}

struct LoadedModule {
    instance_id: ModuleInstanceId,
    canonical_path: PathBuf,
    descriptor_len: usize,
    // Keep this last so diagnostic metadata is destroyed before the platform
    // loader is allowed to run module terminators.
    _library: Library,
}

impl NativeModuleLease {
    /// Creates another explicit strong lease on this exact module instance.
    #[must_use]
    pub fn retain(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _same_thread: PhantomData,
        }
    }

    /// Returns the process-local identity of this exact successful open.
    #[must_use]
    pub fn instance_id(&self) -> ModuleInstanceId {
        self.inner.instance_id
    }

    /// Returns whether two leases retain the same exact successful open.
    #[must_use]
    pub fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Returns the already-validated canonical absolute library path.
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.inner.canonical_path
    }

    /// Returns the number of descriptor bytes validated during admission.
    #[must_use]
    pub fn descriptor_len(&self) -> usize {
        self.inner.descriptor_len
    }
}

/// Failure while validating or opening an admitted native module.
#[derive(Debug)]
pub enum OpenError {
    /// The supplied path was not absolute.
    PathNotAbsolute,
    /// The supplied path could not be canonicalized.
    PathCanonicalization(std::io::Error),
    /// The supplied absolute path was not already in canonical form.
    PathNotCanonical,
    /// The getter symbol was empty, contained NUL, or exceeded the bound.
    InvalidGetterSymbol,
    /// The expected descriptor was empty or exceeded the bound.
    InvalidExpectedDescriptorLength { actual: usize, maximum: usize },
    /// The platform loader rejected the library.
    LibraryOpen(libloading::Error),
    /// The platform loader could not resolve the exact getter symbol.
    GetterLookup(libloading::Error),
    /// The admitted getter returned a null descriptor pointer.
    NullDescriptor,
    /// The returned bytes did not exactly equal the expected descriptor.
    DescriptorMismatch,
    /// The process-local instance identity space was exhausted.
    InstanceIdentityExhausted,
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotAbsolute => formatter.write_str("native module path is not absolute"),
            Self::PathCanonicalization(error) => {
                write!(
                    formatter,
                    "native module path cannot be canonicalized: {error}"
                )
            }
            Self::PathNotCanonical => {
                formatter.write_str("native module path is not already canonical")
            }
            Self::InvalidGetterSymbol => formatter.write_str(
                "native descriptor getter must be nonempty, NUL-free, and within the length bound",
            ),
            Self::InvalidExpectedDescriptorLength { actual, maximum } => write!(
                formatter,
                "expected native descriptor length {actual} is outside 1..={maximum}"
            ),
            Self::LibraryOpen(error) => write!(formatter, "native module open failed: {error}"),
            Self::GetterLookup(error) => {
                write!(formatter, "native descriptor getter lookup failed: {error}")
            }
            Self::NullDescriptor => {
                formatter.write_str("native descriptor getter returned a null pointer")
            }
            Self::DescriptorMismatch => {
                formatter.write_str("native descriptor bytes do not match exactly")
            }
            Self::InstanceIdentityExhausted => {
                formatter.write_str("native module instance identity space is exhausted")
            }
        }
    }
}

impl Error for OpenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PathCanonicalization(error) => Some(error),
            Self::LibraryOpen(error) | Self::GetterLookup(error) => Some(error),
            _ => None,
        }
    }
}

/// Opens one already-admitted native module and compares resolved getter bytes.
///
/// This function intentionally performs no search-path lookup and accepts only
/// an existing canonical absolute path. The caller supplies `getter_symbol` as
/// nonempty, NUL-free symbol-name bytes without a terminator; `libloading`
/// appends the platform-required C terminator internally. On success, the
/// returned opaque lease keeps this exact [`Library`] instance alive until its
/// final retain is dropped.
///
/// Exact equality proves only that the resolved getter returned
/// `expected_descriptor`. It does not prove that the getter belongs to the root
/// image, that the root image is compatible, or that admission was sound.
///
/// # Safety
///
/// The caller must have already admitted the root image and every dependency
/// that the platform loader can select at execution time as trusted native
/// code. The path, containing module directory, and dependency-search namespace
/// must not be adversarially replaced or mutated from admission until this call
/// returns. Loading may execute arbitrary initializers, and dropping the final
/// lease may execute arbitrary terminators.
///
/// `getter_symbol` must name a function with the exact C ABI type
/// `unsafe extern "C" fn() -> *const u8`. Calling it must not unwind and must be
/// valid at admission time. Its non-null return must remain readable and
/// immutable for at least `expected_descriptor.len()` bytes while the library
/// is loaded. No other code may concurrently mutate those bytes or invalidate
/// that storage. Satisfying these conditions is necessary because a native
/// symbol's actual type and returned allocation cannot be verified by Rust or
/// the platform loader.
pub unsafe fn open_admitted_exact(
    canonical_path: &Path,
    getter_symbol: &[u8],
    expected_descriptor: &[u8],
) -> Result<NativeModuleLease, OpenError> {
    validate_inputs(canonical_path, getter_symbol, expected_descriptor)?;

    let resolved_path =
        std::fs::canonicalize(canonical_path).map_err(OpenError::PathCanonicalization)?;
    if resolved_path != canonical_path {
        return Err(OpenError::PathNotCanonical);
    }

    // SAFETY: The caller's admission contract covers arbitrary initializer and
    // terminator execution by this exact canonical file.
    let library = unsafe { Library::new(canonical_path) }.map_err(OpenError::LibraryOpen)?;

    type DescriptorGetter = unsafe extern "C" fn() -> *const u8;
    // SAFETY: The caller guarantees the exact symbol has DescriptorGetter's ABI
    // and signature. Copying the function pointer lets the Symbol borrow end
    // before the Library moves into its lease.
    let getter: DescriptorGetter = *unsafe { library.get::<DescriptorGetter>(getter_symbol) }
        .map_err(OpenError::GetterLookup)?;

    // SAFETY: The caller guarantees invoking the admitted getter is valid and
    // cannot unwind across the C ABI boundary.
    let descriptor_pointer = unsafe { getter() };
    if descriptor_pointer.is_null() {
        return Err(OpenError::NullDescriptor);
    }

    // SAFETY: The caller guarantees a non-null result remains readable and
    // immutable for expected_descriptor.len() bytes while Library is alive.
    let actual_descriptor =
        unsafe { std::slice::from_raw_parts(descriptor_pointer, expected_descriptor.len()) };
    if actual_descriptor != expected_descriptor {
        return Err(OpenError::DescriptorMismatch);
    }

    let instance_id = allocate_instance_id()?;
    Ok(NativeModuleLease {
        inner: Arc::new(LoadedModule {
            instance_id,
            canonical_path: resolved_path,
            descriptor_len: expected_descriptor.len(),
            _library: library,
        }),
        _same_thread: PhantomData,
    })
}

fn validate_inputs(
    canonical_path: &Path,
    getter_symbol: &[u8],
    expected_descriptor: &[u8],
) -> Result<(), OpenError> {
    if !canonical_path.is_absolute() {
        return Err(OpenError::PathNotAbsolute);
    }
    if getter_symbol.is_empty()
        || getter_symbol.len() > MAX_GETTER_SYMBOL_BYTES
        || getter_symbol.contains(&0)
    {
        return Err(OpenError::InvalidGetterSymbol);
    }
    if expected_descriptor.is_empty() || expected_descriptor.len() > MAX_DESCRIPTOR_BYTES {
        return Err(OpenError::InvalidExpectedDescriptorLength {
            actual: expected_descriptor.len(),
            maximum: MAX_DESCRIPTOR_BYTES,
        });
    }
    Ok(())
}

fn allocate_instance_id() -> Result<ModuleInstanceId, OpenError> {
    loop {
        let current = NEXT_INSTANCE_ID.load(Ordering::Relaxed);
        let next = current
            .checked_add(1)
            .ok_or(OpenError::InstanceIdentityExhausted)?;
        if NEXT_INSTANCE_ID
            .compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let nonzero = NonZeroU64::new(current).ok_or(OpenError::InstanceIdentityExhausted)?;
            return Ok(ModuleInstanceId(nonzero));
        }
    }
}
