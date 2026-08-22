//! Deterministic generated-crate sources and SDK manifest replay.

use super::*;

fn parameters(parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("arg_{index}: {}", parameter.ty.rust()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn arguments(parameters: &[Parameter]) -> String {
    (0..parameters.len())
        .map(|index| format!("arg_{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_lib(facts: &DescriptorFacts, capabilities: &[String]) -> String {
    let mut output = String::with_capacity(65_536);
    output.push_str("#[path=\"semaprax_native_rust_interop.rs\"]mod inner;\nmod public_api{#![forbid(unsafe_code)]\nuse super::inner;\nuse core::num::NonZeroU32;\n#[repr(u8)]#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub enum NativeRustSdkStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n#[derive(Debug,Eq,PartialEq)]pub enum NativeRustSdkImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustSdkStatusClass,retryable:bool},HostFailure}\n#[derive(Debug,Eq,PartialEq)]pub enum NativeRustSdkCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustSdkStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\n#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub struct NativeRustSdkAdmissionError;\n");
    output.push_str("pub trait NativeRustSdkImports{");
    for import in &facts.imports {
        write!(
            output,
            "fn {}(&mut self{}{})->NativeRustSdkImportResult<{}>;",
            import.public_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ","
            },
            parameters(&import.parameters),
            import.result.rust(),
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\nstruct HostAdapter<H>(H);\nimpl<H:NativeRustSdkImports>inner::NativeRustImports for HostAdapter<H>{");
    for import in &facts.imports {
        let args = arguments(&import.parameters);
        write!(
            output,
            "fn {}(&mut self{}{})->inner::NativeRustImportResult<{}>{{match self.0.{}({}){{NativeRustSdkImportResult::Success(value)=>inner::NativeRustImportResult::Success(value),NativeRustSdkImportResult::Status{{code,class,retryable}}=>inner::NativeRustImportResult::Status{{code,class:match class{{NativeRustSdkStatusClass::Semantic=>inner::NativeRustStatusClass::Semantic,NativeRustSdkStatusClass::Contract=>inner::NativeRustStatusClass::Contract,NativeRustSdkStatusClass::Import=>inner::NativeRustStatusClass::Import,NativeRustSdkStatusClass::Adapter=>inner::NativeRustStatusClass::Adapter}},retryable}},NativeRustSdkImportResult::HostFailure=>inner::NativeRustImportResult::HostFailure}}}}",
            import.inner_method,
            if import.parameters.is_empty() { "" } else { "," },
            parameters(&import.parameters),
            import.result.rust(),
            import.public_method,
            args,
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\npub struct NativeRustSdk<H:NativeRustSdkImports>{bridge:inner::NativeRustBridge<HostAdapter<H>>}\nimpl<H:NativeRustSdkImports>NativeRustSdk<H>{pub fn new(host:H,capabilities:&[&str])->Result<Self,NativeRustSdkAdmissionError>{let capabilities=inner::NativeRustCapabilities::new(capabilities).map_err(|_|NativeRustSdkAdmissionError)?;Ok(Self{bridge:inner::NativeRustBridge::new(HostAdapter(host),capabilities)})}\n");
    for export in &facts.exports {
        let args = arguments(&export.parameters);
        write!(
            output,
            "pub fn {}(&mut self{}{})->Result<{},NativeRustSdkCallError>{{self.bridge.{}({}).map_err(|error|match error{{inner::NativeRustCallError::Semantic{{domain_id,code,class,retryable}}=>NativeRustSdkCallError::Semantic{{domain_id,code,class:match class{{inner::NativeRustStatusClass::Semantic=>NativeRustSdkStatusClass::Semantic,inner::NativeRustStatusClass::Contract=>NativeRustSdkStatusClass::Contract,inner::NativeRustStatusClass::Import=>NativeRustSdkStatusClass::Import,inner::NativeRustStatusClass::Adapter=>NativeRustSdkStatusClass::Adapter}},retryable}},inner::NativeRustCallError::HostFailed=>NativeRustSdkCallError::HostFailed,inner::NativeRustCallError::HostPanicked=>NativeRustSdkCallError::HostPanicked,inner::NativeRustCallError::AdapterRejected=>NativeRustSdkCallError::AdapterRejected}})}}",
            export.public_method,
            if export.parameters.is_empty() { "" } else { "," },
            parameters(&export.parameters),
            export.result.rust(),
            export.inner_method,
            args,
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\n");
    output.push_str("pub const SEMAPRAX_NATIVE_RUST_SDK_CAPABILITIES:&[&str]=&[");
    for (index, capability) in capabilities.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, capability);
    }
    output.push_str("];\n}\npub use public_api::*;\n");
    output
}

pub(super) fn render_package_sources(
    facts: &DescriptorFacts,
    capabilities: &[String],
) -> PackageSources {
    let cargo_toml = format!(
        "[package]\nname = \"{CRATE_NAME}\"\nversion = \"{CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"src/lib.rs\"\n\n[workspace]\n"
    );
    let archive = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let build_rs = format!(
        "#![forbid(unsafe_code)]\nfn main(){{let target=std::env::var(\"TARGET\").unwrap_or_default();if target!={:?}{{panic!(\"generated SEMAPRAX SDK target mismatch\")}}let root=std::env::var_os(\"CARGO_MANIFEST_DIR\").expect(\"Cargo must set CARGO_MANIFEST_DIR\");let native=std::path::PathBuf::from(root).join(\"native\");let native=native.to_str().filter(|path|!path.contains(['\\r','\\n'])).expect(\"generated SDK package path must be Unicode without CR/LF\");println!(\"cargo:rerun-if-changed=native/{archive}\");println!(\"cargo:rustc-link-search=native={{native}}\");println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_sdk\");}}\n",
        facts.target,
    );
    PackageSources {
        cargo_toml,
        build_rs,
        lib_rs: render_lib(facts, capabilities),
    }
}

fn file_row(output: &mut String, path: &str, bytes: &[u8]) {
    output.push_str("{\"path\":");
    json_string(output, path);
    write!(output, ",\"bytes\":{},\"sha256\":", bytes.len()).expect("writing manifest cannot fail");
    json_string(output, &raw_digest(bytes));
    output.push('}');
}

fn signature(output: &mut String, parameters: &[Parameter], result: Scalar) {
    output.push_str("\"parameters\":[");
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"name\":");
        json_string(output, &parameter.name);
        output.push_str(",\"type\":");
        json_string(output, parameter.ty.wire());
        output.push('}');
    }
    output.push_str("],\"result\":");
    json_string(output, result.wire());
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_sdk_manifest(
    facts: &DescriptorFacts,
    options: &NativeRustSdkOptions,
    descriptor: &[u8],
    inner_manifest: &[u8],
    sources: &PackageSources,
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
) -> Result<String, Diagnostic> {
    let archive_path = if cfg!(windows) {
        "native/semaprax_native_rust_sdk.lib"
    } else {
        "native/libsemaprax_native_rust_sdk.a"
    };
    let mut files = vec![
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("native/descriptor.json", descriptor),
        (archive_path, archive),
        ("native/semaprax.native-rust-interop.json", inner_manifest),
        ("src/lib.rs", sources.lib_rs.as_bytes()),
        ("src/semaprax_native_rust_interop.rs", safe_inner),
        ("src/semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ];
    files.sort_by_key(|(path, _)| path.as_bytes());
    let mut output = String::with_capacity(65_536);
    output.push_str("{\"schema\":");
    json_string(&mut output, SDK_SCHEMA);
    output.push_str(",\"crate\":{\"name\":");
    json_string(&mut output, CRATE_NAME);
    output.push_str(",\"version\":");
    json_string(&mut output, CRATE_VERSION);
    output.push_str(",\"target\":");
    json_string(&mut output, &facts.target);
    output.push_str("},\"source\":{\"module\":");
    json_string(&mut output, &facts.module);
    output.push_str(",\"revision\":");
    json_string(&mut output, &facts.source_revision);
    output.push_str("},\"inner\":{\"descriptor_digest\":");
    json_string(&mut output, &domain_digest(DESCRIPTOR_DOMAIN, descriptor));
    output.push_str(",\"bundle_digest\":");
    json_string(
        &mut output,
        &domain_digest(INNER_BUNDLE_DOMAIN, inner_manifest),
    );
    output.push_str("},\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        file_row(&mut output, path, bytes);
    }
    output.push_str("],\"exports\":[");
    for (index, export) in facts.exports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        json_string(&mut output, &export.id);
        output.push_str(",\"method\":");
        json_string(&mut output, &export.public_method);
        output.push_str(",\"inner_method\":");
        json_string(&mut output, &export.inner_method);
        output.push(',');
        signature(&mut output, &export.parameters, export.result);
        output.push('}');
    }
    output.push_str("],\"imports\":[");
    for (index, import) in facts.imports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        json_string(&mut output, &import.id);
        output.push_str(",\"method\":");
        json_string(&mut output, &import.public_method);
        output.push_str(",\"inner_method\":");
        json_string(&mut output, &import.inner_method);
        output.push(',');
        signature(&mut output, &import.parameters, import.result);
        output.push_str(",\"failure\":{\"kind\":");
        if let Some(domain) = &import.failure_domain {
            json_string(&mut output, "status");
            output.push_str(",\"domain_id\":");
            json_string(&mut output, domain);
        } else {
            json_string(&mut output, "infallible");
        }
        output.push('}');
        output.push('}');
    }
    output.push_str("],\"capabilities\":");
    string_array(&mut output, &options.capabilities);
    output.push_str(",\"limits\":");
    output.push_str(SDK_LIMITS_JSON);
    output.push_str(",\"nonclaims\":[");
    for (index, value) in SDK_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, value);
    }
    output.push_str("]}\n");
    if output.len() > MAX_SDK_MANIFEST_BYTES {
        return Err(sdk_error("Native Rust SDK manifest exceeds its bound"));
    }
    Ok(output)
}

struct ManifestReplay<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ManifestReplay<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn text(&mut self, expected: &str) -> Result<(), Diagnostic> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or_else(|| sdk_error("Native Rust SDK manifest replay failed"))?;
        if self.bytes.get(self.offset..end) != Some(expected.as_bytes()) {
            return Err(sdk_error("Native Rust SDK manifest replay failed"));
        }
        self.offset = end;
        Ok(())
    }

    fn json(&mut self, expected: &str) -> Result<(), Diagnostic> {
        let encoded = serde_json::to_string(expected)
            .map_err(|_| sdk_error("Native Rust SDK manifest replay failed"))?;
        self.text(&encoded)
    }

    fn number(&mut self, value: usize) -> Result<(), Diagnostic> {
        self.text(&value.to_string())
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(sdk_error("Native Rust SDK manifest replay failed"))
        }
    }
}

fn replay_file(
    replay: &mut ManifestReplay<'_>,
    path: &str,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    replay.text("{\"path\":")?;
    replay.json(path)?;
    replay.text(",\"bytes\":")?;
    replay.number(bytes.len())?;
    replay.text(",\"sha256\":")?;
    replay.json(&raw_digest(bytes))?;
    replay.text("}")
}

fn replay_signature(
    replay: &mut ManifestReplay<'_>,
    parameters: &[Parameter],
    result: Scalar,
) -> Result<(), Diagnostic> {
    replay.text("\"parameters\":[")?;
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"name\":")?;
        replay.json(&parameter.name)?;
        replay.text(",\"type\":")?;
        replay.json(parameter.ty.wire())?;
        replay.text("}")?;
    }
    replay.text("],\"result\":")?;
    replay.json(result.wire())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_sdk_manifest(
    manifest: &[u8],
    facts: &DescriptorFacts,
    options: &NativeRustSdkOptions,
    descriptor: &[u8],
    inner_manifest: &[u8],
    sources: &PackageSources,
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
) -> Result<(), Diagnostic> {
    if manifest.len() > MAX_SDK_MANIFEST_BYTES || !manifest.ends_with(b"\n") {
        return Err(sdk_error("Native Rust SDK manifest replay failed"));
    }
    let archive_path = if cfg!(windows) {
        "native/semaprax_native_rust_sdk.lib"
    } else {
        "native/libsemaprax_native_rust_sdk.a"
    };
    let mut files = vec![
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("native/descriptor.json", descriptor),
        (archive_path, archive),
        ("native/semaprax.native-rust-interop.json", inner_manifest),
        ("src/lib.rs", sources.lib_rs.as_bytes()),
        ("src/semaprax_native_rust_interop.rs", safe_inner),
        ("src/semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ];
    files.sort_by_key(|(path, _)| path.as_bytes());
    let mut replay = ManifestReplay::new(manifest);
    replay.text("{\"schema\":")?;
    replay.json(SDK_SCHEMA)?;
    replay.text(",\"crate\":{\"name\":")?;
    replay.json(CRATE_NAME)?;
    replay.text(",\"version\":")?;
    replay.json(CRATE_VERSION)?;
    replay.text(",\"target\":")?;
    replay.json(&facts.target)?;
    replay.text("},\"source\":{\"module\":")?;
    replay.json(&facts.module)?;
    replay.text(",\"revision\":")?;
    replay.json(&facts.source_revision)?;
    replay.text("},\"inner\":{\"descriptor_digest\":")?;
    replay.json(&domain_digest(DESCRIPTOR_DOMAIN, descriptor))?;
    replay.text(",\"bundle_digest\":")?;
    replay.json(&domain_digest(INNER_BUNDLE_DOMAIN, inner_manifest))?;
    replay.text("},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay_file(&mut replay, path, bytes)?;
    }
    replay.text("],\"exports\":[")?;
    for (index, export) in facts.exports.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"id\":")?;
        replay.json(&export.id)?;
        replay.text(",\"method\":")?;
        replay.json(&export.public_method)?;
        replay.text(",\"inner_method\":")?;
        replay.json(&export.inner_method)?;
        replay.text(",")?;
        replay_signature(&mut replay, &export.parameters, export.result)?;
        replay.text("}")?;
    }
    replay.text("],\"imports\":[")?;
    for (index, import) in facts.imports.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"id\":")?;
        replay.json(&import.id)?;
        replay.text(",\"method\":")?;
        replay.json(&import.public_method)?;
        replay.text(",\"inner_method\":")?;
        replay.json(&import.inner_method)?;
        replay.text(",")?;
        replay_signature(&mut replay, &import.parameters, import.result)?;
        replay.text(",\"failure\":{\"kind\":")?;
        if let Some(domain) = &import.failure_domain {
            replay.json("status")?;
            replay.text(",\"domain_id\":")?;
            replay.json(domain)?;
        } else {
            replay.json("infallible")?;
        }
        replay.text("}")?;
        replay.text("}")?;
    }
    replay.text("],\"capabilities\":[")?;
    for (index, capability) in options.capabilities.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.json(capability)?;
    }
    replay.text("],\"limits\":")?;
    replay.text(SDK_LIMITS_JSON)?;
    replay.text(",\"nonclaims\":[")?;
    for (index, nonclaim) in SDK_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.json(nonclaim)?;
    }
    replay.text("]}\n")?;
    replay.finish()
}
