import { afterAll, beforeAll, expect, test } from "bun:test";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = mkdtempSync(join(tmpdir(), "jadpo-service-adapter-"));
const project = join(root, "application");
const source = resolve("tests/compile/pass/170_checked_service_operation.jadpo");
const sourceText = readFileSync(source, "utf8");
const route = `\nroute POST /deliver {\n    auth: none\n    input: ReminderMessage\n    output: ReminderReceipt\n    run: deliver(input)\n}\n`;
mkdirSync(project, { recursive: true });
writeFileSync(join(project, "app.jadpo"), `${sourceText}${route}`);
const assurance = join(project, "tests/assurance");
mkdirSync(assurance, { recursive: true });
cpSync(resolve("tests/assurance/service-reference-mail-v0.1.json"), join(assurance, "service-reference-mail-v0.1.json"));
const compiler = Bun.env.JADPO_BIN ?? resolve("jadpo/target/debug/jadpo");
const build = Bun.spawnSync([compiler, "build", project], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Service fixture failed to build:\n${build.stdout}\n${build.stderr}`);

const previousNodeEnv = process.env.NODE_ENV;
const previousMailKey = Bun.env.MAIL_API_KEY;
const canary = "service-adapter-secret-canary";
let provider = createServer();
let tlsProvider: ReturnType<typeof createHttpsServer> | undefined;
let wrongTlsProvider: ReturnType<typeof createHttpsServer> | undefined;
let providerPort = 0;
let tlsProviderPort = 0;
let wrongTlsProviderPort = 0;
let reply: (request: IncomingMessage, response: ServerResponse) => void = (_request, response) => {
  response.writeHead(500, { "content-type": "application/json" });
  response.end('{"code":"unexpected"}');
};
const received: Array<{ method: string; url: string; host: string | undefined; authorization: string | undefined; key: string | undefined; body: string }> = [];
const providerReceipts = new Map<string, { body: string; accepted_at: string }>();
let dropNextAcceptedResponse = false;
let app: any;
let testAdapter: any;
let serviceAudit: any;

function receiveProviderRequest(request: IncomingMessage, response: ServerResponse): void {
  const chunks: Buffer[] = [];
  request.on("data", chunk => chunks.push(Buffer.from(chunk)));
  request.on("end", () => {
    received.push({
      method: request.method ?? "",
      url: request.url ?? "",
      host: request.headers.host,
      authorization: request.headers.authorization,
      key: request.headers["idempotency-key"] as string | undefined,
      body: Buffer.concat(chunks).toString("utf8"),
    });
    reply(request, response);
  });
}

beforeAll(async () => {
  process.env.NODE_ENV = "test";
  Bun.env.MAIL_API_KEY = canary;
  provider = createServer(receiveProviderRequest);
  await new Promise<void>(resolveListen => provider.listen(0, "127.0.0.1", resolveListen));
  providerPort = (provider.address() as { port: number }).port;
  const fixtureDirectory = join(import.meta.dir, "fixtures/service-adapter");
  tlsProvider = createHttpsServer({
    key: readFileSync(join(fixtureDirectory, "test-key.pem")),
    cert: readFileSync(join(fixtureDirectory, "test-cert.pem")),
  }, receiveProviderRequest);
  await new Promise<void>(resolveListen => tlsProvider!.listen(0, "127.0.0.1", resolveListen));
  tlsProviderPort = (tlsProvider.address() as { port: number }).port;
  wrongTlsProvider = createHttpsServer({
    key: readFileSync(join(fixtureDirectory, "wrong-host-key.pem")),
    cert: readFileSync(join(fixtureDirectory, "wrong-host-cert.pem")),
  }, receiveProviderRequest);
  await new Promise<void>(resolveListen => wrongTlsProvider!.listen(0, "127.0.0.1", resolveListen));
  wrongTlsProviderPort = (wrongTlsProvider.address() as { port: number }).port;
const target = join(project, "build/target");
app = await import(pathToFileURL(join(target, "app.ts")).href);
testAdapter = await import(pathToFileURL(join(target, "service-adapter.ts")).href);
serviceAudit = JSON.parse(readFileSync(join(project, "build/audit/services.json"), "utf8"));
testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
});

afterAll(() => {
  testAdapter?.setReferenceMailEndpointForTesting(null);
  provider.close();
  tlsProvider?.close();
  wrongTlsProvider?.close();
  if (previousNodeEnv === undefined) delete process.env.NODE_ENV;
  else process.env.NODE_ENV = previousNodeEnv;
  if (previousMailKey === undefined) delete Bun.env.MAIL_API_KEY;
  else Bun.env.MAIL_API_KEY = previousMailKey;
  rmSync(root, { recursive: true, force: true });
});

function message(key = "00000000-0000-4000-8000-000000000001") {
  return {
    idempotency_key: key,
    from: "todo@example.test",
    to: "person@example.test",
    todo_title: "Pay rent",
    due_at: "2026-10-02T12:00:00.000Z",
  };
}

function request(body: unknown, signal?: AbortSignal): Request {
  return new Request("https://application.test/deliver", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    ...(signal === undefined ? {} : { signal }),
  });
}

function send(status: number, body: unknown, key?: string, location?: string): void {
  reply = (_request, response) => {
    response.writeHead(status, {
      "content-type": "application/json",
      ...(key === undefined ? {} : { "idempotency-key": key }),
      ...(location === undefined ? {} : { location }),
    });
    response.end(JSON.stringify(body));
  };
}

function acceptWithDeduplication(request: IncomingMessage, response: ServerResponse): void {
  const receivedRequest = received.at(-1)!;
  const key = receivedRequest.key ?? "";
  const prior = providerReceipts.get(key);
  if (prior !== undefined && prior.body !== receivedRequest.body) {
    response.writeHead(409, { "content-type": "application/json" });
    response.end('{"code":"idempotency_conflict"}');
    return;
  }
  const receipt = prior ?? { body: receivedRequest.body, accepted_at: "2026-10-03T20:00:00.000Z" };
  if (prior === undefined) providerReceipts.set(key, receipt);
  if (dropNextAcceptedResponse) {
    dropNextAcceptedResponse = false;
    response.destroy();
    return;
  }
  response.writeHead(202, { "content-type": "application/json", "idempotency-key": key });
  response.end(JSON.stringify({ accepted_at: receipt.accepted_at }));
  void request;
}

function serviceContext(monotonicStartedAt = performance.now()) {
  return {
    operationId: "service-adapter-test-operation",
    monotonicStartedAt,
    serviceAttemptBudget: { attemptsUsed: 0 },
    signal: null,
  };
}

test("keeps the emitted service audit pinned and honest about broader runtime conformance", () => {
  expect(serviceAudit.kind).toBe("service_effect_contract");
  expect(serviceAudit.runtime_conformance).toBe("not_established");
  expect(serviceAudit.effects).toHaveLength(1);
  expect(serviceAudit.effects[0]).toMatchObject({
    service: "ReminderMail",
    operation: "send_overdue_reminder",
    method: "POST",
    path: "/v1/messages",
    input: "ReminderMessage",
    output: "ReminderReceipt",
    idempotency_type: "ReminderIntentId",
    egress: "mail.example.invalid:443",
    credential_slot: "config.mail_api_key",
    credential_header: "Authorization",
    timeout_ms: 5_000,
    proxy_allowed: false,
    redirects_allowed: false,
    retry: { max_attempts: 3, max_elapsed_ms: 30_000, jitter: "full" },
  });
  expect(JSON.stringify(serviceAudit)).not.toContain(canary);
});

function attemptCount(): number { return received.length; }

test("pins host, path, body and idempotency header and returns a checked receipt", async () => {
  received.length = 0;
  const input = message();
  send(202, { accepted_at: "2026-10-03T20:00:00.000Z" }, input.idempotency_key);
  const response = await app.handleRequest(request(input));
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({ accepted_at: "2026-10-03T20:00:00.000Z" });
  expect(received).toHaveLength(1);
  expect(received[0]).toMatchObject({
    method: "POST",
    url: "/v1/messages",
    host: "mail.example.invalid",
    authorization: `Bearer ${canary}`,
    key: input.idempotency_key,
    body: JSON.stringify(input),
  });
});

test("uses verified TLS and the pinned DNS identity for provider egress", async () => {
  received.length = 0;
  const input = message("00000000-0000-4000-8000-000000000003");
  send(202, { accepted_at: "2026-10-03T20:00:00.000Z" }, input.idempotency_key);
  const cert = readFileSync(join(import.meta.dir, "fixtures/service-adapter/test-cert.pem"), "utf8");
  testAdapter.setReferenceMailEndpointForTesting(`https://127.0.0.1:${tlsProviderPort}/`, cert);
  try {
    const response = await app.handleRequest(request(input));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ accepted_at: "2026-10-03T20:00:00.000Z" });
    expect(received).toHaveLength(1);
    expect(received[0]).toMatchObject({ host: "mail.example.invalid", url: "/v1/messages" });
  } finally {
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
  }
});

test("rejects a trusted TLS certificate whose hostname does not match the pinned provider", async () => {
  received.length = 0;
  const wrongCert = readFileSync(join(import.meta.dir, "fixtures/service-adapter/wrong-host-cert.pem"), "utf8");
  testAdapter.setReferenceMailEndpointForTesting(`https://127.0.0.1:${wrongTlsProviderPort}/`, wrongCert);
  try {
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(503);
    expect((await response.json()).error.code).toBe("service_unavailable");
    expect(received).toHaveLength(0);
  } finally {
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
  }
});

test("fails closed and never writes HTTP when a TLS handshake fails", async () => {
  const failedHandshake = createServer(socket => {
    socket.write("not a TLS handshake");
    socket.destroy();
  });
  await new Promise<void>(resolveListen => failedHandshake.listen(0, "127.0.0.1", resolveListen));
  const port = (failedHandshake.address() as { port: number }).port;
  const cert = readFileSync(join(import.meta.dir, "fixtures/service-adapter/test-cert.pem"), "utf8");
  received.length = 0;
  testAdapter.setReferenceMailEndpointForTesting(`https://127.0.0.1:${port}/`, cert);
  try {
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(503);
    expect((await response.json()).error.code).toBe("service_unavailable");
    expect(received).toHaveLength(0);
  } finally {
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
    await new Promise<void>(resolveClose => failedHandshake.close(() => resolveClose()));
  }
});

test("retries a TLS handshake timeout only before any HTTP write", async () => {
  const stalledTls = createServer(socket => socket.on("error", () => {}));
  await new Promise<void>(resolveListen => stalledTls.listen(0, "127.0.0.1", resolveListen));
  const port = (stalledTls.address() as { port: number }).port;
  const cert = readFileSync(join(import.meta.dir, "fixtures/service-adapter/test-cert.pem"), "utf8");
  received.length = 0;
  const lines: string[] = [];
  const previousError = console.error;
  console.error = value => { lines.push(String(value)); };
  testAdapter.setReferenceMailEndpointForTesting(`https://127.0.0.1:${port}/`, cert, 30);
  try {
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(503);
    expect((await response.json()).error.code).toBe("service_unavailable");
    expect(received).toHaveLength(0);
    expect(lines.filter(line => JSON.parse(line).eventName === "service.attempt").map(line => JSON.parse(line).outcome))
      .toEqual(["pre_dispatch_failure", "pre_dispatch_failure", "pre_dispatch_failure"]);
  } finally {
    console.error = previousError;
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
    await new Promise<void>(resolveClose => stalledTls.close(() => resolveClose()));
  }
});

test("test transport cannot target non-loopback or host-selected egress", () => {
  expect(() => testAdapter.setReferenceMailEndpointForTesting("http://192.0.2.1:8080/")).toThrow();
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = "production";
  try {
    expect(() => testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`)).toThrow();
  } finally {
    if (previous === undefined) delete process.env.NODE_ENV;
    else process.env.NODE_ENV = previous;
  }
});

test("maps only the exact recipient response and keeps provider text private", async () => {
  received.length = 0;
  send(400, { code: "invalid_recipient" });
  const response = await app.handleRequest(request(message("00000000-0000-4000-8000-000000000002")));
  expect(response.status).toBe(422);
  const body = await response.json();
  expect(body.error.code).toBe("recipient_rejected");
  expect(JSON.stringify(body)).not.toContain("invalid_recipient");
  expect(attemptCount()).toBe(1);
});

test("contains authentication and idempotency conflicts as misconfiguration", async () => {
  for (const [status, code] of [[401, "authentication"], [409, "idempotency_conflict"]] as const) {
    received.length = 0;
    send(status, { code });
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(500);
    expect((await response.json()).error.code).toBe("internal_fault");
    expect(attemptCount()).toBe(1);
  }
});

test("retries only explicit no-acceptance rate limits and preserves the exact key and payload", async () => {
  received.length = 0;
  send(429, { code: "rate_limited" });
  const response = await app.handleRequest(request(message()));
  expect(response.status).toBe(503);
  expect((await response.json()).error.code).toBe("temporarily_unavailable");
  expect(received).toHaveLength(3);
  expect(new Set(received.map(item => item.key))).toEqual(new Set([message().idempotency_key]));
  expect(new Set(received.map(item => item.body))).toEqual(new Set([JSON.stringify(message())]));
});

test("shares one elapsed deadline and three-attempt budget across adapter calls", async () => {
  received.length = 0;
  const context = serviceContext();
  testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`, null, 5_000, 300);
  try {
    reply = (_request, response) => {
      response.writeHead(202, { "content-type": "application/json", "idempotency-key": message("00000000-0000-4000-8000-000000000010").idempotency_key });
      setTimeout(() => response.end('{"accepted_at":"2026-10-03T20:00:00.000Z"}'), 220);
    };
    const first = await testAdapter.invokeReferenceMail(message("00000000-0000-4000-8000-000000000010"), canary, context);
    expect(first).toEqual({ kind: "accepted", accepted_at: "2026-10-03T20:00:00.000Z" });
    expect(context.serviceAttemptBudget.attemptsUsed).toBe(1);

    reply = () => {};
    const secondCallStarted = performance.now();
    await expect(testAdapter.invokeReferenceMail(message("00000000-0000-4000-8000-000000000011"), canary, context))
      .rejects.toMatchObject({ kind: "outcome_unknown" });
    expect(performance.now() - secondCallStarted).toBeLessThan(180);
    expect(received).toHaveLength(2);
    expect(context.serviceAttemptBudget.attemptsUsed).toBe(2);
  } finally {
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
  }

  received.length = 0;
  const retryContext = serviceContext();
  send(429, { code: "rate_limited" });
  const firstRetryableCall = await testAdapter.invokeReferenceMail(message(), canary, retryContext);
  expect(firstRetryableCall).toEqual({ kind: "temporarily_unavailable" });
  expect(retryContext.serviceAttemptBudget.attemptsUsed).toBe(3);
  await expect(testAdapter.invokeReferenceMail(message("00000000-0000-4000-8000-000000000012"), canary, retryContext))
    .rejects.toMatchObject({ kind: "unavailable" });
  expect(received).toHaveLength(3);
});

test("bounds credential bytes before constructing or dispatching the HTTP request", async () => {
  received.length = 0;
  await expect(testAdapter.invokeReferenceMail(message(), "x".repeat(4 * 1024 + 1), serviceContext()))
    .rejects.toMatchObject({ kind: "misconfigured" });
  expect(received).toHaveLength(0);
});

test("a timeout after the request is written is unknown and never retried", async () => {
  received.length = 0;
  reply = () => {};
  const previousError = console.error;
  const lines: string[] = [];
  console.error = value => { lines.push(String(value)); };
  testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`, null, 30);
  try {
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(500);
    expect((await response.json()).error.code).toBe("outcome_unknown");
    expect(received).toHaveLength(1);
    expect(lines.filter(line => JSON.parse(line).eventName === "service.attempt").map(line => JSON.parse(line).outcome))
      .toEqual(["outcome_unknown"]);
  } finally {
    console.error = previousError;
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
  }
});

test("cancellation after dispatch is unknown and stops all further attempts", async () => {
  received.length = 0;
  const controller = new AbortController();
  reply = (_request, response) => {
    controller.abort();
    response.destroy();
  };
  const response = await app.handleRequest(request(message(), controller.signal));
  expect(response.status).toBe(500);
  expect((await response.json()).error.code).toBe("outcome_unknown");
  expect(received).toHaveLength(1);
});

test("retries a connection refusal known to occur before HTTP dispatch", async () => {
  const closed = createServer();
  await new Promise<void>(resolveListen => closed.listen(0, "127.0.0.1", resolveListen));
  const port = (closed.address() as { port: number }).port;
  await new Promise<void>(resolveClose => closed.close(() => resolveClose()));
  received.length = 0;
  testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${port}/`);
  try {
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(503);
    expect((await response.json()).error.code).toBe("service_unavailable");
    expect(received).toHaveLength(0);
  } finally {
    testAdapter.setReferenceMailEndpointForTesting(`http://127.0.0.1:${providerPort}/`);
  }
});

test("treats a mismatched receipt, redirect and malformed response as unknown without retry", async () => {
  const cases: Array<(key: string) => void> = [
    key => send(202, { accepted_at: "2026-10-03T20:00:00.000Z" }, `${key.slice(0, -1)}3`),
    () => send(307, { code: "redirect" }, undefined, "http://127.0.0.1/other"),
    () => send(202, { accepted_at: "not-an-instant" }, message().idempotency_key),
  ];
  for (const configure of cases) {
    received.length = 0;
    configure(message().idempotency_key);
    const response = await app.handleRequest(request(message()));
    expect(response.status).toBe(500);
    expect((await response.json()).error.code).toBe("outcome_unknown");
    expect(received).toHaveLength(1);
  }
});

test("classifies a committed provider acceptance with a lost acknowledgement as unknown", async () => {
  received.length = 0;
  providerReceipts.clear();
  dropNextAcceptedResponse = true;
  reply = acceptWithDeduplication;
  const input = message("00000000-0000-4000-8000-000000000013");
  const response = await app.handleRequest(request(input));
  expect(response.status).toBe(500);
  expect((await response.json()).error.code).toBe("outcome_unknown");
  expect(received).toHaveLength(1);
  expect(providerReceipts.get(input.idempotency_key)).toEqual({
    body: JSON.stringify(input),
    accepted_at: "2026-10-03T20:00:00.000Z",
  });
});

test("deduplicates an exact key and body using the stored provider receipt", async () => {
  received.length = 0;
  providerReceipts.clear();
  reply = acceptWithDeduplication;
  const input = message("00000000-0000-4000-8000-000000000014");
  const first = await testAdapter.invokeReferenceMail(input, canary, serviceContext());
  const duplicate = await testAdapter.invokeReferenceMail(input, canary, serviceContext());
  expect(first).toEqual(duplicate);
  expect(received).toHaveLength(2);
  expect(providerReceipts.size).toBe(1);
  await expect(testAdapter.invokeReferenceMail({ ...input, todo_title: "Changed payload" }, canary, serviceContext()))
    .rejects.toMatchObject({ kind: "misconfigured" });
  expect(received).toHaveLength(3);
  expect(providerReceipts.size).toBe(1);
});

test("never exposes provider credentials or response contents in safe attempt logs", async () => {
  const previousError = console.error;
  const lines: string[] = [];
  console.error = value => { lines.push(String(value)); };
  try {
    received.length = 0;
    send(400, { code: "invalid_recipient", message: canary });
    await app.handleRequest(request(message()));
  } finally {
    console.error = previousError;
  }
  expect(lines).toHaveLength(2);
  expect(lines[0]).not.toContain(canary);
  expect(lines[0]).not.toContain("person@example.test");
  expect(lines[0]).not.toContain("invalid_recipient");
  expect(lines[1]).not.toContain(canary);
  expect(lines[1]).not.toContain("person@example.test");
  expect(lines[1]).not.toContain("invalid_recipient");
  expect(JSON.parse(lines[0])).toMatchObject({ eventName: "service.attempt", attempt: 1, outcome: "outcome_unknown" });
  expect(JSON.parse(lines[1])).toMatchObject({ eventName: "operation.failed", classification: "RUNTIME_OUTCOME_UNKNOWN", attributes: {} });
});
