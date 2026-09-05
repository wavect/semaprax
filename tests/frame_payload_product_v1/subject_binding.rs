//! Existing execution lanes over the shared held-Project artifact observations.
use super::*;

#[path = "../support/owned_frame_artifacts.rs"]
mod shared;
pub(super) use shared::{native_provider, retain, verify_display_rename};

pub(super) fn verify_product(project: &Path, npm: &Path, rust: &Path) -> shared::BoundProduct {
    let product = shared::verify_artifacts(project, npm, rust);
    // Preserve execution over the actual linked Project, including source origins.
    assert_interpreter_corpus(product.revision().public_api_program());
    let provider = native_provider(&product);
    assert_native_corpus(provider.source(), "bound-project-native");
    raw_wasm::run(npm, product.descriptor());
    product
}
