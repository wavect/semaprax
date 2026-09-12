//! The `std/packages.json` manifest loaded, cross-checked against `std/` on
//! disk, and typed as `PackageMetadata` for every other standard-library
//! harness module.

use super::{root, PACKAGES};

#[derive(Clone, Debug)]
pub(super) struct PackageMetadata {
    pub(super) directory: String,
    pub(super) module: String,
    pub(super) tier: String,
    pub(super) targets: Vec<String>,
    pub(super) status: String,
}

pub(super) fn packages() -> Vec<PackageMetadata> {
    let text = std::fs::read_to_string(root().join(PACKAGES)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], "semaprax.standard-library-packages.v1");
    let packages = value["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| PackageMetadata {
            directory: package["directory"].as_str().unwrap().to_owned(),
            module: package["module"].as_str().unwrap().to_owned(),
            tier: package["tier"].as_str().unwrap().to_owned(),
            targets: package["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|target| target.as_str().unwrap().to_owned())
                .collect(),
            status: package["status"].as_str().unwrap().to_owned(),
        })
        .collect::<Vec<_>>();
    assert!(!packages.is_empty(), "{PACKAGES} lists no packages");
    let mut directories = std::fs::read_dir(root().join("std"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("semaprax.toml").is_file())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    directories.sort();
    let mut listed = packages
        .iter()
        .map(|package| package.directory.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        listed, directories,
        "{PACKAGES} must list exactly the package directories under std/ in sorted order"
    );
    listed.dedup();
    assert_eq!(
        listed.len(),
        packages.len(),
        "{PACKAGES} lists a directory twice"
    );
    for package in &packages {
        assert!(
            matches!(
                package.tier.as_str(),
                "core"
                    | "alloc"
                    | "portable"
                    | "hosted"
                    | "browser"
                    | "embedded"
                    | "agent"
                    | "test"
            ),
            "{}: unknown portability tier `{}`",
            package.directory,
            package.tier
        );
        assert!(
            matches!(package.status.as_str(), "partial" | "implemented"),
            "{}: unknown status `{}`",
            package.directory,
            package.status
        );
        assert!(
            package.module.starts_with("std."),
            "{}: module `{}` is outside the std namespace",
            package.directory,
            package.module
        );
    }
    packages
}
