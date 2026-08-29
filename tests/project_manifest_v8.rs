use semaprax::project::{
    ProjectManifest, ProjectProfile, PROJECT_PROFILE_OWNED_DATA_API_V1, PROJECT_SCHEMA_V8,
};

const MANIFEST: &str = "schema = \"semaprax.project.v8\"\nname = \"frame-payload\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"frame_payload.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nweb_exports = [\"frame.payload\", \"frame.payload-maybe\", \"frame.payload-result\"]\ntests = [\"frame_payload.tests\"]\n";

#[test]
fn canonical_v8_manifest_is_exact_and_closed() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V8);
    assert_eq!(manifest.project_profile(), ProjectProfile::OwnedDataApiV1);
    assert_eq!(manifest.profile(), Some(PROJECT_PROFILE_OWNED_DATA_API_V1));
    assert_eq!(manifest.name(), "frame-payload");
    assert_eq!(manifest.package_version(), Some("0.1.0"));
    assert_eq!(manifest.entry(), "frame_payload.app");
    assert_eq!(manifest.sources().len(), 3);
    assert_eq!(manifest.web_exports().len(), 3);
    assert_eq!(manifest.test_module(), "frame_payload.tests");
    assert!(manifest.command().is_none());
    assert!(manifest.command_input().is_none());
    assert!(manifest.capabilities().is_empty());
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
}

#[test]
fn v8_rejects_shape_profile_order_count_and_capacity_drift() {
    let earlier_profiles = [
        "useful-text-consumer.v1",
        "useful-data.v1",
        "useful-data-command.v1",
        "useful-data-command.v2",
        "language-command-io.v1",
        "line-command-io.v1",
    ];
    for profile in earlier_profiles {
        assert!(ProjectManifest::parse(
            &MANIFEST.replace(PROJECT_PROFILE_OWNED_DATA_API_V1, profile,)
        )
        .is_err());
    }
    for malformed in [
        MANIFEST.replace("semaprax.project.v8", "semaprax.project.v7"),
        MANIFEST.replace(
            "name = \"frame-payload\"\nversion",
            "version = \"0.1.0\"\nname = \"frame-payload\"\nversion",
        ),
        MANIFEST.trim_end().to_owned(),
        MANIFEST.replace(
            "tests = [\"frame_payload.tests\"]\n",
            "command = \"frame.payload\"\ntests = [\"frame_payload.tests\"]\n",
        ),
        MANIFEST.replace(
            "sources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]",
            "sources = [\"src/app.spx\"]",
        ),
        MANIFEST.replace(
            "web_exports = [\"frame.payload\", \"frame.payload-maybe\", \"frame.payload-result\"]",
            "web_exports = []",
        ),
        MANIFEST.replace(
            "frame.payload-maybe\", \"frame.payload-result",
            "frame.payload-result\", \"frame.payload-maybe",
        ),
        MANIFEST.replace(
            "frame.payload-maybe\", \"frame.payload-result",
            "frame.payload-maybe\", \"frame.payload-maybe",
        ),
        MANIFEST.replace("name = \"frame-payload\"", "name = \"Frame Payload\""),
        MANIFEST.replace("version = \"0.1.0\"", "version = \"01.0.0\""),
        MANIFEST.replace(
            "entry = \"frame_payload.app\"",
            "entry = \"frame-payload.app\"",
        ),
        MANIFEST.replace("src/core.spx", "../core.spx"),
        MANIFEST.replace("frame.payload-result", "Frame Payload"),
        MANIFEST.replace('\n', "\r\n"),
    ] {
        assert!(
            ProjectManifest::parse(&malformed).is_err(),
            "accepted: {malformed}"
        );
    }

    let exports = (0..33)
        .map(|index| format!("\"frame.export-{index:02}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let over = MANIFEST.replace(
        "\"frame.payload\", \"frame.payload-maybe\", \"frame.payload-result\"",
        &exports,
    );
    assert!(ProjectManifest::parse(&over).is_err());
}

#[test]
fn v8_pins_source_and_export_minus_one_exact_and_plus_one_boundaries() {
    let paths = |count: usize| {
        (0..count)
            .map(|index| format!("\"src/{index:02}.spx\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (count, admitted) in [(1, false), (2, true), (16, true), (17, false)] {
        let candidate = MANIFEST.replace(
            "\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"",
            &paths(count),
        );
        assert_eq!(
            ProjectManifest::parse(&candidate).is_ok(),
            admitted,
            "source boundary {count}"
        );
    }

    let exports = |count: usize| {
        (0..count)
            .map(|index| format!("\"frame.export-{index:02}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (count, admitted) in [(0, false), (1, true), (32, true), (33, false)] {
        let candidate = MANIFEST.replace(
            "\"frame.payload\", \"frame.payload-maybe\", \"frame.payload-result\"",
            &exports(count),
        );
        assert_eq!(
            ProjectManifest::parse(&candidate).is_ok(),
            admitted,
            "export boundary {count}"
        );
    }
}

#[test]
fn v8_rejects_every_missing_reordered_and_extra_assignment() {
    let lines = MANIFEST.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 8);
    for missing in 0..lines.len() {
        let mut candidate = lines.clone();
        candidate.remove(missing);
        assert!(ProjectManifest::parse(&(candidate.join("\n") + "\n")).is_err());
    }
    for left in 0..lines.len() - 1 {
        let mut candidate = lines.clone();
        candidate.swap(left, left + 1);
        assert!(ProjectManifest::parse(&(candidate.join("\n") + "\n")).is_err());
    }
    let extra = MANIFEST.replace(
        "tests = [\"frame_payload.tests\"]\n",
        "capabilities = []\ntests = [\"frame_payload.tests\"]\n",
    );
    assert!(ProjectManifest::parse(&extra).is_err());
}

#[test]
fn earlier_schemas_reject_v8_profile_and_preserve_canonical_bytes() {
    let legacy = [
        "schema = \"semaprax.project.v1\"\nname = \"legacy\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v2\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v3\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v4\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ncapabilities = [\"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v5\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"useful-data-command.v2\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v6\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"language-command-io.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
        "schema = \"semaprax.project.v7\"\nname = \"legacy\"\nversion = \"1.0.0\"\nprofile = \"line-command-io.v1\"\nentry = \"legacy.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [\"legacy.value\"]\ncommand = \"legacy.value\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"legacy.tests\"]\n",
    ];
    for source in legacy {
        assert_eq!(
            ProjectManifest::parse(source).unwrap().to_canonical_toml(),
            source
        );
        if source.contains("profile = ") {
            assert!(ProjectManifest::parse(&source.replace(
                ProjectManifest::parse(source).unwrap().profile().unwrap(),
                PROJECT_PROFILE_OWNED_DATA_API_V1,
            ))
            .is_err());
        }
    }
}
