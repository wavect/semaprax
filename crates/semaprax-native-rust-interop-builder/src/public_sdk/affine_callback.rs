//! Inert projection of an authenticated source-owned affine capture.
use super::*;
use semaprax::ast::{Span, Type};

#[cfg(test)]
#[path = "affine_callback_tests.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeAffineCallbackProjection {
    pub source_revision: String,
    pub c_source: String,
    pub safe_rust: String,
}

/// Select a pure, zero-argument source factory returning the closed owned
/// `FnOnce() -> i64` profile. The generated Rust owner holds the actual native
/// carrier produced by that checked body, including its owned Bytes capture.
/// This function emits source only; it invokes no compiler and grants no host
/// authority. It does not admit FnMut, borrowed captures, or Send/Sync.
pub fn prepare_native_rust_affine_callback(
    source: &str,
    path: &Path,
    factory_id: &str,
) -> Result<NativeAffineCallbackProjection, Vec<Diagnostic>> {
    let fail = |message: &str| vec![Diagnostic::error("SPX-B154", message, Span::default())];
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("affine callback source exceeds its bound"));
    }
    let program = semaprax::check(source, path)?;
    let factory = program
        .functions
        .iter()
        .find(|f| f.stable_id == factory_id)
        .ok_or_else(|| fail("affine callback factory is absent"))?;
    if !factory.explicit_id
        || !factory.params.is_empty()
        || !factory.type_parameters.is_empty()
        || factory.return_type != Type::OnceFunction
        || !factory.effects.is_empty()
        || !program.interfaces.is_empty()
    {
        return Err(fail(
            "affine callback factory requires pure fn()->FnOnce()->i64 without foreign interfaces",
        ));
    }
    let resolved = semaprax::hir::resolve(&program)?;
    semaprax::hir::validate(&resolved).map_err(|e| vec![e])?;
    let canonical = semaprax::format::canonical(&program);
    let source_revision = domain_digest(
        b"semaprax.affine-callback-source.v1\0",
        canonical.as_bytes(),
    );
    let mut c_source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    c_source.push_str(&semaprax::codegen::emit_c(&program).map_err(|error| vec![error])?);
    let symbol = format!(
        "spx_decl_{}",
        factory_id
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    c_source.push_str(&C_BRIDGE.replace("FACTORY_SYMBOL", &symbol));
    Ok(NativeAffineCallbackProjection {
        source_revision,
        c_source,
        safe_rust: RUST_BRIDGE.into(),
    })
}

const C_BRIDGE: &str = r#"
struct spx_affine_owner {
    struct spx_context context;
    struct spx_status_entry entries[8];
    spx_once_v1 callback;
};
void *spx_affine_create(void) {
    struct spx_affine_owner *owner = calloc(1, sizeof *owner);
    if (!owner) return NULL;
    if (!spx_context_init(&owner->context, UINT64_C(366), owner->entries, UINT32_C(8), NULL, NULL, NULL)) {
        free(owner); return NULL;
    }
    spx_status_token status = FACTORY_SYMBOL(&owner->context, &owner->callback);
    if (status != SPX_STATUS_SUCCESS) { free(owner); return NULL; }
    return owner;
}
uint32_t spx_affine_invoke(void *opaque, int64_t *out) {
    struct spx_affine_owner *owner = opaque;
    spx_once_v1 callback = spx_once_move(&owner->callback);
    spx_status_token status = callback.entry(&owner->context, spx_bytes_move(&callback.capture), out);
    free(owner);
    return status == SPX_STATUS_SUCCESS ? UINT32_C(0) : UINT32_C(1);
}
void spx_affine_destroy(void *opaque) {
    struct spx_affine_owner *owner = opaque;
    spx_once_drop(&owner->callback);
    free(owner);
}
"#;

const RUST_BRIDGE: &str = r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffineCallbackError { Creation, Call, RegistrationClosed }
mod affine_ffi {
    use std::ffi::c_void;
    extern "C" {
        pub fn spx_affine_create() -> *mut c_void;
        pub fn spx_affine_invoke(owner: *mut c_void, out: *mut i64) -> u32;
        pub fn spx_affine_destroy(owner: *mut c_void);
    }
}
/// A unique owner of the source-created environment. It cannot be cloned,
/// sent across threads, or called twice. Dropping it settles an unused capture.
pub struct AffineCallback {
    owner: Option<std::ptr::NonNull<std::ffi::c_void>>,
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl AffineCallback {
    pub fn new() -> Result<Self, AffineCallbackError> {
        let owner = std::ptr::NonNull::new(unsafe { affine_ffi::spx_affine_create() })
            .ok_or(AffineCallbackError::Creation)?;
        Ok(Self { owner: Some(owner), thread: std::marker::PhantomData })
    }
    pub fn call(mut self) -> Result<i64, AffineCallbackError> {
        let owner = self.owner.take().expect("unique affine owner");
        let mut out = 0;
        let status = unsafe { affine_ffi::spx_affine_invoke(owner.as_ptr(), &mut out) };
        if status == 0 { Ok(out) } else { Err(AffineCallbackError::Call) }
    }
    pub fn into_fn_once(self) -> impl FnOnce() -> Result<i64, AffineCallbackError> {
        move || self.call()
    }
    /// Gives a same-thread foreign registry one opaque affine lease. The
    /// registry can invoke it once or unregister it; it cannot obtain the C
    /// owner or a second callable copy.
    pub fn retain(self) -> RetainedAffineCallback {
        RetainedAffineCallback { callback: Some(self), active: true }
    }
}
impl Drop for AffineCallback {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            unsafe { affine_ffi::spx_affine_destroy(owner.as_ptr()) }
        }
    }
}
/// A same-thread registration lease for a source-created affine callback.
/// `invoke` closes the lease before entering C, and `unregister` drops an
/// uncalled owner. Therefore a foreign registry can neither call after
/// teardown nor free an environment it still retains.
pub struct RetainedAffineCallback {
    callback: Option<AffineCallback>,
    active: bool,
}
impl RetainedAffineCallback {
    pub fn is_active(&self) -> bool { self.active }
    pub fn invoke(&mut self) -> Result<i64, AffineCallbackError> {
        if !self.active { return Err(AffineCallbackError::RegistrationClosed); }
        self.active = false;
        self.callback.take().expect("active affine lease has an owner").call()
    }
    pub fn unregister(&mut self) {
        self.active = false;
        drop(self.callback.take());
    }
}
impl Drop for RetainedAffineCallback {
    fn drop(&mut self) { self.unregister(); }
}
"#;
