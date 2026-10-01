//! Independent replay for the generated service host-configuration fixture.
//!
//! The JSON Schema is guidance for external tooling. This decoder owns the
//! compiler-side closed shape, cross-field rules, finite bounds, and canonical
//! bytes. Decoding grants no database, network, telemetry, or secret authority.

use serde_json::{Map, Value};

pub(super) const MAX_SERVICE_CONFIG_BYTES: usize = 16 * 1024;
pub(super) const MAX_SERVICE_ADAPTER_REQUEST_BYTES: usize = 16 * 1024;
const SCHEMA: &str = "semaprax.service-config.v1";
const ADAPTER_REQUEST_SCHEMA: &str = "semaprax.service-host-adapter-request.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Fixture,
    Host,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ServiceConfigV1 {
    canonical: Vec<u8>,
    adapter_request: Vec<u8>,
}

impl ServiceConfigV1 {
    pub(super) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// The bounded, canonical declaration a host adapter must accept before it
    /// performs anything. It contains only adapter selections, origins, and
    /// secret *references*; it neither resolves a secret nor grants authority.
    pub(super) fn adapter_request_bytes(&self) -> &[u8] {
        &self.adapter_request
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<ServiceConfigV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_SERVICE_CONFIG_BYTES {
        return Err("service configuration exceeds its exact byte bound".into());
    }
    let mut value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "service configuration JSON is malformed".to_owned())?;
    let root = closed_object(
        &value,
        &["database", "http", "mode", "schema", "secrets", "telemetry"],
        "root",
    )?;
    if text(root, "schema")? != SCHEMA {
        return Err("service configuration schema is unknown".into());
    }
    let mode = match text(root, "mode")? {
        "fixture" => Mode::Fixture,
        "host" => Mode::Host,
        _ => return Err("service configuration mode is unknown".into()),
    };

    let database = closed_member(
        root,
        "database",
        &["adapter", "dsn_secret_ref", "migration_table"],
    )?;
    if text(database, "migration_table")? != "semaprax_migrations" {
        return Err("service migration table is not the fixed v1 identity".into());
    }
    let database_adapter = text(database, "adapter")?;
    let dsn = reference(database, "dsn_secret_ref")?;
    match (mode, database_adapter) {
        (Mode::Fixture, "fixture") | (Mode::Host, "snapshot") => {}
        (_, "sqlite") => {
            return Err(
                "service database adapter sqlite is unsupported; service-config.v1 admits snapshot only"
                    .into(),
            );
        }
        (_, "postgresql") => {
            return Err(
                "service database adapter postgresql is unsupported; service-config.v1 admits snapshot only"
                    .into(),
            );
        }
        _ => return Err("service database adapter is not admitted by service-config.v1".into()),
    }
    if dsn.is_some() {
        return Err(
            "service database dsn_secret_ref is unsupported; snapshot mode stores state under --state-dir"
                .into(),
        );
    }

    let http = closed_member(root, "http", &["adapter", "listen_origin", "tls_profile"])?;
    let http_adapter = text(http, "adapter")?;
    let listen_origin = origin(http, "listen_origin")?;
    let tls_profile = text(http, "tls_profile")?;

    let secrets = closed_member(
        root,
        "secrets",
        &[
            "password_pepper_ref",
            "session_signing_key_ref",
            "webhook_signing_key_ref",
        ],
    )?;
    let password = reference(secrets, "password_pepper_ref")?;
    let session = reference(secrets, "session_signing_key_ref")?;
    let webhook = reference(secrets, "webhook_signing_key_ref")?;

    let telemetry = closed_member(root, "telemetry", &["adapter", "endpoint_origin"])?;
    let telemetry_adapter = text(telemetry, "adapter")?;
    let telemetry_origin = origin(telemetry, "endpoint_origin")?;

    let fixture = database_adapter == "fixture"
        && dsn.is_none()
        && http_adapter == "fixture"
        && listen_origin.is_none()
        && tls_profile == "fixture"
        && password.is_none()
        && session.is_none()
        && webhook.is_none()
        && telemetry_adapter == "fixture"
        && telemetry_origin.is_none();
    let host = database_adapter == "snapshot"
        && dsn.is_none()
        && http_adapter == "native"
        && listen_origin.is_some()
        && tls_profile == "modern"
        && password.is_some()
        && session.is_some()
        && webhook.is_some()
        && matches!(
            telemetry_adapter,
            "semaprax-json-events" | "semaprax-json-events-v2" | "otlp-http-json"
        )
        && telemetry_origin.is_some();
    if !matches!(
        (mode, fixture, host),
        (Mode::Fixture, true, false) | (Mode::Host, false, true)
    ) {
        return Err("service configuration mode and adapter selections disagree".into());
    }

    let adapter_request = adapter_request(
        mode,
        database_adapter,
        dsn,
        text(database, "migration_table")?,
        http_adapter,
        listen_origin,
        tls_profile,
        password,
        session,
        webhook,
        telemetry_adapter,
        telemetry_origin,
    )?;

    value.sort_all_objects();
    let mut canonical = serde_json::to_vec(&value)
        .map_err(|_| "service configuration cannot be rendered".to_owned())?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err("service configuration must use canonical JSON plus one line feed".into());
    }
    Ok(ServiceConfigV1 {
        canonical,
        adapter_request,
    })
}

#[allow(clippy::too_many_arguments)]
fn adapter_request(
    mode: Mode,
    database_adapter: &str,
    dsn: Option<&str>,
    migration_table: &str,
    http_adapter: &str,
    listen_origin: Option<&str>,
    tls_profile: &str,
    password: Option<&str>,
    session: Option<&str>,
    webhook: Option<&str>,
    telemetry_adapter: &str,
    telemetry_origin: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut request = match mode {
        Mode::Fixture => serde_json::json!({
            "schema": ADAPTER_REQUEST_SCHEMA,
            "mode": "fixture",
            "capabilities": [],
            "database": {"adapter": database_adapter, "migration_table": migration_table},
            "http": {"adapter": http_adapter, "tls_profile": tls_profile},
            "telemetry": {"adapter": telemetry_adapter},
        }),
        Mode::Host => {
            if dsn.is_some() {
                return Err(
                    "service database dsn_secret_ref is unsupported; snapshot mode stores state under --state-dir"
                        .into(),
                );
            }
            let listen_origin = listen_origin.ok_or("service host request lacks HTTPS origin")?;
            let password = password.ok_or("service host request lacks password reference")?;
            let session = session.ok_or("service host request lacks session reference")?;
            let webhook = webhook.ok_or("service host request lacks webhook reference")?;
            let telemetry_origin =
                telemetry_origin.ok_or("service host request lacks telemetry origin")?;
            serde_json::json!({
                "schema": ADAPTER_REQUEST_SCHEMA,
                "mode": "host",
                "capabilities": [
                    "semaprax.service.http.serve-tls.v1",
                    "semaprax.service.secrets.resolve.v1",
                    "semaprax.service.telemetry.emit.v1",
                ],
                "database": {
                    "adapter": database_adapter,
                    "migration_table": migration_table,
                },
                "http": {
                    "adapter": http_adapter,
                    "listen_origin": listen_origin,
                    "tls_profile": tls_profile,
                },
                "secrets": {
                    "password_pepper_ref": password,
                    "session_signing_key_ref": session,
                    "webhook_signing_key_ref": webhook,
                },
                "telemetry": {
                    "adapter": telemetry_adapter,
                    "endpoint_origin": telemetry_origin,
                },
            })
        }
    };
    request.sort_all_objects();
    let mut bytes = serde_json::to_vec(&request)
        .map_err(|_| "service adapter request cannot be rendered".to_owned())?;
    bytes.push(b'\n');
    if bytes.len() > MAX_SERVICE_ADAPTER_REQUEST_BYTES {
        return Err("service adapter request exceeds its exact byte bound".into());
    }
    Ok(bytes)
}

fn closed_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("service configuration {label} must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!(
            "service configuration {label} fields are not closed"
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
            .ok_or_else(|| format!("service configuration lacks {key}"))?,
        keys,
        key,
    )
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("service configuration {key} must be text"))
}

fn reference<'a>(object: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    let Some(value) = object.get(key) else {
        return Err(format!("service configuration lacks {key}"));
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value
        .as_str()
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.as_bytes()[0].is_ascii_lowercase()
                && value.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        })
        .ok_or_else(|| format!("service configuration {key} reference is invalid"))?;
    Ok(Some(value))
}

fn origin<'a>(object: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    let Some(value) = object.get(key) else {
        return Err(format!("service configuration lacks {key}"));
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value
        .as_str()
        .filter(|value| valid_https_origin(value))
        .ok_or_else(|| format!("service configuration {key} origin is invalid"))?;
    Ok(Some(value))
}

fn valid_https_origin(value: &str) -> bool {
    // Keep every admitted origin constructible as a telemetry collector
    // target. The longest fixed collector route is `/v1/metrics` (11 bytes),
    // and collector endpoints are bounded to 2,048 bytes in total.
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
        include_bytes!("../../../examples/task-service-project/service.config.json").to_vec()
    }

    #[test]
    fn fixture_replays_exactly_and_host_shape_is_distinct() {
        let decoded = decode(&fixture()).unwrap();
        assert_eq!(decoded.canonical_bytes(), fixture());
        assert_eq!(
            decoded.adapter_request_bytes(),
            include_bytes!(
                "../../../examples/task-service-project/service-host-adapter-request.json"
            )
        );

        let host = serde_json::json!({
            "schema": SCHEMA,
            "mode": "host",
            "database": {"adapter":"snapshot","dsn_secret_ref":null,"migration_table":"semaprax_migrations"},
            "http": {"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},
            "secrets": {"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},
            "telemetry": {"adapter":"semaprax-json-events","endpoint_origin":"https://telemetry.example"},
        });
        let mut host = serde_json::to_vec(&host).unwrap();
        host.push(b'\n');
        let decoded = decode(&host).unwrap();
        assert_eq!(decoded.canonical_bytes(), host);
        let request: Value = serde_json::from_slice(decoded.adapter_request_bytes()).unwrap();
        assert_eq!(request["mode"], "host");
        assert_eq!(request["database"]["adapter"], "snapshot");
        assert_eq!(request["http"]["listen_origin"], "https://service.example");
        assert_eq!(
            request["capabilities"],
            serde_json::json!([
                "semaprax.service.http.serve-tls.v1",
                "semaprax.service.secrets.resolve.v1",
                "semaprax.service.telemetry.emit.v1",
            ])
        );
        assert!(!decoded
            .adapter_request_bytes()
            .windows(b"secret-value".len())
            .any(|window| window == b"secret-value"));

        let canonical_host: Value = serde_json::from_slice(&host).unwrap();
        for label in ["semaprax-json-events-v2", "semaprax-json-events-v3"] {
            let mut versioned = canonical_host.clone();
            versioned["telemetry"]["adapter"] = Value::String(label.into());
            versioned.sort_all_objects();
            let mut bytes = serde_json::to_vec(&versioned).unwrap();
            bytes.push(b'\n');
            if label.ends_with("v2") {
                let decoded = decode(&bytes).unwrap();
                let request = crate::project::service_host_adapter_request::decode(
                    decoded.adapter_request_bytes(),
                )
                .unwrap();
                assert_eq!(request.telemetry().unwrap().adapter(), crate::project::service_host_adapter_request::ServiceTelemetryAdapter::SemapraxJsonEventsV2);
            } else {
                assert!(decode(&bytes).is_err());
            }
        }

        let mut otlp = canonical_host.clone();
        otlp["telemetry"]["adapter"] = Value::String("otlp-http-json".into());
        otlp.sort_all_objects();
        let mut otlp = serde_json::to_vec(&otlp).unwrap();
        otlp.push(b'\n');
        let decoded = decode(&otlp).unwrap();
        let request: Value = serde_json::from_slice(decoded.adapter_request_bytes()).unwrap();
        assert_eq!(request["telemetry"]["adapter"], "otlp-http-json");

        for (field, legacy_label, expected) in [
            (
                "database",
                "sqlite",
                "service database adapter sqlite is unsupported; service-config.v1 admits snapshot only",
            ),
            (
                "database",
                "postgresql",
                "service database adapter postgresql is unsupported; service-config.v1 admits snapshot only",
            ),
            ("telemetry", "otlp", "service configuration mode and adapter selections disagree"),
        ] {
            let mut legacy = canonical_host.clone();
            legacy[field]["adapter"] = Value::String(legacy_label.into());
            legacy.sort_all_objects();
            let mut legacy = serde_json::to_vec(&legacy).unwrap();
            legacy.push(b'\n');
            assert_eq!(
                decode(&legacy).unwrap_err(),
                expected,
                "legacy {legacy_label} adapter label must refuse with its stable diagnostic"
            );
        }
        let mut dsn = canonical_host;
        dsn["database"]["dsn_secret_ref"] = Value::String("db.primary".into());
        dsn.sort_all_objects();
        let mut dsn = serde_json::to_vec(&dsn).unwrap();
        dsn.push(b'\n');
        assert_eq!(
            decode(&dsn).unwrap_err(),
            "service database dsn_secret_ref is unsupported; snapshot mode stores state under --state-dir"
        );
    }

    #[test]
    fn hostile_shape_mode_and_canonical_drift_refuse() {
        let valid: Value = serde_json::from_slice(&fixture()).unwrap();
        let mutations: [fn(&mut Value); 7] = [
            |value: &mut Value| value["unknown"] = Value::Bool(true),
            |value: &mut Value| value["mode"] = Value::String("host".into()),
            |value: &mut Value| value["database"]["adapter"] = Value::String("sqlite".into()),
            |value: &mut Value| value["database"]["adapter"] = Value::String("postgresql".into()),
            |value: &mut Value| {
                value["database"]["dsn_secret_ref"] = Value::String("postgres://credential".into())
            },
            |value: &mut Value| value["telemetry"]["adapter"] = Value::String("otlp".into()),
            |value: &mut Value| {
                value["http"]["listen_origin"] = Value::String("http://insecure.example".into())
            },
        ];
        for mutation in mutations {
            let mut changed = valid.clone();
            mutation(&mut changed);
            changed.sort_all_objects();
            let mut bytes = serde_json::to_vec(&changed).unwrap();
            bytes.push(b'\n');
            assert!(decode(&bytes).is_err());
        }
        let mut noncanonical = fixture();
        noncanonical.insert(0, b' ');
        assert!(decode(&noncanonical).is_err());
        assert!(decode(&vec![b' '; MAX_SERVICE_CONFIG_BYTES + 1]).is_err());
    }

    #[test]
    fn explicit_https_ports_match_the_published_schema_boundary() {
        let valid: Value = serde_json::from_slice(&fixture()).unwrap();
        for (port, accepted) in [
            ("1", true),
            ("443", false),
            ("65535", true),
            ("0", false),
            ("00001", false),
            ("65536", false),
            ("99999", false),
        ] {
            let mut host = valid.clone();
            host["mode"] = Value::String("host".into());
            host["database"]["adapter"] = Value::String("snapshot".into());
            host["http"]["adapter"] = Value::String("native".into());
            host["http"]["listen_origin"] =
                Value::String(format!("https://service.example:{port}"));
            host["http"]["tls_profile"] = Value::String("modern".into());
            host["secrets"]["password_pepper_ref"] = Value::String("auth.pepper".into());
            host["secrets"]["session_signing_key_ref"] = Value::String("auth.session".into());
            host["secrets"]["webhook_signing_key_ref"] = Value::String("webhook.signing".into());
            host["telemetry"]["adapter"] = Value::String("semaprax-json-events".into());
            host["telemetry"]["endpoint_origin"] =
                Value::String(format!("https://telemetry.example:{port}"));
            host.sort_all_objects();
            let mut bytes = serde_json::to_vec(&host).unwrap();
            bytes.push(b'\n');
            assert_eq!(decode(&bytes).is_ok(), accepted, "port {port}");
        }
    }

    #[test]
    fn telemetry_origins_must_be_collector_canonical() {
        let valid: Value = serde_json::from_slice(&fixture()).unwrap();
        let long_label = "a".repeat(64);
        for origin in [
            "https://telemetry.example:443".to_owned(),
            "https://telemetry..example".to_owned(),
            format!("https://{long_label}.example"),
        ] {
            let mut host = valid.clone();
            host["mode"] = Value::String("host".into());
            host["database"]["adapter"] = Value::String("snapshot".into());
            host["http"]["adapter"] = Value::String("native".into());
            host["http"]["listen_origin"] = Value::String("https://service.example".into());
            host["http"]["tls_profile"] = Value::String("modern".into());
            host["secrets"]["password_pepper_ref"] = Value::String("auth.pepper".into());
            host["secrets"]["session_signing_key_ref"] = Value::String("auth.session".into());
            host["secrets"]["webhook_signing_key_ref"] = Value::String("webhook.signing".into());
            host["telemetry"]["adapter"] = Value::String("semaprax-json-events".into());
            host["telemetry"]["endpoint_origin"] = Value::String(origin.clone());
            host.sort_all_objects();
            let mut bytes = serde_json::to_vec(&host).unwrap();
            bytes.push(b'\n');
            assert!(decode(&bytes).is_err(), "accepted {origin}");
        }
    }
}
