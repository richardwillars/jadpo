import { afterAll, beforeEach, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Real JOSE crypto, generated application, real HTTP and native storage. Only
// the external provider's HTTPS document fetch is replaced by this test host.
const root = mkdtempSync(join(tmpdir(), "jadpo-jwt-integration-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
writeFileSync(join(root, "app.jadpo"), readFileSync(new URL("./fixtures/jwt-auth/app.jadpo", import.meta.url)));
const build = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`JWT fixture failed its checked build:\n${build.stdout}\n${build.stderr}`);
const target = join(root, "build/target");
const dependency = resolve(Bun.env.JADPO_JWT_DEPENDENCY_DIR ?? "build/validation/jwt-dependencies");
const canonical = new URL("../../jadpo/crates/core/src/runtime/jwt/", import.meta.url);
const pin = JSON.parse(readFileSync(new URL("dependency.json", canonical), "utf8"));
if (!existsSync(join(dependency, "node_modules/jose/package.json"))) {
  throw new Error("Explicitly install the pinned dependency before this suite: python3 tools/install-jwt-dependency.py");
}
for (const name of ["package.json", "bun.lock"]) {
  const expected = readFileSync(new URL(name, canonical), "utf8");
  if (readFileSync(join(target, name), "utf8") !== expected || readFileSync(join(dependency, name), "utf8") !== expected) {
    throw new Error(`Generated/cached JWT ${name} differs from compiler-owned declaration`);
  }
}
const installed = JSON.parse(readFileSync(join(dependency, "node_modules/jose/package.json"), "utf8"));
if (installed.version !== pin.version || ["dependencies", "optionalDependencies", "peerDependencies"].some(name => Object.keys(installed[name] ?? {}).length)) {
  throw new Error("JWT cache must contain the exact pinned jose with zero transitive dependencies");
}
// Explicit installation is a separate gate. Reuse only that verified cache;
// never install packages or edit generated source from inside a test run.
symlinkSync(join(dependency, "node_modules"), join(target, "node_modules"), "dir");
const { SignJWT, generateKeyPair, exportJWK } = await import(pathToFileURL(join(dependency, "node_modules/jose/dist/webapi/index.js")).href);
const keys = await generateKeyPair("RS256", { modulusLength: 2048, extractable: true });
const publicKey = { ...await exportJWK(keys.publicKey), kid: "integration-rsa", alg: "RS256", use: "sig" };
const issuer = "https://jwt-provider.example.test";
const jwksUri = `${issuer}/jwks`;
const audience = "jadpo-jwt-integration";
const originalFetch = globalThis.fetch;
let providerCalls = 0;
let providerUnavailable = false;
globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
  const url = input instanceof Request ? input.url : String(input);
  if (url !== jwksUri) throw new Error("Unexpected provider fetch in JWT integration test");
  providerCalls++;
  if (init?.redirect !== "error" || init?.credentials !== "omit") throw new Error("Provider fetch lost its closed request policy");
  if (providerUnavailable) throw new Error("PROVIDER_PRIVATE_SENTINEL");
  return Response.json({ keys: [publicKey] });
}) as typeof fetch;
afterAll(() => { globalThis.fetch = originalFetch; });

const oldDatabase = Bun.env.DATABASE_URL;
const oldSqlite = Bun.env.SQLITE_PATH;
const postgresUrl = Bun.env.JADPO_JWT_AUTH_DATABASE_URL;
if (postgresUrl) {
  const url = new URL(postgresUrl);
  if (!["127.0.0.1", "localhost"].includes(url.hostname) || !/^\/jadpo_auth_test_[a-z0-9_]+$/u.test(url.pathname)) {
    throw new Error("JWT integration requires a disposable local jadpo_auth_test_* database");
  }
  Bun.env.DATABASE_URL = postgresUrl;
} else { delete Bun.env.DATABASE_URL; }
Bun.env.SQLITE_PATH = join(root, "jwt.sqlite");
afterAll(() => {
  if (oldDatabase === undefined) delete Bun.env.DATABASE_URL; else Bun.env.DATABASE_URL = oldDatabase;
  if (oldSqlite === undefined) delete Bun.env.SQLITE_PATH; else Bun.env.SQLITE_PATH = oldSqlite;
});
const app = await import(pathToFileURL(join(target, "app.ts")).href);
const sqlite = postgresUrl ? null : new Database(Bun.env.SQLITE_PATH, { strict: true });
const postgres = postgresUrl ? new SQL({ url: postgresUrl, prepare: false }) : null;
const db = {
  async exec(sql: string) { if (postgres) await postgres.unsafe(sql); else sqlite!.exec(sql); },
  async all(sql: string, ...values: any[]): Promise<any[]> {
    let index = 0;
    return postgres ? [...await postgres.unsafe(sql.replace(/\?/gu, () => `$${++index}`), values)] : sqlite!.prepare(sql).all(...values);
  },
};
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: app.handleRequest });
afterAll(async () => { server.stop(true); if (postgres) await postgres.close(); else sqlite!.close(); });
const alice = "00000000-0000-4000-8000-000000000011";
const bob = "00000000-0000-4000-8000-000000000012";
const aliceNote = "00000000-0000-4000-8000-000000000013";
const bobNote = "00000000-0000-4000-8000-000000000014";
const signingKey = Buffer.alloc(32, 91).toString("base64url");
const environment = {
  AUTH_SIGNING_KEY: signingKey, BROWSER_ORIGIN: "https://jwt-app.example.test",
  JWT_ISSUER: issuer, JWT_AUDIENCE: audience, JWT_JWKS_URI: jwksUri,
};
let now: number;
beforeEach(async () => {
  providerCalls = 0; providerUnavailable = false;
  await db.exec('DELETE FROM "__jadpo_auth_service_credentials"; DELETE FROM "__jadpo_auth_sessions"; DELETE FROM "jwt_note"; DELETE FROM "jwt_user";');
  for (const [id, subject] of [[alice, "alice"], [bob, "bob"]]) {
    await db.all('INSERT INTO "jwt_user" (id, authentication_subject, enabled, private_profile) VALUES (?, ?, TRUE, ?)', id, subject, "AUTHORITY_PROFILE_SENTINEL");
  }
  for (const [id, owner] of [[aliceNote, alice], [bobNote, bob]]) {
    await db.all('INSERT INTO "jwt_note" (id, owner_id, title) VALUES (?, ?, ?)', id, owner, "Private note");
  }
  await app.initializeApplication(environment);
  now = Date.now();
});
async function token(changes: Record<string, unknown> = {}) {
  const seconds = Math.floor(now / 1000);
  return new SignJWT({ iss: issuer, aud: audience, sub: "alice", iat: seconds, exp: seconds + 120, ...changes })
    .setProtectedHeader({ alg: "RS256", kid: "integration-rsa", typ: "JWT" }).sign(keys.privateKey);
}
async function request(credential?: string, path = "/identity", body?: unknown, extra: Record<string, string> = {}) {
  return originalFetch(new URL(path, server.url), {
    method: body === undefined ? "GET" : "POST",
    headers: { ...(credential === undefined ? {} : { authorization: `Bearer ${credential}` }), ...extra },
    body: body === undefined ? undefined : typeof body === "string" ? body : JSON.stringify(body),
  });
}
async function accepted(credential: string, id = alice, path = "/identity") {
  const response = await request(credential, path);
  expect(response.status).toBe(200); expect(await response.json()).toEqual({ id });
}
async function rejected(credential: string | undefined, code = "invalid_credentials", path = "/identity", status = 401) {
  const response = await request(credential, path);
  expect(response.status).toBe(status);
  const text = await response.text();
  expect(JSON.parse(text).error.code).toBe(code);
  if (credential) expect(text).not.toContain(credential);
  for (const secret of [signingKey, "AUTHORITY_PROFILE_SENTINEL", "TOKEN_PROFILE_SENTINEL", "PROVIDER_PRIVATE_SENTINEL"]) expect(text).not.toContain(secret);
  expect(text).not.toContain("stack");
}

test("real RS256 JWT resolves an authoritative principal on protected and fresh HTTP routes", async () => {
  const credential = await token();
  await accepted(credential); await accepted(credential, alice, "/fresh");
  expect(providerCalls).toBe(1);
});
test("missing authentication precedes malformed protected input", async () => {
  await rejected(undefined, "authentication_required");
  const response = await request(undefined, "/identity", "{broken");
  expect(response.status).toBe(401); expect((await response.json()).error.code).toBe("authentication_required");
  expect(providerCalls).toBe(0);
});
test("wrong issuer, audience, unknown subject and expired JWT cannot enter the route", async () => {
  for (const changes of [{ iss: "https://wrong.example.test" }, { aud: "wrong-audience" }, { sub: "unknown" }, { exp: Math.floor(now / 1000) - 60, iat: Math.floor(now / 1000) - 120 }]) {
    await rejected(await token(changes));
  }
});
test("signature corruption is rejected by real verification", async () => {
  const parts = (await token()).split(".");
  const signature = Buffer.from(parts[2], "base64url"); signature[0] ^= 1;
  parts[2] = signature.toString("base64url");
  await rejected(parts.join("."));
});
test("disabled authority is checked on ordinary and fresh external JWT requests", async () => {
  const credential = await token(); await accepted(credential);
  await db.all('UPDATE "jwt_user" SET enabled = FALSE WHERE id = ?', alice);
  await rejected(credential, "user_disabled", "/identity", 403);
  await rejected(credential, "user_disabled", "/fresh", 403);
});
test("JWT, opaque bearer and browser cookie preserve dispatch without validator fallback", async () => {
  const opaque = await app.authenticationHost().issue("api_bearer", "bob", now + 60_000, now);
  const browser = await app.authenticationHost().issue("browser_session", "bob", now + 60_000, now);
  await accepted(opaque.credential, bob);
  const cookie = await request(undefined, "/identity", undefined, { cookie: `__Host-jwt-test=${browser.credential}` });
  expect(cookie.status).toBe(200); expect(await cookie.json()).toEqual({ id: bob });
  expect(providerCalls).toBe(0);
  await accepted(await token());
  const jwtInCookie = await request(undefined, "/identity", undefined, { cookie: `__Host-jwt-test=${await token()}` });
  expect(jwtInCookie.status).toBe(401); expect((await jwtInCookie.json()).error.code).toBe("invalid_credentials");
  await rejected("jdo1.invalid.invalid");
  expect(providerCalls).toBe(1);
});
test("mixed, duplicate and invalid-beside-valid credential slots reject before provider work", async () => {
  const credential = await token();
  for (const extra of [
    { cookie: "__Host-jwt-test=invalid" },
    { authorization: `Bearer ${credential}, Bearer invalid` },
    { cookie: "__Host-jwt-test=invalid; __Host-jwt-test=also-invalid" },
  ]) {
    const response = await request(credential, "/identity", undefined, extra);
    expect(response.status).toBe(401); expect((await response.json()).error.code).toBe("ambiguous_credentials");
  }
  expect(providerCalls).toBe(0);
});
test("public liveness ignores malformed and conflicting credentials", async () => {
  providerUnavailable = true;
  const response = await request("not.a.jwt", "/live", undefined, { cookie: "__Host-jwt-test=invalid" });
  expect(response.status).toBe(200); expect(await response.json()).toBe("healthy");
  expect(providerCalls).toBe(0);
});
test("JWT profile, identity and permission claims cannot replace local identity or policy", async () => {
  const credential = await token({ email: "TOKEN_PROFILE_SENTINEL", user_id: bob, principal: { kind: "service" }, roles: ["admin"], permissions: ["all"] });
  await accepted(credential);
  const own = await request(credential, "/notes", { id: aliceNote }); expect(own.status).toBe(200);
  expect(await own.json()).toEqual({ id: aliceNote, title: "Private note" });
  const other = await request(credential, "/notes", { id: bobNote }); expect(other.status).toBe(404);
  const text = await other.text(); expect(JSON.parse(text).error.code).toBe("note_missing");
  expect(text).not.toContain("TOKEN_PROFILE_SENTINEL"); expect(text).not.toContain("AUTHORITY_PROFILE_SENTINEL");
  expect(text).not.toContain(credential);
});
test("concurrent verified subjects retain their own authoritative local identities", async () => {
  const credentials = [await token(), await token({ sub: "bob" })];
  const replies = await Promise.all(Array.from({ length: 20 }, async (_, index) => {
    const response = await request(credentials[index % 2], index % 3 ? "/identity" : "/fresh");
    expect(response.status).toBe(200); expect(await response.json()).toEqual({ id: index % 2 ? bob : alice });
  }));
  expect(replies).toHaveLength(20); expect(providerCalls).toBe(1);
});
test("provider failure is a safe operational response rather than a principal or provider disclosure", async () => {
  providerUnavailable = true;
  await rejected(await token(), "authentication_unavailable", "/identity", 503);
});
test.skipIf(Boolean(postgresUrl))("ordinary and fresh JWT each execute exactly one native SQLite authority query", async () => {
  const credential = await token();
  const originalPrepare = Database.prototype.prepare;
  let calls = 0;
  Database.prototype.prepare = function (sql: string, ...arguments_: any[]) {
    if (/^SELECT "id", "authentication_subject", "enabled", "private_profile" FROM "jwt_user" WHERE "authentication_subject" =/u.test(sql)) calls++;
    return originalPrepare.call(this, sql, ...arguments_);
  } as typeof originalPrepare;
  try {
    await accepted(credential); expect(calls).toBe(1);
    await accepted(credential, alice, "/fresh"); expect(calls).toBe(2);
  } finally { Database.prototype.prepare = originalPrepare; }
});
test("invalid JWT configuration fails initialization and leaves authentication unavailable", async () => {
  for (const changes of [{ JWT_ISSUER: "http://insecure.example.test" }, { JWT_JWKS_URI: "https://keys.example.test/jwks?secret=forbidden" }, { JWT_AUDIENCE: "" }]) {
    await expect(app.initializeApplication({ ...environment, ...changes })).rejects.toThrow();
    expect((await request(await token())).status).toBe(503);
  }
  expect(providerCalls).toBe(0);
});
test("invalid JWT configuration exits generated production startup before a listener is opened", async () => {
  const child = Bun.spawn([process.execPath, "--no-install", `--env-file=${process.platform === "win32" ? "NUL" : "/dev/null"}`, join(target, "app.ts")], {
    env: { ...environment, JWT_ISSUER: "http://insecure.example.test", SQLITE_PATH: join(root, "startup.sqlite"), ...(postgresUrl ? { DATABASE_URL: postgresUrl } : {}) },
    stdout: "pipe", stderr: "pipe",
  });
  const timeout = setTimeout(() => child.kill(), 5000);
  try {
    const [exit, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(exit).not.toBe(0);
    const output = stdout + stderr;
    expect(output).toContain("RUNTIME_STARTUP_FAILED"); expect(output).not.toContain("runtime.ready");
    expect(output).not.toContain(signingKey); expect(output).not.toContain("http://insecure.example.test");
  } finally { clearTimeout(timeout); }
});
