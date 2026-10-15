//! Complete fresh application bytes, derived through the existing codec owner.
use super::{JsonCodecBundle, ProjectRevision};
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceSource;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Explicit command activation; no path is read by this authority-free API.
#[derive(Clone, Debug)]
pub struct ApplicationCommandSource {
    pub path: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct ApplicationStarter {
    files: Vec<(String, Vec<u8>)>,
    project_revision: String,
    workspace_revision: String,
}
impl ApplicationStarter {
    pub fn files(&self) -> &[(String, Vec<u8>)] {
        &self.files
    }
    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
}

fn refusal(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J181", message)]
}
fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

#[derive(Serialize)]
struct Receipt<'a> {
    schema: &'static str,
    command_profile: &'a str,
    codec_plan_sha256: String,
    codec_plan: &'a str,
    codec_bundle: &'a JsonCodecBundle,
    activated_command: Option<CommandReceipt<'a>>,
    project_revision: &'a str,
    workspace_revision: &'a str,
    semantic_graph_digest: &'a str,
    manifest_sha256: String,
    sources: Vec<SourceReceipt<'a>>,
    check: &'static str,
    test: &'a str,
    build: &'static str,
    run: &'a str,
}
#[derive(Serialize)]
struct CommandReceipt<'a> {
    path: &'a str,
    source: &'a str,
    sha256: String,
}
#[derive(Serialize)]
struct SourceReceipt<'a> {
    path: &'a str,
    sha256: String,
}

/// Derive and replay one complete candidate before a host acquires fresh-tree
/// publication authority. Source/HIR/profile admission is never bypassed.
pub fn derive_application_starter(
    revision: &ProjectRevision,
    plan_bytes: &[u8],
    command_profile: &str,
    command: Option<&ApplicationCommandSource>,
) -> Result<ApplicationStarter, Vec<Diagnostic>> {
    let (expected, entries) = super::decode_json_codec_plan(plan_bytes).map_err(|e| vec![e])?;
    if expected
        .as_deref()
        .is_some_and(|expected| expected != revision.project_revision())
    {
        return Err(refusal("codec plan authored Project revision is stale"));
    }
    if revision.manifest().profile() != Some(command_profile)
        || !(revision.manifest().project_profile().is_source_command()
            || revision.manifest().project_profile()
                == super::ProjectProfile::StdinStreamCollectionRecordCommandIoV1)
    {
        return Err(refusal("starter requires an exact admitted source-command or v31 command profile; it never changes a manifest grant"));
    }
    if !revision.manifest().dependency_sources().is_empty()
        || !revision.manifest().rust_dependencies().is_empty()
        || !revision.manifest().law_sources().is_empty()
    {
        return Err(refusal("starter admits compiler-bundled dependencies only; external subjects, Rust packages and law sources need their own complete installation contract"));
    }
    let bundle = super::derive_json_codec_plan(revision, &entries)?;
    super::verify_json_codec_plan(revision, &entries, &bundle)?;
    if let Some(command) = command {
        if !revision.manifest().sources().contains(&command.path)
            || entries
                .iter()
                .any(|entry| entry.source_path == command.path)
            || command.source.len() > 131_072
        {
            return Err(refusal(
                "command activation is absent, overlaps codec output or exceeds its source bound",
            ));
        }
    }
    let mut sources = Vec::new();
    for path in revision.manifest().sources() {
        let name = path
            .strip_prefix("src/")
            .ok_or_else(|| refusal("starter sources require direct src/*.spx paths"))?;
        if name.is_empty() || name.contains('/') || name.contains('\\') || !name.ends_with(".spx") {
            return Err(refusal("starter sources require direct src/*.spx paths"));
        }
        let original = bundle
            .candidate_sources
            .iter()
            .find(|s| &s.path == path)
            .ok_or_else(|| refusal("complete candidate source is missing"))?;
        let text = command
            .filter(|c| &c.path == path)
            .map_or(original.source.as_str(), |c| c.source.as_str());
        let parsed = crate::parse(text, path).map_err(|e| vec![e])?;
        sources.push(SemanticWorkspaceSource {
            path: path.clone(),
            source: crate::format::canonical(&parsed),
        });
    }
    if !(1..=32).contains(&sources.len())
        || sources.iter().map(|s| s.source.len()).sum::<usize>() > 2 * 1024 * 1024
    {
        return Err(refusal(
            "starter exceeds 32 authored modules or 2 MiB of source",
        ));
    }
    let manifest = revision.manifest().to_canonical_toml();
    let parsed_manifest = super::ProjectManifest::parse(&manifest)?;
    let candidate = super::build::build_owned(&parsed_manifest, sources)?;
    crate::hir::validate(&candidate.entry_program).map_err(|e| vec![e])?;
    crate::hir::validate(&candidate.test_program).map_err(|e| vec![e])?;
    // v31 native tests admit pure typed closures only. Effects cannot acquire
    // a command adapter merely by being listed as tests.
    if revision.manifest().project_profile()
        == super::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
    {
        crate::hir::validate_stream_collection_record_program(&candidate.test_program, None)
            .map_err(|e| vec![e])?;
        if !candidate.test_program.permits.is_empty() {
            return Err(refusal(
                "v31 starter requires a pure declared native test closure",
            ));
        }
    }
    let authored = candidate
        .sources
        .iter()
        .filter(|s| revision.manifest().sources().iter().any(|p| p == s.path()))
        .collect::<Vec<_>>();
    let run = if matches!(
        revision.manifest().project_profile(),
        super::ProjectProfile::SourceCommandSelectedFileV1
            | super::ProjectProfile::SourceCommandSelectedFileResourceV1
    ) {
        "./app --spx-selected-file-v1 0 -- <selected-file> [arguments]"
    } else {
        "./app [arguments]"
    };
    let test = if revision.manifest().project_profile()
        == super::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
    {
        "semaprax test . --target native-collection-record-pure.v1"
    } else {
        "semaprax test . --target native"
    };
    let receipt = Receipt {
        schema: "semaprax.application-starter.v1",
        command_profile,
        codec_plan_sha256: digest(plan_bytes),
        codec_plan: std::str::from_utf8(plan_bytes)
            .map_err(|_| refusal("codec plan is not UTF-8"))?,
        codec_bundle: &bundle,
        activated_command: command.map(|c| CommandReceipt {
            path: &c.path,
            source: &c.source,
            sha256: digest(c.source.as_bytes()),
        }),
        project_revision: &candidate.project_revision,
        workspace_revision: &candidate.workspace_revision,
        semantic_graph_digest: candidate.semantic.graph_digest(),
        manifest_sha256: digest(manifest.as_bytes()),
        sources: authored
            .iter()
            .map(|s| SourceReceipt {
                path: s.path(),
                sha256: digest(s.source().as_bytes()),
            })
            .collect(),
        check: "semaprax check .",
        test,
        build: "semaprax build . --target native -o app",
        run,
    };
    let encoded =
        serde_json::to_string(&receipt).map_err(|_| refusal("cannot encode starter receipt"))?;
    if encoded.len() > 12 * 1024 * 1024 {
        return Err(refusal("starter receipt exceeds 12 MiB"));
    }
    let readme = format!("# {}\n\nCompiler-derived application starter.\n\n```sh\nsemaprax check .\n{test}\nsemaprax build . --target native -o app\n{run}\n```\n\nSupply the command's explicit argv, selected-file or stdin input when running.\nThese routes grant only the original manifest capabilities. Native compiler and\nprocess execution remain explicit host operations; installation runs neither.\n\nExact generation receipt (evidence, without publication authority):\n\n```json\n{}\n```\n", revision.manifest().name(), encoded);
    let guide = "# Application authoring\n\nStart with `semaprax help language author:routes`; select only the bounded card\nyou need. The complete declared source and test inventory is in semaprax.toml.\nUse the exact native test target in README.md for declared tests; build the command\nwith `semaprax build . --target native -o app`. Read README.md for exact source,\ncodec-plan, command activation and generation receipts. A fresh source tree is\nnot runtime qualification. No installation executes native code or downloads\ndependencies. Do not change stable IDs or silently add capabilities.\n";
    let mut files = vec![
        ("README.md".to_owned(), readme.into_bytes()),
        ("AGENTS.md".to_owned(), guide.as_bytes().to_vec()),
        ("semaprax.toml".to_owned(), manifest.into_bytes()),
    ];
    files.extend(
        authored
            .iter()
            .map(|s| (s.path().to_owned(), s.source().as_bytes().to_vec())),
    );
    Ok(ApplicationStarter {
        files,
        project_revision: candidate.project_revision,
        workspace_revision: candidate.workspace_revision,
    })
}
