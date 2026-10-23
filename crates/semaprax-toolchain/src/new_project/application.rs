//! Explicit full-host fresh installation, reusing the scaffold publisher.
use super::*;
use semaprax_native_rust_interop_platform as platform;

#[path = "application/held_input_path.rs"]
mod held_input_path;

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
    read_bound_with_hook(path, maximum, &|_| Ok(()))
}

fn selected_input_path(path: &Path) -> Result<PathBuf, NewProjectFailure> {
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(failure("starter input parent traversal is not admitted"));
    }
    std::path::absolute(path)
        .map_err(|error| failure(format!("cannot resolve starter input: {error}")))
}

fn read_bound_with_hook(
    path: &Path,
    maximum: usize,
    before_open: &dyn Fn(&Path) -> Result<(), NewProjectFailure>,
) -> Result<Vec<u8>, NewProjectFailure> {
    let parent_path = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| failure("starter input must name one regular file"))?;
    let held_parent_path = held_input_path::parent_spelling(parent_path)?;
    let parent = platform::hold_directory(&held_parent_path)
        .map_err(|_| failure("cannot hold starter input parent"))?;
    // This early inspection is a convenience refusal only. The actual file
    // classification and no-follow/nonblocking open belong to the held owner.
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| failure(format!("cannot inspect starter input: {e}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(failure("starter input must be a regular non-symlink file"));
    }
    before_open(path)?;
    let file = platform::hold_regular_file_bounded(&parent, name, maximum)
        .map_err(|_| failure("starter input is not a bounded held regular non-symlink file"))?;
    let bytes = platform::read_exact(&file, maximum)
        .map_err(|_| failure("starter input changed while reading"))?;
    platform::recheck_regular_file_named_bounded(&parent, name, &file, maximum)
        .map_err(|_| failure("starter input binding changed while reading"))?;
    if !platform::same_directory_path(&parent, parent_path)
        .map_err(|_| failure("starter input parent changed"))?
    {
        return Err(failure("starter input parent binding changed"));
    }
    Ok(bytes)
}

pub(super) fn run(arguments: &[String]) -> Result<PathBuf, NewProjectFailure> {
    run_with_input_hook(arguments, &|_| Ok(()))
}

fn run_with_input_hook(
    arguments: &[String],
    before_open: &dyn Fn(&Path) -> Result<(), NewProjectFailure>,
) -> Result<PathBuf, NewProjectFailure> {
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
    // Resolve each selected spelling once, without following input symlinks.
    // Reject ParentDir before platform-specific absolute path normalization.
    let plan_path = selected_input_path(&plan_path)?;
    let plan = read_bound_with_hook(&plan_path, 16_384, before_open)?;
    let activation = activation
        .map(|(path, file)| selected_input_path(&file).map(|file| (path, file)))
        .transpose()?;
    let command_bytes = activation
        .as_ref()
        .map(|(_, file)| read_bound_with_hook(file, 131_072, before_open))
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
    #[cfg(unix)]
    #[test]
    fn starter_plan_and_activation_swap_to_symlink_or_fifo_refuses_before_staging() {
        for selected in ["plan.json", "command.spx"] {
            for replacement in ["symlink", "fifo"] {
                let parent = root();
                let plan = parent.join("plan.json");
                let activation = parent.join("command.spx");
                let outside = parent.join("outside");
                fs::write(&plan, b"{}").unwrap();
                fs::write(&activation, b"module app;\n").unwrap();
                fs::write(&outside, b"outside original").unwrap();
                let destination = parent.join("new");
                let arguments = [
                    destination.to_str().unwrap(),
                    "--from",
                    "not-opened-project",
                    "--codec-plan",
                    plan.to_str().unwrap(),
                    "--command-profile",
                    "source-command.v1",
                    "--command-path",
                    "src/app.spx",
                    "--command-source",
                    activation.to_str().unwrap(),
                ]
                .map(str::to_owned)
                .to_vec();
                let swapped = std::cell::Cell::new(false);
                let result = run_with_input_hook(&arguments, &|path| {
                    if path.file_name().unwrap() == std::ffi::OsStr::new(selected)
                        && !swapped.replace(true)
                    {
                        fs::remove_file(path).unwrap();
                        if replacement == "symlink" {
                            std::os::unix::fs::symlink(&outside, path).unwrap();
                        } else {
                            assert!(std::process::Command::new("mkfifo")
                                .arg(path)
                                .status()
                                .unwrap()
                                .success());
                        }
                    }
                    Ok(())
                });
                assert!(swapped.get());
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("bounded held regular non-symlink"));
                assert!(!destination.exists());
                assert!(fs::read_dir(&parent).unwrap().all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".semaprax-new-")));
                assert_eq!(fs::read(&outside).unwrap(), b"outside original");
                fs::remove_dir_all(parent).unwrap();
            }
        }
    }

    #[test]
    fn starter_input_held_regular_reader_preserves_exact_bytes_and_bounds() {
        let parent = root();
        let input = parent.join("plan.json");
        fs::write(&input, b"stable").unwrap();
        assert_eq!(read_bound(&input, 6).unwrap(), b"stable");
        assert!(read_bound(&input, 5).is_err());
        assert_eq!(fs::read(&input).unwrap(), b"stable");
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn starter_reader_refuses_parent_rebinding_after_the_held_open() {
        let parent = root();
        let held = parent.with_extension("held");
        let input = parent.join("plan.json");
        fs::write(&input, b"stable").unwrap();
        let result = read_bound_with_hook(&input, 6, &|_| {
            fs::rename(&parent, &held).unwrap();
            fs::create_dir(&parent).unwrap();
            fs::write(parent.join("plan.json"), b"forged").unwrap();
            Ok(())
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "starter input parent binding changed"
        );
        assert_eq!(fs::read(held.join("plan.json")).unwrap(), b"stable");
        assert_eq!(fs::read(&input).unwrap(), b"forged");
        fs::remove_dir_all(held).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn starter_initial_symlink_and_existing_destination_preserve_originals() {
        let parent = root();
        let outside = parent.join("outside");
        let alias = parent.join("plan.json");
        fs::write(&outside, b"outside original").unwrap();
        std::os::unix::fs::symlink(&outside, &alias).unwrap();
        assert!(read_bound(&alias, 64).is_err());
        let destination = parent.join("existing");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("authored"), b"do not overwrite").unwrap();
        let arguments = [
            destination.to_str().unwrap(),
            "--from",
            "not-opened-project",
            "--codec-plan",
            alias.to_str().unwrap(),
            "--command-profile",
            "source-command.v1",
        ]
        .map(str::to_owned)
        .to_vec();
        assert!(run(&arguments).is_err());
        assert_eq!(
            fs::read(destination.join("authored")).unwrap(),
            b"do not overwrite"
        );
        assert_eq!(fs::read(&outside).unwrap(), b"outside original");
        fs::remove_dir_all(parent).unwrap();
    }
}
