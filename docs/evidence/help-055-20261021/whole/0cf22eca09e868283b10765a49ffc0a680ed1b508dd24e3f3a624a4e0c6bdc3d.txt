//! Availability of mixed public/full-toolchain invocation shapes.
use super::{CommandId, CommandSpec};

pub(super) fn visible(spec: &CommandSpec, private: bool) -> &[&str] {
    if spec.id == CommandId::New && !private {
        &spec.usages[..1]
    } else {
        spec.usages
    }
}

#[cfg(test)]
mod tests {
    use super::super::{catalog, scoped};
    #[test]
    fn new_help_preserves_exact_public_and_full_grammar() {
        let public = "Usage:\n  semaprax new <destination> [--name project-name] [--template calculator|library|service|stdin-stream-text|stdin-stream-data|stdin-stream-collection-record|source-command-file-text]\n";
        let full = "  semaprax-full new <fresh-directory> --from <project> --codec-plan <plan-json> --command-profile <exact-profile> [--command-path <declared-path> --command-source <source-file>]\n";
        assert_eq!(scoped("new", false).unwrap(), public);
        assert_eq!(scoped("new", true).unwrap(), format!("{public}{full}"));
    }
    #[test]
    fn exhaustive_catalog_preserves_the_qualified_full_new_shape() {
        let public = catalog(false);
        let full = catalog(true);
        assert!(public.contains("\nsemaprax new <destination>"));
        assert!(full.contains("\nsemaprax new <destination>"));
        assert_eq!(public.matches("\nsemaprax-full new ").count(), 1);
        assert_eq!(full.matches("\nsemaprax-full new ").count(), 1);
    }
}
