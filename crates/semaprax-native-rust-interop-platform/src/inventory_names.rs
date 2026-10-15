//! Runtime names own their spelling; frozen static inventories allocate nothing new.
use super::*;
pub(super) enum InventoryNames<const N: usize> {
    Static([&'static OsStr; N]),
    Owned([std::ffi::OsString; N]),
}
impl<const N: usize> InventoryNames<N> {
    pub(super) fn get(&self, index: usize) -> &OsStr {
        match self {
            Self::Static(names) => names[index],
            Self::Owned(names) => &names[index],
        }
    }
    pub(super) fn owned_capacity(&self) -> usize {
        match self {
            Self::Static(_) => 0,
            Self::Owned(names) => names.iter().map(|name| name.capacity()).sum(),
        }
    }
}
/// Prepare explicit owned names before any file authority is acquired.
pub fn prepare_owned_discard_inventory<const N: usize>(
    names: [&OsStr; N],
) -> Result<PreparedDiscardInventory<N>, Error> {
    let native = semaprax_native_rust_interop_platform_sys::prepare_discard_names(names)?;
    Ok(PreparedDiscardInventory {
        names: InventoryNames::Owned(names.map(OsStr::to_os_string)),
        native,
        files: [const { None }; N],
        settled: [const { None }; N],
        attached: 0,
        #[cfg(debug_assertions)]
        failure_after_delete: None,
    })
}
