import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Contract: AUTH-001 §2 and POLICY-D13/D18/D21/D23, policy-plan §§5.4,
// 9.5 and 12. Real credentials enter the generated HTTP handler; assertions
// cover both public responses and committed authoritative state.
const root = mkdtempSync(join(tmpdir(), "jadpo-validation-auth-policy-"));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
const base = readFileSync(new URL("../../examples/first-party-authentication/app.jadpo", import.meta.url), "utf8");
writeFileSync(join(root, "app.jadpo"), `${base}
input PairChange { first: Note.id second: Note.id title: Note.title }
action rename_pair(input: PairChange) fails NoteMissing, NoteConflict -> NoteSummary {
    var first = attempt Note.rename(NoteChange { id: input.first title: input.title })
    return attempt Note.rename(NoteChange { id: input.second title: input.title })
}
failure ChangeRejected { kind: Rejected code: "change_rejected" }
action rename_then_reject(input: NoteChange) fails NoteMissing, NoteConflict, ChangeRejected -> NoteSummary {
    var changed = attempt Note.rename(input)
    reject ChangeRejected
}
route POST /notes/pair { input: PairChange output: NoteSummary run: rename_pair(input) }
route POST /notes/fresh-pair { auth: fresh input: PairChange output: NoteSummary run: rename_pair(input) }
route POST /notes/reject { input: NoteChange output: NoteSummary run: rename_then_reject(input) }
`);
const built = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (built.exitCode !== 0) throw new Error(`Auth/policy fixture failed to build:\n${built.stdout}\n${built.stderr}`);
// Never consume an application database supplied by the caller.
delete Bun.env.DATABASE_URL;
const postgresUrl = Bun.env.JADPO_VALIDATION_AUTH_POLICY_DATABASE_URL;
if (postgresUrl) {
  const url = new URL(postgresUrl);
  if (!["127.0.0.1", "localhost"].includes(url.hostname) || !/^\/jadpo_auth_test_[a-z0-9_]+$/u.test(url.pathname)) {
    throw new Error("Auth/policy validation requires a disposable local jadpo_auth_test_* database");
  }
  Bun.env.DATABASE_URL = postgresUrl;
}
Bun.env.SQLITE_PATH = join(root, "state.sqlite");
const app = await import(pathToFileURL(join(root, "build/target/app.ts")).href);
const sqlite = postgresUrl ? null : new Database(Bun.env.SQLITE_PATH, { strict: true });
const postgres = postgresUrl ? new SQL({ url: postgresUrl, prepare: false }) : null;
const db = {
  async exec(text: string) { if (postgres) await postgres.unsafe(text); else sqlite!.exec(text); },
  prepare(text: string) {
    let index = 0;
    const parameterized = text.replace(/\?/gu, () => `$${++index}`);
    return {
      async run(...values: any[]) { if (postgres) await postgres.unsafe(parameterized, values); else sqlite!.prepare(text).run(...values); },
      async all(...values: any[]): Promise<any[]> { return postgres ? [...await postgres.unsafe(parameterized, values)] : sqlite!.prepare(text).all(...values); },
      async get(...values: any[]): Promise<any> { return (await this.all(...values))[0] ?? null; },
    };
  },
  async close() { if (postgres) await postgres.close(); else sqlite!.close(); },
};
const alice = "00000000-0000-4000-8000-000000000001";
const bob = "00000000-0000-4000-8000-000000000002";
const a1 = "00000000-0000-4000-8000-000000000011";
const a2 = "00000000-0000-4000-8000-000000000012";
const b1 = "00000000-0000-4000-8000-000000000021";
const b2 = "00000000-0000-4000-8000-000000000022";
const absent = "00000000-0000-4000-8000-000000000099";
const origin = "https://auth-policy.test";
const config = { AUTH_SIGNING_KEY: Buffer.alloc(32, 43).toString("base64url"), AUTH_PREVIOUS_SIGNING_KEY: Buffer.alloc(32, 44).toString("base64url"), BROWSER_ORIGIN: origin };
beforeEach(async () => {
  await db.exec('DELETE FROM "__jadpo_auth_sessions"; DELETE FROM "note"; DELETE FROM "user";');
  const user = db.prepare('INSERT INTO "user" (id, authentication_subject, enabled) VALUES (?, ?, TRUE)');
  await user.run(alice, "alice"); await user.run(bob, "bob");
  const note = db.prepare('INSERT INTO note (id, owner_id, title) VALUES (?, ?, ?)');
  for (const [id, owner] of [[a1, alice], [a2, alice], [b1, bob], [b2, bob]]) await note.run(id, owner, "Original");
  await app.initializeApplication(config);
});
afterAll(async () => { await db.close(); delete Bun.env.SQLITE_PATH; rmSync(root, { recursive: true, force: true }); });
async function identity(subject = "alice", strategy = "api_bearer") {
  const now = Date.now();
  const issued = await app.authenticationHost().issue(strategy, subject, now + 60_000, now);
  const headers: Record<string, string> = strategy === "browser_session"
    ? { cookie: `__Host-jadpo_session=${issued.credential}`, origin, "x-jadpo-csrf": issued.csrfToken }
    : { authorization: `Bearer ${issued.credential}` };
  return { ...issued, strategy, headers };
}
function request(path: string, headers: Record<string, string>, body: unknown) {
  return new Request(`${origin}${path}`, { method: "POST", headers, body: typeof body === "string" ? body : JSON.stringify(body) });
}
function state() { return db.prepare('SELECT id, owner_id, title FROM note ORDER BY id').all(); }
async function title(id: string) { return ((await db.prepare('SELECT title FROM note WHERE id = ?').get(id)) as { title: string }).title; }
async function response(path: string, headers: Record<string, string>, body: unknown) {
  const result = await app.handleRequest(request(path, headers, body));
  return { status: result.status, body: await result.json() };
}
function publicError(body: any) { const { request_id: _requestId, ...error } = body.error; return error; }

describe("independent authentication, policy and transaction integration", () => {
  test("policy denial in a later nested mutation rolls back the earlier authorized write", async () => {
    for (const strategy of ["api_bearer", "browser_session"]) {
      const auth = await identity("alice", strategy);
      const before = await state();
      const denied = await response("/notes/pair", auth.headers, { first: a1, second: b1, title: "Must roll back" });
      expect(denied.status).toBe(404);
      expect(denied.body.error.code).toBe("note_missing");
      expect(await state()).toEqual(before);
    }
  });
  test("authorized nested writes commit together while domain rejection rolls them back", async () => {
    const auth = await identity();
    const success = await response("/notes/pair", auth.headers, { first: a1, second: a2, title: "Committed" });
    expect(success.status).toBe(200);
    expect(await title(a1)).toBe("Committed"); expect(await title(a2)).toBe("Committed");
    const before = await state();
    const rejected = await response("/notes/reject", auth.headers, { id: a1, title: "Must roll back" });
    expect(rejected.status).toBe(422); expect(rejected.body.error.code).toBe("change_rejected");
    expect(await state()).toEqual(before);
  });
  test("missing and unauthorized second rows have the same safe public failure and rollback", async () => {
    const auth = await identity();
    const before = await state();
    const absentResult = await response("/notes/pair", auth.headers, { first: a1, second: absent, title: "Changed" });
    const deniedResult = await response("/notes/pair", auth.headers, { first: a1, second: b1, title: "Changed" });
    expect(absentResult.status).toBe(404); expect(deniedResult.status).toBe(404);
    expect(publicError(absentResult.body)).toEqual(publicError(deniedResult.body));
    expect(await state()).toEqual(before);
  });
  test("bounded signed identity never freezes direct ownership authority", async () => {
    const oldOwner = await identity("alice", "browser_session");
    const newOwner = await identity("bob", "browser_session");
    await db.prepare('UPDATE note SET owner_id = ? WHERE id = ?').run(bob, a1);
    const denied = await response("/notes/rename", oldOwner.headers, { id: a1, title: "Stale owner" });
    expect(denied.status).toBe(404); expect(await title(a1)).toBe("Original");
    const allowed = await response("/notes/rename", newOwner.headers, { id: a1, title: "Current owner" });
    expect(allowed.status).toBe(200); expect(await title(a1)).toBe("Current owner");
  });
  test("same-principal and conflicting credentials reject before malformed body or writes", async () => {
    const browser = await identity("alice", "browser_session");
    const same = await identity("alice"); const other = await identity("bob");
    const before = await state();
    for (const bearer of [same, other]) {
      const mixed = await response("/notes/pair", { ...browser.headers, ...bearer.headers }, "{ malformed");
      expect(mixed.status).toBe(401); expect(mixed.body.error.code).toBe("ambiguous_credentials");
      expect(await state()).toEqual(before);
    }
  });
  test("revoked opaque credentials and fresh signed credentials cannot begin mutation", async () => {
    const opaque = await identity(); const signed = await identity("alice", "browser_session");
    for (const auth of [opaque, signed]) await app.authenticationHost().revoke(auth.strategy, auth.credential, Date.now());
    const before = await state();
    for (const [path, auth] of [["/notes/pair", opaque], ["/notes/fresh-pair", signed]] as const) {
      const denied = await response(path, auth.headers, { first: a1, second: a2, title: "Revoked" });
      expect(denied.status).toBe(401); expect(denied.body.error.code).toBe("invalid_credentials");
      expect(await state()).toEqual(before);
    }
  });
  test("concurrent transactions keep each principal and all nested policy checks isolated", async () => {
    const users = [await identity("alice"), await identity("bob")];
    const results = await Promise.all(Array.from({ length: 24 }, (_, index) => {
      const user = index % 2; const attack = index % 3 === 0;
      const own = user === 0 ? [a1, a2] : [b1, b2];
      const foreign = user === 0 ? b1 : a1;
      return response("/notes/pair", users[user].headers, { first: own[0], second: attack ? foreign : own[1], title: attack ? "Unauthorized" : `Owner ${user}` }).then(result => ({ ...result, user, attack }));
    }));
    for (const result of results) expect(result.status).toBe(result.attack ? 404 : 200);
    expect(await title(a1)).toBe("Owner 0"); expect(await title(a2)).toBe("Owner 0");
    expect(await title(b1)).toBe("Owner 1"); expect(await title(b2)).toBe("Owner 1");
  });
});
