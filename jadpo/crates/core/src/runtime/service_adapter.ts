import { isIP } from "node:net";

const SERVICE_HOST = "mail.example.invalid";
const SERVICE_PATH = "/v1/messages";
const ATTEMPT_TIMEOUT_MS = 5_000;
const MAX_ATTEMPTS = 3;
const MAX_ELAPSED_MS = 30_000;
const MAX_HEADER_BYTES = 8 * 1024;
const MAX_BODY_BYTES = 16 * 1024;
const MAX_CREDENTIAL_BYTES = 4 * 1024;

export type ServiceOperationContext = Readonly<{
  operationId: string;
  monotonicStartedAt: number;
  deadlineAt?: number | null;
  serviceAttemptBudget: { attemptsUsed: number };
  signal: AbortSignal | null;
}>;

export type ReferenceMailResult =
  | Readonly<{ kind: "accepted"; accepted_at: string }>
  | Readonly<{ kind: "recipient_rejected" }>
  | Readonly<{ kind: "temporarily_unavailable" }>;

export type ReferenceMailTestOutcome =
  | Readonly<{ kind: "accepted"; value: Readonly<Record<string, unknown>> }>
  | Readonly<{ kind: "declared"; name: string }>;

export type ReferenceMailTestFake = {
  readonly outcomes: readonly ReferenceMailTestOutcome[];
  readonly cursor: { index: number };
  elapsedMs: number;
};

// A fixture sequence is shared by its test; elapsed budgets belong to each
// logical operation. Nested calls share the mutable attempt-budget identity.
const fakeOperationBaselines = new WeakMap<object, WeakMap<ReferenceMailTestFake, number>>();

export class ServiceAdapterFault extends Error {
  constructor(readonly kind: "unavailable" | "deadline_exceeded" | "misconfigured" | "outcome_unknown") {
    super(`external service ${kind}`);
    this.name = "ServiceAdapterFault";
  }
}

let testEndpoint: URL | null = null;
let testCa: string | null = null;
let testAttemptTimeoutMs: number | null = null;
let testOperationTimeoutMs: number | null = null;

/** Test-runner-only loopback redirect. Authored Jadpo configuration cannot set it. */
export function setReferenceMailEndpointForTesting(
  value: string | null,
  ca: string | null = null,
  attemptTimeoutMs: number | null = null,
  operationTimeoutMs: number | null = null,
): void {
  if (process.env.NODE_ENV !== "test") throw new Error("test transport is unavailable");
  if (value === null) {
    testEndpoint = null;
    testCa = null;
    testAttemptTimeoutMs = null;
    testOperationTimeoutMs = null;
    return;
  }
  const endpoint = new URL(value);
  const hostname = endpoint.hostname.startsWith("[")
    ? endpoint.hostname.slice(1, -1)
    : endpoint.hostname;
  const octets = hostname.split(".").map(Number);
  const loopbackV4 = isIP(hostname) === 4 && octets.length === 4 && octets[0] === 127 && octets.every(part => Number.isInteger(part) && part >= 0 && part <= 255);
  if (
    endpoint.protocol !== "http:" && endpoint.protocol !== "https:" ||
    endpoint.username !== "" ||
    endpoint.password !== "" ||
    endpoint.pathname !== "/" ||
    endpoint.search !== "" ||
    endpoint.hash !== "" ||
    endpoint.port === "" ||
    (endpoint.protocol === "https:" && (ca === null || ca.length === 0)) ||
    (endpoint.protocol === "http:" && ca !== null) ||
    (attemptTimeoutMs !== null && (!Number.isInteger(attemptTimeoutMs) || attemptTimeoutMs < 1 || attemptTimeoutMs > ATTEMPT_TIMEOUT_MS)) ||
    (operationTimeoutMs !== null && (!Number.isInteger(operationTimeoutMs) || operationTimeoutMs < 1 || operationTimeoutMs > MAX_ELAPSED_MS)) ||
    (!loopbackV4 && hostname !== "::1")
  ) {
    throw new Error("test transport must use a loopback HTTP origin with an explicit port");
  }
  testEndpoint = endpoint;
  testCa = ca;
  testAttemptTimeoutMs = attemptTimeoutMs;
  testOperationTimeoutMs = operationTimeoutMs;
}

type AttemptResult =
  | Readonly<{ kind: "accepted"; accepted_at: string }>
  | Readonly<{ kind: "recipient_rejected" }>
  | Readonly<{ kind: "rate_limited" }>
  | Readonly<{ kind: "pre_dispatch_failure" }>
  | Readonly<{ kind: "misconfigured" }>
  | Readonly<{ kind: "outcome_unknown" }>;

type ParsedResponse = Readonly<{
  status: number;
  headers: ReadonlyMap<string, readonly string[]>;
  body: Buffer;
}>;

type ParseResult = Readonly<{ kind: "need_more" } | { kind: "invalid" } | { kind: "complete"; response: ParsedResponse }>;

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value).sort();
  const wanted = [...expected].sort();
  return keys.length === wanted.length && keys.every((key, index) => key === wanted[index]);
}

function parseJsonObject(bytes: Buffer): Record<string, unknown> | null {
  try {
    const value: unknown = JSON.parse(bytes.toString("utf8"));
    return typeof value === "object" && value !== null && !Array.isArray(value)
      ? value as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

function parseChunkedBody(bytes: Buffer, offset: number): ParseResult | { kind: "body"; body: Buffer } {
  const chunks: Buffer[] = [];
  let total = 0;
  let cursor = offset;
  while (true) {
    const lineEnd = bytes.indexOf("\r\n", cursor);
    if (lineEnd < 0) return { kind: bytes.length - offset > MAX_BODY_BYTES + 1024 ? "invalid" : "need_more" };
    const sizeLine = bytes.toString("ascii", cursor, lineEnd);
    if (!/^[0-9a-f]{1,8}(?:;[^\r\n]*)?$/iu.test(sizeLine)) return { kind: "invalid" };
    const sizeText = sizeLine.split(";", 1)[0]!;
    const size = Number.parseInt(sizeText, 16);
    if (!Number.isSafeInteger(size) || size < 0 || total + size > MAX_BODY_BYTES) return { kind: "invalid" };
    cursor = lineEnd + 2;
    if (size === 0) {
      if (bytes.length < cursor + 2) return { kind: "need_more" };
      if (bytes[cursor] !== 13 || bytes[cursor + 1] !== 10) return { kind: "invalid" };
      if (bytes.length !== cursor + 2) return { kind: "invalid" };
      return { kind: "body", body: Buffer.concat(chunks, total) };
    }
    if (bytes.length < cursor + size + 2) return { kind: "need_more" };
    if (bytes[cursor + size] !== 13 || bytes[cursor + size + 1] !== 10) return { kind: "invalid" };
    chunks.push(bytes.subarray(cursor, cursor + size));
    total += size;
    cursor += size + 2;
  }
}

function parseHttpResponse(bytes: Buffer, eof = false): ParseResult {
  if (bytes.length > MAX_HEADER_BYTES + MAX_BODY_BYTES + 2048) return { kind: "invalid" };
  const headerEnd = bytes.indexOf("\r\n\r\n");
  if (headerEnd < 0) return { kind: bytes.length > MAX_HEADER_BYTES ? "invalid" : "need_more" };
  if (headerEnd > MAX_HEADER_BYTES) return { kind: "invalid" };
  const lines = bytes.toString("latin1", 0, headerEnd).split("\r\n");
  const statusLine = lines.shift() ?? "";
  const statusMatch = /^HTTP\/1\.[01] ([1-5][0-9]{2})(?: [^\r\n]*)?$/u.exec(statusLine);
  if (statusMatch === null) return { kind: "invalid" };
  const headers = new Map<string, string[]>();
  for (const line of lines) {
    const separator = line.indexOf(":");
    if (separator <= 0 || /^[ \t]/u.test(line)) return { kind: "invalid" };
    const name = line.slice(0, separator);
    if (!/^[!#$%&'*+.^_`|~0-9a-z-]+$/iu.test(name)) return { kind: "invalid" };
    const value = line.slice(separator + 1).trim();
    const folded = name.toLowerCase();
    const values = headers.get(folded) ?? [];
    values.push(value);
    headers.set(folded, values);
  }
  const offset = headerEnd + 4;
  const lengthHeaders = headers.get("content-length") ?? [];
  const transferHeaders = headers.get("transfer-encoding") ?? [];
  if (lengthHeaders.length > 1 || transferHeaders.length > 1 || lengthHeaders.length > 0 && transferHeaders.length > 0) {
    return { kind: "invalid" };
  }
  let body: Buffer;
  if (transferHeaders.length === 1) {
    if (transferHeaders[0]?.toLowerCase() !== "chunked") return { kind: "invalid" };
    const parsed = parseChunkedBody(bytes, offset);
    if (parsed.kind !== "body") return parsed;
    body = parsed.body;
  } else if (lengthHeaders.length === 1) {
    const rawLength = lengthHeaders[0] ?? "";
    if (!/^(?:0|[1-9][0-9]*)$/u.test(rawLength)) return { kind: "invalid" };
    const length = Number(rawLength);
    if (!Number.isSafeInteger(length) || length > MAX_BODY_BYTES) return { kind: "invalid" };
    if (bytes.length < offset + length) return { kind: eof ? "invalid" : "need_more" };
    if (bytes.length !== offset + length) return { kind: "invalid" };
    body = bytes.subarray(offset);
  } else {
    if (!eof) return { kind: "need_more" };
    if (bytes.length - offset > MAX_BODY_BYTES) return { kind: "invalid" };
    body = bytes.subarray(offset);
  }
  return { kind: "complete", response: { status: Number(statusMatch[1]), headers, body } };
}

function classifyResponse(response: ParsedResponse, key: string): AttemptResult {
  const contentTypes = response.headers.get("content-type") ?? [];
  if (contentTypes.length !== 1 || contentTypes[0]?.split(";", 1)[0]?.trim().toLowerCase() !== "application/json") {
    return { kind: "outcome_unknown" };
  }
  const body = parseJsonObject(response.body);
  if (body === null) return { kind: "outcome_unknown" };
  if (response.status === 202) {
    const keys = response.headers.get("idempotency-key") ?? [];
    if (keys.length !== 1 || keys[0] !== key || !hasExactKeys(body, ["accepted_at"])) {
      return { kind: "outcome_unknown" };
    }
    const acceptedAt = body.accepted_at;
    if (typeof acceptedAt !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u.test(acceptedAt)) {
      return { kind: "outcome_unknown" };
    }
    const milliseconds = Date.parse(acceptedAt);
    if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== acceptedAt) {
      return { kind: "outcome_unknown" };
    }
    return { kind: "accepted", accepted_at: acceptedAt };
  }
  if (!hasExactKeys(body, ["code"]) || typeof body.code !== "string") return { kind: "outcome_unknown" };
  if (response.status === 400 && body.code === "invalid_recipient") return { kind: "recipient_rejected" };
  if (response.status === 401 && body.code === "authentication") return { kind: "misconfigured" };
  if (response.status === 409 && body.code === "idempotency_conflict") return { kind: "misconfigured" };
  if (response.status === 429 && body.code === "rate_limited") return { kind: "rate_limited" };
  return { kind: "outcome_unknown" };
}

function oneAttempt(
  input: Readonly<Record<string, unknown>>,
  credential: string,
  key: string,
  signal: AbortSignal | null,
  operationRemainingMs: number,
): Promise<AttemptResult> {
  return new Promise(resolve => {
    const endpoint = testEndpoint;
    const hostname = endpoint?.hostname ?? SERVICE_HOST;
    const port = endpoint === null ? 443 : Number(endpoint.port);
    const body = JSON.stringify(input);
    const requestBytes = Buffer.from(
      `POST ${SERVICE_PATH} HTTP/1.1\r\n` +
      `Host: ${SERVICE_HOST}\r\n` +
      `Authorization: Bearer ${credential}\r\n` +
      "Content-Type: application/json\r\n" +
      "Accept: application/json\r\n" +
      "Accept-Encoding: identity\r\n" +
      `Content-Length: ${Buffer.byteLength(body)}\r\n` +
      `Idempotency-Key: ${key}\r\n` +
      "Connection: close\r\n\r\n" + body,
      "utf8",
    );
    const headerEnd = requestBytes.indexOf("\r\n\r\n");
    if (headerEnd < 0 || headerEnd + 4 > MAX_HEADER_BYTES) {
      resolve({ kind: "misconfigured" });
      return;
    }
    let settled = false;
    let dispatched = false;
    let socket: Bun.Socket | undefined;
    let connectedSocket: Bun.Socket | undefined;
    let socketOpen = false;
    let requestStarted = false;
    let received = Buffer.alloc(0);
    let timer: ReturnType<typeof setTimeout> | undefined;
    let tlsPollTimer: ReturnType<typeof setTimeout> | undefined;
    const finish = (result: AttemptResult): void => {
      if (settled) return;
      settled = true;
      if (timer !== undefined) clearTimeout(timer);
      if (tlsPollTimer !== undefined) clearTimeout(tlsPollTimer);
      signal?.removeEventListener("abort", abort);
      socket?.terminate();
      resolve(result);
    };
    const abort = (): void => finish(dispatched
      ? { kind: "outcome_unknown" }
      : { kind: "pre_dispatch_failure" });
    if (signal?.aborted) {
      finish({ kind: "pre_dispatch_failure" });
      return;
    }
    signal?.addEventListener("abort", abort, { once: true });
    const configuredAttemptTimeoutMs = testEndpoint === null ? ATTEMPT_TIMEOUT_MS : testAttemptTimeoutMs ?? ATTEMPT_TIMEOUT_MS;
    const attemptTimeoutMs = Math.min(configuredAttemptTimeoutMs, Math.floor(operationRemainingMs));
    if (attemptTimeoutMs < 1) {
      finish({ kind: "pre_dispatch_failure" });
      return;
    }
    timer = setTimeout(() => finish(dispatched
      ? { kind: "outcome_unknown" }
      : { kind: "pre_dispatch_failure" }), attemptTimeoutMs);

    const dispatch = (connected: Bun.Socket): void => {
      if (settled || requestStarted) return;
      requestStarted = true;
      socket = connected;
      dispatched = true;
      let offset = 0;
      while (!settled && offset < requestBytes.length) {
        const written = connected.write(requestBytes, offset, requestBytes.length - offset);
        if (written <= 0) {
          finish({ kind: "outcome_unknown" });
          break;
        }
        offset += written;
      }
    };

    const connect = Bun.connect({
      hostname,
      port,
      ...(endpoint === null || endpoint.protocol === "https:"
        ? { tls: {
            serverName: SERVICE_HOST,
            ALPNProtocols: "http/1.1",
            rejectUnauthorized: true,
            ...(endpoint !== null && testCa !== null ? { ca: testCa } : {}),
          } }
        : {}),
      socket: {
        open(connected) {
          socket = connected;
          socketOpen = true;
          if (endpoint === null || endpoint.protocol === "https:") {
            const waitForVerifiedTls = (): void => {
              if (settled) return;
              let identityVerified = false;
              try {
                const certificate = connected.getPeerX509Certificate?.();
                identityVerified = connected.authorized && certificate?.checkHost(SERVICE_HOST, { subject: "default" }) !== undefined;
              } catch {
                identityVerified = false;
              }
              if (identityVerified) {
                dispatch(connected);
                return;
              }
              if (connected.errored !== null) {
                finish({ kind: "pre_dispatch_failure" });
                return;
              }
              tlsPollTimer = setTimeout(waitForVerifiedTls, 10);
            };
            waitForVerifiedTls();
          } else if (connectedSocket === connected) {
            dispatch(connected);
          }
        },
        data(connected, chunk) {
          if (settled) return;
          const bytes = Buffer.from(chunk);
          received = Buffer.concat([received, bytes]);
          if (received.length > MAX_HEADER_BYTES + MAX_BODY_BYTES + 2048) {
            finish({ kind: "outcome_unknown" });
            return;
          }
          const parsed = parseHttpResponse(received);
          if (parsed.kind === "invalid") finish({ kind: "outcome_unknown" });
          else if (parsed.kind === "complete") finish(classifyResponse(parsed.response, key));
          void connected;
        },
        close(_connected, error) {
          if (settled) return;
          const parsed = parseHttpResponse(received, true);
          if (parsed.kind === "complete") finish(classifyResponse(parsed.response, key));
          else finish(dispatched ? { kind: "outcome_unknown" } : { kind: "pre_dispatch_failure" });
          void error;
        },
        error(_connected, _error) {
          finish(dispatched ? { kind: "outcome_unknown" } : { kind: "pre_dispatch_failure" });
        },
        connectError(_connected, _error) {
          finish({ kind: "pre_dispatch_failure" });
        },
        timeout(_connected) {
          finish(dispatched ? { kind: "outcome_unknown" } : { kind: "pre_dispatch_failure" });
        },
      },
    });
    void connect.then(connected => {
      if (settled) {
        connected.terminate();
        return;
      }
      connectedSocket = connected;
      socket = connected;
      if (socketOpen && endpoint?.protocol === "http:") dispatch(connected);
    }).catch(() => {
      finish({ kind: "pre_dispatch_failure" });
    });
  });
}

function retryDelay(attempt: number, remainingMs: number, random: () => number = Math.random): number {
  const ceiling = Math.min(1_000, 100 * 2 ** (attempt - 1), remainingMs);
  return Math.floor(random() * Math.max(0, ceiling));
}

function waitForRetry(
  delayMs: number,
  signal: AbortSignal | null,
  testFake: ReferenceMailTestFake | null,
): Promise<boolean> {
  if (testFake !== null) {
    if (signal?.aborted) return Promise.resolve(false);
    testFake.elapsedMs += delayMs;
    return Promise.resolve(true);
  }
  return new Promise(resolve => {
    if (signal?.aborted) return resolve(false);
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", abort);
      resolve(true);
    }, delayMs);
    const abort = (): void => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      resolve(false);
    };
    signal?.addEventListener("abort", abort, { once: true });
  });
}

function fakeAttempt(
  testFake: ReferenceMailTestFake,
  attemptTimeoutMs: number,
): AttemptResult {
  const selected = testFake.outcomes[testFake.cursor.index];
  testFake.cursor.index += 1;
  if (selected === undefined) return { kind: "outcome_unknown" };
  if (selected.kind === "accepted") {
    const acceptedAt = selected.value.accepted_at;
    return typeof acceptedAt === "string"
      ? { kind: "accepted", accepted_at: acceptedAt }
      : { kind: "outcome_unknown" };
  }
  switch (selected.name) {
    case "provider.invalid_recipient": return { kind: "recipient_rejected" };
    case "provider.rate_limited": return { kind: "rate_limited" };
    case "provider.authentication":
    case "provider.idempotency_conflict": return { kind: "misconfigured" };
    case "transport.pre_dispatch_refusal": return { kind: "pre_dispatch_failure" };
    case "transport.pre_dispatch_timeout":
      testFake.elapsedMs += attemptTimeoutMs;
      return { kind: "pre_dispatch_failure" };
    case "transport.possible_dispatch_timeout":
      testFake.elapsedMs += attemptTimeoutMs;
      return { kind: "outcome_unknown" };
    case "transport.possible_dispatch_loss":
    case "transport.invalid_acknowledgement":
    case "transport.unexpected_response": return { kind: "outcome_unknown" };
    default: return { kind: "outcome_unknown" };
  }
}

export async function invokeReferenceMail(
  input: Readonly<Record<string, unknown>>,
  credential: string,
  context: ServiceOperationContext,
  testFake: ReferenceMailTestFake | null = null,
): Promise<ReferenceMailResult> {
  const key = input.idempotency_key;
  if (
    typeof credential !== "string" || credential.length === 0 || Buffer.byteLength(credential, "utf8") > MAX_CREDENTIAL_BYTES || /[\r\n]/u.test(credential) ||
    typeof key !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(key)
  ) {
    throw new ServiceAdapterFault("misconfigured");
  }
  const body = JSON.stringify(input);
  if (Buffer.byteLength(body) > MAX_BODY_BYTES) throw new ServiceAdapterFault("misconfigured");
  if (!Number.isFinite(context.monotonicStartedAt) || !Number.isInteger(context.serviceAttemptBudget.attemptsUsed) || context.serviceAttemptBudget.attemptsUsed < 0) {
    throw new ServiceAdapterFault("misconfigured");
  }
  const operationTimeoutMs = testFake !== null || testEndpoint === null
    ? MAX_ELAPSED_MS
    : testOperationTimeoutMs ?? MAX_ELAPSED_MS;
  const serviceDeadlineAt = context.monotonicStartedAt + operationTimeoutMs;
  const requestDeadlineAt = context.deadlineAt ?? Infinity;
  const deadlineAt = Math.min(serviceDeadlineAt, requestDeadlineAt);
  let fakeBaseline = 0;
  if (testFake !== null) {
    let baselines = fakeOperationBaselines.get(context.serviceAttemptBudget);
    if (baselines === undefined) { baselines = new WeakMap(); fakeOperationBaselines.set(context.serviceAttemptBudget, baselines); }
    const previous = baselines.get(testFake);
    fakeBaseline = previous ?? testFake.elapsedMs;
    if (previous === undefined) baselines.set(testFake, fakeBaseline);
  }
  while (context.serviceAttemptBudget.attemptsUsed < MAX_ATTEMPTS) {
    if (context.signal?.aborted) throw new ServiceAdapterFault("unavailable");
    const monotonicNow = testFake === null
      ? performance.now()
      : context.monotonicStartedAt + testFake.elapsedMs - fakeBaseline;
    const remainingBeforeAttemptMs = deadlineAt - monotonicNow;
    if (remainingBeforeAttemptMs < 1) throw new ServiceAdapterFault(requestDeadlineAt <= serviceDeadlineAt ? "deadline_exceeded" : "unavailable");
    const attempt = ++context.serviceAttemptBudget.attemptsUsed;
    const attemptTimeoutMs = Math.min(ATTEMPT_TIMEOUT_MS, Math.floor(remainingBeforeAttemptMs));
    const result = testFake === null
      ? await oneAttempt(input, credential, key, context.signal, remainingBeforeAttemptMs)
      : fakeAttempt(testFake, attemptTimeoutMs);
    const canRetry = result.kind === "pre_dispatch_failure" || result.kind === "rate_limited";
    const afterAttemptNow = testFake === null
      ? performance.now()
      : context.monotonicStartedAt + testFake.elapsedMs - fakeBaseline;
    const remainingMs = deadlineAt - afterAttemptNow;
    const retryScheduled = canRetry && context.serviceAttemptBudget.attemptsUsed < MAX_ATTEMPTS && remainingMs > 0 && context.signal?.aborted !== true;
    const delayMs = retryScheduled
      ? retryDelay(attempt, remainingMs, testFake === null ? Math.random : () => 0.5)
      : 0;
    console.error(JSON.stringify({
      schemaVersion: 1,
      kind: "operational_log_event",
      eventName: "service.attempt",
      operationId: context.operationId,
      attempt,
      elapsedMs: testFake === null
        ? Math.max(0, Math.round(performance.now() - context.monotonicStartedAt))
        : Math.round(testFake.elapsedMs - fakeBaseline),
      outcome: result.kind,
      retryScheduled,
      retryDelayMs: retryScheduled ? delayMs : null,
    }));
    if (result.kind === "accepted") return result;
    if (result.kind === "recipient_rejected") return result;
    if (result.kind === "misconfigured") throw new ServiceAdapterFault("misconfigured");
    if (result.kind === "outcome_unknown") throw new ServiceAdapterFault("outcome_unknown");
    if (!retryScheduled || !await waitForRetry(delayMs, context.signal, testFake)) {
      if (requestDeadlineAt <= serviceDeadlineAt && remainingMs < 1 && result.kind !== "accepted" && result.kind !== "recipient_rejected") {
        throw new ServiceAdapterFault("deadline_exceeded");
      }
      if (result.kind === "rate_limited") return { kind: "temporarily_unavailable" };
      throw new ServiceAdapterFault("unavailable");
    }
  }
  throw new ServiceAdapterFault("unavailable");
}
