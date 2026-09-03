//! Windows archive admission, C compile plans, and the real `lib.exe`
//! `/BREPRO` round trip with its live descendant-authority control.

#[cfg(windows)]
use super::*;

#[cfg(windows)]
fn append_windows_archive_member(archive: &mut Vec<u8>, name: &[u8], mode: &[u8; 8], data: &[u8]) {
    assert!(name.len() <= 16);
    let mut header = [b' '; 60];
    header[..name.len()].copy_from_slice(name);
    header[16..28].copy_from_slice(b"-1          ");
    header[40..48].copy_from_slice(mode);
    let encoded_size = data.len().to_string();
    header[48..48 + encoded_size.len()].copy_from_slice(encoded_size.as_bytes());
    header[58..].copy_from_slice(b"`\n");
    archive.extend_from_slice(&header);
    archive.extend_from_slice(data);
    if data.len() & 1 != 0 {
        archive.push(b'\n');
    }
}

#[cfg(windows)]
fn synthetic_windows_archive(input: &[u8], empty_longnames: bool) -> Vec<u8> {
    let mut archive = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut archive, b"/", b"0       ", b"");
    append_windows_archive_member(&mut archive, b"/", b"0       ", b"");
    if empty_longnames {
        append_windows_archive_member(&mut archive, b"//", b"0       ", b"");
    }
    append_windows_archive_member(&mut archive, b"module.obj/", b"100666  ", input);
    archive
}

#[cfg(windows)]
#[test]
fn windows_archive_admission_is_closed_over_the_two_brepro_layouts() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-archive-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let input_bytes = b"exact-coff-object";
    std::fs::write(root.join("module.obj"), input_bytes).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.obj")).unwrap();

    for (index, bytes) in [
        synthetic_windows_archive(input_bytes, false),
        synthetic_windows_archive(input_bytes, true),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("accepted-{index}.lib");
        std::fs::write(root.join(&name), bytes).unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        super::platform::test_exact_archive_member(&archive, &input).unwrap();
    }

    let mut hostile = Vec::new();

    let mut nonempty_longnames = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut nonempty_longnames, b"/", b"0       ", b"");
    append_windows_archive_member(&mut nonempty_longnames, b"/", b"0       ", b"");
    append_windows_archive_member(&mut nonempty_longnames, b"//", b"0       ", b"module.obj\0");
    append_windows_archive_member(
        &mut nonempty_longnames,
        b"module.obj/",
        b"100666  ",
        input_bytes,
    );
    hostile.push(nonempty_longnames);

    let mut indirect_name = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut indirect_name, b"/", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"/", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"//", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"/0", b"100666  ", input_bytes);
    hostile.push(indirect_name);

    let mut hybrid = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut hybrid, b"/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"/<HYBRIDMAP>/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"module.obj/", b"100666  ", input_bytes);
    hostile.push(hybrid);

    let mut duplicate = synthetic_windows_archive(input_bytes, false);
    append_windows_archive_member(&mut duplicate, b"module.obj/", b"100666  ", input_bytes);
    hostile.push(duplicate);

    let mut foreign = synthetic_windows_archive(input_bytes, false);
    append_windows_archive_member(&mut foreign, b"foreign.obj/", b"100666  ", input_bytes);
    hostile.push(foreign);

    let mut reordered = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut reordered, b"module.obj/", b"100666  ", input_bytes);
    append_windows_archive_member(&mut reordered, b"/", b"0       ", b"");
    append_windows_archive_member(&mut reordered, b"/", b"0       ", b"");
    hostile.push(reordered);

    for (index, bytes) in hostile.into_iter().enumerate() {
        let name = format!("rejected-{index}.lib");
        std::fs::write(root.join(&name), bytes).unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        assert_eq!(
            super::platform::test_exact_archive_member(&archive, &input),
            Err(Error::Invalid),
        );
    }

    drop((input, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn windows_c_compile_plan_disables_incremental_linker_compatible_timestamps() {
    use std::ffi::OsStr;

    let prepared = super::platform::prepare_c_compile_invocation(
        "x86_64-pc-windows-msvc",
        OsStr::new("module.c"),
        2,
        false,
        33_554_432,
    )
    .unwrap();
    let expected = [
        "-std=c11",
        "-target",
        "x86_64-pc-windows-msvc",
        "-mno-incremental-linker-compatible",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-c",
        "module.c",
        "-o",
        "-",
    ];
    let (arguments, capacity) = super::platform::test_prepared_c_compile_arguments(&prepared);
    assert!(arguments.iter().map(String::as_str).eq(expected));
    assert_eq!(capacity, expected.len());
}

#[cfg(windows)]
#[test]
fn windows_c_compile_plan_selects_the_bounded_middle_optimization_exactly() {
    use std::ffi::OsStr;

    let prepared = super::platform::prepare_c_compile_invocation(
        "x86_64-pc-windows-msvc",
        OsStr::new("module.c"),
        1,
        false,
        33_554_432,
    )
    .unwrap();
    let (arguments, _) = super::platform::test_prepared_c_compile_arguments(&prepared);
    assert!(arguments.iter().any(|argument| argument == "-O1"));
    assert!(!arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "-O0" | "-O2")));
}

#[cfg(windows)]
#[test]
fn windows_real_brepro_archive_round_trips_through_exact_admission() {
    use sha2::{Digest as _, Sha256};
    use std::ffi::OsStr;
    use std::path::Path;

    let required = std::env::var_os("SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE").as_deref()
        == Some(OsStr::new("1"));
    let archiver = std::env::var_os("SEMAPRAX_ARCHIVER");
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let clang = std::env::var_os("CLANG");
    if required {
        assert!(
            archiver.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires SEMAPRAX_ARCHIVER"
        );
        assert!(
            vctools.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires SEMAPRAX_VCTOOLS"
        );
        assert!(
            clang.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires CLANG"
        );
    }
    let (Some(archiver), Some(vctools), Some(clang)) = (archiver, vctools, clang) else {
        return;
    };
    let archiver = std::path::PathBuf::from(archiver);
    let vctools = std::path::PathBuf::from(vctools);
    assert_eq!(
        archiver.strip_prefix(&vctools).unwrap(),
        Path::new(r"bin\Hostx64\x64\lib.exe"),
    );

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-real-archive-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::write(
        root.join("module.c"),
        b"int semaprax_archive_probe(void){return 7;}\n",
    )
    .unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let clang = super::platform::hold_external_executable(std::path::Path::new(&clang)).unwrap();
    let mut compile_process = super::platform::prepare_process_arena(2).unwrap();
    let compile = || {
        super::platform::prepare_c_compile_invocation(
            "x86_64-pc-windows-msvc",
            OsStr::new("module.c"),
            2,
            false,
            usize::try_from(super::SDK_ARCHIVE_MAX_BYTES).unwrap(),
        )
        .unwrap()
    };
    let first =
        super::platform::compile_c_prepared(&clang, &directory, compile(), &mut compile_process)
            .unwrap();
    let second =
        super::platform::compile_c_prepared(&clang, &directory, compile(), &mut compile_process)
            .unwrap();
    let digest = |bytes: &[u8]| {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        rendered
    };
    assert_eq!(
        first,
        second,
        "production Windows C objects differ: first_sha256={} second_sha256={}",
        digest(&first),
        digest(&second),
    );
    assert!(first.len() >= 8, "COFF object is shorter than its header");
    assert_eq!(
        &first[4..8],
        &[0, 0, 0, 0],
        "reproducible COFF TimeDateStamp must be zero"
    );
    let owned_input =
        super::platform::write_file_new(&directory, OsStr::new("module.obj"), &first, 0o600)
            .unwrap();
    let input_names = super::platform::prepare_discard_names([OsStr::new("module.obj")]).unwrap();
    let input = super::platform::transition_regular_file_to_external_read_prepared(
        &directory,
        &input_names,
        0,
        &owned_input,
    )
    .unwrap();
    drop(owned_input);
    let archiver_image = archiver;
    let archiver = super::platform::hold_external_executable(&archiver_image).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let archive =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let elapsed = start.elapsed();
    let archive = archive.unwrap_or_else(|error| {
        let captured =
            String::from_utf8_lossy(&super::platform::test_last_captured_stdout()).into_owned();
        let verbatim = root.as_os_str().to_string_lossy().into_owned();
        let plain_root = verbatim
            .strip_prefix(r"\\?\")
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or_else(|| verbatim.clone());
        let clean_root = std::path::PathBuf::from(&plain_root);
        let absolute_input = format!("{plain_root}\\module.obj");
        std::fs::copy(root.join("module.obj"), root.join("m2.obj")).ok();
        let rendered = |name: &str, attempt: &std::io::Result<std::process::Output>| {
            attempt
                .as_ref()
                .map(|output| {
                    format!(
                        "{name}:exit={} stdout={:?} stderr={:?}",
                        output.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr),
                    )
                })
                .unwrap_or_else(|error| format!("{name}:spawn-failed:{error:?}"))
        };
        let spawn = |cwd: &std::path::Path, arguments: &[&str]| {
            std::process::Command::new(&archiver_image)
                .current_dir(cwd)
                .args(arguments)
                .output()
        };
        let repo_cwd = std::env::current_dir().unwrap_or_else(|_| clean_root.clone());
        let probes = [
            rendered(
                "E_relative_brepro",
                &spawn(
                    &clean_root,
                    &[
                        "/NOLOGO",
                        "/BREPRO",
                        "/OUT:semaprax_probe_e.lib",
                        "module.obj",
                    ],
                ),
            ),
            rendered(
                "F_relative_nobrepro",
                &spawn(
                    &clean_root,
                    &["/NOLOGO", "/OUT:semaprax_probe_f.lib", "module.obj"],
                ),
            ),
            rendered(
                "G_copy_m2",
                &spawn(
                    &clean_root,
                    &["/NOLOGO", "/BREPRO", "/OUT:semaprax_probe_g.lib", "m2.obj"],
                ),
            ),
            rendered(
                "H_abs_from_repo_cwd",
                &spawn(
                    &repo_cwd,
                    &[
                        "/NOLOGO",
                        "/BREPRO",
                        &format!("/OUT:{plain_root}\\semaprax_probe_h.lib"),
                        absolute_input.as_str(),
                    ],
                ),
            ),
            rendered("I_cmd_type_read", &{
                let mut command = std::process::Command::new("cmd.exe");
                command
                    .current_dir(&clean_root)
                    .args(["/c", "cd", "&", "type", "module.obj"]);
                command.output()
            }),
        ];
        let plain_probe = probes.join(" | ");
        panic!(
            "{} strict_captured={captured:?}",
            windows_real_archive_failure_evidence(
                &directory,
                &input,
                OsStr::new("semaprax_native_rust_sdk.lib"),
                &root.join("semaprax_native_rust_sdk.lib"),
                error,
                elapsed,
                &plain_probe,
            )
        )
    });
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    super::platform::test_exact_archive_member(&archive, &input).unwrap();

    drop((
        archive,
        archiver,
        clang,
        input,
        directory,
        process,
        compile_process,
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
fn windows_real_archive_failure_evidence(
    directory: &super::platform::Directory,
    input: &super::platform::RegularFile,
    output_name: &std::ffi::OsStr,
    archive: &std::path::Path,
    error: Error,
    elapsed: std::time::Duration,
    plain_probe: &str,
) -> String {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    use std::io::{Read as _, Seek as _, SeekFrom};

    const MEMBER_CAP: usize = 8;
    const PREVIEW_CAP: usize = 64;
    const HASH_BYTE_CAP: u64 = super::SDK_ARCHIVE_MAX_BYTES;
    const DIAGNOSTIC_BYTE_CAP: usize = 16_384;

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(rendered, "{byte:02x}").unwrap();
        }
        rendered
    }

    fn escaped(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(bytes.len() * 4);
        for byte in bytes {
            match byte {
                b' '..=b'~' if *byte != b'\\' => rendered.push(char::from(*byte)),
                b'\\' => rendered.push_str("\\\\"),
                _ => write!(rendered, "\\x{byte:02x}").unwrap(),
            }
        }
        rendered
    }

    let metadata = std::fs::metadata(archive).ok();
    let exists = metadata.is_some();
    let length = metadata.as_ref().map_or(0, std::fs::Metadata::len);
    let exact_replay = if !exists {
        "absent".to_owned()
    } else {
        match super::platform::hold_regular_file(directory, output_name) {
            Ok(output) => match super::platform::test_exact_archive_member(&output, input) {
                Ok(()) => "replay_ok".to_owned(),
                Err(replay) => format!("output_replay_err:{replay:?}"),
            },
            Err(replay) => format!("output_replay_err:hold:{replay:?}"),
        }
    };
    let mut evidence = format!(
        "Windows real archive admission failed: error={error:?} elapsed_ms={} output_exists={exists} output_length={length} exact_replay={exact_replay}",
        elapsed.as_millis(),
    );
    evidence.push_str(" | ");
    evidence.push_str(plain_probe);
    let Ok(mut file) = std::fs::File::open(archive) else {
        return evidence;
    };

    let hashed_bytes = length.min(HASH_BYTE_CAP);
    let mut hasher = Sha256::new();
    let mut remaining = hashed_bytes;
    let mut buffer = [0_u8; 4096];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        let read = file.read(&mut buffer[..requested]).unwrap_or(0);
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let digest = hasher.finalize();
    write!(
        evidence,
        " hash_bytes={} sha256={}",
        hashed_bytes - remaining,
        hex(&digest)
    )
    .unwrap();

    if file.seek(SeekFrom::Start(0)).is_ok() {
        let mut magic = [0_u8; 8];
        if file.read_exact(&mut magic).is_ok() {
            write!(
                evidence,
                " magic_hex={} magic_escaped={}",
                hex(&magic),
                escaped(&magic)
            )
            .unwrap();
            let mut offset = 8_u64;
            for index in 0..MEMBER_CAP {
                let mut header = [0_u8; 60];
                if file.seek(SeekFrom::Start(offset)).is_err()
                    || file.read_exact(&mut header).is_err()
                {
                    break;
                }
                let parsed_size = super::archive_member_size(&header[48..58]);
                let payload_size = parsed_size.as_ref().copied().unwrap_or(0);
                let preview_length = usize::try_from(payload_size.min(PREVIEW_CAP as u64)).unwrap();
                let mut preview = [0_u8; PREVIEW_CAP];
                let preview_read = file.read(&mut preview[..preview_length]).unwrap_or(0);
                write!(
                    evidence,
                    "\nmember[{index}] offset={offset} header_hex={} name_hex={} name_escaped={} timestamp_hex={} timestamp_escaped={} owner_hex={} owner_escaped={} group_hex={} group_escaped={} mode_hex={} mode_escaped={} size_hex={} size_escaped={} end_hex={} end_escaped={} parsed_size={parsed_size:?} preview_len={preview_read} preview_hex={} preview_escaped={}",
                    hex(&header),
                    hex(&header[0..16]),
                    escaped(&header[0..16]),
                    hex(&header[16..28]),
                    escaped(&header[16..28]),
                    hex(&header[28..34]),
                    escaped(&header[28..34]),
                    hex(&header[34..40]),
                    escaped(&header[34..40]),
                    hex(&header[40..48]),
                    escaped(&header[40..48]),
                    hex(&header[48..58]),
                    escaped(&header[48..58]),
                    hex(&header[58..60]),
                    escaped(&header[58..60]),
                    hex(&preview[..preview_read]),
                    escaped(&preview[..preview_read]),
                )
                .unwrap();
                if parsed_size.is_err() {
                    break;
                }
                let Some(next) = offset
                    .checked_add(60)
                    .and_then(|value| value.checked_add(payload_size))
                    .and_then(|value| value.checked_add(payload_size & 1))
                else {
                    break;
                };
                if next <= offset || next > length {
                    break;
                }
                offset = next;
            }
        }
    }
    evidence.truncate(DIAGNOSTIC_BYTE_CAP);
    evidence
}

#[cfg(windows)]
#[test]
#[ignore = "negative control intentionally retains descendant file and directory authorities; CI requires exact AccessDenied with no publication"]
fn windows_live_descendant_authorities_deny_root_publish_without_later_action() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-mixed-inventory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let parent = super::platform::test_hold_directory_owned(&root).unwrap();
    let stage = super::platform::create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let source =
        super::platform::create_directory_new(&stage, OsStr::new("source"), 0o700).unwrap();
    let native =
        super::platform::create_directory_new(&stage, OsStr::new("native"), 0o700).unwrap();

    let root_files = [
        super::platform::write_file_new(&stage, OsStr::new("Cargo.toml"), b"cargo", 0o600).unwrap(),
        super::platform::write_file_new(&stage, OsStr::new("build.rs"), b"build", 0o600).unwrap(),
        super::platform::write_file_new(&stage, OsStr::new("sdk.json"), b"sdk", 0o600).unwrap(),
    ];
    let source_files = [
        super::platform::write_file_new(&source, OsStr::new("lib.rs"), b"lib", 0o600).unwrap(),
        super::platform::write_file_new(&source, OsStr::new("ffi.rs"), b"ffi", 0o600).unwrap(),
        super::platform::write_file_new(&source, OsStr::new("api.rs"), b"api", 0o600).unwrap(),
    ];
    let native_files = [
        super::platform::write_file_new(&native, OsStr::new("sdk.lib"), b"archive", 0o600).unwrap(),
        super::platform::write_file_new(
            &native,
            OsStr::new("descriptor.json"),
            b"descriptor",
            0o600,
        )
        .unwrap(),
        super::platform::write_file_new(&native, OsStr::new("manifest.json"), b"manifest", 0o600)
            .unwrap(),
    ];

    let authenticate = || {
        let mut root_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("Cargo.toml"),
                OsStr::new("build.rs"),
                OsStr::new("sdk.json"),
                OsStr::new("source"),
                OsStr::new("native"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut root_inventory,
            &stage,
            [&root_files[0], &root_files[1], &root_files[2]],
            [&source, &native],
        )
        .unwrap();
        let mut source_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("lib.rs"),
                OsStr::new("ffi.rs"),
                OsStr::new("api.rs"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut source_inventory,
            &source,
            [&source_files[0], &source_files[1], &source_files[2]],
            [],
        )
        .unwrap();
        let mut native_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("sdk.lib"),
                OsStr::new("descriptor.json"),
                OsStr::new("manifest.json"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut native_inventory,
            &native,
            [&native_files[0], &native_files[1], &native_files[2]],
            [],
        )
        .unwrap();
    };
    authenticate();

    let mut stage_name = super::platform::prepare_relative_name_arena(9).unwrap();
    super::platform::set_relative_name_arena(&mut stage_name, OsStr::new("stage")).unwrap();
    let identity_before =
        super::platform::test_publish_stage_identity_probe(&parent, &stage, &stage_name);
    let mut publish = super::platform::prepare_publish_directory(OsStr::new("published")).unwrap();
    assert_eq!(
        super::platform::publish_directory_new_prepared(
            &mut publish,
            &parent,
            &stage,
            &stage_name,
            OsStr::new("published"),
        ),
        Err(Error::Changed),
    );
    let statuses = super::platform::test_last_publish_statuses();
    let access_denied = windows_sys::Win32::Foundation::STATUS_ACCESS_DENIED;
    assert!(
        statuses.iter().all(|status| *status == access_denied),
        "live descendant authorities must deny every root rename attempt: {statuses:?}",
    );
    assert!(root.join("stage").is_dir());
    assert!(!root.join("published").exists());
    assert!(super::platform::same_directory_path(&stage, &root.join("stage")).unwrap());
    assert_eq!(
        super::platform::test_publish_stage_identity_probe(&parent, &stage, &stage_name),
        identity_before,
    );
    super::platform::recheck_directory(&parent).unwrap();
    super::platform::recheck_directory(&stage).unwrap();
    super::platform::recheck_directory(&source).unwrap();
    super::platform::recheck_directory(&native).unwrap();
    for file in root_files.iter().chain(&source_files).chain(&native_files) {
        super::platform::recheck_regular(file).unwrap();
    }
    authenticate();

    drop((
        publish,
        stage_name,
        native_files,
        source_files,
        root_files,
        native,
        source,
        stage,
        parent,
    ));
    std::fs::remove_dir_all(&root).unwrap();
}
