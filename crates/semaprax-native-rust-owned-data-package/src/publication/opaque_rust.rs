//! Held stable-rustc identity and signature compilation for opaque packages.
use super::*;

pub(crate) struct Compiler(platform::HeldDirectRustc);
impl Compiler {
    pub(crate) fn hold(
        authority: &PublicationAuthority,
        expected: &str,
    ) -> Result<Self, PackageError> {
        let configured = absolute_environment_path("RUSTC")?;
        let resolver = platform::prepare_tool_resolver("rustc", 32_768)
            .map_err(|_| tool("prepare tool resolver"))?;
        // Match the existing Phase B toolchain route: derive the discovery
        // launch spelling from the held executable, including configured
        // symlinks, without consulting PATH.
        let (held, resolver) = platform::resolve_and_hold_tool_reusing_prepared(
            resolver,
            Some(configured.as_os_str()),
            None,
        )
        .map_err(|_| tool("resolve configured rustc"))?;
        let launch = platform::tool_path(&held).to_owned();
        let discovery = platform::hold_rustc_discovery_prepared(resolver, OsStr::new(&launch))
            .map_err(|_| tool("hold configured rustc discovery"))?;
        drop(held);
        let plan = platform::prepare_process_arena_plan(3)
            .map_err(|_| tool("prepare discovery process arena"))?;
        let mut arena = platform::materialize_process_arena(plan)
            .map_err(|_| tool("materialize discovery process arena"))?;
        let discover = platform::prepare_sysroot_invocation(65_536)
            .map_err(|_| tool("prepare sysroot discovery"))?;
        let sysroot = platform::rustc_discovery_output_prepared(
            &discovery,
            &authority.parent,
            discover,
            &mut arena,
        )
        .map_err(|_| tool("execute sysroot discovery"))?;
        let mut direct = platform::hold_direct_rustc_prepared(discovery, sysroot.bytes())
            .map_err(|_| tool("hold direct rustc"))?;
        let reproduce = platform::prepare_sysroot_invocation(65_536)
            .map_err(|_| tool("prepare sysroot reproduction"))?;
        let sysroot = platform::direct_rustc_output_prepared(
            &direct,
            &authority.parent,
            reproduce,
            &mut arena,
        )
        .map_err(|_| tool("execute direct sysroot reproduction"))?;
        platform::direct_rustc_reproduces_sysroot(&mut direct, sysroot.bytes())
            .map_err(|_| tool("authenticate reproduced sysroot"))?;
        let version = platform::prepare_rustc_version_invocation(65_536)
            .map_err(|_| tool("prepare compiler version"))?;
        let version = platform::direct_rustc_version_prepared(
            &direct,
            &authority.parent,
            version,
            &mut arena,
        )
        .map_err(|_| tool("execute compiler version"))?;
        if std::str::from_utf8(version.bytes())
            .ok()
            .and_then(|s| s.lines().next())
            != Some(expected)
        {
            return Err(tool("selected compiler version mismatch"));
        }
        authority.recheck()?;
        Ok(Self(direct))
    }

    pub(crate) fn check(
        &self,
        authority: &PublicationAuthority,
        target: HostTarget,
        source: &[u8],
    ) -> Result<(), PackageError> {
        authority.recheck()?;
        let text = format!(
            ".semaprax-owner-rust-{}-{}",
            std::process::id(),
            STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
        );
        let name = platform::prepare_stage_name(OsStr::new(&text))
            .map_err(|_| PackageError::publication())?;
        let mut files =
            platform::prepare_discard_inventory([OsStr::new("lib.rs"), OsStr::new("checked.rlib")])
                .map_err(|_| PackageError::publication())?;
        let invocation = platform::prepare_rust_compile_invocation(
            target.triple(),
            OsStr::new("lib.rs"),
            OsStr::new("checked.rlib"),
        )
        .map_err(|_| tool("prepare Rust compilation"))?;
        let plan = platform::prepare_process_arena_plan(1)
            .map_err(|_| tool("prepare compilation process arena"))?;
        let mut arena = platform::materialize_process_arena(plan)
            .map_err(|_| tool("materialize compilation process arena"))?;
        let directory = platform::create_directory_new_prepared(&authority.parent, &name, 0o700)
            .map_err(|_| PackageError::publication())?;
        let result = (|| {
            if !platform::same_directory_path(&directory, &authority.parent_path.join(&text))
                .map_err(|_| PackageError::publication())?
            {
                return Err(PackageError::publication());
            }
            platform::write_file_new_prepared(&directory, &mut files, "lib.rs", source, 0o600)
                .map_err(|_| PackageError::publication())?;
            platform::transition_regular_file_to_external_read_prepared(
                &directory, &mut files, "lib.rs",
            )
            .map_err(|_| PackageError::publication())?;
            let object =
                platform::compile_rust_tool_prepared(&self.0, &directory, invocation, &mut arena)
                    .map_err(|_| tool("compile Rust signatures"))?;
            files
                .attach("checked.rlib", object)
                .map_err(|_| PackageError::publication())?;
            let mut scan = platform::prepare_inventory_exact(&files)
                .map_err(|_| PackageError::publication())?;
            platform::inventory_exact_prepared(&mut scan, &directory, &files)
                .map_err(|_| PackageError::publication())?;
            files
                .recheck(&["lib.rs", "checked.rlib"])
                .map_err(|_| PackageError::publication())?;
            Ok(())
        })();
        // On a failed compiler invocation its output has no returned held-file
        // authority. Retain the stage for reconciliation, rather than guessing.
        if result.is_err() {
            return result;
        }
        platform::discard_owned_stage_prepared(&authority.parent, &directory, &name, &files)
            .map_err(|_| PackageError::publication())?;
        authority.recheck()
    }
}

fn tool(stage: &'static str) -> PackageError {
    PackageError {
        kind: crate::PackageErrorKind::ToolConfiguration,
        detail: Some(stage),
    }
}
