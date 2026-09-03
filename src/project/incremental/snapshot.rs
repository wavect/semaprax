//! Private compiler-created snapshot carrier. Only the authenticated store may
//! decode this into a cache; source admission remains the ordinary build path.
use super::*;
use crate::cache_codec::{self, codec_struct};

struct Snapshot {
    context: String,
    project_revision: String,
    workspace_revision: String,
    graph_json: String,
    entries: Vec<Entry>,
}
struct Entry {
    path: String,
    source: String,
    synthetic: Program,
    resolved: crate::hir::ResolvedProgram,
    resolver_bytes: usize,
    retained_loan_bytes: usize,
}
codec_struct!(Snapshot {
    context,
    project_revision,
    workspace_revision,
    graph_json,
    entries
});
codec_struct!(Entry {
    path,
    source,
    synthetic,
    resolved,
    resolver_bytes,
    retained_loan_bytes
});

fn manifest(context: &str) -> Result<ProjectManifest> {
    let prefix = format!(
        "{}\0{}\0{}\0",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        PROJECT_SEMANTIC_CACHE_COMPATIBILITY
    );
    let bytes = context
        .strip_prefix(&prefix)
        .ok_or_else(|| invalid("semantic snapshot compiler context is incompatible"))?;
    if bytes.len() > super::super::MAX_MANIFEST_BYTES {
        return Err(capacity("semantic snapshot manifest exceeds its bound"));
    }
    let manifest = ProjectManifest::parse(bytes)?;
    if manifest.to_canonical_toml() != bytes {
        return Err(invalid("semantic snapshot manifest is not canonical"));
    }
    Ok(manifest)
}

pub(crate) fn encode_snapshot(cache: &ProjectFrontendCache) -> Result<Vec<u8>> {
    if !cache.semantic
        || !(2..=MAX_SOURCES).contains(&cache.entries.len())
        || cache.entries.keys().ne(cache.checked.keys())
    {
        return Err(invalid(
            "persistence requires a completely admitted semantic cache",
        ));
    }
    let manifest = manifest(&cache.context)?;
    let sources = cache
        .entries
        .iter()
        .map(|(path, entry)| ProjectFrontendSource::new(path, &entry.source))
        .collect::<Result<Vec<_>>>()?;
    let mut replay = cache.fork();
    let built = replay.build(&manifest, &sources)?;
    require_warm(&built)?;
    let entries = cache
        .entries
        .iter()
        .map(|(path, entry)| {
            let checked = &cache.checked[path];
            Entry {
                path: path.clone(),
                source: entry.source.clone(),
                synthetic: checked.synthetic.clone(),
                resolved: checked.resolved.clone(),
                resolver_bytes: checked.resolver_bytes,
                retained_loan_bytes: checked.retained_loan_bytes,
            }
        })
        .collect();
    cache_codec::encode(&Snapshot {
        context: cache.context.clone(),
        project_revision: built.revision().project_revision().to_owned(),
        workspace_revision: built.revision().workspace_revision().to_owned(),
        graph_json: built.revision().semantic_graph().to_owned(),
        entries,
    })
}

/// The caller must authenticate the complete bounded payload and executing
/// compiler binding before invoking this private constructor.
pub(crate) fn decode_snapshot(bytes: &[u8]) -> Result<ProjectFrontendCache> {
    let snapshot: Snapshot = cache_codec::decode(bytes)?;
    let manifest = manifest(&snapshot.context)?;
    if !(2..=MAX_SOURCES).contains(&snapshot.entries.len())
        || snapshot.entries.len() != manifest.sources().len()
        || snapshot.graph_json.len() > 16 * 1024 * 1024
    {
        return Err(capacity(
            "semantic snapshot inventory or graph exceeds its bound",
        ));
    }
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.context = snapshot.context;
    let mut source_bytes = 0usize;
    let mut sources = Vec::new();
    for (entry, expected_path) in snapshot.entries.into_iter().zip(manifest.sources()) {
        if &entry.path != expected_path || entry.path.len() > MAX_PATH_BYTES {
            return Err(invalid(
                "semantic snapshot source inventory disagrees with its manifest",
            ));
        }
        source_bytes = source_bytes
            .checked_add(entry.source.len())
            .ok_or_else(|| capacity("semantic snapshot source accounting overflow"))?;
        if source_bytes > MAX_TOTAL_SOURCE_BYTES
            || entry.resolver_bytes > MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND
            || entry.retained_loan_bytes > MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND
        {
            return Err(capacity(
                "semantic snapshot source or retained work exceeds its bound",
            ));
        }
        // Source is canonical authority. Reconstruct its AST independently;
        // the subsequent ordinary builder rederives the complete synthetic
        // input before permitting any decoded checked-module hit.
        let program = crate::parse(&entry.source, &entry.path).map_err(|error| vec![error])?;
        let (canonical, overflowed) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(&program)
            });
        if overflowed || canonical != entry.source {
            return Err(invalid("semantic snapshot source is not canonical"));
        }
        sources.push(ProjectFrontendSource::new(&entry.path, &entry.source)?);
        cache.entries.insert(
            entry.path.clone(),
            Arc::new(CachedModule {
                source: entry.source,
                program: Arc::new(program),
            }),
        );
        cache.checked.insert(
            entry.path,
            Arc::new(CheckedModule {
                synthetic: entry.synthetic,
                resolved: entry.resolved,
                resolver_bytes: entry.resolver_bytes,
                retained_loan_bytes: entry.retained_loan_bytes,
                // Persistent checked HIR remains reusable as a complete exact
                // module. Function work costs are invocation-local evidence;
                // they are rebuilt by the first changed-module resolution and
                // are never accepted from serialized cache bytes.
                function_costs: BTreeMap::new(),
            }),
        );
    }
    let built = cache.build(&manifest, &sources)?;
    require_warm(&built)?;
    if built.revision().project_revision() != snapshot.project_revision
        || built.revision().workspace_revision() != snapshot.workspace_revision
        || built.revision().semantic_graph() != snapshot.graph_json
    {
        return Err(invalid(
            "semantic snapshot full Project replay disagrees with stored projections",
        ));
    }
    cache.restore_work = Some(built.to_json().to_owned());
    Ok(cache)
}

fn require_warm(build: &ProjectFrontendBuild) -> Result<()> {
    let report = work_value(build)?;
    if report["work"]["modules_resolved"] != 0
        || report["work"]["checked_HIR_reused"].as_u64()
            != Some(build.revision().sources().len() as u64)
    {
        return Err(invalid(
            "semantic snapshot requires exact checked-module reuse during replay",
        ));
    }
    Ok(())
}
