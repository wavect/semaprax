//! Exact archive admission evidence: header parsing, Darwin member modes,
//! oversize rejection, and output-insertion settlement.

use super::*;

#[cfg(any(target_os = "linux", windows))]
fn archive_prepared_for_test(
    archiver: &super::platform::Executable,
    cwd: &super::platform::Directory,
    input: &super::platform::RegularFile,
    prepared: super::platform::PreparedArchiveInvocation,
    process: &mut super::platform::PreparedProcessArena,
) -> Result<super::platform::RegularFile, Error> {
    super::platform::archive_prepared(archiver, cwd, input, prepared, process)
}

#[cfg(target_os = "macos")]
fn archive_prepared_for_test(
    archiver: &super::platform::Executable,
    cwd: &super::platform::Directory,
    input: &super::platform::RegularFile,
    prepared: super::platform::PreparedArchiveInvocation,
    process: &mut super::platform::PreparedProcessArena,
) -> Result<super::platform::RegularFile, Error> {
    super::platform::archive_prepared_settled(archiver, cwd, input, prepared, process)
        .map_err(|failure| failure.error)
}

#[test]
fn archive_header_parser_rejects_noncanonical_sizes_and_unknown_members() {
    use super::{
        archive_extended_name, archive_member_kind, archive_member_size,
        exact_archive_member_metadata, ArchiveMemberKind,
    };

    assert_eq!(archive_member_size(b"123       "), Ok(123));
    for invalid in [
        b"          ".as_slice(),
        b"+1        ".as_slice(),
        b"01x       ".as_slice(),
    ] {
        assert_eq!(archive_member_size(invalid), Err(Error::Invalid));
    }
    assert_eq!(
        archive_member_kind(b"module.o/       ", b"module.o"),
        Ok(ArchiveMemberKind::Input),
    );
    assert_eq!(
        archive_member_kind(b"/               ", b"module.o"),
        Ok(ArchiveMemberKind::GnuLinkerIndex),
    );
    assert_eq!(
        archive_member_kind(b"__.SYMDEF SORTED", b"module.o"),
        Ok(ArchiveMemberKind::BsdSortedLinkerIndex),
    );
    assert_eq!(
        archive_member_kind(b"foreign.o/      ", b"module.o"),
        Err(Error::Invalid),
    );
    assert_eq!(
        archive_member_kind(b"__.SYMDEF_EVIL  ", b"module.o"),
        Err(Error::Invalid),
    );
    assert_eq!(
        archive_member_kind(b"#1/8            ", b"module.o"),
        Ok(ArchiveMemberKind::Extended(8)),
    );
    assert_eq!(
        archive_extended_name(b"module.o\0\0\0\0"),
        Ok(b"module.o".as_slice()),
    );
    for invalid in [
        b"module.o".as_slice(),
        b"module.o\0\0\0".as_slice(),
        b"module.o\0x\0\0".as_slice(),
        b"module.o\0\0\0\0\0".as_slice(),
    ] {
        assert_eq!(archive_extended_name(invalid), Err(Error::Invalid));
    }

    #[cfg(target_os = "linux")]
    let metadata = [
        (ArchiveMemberKind::GnuLinkerIndex, b"0       ".as_slice()),
        (ArchiveMemberKind::Input, b"644     ".as_slice()),
    ];
    #[cfg(target_os = "macos")]
    let metadata = [
        (ArchiveMemberKind::Extended(20), b"100644  ".as_slice()),
        (ArchiveMemberKind::Extended(12), b"644     ".as_slice()),
    ];
    #[cfg(windows)]
    let metadata = [
        (ArchiveMemberKind::GnuLinkerIndex, b"0       ".as_slice()),
        (ArchiveMemberKind::Input, b"100666  ".as_slice()),
    ];
    for (kind, mode) in metadata {
        let mut header = [b' '; 60];
        #[cfg(not(windows))]
        header[16..28].copy_from_slice(b"0           ");
        #[cfg(windows)]
        header[16..28].copy_from_slice(b"-1          ");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            header[28..34].copy_from_slice(b"0     ");
            header[34..40].copy_from_slice(b"0     ");
        }
        header[40..48].copy_from_slice(mode);
        #[cfg(target_os = "linux")]
        let input_mode = 0o600;
        #[cfg(not(target_os = "linux"))]
        let input_mode = 0o644;
        assert_eq!(
            exact_archive_member_metadata(&header, kind, input_mode),
            Ok(())
        );
        for offset in [16, 28, 34, 40] {
            let mut hostile = header;
            hostile[offset] = b'9';
            assert_eq!(
                exact_archive_member_metadata(&hostile, kind, input_mode),
                Err(Error::Invalid),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut header = [b' '; 60];
        header[16..28].copy_from_slice(b"0           ");
        header[28..34].copy_from_slice(b"0     ");
        header[34..40].copy_from_slice(b"0     ");
        header[40..48].copy_from_slice(b"100644  ");
        assert_eq!(
            exact_archive_member_metadata(&header, ArchiveMemberKind::Extended(12), 0o100600),
            Ok(()),
        );
        for hostile_mode in [
            b"100600  ".as_slice(),
            b"644     ".as_slice(),
            b"100666  ".as_slice(),
        ] {
            let mut hostile = header;
            hostile[40..48].copy_from_slice(hostile_mode);
            assert_eq!(
                exact_archive_member_metadata(&hostile, ArchiveMemberKind::Extended(12), 0o100600,),
                Err(Error::Invalid),
            );
        }
    }
    #[cfg(windows)]
    {
        let mut old_synthetic_date = [b' '; 60];
        old_synthetic_date[16..28].copy_from_slice(b"0           ");
        old_synthetic_date[40..48].copy_from_slice(b"0       ");
        assert_eq!(
            exact_archive_member_metadata(
                &old_synthetic_date,
                ArchiveMemberKind::GnuLinkerIndex,
                0,
            ),
            Err(Error::Invalid),
        );
    }
    #[cfg(target_os = "linux")]
    {
        let mut nondeterministic = [b' '; 60];
        nondeterministic[16..28].copy_from_slice(b"0           ");
        nondeterministic[28..34].copy_from_slice(b"0     ");
        nondeterministic[34..40].copy_from_slice(b"0     ");
        nondeterministic[40..48].copy_from_slice(b"600     ");
        assert_eq!(
            exact_archive_member_metadata(&nondeterministic, ArchiveMemberKind::Input, 0o600),
            Err(Error::Invalid),
        );
    }
}

#[cfg(target_os = "macos")]
fn append_darwin_archive_member(
    archive: &mut Vec<u8>,
    extended_name: &[u8],
    mode: &[u8; 8],
    data: &[u8],
) {
    assert!(extended_name.len().is_multiple_of(4));
    let mut header = [b' '; 60];
    let encoded_name = format!("#1/{}", extended_name.len());
    header[..encoded_name.len()].copy_from_slice(encoded_name.as_bytes());
    header[16..28].copy_from_slice(b"0           ");
    header[28..34].copy_from_slice(b"0     ");
    header[34..40].copy_from_slice(b"0     ");
    header[40..48].copy_from_slice(mode);
    let size = extended_name.len() + data.len();
    let encoded_size = size.to_string();
    header[48..48 + encoded_size.len()].copy_from_slice(encoded_size.as_bytes());
    header[58..].copy_from_slice(b"`\n");
    archive.extend_from_slice(&header);
    archive.extend_from_slice(extended_name);
    archive.extend_from_slice(data);
    if size & 1 != 0 {
        archive.push(b'\n');
    }
}

#[cfg(target_os = "macos")]
fn synthetic_darwin_archive(input: &[u8], input_mode: &[u8; 8]) -> Vec<u8> {
    let mut archive = b"!<arch>\n".to_vec();
    append_darwin_archive_member(&mut archive, b"__.SYMDEF SORTED\0\0\0\0", b"100644  ", b"");
    append_darwin_archive_member(&mut archive, b"module.o\0\0\0\0", input_mode, input);
    archive
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_admission_accepts_only_the_two_deterministic_input_modes() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-mode-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let input_bytes = b"exact-mach-o-object";
    std::fs::write(root.join("module.o"), input_bytes).unwrap();
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();

    for (index, mode) in [b"600     ", b"100644  "].into_iter().enumerate() {
        let name = format!("accepted-{index}.a");
        std::fs::write(
            root.join(&name),
            synthetic_darwin_archive(input_bytes, mode),
        )
        .unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        super::platform::test_exact_archive_member(&archive, &input).unwrap();
    }

    for (index, mode) in [b"100600  ", b"644     ", b"100666  "]
        .into_iter()
        .enumerate()
    {
        let name = format!("rejected-{index}.a");
        std::fs::write(
            root.join(&name),
            synthetic_darwin_archive(input_bytes, mode),
        )
        .unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        assert_eq!(
            super::platform::test_exact_archive_member(&archive, &input),
            Err(Error::Invalid),
        );
    }

    drop((input, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_real_d_archive_is_exact_and_reproducible_across_tool_versions() {
    use std::ffi::OsStr;

    if std::env::var_os("SEMAPRAX_REQUIRE_DARWIN_REAL_ARCHIVE").as_deref() != Some(OsStr::new("1"))
    {
        return;
    }

    let source_root = std::env::temp_dir().join(format!(
        "semaprax-darwin-real-archive-source-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&source_root).unwrap();
    std::fs::write(
        source_root.join("module.c"),
        b"int semaprax_darwin_real_archive_probe(void){return 17;}\n",
    )
    .unwrap();
    let compile = std::process::Command::new(
        std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into()),
    )
    .current_dir(&source_root)
    .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-c"])
    .arg("module.c")
    .args(["-o", "module.o"])
    .output()
    .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let object = std::fs::read(source_root.join("module.o")).unwrap();
    std::fs::remove_dir_all(source_root).unwrap();

    let mut archives = Vec::new();
    let mut admitted_modes = Vec::new();
    for index in 0..2 {
        let root = std::env::temp_dir().join(format!(
            "semaprax-darwin-real-archive-{}-{index}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let directory = super::platform::hold_directory(&root).unwrap();
        let input =
            super::platform::write_file_new(&directory, OsStr::new("module.o"), &object, 0o600)
                .unwrap();
        let archiver =
            super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
                .unwrap();
        let prepared = super::platform::prepare_archive_invocation(
            OsStr::new("module.o"),
            OsStr::new("libsemaprax_native_rust_sdk.a"),
        )
        .unwrap();
        let mut process = super::platform::prepare_process_arena(1).unwrap();
        let archive = super::platform::archive_prepared_settled(
            &archiver,
            &directory,
            &input,
            prepared,
            &mut process,
        )
        .unwrap();
        let bytes = super::platform::read_exact(
            &archive,
            usize::try_from(super::SDK_ARCHIVE_MAX_BYTES).unwrap(),
        )
        .unwrap();
        let first_size =
            usize::try_from(super::archive_member_size(&bytes[56..66]).unwrap()).unwrap();
        let second_header = 68 + first_size + (first_size & 1);
        let mode = &bytes[second_header + 40..second_header + 48];
        assert!(
            mode == b"600     " || mode == b"100644  ",
            "Darwin -D emitted an unauthenticated input mode: {mode:?}",
        );
        admitted_modes.push(mode.to_vec());
        archives.push(bytes);
        drop((archive, archiver, input, directory, process));
        std::fs::remove_dir_all(root).unwrap();
    }
    assert_eq!(archives[0], archives[1]);
    assert_eq!(admitted_modes[0], admitted_modes[1]);
}

#[test]
fn sparse_oversize_archive_is_rejected_before_digest_io() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-oversize-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let name = OsStr::new("oversize.a");
    let file = std::fs::File::create(root.join(name)).unwrap();
    file.set_len(super::SDK_ARCHIVE_MAX_BYTES + 1).unwrap();
    drop(file);
    let directory = super::platform::hold_directory(&root).unwrap();
    let start = std::time::Instant::now();
    assert!(matches!(
        super::platform::test_hold_regular_file_name_bounded(
            &directory,
            name,
            super::SDK_ARCHIVE_MAX_BYTES,
        ),
        Err(Error::OutputLimit),
    ));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    drop(directory);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn archive_output_insertion_is_rejected_before_process_consumption_and_preserved() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-insertion-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    #[cfg(unix)]
    let (input_name, output_name) = (
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    );
    #[cfg(windows)]
    let (input_name, output_name) = (
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    );
    let input = super::platform::write_file_new(&cwd, input_name, b"owned-object", 0o600).unwrap();
    let foreign =
        super::platform::write_file_new(&cwd, output_name, b"foreign-must-survive", 0o600).unwrap();
    let executable_path = std::env::current_exe().unwrap();
    let executable = super::platform::hold_external_executable(&executable_path).unwrap();
    let prepared = super::platform::prepare_archive_invocation(input_name, output_name).unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert!(matches!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,),
        Err(Error::Exists)
    ));
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert_eq!(
        super::platform::read_exact(&foreign, 64).unwrap(),
        b"foreign-must-survive",
    );
    drop((foreign, input, executable, cwd, process));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn archive_nonregular_output_insertions_are_exactly_present_and_never_followed() {
    use std::ffi::OsStr;
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-nonregular-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let output = root.join("libsemaprax_native_rust_sdk.a");
    let foreign = root.join("foreign-target");
    std::fs::write(&foreign, b"foreign-must-survive").unwrap();
    symlink(&foreign, &output).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert_eq!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,).err(),
        Some(Error::Exists),
    );
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign-must-survive");
    std::fs::remove_file(&output).unwrap();
    std::fs::create_dir(&output).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    assert_eq!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,).err(),
        Some(Error::Exists),
    );
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_create_directory_reports_unheld_created_namespace() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-settled-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let parent = super::platform::hold_directory(&root).unwrap();
    let mut name = super::platform::prepare_relative_name_arena(13).unwrap();
    super::platform::set_relative_name_arena(&mut name, OsStr::new("unheld-stage")).unwrap();
    super::platform::test_inject_archive_scratch_open_failure(true);
    let failure = super::platform::create_directory_new_prepared_settled(&parent, &name, 0o700)
        .err()
        .expect("injected post-create open failure must fail");
    super::platform::test_inject_archive_scratch_open_failure(false);
    assert_eq!(failure.error, Error::Changed);
    assert!(failure.namespace_created);
    assert!(root.join("unheld-stage").is_dir());
    drop(parent);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_scratch_open_failure_reports_created_namespace_as_uncertain() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-scratch-open-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_reset_archive_later_actions();
    super::platform::test_inject_archive_scratch_open_failure(true);
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("injected post-mkdir open failure must fail");
    super::platform::test_inject_archive_scratch_open_failure(false);
    assert_eq!(
        failure.phase,
        super::DarwinArchiveFailurePhase::ScratchCreation
    );
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    assert_eq!(super::platform::test_archive_later_actions(), 0);
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert!(root.join("archive-tmp").is_dir());
    assert!(!root.join("libsemaprax_native_rust_sdk.a").exists());

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_every_post_process_failure_is_absorbing_and_preserves_namespace_bytes() {
    use super::platform::TestDarwinArchiveFailurePoint as Point;
    use super::DarwinArchiveFailurePhase as Phase;
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;

    let cases = [
        (Point::ProcessOutput, Phase::ProcessOutput, 0, true),
        (Point::ScratchCleanup, Phase::ScratchCleanup, 0, true),
        (
            Point::ArchiverRecheckBeforeHold,
            Phase::ArchiverRecheck,
            1,
            false,
        ),
        (
            Point::WorkingDirectoryRecheckBeforeHold,
            Phase::WorkingDirectoryRecheck,
            2,
            false,
        ),
        (Point::InputRecheckBeforeHold, Phase::InputRecheck, 3, false),
        (Point::OutputHold, Phase::OutputHold, 4, false),
        (Point::ExactArchive, Phase::ExactArchive, 5, false),
        (
            Point::ArchiverRecheckAfterAuthentication,
            Phase::ArchiverRecheck,
            6,
            false,
        ),
        (Point::LaunchPathRecheck, Phase::LaunchPathRecheck, 7, false),
        (
            Point::WorkingDirectoryRecheckAfterAuthentication,
            Phase::WorkingDirectoryRecheck,
            8,
            false,
        ),
        (
            Point::InputRecheckAfterAuthentication,
            Phase::InputRecheck,
            9,
            false,
        ),
        (Point::OutputRecheck, Phase::OutputRecheck, 10, false),
    ];

    for (point, phase, expected_actions, scratch_remains) in cases {
        let root = std::env::temp_dir().join(format!(
            "semaprax-darwin-archive-boundary-{}-{}-{point:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("module.c"),
            b"int semaprax_archive_boundary_probe(void){return 11;}\n",
        )
        .unwrap();
        let clang = std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into());
        let compile = std::process::Command::new(clang)
            .current_dir(&root)
            .args(["-c", "module.c", "-o", "module.o"])
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{point:?}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        std::fs::set_permissions(
            root.join("module.o"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        let root = std::fs::canonicalize(root).unwrap();
        let cwd = super::platform::hold_directory(&root).unwrap();
        let input = super::platform::test_hold_regular_file_name_bounded(
            &cwd,
            OsStr::new("module.o"),
            super::SDK_ARCHIVE_MAX_BYTES,
        )
        .unwrap();
        let executable =
            super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
                .unwrap();
        let prepared = super::platform::prepare_archive_invocation(
            OsStr::new("module.o"),
            OsStr::new("libsemaprax_native_rust_sdk.a"),
        )
        .unwrap();
        let mut process = super::platform::prepare_process_arena(1).unwrap();
        super::platform::test_reset_archive_later_actions();
        super::platform::test_inject_darwin_archive_failure(Some(point));
        let failure = super::platform::archive_prepared_settled(
            &executable,
            &cwd,
            &input,
            prepared,
            &mut process,
        )
        .err()
        .expect("injected post-process boundary must fail");
        super::platform::test_inject_darwin_archive_failure(None);
        assert_eq!(failure.phase, phase, "{point:?}");
        assert_eq!(
            failure.settlement,
            super::DarwinArchiveSettlement::Uncertain,
            "{point:?}"
        );
        assert_eq!(
            super::platform::test_archive_later_actions(),
            expected_actions,
            "{point:?} performed an action after the selected boundary"
        );
        assert_eq!(root.join("archive-tmp").is_dir(), scratch_remains);
        let archive = root.join("libsemaprax_native_rust_sdk.a");
        assert!(archive.is_file(), "{point:?} lost the archiver output");
        let preserved = root.join("authenticated-or-unsettled-archive");
        std::fs::rename(&archive, &preserved).unwrap();
        std::fs::write(&archive, b"foreign-must-survive").unwrap();
        assert!(preserved.is_file());
        assert_eq!(std::fs::read(&archive).unwrap(), b"foreign-must-survive");

        drop((input, executable, cwd, process));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_process_failure_is_absorbing_after_the_effect_boundary() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-settlement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    // The test image is held exactly but is not an archiver. Once it has been
    // invoked, its namespace effects cannot be inferred from its exit status.
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_reset_archive_later_actions();
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("non-archiver test image must fail");
    assert_eq!(failure.phase, super::DarwinArchiveFailurePhase::Process);
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    assert_eq!(super::platform::test_archive_later_actions(), 0);
    assert!(root.join("archive-tmp").is_dir());

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_authenticated_archive_is_preserved_on_later_rejection() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-owned-rejection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("module.c"),
        b"int semaprax_archive_settlement_probe(void){return 7;}\n",
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into());
    let compile = std::process::Command::new(clang)
        .current_dir(&root)
        .args(["-c", "module.c", "-o", "module.o"])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::test_hold_regular_file_name_bounded(
        &cwd,
        OsStr::new("module.o"),
        super::SDK_ARCHIVE_MAX_BYTES,
    )
    .unwrap();
    let executable =
        super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
            .unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_inject_archive_post_authentication_failure(true);
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("injected post-authentication rejection must fail");
    super::platform::test_inject_archive_post_authentication_failure(false);
    assert_eq!(
        failure.phase,
        super::DarwinArchiveFailurePhase::ArchiverRecheck
    );
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    let archive = root.join("libsemaprax_native_rust_sdk.a");
    assert!(archive.is_file());
    assert!(!root.join("archive-tmp").exists());
    let preserved = root.join("authenticated-archive-preserved");
    std::fs::rename(&archive, &preserved).unwrap();
    std::fs::write(&archive, b"foreign-must-survive").unwrap();
    assert!(preserved.is_file());
    assert_eq!(std::fs::read(&archive).unwrap(), b"foreign-must-survive");

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn unix_archive_preparation_is_fixed_to_one_sdk_object() {
    use std::ffi::OsStr;

    for (input, output) in [
        ("../module.o", "libsemaprax_native_rust_sdk.a"),
        ("module.o", "../libsemaprax_native_rust_sdk.a"),
        ("foreign.o", "libsemaprax_native_rust_sdk.a"),
        ("module.o", "foreign.a"),
    ] {
        assert!(matches!(
            super::platform::prepare_archive_invocation(OsStr::new(input), OsStr::new(output),),
            Err(Error::Invalid)
        ));
    }
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    assert!(super::platform::prepared_archive_owned_capacity(&prepared) > 0);
    let arguments = super::platform::test_prepared_archive_arguments(&prepared);
    #[cfg(target_os = "linux")]
    assert_eq!(
        arguments,
        [
            b"rcsD".as_slice(),
            b"libsemaprax_native_rust_sdk.a",
            b"module.o"
        ]
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        arguments,
        [
            b"-static".as_slice(),
            b"-D",
            b"-o",
            b"libsemaprax_native_rust_sdk.a",
            b"module.o",
        ],
    );
}

#[cfg(windows)]
#[test]
fn windows_archive_preparation_is_fixed_to_one_sdk_object() {
    use std::ffi::OsStr;

    for (input, output) in [
        (r"..\module.obj", "semaprax_native_rust_sdk.lib"),
        ("module.obj", r"..\semaprax_native_rust_sdk.lib"),
        ("foreign.obj", "semaprax_native_rust_sdk.lib"),
        ("module.obj", "foreign.lib"),
    ] {
        assert!(matches!(
            super::platform::prepare_archive_invocation(OsStr::new(input), OsStr::new(output),),
            Err(Error::Invalid)
        ));
    }
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    )
    .unwrap();
    assert!(super::platform::prepared_archive_owned_capacity(&prepared) > 0);
    assert_eq!(
        super::platform::test_prepared_archive_arguments(&prepared),
        [
            "/NOLOGO",
            "/BREPRO",
            "/OUT:semaprax_native_rust_sdk.lib",
            "module.obj",
        ],
    );
}
