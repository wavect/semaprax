export const WORKFLOW_ID = 'function_signature_review_publish_v1' as const;
export const PROTOCOL = 'semaprax.image-agent-protocol.v5' as const;
export const MCP_PROTOCOL_VERSION = '2025-11-25' as const;

const REVIEW_METHODS = [
  'workspace/open',
  'image/function-reference-export',
  'image/function-reference-resolve',
  'image/analysis-coverage',
  'candidate/open',
  'candidate/apply-intent',
  'candidate/validate',
  'candidate/semantic-delta',
  'candidate/test-plan',
  'candidate/test',
  'candidate/source-review',
  'candidate/analysis-coverage',
  'candidate/recovery-export',
] as const;

const PUBLISH_METHODS = [
  'workspace/open',
  'image/function-reference-resolve',
  'candidate/recovery-restore',
  'candidate/validate',
  'candidate/source-review',
  'source-commit/status',
  'candidate/commit',
  'source-commit/status',
  'candidate/commit-report',
] as const;

const HANDOFF_KEYS = [
  'schema', 'workflow', 'protocol', 'reviewClientContractRevision', 'reviewProfileRevision',
  'handoffSha256', 'reviewSessionId', 'imageRevision', 'projectRevision', 'candidateRevision',
  'target', 'compactReference', 'resolvedFunction', 'typedIntention', 'validation',
  'semanticDelta', 'semanticDeltaSha256', 'testPlan', 'testReport', 'sourceReview',
  'sourceReviewSha256', 'baseAnalysisCoverage', 'baseAnalysisCoverageSha256',
  'candidateAnalysisCoverage', 'candidateAnalysisCoverageSha256', 'recoveryCapsule',
  'recoveryCapsuleSha256', 'compilerRepairOptions',
] as const;

export type Digest = `sha256:${string}`;
export type JsonObject = { readonly [key: string]: unknown };
export type WorkflowEvent =
  | 'transport_or_response_uncertain_before_publication'
  | 'stale_image_reference_or_source_drift'
  | 'semantic_review_rejection'
  | 'publish_precondition_rejection'
  | 'definite_pre_pivot_commit_failure'
  | 'publication_uncertain';
export type WorkflowOutcome =
  | 'transport_uncertain_no_publish_claim'
  | 'stale_subject'
  | 'review_rejected'
  | 'publish_precondition_rejected'
  | 'publish_failed_pre_pivot'
  | 'publication_uncertain';
export type CompilerRepairOptions = readonly [];
export type WorkflowTransitionRepairOptions =
  | readonly []
  | readonly ['start_new_review_with_different_intention'];

export type WorkflowPhaseId = 'review' | 'publish';
export type WorkflowEffect =
  | 'read_only'
  | 'candidate_overlay_mutation'
  | 'bounded_test_execution'
  | 'source_publication'
  | 'receipt_read';

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

/**
 * Initialize the pinned MCP protocol and adapt its exact Semaprax tools/call
 * envelope to the generated v5 codec expected by runReview and runPublish.
 */
export async function connectMcpWorkflowTransport(wire: McpWireTransport): Promise<WorkflowTransport> {
  const sessionId = transportSession(wire);
  const initialized = await wire.exchange(`${JSON.stringify({
    jsonrpc: '2.0',
    id: 'semaprax-workflow-initialize',
    method: 'initialize',
    params: {
      protocolVersion: MCP_PROTOCOL_VERSION,
      capabilities: {},
      clientInfo: { name: '@semaprax/agent-workflow', version: '0.1.0' },
    },
  })}\n`);
  const response = parseObjectFrame(initialized, 'MCP initialize response');
  if (response.jsonrpc !== '2.0' || response.id !== 'semaprax-workflow-initialize' ||
      !same(Object.keys(response).sort(), ['jsonrpc', 'id', 'result'].sort())) {
    throw new Error('MCP initialize response is not the pinned result envelope');
  }
  const result = record(response.result, 'MCP initialize result');
  const capabilities = record(result.capabilities, 'MCP server capabilities');
  const tools = record(capabilities.tools, 'MCP tools capability');
  if (result.protocolVersion !== MCP_PROTOCOL_VERSION || tools.listChanged !== false) {
    throw new Error('MCP server did not select the pinned tools protocol');
  }
  await wire.notify(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);

  let sequence = 0;
  return Object.freeze({
    sessionId,
    exchange: async (frame: string): Promise<string> => {
      const request = parseObjectFrame(frame, 'generated v5 request');
      if (!same(Object.keys(request).sort(), ['jsonrpc', 'id', 'method', 'params'].sort()) ||
          request.jsonrpc !== '2.0' || typeof request.id !== 'string' || request.id.length === 0 || request.id.length > 128 ||
          typeof request.method !== 'string' || request.method.length === 0 || !object(request.params)) {
        throw new Error('generated v5 request is not a closed method call');
      }
      const innerId = request.id;
      sequence += 1;
      const outerId = `semaprax-workflow-call:${sequence}`;
      const outer = await wire.exchange(`${JSON.stringify({
        jsonrpc: '2.0',
        id: outerId,
        method: 'tools/call',
        params: {
          name: request.method.replaceAll('/', '__'),
          arguments: request.params,
        },
      })}\n`);
      const call = parseObjectFrame(outer, 'MCP tools/call response');
      if (!same(Object.keys(call).sort(), ['jsonrpc', 'id', 'result'].sort()) || call.jsonrpc !== '2.0' || call.id !== outerId) {
        throw new Error('MCP tools/call response is not the exact result envelope');
      }
      const callResult = record(call.result, 'MCP tools/call result');
      if (!same(Object.keys(callResult).sort(), ['content', 'isError'].sort()) ||
          typeof callResult.isError !== 'boolean' || !Array.isArray(callResult.content) || callResult.content.length !== 1) {
        throw new Error('MCP tools/call result is not the exact Semaprax text envelope');
      }
      const content = record(callResult.content[0], 'MCP tools/call content');
      if (!same(Object.keys(content).sort(), ['type', 'text'].sort()) || content.type !== 'text' || typeof content.text !== 'string') {
        throw new Error('MCP tools/call content is not one text item');
      }
      const inner = parseObjectFrame(content.text, 'MCP inner v5 response');
      const resultResponse = Object.hasOwn(inner, 'result');
      const errorResponse = Object.hasOwn(inner, 'error');
      if (inner.jsonrpc !== '2.0' || inner.id !== 0 || resultResponse === errorResponse ||
          callResult.isError !== errorResponse ||
          !same(Object.keys(inner).sort(), ['jsonrpc', 'id', resultResponse ? 'result' : 'error'].sort())) {
        throw new Error('MCP inner v5 response is not exactly correlated');
      }
      inner.id = innerId;
      return JSON.stringify(inner);
    },
  });
}

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

export type WorkflowFailureDetail =
  | {
      readonly kind: 'application_failure';
      readonly applicationFailure: ApplicationFailure;
    }
  | {
      readonly kind: 'workflow_transition_failure';
      readonly message: string;
    }
  | {
      readonly kind: 'transport_or_response_failure';
      readonly message: string;
      readonly opaqueCause: unknown;
    };

export type ScalarSignatureLiteral =
  | { readonly kind: 'i64' | 'i32' | 'u8' | 'usize'; readonly value: number }
  | { readonly kind: 'bool'; readonly value: boolean }
  | { readonly kind: 'char'; readonly scalar: string }
  | { readonly kind: 'f32' | 'f64'; readonly bits: string };

export type ExistingSignatureParameter = {
  readonly from: string;
  readonly name?: string;
};

export type NewSignatureParameter =
  | { readonly name: string; readonly type: 'i64'; readonly argument: { readonly kind: 'i64'; readonly value: number } }
  | { readonly name: string; readonly type: 'i32'; readonly argument: { readonly kind: 'i32'; readonly value: number } }
  | { readonly name: string; readonly type: 'u8'; readonly argument: { readonly kind: 'u8'; readonly value: number } }
  | { readonly name: string; readonly type: 'usize'; readonly argument: { readonly kind: 'usize'; readonly value: number } }
  | { readonly name: string; readonly type: 'bool'; readonly argument: { readonly kind: 'bool'; readonly value: boolean } }
  | { readonly name: string; readonly type: 'char'; readonly argument: { readonly kind: 'char'; readonly scalar: string } }
  | { readonly name: string; readonly type: 'f32'; readonly argument: { readonly kind: 'f32'; readonly bits: string } }
  | { readonly name: string; readonly type: 'f64'; readonly argument: { readonly kind: 'f64'; readonly bits: string } };

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

type MutableObject = { [key: string]: unknown };

function object(value: unknown): value is MutableObject {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseObjectFrame(frame: string, name: string): MutableObject {
  if (typeof frame !== 'string' || frame.length === 0) throw new Error(`${name} must be one JSON frame`);
  const body = frame.endsWith('\n') ? frame.slice(0, -1) : frame;
  if (body.length === 0 || body.includes('\n') || body.includes('\r')) throw new Error(`${name} must be one JSON frame`);
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    throw new Error(`${name} is not valid JSON`);
  }
  if (!object(value)) throw new Error(`${name} must be one object`);
  return value;
}

function text(value: unknown, name: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${name} must be nonempty`);
  return value;
}

function digest(value: unknown, name: string): Digest {
  const selected = text(value, name);
  if (!/^sha256:[0-9a-f]{64}$/.test(selected)) throw new Error(`${name} must be a SHA-256 digest`);
  return selected as Digest;
}

function record(value: unknown, name: string): JsonObject {
  if (!object(value)) throw new Error(`${name} must be an object`);
  return value;
}

function same(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.length === right.length && left.every((value, index) => same(value, right[index]));
  }
  if (object(left) && object(right)) {
    const keys = Object.keys(left);
    return keys.length === Object.keys(right).length && keys.every((key) => Object.hasOwn(right, key) && same(left[key], right[key]));
  }
  return false;
}

function canonical(value: unknown): string {
  const ordered = (selected: unknown): unknown => {
    if (Array.isArray(selected)) return selected.map(ordered);
    if (!object(selected)) return selected;
    return Object.fromEntries(Object.keys(selected).sort().map((key) => [key, ordered(selected[key])]));
  };
  return JSON.stringify(ordered(value));
}

function intentionText(target: string, parameters: readonly JsonObject[]): string {
  // The public constructor order is frozen separately from sorted digest JSON.
  return JSON.stringify({ kind: 'change_function_signature', target, parameters });
}

async function sha256(value: string): Promise<Digest> {
  const bytes = new TextEncoder().encode(value);
  const hash = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return `sha256:${Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

async function domainDigest(domain: string, value: string): Promise<Digest> {
  const owner = new TextEncoder().encode(`${domain}\0`);
  const bytes = new TextEncoder().encode(value);
  const length = new Uint8Array(8);
  new DataView(length.buffer).setBigUint64(0, BigInt(bytes.length), true);
  const framed = new Uint8Array(owner.length + length.length + bytes.length);
  framed.set(owner);
  framed.set(length, owner.length);
  framed.set(bytes, owner.length + length.length);
  const hash = await globalThis.crypto.subtle.digest('SHA-256', framed);
  return `sha256:${Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

async function receiptRevision(value: string): Promise<Digest> {
  const owner = new TextEncoder().encode('semaprax.image-source-commit.receipt.v1\0');
  const bytes = new TextEncoder().encode(value);
  const length = new Uint8Array(8);
  new DataView(length.buffer).setBigUint64(0, BigInt(bytes.length), false);
  const framed = new Uint8Array(owner.length + length.length + bytes.length);
  framed.set(owner);
  framed.set(length, owner.length);
  framed.set(bytes, owner.length + length.length);
  const hash = await globalThis.crypto.subtle.digest('SHA-256', framed);
  return `sha256:${Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

interface BoundWorkflow {
  readonly metadata: JsonObject;
  readonly clientContractRevision: Digest;
  readonly profileRevision: Digest;
  readonly reviewSteps: readonly BoundWorkflowStep[];
  readonly publishSteps: readonly BoundWorkflowStep[];
}

interface BoundWorkflowStep {
  readonly index: number;
  readonly method: string;
  readonly responseContract: WorkflowResponseContract;
}

function workflow(codec: WorkflowCodec, requirePublish = false): BoundWorkflow {
  if (codec.PROTOCOL !== PROTOCOL) throw new Error('codec protocol mismatch');
  const contract = digest(codec.CLIENT_CONTRACT_REVISION, 'codec contract revision');
  if (!Array.isArray(codec.WORKFLOWS) || codec.WORKFLOWS.length !== 1) throw new Error('codec workflow inventory mismatch');
  const selected = record(codec.WORKFLOWS[0], 'supported workflow');
  if (selected.id !== WORKFLOW_ID) throw new Error('supported workflow id mismatch');
  const qualification = record(selected.qualification, 'workflow qualification');
  const profileRevision = digest(qualification.selected_profile_revision, 'selected workflow profile revision');
  const phases = selected.phases;
  if (!Array.isArray(phases)) throw new Error('workflow phases missing');
  const reviewSteps = checkPhase(phases, 'review', REVIEW_METHODS);
  const publish = phases.find((phase) => object(phase) && phase.id === 'publish');
  if (requirePublish && publish === undefined) throw new Error('codec publish workflow phase missing');
  const publishSteps = publish === undefined ? [] : checkPhase(phases, 'publish', PUBLISH_METHODS);
  return { metadata: selected, clientContractRevision: contract, profileRevision, reviewSteps, publishSteps };
}

function checkPhase(
  phases: readonly unknown[],
  id: WorkflowPhaseId,
  methods: readonly string[],
): readonly BoundWorkflowStep[] {
  const phase = phases.find((value) => object(value) && value.id === id);
  if (!object(phase) || !Array.isArray(phase.ordered_steps)) throw new Error(`workflow ${id} phase missing`);
  const actual = phase.ordered_steps.map((step) => object(step) ? step.method : undefined);
  if (!same(actual, methods)) throw new Error(`workflow ${id} method order mismatch`);
  return Object.freeze(phase.ordered_steps.map((raw, position) => {
    const step = record(raw, `workflow ${id} step ${position}`);
    if (!Number.isSafeInteger(step.index) || step.index !== position + 1 || step.method !== methods[position]) {
      throw new Error(`workflow ${id} step index mismatch`);
    }
    return Object.freeze({
      index: position + 1,
      method: methods[position]!,
      responseContract: responseContract(step.response_contract, methods[position]!, `workflow ${id} step ${position}`),
    });
  }));
}

function expectedGrants(method: string): readonly string[] {
  if (method === 'candidate/test' || method === 'candidate/test-plan') return ['candidate_prepare', 'candidate_test'];
  if (method === 'candidate/commit') return ['candidate_prepare', 'source_commit'];
  if (method === 'candidate/commit-report' || method === 'source-commit/status') return ['source_commit'];
  if (method.startsWith('candidate/')) return ['candidate_prepare'];
  return ['semantic_read'];
}

function expectedEffect(method: string): WorkflowEffect {
  if (method === 'candidate/open' || method === 'candidate/apply-intent' || method === 'candidate/recovery-restore') return 'candidate_overlay_mutation';
  if (method === 'candidate/test') return 'bounded_test_execution';
  if (method === 'candidate/commit') return 'source_publication';
  if (method === 'candidate/commit-report') return 'receipt_read';
  return 'read_only';
}

function responseContract(value: unknown, method: string, name: string): WorkflowResponseContract {
  const contract = record(value, `${name} response contract`);
  const keys = ['schema', 'payload_schema', 'required_grants', 'effect', 'authority', 'blind_spots'];
  if (!same(Object.keys(contract).sort(), keys.sort()) ||
      contract.schema !== 'semaprax.supported-product-workflow-response-contract.v1' ||
      typeof contract.payload_schema !== 'string' || contract.payload_schema.length === 0 ||
      !Array.isArray(contract.required_grants) || !contract.required_grants.every((grant) => typeof grant === 'string' && grant.length > 0) ||
      new Set(contract.required_grants).size !== contract.required_grants.length ||
      !same(contract.required_grants, expectedGrants(method))) {
    throw new Error(`${name} response contract is not closed`);
  }
  const effects: readonly WorkflowEffect[] = ['read_only', 'candidate_overlay_mutation', 'bounded_test_execution', 'source_publication', 'receipt_read'];
  if (!effects.includes(contract.effect as WorkflowEffect) || contract.effect !== expectedEffect(method)) throw new Error(`${name} response effect is unknown`);
  const authority = record(contract.authority, `${name} response authority`);
  const authorityKeys = ['request_capability_changes', 'evidence_or_handoff_grants_authority', 'candidate_overlay_mutation', 'test_execution', 'source_publication'];
  if (!same(Object.keys(authority).sort(), authorityKeys.sort()) ||
      authority.request_capability_changes !== false || authority.evidence_or_handoff_grants_authority !== false ||
      typeof authority.candidate_overlay_mutation !== 'boolean' || typeof authority.test_execution !== 'boolean' ||
      typeof authority.source_publication !== 'boolean') {
    throw new Error(`${name} response authority is not closed`);
  }
  if (authority.candidate_overlay_mutation !== (contract.effect === 'candidate_overlay_mutation') ||
      authority.test_execution !== (contract.effect === 'bounded_test_execution') ||
      authority.source_publication !== (contract.effect === 'source_publication')) {
    throw new Error(`${name} response authority disagrees with its effect`);
  }
  const blindSpots = record(contract.blind_spots, `${name} response blind spots`);
  if (!same(Object.keys(blindSpots).sort(), ['ledger_reference', 'permitted_runtime_update'].sort()) ||
      blindSpots.ledger_reference !== 'workflow.blind_spots') {
    throw new Error(`${name} response blind spots are not closed`);
  }
  let update: WorkflowRuntimeBlindSpotUpdate | null = null;
  if (blindSpots.permitted_runtime_update !== null) {
    const selected = record(blindSpots.permitted_runtime_update, `${name} permitted runtime update`);
    if (!same(Object.keys(selected).sort(), ['area', 'from', 'to', 'requires'].sort()) ||
        selected.area !== 'runtime_environment' || selected.from !== 'not_inspected' || selected.to !== 'partial' ||
        selected.requires !== 'bound_successful_reference_interpreter_report') {
      throw new Error(`${name} permitted runtime update is not closed`);
    }
    update = Object.freeze({
      area: 'runtime_environment',
      from: 'not_inspected',
      to: 'partial',
      requires: 'bound_successful_reference_interpreter_report',
    });
  }
  if ((update !== null) !== (contract.effect === 'bounded_test_execution')) {
    throw new Error(`${name} permitted runtime update disagrees with its effect`);
  }
  return Object.freeze({
    schema: 'semaprax.supported-product-workflow-response-contract.v1',
    payload_schema: contract.payload_schema,
    required_grants: Object.freeze([...(contract.required_grants as string[])]),
    effect: contract.effect as WorkflowEffect,
    authority: Object.freeze({
      request_capability_changes: false,
      evidence_or_handoff_grants_authority: false,
      candidate_overlay_mutation: authority.candidate_overlay_mutation as boolean,
      test_execution: authority.test_execution as boolean,
      source_publication: authority.source_publication as boolean,
    }),
    blind_spots: Object.freeze({
      ledger_reference: 'workflow.blind_spots',
      permitted_runtime_update: update,
    }),
  });
}

function transportSession(transport: WorkflowTransport): string {
  return text(transport.sessionId, 'transport sessionId');
}

function applicationFailure(error: unknown): ApplicationFailure | null {
  if (!object(error) || !object(error.rpc)) return null;
  const rpc = error.rpc;
  if (!same(Object.keys(rpc).sort(), ['code', 'message', 'data'].sort()) ||
      !Number.isSafeInteger(rpc.code) || typeof rpc.message !== 'string' || !object(rpc.data) ||
      !same(Object.keys(rpc.data).sort(), ['schema', 'diagnostics'].sort())) return null;
  if (rpc.data.schema !== 'semaprax.image-agent-application-error-data.v1' || !Array.isArray(rpc.data.diagnostics) || rpc.data.diagnostics.length === 0 || !rpc.data.diagnostics.every(diagnostic)) return null;
  const diagnostics = rpc.data.diagnostics.map((value) => {
    const selected = value as RpcDiagnostic;
    return Object.freeze({
      code: selected.code,
      severity: selected.severity,
      message: selected.message,
      path: selected.path,
      location: selected.location === null ? null : Object.freeze({ ...selected.location }),
      help: selected.help,
    });
  }) as unknown as readonly [RpcDiagnostic, ...RpcDiagnostic[]];
  return Object.freeze({
    code: rpc.code as number,
    message: rpc.message,
    data: Object.freeze({
      schema: 'semaprax.image-agent-application-error-data.v1',
      diagnostics: Object.freeze(diagnostics),
    }),
  });
}

function diagnostic(value: unknown): value is RpcDiagnostic {
  if (!object(value) || !same(Object.keys(value).sort(), ['code', 'help', 'location', 'message', 'path', 'severity'].sort())) return false;
  if (typeof value.code !== 'string' || typeof value.message !== 'string' || (value.severity !== 'error' && value.severity !== 'warning')) return false;
  if (value.path !== null && typeof value.path !== 'string') return false;
  if (value.help !== null && typeof value.help !== 'string') return false;
  if (value.location === null) return true;
  if (!object(value.location) || !same(Object.keys(value.location).sort(), ['line', 'column', 'start', 'end'].sort())) return false;
  const location = value.location;
  return ['line', 'column', 'start', 'end'].every((key) => Number.isSafeInteger(location[key]) && (location[key] as number) >= 0);
}

const RESERVED_NAMES = new Set(['module','use','fn','let','mut','if','else','while','match','true','false','requires','ensures','uses','permit','unsafe','return','own','borrow','shared','self','super']);

function boundedText(value: unknown, name: string, maximum: number): string {
  const selected = text(value, name);
  if ([...selected].length > maximum || new TextEncoder().encode(selected).length > maximum) throw new Error(`${name} exceeds its bound`);
  return selected;
}

function identifier(value: unknown, name: string): string {
  const selected = boundedText(value, name, 128);
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(selected) || RESERVED_NAMES.has(selected)) throw new Error(`${name} is not an admitted identifier`);
  return selected;
}

function signatureParameters(values: readonly SignatureParameter[]): JsonObject[] {
  if (!Array.isArray(values) || values.length > 4096) throw new Error('signature parameters exceed their bound');
  return values.map((value, index) => {
    if (!object(value)) throw new Error(`signature parameter ${index} must be an object`);
    if (Object.hasOwn(value, 'from')) {
      if (!same(Object.keys(value).sort(), (Object.hasOwn(value, 'name') ? ['from', 'name'] : ['from']).sort())) throw new Error(`signature parameter ${index} is not closed`);
      const from = boundedText(value.from, `signature parameter ${index} from`, 4096);
      return Object.hasOwn(value, 'name') ? { from, name: identifier(value.name, `signature parameter ${index} name`) } : { from };
    }
    if (!same(Object.keys(value).sort(), ['name', 'type', 'argument'].sort())) throw new Error(`signature parameter ${index} is not closed`);
    const name = identifier(value.name, `signature parameter ${index} name`);
    const kind = value.type;
    const argument = record(value.argument, `signature parameter ${index} argument`);
    if (argument.kind !== kind) throw new Error(`signature parameter ${index} literal kind mismatch`);
    let selected: JsonObject;
    if (kind === 'i64' || kind === 'i32' || kind === 'u8' || kind === 'usize') {
      if (!same(Object.keys(argument).sort(), ['kind', 'value'].sort()) || !Number.isSafeInteger(argument.value)) throw new Error(`signature parameter ${index} requires a safe integer literal`);
      const numeric = argument.value as number;
      if ((kind === 'i32' && (numeric < -2147483648 || numeric > 2147483647)) || (kind === 'u8' && (numeric < 0 || numeric > 255)) || (kind === 'usize' && numeric < 0)) throw new Error(`signature parameter ${index} integer literal is out of range`);
      selected = { kind, value: numeric };
    } else if (kind === 'bool') {
      if (!same(Object.keys(argument).sort(), ['kind', 'value'].sort()) || typeof argument.value !== 'boolean') throw new Error(`signature parameter ${index} requires a boolean literal`);
      selected = { kind, value: argument.value };
    } else if (kind === 'char' || kind === 'f32' || kind === 'f64') {
      const field = kind === 'char' ? 'scalar' : 'bits';
      const width = kind === 'f64' ? 16 : 8;
      if (!same(Object.keys(argument).sort(), ['kind', field].sort()) || typeof argument[field] !== 'string' || !new RegExp(`^[0-9a-f]{${width}}$`).test(argument[field] as string)) throw new Error(`signature parameter ${index} requires an exact hexadecimal literal`);
      selected = { kind, [field]: argument[field] };
    } else {
      throw new Error(`signature parameter ${index} has an unsupported scalar type`);
    }
    return { name, type: kind, argument: selected };
  });
}

function exactValidation(value: JsonObject, candidate: Digest, event: WorkflowEvent): void {
  if (value.schema !== 'semaprax.image-candidate-validation.v1' ||
      value.candidate_revision !== candidate || value.independently_replayed !== true ||
      value.source_reparsed !== true || value.project_profile_admitted !== true ||
      value.tests !== 'not_run' || value.target_execution !== false ||
      value.commit_authority !== false) {
    throw new WorkflowTransitionError(event, 'candidate validation did not preserve the exact admitted replay facts');
  }
}

const REQUIRED_COVERAGE: Readonly<Record<string, readonly string[]>> = {
  declared_source_inputs: ['known'],
  declared_external_contracts: ['partial', 'not_inspected'],
  deployment_configuration: ['not_inspected'],
  generated_file_provenance: ['not_inspected'],
  generated_artifacts: ['not_inspected'],
  external_api_behavior: ['not_inspected'],
  runtime_environment: ['not_inspected'],
  external_consumers: ['not_inspected'],
};

function exactCoverage(
  value: JsonObject,
  binding: { readonly image?: Digest; readonly project: Digest; readonly candidate?: Digest },
): void {
  const candidate = binding.candidate;
  const expectedSchema = candidate === undefined
    ? 'semaprax.image-analysis-coverage.v1'
    : 'semaprax.project-candidate-analysis-coverage.v1';
  if (value.schema !== expectedSchema ||
      (binding.image !== undefined && value.image_revision !== binding.image) ||
      (candidate === undefined ? value.project_revision !== binding.project : value.base_project_revision !== binding.project) ||
      (candidate !== undefined && value.candidate_revision !== candidate) ||
      !Array.isArray(value.areas) || value.areas.length !== 8) {
    throw new WorkflowTransitionError('semantic_review_rejection', 'analysis coverage is not bound to the reviewed subject');
  }
  const seen = new Set<string>();
  for (const row of value.areas) {
    if (!object(row) || typeof row.area !== 'string' || typeof row.status !== 'string' || seen.has(row.area)) {
      throw new WorkflowTransitionError('semantic_review_rejection', 'analysis coverage area inventory is not exact');
    }
    const allowed = REQUIRED_COVERAGE[row.area];
    if (allowed === undefined || !allowed.includes(row.status)) {
      throw new WorkflowTransitionError('semantic_review_rejection', 'analysis coverage weakens a required blind-spot status');
    }
    seen.add(row.area);
  }
  if (seen.size !== Object.keys(REQUIRED_COVERAGE).length) {
    throw new WorkflowTransitionError('semantic_review_rejection', 'analysis coverage omits a required blind-spot area');
  }
}

function exactReference(
  reference: JsonObject,
  resolved: JsonObject,
  binding: { readonly image: Digest; readonly project: Digest; readonly target: string },
  event: WorkflowEvent,
): void {
  const summary = object(resolved.function_summary) ? resolved.function_summary : null;
  if (reference.schema !== 'semaprax.image-function-reference.v1' ||
      reference.image_revision !== binding.image || reference.project_revision !== binding.project ||
      reference.target_kind !== 'function' || reference.target !== binding.target || reference.facet !== null ||
      resolved.schema !== 'semaprax.image-function-reference-resolution.v1' ||
      resolved.reference_revision !== reference.reference_revision ||
      resolved.image_revision !== binding.image || resolved.project_revision !== binding.project ||
      resolved.target !== binding.target || resolved.facet !== null || resolved.facet_handle !== null ||
      summary === null || summary.id !== binding.target) {
    throw new WorkflowTransitionError(event, 'function reference or resolution is not bound to the exact selected target');
  }
}

async function exactSourceReview(value: JsonObject, candidate: Digest, base: Digest, event: WorkflowEvent): Promise<{ candidateProjectRevision: Digest; paths: string[] }> {
  if (value.schema !== 'semaprax.project-candidate-source-review.v1' ||
      value.candidate_revision !== candidate || value.base_project_revision !== base ||
      value.source_authority !== false || !Array.isArray(value.files)) {
    throw new WorkflowTransitionError(event, 'source review is not bound to the exact candidate and base');
  }
  const candidateProjectRevision = digest(value.candidate_project_revision, 'candidate project revision');
  const reportRevision = digest(value.report_revision, 'source review report revision');
  const core = { ...value };
  delete core.report_revision;
  if (await domainDigest('semaprax.project-candidate-source-review.v1', `${canonical(core)}\n`) !== reportRevision) {
    throw new WorkflowTransitionError(event, 'source review report revision is inconsistent');
  }
  const paths: string[] = [];
  for (const [index, raw] of value.files.entries()) {
    const file = record(raw, `source review file ${index}`);
    const path = boundedText(file.path, `source review file ${index} path`, 240);
    if (paths.at(-1) !== undefined && paths.at(-1)! >= path) throw new WorkflowTransitionError(event, 'source review file inventory is not canonical');
    if (typeof file.base_source !== 'string' || typeof file.candidate_source !== 'string' || file.base_source === file.candidate_source || typeof file.source_diff !== 'string' || file.source_diff.length === 0) {
      throw new WorkflowTransitionError(event, 'source review omits complete changed source or diff bytes');
    }
    if (await domainDigest('semaprax.semantic-review.source-digest.v1', file.base_source) !== file.base_digest ||
        await domainDigest('semaprax.semantic-review.source-digest.v1', file.candidate_source) !== file.candidate_digest ||
        await domainDigest('semaprax.candidate.source-diff.v1', file.source_diff) !== file.source_diff_digest) {
      throw new WorkflowTransitionError(event, 'source review file digest is inconsistent');
    }
    paths.push(path);
  }
  if (paths.length === 0) throw new WorkflowTransitionError(event, 'source review contains no changed source files');
  return { candidateProjectRevision, paths };
}

function exactPrecommitStatus(value: JsonObject, candidate: Digest): { approval: Digest } {
  const pending = record(value.pending_approval, 'pending approval');
  if (value.schema !== 'semaprax.image-source-commit-status.v1' || value.capability !== 'source_commit' ||
      value.authority !== 'startup_fixed_host_git_policy' || value.state !== 'available' ||
      value.report_revision !== null || !Array.isArray(value.last_error_codes) || value.last_error_codes.length !== 0 ||
      value.approval_via_request !== false || value.raw_working_tree_write !== false || value.host_state_only !== true ||
      !same(Object.keys(pending).sort(), ['approval_revision', 'candidate_revision']) || pending.candidate_revision !== candidate) {
    throw new WorkflowTransitionError('publish_precondition_rejection', 'precommit status is not the exact clean approved state');
  }
  return { approval: digest(pending.approval_revision, 'approval revision') };
}

function exactPostcommitStatus(value: JsonObject, report: Digest): void {
  if (value.schema !== 'semaprax.image-source-commit-status.v1' || value.capability !== 'source_commit' ||
      value.authority !== 'startup_fixed_host_git_policy' || value.state !== 'published' ||
      value.pending_approval !== null || value.report_revision !== report ||
      !Array.isArray(value.last_error_codes) || value.last_error_codes.length !== 0 ||
      value.approval_via_request !== false || value.raw_working_tree_write !== false || value.host_state_only !== true) {
    throw new WorkflowTransitionError('publication_uncertain', 'postcommit status does not exactly confirm publication');
  }
}

function exactCommitHandle(value: JsonObject, candidate: Digest, approval: Digest): Digest {
  if (value.schema !== 'semaprax.image-source-commit-handle.v1' || value.state !== 'published' ||
      value.candidate_revision !== candidate || value.approval_revision !== approval ||
      value.receipt_method !== 'candidate/commit-report' || value.raw_working_tree_write !== false ||
      value.source_commit_authority !== 'startup_fixed_host_git_policy' || !Number.isSafeInteger(value.report_bytes) || (value.report_bytes as number) <= 0) {
    throw new WorkflowTransitionError('publication_uncertain', 'commit handle does not exactly bind the publication');
  }
  return digest(value.report_revision, 'publication report revision');
}

function exactReceipt(
  value: JsonObject,
  binding: { candidate: Digest; baseProject: Digest; candidateProject: Digest; paths: readonly string[] },
): void {
  const keys = ['schema','repository','reference','previous_commit','published_commit','tree','approved_candidate_digest','base_project_revision','candidate_project_revision','updated_source_paths','publication','git_object_format','working_tree_rewritten','project_manifest_changed','managed_active_changed','source_authority','tests','nonclaims'];
  const format = value.git_object_format;
  const width = format === 'sha1' ? 40 : format === 'sha256' ? 64 : 0;
  const oid = (candidate: unknown) => typeof candidate === 'string' && new RegExp(`^[0-9a-f]{${width}}$`).test(candidate);
  if (!same(Object.keys(value).sort(), keys.sort()) || value.schema !== 'semaprax.project-candidate-git-publication.v1' ||
      typeof value.repository !== 'string' || value.repository.length === 0 || typeof value.reference !== 'string' || !value.reference.startsWith('refs/heads/') ||
      width === 0 || !oid(value.previous_commit) || !oid(value.published_commit) || !oid(value.tree) || value.previous_commit === value.published_commit ||
      value.approved_candidate_digest !== binding.candidate || value.base_project_revision !== binding.baseProject ||
      value.candidate_project_revision !== binding.candidateProject || !same(value.updated_source_paths, binding.paths) ||
      value.publication !== 'git_branch_ref_compare_and_swap' || value.working_tree_rewritten !== false ||
      value.project_manifest_changed !== false || value.managed_active_changed !== false ||
      value.source_authority !== 'explicit_host_git_ref_authority' || value.tests !== 'not_run' ||
      !same(value.nonclaims, ['no_atomic_raw_working_tree_rewrite','no_network_push_or_remote_publication','no_signature_or_approval_service','unreachable_objects_may_remain_after_failure'])) {
    throw new WorkflowTransitionError('publication_uncertain', 'publication receipt is incomplete or disagrees with the reviewed source');
  }
}

const ALLOWED_APPLICATION_EVENTS = new Set<WorkflowEvent>([
  'stale_image_reference_or_source_drift',
  'semantic_review_rejection',
  'publish_precondition_rejection',
  'definite_pre_pivot_commit_failure',
  'publication_uncertain',
]);

class WorkflowTransitionError extends Error {
  constructor(readonly event: WorkflowEvent, message: string) {
    super(message);
    this.name = 'WorkflowTransitionError';
  }
}

function failure(
  error: unknown,
  phase: WorkflowPhaseId,
  method: string,
  classifier: FailureClassifier,
  commitInvoked: boolean,
  transcript: readonly WorkflowStepObservation[],
): WorkflowFailure {
  const application = applicationFailure(error);
  const detail: WorkflowFailureDetail = error instanceof WorkflowTransitionError
    ? Object.freeze({ kind: 'workflow_transition_failure', message: error.message })
    : application !== null
      ? Object.freeze({ kind: 'application_failure', applicationFailure: application })
      : Object.freeze({
          kind: 'transport_or_response_failure',
          message: error instanceof Error && error.message.length > 0 ? error.message : 'transport or response failed',
          opaqueCause: error,
        });
  let event: WorkflowEvent;
  if (commitInvoked) {
    event = 'publication_uncertain';
  } else if (error instanceof WorkflowTransitionError) {
    event = error.event;
  } else if (application !== null) {
    event = classifier({ phase, method, applicationFailure: application, commitInvoked });
    if (!ALLOWED_APPLICATION_EVENTS.has(event)) throw new Error('host classified application failure outside the closed workflow events');
    const validForPhase = phase === 'review'
      ? event === 'stale_image_reference_or_source_drift' || event === 'semantic_review_rejection'
      : event === 'stale_image_reference_or_source_drift' || event === 'publish_precondition_rejection' || event === 'definite_pre_pivot_commit_failure' || event === 'publication_uncertain';
    if (!validForPhase) throw new Error('host classified application failure outside the current workflow phase');
  } else {
    event = commitInvoked ? 'publication_uncertain' : 'transport_or_response_uncertain_before_publication';
  }
  const outcomes: Record<WorkflowEvent, WorkflowOutcome> = {
    transport_or_response_uncertain_before_publication: 'transport_uncertain_no_publish_claim',
    stale_image_reference_or_source_drift: 'stale_subject',
    semantic_review_rejection: 'review_rejected',
    publish_precondition_rejection: 'publish_precondition_rejected',
    definite_pre_pivot_commit_failure: 'publish_failed_pre_pivot',
    publication_uncertain: 'publication_uncertain',
  };
  const transitionRepairOptions: WorkflowTransitionRepairOptions = event === 'semantic_review_rejection'
    ? ['start_new_review_with_different_intention']
    : [];
  return { status: 'failure', outcome: outcomes[event], event, phase, method, commitInvoked, blindRetry: false, compilerRepairOptions: [], transitionRepairOptions, failure: detail, transcript: transcriptSnapshot(transcript) };
}

function transcriptSnapshot(transcript: readonly WorkflowStepObservation[]): WorkflowTranscript {
  return Object.freeze([...transcript]);
}

function caller(
  codec: WorkflowCodec,
  transport: WorkflowTransport,
  phase: WorkflowPhaseId,
  steps: readonly BoundWorkflowStep[],
  transcript: WorkflowStepObservation[],
) {
  let sequence = 0;
  let stepPosition = 0;
  return async (method: string, params: object): Promise<ResultEnvelope> => {
    sequence += 1;
    const id = `${phase}:${sequence}`;
    let step = steps[stepPosition];
    if (step?.method !== method) {
      stepPosition += 1;
      step = steps[stepPosition];
    }
    if (step?.method !== method) throw new Error(`workflow ${phase} call order mismatch`);
    const selectedStep = step;
    const observe = (outcome: WorkflowStepObservation['outcome']): void => {
      transcript.push(Object.freeze({
        phase,
        stepIndex: selectedStep.index,
        requestId: id,
        method,
        outcome,
        responseContract: selectedStep.responseContract,
      }));
    };
    try {
      const frame = codec.request(id, method, params);
      if (!frame.endsWith('\n') || frame.slice(0, -1).includes('\n')) throw new Error('codec request is not one NDJSON frame');
      const response = await transport.exchange(frame);
      if (typeof response !== 'string' || response.length === 0) {
        throw new Error('transport response must be one nonempty JSON line');
      }
      const line = response.endsWith('\n') ? response.slice(0, -1) : response;
      if (line.length === 0 || line.includes('\n') || line.includes('\r')) throw new Error('transport response must be one nonempty JSON line');
      const decoded = codec.decodeTyped(line, method, id);
      if (decoded.protocol !== PROTOCOL) throw new Error('response protocol mismatch');
      observe('decoded_response');
      return decoded;
    } catch (error) {
      observe('failed_response');
      throw error;
    }
  };
}

async function chunks(
  call: (method: string, params: object) => Promise<ResultEnvelope>,
  method: string,
  fixed: MutableObject,
): Promise<string> {
  let offset = 0;
  let total: number | null = null;
  let result = '';
  for (let page = 0; page < 1024; page += 1) {
    const envelope = await call(method, { ...fixed, offset, chunk_bytes: 16384 });
    const payload = record(envelope.payload, `${method} chunk`);
    if (payload.offset !== offset || typeof payload.total_bytes !== 'number' || !Number.isSafeInteger(payload.total_bytes) || payload.total_bytes < 0 || typeof payload.chunk !== 'string') {
      throw new Error(`${method} chunk accounting mismatch`);
    }
    total ??= payload.total_bytes;
    if (payload.total_bytes !== total) throw new Error(`${method} total changed during paging`);
    result += payload.chunk;
    const bytes = new TextEncoder().encode(result).length;
    if (payload.next_offset === null) {
      if (bytes !== total) throw new Error(`${method} terminal byte count mismatch`);
      return result;
    }
    if (!Number.isSafeInteger(payload.next_offset) || payload.next_offset !== bytes || payload.next_offset <= offset || payload.next_offset > total) {
      throw new Error(`${method} next offset mismatch`);
    }
    offset = payload.next_offset as number;
  }
  throw new Error(`${method} page bound exceeded`);
}

export async function runReview(codec: WorkflowCodec, transport: WorkflowTransport, input: ReviewInput): Promise<ReviewResult> {
  const selected = workflow(codec);
  const sessionId = transportSession(transport);
  if (typeof input?.classifyFailure !== 'function') throw new Error('classifyFailure callback is required');
  const target = boundedText(input.target, 'signature target', 4096);
  const parameters = signatureParameters(input.parameters);
  const intention = { kind: 'change_function_signature', target, parameters };
  const transcript: WorkflowStepObservation[] = [];
  const call = caller(codec, transport, 'review', selected.reviewSteps, transcript);
  let method: string = REVIEW_METHODS[0];
  try {
    const openedEnvelope = await call(method, {});
    const opened = record(openedEnvelope.payload, 'workspace/open payload');
    const image = digest(opened.image_revision ?? openedEnvelope.image_revision, 'image revision');
    const project = digest(opened.project_revision ?? openedEnvelope.project_revision, 'project revision');
    method = REVIEW_METHODS[1];
    const reference = record((await call(method, { image_revision: image, target })).payload, 'function reference');
    const compactReference = canonical(reference);
    method = REVIEW_METHODS[2];
    const resolved = record((await call(method, { image_revision: image, reference: compactReference })).payload, 'resolved function');
    exactReference(reference, resolved, { image, project, target }, 'stale_image_reference_or_source_drift');
    method = REVIEW_METHODS[3];
    const baseCoverage = record((await call(method, { image_revision: image })).payload, 'base analysis coverage');
    exactCoverage(baseCoverage, { image, project });
    method = REVIEW_METHODS[4];
    const root = record((await call(method, { image_revision: image })).payload, 'candidate root');
    const rootRevision = digest(root.candidate_revision, 'root candidate revision');
    method = REVIEW_METHODS[5];
    const typedIntention = intentionText(target, parameters);
    const changed = record((await call(method, { image_revision: image, candidate_revision: rootRevision, intent: intention })).payload, 'changed candidate');
    const candidate = digest(changed.candidate_revision, 'candidate revision');
    method = REVIEW_METHODS[6];
    const validation = record((await call(method, { image_revision: image, candidate_revision: candidate })).payload, 'candidate validation');
    exactValidation(validation, candidate, 'semantic_review_rejection');
    method = REVIEW_METHODS[7];
    const semanticDelta = await chunks(call, method, { image_revision: image, candidate_revision: candidate, target });
    const semanticDeltaReport = record(JSON.parse(semanticDelta), 'semantic delta report');
    if (semanticDeltaReport.schema !== 'semaprax.project-candidate-semantic-delta.v1' ||
        semanticDeltaReport.candidate_digest !== candidate || semanticDeltaReport.target !== target) {
      throw new WorkflowTransitionError('semantic_review_rejection', 'semantic delta is not bound to the exact candidate and target');
    }
    method = REVIEW_METHODS[8];
    const testPlan = record((await call(method, { image_revision: image, candidate_revision: candidate })).payload, 'candidate test plan');
    if (testPlan.schema !== 'semaprax.project-candidate-test-plan.v1' ||
        testPlan.candidate_digest !== candidate || testPlan.execution !== 'not_run') {
      throw new WorkflowTransitionError('semantic_review_rejection', 'test plan is not bound to the exact unexecuted candidate');
    }
    method = REVIEW_METHODS[9];
    const testReport = record((await call(method, { image_revision: image, candidate_revision: candidate })).payload, 'candidate test report');
    if (testReport.schema !== 'semaprax.project-candidate-test-report.v1' ||
        testReport.candidate_digest !== candidate || testReport.passed !== true ||
        testReport.execution_scope !== 'complete_manifest_declared_test_closure' || !object(testReport.execution)) {
      throw new WorkflowTransitionError('semantic_review_rejection', 'candidate test report is not an exact passing complete-closure report');
    }
    method = REVIEW_METHODS[10];
    const sourceReview = await chunks(call, method, { image_revision: image, candidate_revision: candidate });
    const sourceReviewReport = record(JSON.parse(sourceReview), 'source review report');
    await exactSourceReview(sourceReviewReport, candidate, project, 'semantic_review_rejection');
    method = REVIEW_METHODS[11];
    const candidateCoverage = record((await call(method, { image_revision: image, candidate_revision: candidate })).payload, 'candidate analysis coverage');
    exactCoverage(candidateCoverage, { project, candidate });
    method = REVIEW_METHODS[12];
    const recoveryCapsule = await chunks(call, method, { image_revision: image, candidate_revision: candidate });
    const core: Omit<WorkflowHandoff, 'handoffSha256'> = {
      schema: 'semaprax.agent-workflow-handoff.v1', workflow: WORKFLOW_ID, protocol: PROTOCOL,
      reviewClientContractRevision: selected.clientContractRevision, reviewProfileRevision: selected.profileRevision,
      reviewSessionId: sessionId, imageRevision: image, projectRevision: project, candidateRevision: candidate, target, compactReference,
      resolvedFunction: resolved, typedIntention, validation, semanticDelta,
      semanticDeltaSha256: await sha256(semanticDelta), testPlan, testReport, sourceReview,
      sourceReviewSha256: await sha256(sourceReview), baseAnalysisCoverage: baseCoverage,
      baseAnalysisCoverageSha256: await sha256(canonical(baseCoverage)), candidateAnalysisCoverage: candidateCoverage,
      candidateAnalysisCoverageSha256: await sha256(canonical(candidateCoverage)), recoveryCapsule,
      recoveryCapsuleSha256: await sha256(recoveryCapsule), compilerRepairOptions: [],
    };
    const handoff: WorkflowHandoff = { ...core, handoffSha256: await sha256(canonical(core)) };
    return { status: 'ready', outcome: 'reviewed_candidate_and_source_backed_recovery_capsule', handoff, compilerRepairOptions: [], blindRetry: false, transcript: transcriptSnapshot(transcript) };
  } catch (error) {
    return failure(error, 'review', method, input.classifyFailure, false, transcript);
  }
}

export async function runPublish(
  codec: WorkflowCodec,
  transport: WorkflowTransport,
  handoff: WorkflowHandoff,
  inspectPublication: InspectPublication,
): Promise<PublishResult> {
  const selected = workflow(codec, true);
  const sessionId = transportSession(transport);
  if (typeof inspectPublication !== 'function' || typeof inspectPublication.classifyFailure !== 'function') throw new Error('inspectPublication and its classifyFailure callback are required');
  const transcript: WorkflowStepObservation[] = [];
  const call = caller(codec, transport, 'publish', selected.publishSteps, transcript);
  let method = 'handoff/preflight';
  let commitInvoked = false;
  try {
    if (!object(handoff) || handoff.schema !== 'semaprax.agent-workflow-handoff.v1' || handoff.workflow !== WORKFLOW_ID || handoff.protocol !== PROTOCOL) throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff contract mismatch');
    if (!same(Object.keys(handoff).sort(), [...HANDOFF_KEYS].sort())) throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff is not a closed object');
    if (sessionId === handoff.reviewSessionId) throw new WorkflowTransitionError('publish_precondition_rejection', 'publish requires a distinct session');
    digest(handoff.reviewClientContractRevision, 'review client contract revision');
    digest(handoff.reviewProfileRevision, 'review workflow profile revision');
    const { handoffSha256, ...core } = handoff;
    if (await sha256(canonical(core)) !== handoffSha256 ||
        await sha256(handoff.semanticDelta) !== handoff.semanticDeltaSha256 ||
        await sha256(handoff.sourceReview) !== handoff.sourceReviewSha256 ||
        await sha256(canonical(handoff.baseAnalysisCoverage)) !== handoff.baseAnalysisCoverageSha256 ||
        await sha256(canonical(handoff.candidateAnalysisCoverage)) !== handoff.candidateAnalysisCoverageSha256 ||
        await sha256(handoff.recoveryCapsule) !== handoff.recoveryCapsuleSha256 || handoff.compilerRepairOptions.length !== 0) {
      throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff replay digest mismatch');
    }
    const intention = record(JSON.parse(handoff.typedIntention), 'typed signature intention');
    if (intention.kind !== 'change_function_signature' || intention.target !== handoff.target || !Array.isArray(intention.parameters)) throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff signature intention mismatch');
    const normalizedParameters = signatureParameters(intention.parameters as SignatureParameter[]);
    if (handoff.typedIntention !== intentionText(handoff.target, normalizedParameters)) throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff signature intention is not canonical');
    const recovery = record(JSON.parse(handoff.recoveryCapsule), 'recovery capsule');
    const compactReference = record(JSON.parse(handoff.compactReference), 'compact function reference');
    const reviewedSource = await exactSourceReview(record(JSON.parse(handoff.sourceReview), 'source review report'), handoff.candidateRevision, handoff.projectRevision, 'publish_precondition_rejection');
    method = PUBLISH_METHODS[0];
    const opened = record((await call(method, {})).payload, 'workspace/open payload');
    if ((opened.image_revision ?? handoff.imageRevision) !== handoff.imageRevision || (opened.project_revision ?? handoff.projectRevision) !== handoff.projectRevision) throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'publish subject differs from reviewed subject');
    method = PUBLISH_METHODS[1];
    const resolved = record((await call(method, { image_revision: handoff.imageRevision, reference: handoff.compactReference })).payload, 'resolved function');
    exactReference(compactReference, resolved, { image: handoff.imageRevision, project: handoff.projectRevision, target: handoff.target }, 'publish_precondition_rejection');
    if (!same(resolved, handoff.resolvedFunction)) throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'function reference replay mismatch');
    method = PUBLISH_METHODS[2];
    const restored = record((await call(method, { image_revision: handoff.imageRevision, capsule: recovery })).payload, 'restored candidate');
    if (restored.candidate_revision !== handoff.candidateRevision || restored.base_revision !== handoff.projectRevision || restored.project_revision !== reviewedSource.candidateProjectRevision) throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'recovery replay mismatch');
    method = PUBLISH_METHODS[3];
    const validation = record((await call(method, { image_revision: handoff.imageRevision, candidate_revision: handoff.candidateRevision })).payload, 'candidate validation');
    exactValidation(validation, handoff.candidateRevision, 'publish_precondition_rejection');
    if (!same(validation, handoff.validation)) throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'validation replay mismatch');
    method = PUBLISH_METHODS[4];
    const review = await chunks(call, method, { image_revision: handoff.imageRevision, candidate_revision: handoff.candidateRevision });
    if (review !== handoff.sourceReview || await sha256(review) !== handoff.sourceReviewSha256) throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'source review replay mismatch');
    method = PUBLISH_METHODS[5];
    const pre = record((await call(method, { image_revision: handoff.imageRevision })).payload, 'precommit status');
    const { approval } = exactPrecommitStatus(pre, handoff.candidateRevision);
    method = PUBLISH_METHODS[6];
    commitInvoked = true;
    const committed = record((await call(method, { image_revision: handoff.imageRevision, candidate_revision: handoff.candidateRevision, approval_revision: approval })).payload, 'commit result');
    const reportRevision = exactCommitHandle(committed, handoff.candidateRevision, approval);
    method = PUBLISH_METHODS[7];
    const post = record((await call(method, { image_revision: handoff.imageRevision })).payload, 'postcommit status');
    exactPostcommitStatus(post, reportRevision);
    method = PUBLISH_METHODS[8];
    const receiptText = await chunks(call, method, { image_revision: handoff.imageRevision, report_revision: reportRevision });
    if (new TextEncoder().encode(receiptText).length !== committed.report_bytes) throw new WorkflowTransitionError('publication_uncertain', 'publication receipt byte count disagrees with the commit handle');
    if (await receiptRevision(receiptText) !== reportRevision) throw new WorkflowTransitionError('publication_uncertain', 'publication receipt revision is inconsistent');
    const receipt = record(JSON.parse(receiptText), 'publication receipt');
    exactReceipt(receipt, { candidate: handoff.candidateRevision, baseProject: handoff.projectRevision, candidateProject: reviewedSource.candidateProjectRevision, paths: reviewedSource.paths });
    const receiptSha256 = await sha256(receiptText);
    const finalTranscript = transcriptSnapshot(transcript);
    const inspected = await inspectPublication({ workflow: WORKFLOW_ID, candidateRevision: handoff.candidateRevision, approvalRevision: approval, reportRevision, precommitStatus: pre, commitHandle: committed, postcommitStatus: post, receipt, receiptSha256, publishClientContractRevision: selected.clientContractRevision, publishProfileRevision: selected.profileRevision, transcript: finalTranscript });
    if (inspected !== true) throw new WorkflowTransitionError('publication_uncertain', 'host publication inspection did not confirm the fixed ref and prepared commit');
    return { status: 'published', outcome: 'published', candidateRevision: handoff.candidateRevision, approvalRevision: approval, reportRevision, receipt, receiptSha256, publishClientContractRevision: selected.clientContractRevision, publishProfileRevision: selected.profileRevision, inspected: true, commitCalls: 1, blindRetry: false, transcript: finalTranscript };
  } catch (error) {
    const selectedError = method === 'handoff/preflight' && !(error instanceof WorkflowTransitionError)
      ? new WorkflowTransitionError('publish_precondition_rejection', 'handoff is malformed or incomplete')
      : error;
    return failure(selectedError, 'publish', method, inspectPublication.classifyFailure, commitInvoked, transcript);
  }
}
