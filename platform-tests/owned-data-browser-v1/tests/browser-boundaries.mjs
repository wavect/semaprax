// Serialized by page.evaluate: deliberately self-contained, with no host globals
// supplying the runtime and no runtime-source rewriting or ownership mocks.
export async function runBrowserBoundaries({ packageUrl, variantPackageUrl }) {
  function check(condition, label) { if (!condition) throw new Error(label); }
  function equalBytes(actual, expected, label) {
    check(Object.getPrototypeOf(actual) === Uint8Array.prototype && actual.length === expected.length,
      `${label}: ordinary result type/length`);
    check(Object.getPrototypeOf(actual.buffer) === ArrayBuffer.prototype && actual.buffer.resizable === false,
      `${label}: ordinary fixed result backing`);
    for (let i = 0; i < expected.length; i++) check(actual[i] === expected[i], `${label}: byte ${i}`);
  }
  function rejects(operation, kind, message, label) {
    let caught = false;
    try { operation(); } catch (error) {
      caught = true;
      check(error instanceof kind, `${label}: wrong error class`);
      if (message !== undefined) check(error.message === message, `${label}: wrong error message`);
    }
    check(caught, `${label}: unexpectedly accepted`);
  }
  check(crossOriginIsolated === true, 'fixture requires cross-origin isolation');
  check(typeof SharedArrayBuffer === 'function', 'fixture requires SharedArrayBuffer');
  check(typeof ArrayBuffer.prototype.resize === 'function', 'fixture requires resizable ArrayBuffer');
  check(typeof structuredClone === 'function', 'fixture requires structuredClone transfer');
  const originalInstantiate = WebAssembly.instantiate;
  let instantiated = 0;
  WebAssembly.instantiate = function (...args) {
    instantiated++;
    return Reflect.apply(originalInstantiate, WebAssembly, args);
  };
  try {
    const ids = ['frame.fail-after', 'frame.fail-before', 'frame.mixed', 'frame.payload'];
    const parameters = [['borrow-slice-u8', 'i64'], ['borrow-slice-u8', 'i64'],
      ['bool', 'borrow-str', 'borrow-slice-u8'], ['borrow-slice-u8']];
    const results = ['owned-bytes', 'owned-bytes', 'owned-bytes', 'owned-bytes'];
    const variantIds = ['frame.maybe', 'frame.result'];
    async function loadPackage(url, name, exportIds, parameterTypes, resultTypes) {
      // Observe before importing the real generated module; authentication must
      // reject modified bytes before the engine gets an instantiation request.
      const module = await import(new URL('semaprax.bindings.js', url).href);
      const response = await fetch(new URL('app.wasm', url));
      check(response.ok, 'Wasm fetch must succeed');
      const wasm = new Uint8Array(await response.arrayBuffer());
      check(wasm.length > 8, 'Wasm fixture must not be empty');
      const metadataResponse = await fetch(new URL('semaprax.api.json', url));
      check(metadataResponse.ok, 'metadata fetch must succeed');
      const metadata = await metadataResponse.json();
      const packageResponse = await fetch(new URL('package.json', url));
      check(packageResponse.ok, 'package manifest fetch must succeed');
      const packageManifest = await packageResponse.json();
      check(packageManifest.name === name && packageManifest.version === '0.1.0' &&
        packageManifest.type === 'module', 'wrong package manifest');
      const declarationResponse = await fetch(new URL('semaprax.bindings.d.ts', url));
      check(declarationResponse.ok, 'TypeScript declarations fetch must succeed');
      check((await declarationResponse.text()).trim().length > 0, 'TypeScript declarations must not be empty');
      // Presence is not TypeScript compilation or a six-file provenance proof.
      check(metadata.schema === 'semaprax.owned-data-api.v1' &&
        metadata.package === name && metadata.version === '0.1.0', 'wrong fixture metadata');
      check(metadata.limits.borrowed_input_bytes === 65536 && metadata.limits.owned_output_bytes === 65536,
        'wrong published byte limits');
      check(metadata.wasm.path === 'app.wasm' && metadata.wasm.sha256 === module.wasmSha256,
        'metadata/runtime Wasm binding');
      const descriptor = JSON.parse(metadata.descriptor);
      check(descriptor.schema === 'semaprax.public-owned-data-api.v1' &&
        descriptor.project_schema === 'semaprax.project.v8', 'wrong descriptor profile');
      const encoder = new TextEncoder();
      const domain = encoder.encode('semaprax.public-owned-data-api.digest.v1\0');
      const descriptorBytes = encoder.encode(metadata.descriptor);
      const encoded = new Uint8Array(domain.length + 8 + descriptorBytes.length);
      encoded.set(domain);
      new DataView(encoded.buffer).setBigUint64(domain.length, BigInt(descriptorBytes.length), true);
      encoded.set(descriptorBytes, domain.length + 8);
      const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', encoded));
      check(metadata.descriptor_digest === 'sha256:' + Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join(''),
        'metadata descriptor digest binding');
      check(descriptor.exports.length === exportIds.length && metadata.target.length === exportIds.length,
        'metadata export inventory');
      for (let i = 0; i < exportIds.length; i++) {
        const declaration = descriptor.exports[i], target = metadata.target[i];
        check(declaration.stable_id === exportIds[i] && declaration.typescript_name === exportIds[i] &&
          declaration.result === resultTypes[i], `descriptor export ${i}`);
        check(JSON.stringify(declaration.parameters.map(value => value.type)) === JSON.stringify(parameterTypes[i]),
          `descriptor parameters ${i}`);
        check(target.stable_id === exportIds[i] && target.result === resultTypes[i] &&
          JSON.stringify(target.parameters) === JSON.stringify(parameterTypes[i]), `target shape ${i}`);
      }
      check(JSON.stringify(module.exportIds) === JSON.stringify(exportIds), 'wrong generated export inventory');
      return { module, wasm };
    }
    const basePackage = await loadPackage(packageUrl, 'owned-data-browser', ids, parameters, results);
    const variantPackage = await loadPackage(variantPackageUrl, 'owned-data-browser-variants', variantIds,
      [['borrow-slice-u8', 'bool'], ['borrow-slice-u8', 'bool']],
      ['option-owned-bytes', 'result-owned-bytes-i64']);
    const { module, wasm } = basePackage;
    // These are served-artifact consistency checks, not host-source provenance
    // or independent semantic replay of a Project descriptor in JavaScript.
    let tamperCases = 0;
    for (const candidate of [basePackage, variantPackage]) {
      const tampered = new Uint8Array(candidate.wasm);
      tampered[tampered.length - 1] ^= 1;
      let tamperRejected = false;
      try { await candidate.module.instantiate(tampered); } catch (error) {
        tamperRejected = true;
        check(error.message === 'SEMAPRAX WebAssembly artifact authentication failed', 'wrong authentication diagnostic');
      }
      check(tamperRejected && instantiated === 0, 'tampered Wasm reached engine instantiation');
      tamperCases++;
    }
    const api = await module.instantiate(wasm);
    check(instantiated === 1, 'genuine artifact must calibrate the instantiation observer');
    check(JSON.stringify(module.exportIds) === JSON.stringify(ids), 'wrong generated export inventory');
    check(Object.isFrozen(api) && Object.isFrozen(api.functions), 'facade must be frozen');
    check(Object.getPrototypeOf(api.functions) === null, 'function map must have null prototype');
    const variantApi = await variantPackage.module.instantiate(variantPackage.wasm);
    check(instantiated === 2, 'genuine variant artifact must calibrate the instantiation observer');
    check(Object.isFrozen(variantApi) && Object.isFrozen(variantApi.functions), 'variant facade must be frozen');
    check(Object.getPrototypeOf(variantApi.functions) === null, 'variant function map must have null prototype');
    const payload = api.functions['frame.payload'];
    const mixed = api.functions['frame.mixed'];
    const input = new Uint8Array([0, 255, 195, 40]);
    const output = payload(input);
    equalBytes(output, input, 'raw invalid-UTF8 Bytes');
    check(output !== input && output.buffer !== input.buffer, 'output must have independent storage');
    const original = [...output];
    input.fill(7);
    equalBytes(output, original, 'input mutation must not change output');
    output.fill(9);
    equalBytes(input, [7, 7, 7, 7], 'output mutation must not change input');
    function recover(label) { equalBytes(payload(new Uint8Array([0, 255, 3])), [0, 255, 3], label); }
    for (const length of [65535, 65536]) {
      const value = Uint8Array.from({ length }, (_, i) => i % 256);
      equalBytes(payload(value), value, `borrowed input capacity ${length}`);
    }
    rejects(() => payload(new Uint8Array(65537)), RangeError,
      'SEMAPRAX borrowed input capacity exceeded', 'borrowed input capacity +1');
    recover('reuse after oversized input');
    equalBytes(payload(new Uint8Array()), [], 'empty owned Bytes');

    let offsetCases = 0;
    const offsetBacking = new Uint8Array([91, 0, 255, 195, 40, 92]);
    const offsetView = new Uint8Array(offsetBacking.buffer, 1, 4);
    const offsetOutput = payload(offsetView);
    equalBytes(offsetOutput, [0, 255, 195, 40], 'nonzero-offset view');
    check(offsetOutput.buffer !== offsetBacking.buffer, 'offset result must own fresh storage');
    equalBytes(offsetBacking, [91, 0, 255, 195, 40, 92], 'whole offset backing preserved');
    offsetCases++;
    const emptyOffset = new Uint8Array(offsetBacking.buffer, 6, 0);
    const emptyOutput = payload(emptyOffset);
    equalBytes(emptyOutput, [], 'empty view at nonzero backing end');
    check(emptyOutput.buffer !== offsetBacking.buffer, 'empty offset result must own fresh storage');
    equalBytes(offsetBacking, [91, 0, 255, 195, 40, 92], 'empty offset backing preserved');
    offsetCases++;
    const capacityBacking = new Uint8Array(65538).fill(173);
    capacityBacking[0] = 91; capacityBacking[65537] = 92;
    const capacityView = new Uint8Array(capacityBacking.buffer, 1, 65536);
    const capacityOutput = payload(capacityView);
    equalBytes(capacityOutput, new Uint8Array(65536).fill(173), 'capacity applies to view, not backing');
    check(capacityOutput.buffer !== capacityBacking.buffer, 'capacity subview result must be fresh');
    check(capacityBacking[0] === 91 && capacityBacking[65537] === 92, 'capacity backing canaries');
    equalBytes(capacityView, new Uint8Array(65536).fill(173), 'capacity backing payload preserved');
    offsetCases++;
    offsetOutput.fill(7);
    equalBytes(offsetBacking, [91, 0, 255, 195, 40, 92], 'offset output mutation isolation');
    offsetView.fill(8);
    equalBytes(offsetOutput, [7, 7, 7, 7], 'offset input mutation isolation');

    // Genuine staged-owner branches; these count public calls/results, not
    // physical allocation or a mocked arena's owner-finalization events.
    let variantCalls = 0, variantActive = 0, variantInactive = 0;
    let variantOversized = 0, variantRecoveries = 0;
    const variantHeld = [];
    function variant(shape, bytes, active, label, counted = true) {
      const expected = [...bytes];
      const answer = variantApi.functions[shape === 0 ? 'frame.maybe' : 'frame.result'](bytes, active);
      if (counted) {
        variantCalls++;
        if (active) variantActive++; else variantInactive++;
      }
      let owned;
      if (shape === 0) {
        if (active) owned = answer;
        else check(answer === null, `${label}: None must be null`);
      } else {
        check(Object.getPrototypeOf(answer) === Object.prototype && Object.isFrozen(answer),
          `${label}: ordinary frozen Result`);
        check(JSON.stringify(Reflect.ownKeys(answer)) === JSON.stringify(active ? ['ok', 'value'] : ['ok', 'error']),
          `${label}: exact Result properties`);
        check(answer.ok === active, `${label}: exact Result discriminator`);
        if (active) owned = answer.value;
        else check(answer.error === -7n, `${label}: Err is a language value`);
      }
      equalBytes(bytes, expected, `${label}: input preserved`);
      if (active) {
        equalBytes(owned, expected, `${label}: active bytes`);
        check(owned !== bytes && owned.buffer !== bytes.buffer, `${label}: independent active copy`);
        for (const retained of variantHeld) {
          check(owned !== retained.bytes && owned.buffer !== retained.bytes.buffer,
            `${label}: distinct retained storage, including empty results`);
        }
        variantHeld.push({ bytes: owned, expected });
      }
      bytes.fill(93);
      if (active) equalBytes(owned, expected, `${label}: retained after input mutation`);
    }
    const variantCorpus = [new Uint8Array(), new Uint8Array([0, 255, 195, 40, 128]),
      Uint8Array.from({ length: 65535 }, (_, i) => i % 251),
      Uint8Array.from({ length: 65536 }, (_, i) => i % 251)];
    for (let round = 0; round < 4; round++) {
      for (const bytes of variantCorpus) {
        for (const shape of [0, 1]) {
          const label = `variant ${round}/${bytes.length}/${shape}`;
          variant(shape, new Uint8Array(bytes), true, `${label}/active`);
          variant(shape, new Uint8Array(bytes), false, `${label}/inactive`);
          variant(shape, new Uint8Array([255, 0, 128]), true, `${label}/recovery`);
        }
      }
    }
    for (const shape of [0, 1]) {
      for (const active of [true, false]) {
        const oversized = new Uint8Array(65537).fill(173);
        rejects(() => variantApi.functions[shape === 0 ? 'frame.maybe' : 'frame.result'](oversized, active),
          RangeError, 'SEMAPRAX borrowed input capacity exceeded', `variant oversized ${shape}/${active}`);
        variantOversized++;
        equalBytes(oversized, new Uint8Array(65537).fill(173), 'oversized variant input preserved');
        variant(shape, new Uint8Array([255, 0, 128]), true, 'variant oversized recovery', false);
        variantRecoveries++;
      }
    }

    // These literal bytes independently count BOM + NUL + euro + supplementary
    // scalar as 11 UTF-8 bytes. Both branches charge the unused borrowed input.
    const text = '\ufeff\0€😀';
    const textBytes = [239, 187, 191, 0, 226, 130, 172, 240, 159, 152, 128];
    for (const total of [65535, 65536, 65537]) {
      const bytes = new Uint8Array(total - textBytes.length).fill(173);
      for (const flag of [true, false]) {
        if (total <= 65536) equalBytes(mixed(flag, text, bytes), flag ? bytes : textBytes, `mixed ${flag}/${total}`);
        else rejects(() => mixed(flag, text, bytes), RangeError,
          'SEMAPRAX borrowed input capacity exceeded', `mixed unused input ${flag}`);
        recover(`reuse after mixed ${flag}/${total}`);
      }
    }
    rejects(() => mixed(true, '\ud800', new Uint8Array()), TypeError, undefined, 'unused invalid Unicode');
    recover('reuse after invalid Unicode');

    let hooks = 0;
    function forbiddenHook() { hooks++; throw new Error('caller hook must not execute'); }
    const shadowed = new Uint8Array([4, 0, 255]);
    for (const name of ['buffer', 'byteOffset', 'byteLength', Symbol.toStringTag]) {
      Object.defineProperty(shadowed, name, { get: forbiddenHook });
    }
    equalBytes(payload(shadowed), [4, 0, 255], 'captured intrinsic view getters');
    check(hooks === 0, 'shadowed own view getters must not run');
    const isolatedInput = new Uint8Array([4, 5, 0, 255]);
    const savedSet = Uint8Array.prototype.set;
    let setHooks = 0, isolatedOutput;
    Uint8Array.prototype.set = function () {
      setHooks++;
      isolatedInput.fill(99);
      throw new Error('caller mutation hook must not execute');
    };
    try { isolatedOutput = payload(isolatedInput); }
    finally { Uint8Array.prototype.set = savedSet; }
    check(setHooks === 0, 'snapshot must use captured set intrinsic');
    equalBytes(isolatedInput, [4, 5, 0, 255], 'snapshot input was not mutated');
    equalBytes(isolatedOutput, [4, 5, 0, 255], 'snapshot intrinsic isolation');
    const constructorGetter = new Uint8Array(3);
    Object.defineProperty(constructorGetter, 'constructor', { get: forbiddenHook });
    const bufferGetter = new Uint8Array(3);
    Object.defineProperty(bufferGetter.buffer, 'constructor', { get: forbiddenHook });
    const species = new Uint8Array(3);
    Object.defineProperty(species, 'constructor', { value: { get [Symbol.species]() { return forbiddenHook(); } } });
    const impostor = {};
    for (const name of ['buffer', 'byteOffset', 'byteLength']) Object.defineProperty(impostor, name, { get: forbiddenHook });
    class DerivedBytes extends Uint8Array {}
    const detached = new Uint8Array(3);
    structuredClone(detached.buffer, { transfer: [detached.buffer] });
    const shared = new Uint8Array(new SharedArrayBuffer(3));
    const resizableBuffer = new ArrayBuffer(3, { maxByteLength: 6 });
    check(resizableBuffer.resizable === true, 'resizable buffer construction must be effective');
    const fixedInputError = 'argument 0 must be an ordinary attached fixed Uint8Array';
    const hostile = [constructorGetter, bufferGetter, species, impostor,
      new DerivedBytes(3), new DataView(new ArrayBuffer(3)), new Uint16Array(3),
      new Proxy(new Uint8Array(3), {}), detached, shared, new Uint8Array(resizableBuffer)];
    for (let i = 0; i < hostile.length; i++) {
      rejects(() => payload(hostile[i]), TypeError, fixedInputError, `hostile byte input ${i}`);
      check(hooks === 0, `hostile byte input ${i} invoked a getter/species hook`);
      recover(`reuse after hostile byte input ${i}`);
    }

    const changingBuffer = new ArrayBuffer(8, { maxByteLength: 16 });
    check(changingBuffer.resizable === true && changingBuffer.maxByteLength === 16,
      'resize matrix requires actual resizable backing');
    const fixedChanging = new Uint8Array(changingBuffer, 4, 4);
    const trackingChanging = new Uint8Array(changingBuffer, 4);
    let resizableCases = 0;
    for (const [backingLength, fixedLength, trackingLength] of [[8, 4, 4], [6, 0, 2], [2, 0, 0], [8, 4, 4]]) {
      changingBuffer.resize(backingLength);
      check(changingBuffer.byteLength === backingLength && fixedChanging.byteLength === fixedLength &&
        trackingChanging.byteLength === trackingLength, `effective resize ${backingLength}`);
      for (const view of [fixedChanging, trackingChanging]) {
        rejects(() => payload(view), TypeError, fixedInputError, `resized view ${backingLength}/${resizableCases}`);
        resizableCases++;
        recover(`reuse after resized view ${resizableCases}`);
      }
    }

    const held = [];
    for (let round = 0; round < 16; round++) {
      for (const id of ['frame.fail-before', 'frame.fail-after']) {
        let caught = false;
        try { api.functions[id](new Uint8Array([round, 0, 255]), 0n); } catch (error) {
          caught = true;
          check(error instanceof Error && error.status === 4 &&
            error.message === 'SEMAPRAX semantic failure 4', `${id}: checked divide-by-zero status`);
        }
        check(caught, `${id}: division by zero must fail`);
        recover(`${id}: same-instance recovery ${round}`);
        equalBytes(api.functions[id](new Uint8Array([round, 0, 255]), 1n), [round, 0, 255], `${id}: successful control`);
      }
      held.push(payload(new Uint8Array([round, 0, 255])));
    }
    const second = await module.instantiate(wasm);
    check(instantiated === 3, 'second base artifact must instantiate independently');
    equalBytes(second.call('frame.payload', new Uint8Array([4, 3, 2])), [4, 3, 2], 'second facade');
    for (let round = 0; round < held.length; round++) equalBytes(held[round], [round, 0, 255], 'retained fresh output');
    check(variantHeld.length === 68, '64 corpus active copies plus four oversized recoveries');
    for (const retained of variantHeld) equalBytes(retained.bytes, retained.expected, 'variant retained through second facade');
    // Mutate one nonempty result and prove every other retained result remains
    // independent, including equal-valued recovery copies and empty outputs.
    variantHeld[1].bytes.fill(17);
    equalBytes(variantHeld[1].bytes, [17, 17, 17], 'variant output remains caller-mutable');
    for (let i = 0; i < variantHeld.length; i++) {
      if (i !== 1) equalBytes(variantHeld[i].bytes, variantHeld[i].expected, 'independent retained variant output');
    }
    recover('first facade after second facade');
    return { bytes: original, fresh: output !== input, instantiated, tamperCases, capacityCases: 3, failureRounds: 16,
      offsetCases, variantCalls, variantActive, variantInactive, variantOversized, variantRecoveries, resizableCases };
  } finally {
    WebAssembly.instantiate = originalInstantiate;
  }
}
