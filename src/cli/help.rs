use std::fmt::Write as _;
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum CommandId {
    Check,
    Graph,
    Doc,
    Verify,
    Agent,
    Skills,
    Explain,
    Fix,
    Query,
    Change,
    Package,
    Add,
    Fetch,
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
    SemanticCacheEvict,
    SemanticCacheLifecycle,
    RetentionMetadataInventory,
    RetentionMetadataPlan,
    RetentionMetadataPersist,
    RetentionMetadataLoad,
    ProjectCandidatePersist,
    ProjectCandidateLoad,
    ProjectDraftPersist,
    ProjectDraftLoad,
    ProjectCandidateGitPublish,
    ServeWorkspace,
    ServeWorkspaceMcp,
    Service,
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
    NetworkRun,
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
    Lock,
    Resolve,
    PackageResolve,
    RegionReport,
    SimdReport,
    ProtocolCheck,
    Interpret,
    InterpretStrings,
    UiSchema,
    PluginManifest,
    CxxShim,
    CxxPackage,
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
    /// Retained for commands that only the unpublished toolchain can serve;
    /// none is catalogued today, since `doctor` moved into the root crate.
    #[allow(dead_code)]
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
    CommandSpec { id: CommandId::Check, canonical: "check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax check [<file>|<dir>|semaprax.toml|--manifest-path path] [--json]"] },
    CommandSpec { id: CommandId::Graph, canonical: "graph", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax graph <file>"] },
    CommandSpec { id: CommandId::Doc, canonical: "doc", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax doc <file> [--json]"] },
    CommandSpec { id: CommandId::Verify, canonical: "verify", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify <file> <patch.spatch> <evidence.json>", "semaprax verify <root> <patch.wspatch>|<proposal.json> <evidence.json>", "semaprax verify <definition.json> <profile.json> <graph.json>", "semaprax verify <manifest> <image.json>"] },
    CommandSpec { id: CommandId::Agent, canonical: "agent", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax agent inspect <definition.json> [--profile]", "semaprax agent run <definition.json> <task.json> <transcript.json> [--evidence|--trace]", "semaprax agent replay <definition.json> <task.json> <transcript.json> <evidence.json>"] },
    CommandSpec { id: CommandId::Skills, canonical: "skills", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax skills get <agent|language|graph|stdlib|packages|effects>"] },
    CommandSpec { id: CommandId::Explain, canonical: "explain", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax explain <SPX-CODE> [--json]"] },
    CommandSpec { id: CommandId::Fix, canonical: "fix", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fix --plan", "semaprax fix <file> assign-function-id <automatic-function-id> --plan"] },
    CommandSpec { id: CommandId::Query, canonical: "query", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax query --capabilities", "semaprax query <file|project> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]", "semaprax query <project> declarations [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--offset N] [--limit N] [--revision digest]", "semaprax query <project> symbol <stable-id> [--revision digest]", "semaprax query <project> context <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--revision digest]", "semaprax query <project> impact <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N] [--revision digest]", "semaprax query <project> available-operations <stable-id> [--revision digest]"] },
    CommandSpec { id: CommandId::Change, canonical: "change", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax change preview <project> rename-display-name <stable-id> <new-name> [--revision digest] [--evidence|--structural-diff]", "semaprax change rebase <base-project> rename-display-name <stable-id> <new-name> --onto <onto-project> [--revision digest] [--onto-revision digest]", "semaprax change merge <project> rename-display-name <left-id> <left-new-name> --with rename-display-name <right-id> <right-new-name> [--revision digest] --order <left-then-right|right-then-left>"] },
    CommandSpec { id: CommandId::Package, canonical: "package", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax package report <file> [--max-bytes N]", "semaprax package lock <subject.json>... [--max-bytes N]", "semaprax package resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::Add, canonical: "add", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax add <dir>|semaprax.toml <package> <range>"] },
    CommandSpec { id: CommandId::Fetch, canonical: "fetch", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fetch <cache-dir> <subject.json>..."] },
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
    CommandSpec { id: CommandId::SemanticCacheEvict, canonical: "semantic-cache-evict", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-evict <store-root> <entry-digest>"] },
    CommandSpec { id: CommandId::SemanticCacheLifecycle, canonical: "semantic-cache-lifecycle", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-lifecycle <manifest> <empty-store-root>"] },
    CommandSpec { id: CommandId::RetentionMetadataInventory, canonical: "retention-metadata-inventory", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax retention-metadata-inventory <declarations.json>"] },
    CommandSpec { id: CommandId::RetentionMetadataPlan, canonical: "retention-metadata-plan", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax retention-metadata-plan <inventory.json> <sequence> <max-subjects> <max-bytes> <protected-generations> <previous-checkpoint.json|none> <previous-digest|none> <previous-predecessor-digest|none>"] },
    CommandSpec { id: CommandId::RetentionMetadataPersist, canonical: "retention-metadata-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax retention-metadata-persist <store-root> <checkpoint.json> <checkpoint-digest> <previous-digest|none> <plan.json> <plan-digest>"] },
    CommandSpec { id: CommandId::RetentionMetadataLoad, canonical: "retention-metadata-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax retention-metadata-load <store-root> <checkpoint-digest> <previous-digest|none> <plan-digest>"] },
    CommandSpec { id: CommandId::ProjectCandidatePersist, canonical: "project-candidate-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-persist <manifest> <capsule.json> <store-root>"] },
    CommandSpec { id: CommandId::ProjectCandidateLoad, canonical: "project-candidate-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-load <store-root> <archive-digest> <candidate-digest>"] },
    CommandSpec { id: CommandId::ProjectDraftPersist, canonical: "project-draft-persist", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-draft-persist <manifest> <draft-capsule.json> <store-root>"] },
    CommandSpec { id: CommandId::ProjectDraftLoad, canonical: "project-draft-load", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-draft-load <store-root> <archive-digest> <draft-digest>"] },
    CommandSpec { id: CommandId::ProjectCandidateGitPublish, canonical: "project-candidate-git-publish", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>"] },
    CommandSpec { id: CommandId::ServeWorkspace, canonical: "serve-workspace", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-workspace <manifest> <host-policy.json>"] },
    CommandSpec { id: CommandId::ServeWorkspaceMcp, canonical: "serve-workspace-mcp", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-workspace-mcp <manifest> <host-policy.json>"] },
    CommandSpec { id: CommandId::Service, canonical: "service", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax service <project>"] },
    CommandSpec { id: CommandId::ServeImage, canonical: "serve-image", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-image <manifest>"] },
    CommandSpec { id: CommandId::ServeCandidates, canonical: "serve-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeTestCandidates, canonical: "serve-test-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-test-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnostics, canonical: "serve-diagnostics", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnosticsTested, canonical: "serve-diagnostics-tested", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics-tested <manifest>"] },
    CommandSpec { id: CommandId::Context, canonical: "context", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context <file|project> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters contracts,ownership,effects,types,targets,diagnostics,tests]"] },
    CommandSpec { id: CommandId::ContextBenchmark, canonical: "context-benchmark", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context-benchmark <manifest>"] },
    CommandSpec { id: CommandId::Serve, canonical: "serve", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve <file> [--max-request-bytes N]"] },
    CommandSpec { id: CommandId::QualityPlan, canonical: "quality-plan", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax quality-plan <quick|changed|full> [exact-changed-path ...]"] },
    CommandSpec { id: CommandId::Doctor, canonical: "doctor", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax doctor [--profile <id>] [--target native|web|all] [--json]"] },
    CommandSpec { id: CommandId::New, canonical: "new", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax new <destination> [--name project-name] [--template calculator|library]"] },
    CommandSpec { id: CommandId::ProjectScaffold, canonical: "project-scaffold", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-scaffold --name project-name [--template calculator|library] [--layout frozen|tables]"] },
    CommandSpec { id: CommandId::Build, canonical: "build", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax build <file> [--target native|native-callable|web|wasm] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o|--output path] [--json]", "semaprax build [<dir>|semaprax.toml|--manifest-path path] [--target native|web|wasm|npm|rust] [-o|--output path] [--json]"] },
    CommandSpec { id: CommandId::Run, canonical: "run", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax run <file> [--json] [--max-steps N] [--max-bytes N] [--native]", "semaprax run [<dir>|semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]"] },
    CommandSpec { id: CommandId::NetworkRun, canonical: "network-run", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax network-run [<dir>|semaprax.toml|--manifest-path path] --fixture fixture.json [--arg UTF8]... [--stdin path] [--max-steps N]"] },
    CommandSpec { id: CommandId::Test, canonical: "test", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax test [<dir>|semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]"] },
    CommandSpec { id: CommandId::Fmt, canonical: "fmt", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fmt <file>|<dir>|semaprax.toml [--check]"] },
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
    CommandSpec { id: CommandId::Lock, canonical: "lock", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax lock [<dir>|semaprax.toml] [--write|--verify|--compare <baseline.lock>|--emit-interface|--compare-interface <baseline.json>]"] },
    CommandSpec { id: CommandId::Resolve, canonical: "resolve", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax resolve [<dir>|semaprax.toml] --target <native64|wasm32> --cache <dir> [--write|--verify] [--max-bytes N]"] },
    CommandSpec { id: CommandId::RegionReport, canonical: "region-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax region-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::SimdReport, canonical: "simd-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax simd-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::ProtocolCheck, canonical: "protocol-check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax protocol-check <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::Interpret, canonical: "interpret", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::InterpretStrings, canonical: "interpret-strings", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret-strings <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::UiSchema, canonical: "ui-schema", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax ui-schema <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::PluginManifest, canonical: "plugin-manifest", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax plugin-manifest <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::CxxShim, canonical: "cxx-shim", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax cxx-shim <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-fragment]"] },
    CommandSpec { id: CommandId::CxxPackage, canonical: "cxx-package", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax cxx-package <file> --function name|stable-id[,...] [--function ...] [--max-bytes N]"] },
    CommandSpec { id: CommandId::Review, canonical: "review", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax review <file> <patch.spatch>", "semaprax review <project> <transaction.json> [--evidence]"] },
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
    CommandSpec { id: CommandId::Help, canonical: "help", aliases: &["--help", "-h"], availability: Availability::Public, global: false, usages: &["semaprax help <command>", "semaprax help all", "semaprax help diagnostic <SPX-code|codes>", "semaprax help language", "semaprax help language <topic|topics>", "semaprax help library", "semaprax help library <module|name|stable-id>", "semaprax help shapes", "semaprax help shapes <kind|stable-id|path#stable-id>"] },
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
pub(crate) fn unknown_diagnostic(name: &str, private: bool) -> String {
    match suggestion(name, private) {
        Some(candidate) => format!("unknown command `{name}`; did you mean `{candidate}`?\n\n"),
        None => format!("unknown command `{name}`\n\n"),
    }
}
fn suggestion(name: &str, private: bool) -> Option<&'static str> {
    if !name.is_ascii() || name.len() > 64 {
        return None;
    }
    let threshold = if name.len() <= 4 { 1 } else { 2 };
    let mut nearest = None;
    let mut nearest_distance = usize::MAX;
    let mut ambiguous = false;
    for spec in COMMANDS.iter().filter(|spec| available(spec, private)) {
        for candidate in std::iter::once(spec.canonical).chain(spec.aliases.iter().copied()) {
            if candidate.len() > 64 {
                continue;
            }
            let distance = edit_distance(name.as_bytes(), candidate.as_bytes());
            if distance < nearest_distance {
                nearest = Some(candidate);
                nearest_distance = distance;
                ambiguous = false;
            } else if distance == nearest_distance {
                ambiguous = true;
            }
        }
    }
    (nearest_distance > 0 && nearest_distance <= threshold && !ambiguous)
        .then_some(nearest)
        .flatten()
}
fn edit_distance(left: &[u8], right: &[u8]) -> usize {
    let mut previous = [0usize; 65];
    let mut current = [0usize; 65];
    for (index, slot) in previous.iter_mut().take(right.len() + 1).enumerate() {
        *slot = index;
    }
    for (left_index, left_byte) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_byte) in right.iter().enumerate() {
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + usize::from(left_byte != right_byte));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}
const BANNER: &str = "SEMAPRAX — Meaning in. Verified machine code out.\n";

/// The compiler-checked language card, printed by `semaprax help language` so
/// an agent or developer with only the installed binary can read the admitted
/// shapes, the diagnostics foreign habits trigger, and their fixes offline.
/// The bytes are the repository document; `tests/documentation.rs` checks its
/// code blocks against this compiler.
pub(crate) const LANGUAGE_REFERENCE: &str = include_str!("../../docs/AGENT-QUICK-REFERENCE.md");
/// The deterministic diagnostic index generated and pinned by the quick
/// reference documentation gate.
const DIAGNOSTIC_INDEX: &str = include_str!("../../docs/AGENT-DIAGNOSTIC-HELP.json");

const LANGUAGE_TOPICS: &[(&str, &str)] = &[
    ("workflow", "Spend tokens on source, not on dumps"),
    ("module", "A complete file"),
    ("scalars", "Scalars and literals"),
    ("control-flow", "Control flow, mutation, contracts, effects"),
    ("records", "Records, variants, classes"),
    ("ownership", "Ownership and resources"),
    ("strings", "Strings and bytes"),
    ("builtins", "Compiler-owned functions"),
    (
        "mistakes-code",
        "Habits from other languages: diagnostic examples",
    ),
    (
        "mistakes-index",
        "Habits from other languages: diagnostic index",
    ),
    ("projects", "Projects"),
    ("specifications", "Where the rules live"),
];

fn language_topics() -> String {
    let width = LANGUAGE_TOPICS
        .iter()
        .map(|(selector, _)| selector.len())
        .max()
        .unwrap_or(0);
    let mut output = String::from("Language topics:\n");
    for (selector, heading) in LANGUAGE_TOPICS {
        writeln!(output, "  {selector:<width$}  {heading}")
            .expect("writing to a string cannot fail");
    }
    output
}

pub(crate) fn language_topic(query: &str) -> Result<String, String> {
    if query == "topics" {
        return Ok(language_topics());
    }
    let heading = LANGUAGE_TOPICS
        .iter()
        .find_map(|(selector, heading)| (*selector == query).then_some(*heading))
        .ok_or_else(|| format!("language card has no exact topic `{query}`"))?;
    let marker = format!("## {heading}\n");
    let mut matches = LANGUAGE_REFERENCE.match_indices(&marker);
    let start = matches
        .next()
        .map(|(index, _)| index)
        .expect("every language topic must name a card heading");
    assert!(
        matches.next().is_none(),
        "every language topic heading must be unique"
    );
    let section = &LANGUAGE_REFERENCE[start..];
    let end = section.find("\n## ").unwrap_or(section.len());
    Ok(section[..end].to_owned())
}

pub(crate) fn diagnostic_entry(query: &str) -> Result<String, String> {
    let index: serde_json::Value =
        serde_json::from_str(DIAGNOSTIC_INDEX).expect("generated diagnostic-help JSON must parse");
    assert_eq!(
        index["schema"].as_str(),
        Some("semaprax.agent-diagnostic-help.v1"),
        "generated diagnostic-help JSON must have the current schema"
    );
    let entries = index["entries"]
        .as_array()
        .expect("generated diagnostic-help JSON must contain entries");
    if query == "codes" {
        let mut output = String::from("Diagnostic codes:\n");
        for entry in entries {
            writeln!(
                output,
                "  {}",
                entry["code"]
                    .as_str()
                    .expect("generated diagnostic-help entry must have a code")
            )
            .expect("writing to a string cannot fail");
        }
        return Ok(output);
    }

    let entry = entries
        .iter()
        .find(|entry| entry["code"].as_str() == Some(query));
    let Some(entry) = entry else {
        return Err(format!("diagnostic help has no exact match for `{query}`"));
    };
    let mut output = format!("{query}\n");
    for (index, row) in entry["rows"]
        .as_array()
        .expect("generated diagnostic-help entry must contain rows")
        .iter()
        .enumerate()
    {
        if index > 0 {
            output.push('\n');
        }
        writeln!(
            output,
            "wrote: {}",
            row["wrote"]
                .as_str()
                .expect("generated diagnostic-help row must describe the attempt")
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "fix: {}",
            row["fix"]
                .as_str()
                .expect("generated diagnostic-help row must describe the fix")
        )
        .expect("writing to a string cannot fail");
    }
    Ok(output)
}

/// The generated standard-library catalog, printed by `semaprax help library`:
/// every `std.*` declaration with its signature and contracts, so an agent can
/// pick a library function offline. The bytes are the repository document that
/// `tests/project.rs::standard_library` regenerates from `std/` and pins.
pub(crate) const LIBRARY_CATALOG: &str = include_str!("../../docs/STANDARD-LIBRARY-CATALOG.md");
const LIBRARY_INDEX: &str = include_str!("../../std/catalog.json");
const SHAPES_INDEX: &str = include_str!("../../docs/LANGUAGE-SHAPES-CATALOG.json");

pub(crate) fn library_entry(query: &str) -> Result<String, String> {
    let catalog: serde_json::Value =
        serde_json::from_str(LIBRARY_INDEX).expect("generated standard-library JSON must parse");
    let modules = catalog["modules"]
        .as_array()
        .expect("generated standard-library JSON must contain modules");
    let mut output = String::new();
    for module in modules {
        let module_id = module["module"]
            .as_str()
            .expect("generated standard-library module must have an identity");
        let whole_module = query == module_id;
        for declaration in module["declarations"]
            .as_array()
            .expect("generated standard-library module must contain declarations")
        {
            let id = declaration["id"]
                .as_str()
                .expect("generated standard-library declaration must have an identity");
            let name = declaration["name"]
                .as_str()
                .expect("generated standard-library declaration must have a name");
            if !whole_module && query != id && query != name {
                continue;
            }
            if !output.is_empty() {
                output.push('\n');
            }
            writeln!(output, "{id}").expect("writing to a string cannot fail");
            writeln!(
                output,
                "dependency {}",
                module["dependency"]
                    .as_str()
                    .expect("generated standard-library dependency must be text")
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "profile {}",
                module["required_profile"]
                    .as_str()
                    .expect("generated standard-library profile must be text")
            )
            .expect("writing to a string cannot fail");
            for line in declaration["head"]
                .as_array()
                .expect("generated standard-library declaration must have a head")
            {
                writeln!(
                    output,
                    "{}",
                    line.as_str()
                        .expect("generated standard-library head line must be text")
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
    if output.is_empty() {
        Err(format!("standard library has no exact match for `{query}`"))
    } else {
        Ok(output)
    }
}

fn shape_fields(entry: &serde_json::Value) -> (&str, &str, &str, &str) {
    (
        entry["id"]
            .as_str()
            .expect("generated shape must have an identity"),
        entry["kind"]
            .as_str()
            .expect("generated shape must have a kind"),
        entry["path"]
            .as_str()
            .expect("generated shape must have a source path"),
        entry["signature"]
            .as_str()
            .expect("generated shape must have a signature"),
    )
}

fn shape_rank(entry: &serde_json::Value) -> (usize, usize, &str, &str) {
    let (id, _, path, signature) = shape_fields(entry);
    (
        semaprax::agent_economics::lexical_tokens(path)
            + semaprax::agent_economics::lexical_tokens(signature),
        path.len() + signature.len(),
        id,
        path,
    )
}

fn write_shape(output: &mut String, entry: &serde_json::Value, representative: bool) {
    let (id, kind, path, signature) = shape_fields(entry);
    if !output.is_empty() {
        output.push('\n');
    }
    if representative {
        writeln!(output, "representative {kind}").expect("writing to a string cannot fail");
    } else {
        writeln!(output, "{kind} {id}").expect("writing to a string cannot fail");
    }
    writeln!(output, "source {path}").expect("writing to a string cannot fail");
    output.push_str(signature);
    if !signature.ends_with('\n') {
        output.push('\n');
    }
}

pub(crate) fn shape_entry(query: &str) -> Result<String, String> {
    let catalog: serde_json::Value =
        serde_json::from_str(SHAPES_INDEX).expect("generated language-shapes JSON must parse");
    let entries = catalog["entries"]
        .as_array()
        .expect("generated language-shapes JSON must contain entries");

    if let Some(exemplar) = entries
        .iter()
        .filter(|entry| shape_fields(entry).1 == query)
        .min_by_key(|entry| shape_rank(entry))
    {
        let mut output = String::new();
        write_shape(&mut output, exemplar, true);
        return Ok(output);
    }

    let path_identity = query
        .split_once('#')
        .filter(|(path, id)| !path.is_empty() && !id.is_empty());
    let mut output = String::new();
    for entry in entries {
        let (id, _, path, _) = shape_fields(entry);
        let selected = match path_identity {
            Some((selected_path, selected_id)) => path == selected_path && id == selected_id,
            None => id == query,
        };
        if selected {
            write_shape(&mut output, entry, false);
        }
    }
    if output.is_empty() {
        Err(format!(
            "language shapes catalog has no exact match for `{query}`"
        ))
    } else {
        Ok(output)
    }
}

pub(crate) fn dispatch(args: &[String], private: bool) -> Option<Result<(), u8>> {
    if args.first().map(String::as_str) != Some("help") || args.len() == 1 {
        return None;
    }
    if args.len() == 2 {
        let output = match args[1].as_str() {
            "all" => catalog(private),
            "language" => LANGUAGE_REFERENCE.to_owned(),
            "library" => LIBRARY_CATALOG.to_owned(),
            "shapes" => SHAPES_CATALOG.to_owned(),
            command => match scoped(command, private) {
                Some(output) => output,
                None => {
                    eprint!("{}", unknown_diagnostic(command, private));
                    print!("{}", global(private));
                    return Some(Err(2));
                }
            },
        };
        print!("{output}");
        return Some(Ok(()));
    }
    if args.len() == 3
        && matches!(
            args[1].as_str(),
            "diagnostic" | "language" | "library" | "shapes"
        )
    {
        let result = match args[1].as_str() {
            "diagnostic" => diagnostic_entry(&args[2]),
            "language" => language_topic(&args[2]),
            "library" => library_entry(&args[2]),
            "shapes" => shape_entry(&args[2]),
            _ => unreachable!("closed scoped help catalog"),
        };
        return Some(match result {
            Ok(output) => {
                print!("{output}");
                Ok(())
            }
            Err(error) => {
                eprintln!("{error}");
                Err(2)
            }
        });
    }
    let extra = if matches!(
        args[1].as_str(),
        "diagnostic" | "language" | "library" | "shapes"
    ) {
        &args[3]
    } else {
        &args[2]
    };
    eprintln!("help accepts exactly one operand; unexpected extra operand `{extra}`");
    Some(Err(2))
}

/// The generated language shapes catalog, printed by `semaprax help shapes`:
/// every declaration of every committed example as the documentation model
/// renders it. `tests/projections.rs::shapes_catalog` regenerates and pins it.
pub(crate) const SHAPES_CATALOG: &str = include_str!("../../docs/LANGUAGE-SHAPES-CATALOG.md");

/// Upper bound on the guided global help, in bytes, for either capability
/// class. An agent reads this page before its first command; it must stay one
/// screen, so the bound is a contract and the unit test below enforces it.
pub(crate) const GUIDE_MAX_BYTES: usize = 2048;

struct GuideEntry {
    id: CommandId,
    shape: &'static str,
    summary: &'static str,
}

struct GuideGroup {
    heading: &'static str,
    entries: &'static [GuideEntry],
}

/// The guided global help: the commands a developer or coding agent needs to
/// write, check, run, inspect, and change a program, grouped by task, each
/// with a one-line purpose. Shapes are abbreviated; the catalog rendered by
/// `help all` and by scoped help remains the exact grammar authority.
static GUIDE: &[GuideGroup] = &[
    GuideGroup {
        heading: "Write, check, and run",
        entries: &[
            GuideEntry {
                id: CommandId::Check,
                shape: "check [<input>] [--json]",
                summary: "Parse, resolve, type-check, verify",
            },
            GuideEntry {
                id: CommandId::Fmt,
                shape: "fmt <input> [--check]",
                summary: "Rewrite canonically; --check reports drift",
            },
            GuideEntry {
                id: CommandId::Run,
                shape: "run <input>",
                summary: "Execute main and print its i64 result",
            },
            GuideEntry {
                id: CommandId::Test,
                shape: "test [<dir>|semaprax.toml]",
                summary: "Run the project's test modules",
            },
            GuideEntry {
                id: CommandId::Build,
                shape: "build <input> --target <target>",
                summary: "Emit a native, web, wasm, or npm artifact",
            },
        ],
    },
    GuideGroup {
        heading: "Inspect meaning",
        entries: &[
            GuideEntry {
                id: CommandId::Graph,
                shape: "graph <file>",
                summary: "The complete semantic graph as JSON",
            },
            GuideEntry {
                id: CommandId::Context,
                shape: "context <input> <stable-id>",
                summary: "Bounded facts about one declaration",
            },
            GuideEntry {
                id: CommandId::Doc,
                shape: "doc <file> [--json]",
                summary: "Documentation from the graph",
            },
            GuideEntry {
                id: CommandId::Query,
                shape: "query <input> [--kind K]",
                summary: "Find declarations by kind, name, effect, call",
            },
        ],
    },
    GuideGroup {
        heading: "Change by meaning",
        entries: &[
            GuideEntry {
                id: CommandId::Change,
                shape: "change preview <project> rename",
                summary: "Validate a semantic rename without writing",
            },
            GuideEntry {
                id: CommandId::Impact,
                shape: "impact <file> <patch.spatch>",
                summary: "Preview what a patch would change",
            },
            GuideEntry {
                id: CommandId::Review,
                shape: "review <input> <change>",
                summary: "Review a patch or transaction without writing",
            },
            GuideEntry {
                id: CommandId::Verify,
                shape: "verify <subject> <change> <cap>",
                summary: "Replay an evidence capsule",
            },
        ],
    },
    GuideGroup {
        heading: "Agents",
        entries: &[GuideEntry {
            id: CommandId::Agent,
            shape: "agent inspect <definition.json>",
            summary: "An agent definition's AgentGraph",
        }],
    },
    GuideGroup {
        heading: "Start a project",
        entries: &[
            GuideEntry {
                id: CommandId::New,
                shape: "new <destination>",
                summary: "Create a calculator or library project",
            },
            GuideEntry {
                id: CommandId::ProjectScaffold,
                shape: "project-scaffold --name <name>",
                summary: "The calculator template as one JSON capsule",
            },
        ],
    },
    GuideGroup {
        heading: "Toolchain",
        entries: &[
            GuideEntry {
                id: CommandId::Doctor,
                shape: "doctor [--profile <id>]",
                summary: "Check the toolchain offline",
            },
            GuideEntry {
                id: CommandId::Version,
                shape: "version",
                summary: "Package and commit identity",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help <command>",
                summary: "Exact grammar for one command",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help all",
                summary: "The full command catalog",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help language [topic]",
                summary: "One topic or the language card",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help library [selector]",
                summary: "One API or the full catalog",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help shapes [selector]",
                summary: "One declaration shape",
            },
        ],
    },
];

const GUIDE_FOOTER: &str =
    "Start with `semaprax check <file>`. Diagnostics carry SPX codes; `semaprax help diagnostic <code>`\n\
prints an indexed fix. `--json` emits one diagnostic per line.\n";

fn guide_spec(id: CommandId) -> &'static CommandSpec {
    COMMANDS
        .iter()
        .find(|spec| spec.id == id)
        .expect("every guide entry names a catalog command")
}

/// The guided global help for `semaprax`, `semaprax help`, `--help`, and `-h`.
pub(crate) fn global(private: bool) -> String {
    let visible = |entry: &&GuideEntry| available(guide_spec(entry.id), private);
    let width = GUIDE
        .iter()
        .flat_map(|group| group.entries.iter().filter(visible))
        .map(|entry| entry.shape.len())
        .max()
        .unwrap_or(0);
    let mut out = String::from(BANNER);
    out.push_str("\nUsage: semaprax <command> [arguments]\n");
    out.push_str("<input>: a .spx file, a project directory, or semaprax.toml.\n");
    for group in GUIDE {
        let entries: Vec<_> = group.entries.iter().filter(visible).collect();
        if entries.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(group.heading);
        out.push_str(":\n");
        for entry in entries {
            out.push_str("  ");
            out.push_str(entry.shape);
            for _ in entry.shape.len()..width + 2 {
                out.push(' ');
            }
            out.push_str(entry.summary);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(GUIDE_FOOTER);
    debug_assert!(
        out.len() <= GUIDE_MAX_BYTES,
        "guided help must stay one screen: {} bytes",
        out.len()
    );
    out
}

/// The exhaustive command catalog for `semaprax help all`: every
/// capability-visible global usage line, in catalog order.
pub(crate) fn catalog(private: bool) -> String {
    let mut out = String::from(BANNER);
    out.push_str("\nUsage:\n");
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
pub(crate) fn usage_recovery_hint(args: &[String], private: bool) -> Option<String> {
    let command = args.first()?;
    if command == "help"
        || args[1..]
            .iter()
            .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
        || selected(command, private).is_none()
    {
        return None;
    }
    Some(format!("hint: run `semaprax {command} --help` for usage\n"))
}
pub(crate) fn finish(outcome: Result<(), u8>, recovery_hint: Option<String>) -> ExitCode {
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            if let (2, Some(hint)) = (code, recovery_hint) {
                eprint!("{hint}");
            }
            ExitCode::from(code)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const DISPATCHER_INVENTORY: &[&str] = &[
        "check",
        "graph",
        "doc",
        "verify",
        "agent",
        "skills",
        "explain",
        "fix",
        "query",
        "change",
        "package",
        "add",
        "fetch",
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
        "semantic-cache-evict",
        "semantic-cache-lifecycle",
        "retention-metadata-inventory",
        "retention-metadata-plan",
        "retention-metadata-persist",
        "retention-metadata-load",
        "project-candidate-persist",
        "project-candidate-load",
        "project-draft-persist",
        "project-draft-load",
        "project-candidate-git-publish",
        "serve-workspace",
        "serve-workspace-mcp",
        "service",
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
        "network-run",
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
        "lock",
        "resolve",
        "ui-schema",
        "plugin-manifest",
        "cxx-shim",
        "cxx-package",
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

    #[test]
    fn guide_names_only_catalog_commands_and_stays_one_screen() {
        for group in GUIDE {
            assert!(!group.heading.is_empty() && !group.heading.ends_with(':'));
            for entry in group.entries {
                let spec = guide_spec(entry.id);
                let name = entry.shape.split_whitespace().next().unwrap();
                assert_eq!(
                    name, spec.canonical,
                    "guide shape must start with the canonical name"
                );
                assert!(!entry.summary.is_empty() && !entry.summary.ends_with('.'));
            }
        }
        for private in [false, true] {
            let help = global(private);
            assert!(help.starts_with(BANNER));
            assert!(
                help.len() <= GUIDE_MAX_BYTES,
                "guided help is {} bytes for private={private}",
                help.len()
            );
            assert!(help.contains("\n  help all "));
            assert!(help.contains("\n  help language "));
            assert!(help.contains("semaprax help diagnostic <code>`\n"));
            assert!(help.contains("\n  new "));
            assert!(help.contains("\n  doctor "), "private={private}");
            assert!(!help.contains("|rust"));
            assert!(catalog(private).starts_with(BANNER));
            assert!(catalog(private).contains("\nsemaprax check "));
        }
    }

    #[test]
    fn library_catalog_is_the_generated_repository_document() {
        assert!(LIBRARY_CATALOG.starts_with("# Standard library catalog\n"));
        assert!(LIBRARY_CATALOG.contains("\n## `std.core`\n"));
        assert!(LIBRARY_CATALOG.contains("Dependency: `std.num = \"^0.1.0\"`"));
        assert!(LIBRARY_CATALOG.contains("Required project profile: `useful-text-consumer.v1`"));
        assert!(LIBRARY_CATALOG.contains("```semaprax\n"));
        assert!(LIBRARY_CATALOG.ends_with('\n'));
    }

    #[test]
    fn shapes_catalog_is_the_generated_repository_document() {
        assert!(SHAPES_CATALOG.starts_with("# Language shapes catalog\n"));
        assert!(SHAPES_CATALOG.contains("\n## Functions\n"));
        assert!(SHAPES_CATALOG.contains("```semaprax\n"));
        assert!(SHAPES_CATALOG.ends_with('\n'));
    }

    #[test]
    fn shape_entry_is_exact_disambiguated_and_cheap_by_kind() {
        let expected = concat!(
            "function calculator.add\n",
            "source examples/calculator.spx\n",
            "@id(\"calculator.add\")\n",
            "fn add(left: i64, right: i64) -> i64\n",
        );
        assert_eq!(shape_entry("calculator.add").unwrap(), expected);

        let main = shape_entry("examples/calculator.spx#app.main").unwrap();
        assert!(main.starts_with("function app.main\nsource examples/calculator.spx\n"));
        assert!(!main.contains("examples/banking_ledger.spx"));

        let catalog_units = semaprax::agent_economics::lexical_tokens(SHAPES_CATALOG);
        let catalog: serde_json::Value = serde_json::from_str(SHAPES_INDEX).unwrap();
        let kinds: std::collections::BTreeSet<_> = catalog["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| shape_fields(entry).1)
            .collect();
        assert!(kinds.len() >= 7, "{kinds:?}");
        for kind in kinds {
            let exemplar = shape_entry(kind).unwrap();
            assert!(exemplar.starts_with(&format!("representative {kind}\n")));
            assert!(exemplar.len() <= 512, "{kind}: {} bytes", exemplar.len());
            assert!(
                exemplar.len() * 40 < SHAPES_CATALOG.len(),
                "{kind}: {}/{} bytes",
                exemplar.len(),
                SHAPES_CATALOG.len()
            );
            let units = semaprax::agent_economics::lexical_tokens(&exemplar);
            assert!(units <= 128, "{kind}: {units} lexical units");
            assert!(
                units * 40 < catalog_units,
                "{kind}: {units}/{catalog_units} lexical units"
            );
        }
        assert_eq!(
            shape_entry("not_a_shape").unwrap_err(),
            "language shapes catalog has no exact match for `not_a_shape`"
        );
    }

    #[test]
    fn library_entry_is_exact_deterministic_and_compact() {
        let expected = concat!(
            "std.core.compare\n",
            "dependency std.core = \"^0.1.0\"\n",
            "profile scalar\n",
            "fn compare(left: i64, right: i64) -> i64\n",
            "    ensures result >= -1 && result <= 1\n",
            "    ensures result != 0 || left == right\n",
            "    ensures result == 0 || left != right\n",
        );
        assert_eq!(library_entry("compare").unwrap(), expected);
        assert_eq!(library_entry("std.core.compare").unwrap(), expected);
        assert!(expected.len() <= 512);
        assert!(expected.len() * 50 < LIBRARY_CATALOG.len());

        let module = library_entry("std.core").unwrap();
        assert!(module.starts_with("std.core.ordering.less\n"));
        assert!(module.contains("\nstd.core.compare\n"));
        assert!(!module.contains("std.bytes."));
        assert_eq!(
            library_entry("not_a_library_function").unwrap_err(),
            "standard library has no exact match for `not_a_library_function`"
        );
    }

    #[test]
    fn language_reference_and_exact_topics_are_bounded_repository_sections() {
        assert!(LANGUAGE_REFERENCE.starts_with("# Agent quick reference\n"));
        assert!(LANGUAGE_REFERENCE.contains("```semaprax\n"));
        assert!(LANGUAGE_REFERENCE.ends_with('\n'));
        let reference_units = semaprax::agent_economics::lexical_tokens(LANGUAGE_REFERENCE);
        assert_eq!(LANGUAGE_TOPICS.len(), 12);
        for (selector, heading) in LANGUAGE_TOPICS {
            let topic = language_topic(selector).unwrap();
            assert!(topic.starts_with(&format!("## {heading}\n")), "{selector}");
            assert!(!topic.contains("\n## "), "{selector}");
            assert!(topic.len() <= 4_600, "{selector}: {} bytes", topic.len());
            assert!(
                topic.len() * 5 < LANGUAGE_REFERENCE.len(),
                "{selector}: {}/{} bytes",
                topic.len(),
                LANGUAGE_REFERENCE.len()
            );
            let units = semaprax::agent_economics::lexical_tokens(&topic);
            assert!(units <= 1_500, "{selector}: {units} lexical units");
            assert!(
                units * 5 < reference_units,
                "{selector}: {units}/{reference_units} lexical units"
            );
        }
        let topics = language_topic("topics").unwrap();
        assert!(topics.starts_with("Language topics:\n  workflow"));
        assert!(topics.ends_with("specifications  Where the rules live\n"));
        assert!(topics.len() <= 768);
        assert_eq!(topics.lines().count(), LANGUAGE_TOPICS.len() + 1);
        assert_eq!(
            language_topic("Scalars").unwrap_err(),
            "language card has no exact topic `Scalars`"
        );
    }

    #[test]
    fn diagnostic_help_is_exact_complete_and_cheaper_than_the_index() {
        let index: serde_json::Value = serde_json::from_str(DIAGNOSTIC_INDEX).unwrap();
        assert_eq!(
            index["schema"], "semaprax.agent-diagnostic-help.v1",
            "the embedded companion must use the current schema"
        );
        let entries = index["entries"].as_array().unwrap();
        assert!(entries.len() >= 20);

        let codes = diagnostic_entry("codes").unwrap();
        assert!(codes.starts_with("Diagnostic codes:\n  SPX-O101\n"));
        assert!(codes.ends_with("  SPX-U101\n"));
        assert_eq!(codes.lines().count(), entries.len() + 1);
        assert!(codes.len() <= 256, "{} bytes", codes.len());
        assert!(semaprax::agent_economics::lexical_tokens(&codes) <= 100);

        for entry in entries {
            let code = entry["code"].as_str().unwrap();
            let output = diagnostic_entry(code).unwrap();
            assert!(output.starts_with(&format!("{code}\nwrote: ")));
            assert!(output.ends_with('\n'));
            assert!(output.len() <= 1_024, "{code}: {} bytes", output.len());
            let units = semaprax::agent_economics::lexical_tokens(&output);
            assert!(units <= 300, "{code}: {units} lexical units");
        }

        let t208 = diagnostic_entry("SPX-T208").unwrap();
        assert_eq!(
            t208,
            concat!(
                "SPX-T208\n",
                "wrote: `index + 1` when `index: usize`\n",
                "fix: Integer literals default to `i64`; write `index + 1usize`\n",
            )
        );
        assert!(t208.len() <= 256);
        let full_index = language_topic("mistakes-index").unwrap();
        assert!(t208.len() * 20 < full_index.len());
        assert!(
            semaprax::agent_economics::lexical_tokens(&t208) * 20
                < semaprax::agent_economics::lexical_tokens(&full_index)
        );

        let p106 = diagnostic_entry("SPX-P106").unwrap();
        assert_eq!(p106.matches("\nwrote: ").count(), 6);
        assert!(p106.contains("No tuples; declare a `record`"));
        assert_eq!(
            diagnostic_entry("spx-t208").unwrap_err(),
            "diagnostic help has no exact match for `spx-t208`"
        );
    }

    #[test]
    fn typo_suggestions_are_bounded_unique_and_capability_aware() {
        assert_eq!(suggestion("chck", false), Some("check"));
        assert_eq!(suggestion("checck", false), Some("check"));
        assert_eq!(suggestion("checl", false), Some("check"));
        assert_eq!(suggestion("buidl", false), Some("build"));
        assert_eq!(suggestion("-v", false), None);
        assert_eq!(suggestion("doctro", false), Some("doctor"));
        assert_eq!(suggestion("doctro", true), Some("doctor"));
        assert_eq!(suggestion("not-a-command", true), None);
        assert_eq!(suggestion("gráph", true), None);
        assert_eq!(suggestion(&"x".repeat(65), true), None);
        assert_eq!(
            unknown_diagnostic("chek", false),
            "unknown command `chek`; did you mean `check`?\n\n"
        );
        assert_eq!(
            unknown_diagnostic("not-a-command", false),
            "unknown command `not-a-command`\n\n"
        );
    }
}
