import {
  createFixture,
  createInvocation,
  instantiate,
} from "/generated/semaprax.bindings.js";

const fixtureDocument = {
  schema: "semaprax.network-fixture.v3",
  connections: [],
  listeners: [],
  https: [
    {
      url: "https://example.test/data",
      response: "HTTP/1.1 200 semaprax\r\ncontent-length: 2\r\n\r\nok",
    },
  ],
};

async function run() {
  const wasm = new Uint8Array(await (await fetch("/generated/app.wasm")).arrayBuffer());
  const fixture = createFixture(fixtureDocument);
  const invocation = createInvocation([], new Uint8Array(), fixture);
  const result = await instantiate(wasm, invocation);
  let reuseRejected = false;
  try {
    await instantiate(wasm, invocation);
  } catch (error) {
    reuseRejected = error instanceof TypeError;
  }
  const tampered = new Uint8Array(wasm);
  tampered[tampered.length - 1] ^= 1;
  let authenticationRejected = false;
  try {
    await instantiate(
      tampered,
      createInvocation([], new Uint8Array(), createFixture(fixtureDocument)),
    );
  } catch (error) {
    authenticationRejected = error instanceof Error && error.message === "Wasm authentication";
  }
  if (!result.result || !reuseRejected || !authenticationRejected || result.stderr.length !== 0) {
    throw new Error("HTTPS browser contract failed");
  }
  const stdout = new TextDecoder("utf-8", { fatal: true }).decode(result.stdout);
  document.querySelector("#stdout").textContent = stdout;
  document.querySelector("#status").textContent = "passed";
  return { stdout, reuseRejected, authenticationRejected };
}

globalThis.semapraxHttpsResult = run();
