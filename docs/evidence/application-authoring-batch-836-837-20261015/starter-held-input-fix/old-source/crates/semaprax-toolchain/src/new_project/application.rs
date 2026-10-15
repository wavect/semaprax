//! Explicit full-host fresh installation, reusing the scaffold publisher.
use super::*;
use std::io::Read;

fn failure(message: impl Into<String>) -> NewProjectFailure {
    NewProjectFailure::creation(message)
}
fn diagnostic(errors: Vec<semaprax::diagnostic::Diagnostic>) -> NewProjectFailure {
    failure(
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "),
    )
}

fn read_bound(path: &Path, maximum: usize) -> Result<Vec<u8>, NewProjectFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| failure(format!("cannot inspect starter input: {e}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(failure("starter input must be a regular non-symlink file"));
    }
    let before = Handle::from_path(path).map_err(|e| failure(e.to_string()))?;
    let file = fs::File::open(path).map_err(|e| failure(e.to_string()))?;
    if Handle::from_file(file.try_clone().map_err(|e| failure(e.to_string()))?)
        .map_err(|e| failure(e.to_string()))?
        != before
    {
        return Err(failure("starter input changed while opening"));
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(e.to_string()))?;
    if bytes.len() > maximum
        || Handle::from_path(path).map_err(|e| failure(e.to_string()))? != before
    {
        return Err(failure("starter input changed or exceeds its byte bound"));
    }
    Ok(bytes)
}

pub(super) fn run(arguments: &[String]) -> Result<PathBuf, NewProjectFailure> {
    let usage = || {
        NewProjectFailure::invocation("new <fresh-directory> --from <project> --codec-plan <plan-json> --command-profile <exact-profile> [--command-path <declared-path> --command-source <source-file>]")
    };
    if arguments.is_empty()
        || arguments[0].is_empty()
        || arguments[0].starts_with('-')
        || arguments.len() % 2 != 1
    {
        return Err(usage());
    }
    let mut options = std::collections::BTreeMap::new();
    for pair in arguments[1..].as_chunks::<2>().0 {
        if ![
            "--from",
            "--codec-plan",
            "--command-profile",
            "--command-path",
            "--command-source",
        ]
        .contains(&pair[0].as_str())
            || pair[1].is_empty()
            || pair[1].starts_with('-')
            || options.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(usage());
        }
    }
    let original = PathBuf::from(options.get("--from").ok_or_else(usage)?);
    let plan_path = PathBuf::from(options.get("--codec-plan").ok_or_else(usage)?);
    let profile = *options.get("--command-profile").ok_or_else(usage)?;
    // Vocabulary closes before any filesystem authority; matching the exact
    // authenticated profile below cannot elevate a project to this selection.
    if ![
        "source-command.v1",
        "source-command.resource-output.v1",
        "source-command.selected-file.v1",
        "source-command.selected-file.resource-output.v1",
        "language-command-io.collection-record.v1",
    ]
    .contains(&profile)
    {
        return Err(usage());
    }
    let activation = match (
        options.get("--command-path"),
        options.get("--command-source"),
    ) {
        (None, None) => None,
        (Some(path), Some(file)) => Some(((*path).to_owned(), PathBuf::from(file))),
        _ => return Err(usage()),
    };
    let plan = read_bound(&plan_path, 16_384)?;
    let command_bytes = activation
        .as_ref()
        .map(|(_, file)| read_bound(file, 131_072))
        .transpose()?;
    let command = activation
        .as_ref()
        .zip(command_bytes.as_ref())
        .map(|((path, _), bytes)| {
            Ok::<_, NewProjectFailure>(project::ApplicationCommandSource {
                path: path.clone(),
                source: String::from_utf8(bytes.clone())
                    .map_err(|_| failure("command source is not UTF-8"))?,
            })
        })
        .transpose()?;
    let manifest = if original.is_dir() {
        original.join("semaprax.toml")
    } else {
        original
    };
    let starter = project::with_authenticated_project(&manifest, |snapshot| {
        project::derive_application_starter(
            &snapshot.retain_revision(),
            &plan,
            profile,
            command.as_ref(),
        )
    })
    .map_err(diagnostic)?;
    // Recheck every separately selected input before any stage is created.
    if read_bound(&plan_path, 16_384)? != plan {
        return Err(failure("codec plan changed before publication"));
    }
    if let Some((_, path)) = &activation {
        if Some(read_bound(path, 131_072)?) != command_bytes {
            return Err(failure("command source changed before publication"));
        }
    }
    let destination = PathBuf::from(&arguments[0]);
    let absolute = std::path::absolute(&destination).map_err(|e| failure(e.to_string()))?;
    let files = starter
        .files()
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    publish_files(
        &destination,
        &absolute,
        &files,
        None,
        &NoopWriteHook,
        &mut || STAGING_SERIAL.fetch_add(1, Ordering::Relaxed),
    )
}

pub(super) fn create_authority(
    parent: &Path,
    output: &std::ffi::OsStr,
    files: &[(&str, &[u8])],
    serial: &mut dyn FnMut() -> u64,
) -> Result<(NewProjectAuthority, PathBuf), NewProjectFailure> {
    let paths = files
        .iter()
        .skip(3)
        .map(|(path, _)| (*path).to_owned())
        .collect::<Vec<_>>();
    for _ in 0..MAX_STAGING_ATTEMPTS {
        let name = format!(".semaprax-new-{}-{}", std::process::id(), serial());
        if name
            .as_bytes()
            .eq_ignore_ascii_case(output.as_encoded_bytes())
        {
            continue;
        }
        match NewProjectAuthority::create_application(
            parent,
            output,
            std::ffi::OsStr::new(&name),
            &paths,
        ) {
            Ok(authority) => return Ok((authority, parent.join(name))),
            Err(NewProjectAuthorityError::StageExists) => continue,
            Err(error) => return Err(authority_failure("create staged application", error)),
        }
    }
    Err(failure(
        "cannot allocate a fresh same-parent application stage",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const FILES: [(&str, &[u8]); 5] = [
        ("README.md", b"readme"),
        ("AGENTS.md", b"agents"),
        ("semaprax.toml", b"manifest"),
        ("src/app.spx", b"app"),
        ("src/tests.spx", b"tests"),
    ];
    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "spx-application-publish-{}-{}",
            std::process::id(),
            STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    struct Fail(usize);
    impl WriteHook for Fail {
        fn before_write(&self, index: usize, _: &str) -> Result<(), String> {
            if index == self.0 {
                Err("selected write failure".to_owned())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn fresh_application_partial_writes_never_publish_or_overwrite() {
        let parent = root();
        for index in 0..FILES.len() {
            let destination = parent.join(format!("fail-{index}"));
            let error = publish_files(
                &destination,
                &destination,
                &FILES,
                None,
                &Fail(index),
                &mut || STAGING_SERIAL.fetch_add(1, Ordering::Relaxed),
            )
            .unwrap_err();
            assert!(error.to_string().contains("selected write failure"));
            assert!(!destination.exists());
            assert_eq!(
                fs::read_dir(&parent).unwrap().count(),
                0,
                "owned private stage must be cleaned"
            );
        }
        let destination = parent.join("complete");
        publish_files(
            &destination,
            &destination,
            &FILES,
            None,
            &NoopWriteHook,
            &mut || STAGING_SERIAL.fetch_add(1, Ordering::Relaxed),
        )
        .unwrap();
        assert!(publish_files(
            &destination,
            &destination,
            &FILES,
            None,
            &NoopWriteHook,
            &mut || STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
        )
        .is_err());
        for (path, bytes) in FILES {
            assert_eq!(fs::read(destination.join(path)).unwrap(), bytes);
        }
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn fresh_application_symlink_destinations_and_parents_are_refused() {
        let parent = root();
        let original = parent.join("original");
        fs::create_dir(&original).unwrap();
        let destination = parent.join("alias");
        std::os::unix::fs::symlink(&original, &destination).unwrap();
        assert!(publish_files(
            &destination,
            &destination,
            &FILES,
            None,
            &NoopWriteHook,
            &mut || 0
        )
        .is_err());
        let nested = destination.join("new");
        assert!(publish_files(&nested, &nested, &FILES, None, &NoopWriteHook, &mut || 0).is_err());
        assert_eq!(fs::read_dir(&original).unwrap().count(), 0);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn starter_unknown_duplicate_and_incomplete_profiles_refuse_before_input_reads() {
        for arguments in [
            vec![
                "fresh",
                "--from",
                "absent",
                "--codec-plan",
                "absent",
                "--command-profile",
                "unknown",
            ],
            vec![
                "fresh",
                "--from",
                "absent",
                "--codec-plan",
                "absent",
                "--command-profile",
                "source-command.v1",
                "--command-path",
                "src/app.spx",
            ],
            vec![
                "fresh",
                "--from",
                "absent",
                "--codec-plan",
                "absent",
                "--command-profile",
                "source-command.v1",
                "--from",
                "other",
            ],
        ] {
            let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(run(&arguments).unwrap_err().exit_code(), 2);
        }
    }
}
