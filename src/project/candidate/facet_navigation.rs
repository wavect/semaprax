//! Candidate-bound function facets over one ephemeral admitted semantic image.
//! Existing image collectors remain the only owners of HIR facet contents.

use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ImageFacet, ImageFacetOptions, ProjectSemanticImage, IMAGE_FACET_SCHEMA,
    IMAGE_FUNCTION_SUMMARY_SCHEMA,
};

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA: &str =
    "semaprax.project-candidate-function-summary.v1";
pub const PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA: &str =
    "semaprax.project-candidate-function-facet.v1";
pub const PROJECT_CANDIDATE_FUNCTION_FACET_ITEM_SCHEMA: &str =
    "semaprax.project-candidate-function-facet-item.v1";
pub const MAX_PROJECT_CANDIDATE_FUNCTION_SUMMARY_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_CANDIDATE_FUNCTION_FACET_BYTES: usize = 1024 * 1024;

const MAX_ITEMS: usize = 65_536;
const MAX_CURSOR_BYTES: usize = 128;
const NONCLAIMS: [&str; 5] = [
    "not_a_candidate_semantic_delta_or_behavioral_change",
    "only_functions_present_in_the_final_candidate_hir_are_selectable",
    "no_runtime_liveness_test_coverage_or_external_dynamic_callers",
    "no_persistent_derived_image_candidate_or_cursor_retention",
    "no_source_execution_or_publication_authority",
];

impl ProjectCandidate {
    /// Compact candidate-bound handles for the existing nine function facets.
    pub fn function_summary(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let image = self.facet_image()?;
        let mut value = parse(&image.function_summary(image.image_digest(), target)?)?;
        validate_summary(&value, &image, target)?;
        let facets = value["facets"]
            .as_array_mut()
            .ok_or_else(|| invalid("candidate function summary has no facet inventory"))?;
        for (row, facet) in facets.iter_mut().zip(ImageFacet::ALL) {
            row["handle"] = json!(candidate_handle(
                self.candidate_digest(),
                image.image_digest(),
                target,
                facet,
            ));
        }
        value["schema"] = json!(PROJECT_CANDIDATE_FUNCTION_SUMMARY_SCHEMA);
        value["candidate_revision"] = json!(self.candidate_digest());
        value["base_project_revision"] = json!(self.base.project_revision());
        value["workspace_revision"] = json!(self.revision.workspace_revision());
        value["project_graph_digest"] = json!(self.revision.semantic_graph_digest());
        value["candidate_retained"] = json!(false);
        value["execution"] = json!(false);
        value["publication_authority"] = json!(false);
        value["nonclaims"] = json!(NONCLAIMS);
        render(value, MAX_PROJECT_CANDIDATE_FUNCTION_SUMMARY_BYTES)
    }

    /// Expand one candidate-bound facet while retaining the image collector's
    /// exact item order and heterogeneous compiler-owned item values.
    #[allow(clippy::too_many_arguments)]
    pub fn expand_function_facet(
        &self,
        expected_candidate: &str,
        target: &str,
        facet: ImageFacet,
        expected_handle: &str,
        cursor: Option<&str>,
        options: ImageFacetOptions,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let image = self.facet_image()?;
        let summary = parse(&image.function_summary(image.image_digest(), target)?)?;
        validate_summary(&summary, &image, target)?;
        let image_handle = summary["facets"]
            .as_array()
            .and_then(|facets| {
                facets
                    .iter()
                    .find(|row| row["facet"].as_str() == Some(facet.name()))
            })
            .and_then(|row| row["handle"].as_str())
            .ok_or_else(|| invalid("candidate function facet has no image handle"))?;
        let actual_handle =
            candidate_handle(self.candidate_digest(), image.image_digest(), target, facet);
        if expected_handle.len() != 71 || expected_handle != actual_handle {
            return Err(reference(
                "candidate function facet handle does not match its candidate, image, target and facet",
            ));
        }
        let offset = cursor
            .map(|cursor| parse_cursor(cursor, &actual_handle, options))
            .transpose()?
            .unwrap_or(0);
        let image_cursor = cursor.map(|_| image_cursor(offset, image_handle, options));
        let mut value = parse(&image.expand_facet(
            image.image_digest(),
            target,
            facet,
            image_handle,
            image_cursor.as_deref(),
            options,
        )?)?;
        validate_page(
            &value,
            &image,
            target,
            facet,
            image_handle,
            offset,
            cursor,
            options,
        )?;
        let items = value["items"]
            .as_array()
            .ok_or_else(|| invalid("candidate function facet items are absent"))?;
        let end = offset
            .checked_add(items.len())
            .ok_or_else(|| capacity("candidate function facet page offset overflow"))?;
        let wrapped_items = items
            .iter()
            .map(|item| {
                json!({
                    "schema": PROJECT_CANDIDATE_FUNCTION_FACET_ITEM_SCHEMA,
                    "value": item,
                })
            })
            .collect::<Vec<_>>();
        let next_cursor = if value["next_cursor"].is_null() {
            Value::Null
        } else {
            json!(candidate_cursor(end, &actual_handle, options))
        };
        value["schema"] = json!(PROJECT_CANDIDATE_FUNCTION_FACET_SCHEMA);
        value["candidate_revision"] = json!(self.candidate_digest());
        value["base_project_revision"] = json!(self.base.project_revision());
        value["workspace_revision"] = json!(self.revision.workspace_revision());
        value["project_graph_digest"] = json!(self.revision.semantic_graph_digest());
        value["handle"] = json!(actual_handle);
        value["cursor"] = json!(cursor);
        value["page_size"] = json!(options.page_size());
        value["max_bytes"] = json!(options.max_bytes());
        value["next_cursor"] = next_cursor;
        value["items"] = json!(wrapped_items);
        value["source_authority"] = json!(false);
        value["target_execution"] = json!(false);
        value["candidate_retained"] = json!(false);
        value["execution"] = json!(false);
        value["publication_authority"] = json!(false);
        value["nonclaims"] = json!(NONCLAIMS);
        render(value, options.max_bytes())
    }

    fn facet_image(&self) -> Result<ProjectSemanticImage> {
        ProjectSemanticImage::derive(Arc::clone(&self.revision), self.revision.project_revision())
    }
}

fn validate_summary(value: &Value, image: &ProjectSemanticImage, target: &str) -> Result<()> {
    if value.as_object().map_or(0, |object| object.len()) != 18
        || value["schema"] != IMAGE_FUNCTION_SUMMARY_SCHEMA
        || value["image_revision"] != image.image_digest()
        || value["project_revision"] != image.revision().project_revision()
        || value["id"] != target
        || value["evidence_class"] != "descriptive_projection_of_validated_hir"
        || value["source_authority"] != false
        || value["target_execution"] != false
    {
        return Err(invalid(
            "candidate function summary has unexpected image bindings",
        ));
    }
    let facets = value["facets"]
        .as_array()
        .filter(|facets| facets.len() == ImageFacet::ALL.len())
        .ok_or_else(|| invalid("candidate function summary facet inventory is not canonical"))?;
    for (row, facet) in facets.iter().zip(ImageFacet::ALL) {
        if row.as_object().map_or(0, |object| object.len()) != 2
            || row["facet"] != facet.name()
            || row["handle"] != image_handle(image.image_digest(), target, facet)
        {
            return Err(invalid(
                "candidate function summary facet binding is invalid",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_page(
    value: &Value,
    image: &ProjectSemanticImage,
    target: &str,
    facet: ImageFacet,
    image_handle: &str,
    offset: usize,
    cursor: Option<&str>,
    options: ImageFacetOptions,
) -> Result<()> {
    let items = value["items"]
        .as_array()
        .filter(|items| items.len() <= options.page_size())
        .ok_or_else(|| invalid("candidate function facet page items are invalid"))?;
    let total = value["total_items"]
        .as_u64()
        .and_then(|total| usize::try_from(total).ok())
        .filter(|total| *total <= MAX_ITEMS)
        .ok_or_else(|| invalid("candidate function facet total is invalid"))?;
    let end = offset
        .checked_add(items.len())
        .ok_or_else(|| capacity("candidate function facet page offset overflow"))?;
    if end > total {
        return Err(invalid(
            "candidate function facet page exceeds its selected inventory",
        ));
    }
    let expected_next = (end < total).then(|| image_cursor(end, image_handle, options));
    if value.as_object().map_or(0, |object| object.len()) != 14
        || value["schema"] != IMAGE_FACET_SCHEMA
        || value["image_revision"] != image.image_digest()
        || value["project_revision"] != image.revision().project_revision()
        || value["target"] != target
        || value["facet"] != facet.name()
        || value["handle"] != image_handle
        || value["offset"] != offset
        || value["next_cursor"] != json!(expected_next)
        || value["evidence_class"] != "descriptive_projection_of_validated_hir"
        || cursor.is_some() && offset == 0
    {
        return Err(invalid(
            "candidate function facet page has unexpected image bindings",
        ));
    }
    Ok(())
}

fn parse(report: &str) -> Result<Value> {
    serde_json::from_str(report)
        .map_err(|_| invalid("candidate function facet compiler report is not JSON"))
}

fn candidate_handle(candidate: &str, image: &str, target: &str, facet: ImageFacet) -> String {
    framed_digest(
        b"semaprax.project-candidate-function-facet-handle.v1\0",
        &[candidate, image, target, facet.name()],
    )
}

fn image_handle(image: &str, target: &str, facet: ImageFacet) -> String {
    framed_digest(
        b"semaprax.image-facet-handle.v1\0",
        &[image, target, facet.name()],
    )
}

fn candidate_cursor(offset: usize, handle: &str, options: ImageFacetOptions) -> String {
    let offset = offset.to_string();
    format!(
        "{offset}:{}",
        framed_digest(
            b"semaprax.project-candidate-function-facet-cursor.v1\0",
            &[handle, &offset, &options.page_size().to_string(),],
        )
    )
}

fn image_cursor(offset: usize, handle: &str, options: ImageFacetOptions) -> String {
    let offset = offset.to_string();
    format!(
        "{offset}:{}",
        framed_digest(
            b"semaprax.image-facet-cursor.v1\0",
            &[handle, &offset, &options.page_size().to_string()],
        )
    )
}

fn parse_cursor(cursor: &str, handle: &str, options: ImageFacetOptions) -> Result<usize> {
    if cursor.len() > MAX_CURSOR_BYTES {
        return Err(reference(
            "candidate function facet cursor exceeds its bound",
        ));
    }
    let (number, _) = cursor
        .split_once(':')
        .ok_or_else(|| reference("candidate function facet cursor is malformed"))?;
    let offset = number
        .parse::<usize>()
        .map_err(|_| reference("candidate function facet cursor offset is invalid"))?;
    if offset == 0
        || offset > MAX_ITEMS
        || offset % options.page_size() != 0
        || offset.to_string() != number
        || candidate_cursor(offset, handle, options) != cursor
    {
        return Err(reference(
            "candidate function facet cursor does not match its handle and options",
        ));
    }
    Ok(offset)
}

fn framed_digest(domain: &[u8], values: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for value in values {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn render(value: Value, max_bytes: usize) -> Result<String> {
    super::super::image::render(value, true, max_bytes)
        .map_err(|_| capacity("candidate function facet output exceeds its byte bound"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G358", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G359", message)]
}

fn reference(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G360", message)]
}
