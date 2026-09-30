// Compiler-owned JWT capability. Only emitted for an explicitly declared JWT
// validator; jose must be installed explicitly at the compiler-pinned version.
import { createLocalJWKSet, jwtVerify, type JSONWebKeySet } from "jose";

export type JwtSettings = Readonly<{ name: string; issuer: string; audience: string; jwksUri?: string }>;
export type JwtVerification = Readonly<{ kind: "valid"; subject: string; authenticationStrength: "jwt" }>
  | Readonly<{ kind: "invalid" | "unavailable" | "misconfigured" }>;
type Dependencies = Readonly<{ fetch?: typeof fetch; cacheClock?: () => number }>;
const LIMITS = Object.freeze({ tokenBytes: 8192, headerBytes: 2048, responseBytes: 131072, keys: 32,
  fetchMs: 2000, cacheMs: 300000, cooldownMs: 30000, clockSkewSeconds: 30 });
const algorithms = ["RS256", "ES256"];
const text = new TextDecoder("utf-8", { fatal: true });
const invalid = Object.freeze({ kind: "invalid" } as const);
const unavailable = Object.freeze({ kind: "unavailable" } as const);
const misconfigured = Object.freeze({ kind: "misconfigured" } as const);
function object(value: unknown): value is Record<string, any> { return value !== null && typeof value === "object" && !Array.isArray(value); }
function https(value: string): URL {
  if (typeof value !== "string" || value.length === 0 || value.length > 2048) throw new Error("URL size");
  const url = new URL(value);
  if (url.protocol !== "https:" || url.username || url.password || url.hash || url.search) throw new Error("invalid URL");
  return url;
}
function bytes(value: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]+$/u.test(value)) throw new Error("invalid encoding");
  const decoded = Uint8Array.from(atob(value.replaceAll("-", "+").replaceAll("_", "/")), c => c.charCodeAt(0));
  const canonical = btoa(String.fromCharCode(...decoded)).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
  if (canonical !== value) throw new Error("noncanonical encoding");
  return decoded;
}
// Bounded JSON grammar rejects duplicate keys (including escaped spellings),
// invalid UTF-8 and excessive nesting before JOSE sees the same signed bytes.
function json(source: string): any {
  let at = 0;
  const whitespace = () => { while (/\s/u.test(source[at] ?? "") && at < source.length) at++; };
  function string(): string {
    const start = at++;
    while (at < source.length) {
      const c = source[at++];
      if (c === "\\") at++;
      else if (c === '"') return JSON.parse(source.slice(start, at));
    }
    throw new Error("unterminated JSON string");
  }
  function value(depth: number): any {
    if (depth > 16) throw new Error("JSON depth");
    whitespace();
    if (source[at] === '"') return string();
    if (source[at] === "{") {
      at++; whitespace(); const result = Object.create(null); const seen = new Set<string>();
      if (source[at] === "}") { at++; return result; }
      while (at < source.length) {
        whitespace(); if (source[at] !== '"') throw new Error("JSON key");
        const key = string(); if (seen.has(key)) throw new Error("duplicate JSON key"); seen.add(key);
        whitespace(); if (source[at++] !== ":") throw new Error("JSON colon");
        result[key] = value(depth + 1); whitespace();
        const end = source[at++]; if (end === "}") return result; if (end !== ",") throw new Error("JSON object");
      }
    } else if (source[at] === "[") {
      at++; whitespace(); const result: any[] = [];
      if (source[at] === "]") { at++; return result; }
      while (at < source.length) {
        result.push(value(depth + 1)); whitespace(); const end = source[at++];
        if (end === "]") return result; if (end !== ",") throw new Error("JSON array");
      }
    } else {
      const match = /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/u.exec(source.slice(at));
      if (match) { at += match[0].length; return JSON.parse(match[0]); }
    }
    throw new Error("invalid JSON");
  }
  // JSON.parse independently enforces JSON whitespace, escapes and token syntax.
  JSON.parse(source);
  const result = value(0); whitespace(); if (at !== source.length) throw new Error("JSON trailing data");
  return result;
}
function envelope(credential: string): { kid: string; alg: string } {
  if (typeof credential !== "string" || credential.length > LIMITS.tokenBytes) throw new Error("token size");
  const parts = credential.split(".");
  if (parts.length !== 3 || parts.some(part => !part)) throw new Error("compact JWT required");
  const headerBytes = bytes(parts[0]); bytes(parts[2]);
  if (headerBytes.length > LIMITS.headerBytes) throw new Error("header size");
  const header = json(text.decode(headerBytes));
  if (!object(header) || Object.keys(header).some(key => !["alg", "kid", "typ"].includes(key))
      || !algorithms.includes(header.alg) || typeof header.kid !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/u.test(header.kid)
      || (header.typ !== undefined && !["JWT", "at+jwt"].includes(header.typ))) throw new Error("header policy");
  if (!object(json(text.decode(bytes(parts[1]))))) throw new Error("claims object");
  return { kid: header.kid, alg: header.alg };
}
function keySet(value: unknown): JSONWebKeySet {
  if (!object(value) || !Array.isArray(value.keys) || value.keys.length < 1 || value.keys.length > LIMITS.keys) throw new Error("JWKS shape");
  const keys: any[] = []; const seen = new Set<string>();
  for (const key of value.keys) {
    if (!object(key)) throw new Error("JWK shape");
    if (key.kty !== "RSA" && key.kty !== "EC") continue;
    if (key.alg !== undefined && !algorithms.includes(key.alg)) continue;
    if (key.use !== undefined && key.use !== "sig") continue;
    if (key.key_ops !== undefined && (!Array.isArray(key.key_ops) || key.key_ops.length !== 1 || key.key_ops[0] !== "verify")) throw new Error("JWK operations");
    if (typeof key.kid !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/u.test(key.kid) || seen.has(key.kid)
        || ["d", "p", "q", "dp", "dq", "qi", "oth", "k"].some(name => name in key)) throw new Error("JWK identity");
    if (key.kty === "RSA") {
      const n = bytes(key.n); const e = bytes(key.e);
      if (n.length < 256 || n.length > 512 || n[0] < 128 || e.length < 1 || e.length > 4
          || (key.alg !== undefined && key.alg !== "RS256")) throw new Error("RSA bounds");
    } else if (key.crv !== "P-256" || bytes(key.x).length !== 32 || bytes(key.y).length !== 32
        || (key.alg !== undefined && key.alg !== "ES256")) throw new Error("EC bounds");
    seen.add(key.kid); keys.push(key);
  }
  if (!keys.length) throw new Error("no supported key");
  return { keys };
}
export function createJwtAuthentication(settings: readonly JwtSettings[], dependencies: Dependencies = {}) {
  const request = dependencies.fetch ?? globalThis.fetch;
  const cacheClock = dependencies.cacheClock ?? (() => performance.now());
  type Cache = { resolver: ReturnType<typeof createLocalJWKSet>; kids: Set<string>; fetched: number };
  type State = { config: JwtSettings; cache?: Cache; pending?: Promise<Cache>; attempted: number };
  const states = new Map<string, State>(); let configurationValid = true;
  try {
    for (const item of settings) {
      if (!/^[A-Za-z0-9_-]{1,128}$/u.test(item.name) || states.has(item.name)
          || typeof item.audience !== "string" || !item.audience.length || item.audience.length > 256) throw new Error("settings");
      https(item.issuer); if (item.jwksUri !== undefined) https(item.jwksUri);
      states.set(item.name, { config: Object.freeze({ ...item }), attempted: -Infinity });
    }
  } catch { configurationValid = false; }
  async function document(url: URL): Promise<any> {
    const controller = new AbortController(); let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([
        (async () => {
          const response = await request(url, { redirect: "error", credentials: "omit", signal: controller.signal,
            headers: { accept: "application/json, application/jwk-set+json" } });
          if (response.status !== 200 || !response.body) throw new Error("provider response");
          const length = response.headers.get("content-length");
          if (length !== null && (!/^[0-9]+$/u.test(length) || Number(length) > LIMITS.responseBytes)) throw new Error("provider size");
          const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let size = 0;
          try { while (true) { const next = await reader.read(); if (next.done) break;
            size += next.value.byteLength; if (size > LIMITS.responseBytes) throw new Error("provider size"); chunks.push(next.value);
          } } finally { await reader.cancel().catch(() => {}); }
          const buffer = new Uint8Array(size); let offset = 0; for (const chunk of chunks) { buffer.set(chunk, offset); offset += chunk.length; }
          return json(text.decode(buffer));
        })(),
        new Promise<never>((_, reject) => { timeout = setTimeout(() => { controller.abort(); reject(new Error("provider timeout")); }, LIMITS.fetchMs); }),
      ]);
    } finally { clearTimeout(timeout); controller.abort(); }
  }
  async function refresh(state: State): Promise<Cache> {
    if (state.pending) return state.pending;
    state.attempted = cacheClock();
    state.pending = (async () => {
      let uri = state.config.jwksUri;
      if (uri === undefined) {
        const discovery = https(state.config.issuer.replace(/\/$/u, "") + "/.well-known/openid-configuration");
        const metadata = await document(discovery);
        if (!object(metadata) || metadata.issuer !== state.config.issuer || typeof metadata.jwks_uri !== "string") throw new Error("discovery mismatch");
        uri = metadata.jwks_uri;
      }
      const jwks = keySet(await document(https(uri)));
      const cache = { resolver: createLocalJWKSet(jwks), kids: new Set(jwks.keys.map(key => key.kid!)), fetched: cacheClock() };
      state.cache = cache; return cache;
    })();
    try { return await state.pending; } finally { state.pending = undefined; }
  }
  return Object.freeze({
    configured: configurationValid,
    async verify(name: string, credential: string, operationNowMs: number): Promise<JwtVerification> {
      if (!configurationValid || !states.has(name) || !Number.isSafeInteger(operationNowMs) || operationNowMs < 0
          || !Number.isFinite(new Date(operationNowMs).getTime())) return misconfigured;
      let header: ReturnType<typeof envelope>;
      try { header = envelope(credential); } catch { return invalid; }
      const state = states.get(name)!; let cache = state.cache;
      const cacheNow = cacheClock();
      if (!cache || cacheNow - cache.fetched >= LIMITS.cacheMs) {
        if (!state.pending && cacheNow - state.attempted < LIMITS.cooldownMs) return unavailable;
        try { cache = await refresh(state); } catch { return unavailable; }
      } else if (!cache.kids.has(header.kid) && cacheNow - state.attempted >= LIMITS.cooldownMs) {
        try { cache = await refresh(state); } catch { return unavailable; }
      }
      if (!cache.kids.has(header.kid)) return invalid;
      try {
        const { payload } = await jwtVerify(credential, cache.resolver, {
          algorithms, issuer: state.config.issuer, audience: state.config.audience,
          requiredClaims: ["iss", "aud", "sub", "iat", "exp"],
          currentDate: new Date(operationNowMs), clockTolerance: LIMITS.clockSkewSeconds,
        });
        const now = operationNowMs / 1000;
        if (!(typeof payload.aud === "string" || (Array.isArray(payload.aud) && payload.aud.length > 0 && payload.aud.length <= 16
              && payload.aud.every(value => typeof value === "string" && value.length > 0 && value.length <= 256)))) return invalid;
        if (typeof payload.sub !== "string" || !payload.sub.length || payload.sub.length > 512
            || /[\u0000-\u001f\u007f]/u.test(payload.sub)
            || !Number.isSafeInteger(payload.iat) || !Number.isSafeInteger(payload.exp)
            || payload.iat! < 0 || payload.exp! <= payload.iat! || payload.iat! > now + LIMITS.clockSkewSeconds
            || (payload.nbf !== undefined && (!Number.isSafeInteger(payload.nbf) || payload.nbf < 0 || payload.nbf >= payload.exp!))) return invalid;
        return Object.freeze({ kind: "valid", subject: payload.sub, authenticationStrength: "jwt" });
      } catch { return invalid; }
    },
  });
}
