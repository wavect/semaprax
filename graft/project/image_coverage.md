# project/image_coverage.rs

- IMAGE_ANALYSIS_COVERAGE_SCHEMA · constant · L9-L9 — pub const IMAGE_ANALYSIS_COVERAGE_SCHEMA: &str = "semaprax.image-analysis-coverage.v1";
- MAX_IMAGE_ANALYSIS_COVERAGE_BYTES · constant · L10-L10 — pub const MAX_IMAGE_ANALYSIS_COVERAGE_BYTES: usize = 1024 * 1024;
- MAX_FACTS · constant · L11-L11 — const MAX_FACTS: usize = 65_536;
- MAX_SOURCES · constant · L12-L12 — const MAX_SOURCES: usize = 16;
- analysis_coverage · function · L19-L176 — pub fn analysis_coverage(&self, expected_image: &str) -> Result<String, Vec<Diagnostic>>
- blind_spots · function · L182-L203 — fn blind_spots(project_revision: &str) -> Vec<Value>
- blind_spot · function · L205-L221 — fn blind_spot(
- areas · function · L223-L313 — fn areas(has_imports: bool) -> Vec<Value>
- area · function · L315-L323 — fn area(
- Budget · struct · L325-L328 — struct Budget
- count · function · L330-L336 — fn count(&mut self, count: usize) -> Result<(), Vec<Diagnostic>>
- reserve · function · L337-L349 — fn reserve(&mut self, text_bytes: usize, fixed_bytes: usize) -> Result<(), Vec<Diagnostic>>
- invalid · function · L351-L353 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- limit · function · L354-L356 — fn limit(message: &'static str) -> Vec<Diagnostic>
