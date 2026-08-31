//! Compact dependency navigation over one already admitted candidate revision.
//! The derived image and translated references are ephemeral evidence only.

use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project::{ImageDependencyPageOptions, ImageDependencyView, ProjectSemanticImage};

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA: &str =
    "semaprax.project-candidate-dependency-summary.v1";
pub const PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA: &str =
    "semaprax.project-candidate-dependency-page.v1";
pub const MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_CANDIDATE_DEPENDENCY_PAGE_BYTES: usize = 1024 * 1024;

impl ProjectCandidate {
    /// Summarize dependencies from the complete checked candidate revision.
    /// This creates no retained image, candidate, cursor, or source authority.
    pub fn dependency_summary(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let image = self.dependency_image()?;
        let mut value = parse_report(&image.dependency_summary(image.image_digest(), target)?)?;
        let facets = value["facets"]
            .as_array_mut()
            .ok_or_else(|| invalid("candidate dependency summary has no facet inventory"))?;
        for facet in facets {
            let view = facet["view"]
                .as_str()
                .ok_or_else(|| invalid("candidate dependency facet has no view"))?;
            let view = ImageDependencyView::parse(view)?;
            facet["handle"] = json!(candidate_handle(self.candidate_digest(), target, view));
        }
        value["schema"] = json!(PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA);
        value["candidate_revision"] = json!(self.candidate_digest());
        value["base_project_revision"] = json!(self.base.project_revision());
        value["candidate_retained"] = json!(false);
        value["execution"] = json!(false);
        value["publication_authority"] = json!(false);
        render(value, MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES)
    }

    /// Expand a candidate-bound dependency view while preserving the image
    /// collector's exact inventory and ordering.
    #[allow(clippy::too_many_arguments)]
    pub fn dependency_page(
        &self,
        expected_candidate: &str,
        target: &str,
        view: ImageDependencyView,
        expected_handle: &str,
        cursor: Option<&str>,
        options: ImageDependencyPageOptions,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let image = self.dependency_image()?;
        let summary = parse_report(&image.dependency_summary(image.image_digest(), target)?)?;
        let image_handle = summary["facets"]
            .as_array()
            .and_then(|facets| facets.iter().find(|facet| facet["view"] == view.name()))
            .and_then(|facet| facet["handle"].as_str())
            .ok_or_else(|| invalid("candidate dependency view has no image handle"))?;
        let actual_handle = candidate_handle(self.candidate_digest(), target, view);
        if expected_handle.len() != 71 || expected_handle != actual_handle {
            return Err(reference(
                "candidate dependency handle does not match its candidate, target and view",
            ));
        }
        let offset = cursor
            .map(|cursor| parse_cursor(cursor, &actual_handle, options))
            .transpose()?;
        let image_cursor = offset.map(|offset| make_image_cursor(offset, image_handle, options));
        let mut value = parse_report(&image.dependency_page(
            image.image_digest(),
            target,
            view,
            image_handle,
            image_cursor.as_deref(),
            options,
        )?)?;
        let next_cursor = if value["next_cursor"].is_null() {
            Value::Null
        } else {
            let offset = value["offset"]
                .as_u64()
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(|| invalid("candidate dependency page offset is invalid"))?;
            let count = value["items"]
                .as_array()
                .ok_or_else(|| invalid("candidate dependency page items are invalid"))?
                .len();
            let next = offset
                .checked_add(count)
                .ok_or_else(|| capacity("candidate dependency cursor offset overflow"))?;
            json!(make_cursor(next, &actual_handle, options))
        };
        value["schema"] = json!(PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA);
        value["candidate_revision"] = json!(self.candidate_digest());
        value["base_project_revision"] = json!(self.base.project_revision());
        value["candidate_retained"] = json!(false);
        value["execution"] = json!(false);
        value["publication_authority"] = json!(false);
        value["handle"] = json!(actual_handle);
        value["cursor"] = json!(cursor);
        value["next_cursor"] = next_cursor;
        render(value, options.max_bytes())
    }

    fn dependency_image(&self) -> Result<ProjectSemanticImage> {
        ProjectSemanticImage::derive(Arc::clone(&self.revision), self.revision.project_revision())
    }
}

fn parse_report(report: &str) -> Result<Value> {
    serde_json::from_str(report)
        .map_err(|_| invalid("candidate dependency compiler report is invalid"))
}

fn candidate_handle(candidate: &str, target: &str, view: ImageDependencyView) -> String {
    framed_digest(
        b"semaprax.project-candidate-dependency-handle.v1\0",
        &[candidate, target, view.name()],
    )
}

fn make_cursor(offset: usize, handle: &str, options: ImageDependencyPageOptions) -> String {
    let offset = offset.to_string();
    let digest = framed_digest(
        b"semaprax.project-candidate-dependency-cursor.v1\0",
        &[
            handle,
            &offset,
            &options.page_size().to_string(),
            &options.max_bytes().to_string(),
        ],
    );
    format!("{offset}:{digest}")
}

fn parse_cursor(cursor: &str, handle: &str, options: ImageDependencyPageOptions) -> Result<usize> {
    if cursor.len() > 128 {
        return Err(reference("candidate dependency cursor exceeds its bound"));
    }
    let (number, _) = cursor
        .split_once(':')
        .ok_or_else(|| reference("candidate dependency cursor is malformed"))?;
    let offset = number
        .parse::<usize>()
        .map_err(|_| reference("candidate dependency cursor offset is invalid"))?;
    if offset == 0
        || offset > 65_536
        || offset % options.page_size() != 0
        || offset.to_string() != number
        || make_cursor(offset, handle, options) != cursor
    {
        return Err(reference(
            "candidate dependency cursor does not match its handle and options",
        ));
    }
    Ok(offset)
}

fn make_image_cursor(
    offset: usize,
    image_handle: &str,
    options: ImageDependencyPageOptions,
) -> String {
    let offset = offset.to_string();
    let digest = framed_digest(
        b"semaprax.image-dependency-cursor.v1\0",
        &[
            image_handle,
            &offset,
            &options.page_size().to_string(),
            &options.max_bytes().to_string(),
        ],
    );
    format!("{offset}:{digest}")
}

fn framed_digest(domain: &[u8], values: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for value in values {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn render(value: Value, max_bytes: usize) -> Result<String> {
    super::super::image::render(value, true, max_bytes)
        .map_err(|_| capacity("candidate dependency navigation output exceeds its byte bound"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G320", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G323", message)]
}

fn reference(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G324", message)]
}
