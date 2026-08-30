//! Compact, revision-bound navigation over the shared immutable dependency index.
use super::*;
use sha2::{Digest, Sha256};

pub const IMAGE_DEPENDENCY_SUMMARY_SCHEMA: &str = "semaprax.image-dependency-summary.v1";
pub const IMAGE_DEPENDENCY_PAGE_SCHEMA: &str = "semaprax.image-dependency-page.v1";
const MAX_SUMMARY_BYTES: usize = 64 * 1024;
const MAX_PAGE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageDependencyView {
    Sites,
    Callers,
    Calls,
    Members,
}
impl ImageDependencyView {
    pub const ALL: [Self; 4] = [Self::Sites, Self::Callers, Self::Calls, Self::Members];
    pub fn name(self) -> &'static str {
        match self {
            Self::Sites => "sites",
            Self::Callers => "callers",
            Self::Calls => "calls",
            Self::Members => "members",
        }
    }
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "sites" => Ok(Self::Sites),
            "callers" => Ok(Self::Callers),
            "calls" => Ok(Self::Calls),
            "members" => Ok(Self::Members),
            _ => Err(grammar("dependency view is unsupported")),
        }
    }
    fn count(self, selection: &DependencySelection) -> usize {
        match self {
            Self::Sites => selection.ordinals.len(),
            Self::Callers => selection.closure.len(),
            Self::Calls => selection.calls.len(),
            Self::Members => selection.selected.len(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageDependencyPageOptions {
    page_size: usize,
    max_bytes: usize,
}
impl ImageDependencyPageOptions {
    pub fn new(page_size: usize, max_bytes: usize) -> Result<Self> {
        if !(1..=128).contains(&page_size) || !(1024..=MAX_PAGE_BYTES).contains(&max_bytes) {
            return Err(grammar(
                "dependency page options require 1..128 items and 1024..1048576 bytes",
            ));
        }
        Ok(Self {
            page_size,
            max_bytes,
        })
    }
    pub fn page_size(self) -> usize {
        self.page_size
    }
    pub fn max_bytes(self) -> usize {
        self.max_bytes
    }
}
impl Default for ImageDependencyPageOptions {
    fn default() -> Self {
        Self {
            page_size: 32,
            max_bytes: 65536,
        }
    }
}

impl ProjectSemanticImage {
    /// Compact inventories and opaque handles; no complete site payloads are
    /// cloned merely to count them. Test relevance is not coverage evidence.
    pub fn dependency_summary(&self, expected_image: &str, target: &str) -> Result<String> {
        let (declaration, source) = authenticate(self, expected_image, target)?;
        let index = self.dependency_index()?;
        let selection = index.selection(target, true)?;
        let typed = index.typed.get(target);
        let name = declaration
            .get("name")
            .filter(|name| name.is_string())
            .or_else(|| typed.and_then(|typed| typed.get("name")))
            .or_else(|| {
                typed
                    .and_then(|typed| typed.get("field"))
                    .and_then(|field| field.get("name"))
            })
            .cloned()
            .unwrap_or(Value::Null);
        let facets=ImageDependencyView::ALL.iter().map(|view|json!({"view":view.name(),"handle":handle(self.image_digest(),target,*view),"total_items":view.count(&selection)})).collect::<Vec<_>>();
        let revision = self.revision();
        let test_root = revision.test_program().entrypoint.as_str();
        encode(
            json!({"schema":IMAGE_DEPENDENCY_SUMMARY_SCHEMA,"image_digest":self.image_digest(),
            "project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),
            "target":target,"name":name,"kind":declaration["kind"],"source_binding":source,
            "facets":facets,"declared_test_root":test_root,"test_reachable":selection.closure.contains(test_root),
            "source_authority":false,"evidence_owner":"retained_checked_hir",
            "nonclaims":["not_test_coverage","no_execution_or_source_authority","no_runtime_liveness_or_path_feasibility","no_external_or_dynamic_callers","materialized_generic_instances_not_rescanned"]}),
            MAX_SUMMARY_BYTES,
        )
    }

    /// Expand only the requested page. A cursor authenticates its exact image,
    /// target, view, offset and options; it does not grant any host authority.
    pub fn dependency_page(
        &self,
        expected_image: &str,
        target: &str,
        view: ImageDependencyView,
        expected_handle: &str,
        cursor: Option<&str>,
        options: ImageDependencyPageOptions,
    ) -> Result<String> {
        let (_, source) = authenticate(self, expected_image, target)?;
        let actual_handle = handle(self.image_digest(), target, view);
        if expected_handle.len() != 71 || expected_handle != actual_handle {
            return Err(reference(
                "dependency handle does not match its image, target and view",
            ));
        }
        let offset = cursor
            .map(|cursor| parse_cursor(cursor, &actual_handle, options))
            .transpose()?
            .unwrap_or(0);
        let index = self.dependency_index()?;
        let selection = index.selection(target, true)?;
        let total = view.count(&selection);
        if cursor.is_some() && offset >= total {
            return Err(reference(
                "dependency cursor is outside its selected inventory",
            ));
        }
        let end = offset.saturating_add(options.page_size).min(total);
        let items=match view {
            ImageDependencyView::Sites=>selection.ordinals.iter().skip(offset).take(options.page_size).map(|ordinal|Ok(index.rows[*ordinal].clone())).collect::<Result<Vec<_>>>()?,
            ImageDependencyView::Calls=>selection.calls.iter().skip(offset).take(options.page_size).map(|ordinal|Ok(index.call_sites[*ordinal].clone())).collect::<Result<Vec<_>>>()?,
            ImageDependencyView::Callers=>selection.closure.iter().skip(offset).take(options.page_size).map(|id| {
                let declaration=self.revision().semantic.image_symbol(id).ok_or_else(||invalid("dependency caller declaration is absent"))?;
                let source=source_binding(self.revision(),&declaration)?;
                let reason=if id==target {"target"} else if selection.users.contains(id) {"direct_site_user"} else {"reverse_direct_caller"};
                Ok(json!({"id":id,"source_binding":source,"reason":reason,"evidence_owner":"retained_checked_hir"}))
            }).collect::<Result<Vec<_>>>()?,
            ImageDependencyView::Members=>selection.selected.iter().skip(offset).take(options.page_size).map(|id| {
                let declaration=self.revision().semantic.image_symbol(id).ok_or_else(||invalid("dependency member declaration is absent"))?;
                Ok(json!({"id":id,"declaration":declaration}))
            }).collect::<Result<Vec<_>>>()?,
        };
        let next_cursor = (end < total).then(|| make_cursor(end, &actual_handle, options));
        let revision = self.revision();
        encode(
            json!({"schema":IMAGE_DEPENDENCY_PAGE_SCHEMA,"image_digest":self.image_digest(),
            "project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),
            "target":target,"source_binding":source,"view":view.name(),"handle":actual_handle,"cursor":cursor,
            "offset":offset,"total_items":total,"page_size":options.page_size,"max_bytes":options.max_bytes,
            "next_cursor":next_cursor,"items":items,"source_authority":false,"evidence_owner":"retained_checked_hir"}),
            options.max_bytes,
        )
    }
}

fn authenticate(
    image: &ProjectSemanticImage,
    expected: &str,
    target: &str,
) -> Result<(Value, Value)> {
    image.require_digest(expected)?;
    if target.is_empty() || target.len() > 4096 || target.contains('\0') {
        return Err(invalid(
            "dependency target must be a bounded stable identity",
        ));
    }
    let declaration = image
        .revision()
        .semantic
        .image_symbol(target)
        .ok_or_else(|| {
            invalid("dependency target is absent from the retained declaration index")
        })?;
    let source = source_binding(image.revision(), &declaration)?;
    Ok((declaration, source))
}
fn source_binding(revision: &ProjectRevision, declaration: &Value) -> Result<Value> {
    let path = declaration["path"]
        .as_str()
        .ok_or_else(|| invalid("dependency target has no source owner"))?;
    let module = declaration["module"]
        .as_str()
        .ok_or_else(|| invalid("dependency target has no source module"))?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .ok_or_else(|| invalid("dependency source binding is absent"))?;
    Ok(
        json!({"path":path,"module":module,"source_revision":source.source_revision(),"source_digest":source.source_digest()}),
    )
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
fn handle(image: &str, target: &str, view: ImageDependencyView) -> String {
    framed_digest(
        b"semaprax.image-dependency-handle.v1\0",
        &[image, target, view.name()],
    )
}
fn make_cursor(offset: usize, handle: &str, options: ImageDependencyPageOptions) -> String {
    let offset = offset.to_string();
    let digest = framed_digest(
        b"semaprax.image-dependency-cursor.v1\0",
        &[
            handle,
            &offset,
            &options.page_size.to_string(),
            &options.max_bytes.to_string(),
        ],
    );
    format!("{offset}:{digest}")
}
fn parse_cursor(cursor: &str, handle: &str, options: ImageDependencyPageOptions) -> Result<usize> {
    if cursor.len() > 128 {
        return Err(reference("dependency cursor exceeds its bound"));
    }
    let (number, _) = cursor
        .split_once(':')
        .ok_or_else(|| reference("dependency cursor is malformed"))?;
    let offset = number
        .parse::<usize>()
        .map_err(|_| reference("dependency cursor offset is invalid"))?;
    if offset == 0
        || offset > MAX_ITEMS
        || offset % options.page_size != 0
        || offset.to_string() != number
        || make_cursor(offset, handle, options) != cursor
    {
        return Err(reference(
            "dependency cursor does not match its canonical offset, handle and options",
        ));
    }
    Ok(offset)
}
fn encode(value: Value, max_bytes: usize) -> Result<String> {
    super::super::image::render(value, true, max_bytes)
        .map_err(|_| limit("dependency navigation output exceeds its byte bound"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G322", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G323", message)]
}
fn reference(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G324", message)]
}
