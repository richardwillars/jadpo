// Compiler-owned first-party credential boundary. No authored login endpoints.
import { authenticateRequest, AuthenticationFault, type AuthPrincipal, type ResolutionResult } from "./authentication.ts";

type Settings = Readonly<{ name: string; mode: "signed" | "opaque"; cookie: string | null; audience: string; origin: string | null; secret: string; previousSecret?: string; maximumDelayMs: number }>;
type Session = Readonly<{ id: string; strategy: string; subject: string; userId: string; expires: number; verifier: string; keyId: string; revoked: boolean }>;
export type AuthenticationStorage = Readonly<{
  get(id: string): Promise<Session | null>;
  put(session: Session): Promise<void>;
  revoke(id: string): Promise<void>;
}>;
type Key = Readonly<{ id: string; key: CryptoKey }>;
type Configured = Settings & { keys: readonly Key[] };
type UserPrincipal = Extract<AuthPrincipal, { kind: "user" }>;
type Verified = { principal: UserPrincipal; sessionId: string; credential: string; key: Key; strategy: Configured };
const encoder = new TextEncoder();
const identifier = /^[A-Za-z0-9_-]{1,128}$/u;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const invalid = (): never => { throw new AuthenticationFault("invalid_credentials", 401); };
const misconfigured = (): never => { throw new AuthenticationFault("authentication_misconfigured", 503); };
function encode(bytes: ArrayBuffer | Uint8Array): string {
  return btoa(String.fromCharCode(...new Uint8Array(bytes))).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}
function decode(value: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]+$/u.test(value)) return invalid();
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value.replaceAll("-", "+").replaceAll("_", "/")), c => c.charCodeAt(0)); } catch { return invalid(); }
  if (encode(bytes) !== value) return invalid();
  return bytes;
}
async function keyFromSecret(secret: string): Promise<Key> {
  let bytes: Uint8Array;
  try { bytes = decode(secret); } catch { return misconfigured(); }
  if (bytes.length !== 32) return misconfigured();
  const id = encode(await crypto.subtle.digest("SHA-256", bytes)).slice(0, 22);
  const key = await crypto.subtle.importKey("raw", bytes, { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
  return { id, key };
}
async function sign(key: Key, message: string): Promise<string> {
  return encode(await crypto.subtle.sign("HMAC", key.key, encoder.encode(message)));
}
async function verifies(key: Key, message: string, signature: string): Promise<boolean> {
  const bytes = decode(signature);
  return bytes.length === 32 && await crypto.subtle.verify("HMAC", key.key, bytes, encoder.encode(message));
}
function sessionRecord(value: Session | null): Session | null {
  if (value === null) return null;
  if (typeof value !== "object" || Array.isArray(value)
      || Object.keys(value).sort().join(",") !== "expires,id,keyId,revoked,strategy,subject,userId,verifier"
      || typeof value.id !== "string" || !identifier.test(value.id) || typeof value.strategy !== "string" || !identifier.test(value.strategy)
      || typeof value.subject !== "string" || value.subject.length === 0 || value.subject.length > 512
      || !uuid.test(value.userId) || !Number.isSafeInteger(value.expires)
      || typeof value.revoked !== "boolean" || typeof value.verifier !== "string" || typeof value.keyId !== "string") {
    throw new AuthenticationFault("authority_invariant", 500);
  }
  return value;
}
function identity(subject: string, userId: string, strength: string): UserPrincipal {
  return Object.freeze({ kind: "user", subject, authenticationStrength: strength, values: Object.freeze({ user_id: userId }) }) as UserPrincipal;
}
async function storageCall<T>(operation: () => Promise<T>): Promise<T> {
  try { return await operation(); } catch { throw new AuthenticationFault("authentication_unavailable", 503); }
}
function milliseconds(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0) throw new AuthenticationFault("authority_invariant", 500);
  return value;
}

export async function createFirstPartyAuthentication(
  settings: readonly Settings[], storage: AuthenticationStorage,
  resolve: (strategy: string, subject: string, strength: string) => Promise<ResolutionResult>,
) {
  const configured: Configured[] = [];
  for (const item of settings) {
    if (!identifier.test(item.name) || typeof item.audience !== "string" || item.audience.length === 0 || item.audience.length > 256
        || !Number.isSafeInteger(item.maximumDelayMs) || item.maximumDelayMs < 0
        || (item.mode === "signed" && item.maximumDelayMs === 0)) return misconfigured();
    if (item.cookie !== null) {
      if (!/^[A-Za-z0-9_-]{1,128}$/u.test(item.cookie) || item.origin === null) return misconfigured();
      try { const origin = new URL(item.origin); if (origin.protocol !== "https:" || origin.origin !== item.origin) return misconfigured(); } catch { return misconfigured(); }
    }
    const keys = [await keyFromSecret(item.secret)];
    if (item.previousSecret !== undefined) {
      const previous = await keyFromSecret(item.previousSecret);
      if (previous.id === keys[0].id) return misconfigured();
      keys.push(previous);
    }
    configured.push({ ...item, secret: "", previousSecret: undefined, keys });
  }
  const config = (name: string): Configured => configured.find(item => item.name === name) ?? misconfigured();
  async function authority(strategy: Configured, subject: string, userId?: string): Promise<UserPrincipal> {
    let result: ResolutionResult;
    try { result = await resolve(strategy.name, subject, strategy.mode); } catch { throw new AuthenticationFault("authentication_unavailable", 503); }
    if (result.kind === "inactive") throw new AuthenticationFault("principal_inactive", 401, result.failureName);
    if (result.kind === "missing") return invalid();
    if (result.kind === "unavailable") throw new AuthenticationFault("authentication_unavailable", 503);
    if (result.kind !== "active" || result.principal.kind !== "user" || result.principal.subject !== subject
        || (userId !== undefined && result.principal.values.user_id !== userId)) throw new AuthenticationFault("authority_invariant", 500);
    return result.principal;
  }
  async function liveSession(verified: Verified, now: number): Promise<Session> {
    const session = sessionRecord(await storageCall(() => storage.get(verified.sessionId)));
    if (session === null || session.id !== verified.sessionId || session.revoked || session.expires <= now
        || session.strategy !== verified.strategy.name || session.subject !== verified.principal.subject
        || session.userId !== verified.principal.values.user_id) return invalid();
    return session;
  }
  async function verify(strategy: Configured, credential: string, now: number): Promise<Verified> {
    if (credential.length > 4096) return invalid();
    if (strategy.mode === "opaque") {
      const parts = credential.split(".");
      if (parts.length !== 3 || parts[0] !== "jdo1" || !identifier.test(parts[1]) || decode(parts[2]).length !== 32) return invalid();
      const session = sessionRecord(await storageCall(() => storage.get(parts[1])));
      if (session === null || session.id !== parts[1] || session.revoked || session.expires <= now || session.strategy !== strategy.name) return invalid();
      const key = strategy.keys.find(key => key.id === session.keyId);
      if (!key || !(await verifies(key, `opaque:${strategy.audience}:${credential}`, session.verifier))) return invalid();
      return { principal: identity(session.subject, session.userId, "opaque"), sessionId: session.id, credential, key, strategy };
    }
    const parts = credential.split(".");
    if (parts.length !== 4 || parts[0] !== "jds1") return invalid();
    const key = strategy.keys.find(key => key.id === parts[1]);
    if (!key || !(await verifies(key, parts.slice(0, 3).join("."), parts[3]))) return invalid();
    let payload: unknown;
    try { payload = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(decode(parts[2]))); } catch { return invalid(); }
    // Fixed tuple: version, audience, strategy, session, subject, identity, issued, expires.
    if (!Array.isArray(payload) || payload.length !== 8 || payload[0] !== 1
        || payload[1] !== strategy.audience || payload[2] !== strategy.name || typeof payload[3] !== "string" || !identifier.test(payload[3])
        || typeof payload[4] !== "string" || payload[4].length === 0 || payload[4].length > 512
        || typeof payload[5] !== "string" || !uuid.test(payload[5])
        || !Number.isSafeInteger(payload[6]) || !Number.isSafeInteger(payload[7])
        || payload[6] < 0 || payload[6] > now || payload[7] <= now || payload[7] <= payload[6]
        || payload[7] - payload[6] > strategy.maximumDelayMs) return invalid();
    return { principal: identity(payload[4], payload[5], "signed"), sessionId: payload[3], credential, key, strategy };
  }
  async function credentialFor(strategy: Configured, session: Session, now: number, opaque?: string) {
    const key = strategy.keys[0];
    const expires = strategy.mode === "signed" ? Math.min(session.expires, now + strategy.maximumDelayMs) : session.expires;
    let credential = opaque;
    if (strategy.mode === "signed") {
      const payload = encode(encoder.encode(JSON.stringify([1, strategy.audience, strategy.name, session.id, session.subject, session.userId, now, expires])));
      const message = `jds1.${key.id}.${payload}`;
      credential = `${message}.${await sign(key, message)}`;
    }
    if (!credential) return invalid();
    const csrfToken = await sign(key, `csrf:${strategy.name}:${credential}`);
    const setCookie = strategy.cookie === null ? null : `${strategy.cookie}=${credential}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=${Math.max(0, Math.floor((expires - now) / 1000))}`;
    return { credential, csrfToken, setCookie, expires };
  }
  return Object.freeze({
    // Internal host integration only: caller must already have authenticated the
    // human. No Jadpo action, HTTP route, password or account provisioning API.
    async issue(strategyName: string, subject: string, expires: number, operationNow: number) {
      const now = milliseconds(operationNow); milliseconds(expires);
      if (expires <= now || subject.length === 0 || subject.length > 512) return invalid();
      const strategy = config(strategyName);
      const principal = await authority(strategy, subject);
      const userId = principal.values.user_id;
      if (typeof userId !== "string" || !uuid.test(userId)) throw new AuthenticationFault("authority_invariant", 500);
      const id = crypto.randomUUID();
      const opaque = strategy.mode === "opaque" ? `jdo1.${id}.${encode(crypto.getRandomValues(new Uint8Array(32)))}` : undefined;
      const session: Session = { id, strategy: strategy.name, subject, userId, expires, revoked: false, keyId: strategy.keys[0].id,
        verifier: opaque ? await sign(strategy.keys[0], `opaque:${strategy.audience}:${opaque}`) : "" };
      await storageCall(() => storage.put(session));
      return credentialFor(strategy, session, now, opaque);
    },
    async authenticate(request: Request, freshAuthority: boolean, operationNow: number, mutates = false): Promise<AuthPrincipal> {
      const now = milliseconds(operationNow);
      let verified: Verified | null = null;
      const principal = await authenticateRequest(request, { freshAuthority }, {
        async validate(name, credential) {
          try { verified = await verify(config(name), credential, now); return { kind: "valid", identity: { principal: verified.principal, authorityRequired: verified.strategy.mode === "opaque" } }; }
          catch (error) { if (error instanceof AuthenticationFault && error.code === "invalid_credentials") return { kind: "invalid" }; throw error; }
        },
        async resolve() {
          if (!verified) throw new AuthenticationFault("authority_invariant", 500);
          try {
            await liveSession(verified, now);
            const principal = await authority(verified.strategy, verified.principal.subject, verified.principal.values.user_id as string);
            return { kind: "active", principal };
          } catch (error) {
            if (error instanceof AuthenticationFault && error.code === "invalid_credentials") return { kind: "missing" };
            if (error instanceof AuthenticationFault && error.code === "principal_inactive") return { kind: "inactive", failureName: error.declaredFailure! };
            throw error;
          }
        },
      });
      if (verified === null) throw new AuthenticationFault("authority_invariant", 500);
      const selected = verified as Verified;
      if (selected.strategy.cookie !== null && (mutates || !["GET", "HEAD", "OPTIONS"].includes(request.method))) {
        const token = request.headers.get("x-jadpo-csrf");
        if (request.headers.get("origin") !== selected.strategy.origin || request.headers.get("sec-fetch-site") === "cross-site"
            || token === null || token.length > 128) throw new AuthenticationFault("csrf_rejected", 403);
        let valid = false;
        try { valid = await verifies(selected.key, `csrf:${selected.strategy.name}:${selected.credential}`, token); } catch { /* invalid encoding */ }
        if (!valid) throw new AuthenticationFault("csrf_rejected", 403);
      }
      return principal;
    },
    async refresh(strategyName: string, credential: string, operationNow: number) {
      const now = milliseconds(operationNow);
      const strategy = config(strategyName);
      const verified = await verify(strategy, credential, now);
      const session = await liveSession(verified, now);
      await authority(strategy, session.subject, session.userId);
      // Opaque credentials retain their original verifier/key until reissued.
      if (strategy.mode === "opaque") return invalid();
      return credentialFor(strategy, session, now);
    },
    async revoke(strategyName: string, credential: string, operationNow: number) {
      const verified = await verify(config(strategyName), credential, milliseconds(operationNow));
      await storageCall(() => storage.revoke(verified.sessionId));
    },
  });
}
