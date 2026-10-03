//! Pure, checked-HIR projection of one Semaprax record into a nominal Serde mirror.

use super::*;
use semaprax::hir::{ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind};

#[path = "serde_wire.rs"]
mod wire;
#[cfg(test)]
#[path = "serde_wire_tests.rs"]
mod wire_tests;

const MAX_MIRROR_FIELDS: usize = 16;
const MAX_MIRROR_SOURCE_BYTES: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerdeRecordProjection {
    pub record_id: String,
    pub mirror_name: String,
    pub rust_source: String,
}

/// Renders a nominal local Rust type from exactly one checked, nongeneric
/// Semaprax record. The output contains the field-by-field conversion used by
/// a later generated adapter; it neither publishes a package nor claims ABI or
/// layout compatibility with the Semaprax record.
pub fn prepare_serde_record_projection(
    program: &ResolvedProgram,
    record_id: &str,
) -> Result<SerdeRecordProjection, Diagnostic> {
    semaprax::hir::validate(program)?;
    let record = program
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == record_id)
        .ok_or_else(|| sdk_error("Serde projection record is absent from checked HIR"))?;
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(sdk_error("Serde projection requires a record declaration"));
    };
    if !record.type_parameters.is_empty() || fields.is_empty() || fields.len() > MAX_MIRROR_FIELDS {
        return Err(sdk_error(
            "Serde projection record is outside the bounded profile",
        ));
    }
    if fields.iter().any(|field| !rust_identifier(&field.name)) {
        return Err(sdk_error(
            "Serde projection record field is not Rust-portable",
        ));
    }
    let mirror_name = mirror_name(&record.id)?;
    let mut source = String::with_capacity(4096);
    source.push_str("#[derive(Clone,Debug,PartialEq)]\npub struct ");
    source.push_str(&record.name);
    source.push_str("{");
    for field in fields {
        source.push_str("pub ");
        source.push_str(&field.name);
        source.push(':');
        source.push_str(rust_type(&field.ty)?);
        source.push(',');
    }
    source.push_str("}\n#[derive(::serde::Serialize,::serde::Deserialize)]\npub struct ");
    source.push_str(&mirror_name);
    source.push_str("{");
    for field in fields {
        source.push_str("pub ");
        source.push_str(&field.name);
        source.push(':');
        source.push_str(rust_type(&field.ty)?);
        source.push(',');
    }
    source.push_str("}\nimpl core::convert::From<");
    source.push_str(&mirror_name);
    source.push_str("> for ");
    source.push_str(&record.name);
    source.push_str("{fn from(value:");
    source.push_str(&mirror_name);
    source.push_str(")->Self{Self{");
    for field in fields {
        source.push_str(&field.name);
        source.push_str(":value.");
        source.push_str(&field.name);
        source.push(',');
    }
    source.push_str("}}}\nimpl core::convert::From<");
    source.push_str(&record.name);
    source.push_str("> for ");
    source.push_str(&mirror_name);
    source.push_str("{fn from(value:");
    source.push_str(&record.name);
    source.push_str(")->Self{Self{");
    for field in fields {
        source.push_str(&field.name);
        source.push_str(":value.");
        source.push_str(&field.name);
        source.push(',');
    }
    source.push_str("}}}\npub fn deserialize_");
    source.push_str(&mirror_name.to_ascii_lowercase());
    source.push_str("(input:&str)->Result<");
    source.push_str(&record.name);
    source.push_str(",::serde_json::Error>{::serde_json::from_str::<");
    source.push_str(&mirror_name);
    source.push_str(">(input).map(core::convert::Into::into)}\n");
    source.push_str("pub fn serialize_");
    source.push_str(&mirror_name.to_ascii_lowercase());
    source.push_str("(value:&");
    source.push_str(&record.name);
    source.push_str(")->Result<String,::serde_json::Error>{let mirror=");
    source.push_str(&mirror_name);
    source.push_str("{");
    for field in fields {
        source.push_str(&field.name);
        source.push_str(":value.");
        source.push_str(&field.name);
        source.push_str(".clone(),");
    }
    source.push_str("};::serde_json::to_string(&mirror)}\n");
    source.push_str(&wire::render(&record.name, &mirror_name, fields)?);
    if source.len() > MAX_MIRROR_SOURCE_BYTES {
        return Err(sdk_error("Serde projection source exceeds its bound"));
    }
    Ok(SerdeRecordProjection {
        record_id: record.id.as_str().to_owned(),
        mirror_name,
        rust_source: source,
    })
}

fn rust_type(ty: &ResolvedType) -> Result<&'static str, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok("i64"),
        ResolvedType::Bool => Ok("bool"),
        ResolvedType::String => Ok("String"),
        ResolvedType::Bytes => Ok("Vec<u8>"),
        _ => Err(sdk_error("Serde projection field type is unsupported")),
    }
}

fn rust_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
        })
        && !matches!(
            value,
            "self" | "super" | "crate" | "type" | "match" | "mod" | "use"
        )
}

fn mirror_name(id: &semaprax::hir::DeclarationId) -> Result<String, Diagnostic> {
    let mut name = String::from("SpxMirror");
    for byte in id.as_str().bytes() {
        if byte.is_ascii_alphanumeric() {
            name.push(char::from(byte));
        }
    }
    if name.len() == "SpxMirror".len() || name.len() > 128 {
        return Err(sdk_error(
            "Serde projection record identity is not portable",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_NONCE: AtomicU64 = AtomicU64::new(0);

    struct FixtureDirectory(std::path::PathBuf);

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn checked_record_renders_nominal_serde_mirror_and_field_conversions() {
        let source = "module ri07.projection; @id(\"ri07.record\") record Projected { @id(\"ri07.record.id\") id: i64, @id(\"ri07.record.label\") label: string, @id(\"ri07.record.enabled\") enabled: bool, } @id(\"app.main\") fn main() -> i64 { 0 }";
        let program = semaprax::parse(source, Path::new("ri07-projection.spx")).unwrap();
        let resolved = semaprax::hir::resolve(&program).unwrap();
        let projection = prepare_serde_record_projection(&resolved, "ri07.record").unwrap();
        assert_eq!(projection.mirror_name, "SpxMirrorri07record");
        assert!(projection
            .rust_source
            .contains("#[derive(::serde::Serialize,::serde::Deserialize)]"));
        assert!(projection
            .rust_source
            .contains("::serde_json::from_str::<SpxMirrorri07record>"));
        assert!(projection
            .rust_source
            .contains("::serde_json::to_string(&mirror)"));
        assert!(projection
            .rust_source
            .contains("id:value.id,label:value.label,enabled:value.enabled,"));
    }

    #[test]
    fn nongeneric_scalar_record_controls_refuse_before_rendering() {
        let source = "module ri07.projection.bad; @id(\"ri07.bad\") record Bad<T> { @id(\"ri07.bad.value\") value: T, } @id(\"app.main\") fn main() -> i64 { 0 }";
        let program = semaprax::parse(source, Path::new("ri07-projection-bad.spx")).unwrap();
        let resolved = semaprax::hir::resolve(&program).unwrap();
        assert!(prepare_serde_record_projection(&resolved, "ri07.bad").is_err());
        assert!(prepare_serde_record_projection(&resolved, "ri07.absent").is_err());
    }

    #[test]
    fn generated_mirror_round_trips_with_real_serde_json_and_vec() {
        if std::env::var_os("SEMAPRAX_RI07_REAL_SERDE").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            eprintln!("RI-07 real Serde consumer is opt-in; set SEMAPRAX_RI07_REAL_SERDE=1");
            return;
        }
        let source = "module ri07.projection; @id(\"ri07.record\") record Projected { @id(\"ri07.record.id\") id: i64, @id(\"ri07.record.label\") label: string, @id(\"ri07.record.enabled\") enabled: bool, } @id(\"app.main\") fn main() -> i64 { 0 }";
        let parsed = semaprax::parse(source, Path::new("ri07-real-serde.spx")).unwrap();
        let resolved = semaprax::hir::resolve(&parsed).unwrap();
        let projection = prepare_serde_record_projection(&resolved, "ri07.record").unwrap();
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri07-serde-{}-{}",
            std::process::id(),
            FIXTURE_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _cleanup = FixtureDirectory(root.clone());
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"semaprax-ri07-serde-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\nserde = { version = \"=1.0.229\", features = [\"derive\"] }\nserde_json = \"=1.0.151\"\n",
        )
        .unwrap();
        let mut consumer = projection.rust_source;
        consumer.push_str(
            r##"
fn main() {
    let record = Projected { id: 7, label: String::from("bounded"), enabled: true };
    let json = serialize_spxmirrorri07record(&record).unwrap();
    let recovered = deserialize_spxmirrorri07record(&json).unwrap();
    assert_eq!(recovered, record);
    let mut values = Vec::<Projected>::new();
    values.push(recovered);
    assert_eq!(values.iter().filter(|item| item.enabled).count(), 1);
    let malformed: Result<Projected, serde_json::Error> =
        deserialize_spxmirrorri07record(r#"{"id":"bad","label":"bounded","enabled":true}"#);
    assert!(malformed.unwrap_err().is_data());
}
"##,
        );
        std::fs::write(root.join("src/main.rs"), consumer).unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let lock = Command::new(&cargo)
            .args(["generate-lockfile", "--offline", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "offline lockfile: {}",
            String::from_utf8_lossy(&lock.stderr)
        );
        let run = Command::new(&cargo)
            .args(["run", "--offline", "--locked", "--quiet", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(run.status.success(), "real Serde consumer: {stderr}");
    }
}
