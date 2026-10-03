//! Closed owner signatures preserve general index refusals and pin the exact
//! receiver relation. Package authority belongs to the calling Project route.
use super::*;
impl RustApiIndex {
    /// Returns selected records only when every path exists and is supported.
    /// This is discovery admission; the selected stable compiler must still
    /// validate generated signatures and calls before foreign execution.
    pub fn select_supported<'a>(&'a self, paths: &[&str]) -> Result<Vec<&'a ApiItem>, IndexError> {
        if paths.is_empty() || paths.len() > MAX_ITEMS {
            return Err(IndexError::ItemUnavailable);
        }
        let mut selected = Vec::with_capacity(paths.len());
        let mut previous: Option<&str> = None;
        for path in paths {
            if previous.is_some_and(|value| value.as_bytes() >= path.as_bytes()) {
                return Err(IndexError::ItemUnavailable);
            }
            previous = Some(path);
            let item = self
                .items
                .binary_search_by(|item| item.path.as_str().cmp(path))
                .ok()
                .map(|index| &self.items[index])
                .filter(|item| {
                    item.support == Support::Supported
                        && item.visibility == Visibility::Public
                        && item.closure_complete
                })
                .ok_or(IndexError::ItemUnavailable)?;
            selected.push(item);
        }
        Ok(selected)
    }
    pub fn require_stable_compiler_identity(&self, selected_rustc: &str) -> Result<(), IndexError> {
        if self.stable_rustc_version != selected_rustc {
            return Err(IndexError::IdentityMismatch);
        }
        Ok(())
    }

    /// Rejects a prepared index when the selected stable target/features or
    /// package source have drifted, before any foreign call can be prepared.
    pub fn require_identity(
        &self,
        source_sha256: &str,
        target: &str,
        feature_digest: &str,
    ) -> Result<(), IndexError> {
        if self.package.source_sha256 != source_sha256
            || self.target != target
            || self.feature_digest != feature_digest
        {
            return Err(IndexError::IdentityMismatch);
        }
        Ok(())
    }

    /// Checks the package as well as target, enabled features, and source
    /// digest. Callers should do this before validating or preparing wrappers.
    pub fn require_package_identity(
        &self,
        name: &str,
        version: &str,
        source_sha256: &str,
        target: &str,
        feature_digest: &str,
    ) -> Result<(), IndexError> {
        if self.package.name != name || self.package.version != version {
            return Err(IndexError::IdentityMismatch);
        }
        self.require_identity(source_sha256, target, feature_digest)
    }

    pub fn require_cargo_alias_identity(&self, cargo_alias: &str) -> Result<(), IndexError> {
        let expected = self
            .package
            .renamed_from
            .as_deref()
            .unwrap_or(&self.package.name);
        if expected != cargo_alias {
            return Err(IndexError::IdentityMismatch);
        }
        Ok(())
    }

    /// Admit the exact closed construction shape needed by a future owner
    /// carrier without relabeling its generic `Result` as generally supported.
    /// The caller must still lower both arms through a typed, audited carrier.
    pub fn select_closed_owner_result(
        &self,
        path: &str,
        owner: &str,
        error: &str,
    ) -> Result<&ApiItem, IndexError> {
        if path.is_empty() || owner.is_empty() || error.is_empty() {
            return Err(IndexError::ItemUnavailable);
        }
        let item = self
            .items
            .binary_search_by(|item| item.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.items[index])
            .filter(|item| {
                item.kind == ItemKind::InherentMethod
                    && item.receiver == Receiver::None
                    && item.visibility == Visibility::Public
                    && item.generics.parameters.is_empty()
                    && item.generics.where_predicates.is_empty()
                    && item.support
                        == Support::Rejected {
                            reason: RejectionReason::IncompleteTypeClosure,
                        }
                    && item.signature
                        == format!("fn new(re: &str) -> core::result::Result<{owner}, {error}>")
                    && item.type_roots.iter().map(String::as_str).eq([
                        "core::result::Result",
                        error,
                        owner,
                    ])
                    && item.reachable_types.iter().map(String::as_str).eq([
                        "core::result::Result",
                        error,
                        owner,
                    ])
            })
            .ok_or(IndexError::ItemUnavailable)?;
        let type_record = |path: &str, kind: TypeRecordKind| {
            self.types.iter().any(|record| {
                record.path == path
                    && record.kind == kind
                    && record.visibility == Visibility::Public
                    && record.generics.parameters.is_empty()
                    && record.generics.where_predicates.is_empty()
            })
        };
        if !type_record(owner, TypeRecordKind::Struct) || !type_record(error, TypeRecordKind::Enum)
        {
            return Err(IndexError::ItemUnavailable);
        }
        Ok(item)
    }
    /// Two Url methods whose complete runtime meaning is implemented by the
    /// bounded owner and returned-view bridge, not by the scalar profile.
    pub fn select_closed_url_method(&self, path: &str) -> Result<&ApiItem, IndexError> {
        let (signature, receiver, roots): (&str, Receiver, &[&str]) = match path {
            "url::Url::parse" => (
                "fn parse(input: &str) -> core::result::Result<Self, url::ParseError>",
                Receiver::None,
                &["core::result::Result", "url::ParseError", "url::Url"],
            ),
            "url::Url::as_str" => ("fn as_str(&self) -> &str", Receiver::Shared, &["url::Url"]),
            _ => return Err(IndexError::ItemUnavailable),
        };
        let item = self
            .items
            .binary_search_by(|item| item.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.items[index])
            .filter(|item| {
                item.kind == ItemKind::InherentMethod
                    && item.visibility == Visibility::Public
                    && item.receiver == receiver
                    && item.signature == signature
                    && item.generics.parameters.is_empty()
                    && item.generics.where_predicates.is_empty()
                    && item
                        .type_roots
                        .iter()
                        .map(String::as_str)
                        .eq(roots.iter().copied())
                    && item
                        .reachable_types
                        .iter()
                        .map(String::as_str)
                        .eq(roots.iter().copied())
                    && if receiver == Receiver::None {
                        item.support
                            == Support::Rejected {
                                reason: RejectionReason::UnsupportedSignature,
                            }
                    } else {
                        item.support == Support::Supported && item.closure_complete
                    }
            })
            .ok_or(IndexError::ItemUnavailable)?;
        for (path, kind) in [
            ("url::Url", TypeRecordKind::Struct),
            ("url::ParseError", TypeRecordKind::Enum),
        ] {
            if !self.types.iter().any(|record| {
                record.path == path
                    && record.kind == kind
                    && record.visibility == Visibility::Public
                    && record.generics.parameters.is_empty()
                    && record.generics.where_predicates.is_empty()
            }) {
                return Err(IndexError::ItemUnavailable);
            }
        }
        Ok(item)
    }
}
