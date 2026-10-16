//! Overlay-only closed stdin receiver. No default module registration exists.
//! Compile the entire bound engine once per subject; never import a frozen
//! trait into the current compiler and call it an independent old interface.
use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectFrontendCache, ProjectFrontendSource, ProjectManifest, ProjectRevision,
    ProjectSemanticImage,
};
use crate::semantic_workspace::SemanticWorkspaceSource;
use crate::test_allocation_observer::{measure_with_capacity, Metrics};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::Arc;

const MAX_INPUT: usize = 32 * 1024 * 1024;
const POINTER_LIMIT: usize = 1 << 20; // excluded observation registry, not compiler allowance
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    path: String,
    source: String,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Semantic,
    Cache,
    ColdProject,
    WarmProject,
    ColdImage,
    WarmImage,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    mode: Mode,
    manifest: String,
    passes: Vec<Vec<Source>>,
}
impl Request {
    fn validate(&self) -> Result<(), &'static str> {
        let count = if matches!(self.mode, Mode::Cache) {
            3
        } else {
            1
        };
        if self.version != 1 || self.manifest.len() > 1024 * 1024 || self.passes.len() != count {
            return Err("closed request version/manifest/pass bound");
        }
        let mut total = 0usize;
        for pass in &self.passes {
            if pass.len() > 512 {
                return Err("source count bound");
            }
            for source in pass {
                if source.path.len() > 4096 {
                    return Err("path byte bound");
                }
                total = total
                    .checked_add(source.path.len())
                    .and_then(|n| n.checked_add(source.source.len()))
                    .ok_or("source arithmetic")?;
            }
        }
        if total > MAX_INPUT {
            return Err("aggregate source byte bound");
        }
        Ok(())
    }
}
fn metrics(m: Metrics) -> Value {
    json!({"allocations":m.allocations,"reallocations":m.reallocations,"allocated":m.allocated,"freed":m.freed,"live":m.live,"highwater":m.highwater,"invalid":m.invalid})
}
fn diagnostics(errors: &[Diagnostic]) -> Value {
    Value::Array(
        errors
            .iter()
            .map(|d| serde_json::from_str(&d.json()).expect("complete Diagnostic JSON"))
            .collect(),
    )
}
fn build(request: &Request, pass: usize) -> Result<ProjectRevision, Vec<Diagnostic>> {
    let manifest = ProjectManifest::parse(&request.manifest)?;
    let sources = request.passes[pass]
        .iter()
        .map(|s| SemanticWorkspaceSource::new(s.path.clone(), s.source.clone()))
        .collect();
    crate::project::rebuild_owned_revision(manifest, sources)
}
fn artifacts(revision: &Arc<ProjectRevision>) -> Value {
    let image = ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision());
    let image = match image {
        Ok(image) => json!({"json":image.to_json(),"digest":image.image_digest()}),
        Err(errors) => json!({"diagnostics":diagnostics(&errors)}),
    };
    json!({
        "project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),
        "graph":revision.semantic_graph(),"graph_digest":revision.semantic_graph_digest(),
        "entry_hir":format!("{:?}",revision.entry_program()),
        "public_api_hir":format!("{:?}",revision.public_api_program()),
        "test_hir":format!("{:?}",revision.test_program()),
        "sources":revision.sources().iter().map(|s|json!({"path":s.path(),"schema":s.source_graph_schema(),"revision":s.source_revision(),"digest":s.source_digest(),"source":s.source()})).collect::<Vec<_>>(),
        "image":image
    })
}
fn physical(request: &Request, image: bool) -> bool {
    match build(request, 0) {
        Ok(revision) => {
            let revision = Arc::new(revision);
            let accepted = if image {
                let result = ProjectSemanticImage::derive(
                    Arc::clone(&revision),
                    revision.project_revision(),
                );
                let accepted = result.is_ok();
                drop(result);
                accepted
            } else {
                true
            };
            drop(revision);
            accepted
        }
        Err(errors) => {
            drop(errors);
            false
        }
    }
}
fn execute(request: &Request) -> Value {
    match request.mode {
        Mode::Semantic => {
            // Serialization is a separately observed owner. This result is not
            // a through-Drop physical observation of the engine.
            let result = build(request, 0).map(Arc::new);
            let (wire, serializer) = measure_with_capacity(POINTER_LIMIT, || match &result {
                Ok(revision) => artifacts(revision),
                Err(errors) => json!({"diagnostics":diagnostics(errors)}),
            });
            drop(result);
            json!({"result":wire,"serializer_retained_output":metrics(serializer)})
        }
        Mode::Cache => {
            let manifest = match ProjectManifest::parse(&request.manifest) {
                Ok(m) => m,
                Err(errors) => return json!({"diagnostics":diagnostics(&errors)}),
            };
            let mut cache = ProjectFrontendCache::new_with_semantic_cache();
            let mut passes = Vec::new();
            for sources in &request.passes {
                let sources: Result<Vec<_>, _> = sources
                    .iter()
                    .map(|s| ProjectFrontendSource::new(&s.path, &s.source))
                    .collect();
                let result = sources.and_then(|s| cache.build(&manifest, &s));
                let (wire, serializer) = measure_with_capacity(POINTER_LIMIT, || match &result {
                    Ok(built) => {
                        json!({"work":built.to_json(),"artifacts":artifacts(built.revision())})
                    }
                    Err(errors) => json!({"diagnostics":diagnostics(errors)}),
                });
                passes
                    .push(json!({"result":wire,"serializer_retained_output":metrics(serializer)}));
                drop(result);
            }
            drop(cache);
            json!({"passes":passes})
        }
        Mode::ColdProject | Mode::WarmProject | Mode::ColdImage | Mode::WarmImage => {
            let image = matches!(request.mode, Mode::ColdImage | Mode::WarmImage);
            let warm = matches!(request.mode, Mode::WarmProject | Mode::WarmImage);
            let warmed = warm.then(|| physical(request, image));
            let (accepted, observed) =
                measure_with_capacity(POINTER_LIMIT, || physical(request, image));
            json!({"accepted":accepted,"warmup_accepted":warmed,"through_drop":metrics(observed),"scope":"manifest parse + cloned authored source inputs + complete Project build + optional image derive + all result Drop; request decode/output serializer excluded; process caches retained as observed"})
        }
    }
}
#[test]
#[ignore = "explicitly bound overlay-only closed stdin protocol"]
fn closed_stdio() {
    let subject = option_env!("SEMAPRAX_847_ENGINE_SUBJECT")
        .expect("compile with explicit engine subject label");
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= MAX_INPUT, "bounded protocol input");
    let (request, decode) =
        measure_with_capacity(POINTER_LIMIT, || serde_json::from_slice::<Request>(&bytes));
    let request = request.expect("closed typed JSON request");
    request.validate().expect("bounded closed request");
    let mode = match request.mode {
        Mode::Semantic => "semantic",
        Mode::Cache => "cache",
        Mode::ColdProject => "cold-project",
        Mode::WarmProject => "warm-project",
        Mode::ColdImage => "cold-image",
        Mode::WarmImage => "warm-image",
    };
    let request_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let result = execute(&request);
    let (wire, encode) = measure_with_capacity(POINTER_LIMIT, || {
        serde_json::to_vec(&json!({"protocol":"semaprax.private-847-oracle.v1","subject":subject,"mode":mode,"request_sha256":request_sha256,"decode_retained_input":metrics(decode),"observation_pointer_limit":POINTER_LIMIT,"result":result})).unwrap()
    });
    let mut out = std::io::stdout().lock();
    write!(out, "\n@847 result {}\n", wire.len()).unwrap();
    out.write_all(&wire).unwrap();
    out.write_all(b"\n").unwrap();
    let encoded = serde_json::to_vec(&metrics(encode)).unwrap();
    write!(out, "@847 protocol-encode {}\n", encoded.len()).unwrap();
    out.write_all(&encoded).unwrap();
    out.write_all(b"\n").unwrap();
    out.flush().unwrap();
}
