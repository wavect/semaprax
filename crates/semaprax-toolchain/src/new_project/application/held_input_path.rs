//! Lexical Windows spelling for a held parent; never resolves filesystem aliases.
use std::borrow::Cow;
use std::path::Path;

pub(super) fn parent_spelling(path: &Path) -> Result<Cow<'_, Path>, super::NewProjectFailure> {
    #[cfg(any(windows, test))]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::UNC(_, _)) {
                return from_prefix(prefix.kind(), components)
                    .map(Cow::Owned)
                    .ok_or_else(|| {
                        super::failure("starter input parent spelling is not admitted")
                    });
            }
        }
    }
    Ok(Cow::Borrowed(path))
}

#[cfg(any(windows, test))]
fn from_prefix<'a>(
    prefix: std::path::Prefix<'a>,
    mut components: impl Iterator<Item = std::path::Component<'a>>,
) -> Option<std::path::PathBuf> {
    use std::ffi::OsString;
    use std::path::{Component, Prefix};
    let mut spelling = OsString::from(r"\\?\");
    match prefix {
        Prefix::Disk(drive) => spelling.push(format!("{}:", char::from(drive))),
        Prefix::UNC(server, share) => {
            spelling.push(r"UNC\");
            spelling.push(server);
            spelling.push(r"\");
            spelling.push(share);
        }
        _ => return None,
    }
    if components.next() != Some(Component::RootDir) {
        return None;
    }
    spelling.push(r"\");
    let mut separator = false;
    for component in components {
        let Component::Normal(name) = component else {
            return None;
        };
        if separator {
            spelling.push(r"\");
        }
        spelling.push(name);
        separator = true;
    }
    Some(spelling.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::{Component, Prefix};

    #[test]
    fn drive_prefix_and_separators_are_lexical_and_lossless() {
        let result = from_prefix(
            Prefix::Disk(b'C'),
            [
                Component::RootDir,
                Component::Normal(OsStr::new("世界")),
                Component::Normal(OsStr::new("plan")),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(result.as_os_str(), OsStr::new(r"\\?\C:\世界\plan"));
    }

    #[test]
    fn unc_prefix_keeps_the_exact_server_share_and_names() {
        let result = from_prefix(
            Prefix::UNC(OsStr::new("server"), OsStr::new("share")),
            [Component::RootDir, Component::Normal(OsStr::new("inputs"))].into_iter(),
        )
        .unwrap();
        assert_eq!(
            result.as_os_str(),
            OsStr::new(r"\\?\UNC\server\share\inputs")
        );
    }

    #[test]
    fn drive_relative_and_parent_traversal_are_refused() {
        assert!(from_prefix(
            Prefix::Disk(b'C'),
            [Component::Normal(OsStr::new("relative"))].into_iter()
        )
        .is_none());
        assert!(from_prefix(
            Prefix::Disk(b'C'),
            [Component::RootDir, Component::ParentDir].into_iter()
        )
        .is_none());
    }

    #[test]
    fn device_and_verbatim_prefixes_are_not_reinterpreted() {
        for prefix in [
            Prefix::DeviceNS(OsStr::new("device")),
            Prefix::VerbatimDisk(b'C'),
            Prefix::VerbatimUNC(OsStr::new("server"), OsStr::new("share")),
        ] {
            assert!(from_prefix(prefix, [Component::RootDir].into_iter()).is_none());
        }
    }

    #[test]
    fn existing_verbatim_parent_is_borrowed_without_rewriting() {
        let path = Path::new(r"\\?\C:\inputs");
        assert!(matches!(parent_spelling(path).unwrap(), Cow::Borrowed(p) if p == path));
    }
}
