export const WORKFLOW_ID = 'function_signature_review_publish_v1';
export const PROTOCOL = 'semaprax.image-agent-protocol.v5';
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
];
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
];
const HANDOFF_KEYS = [
    'schema', 'workflow', 'protocol', 'reviewClientContractRevision', 'reviewProfileRevision',
    'handoffSha256', 'reviewSessionId', 'imageRevision', 'projectRevision', 'candidateRevision',
    'target', 'compactReference', 'resolvedFunction', 'typedIntention', 'validation',
    'semanticDelta', 'semanticDeltaSha256', 'testPlan', 'testReport', 'sourceReview',
    'sourceReviewSha256', 'baseAnalysisCoverage', 'baseAnalysisCoverageSha256',
    'candidateAnalysisCoverage', 'candidateAnalysisCoverageSha256', 'recoveryCapsule',
    'recoveryCapsuleSha256', 'compilerRepairOptions',
];
function object(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}
function text(value, name) {
    if (typeof value !== 'string' || value.length === 0)
        throw new Error(`${name} must be nonempty`);
    return value;
}
function digest(value, name) {
    const selected = text(value, name);
    if (!/^sha256:[0-9a-f]{64}$/.test(selected))
        throw new Error(`${name} must be a SHA-256 digest`);
    return selected;
}
function record(value, name) {
    if (!object(value))
        throw new Error(`${name} must be an object`);
    return value;
}
function same(left, right) {
    if (left === right)
        return true;
    if (Array.isArray(left) && Array.isArray(right)) {
        return left.length === right.length && left.every((value, index) => same(value, right[index]));
    }
    if (object(left) && object(right)) {
        const keys = Object.keys(left);
        return keys.length === Object.keys(right).length && keys.every((key) => Object.hasOwn(right, key) && same(left[key], right[key]));
    }
    return false;
}
function canonical(value) {
    const ordered = (selected) => {
        if (Array.isArray(selected))
            return selected.map(ordered);
        if (!object(selected))
            return selected;
        return Object.fromEntries(Object.keys(selected).sort().map((key) => [key, ordered(selected[key])]));
    };
    return JSON.stringify(ordered(value));
}
function intentionText(target, parameters) {
    // The public constructor order is frozen separately from sorted digest JSON.
    return JSON.stringify({ kind: 'change_function_signature', target, parameters });
}
async function sha256(value) {
    const bytes = new TextEncoder().encode(value);
    const hash = await globalThis.crypto.subtle.digest('SHA-256', bytes);
    return `sha256:${Array.from(new Uint8Array(hash), (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}
async function domainDigest(domain, value) {
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
async function receiptRevision(value) {
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
function workflow(codec, requirePublish = false) {
    if (codec.PROTOCOL !== PROTOCOL)
        throw new Error('codec protocol mismatch');
    const contract = digest(codec.CLIENT_CONTRACT_REVISION, 'codec contract revision');
    if (!Array.isArray(codec.WORKFLOWS) || codec.WORKFLOWS.length !== 1)
        throw new Error('codec workflow inventory mismatch');
    const selected = record(codec.WORKFLOWS[0], 'supported workflow');
    if (selected.id !== WORKFLOW_ID)
        throw new Error('supported workflow id mismatch');
    const qualification = record(selected.qualification, 'workflow qualification');
    const profileRevision = digest(qualification.selected_profile_revision, 'selected workflow profile revision');
    const phases = selected.phases;
    if (!Array.isArray(phases))
        throw new Error('workflow phases missing');
    checkPhase(phases, 'review', REVIEW_METHODS);
    const publish = phases.find((phase) => object(phase) && phase.id === 'publish');
    if (requirePublish && publish === undefined)
        throw new Error('codec publish workflow phase missing');
    if (publish !== undefined)
        checkPhase(phases, 'publish', PUBLISH_METHODS);
    return { metadata: selected, clientContractRevision: contract, profileRevision };
}
function checkPhase(phases, id, methods) {
    const phase = phases.find((value) => object(value) && value.id === id);
    if (!object(phase) || !Array.isArray(phase.ordered_steps))
        throw new Error(`workflow ${id} phase missing`);
    const actual = phase.ordered_steps.map((step) => object(step) ? step.method : undefined);
    if (!same(actual, methods))
        throw new Error(`workflow ${id} method order mismatch`);
}
function transportSession(transport) {
    return text(transport.sessionId, 'transport sessionId');
}
function applicationFailure(error) {
    if (!object(error) || !object(error.rpc))
        return null;
    const rpc = error.rpc;
    if (!Number.isSafeInteger(rpc.code) || typeof rpc.message !== 'string' || !object(rpc.data))
        return null;
    if (rpc.data.schema !== 'semaprax.image-agent-application-error-data.v1' || !Array.isArray(rpc.data.diagnostics) || rpc.data.diagnostics.length === 0 || !rpc.data.diagnostics.every(diagnostic))
        return null;
    return rpc;
}
function diagnostic(value) {
    if (!object(value) || !same(Object.keys(value).sort(), ['code', 'help', 'location', 'message', 'path', 'severity'].sort()))
        return false;
    if (typeof value.code !== 'string' || typeof value.message !== 'string' || (value.severity !== 'error' && value.severity !== 'warning'))
        return false;
    if (value.path !== null && typeof value.path !== 'string')
        return false;
    if (value.help !== null && typeof value.help !== 'string')
        return false;
    if (value.location === null)
        return true;
    if (!object(value.location) || !same(Object.keys(value.location).sort(), ['line', 'column', 'start', 'end'].sort()))
        return false;
    const location = value.location;
    return ['line', 'column', 'start', 'end'].every((key) => Number.isSafeInteger(location[key]) && location[key] >= 0);
}
const RESERVED_NAMES = new Set(['module', 'use', 'fn', 'let', 'mut', 'if', 'else', 'while', 'match', 'true', 'false', 'requires', 'ensures', 'uses', 'permit', 'unsafe', 'return', 'own', 'borrow', 'shared', 'self', 'super']);
function boundedText(value, name, maximum) {
    const selected = text(value, name);
    if ([...selected].length > maximum || new TextEncoder().encode(selected).length > maximum)
        throw new Error(`${name} exceeds its bound`);
    return selected;
}
function identifier(value, name) {
    const selected = boundedText(value, name, 128);
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(selected) || RESERVED_NAMES.has(selected))
        throw new Error(`${name} is not an admitted identifier`);
    return selected;
}
function signatureParameters(values) {
    if (!Array.isArray(values) || values.length > 4096)
        throw new Error('signature parameters exceed their bound');
    return values.map((value, index) => {
        if (!object(value))
            throw new Error(`signature parameter ${index} must be an object`);
        if (Object.hasOwn(value, 'from')) {
            if (!same(Object.keys(value).sort(), (Object.hasOwn(value, 'name') ? ['from', 'name'] : ['from']).sort()))
                throw new Error(`signature parameter ${index} is not closed`);
            const from = boundedText(value.from, `signature parameter ${index} from`, 4096);
            return Object.hasOwn(value, 'name') ? { from, name: identifier(value.name, `signature parameter ${index} name`) } : { from };
        }
        if (!same(Object.keys(value).sort(), ['name', 'type', 'argument'].sort()))
            throw new Error(`signature parameter ${index} is not closed`);
        const name = identifier(value.name, `signature parameter ${index} name`);
        const kind = value.type;
        const argument = record(value.argument, `signature parameter ${index} argument`);
        if (argument.kind !== kind)
            throw new Error(`signature parameter ${index} literal kind mismatch`);
        let selected;
        if (kind === 'i64' || kind === 'i32' || kind === 'u8' || kind === 'usize') {
            if (!same(Object.keys(argument).sort(), ['kind', 'value'].sort()) || !Number.isSafeInteger(argument.value))
                throw new Error(`signature parameter ${index} requires a safe integer literal`);
            const numeric = argument.value;
            if ((kind === 'i32' && (numeric < -2147483648 || numeric > 2147483647)) || (kind === 'u8' && (numeric < 0 || numeric > 255)) || (kind === 'usize' && numeric < 0))
                throw new Error(`signature parameter ${index} integer literal is out of range`);
            selected = { kind, value: numeric };
        }
        else if (kind === 'bool') {
            if (!same(Object.keys(argument).sort(), ['kind', 'value'].sort()) || typeof argument.value !== 'boolean')
                throw new Error(`signature parameter ${index} requires a boolean literal`);
            selected = { kind, value: argument.value };
        }
        else if (kind === 'char' || kind === 'f32' || kind === 'f64') {
            const field = kind === 'char' ? 'scalar' : 'bits';
            const width = kind === 'f64' ? 16 : 8;
            if (!same(Object.keys(argument).sort(), ['kind', field].sort()) || typeof argument[field] !== 'string' || !new RegExp(`^[0-9a-f]{${width}}$`).test(argument[field]))
                throw new Error(`signature parameter ${index} requires an exact hexadecimal literal`);
            selected = { kind, [field]: argument[field] };
        }
        else {
            throw new Error(`signature parameter ${index} has an unsupported scalar type`);
        }
        return { name, type: kind, argument: selected };
    });
}
function exactValidation(value, candidate, event) {
    if (value.schema !== 'semaprax.image-candidate-validation.v1' ||
        value.candidate_revision !== candidate || value.independently_replayed !== true ||
        value.source_reparsed !== true || value.project_profile_admitted !== true ||
        value.tests !== 'not_run' || value.target_execution !== false ||
        value.commit_authority !== false) {
        throw new WorkflowTransitionError(event, 'candidate validation did not preserve the exact admitted replay facts');
    }
}
const REQUIRED_COVERAGE = {
    declared_source_inputs: ['known'],
    declared_external_contracts: ['partial', 'not_inspected'],
    deployment_configuration: ['not_inspected'],
    generated_file_provenance: ['not_inspected'],
    generated_artifacts: ['not_inspected'],
    external_api_behavior: ['not_inspected'],
    runtime_environment: ['not_inspected'],
    external_consumers: ['not_inspected'],
};
function exactCoverage(value, binding) {
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
    const seen = new Set();
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
function exactReference(reference, resolved, binding, event) {
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
async function exactSourceReview(value, candidate, base, event) {
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
    const paths = [];
    for (const [index, raw] of value.files.entries()) {
        const file = record(raw, `source review file ${index}`);
        const path = boundedText(file.path, `source review file ${index} path`, 240);
        if (paths.at(-1) !== undefined && paths.at(-1) >= path)
            throw new WorkflowTransitionError(event, 'source review file inventory is not canonical');
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
    if (paths.length === 0)
        throw new WorkflowTransitionError(event, 'source review contains no changed source files');
    return { candidateProjectRevision, paths };
}
function exactPrecommitStatus(value, candidate) {
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
function exactPostcommitStatus(value, report) {
    if (value.schema !== 'semaprax.image-source-commit-status.v1' || value.capability !== 'source_commit' ||
        value.authority !== 'startup_fixed_host_git_policy' || value.state !== 'published' ||
        value.pending_approval !== null || value.report_revision !== report ||
        !Array.isArray(value.last_error_codes) || value.last_error_codes.length !== 0 ||
        value.approval_via_request !== false || value.raw_working_tree_write !== false || value.host_state_only !== true) {
        throw new WorkflowTransitionError('publication_uncertain', 'postcommit status does not exactly confirm publication');
    }
}
function exactCommitHandle(value, candidate, approval) {
    if (value.schema !== 'semaprax.image-source-commit-handle.v1' || value.state !== 'published' ||
        value.candidate_revision !== candidate || value.approval_revision !== approval ||
        value.receipt_method !== 'candidate/commit-report' || value.raw_working_tree_write !== false ||
        value.source_commit_authority !== 'startup_fixed_host_git_policy' || !Number.isSafeInteger(value.report_bytes) || value.report_bytes <= 0) {
        throw new WorkflowTransitionError('publication_uncertain', 'commit handle does not exactly bind the publication');
    }
    return digest(value.report_revision, 'publication report revision');
}
function exactReceipt(value, binding) {
    const keys = ['schema', 'repository', 'reference', 'previous_commit', 'published_commit', 'tree', 'approved_candidate_digest', 'base_project_revision', 'candidate_project_revision', 'updated_source_paths', 'publication', 'git_object_format', 'working_tree_rewritten', 'project_manifest_changed', 'managed_active_changed', 'source_authority', 'tests', 'nonclaims'];
    const format = value.git_object_format;
    const width = format === 'sha1' ? 40 : format === 'sha256' ? 64 : 0;
    const oid = (candidate) => typeof candidate === 'string' && new RegExp(`^[0-9a-f]{${width}}$`).test(candidate);
    if (!same(Object.keys(value).sort(), keys.sort()) || value.schema !== 'semaprax.project-candidate-git-publication.v1' ||
        typeof value.repository !== 'string' || value.repository.length === 0 || typeof value.reference !== 'string' || !value.reference.startsWith('refs/heads/') ||
        width === 0 || !oid(value.previous_commit) || !oid(value.published_commit) || !oid(value.tree) || value.previous_commit === value.published_commit ||
        value.approved_candidate_digest !== binding.candidate || value.base_project_revision !== binding.baseProject ||
        value.candidate_project_revision !== binding.candidateProject || !same(value.updated_source_paths, binding.paths) ||
        value.publication !== 'git_branch_ref_compare_and_swap' || value.working_tree_rewritten !== false ||
        value.project_manifest_changed !== false || value.managed_active_changed !== false ||
        value.source_authority !== 'explicit_host_git_ref_authority' || value.tests !== 'not_run' ||
        !same(value.nonclaims, ['no_atomic_raw_working_tree_rewrite', 'no_network_push_or_remote_publication', 'no_signature_or_approval_service', 'unreachable_objects_may_remain_after_failure'])) {
        throw new WorkflowTransitionError('publication_uncertain', 'publication receipt is incomplete or disagrees with the reviewed source');
    }
}
const ALLOWED_APPLICATION_EVENTS = new Set([
    'stale_image_reference_or_source_drift',
    'semantic_review_rejection',
    'publish_precondition_rejection',
    'definite_pre_pivot_commit_failure',
    'publication_uncertain',
]);
class WorkflowTransitionError extends Error {
    event;
    constructor(event, message) {
        super(message);
        this.event = event;
        this.name = 'WorkflowTransitionError';
    }
}
function failure(error, phase, method, classifier, commitInvoked) {
    const application = applicationFailure(error);
    let event;
    if (commitInvoked) {
        event = 'publication_uncertain';
    }
    else if (error instanceof WorkflowTransitionError) {
        event = error.event;
    }
    else if (application !== null) {
        event = classifier({ phase, method, applicationFailure: application, commitInvoked });
        if (!ALLOWED_APPLICATION_EVENTS.has(event))
            throw new Error('host classified application failure outside the closed workflow events');
        const validForPhase = phase === 'review'
            ? event === 'stale_image_reference_or_source_drift' || event === 'semantic_review_rejection'
            : event === 'stale_image_reference_or_source_drift' || event === 'publish_precondition_rejection' || event === 'definite_pre_pivot_commit_failure' || event === 'publication_uncertain';
        if (!validForPhase)
            throw new Error('host classified application failure outside the current workflow phase');
    }
    else {
        event = commitInvoked ? 'publication_uncertain' : 'transport_or_response_uncertain_before_publication';
    }
    const outcomes = {
        transport_or_response_uncertain_before_publication: 'transport_uncertain_no_publish_claim',
        stale_image_reference_or_source_drift: 'stale_subject',
        semantic_review_rejection: 'review_rejected',
        publish_precondition_rejection: 'publish_precondition_rejected',
        definite_pre_pivot_commit_failure: 'publish_failed_pre_pivot',
        publication_uncertain: 'publication_uncertain',
    };
    const transitionRepairOptions = event === 'semantic_review_rejection'
        ? ['start_new_review_with_different_intention']
        : [];
    return { status: 'failure', outcome: outcomes[event], event, phase, method, commitInvoked, blindRetry: false, compilerRepairOptions: [], transitionRepairOptions, error };
}
function caller(codec, transport, phase) {
    let sequence = 0;
    return async (method, params) => {
        sequence += 1;
        const id = `${phase}:${sequence}`;
        const frame = codec.request(id, method, params);
        if (!frame.endsWith('\n') || frame.slice(0, -1).includes('\n'))
            throw new Error('codec request is not one NDJSON frame');
        const response = await transport.exchange(frame);
        if (typeof response !== 'string' || response.length === 0) {
            throw new Error('transport response must be one nonempty JSON line');
        }
        const line = response.endsWith('\n') ? response.slice(0, -1) : response;
        if (line.length === 0 || line.includes('\n') || line.includes('\r'))
            throw new Error('transport response must be one nonempty JSON line');
        const decoded = codec.decodeTyped(line, method, id);
        if (decoded.protocol !== PROTOCOL)
            throw new Error('response protocol mismatch');
        return decoded;
    };
}
async function chunks(call, method, fixed) {
    let offset = 0;
    let total = null;
    let result = '';
    for (let page = 0; page < 1024; page += 1) {
        const envelope = await call(method, { ...fixed, offset, chunk_bytes: 16384 });
        const payload = record(envelope.payload, `${method} chunk`);
        if (payload.offset !== offset || typeof payload.total_bytes !== 'number' || !Number.isSafeInteger(payload.total_bytes) || payload.total_bytes < 0 || typeof payload.chunk !== 'string') {
            throw new Error(`${method} chunk accounting mismatch`);
        }
        total ??= payload.total_bytes;
        if (payload.total_bytes !== total)
            throw new Error(`${method} total changed during paging`);
        result += payload.chunk;
        const bytes = new TextEncoder().encode(result).length;
        if (payload.next_offset === null) {
            if (bytes !== total)
                throw new Error(`${method} terminal byte count mismatch`);
            return result;
        }
        if (!Number.isSafeInteger(payload.next_offset) || payload.next_offset !== bytes || payload.next_offset <= offset || payload.next_offset > total) {
            throw new Error(`${method} next offset mismatch`);
        }
        offset = payload.next_offset;
    }
    throw new Error(`${method} page bound exceeded`);
}
export async function runReview(codec, transport, input) {
    const selected = workflow(codec);
    const sessionId = transportSession(transport);
    if (typeof input?.classifyFailure !== 'function')
        throw new Error('classifyFailure callback is required');
    const target = boundedText(input.target, 'signature target', 4096);
    const parameters = signatureParameters(input.parameters);
    const intention = { kind: 'change_function_signature', target, parameters };
    const call = caller(codec, transport, 'review');
    let method = REVIEW_METHODS[0];
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
        const core = {
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
        const handoff = { ...core, handoffSha256: await sha256(canonical(core)) };
        return { status: 'ready', outcome: 'reviewed_candidate_and_source_backed_recovery_capsule', handoff, compilerRepairOptions: [], blindRetry: false };
    }
    catch (error) {
        return failure(error, 'review', method, input.classifyFailure, false);
    }
}
export async function runPublish(codec, transport, handoff, inspectPublication) {
    const selected = workflow(codec, true);
    const sessionId = transportSession(transport);
    if (typeof inspectPublication !== 'function' || typeof inspectPublication.classifyFailure !== 'function')
        throw new Error('inspectPublication and its classifyFailure callback are required');
    const call = caller(codec, transport, 'publish');
    let method = 'handoff/preflight';
    let commitInvoked = false;
    try {
        if (!object(handoff) || handoff.schema !== 'semaprax.agent-workflow-handoff.v1' || handoff.workflow !== WORKFLOW_ID || handoff.protocol !== PROTOCOL)
            throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff contract mismatch');
        if (!same(Object.keys(handoff).sort(), [...HANDOFF_KEYS].sort()))
            throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff is not a closed object');
        if (sessionId === handoff.reviewSessionId)
            throw new WorkflowTransitionError('publish_precondition_rejection', 'publish requires a distinct session');
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
        if (intention.kind !== 'change_function_signature' || intention.target !== handoff.target || !Array.isArray(intention.parameters))
            throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff signature intention mismatch');
        const normalizedParameters = signatureParameters(intention.parameters);
        if (handoff.typedIntention !== intentionText(handoff.target, normalizedParameters))
            throw new WorkflowTransitionError('publish_precondition_rejection', 'handoff signature intention is not canonical');
        const recovery = record(JSON.parse(handoff.recoveryCapsule), 'recovery capsule');
        const compactReference = record(JSON.parse(handoff.compactReference), 'compact function reference');
        const reviewedSource = await exactSourceReview(record(JSON.parse(handoff.sourceReview), 'source review report'), handoff.candidateRevision, handoff.projectRevision, 'publish_precondition_rejection');
        method = PUBLISH_METHODS[0];
        const opened = record((await call(method, {})).payload, 'workspace/open payload');
        if ((opened.image_revision ?? handoff.imageRevision) !== handoff.imageRevision || (opened.project_revision ?? handoff.projectRevision) !== handoff.projectRevision)
            throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'publish subject differs from reviewed subject');
        method = PUBLISH_METHODS[1];
        const resolved = record((await call(method, { image_revision: handoff.imageRevision, reference: handoff.compactReference })).payload, 'resolved function');
        exactReference(compactReference, resolved, { image: handoff.imageRevision, project: handoff.projectRevision, target: handoff.target }, 'publish_precondition_rejection');
        if (!same(resolved, handoff.resolvedFunction))
            throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'function reference replay mismatch');
        method = PUBLISH_METHODS[2];
        const restored = record((await call(method, { image_revision: handoff.imageRevision, capsule: recovery })).payload, 'restored candidate');
        if (restored.candidate_revision !== handoff.candidateRevision || restored.base_revision !== handoff.projectRevision || restored.project_revision !== reviewedSource.candidateProjectRevision)
            throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'recovery replay mismatch');
        method = PUBLISH_METHODS[3];
        const validation = record((await call(method, { image_revision: handoff.imageRevision, candidate_revision: handoff.candidateRevision })).payload, 'candidate validation');
        exactValidation(validation, handoff.candidateRevision, 'publish_precondition_rejection');
        if (!same(validation, handoff.validation))
            throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'validation replay mismatch');
        method = PUBLISH_METHODS[4];
        const review = await chunks(call, method, { image_revision: handoff.imageRevision, candidate_revision: handoff.candidateRevision });
        if (review !== handoff.sourceReview || await sha256(review) !== handoff.sourceReviewSha256)
            throw new WorkflowTransitionError('stale_image_reference_or_source_drift', 'source review replay mismatch');
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
        if (new TextEncoder().encode(receiptText).length !== committed.report_bytes)
            throw new WorkflowTransitionError('publication_uncertain', 'publication receipt byte count disagrees with the commit handle');
        if (await receiptRevision(receiptText) !== reportRevision)
            throw new WorkflowTransitionError('publication_uncertain', 'publication receipt revision is inconsistent');
        const receipt = record(JSON.parse(receiptText), 'publication receipt');
        exactReceipt(receipt, { candidate: handoff.candidateRevision, baseProject: handoff.projectRevision, candidateProject: reviewedSource.candidateProjectRevision, paths: reviewedSource.paths });
        const receiptSha256 = await sha256(receiptText);
        const inspected = await inspectPublication({ workflow: WORKFLOW_ID, candidateRevision: handoff.candidateRevision, approvalRevision: approval, reportRevision, precommitStatus: pre, commitHandle: committed, postcommitStatus: post, receipt, receiptSha256, publishClientContractRevision: selected.clientContractRevision, publishProfileRevision: selected.profileRevision });
        if (inspected !== true)
            throw new WorkflowTransitionError('publication_uncertain', 'host publication inspection did not confirm the fixed ref and prepared commit');
        return { status: 'published', outcome: 'published', candidateRevision: handoff.candidateRevision, approvalRevision: approval, reportRevision, receipt, receiptSha256, publishClientContractRevision: selected.clientContractRevision, publishProfileRevision: selected.profileRevision, inspected: true, commitCalls: 1, blindRetry: false };
    }
    catch (error) {
        const selectedError = method === 'handoff/preflight' && !(error instanceof WorkflowTransitionError)
            ? new WorkflowTransitionError('publish_precondition_rejection', 'handoff is malformed or incomplete')
            : error;
        return failure(selectedError, 'publish', method, inspectPublication.classifyFailure, commitInvoked);
    }
}
