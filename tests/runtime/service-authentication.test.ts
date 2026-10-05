import { afterAll, beforeEach, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// AUTH-001 accepted §2 / AUTH-P5: service ownership, one-time verifier-only
// issuance, direct immediate authority, bounded exchange, rotation and revocation.
const root = mkdtempSync(join(tmpdir(), "jadpo-service-auth-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
const source = readFileSync(new URL("./fixtures/service-auth/app.jadpo", import.meta.url), "utf8");
// Policy-composition pressure fixture, not the golden reminder worker/grant.
writeFileSync(join(root, "app.jadpo"), `${source}
enum ScopedNoteRole { owner }
enum ReminderRole { worker }
entity ReminderMembership {
    id: Uuid identity worker_id: Worker.id role: ReminderRole
    membership { scope: application member: worker_id role: role }
}
input ScopedLookup { id: ScopedNote.id }
input ScopedChange { id: ScopedNote.id title: ScopedNote.title sent: ScopedNote.sent }
output ScopedView { id: ScopedNote.id title: ScopedNote.title }
output ScopedReceipt { id: ScopedNote.id sent: ScopedNote.sent }
failure ScopedMissing { kind: NotFound code: "scoped_missing" }
failure ScopedConflict { kind: Conflict code: "scoped_conflict" }
entity ScopedNote {
    id: Uuid identity
    owner_id: Operator.id { role: ScopedNoteRole.owner immutable: true }
    title: Text { policy { ScopedNoteRole.owner: [read, update] } }
    sent: Text? { policy { ScopedNoteRole.owner: [read, update] ReminderRole.worker: [read, update] } }
    hidden: Text? { policy {} }
    readonly: Text { policy { ScopedNoteRole.owner: [read] } }
    policy {
        ScopedNoteRole.owner: [read, update]
        operations {
            complete { ReminderRole.worker: [update] }
            bad_complete { ReminderRole.worker: [update] }
            peek { ReminderRole.worker: [read] }
        }
    }
    query summary(input: ScopedLookup) freshness: authoritative fails ScopedMissing -> ScopedView {
        var note = attempt query required ScopedNote { where: id == input.id missing: ScopedMissing }
        return ScopedView { id: note.id title: note.title }
    }
    query peek(input: ScopedLookup) freshness: authoritative fails ScopedMissing -> ScopedReceipt {
        var note = attempt query required ScopedNote { where: id == input.id missing: ScopedMissing }
        return ScopedReceipt { id: note.id sent: note.sent }
    }
    action complete(input: ScopedChange) fails ScopedMissing, ScopedConflict -> Unit {
        var note = attempt update required ScopedNote { where: id == input.id set: { sent: input.sent } missing: ScopedMissing conflict: ScopedConflict }
    }
    action bad_complete(input: ScopedChange) fails ScopedMissing, ScopedConflict -> Unit {
        var note = attempt update required ScopedNote { where: id == input.id set: { sent: input.sent title: input.title } missing: ScopedMissing conflict: ScopedConflict }
    }
    action rename(input: ScopedChange) fails ScopedMissing, ScopedConflict -> Unit {
        var note = attempt update required ScopedNote { where: id == input.id set: { title: input.title } missing: ScopedMissing conflict: ScopedConflict }
    }
    action write_hidden(input: ScopedChange) fails ScopedMissing, ScopedConflict -> Unit {
        var note = attempt update required ScopedNote { where: id == input.id set: { hidden: none } missing: ScopedMissing conflict: ScopedConflict }
    }
    action write_readonly(input: ScopedChange) fails ScopedMissing, ScopedConflict -> Unit {
        var note = attempt update required ScopedNote { where: id == input.id set: { readonly: ScopedNote.readonly("Forbidden") } missing: ScopedMissing conflict: ScopedConflict }
    }
}
route POST /scoped/read { input: ScopedLookup output: ScopedView run: ScopedNote.summary(input) }
route POST /scoped/peek { auth: fresh input: ScopedLookup output: ScopedReceipt run: ScopedNote.peek(input) }
route POST /scoped/complete { auth: fresh input: ScopedChange run: ScopedNote.complete(input) success: no_content }
route POST /scoped/bad-complete { auth: fresh input: ScopedChange run: ScopedNote.bad_complete(input) success: no_content }
route POST /scoped/rename { input: ScopedChange run: ScopedNote.rename(input) success: no_content }
route POST /scoped/hidden { input: ScopedChange run: ScopedNote.write_hidden(input) success: no_content }
route POST /scoped/readonly { input: ScopedChange run: ScopedNote.write_readonly(input) success: no_content }
`);
const build = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Service fixture failed to build:\n${build.stdout}\n${build.stderr}`);
const postgresUrl = Bun.env.JADPO_SERVICE_AUTH_DATABASE_URL;
if (postgresUrl) {
  const url = new URL(postgresUrl);
  if (!["localhost", "127.0.0.1"].includes(url.hostname) || !/^\/jadpo_auth_test_[a-z0-9_]+$/u.test(url.pathname)) throw new Error("Requires a disposable local authentication database");
  Bun.env.DATABASE_URL = postgresUrl;
} else { delete Bun.env.DATABASE_URL; }
Bun.env.SQLITE_PATH = join(root, "service.sqlite");
const app = await import(pathToFileURL(join(root, "build/target/app.ts")).href);
const persistence = await import(pathToFileURL(join(root, "build/target/persistence.ts")).href);
const sqlite = postgresUrl ? null : new Database(Bun.env.SQLITE_PATH, { strict: true });
const postgres = postgresUrl ? new SQL({ url: postgresUrl, prepare: false }) : null;
const db = {
  async exec(sql: string) { if (postgres) await postgres.unsafe(sql); else sqlite!.exec(sql); },
  async all(sql: string, ...values: any[]): Promise<any[]> {
    let index = 0; const pg = sql.replace(/\?/gu, () => `$${++index}`);
    return postgres ? [...await postgres.unsafe(pg, values)] : sqlite!.prepare(sql).all(...values);
  },
};
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: app.handleRequest });
afterAll(async () => { server.stop(true); if (postgres) await postgres.close(); else sqlite!.close(); delete Bun.env.SQLITE_PATH; });
const operator = "00000000-0000-4000-8000-000000000001";
const worker = "00000000-0000-4000-8000-000000000002";
const otherWorker = "00000000-0000-4000-8000-000000000003";
const asset = "00000000-0000-4000-8000-000000000004";
const otherAsset = "00000000-0000-4000-8000-000000000005";
const replacement = "00000000-0000-4000-8000-000000000006";
const currentKey = Buffer.alloc(32, 71).toString("base64url");
const previousKey = Buffer.alloc(32, 72).toString("base64url");
const nextKey = Buffer.alloc(32, 73).toString("base64url");
const environment = { AUTH_SIGNING_KEY: currentKey, AUTH_PREVIOUS_SIGNING_KEY: previousKey, BROWSER_ORIGIN: "https://service.test" };
let now: number;
beforeEach(async () => {
  await db.exec('DELETE FROM "__jadpo_auth_service_credentials"; DELETE FROM "__jadpo_auth_sessions"; DELETE FROM reminder_membership; DELETE FROM scoped_note; DELETE FROM asset; DELETE FROM worker; DELETE FROM operator;');
  await db.all('INSERT INTO operator (id, authentication_subject, enabled) VALUES (?, ?, TRUE)', operator, "operator");
  for (const [id, subject] of [[worker, "worker"], [otherWorker, "other"]]) await db.all('INSERT INTO worker (id, authentication_subject, responsible_operator, enabled) VALUES (?, ?, ?, TRUE)', id, subject, operator);
  for (const [id, owner] of [[asset, worker], [otherAsset, otherWorker]]) await db.all('INSERT INTO asset (id, worker_id, title) VALUES (?, ?, ?)', id, owner, "Private");
  await db.all('INSERT INTO reminder_membership (id, worker_id, role) VALUES (?, ?, ?)', replacement, worker, "worker");
  await db.all('INSERT INTO scoped_note (id, owner_id, title, sent, hidden, readonly) VALUES (?, ?, ?, NULL, NULL, ?)', asset, operator, "Original", "Read only");
  await app.initializeApplication(environment); now = Date.now();
});
const host = () => app.authenticationHost();
const issue = (subject = "worker", expires = now + 3_600_000) => host().issueServiceCredential("api_bearer", subject, expires, now);
const exchange = (credential: string, at = now) => host().exchangeServiceCredential("api_bearer", credential, at);
async function request(credential?: string, path = "/identity", body?: unknown, headers: Record<string, string> = {}) {
  return fetch(new URL(path, server.url), { method: body === undefined ? "GET" : "POST", headers: { ...(credential ? { authorization: `Bearer ${credential}` } : {}), ...headers }, body: body === undefined ? undefined : JSON.stringify(body) });
}
async function accepted(credential: string, id = worker, path = "/identity") {
  const response = await request(credential, path); expect(response.status).toBe(200); expect(await response.json()).toEqual({ id });
}
async function rejected(credential: string, code = "invalid_credentials", path = "/identity", expectedStatus = 401) {
  const response = await request(credential, path); expect(response.status).toBe(expectedStatus);
  const body = await response.json(); expect(body.error.code).toBe(code);
  const serialized = JSON.stringify(body); expect(serialized).not.toContain(credential); expect(serialized).not.toContain("stack");
}
function rawRequest(credential: string) { return new Request("https://service.test/identity", { headers: { authorization: `Bearer ${credential}` } }); }
async function resign(token: string, mutate: (payload: any[]) => void) {
  const parts = token.split("."); const payload = JSON.parse(Buffer.from(parts[2], "base64url").toString()); mutate(payload);
  parts[2] = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const key = await crypto.subtle.importKey("raw", Buffer.from(currentKey, "base64url"), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  parts[3] = Buffer.from(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(parts.slice(0, 3).join(".")))).toString("base64url");
  return parts.join(".");
}

test("service issuance reveals a random secret once and persists only an identified verifier", async () => {
  const first = await issue(); const second = await issue();
  expect(first.credential).toMatch(/^jdk1\.[a-f0-9-]+\.[A-Za-z0-9_-]{43}$/u);
  expect(first.credential).not.toBe(second.credential); expect(first.credentialId).not.toBe(second.credentialId);
  expect(first.serviceId).toBe(worker); await accepted(first.credential); await accepted(second.credential);
  await accepted(first.credential, worker, "/service-identity");
  const userCredential = await host().issue("api_bearer", "operator", now + 3_600_000, now);
  const wrongPrincipal = await request(userCredential.credential, "/service-identity");
  expect(wrongPrincipal.status).toBe(403);
  expect((await wrongPrincipal.json()).error.code).toBe("not_permitted");
  const rows = await db.all('SELECT * FROM "__jadpo_auth_service_credentials"'); expect(rows).toHaveLength(2);
  const text = JSON.stringify(rows); expect(text).not.toContain(first.credential); expect(text).not.toContain(second.credential); expect(text).not.toContain(currentKey);
  const stored = JSON.parse(rows.find(row => row.id === first.credentialId).data);
  expect(stored.serviceId).toBe(worker); expect(stored.subject).toBe("worker"); expect(stored.verifier).toMatch(/^[A-Za-z0-9_-]{43}$/u);
  expect(Object.keys(stored).sort()).toEqual(["expires", "id", "keyId", "revoked", "serviceId", "strategy", "subject", "verifier"]);
  expect(host()).not.toHaveProperty("revealServiceCredential");
});
test("scoped policy composition permits only named service operations and preserves owner writes", async () => {
  const key = await issue();
  const user = await host().issue("api_bearer", "operator", now + 60000, now);
  const change = { id: asset, title: "Changed", sent: "Observed" };
  expect((await request(key.credential, "/scoped/read", { id: asset })).status).toBe(404);
  expect((await request(key.credential, "/scoped/rename", change)).status).toBe(404);
  expect((await request(key.credential, "/scoped/complete", change)).status).toBe(204);
  const peek = await request(key.credential, "/scoped/peek", { id: asset });
  expect(peek.status).toBe(200); expect(await peek.json()).toEqual({ id: asset, sent: "Observed" });
  expect((await request(key.credential, "/scoped/bad-complete", { ...change, sent: "Must roll back" })).status).toBe(404);
  expect(await db.all('SELECT title, sent FROM scoped_note WHERE id = ?', asset)).toEqual([{ title: "Original", sent: "Observed" }]);
  expect((await request(user.credential, "/scoped/complete", change)).status).toBe(404);
  expect((await request(user.credential, "/scoped/rename", change)).status).toBe(204);
  const read = await request(user.credential, "/scoped/read", { id: asset });
  expect(read.status).toBe(200); expect(await read.json()).toEqual({ id: asset, title: "Changed" });
});
test("scoped policy composition treats empty and read-only field policies as explicit write denial", async () => {
  const user = await host().issue("api_bearer", "operator", now + 60000, now);
  const before = await db.all('SELECT * FROM scoped_note WHERE id = ?', asset);
  for (const path of ["/scoped/hidden", "/scoped/readonly"]) {
    const denied = await request(user.credential, path, { id: asset, title: "Forbidden", sent: "Forbidden" });
    expect(denied.status).toBe(404); expect((await denied.json()).error.code).toBe("scoped_missing");
    expect(await db.all('SELECT * FROM scoped_note WHERE id = ?', asset)).toEqual(before);
  }
});
test("scoped policy composition rereads service membership and does not lend its role to a user UUID", async () => {
  const key = await issue(); const other = await issue("other");
  const change = { id: asset, title: "Forbidden", sent: "Observed" };
  expect((await request(other.credential, "/scoped/complete", change)).status).toBe(404);
  // A user with the same identifier still lacks the service membership.
  await db.all('INSERT INTO operator (id, authentication_subject, enabled) VALUES (?, ?, TRUE)', worker, "same-uuid-user");
  const sameUuid = await host().issue("api_bearer", "same-uuid-user", now + 60000, now);
  expect((await request(sameUuid.credential, "/scoped/complete", change)).status).toBe(404);
  expect((await request(key.credential, "/scoped/complete", change)).status).toBe(204);
  await db.all('DELETE FROM reminder_membership WHERE worker_id = ?', worker);
  expect((await request(key.credential, "/scoped/complete", { ...change, sent: "After revocation" })).status).toBe(404);
  expect((await request(key.credential, "/scoped/peek", { id: asset })).status).toBe(404);
  expect(await db.all('SELECT sent FROM scoped_note WHERE id = ?', asset)).toEqual([{ sent: "Observed" }]);
});
test("direct and exchanged services share the existing policy boundary without acquiring user identity", async () => {
  const key = await issue(); const bearer = await exchange(key.credential);
  for (const credential of [key.credential, bearer.credential]) {
    await accepted(credential);
    const principal = await host().authenticate(rawRequest(credential), false, now);
    expect(principal.kind).toBe("service"); expect(principal.values).toEqual({ service_id: worker });
    expect(principal.authenticationStrength).toBe("api_key");
    const fresh = await host().authenticate(rawRequest(credential), true, now);
    expect(fresh.authenticationStrength).toBe("api_key");
    const own = await request(credential, "/assets", { id: asset }); expect(own.status).toBe(200);
    const denied = await request(credential, "/assets", { id: otherAsset }); expect(denied.status).toBe(404); expect((await denied.json()).error.code).toBe("asset_missing");
  }
  const refreshed = await host().refresh("api_bearer", bearer.credential, now + 1);
  expect((await host().authenticate(rawRequest(refreshed.credential), true, now + 1)).authenticationStrength).toBe("api_key");
  const user = await host().issue("api_bearer", "operator", now + 60_000, now); await accepted(user.credential, operator);
});
test("service credentials overlap for rotation and individual revocation does not revoke the replacement", async () => {
  const first = await issue(); const second = await issue(); const token = await exchange(first.credential);
  const audit = await host().revokeServiceCredential("api_bearer", first.credentialId, now);
  expect(audit).toEqual({ credentialId: first.credentialId, serviceId: worker });
  await rejected(first.credential); await accepted(second.credential);
  await accepted(token.credential); await rejected(token.credential, "invalid_credentials", "/fresh");
  await expect(exchange(first.credential)).rejects.toMatchObject({ code: "invalid_credentials" });
  await expect(host().refresh("api_bearer", token.credential, now)).rejects.toMatchObject({ code: "invalid_credentials" });
});
test("service disable reaches the declared inactive failure on direct, exchange, refresh and fresh paths", async () => {
  const key = await issue(); const token = await exchange(key.credential);
  await db.all('UPDATE worker SET enabled = FALSE WHERE id = ?', worker);
  await rejected(key.credential, "service_disabled", "/identity", 422);
  await rejected(token.credential, "service_disabled", "/fresh", 422); await accepted(token.credential);
  await expect(exchange(key.credential)).rejects.toMatchObject({ code: "principal_inactive", declaredFailure: "ServiceDisabled" });
  await expect(host().refresh("api_bearer", token.credential, now)).rejects.toMatchObject({ code: "principal_inactive", declaredFailure: "ServiceDisabled" });
  await expect(issue()).rejects.toMatchObject({ code: "principal_inactive", declaredFailure: "ServiceDisabled" });
});
test("bounded service expiry is exclusive and cannot outlive the originating credential", async () => {
  const key = await issue("worker", now + 1000); const token = await exchange(key.credential);
  expect(token.expires).toBe(now + 1000);
  await expect(host().authenticate(rawRequest(token.credential), false, now + 999)).resolves.toMatchObject({ kind: "service" });
  for (const credential of [key.credential, token.credential]) await expect(host().authenticate(rawRequest(credential), false, now + 1000)).rejects.toMatchObject({ code: "invalid_credentials" });
  await expect(exchange(key.credential, now + 1000)).rejects.toMatchObject({ code: "invalid_credentials" });
  const long = await issue(); const bounded = await exchange(long.credential); expect(bounded.expires).toBe(now + 300_000);
});
test("ordinary bounded tokens require neither credential nor principal storage while fresh tokens fail closed", async () => {
  const key = await issue(); const token = await exchange(key.credential);
  await db.exec('ALTER TABLE "__jadpo_auth_service_credentials" RENAME TO "unavailable_credentials"');
  try { await accepted(token.credential); await rejected(token.credential, "authentication_unavailable", "/fresh", 503); await rejected(key.credential, "authentication_unavailable", "/identity", 503); }
  finally { await db.exec('ALTER TABLE "unavailable_credentials" RENAME TO "__jadpo_auth_service_credentials"'); }
  await db.exec('ALTER TABLE worker RENAME TO unavailable_worker');
  try { await accepted(token.credential); await rejected(token.credential, "authentication_unavailable", "/fresh", 503); }
  finally { await db.exec('ALTER TABLE unavailable_worker RENAME TO worker'); }
});
test("exactly-one selection rejects mixed valid and invalid credentials before service lookup", async () => {
  const key = await issue(); const user = await host().issue("browser_session", "operator", now + 60_000, now);
  const original = persistence.authenticationStorage.getServiceCredential; let reads = 0;
  persistence.authenticationStorage.getServiceCredential = async (...args: any[]) => { reads++; return original(...args); };
  try {
    for (const cookie of [user.credential, "invalid"]) {
      const response = await request(key.credential, "/identity", undefined, { cookie: `__Host-service-test=${cookie}` });
      expect(response.status).toBe(401); expect((await response.json()).error.code).toBe("ambiguous_credentials");
    }
    const duplicate = await request(`${key.credential}, Bearer ${key.credential}`); expect(duplicate.status).toBe(401); expect((await duplicate.json()).error.code).toBe("ambiguous_credentials");
    expect(reads).toBe(0);
  } finally { persistence.authenticationStorage.getServiceCredential = original; }
  const live = await request("invalid", "/live"); expect(live.status).toBe(200); expect(await live.json()).toBe("healthy");
});
test("service keys reject tampering, unknown records, invalid encoding and user/service format confusion", async () => {
  const key = await issue(); const token = await exchange(key.credential);
  for (const credential of [key.credential + "=", key.credential.replace("jdk1", "jdo1"), token.credential.replace("jdx1", "jds1"), key.credential.replace(key.credentialId, crypto.randomUUID()), `jdk1.${key.credentialId}.${"A".repeat(43)}`, "jdk1." + "a".repeat(5000), token.credential + "x"]) await rejected(credential);
  await expect(exchange(token.credential)).rejects.toMatchObject({ code: "invalid_credentials" });
});
test("correctly signed service envelopes still enforce audience, kind, lifetime and closed claims", async () => {
  const token = await exchange((await issue()).credential);
  const changes = [(p: any[]) => { p[1] = "user-api"; }, (p: any[]) => { p[8] = "user"; }, (p: any[]) => { p[9] = "administrator"; }, (p: any[]) => { p.push({ role: "owner" }); }, (p: any[]) => { p[6] = now + 60_000; }, (p: any[]) => { p[7] = now + 300_001; }];
  for (const change of changes) await rejected(await resign(token.credential, change));
  const payload = JSON.parse(Buffer.from(token.credential.split(".")[2], "base64url").toString());
  expect(payload).toEqual([1, "service-token", "api_bearer", token.credentialId, "worker", worker, now, now + 300_000, "service", "api_key"]);
});
test("service signing-key overlap permits old credentials until the old key is retired", async () => {
  const key = await issue(); const token = await exchange(key.credential);
  await app.initializeApplication({ ...environment, AUTH_SIGNING_KEY: nextKey, AUTH_PREVIOUS_SIGNING_KEY: currentKey });
  await accepted(key.credential); await accepted(token.credential); const rotated = await issue(); await accepted(rotated.credential);
  await app.initializeApplication({ ...environment, AUTH_SIGNING_KEY: nextKey, AUTH_PREVIOUS_SIGNING_KEY: previousKey });
  await rejected(key.credential); await rejected(token.credential); await accepted(rotated.credential);
});
test("subject reuse cannot transfer a service credential to a different identity", async () => {
  const key = await issue(); const token = await exchange(key.credential);
  await db.all('UPDATE worker SET authentication_subject = ? WHERE id = ?', "retired", worker);
  await db.all('INSERT INTO worker (id, authentication_subject, responsible_operator, enabled) VALUES (?, ?, ?, TRUE)', replacement, "worker", operator);
  await rejected(key.credential, "authentication_unavailable", "/identity", 503);
  await rejected(token.credential, "authentication_unavailable", "/fresh", 503);
  await expect(exchange(key.credential)).rejects.toMatchObject({ code: "authority_invariant" });
  await accepted(token.credential); // original identity only, within the declared bound
});
test("service credential state survives a fresh runtime process without revealing secrets", async () => {
  const key = await issue();
  const script = `const app = await import(${JSON.stringify(pathToFileURL(join(root, "build/target/app.ts")).href)}); await app.initializeApplication(JSON.parse(process.env.TEST_AUTH_CONFIGURATION)); const principal = await app.authenticationHost().authenticate(new Request('https://service.test/identity', { headers: {authorization:'Bearer '+process.env.TEST_SERVICE_CREDENTIAL} }), false, Number(process.env.TEST_NOW)); console.log(JSON.stringify(principal));`;
  const run = Bun.spawn([process.execPath, "--no-install", "--env-file=/dev/null", "-e", script], { env: { PATH: Bun.env.PATH, SQLITE_PATH: Bun.env.SQLITE_PATH, ...(postgresUrl ? { DATABASE_URL: postgresUrl } : {}), TEST_AUTH_CONFIGURATION: JSON.stringify(environment), TEST_SERVICE_CREDENTIAL: key.credential, TEST_NOW: String(now) }, stdout: "pipe", stderr: "pipe" });
  const output = await new Response(run.stdout).text(); const stderr = await new Response(run.stderr).text(); expect(await run.exited).toBe(0); expect(stderr).not.toContain(key.credential);
  expect(JSON.parse(output)).toMatchObject({ kind: "service", values: { service_id: worker } }); expect(output).not.toContain(key.credential);
});

test("an equal UUID in the user namespace cannot acquire a service-owned resource role", async () => {
  await db.all('INSERT INTO operator (id, authentication_subject, enabled) VALUES (?, ?, TRUE)', worker, "same-id-user");
  const user = await host().issue("api_bearer", "same-id-user", now + 60_000, now);
  await accepted(user.credential, worker);
  const denied = await request(user.credential, "/assets", { id: asset }); expect(denied.status).toBe(404);
  expect((await denied.json()).error.code).toBe("asset_missing");
  await accepted((await issue()).credential, worker);
});
test("malformed stored verifier material fails closed without public record disclosure", async () => {
  for (const field of ["keyId", "verifier"] as const) {
    const key = await issue(); const row = (await db.all('SELECT data FROM "__jadpo_auth_service_credentials" WHERE id = ?', key.credentialId))[0];
    const data = JSON.parse(row.data); data[field] = "PRIVATE_CORRUPTED_RECORD";
    await db.all('UPDATE "__jadpo_auth_service_credentials" SET data = ? WHERE id = ?', JSON.stringify(data), key.credentialId);
    const response = await request(key.credential); expect(response.status).toBe(503); const text = await response.text(); expect(text).not.toContain(data[field]); expect(text).not.toContain(key.credential);
    await expect(exchange(key.credential)).rejects.toMatchObject({ code: "authority_invariant" });
  }
});
test("service lifecycle audit emits only closed non-secret identity facts", async () => {
  const original = console.error; const events: unknown[] = [];
  console.error = (...values: unknown[]) => { for (const value of values) if (typeof value === "string") events.push(JSON.parse(value)); };
  try {
    const key = await issue(); const token = await exchange(key.credential); await host().refresh("api_bearer", token.credential, now + 1);
    await host().revokeServiceCredential("api_bearer", key.credentialId, now + 2);
    expect(events.map((event: any) => event.eventName)).toEqual(["service_credential.issued", "service_credential.exchanged", "service_credential.refreshed", "service_credential.revoked"]);
    for (const event of events as any[]) {
      expect(Object.keys(event).sort()).toEqual(["credentialId", "eventName", "kind", "occurredAt", "schemaVersion", "serviceId", "strategy"]);
      expect(event).toMatchObject({ schemaVersion: 1, kind: "authentication_audit", strategy: "api_bearer", serviceId: worker, credentialId: key.credentialId });
    }
    const evidence = JSON.stringify(events); for (const secret of [key.credential, token.credential, currentKey, previousKey]) expect(evidence).not.toContain(secret);
  } finally { console.error = original; }
});
