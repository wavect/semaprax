use std::fmt::Write as _;
use std::process::ExitCode;

mod diagnostic_index;
mod library;
mod shapes;

use library::library_help;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum CommandId {
    Dev,
    Check,
    Graph,
    Explore,
    Compact,
    Doc,
    Verify,
    Agent,
    SourceLive,
    NativeAuthorityCheck,
    Skills,
    Explain,
    Fix,
    Query,
    Change,
    Package,
    Release,
    Add,
    Fetch,
    ProjectImage,
    ProjectImageStore,
    ProjectImageLoad,
    ProjectImageVerify,
    ProjectAssuranceManifest,
    ProjectProofCheck,
    ProjectSymbol,
    ProjectCandidatePreview,
    ProjectCandidateExport,
    ProjectCandidateRestore,
    SemanticCacheInit,
    SemanticCachePersist,
    SemanticCacheLoad,
    SemanticCacheEvict,
    SemanticCacheLifecycle,
    SemanticCacheColdOpen,
    SemanticCacheWarmOpen,
    SemanticCacheRefresh,
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
    PatchReceipt,
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
    AssurancePolicy,
    AssuranceDiff,
    AssuranceManifest,
    SimdReport,
    ProtocolCheck,
    Interpret,
    InterpretStrings,
    UiSchema,
    Webapp,
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
    Audit,
    Workflow,
    Registry,
    Version,
    VersionFlag,
    Harness,
    // Keep this final: the closed-catalog test uses its ordinal as the count.
    Help,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Availability {
    Public,
    /// Commands supplied only by the unpublished physical host.
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
    CommandSpec { id: CommandId::Dev, canonical: "dev", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax dev <semaprax.toml> --jsonl|--human [--interpreter|--source-agent]"] },
    CommandSpec { id: CommandId::Check, canonical: "check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax check [<file>|<dir>|semaprax.toml|--manifest-path path] [--json]"] },
    CommandSpec { id: CommandId::Compact, canonical: "compact", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax compact graph|agent-definition <file> [--encoding text|binary|model-text] [--replay <encoded>]", "semaprax compact context <file> <stable-id> [--max-bytes N] [--encoding text|binary|model-text] [--replay <encoded>]", "semaprax compact task-context <file> <stable-id> [--goal text] [--priority N] [--reason text] [--seed stable-id [--priority N] [--reason text]]... [--revision digest] [--tokenizer byte-v1|lexical-v1] [--max-bytes N] [--max-tokens N] [--encoding text|binary|model-text] [--replay <encoded>]", "semaprax compact api-surface <project> [--encoding text|binary|model-text] [--replay <encoded>]", "semaprax compact candidate-diff <project> <capsule> [--encoding text|binary|model-text] [--replay <encoded>]"] },
    CommandSpec { id: CommandId::Graph, canonical: "graph", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax graph <file>"] },
    CommandSpec { id: CommandId::Explore, canonical: "explore", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax explore <manifest> [--target <id> --depth <n>] [--candidate-capsule <path> --expect-candidate <digest>] --format html|json|markdown|svg --output <path>"] },
    CommandSpec { id: CommandId::Doc, canonical: "doc", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax doc <file|project> [--module <source-path>] [--json]"] },
    CommandSpec { id: CommandId::Verify, canonical: "verify", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax verify <file> <patch.spatch> <evidence.json>", "semaprax verify <root> <patch.wspatch>|<proposal.json> <evidence.json>", "semaprax verify <definition.json> <profile.json> <graph.json>", "semaprax verify <manifest> <image.json>"] },
    CommandSpec { id: CommandId::Agent, canonical: "agent", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax agent inspect <definition.json> [--profile]", "semaprax agent run <definition.json> <task.json> <transcript.json> [--evidence|--trace]", "semaprax agent replay <definition.json> <task.json> <transcript.json> <evidence.json>", "semaprax agent skill [--require-schema <schema>]"] },
    CommandSpec { id: CommandId::SourceLive, canonical: "source-live", aliases: &[], availability: Availability::Private, global: true, usages: &["semaprax-full source-live run <config.json> <checkpoint-dir> --opencode <absolute-executable> --scratch <empty-absolute-dir>", "semaprax-full source-live resume <config.json> <checkpoint-dir> --opencode <absolute-executable> --scratch <empty-absolute-dir>", "semaprax-full source-live migrate <old-config.json> <old-checkpoint-dir> <new-config.json> <new-checkpoint-dir> <function-id> <steps> --opencode <absolute-executable> --scratch <empty-absolute-dir>", "semaprax-full source-live repair run <repair-config.json> <checkpoint-dir>", "semaprax-full source-live repair resume <repair-config.json> <checkpoint-dir>"] },
    CommandSpec { id: CommandId::NativeAuthorityCheck, canonical: "native-authority-check", aliases: &[], availability: Availability::Private, global: true, usages: &["semaprax-full native-authority-check <plan-file> <crate-file> <cargo> <rustc> <target> <strict|sandbox|trusted> <opaque|audited-assertion> <build|dispatch> [--effect name]... [--grant name]... [--require name]... [--json]"] },
    CommandSpec { id: CommandId::Skills, canonical: "skills", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax skills get <agent|language|graph|stdlib|packages|effects>"] },
    CommandSpec { id: CommandId::Explain, canonical: "explain", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax explain <SPX-CODE> [--json]"] },
    CommandSpec { id: CommandId::Fix, canonical: "fix", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fix --plan", "semaprax fix <file> assign-function-id <automatic-function-id> --plan"] },
    CommandSpec { id: CommandId::Query, canonical: "query", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax query --capabilities", "semaprax query <file|project> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]", "semaprax query <project> declarations [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--offset N] [--limit N] [--revision digest]", "semaprax query <project> symbol <stable-id> [--revision digest]", "semaprax query <project> context <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--revision digest]", "semaprax query <project> impact <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N] [--revision digest]", "semaprax query <project> available-operations <stable-id> [--revision digest]"] },
    CommandSpec { id: CommandId::Change, canonical: "change", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax change preview <project> rename-display-name <stable-id> <new-name> [--revision digest] [--evidence|--structural-diff]", "semaprax change preview <project> replace-expression <stable-id> <expression-id> <replacement-json> [--revision digest] [--evidence|--structural-diff]", "semaprax change preview <project> add-contract <stable-id> <requires|ensures> <predicate-json> [--revision digest] [--evidence|--structural-diff]", "semaprax change preview <project> add-declaration <anchor-stable-id> <declaration-json> [--revision digest] [--evidence|--structural-diff]", "semaprax change rebase <base-project> rename-display-name <stable-id> <new-name> --onto <onto-project> [--revision digest] [--onto-revision digest]", "semaprax change merge <project> rename-display-name <left-id> <left-new-name> --with rename-display-name <right-id> <right-new-name> [--revision digest] --order <left-then-right|right-then-left>"] },
    CommandSpec { id: CommandId::Package, canonical: "package", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax package report <file> [--max-bytes N]", "semaprax package lock <subject.json>... [--max-bytes N]", "semaprax package resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::Release, canonical: "release", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax release verify <release-dir>"] },
    CommandSpec { id: CommandId::Audit, canonical: "audit", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax audit inspect <capsule.json>", "semaprax audit verify <capsule.json> <objects-dir> [--require-role <role>]... [--revoke <identity>]... [--trust-log <log-id>]... [--trust-roster <path.json>] [--min-checkpoint-size <n>] [--now <unix-seconds>]", "semaprax audit diff <capsule-a.json> <capsule-b.json>"] },
    CommandSpec { id: CommandId::Workflow, canonical: "workflow", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax workflow inspect <workflow.json>", "semaprax workflow validate <workflow.json>", "semaprax workflow checkpoint <checkpoint.json>", "semaprax workflow dispatch <policy.json> <request.json>"] },
    CommandSpec { id: CommandId::Registry, canonical: "registry", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax registry search <registry.json> <query>", "semaprax registry add <registry.json> <package> <range>", "semaprax registry lock <registry.json> <template.json> [--raw]", "semaprax registry fetch <registry.json> <package> <version> [--raw]", "semaprax registry verify <registry.json> <snapshot-evidence.json>", "semaprax registry verify <registry.json> <template.json> <lock-evidence.json>", "semaprax registry publish <registry.json> <entry.json>"] },
    CommandSpec { id: CommandId::Add, canonical: "add", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax add <dir>|semaprax.toml <package> <range>"] },
    CommandSpec { id: CommandId::Fetch, canonical: "fetch", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fetch <cache-dir> <subject.json>...", "semaprax fetch --lock <lock.json> <cache-dir> <subject.json>..."] },
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
    CommandSpec { id: CommandId::SemanticCacheColdOpen, canonical: "semantic-cache-cold-open", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-cold-open <manifest>"] },
    CommandSpec { id: CommandId::SemanticCacheWarmOpen, canonical: "semantic-cache-warm-open", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-warm-open <manifest> <store-root> <entry-digest>"] },
    CommandSpec { id: CommandId::SemanticCacheRefresh, canonical: "semantic-cache-refresh", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax semantic-cache-refresh <manifest> <store-root> <entry-digest>"] },
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
    CommandSpec { id: CommandId::Service, canonical: "service", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax service <project> [--mcp]"] },
    CommandSpec { id: CommandId::ServeImage, canonical: "serve-image", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-image <manifest>"] },
    CommandSpec { id: CommandId::ServeCandidates, canonical: "serve-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeTestCandidates, canonical: "serve-test-candidates", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-test-candidates <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnostics, canonical: "serve-diagnostics", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics <manifest>"] },
    CommandSpec { id: CommandId::ServeDiagnosticsTested, canonical: "serve-diagnostics-tested", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve-diagnostics-tested <manifest>"] },
    CommandSpec { id: CommandId::Context, canonical: "context", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context <file|project> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters contracts,ownership,effects,types,targets,diagnostics,tests,session_protocol]", "semaprax context <file.spx> <selected-rust-import-id|rust-path> --rust-index <canonical-index.json> [--max-bytes N]", "semaprax context <saved-file.spx> <rust-path-prefix> --rust-index <canonical-index.json> --candidates [--max-bytes N]"] },
    CommandSpec { id: CommandId::ContextBenchmark, canonical: "context-benchmark", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax context-benchmark <manifest>"] },
    CommandSpec { id: CommandId::Serve, canonical: "serve", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax serve <file> [--max-request-bytes N]"] },
    CommandSpec { id: CommandId::QualityPlan, canonical: "quality-plan", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax quality-plan <quick|changed|full> [exact-changed-path ...]"] },
    CommandSpec { id: CommandId::Doctor, canonical: "doctor", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax doctor [--profile <id>] [--target native|web|all] [--json]", "semaprax doctor verify-release <release-dir> --trusted-root-sha256 <64-lowercase-hex>"] },
    CommandSpec { id: CommandId::New, canonical: "new", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax new <destination> [--name project-name] [--template calculator|library|service|stdin-stream-text|stdin-stream-data|source-command-file-text]"] },
    CommandSpec { id: CommandId::ProjectScaffold, canonical: "project-scaffold", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-scaffold --name project-name [--template calculator|library|service|stdin-stream-text|stdin-stream-data|source-command-file-text] [--layout frozen|tables]"] },
    CommandSpec { id: CommandId::Build, canonical: "build", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax build <file> [--target native] [-o|--output path] [--json]", "semaprax build <file> --target native-callable --function stable-id [-o|--output path] [--json]", "semaprax build <file> --target web|wasm [--profile internal-strings-v1|text-toolkit-v1] [--export stable-id ...] [-o|--output path] [--json]", "semaprax build [<dir>|semaprax.toml|--manifest-path path] [--target native|web|wasm|npm|oci|rust] [-o|--output path] [--json]"] },
    CommandSpec { id: CommandId::Run, canonical: "run", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax run <file> [--json] [--max-steps N] [--max-bytes N] [--native] [-- <arg>...]", "semaprax run [<dir>|semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]"] },
    CommandSpec { id: CommandId::NetworkRun, canonical: "network-run", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax network-run [<dir>|semaprax.toml|--manifest-path path] --fixture fixture.json [--arg UTF8]... [--stdin path] [--max-steps N]"] },
    CommandSpec { id: CommandId::Test, canonical: "test", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax test [<dir>|semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N] [--target interpreter]", "semaprax test [<dir>|semaprax.toml|--manifest-path path] [--json] --target native [--native-timeout-ms N] [--native-max-output-bytes N]"] },
    CommandSpec { id: CommandId::Fmt, canonical: "fmt", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax fmt <file>|<dir>|semaprax.toml [--check]", "semaprax fmt --manifest <semaprax.toml> [--check]"] },
    CommandSpec { id: CommandId::Patch, canonical: "patch", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch <file> <patch.spatch>"] },
    CommandSpec { id: CommandId::PatchReceipt, canonical: "patch-receipt", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax patch-receipt <project> render <transaction-json> <candidate-digest>", "semaprax patch-receipt <project> verify <transaction-json> <candidate-digest> <receipt-json>", "semaprax patch-receipt <project> refusal <transaction-json> <requested-candidate-digest>", "semaprax patch-receipt <project> verify-refusal <transaction-json> <requested-candidate-digest> <receipt-json>", "semaprax patch-receipt <project> compare <left-transaction-json> <left-candidate-digest> <left-receipt-json> <right-transaction-json> <right-candidate-digest> <right-receipt-json>", "semaprax patch-receipt <project> evidence-summary <transaction-json> <candidate-digest>", "semaprax patch-receipt <project> evidence-page <transaction-json> <candidate-digest> <evidence-id> <handle> <cursor|->"] },
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
    CommandSpec { id: CommandId::AssurancePolicy, canonical: "assurance-policy", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax assurance-policy <file> --profile <require-static|allow-runtime-guard|allow-test-evidence|report-only> [--max-bytes N] [--max-obligations N]"] },
    CommandSpec { id: CommandId::AssuranceDiff, canonical: "assurance-diff", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax assurance-diff <base-file> <candidate-file> --profile <require-static|allow-runtime-guard|allow-test-evidence|report-only> [--as-of YYYY-MM-DD] [--max-bytes N] [--max-obligations N]"] },
    CommandSpec { id: CommandId::AssuranceManifest, canonical: "assurance-manifest", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax assurance-manifest <file> [--max-bytes N] [--max-obligations N]"] },
    CommandSpec { id: CommandId::ProjectAssuranceManifest, canonical: "project-assurance-manifest", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-assurance-manifest <manifest> [--max-bytes N] [--max-obligations N] [--forbid-reaches <claim-id> <from-id> <to-id>]..."] },
    CommandSpec { id: CommandId::ProjectProofCheck, canonical: "project-proof-check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax project-proof-check <absolute-manifest> --tool lean|z3 --executable <absolute-path> --version-line <exact-version> --host-profile trusted-local|confined (--law <stable-id> | --source <project-path> --declaration <stable-id> --ensures <index>)", "semaprax project-proof-check <absolute-manifest> --workflow summary|detail --law <selected-id> --tool lean|z3 --executable <absolute-path> --version-line <exact-version> --host-profile trusted-local [--source <project-path> --declaration <stable-id> --ensures <index>] [--offset <n> --limit <n> --max-bytes <n>] [--show-witness-values]"] },
    CommandSpec { id: CommandId::SimdReport, canonical: "simd-report", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax simd-report <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::ProtocolCheck, canonical: "protocol-check", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax protocol-check <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::Interpret, canonical: "interpret", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::InterpretStrings, canonical: "interpret-strings", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax interpret-strings <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]"] },
    CommandSpec { id: CommandId::UiSchema, canonical: "ui-schema", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax ui-schema <file> [--max-bytes N]"] },
    CommandSpec { id: CommandId::Webapp, canonical: "webapp", aliases: &[], availability: Availability::Public, global: true, usages: &["semaprax webapp <file> [-o|--output dir] [--title text] [--api]"] },
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
    CommandSpec { id: CommandId::Harness, canonical: "harness", aliases: &[], availability: Availability::Private, global: true, usages: &["semaprax-full harness <verb> [args]  (status explain resolve adopt trust revoke inspect run context exec recover decide endpoints skills bridge report conformance bench)"] },
    CommandSpec { id: CommandId::Help, canonical: "help", aliases: &["--help", "-h"], availability: Availability::Public, global: false, usages: &["semaprax help <command>", "semaprax help all", "semaprax help diagnostic <SPX-code|codes>", "semaprax help language", "semaprax help language <topic|topics>", "semaprax help library", "semaprax help library all", "semaprax help library <module|name|stable-id>", "semaprax help shapes", "semaprax help shapes kinds", "semaprax help shapes <kind|stable-id|path#stable-id>"] },
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
    ("cli", "Command-line programs"),
    ("maps", "String-keyed maps"),
    ("lists", "Lists and iterators"),
    (
        "mistakes-code",
        "Habits from other languages: diagnostic examples",
    ),
    (
        "mistakes-index",
        "Habits from other languages: diagnostic index",
    ),
    ("web", "Web applications"),
    ("projects", "Projects"),
    ("json", "JSON documents and cursors"),
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
    diagnostic_index::response(query, entries)
}

/// The generated standard-library catalog, printed by `semaprax help library all`.
/// The bytes are the repository document that `tests/project.rs::standard_library`
/// regenerates from `std/` and pins.
pub(crate) const LIBRARY_CATALOG: &str = include_str!("../../docs/STANDARD-LIBRARY-CATALOG.md");
const LIBRARY_INDEX: &str = include_str!("../../std/catalog.json");
const SHAPES_INDEX: &str = include_str!("../../docs/LANGUAGE-SHAPES-CATALOG.json");

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
            // The code list, not a usage error followed by the whole guide.
            "diagnostic" => {
                diagnostic_entry("codes").expect("the indexed diagnostic help lists its codes")
            }
            "library" => library_help(None).expect("the standard-library index is generated"),
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
            "library" => library_help(Some(&args[2])),
            "shapes" if args[2] == "kinds" => Ok(shapes::kind_index()),
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
                summary: "Parse, type-check, verify",
            },
            GuideEntry {
                id: CommandId::Fmt,
                shape: "fmt <input> [--check]",
                summary: "Format source; --manifest canonicalizes TOML",
            },
            GuideEntry {
                id: CommandId::Run,
                shape: "run <input>",
                summary: "Execute main; print its result",
            },
            GuideEntry {
                id: CommandId::Test,
                shape: "test [<dir>|semaprax.toml]",
                summary: "Run the project's test modules",
            },
            GuideEntry {
                id: CommandId::Build,
                shape: "build <input> --target <target>",
                summary: "Emit native, web, wasm, or npm",
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
                shape: "doc <input> [--json]",
                summary: "Documentation from the graph",
            },
            GuideEntry {
                id: CommandId::Query,
                shape: "query <input> [--kind K]",
                summary: "Find declarations and callers",
            },
        ],
    },
    GuideGroup {
        heading: "Change by meaning",
        entries: &[
            GuideEntry {
                id: CommandId::Change,
                shape: "change preview <project> <change>",
                summary: "Validate a semantic change without writing",
            },
            GuideEntry {
                id: CommandId::Impact,
                shape: "impact <file> <patch.spatch>",
                summary: "Preview what a patch would change",
            },
            GuideEntry {
                id: CommandId::Review,
                shape: "review <input> <change>",
                summary: "Review a patch or transaction",
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
                summary: "Create a project from a built-in template",
            },
            GuideEntry {
                id: CommandId::ProjectScaffold,
                shape: "project-scaffold --name <name>",
                summary: "Render a built-in template as JSON",
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
                summary: "One topic (`topics` lists them)",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help library [all|selector]",
                summary: "Module index, one API, or full catalog",
            },
            GuideEntry {
                id: CommandId::Help,
                shape: "help shapes [selector]",
                summary: "Catalog; `kinds` lists exact selectors",
            },
        ],
    },
];

const GUIDE_FOOTER: &str =
    "Start: `semaprax check <file>`. SPX diagnostics: `semaprax help diagnostic <code>`\n\
shows an indexed fix. `--json` emits one diagnostic per line.\n";

fn guide_spec(id: CommandId) -> &'static CommandSpec {
    COMMANDS
        .iter()
        .find(|spec| spec.id == id)
        .expect("every guide entry names a catalog command")
}

/// The guided global help for `semaprax`, `semaprax help`, `--help`, and `-h`.
pub(crate) fn global(private: bool) -> String {
    let out = render_global(private);
    debug_assert!(
        out.len() <= GUIDE_MAX_BYTES,
        "guided help must stay one screen: {} bytes",
        out.len()
    );
    out
}

/// Renders the guide without enforcing its budget. `global` adds the debug
/// assertion; the unit test below measures this function instead, so the
/// budget is checked — and reported in bytes — in every profile rather than
/// only where `debug_assert!` is live.
fn render_global(private: bool) -> String {
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
/// Every canonical top-level command name this compiler admits, public or
/// private. Exposed so a sibling module's own closed vocabulary (for example
/// `agent_skill_bundle::PUBLIC_WORKFLOW`'s `cli_command` field) can be
/// cross-checked against the real, single-sourced CLI catalog instead of
/// duplicating it; see `cli::agent::tests::public_workflow_commands_are_all_
/// catalogued`.
#[allow(
    dead_code,
    reason = "used only in `#[cfg(test)]` cross-check; bin build sees no call site"
)]
pub(crate) fn canonical_command_names() -> std::collections::BTreeSet<&'static str> {
    COMMANDS.iter().map(|spec| spec.canonical).collect()
}

pub(crate) fn usage_recovery_hint(args: &[String], private: bool) -> Option<String> {
    let command = args.first()?;
    if command == "help" || requests_help(&args[1..]) || selected(command, private).is_none() {
        return None;
    }
    Some(format!("hint: run `semaprax {command} --help` for usage\n"))
}
/// Whether operands ask for help. Operands after `--` belong to a program
/// that `run` executes, so they are never help flags.
pub(crate) fn requests_help(operands: &[String]) -> bool {
    operands
        .iter()
        .take_while(|argument| argument.as_str() != "--")
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
}

/// Set when the exit status is a single-file command-line program's own
/// result, which is never a CLI usage error and so never earns the hint.
static PROGRAM_EXIT_STATUS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn mark_program_exit_status() {
    PROGRAM_EXIT_STATUS.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn finish(outcome: Result<(), u8>, recovery_hint: Option<String>) -> ExitCode {
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            let program = PROGRAM_EXIT_STATUS.load(std::sync::atomic::Ordering::SeqCst);
            if let (2, Some(hint), false) = (code, recovery_hint, program) {
                eprint!("{hint}");
            }
            ExitCode::from(code)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::library::library_entry;
    use super::*;
    const DISPATCHER_INVENTORY: &[&str] = &[
        "compact",
        "check",
        "graph",
        "doc",
        "verify",
        "agent",
        "source-live",
        "native-authority-check",
        "dev",
        "explore",
        "patch-receipt",
        "harness",
        "skills",
        "explain",
        "fix",
        "query",
        "change",
        "package",
        "release",
        "audit",
        "workflow",
        "registry",
        "add",
        "assurance-policy",
        "assurance-diff",
        "assurance-manifest",
        "project-assurance-manifest",
        "project-proof-check",
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
        "semantic-cache-cold-open",
        "semantic-cache-warm-open",
        "semantic-cache-refresh",
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
        "webapp",
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
    fn source_live_is_visible_only_with_the_private_host() {
        assert!(parse("source-live", false).is_none());
        assert_eq!(parse("source-live", true), Some(CommandId::SourceLive));
        assert!(scoped("source-live", false).is_none());
        assert!(!catalog(false).contains("source-live"));
        let help = scoped("source-live", true).unwrap();
        for verb in ["run", "resume", "migrate"] {
            assert!(help.contains(&format!("source-live {verb} ")));
        }
        for verb in ["repair run", "repair resume"] {
            assert!(help.contains(&format!("source-live {verb} ")));
        }
        assert!(catalog(true).contains("semaprax-full source-live"));
    }

    #[test]
    fn harness_is_visible_only_with_the_private_host() {
        assert!(parse("harness", false).is_none());
        assert_eq!(parse("harness", true), Some(CommandId::Harness));
        assert!(scoped("harness", false).is_none());
        assert!(!catalog(false).contains("harness"));
        assert!(scoped("harness", true).unwrap().contains("harness <verb>"));
        assert!(catalog(true).contains("semaprax-full harness"));
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
            // `render_global`, not `global`: the budget is a contract in every
            // profile, and measuring the unasserted render keeps this test the
            // failure that names the overage even where `debug_assert!` is
            // compiled out.
            let help = render_global(private);
            assert!(
                help.len() <= GUIDE_MAX_BYTES,
                "guided help is {} bytes for private={private}, over the \
                 one-screen budget of {GUIDE_MAX_BYTES}; trim the summaries \
                 or group the inventory rather than raising the bound",
                help.len()
            );
            // Only once the budget holds, since `global` aborts on the overage
            // wherever `debug_assert!` is live.
            assert_eq!(help, global(private));
            assert!(help.starts_with(BANNER));
            assert!(help.contains("\n  help all "));
            assert!(help.contains("\n  help language "));
            assert!(help.contains("help shapes [selector]"));
            assert!(help.contains("Catalog; `kinds` lists exact selectors"));
            assert!(help.contains("semaprax help diagnostic <code>`\n"));
            assert!(help.contains("\n  new "));
            assert!(help.contains("\n  doctor "), "private={private}");
            assert!(!help.contains("|rust"));
            assert!(catalog(private).starts_with(BANNER));
            assert!(catalog(private).contains("\nsemaprax check "));
        }
    }

    #[test]
    fn build_help_separates_native_commands_from_explicit_web_profiles() {
        for private in [false, true] {
            let text = scoped("build", private).unwrap();
            let native = text
                .lines()
                .find(|line| line.contains("[--target native]"))
                .unwrap();
            assert!(!native.contains("--profile"));
            assert!(!native.contains("--export"));
            let web = text
                .lines()
                .find(|line| line.contains("--target web|wasm"))
                .unwrap();
            assert!(web.contains("--profile internal-strings-v1|text-toolkit-v1"));
            assert!(web.contains("--export stable-id"));
            assert!(text.contains("--target native-callable --function stable-id"));
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
        let decimal = concat!(
            "std.int.decimal.compare\n",
            "dependency std.int.decimal = \"^0.1.0\"\n",
            "profile owned-data-api.v1\n",
            "fn compare(left: borrow str, right: borrow str) -> i64\n",
            "    requires valid(left) && valid(right)\n",
            "    ensures result >= -1 && result <= 1\n",
        );
        assert_eq!(library_entry("std.int.decimal.compare").unwrap(), decimal);
        assert_eq!(
            library_entry("compare").unwrap(),
            format!("{expected}\n{decimal}")
        );
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
        assert_eq!(LANGUAGE_TOPICS.len(), 17);
        for (selector, heading) in LANGUAGE_TOPICS {
            let topic = language_topic(selector).unwrap();
            assert!(topic.starts_with(&format!("## {heading}\n")), "{selector}");
            assert!(!topic.contains("\n## "), "{selector}");
            assert!(topic.len() <= 5_000, "{selector}: {} bytes", topic.len());
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
        assert!(
            codes.starts_with("Common diagnostic codes:\n  SPX-P106 SPX-H006 SPX-T252 SPX-T203 ")
        );
        assert!(codes.ends_with("All: semaprax help language mistakes-index\n"));
        assert_eq!(codes.lines().count(), 4);
        assert!(codes.len() <= 256, "{} bytes", codes.len());
        assert!(semaprax::agent_economics::lexical_tokens(&codes) <= 100);

        for entry in entries {
            let code = entry["code"].as_str().unwrap();
            let output = diagnostic_entry(code).unwrap();
            assert!(output.starts_with(&format!("{code}\nwrote: ")));
            assert!(output.ends_with('\n'));
            assert!(language_topic("mistakes-index").unwrap().contains(code));
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
        let f102 = diagnostic_entry("SPX-F102").unwrap();
        assert!(f102.contains("source-command.v1"));
        assert!(f102.contains("semaprax build <manifest> --target native -o <fresh-path>"));
        let g170 = diagnostic_entry("SPX-G170").unwrap();
        assert!(g170.contains("noncanonical Project source"));
        assert!(g170.contains("semaprax fmt --manifest <manifest>"));
        assert!(g170.contains("declare its Project dependency"));
        let t269 = diagnostic_entry("SPX-T269").unwrap();
        assert_eq!(
            t269,
            concat!(
                "SPX-T269\n",
                "wrote: repeated direct output on one path or direct output reachable from a loop\n",
                "fix: Keep direct writes outside loops and within selected-profile limits. ",
                "Default combined stdout + stderr cap: 65,536 bytes; Project v28 staged appends: 1 MiB.\n",
            )
        );
        for (code, expected) in [
            (
                "SPX-J100",
                concat!(
                    "SPX-J100\n",
                    "wrote: bad [modules] lists\n",
                    "fix: 2–16 sorted sources; one bounded test module ≠ entry. entry=\"app\", sources=[\"a.spx\",\"b.spx\"], tests=[\"app.tests\"]; [Manifest](PACKAGE-MANIFEST-V1.md)\n",
                ),
            ),
            (
                "SPX-T252",
                concat!(
                    "SPX-T252\n",
                    "wrote: generic call in while body\n",
                    "fix: vec_len<T>; imported generic aliases stay closed; see [While](WHILE-LOOPS-V1.md)\n",
                    "\n",
                    "wrote: rejected while helper\n",
                    "fix: Borrow exact compiler Vec<T> of Copy scalars; result scalar, flat Copy variant or string\n",
                    "\n",
                    "wrote: outer owned binding changes in while\n",
                    "fix: Keep outer ownership unchanged\n",
                    "\n",
                    "wrote: one-Bytes-plus-usize owner renewal with input views\n",
                    "fix: Pure nongeneric call; exactly one whole owner returned as the same type; whole named independent Slice/str borrows only; [renewal hook](IO-LINES-V1.md#cursor-transitions)\n",
                ),
            ),
            (
                "SPX-T282",
                concat!(
                    "SPX-T282\n",
                    "wrote: Vec literal capacity >8192\n",
                    "fix: Reduce vec_with_capacity<T>; Vec-only limit; see [Vec](OWNED-BOUNDED-VEC-V1.md)\n",
                ),
            ),
            (
                "SPX-T283",
                concat!(
                    "SPX-T283\n",
                    "wrote: lookalike Vec wrapper\n",
                    "fix: Import exact std.collections.vec.* stable ID; no authored substitute; see [Vec](OWNED-BOUNDED-VEC-V1.md)\n",
                ),
            ),
            (
                "SPX-H006",
                concat!(
                    "SPX-H006\n",
                    "wrote: function exceeds 256 shared loans\n",
                    "fix: Reduce shared loans; never raise limit; see [Loan Plan](SHARED-LOAN-PLAN-V1.md)\n",
                    "\n",
                    "wrote: function exceeds 4096 loan points\n",
                    "fix: Simplify flow; extract admitted helpers\n",
                    "\n",
                    "wrote: function exceeds 4096 CFG edges\n",
                    "fix: Simplify flow; extract admitted helpers\n",
                    "\n",
                    "wrote: loan analysis exceeds 1000000 checked work\n",
                    "fix: Reduce analysis work; never raise bound\n",
                ),
            ),
        ] {
            assert_eq!(diagnostic_entry(code).unwrap(), expected);
        }
        let full_index = language_topic("mistakes-index").unwrap();
        assert!(t208.len() * 20 < full_index.len());
        assert!(
            semaprax::agent_economics::lexical_tokens(&t208) * 20
                < semaprax::agent_economics::lexical_tokens(&full_index)
        );

        let p106 = diagnostic_entry("SPX-P106").unwrap();
        assert_eq!(p106.matches("\nwrote: ").count(), 9);
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
