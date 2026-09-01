#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum CommandId {
    Check,
    Graph,
    ProjectImage,
    ProjectImageStore,
    ProjectImageLoad,
    ProjectImageVerify,
    ProjectSymbol,
    ProjectCandidatePreview,
    ProjectCandidateExport,
    ProjectCandidateRestore,
    SemanticCacheInit,
    SemanticCachePersist,
    SemanticCacheLoad,
    ProjectCandidatePersist,
    ProjectCandidateLoad,
    ProjectDraftPersist,
    ProjectDraftLoad,
    ProjectCandidateGitPublish,
    ServeWorkspace,
    ServeWorkspaceMcp,
    ServeImage,
    ServeCandidates,
    ServeTestCandidates,
    ServeDiagnostics,
    ServeDiagnosticsTested,
    Context,
    ContextBenchmark,
    Serve,
    QualityPlan,
    Doctor,
    New,
    ProjectScaffold,
    Build,
    Run,
    Test,
    Fmt,
    Patch,
    WorkspaceInit,
    SemanticWorkspaceInit,
    SemanticWorkspaceChangePreview,
    SemanticWorkspaceChangeEvidence,
    VerifySemanticWorkspaceChangeEvidence,
    ApplySemanticWorkspaceChangeEvidence,
    SemanticWorkspaceStructuralChangePreview,
    SemanticWorkspaceStructuralChangeEvidence,
    VerifySemanticWorkspaceStructuralChangeEvidence,
    ApplySemanticWorkspaceStructuralChangeEvidence,
    SemanticWorkspaceOperationsDerive,
    SemanticWorkspaceOperationsChangeProposal,
    SemanticWorkspaceOperationsEvidence,
    VerifySemanticWorkspaceOperationsEvidence,
    ApplySemanticWorkspaceOperationsEvidence,
    WorkspaceSnapshot,
    WorkspaceGraph,
    WorkspaceContext,
    WorkspaceImpact,
    WorkspaceReview,
    WorkspacePreview,
    WorkspaceApply,
    WorkspacePatchEvidence,
    VerifyWorkspacePatchEvidence,
    WorkspaceApplyWithEvidence,
    Impact,
    Properties,
    HygienicGen,
    Openapi,
    OpenapiCompat,
    CHeader,
    FreestandingObject,
    AbiReport,
    CapabilityManifest,
    PackageReport,
    PackageLock,
    PackageResolve,
    RegionReport,
    SimdReport,
    ProtocolCheck,
    Interpret,
    InterpretStrings,
    UiSchema,
    PluginManifest,
    CxxShim,
    Review,
    TargetEvidence,
    PatchEvidence,
    PatchEvidenceV2,
    VerifyPatchEvidence,
    VerifyPatchEvidenceV2,
    PatchWithEvidence,
    PatchWithEvidenceV2,
    Repairs,
    Repair,
    Version,
    VersionFlag,
    // Keep this final: the closed-catalog test uses its ordinal as the count.
    Help,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Public,
    Private,
}
#[derive(Clone, Copy, Debug)]
struct CommandSpec {
    id: CommandId,
    canonical: &'static str,
    aliases: &'static [&'static str],
    availability: Availability,
    global: bool,
    usages: &'static [&'static str],
}
static COMMANDS: &[CommandSpec] = &[
    CommandSpec { id: CommandId::Check, canonical: "check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax check [<file>|semaprax.toml|--manifest-path path] [--json]"] },
    CommandSpec { id: CommandId::Graph, canonical: "graph", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax graph <file>"] },
    CommandSpec { id: CommandId::ProjectImage, canonical: "project-image", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-image <manifest>"] },
    CommandSpec { id: CommandId::ProjectImageStore, canonical: "project-image-store", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-image-store <manifest> <store-root>"] },
    CommandSpec { id: CommandId::ProjectImageLoad, canonical: "project-image-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-image-load <store-root> <receipt.json> <expected-image-digest>"] },
    CommandSpec { id: CommandId::ProjectImageVerify, canonical: "project-image-verify", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-image-verify <manifest> <image.json>"] },
    CommandSpec { id: CommandId::ProjectSymbol, canonical: "project-symbol", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-symbol <manifest> <stable-id>"] },
    CommandSpec { id: CommandId::ProjectCandidatePreview, canonical: "project-candidate-preview", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-preview <manifest> <change.json>"] },
    CommandSpec { id: CommandId::ProjectCandidateExport, canonical: "project-candidate-export", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-export <manifest> <change.json>"] },
    CommandSpec { id: CommandId::ProjectCandidateRestore, canonical: "project-candidate-restore", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-restore <manifest> <capsule.json>"] },
    CommandSpec { id: CommandId::SemanticCacheInit, canonical: "semantic-cache-init", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-init <store-root>"] },
    CommandSpec { id: CommandId::SemanticCachePersist, canonical: "semantic-cache-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-persist <manifest> <store-root>"] },
    CommandSpec { id: CommandId::SemanticCacheLoad, canonical: "semantic-cache-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-load <store-root> <entry-digest>"] },
    CommandSpec { id: CommandId::ProjectCandidatePersist, canonical: "project-candidate-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-persist <manifest> <capsule.json> <store-root>"] },
    CommandSpec { id: CommandId::ProjectCandidateLoad, canonical: "project-candidate-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-load <store-root> <archive-digest> <candidate-digest>"] },
    CommandSpec { id: CommandId::ProjectDraftPersist, canonical: "project-draft-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-draft-persist <manifest> <draft-capsule.json> <store-root>"] },
    CommandSpec { id: CommandId::ProjectDraftLoad, canonical: "project-draft-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-draft-load <store-root> <archive-digest> <draft-digest>"] },
    CommandSpec { id: CommandId::ProjectCandidateGitPublish, canonical: "project-candidate-git-publish", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>"] },
    CommandSpec { id: CommandId::ServeWorkspace, canonical: "serve-workspace", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-workspace <manifest> <host-policy.json>"] },
    CommandSpec { id: CommandId::ServeWorkspaceMcp, canonical: "serve-workspace-mcp", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-workspace-mcp <manifest> <host-policy.json>"] },
    CommandSpec { id: CommandId::ServeImage, canonical: "serve-image", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-image <manifest>"] },
    CommandSpec { id: CommandId::ServeCandidates, canonical: "serve-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeTestCandidates, canonical: "serve-test-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-test-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnostics, canonical: "serve-diagnostics", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnosticsTested, canonical: "serve-diagnostics-tested", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics-tested <manifest>"] },
    CommandSpec { id: CommandId::Context, canonical: "context", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context <file> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters contracts,ownership,effects,types,targets,diagnostics,tests]"] },
    CommandSpec { id: CommandId::ContextBenchmark, canonical: "context-benchmark", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context-benchmark <manifest>"] },
    CommandSpec { id: CommandId::Serve, canonical: "serve", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve <file> [--max-request-bytes N]"] },
    CommandSpec { id: CommandId::QualityPlan, canonical: "quality-plan", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax quality-plan <quick|changed|full> [exact-changed-path ...]"] },
    CommandSpec { id: CommandId::Doctor, canonical: "doctor", aliases: &[], availability: Availability::Private, global: true, usages: &["semaprax doctor [--profile <id>] [--target native|web|all] [--json]"] },
    CommandSpec { id: CommandId::New, canonical: "new", aliases: &[], availability: Availability::Private, global: true, usages: &["semaprax new <destination> [--name project-name] [--template calculator]"] },
    CommandSpec { id: CommandId::ProjectScaffold, canonical: "project-scaffold", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-scaffold --name project-name [--template calculator]"] },
    CommandSpec { id: CommandId::Build, canonical: "build", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax build [<file>|semaprax.toml|--manifest-path path] [--target native|native-callable|web|wasm|npm|rust] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o path]"] },
    CommandSpec { id: CommandId::Run, canonical: "run", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax run <file>", "semaprax run [semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]"] },
    CommandSpec { id: CommandId::Test, canonical: "test", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax test [semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]"] },
    CommandSpec { id: CommandId::Fmt, canonical: "fmt", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fmt <file> [--check]"] },
    CommandSpec { id: CommandId::Patch, canonical: "patch", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::WorkspaceInit, canonical: "workspace-init", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-init <root> <path-set.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceInit, canonical: "semantic-workspace-init", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-init <root> <path-set.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceChangePreview, canonical: "semantic-workspace-change-preview", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-change-preview <root> <proposal.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceChangeEvidence, canonical: "semantic-workspace-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-change-evidence <root> <proposal.json>"] },
    CommandSpec { id: CommandId::VerifySemanticWorkspaceChangeEvidence, canonical: "verify-semantic-workspace-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::ApplySemanticWorkspaceChangeEvidence, canonical: "apply-semantic-workspace-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax apply-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceStructuralChangePreview, canonical: "semantic-workspace-structural-change-preview", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-structural-change-preview <root> <proposal.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceStructuralChangeEvidence, canonical: "semantic-workspace-structural-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-structural-change-evidence <root> <proposal.json>"] },
    CommandSpec { id: CommandId::VerifySemanticWorkspaceStructuralChangeEvidence, canonical: "verify-semantic-workspace-structural-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-semantic-workspace-structural-change-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::ApplySemanticWorkspaceStructuralChangeEvidence, canonical: "apply-semantic-workspace-structural-change-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax apply-semantic-workspace-structural-change-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceOperationsDerive, canonical: "semantic-workspace-operations-derive", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-operations-derive <root> <proposal.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceOperationsChangeProposal, canonical: "semantic-workspace-operations-change-proposal", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-operations-change-proposal <root> <proposal.json>"] },
    CommandSpec { id: CommandId::SemanticWorkspaceOperationsEvidence, canonical: "semantic-workspace-operations-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-workspace-operations-evidence <root> <proposal.json>"] },
    CommandSpec { id: CommandId::VerifySemanticWorkspaceOperationsEvidence, canonical: "verify-semantic-workspace-operations-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::ApplySemanticWorkspaceOperationsEvidence, canonical: "apply-semantic-workspace-operations-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax apply-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>"] },
    CommandSpec { id: CommandId::WorkspaceSnapshot, canonical: "workspace-snapshot", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-snapshot <root>"] },
    CommandSpec { id: CommandId::WorkspaceGraph, canonical: "workspace-graph", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-graph <root> <entry-module>"] },
    CommandSpec { id: CommandId::WorkspaceContext, canonical: "workspace-context", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-context <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]"] },
    CommandSpec { id: CommandId::WorkspaceImpact, canonical: "workspace-impact", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-impact <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]"] },
    CommandSpec { id: CommandId::WorkspaceReview, canonical: "workspace-review", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-review <root> <entry-module> <declaration|capability> <target>"] },
    CommandSpec { id: CommandId::WorkspacePreview, canonical: "workspace-preview", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-preview <root> <patch.wspatch>"] },
    CommandSpec { id: CommandId::WorkspaceApply, canonical: "workspace-apply", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-apply <root> <patch.wspatch>"] },
    CommandSpec { id: CommandId::WorkspacePatchEvidence, canonical: "workspace-patch-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-patch-evidence <root> <patch.wspatch>"] },
    CommandSpec { id: CommandId::VerifyWorkspacePatchEvidence, canonical: "verify-workspace-patch-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-workspace-patch-evidence <root> <patch.wspatch> <evidence.json>"] },
    CommandSpec { id: CommandId::WorkspaceApplyWithEvidence, canonical: "workspace-apply-with-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workspace-apply-with-evidence <root> <patch.wspatch> <evidence.json>"] },
    CommandSpec { id: CommandId::Impact, canonical: "impact", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax impact <file> <patch.spatch> [--depth N] [--max-bytes N] [--max-nodes N]"] },
    CommandSpec { id: CommandId::Properties, canonical: "properties", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax properties <file> [--max-cases N] [--max-functions N] [--max-bytes N] [--seed N]"] },
    CommandSpec { id: CommandId::HygienicGen, canonical: "hygienic-gen", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax hygienic-gen <file> [--templates default-constructor,field-accessors] [--max-bytes N]"] },
    CommandSpec { id: CommandId::Openapi, canonical: "openapi", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax openapi <file> --function <name|stable-id> ... [--max-bytes N]"] },
    CommandSpec { id: CommandId::OpenapiCompat, canonical: "openapi-compat", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax openapi-compat <base.json> <candidate.json> [--max-bytes N]"] },
    CommandSpec { id: CommandId::CHeader, canonical: "c-header", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax c-header <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-header]"] },
    CommandSpec { id: CommandId::FreestandingObject, canonical: "freestanding-object", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax freestanding-object <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::AbiReport, canonical: "abi-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax abi-report <file> --function name|stable-id[,...] [--function ...] [--max-bytes N]"] },
    CommandSpec { id: CommandId::CapabilityManifest, canonical: "capability-manifest", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax capability-manifest <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::PackageReport, canonical: "package-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax package-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::PackageLock, canonical: "package-lock", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax package-lock <subject.json>... [--max-bytes N]"] },
    CommandSpec { id: CommandId::PackageResolve, canonical: "package-resolve", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax package-resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::RegionReport, canonical: "region-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax region-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::SimdReport, canonical: "simd-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax simd-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::ProtocolCheck, canonical: "protocol-check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax protocol-check <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::Interpret, canonical: "interpret", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::InterpretStrings, canonical: "interpret-strings", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret-strings <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::UiSchema, canonical: "ui-schema", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax ui-schema <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::PluginManifest, canonical: "plugin-manifest", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax plugin-manifest <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::CxxShim, canonical: "cxx-shim", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax cxx-shim <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-fragment]"] },
    CommandSpec { id: CommandId::Review, canonical: "review", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax review <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::TargetEvidence, canonical: "target-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax target-evidence <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::PatchEvidence, canonical: "patch-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch-evidence <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::PatchEvidenceV2, canonical: "patch-evidence-v2", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch-evidence-v2 <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::VerifyPatchEvidence, canonical: "verify-patch-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-patch-evidence <file> <patch.spatch> <evidence.json>"] },
    CommandSpec { id: CommandId::VerifyPatchEvidenceV2, canonical: "verify-patch-evidence-v2", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify-patch-evidence-v2 <file> <patch.spatch> <evidence.json>"] },
    CommandSpec { id: CommandId::PatchWithEvidence, canonical: "patch-with-evidence", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch-with-evidence <file> <patch.spatch> <evidence.json>"] },
    CommandSpec { id: CommandId::PatchWithEvidenceV2, canonical: "patch-with-evidence-v2", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch-with-evidence-v2 <file> <patch.spatch> <evidence.json>"] },
    CommandSpec { id: CommandId::Repairs, canonical: "repairs", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax repairs <file> assign-function-id <automatic-function-id>"] },
    CommandSpec { id: CommandId::Repair, canonical: "repair", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax repair <file> <repair-id> --persistent-id <persistent-id>"] },
    CommandSpec { id: CommandId::Version, canonical: "version", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax version [--json]"] },
    CommandSpec { id: CommandId::VersionFlag, canonical: "--version", aliases: &["-V"], availability: Availability::Public, global: true, usages: &["semaprax --version"] },
    CommandSpec { id: CommandId::Help, canonical: "help", aliases: &["--help", "-h"], availability: Availability::Public, global: false, usages: &["semaprax help <command>"] },
];
fn available(spec: &CommandSpec, private: bool) -> bool {
    spec.availability == Availability::Public || private
}
fn selected(name: &str, private: bool) -> Option<&'static CommandSpec> {
    COMMANDS
        .iter()
        .find(|s| available(s, private) && (s.canonical == name || s.aliases.contains(&name)))
}
pub(crate) fn parse(name: &str, private: bool) -> Option<CommandId> {
    selected(name, private).map(|spec| spec.id)
}
pub(crate) fn global(private: bool) -> String {
    let mut out = String::from("SEMAPRAX — Meaning in. Verified machine code out.\n\nUsage:\n");
    for spec in COMMANDS
        .iter()
        .filter(|s| s.global && available(s, private))
    {
        for usage in spec.usages {
            if spec.canonical == "build" && !private {
                out.push_str(&usage.replace("|rust", ""));
            } else {
                out.push_str(usage);
            }
            out.push('\n');
        }
    }
    out
}
pub(crate) fn scoped(name: &str, private: bool) -> Option<String> {
    let spec = selected(name, private)?;
    let mut out = String::from("Usage:\n");
    for usage in spec.usages {
        out.push_str("  ");
        if spec.canonical == "build" && !private {
            out.push_str(&usage.replace("|rust", ""));
        } else {
            out.push_str(usage);
        }
        out.push('\n');
    }
    Some(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    const DISPATCHER_INVENTORY: &[&str] = &[
        "check",
        "graph",
        "project-image",
        "project-image-store",
        "project-image-load",
        "project-image-verify",
        "project-symbol",
        "project-candidate-preview",
        "project-candidate-export",
        "project-candidate-restore",
        "semantic-cache-init",
        "semantic-cache-persist",
        "semantic-cache-load",
        "project-candidate-persist",
        "project-candidate-load",
        "project-draft-persist",
        "project-draft-load",
        "project-candidate-git-publish",
        "serve-workspace",
        "serve-workspace-mcp",
        "serve-image",
        "serve-candidates",
        "serve-test-candidates",
        "serve-diagnostics",
        "serve-diagnostics-tested",
        "context",
        "context-benchmark",
        "serve",
        "quality-plan",
        "doctor",
        "new",
        "project-scaffold",
        "build",
        "run",
        "test",
        "fmt",
        "patch",
        "workspace-init",
        "semantic-workspace-init",
        "semantic-workspace-change-preview",
        "semantic-workspace-change-evidence",
        "verify-semantic-workspace-change-evidence",
        "apply-semantic-workspace-change-evidence",
        "semantic-workspace-structural-change-preview",
        "semantic-workspace-structural-change-evidence",
        "verify-semantic-workspace-structural-change-evidence",
        "apply-semantic-workspace-structural-change-evidence",
        "semantic-workspace-operations-derive",
        "semantic-workspace-operations-change-proposal",
        "semantic-workspace-operations-evidence",
        "verify-semantic-workspace-operations-evidence",
        "apply-semantic-workspace-operations-evidence",
        "workspace-snapshot",
        "workspace-graph",
        "workspace-context",
        "workspace-impact",
        "workspace-review",
        "workspace-preview",
        "workspace-apply",
        "workspace-patch-evidence",
        "verify-workspace-patch-evidence",
        "workspace-apply-with-evidence",
        "impact",
        "properties",
        "hygienic-gen",
        "openapi",
        "openapi-compat",
        "c-header",
        "freestanding-object",
        "abi-report",
        "capability-manifest",
        "package-report",
        "package-lock",
        "package-resolve",
        "region-report",
        "simd-report",
        "protocol-check",
        "interpret",
        "interpret-strings",
        "ui-schema",
        "plugin-manifest",
        "cxx-shim",
        "review",
        "target-evidence",
        "patch-evidence",
        "patch-evidence-v2",
        "verify-patch-evidence",
        "verify-patch-evidence-v2",
        "patch-with-evidence",
        "patch-with-evidence-v2",
        "repairs",
        "repair",
        "version",
        "--version",
        "-V",
        "help",
        "--help",
        "-h",
    ];
    #[test]
    fn catalog_and_dispatcher_are_closed_and_aliases_unique() {
        let mut catalog = std::collections::BTreeSet::new();
        let mut ids = vec![false; CommandId::Help as usize + 1];
        for s in COMMANDS {
            assert!(!ids[s.id as usize], "duplicate command id {:?}", s.id);
            ids[s.id as usize] = true;
            assert!(catalog.insert(s.canonical));
            for a in s.aliases {
                assert!(catalog.insert(a));
            }
        }
        let dispatcher: std::collections::BTreeSet<_> =
            DISPATCHER_INVENTORY.iter().copied().collect();
        assert_eq!(dispatcher.len(), DISPATCHER_INVENTORY.len());
        assert_eq!(catalog, dispatcher);
        assert!(ids.into_iter().all(|present| present));
    }
}
