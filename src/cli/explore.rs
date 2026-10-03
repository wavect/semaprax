use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    self, ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerSide, ExplorerView,
    ProjectCandidate, ProjectSemanticImage,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
const MAX: usize = 16 * 1024 * 1024;
pub(crate) struct Options {
    pub manifest: PathBuf,
    pub output: PathBuf,
    pub html: bool,
    pub export_format: Option<super::explorer_export::Format>,
    pub target: Option<String>,
    pub depth: usize,
    pub candidate_capsule: Option<PathBuf>,
    pub expect_candidate: Option<String>,
    pub include_source: bool,
}
pub(crate) fn parse(args: &[String]) -> Result<Options, u8> {
    let Some(manifest) = args.first().filter(|v| !v.starts_with('-')) else {
        eprintln!("explore requires <manifest> --format html|json --output <path>");
        return Err(2);
    };
    let (mut format, mut output, mut target, mut depth) = (None, None, None, 1);
    let (mut candidate_capsule, mut expect_candidate, mut include_source) = (None, None, false);
    let mut seen = std::collections::HashSet::new();
    let mut i = 1;
    while i < args.len() {
        if !seen.insert(args[i].as_str()) {
            eprintln!("duplicate explore option `{}`", args[i]);
            return Err(2);
        }
        match args[i].as_str() {
            "--format" => {
                i += 1;
                format = args.get(i).cloned()
            }
            "--output" => {
                i += 1;
                output = args.get(i).map(PathBuf::from)
            }
            "--target" => {
                i += 1;
                target = args.get(i).cloned()
            }
            "--depth" => {
                i += 1;
                depth = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(usize::MAX)
            }
            "--candidate-capsule" => {
                i += 1;
                candidate_capsule = args.get(i).map(PathBuf::from);
            }
            "--expect-candidate" => {
                i += 1;
                expect_candidate = args.get(i).cloned();
            }
            "--include-source" => {
                include_source = true;
            }
            x => {
                eprintln!("unknown explore option `{x}`");
                return Err(2);
            }
        }
        i += 1
    }
    let (html, export_format) = match format.as_deref() {
        Some("html") => (true, None),
        Some("json") => (false, None),
        Some(value) if super::explorer_export::Format::parse(value).is_some() => {
            (false, super::explorer_export::Format::parse(value))
        }
        _ => {
            eprintln!("explore requires --format html|json|markdown|svg");
            return Err(2);
        }
    };
    let Some(output) = output else {
        eprintln!("explore requires --output");
        return Err(2);
    };
    if depth > 16 || (target.is_none() && depth != 1) {
        eprintln!("--depth requires --target and is at most 16");
        return Err(2);
    };
    if candidate_capsule.is_some() != expect_candidate.is_some()
        || expect_candidate.as_deref().is_some_and(|value| {
            !value.starts_with("sha256:")
                || value.len() != 71
                || !value[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
    {
        eprintln!(
            "--candidate-capsule and --expect-candidate require each other and a canonical digest"
        );
        return Err(2);
    }
    if include_source && export_format.is_some() {
        eprintln!("--include-source is supported only with --format html or json");
        return Err(2);
    }
    Ok(Options {
        manifest: manifest.into(),
        output,
        html,
        export_format,
        target,
        depth,
        candidate_capsule,
        expect_candidate,
        include_source,
    })
}
pub(crate) fn run(options: Options) -> Result<(), Vec<Diagnostic>> {
    reject_existing_output(&options.output)?;
    let bytes = project::with_authenticated_project(&options.manifest, |snapshot| {
        let candidate = if let (Some(path), Some(expected)) =
            (&options.candidate_capsule, &options.expect_candidate)
        {
            let bytes =
                super::project_candidate::read_capsule(path).map_err(|error| vec![error])?;
            let restored = ProjectCandidate::restore(
                snapshot.retain_revision(),
                snapshot.project_revision(),
                &bytes,
            )?;
            if restored.candidate_digest() != expected {
                return Err(invalid(
                    "restored candidate digest does not match --expect-candidate",
                ));
            }
            Some(restored)
        } else {
            None
        };
        let image = if candidate.is_none() {
            Some(ProjectSemanticImage::derive(
                snapshot.retain_revision(),
                snapshot.project_revision(),
            )?)
        } else {
            None
        };
        let catalog = if let Some(candidate) = &candidate {
            Some(
                serde_json::from_str::<Value>(
                    &candidate.semantic_delta_catalog(candidate.candidate_digest())?,
                )
                .map_err(|_| invalid("invalid candidate change catalog"))?,
            )
        } else {
            None
        };
        let focus_sides = focus_sides(options.target.as_deref(), catalog.as_ref())?;
        let source_files = if options.include_source {
            let mut files = Vec::new();
            if let Some(candidate) = &candidate {
                for (side, revision) in [
                    ("base", candidate.base_revision()),
                    ("candidate", candidate.revision()),
                ] {
                    for source in revision.sources() {
                        files.push(json!({
                            "side": side,
                            "path": source.path(),
                            "source_revision": source.source_revision(),
                            "source_digest": source.source_digest(),
                            "text": source.source(),
                        }));
                    }
                }
            } else {
                for source in snapshot.sources() {
                    files.push(json!({
                        "side": "current",
                        "path": source.path(),
                        "source_revision": source.source_revision(),
                        "source_digest": source.source_digest(),
                        "text": source.source(),
                    }));
                }
            }
            files
        } else {
            Vec::new()
        };
        let mut views = Vec::new();
        let sides: &[ExplorerSide] = if candidate.is_some() {
            &[ExplorerSide::Candidate, ExplorerSide::Base]
        } else {
            &[ExplorerSide::Current]
        };
        for &side in sides {
            for (mode, target, depth) in [
                (ExplorerMode::Overview, None, 1),
                (
                    ExplorerMode::Context,
                    options.target.as_deref(),
                    options.depth,
                ),
            ] {
                if mode == ExplorerMode::Context && target.is_none() {
                    continue;
                }
                if mode == ExplorerMode::Context
                    && !focus_sides.iter().any(|value| *value == side.name())
                {
                    continue;
                }
                let query = ExplorerQuery::new(
                    semaprax::project::ExplorerDirection::Both,
                    depth,
                    256,
                    256 * 1024,
                )?;
                let summary_text = if let Some(candidate) = &candidate {
                    candidate.explorer_summary(
                        candidate.candidate_digest(),
                        side,
                        mode,
                        target,
                        query,
                    )?
                } else {
                    let image = image.as_ref().unwrap();
                    image.explorer_summary(image.image_digest(), mode, target, query)?
                };
                let summary: Value = serde_json::from_str(&summary_text)
                    .map_err(|_| invalid("invalid explorer summary"))?;
                let mut pages = Vec::new();
                for row in summary["inventories"]
                    .as_array()
                    .ok_or_else(|| invalid("invalid explorer inventories"))?
                {
                    let view = ExplorerView::parse(
                        row["view"]
                            .as_str()
                            .ok_or_else(|| invalid("invalid explorer view"))?,
                    )?;
                    let handle = row["handle"]
                        .as_str()
                        .ok_or_else(|| invalid("invalid explorer handle"))?;
                    let mut cursor: Option<String> = None;
                    loop {
                        let page_text = if let Some(candidate) = &candidate {
                            candidate.explorer_page(
                                candidate.candidate_digest(),
                                side,
                                mode,
                                target,
                                query,
                                view,
                                handle,
                                cursor.as_deref(),
                                ExplorerPageOptions::default(),
                            )?
                        } else {
                            let image = image.as_ref().unwrap();
                            image.explorer_page(
                                image.image_digest(),
                                mode,
                                target,
                                query,
                                view,
                                handle,
                                cursor.as_deref(),
                                ExplorerPageOptions::default(),
                            )?
                        };
                        let page: Value = serde_json::from_str(&page_text)
                            .map_err(|_| invalid("invalid explorer page"))?;
                        cursor = page["next_cursor"].as_str().map(str::to_owned);
                        pages.push(page);
                        if pages.len() > 1024 {
                            return Err(invalid(
                                "explorer inventory exceeds page budget; select a smaller focus",
                            ));
                        }
                        if cursor.is_none() {
                            break;
                        }
                    }
                }
                views.push(json!({"query":{"mode":mode.name(),"target":target,"direction":"both","depth":depth,"side":side.name()},"summary":summary,"pages":pages}));
            }
        }
        let evidence = focused_evidence(
            options.target.as_deref(),
            &views,
            image.as_ref(),
            candidate.as_ref(),
        );
        let mut v = json!({"schema":"semaprax.explorer-snapshot.v1","generator":"semaprax explore","views":views,"focus":options.target,"focus_sides":focus_sides,"source_included":options.include_source,"evidence_availability":if options.target.is_some() { "available" } else { "not_requested" },"evidence":evidence,"confidentiality":"names_ids_and_paths_may_be_confidential"});
        if options.include_source {
            v["source_files"] = json!(source_files);
            if let Some(candidate) = &candidate {
                let report = candidate.source_review(candidate.candidate_digest())?;
                v["source_review"] = serde_json::from_str::<Value>(&report)
                    .map_err(|_| invalid("invalid authenticated candidate source review"))?;
            }
        }
        if let Some(catalog) = catalog {
            v["changes"] = json!({"catalog":catalog,"details":[]});
        }
        v.sort_all_objects();
        v["snapshot_digest"] = json!(snapshot_digest(&v));
        v.sort_all_objects();
        Ok(v.to_string().into_bytes())
    })?;
    if bytes.len() > MAX {
        return Err(vec![Diagnostic::io(
            "SPX-G328",
            "explorer snapshot exceeds 16MiB",
        )]);
    }
    let out = if let Some(format) = options.export_format {
        let snapshot: Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid("invalid explorer snapshot"))?;
        super::explorer_export::render(&snapshot, format).map_err(invalid)?
    } else if options.html {
        html(&bytes).into_bytes()
    } else {
        bytes
    };
    if out.len() > if options.html { 24 * 1024 * 1024 } else { MAX } {
        return Err(invalid(
            "explorer output exceeds its byte budget; select a smaller focus",
        ));
    }
    let parent = options
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut suffix = [0u8; 16];
    getrandom::fill(&mut suffix).map_err(|_| invalid("cannot create explorer temporary name"))?;
    let temp = parent.join(format!(
        ".semaprax-explore-{}-{}",
        std::process::id(),
        suffix
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| {
            vec![Diagnostic::io(
                "SPX-G328",
                "explorer output already exists or cannot be created",
            )]
        })?;
    let result = (|| {
        f.write_all(&out)
            .map_err(|_| invalid("cannot write explorer output"))?;
        f.sync_all()
            .map_err(|_| invalid("cannot sync explorer output"))?;
        fs::hard_link(&temp, &options.output)
            .map_err(|_| invalid("explorer output already exists or cannot be created"))?;
        Ok(())
    })();
    drop(f);
    let _ = fs::remove_file(&temp);
    result
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G328", message)]
}

fn focused_evidence(
    target: Option<&str>,
    views: &[Value],
    image: Option<&ProjectSemanticImage>,
    candidate: Option<&ProjectCandidate>,
) -> Value {
    let Some(target) = target.filter(|target| !target.is_empty()) else {
        return json!({"schema":"semaprax.explorer-evidence-index.v1","entries":[]});
    };
    let mut entries = Vec::new();
    for view in views
        .iter()
        .filter(|view| view["query"]["mode"] == "context")
    {
        let side = view["query"]["side"].as_str().unwrap_or("");
        let subject = view["summary"]["subject"].clone();
        if subject.is_null() {
            continue;
        }
        let is_function = view["pages"].as_array().is_some_and(|pages| {
            pages.iter().any(|page| {
                page["view"] == "declarations"
                    && page["items"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item["id"] == target && item["kind"] == "function")
                    })
            })
        });
        let candidate_side = side == "candidate";
        let base_image = if !candidate_side {
            candidate.filter(|_| side == "base").and_then(|candidate| {
                ProjectSemanticImage::derive(
                    std::sync::Arc::clone(candidate.base_revision()),
                    candidate.base_revision().project_revision(),
                )
                .ok()
            })
        } else {
            None
        };
        let function = if let Some(candidate) = candidate.filter(|_| candidate_side) {
            candidate
                .function_summary(candidate.candidate_digest(), target)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        } else {
            base_image
                .as_ref()
                .or(image)
                .and_then(|image| image.function_summary(image.image_digest(), target).ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        };
        let dependency = if let Some(candidate) = candidate.filter(|_| candidate_side) {
            candidate
                .dependency_summary(candidate.candidate_digest(), target)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        } else {
            base_image
                .as_ref()
                .or(image)
                .and_then(|image| image.dependency_summary(image.image_digest(), target).ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        };
        let coverage = if let Some(candidate) = candidate.filter(|_| candidate_side) {
            candidate
                .analysis_coverage(candidate.candidate_digest())
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        } else {
            base_image
                .as_ref()
                .or(image)
                .and_then(|image| image.analysis_coverage(image.image_digest()).ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        };
        let mut states = json!({
            "function_summary": if function.is_some() { "available" } else if !is_function { "not applicable" } else { "error" },
            "dependency_summary": if dependency.is_some() { "available" } else { "error" },
            "analysis_coverage": if coverage.is_some() { "available" } else { "error" },
        });
        let compact = json!({
            "function_summary": function.as_ref().map(compact_function),
            "dependency_summary": dependency.as_ref().map(compact_dependencies),
            "analysis_coverage": coverage.as_ref().map(compact_coverage),
        });
        let mut compact = compact;
        if candidate_side {
            let contract = candidate
                .and_then(|candidate| candidate.contract_delta(candidate.candidate_digest()).ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            let ownership = candidate
                .and_then(|candidate| candidate.ownership_delta(candidate.candidate_digest()).ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            states["contract_delta"] = json!(if contract.is_some() {
                "available"
            } else {
                "error"
            });
            states["ownership_delta"] = json!(if ownership.is_some() {
                "available"
            } else {
                "error"
            });
            compact["contract_delta"] = contract
                .as_ref()
                .map(|report| compact_delta(report, target, true))
                .unwrap_or(Value::Null);
            compact["ownership_delta"] = ownership
                .as_ref()
                .map(|report| compact_delta(report, target, false))
                .unwrap_or(Value::Null);
        }
        entries.push(json!({"subject":subject,"target":target,"states":states,"compact":compact,"omitted":["source bodies","raw facet items","spans","literal-bearing report fields"]}));
    }
    json!({"schema":"semaprax.explorer-evidence-index.v1","entries":entries})
}

fn compact_function(report: &Value) -> Value {
    pick(
        report,
        &[
            "schema",
            "id",
            "name",
            "path",
            "module",
            "source_revision",
            "parameter_count",
            "return_type_id",
            "effects",
            "requires_count",
            "ensures_count",
        ],
    )
}

fn compact_dependencies(report: &Value) -> Value {
    let mut value = pick(
        report,
        &[
            "schema",
            "target",
            "name",
            "kind",
            "declared_test_root",
            "test_reachable",
        ],
    );
    value["facets"] = Value::Array(
        report["facets"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| pick(row, &["view", "total_items"]))
            .collect(),
    );
    value
}

fn compact_coverage(report: &Value) -> Value {
    json!({"schema":report["schema"],"areas":report["areas"].as_array().into_iter().flatten().map(|row|pick(row,&["area","status"])).collect::<Vec<_>>()})
}

fn compact_delta(report: &Value, target: &str, contract: bool) -> Value {
    let inventory = &report["inventory"];
    let fields: &[&str] = if contract {
        &[
            "base_functions",
            "candidate_functions",
            "base_predicates",
            "candidate_predicates",
            "base_functions_with_contracts",
            "candidate_functions_with_contracts",
            "unchanged_functions",
            "affected_functions",
            "base_source_only_functions",
            "candidate_source_only_functions",
        ]
    } else {
        &[
            "base_functions",
            "candidate_functions",
            "base_instances",
            "candidate_instances",
            "unchanged_functions",
            "affected_functions",
            "base_types",
            "candidate_types",
            "unchanged_types",
            "affected_types",
        ]
    };
    let selected = report["functions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| row["id"] == target);
    let comparison_fields: &[&str] = if contract {
        &[
            "exact_equal",
            "predicate_projection_equal",
            "dependency_equal",
            "source_equal",
            "reasons",
        ]
    } else {
        &[
            "signature_equal",
            "cleanup_inventory_equal",
            "loan_plan_equal",
            "cleanup_plan_equal",
            "instances_equal",
            "source_equal",
            "exact_equal",
            "reasons",
        ]
    };
    json!({"schema":report["schema"],"inventory":pick(inventory,fields),"selected_target":selected.map(|row|json!({"change":row["change"],"comparison":pick(&row["comparison"],comparison_fields)})),"selected_target_state":if selected.is_some(){"changed_row"}else{"not_in_changed_inventory"}})
}

fn pick(value: &Value, fields: &[&str]) -> Value {
    let mut result = serde_json::Map::new();
    for field in fields {
        if let Some(value) = value.get(*field) {
            result.insert((*field).to_owned(), value.clone());
        }
    }
    Value::Object(result)
}

fn reject_existing_output(output: &Path) -> Result<(), Vec<Diagnostic>> {
    match fs::symlink_metadata(output) {
        Ok(_) => Err(invalid(
            "explorer output already exists or cannot be created",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(invalid(
            "explorer output already exists or cannot be created",
        )),
    }
}
fn focus_sides<'a>(
    target: Option<&str>,
    catalog: Option<&Value>,
) -> Result<Vec<&'a str>, Vec<Diagnostic>> {
    let Some(target) = target else {
        return Ok(Vec::new());
    };
    let Some(catalog) = catalog else {
        return Ok(vec!["current"]);
    };
    let roots = catalog["roots"]
        .as_array()
        .ok_or_else(|| invalid("invalid candidate change catalog"))?;
    let Some(root) = roots
        .iter()
        .find(|root| root["target"].as_str() == Some(target))
    else {
        return Ok(vec!["candidate", "base"]);
    };
    let mut sides = Vec::new();
    if root["candidate"].is_object() {
        sides.push("candidate");
    }
    if root["base"].is_object() {
        sides.push("base");
    }
    if sides.is_empty() {
        return Err(invalid("candidate focus is absent from both revisions"));
    }
    Ok(sides)
}
/// Identifies canonical snapshot JSON while excluding this self-describing field.
fn snapshot_digest(snapshot: &Value) -> String {
    let mut canonical = snapshot.clone();
    canonical
        .as_object_mut()
        .expect("explorer snapshot is an object")
        .remove("snapshot_digest");
    canonical.sort_all_objects();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.explorer-snapshot.v1\0");
    hash.update(canonical.to_string().as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}
fn html(snapshot: &[u8]) -> String {
    let esc = String::from_utf8_lossy(snapshot)
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let assets = [
        include_str!("../../ui/semantic-explorer/model.js"),
        include_str!("../../ui/semantic-explorer/layout.js"),
        include_str!("../../ui/semantic-explorer/changes.js"),
        include_str!("../../ui/semantic-explorer/evidence.js"),
        include_str!("../../ui/semantic-explorer/hosts.js"),
        include_str!("../../ui/semantic-explorer/cache.js"),
        include_str!("../../ui/semantic-explorer/view.js"),
    ];
    let mut bodies = assets
        .iter()
        .map(|asset| format!("(function(){{\n{asset}\n}})();"))
        .collect::<Vec<_>>();
    bodies.push("(function(){const data=JSON.parse(document.getElementById('snapshot').textContent);SemapraxExplorerView.createExplorer(document.getElementById('app'),SemapraxExplorerHosts.snapshotHost(data),{side:data.views[0].query.side});})();".to_owned());
    let css = include_str!("../../ui/semantic-explorer/explorer.css");
    let source_section = serde_json::from_slice::<Value>(snapshot)
        .ok()
        .filter(|value| value["source_included"] == true)
        .map(|value| {
            let files = value["source_files"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|file| {
                    let side = file["side"].as_str().unwrap_or("");
                    let path = file["path"].as_str().unwrap_or("");
                    let source_revision = file["source_revision"].as_str().unwrap_or("");
                    let text = file["text"].as_str().unwrap_or("");
                    format!(
                        "<details><summary>{} · {} · {}</summary><pre>{}</pre></details>",
                        html_text(side),
                        html_text(path),
                        html_text(source_revision),
                        html_text(text)
                    )
                })
                .collect::<String>();
            format!(
                "<section aria-label=\"Source-inclusive review\"><h2>Source included</h2><p>This local review contains complete source text. Names, IDs, paths, and source may be confidential; review before sharing.</p>{files}</section>"
            )
        })
        .unwrap_or_default();
    let hashes = bodies
        .iter()
        .map(|body| {
            format!(
                "'sha256-{}'",
                base64(Sha256::digest(body.as_bytes()).as_slice())
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let csp = format!(
        "default-src 'none'; script-src {hashes}; style-src-elem 'sha256-{}'; style-src-attr 'unsafe-inline'; img-src data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'",
        base64(Sha256::digest(css.as_bytes()).as_slice())
    );
    let scripts = bodies
        .iter()
        .map(|body| format!("<script>{body}</script>"))
        .collect::<String>();
    format!(
        "<!doctype html><html><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><style>{css}</style></head><body><div id=app></div>{source_section}<script id=snapshot type=application/json>{esc}</script>{scripts}</body></html>"
    )
}
fn html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_focus_only_requests_the_sides_that_contain_the_target() {
        let catalog = json!({"roots":[
            {"target":"added","base":null,"candidate":{"id":"added"}},
            {"target":"removed","base":{"id":"removed"},"candidate":null},
            {"target":"changed","base":{"id":"changed"},"candidate":{"id":"changed"}}
        ]});
        assert_eq!(
            focus_sides(Some("added"), Some(&catalog)).unwrap(),
            ["candidate"]
        );
        assert_eq!(
            focus_sides(Some("removed"), Some(&catalog)).unwrap(),
            ["base"]
        );
        assert_eq!(
            focus_sides(Some("changed"), Some(&catalog)).unwrap(),
            ["candidate", "base"]
        );
        assert_eq!(
            focus_sides(None, Some(&catalog)).unwrap(),
            Vec::<&str>::new()
        );
        assert_eq!(focus_sides(Some("current"), None).unwrap(), ["current"]);
    }

    #[test]
    fn snapshot_digest_is_stable_with_its_self_describing_field_present() {
        let mut snapshot = json!({"schema":"semaprax.explorer-snapshot.v1","views":[]});
        let expected = snapshot_digest(&snapshot);
        snapshot["snapshot_digest"] = json!(expected.clone());
        assert_eq!(snapshot_digest(&snapshot), expected);
    }
}
