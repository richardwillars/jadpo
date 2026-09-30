import { afterAll, describe, expect, test } from "bun:test";
import { copyFileSync, existsSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

const dependency = resolve(process.env.JADPO_JWT_DEPENDENCY_DIR ?? "build/validation/jwt-dependencies");
if (!existsSync(join(dependency, "node_modules/jose/package.json"))) throw new Error("Install the pinned JWT test dependency explicitly: python3 tools/install-jwt-dependency.py");
const root = mkdtempSync(join(tmpdir(), "jadpo-jwt-runtime-"));
symlinkSync(join(dependency, "node_modules"), join(root, "node_modules"));
copyFileSync("jadpo/crates/core/src/runtime/jwt_authentication.ts", join(root, "jwt_authentication.ts"));
const { createJwtAuthentication } = await import(pathToFileURL(join(root, "jwt_authentication.ts")).href);
const { SignJWT, CompactSign, generateKeyPair, exportJWK } = await import(pathToFileURL(join(dependency, "node_modules/jose/dist/webapi/index.js")).href);
afterAll(() => rmSync(root, { recursive: true, force: true }));
const issuer = "https://issuer.example.test/tenant";
const audience = "jadpo-api";
const now = Date.parse("2026-01-15T12:00:00Z");
const seconds = now / 1000;
const rsa = await generateKeyPair("RS256", { modulusLength: 2048, extractable: true });
const ec = await generateKeyPair("ES256", { extractable: true });
const rsaJwk = { ...await exportJWK(rsa.publicKey), kid: "rsa-current", alg: "RS256", use: "sig" };
const ecJwk = { ...await exportJWK(ec.publicKey), kid: "ec-current", alg: "ES256", use: "sig" };
const claims = () => ({ iss: issuer, aud: audience, sub: "alice", iat: seconds - 1, exp: seconds + 300 });
async function token(payload: Record<string, unknown> = claims(), header: Record<string, unknown> = { alg: "RS256", kid: "rsa-current", typ: "JWT" }, key = rsa.privateKey) {
  return new SignJWT(payload).setProtectedHeader(header).sign(key);
}
function provider(options: { direct?: boolean; keys?: unknown; metadata?: unknown; respond?: (url: string) => Response | Promise<Response> } = {}) {
  const urls: string[] = []; let clock = 0;
  let keys = options.keys ?? { keys: [rsaJwk, ecJwk] };
  const fetcher = async (url: URL, init: RequestInit) => {
    urls.push(url.href); expect(init.redirect).toBe("error"); expect(init.credentials).toBe("omit");
    if (options.respond) return options.respond(url.href);
    return Response.json(url.pathname.endsWith("openid-configuration")
      ? options.metadata ?? { issuer, jwks_uri: "https://keys.example.test/jwks" } : keys);
  };
  const settings = { name: "external", issuer, audience, ...(options.direct ? { jwksUri: "https://keys.example.test/jwks" } : {}) };
  const adapter = createJwtAuthentication([settings], { fetch: fetcher, cacheClock: () => clock });
  return { adapter, urls, advance(ms: number) { clock += ms; }, setKeys(value: unknown) { keys = value; } };
}
function encode(value: string | Uint8Array) { return Buffer.from(value).toString("base64url"); }
function raw(header: string, payload: string, signature = "AA") { return `${encode(header)}.${encode(payload)}.${signature}`; }

describe("compiler-owned JWT verification with pinned jose", () => {
  test("real RS256 and ES256 signatures resolve only a frozen subject", async () => {
    const p = provider();
    for (const credential of [await token({ ...claims(), email: "private@example.test", roles: ["admin"], user_id: "forged" }), await token(claims(), { alg: "ES256", kid: "ec-current" }, ec.privateKey)]) {
      const result = await p.adapter.verify("external", credential, now);
      expect(result).toEqual({ kind: "valid", subject: "alice", authenticationStrength: "jwt" });
      expect(Object.isFrozen(result)).toBe(true);
    }
    expect(p.urls).toEqual([issuer + "/.well-known/openid-configuration", "https://keys.example.test/jwks"]);
  });
  test("explicit JWKS skips discovery and concurrent requests share one bounded fetch", async () => {
    const p = provider({ direct: true }); const credential = await token();
    const results = await Promise.all(Array.from({ length: 24 }, () => p.adapter.verify("external", credential, now)));
    expect(results.every(result => result.kind === "valid")).toBe(true); expect(p.urls).toHaveLength(1);
  });
  test("invalid issuer configuration never reaches network", async () => {
    for (const bad of ["http://issuer.test", "https://u:p@issuer.test", "https://issuer.test/#fragment", "https://issuer.test/?query"]) {
      let fetches = 0; const p = createJwtAuthentication([{ name: "external", issuer: bad, audience }], { fetch: async () => { fetches++; throw Error(); } });
      expect(p.configured).toBe(false);
      expect(await p.verify("external", await token(), now)).toEqual({ kind: "misconfigured" }); expect(fetches).toBe(0);
    }
  });
  test("invalid operation clocks fail before fetching", async () => {
    const p = provider(); expect(p.adapter.configured).toBe(true);
    for (const invalidClock of [NaN, Infinity, -1, Number.MAX_SAFE_INTEGER]) {
      expect(await p.adapter.verify("external", await token(), invalidClock)).toEqual({ kind: "misconfigured" });
    }
    expect(p.urls).toHaveLength(0);
  });
  test("missing, wrong-type, wrong-issuer and wrong-audience claims reject", async () => {
    const p = provider();
    for (const field of ["iss", "aud", "sub", "iat", "exp"]) { const value = claims(); delete value[field]; expect(await p.adapter.verify("external", await token(value), now)).toEqual({ kind: "invalid" }); }
    for (const changed of [{ iss: "https://attacker.test" }, { aud: "other" }, { sub: "" }, { sub: "a".repeat(513) }, { sub: "a\u0000b" }, { iat: "bad" }, { exp: 1.5 }, { aud: [audience, 7] }]) {
      expect(await p.adapter.verify("external", await token({ ...claims(), ...changed }), now)).toEqual({ kind: "invalid" });
    }
  });
  test("expiry, not-before, future issuance and inconsistent times reject with fixed skew", async () => {
    const p = provider();
    for (const changed of [{ exp: seconds - 30 }, { nbf: seconds + 31 }, { iat: seconds + 31 }, { iat: -1 }, { iat: seconds + 10, exp: seconds + 5 }, { nbf: seconds + 300 }]) {
      expect(await p.adapter.verify("external", await token({ ...claims(), ...changed }), now)).toEqual({ kind: "invalid" });
    }
    expect((await p.adapter.verify("external", await token({ ...claims(), iat: seconds + 30, exp: seconds + 100 }), now)).kind).toBe("valid");
  });
  test("tampering and different signing key reject", async () => {
    const p = provider(); const credential = await token(); const parts = credential.split(".");
    parts[1] = encode(JSON.stringify({ ...claims(), sub: "bob" }));
    expect(await p.adapter.verify("external", parts.join("."), now)).toEqual({ kind: "invalid" });
    const other = await generateKeyPair("RS256");
    expect(await p.adapter.verify("external", await token(claims(), { alg: "RS256", kid: "rsa-current" }, other.privateKey), now)).toEqual({ kind: "invalid" });
  });
  test("none, HMAC confusion, arbitrary URLs, embedded keys and critical headers reject before fetch", async () => {
    const p = provider();
    for (const header of [{ alg: "none", kid: "rsa-current" }, { alg: "HS256", kid: "rsa-current" }, ...["jku", "x5u", "jwk", "x5c", "crit", "b64"].map(key => ({ alg: "RS256", kid: "rsa-current", [key]: "https://attacker.test" })), { alg: "RS256" }, { alg: "RS256", kid: "../../key" }, { alg: "RS256", kid: "rsa-current", typ: "JWE" }]) {
      expect(await p.adapter.verify("external", raw(JSON.stringify(header), JSON.stringify(claims())), now)).toEqual({ kind: "invalid" });
    }
    expect(p.urls).toHaveLength(0);
  });
  test("duplicate escaped header/claim names, invalid UTF-8 and nested JSON reject before fetch", async () => {
    const p = provider();
    const header = JSON.stringify({ alg: "RS256", kid: "rsa-current" });
    const malformed = [raw('{"alg":"RS256","\\u0061lg":"none","kid":"rsa-current"}', JSON.stringify(claims())), raw(header, '{"sub":"alice","sub":"bob"}'), `${encode(header)}.${encode(Uint8Array.of(0xff))}.AA`, raw(header, '{"sub":' + '['.repeat(20) + '1' + ']'.repeat(20) + '}')];
    for (const credential of malformed) expect(await p.adapter.verify("external", credential, now)).toEqual({ kind: "invalid" });
    expect(p.urls).toHaveLength(0);
  });
  test("malformed, oversized, padded and noncanonical compact tokens reject", async () => {
    const p = provider(); const valid = await token();
    for (const credential of ["", "a.b", "a.b.c.d.e", "a".repeat(8193), valid + "=", valid.replace(/\./u, "=."), "Bearer " + valid, valid + "," + valid, "a.b.!"]) {
      expect(await p.adapter.verify("external", credential, now)).toEqual({ kind: "invalid" });
    }
    expect(p.urls).toHaveLength(0);
  });
  test("unknown key refreshes only after cooldown and rotation replaces cache", async () => {
    const p = provider(); const current = await token(); expect((await p.adapter.verify("external", current, now)).kind).toBe("valid");
    const next = await token(claims(), { alg: "RS256", kid: "rsa-next" });
    p.setKeys({ keys: [{ ...rsaJwk, kid: "rsa-next" }] });
    expect(await p.adapter.verify("external", next, now)).toEqual({ kind: "invalid" }); expect(p.urls).toHaveLength(2);
    p.advance(30000); expect((await p.adapter.verify("external", next, now)).kind).toBe("valid"); expect(p.urls).toHaveLength(4);
    expect(await p.adapter.verify("external", current, now)).toEqual({ kind: "invalid" }); expect(p.urls).toHaveLength(4);
  });
  test("expired key cache fails closed during outage and outage requests are coalesced", async () => {
    let outage = false;
    const p = provider({ direct: true, respond: () => { if (outage) throw Error("provider secret details"); return Response.json({ keys: [rsaJwk] }); } });
    const credential = await token(); expect((await p.adapter.verify("external", credential, now)).kind).toBe("valid");
    outage = true; p.advance(300000);
    const results = await Promise.all(Array.from({ length: 10 }, () => p.adapter.verify("external", credential, now)));
    expect(results).toEqual(Array(10).fill({ kind: "unavailable" })); expect(p.urls).toHaveLength(2);
    expect(await p.adapter.verify("external", credential, now)).toEqual({ kind: "unavailable" }); expect(p.urls).toHaveLength(2);
  });
  test("discovery issuer mismatch and unsafe JWKS addresses are operational failures", async () => {
    for (const metadata of [{ issuer: "https://other.test", jwks_uri: "https://keys.test" }, { issuer, jwks_uri: "http://keys.test" }, { issuer, jwks_uri: "https://user:pass@keys.test" }]) {
      const p = provider({ metadata }); expect(await p.adapter.verify("external", await token(), now)).toEqual({ kind: "unavailable" }); expect(p.urls).toHaveLength(1);
    }
  });
  test("provider response size, status, malformed JSON and duplicate keys are bounded", async () => {
    for (const response of [() => new Response("x", { status: 302 }), () => new Response("{}", { headers: { "content-length": "131073" } }), () => new Response("x".repeat(131073)), () => new Response("not json"), () => new Response('{"keys":[],"keys":[]}')]) {
      const p = provider({ direct: true, respond: response }); expect(await p.adapter.verify("external", await token(), now)).toEqual({ kind: "unavailable" });
    }
  });
  test("JWKS rejects ambiguous kids, private keys, oversized keys and excessive key counts", async () => {
    for (const keys of [{ keys: [rsaJwk, rsaJwk] }, { keys: [{ ...rsaJwk, d: "private" }] }, { keys: Array(33).fill(rsaJwk) }, { keys: [{ ...rsaJwk, n: encode(new Uint8Array(1024).fill(255)) }] }, { keys: [{ ...ecJwk, crv: "P-521" }] }]) {
      const p = provider({ direct: true, keys }); expect(await p.adapter.verify("external", await token(), now)).toEqual({ kind: "unavailable" });
    }
  });
  test("hung fetch is aborted and produces a bounded safe failure", async () => {
    let signal: AbortSignal | undefined;
    const p = createJwtAuthentication([{ name: "external", issuer, audience }], { fetch: async (_, init) => { signal = init.signal; return new Promise(() => {}); } });
    const began = performance.now(); expect(await p.verify("external", await token(), now)).toEqual({ kind: "unavailable" });
    expect(performance.now() - began).toBeLessThan(3000); expect(signal!.aborted).toBe(true);
  });
  test("structured mutation fuzz rejects 1024 corruptions of a real signed JWT", async () => {
    const p = provider(); const valid = await token({ ...claims(), profile: { name: "private", roles: ["admin"] } });
    expect((await p.adapter.verify("external", valid, now)).kind).toBe("valid");
    let seed = 0x76543210;
    for (let n = 0; n < 1024; n++) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const position = seed % valid.length;
      const changed = valid.slice(0, position) + (valid[position] === "A" ? "B" : "A") + valid.slice(position + 1);
      expect(await p.adapter.verify("external", changed, now)).toEqual({ kind: "invalid" });
    }
  });
  test("deterministic parser fuzz corpus rejects 2048 malformed credentials without fetching", async () => {
    const p = provider(); let state = 0x12345678;
    for (let n = 0; n < 2048; n++) {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      const garbage = `${state.toString(16)}.${n.toString(36)}.!${String.fromCharCode(state & 255)}`;
      expect(await p.adapter.verify("external", garbage, now)).toEqual({ kind: "invalid" });
    }
    expect(p.urls).toHaveLength(0);
  });
});
