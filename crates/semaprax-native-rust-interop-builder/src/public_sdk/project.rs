//! Authenticated Project v1 entry point and canonical Project SDK subject.

use super::authority::build_project_native_rust_sdk_inner;
use super::*;

const MAX_PROJECT_SUBJECT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectSourceFact {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectExportFact {
    pub(super) id: String,
    module: String,
    path: String,
}

/// Target-neutral authenticated Project authority. The private descriptor,
/// inner bundle, and outer SDK manifest bind the current target separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectSdkSubject {
    pub(super) canonical: String,
    pub(super) digest: String,
    pub(super) name: String,
    pub(super) manifest: String,
    pub(super) manifest_digest: String,
    pub(super) manifest_bytes: usize,
    pub(super) project_revision: String,
    pub(super) workspace_revision: String,
    pub(super) entry_module: String,
    pub(super) graph_digest: String,
    pub(super) sources: Vec<ProjectSourceFact>,
    pub(super) exports: Vec<ProjectExportFact>,
    pub(super) imports: Vec<String>,
    pub(super) capabilities: Vec<String>,
}

/// Builds one fresh Native Rust SDK from the exact authenticated linked entry
/// program and stable-ID scalar export set of a Project v1 manifest. Its
/// Project subject is target-neutral; the generated descriptor owns the exact
/// current-target ABI facts.
pub fn build_project_native_rust_sdk(
    manifest_path: &Path,
    output: &Path,
) -> Result<ProjectNativeRustSdkBundle, Vec<Diagnostic>> {
    semaprax::project::with_authenticated_project(manifest_path, |snapshot| {
        build_authenticated_project_native_rust_sdk(snapshot, output)
    })
}

/// Builds from a caller-held authenticated snapshot without reopening any
/// Project authority. This is the full-toolchain CLI's publication boundary.
pub fn build_authenticated_project_native_rust_sdk(
    snapshot: &mut semaprax::project::ProjectSnapshot,
    output: &Path,
) -> Result<ProjectNativeRustSdkBundle, Vec<Diagnostic>> {
    snapshot.with_authenticated_native_rust_sdk_subject(|input| {
        let subject = ProjectSdkSubject::from_authenticated(&input)?;
        verify_project_subject(subject.canonical.as_bytes(), &subject)
            .map_err(|error| vec![error])?;
        let sdk = build_project_native_rust_sdk_inner(input.program(), &subject, output)
            .map_err(PublicBuildError::into_diagnostics)?;
        Ok(ProjectNativeRustSdkBundle {
            sdk,
            project_revision: subject.project_revision.clone(),
            workspace_revision: subject.workspace_revision.clone(),
            subject_digest: subject.digest.clone(),
        })
    })
}

impl ProjectSdkSubject {
    fn from_authenticated(
        input: &semaprax::project::ProjectNativeSdkSubject<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let manifest = input.canonical_manifest();
        let sources = input
            .sources()
            .iter()
            .map(|source| ProjectSourceFact {
                path: source.path().to_owned(),
                source_graph_schema: source.source_graph_schema().to_owned(),
                source_revision: source.source_revision().to_owned(),
                source_digest: source.source_digest().to_owned(),
                bytes: source.source().len(),
            })
            .collect::<Vec<_>>();
        let exports = input
            .exports()
            .iter()
            .map(|export| ProjectExportFact {
                id: export.stable_id().to_owned(),
                module: export.module().to_owned(),
                path: export.path().to_owned(),
            })
            .collect::<Vec<_>>();
        // Every native Rust import the authenticated entry program declares is
        // selected, and the capabilities are exactly the union of their
        // declared effects. Phase A independently re-derives the reached set
        // and rejects any disagreement.
        let mut imports = Vec::new();
        let mut effects = BTreeSet::new();
        for interface in &input.program().interfaces {
            for import in &interface.imports {
                if !import.native_rust {
                    continue;
                }
                imports.push(import.id.as_str().to_owned());
                for effect in &import.effects {
                    effects.insert(effect.as_str());
                }
            }
        }
        imports.sort();
        let capabilities = effects.into_iter().map(str::to_owned).collect::<Vec<_>>();
        if sources.len() > semaprax::project::MAX_SOURCES
            || sources
                .windows(2)
                .any(|rows| rows[0].path.as_bytes() >= rows[1].path.as_bytes())
            || exports.is_empty()
            || exports.len() > semaprax::project::MAX_WEB_EXPORTS
            || exports
                .windows(2)
                .any(|rows| rows[0].id.as_bytes() >= rows[1].id.as_bytes())
            || imports.len() > MAX_IMPORTS
            || imports
                .windows(2)
                .any(|rows| rows[0].as_bytes() >= rows[1].as_bytes())
            || capabilities.len() > MAX_EFFECTS
            || capabilities
                .windows(2)
                .any(|rows| rows[0].as_bytes() >= rows[1].as_bytes())
        {
            return Err(vec![sdk_error(
                "Native Rust Project SDK subject facts are not canonical",
            )]);
        }
        let graph_digest = input.project_graph_digest().to_owned();
        let mut subject = Self {
            canonical: String::new(),
            digest: String::new(),
            name: input.project_name().to_owned(),
            manifest: manifest.to_owned(),
            manifest_digest: raw_digest(manifest.as_bytes()),
            manifest_bytes: manifest.len(),
            project_revision: input.project_revision().to_owned(),
            workspace_revision: input.workspace_revision().to_owned(),
            entry_module: input.entry_module().to_owned(),
            graph_digest,
            sources,
            exports,
            imports,
            capabilities,
        };
        subject.canonical = render_project_subject(&subject)?;
        subject.digest = domain_digest(PROJECT_SUBJECT_DOMAIN, subject.canonical.as_bytes());
        Ok(subject)
    }
}

fn render_project_subject(subject: &ProjectSdkSubject) -> Result<String, Vec<Diagnostic>> {
    let mut output = String::with_capacity(16_384);
    output.push_str("{\"schema\":");
    json_string(&mut output, PROJECT_NATIVE_RUST_SUBJECT_SCHEMA);
    output.push_str(",\"project_schema\":");
    json_string(&mut output, semaprax::project::PROJECT_SCHEMA);
    output.push_str(",\"name\":");
    json_string(&mut output, &subject.name);
    write!(
        output,
        ",\"manifest\":{{\"bytes\":{}",
        subject.manifest_bytes
    )
    .expect("String writing cannot fail");
    output.push_str(",\"digest\":");
    json_string(&mut output, &subject.manifest_digest);
    output.push_str(",\"canonical\":");
    json_string(&mut output, &subject.manifest);
    output.push_str("},\"project_revision\":");
    json_string(&mut output, &subject.project_revision);
    output.push_str(",\"workspace_revision\":");
    json_string(&mut output, &subject.workspace_revision);
    output.push_str(",\"project_graph\":{\"schema\":");
    json_string(
        &mut output,
        semaprax::project::PROJECT_SEMANTIC_GRAPH_SCHEMA,
    );
    output.push_str(",\"digest\":");
    json_string(&mut output, &subject.graph_digest);
    output.push_str("},\"entry_module\":");
    json_string(&mut output, &subject.entry_module);
    output.push_str(",\"sources\":[");
    for (index, source) in subject.sources.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        json_string(&mut output, &source.path);
        output.push_str(",\"source_graph_schema\":");
        json_string(&mut output, &source.source_graph_schema);
        output.push_str(",\"source_revision\":");
        json_string(&mut output, &source.source_revision);
        output.push_str(",\"source_digest\":");
        json_string(&mut output, &source.source_digest);
        write!(output, ",\"bytes\":{}}}", source.bytes).expect("String writing cannot fail");
    }
    output.push_str("],\"exports\":[");
    for (index, export) in subject.exports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        json_string(&mut output, &export.id);
        output.push_str(",\"module\":");
        json_string(&mut output, &export.module);
        output.push_str(",\"path\":");
        json_string(&mut output, &export.path);
        output.push('}');
    }
    output.push_str("],\"imports\":");
    string_array(&mut output, &subject.imports);
    output.push_str(",\"capabilities\":");
    string_array(&mut output, &subject.capabilities);
    output.push_str("}\n");
    if output.len() > MAX_PROJECT_SUBJECT_BYTES {
        return Err(vec![sdk_error(
            "Native Rust Project SDK subject exceeds its bound",
        )]);
    }
    Ok(output)
}

pub(super) fn verify_project_subject(
    bytes: &[u8],
    expected: &ProjectSdkSubject,
) -> Result<(), Diagnostic> {
    if bytes.len() > MAX_PROJECT_SUBJECT_BYTES || !bytes.ends_with(b"\n") {
        return Err(sdk_error("Native Rust Project SDK subject replay failed"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| sdk_error("Native Rust Project SDK subject replay failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 12)
        .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
    let manifest = project_object(&value, "manifest")?;
    let graph = project_object(&value, "project_graph")?;
    if root.get("schema").and_then(Value::as_str) != Some(PROJECT_NATIVE_RUST_SUBJECT_SCHEMA)
        || root.get("project_schema").and_then(Value::as_str)
            != Some(semaprax::project::PROJECT_SCHEMA)
        || root.get("name").and_then(Value::as_str) != Some(expected.name.as_str())
        || manifest.len() != 3
        || manifest.get("digest").and_then(Value::as_str) != Some(expected.manifest_digest.as_str())
        || manifest.get("bytes").and_then(Value::as_u64)
            != u64::try_from(expected.manifest_bytes).ok()
        || manifest.get("canonical").and_then(Value::as_str) != Some(expected.manifest.as_str())
        || expected.manifest.len() != expected.manifest_bytes
        || raw_digest(expected.manifest.as_bytes()) != expected.manifest_digest
        || root.get("project_revision").and_then(Value::as_str)
            != Some(expected.project_revision.as_str())
        || root.get("workspace_revision").and_then(Value::as_str)
            != Some(expected.workspace_revision.as_str())
        || root.get("entry_module").and_then(Value::as_str) != Some(expected.entry_module.as_str())
        || graph.len() != 2
        || graph.get("schema").and_then(Value::as_str)
            != Some(semaprax::project::PROJECT_SEMANTIC_GRAPH_SCHEMA)
        || graph.get("digest").and_then(Value::as_str) != Some(expected.graph_digest.as_str())
    {
        return Err(sdk_error("Native Rust Project SDK subject replay failed"));
    }
    project_string_array(root, "imports", &expected.imports)?;
    project_string_array(root, "capabilities", &expected.capabilities)?;
    let sources = root
        .get("sources")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == expected.sources.len())
        .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
    for (row, expected) in sources.iter().zip(&expected.sources) {
        let row = row
            .as_object()
            .filter(|row| row.len() == 5)
            .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
        if row.get("path").and_then(Value::as_str) != Some(expected.path.as_str())
            || row.get("source_graph_schema").and_then(Value::as_str)
                != Some(expected.source_graph_schema.as_str())
            || row.get("source_revision").and_then(Value::as_str)
                != Some(expected.source_revision.as_str())
            || row.get("source_digest").and_then(Value::as_str)
                != Some(expected.source_digest.as_str())
            || row.get("bytes").and_then(Value::as_u64) != u64::try_from(expected.bytes).ok()
        {
            return Err(sdk_error("Native Rust Project SDK subject replay failed"));
        }
    }
    let exports = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == expected.exports.len())
        .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
    for (row, expected) in exports.iter().zip(&expected.exports) {
        let row = row
            .as_object()
            .filter(|row| row.len() == 3)
            .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
        if row.get("stable_id").and_then(Value::as_str) != Some(expected.id.as_str())
            || row.get("module").and_then(Value::as_str) != Some(expected.module.as_str())
            || row.get("path").and_then(Value::as_str) != Some(expected.path.as_str())
        {
            return Err(sdk_error("Native Rust Project SDK subject replay failed"));
        }
    }
    let canonical = render_project_subject(expected)
        .map_err(|_| sdk_error("Native Rust Project SDK subject replay failed"))?;
    let manifest = semaprax::project::ProjectManifest::parse(&expected.manifest)
        .map_err(|_| sdk_error("Native Rust Project SDK subject replay failed"))?;
    if manifest.name() != expected.name
        || manifest.entry() != expected.entry_module
        || manifest.sources().len() > expected.sources.len()
        || manifest
            .sources()
            .iter()
            .any(|path| !expected.sources.iter().any(|source| path == &source.path))
        || expected.sources.iter().any(|source| {
            !manifest.sources().contains(&source.path) && !source.path.starts_with("dependencies/")
        })
        || manifest.web_exports().len() != expected.exports.len()
        || manifest
            .web_exports()
            .iter()
            .zip(&expected.exports)
            .any(|(id, export)| id != &export.id)
    {
        return Err(sdk_error("Native Rust Project SDK subject replay failed"));
    }
    if bytes != canonical.as_bytes()
        || domain_digest(PROJECT_SUBJECT_DOMAIN, bytes) != expected.digest
    {
        return Err(sdk_error("Native Rust Project SDK subject replay failed"));
    }
    Ok(())
}

/// Replays one canonical string array against its exact expected elements.
fn project_string_array(
    root: &Map<String, Value>,
    key: &str,
    expected: &[String],
) -> Result<(), Diagnostic> {
    let rows = root
        .get(key)
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == expected.len())
        .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))?;
    if rows
        .iter()
        .zip(expected)
        .any(|(row, expected)| row.as_str() != Some(expected.as_str()))
    {
        return Err(sdk_error("Native Rust Project SDK subject replay failed"));
    }
    Ok(())
}

fn project_object<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>, Diagnostic> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| sdk_error("Native Rust Project SDK subject replay failed"))
}

#[cfg(test)]
mod tests {
    use super::super::package::{
        render_sdk_manifest, verify_sdk_manifest, SdkManifestInputs, SdkManifestSubject,
    };
    use super::*;

    fn subject() -> ProjectSdkSubject {
        let manifest = concat!(
            "schema = \"semaprax.project.v1\"\n",
            "name = \"calculator\"\n",
            "entry = \"calculator.app\"\n",
            "sources = [\"src/app.spx\", \"src/math.spx\"]\n",
            "web_exports = [\"calculator.add\"]\n",
            "tests = [\"calculator.tests\"]\n",
        )
        .to_owned();
        let manifest_digest = raw_digest(manifest.as_bytes());
        let mut subject = ProjectSdkSubject {
            canonical: String::new(),
            digest: String::new(),
            name: "calculator".to_owned(),
            manifest_bytes: manifest.len(),
            manifest,
            manifest_digest,
            project_revision: format!("sha256:{}", "2".repeat(64)),
            workspace_revision: format!("sha256:{}", "3".repeat(64)),
            entry_module: "calculator.app".to_owned(),
            graph_digest: format!("sha256:{}", "4".repeat(64)),
            sources: vec![
                ProjectSourceFact {
                    path: "src/app.spx".to_owned(),
                    source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                    source_revision: format!("sha256:{}", "5".repeat(64)),
                    source_digest: format!("sha256:{}", "6".repeat(64)),
                    bytes: 127,
                },
                ProjectSourceFact {
                    path: "src/math.spx".to_owned(),
                    source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                    source_revision: format!("sha256:{}", "7".repeat(64)),
                    source_digest: format!("sha256:{}", "8".repeat(64)),
                    bytes: 311,
                },
            ],
            exports: vec![ProjectExportFact {
                id: "calculator.add".to_owned(),
                module: "calculator.math".to_owned(),
                path: "src/math.spx".to_owned(),
            }],
            imports: Vec::new(),
            capabilities: Vec::new(),
        };
        subject.canonical = render_project_subject(&subject).unwrap();
        subject.digest = domain_digest(PROJECT_SUBJECT_DOMAIN, subject.canonical.as_bytes());
        subject
    }

    #[test]
    fn project_subject_is_exact_and_replayed_independently() {
        let subject = subject();
        assert!(subject.canonical.starts_with(
            "{\"schema\":\"semaprax.project-native-rust-subject.v1\",\"project_schema\":\"semaprax.project.v1\""
        ));
        assert!(subject
            .canonical
            .ends_with("\"imports\":[],\"capabilities\":[]}\n"));
        assert!(!subject.canonical.contains("\"target\""));
        verify_project_subject(subject.canonical.as_bytes(), &subject).unwrap();

        let forged = subject
            .canonical
            .replacen("\"bytes\":127", "\"bytes\":128", 1);
        assert!(verify_project_subject(forged.as_bytes(), &subject).is_err());
    }

    #[test]
    fn project_sdk_manifest_replays_only_with_its_closed_subject() {
        let subject = subject();
        let facts = DescriptorFacts {
            module: subject.entry_module.clone(),
            source_revision: subject.digest.clone(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            exports: Vec::new(),
            imports: Vec::new(),
        };
        let options = NativeRustSdkOptions {
            exports: Vec::new(),
            imports: Vec::new(),
            capabilities: Vec::new(),
        };
        let sources = PackageSources {
            cargo_toml: "cargo".to_owned(),
            build_rs: "build".to_owned(),
            lib_rs: "lib".to_owned(),
        };
        let inputs = SdkManifestInputs {
            facts: &facts,
            options: &options,
            descriptor: b"project descriptor",
            inner_manifest: b"project inner manifest",
            sources: &sources,
            safe_inner: b"safe",
            ffi_inner: b"ffi",
            archive: b"archive",
        };
        let manifest = render_sdk_manifest(inputs, SdkManifestSubject::Project(&subject)).unwrap();
        verify_sdk_manifest(
            manifest.as_bytes(),
            inputs,
            SdkManifestSubject::Project(&subject),
        )
        .unwrap();
        assert!(manifest.starts_with("{\"schema\":\"semaprax.project-native-rust-sdk.v1\""));
        assert!(
            verify_sdk_manifest(manifest.as_bytes(), inputs, SdkManifestSubject::Source,).is_err()
        );
    }
}
