export declare const WORKFLOW_ID: "function_signature_review_publish_v1";
export declare const PROTOCOL: "semaprax.image-agent-protocol.v5";
export declare const MCP_PROTOCOL_VERSION: "2025-11-25";
export type Digest = `sha256:${string}`;
export type JsonObject = {
    readonly [key: string]: unknown;
};
export type WorkflowEvent = 'transport_or_response_uncertain_before_publication' | 'stale_image_reference_or_source_drift' | 'semantic_review_rejection' | 'publish_precondition_rejection' | 'definite_pre_pivot_commit_failure' | 'publication_uncertain';
export type WorkflowOutcome = 'transport_uncertain_no_publish_claim' | 'stale_subject' | 'review_rejected' | 'publish_precondition_rejected' | 'publish_failed_pre_pivot' | 'publication_uncertain';
export type CompilerRepairOptions = readonly [];
export type WorkflowTransitionRepairOptions = readonly [] | readonly ['start_new_review_with_different_intention'];
export type WorkflowPhaseId = 'review' | 'publish';
export type WorkflowEffect = 'read_only' | 'candidate_overlay_mutation' | 'bounded_test_execution' | 'source_publication' | 'receipt_read';
export interface WorkflowResponseAuthority {
    readonly request_capability_changes: false;
    readonly evidence_or_handoff_grants_authority: false;
    readonly candidate_overlay_mutation: boolean;
    readonly test_execution: boolean;
    readonly source_publication: boolean;
}
export interface WorkflowRuntimeBlindSpotUpdate {
    readonly area: 'runtime_environment';
    readonly from: 'not_inspected';
    readonly to: 'partial';
    readonly requires: 'bound_successful_reference_interpreter_report';
}
export interface WorkflowResponseBlindSpots {
    readonly ledger_reference: 'workflow.blind_spots';
    readonly permitted_runtime_update: WorkflowRuntimeBlindSpotUpdate | null;
}
export interface WorkflowResponseContract {
    readonly schema: 'semaprax.supported-product-workflow-response-contract.v1';
    readonly payload_schema: string;
    readonly required_grants: readonly string[];
    readonly effect: WorkflowEffect;
    readonly authority: WorkflowResponseAuthority;
    readonly blind_spots: WorkflowResponseBlindSpots;
}
export interface WorkflowStepObservation {
    readonly phase: WorkflowPhaseId;
    readonly stepIndex: number;
    readonly requestId: string;
    readonly method: string;
    readonly outcome: 'decoded_response' | 'failed_response';
    readonly responseContract: WorkflowResponseContract;
}
export type WorkflowTranscript = readonly WorkflowStepObservation[];
export interface ResultEnvelope {
    readonly schema: string;
    readonly protocol: string;
    readonly image_revision: Digest;
    readonly project_revision: Digest;
    readonly payload: unknown;
}
export interface WorkflowCodec {
    readonly PROTOCOL: string;
    readonly CLIENT_CONTRACT_REVISION: string;
    readonly WORKFLOWS: readonly unknown[];
    request(id: string, method: string, params: object): string;
    decodeTyped(line: string, method: string, expectedId: string): ResultEnvelope;
}
export interface WorkflowTransport {
    readonly sessionId: string;
    exchange(frame: string): string | Promise<string>;
}
/** One already connected, caller-owned MCP byte transport. It grants no authority. */
export interface McpWireTransport {
    readonly sessionId: string;
    exchange(frame: string): string | Promise<string>;
    notify(frame: string): void | Promise<void>;
}
/** A host-selected local tokenizer. The package neither supplies nor starts one. */
export interface ToolPayloadMeasureText {
    readonly tokenizer: string;
    readonly fingerprint: string;
    measureText(text: string): number | Promise<number>;
}
export type ToolPayloadBoundary = 'mcp_content_0_text' | 'direct_v5_response';
export type ToolPayloadReferenceKind = 'same_selected_json' | 'caller_context';
export interface ToolPayloadReference {
    readonly kind: ToolPayloadReferenceKind;
    /** Transient local text. It is counted, hashed, and then discarded. */
    readonly text?: string;
    /** A caller-attested count for a context which the package must not fetch. */
    readonly tokenCount?: number;
}
export interface ToolPayloadObservationContext {
    readonly sessionId: string;
    readonly sequence: number;
    readonly method: string;
    readonly boundary: ToolPayloadBoundary;
    readonly subjectRevision: Digest | null;
    readonly outcome: 'success' | 'error' | 'malformed' | 'timeout' | 'incomplete';
}
export interface ToolPayloadObservationEvent {
    readonly schema: 'semaprax.token-observation.v1';
    readonly eventId: string;
    readonly sessionId: string;
    readonly attemptSequence: number;
    readonly deliverySequence: number;
    readonly method: string;
    readonly boundary: ToolPayloadBoundary;
    readonly subjectRevision: Digest | null;
    readonly outcome: ToolPayloadObservationContext['outcome'];
    readonly status: 'measured' | 'tokenizer_unavailable' | 'tokenizer_failed' | 'baseline_unavailable' | 'incomplete';
    readonly bytes: number | null;
    readonly digest: Digest | null;
    readonly tokenizer: string | null;
    readonly tokenizerFingerprint: string | null;
    readonly tokens: number | null;
    readonly referenceKind: ToolPayloadReferenceKind | null;
    readonly baselineTokens: number | null;
}
export interface ToolPayloadObserverOptions {
    readonly sessionId: string;
    readonly measureText?: ToolPayloadMeasureText;
    readonly reference?: (context: ToolPayloadObservationContext) => ToolPayloadReference | null;
    readonly maxEvents?: number;
    readonly sink?: (event: ToolPayloadObservationEvent) => void | Promise<void>;
}
export interface ToolPayloadSessionGroup {
    readonly tokenizer: string | null;
    readonly tokenizerFingerprint: string | null;
    readonly boundary: ToolPayloadBoundary;
    readonly referenceKind: ToolPayloadReferenceKind | null;
    readonly observed: number;
    readonly tokenMeasured: number;
    readonly paired: number;
    readonly unpaired: number;
    readonly failed: number;
    readonly incomplete: number;
    readonly baselineTokens: number;
    readonly actualTokens: number;
    readonly signedDeltaTokens: number;
    readonly weightedReductionFraction: number | null;
    readonly tokenMeasurementCoverage: {
        readonly numerator: number;
        readonly denominator: number;
    };
    readonly pairedCoverage: {
        readonly numerator: number;
        readonly denominator: number;
    };
}
export interface ToolPayloadSessionAggregate {
    readonly schema: 'semaprax.token-session.v1';
    readonly events: readonly ToolPayloadObservationEvent[];
    readonly groups: readonly ToolPayloadSessionGroup[];
    readonly coverage: {
        readonly observed: number;
        readonly tokenMeasured: number;
        readonly paired: number;
        readonly unpaired: number;
        readonly failed: number;
        readonly incomplete: number;
        readonly tokenMeasurement: {
            readonly numerator: number;
            readonly denominator: number;
        };
        readonly pairedMeasurement: {
            readonly numerator: number;
            readonly denominator: number;
        };
    };
    readonly dropped: number;
    readonly partial: boolean;
}
/** Re-import immutable metadata safely and aggregate only comparable cohorts. */
export declare function aggregateToolPayloadSession(events: readonly ToolPayloadObservationEvent[], dropped?: number): ToolPayloadSessionAggregate;
/**
 * Bounded, metadata-only collection. Observation work is deliberately detached
 * from dispatch: a counter, hash, or sink failure cannot alter protocol flow.
 */
export declare class ToolPayloadObserver {
    readonly sessionId: string;
    private readonly measure;
    private readonly reference;
    private readonly sink;
    private readonly maximum;
    private readonly collected;
    private readonly pending;
    private sequence;
    private dropped;
    constructor(options: ToolPayloadObserverOptions);
    observe(input: Omit<ToolPayloadObservationContext, 'sessionId' | 'sequence'>, payload: string | null): void;
    drain(): Promise<void>;
    events(): readonly ToolPayloadObservationEvent[];
    summary(): Readonly<{
        schema: 'semaprax.token-session.v1';
        sessionId: string;
        events: number;
        dropped: number;
        partial: boolean;
    }>;
    private capture;
}
/** Observe a caller-owned already-normalized v5 transport without changing it. */
export declare function observeDirectWorkflowTransport(transport: WorkflowTransport, observer: ToolPayloadObserver): WorkflowTransport;
/**
 * Initialize the pinned MCP protocol and adapt its exact Semaprax tools/call
 * envelope to the generated v5 codec expected by runReview and runPublish.
 */
export declare function connectMcpWorkflowTransport(wire: McpWireTransport, observer?: ToolPayloadObserver): Promise<WorkflowTransport>;
export interface ApplicationFailure {
    readonly code: number;
    readonly message: string;
    readonly data: {
        readonly schema: 'semaprax.image-agent-application-error-data.v1';
        readonly diagnostics: readonly [RpcDiagnostic, ...RpcDiagnostic[]];
    };
}
export interface RpcDiagnosticLocation {
    readonly line: number;
    readonly column: number;
    readonly start: number;
    readonly end: number;
}
export interface RpcDiagnostic {
    readonly code: string;
    readonly severity: 'error' | 'warning';
    readonly message: string;
    readonly path: string | null;
    readonly location: RpcDiagnosticLocation | null;
    readonly help: string | null;
}
export interface FailureContext {
    readonly phase: WorkflowPhaseId;
    readonly method: string;
    readonly applicationFailure: ApplicationFailure;
    readonly commitInvoked: boolean;
}
export type FailureClassifier = (context: FailureContext) => WorkflowEvent;
export type WorkflowFailureDetail = {
    readonly kind: 'application_failure';
    readonly applicationFailure: ApplicationFailure;
} | {
    readonly kind: 'workflow_transition_failure';
    readonly message: string;
} | {
    readonly kind: 'transport_or_response_failure';
    readonly message: string;
    readonly opaqueCause: unknown;
};
export type ScalarSignatureLiteral = {
    readonly kind: 'i64' | 'i32' | 'u8' | 'usize';
    readonly value: number;
} | {
    readonly kind: 'bool';
    readonly value: boolean;
} | {
    readonly kind: 'char';
    readonly scalar: string;
} | {
    readonly kind: 'f32' | 'f64';
    readonly bits: string;
};
export type ExistingSignatureParameter = {
    readonly from: string;
    readonly name?: string;
};
export type NewSignatureParameter = {
    readonly name: string;
    readonly type: 'i64';
    readonly argument: {
        readonly kind: 'i64';
        readonly value: number;
    };
} | {
    readonly name: string;
    readonly type: 'i32';
    readonly argument: {
        readonly kind: 'i32';
        readonly value: number;
    };
} | {
    readonly name: string;
    readonly type: 'u8';
    readonly argument: {
        readonly kind: 'u8';
        readonly value: number;
    };
} | {
    readonly name: string;
    readonly type: 'usize';
    readonly argument: {
        readonly kind: 'usize';
        readonly value: number;
    };
} | {
    readonly name: string;
    readonly type: 'bool';
    readonly argument: {
        readonly kind: 'bool';
        readonly value: boolean;
    };
} | {
    readonly name: string;
    readonly type: 'char';
    readonly argument: {
        readonly kind: 'char';
        readonly scalar: string;
    };
} | {
    readonly name: string;
    readonly type: 'f32';
    readonly argument: {
        readonly kind: 'f32';
        readonly bits: string;
    };
} | {
    readonly name: string;
    readonly type: 'f64';
    readonly argument: {
        readonly kind: 'f64';
        readonly bits: string;
    };
};
export type SignatureParameter = ExistingSignatureParameter | NewSignatureParameter;
export interface ReviewInput {
    readonly target: string;
    readonly parameters: readonly SignatureParameter[];
    readonly classifyFailure: FailureClassifier;
}
export interface WorkflowHandoff {
    readonly schema: 'semaprax.agent-workflow-handoff.v1';
    readonly workflow: typeof WORKFLOW_ID;
    readonly protocol: typeof PROTOCOL;
    readonly reviewClientContractRevision: Digest;
    readonly reviewProfileRevision: Digest;
    readonly handoffSha256: Digest;
    readonly reviewSessionId: string;
    readonly imageRevision: Digest;
    readonly projectRevision: Digest;
    readonly candidateRevision: Digest;
    readonly target: string;
    readonly compactReference: string;
    readonly resolvedFunction: JsonObject;
    readonly typedIntention: string;
    readonly validation: JsonObject;
    readonly semanticDelta: string;
    readonly semanticDeltaSha256: Digest;
    readonly testPlan: JsonObject;
    readonly testReport: JsonObject;
    readonly sourceReview: string;
    readonly sourceReviewSha256: Digest;
    readonly baseAnalysisCoverage: JsonObject;
    readonly baseAnalysisCoverageSha256: Digest;
    readonly candidateAnalysisCoverage: JsonObject;
    readonly candidateAnalysisCoverageSha256: Digest;
    readonly recoveryCapsule: string;
    readonly recoveryCapsuleSha256: Digest;
    readonly compilerRepairOptions: CompilerRepairOptions;
}
export interface WorkflowFailure {
    readonly status: 'failure';
    readonly outcome: WorkflowOutcome;
    readonly event: WorkflowEvent;
    readonly phase: WorkflowPhaseId;
    readonly method: string;
    readonly commitInvoked: boolean;
    readonly blindRetry: false;
    readonly compilerRepairOptions: CompilerRepairOptions;
    readonly transitionRepairOptions: WorkflowTransitionRepairOptions;
    readonly failure: WorkflowFailureDetail;
    readonly transcript: WorkflowTranscript;
}
export interface ReviewReady {
    readonly status: 'ready';
    readonly outcome: 'reviewed_candidate_and_source_backed_recovery_capsule';
    readonly handoff: WorkflowHandoff;
    readonly compilerRepairOptions: CompilerRepairOptions;
    readonly blindRetry: false;
    readonly transcript: WorkflowTranscript;
}
export type ReviewResult = ReviewReady | WorkflowFailure;
export interface PublicationInspection {
    readonly workflow: typeof WORKFLOW_ID;
    readonly candidateRevision: Digest;
    readonly approvalRevision: Digest;
    readonly reportRevision: Digest;
    readonly precommitStatus: JsonObject;
    readonly commitHandle: JsonObject;
    readonly postcommitStatus: JsonObject;
    readonly receipt: JsonObject;
    readonly receiptSha256: Digest;
    readonly publishClientContractRevision: Digest;
    readonly publishProfileRevision: Digest;
    readonly transcript: WorkflowTranscript;
}
export type InspectPublication = ((inspection: PublicationInspection) => boolean | Promise<boolean>) & {
    readonly classifyFailure: FailureClassifier;
};
export interface PublishComplete {
    readonly status: 'published';
    readonly outcome: 'published';
    readonly candidateRevision: Digest;
    readonly approvalRevision: Digest;
    readonly reportRevision: Digest;
    readonly receipt: JsonObject;
    readonly receiptSha256: Digest;
    readonly publishClientContractRevision: Digest;
    readonly publishProfileRevision: Digest;
    readonly inspected: true;
    readonly commitCalls: 1;
    readonly blindRetry: false;
    readonly transcript: WorkflowTranscript;
}
export type PublishResult = PublishComplete | WorkflowFailure;
export declare function runReview(codec: WorkflowCodec, transport: WorkflowTransport, input: ReviewInput): Promise<ReviewResult>;
export declare function runPublish(codec: WorkflowCodec, transport: WorkflowTransport, handoff: WorkflowHandoff, inspectPublication: InspectPublication): Promise<PublishResult>;
