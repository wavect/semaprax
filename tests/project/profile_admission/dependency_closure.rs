use semaprax::project::with_authenticated_project;

use super::{fixture, manifest, TESTS};

const V1: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"dependency-closure\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"compat.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"profile.tests\"]\n\n[exports]\nweb = [\"compat.main\"]\n";

#[test]
fn useful_data_v1_authored_nominal_boundary_is_still_rejected() {
    let app = r#"
module compat.app;

@id("compat.reader")
record Reader { @id("compat.reader.data") data: Bytes, }

@id("compat.hidden")
fn hidden(value: own Reader) -> Reader { value }

@id("compat.main")
fn main() -> i64 { 0 }
"#;
    let fixture = fixture("authored-nominal", V1, app, TESTS);
    let error =
        with_authenticated_project(&manifest(&fixture.0), |snapshot| snapshot.check()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G172", "{error:?}");
    assert_eq!(error[0].message, "workspace module `compat.app` contains declarations outside the selected project linker profile");
}

#[test]
fn useful_data_v1_rejects_a_reached_json_cursor_dependency_member() {
    let manifest_text =
        format!("{V1}\n[dependencies]\nstd.data.json.write = \"=0.1.0\"\nstd.io = \"=0.1.0\"\n");
    let app = r#"
module compat.app;
use function @id("std.io.reader.from-bytes") from std.io as reader_from_bytes;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;
use function @id("std.data.json.write.quoted-into") from std.data.json.write as quoted_into;

@id("compat.main")
fn main() -> i64 {
    let source = [97u8];
    let destination = [0u8, 0u8, 0u8];
    let input = reader_from_bytes(bytes_copy(array_as_slice(source)));
    let output = writer_from_bytes(bytes_copy(array_as_slice(destination)));
    let rendered = quoted_into(input, output);
    0
}
"#;
    let fixture = fixture("reached-json-cursor", &manifest_text, app, TESTS);
    let error =
        with_authenticated_project(&manifest(&fixture.0), |snapshot| snapshot.check()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G172", "{error:?}");
    assert_eq!(error[0].message, "function target must be monomorphic with admitted value parameters, or borrowed byte-slice parameters and a scalar return");
}
