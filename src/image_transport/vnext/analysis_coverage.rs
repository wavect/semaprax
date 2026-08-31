//! Retained analysis boundaries: a pure image report, never external discovery.
use super::*;
use crate::project::{IMAGE_ANALYSIS_COVERAGE_SCHEMA, MAX_IMAGE_ANALYSIS_COVERAGE_BYTES};

const METHOD: Method = Method {
    name: "image/analysis-coverage",
    operation: Operation::VNext(Action::AnalysisCoverage),
    parameters: &[REVISION],
    query: true,
    payload_schema: IMAGE_ANALYSIS_COVERAGE_SCHEMA,
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let report = image.analysis_coverage(text(params, "image_revision"))?;
    if report.len() > MAX_IMAGE_ANALYSIS_COVERAGE_BYTES {
        return Err(failure(
            "SPX-G220",
            "analysis coverage report exceeds its transport byte bound",
        ));
    }
    serde_json::from_str(&report)
        .map_err(|_| failure("SPX-G219", "analysis coverage report is not compiler JSON"))
}
