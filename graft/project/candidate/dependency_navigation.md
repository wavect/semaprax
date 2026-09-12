# project/candidate/dependency_navigation.rs

- Result · type · L14-L14 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA · constant · L16-L17 — pub const PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_SCHEMA: &str =
- PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA · constant · L18-L19 — pub const PROJECT_CANDIDATE_DEPENDENCY_PAGE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES · constant · L20-L20 — pub const MAX_PROJECT_CANDIDATE_DEPENDENCY_SUMMARY_BYTES: usize = 64 * 1024;
- MAX_PROJECT_CANDIDATE_DEPENDENCY_PAGE_BYTES · constant · L21-L21 — pub const MAX_PROJECT_CANDIDATE_DEPENDENCY_PAGE_BYTES: usize = 1024 * 1024;
- dependency_summary · function · L26-L47 — pub fn dependency_summary(&self, expected_candidate: &str, target: &str) -> Result<String>
- dependency_page · function · L52-L113 — pub fn dependency_page(
- dependency_image · function · L115-L117 — fn dependency_image(&self) -> Result<ProjectSemanticImage>
- parse_report · function · L120-L123 — fn parse_report(report: &str) -> Result<Value>
- candidate_handle · function · L125-L130 — fn candidate_handle(candidate: &str, target: &str, view: ImageDependencyView) -> String
- make_cursor · function · L132-L144 — fn make_cursor(offset: usize, handle: &str, options: ImageDependencyPageOptions) -> String
- parse_cursor · function · L146-L167 — fn parse_cursor(cursor: &str, handle: &str, options: ImageDependencyPageOptions) -> Result<usize>
- make_image_cursor · function · L169-L185 — fn make_image_cursor(
- framed_digest · function · L187-L198 — fn framed_digest(domain: &[u8], values: &[&str]) -> String
- render · function · L200-L203 — fn render(value: Value, max_bytes: usize) -> Result<String>
- invalid · function · L205-L207 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L209-L211 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- reference · function · L213-L215 — fn reference(message: &'static str) -> Vec<Diagnostic>
