use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    verify_execution_envelope, with_authenticated_project, ProjectExecutionOptions,
    ProjectExecutionOutcome, ProjectProfile, MAX_PROJECT_NPM_BUILD_BYTES,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(label: &str, manifest: &str, app: &str, tests: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-admission-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    let app = semaprax::format::canonical(&semaprax::parse(app, Path::new("src/app.spx")).unwrap());
    let tests =
        semaprax::format::canonical(&semaprax::parse(tests, Path::new("src/tests.spx")).unwrap());
    std::fs::write(root.join("src/app.spx"), app).unwrap();
    std::fs::write(root.join("src/tests.spx"), tests).unwrap();
    Fixture(root.canonicalize().unwrap())
}

fn manifest(root: &Path) -> PathBuf {
    root.join("semaprax.toml")
}

const TESTS: &str =
    "module profile.tests;\n\n@id(\"profile.tests.main\")\nfn main() -> i64 { 0 }\n";

#[test]
fn project_v9_phase_a_reaches_revision_npm_and_replayable_execution() {
    let fixture = fixture(
        "v9",
        "schema = \"semaprax.project.v9\"\nname = \"frame-info\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"frame.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"frame.info\"]\ntests = [\"profile.tests\"]\n",
        "module frame.app;\n\n@id(\"frame.info.type\")\nrecord FrameInfo {\n    @id(\"frame.info.payload\") payload: Bytes,\n    @id(\"frame.info.kind\") kind: i64,\n}\n\n@id(\"frame.info\")\nfn info(value: borrow Slice<u8>) -> FrameInfo\n{\n    FrameInfo { payload: bytes_copy(value), kind: 7 }\n}\n\n@id(\"frame.main\")\nfn main() -> i64 { 0 }\n",
        TESTS,
    );
    with_authenticated_project(&manifest(&fixture.0), |snapshot| {
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::FlatOwnedRecordApiV1
        );
        snapshot.check()?;
        let npm = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        let npm_envelope: serde_json::Value = serde_json::from_str(npm.envelope()).unwrap();
        assert_eq!(npm_envelope["schema"], "semaprax.project-npm-build.v8");
        let execution = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        assert_eq!(execution.outcome(), &ProjectExecutionOutcome::Returned(0));
        verify_execution_envelope(execution.envelope()).map_err(|error| vec![error])?;
        let error = match snapshot.build_web_inline(16 * 1024 * 1024) {
            Err(error) => error,
            Ok(_) => panic!("v9 pathless Web builds must select npm"),
        };
        assert_eq!(error[0].code, "SPX-W120");
        assert_eq!(
            error[0].message,
            "Project v9 pathless Web builds use build_npm_inline"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn project_v10_execution_replays_and_pathless_web_error_is_version_exact() {
    let fixture = fixture(
        "v10",
        "schema = \"semaprax.project.v10\"\nname = \"utf8-api\"\nversion = \"1.0.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.greeting\"]\ntests = [\"profile.tests\"]\n",
        "module utf8.app;\n\n@id(\"utf8.greeting\")\nfn greeting() -> string\n{\n    \"hello\"\n}\n\n@id(\"utf8.main\")\nfn main() -> i64 { 0 }\n",
        TESTS,
    );
    with_authenticated_project(&manifest(&fixture.0), |snapshot| {
        let execution = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        verify_execution_envelope(execution.envelope()).map_err(|error| vec![error])?;
        let error = match snapshot.build_web_inline(16 * 1024 * 1024) {
            Err(error) => error,
            Ok(_) => panic!("v10 pathless Web builds must select npm"),
        };
        assert_eq!(error[0].code, "SPX-W120");
        assert_eq!(
            error[0].message,
            "Project v10 pathless Web builds use build_npm_inline"
        );
        Ok(())
    })
    .unwrap();
}
