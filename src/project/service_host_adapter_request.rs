//! Independent host consumption for the service adapter-request handoff.
//!
//! The service configuration decoder renders this compact declaration, but a
//! host must replay it independently before selecting any adapter.  The
//! resulting requirements preserve origins and secret *references* only.
//! They do not resolve a secret, create an outbound policy, grant a capability,
//! or authorize I/O.

use serde_json::{Map, Value};

pub(crate) const MAX_SERVICE_HOST_ADAPTER_REQUEST_BYTES: usize = 16 * 1024;
pub(crate) const SERVICE_HOST_ADAPTER_REQUEST_SCHEMA: &str =
    "semaprax.service-host-adapter-request.v1";

const DATABASE_CONNECT: &str = "semaprax.service.database.connect.v1";
const HTTP_SERVE_TLS: &str = "semaprax.service.http.serve-tls.v1";
const SECRETS_RESOLVE: &str = "semaprax.service.secrets.resolve.v1";
const TELEMETRY_EMIT: &str = "semaprax.service.telemetry.emit.v1";

/// One exact declaration the host must satisfy outside SEMAPRAX source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceHostAdapterCapability {
    DatabaseConnect,
    HttpServeTls,
    SecretsResolve,
    TelemetryEmit,
}

impl ServiceHostAdapterCapability {
    pub const fn name(self) -> &'static str {
        match self {
            Self::DatabaseConnect => DATABASE_CONNECT,
            Self::HttpServeTls => HTTP_SERVE_TLS,
            Self::SecretsResolve => SECRETS_RESOLVE,
            Self::TelemetryEmit => TELEMETRY_EMIT,
        }
    }
}

const HOST_REQUIREMENTS: [ServiceHostAdapterCapability; 4] = [
    ServiceHostAdapterCapability::DatabaseConnect,
    ServiceHostAdapterCapability::HttpServeTls,
    ServiceHostAdapterCapability::SecretsResolve,
    ServiceHostAdapterCapability::TelemetryEmit,
];

/// A closed telemetry-export intent. The origin is an adapter target, not an
/// outbound grant: a separately trusted host must still provide a policy whose
/// allowed origins contain this exact value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceTelemetryRequirement {
    endpoint_origin: String,
}

impl ServiceTelemetryRequirement {
    pub fn endpoint_origin(&self) -> &str {
        &self.endpoint_origin
    }
}

/// The database connection declaration retained from a host-mode request.
/// The secret member remains a reference; decoding never resolves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDatabaseRequirement {
    adapter: ServiceDatabaseAdapter,
    dsn_secret_reference: String,
    migration_table: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceDatabaseAdapter {
    Sqlite,
    Postgresql,
}

impl ServiceDatabaseRequirement {
    pub const fn adapter(&self) -> ServiceDatabaseAdapter {
        self.adapter
    }

    pub fn dsn_secret_reference(&self) -> &str {
        &self.dsn_secret_reference
    }

    pub fn migration_table(&self) -> &str {
        &self.migration_table
    }
}

/// The TLS server declaration retained from a host-mode request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceHttpTlsRequirement {
    listen_origin: String,
}

impl ServiceHttpTlsRequirement {
    pub fn listen_origin(&self) -> &str {
        &self.listen_origin
    }
}

/// The three host-owned secret references a service host must resolve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSecretResolutionRequirement {
    password_pepper_reference: String,
    session_signing_key_reference: String,
    webhook_signing_key_reference: String,
}

impl ServiceSecretResolutionRequirement {
    pub fn password_pepper_reference(&self) -> &str {
        &self.password_pepper_reference
    }

    pub fn session_signing_key_reference(&self) -> &str {
        &self.session_signing_key_reference
    }

    pub fn webhook_signing_key_reference(&self) -> &str {
        &self.webhook_signing_key_reference
    }
}

/// A bounded, independently replayed service host-adapter request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceHostAdapterRequestV1 {
    canonical: Vec<u8>,
    requirements: Vec<ServiceHostAdapterCapability>,
    database: Option<ServiceDatabaseRequirement>,
    http: Option<ServiceHttpTlsRequirement>,
    secrets: Option<ServiceSecretResolutionRequirement>,
    telemetry: Option<ServiceTelemetryRequirement>,
}

impl ServiceHostAdapterRequestV1 {
    /// Exact canonical bytes checked by this decoder.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// Closed required capabilities. Fixture requests retain an empty set.
    pub fn requirements(&self) -> &[ServiceHostAdapterCapability] {
        &self.requirements
    }

    pub fn database(&self) -> Option<&ServiceDatabaseRequirement> {
        self.database.as_ref()
    }

    pub fn http(&self) -> Option<&ServiceHttpTlsRequirement> {
        self.http.as_ref()
    }

    pub fn secrets(&self) -> Option<&ServiceSecretResolutionRequirement> {
        self.secrets.as_ref()
    }

    /// The host-mode telemetry target intent, if this request declares one.
    pub fn telemetry(&self) -> Option<&ServiceTelemetryRequirement> {
        self.telemetry.as_ref()
    }
}

/// Independently decode a canonical, bounded service host-adapter request.
///
/// A success is only a read-only declaration replay. In particular, this API
/// cannot construct an `OutboundPolicy`, `OutboundCapability`, secret, store,
/// transport, or any physical adapter.
pub fn decode(bytes: &[u8]) -> Result<ServiceHostAdapterRequestV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_SERVICE_HOST_ADAPTER_REQUEST_BYTES {
        return Err("service host adapter request exceeds its exact byte bound".into());
    }
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "service host adapter request JSON is malformed".to_owned())?;
    let root = value
        .as_object()
        .ok_or_else(|| "service host adapter request root must be an object".to_owned())?;
    if text(root, "schema")? != SERVICE_HOST_ADAPTER_REQUEST_SCHEMA {
        return Err("service host adapter request schema is unknown".into());
    }
    let mode = text(root, "mode")?.to_owned();
    let root = match mode.as_str() {
        "fixture" => closed_object(
            &value,
            &[
                "capabilities",
                "database",
                "http",
                "mode",
                "schema",
                "telemetry",
            ],
            "root",
        )?,
        "host" => closed_object(
            &value,
            &[
                "capabilities",
                "database",
                "http",
                "mode",
                "schema",
                "secrets",
                "telemetry",
            ],
            "root",
        )?,
        _ => return Err("service host adapter request mode is unknown".into()),
    };
    let capabilities = strings(root, "capabilities")?;

    let (requirements, database_requirement, http_requirement, secret_requirement, telemetry) =
        match mode.as_str() {
            "fixture" => {
                let database = closed_member(root, "database", &["adapter", "migration_table"])?;
                let http = closed_member(root, "http", &["adapter", "tls_profile"])?;
                let telemetry = closed_member(root, "telemetry", &["adapter"])?;
                if !capabilities.is_empty()
                    || text(database, "adapter")? != "fixture"
                    || text(database, "migration_table")? != "semaprax_migrations"
                    || text(http, "adapter")? != "fixture"
                    || text(http, "tls_profile")? != "fixture"
                    || text(telemetry, "adapter")? != "fixture"
                {
                    return Err("service fixture adapter request is not capability-free".into());
                }
                (Vec::new(), None, None, None, None)
            }
            "host" => {
                let database = closed_member(
                    root,
                    "database",
                    &["adapter", "dsn_secret_ref", "migration_table"],
                )?;
                let http =
                    closed_member(root, "http", &["adapter", "listen_origin", "tls_profile"])?;
                let secrets = closed_member(
                    root,
                    "secrets",
                    &[
                        "password_pepper_ref",
                        "session_signing_key_ref",
                        "webhook_signing_key_ref",
                    ],
                )?;
                let telemetry = closed_member(root, "telemetry", &["adapter", "endpoint_origin"])?;
                if capabilities.len() != HOST_REQUIREMENTS.len()
                    || capabilities
                        .iter()
                        .zip(HOST_REQUIREMENTS)
                        .any(|(actual, expected)| *actual != expected.name())
                    || !matches!(text(database, "adapter")?, "sqlite" | "postgresql")
                    || text(database, "migration_table")? != "semaprax_migrations"
                    || text(http, "adapter")? != "native"
                    || text(http, "tls_profile")? != "modern"
                    || text(telemetry, "adapter")? != "otlp"
                {
                    return Err("service host adapter requirements are not exact".into());
                }
                for (object, key) in [
                    (database, "dsn_secret_ref"),
                    (secrets, "password_pepper_ref"),
                    (secrets, "session_signing_key_ref"),
                    (secrets, "webhook_signing_key_ref"),
                ] {
                    valid_reference(text(object, key)?)
                        .then_some(())
                        .ok_or_else(|| {
                            format!("service host adapter request {key} reference is invalid")
                        })?;
                }
                for (object, key) in [(http, "listen_origin"), (telemetry, "endpoint_origin")] {
                    valid_https_origin(text(object, key)?)
                        .then_some(())
                        .ok_or_else(|| {
                            format!("service host adapter request {key} origin is invalid")
                        })?;
                }
                let database_requirement = ServiceDatabaseRequirement {
                    adapter: match text(database, "adapter")? {
                        "sqlite" => ServiceDatabaseAdapter::Sqlite,
                        "postgresql" => ServiceDatabaseAdapter::Postgresql,
                        _ => unreachable!("validated above"),
                    },
                    dsn_secret_reference: text(database, "dsn_secret_ref")?.to_owned(),
                    migration_table: text(database, "migration_table")?.to_owned(),
                };
                let http_requirement = ServiceHttpTlsRequirement {
                    listen_origin: text(http, "listen_origin")?.to_owned(),
                };
                let secret_requirement = ServiceSecretResolutionRequirement {
                    password_pepper_reference: text(secrets, "password_pepper_ref")?.to_owned(),
                    session_signing_key_reference: text(secrets, "session_signing_key_ref")?
                        .to_owned(),
                    webhook_signing_key_reference: text(secrets, "webhook_signing_key_ref")?
                        .to_owned(),
                };
                let telemetry = ServiceTelemetryRequirement {
                    endpoint_origin: text(telemetry, "endpoint_origin")?.to_owned(),
                };
                (
                    HOST_REQUIREMENTS.to_vec(),
                    Some(database_requirement),
                    Some(http_requirement),
                    Some(secret_requirement),
                    Some(telemetry),
                )
            }
            _ => unreachable!("checked before request member decoding"),
        };

    value.sort_all_objects();
    let mut canonical = serde_json::to_vec(&value)
        .map_err(|_| "service host adapter request cannot be rendered".to_owned())?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(
            "service host adapter request must use canonical JSON plus one line feed".into(),
        );
    }
    Ok(ServiceHostAdapterRequestV1 {
        canonical,
        requirements,
        database: database_requirement,
        http: http_requirement,
        secrets: secret_requirement,
        telemetry,
    })
}

fn closed_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("service host adapter request {label} must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!(
            "service host adapter request {label} fields are not closed"
        ));
    }
    Ok(object)
}

fn closed_member<'a>(
    root: &'a Map<String, Value>,
    key: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    closed_object(
        root.get(key)
            .ok_or_else(|| format!("service host adapter request lacks {key}"))?,
        keys,
        key,
    )
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("service host adapter request {key} must be text"))
}

fn strings<'a>(object: &'a Map<String, Value>, key: &str) -> Result<Vec<&'a str>, String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("service host adapter request {key} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("service host adapter request {key} entries must be text"))
        })
        .collect()
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_https_origin(value: &str) -> bool {
    // This is intentionally a DNS-name subset of the collector target's
    // canonical origin grammar. Reserve room for its longest fixed route.
    if value.len() > 2037 || !value.starts_with("https://") {
        return false;
    }
    let authority = &value[8..];
    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    let port = parts.next();
    if parts.next().is_some()
        || host.len() < 2
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !label.as_bytes()[0].is_ascii_alphanumeric()
                || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
        })
    {
        return false;
    }
    port.is_none_or(|port| {
        !port.is_empty()
            && port != "443"
            && port.len() <= 5
            && (port.len() == 1 || !port.starts_with('0'))
            && port.bytes().all(|byte| byte.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|port| port > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        include_bytes!("../../examples/task-service-project/service-host-adapter-request.json")
            .to_vec()
    }

    fn host() -> Vec<u8> {
        let mut value = serde_json::json!({
            "schema": SERVICE_HOST_ADAPTER_REQUEST_SCHEMA,
            "mode": "host",
            "capabilities": [DATABASE_CONNECT, HTTP_SERVE_TLS, SECRETS_RESOLVE, TELEMETRY_EMIT],
            "database": {"adapter":"sqlite","dsn_secret_ref":"db.primary","migration_table":"semaprax_migrations"},
            "http": {"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},
            "secrets": {"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},
            "telemetry": {"adapter":"otlp","endpoint_origin":"https://telemetry.example"},
        });
        value.sort_all_objects();
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn fixture_is_capability_free_and_host_retains_exact_telemetry_intent() {
        let fixture = decode(&fixture()).unwrap();
        assert!(fixture.requirements().is_empty());
        assert_eq!(fixture.telemetry(), None);

        let host = host();
        let decoded = decode(&host).unwrap();
        assert_eq!(decoded.canonical_bytes(), host);
        assert_eq!(decoded.requirements(), HOST_REQUIREMENTS);
        assert_eq!(
            decoded.database().unwrap().adapter(),
            ServiceDatabaseAdapter::Sqlite
        );
        assert_eq!(
            decoded.database().unwrap().dsn_secret_reference(),
            "db.primary"
        );
        assert_eq!(
            decoded.database().unwrap().migration_table(),
            "semaprax_migrations"
        );
        assert_eq!(
            decoded.http().unwrap().listen_origin(),
            "https://service.example"
        );
        assert_eq!(
            decoded.secrets().unwrap().password_pepper_reference(),
            "auth.pepper"
        );
        assert_eq!(
            decoded.secrets().unwrap().session_signing_key_reference(),
            "auth.session"
        );
        assert_eq!(
            decoded.secrets().unwrap().webhook_signing_key_reference(),
            "webhook.signing"
        );
        assert_eq!(
            decoded.telemetry().unwrap().endpoint_origin(),
            "https://telemetry.example"
        );
    }

    #[test]
    fn hostile_unknown_duplicate_noncanonical_and_max_plus_one_refuse() {
        let canonical: Value = serde_json::from_slice(&host()).unwrap();
        let mut unknown = canonical.clone();
        unknown["unknown"] = Value::Bool(true);
        unknown.sort_all_objects();
        let mut unknown = serde_json::to_vec(&unknown).unwrap();
        unknown.push(b'\n');
        assert!(decode(&unknown).is_err());

        let mut reordered = canonical.clone();
        reordered["capabilities"] = serde_json::json!([
            TELEMETRY_EMIT,
            SECRETS_RESOLVE,
            HTTP_SERVE_TLS,
            DATABASE_CONNECT,
        ]);
        reordered.sort_all_objects();
        let mut reordered = serde_json::to_vec(&reordered).unwrap();
        reordered.push(b'\n');
        assert!(decode(&reordered).is_err());

        let duplicate = format!(
            r#"{{"schema":"duplicate",{}"#,
            std::str::from_utf8(&host())
                .unwrap()
                .strip_prefix('{')
                .unwrap()
        );
        assert!(decode(duplicate.as_bytes()).is_err());

        let mut noncanonical = host();
        noncanonical.insert(0, b' ');
        assert!(decode(&noncanonical).is_err());

        for origin in [
            "https://telemetry.example:443".to_owned(),
            "https://telemetry..example".to_owned(),
            format!("https://{}.example", "a".repeat(64)),
        ] {
            let mut request = canonical.clone();
            request["telemetry"]["endpoint_origin"] = Value::String(origin);
            request.sort_all_objects();
            let mut request = serde_json::to_vec(&request).unwrap();
            request.push(b'\n');
            assert!(decode(&request).is_err());
        }
        assert!(decode(&vec![b' '; MAX_SERVICE_HOST_ADAPTER_REQUEST_BYTES + 1]).is_err());
    }
}
