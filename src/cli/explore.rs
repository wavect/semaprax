use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    self, ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerView, ProjectSemanticImage,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
const MAX: usize = 16 * 1024 * 1024;
pub(crate) struct Options {
    pub manifest: PathBuf,
    pub output: PathBuf,
    pub html: bool,
    pub target: Option<String>,
    pub depth: usize,
}
pub(crate) fn parse(args: &[String]) -> Result<Options, u8> {
    let Some(manifest) = args.first().filter(|v| !v.starts_with('-')) else {
        eprintln!("explore requires <manifest> --format html|json --output <path>");
        return Err(2);
    };
    let (mut format, mut output, mut target, mut depth) = (None, None, None, 1);
    let mut i = 1;
    while i < args.len() {
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
            x => {
                eprintln!("unknown explore option `{x}`");
                return Err(2);
            }
        }
        i += 1
    }
    let html = match format.as_deref() {
        Some("html") => true,
        Some("json") => false,
        _ => {
            eprintln!("explore requires --format html|json");
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
    Ok(Options {
        manifest: manifest.into(),
        output,
        html,
        target,
        depth,
    })
}
pub(crate) fn run(options: Options) -> Result<(), Vec<Diagnostic>> {
    let bytes = project::with_authenticated_project(&options.manifest, |snapshot| {
        let image =
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())?;
        let mode = if options.target.is_some() {
            ExplorerMode::Context
        } else {
            ExplorerMode::Overview
        };
        let summary: Value = serde_json::from_str(&image.explorer_summary(
            image.image_digest(),
            mode,
            options.target.as_deref(),
            ExplorerQuery::new(
                semaprax::project::ExplorerDirection::Both,
                options.depth,
                256,
                256 * 1024,
            )?,
        )?)
        .unwrap();
        let mut pages = Vec::new();
        for row in summary["inventories"].as_array().unwrap() {
            let view = ExplorerView::parse(row["view"].as_str().unwrap())?;
            pages.push(
                serde_json::from_str::<Value>(&image.explorer_page(
                    image.image_digest(),
                    mode,
                    options.target.as_deref(),
                    ExplorerQuery::new(
                        semaprax::project::ExplorerDirection::Both,
                        options.depth,
                        256,
                        256 * 1024,
                    )?,
                    view,
                    row["handle"].as_str().unwrap(),
                    None,
                    ExplorerPageOptions::default(),
                )?)
                .unwrap(),
            );
        }
        let mut v = json!({"schema":"semaprax.explorer-snapshot.v1","summary":summary,"pages":pages,"source_included":false,"confidentiality":"names_ids_and_paths_may_be_confidential"});
        v.sort_all_objects();
        Ok(v.to_string().into_bytes())
    })?;
    if bytes.len() > MAX {
        return Err(vec![Diagnostic::io(
            "SPX-G328",
            "explorer snapshot exceeds 16MiB",
        )]);
    }
    let out = if options.html {
        html(&bytes).into_bytes()
    } else {
        bytes
    };
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&options.output)
        .map_err(|_| {
            vec![Diagnostic::io(
                "SPX-G328",
                "explorer output already exists or cannot be created",
            )]
        })?;
    f.write_all(&out)
        .map_err(|_| vec![Diagnostic::io("SPX-G328", "cannot write explorer output")])?;
    Ok(())
}
fn html(snapshot: &[u8]) -> String {
    let esc = String::from_utf8_lossy(snapshot).replace("</", "<\\/");
    format!("<!doctype html><meta charset=utf-8><style>{}</style><div id=app></div><script>{}</script><script>{}</script><script>{}</script><script>{}</script><script>SemapraxExplorerView.createExplorer(document.getElementById('app'),SemapraxExplorerHosts.snapshotHost({{views:[{{query:{{mode:'overview',side:'current',depth:1}},summary:JSON.parse(document.getElementById('snapshot').textContent).summary,pages:JSON.parse(document.getElementById('snapshot').textContent).pages}}]}}));</script><script id=snapshot type=application/json>{}</script>",include_str!("../../ui/semantic-explorer/explorer.css"),include_str!("../../ui/semantic-explorer/model.js"),include_str!("../../ui/semantic-explorer/layout.js"),include_str!("../../ui/semantic-explorer/hosts.js"),include_str!("../../ui/semantic-explorer/view.js"),esc)
}
