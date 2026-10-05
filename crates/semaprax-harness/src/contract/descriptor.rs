//! `semaprax.harness-provider.v1` descriptor: strict parse, typed model, digest.
//!
//! Permissions are a *request* only. Nothing in this module converts them into
//! a grant; grants live in the machine-local trust store (HP-02).

use super::kind::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{self, parse_strict, JsonLimits};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const DESCRIPTOR_SCHEMA: &str = "semaprax.harness-provider.v1";
const PROTOCOL_NAME: &str = "semaprax.harness-rpc.v1";
const PLUGIN_MANIFEST_SCHEMA: &str = "semaprax.plugin-manifest.v1";
const DESCRIPTOR_MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    Builtin,
    Native,
    Node,
    Python,
}

impl Runtime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Native => "native",
            Self::Node => "node",
            Self::Python => "python",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationMode {
    Cooperative,
    Kill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpstreamIdentity {
    pub name: String,
    pub package: String,
    pub repository: String,
    pub versions: Vec<String>,
    pub identity_probe: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredCapability {
    pub kind_name: String,
    /// `Some` only for kinds the host knows; unknown kinds stay inactive.
    pub kind: Option<CapabilityKind>,
    pub version: u32,
    pub required: bool,
    pub operations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionCapability {
    pub kind: String,
    pub version: u32,
}

/// What the descriptor *asks for*. There is deliberately no conversion to a grant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionRequest {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub network: Vec<String>,
    pub process: Vec<String>,
    pub secrets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceBounds {
    pub handshake_timeout_ms: u64,
    pub invoke_timeout_ms: u64,
    pub max_frame_bytes: usize,
    pub max_concurrency: u32,
    pub idle_shutdown_ms: u64,
}

/// Host caps on [`ResourceBounds`].
pub const MAX_FRAME_BYTES_CAP: usize = 4 * 1024 * 1024;
pub const MAX_HANDSHAKE_MS_CAP: u64 = 60_000;
pub const MAX_INVOKE_MS_CAP: u64 = 600_000;
pub const MAX_CONCURRENCY_CAP: u32 = 8;
pub const MAX_IDLE_MS_CAP: u64 = 3_600_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigField {
    /// `bool`, `int`, `string` or `string-list`.
    pub ty: String,
    pub default: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestedRecord {
    pub upstream: String,
    pub os: String,
    pub result: String,
}

/// A record, never a claim of production support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportRecord {
    pub license: String,
    pub isolation: String,
    pub tested: Vec<TestedRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub provider_id: String,
    pub provider_version: String,
    pub runtime: Runtime,
    pub entry: Vec<String>,
    pub adapter_version: String,
    pub upstream: Option<UpstreamIdentity>,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub capabilities: Vec<DeclaredCapability>,
    pub extensions: Vec<ExtensionCapability>,
    pub platforms: Vec<String>,
    pub config_fields: BTreeMap<String, ConfigField>,
    pub permissions: PermissionRequest,
    pub resources: ResourceBounds,
    pub cancellation: CancellationMode,
    pub support: SupportRecord,
    digest: String,
}

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn obj<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| bad("SPX-HPA013", format!("{what} must be an object")))?;
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(bad("SPX-HPA012", format!("unknown member `{k}` in {what}")));
        }
    }
    for r in required {
        if !m.contains_key(*r) {
            return Err(bad("SPX-HPA013", format!("{what} is missing `{r}`")));
        }
    }
    Ok(m)
}

fn string(m: &Map<String, Value>, k: &str) -> HarnessResult<String> {
    match m.get(k).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.len() <= 1024 && !s.contains('\0') => Ok(s.to_string()),
        _ => Err(bad(
            "SPX-HPA013",
            format!("`{k}` must be a non-empty string"),
        )),
    }
}

fn strings(m: &Map<String, Value>, k: &str, max: usize) -> HarnessResult<Vec<String>> {
    let a = m
        .get(k)
        .and_then(Value::as_array)
        .filter(|a| a.len() <= max);
    let a = a.ok_or_else(|| {
        bad(
            "SPX-HPA013",
            format!("`{k}` must be an array of at most {max} strings"),
        )
    })?;
    a.iter()
        .map(|x| {
            x.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 1024 && !s.contains('\0'))
                .map(str::to_string)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            bad(
                "SPX-HPA013",
                format!("`{k}` must contain non-empty strings"),
            )
        })
}

fn uint(m: &Map<String, Value>, k: &str) -> HarnessResult<u64> {
    m.get(k).and_then(Value::as_u64).ok_or_else(|| {
        bad(
            "SPX-HPA013",
            format!("`{k}` must be a non-negative integer"),
        )
    })
}

fn version_u32(m: &Map<String, Value>, k: &str) -> HarnessResult<u32> {
    match uint(m, k)? {
        v @ 1..=1_000_000 => Ok(v as u32),
        _ => Err(bad(
            "SPX-HPA013",
            format!("`{k}` must be between 1 and 1000000"),
        )),
    }
}

/// Numeric dotted version (`1.2.3`, optional `-pre` suffix ignored for ordering).
pub fn parse_version(s: &str) -> Option<Vec<u64>> {
    let core = s.split_once('-').map_or(s, |(c, _)| c);
    let parts: Option<Vec<u64>> = core
        .split('.')
        .map(|p| {
            if p.is_empty() || p.len() > 9 {
                None
            } else {
                p.parse().ok()
            }
        })
        .collect();
    parts.filter(|p| !p.is_empty() && p.len() <= 4)
}

fn version(m: &Map<String, Value>, k: &str) -> HarnessResult<String> {
    let s = string(m, k)?;
    parse_version(&s).map(|_| s.clone()).ok_or_else(|| {
        bad(
            "SPX-HPA013",
            format!("`{k}` must be a dotted numeric version"),
        )
    })
}

/// `<org>/<name>`, ASCII `[a-z0-9._-]`, one `/`, at most 128 bytes.
pub fn valid_provider_id(id: &str) -> bool {
    let ok = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
    };
    id.len() <= 128 && id.split_once('/').is_some_and(|(a, b)| ok(a) && ok(b))
}

fn valid_kind_name(k: &str) -> bool {
    let ok = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|b| {
                b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || matches!(b, b'.' | b'_' | b'-' | b'/')
            })
    };
    k.len() <= 128 && k.as_bytes().first().is_some_and(u8::is_ascii_lowercase) && ok(k)
}

impl Descriptor {
    /// Strictly parse and validate a descriptor document.
    pub fn parse(bytes: &[u8]) -> HarnessResult<Descriptor> {
        let limits = JsonLimits {
            max_bytes: DESCRIPTOR_MAX_BYTES,
            max_depth: 16,
            max_nodes: 8192,
        };
        let doc = parse_strict(bytes, &limits)?;
        match doc.get("schema").and_then(Value::as_str) {
            Some(DESCRIPTOR_SCHEMA) => {}
            Some(PLUGIN_MANIFEST_SCHEMA) => {
                return Err(bad("SPX-HPA011", "a `semaprax.plugin-manifest.v1` document is a compiled-export descriptor, not a harness-provider descriptor"))
            }
            other => return Err(bad("SPX-HPA010", format!("unsupported descriptor schema {other:?}; expected `{DESCRIPTOR_SCHEMA}`"))),
        }
        let top = obj(
            &doc,
            "descriptor",
            &[
                "schema",
                "provider",
                "adapter",
                "protocol",
                "capabilities",
                "platforms",
                "permissions",
                "resources",
                "cancellation",
                "support",
            ],
            &["upstream", "extensions", "config"],
        )?;

        let p = obj(&top["provider"], "provider", &["id", "version"], &[])?;
        let provider_id = string(p, "id")?;
        if !valid_provider_id(&provider_id) {
            return Err(bad(
                "SPX-HPA014",
                format!("malformed provider id `{provider_id}`"),
            ));
        }
        let provider_version = version(p, "version")?;

        let a = obj(
            &top["adapter"],
            "adapter",
            &["runtime", "version"],
            &["entry"],
        )?;
        let runtime = match string(a, "runtime")?.as_str() {
            "builtin" => Runtime::Builtin,
            "native" => Runtime::Native,
            "node" => Runtime::Node,
            "python" => Runtime::Python,
            r => return Err(bad("SPX-HPA013", format!("unknown adapter runtime `{r}`"))),
        };
        let entry = match (runtime, a.contains_key("entry")) {
            (Runtime::Builtin, true) => {
                return Err(bad(
                    "SPX-HPA015",
                    "a builtin descriptor must not have an `entry`",
                ))
            }
            (Runtime::Builtin, false) => Vec::new(),
            (_, false) => {
                return Err(bad(
                    "SPX-HPA015",
                    "a non-builtin descriptor needs an `entry` argv",
                ))
            }
            (_, true) => {
                let e = strings(a, "entry", 64)?;
                if e.is_empty() {
                    return Err(bad("SPX-HPA015", "`entry` must not be empty"));
                }
                if runtime == Runtime::Native
                    && (e[0].starts_with('/') || e[0].split('/').any(|s| s == ".."))
                {
                    return Err(bad(
                        "SPX-HPA015",
                        "a native `entry` must be relative to the descriptor directory",
                    ));
                }
                e
            }
        };
        let adapter_version = version(a, "version")?;

        let upstream = match top.get("upstream") {
            None => None,
            Some(u) => {
                let u = obj(
                    u,
                    "upstream",
                    &[
                        "name",
                        "package",
                        "repository",
                        "versions",
                        "identity_probe",
                    ],
                    &[],
                )?;
                Some(UpstreamIdentity {
                    name: string(u, "name")?,
                    package: string(u, "package")?,
                    repository: string(u, "repository")?,
                    versions: strings(u, "versions", 64)?,
                    identity_probe: strings(u, "identity_probe", 16)?,
                })
            }
        };

        let pr = obj(&top["protocol"], "protocol", &["name", "min", "max"], &[])?;
        if string(pr, "name")? != PROTOCOL_NAME {
            return Err(bad(
                "SPX-HPA016",
                format!("protocol name must be `{PROTOCOL_NAME}`"),
            ));
        }
        let (protocol_min, protocol_max) = (version_u32(pr, "min")?, version_u32(pr, "max")?);
        if !(protocol_min <= 1 && 1 <= protocol_max) {
            return Err(bad(
                "SPX-HPA016",
                format!("protocol range {protocol_min}..{protocol_max} does not cover protocol 1"),
            ));
        }

        let capabilities = parse_capabilities(&top["capabilities"])?;
        let extensions = parse_extensions(top.get("extensions"))?;
        let platforms = strings(top, "platforms", 32)?;
        if platforms.is_empty() {
            return Err(bad("SPX-HPA013", "`platforms` must not be empty"));
        }
        let config_fields = parse_config(top.get("config"))?;

        let pm = obj(
            &top["permissions"],
            "permissions",
            &["read", "write", "network", "process", "secrets"],
            &[],
        )?;
        let permissions = PermissionRequest {
            read: strings(pm, "read", 64)?,
            write: strings(pm, "write", 64)?,
            network: strings(pm, "network", 64)?,
            process: strings(pm, "process", 64)?,
            secrets: strings(pm, "secrets", 64)?,
        };

        let r = obj(
            &top["resources"],
            "resources",
            &[
                "handshake_timeout_ms",
                "invoke_timeout_ms",
                "max_frame_bytes",
                "max_concurrency",
                "idle_shutdown_ms",
            ],
            &[],
        )?;
        let bound = |k: &str, cap: u64| -> HarnessResult<u64> {
            match uint(r, k)? {
                v @ 1..=u64::MAX if v <= cap => Ok(v),
                v => Err(bad(
                    "SPX-HPA018",
                    format!("`{k}` = {v} is outside 1..={cap}"),
                )),
            }
        };
        let resources = ResourceBounds {
            handshake_timeout_ms: bound("handshake_timeout_ms", MAX_HANDSHAKE_MS_CAP)?,
            invoke_timeout_ms: bound("invoke_timeout_ms", MAX_INVOKE_MS_CAP)?,
            max_frame_bytes: bound("max_frame_bytes", MAX_FRAME_BYTES_CAP as u64)? as usize,
            max_concurrency: bound("max_concurrency", MAX_CONCURRENCY_CAP as u64)? as u32,
            idle_shutdown_ms: bound("idle_shutdown_ms", MAX_IDLE_MS_CAP)?,
        };

        let cancellation = match top["cancellation"].as_str() {
            Some("cooperative") => CancellationMode::Cooperative,
            Some("kill") => CancellationMode::Kill,
            _ => {
                return Err(bad(
                    "SPX-HPA013",
                    "`cancellation` must be `cooperative` or `kill`",
                ))
            }
        };

        let s = obj(
            &top["support"],
            "support",
            &["license", "isolation", "tested"],
            &[],
        )?;
        let mut tested = Vec::new();
        for t in s["tested"]
            .as_array()
            .filter(|a| a.len() <= 256)
            .ok_or_else(|| bad("SPX-HPA013", "`support.tested` must be an array"))?
        {
            let t = obj(
                t,
                "support.tested entry",
                &["upstream", "os", "result"],
                &[],
            )?;
            tested.push(TestedRecord {
                upstream: string(t, "upstream")?,
                os: string(t, "os")?,
                result: string(t, "result")?,
            });
        }
        let support = SupportRecord {
            license: string(s, "license")?,
            isolation: string(s, "isolation")?,
            tested,
        };

        let digest = json::digest(DESCRIPTOR_SCHEMA, &doc);
        Ok(Descriptor {
            provider_id,
            provider_version,
            runtime,
            entry,
            adapter_version,
            upstream,
            protocol_min,
            protocol_max,
            capabilities,
            extensions,
            platforms,
            config_fields,
            permissions,
            resources,
            cancellation,
            support,
            digest,
        })
    }

    /// `sha256:<hex>` over the schema domain and the canonical document.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Canonical document form; `parse(canonical(to_json()))` has the same digest.
    pub fn to_json(&self) -> Value {
        let mut top = Map::new();
        top.insert("schema".into(), json!(DESCRIPTOR_SCHEMA));
        top.insert(
            "provider".into(),
            json!({"id": self.provider_id, "version": self.provider_version}),
        );
        let mut adapter =
            json!({"runtime": self.runtime.as_str(), "version": self.adapter_version});
        if self.runtime != Runtime::Builtin {
            adapter["entry"] = json!(self.entry);
        }
        top.insert("adapter".into(), adapter);
        if let Some(u) = &self.upstream {
            top.insert("upstream".into(), json!({"name": u.name, "package": u.package, "repository": u.repository, "versions": u.versions, "identity_probe": u.identity_probe}));
        }
        top.insert(
            "protocol".into(),
            json!({"name": PROTOCOL_NAME, "min": self.protocol_min, "max": self.protocol_max}),
        );
        top.insert(
            "capabilities".into(),
            Value::Array(self.capabilities.iter().map(|c| json!({"kind": c.kind_name, "version": c.version, "required": c.required, "operations": c.operations})).collect()),
        );
        if !self.extensions.is_empty() {
            top.insert(
                "extensions".into(),
                Value::Array(
                    self.extensions
                        .iter()
                        .map(|x| json!({"kind": x.kind, "version": x.version}))
                        .collect(),
                ),
            );
        }
        top.insert("platforms".into(), json!(self.platforms));
        if !self.config_fields.is_empty() {
            let fields: Map<String, Value> = self
                .config_fields
                .iter()
                .map(|(k, f)| {
                    let mut o = json!({"type": f.ty});
                    if let Some(d) = &f.default {
                        o["default"] = d.clone();
                    }
                    (k.clone(), o)
                })
                .collect();
            top.insert("config".into(), json!({"fields": fields}));
        }
        let p = &self.permissions;
        top.insert("permissions".into(), json!({"read": p.read, "write": p.write, "network": p.network, "process": p.process, "secrets": p.secrets}));
        let r = &self.resources;
        top.insert(
            "resources".into(),
            json!({"handshake_timeout_ms": r.handshake_timeout_ms, "invoke_timeout_ms": r.invoke_timeout_ms, "max_frame_bytes": r.max_frame_bytes, "max_concurrency": r.max_concurrency, "idle_shutdown_ms": r.idle_shutdown_ms}),
        );
        top.insert(
            "cancellation".into(),
            json!(match self.cancellation {
                CancellationMode::Cooperative => "cooperative",
                CancellationMode::Kill => "kill",
            }),
        );
        let tested: Vec<Value> = self
            .support
            .tested
            .iter()
            .map(|t| json!({"upstream": t.upstream, "os": t.os, "result": t.result}))
            .collect();
        top.insert("support".into(), json!({"license": self.support.license, "isolation": self.support.isolation, "tested": tested}));
        Value::Object(top)
    }
}

fn parse_capabilities(v: &Value) -> HarnessResult<Vec<DeclaredCapability>> {
    let arr = v
        .as_array()
        .filter(|a| a.len() <= 64)
        .ok_or_else(|| bad("SPX-HPA013", "`capabilities` must be an array"))?;
    let mut out: Vec<DeclaredCapability> = Vec::new();
    for c in arr {
        let c = obj(
            c,
            "capability",
            &["kind", "version", "required", "operations"],
            &[],
        )?;
        let kind_name = string(c, "kind")?;
        if !valid_kind_name(&kind_name) {
            return Err(bad(
                "SPX-HPA022",
                format!("malformed capability kind `{kind_name}`"),
            ));
        }
        let version = version_u32(c, "version")?;
        // One kind may be declared at several distinct versions (MR-01).
        if out
            .iter()
            .any(|d| d.kind_name == kind_name && d.version == version)
        {
            return Err(bad(
                "SPX-HPA017",
                format!("duplicate capability declaration `{kind_name}` v{version}"),
            ));
        }
        let kind = CapabilityKind::parse(&kind_name);
        let operations = strings(c, "operations", 16)?;
        if let Some(k) = kind {
            let mut seen: Vec<&String> = Vec::new();
            if operations.is_empty() {
                return Err(bad(
                    "SPX-HPA022",
                    format!("`{kind_name}` declares no operations"),
                ));
            }
            for op in &operations {
                if !k.operations().contains(&op.as_str()) || seen.contains(&op) {
                    return Err(bad(
                        "SPX-HPA022",
                        format!("`{op}` is not a distinct operation of {kind_name}"),
                    ));
                }
                seen.push(op);
            }
        }
        let required = c["required"]
            .as_bool()
            .ok_or_else(|| bad("SPX-HPA013", "`required` must be a boolean"))?;
        out.push(DeclaredCapability {
            kind_name,
            kind,
            version,
            required,
            operations,
        });
    }
    Ok(out)
}

fn parse_extensions(v: Option<&Value>) -> HarnessResult<Vec<ExtensionCapability>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let arr = v
        .as_array()
        .filter(|a| a.len() <= 64)
        .ok_or_else(|| bad("SPX-HPA013", "`extensions` must be an array"))?;
    let mut out: Vec<ExtensionCapability> = Vec::new();
    for x in arr {
        let x = obj(x, "extension", &["kind", "version"], &[])?;
        let kind = string(x, "kind")?;
        if !kind.starts_with("x.") || !valid_kind_name(&kind) || !kind.contains('/') {
            return Err(bad(
                "SPX-HPA022",
                format!("extension kind `{kind}` must look like `x.<org>/<name>`"),
            ));
        }
        if out.iter().any(|e| e.kind == kind) {
            return Err(bad(
                "SPX-HPA017",
                format!("duplicate extension declaration `{kind}`"),
            ));
        }
        out.push(ExtensionCapability {
            kind,
            version: version_u32(x, "version")?,
        });
    }
    Ok(out)
}

fn parse_config(v: Option<&Value>) -> HarnessResult<BTreeMap<String, ConfigField>> {
    let Some(v) = v else {
        return Ok(BTreeMap::new());
    };
    let c = obj(v, "config", &["fields"], &[])?;
    let fields = c["fields"]
        .as_object()
        .filter(|f| f.len() <= 64)
        .ok_or_else(|| bad("SPX-HPA013", "`config.fields` must be an object"))?;
    let mut out = BTreeMap::new();
    for (name, f) in fields {
        let f = obj(f, "config field", &["type"], &["default"])?;
        let ty = string(f, "type")?;
        let default = f.get("default").cloned();
        let fits = match (ty.as_str(), &default) {
            (_, None) => matches!(ty.as_str(), "bool" | "int" | "string" | "string-list"),
            ("bool", Some(Value::Bool(_))) | ("string", Some(Value::String(_))) => true,
            ("int", Some(d)) => d.is_i64() || d.is_u64(),
            ("string-list", Some(Value::Array(a))) => a.iter().all(Value::is_string),
            _ => false,
        };
        if !fits {
            return Err(bad(
                "SPX-HPA013",
                format!("config field `{name}` has an invalid type or default"),
            ));
        }
        out.insert(name.clone(), ConfigField { ty, default });
    }
    Ok(out)
}
