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

/// Largest callable symbol accepted by the loader.
pub const MAX_CALLABLE_SYMBOL_BYTES: usize = 1024;

/// Largest canonical request or response buffer admitted for one call.
pub const MAX_CALL_WIRE_BYTES: usize = 1024 * 1024;

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

type CallableEntry = unsafe extern "C" fn(*const u8, u32, *mut u8, u32) -> u32;

/// An opaque strong lease on one exact loaded callable module instance.
///
/// The callable pointer is private and cannot outlive the retained platform
/// library. This type deliberately implements neither `Clone` nor `Debug` and
/// remains thread-confined for the same terminator-safety reason as
/// [`NativeModuleLease`].
///
/// ```compile_fail
/// use semaprax_native_loader::NativeCallableModuleLease;
/// fn clone_is_not_implicit(lease: NativeCallableModuleLease) { let _ = lease.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeCallableModuleLease;
/// fn secret_loader_state_is_not_formatted(lease: NativeCallableModuleLease) {
///     let _ = format!("{lease:?}");
/// }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeCallableModuleLease;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeCallableModuleLease>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeCallableModuleLease;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeCallableModuleLease>();
/// ```
pub struct NativeCallableModuleLease {
    inner: Arc<LoadedCallableModule>,
    _same_thread: PhantomData<Rc<()>>,
}

struct LoadedCallableModule {
    instance_id: ModuleInstanceId,
    canonical_path: PathBuf,
    descriptor_len: usize,
    callable: CallableEntry,
    // Keep this last so the callable pointer and metadata disappear before the
    // platform loader may run module terminators.
    _library: Library,
}

/// One preallocated, one-shot canonical byte-wire invocation.
///
/// Construction allocates every request/response byte before an ownership host
/// commits its inputs. The type exposes no raw pointer and is neither cloneable
/// nor formattable.
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedNativeCall;
/// fn clone_is_not_implicit(call: PreparedNativeCall) { let _ = call.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedNativeCall;
/// fn wire_bytes_are_not_formatted(call: PreparedNativeCall) { let _ = format!("{call:?}"); }
/// ```
pub struct PreparedNativeCall {
    module_instance: ModuleInstanceId,
    request: Vec<u8>,
    response: Vec<u8>,
    invoked: bool,
}

/// Stable failure while preallocating a canonical call wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallWireError {
    InvalidRequestLength { actual: usize, maximum: usize },
    InvalidResponseCapacity { actual: usize, maximum: usize },
    WrongModuleInstance,
    AlreadyInvoked,
}

impl fmt::Display for CallWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequestLength { actual, maximum } => write!(
                formatter,
                "native request length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidResponseCapacity { actual, maximum } => write!(
                formatter,
                "native response capacity {actual} is outside 1..={maximum}"
            ),
            Self::WrongModuleInstance => {
                formatter.write_str("prepared native call belongs to a different module instance")
            }
            Self::AlreadyInvoked => formatter.write_str("prepared native call was already invoked"),
        }
    }
}

impl Error for CallWireError {}

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

impl NativeCallableModuleLease {
    /// Creates another explicit strong lease on this exact callable instance.
    #[must_use]
    pub fn retain(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _same_thread: PhantomData,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> ModuleInstanceId {
        self.inner.instance_id
    }

    #[must_use]
    pub fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.inner.canonical_path
    }

    #[must_use]
    pub fn descriptor_len(&self) -> usize {
        self.inner.descriptor_len
    }

    /// Preallocate a one-shot request and zeroed response buffer.
    pub fn prepare_call(
        &self,
        request: Vec<u8>,
        response_capacity: usize,
    ) -> Result<PreparedNativeCall, CallWireError> {
        if request.is_empty() || request.len() > MAX_CALL_WIRE_BYTES {
            return Err(CallWireError::InvalidRequestLength {
                actual: request.len(),
                maximum: MAX_CALL_WIRE_BYTES,
            });
        }
        if response_capacity == 0 || response_capacity > MAX_CALL_WIRE_BYTES {
            return Err(CallWireError::InvalidResponseCapacity {
                actual: response_capacity,
                maximum: MAX_CALL_WIRE_BYTES,
            });
        }
        Ok(PreparedNativeCall {
            module_instance: self.instance_id(),
            request,
            response: vec![0; response_capacity],
            invoked: false,
        })
    }

    /// Invoke the exact eagerly resolved callable once.
    ///
    /// The returned numeric value is the provider's physical adapter result;
    /// semantic outcome decoding belongs to the descriptor-bound host. The
    /// response buffer remains available even for malformed provider output so
    /// the host can normalize it as an executed adapter failure.
    pub fn invoke(&self, call: &mut PreparedNativeCall) -> Result<u32, CallWireError> {
        if call.module_instance != self.instance_id() {
            return Err(CallWireError::WrongModuleInstance);
        }
        if call.invoked {
            return Err(CallWireError::AlreadyInvoked);
        }
        call.invoked = true;
        let request_len =
            u32::try_from(call.request.len()).map_err(|_| CallWireError::InvalidRequestLength {
                actual: call.request.len(),
                maximum: MAX_CALL_WIRE_BYTES,
            })?;
        let response_capacity = u32::try_from(call.response.len()).map_err(|_| {
            CallWireError::InvalidResponseCapacity {
                actual: call.response.len(),
                maximum: MAX_CALL_WIRE_BYTES,
            }
        })?;
        // SAFETY: The unsafe admission contract established the exact callable
        // ABI, bounded synchronous access, absence of retained pointers, and no
        // foreign unwind. Both slices remain live and disjoint for this call.
        let code = unsafe {
            (self.inner.callable)(
                call.request.as_ptr(),
                request_len,
                call.response.as_mut_ptr(),
                response_capacity,
            )
        };
        Ok(code)
    }
}

impl PreparedNativeCall {
    /// Return the complete preallocated response storage. Its canonical prefix
    /// and declared length must still be validated by the descriptor-bound host.
    #[must_use]
    pub fn response_storage(&self) -> &[u8] {
        &self.response
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
    /// The callable symbol was empty, contained NUL, or exceeded the bound.
    InvalidCallableSymbol,
    /// The expected descriptor was empty or exceeded the bound.
    InvalidExpectedDescriptorLength { actual: usize, maximum: usize },
    /// Callable admission accepts only the separately versioned descriptor v2.
    InvalidCallableDescriptorSchema,
    /// The platform loader rejected the library.
    LibraryOpen(libloading::Error),
    /// The platform loader could not resolve the exact getter symbol.
    GetterLookup(libloading::Error),
    /// The platform loader could not resolve the exact callable symbol.
    CallableLookup(libloading::Error),
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
            Self::InvalidCallableSymbol => formatter.write_str(
                "native callable symbol must be nonempty, NUL-free, and within the length bound",
            ),
            Self::InvalidExpectedDescriptorLength { actual, maximum } => write!(
                formatter,
                "expected native descriptor length {actual} is outside 1..={maximum}"
            ),
            Self::InvalidCallableDescriptorSchema => formatter
                .write_str("native callable admission requires an exact SPXNABI2 descriptor"),
            Self::LibraryOpen(error) => write!(formatter, "native module open failed: {error}"),
            Self::GetterLookup(error) => {
                write!(formatter, "native descriptor getter lookup failed: {error}")
            }
            Self::CallableLookup(error) => {
                write!(formatter, "native callable lookup failed: {error}")
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
            Self::LibraryOpen(error) | Self::GetterLookup(error) | Self::CallableLookup(error) => {
                Some(error)
            }
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
    // terminator execution by this exact canonical file. Unix uses RTLD_NOW so
    // every dependency relocation is resolved during admission rather than on
    // a later first call.
    let library = unsafe { open_library(canonical_path) }.map_err(OpenError::LibraryOpen)?;

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

/// Open one already-admitted descriptor-v2 module and eagerly bind its one
/// exact canonical byte-wire callable.
///
/// Input validation, descriptor comparison, and callable lookup all complete
/// before a process-local instance identity is allocated. No generic symbol
/// lookup, raw pointer, raw handle, or manual-close surface is exposed.
///
/// # Safety
///
/// The caller must satisfy every safety requirement of [`open_admitted_exact`]
/// for the root image, dependency namespace, getter, immutable descriptor
/// bytes, initializers, and terminators for the complete lifetime of every
/// returned retain. Before crossing this boundary, the caller must also have
/// decoded `expected_descriptor` with the exact canonical descriptor-v2 codec;
/// this loader checks only its fixed envelope and byte equality.
///
/// In addition, `callable_symbol` must name an eagerly resolvable function with
/// the exact C ABI
/// `unsafe extern "C" fn(*const u8, u32, *mut u8, u32) -> u32`. It must read
/// only the complete request range, write only the complete response range,
/// treat those ranges as disjoint, retain neither pointer, perform no delayed
/// symbol lookup or callbacks, and return synchronously without unwinding,
/// longjmp, trapping, terminating, or otherwise escaping the ABI. The root and
/// every selected dependency must preserve these properties and same-root
/// provenance for the entire lease lifetime. Native code cannot be made safe
/// by descriptor equality; this constructor is the explicit trusted boundary.
pub unsafe fn open_admitted_callable_exact(
    canonical_path: &Path,
    getter_symbol: &[u8],
    callable_symbol: &[u8],
    expected_descriptor: &[u8],
) -> Result<NativeCallableModuleLease, OpenError> {
    validate_inputs(canonical_path, getter_symbol, expected_descriptor)?;
    validate_callable_inputs(getter_symbol, callable_symbol, expected_descriptor)?;

    let resolved_path =
        std::fs::canonicalize(canonical_path).map_err(OpenError::PathCanonicalization)?;
    if resolved_path != canonical_path {
        return Err(OpenError::PathNotCanonical);
    }

    // SAFETY: The caller admits arbitrary initializer/terminator execution and
    // the complete stable dependency namespace. Unix resolution is RTLD_NOW.
    let library = unsafe { open_library(canonical_path) }.map_err(OpenError::LibraryOpen)?;

    type DescriptorGetter = unsafe extern "C" fn() -> *const u8;
    // SAFETY: The caller guarantees the getter's exact C ABI and immutable
    // bounded return. The copied pointer is invoked before the library moves.
    let getter: DescriptorGetter = *unsafe { library.get::<DescriptorGetter>(getter_symbol) }
        .map_err(OpenError::GetterLookup)?;
    // SAFETY: The caller guarantees the getter cannot unwind and its returned
    // bytes remain valid while this library is loaded.
    let descriptor_pointer = unsafe { getter() };
    if descriptor_pointer.is_null() {
        return Err(OpenError::NullDescriptor);
    }
    // SAFETY: The unsafe contract establishes this exact readable immutable
    // range for the complete library lifetime.
    let actual_descriptor =
        unsafe { std::slice::from_raw_parts(descriptor_pointer, expected_descriptor.len()) };
    if actual_descriptor != expected_descriptor {
        return Err(OpenError::DescriptorMismatch);
    }

    // SAFETY: The caller guarantees this exact symbol has CallableEntry's ABI.
    // Copying it ends the Symbol borrow before Library enters the Arc.
    let callable: CallableEntry = *unsafe { library.get::<CallableEntry>(callable_symbol) }
        .map_err(OpenError::CallableLookup)?;

    let instance_id = allocate_instance_id()?;
    Ok(NativeCallableModuleLease {
        inner: Arc::new(LoadedCallableModule {
            instance_id,
            canonical_path: resolved_path,
            descriptor_len: expected_descriptor.len(),
            callable,
            _library: library,
        }),
        _same_thread: PhantomData,
    })
}

#[cfg(unix)]
unsafe fn open_library(canonical_path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::unix::{Library as UnixLibrary, RTLD_LOCAL, RTLD_NOW};

    // SAFETY: This helper preserves the caller's complete trusted-image and
    // initializer/terminator contract while strengthening resolution to NOW.
    let library = unsafe { UnixLibrary::open(Some(canonical_path), RTLD_NOW | RTLD_LOCAL) }?;
    Ok(library.into())
}

#[cfg(windows)]
unsafe fn open_library(canonical_path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::windows::{
        Library as WindowsLibrary, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    };

    // SAFETY: The canonical path is absolute. DLL_LOAD_DIR admits dependencies
    // beside that exact root image, while DEFAULT_DIRS excludes the process
    // current directory and legacy PATH search. The caller still supplies the
    // complete trusted-image and initializer/terminator contract.
    let library = unsafe {
        WindowsLibrary::load_with_flags(
            canonical_path,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    }?;
    Ok(library.into())
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

fn validate_callable_inputs(
    getter_symbol: &[u8],
    callable_symbol: &[u8],
    expected_descriptor: &[u8],
) -> Result<(), OpenError> {
    if callable_symbol.is_empty()
        || callable_symbol.len() > MAX_CALLABLE_SYMBOL_BYTES
        || callable_symbol.contains(&0)
        || callable_symbol == getter_symbol
    {
        return Err(OpenError::InvalidCallableSymbol);
    }
    if !is_callable_descriptor_v2_envelope(expected_descriptor) {
        return Err(OpenError::InvalidCallableDescriptorSchema);
    }
    Ok(())
}

fn is_callable_descriptor_v2_envelope(bytes: &[u8]) -> bool {
    const HEADER_SIZE: usize = 20;
    if bytes.len() < HEADER_SIZE || bytes.get(..8) != Some(b"SPXNABI2") {
        return false;
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().expect("fixed version width"));
    let header_size = u32::from_le_bytes(bytes[12..16].try_into().expect("fixed header width"));
    let Ok(total_size) = usize::try_from(u32::from_le_bytes(
        bytes[16..20].try_into().expect("fixed length width"),
    )) else {
        return false;
    };
    version == 2 && header_size == 20 && total_size == bytes.len()
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
