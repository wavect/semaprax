//! Explicit boundary membership is independent of complete source-role custody.
//! The full predecessor helper remains byte-exact and independently compiled.
#[path = "new_boundary_module.rs"]
mod boundary;
#[path = "production_source_inventory_original.rs"]
mod original;

pub(super) fn production(raw: &str, manifest: &str) -> String {
    // Authenticate ALL raw bytes, sources, registrations and production/test
    // roles first. No outside-boundary source is relabelled or left unaudited.
    let complete_authenticated_production = original::production(raw, manifest);
    boundary::production(raw, manifest, &complete_authenticated_production)
}
