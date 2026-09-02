use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, FLAT_OWNED_RECORD_API_SCHEMA, PUBLIC_OWNED_UTF8_API_SCHEMA,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(label: &str, manifest: &str, app: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-retained-owned-api-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    let app = semaprax::format::canonical(&semaprax::parse(app, Path::new("src/app.spx")).unwrap());
    let tests =
        format!("module {label}.tests;\n\n@id(\"{label}.tests.main\")\nfn main() -> i64 {{ 0 }}\n");
    let tests =
        semaprax::format::canonical(&semaprax::parse(&tests, Path::new("src/tests.spx")).unwrap());
    std::fs::write(root.join("src/app.spx"), app).unwrap();
    std::fs::write(root.join("src/tests.spx"), tests).unwrap();
    Fixture(root.canonicalize().unwrap())
}

fn manifest(root: &Path) -> PathBuf {
    root.join("semaprax.toml")
}

#[test]
fn retained_v9_subject_exposes_only_the_replayed_flat_record_descriptor() {
    let fixture = fixture(
        "flat",
        "schema = \"semaprax.project.v9\"\nname = \"flat\"\nversion = \"1.0.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"flat.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"flat.make\"]\ntests = [\"flat.tests\"]\n",
        "module flat.app;\n\n@id(\"flat.packet\")\nrecord Packet {\n    @id(\"flat.packet.bytes\") bytes: Bytes,\n    @id(\"flat.packet.kind\") kind: i64,\n}\n\n@id(\"flat.make\")\nfn make(input: borrow Slice<u8>) -> Packet\n{\n    Packet { bytes: bytes_copy(input), kind: 7 }\n}\n\n@id(\"flat.app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let (retained, descriptor) = with_authenticated_project(&manifest(&fixture.0), |snapshot| {
        let descriptor = snapshot.flat_owned_record_api_descriptor()?;
        assert!(snapshot.public_api_descriptor().is_err());
        assert!(snapshot.owned_utf8_api_descriptor().is_err());
        Ok((snapshot.retain_revision(), descriptor))
    })
    .unwrap();
    assert!(String::from_utf8(descriptor.canonical_bytes())
        .unwrap()
        .contains(FLAT_OWNED_RECORD_API_SCHEMA));
    assert_eq!(
        retained.flat_owned_record_api_descriptor().unwrap(),
        descriptor
    );
}

#[test]
fn retained_v10_subject_exposes_only_the_replayed_owned_utf8_descriptor() {
    let fixture = fixture(
        "utf8",
        "schema = \"semaprax.project.v10\"\nname = \"utf8\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.greeting\"]\ntests = [\"utf8.tests\"]\n",
        "module utf8.app;\n\n@id(\"utf8.greeting\")\nfn greeting() -> string\n{\n    \"hello\\u{0}world\"\n}\n\n@id(\"utf8.app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let (retained, descriptor) = with_authenticated_project(&manifest(&fixture.0), |snapshot| {
        let descriptor = snapshot.owned_utf8_api_descriptor()?;
        assert!(snapshot.public_api_descriptor().is_err());
        assert!(snapshot.flat_owned_record_api_descriptor().is_err());
        Ok((snapshot.retain_revision(), descriptor))
    })
    .unwrap();
    assert_eq!(descriptor.schema(), PUBLIC_OWNED_UTF8_API_SCHEMA);
    assert_eq!(retained.owned_utf8_api_descriptor().unwrap(), descriptor);
}

#[test]
fn frozen_v8_descriptor_method_does_not_widen_to_the_new_profiles() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project/semaprax.toml");
    with_authenticated_project(&manifest, |snapshot| {
        snapshot.public_api_descriptor()?;
        assert!(snapshot.flat_owned_record_api_descriptor().is_err());
        assert!(snapshot.owned_utf8_api_descriptor().is_err());
        Ok(())
    })
    .unwrap();
}
